//! The lane drive: a saturation that adds harmonics to its track, between
//! the channel EQ and the compressor.
//!
//! A held synth bass is close to a pure tone. Speakers that cannot play its
//! fundamental leave nothing of it, and an EQ cannot raise harmonics that
//! are not there. The drive makes them: it bends a copy of the track
//! through a saturation curve and adds what the curve changed to the clean
//! track, so the clean sound stays and the new harmonics come in under it.
//!
//! - In `RenderQuality::Offline`, the curve and its straight-line reference
//!   run at 8× the mix rate through linear-phase halfband FIR oversampling
//!   (`fx::oversample::FirOversampler`, latency `71` frames) using second-order
//!   antiderivative anti-aliasing (ADAA2) in `f64`, so high-note harmonics do
//!   not fold back as aliases.
//! - In `RenderQuality::Live` with `DriveOversampling::TwoX` (default), both
//!   run at 2× through zero-lookahead polyphase IIR halfband oversampling
//!   (`fx::oversample::StereoUpsampler2x` and `StereoDownsampler2x`, the
//!   two halves of `HalfBandIir2x` for both channels) + ADAA2
//!   (`latency = 0`). With `Off`, both
//!   run at the base rate (`1×` ADAA2, `latency = 0`).
//! - `amount` drives the copy into the curve. `character` bends one half of
//!   the wave less than the other: symmetric at 0% gives odd harmonics,
//!   one-sided at 100% mostly even ones, at any drive.
//! - The copy is matched to the clean track's power before it is compared,
//!   so `amount` changes the tone, not the level. Matching smoothly relaxes
//!   toward unity below `-55 dBFS` so quiet reverb tails never pump upward.
//! - `focus` high-passes the difference between the copy and the clean
//!   track at 24 dB per octave. Below it the clean track passes unchanged;
//!   at about twice the lowest note, the fundamental stays clean and only
//!   harmonics are added. The asymmetric curve's offset goes first through a
//!   5 Hz blocker, so the result carries no DC and no thump.
//! - `mix` is how much of the difference is added: 0% is the clean track,
//!   100% with `focus` at its lowest is the driven copy.
//! - The result is matched to the power of the clean track, so turning the
//!   drive on adds harmonics without making the track louder; `output`
//!   then trims the level by hand.
//!
//! The power detectors follow both channels together, each through two
//! smoothing stages, so the gains move without ripple and the stereo image
//! stays put. The gains take a new value at every eighth frame; the
//! detectors move so slowly that these steps stay far below the signal.
//! Offline delays `dry` by `LATENCY` (`71`) integer
//! frames (compensated per track); Live uses zero `dry` delay, so `mix == 0%`
//! at `0 dB` output returns the clean track bit for bit in every mode.
use crate::fx::RenderQuality;
use crate::fx::oversample::{FirOversampler, StereoDownsampler2x, StereoUpsampler2x};
use crate::master::{
    KnobRange,
    dsp::{
        Coefficients, Delay, POWER_FLOOR, PowerDetector, StereoBiquad, StereoDcBlocker, db_to_gain,
        log_cosh, pole,
    },
    range, ratio_range,
};
use crate::playback_quality::DriveOversampling;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// The frames by which Offline Drive output follows its input (`8×` FIR).
pub(crate) const LATENCY: usize = FirOversampler::LATENCY_8X;
/// Maximum Drive lookahead across all quality modes (`0` in Live, `71` Offline).
pub(crate) const MAX_LATENCY: usize = LATENCY;

/// One track's drive settings. The drive keeps them while it is off.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Drive {
    pub on: bool,
    /// How hard the copy is driven into the curve, in dB.
    pub amount: f32,
    /// From 0% (symmetric, odd harmonics) to 100% (one-sided, even ones).
    pub character: f32,
    /// How much of the change is added, in percent.
    pub mix: f32,
    /// The corner of the high-pass on the change, in Hz.
    pub focus: f32,
    /// The level after the drive, in dB.
    pub output: f32,
}

impl Default for Drive {
    fn default() -> Self {
        let mut drive = Self {
            on: false,
            amount: 0.0,
            character: 0.0,
            mix: 0.0,
            focus: 0.0,
            output: 0.0,
        };
        for param in DriveParam::ALL {
            param.set(&mut drive, param.range().default);
        }
        drive
    }
}

/// A continuous control of the drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DriveParam {
    Amount,
    Character,
    Mix,
    Focus,
    Output,
}

impl DriveParam {
    pub const ALL: [Self; 5] = [
        Self::Amount,
        Self::Character,
        Self::Mix,
        Self::Focus,
        Self::Output,
    ];

    pub const fn range(self) -> KnobRange {
        match self {
            Self::Amount => range(0.0, 24.0, 12.0, 0.5),
            Self::Character => range(0.0, 100.0, 25.0, 1.0),
            Self::Mix => range(0.0, 100.0, 50.0, 1.0),
            Self::Focus => ratio_range(20.0, 2000.0, 100.0, 1.0),
            Self::Output => range(-12.0, 12.0, 0.0, 0.5),
        }
    }

