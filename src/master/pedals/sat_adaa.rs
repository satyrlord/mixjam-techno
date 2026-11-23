//! Second-order antiderivative anti-aliasing (ADAA2), dynamic envelope
//! tracking, and oversampling bridge for the master saturation pedals.
use super::super::MasterChain;
use super::super::dsp::{Delay, OUTER, Oversampler, RATE, db_to_gain, flush, halfband, pole};
use super::super::knobs::PedalKind;
use crate::fx::oversample::{FirOversampler, RenderQuality};
use crate::playback_quality::Oversampling;
use std::f64::consts::{FRAC_PI_2, PI};
use std::sync::LazyLock;

const ADAA_EPS: f64 = 1e-4;

/// Oversampled saturation stage latency in live playback.
pub(in crate::master) const LIVE_SAT_LATENCY: usize = Oversampler::LATENCY;
/// Oversampled saturation stage latency in offline export (8x Kaiser FIR).
pub(in crate::master) const OFFLINE_SAT_LATENCY: usize = FirOversampler::LATENCY_8X;

const GUARDED_OUTER_TAPS: usize = 28;
const GUARDED_FIR_LATENCY: usize = GUARDED_OUTER_TAPS - 1 + 6;
pub(super) const PEAK_GUARD_LATENCY: usize = LIVE_SAT_LATENCY - GUARDED_FIR_LATENCY;
static GUARDED_OUTER: LazyLock<[f32; GUARDED_OUTER_TAPS]> = LazyLock::new(|| halfband(6.0));

/// Piecewise closed-form saturation curves supporting exact f64 ADAA2.
#[derive(Clone, Copy, Default, PartialEq)]
pub(super) enum SatCurve {
    #[default]
    Identity,
    SoftClip {
        knee: f64,
        ceiling: f64,
    },
    Tube {
        gain: f64,
    },
    Tape {
        gain: f64,
    },
}

#[inline]
fn alg_antiderivs(x: f64, k: f64) -> (f64, f64, f64) {
    let u = k * x;
    let u2 = u * u;
    let r = (1.0 + u2).sqrt();
    let f = x / r;
    let inv_k = k.recip();
    let inv_k2 = inv_k * inv_k;
    let f1 = (u2 / (r + 1.0)) * inv_k2;
    let f2 = if u2 < 2.5e-3 {
        let poly = (1.0 / 6.0) + u2 * (-1.0 / 40.0 + u2 * (1.0 / 112.0 - u2 * (5.0 / 1152.0)));
        (u2 * u * poly) * (inv_k2 * inv_k)
    } else {
        let asinh = (u.abs() + r).ln().copysign(u);
        0.5 * (u * r + asinh - 2.0 * u) * (inv_k2 * inv_k)
    };
    (f, f1, f2)
}

#[inline]
fn atan_antiderivs(x: f64, gain_in: f64) -> (f64, f64, f64) {
    let u = gain_in * x;
    let u2 = u * u;
    let at = u.atan();
    let inv_a = 1.0 / gain_in;
    let inv_a2 = inv_a * inv_a;
    let f = inv_a * at;
    let f1 = if u2 < 2.5e-3 {
        let poly = 0.5 + u2 * (-1.0 / 12.0 + u2 * (1.0 / 30.0 - u2 * (1.0 / 56.0)));
        inv_a2 * u2 * poly
    } else {
        inv_a2 * (u * at - 0.5 * (1.0 + u2).ln())
    };
    let f2 = if u2 < 2.5e-3 {
        let poly = (1.0 / 6.0) + u2 * (-1.0 / 60.0 + u2 * (1.0 / 210.0 - u2 * (1.0 / 504.0)));
        (inv_a2 * inv_a) * u2 * u * poly
    } else {
        let l1p = (1.0 + u2).ln();
        (inv_a2 * inv_a) * (0.5 * (u2 - 1.0) * at - 0.5 * u * l1p + 0.5 * u)
    };
    (f, f1, f2)
}

