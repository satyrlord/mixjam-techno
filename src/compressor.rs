//! The lane compressor: it evens out its track's level, such as the
//! transients of a drum loop, after the channel EQ and the drive.
//!
//! One detector follows the mean power of both channels, as the master's
//! Bus compressor does, so both get the same gain. Above the threshold a
//! 6 dB soft knee bends the level toward the ratio, the attack and dual-stage
//! program-dependent release smooth the reduction, and the makeup gain
//! follows. It works before the track's width, automation, fader, pan, and
//! ducker, so the fader does not change how hard it compresses. A track whose
//! compressor is off mixes exactly as it did before the compressor existed.
use crate::master::{
    KnobRange,
    dsp::{
        Biquad, Coefficients, DETECTOR, KNEE_DB, fast_db_to_gain, fast_log10, flush, pole,
        reduction,
    },
    range, ratio_range,
};
use serde::{Deserialize, Serialize};
use std::f64::consts::FRAC_1_SQRT_2;

/// Sidechain high-pass cutoff at or below this frequency is bypassed.
pub const SC_HPF_OFF_HZ: f32 = 20.0;

fn default_sc_hpf() -> f32 {
    SC_HPF_OFF_HZ
}
fn is_default_sc_hpf(hz: &f32) -> bool {
    *hz == SC_HPF_OFF_HZ
}

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
    /// Sidechain high-pass cutoff in Hz (`20.0` = bypassed).
    #[serde(default = "default_sc_hpf", skip_serializing_if = "is_default_sc_hpf")]
    pub sc_hpf_hz: f32,
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
            sc_hpf_hz: SC_HPF_OFF_HZ,
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
    ScHpfHz,
}

impl CompressorParam {
    pub const ALL: [Self; 6] = [
        Self::Threshold,
        Self::Ratio,
        Self::Attack,
        Self::Release,
        Self::Makeup,
        Self::ScHpfHz,
    ];

    pub const fn range(self) -> KnobRange {
        match self {
            Self::Threshold => range(-60.0, 0.0, -20.0, 0.5),
            Self::Ratio => ratio_range(1.0, 20.0, 4.0, 0.1),
            Self::Attack => ratio_range(0.1, 100.0, 10.0, 0.1),
            Self::Release => ratio_range(10.0, 1000.0, 100.0, 1.0),
            Self::Makeup => range(0.0, 24.0, 0.0, 0.5),
            Self::ScHpfHz => ratio_range(SC_HPF_OFF_HZ, 250.0, SC_HPF_OFF_HZ, 1.0),
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
            Self::ScHpfHz => "compressor sidechain HPF",
        }
    }

    pub fn get(self, compressor: &Compressor) -> f32 {
        match self {
            Self::Threshold => compressor.threshold,
            Self::Ratio => compressor.ratio,
            Self::Attack => compressor.attack,
            Self::Release => compressor.release,
            Self::Makeup => compressor.makeup,
            Self::ScHpfHz => compressor.sc_hpf_hz,
        }
    }

    /// Set the control, held to its range.
    pub fn set(self, compressor: &mut Compressor, value: f32) {
        let value = self.range().hold(value);
        match self {
            Self::Threshold => compressor.threshold = value,
            Self::Ratio => compressor.ratio = value,
            Self::Attack => compressor.attack = value,
            Self::Release => compressor.release = value,
            Self::Makeup => compressor.makeup = value,
            Self::ScHpfHz => compressor.sc_hpf_hz = value,
        }
    }
}

impl Compressor {
    pub fn validate(&self) -> Result<(), String> {
        if !CompressorParam::ALL
            .into_iter()
            .all(|param| param.range().contains(param.get(self)))
        {
            return Err("A compressor setting is out of range.".into());
        }
        Ok(())
    }