    /// The control's name after its track's, such as "drive amount".
    pub fn name(self) -> &'static str {
        match self {
            Self::Amount => "drive amount",
            Self::Character => "drive character",
            Self::Mix => "drive mix",
            Self::Focus => "drive focus",
            Self::Output => "drive output",
        }
    }

    pub fn get(self, drive: &Drive) -> f32 {
        match self {
            Self::Amount => drive.amount,
            Self::Character => drive.character,
            Self::Mix => drive.mix,
            Self::Focus => drive.focus,
            Self::Output => drive.output,
        }
    }

    /// Set the control, held to its range.
    pub fn set(self, drive: &mut Drive, value: f32) {
        let value = self.range().hold(value);
        match self {
            Self::Amount => drive.amount = value,
            Self::Character => drive.character = value,
            Self::Mix => drive.mix = value,
            Self::Focus => drive.focus = value,
            Self::Output => drive.output = value,
        }
    }
}

impl Drive {
    pub fn validate(&self) -> Result<(), String> {
        if !DriveParam::ALL
            .into_iter()
            .all(|param| param.range().contains(param.get(self)))
        {
            return Err("A drive setting is out of range.".into());
        }
        Ok(())
    }

    /// The settings and the switch as a new track has them.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The saturation curve at drive `u`: `tanh` on the positive half, and on
/// the negative half `tanh` blended toward the straight line by
/// `character`. Both halves leave zero with slope one, so the curve is
/// smooth for every character.
#[inline]
fn curve(u: f64, character: f64) -> f64 {
    let bent = u.tanh();
    if u >= 0.0 {
        bent
    } else {
        bent + character * (u - bent)
    }
}

/// The antiderivative of `curve` that is zero at zero.
#[cfg(test)]
#[inline]
fn integral(u: f64, character: f64) -> f64 {
    let bent = log_cosh(u);
    if u >= 0.0 {
        bent
    } else {
        bent + character * (0.5 * u * u - bent)
    }
}

const PI2_24: f64 = std::f64::consts::PI * std::f64::consts::PI / 24.0;
const LUT_MAX: f64 = 12.0;
const LUT_SEGMENTS: usize = 512;
const LUT_H: f64 = LUT_MAX / LUT_SEGMENTS as f64;
const LUT_INV_H: f64 = LUT_SEGMENTS as f64 / LUT_MAX;

#[inline]
fn horner(x: f64, coeffs: &[f64]) -> f64 {
    let mut acc = 0.0;
    for &c in coeffs.iter().rev() {
        acc = acc * x + c;
    }
    acc
}

/// The second antiderivative of `tanh` (`∫_0^u ln(cosh(t)) dt`, zero at zero).
#[inline]
fn f2_tanh(u: f64) -> f64 {
    const TAYLOR_COEFFS: [f64; 7] = [
        1.0 / 6.0,
        -1.0 / 60.0,
        1.0 / 315.0,
        -17.0 / 22_680.0,
        31.0 / 155_925.0,
        -1382.0 / 24_324_300.0,
        21_844.0 / 1_277_025_750.0,
    ];
    // Li_2(-z) = sum_{k>=1} (-1)^k * z^k / k^2; unrolled table avoids loop/div.
    const LI2_COEFFS: [f64; 18] = [
        -1.0,
        1.0 / 4.0,
        -1.0 / 9.0,
        1.0 / 16.0,
        -1.0 / 25.0,
        1.0 / 36.0,
        -1.0 / 49.0,
        1.0 / 64.0,
        -1.0 / 81.0,
        1.0 / 100.0,
        -1.0 / 121.0,
        1.0 / 144.0,
        -1.0 / 169.0,
        1.0 / 196.0,
        -1.0 / 225.0,
        1.0 / 256.0,
        -1.0 / 289.0,
        1.0 / 324.0,
    ];
    let a = u.abs();
    let val = if a < 0.5 {
        let a2 = a * a;
        a * a2 * horner(a2, &TAYLOR_COEFFS)
    } else if a >= 18.5 {
        0.5 * a * a - a * std::f64::consts::LN_2 + PI2_24
    } else {
        let z = (-2.0 * a).exp();
        let terms = if z <= 0.05 { 5 } else { 18 };
        let li2 = z * horner(z, &LI2_COEFFS[..terms]);
        0.5 * a * a - a * std::f64::consts::LN_2 + PI2_24 + 0.5 * li2
    };
    if u >= 0.0 { val } else { -val }
}

static F2_TANH_LUT: LazyLock<[[f64; 4]; LUT_SEGMENTS]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let x0 = i as f64 * LUT_H;
        let x1 = (i + 1) as f64 * LUT_H;
        let y0 = f2_tanh(x0);
        let y1 = f2_tanh(x1);
        let m0 = log_cosh(x0) * LUT_H;
        let m1 = log_cosh(x1) * LUT_H;
        [
            y0,
            m0,
            3.0 * (y1 - y0) - 2.0 * m0 - m1,
            2.0 * (y0 - y1) + m0 + m1,
        ]
    })
});