#[inline]
fn soft_clip_antiderivs(x: f64, knee: f64, ceiling: f64) -> (f64, f64, f64) {
    let a = x.abs();
    let s = x.signum();
    if a <= knee {
        return (x, 0.5 * x * x, (x * x * x) / 6.0);
    }
    let room = ceiling - knee;
    let d = a - knee;
    let k2 = knee * knee;
    let k3 = k2 * knee;
    if room > 1e-6 {
        let (g, g1, g2) = alg_antiderivs(d, 1.0 / room);
        let f_mag = knee + g;
        let f1_mag = 0.5 * k2 + knee * d + g1;
        let f2_mag = (k3 / 6.0) + 0.5 * k2 * d + 0.5 * knee * d * d + g2;
        (s * f_mag, f1_mag, s * f2_mag)
    } else {
        let f1_mag = 0.5 * k2 + knee * d;
        let f2_mag = (k3 / 6.0) + 0.5 * k2 * d + 0.5 * knee * d * d;
        (s * knee, f1_mag, s * f2_mag)
    }
}

#[inline]
pub(super) fn eval_curve(curve: SatCurve, x: f64) -> (f64, f64, f64) {
    match curve {
        SatCurve::Identity => (x, 0.5 * x * x, (x * x * x) / 6.0),
        SatCurve::SoftClip { knee, ceiling } => soft_clip_antiderivs(x, knee, ceiling),
        SatCurve::Tube { gain } => {
            if gain < 1e-4 {
                (x, 0.5 * x * x, (x * x * x) / 6.0)
            } else if x >= 0.0 {
                alg_antiderivs(x, 1.35 * gain)
            } else {
                alg_antiderivs(x, 0.35 * gain)
            }
        }
        SatCurve::Tape { gain } => {
            if gain < 1e-4 {
                (x, 0.5 * x * x, (x * x * x) / 6.0)
            } else {
                atan_antiderivs(x, FRAC_PI_2 * gain)
            }
        }
    }
}

#[inline]
fn soft_clip_value(knee: f64, ceiling: f64, x: f64) -> f64 {
    let a = x.abs();
    if a <= knee {
        return x;
    }
    let room = ceiling - knee;
    if room > 1e-6 {
        let u = (a - knee) / room;
        (knee + room * (u / (1.0 + u * u).sqrt())).copysign(x)
    } else {
        ceiling.copysign(x)
    }
}

/// Evaluate `f(x)` only for plotting and ill-conditioned ADAA fallbacks.
#[inline]
pub(super) fn curve_only(curve: SatCurve, x: f64) -> f64 {
    match curve {
        SatCurve::Identity => x,
        SatCurve::SoftClip { knee, ceiling } => soft_clip_value(knee, ceiling, x),
        SatCurve::Tube { gain } if gain >= 1e-4 => {
            let k = if x >= 0.0 { 1.35 * gain } else { 0.35 * gain };
            x / (1.0 + (k * x) * (k * x)).sqrt()
        }
        SatCurve::Tape { gain } if gain >= 1e-4 => {
            let a = FRAC_PI_2 * gain;
            (a * x).atan() / a
        }
        SatCurve::Tube { .. } | SatCurve::Tape { .. } => x,
    }
}

impl SatCurve {
    #[inline]
    fn direct_limit(self) -> Option<f64> {
        match self {
            Self::Identity => Some(f64::INFINITY),
            Self::SoftClip { knee, ceiling }
                if (ceiling - knee).abs() < 1e-6 && ceiling >= 0.9999 =>
            {
                Some(ceiling)
            }
            Self::Tube { gain } | Self::Tape { gain } if gain < 1e-4 => Some(f64::INFINITY),
            _ => None,
        }
    }
}

/// One channel's 2nd-order antiderivative memory.
#[derive(Clone, Copy, Default)]
struct Adaa2Channel {
    x1: f64,
    x2: f64,
    f2_1: f64,
    f2_2: f64,
    curve: SatCurve,
}

impl Adaa2Channel {
    fn clear(&mut self) {
        *self = Self::default();
    }

    #[inline]
    fn d1(curve: SatCurve, xa: f64, xb: f64, f2a: f64, f2b: f64) -> f64 {
        let dx = xa - xb;
        if dx.abs() > ADAA_EPS {
            (f2a - f2b) / dx
        } else {
            eval_curve(curve, 0.5 * (xa + xb)).1
        }
    }

