//! The Emberform Drive's signal processing.
//!
//! The input passes a low cut first, so the lows that the dry sound keeps
//! do not muddy the distortion. The drive gain and the asymmetry's offset
//! push it into the shape's curve. Every curve but the crusher's runs at the
//! mix rate through its antiderivative (first-order antiderivative
//! anti-aliasing), so the harmonics of high notes fold back far less. The
//! offset's static share comes off at once and a 5 Hz blocker takes the
//! rest, so the result carries no DC. The result is then held at the power
//! of the filtered input, so the drive changes the tone, not the level. The
//! high cut, the width, the ducking, and the output trim follow.
//!
//! A drive return sums with the dry sound it came from, so it must not be
//! late: a few milliseconds would comb-filter the pair. The core adds no
//! delay of its own. The curve means lag half a frame, and the filters
//! shift the phase as an analog stage would. The bus replaces its safety
//! limiter's lookahead with an instant ceiling.
use super::parts::{Ducker, Glide, OnePole, widen};
use super::{DistortionShape, Emberform};
use crate::master::dsp::{DcBlocker, RATE, db_to_gain, flush, pole};
use std::f64::consts::PI;

/// A shape change crossfades between the old and new curves over 10 ms.
const FADE: usize = 441;
/// The time constant of each of the two stages of the level detectors.
const LEVEL_TIME: f64 = 0.1;
/// How far the level match may move the result, either way, in dB.
const MATCH_DB: f64 = 18.0;
/// A power far below hearing, so that silence divides by nothing.
const FLOOR: f64 = 1e-12;
/// The offset at full asymmetry, in the curve's units.
const FULL_BIAS: f64 = 0.6;

/// `ln cosh x`, the antiderivative of `tanh`, without overflow.
#[inline]
fn log_cosh(x: f64) -> f64 {
    let size = x.abs();
    size + (-2.0 * size).exp().ln_1p() - std::f64::consts::LN_2
}

impl DistortionShape {
    /// The extra gain the shape puts before its curve.
    fn pre_gain(self) -> f64 {
        match self {
            Self::Fuzz => 4.0,
            _ => 1.0,
        }
    }

    /// The curve.
    #[inline]
    fn curve(self, x: f64) -> f64 {
        match self {
            // The negative half bends later than the positive one: even harmonics.
            Self::Tube => {
                if x >= 0.0 {
                    x.tanh()
                } else {
                    2.0 * (0.5 * x).tanh()
                }
            }
            Self::Tape => x.tanh(),
            Self::Overdrive => {
                if x.abs() <= 1.0 {
                    1.5 * (x - x * x * x / 3.0)
                } else {
                    x.signum()
                }
            }
            Self::Fuzz => x.signum() * (1.0 - (-x.abs()).exp()),
            Self::HardClip => x.clamp(-1.0, 1.0),
            Self::Foldback => x.sin(),
            Self::Rectifier => x.abs().tanh(),
            // The crusher does not use the curve means; its clip is gentle.
            Self::Bitcrush => x.tanh(),
        }
    }

    /// The curve's antiderivative, zero at zero.
    #[inline]
    fn integral(self, x: f64) -> f64 {
        match self {
            Self::Tube => {
                if x >= 0.0 {
                    log_cosh(x)
                } else {
                    4.0 * log_cosh(0.5 * x)
                }
            }
            Self::Tape | Self::Bitcrush => log_cosh(x),
            Self::Overdrive => {
                let size = x.abs();
                if size <= 1.0 {
                    1.5 * (0.5 * x * x - x * x * x * x / 12.0)
                } else {
                    size - 0.375
                }
            }
            Self::Fuzz => {
                let size = x.abs();
                size + (-size).exp() - 1.0
            }
            Self::HardClip => {
                let size = x.abs();
                if size <= 1.0 { 0.5 * x * x } else { size - 0.5 }
            }
            Self::Foldback => 1.0 - x.cos(),
            Self::Rectifier => x.signum() * log_cosh(x),
        }
    }

