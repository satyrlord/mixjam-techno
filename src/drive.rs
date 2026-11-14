//! The lane drive: a saturation that adds harmonics to its track, between
//! the channel EQ and the compressor.
//!
//! A held synth bass is close to a pure tone. Speakers that cannot play its
//! fundamental leave nothing of it, and an EQ cannot raise harmonics that
//! are not there. The drive makes them: it bends a copy of the track
//! through a saturation curve and adds what the curve changed to the clean
//! track, so the clean sound stays and the new harmonics come in under it.
//!
//! - At full quality the curve runs at four times the mix rate
//!   (`master::dsp::Oversampler`),
//!   and each of its samples is the curve's mean between two input samples
//!   (antiderivative anti-aliasing), so the harmonics of high notes and hits
//!   do not fold back as aliases. The clean track that the copy is compared
//!   with takes the same path through a straight line, so the two differ only
//!   by what the curve adds.
//!   Reduced playback runs both interval means at the base rate and keeps
//!   the integer stage delay with plain delays. It retains anti-aliasing by
//!   the antiderivative method, but has more aliases and a different response.
//! - `amount` drives the copy into the curve. `character` bends one half of
//!   the wave less than the other: symmetric at 0% gives odd harmonics,
//!   one-sided at 100% mostly even ones, at any drive.
//! - The copy is matched to the clean track's power before it is compared,
//!   so `amount` changes the tone, not the level.
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
//! smoothing stages, so the gains move without steps or ripple and the
//! stereo image stays put. The settings are fixed for a prepared mix; a
//! change prepares a new mix, as an EQ change does. Both quality modes delay
//! the copy by `LATENCY` integer frames, which the track chain compensates.
//! The curve and its straight-line reference also share an interval mean's
//! fractional delay: an eighth of a frame at 4x, half a frame at the base
//! rate. Their phases agree with each other, but are not equivalent between
//! modes. The clean track itself passes through a plain delay, so a `mix`
//! of 0% at 0 dB output gives the track back bit for bit. A track whose drive is off mixes
//! exactly as it did before the drive existed.
use crate::master::{
    KnobRange,
    dsp::{Biquad, Coefficients, DcBlocker, Delay, Oversampler, db_to_gain, flush, pole},
    range, ratio_range,
};
use crate::playback_quality::Oversampling;
use serde::{Deserialize, Serialize};