    #[inline]
    fn degenerate(&self, curve: SatCurve, x0: f64) -> f64 {
        let mid = 0.5 * (x0 + self.x2);
        let span = mid - self.x1;
        if span.abs() <= ADAA_EPS {
            return curve_only(curve, 0.5 * (mid + self.x1));
        }
        let (_, f1_mid, f2_mid) = eval_curve(curve, mid);
        let d_mid = Self::d1(curve, mid, self.x1, f2_mid, self.f2_1);
        2.0 * (f1_mid - d_mid) / span
    }

    #[inline]
    fn set_curve(&mut self, curve: SatCurve) {
        // Divided differences require all three ordinates on one curve.
        // A live control glide changes that curve between base-rate frames.
        if curve != self.curve {
            self.f2_1 = eval_curve(curve, self.x1).2;
            self.f2_2 = eval_curve(curve, self.x2).2;
            self.curve = curve;
        }
    }

    #[inline]
    fn process(&mut self, x0: f64) -> f64 {
        let curve = self.curve;
        let (_, _, f2_0) = eval_curve(curve, x0);
        let dx2 = x0 - self.x2;
        let out = if dx2.abs() > ADAA_EPS {
            let d_hi = Self::d1(curve, x0, self.x1, f2_0, self.f2_1);
            let d_lo = Self::d1(curve, self.x1, self.x2, self.f2_1, self.f2_2);
            2.0 * (d_hi - d_lo) / dx2
        } else {
            self.degenerate(curve, x0)
        };
        self.x2 = self.x1;
        self.f2_2 = self.f2_1;
        self.x1 = x0;
        self.f2_1 = f2_0;
        out
    }

    #[inline]
    fn bypass(&mut self, x0: f64) -> f64 {
        self.x2 = self.x1;
        self.x1 = x0;
        self.curve = SatCurve::Identity;
        self.x2
    }

    #[inline]
    fn process_flat_linear(&mut self, sample: f64, flat_linear: bool) -> f64 {
        if flat_linear {
            // Replace ADAA2's linear three-sample average with the center
            // sample. The nonlinear part and the group delay stay the same.
            let correction = (2.0 * self.x1 - sample - self.x2) / 3.0;
            self.process(sample) + correction
        } else {
            self.process(sample)
        }
    }

    #[inline]
    fn process_base(&mut self, sample: f32, direct_limit: Option<f64>, flat_linear: bool) -> f32 {
        // At base rate ADAA2 adds one frame. Delay the direct path by
        // that same frame so both paths have the same delay.
        let sample = f64::from(sample);
        if let Some(limit) = direct_limit {
            self.bypass(sample).clamp(-limit, limit) as f32
        } else {
            self.process_flat_linear(sample, flat_linear) as f32
        }
    }
}

/// Two-stage smoothed RMS envelope follower for dynamic bias and HF self-erasure.
/// Squaring eliminates odd-rectification `4*f0` aliases in high-frequency tones.
#[derive(Clone, Copy)]
pub(super) struct EnvFollower {
    attack: f64,
    release: f64,
    stage1: f64,
    stage2: f64,
}

impl EnvFollower {
    pub(super) fn new() -> Self {
        Self {
            attack: pole(0.015),
            release: pole(0.060),
            stage1: 0.0,
            stage2: 0.0,
        }
    }

    pub(super) fn clear(&mut self) {
        self.stage1 = 0.0;
        self.stage2 = 0.0;
    }

    #[inline]
    pub(super) fn follow(&mut self, sample: f64) -> f64 {
        let p = sample * sample;
        let coeff = if p > self.stage1 {
            self.attack
        } else {
            self.release
        };
        self.stage1 = flush(p + (self.stage1 - p) * coeff);
        self.stage2 = flush(self.stage1 + (self.stage2 - self.stage1) * coeff);
        (self.stage2 * 2.0).sqrt()
    }
}

/// Latency-matched dry delay across Live (69) and Offline (71).
#[derive(Clone, Copy)]
pub(super) enum DryDelay {
    Live(Delay<LIVE_SAT_LATENCY>),
    Offline(Delay<OFFLINE_SAT_LATENCY>),
}