/// Force initialization of Drive antiderivative LUT before playback starts.
pub(crate) fn prepare_tables() {
    LazyLock::force(&F2_TANH_LUT);
}

#[inline]
fn fast_f2_tanh(u: f64, lut: &[[f64; 4]; LUT_SEGMENTS]) -> f64 {
    let a = u.abs();
    let val = if a >= LUT_MAX {
        0.5 * a * a - a * std::f64::consts::LN_2 + PI2_24
    } else {
        let scaled = a * LUT_INV_H;
        let idx = (scaled as usize).min(LUT_SEGMENTS - 1);
        let t = scaled - idx as f64;
        let [c0, c1, c2, c3] = lut[idx];
        ((c3 * t + c2) * t + c1) * t + c0
    };
    if u >= 0.0 { val } else { -val }
}

#[inline]
fn fast_log_cosh(u: f64, lut: &[[f64; 4]; LUT_SEGMENTS]) -> f64 {
    let a = u.abs();
    if a >= LUT_MAX {
        a - std::f64::consts::LN_2
    } else {
        let scaled = a * LUT_INV_H;
        let idx = (scaled as usize).min(LUT_SEGMENTS - 1);
        let t = scaled - idx as f64;
        let [_, c1, c2, c3] = lut[idx];
        ((3.0 * c3 * t + 2.0 * c2) * t + c1) * LUT_INV_H
    }
}

/// The second antiderivative of `curve` (`∫_0^u F1(t, character) dt`, zero at zero).
#[cfg(test)]
#[inline]
fn antiderivative2(u: f64, character: f64) -> f64 {
    let bent = f2_tanh(u);
    if u >= 0.0 {
        bent
    } else {
        bent + character * ((u * u * u / 6.0) - bent)
    }
}

#[inline]
fn eval_f1<const FAST: bool>(u: f64, character: f64, lut: &[[f64; 4]; LUT_SEGMENTS]) -> f64 {
    let bent = if FAST {
        fast_log_cosh(u, lut)
    } else {
        log_cosh(u)
    };
    if u >= 0.0 {
        bent
    } else {
        bent + character * (0.5 * u * u - bent)
    }
}

#[inline]
fn eval_f2<const FAST: bool>(u: f64, character: f64, lut: &[[f64; 4]; LUT_SEGMENTS]) -> f64 {
    let bent = if FAST {
        fast_f2_tanh(u, lut)
    } else {
        f2_tanh(u)
    };
    if u >= 0.0 {
        bent
    } else {
        bent + character * ((u * u * u / 6.0) - bent)
    }
}

const ADAA2_EPS: f64 = 1e-4;

#[inline]
fn d1_quotient<const FAST: bool>(
    x0: f64,
    x1: f64,
    f2_0: f64,
    f2_1: f64,
    character: f64,
    lut: &[[f64; 4]; LUT_SEGMENTS],
) -> f64 {
    let dx = x0 - x1;
    if dx.abs() < ADAA2_EPS {
        eval_f1::<FAST>(0.5 * (x0 + x1), character, lut)
    } else {
        (f2_0 - f2_1) / dx
    }
}

/// The output of one ADAA2 step from the inputs `[x0, x1, x2]` (newest
/// first), the second antiderivative at `x1`, and the first divided
/// differences `[d1(x0, x1), d1(x1, x2)]`.
#[inline]
fn d2_quotient<const FAST: bool>(
    [x0, x1, x2]: [f64; 3],
    f2_1: f64,
    [d1_cur, d1_prev]: [f64; 2],
    character: f64,
    lut: &[[f64; 4]; LUT_SEGMENTS],
) -> f64 {
    let dx2 = x0 - x2;
    if dx2.abs() < ADAA2_EPS {
        let m = 0.5 * (x0 + x2);
        let dm = m - x1;
        if dm.abs() < ADAA2_EPS {
            curve(0.5 * (m + x1), character)
        } else {
            2.0 * (eval_f1::<FAST>(m, character, lut)
                - (eval_f2::<FAST>(m, character, lut) - f2_1) / dm)
                / dm
        }
    } else {
        2.0 * (d1_cur - d1_prev) / dx2
    }
}

/// Second-order antiderivative anti-aliasing evaluator in `f64`.
#[derive(Clone, Copy, Default)]
struct Adaa2 {
    x1: f64,
    x2: f64,
    f2_1: f64,
    d1_prev: f64,
}