    /// The settings and the switch as a new track has them.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

const SLOW_ATTACK_SECONDS: f64 = 0.040;
const FAST_RELEASE_SCALE: f64 = 0.35;
const SLOW_RELEASE_SCALE: f64 = 2.5;
const MIN_FAST_RELEASE_SECONDS: f64 = 0.005;
const MAX_SLOW_RELEASE_SECONDS: f64 = 2.5;

/// The most frames that `Run::process_stage` takes at once.
const STAGE: usize = 64;

/// A compressor's settings, ready for the audio thread at the mix rate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Run {
    threshold: f64,
    ratio: f64,
    attack2: f64,
    fast_release2: f64,
    slow_release2: f64,
    slow_attack2: f64,
    detector: f64,
    makeup: f64,
    sc_hpf: Option<(f32, Coefficients)>,
}

impl Run {
    pub(crate) fn new(compressor: &Compressor) -> Self {
        let cutoff = compressor.sc_hpf_hz;
        let sc_hpf = (cutoff > SC_HPF_OFF_HZ + 0.5).then(|| {
            (
                cutoff,
                Coefficients::high_pass(f64::from(cutoff), FRAC_1_SQRT_2),
            )
        });
        let release_s = f64::from(compressor.release) / 1000.0;
        let fast_s = (release_s * FAST_RELEASE_SCALE).max(MIN_FAST_RELEASE_SECONDS);
        let slow_s = (release_s * SLOW_RELEASE_SCALE).min(MAX_SLOW_RELEASE_SECONDS);
        let attack = pole(f64::from(compressor.attack) / 1000.0);
        let fast_release = pole(fast_s);
        let slow_release = pole(slow_s);
        let slow_attack = pole(SLOW_ATTACK_SECONDS);
        Self {
            threshold: f64::from(compressor.threshold),
            ratio: f64::from(compressor.ratio),
            attack2: attack * attack,
            fast_release2: fast_release * fast_release,
            slow_release2: slow_release * slow_release,
            slow_attack2: slow_attack * slow_attack,
            detector: pole(DETECTOR),
            makeup: f64::from(compressor.makeup),
            sc_hpf,
        }
    }

    /// The detector's power after each frame of `chunk`, into `powers`.
    fn follow_power(&self, chunk: &[[f32; 2]], powers: &mut [f64], state: &mut State) {
        let mut follow = |sc_l: f64, sc_r: f64| {
            let power = 0.5 * (sc_l * sc_l + sc_r * sc_r);
            state.power = power + (state.power - power) * self.detector;
            state.power
        };
        if self.sc_hpf.is_some() {
            for (power, frame) in powers.iter_mut().zip(chunk) {
                let sc_l = state.sc_filter.process_unflushed(0, f64::from(frame[0]));
                let sc_r = state.sc_filter.process_unflushed(1, f64::from(frame[1]));
                *power = follow(sc_l, sc_r);
            }
        } else {
            for (power, frame) in powers.iter_mut().zip(chunk) {
                *power = follow(f64::from(frame[0]), f64::from(frame[1]));
            }
        }
    }

    /// The reduction after one update toward `target`.
    #[inline]
    fn update(&self, target: f64, state: &mut State) -> f64 {
        let slow_speed = if target > state.slow_reduction {
            self.slow_attack2
        } else {
            self.slow_release2
        };
        state.slow_reduction = target + (state.slow_reduction - target) * slow_speed;
        let speed = if target > state.reduction {
            self.attack2
        } else {
            let sustain = (state.slow_reduction / state.reduction.max(1e-9)).clamp(0.0, 1.0);
            self.fast_release2 + sustain * (self.slow_release2 - self.fast_release2)
        };
        state.reduction = target + (state.reduction - target) * speed;
        state.reduction
    }