impl DryDelay {
    pub(super) fn new(quality: RenderQuality) -> Self {
        match quality {
            RenderQuality::Live => Self::Live(Delay::ZERO),
            RenderQuality::Offline => Self::Offline(Delay::ZERO),
        }
    }

    pub(super) fn clear(&mut self) {
        match self {
            Self::Live(d) => *d = Delay::ZERO,
            Self::Offline(d) => *d = Delay::ZERO,
        }
    }

    #[inline]
    pub(super) fn push(&mut self, sample: f32) -> f32 {
        match self {
            Self::Live(d) => d.push(sample),
            Self::Offline(d) => d.push(sample),
        }
    }
}

#[derive(Clone, Copy)]
enum SaturationPath {
    Offline,
    Live,
    GuardedLive,
    Base(usize),
}

/// Oversampling + ADAA2 processor for one master saturation channel.
#[derive(Clone, Copy)]
pub(super) struct MasterSaturation {
    path: SaturationPath,
    fir_8x: FirOversampler,
    fir_4x: Oversampler,
    delay_off: Delay<{ LIVE_SAT_LATENCY - 1 }>,
    adaa: Adaa2Channel,
}

impl MasterSaturation {
    pub(super) fn new(render_quality: RenderQuality, live_os: Oversampling) -> Self {
        Self::build(render_quality, live_os, false)
    }

    pub(super) fn guarded(render_quality: RenderQuality, live_os: Oversampling) -> Self {
        Self::build(render_quality, live_os, true)
    }

    fn build(render_quality: RenderQuality, live_os: Oversampling, guarded: bool) -> Self {
        let path = match (render_quality, live_os, guarded) {
            (RenderQuality::Offline, _, _) => SaturationPath::Offline,
            (RenderQuality::Live, Oversampling::FourX, true) => {
                LazyLock::force(&GUARDED_OUTER);
                SaturationPath::GuardedLive
            }
            (RenderQuality::Live, Oversampling::FourX, false) => SaturationPath::Live,
            (RenderQuality::Live, Oversampling::Off, guarded) => SaturationPath::Base(if guarded {
                GUARDED_FIR_LATENCY - 1
            } else {
                LIVE_SAT_LATENCY - 1
            }),
        };
        Self {
            path,
            fir_8x: FirOversampler::new(false),
            fir_4x: Oversampler::ZERO,
            delay_off: Delay::ZERO,
            adaa: Adaa2Channel::default(),
        }
    }

    pub(super) fn clear(&mut self) {
        match self.path {
            SaturationPath::Offline => self.fir_8x.clear(),
            SaturationPath::Live | SaturationPath::GuardedLive => self.fir_4x = Oversampler::ZERO,
            SaturationPath::Base(_) => self.delay_off = Delay::ZERO,
        }
        self.adaa.clear();
    }

    #[inline]
    pub(super) fn process(&mut self, sample: f32, curve: SatCurve) -> f32 {
        self.process_with_linear_response(sample, curve, false)
    }

    #[inline]
    pub(super) fn process_with_linear_response(
        &mut self,
        sample: f32,
        curve: SatCurve,
        flat_linear: bool,
    ) -> f32 {
        let adaa = &mut self.adaa;
        let direct_limit = curve.direct_limit();
        let nonlinear = direct_limit.is_none();
        if nonlinear {
            adaa.set_curve(curve);
        }
        let mut oversampled = |x| {
            if let Some(limit) = direct_limit {
                adaa.bypass(x);
                x.clamp(-limit, limit)
            } else {
                adaa.process_flat_linear(x, flat_linear)
            }
        };
        let shape_f32 = |x| oversampled(f64::from(x)) as f32;
        match self.path {
            SaturationPath::Offline => self.fir_8x.process(f64::from(sample), oversampled) as f32,
            SaturationPath::Live => self
                .fir_4x
                .process_live_saturation(sample, &OUTER, nonlinear, shape_f32),
            SaturationPath::GuardedLive => {
                self.fir_4x
                    .process_live_saturation(sample, &GUARDED_OUTER, nonlinear, shape_f32)
            }
            SaturationPath::Base(frames) => self
                .delay_off
                .push_delayed(adaa.process_base(sample, direct_limit, flat_linear), frames),
        }
    }
}