impl Adaa2 {
    #[inline]
    fn step<const FAST: bool>(
        &mut self,
        x0: f64,
        character: f64,
        lut: &[[f64; 4]; LUT_SEGMENTS],
    ) -> f64 {
        let f2_0 = eval_f2::<FAST>(x0, character, lut);
        let d1_cur = d1_quotient::<FAST>(x0, self.x1, f2_0, self.f2_1, character, lut);
        let y = d2_quotient::<FAST>(
            [x0, self.x1, self.x2],
            self.f2_1,
            [d1_cur, self.d1_prev],
            character,
            lut,
        );
        self.x2 = self.x1;
        self.x1 = x0;
        self.f2_1 = f2_0;
        self.d1_prev = d1_cur;
        y
    }
}

/// `Adaa2` for both channels, one per lane. Each lane gives the same bits as
/// `Adaa2::step` on its own channel.
#[derive(Clone, Copy, Default)]
struct StereoAdaa2 {
    x1: [f64; 2],
    x2: [f64; 2],
    f2_1: [f64; 2],
    d1_prev: [f64; 2],
}

impl StereoAdaa2 {
    /// One channel through the scalar `Adaa2`.
    #[inline]
    fn step_lane<const FAST: bool>(
        &mut self,
        ch: usize,
        x0: f64,
        character: f64,
        lut: &[[f64; 4]; LUT_SEGMENTS],
    ) -> f64 {
        let mut lane = Adaa2 {
            x1: self.x1[ch],
            x2: self.x2[ch],
            f2_1: self.f2_1[ch],
            d1_prev: self.d1_prev[ch],
        };
        let y = lane.step::<FAST>(x0, character, lut);
        self.x1[ch] = lane.x1;
        self.x2[ch] = lane.x2;
        self.f2_1[ch] = lane.f2_1;
        self.d1_prev[ch] = lane.d1_prev;
        y
    }

    /// Run `xs`, both channels in time order, through the fast table in
    /// place: at most `STAGE_HIGH` samples. It makes three passes: the
    /// second antiderivatives, the first divided differences, and the
    /// second ones. No pass waits on a long chain from the sample before, so
    /// the processor overlaps the samples. Each output and the final state
    /// have the same bits as `Adaa2::step::<true>` sample by sample.
    fn run_fast(&mut self, xs: &mut [[f64; 2]], character: f64, lut: &[[f64; 4]; LUT_SEGMENTS]) {
        let m = xs.len();
        if m == 0 {
            return;
        }
        // `x[k + 2]` is sample `k`; `x[0]` and `x[1]` are the two before it.
        let mut x = [[0.0_f64; 2]; STAGE_HIGH + 2];
        x[0] = self.x2;
        x[1] = self.x1;
        x[2..m + 2].copy_from_slice(xs);
        // `f2[k + 1]` and `d1[k + 1]` belong to sample `k`; index 0 to the
        // one before it.
        let mut f2 = [[0.0_f64; 2]; STAGE_HIGH + 1];
        f2[0] = self.f2_1;
        for (f, x0) in f2[1..=m].iter_mut().zip(&x[2..m + 2]) {
            *f = [
                eval_f2::<true>(x0[0], character, lut),
                eval_f2::<true>(x0[1], character, lut),
            ];
        }
        let mut d1 = [[0.0_f64; 2]; STAGE_HIGH + 1];
        d1[0] = self.d1_prev;
        for k in 0..m {
            let (x0, x1) = (x[k + 2], x[k + 1]);
            let dx = [x0[0] - x1[0], x0[1] - x1[1]];
            // `>=` is false for NaN, which takes the scalar path, as
            // `Adaa2::step` does.
            d1[k + 1] = if dx[0].abs() >= ADAA2_EPS && dx[1].abs() >= ADAA2_EPS {
                [
                    (f2[k + 1][0] - f2[k][0]) / dx[0],
                    (f2[k + 1][1] - f2[k][1]) / dx[1],
                ]
            } else {
                std::array::from_fn(|l| {
                    d1_quotient::<true>(x0[l], x1[l], f2[k + 1][l], f2[k][l], character, lut)
                })
            };
        }
        for (k, y) in xs.iter_mut().enumerate() {
            let (x0, x1, x2) = (x[k + 2], x[k + 1], x[k]);
            let dx2 = [x0[0] - x2[0], x0[1] - x2[1]];
            *y = if dx2[0].abs() >= ADAA2_EPS && dx2[1].abs() >= ADAA2_EPS {
                [
                    2.0 * (d1[k + 1][0] - d1[k][0]) / dx2[0],
                    2.0 * (d1[k + 1][1] - d1[k][1]) / dx2[1],
                ]
            } else {
                std::array::from_fn(|l| {
                    d2_quotient::<true>(
                        [x0[l], x1[l], x2[l]],
                        f2[k][l],
                        [d1[k + 1][l], d1[k][l]],
                        character,
                        lut,
                    )
                })
            };
        }
        self.x2 = x[m];
        self.x1 = x[m + 1];
        self.f2_1 = f2[m];
        self.d1_prev = d1[m];
    }
}