    /// Compress up to `STAGE` frames in four passes: the detector's power at
    /// each frame, the target reduction at each frame that updates the gain
    /// (every second frame), the reductions in order, and the gains. Only
    /// the third pass waits on the update before, so the processor overlaps
    /// the frames of the others. Each state takes the same steps in the same
    /// order as frame by frame, so the result does not change.
    fn process_stage(&self, chunk: &mut [[f32; 2]], state: &mut State) {
        let mut values = [0.0_f64; STAGE];
        let values = &mut values[..chunk.len()];
        self.follow_power(chunk, values, state);
        // The frames at an even phase update the gain.
        let first = usize::from(state.phase & 1);
        for value in values.iter_mut().skip(first).step_by(2) {
            let level = 10.0 * fast_log10(value.max(1e-12));
            *value = reduction(level, self.threshold, self.ratio, KNEE_DB);
        }
        for value in values.iter_mut().skip(first).step_by(2) {
            *value = self.update(*value, state);
        }
        for (index, (frame, value)) in chunk.iter_mut().zip(&*values).enumerate() {
            if index % 2 == first {
                state.cached_gain = if *value < 1e-12 && self.makeup == 0.0 {
                    1.0
                } else {
                    fast_db_to_gain(self.makeup - *value)
                };
            }
            let gain = state.cached_gain;
            if gain != 1.0 {
                frame[0] = (f64::from(frame[0]) * gain) as f32;
                frame[1] = (f64::from(frame[1]) * gain) as f32;
            }
        }
        state.phase = state.phase.wrapping_add(chunk.len() as u8);
    }

    /// Compress interleaved stereo frames in place. The state carries from
    /// block to block.
    pub(crate) fn process(&self, frames: &mut [f32], state: &mut State) {
        if let Some((cutoff, coeffs)) = self.sc_hpf {
            if cutoff != state.sc_hpf_hz {
                state.sc_filter.clear();
                state.sc_hpf_hz = cutoff;
            }
            state.sc_filter.set(coeffs);
        } else {
            state.sc_hpf_hz = SC_HPF_OFF_HZ;
        }
        for chunk in frames.as_chunks_mut::<2>().0.chunks_mut(STAGE) {
            self.process_stage(chunk, state);
        }
        if self.sc_hpf.is_some() {
            state.sc_filter.flush_state();
        }
        state.power = flush(state.power);
        if state.slow_reduction < 1e-12 {
            state.slow_reduction = 0.0;
        }
        if state.reduction < 1e-12 {
            state.reduction = 0.0;
        }
    }
}

/// The compressor's history: the detector's power, active and slow-memory
/// reduction in dB, and optional sidechain HPF state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct State {
    power: f64,
    reduction: f64,
    slow_reduction: f64,
    sc_hpf_hz: f32,
    sc_filter: Biquad,
    phase: u8,
    cached_gain: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            power: 0.0,
            reduction: 0.0,
            slow_reduction: 0.0,
            sc_hpf_hz: SC_HPF_OFF_HZ,
            sc_filter: Biquad::FLAT,
            phase: 0,
            cached_gain: 1.0,
        }
    }
}

impl State {
    /// Current gain reduction in dB (`>= 0.0`) for live telemetry.
    pub(crate) fn reduction_db(&self) -> f32 {
        self.reduction as f32
    }

    pub(crate) const WORDS: usize = 6;

    /// Pack the detector, dual-stage release state, and optional sidechain filter history.
    pub(crate) fn pack(self) -> [u64; Self::WORDS] {
        let [power, reduction] = [self.power, self.reduction].map(|value| (value as f32).to_bits());
        let detector = u64::from(power) | (u64::from(reduction) << 32);
        let cutoff_and_slow = u64::from(self.sc_hpf_hz.to_bits())
            | (u64::from((self.slow_reduction as f32).to_bits()) << 32);
        let [[l1, l2], [r1, r2]] = self.sc_filter.history();
        [
            detector,
            cutoff_and_slow,
            l1.to_bits(),
            l2.to_bits(),
            r1.to_bits(),
            r2.to_bits(),
        ]
    }