/// Tape dynamic bias depth relative to normalized drive [0..1].
pub(super) const TAPE_DYNAMIC_BIAS: f64 = 0.02;
/// Tube dynamic bias depth relative to normalized drive [0..1].
pub(super) const TUBE_DYNAMIC_BIAS: f64 = 0.08;
/// Tape normalized pre-emphasis boost in dB at 3 kHz.
pub(super) const TAPE_EMPHASIS_DB: f64 = 4.5;
/// Tape emphasis corner and slope.
pub(super) const TAPE_EMPHASIS_HZ: f64 = 3000.0;
pub(super) const TAPE_EMPHASIS_SLOPE: f64 = 0.7;
/// Maximizer internal ceiling in dBFS so oversampling never exceeds -1.0 dBTP.
pub(super) const MAXIMIZER_CEILING_DB: f64 = -1.35;

/// Sample the normalized static transfer curve `y(x)` over `x in [-1.0, 1.0]`
/// for the Deep Edit CRT oscilloscope.
pub fn sample_saturation_curve(chain: &MasterChain, kind: PedalKind, points: &mut [f32]) {
    if points.is_empty() {
        return;
    }
    let denom = (points.len().saturating_sub(1)).max(1) as f64;
    for (i, dst) in points.iter_mut().enumerate() {
        let x = -1.0 + 2.0 * (i as f64 / denom);
        let y = match kind {
            PedalKind::SoftClip => {
                let ceil = db_to_gain(f64::from(chain.soft_clip.ceiling));
                let knee = db_to_gain(f64::from(chain.soft_clip.ceiling - chain.soft_clip.amount));
                curve_only(
                    SatCurve::SoftClip {
                        knee,
                        ceiling: ceil,
                    },
                    x,
                )
            }
            PedalKind::Tube => {
                let d = f64::from(chain.tube.drive) / 10.0;
                let gain = 4.0 * d * d;
                let mix = f64::from(chain.tube.mix) / 100.0;
                let biased = x + TUBE_DYNAMIC_BIAS * d * x.abs().min(1.0);
                let wet = curve_only(SatCurve::Tube { gain }, biased);
                x + (wet - x) * mix
            }
            PedalKind::Maximizer => {
                let drive_db = 0.25 * f64::from(chain.maximizer.boost);
                let drive = db_to_gain(drive_db);
                let ceiling = db_to_gain(MAXIMIZER_CEILING_DB);
                let knee = db_to_gain(MAXIMIZER_CEILING_DB - 1.0 - drive_db);
                curve_only(SatCurve::SoftClip { knee, ceiling }, x * drive)
            }
            PedalKind::Tape => {
                let d = f64::from(chain.tape.drive) / 10.0;
                let gain = 5.0 * d * d;
                let biased = x + TAPE_DYNAMIC_BIAS * d * x.abs().min(1.0);
                curve_only(SatCurve::Tape { gain }, biased)
            }
            _ => x,
        };
        *dst = y.clamp(-1.2, 1.2) as f32;
    }
}