/// `LinearAdaa2` for both channels, one per lane.
#[derive(Clone, Copy, Default)]
struct StereoLinearAdaa2 {
    x1: [f64; 2],
    x2: [f64; 2],
}

impl StereoLinearAdaa2 {
    #[inline]
    fn step(&mut self, x0: [f64; 2]) -> [f64; 2] {
        let y = [
            (x0[0] + self.x1[0] + self.x2[0]) / 3.0,
            (x0[1] + self.x1[1] + self.x2[1]) / 3.0,
        ];
        self.x2 = self.x1;
        self.x1 = x0;
        y
    }

    /// Run `xs`, both channels in time order, through `step` in place.
    fn run(&mut self, xs: &mut [[f64; 2]]) {
        for x in xs {
            *x = self.step(*x);
        }
    }

    #[inline]
    fn step_lane(&mut self, ch: usize, x0: f64) -> f64 {
        let mut lane = LinearAdaa2 {
            x1: self.x1[ch],
            x2: self.x2[ch],
        };
        let y = lane.step(x0);
        self.x1[ch] = lane.x1;
        self.x2[ch] = lane.x2;
        y
    }
}

/// Exact ADAA2 straight-line reference (`f(x) = x`), sharing the 1-sample
/// high-rate group delay of `Adaa2` (`(x0 + x1 + x2) / 3`).
#[derive(Clone, Copy, Default)]
struct LinearAdaa2 {
    x1: f64,
    x2: f64,
}

impl LinearAdaa2 {
    #[inline]
    fn step(&mut self, x0: f64) -> f64 {
        let y = (x0 + self.x1 + self.x2) / 3.0;
        self.x2 = self.x1;
        self.x1 = x0;
        y
    }
}

/// The time constant of each of the two stages of the detectors that match
/// the copy to the clean track, in seconds.
const MATCH_TIME: f64 = 0.015;
/// The same for the detectors that hold the result at the clean track's power.
const LEVEL_TIME: f64 = 0.15;
/// Soft power-match gate corner (`-55 dBFS` RMS^2 = `10^-5.5`), preventing
/// gain-matching multipliers from lifting quiet tails or noise floors.
const MATCH_GATE_POWER: f64 = 3.162_277_660_168_379_5e-6;
/// The corner of the blockers that take the offset out of the copy.
const BLOCKER_HZ: f64 = 5.0;
/// The quality factors of the two sections of a fourth-order Butterworth
/// high-pass.
const FOCUS_Q: [f64; 2] = [0.541_196_100_146_197, 1.306_562_964_876_376_5];
/// How far the matching gains may go, in dB: the copy's below and above the
/// drive, and the output's either way.
const MATCH_BELOW_DB: f64 = 6.0;
const MATCH_ABOVE_DB: f64 = 12.0;
const OUTPUT_MATCH_DB: f64 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DriveMode {
    Offline8x,
    Live2x,
    LiveOff,
}

/// A drive's settings, ready for the audio thread at the mix rate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Run {
    mode: DriveMode,
    gain: f64,
    inverse: f64,
    character: f64,
    mix: f64,
    output: f64,
    focus: [Coefficients; 2],
    blocker: f64,
    /// The poles of the matching and the level detectors.
    poles: [f64; 2],
    /// The copy's matching gain, from and to.
    copy: [f64; 2],
    /// The output's matching gain, from and to.
    level: [f64; 2],
}

impl Run {
    #[cfg(test)]
    pub(crate) fn new(drive: &Drive) -> Self {
        Self::with_quality(drive, DriveOversampling::TwoX, RenderQuality::Offline)
    }

    pub(crate) fn with_quality(
        drive: &Drive,
        drive_oversampling: DriveOversampling,
        render_quality: RenderQuality,
    ) -> Self {
        let mode = match (render_quality, drive_oversampling) {
            (RenderQuality::Offline, _) => DriveMode::Offline8x,
            (RenderQuality::Live, DriveOversampling::TwoX) => DriveMode::Live2x,
            (RenderQuality::Live, DriveOversampling::Off) => DriveMode::LiveOff,
        };
        let amount = f64::from(drive.amount);
        let gain = db_to_gain(amount);
        let focus = f64::from(drive.focus);
        Self {
            mode,
            gain,
            inverse: 1.0 / gain,
            character: f64::from(drive.character) / 100.0,
            mix: f64::from(drive.mix) / 100.0,
            output: db_to_gain(f64::from(drive.output)),
            focus: FOCUS_Q.map(|q| Coefficients::high_pass(focus, q)),
            blocker: pole(1.0 / (std::f64::consts::TAU * BLOCKER_HZ)),
            poles: [pole(MATCH_TIME), pole(LEVEL_TIME)],
            copy: [
                db_to_gain(-MATCH_BELOW_DB),
                db_to_gain(amount + MATCH_ABOVE_DB),
            ],
            level: [db_to_gain(-OUTPUT_MATCH_DB), db_to_gain(OUTPUT_MATCH_DB)],
        }
    }