    /// A history that `pack` stored, if one is there. NaN detector bits mean none.
    pub(crate) fn unpack(
        [detector, cutoff_and_slow, l1, l2, r1, r2]: [u64; Self::WORDS],
    ) -> Option<Self> {
        let [power, reduction] = [detector as u32, (detector >> 32) as u32].map(f32::from_bits);
        let sc_hpf_hz = f32::from_bits(cutoff_and_slow as u32);
        let slow_reduction = f32::from_bits((cutoff_and_slow >> 32) as u32);
        let mut sc_filter = Biquad::FLAT;
        sc_filter.set_history([[l1, l2], [r1, r2]].map(|channel| channel.map(f64::from_bits)));
        (power.is_finite() && reduction.is_finite() && slow_reduction.is_finite()).then_some(Self {
            power: f64::from(power),
            reduction: f64::from(reduction),
            slow_reduction: f64::from(slow_reduction),
            sc_hpf_hz,
            sc_filter,
            phase: 0,
            cached_gain: 1.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master::dsp::db_to_gain;

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
            ..Compressor::default()
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
    fn sidechain_hpf_stops_sub_bass_from_pumping_highs() {
        let frames = 44_100;
        let composite: Vec<f32> = (0..frames)
            .flat_map(|i| {
                let t = i as f64 / 44_100.0;
                let sub = 0.6 * (std::f64::consts::TAU * 45.0 * t).sin();
                let hi = 0.05 * (std::f64::consts::TAU * 5000.0 * t).sin();
                let x = (sub + hi) as f32;
                [x, x]
            })
            .collect();
        let flat = Compressor {
            on: true,
            threshold: -24.0,
            ratio: 8.0,
            attack: 1.0,
            release: 80.0,
            makeup: 0.0,
            sc_hpf_hz: 20.0,
        };
        let filtered = Compressor {
            sc_hpf_hz: 150.0,
            ..flat
        };
        let mut s_flat = State::default();
        let mut s_filt = State::default();
        Run::new(&flat).process(&mut composite.clone(), &mut s_flat);
        Run::new(&filtered).process(&mut composite.clone(), &mut s_filt);
        assert!(
            s_filt.reduction_db() + 2.0 < s_flat.reduction_db(),
            "flat GR {:.2}, sc_hpf GR {:.2}",
            s_flat.reduction_db(),
            s_filt.reduction_db()
        );
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
                    ..Compressor::default()
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
                compressor.makeup,
                compressor.sc_hpf_hz,
            ),
            (-20.0, 4.0, 10.0, 100.0, 0.0, 20.0)
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
                    CompressorParam::ScHpfHz => changed.sc_hpf_hz = bad,
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

    #[test]
    fn dual_stage_release_recovers_faster_from_short_transient_than_sustained_hold() {
        let settings = Compressor {
            on: true,
            threshold: -24.0,
            ratio: 8.0,
            attack: 0.5,
            release: 150.0,
            makeup: 0.0,
            ..Compressor::default()
        };
        let run = Run::new(&settings);
        let measure_gr_after_gap = |hold_frames: usize| {
            let mut input = tone(-6.0, hold_frames);
            input.extend(tone(-45.0, 2_205)); // 50 ms recovery window
            let mut state = State::default();
            run.process(&mut input, &mut state);
            state.reduction_db()
        };
        let after_short = measure_gr_after_gap(220); // ~5 ms transient
        let after_long = measure_gr_after_gap(22_050); // 500 ms sustained hold
        assert!(
            after_short + 1.5 < after_long,
            "short transient GR {after_short:.2} dB should release faster than sustained {after_long:.2} dB"
        );
    }

    #[test]
    fn state_pack_and_unpack_preserve_dual_stage_release_and_sidechain_filter() {
        let settings = Compressor {
            on: true,
            threshold: -20.0,
            ratio: 6.0,
            attack: 1.0,
            release: 120.0,
            makeup: 0.0,
            sc_hpf_hz: 120.0,
        };
        let run = Run::new(&settings);
        let mut state = State::default();
        run.process(&mut tone(-8.0, 2_000), &mut state);
        let words = state.pack();
        let unpacked = State::unpack(words).unwrap();
        assert!((unpacked.reduction_db() - state.reduction_db()).abs() < 1e-5);
        assert!((unpacked.slow_reduction - state.slow_reduction).abs() < 1e-5);
        assert_eq!(unpacked.sc_hpf_hz, state.sc_hpf_hz);
    }
}