/// First-order dynamic HF loss filter approximating magnetic self-erasure on hot tape peaks.
#[inline]
pub(super) fn tape_self_erase(state: &mut f64, sample: f64, drive_norm: f64, env: f64) -> f64 {
    let erase_hz =
        (21_000.0 - 9_500.0 * drive_norm * (0.4 + 0.6 * env.min(1.0))).clamp(6_000.0, 21_500.0);
    let alpha = (-2.0 * PI * erase_hz / RATE).exp();
    *state = flush(sample * (1.0 - alpha) + *state * alpha);
    *state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_saturation_antiderivatives_match_numerical_derivatives() {
        let curves = [
            (
                "SoftClip",
                SatCurve::SoftClip {
                    knee: 0.4,
                    ceiling: 0.9,
                },
            ),
            (
                "HardKnee",
                SatCurve::SoftClip {
                    knee: 0.8,
                    ceiling: 0.8,
                },
            ),
            ("Tube", SatCurve::Tube { gain: 2.5 }),
            ("Tape", SatCurve::Tape { gain: 3.0 }),
        ];
        let h = 1e-6;
        for (name, curve) in curves {
            for step in -200..=200 {
                let x = f64::from(step) * 0.0123;
                if matches!(curve, SatCurve::SoftClip { knee, ceiling } if (x.abs() - knee).abs() < 0.01 || (x.abs() - ceiling).abs() < 0.01)
                {
                    continue;
                }
                let (f, f1, _) = eval_curve(curve, x);
                let (_, f1_p, f2_p) = eval_curve(curve, x + h);
                let (_, f1_m, f2_m) = eval_curve(curve, x - h);
                let slope_f1 = (f1_p - f1_m) / (2.0 * h);
                let slope_f2 = (f2_p - f2_m) / (2.0 * h);
                assert!(
                    (slope_f1 - f).abs() < 1e-4,
                    "{name} F1' at {x}: {slope_f1} vs {f}"
                );
                assert!(
                    (slope_f2 - f1).abs() < 1e-4,
                    "{name} F2' at {x}: {slope_f2} vs {f1}"
                );
            }
        }
    }

    #[test]
    fn master_adaa2_near_equal_samples_do_not_produce_nan() {
        let curves = [
            SatCurve::SoftClip {
                knee: 0.4,
                ceiling: 0.9,
            },
            SatCurve::Tube { gain: 2.5 },
            SatCurve::Tape { gain: 3.0 },
        ];
        for curve in curves {
            let mut ch = Adaa2Channel::default();
            ch.set_curve(curve);
            ch.process(0.5);
            ch.process(0.5);
            let y1 = ch.process(0.5 + 1e-8);
            let y2 = ch.process(0.5);
            let want = curve_only(curve, 0.5);
            assert!(y1.is_finite() && (y1 - want).abs() < 1e-2, "{y1} vs {want}");
            assert!(y2.is_finite() && (y2 - want).abs() < 1e-2, "{y2} vs {want}");
        }
    }

    #[test]
    fn master_adaa2_uses_one_curve_for_the_entire_sample_history() {
        let ceiling = 0.9;
        let inputs = [0.8, 0.8002, 0.8005];
        let knees = [0.5, 0.4996, 0.4992];
        let mut gliding = Adaa2Channel::default();
        let mut settled = Adaa2Channel::default();
        settled.set_curve(SatCurve::SoftClip {
            knee: knees[2],
            ceiling,
        });
        let (mut edited, mut expected) = (0.0, 0.0);
        for (x, knee) in inputs.into_iter().zip(knees) {
            gliding.set_curve(SatCurve::SoftClip { knee, ceiling });
            edited = gliding.process(x);
            expected = settled.process(x);
        }
        assert!((edited - expected).abs() < 1e-9, "{edited} vs {expected}");
    }

    #[test]
    fn base_rate_adaa_keeps_the_active_curve_centered_on_the_integer_delay() {
        let mut sat = MasterSaturation::new(RenderQuality::Live, Oversampling::Off);
        let curve = SatCurve::SoftClip {
            knee: 0.5,
            ceiling: 0.9,
        };
        let impulse = 0.125;
        let (mut sum, mut moment) = (0.0, 0.0);
        for frame in 0..LIVE_SAT_LATENCY + 4 {
            let sample = sat.process(if frame == 0 { impulse } else { 0.0 }, curve);
            let weight = f64::from(sample);
            sum += weight;
            moment += frame as f64 * weight;
        }
        assert!((sum - f64::from(impulse)).abs() < 1e-7, "{sum}");
        assert!(
            (moment / sum - LIVE_SAT_LATENCY as f64).abs() < 1e-7,
            "{}",
            moment / sum
        );
    }

    #[test]
    fn unity_ceiling_hard_clip_still_limits_peaks_from_upstream_pedals() {
        let mut sat = MasterSaturation::new(RenderQuality::Live, Oversampling::Off);
        let curve = SatCurve::SoftClip {
            knee: 1.0,
            ceiling: 1.0,
        };
        let samples: Vec<_> = (0..=LIVE_SAT_LATENCY)
            .map(|_| sat.process(1.25, curve))
            .collect();
        assert_eq!(samples[LIVE_SAT_LATENCY], 1.0);
    }
}