    /// Lookahead frames compensated per track (`71` Offline, `0` Live).
    pub(crate) fn latency(&self) -> usize {
        match self.mode {
            DriveMode::Offline8x => LATENCY,
            DriveMode::Live2x | DriveMode::LiveOff => 0,
        }
    }

    #[inline]
    fn gated_ratio(num: f64, den: f64, gate_p: f64, bounds: [f64; 2]) -> f64 {
        let raw = ((num + POWER_FLOOR) / (den + POWER_FLOOR))
            .sqrt()
            .clamp(bounds[0], bounds[1]);
        let gate = gate_p / (gate_p + MATCH_GATE_POWER);
        1.0 + gate * (raw - 1.0)
    }

    #[inline]
    fn matched_gain(
        &self,
        state: &mut State,
        dry_fast_p: f64,
        copy_fast_p: f64,
        update: bool,
    ) -> f64 {
        let matched = if dry_fast_p == 0.0 && copy_fast_p == 0.0 {
            1.0
        } else if update {
            Self::gated_ratio(dry_fast_p, copy_fast_p, dry_fast_p, self.copy)
        } else {
            state.cached_matched
        };
        state.cached_matched = matched;
        matched
    }

    /// The blockers, focus, and gains for the frames of `chunk`, in three
    /// passes: the blockers, the fast and dry detectors, and the copy's gain;
    /// the focus filters; the wet detector and the output level. Each state
    /// takes the same steps in the same order as frame by frame.
    fn finish_stage(
        &self,
        state: &mut State,
        chunk: &mut [[f32; 2]],
        dry: &[[f64; 2]],
        shaped: &[[f64; 2]],
        clean_raw: &[[f64; 2]],
    ) {
        let n = chunk.len();
        let [fast, slow] = self.poles;
        let power = |pair: [f64; 2]| 0.5 * (pair[0] * pair[0] + pair[1] * pair[1]);
        let mut change = [[0.0_f64; 2]; STAGE_FRAMES];
        // The fast and the slow dry power, and whether the level updates.
        let mut gates = [([0.0_f64; 2], false); STAGE_FRAMES];
        let (change, gates) = (&mut change[..n], &mut gates[..n]);
        for (i, (c, gate)) in change.iter_mut().zip(gates.iter_mut()).enumerate() {
            let copy = state.blockers[0].process(shaped[i], self.blocker);
            let clean = state.blockers[1].process(clean_raw[i], self.blocker);
            let dry_power = power(dry[i]);
            let dry_fast_p = state.detectors[0].follow(dry_power, fast);
            let copy_fast_p = state.detectors[1].follow(power(copy), fast);
            let dry_slow_p = state.detectors[2].follow(dry_power, slow);
            let update = state.gain_phase == 0;
            let matched = self.matched_gain(state, dry_fast_p, copy_fast_p, update);
            state.gain_phase = if dry_fast_p == 0.0 && dry_slow_p == 0.0 {
                0
            } else {
                (state.gain_phase + 1) & 7
            };
            *c = [matched * copy[0] - clean[0], matched * copy[1] - clean[1]];
            *gate = ([dry_fast_p, dry_slow_p], update);
        }
        for c in change.iter_mut() {
            *c = state.focus[0].process_unflushed(*c);
        }
        for c in change.iter_mut() {
            *c = state.focus[1].process_unflushed(*c);
        }
        for (i, frame) in chunk.iter_mut().enumerate() {
            let wet = [
                dry[i][0] + self.mix * change[i][0],
                dry[i][1] + self.mix * change[i][1],
            ];
            let wet_slow_p = state.detectors[3].follow(power(wet), slow);
            let ([dry_fast_p, dry_slow_p], update) = gates[i];
            let level = if dry_fast_p == 0.0 && dry_slow_p == 0.0 {
                self.output
            } else if update {
                Self::gated_ratio(dry_slow_p, wet_slow_p, dry_fast_p, self.level) * self.output
            } else {
                state.cached_level
            };
            state.cached_level = level;
            *frame = wet.map(|sample| (sample * level) as f32);
        }
    }

    fn first_sound_offset(frames: &[[f32; 2]], state: &mut State) -> Option<usize> {
        if state.started {
            return Some(0);
        }
        let idx = frames
            .iter()
            .position(|frame| frame.iter().any(|s| s.to_bits() != 0))?;
        state.started = true;
        Some(idx)
    }

    /// Shape the playback samples in `line`, in time order, through the
    /// fast table into `curve`, then pass `line` through its straight-line
    /// reference in place.
    fn shape_live(
        &self,
        adaa: &mut StereoAdaa2,
        reference: &mut StereoLinearAdaa2,
        curve: &mut [[f64; 2]],
        line: &mut [[f64; 2]],
        lut: &[[f64; 4]; LUT_SEGMENTS],
    ) {
        for (c, u) in curve.iter_mut().zip(&*line) {
            *c = [u[0] * self.gain, u[1] * self.gain];
        }
        adaa.run_fast(curve, self.character, lut);
        for c in curve.iter_mut() {
            *c = [c[0] * self.inverse, c[1] * self.inverse];
        }
        reference.run(line);
    }

