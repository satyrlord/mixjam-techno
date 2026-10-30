//! The lane compressor: it evens out its track's level, such as the
//! transients of a drum loop, right after the channel EQ.
//!
//! One detector follows the mean power of both channels, as the master's
//! Bus compressor does, so both get the same gain. Above the threshold a
//! 6 dB soft knee bends the level toward the ratio, the attack and release
//! smooth the reduction, and the makeup gain follows. It works before the
//! track's width, automation, fader, pan, and ducker, so the fader does not
//! change how hard it compresses. A track whose compressor is off mixes
//! exactly as it did before the compressor existed.
use crate::master::{
    KnobRange,
    dsp::{DETECTOR, KNEE_DB, db_to_gain, flush, pole, reduction},
    range, ratio_range,
};
use serde::{Deserialize, Serialize};

/// One track's compressor settings. The compressor keeps them while it is off.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Compressor {
    pub on: bool,
    /// The level in dBFS above which the track is compressed.
    pub threshold: f32,
    /// How many decibels over the threshold give one decibel out.
    pub ratio: f32,
    /// Milliseconds.
    pub attack: f32,
    /// Milliseconds.
    pub release: f32,
    /// The gain after the compression, in dB.
    pub makeup: f32,
}

impl Default for Compressor {
    fn default() -> Self {
        let mut compressor = Self {
            on: false,
            threshold: 0.0,
            ratio: 0.0,
            attack: 0.0,
            release: 0.0,
            makeup: 0.0,
        };
        for param in CompressorParam::ALL {
            param.set(&mut compressor, param.range().default);
        }
        compressor
    }
}

/// A continuous control of the compressor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompressorParam {
    Threshold,
    Ratio,
    Attack,
    Release,
    Makeup,
}

impl CompressorParam {
    pub const ALL: [Self; 5] = [
        Self::Threshold,
        Self::Ratio,
        Self::Attack,
        Self::Release,
        Self::Makeup,
    ];

    pub const fn range(self) -> KnobRange {
        match self {
            Self::Threshold => range(-60.0, 0.0, -20.0, 0.5),
            Self::Ratio => ratio_range(1.0, 20.0, 4.0, 0.1),
            Self::Attack => ratio_range(0.1, 100.0, 10.0, 0.1),
            Self::Release => ratio_range(10.0, 1000.0, 100.0, 1.0),
            Self::Makeup => range(0.0, 24.0, 0.0, 0.5),
        }
    }

    /// The control's name after its track's, such as "compressor ratio".
    pub fn name(self) -> &'static str {
        match self {
            Self::Threshold => "compressor threshold",
            Self::Ratio => "compressor ratio",
            Self::Attack => "compressor attack",
            Self::Release => "compressor release",
            Self::Makeup => "compressor makeup",
        }
    }

    pub fn get(self, compressor: &Compressor) -> f32 {
        match self {
            Self::Threshold => compressor.threshold,
            Self::Ratio => compressor.ratio,
            Self::Attack => compressor.attack,
            Self::Release => compressor.release,
            Self::Makeup => compressor.makeup,
        }
    }

    /// Set the control, held to its range.
    pub fn set(self, compressor: &mut Compressor, value: f32) {
        let range = self.range();
        let value = if value.is_nan() {
            range.default
        } else {
            value.clamp(range.low, range.high)
        };
        match self {
            Self::Threshold => compressor.threshold = value,
            Self::Ratio => compressor.ratio = value,
            Self::Attack => compressor.attack = value,
            Self::Release => compressor.release = value,
            Self::Makeup => compressor.makeup = value,
        }
    }
}

impl Compressor {
    pub fn validate(&self) -> Result<(), String> {
        if CompressorParam::ALL.into_iter().any(|param| {
            let (value, range) = (param.get(self), param.range());
            !(value.is_finite() && (range.low..=range.high).contains(&value))
        }) {
            return Err("A compressor setting is out of range.".into());
        }
        Ok(())
    }

    /// The settings and the switch as a new track has them.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// A compressor's settings, ready for the audio thread at the mix rate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Run {
    threshold: f64,
    ratio: f64,
    attack: f64,
    release: f64,
    detector: f64,
    makeup: f64,
}

impl Run {
    pub(crate) fn new(compressor: &Compressor) -> Self {
        Self {
            threshold: f64::from(compressor.threshold),
            ratio: f64::from(compressor.ratio),
            attack: pole(f64::from(compressor.attack) / 1000.0),
            release: pole(f64::from(compressor.release) / 1000.0),
            detector: pole(DETECTOR),
            makeup: f64::from(compressor.makeup),
        }
    }

    /// Compress interleaved stereo frames in place. The state carries from
    /// block to block.
    pub(crate) fn process(&self, frames: &mut [f32], state: &mut State) {
        for frame in frames.as_chunks_mut::<2>().0 {
            let (left, right) = (f64::from(frame[0]), f64::from(frame[1]));
            let power = 0.5 * (left * left + right * right);
            state.power = flush(power + (state.power - power) * self.detector);
            let level = 10.0 * state.power.max(1e-12).log10();
            let target = reduction(level, self.threshold, self.ratio, KNEE_DB);
            let speed = if target > state.reduction {
                self.attack
            } else {
                self.release
            };
            state.reduction = flush(target + (state.reduction - target) * speed);
            if state.reduction == 0.0 && self.makeup == 0.0 {
                continue;
            }
            let gain = db_to_gain(self.makeup - state.reduction);
            *frame = [(left * gain) as f32, (right * gain) as f32];
        }
    }
}