    /// The curve's mean from `from` to `to`: the difference of its
    /// antiderivative over the distance, or the midpoint's value when the
    /// two are too close for the quotient to keep its precision.
    #[inline]
    fn mean(self, from: f64, to: f64) -> f64 {
        let distance = to - from;
        if distance.abs() < 1e-6 {
            self.curve(0.5 * (from + to))
        } else {
            (self.integral(to) - self.integral(from)) / distance
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

/// One channel's history.
#[derive(Clone, Copy)]
struct Side {
    /// The two poles of the low cut and of the high cut.
    low_cut: [OnePole; 2],
    high_cut: [OnePole; 2],
    /// The tape shape's extra softening of the top.
    soften: OnePole,
    dc: DcBlocker,
    /// The last driven sample, where the next curve mean starts.
    last: f64,
    /// The crusher's held sample.
    held: f64,
}
impl Default for Side {
    fn default() -> Self {
        Self {
            low_cut: [OnePole::default(); 2],
            high_cut: [OnePole::default(); 2],
            soften: OnePole::default(),
            dc: DcBlocker::ZERO,
            last: 0.0,
            held: 0.0,
        }
    }
}

pub(super) struct Core {
    sides: [Side; 2],
    shape: DistortionShape,
    /// The shape a change fades from, and the frames left of the fade.
    from: DistortionShape,
    fade: usize,
    /// The crusher's phase from 0 to 1, and its step per frame.
    phase: f64,
    step: f64,
    /// The crusher's steps on each side of zero.
    levels: f64,
    tape: Glide,
    gain: Glide,
    bias: Glide,
    width: Glide,
    output: Glide,
    duck: Glide,
    ducker: Ducker,
    dc_pole: f64,
    level_pole: f64,
    detectors: [Detector; 2],
    activity: f64,
}

impl Core {
    pub(super) fn new() -> Self {
        let smooth = |value| Glide::new(value, 0.005);
        Self {
            sides: [Side::default(); 2],
            shape: DistortionShape::Tube,
            from: DistortionShape::Tube,
            fade: 0,
            phase: 0.0,
            step: 1.0,
            levels: 128.0,
            tape: Glide::new(0.0, 0.01),
            gain: smooth(1.0),
            bias: smooth(0.0),
            width: smooth(1.0),
            output: smooth(0.0),
            duck: smooth(0.0),
            ducker: Ducker::new(),
            dc_pole: pole(1.0 / (2.0 * PI * 5.0)),
            level_pole: pole(LEVEL_TIME),
            detectors: [Detector::default(); 2],
            activity: 0.0,
        }
    }

    /// Take new settings. A new shape fades in from the old one.
    pub(super) fn set(&mut self, settings: &Emberform) {
        if settings.shape != self.shape {
            self.from = self.shape;
            self.shape = settings.shape;
            self.fade = FADE;
        }
        let (low_cut, high_cut) = (
            f64::from(settings.low_cut_hz),
            f64::from(settings.high_cut_hz),
        );
        for side in &mut self.sides {
            side.low_cut.iter_mut().for_each(|pole| pole.tune(low_cut));
            side.high_cut
                .iter_mut()
                .for_each(|pole| pole.tune(high_cut));
            side.soften.tune(0.45 * high_cut);
        }
        self.step = f64::from(settings.crush_rate_hz) / RATE;
        self.levels = f64::from(2u32.pow(u32::from(settings.bits.clamp(1, 16)) - 1));
        self.tape.set(if settings.shape == DistortionShape::Tape {
            1.0
        } else {
            0.0
        });
        self.gain
            .set(db_to_gain(f64::from(settings.drive_db)) * settings.shape.pre_gain());
        self.bias
            .set(f64::from(settings.asymmetry_percent) / 100.0 * FULL_BIAS);
        self.width.set(f64::from(settings.width_percent) / 100.0);
        self.duck
            .set(f64::from(settings.duck_amount_percent) / 100.0);
        self.ducker.set_release(f64::from(settings.duck_release_ms));
        self.output.set(if settings.bypass {
            0.0
        } else {
            db_to_gain(f64::from(settings.output_db))
        });
    }

    /// Move every setting straight to its target.
    pub(super) fn settle(&mut self) {
        for glide in [
            &mut self.tape,
            &mut self.gain,
            &mut self.bias,
            &mut self.width,
            &mut self.output,
            &mut self.duck,
        ] {
            glide.settle();
        }
        self.rest();
    }

    /// Silence every filter and detector.
    pub(super) fn clear(&mut self) {
        for side in &mut self.sides {
            side.low_cut.iter_mut().for_each(OnePole::clear);
            side.high_cut.iter_mut().for_each(OnePole::clear);
            side.soften.clear();
        }
        self.phase = 0.0;
        self.ducker.clear();
        self.detectors = [Detector::default(); 2];
        self.activity = 0.0;
        self.rest();
    }

    /// Put the curve's history where silence leaves it. The driven sample
    /// rests at the asymmetry's offset, not at zero, so a start from zero
    /// would click on the first frame.
    fn rest(&mut self) {
        self.fade = 0;
        self.from = self.shape;
        let bias = self.bias.value();
        let held = (DistortionShape::Bitcrush.curve(bias) * self.levels).round() / self.levels;
        let dc = DcBlocker::resting(Self::shaped(self.shape, bias, bias, bias, held));
        for side in &mut self.sides {
            side.last = bias;
            side.held = held;
            side.dc = dc;
        }
    }

    /// The level of the last frame before the output trim and the ducking.
    pub(super) fn activity(&self) -> f64 {
        self.activity
    }

    /// One shape's output for a driven sample `to`, which follows `from`,
    /// less the offset's static share. `crushed` is the crusher's held step.
    #[inline]
    fn shaped(shape: DistortionShape, from: f64, to: f64, bias: f64, crushed: f64) -> f64 {
        let y = if shape == DistortionShape::Bitcrush {
            crushed
        } else {
            shape.mean(from, to)
        };
        y - shape.curve(bias)
    }

    #[inline]
    #[cfg(test)]
    pub(super) fn process(&mut self, left: f64, right: f64) -> (f64, f64) {
        self.process_keyed(left, right, None)
    }

    /// With a key, the ducking follows it instead of the input.
    pub(super) fn process_keyed(
        &mut self,
        left: f64,
        right: f64,
        key: Option<(f64, f64)>,
    ) -> (f64, f64) {
        let (key_left, key_right) = key.unwrap_or((left, right));
        let duck = self.ducker.gain(key_left, key_right, self.duck.next());
        let (gain, bias, tape) = (self.gain.next(), self.bias.next(), self.tape.next());
        self.phase += self.step;
        let hold = self.phase >= 1.0;
        if hold {
            self.phase -= self.phase.floor();
        }
        let weight = if self.fade > 0 {
            self.fade -= 1;
            let progress = 1.0 - self.fade as f64 / FADE as f64;
            0.5 - 0.5 * (PI * progress).cos()
        } else {
            1.0
        };
        let mut clean = [0.0; 2];
        let mut wet = [0.0; 2];
        for (index, input) in [left, right].into_iter().enumerate() {
            let levels = self.levels;
            let side = &mut self.sides[index];
            let [first, second] = &mut side.low_cut;
            let x = second.high(first.high(input));
            let to = x * gain + bias;
            let from = side.last;
            side.last = to;
            if hold {
                side.held = (DistortionShape::Bitcrush.curve(to) * levels).round() / levels;
            }
            let now = Self::shaped(self.shape, from, to, bias, side.held);
            let mut y = if weight < 1.0 {
                let before = Self::shaped(self.from, from, to, bias, side.held);
                before + (now - before) * weight
            } else {
                now
            };
            y = side.dc.process(y, self.dc_pole);
            if tape > 0.0 {
                let soft = side.soften.low(y);
                y += tape * (soft - y);
            }
            clean[index] = x;
            wet[index] = y;
        }
        let power = |pair: [f64; 2]| 0.5 * (pair[0] * pair[0] + pair[1] * pair[1]);
        let [clean_power, wet_power] = &mut self.detectors;
        let (heard, made) = (
            clean_power.follow(power(clean), self.level_pole),
            wet_power.follow(power(wet), self.level_pole),
        );
        let limit = db_to_gain(MATCH_DB);
        let matched = ((heard + FLOOR) / (made + FLOOR))
            .sqrt()
            .clamp(1.0 / limit, limit);
        let mut out = [0.0; 2];
        for (index, sample) in wet.into_iter().enumerate() {
            let side = &mut self.sides[index];
            let [first, second] = &mut side.high_cut;
            out[index] = second.low(first.low(sample * matched));
        }
        self.activity = out[0].abs().max(out[1].abs());
        let (l, r) = widen(out[0], out[1], self.width.next());
        let trim = self.output.next() * duck;
        (l * trim, r * trim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(shape: DistortionShape) -> Emberform {
        Emberform {
            shape,
            duck_amount_percent: 0.0,
            ..Emberform::default()
        }
    }

    fn render(settings: &Emberform, frames: usize, input: impl Fn(usize) -> f64) -> Vec<f64> {
        let mut core = Core::new();
        core.set(settings);
        core.settle();
        (0..frames)
            .map(|frame| {
                let x = input(frame);
                core.process(x, x).0
            })
            .collect()
    }

    fn rms(samples: &[f64]) -> f64 {
        (samples.iter().map(|x| x * x).sum::<f64>() / samples.len() as f64).sqrt()
    }

    /// The power of `samples` at `hz`, from a single-bin transform.
    fn bin(samples: &[f64], hz: f64) -> f64 {
        let (mut re, mut im) = (0.0, 0.0);
        for (n, x) in samples.iter().enumerate() {
            let phase = 2.0 * PI * hz * n as f64 / RATE;
            re += x * phase.cos();
            im += x * phase.sin();
        }
        (re * re + im * im).sqrt() / samples.len() as f64
    }

    #[test]
    fn every_antiderivative_matches_its_curve() {
        for shape in DistortionShape::ALL {
            for step in -400..400 {
                let x = f64::from(step) * 0.0137;
                let h = 1e-5;
                let slope = (shape.integral(x + h) - shape.integral(x - h)) / (2.0 * h);
                assert!(
                    (slope - shape.curve(x)).abs() < 1e-4,
                    "{shape:?} at {x}: {slope} against {}",
                    shape.curve(x)
                );
            }
            assert!(shape.integral(0.0).abs() < 1e-12, "{shape:?}");
        }
    }

    #[test]
    fn every_shape_holds_the_level_and_carries_no_dc() {
        let tone = |frame: usize| 0.25 * (2.0 * PI * 220.0 * frame as f64 / RATE).sin();
        let input: Vec<f64> = (0..RATE as usize).map(tone).collect();
        let reference = rms(&input[22_050..]);
        for shape in DistortionShape::ALL {
            let out = render(
                &Emberform {
                    drive_db: 24.0,
                    asymmetry_percent: 60.0,
                    low_cut_hz: 20.0,
                    high_cut_hz: 20_000.0,
                    output_db: 0.0,
                    ..settings(shape)
                },
                input.len(),
                tone,
            );
            let tail = &out[22_050..];
            let level = 20.0 * (rms(tail) / reference).log10();
            assert!(level.abs() < 3.0, "{shape:?} is {level:.1} dB off");
            let mean = tail.iter().sum::<f64>() / tail.len() as f64;
            assert!(mean.abs() < 1e-3, "{shape:?} carries {mean} of DC");
            assert!(tail.iter().all(|x| x.is_finite() && x.abs() < 2.0));
        }
    }

    #[test]
    fn the_drive_adds_harmonics_and_the_return_is_not_late() {
        let hz = 441.0;
        let tone = |frame: usize| 0.2 * (2.0 * PI * hz * frame as f64 / RATE).sin();
        let gentle = render(
            &Emberform {
                drive_db: 0.0,
                ..settings(DistortionShape::Tape)
            },
            44_100,
            tone,
        );
        let hot = render(
            &Emberform {
                drive_db: 30.0,
                ..settings(DistortionShape::Tape)
            },
            44_100,
            tone,
        );
        let third = |out: &[f64]| bin(&out[22_050..], 3.0 * hz) / bin(&out[22_050..], hz);
        assert!(third(&hot) > 10.0 * third(&gentle));
        // An impulse comes back within a frame or two: nothing sits in the
        // way but the filters.
        let mut core = Core::new();
        core.set(&Emberform {
            drive_db: 0.0,
            low_cut_hz: 20.0,
            high_cut_hz: 20_000.0,
            ..settings(DistortionShape::HardClip)
        });
        core.settle();
        let out: Vec<f64> = (0..64)
            .map(|frame| core.process(if frame == 10 { 0.3 } else { 0.0 }, 0.0).0)
            .collect();
        let peak = (0..64)
            .max_by(|&a, &b| out[a].abs().total_cmp(&out[b].abs()))
            .unwrap();
        assert!((10..=12).contains(&peak), "{peak}");
    }

    #[test]
    fn the_crusher_steps_and_holds() {
        let ramp = |frame: usize| (frame as f64 / 2000.0).min(0.8);
        let mut core = Core::new();
        core.set(&Emberform {
            drive_db: 0.0,
            bits: 3,
            crush_rate_hz: 4410.0,
            low_cut_hz: 20.0,
            high_cut_hz: 20_000.0,
            ..settings(DistortionShape::Bitcrush)
        });
        core.settle();
        // The held values change at most once in ten frames.
        let mut changes = 0;
        let mut last = None;
        for frame in 0..1000 {
            core.process(ramp(frame), ramp(frame));
            let held = core.sides[0].held;
            if last != Some(held) {
                changes += 1;
                last = Some(held);
            }
            assert_eq!((held * 4.0).fract(), 0.0, "{held} is not on a step");
        }
        assert!(changes <= 101, "{changes}");
    }

    #[test]
    fn bypass_and_a_shape_change_stay_quiet() {
        let tone = |frame: usize| 0.3 * (2.0 * PI * 110.0 * frame as f64 / RATE).sin();
        let mut core = Core::new();
        core.set(&settings(DistortionShape::Tube));
        core.settle();
        let mut largest_step: f64 = 0.0;
        let mut previous = 0.0;
        for frame in 0..20_000 {
            if frame == 10_000 {
                core.set(&settings(DistortionShape::Foldback));
            }
            let (l, _) = core.process(tone(frame), tone(frame));
            if frame > 5000 {
                largest_step = largest_step.max((l - previous).abs());
            }
            previous = l;
        }
        assert!(largest_step < 0.2, "{largest_step}");
        let out = render(
            &Emberform {
                bypass: true,
                ..settings(DistortionShape::Fuzz)
            },
            2000,
            tone,
        );
        assert!(out.iter().all(|x| *x == 0.0));
    }

    /// A core at rest, new or cleared when its bus sleeps, already holds the
    /// asymmetry's offset: silence in is silence out from the first frame.
    #[test]
    fn a_resting_core_wakes_without_a_click() {
        let tone = |frame: usize| 0.5 * (2.0 * PI * 220.0 * frame as f64 / RATE).sin();
        for shape in DistortionShape::ALL {
            let mut core = Core::new();
            core.set(&Emberform {
                asymmetry_percent: 100.0,
                bits: 2,
                ..settings(shape)
            });
            core.settle();
            for wake in ["new", "cleared"] {
                let peak = (0..2205)
                    .map(|_| {
                        let (l, r) = core.process(0.0, 0.0);
                        l.abs().max(r.abs())
                    })
                    .fold(0.0, f64::max);
                assert!(peak < 1e-9, "{shape:?}, {wake}: {peak}");
                for frame in 0..4410 {
                    core.process(tone(frame), tone(frame));
                }
                core.clear();
            }
        }
    }
}