    fn eval_offline_8x(
        &self,
        frame: [f32; 2],
        offline: &mut Offline8xState,
        adaa: &mut StereoAdaa2,
        line: &mut StereoLinearAdaa2,
        lut: &[[f64; 4]; LUT_SEGMENTS],
    ) -> ([f64; 2], [f64; 2], [f64; 2]) {
        let (gain, inverse, character) = (self.gain, self.inverse, self.character);
        let (mut dry, mut shaped, mut clean_raw) = ([0.0; 2], [0.0; 2], [0.0; 2]);
        for ch in 0..2 {
            let x = f64::from(frame[ch]);
            shaped[ch] = offline.curve[ch].process(x, |u| {
                adaa.step_lane::<false>(ch, u * gain, character, lut) * inverse
            });
            clean_raw[ch] = offline.reference[ch].process(x, |u| line.step_lane(ch, u));
            dry[ch] = f64::from(offline.dry[ch].push(frame[ch]));
        }
        (dry, shaped, clean_raw)
    }

    /// Drive up to `STAGE_FRAMES` frames, one stage at a time: the path to
    /// the curve and its reference for every frame, then the blockers, focus,
    /// and gains for every frame. Each stage repeats only its own short chain
    /// of dependent steps, so the processor overlaps the frames, which one
    /// long chain per frame stops. Each state takes the same steps in the
    /// same order as frame by frame, so the result does not change.
    fn process_stage_chunk(
        &self,
        chunk: &mut [[f32; 2]],
        state: &mut State,
        lut: &[[f64; 4]; LUT_SEGMENTS],
    ) {
        let n = chunk.len();
        let mut dry = [[0.0_f64; 2]; STAGE_FRAMES];
        let mut shaped = [[0.0_f64; 2]; STAGE_FRAMES];
        let mut clean_raw = [[0.0_f64; 2]; STAGE_FRAMES];
        let (dry, shaped, clean_raw) = (&mut dry[..n], &mut shaped[..n], &mut clean_raw[..n]);
        for (d, frame) in dry.iter_mut().zip(&*chunk) {
            *d = [f64::from(frame[0]), f64::from(frame[1])];
        }
        match &mut state.path {
            PathState::Offline8x(offline) => {
                for (i, frame) in chunk.iter().enumerate() {
                    (dry[i], shaped[i], clean_raw[i]) = self.eval_offline_8x(
                        *frame,
                        offline,
                        &mut state.adaa,
                        &mut state.line,
                        lut,
                    );
                }
            }
            PathState::Live2x {
                up,
                curve_down,
                line_down,
            } => {
                let mut curve = [[0.0_f64; 2]; STAGE_HIGH];
                let mut line = [[0.0_f64; 2]; STAGE_HIGH];
                let (curve, line) = (&mut curve[..2 * n], &mut line[..2 * n]);
                for (pair, d) in line.as_chunks_mut::<2>().0.iter_mut().zip(&*dry) {
                    *pair = up.step(*d);
                }
                self.shape_live(&mut state.adaa, &mut state.line, curve, line, lut);
                let pairs = curve.as_chunks::<2>().0.iter().zip(line.as_chunks::<2>().0);
                for (i, (c, l)) in pairs.enumerate() {
                    shaped[i] = curve_down.step(*c);
                    clean_raw[i] = line_down.step(*l);
                }
            }
            PathState::LiveOff => {
                clean_raw.copy_from_slice(dry);
                self.shape_live(&mut state.adaa, &mut state.line, shaped, clean_raw, lut);
            }
        }
        self.finish_stage(state, chunk, dry, shaped, clean_raw);
    }

    fn flush_end_of_block(state: &mut State) {
        state.focus[0].flush_state();
        state.focus[1].flush_state();
        if let PathState::Live2x {
            up,
            curve_down,
            line_down,
        } = &mut state.path
        {
            up.flush_state();
            curve_down.flush_state();
            line_down.flush_state();
        }
    }

    /// Drive interleaved stereo frames in place. Offline output follows its
    /// input by `LATENCY` (`71`) frames; Live has `0` lookahead.
    pub(crate) fn process(&self, frames: &mut [f32], state: &mut State) {
        let lut = &*F2_TANH_LUT;
        let frames_slice = frames.as_chunks_mut::<2>().0;
        let Some(first_sound) = Self::first_sound_offset(frames_slice, state) else {
            return;
        };
        for chunk in frames_slice[first_sound..].chunks_mut(STAGE_FRAMES) {
            self.process_stage_chunk(chunk, state, lut);
        }
        Self::flush_end_of_block(state);
    }
}

/// The most frames that `Run::process_stage_chunk` takes at once.
const STAGE_FRAMES: usize = 32;
/// The most samples per channel of one stage at the 2x rate.
const STAGE_HIGH: usize = 2 * STAGE_FRAMES;