/// The compressor's history: the detector's power and the current
/// reduction in dB.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct State {
    power: f64,
    reduction: f64,
}

impl State {
    /// The power and the reduction as two f32 in one value, for an atomic
    /// store that a reader cannot tear.
    pub(crate) fn pack(self) -> u64 {
        let [power, reduction] = [self.power, self.reduction].map(|value| (value as f32).to_bits());
        u64::from(power) | (u64::from(reduction) << 32)
    }

    /// A history that `pack` stored, if one is there: NaN bits mean none.
    pub(crate) fn unpack(packed: u64) -> Option<Self> {
        let [power, reduction] = [packed as u32, (packed >> 32) as u32].map(f32::from_bits);
        (power.is_finite() && reduction.is_finite()).then(|| Self {
            power: f64::from(power),
            reduction: f64::from(reduction),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(level_db: f64, frames: usize) -> Vec<f32> {
        let peak = db_to_gain(level_db) * std::f64::consts::SQRT_2;
        (0..frames)
            .flat_map(|frame| {
                let x =
                    (peak * (std::f64::consts::TAU * 440.0 * frame as f64 / 44_100.0).sin()) as f32;
                [x, x]
            })
            .collect()
    }

    /// The RMS level in dB of the last half of the frames.
    fn level_db(samples: &[f32]) -> f64 {
        let tail = &samples[samples.len() / 2..];
        let power = tail.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / tail.len() as f64;
        10.0 * power.log10()
    }

    fn compress(compressor: &Compressor, input: &[f32]) -> Vec<f32> {
        let mut output = input.to_vec();
        Run::new(compressor).process(&mut output, &mut State::default());
        output
    }

    #[test]
    fn a_level_over_the_threshold_comes_down_by_the_ratio() {
        let compressor = Compressor {
            on: true,
            threshold: -30.0,
            ratio: 4.0,
            attack: 1.0,
            release: 50.0,
            makeup: 0.0,
        };
        // 20 dB over the threshold at 4:1 leaves 5 dB over it.
        let out = level_db(&compress(&compressor, &tone(-10.0, 44_100)));
        assert!((out + 25.0).abs() < 0.5, "{out:.2}");
        // Well under the threshold and the knee, nothing changes.
        let quiet = tone(-50.0, 44_100);
        assert_eq!(compress(&compressor, &quiet), quiet);
        // The makeup gain lifts everything, compressed or not.
        let lifted = Compressor {
            makeup: 6.0,
            ..compressor
        };
        let out = level_db(&compress(&lifted, &quiet));
        assert!((out + 44.0).abs() < 0.1, "{out:.2}");
    }

    #[test]
    fn a_fast_attack_catches_a_transient_that_a_slow_one_lets_through() {
        let mut input = tone(-40.0, 22_050);
        input.extend(tone(-6.0, 4_410));
        let peak_after = |attack: f32| {
            let output = compress(
                &Compressor {
                    on: true,
                    threshold: -30.0,
                    ratio: 10.0,
                    attack,
                    release: 100.0,
                    makeup: 0.0,
                },
                &input,
            );
            // The first 5 ms of the hit.
            output[44_100..44_100 + 441]
                .iter()
                .fold(0.0_f32, |peak, x| peak.max(x.abs()))
        };
        let (fast, slow) = (peak_after(0.1), peak_after(100.0));
        assert!(fast < 0.6 * slow, "fast {fast}, slow {slow}");
    }

    #[test]
    fn settings_validate_their_ranges() {
        let mut compressor = Compressor::default();
        assert!(!compressor.on && compressor.is_default());
        assert_eq!(
            (
                compressor.threshold,
                compressor.ratio,
                compressor.attack,
                compressor.release,
                compressor.makeup
            ),
            (-20.0, 4.0, 10.0, 100.0, 0.0)
        );
        compressor.validate().unwrap();
        for param in CompressorParam::ALL {
            let range = param.range();
            for bad in [range.low - 1.0, range.high + 1.0, f32::NAN, f32::INFINITY] {
                let mut changed = compressor;
                match param {
                    CompressorParam::Threshold => changed.threshold = bad,
                    CompressorParam::Ratio => changed.ratio = bad,
                    CompressorParam::Attack => changed.attack = bad,
                    CompressorParam::Release => changed.release = bad,
                    CompressorParam::Makeup => changed.makeup = bad,
                }
                assert!(changed.validate().is_err(), "{param:?} {bad}");
            }
            let mut held = compressor;
            param.set(&mut held, range.high * 10.0);
            assert_eq!(param.get(&held), range.high);
        }
        CompressorParam::Ratio.set(&mut compressor, 0.5);
        assert_eq!(compressor.ratio, 1.0);
    }
}