/// The frames by which the drive's output follows its input.
pub(crate) const LATENCY: usize = Oversampler::LATENCY;

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
        let range = self.range();
        let value = if value.is_nan() {
            range.default
        } else {
            value.clamp(range.low, range.high)
        };
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
        if DriveParam::ALL.into_iter().any(|param| {
            let (value, range) = (param.get(self), param.range());
            !(value.is_finite() && (range.low..=range.high).contains(&value))
        }) {
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

/// `ln cosh u`, the antiderivative of `tanh`, without overflow.
#[inline]
fn log_cosh(u: f64) -> f64 {
    let size = u.abs();
    size + (-2.0 * size).exp().ln_1p() - std::f64::consts::LN_2
}

/// The antiderivative of `curve` that is zero at zero.
#[inline]
fn integral(u: f64, character: f64) -> f64 {
    let bent = log_cosh(u);
    if u >= 0.0 {
        bent
    } else {
        bent + character * (0.5 * u * u - bent)
    }
}

/// The curve's mean between consecutive samples: the difference of its
/// antiderivative over the distance. Keep the last endpoint's integral so
/// the next sample does not calculate its exponential and logarithm again.
/// The character stays fixed for this prepared mix.
#[derive(Clone, Copy, Default)]
struct Mean {
    last: f64,
    integral: Option<f64>,
}

impl Mean {
    #[inline]
    fn pair(
        &mut self,
        sample: f32,
        held: &mut f32,
        gain: f64,
        inverse: f64,
        character: f64,
    ) -> [f32; 2] {
        let shaped = self.next(f64::from(sample) * gain, character) * inverse;
        let line = 0.5 * (*held + sample);
        *held = sample;
        [shaped as f32, line]
    }

    #[inline]
    fn next(&mut self, to: f64, character: f64) -> f64 {
        let from = self.last;
        self.last = to;
        let distance = to - from;
        if distance.abs() < 1e-5 {
            // Close points use the midpoint, where the quotient loses its
            // precision. Do not calculate an unused integral in silence;
            // the next wider step calculates this endpoint if it needs it.
            self.integral = None;
            curve(0.5 * (from + to), character)
        } else {
            let before = self.integral.unwrap_or_else(|| integral(from, character));
            let after = integral(to, character);
            self.integral = Some(after);
            (after - before) / distance
        }
    }
}

/// The time constant of each of the two stages of the detectors that match
/// the copy to the clean track, in seconds. They follow a note's level
/// within a few tens of milliseconds. The ripple they leave at twice a
/// bass's frequency only moves the copy's harmonics a little.
const MATCH_TIME: f64 = 0.015;
/// The same for the detectors that hold the result at the clean track's
/// power. They are slower, because their gain moves the whole track, but
/// the result's power differs from the clean track's only by the added
/// harmonics, so a change of level needs no fast correction.
const LEVEL_TIME: f64 = 0.15;
/// The corner of the blockers that take the offset out of the copy.
const BLOCKER_HZ: f64 = 5.0;
/// A power far below hearing, so that silence divides by nothing.
const FLOOR: f64 = 1e-12;
/// The quality factors of the two sections of a fourth-order Butterworth
/// high-pass.
const FOCUS_Q: [f64; 2] = [0.541_196_100_146_197, 1.306_562_964_876_376_5];
/// How far the matching gains may go, in dB: the copy's below and above the
/// drive, and the output's either way.
const MATCH_BELOW_DB: f64 = 6.0;
const MATCH_ABOVE_DB: f64 = 12.0;
const OUTPUT_MATCH_DB: f64 = 12.0;

/// A drive's settings, ready for the audio thread at the mix rate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Run {
    oversampling: Oversampling,
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
        Self::with_quality(drive, Oversampling::FourX)
    }

    pub(crate) fn with_quality(drive: &Drive, oversampling: Oversampling) -> Self {
        let amount = f64::from(drive.amount);
        let gain = db_to_gain(amount);
        let focus = f64::from(drive.focus);
        Self {
            oversampling,
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

    /// Drive interleaved stereo frames in place. The output of each frame is
    /// the input of `LATENCY` frames before it. The state carries from block
    /// to block.
    pub(crate) fn process(&self, frames: &mut [f32], state: &mut State) {
        let (gain, inverse, character) = (self.gain, self.inverse, self.character);
        let [fast, slow] = self.poles;
        for frame in frames.as_chunks_mut::<2>().0 {
            if !state.started {
                // Until the first sound, every history remains zero. Do not
                // run the filters and curve for a track that has not started.
                // Preserve negative zero too, and always process later tails.
                if frame.iter().all(|sample| sample.to_bits() == 0) {
                    continue;
                }
                state.started = true;
            }
            let mut dry = [0.0_f64; 2];
            let mut copy = [0.0_f64; 2];
            let mut clean = [0.0_f64; 2];
            for channel in 0..2 {
                let input = frame[channel];
                // Each of the four samples is the curve's mean since the
                // sample before it, and the clean track's the straight
                // line's: both lag half a sample at four times the rate.
                let (mut mean, mut held) = (state.means[channel], state.held[channel]);
                let shape = |sample: f32| mean.pair(sample, &mut held, gain, inverse, character);
                let [shaped, reference] = match self.oversampling {
                    Oversampling::FourX => state.oversamplers[channel].process_pair(
                        &mut state.references[channel],
                        input,
                        shape,
                    ),
                    Oversampling::Off => {
                        // ADAA and its straight-line reference both run at
                        // the base rate. Both lag half a base-rate frame,
                        // rather than the 4x path's eighth of a frame. Keep
                        // their shared phase and the chain's integer delay.
                        let [shaped, reference] = {
                            let mut shape = shape;
                            shape(input)
                        };
                        let [curve_delay, reference_delay] = &mut state.base_delays[channel];
                        [curve_delay.push(shaped), reference_delay.push(reference)]
                    }
                };
                (state.means[channel], state.held[channel]) = (mean, held);
                dry[channel] = f64::from(state.dry[channel].push(input));
                let [copy_blocker, clean_blocker] = &mut state.blockers[channel];
                copy[channel] = copy_blocker.process(f64::from(shaped), self.blocker);
                clean[channel] = clean_blocker.process(f64::from(reference), self.blocker);
            }
            let power = |pair: [f64; 2]| 0.5 * (pair[0] * pair[0] + pair[1] * pair[1]);
            let dry_power = power(dry);
            let [dry_fast, copy_fast, dry_slow, wet_slow] = &mut state.detectors;
            let matched = ((dry_fast.follow(dry_power, fast) + FLOOR)
                / (copy_fast.follow(power(copy), fast) + FLOOR))
                .sqrt()
                .clamp(self.copy[0], self.copy[1]);
            let mut wet = [0.0_f64; 2];
            for channel in 0..2 {
                let change = matched * copy[channel] - clean[channel];
                let change = state.focus[0].process(channel, change);
                let change = state.focus[1].process(channel, change);
                wet[channel] = dry[channel] + self.mix * change;
            }
            let level = ((dry_slow.follow(dry_power, slow) + FLOOR)
                / (wet_slow.follow(power(wet), slow) + FLOOR))
                .sqrt()
                .clamp(self.level[0], self.level[1])
                * self.output;
            *frame = wet.map(|sample| (sample * level) as f32);
        }
    }
}

/// A power detector: two one-pole stages in a row, which smooth the ripple
/// of a low note far better than one stage with the same response time.
#[derive(Clone, Copy, Default)]
struct Detector([f64; 2]);

impl Detector {
    #[inline]
    fn follow(&mut self, power: f64, pole: f64) -> f64 {
        let [first, second] = &mut self.0;
        *first = flush(power + (*first - power) * pole);
        *second = flush(*first + (*second - *first) * pole);
        *second
    }
}

/// The drive's history: the oversampling filters, the clean track's delay,
/// the blockers, the focus filter, and the power detectors.
#[derive(Clone, Copy)]
pub(crate) struct State {
    /// Whether any input since the reset had a nonzero bit pattern. A
    /// pristine silent history needs no DSP; after sound, keep every tail.
    started: bool,
    /// Each channel's curve and its straight line at four times the rate,
    /// and the endpoints where their next means start.
    oversamplers: [Oversampler; 2],
    references: [Oversampler; 2],
    means: [Mean; 2],
    held: [f32; 2],
    dry: [Delay<LATENCY>; 2],
    /// The base-rate curve and reference keep the oversampler's integer delay.
    base_delays: [[Delay<LATENCY>; 2]; 2],
    /// Each channel's blockers for the copy and the clean track.
    blockers: [[DcBlocker; 2]; 2],
    focus: [Biquad; 2],
    /// The clean track's and the copy's power for the matching, and the
    /// clean track's and the result's for the level.
    detectors: [Detector; 4],
    /// The input frame that the next block starts with, when the history
    /// leads up to it.
    pub(crate) next: Option<u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            started: false,
            oversamplers: [Oversampler::ZERO; 2],
            references: [Oversampler::ZERO; 2],
            means: [Mean::default(); 2],
            held: [0.0; 2],
            dry: [Delay::ZERO; 2],
            base_delays: [[Delay::ZERO; 2]; 2],
            blockers: [[DcBlocker::ZERO; 2]; 2],
            focus: [Biquad::FLAT; 2],
            detectors: [Detector::default(); 4],
            next: None,
        }
    }
}

impl State {
    /// A history of silence for `run`.
    pub(crate) fn new(run: &Run) -> Self {
        Self {
            focus: run.focus.map(Biquad::new),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests;