#[derive(Clone, Copy)]
struct Offline8xState {
    curve: [FirOversampler; 2],
    reference: [FirOversampler; 2],
    dry: [Delay<LATENCY>; 2],
}

impl Offline8xState {
    fn new() -> Self {
        Self {
            curve: [FirOversampler::new(false); 2],
            reference: [FirOversampler::new(false); 2],
            dry: [Delay::ZERO; 2],
        }
    }

    fn clear(&mut self) {
        self.curve[0].clear();
        self.curve[1].clear();
        self.reference[0].clear();
        self.reference[1].clear();
        self.dry = [Delay::ZERO; 2];
    }
}

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
enum PathState {
    Offline8x(Box<Offline8xState>),
    /// One upsampler feeds the curve and the reference; each has its own
    /// downsampler.
    Live2x {
        up: StereoUpsampler2x,
        curve_down: StereoDownsampler2x,
        line_down: StereoDownsampler2x,
    },
    LiveOff,
}

impl PathState {
    const LIVE_2X: Self = Self::Live2x {
        up: StereoUpsampler2x::ZERO,
        curve_down: StereoDownsampler2x::ZERO,
        line_down: StereoDownsampler2x::ZERO,
    };

    fn for_mode(mode: DriveMode) -> Self {
        match mode {
            DriveMode::Offline8x => Self::Offline8x(Box::new(Offline8xState::new())),
            DriveMode::Live2x => Self::LIVE_2X,
            DriveMode::LiveOff => Self::LiveOff,
        }
    }

    fn reset(&mut self, mode: DriveMode) {
        match (self, mode) {
            (Self::Offline8x(offline), DriveMode::Offline8x) => offline.clear(),
            (slot @ Self::Live2x { .. }, DriveMode::Live2x) => *slot = Self::LIVE_2X,
            (Self::LiveOff, DriveMode::LiveOff) => {}
            (slot, target) => *slot = Self::for_mode(target),
        }
    }
}

/// The drive's history: oversamplers, ADAA2 integrators, blockers, focus
/// filter, and power detectors. Allocated once before playback starts.
#[derive(Clone)]
pub(crate) struct State {
    started: bool,
    path: PathState,
    adaa: StereoAdaa2,
    line: StereoLinearAdaa2,
    /// The blockers of the copy and of the reference.
    blockers: [StereoDcBlocker; 2],
    focus: [StereoBiquad; 2],
    detectors: [PowerDetector; 4],
    gain_phase: u8,
    cached_matched: f64,
    cached_level: f64,
    /// The input frame that the next block starts with, when the history
    /// leads up to it.
    pub(crate) next: Option<u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            started: false,
            path: PathState::LiveOff,
            adaa: StereoAdaa2::default(),
            line: StereoLinearAdaa2::default(),
            blockers: [StereoDcBlocker::ZERO; 2],
            focus: [StereoBiquad::FLAT; 2],
            detectors: [PowerDetector::default(); 4],
            gain_phase: 0,
            cached_matched: 1.0,
            cached_level: 1.0,
            next: None,
        }
    }
}

impl State {
    /// A history of silence for `run`.
    pub(crate) fn new(run: &Run) -> Self {
        Self {
            path: PathState::for_mode(run.mode),
            focus: run.focus.map(StereoBiquad::new),
            cached_level: run.output,
            ..Self::default()
        }
    }

    /// Clear all internal history in place for a seek, without heap allocations.
    pub(crate) fn reset(&mut self, run: &Run) {
        self.started = false;
        self.path.reset(run.mode);
        self.adaa = StereoAdaa2::default();
        self.line = StereoLinearAdaa2::default();
        self.blockers = [StereoDcBlocker::ZERO; 2];
        self.focus = run.focus.map(StereoBiquad::new);
        self.detectors = [PowerDetector::default(); 4];
        self.gain_phase = 0;
        self.cached_matched = 1.0;
        self.cached_level = run.output;
        self.next = None;
    }
}

/// Sample the static Drive transfer curve over input amplitudes `[-1, 1]`.
/// This preview includes Mix and Output. The focus filter and power matching
/// need a sequence of samples, so they do not enter a static curve.
pub fn sample_drive_curve(drive: &Drive, points: usize) -> Vec<f32> {
    if points == 0 {
        return Vec::new();
    }
    if points == 1 {
        return vec![0.0];
    }
    let gain = db_to_gain(f64::from(drive.amount));
    let character = f64::from(drive.character) / 100.0;
    let mix = f64::from(drive.mix) / 100.0;
    let output = db_to_gain(f64::from(drive.output));
    (0..points)
        .map(|i| {
            let x = -1.0 + 2.0 * (i as f64) / ((points - 1) as f64);
            let shaped = curve(x * gain, character) / gain;
            let blended = (1.0 - mix) * x + mix * shaped;
            (blended * output) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests;
