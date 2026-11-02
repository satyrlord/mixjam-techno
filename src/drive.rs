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
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    const RATE: f64 = 44_100.0;

    /// A stereo tone of `hz` with a peak of `peak`, `frames` long.
    fn tone(hz: f64, peak: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|frame| {
                let x = (peak * (TAU * hz * frame as f64 / RATE).sin()) as f32;
                [x, x]
            })
            .collect()
    }

    /// The input with the drive's delay taken off: frame `n` of the result
    /// is the output for input frame `n`.
    fn drive(drive: &Drive, input: &[f32]) -> Vec<f32> {
        let run = Run::new(drive);
        let mut state = State::new(&run);
        let mut output = input.to_vec();
        output.extend(std::iter::repeat_n(0.0, LATENCY * 2));
        run.process(&mut output, &mut state);
        output.drain(..LATENCY * 2);
        output
    }

    /// The amplitude of `hz` in the left channel of whole seconds of frames,
    /// by correlation.
    fn amplitude(samples: &[f32], hz: f64) -> f64 {
        let frames = samples.len() / 2;
        let (mut re, mut im) = (0.0, 0.0);
        for frame in 0..frames {
            let phase = TAU * hz * frame as f64 / RATE;
            let x = f64::from(samples[frame * 2]);
            re += x * phase.cos();
            im += x * phase.sin();
        }
        2.0 * (re * re + im * im).sqrt() / frames as f64
    }

    fn db(value: f64) -> f64 {
        20.0 * value.log10()
    }

    /// The mean power of the left channel, in dB.
    fn power_db(samples: &[f32]) -> f64 {
        let left: Vec<f64> = samples.iter().step_by(2).map(|x| f64::from(*x)).collect();
        10.0 * (left.iter().map(|x| x * x).sum::<f64>() / left.len() as f64).log10()
    }

    /// The last two seconds of three, past the detectors' settling.
    fn settled(samples: &[f32]) -> &[f32] {
        &samples[44_100 * 2..]
    }

    fn on() -> Drive {
        Drive {
            on: true,
            ..Drive::default()
        }
    }

    #[test]
    fn cached_curve_means_match_uncached_means_across_small_steps() {
        for character in [0.0, 0.25, 0.5, 1.0] {
            let mut mean = Mean::default();
            let mut previous = 0.0;
            // Repeated and close samples take the midpoint branch. The
            // wider step after them must calculate the correct endpoint,
            // including a return to silence and a crossing through zero.
            for next in [
                0.0,
                0.0,
                1e-7,
                0.5,
                0.500_000_1,
                -0.5,
                -0.500_000_1,
                10.0,
                -10.0,
                0.0,
                0.0,
            ] {
                let distance: f64 = next - previous;
                let expected = if distance.abs() < 1e-5 {
                    curve(0.5 * (previous + next), character)
                } else {
                    (integral(next, character) - integral(previous, character)) / distance
                };
                assert_eq!(mean.next(next, character).to_bits(), expected.to_bits());
                previous = next;
            }
        }
    }

    #[test]
    fn initial_silence_fast_path_matches_full_processing_bit_for_bit() {
        for prefix in [0, 1, 68, 69, 70, 140, 1024] {
            for negative_zero in [false, true] {
                let mut input = vec![0.0_f32; prefix * 2];
                if negative_zero && !input.is_empty() {
                    input[0] = -0.0;
                }
                // Start in one channel, then both. Leave a silent tail
                // after the sound so it must keep processing its histories.
                input.extend([0.8, 0.0]);
                input.extend(tone(220.0, 0.5, 512));
                input.extend(std::iter::repeat_n(0.0, 1024 * 2));
                for mix in [0.0, 50.0, 100.0] {
                    let run = Run::new(&Drive {
                        amount: 24.0,
                        character: 100.0,
                        mix,
                        ..on()
                    });
                    let mut state = State::new(&run);
                    let mut full = State {
                        started: true,
                        ..State::new(&run)
                    };
                    let mut actual = input.clone();
                    let mut expected = input.clone();
                    // A prefix can end inside a block or at its boundary.
                    for (actual, expected) in actual.chunks_mut(34).zip(expected.chunks_mut(34)) {
                        run.process(actual, &mut state);
                        run.process(expected, &mut full);
                    }
                    assert!(state.started);
                    let mismatch = actual
                        .iter()
                        .zip(&expected)
                        .position(|(a, b)| a.to_bits() != b.to_bits());
                    assert!(
                        mismatch.is_none(),
                        "prefix {prefix}, negative zero {negative_zero}, mix {mix}: {mismatch:?}"
                    );
                    assert!(actual[(prefix + 513) * 2..].iter().any(|x| *x != 0.0));
                }
            }
        }
        let run = Run::new(&on());
        let mut state = State::new(&run);
        run.process(&mut [0.0; 1024], &mut state);
        assert!(!state.started);
        run.process(&mut [-0.0, 0.0], &mut state);
        assert!(state.started);
    }

    #[test]
    fn settings_validate_their_ranges() {
        let mut drive = Drive::default();
        assert!(!drive.on && drive.is_default());
        assert_eq!(
            (
                drive.amount,
                drive.character,
                drive.mix,
                drive.focus,
                drive.output
            ),
            (12.0, 25.0, 50.0, 100.0, 0.0)
        );
        drive.validate().unwrap();
        for param in DriveParam::ALL {
            let range = param.range();
            for bad in [range.low - 1.0, range.high + 1.0, f32::NAN, f32::INFINITY] {
                let mut changed = drive;
                match param {
                    DriveParam::Amount => changed.amount = bad,
                    DriveParam::Character => changed.character = bad,
                    DriveParam::Mix => changed.mix = bad,
                    DriveParam::Focus => changed.focus = bad,
                    DriveParam::Output => changed.output = bad,
                }
                assert!(changed.validate().is_err(), "{param:?} {bad}");
            }
            let mut held = drive;
            param.set(&mut held, range.high * 10.0);
            assert_eq!(param.get(&held), range.high);
        }
        DriveParam::Focus.set(&mut drive, 5.0);
        assert_eq!(drive.focus, 20.0);
    }

    #[test]
    fn a_zero_mix_passes_the_track_bit_for_bit() {
        let input = tone(55.0, 0.5, 44_100);
        let output = drive(
            &Drive {
                mix: 0.0,
                amount: 24.0,
                character: 100.0,
                ..on()
            },
            &input,
        );
        assert!(output == input);
    }

    #[test]
    fn playback_qualities_keep_delay_stereo_identity_and_block_history() {
        let mut input = tone(55.0, 0.6, 4096);
        // Independent channels, an initial impulse, and a source gap.
        input[..2].copy_from_slice(&[0.8, -0.4]);
        for frame in input.as_chunks_mut::<2>().0 {
            frame[1] *= -0.5;
        }
        input[1024..2048].fill(0.0);
        for quality in [Oversampling::FourX, Oversampling::Off] {
            for mix in [0.0, 100.0] {
                let settings = Drive {
                    amount: 24.0,
                    character: 100.0,
                    mix,
                    ..on()
                };
                let run = Run::with_quality(&settings, quality);
                let mut state = State::new(&run);
                let mut whole = input.clone();
                whole.extend([0.0; LATENCY * 2]);
                run.process(&mut whole, &mut state);
                if quality == Oversampling::Off || mix == 0.0 {
                    assert!(whole[..LATENCY * 2].iter().all(|x| *x == 0.0));
                }
                let mut blocked = input.clone();
                blocked.extend([0.0; LATENCY * 2]);
                let mut state = State::new(&run);
                for block in blocked.chunks_mut(34) {
                    run.process(block, &mut state);
                }
                assert_eq!(whole, blocked, "{quality:?} mix {mix}");
                if mix == 0.0 {
                    assert_eq!(&whole[LATENCY * 2..], input);
                } else {
                    assert_ne!(&whole[LATENCY * 2..], input, "Off must still saturate");
                    assert!(whole.iter().all(|x| x.is_finite()));
                }
                if quality == Oversampling::FourX {
                    let mut legacy = input.clone();
                    legacy.extend([0.0; LATENCY * 2]);
                    let legacy_run = Run::new(&settings);
                    legacy_run.process(&mut legacy, &mut State::new(&legacy_run));
                    assert_eq!(whole, legacy);
                }
            }
        }
    }

    #[test]
    fn base_rate_curve_and_reference_have_the_same_half_frame_phase() {
        let input = tone(10_000.0, 1e-8, 44_100);
        // The reference is the straight line's interval mean at the base
        // rate, not the undelayed current sample. Integer compensation does
        // not remove its half-frame phase or its cos(pi*f/rate) roll-off.
        let mut held = 0.0_f32;
        let mut mean = Mean::default();
        let mut pair_output = Vec::new();
        for (frame, samples) in input.as_chunks::<2>().0.iter().enumerate() {
            let pair = mean.pair(samples[0], &mut held, 1.0, 1.0, 0.0);
            let previous = if frame == 0 {
                0.0
            } else {
                input[(frame - 1) * 2]
            };
            let expected = 0.5 * (previous + samples[0]);
            assert!((pair[0] - expected).abs() < 2e-15);
            assert_eq!(pair[1], expected);
            pair_output.extend([pair[1], pair[1]]);
        }
        let magnitude = amplitude(&pair_output, 10_000.0) / amplitude(&input, 10_000.0);
        let expected = (std::f64::consts::PI * 10_000.0 / RATE).cos();
        assert!((magnitude - expected).abs() < 1e-4);
    }

    #[test]
    fn playback_quality_records_alias_and_frequency_response_tradeoffs() {
        let input = tone(5000.0, 0.9, 2 * 44_100);
        let settings = Drive {
            amount: 24.0,
            character: 0.0,
            mix: 100.0,
            focus: 20.0,
            ..on()
        };
        let mut aliases = [0.0; 2];
        for (index, quality) in [Oversampling::FourX, Oversampling::Off]
            .into_iter()
            .enumerate()
        {
            let run = Run::with_quality(&settings, quality);
            let mut state = State::new(&run);
            let mut output = input.clone();
            output.extend([0.0; LATENCY * 2]);
            run.process(&mut output, &mut state);
            let output = &output[(44_100 + LATENCY) * 2..];
            let fundamental = amplitude(output, 5000.0);
            // The fifth harmonic above Nyquist folds from 25 to 19.1 kHz.
            aliases[index] = db(amplitude(output, 19_100.0) / fundamental);
            println!(
                "Drive {quality:?}: 5 kHz tone, 24 dB Drive, 19.1 kHz alias {:.2} dBc",
                aliases[index]
            );
        }
        assert!(aliases[0] < -65.0);
        assert!(aliases[1] > aliases[0] + 10.0);
        for hz in [55.0, 1000.0, 5000.0, 10_000.0, 19_000.0] {
            let reference_db = db((std::f64::consts::PI * hz / RATE).cos());
            println!(
                "Drive Off interval-mean reference at {hz} Hz: {reference_db:.3} dB, half-frame phase lag"
            );
        }
    }

    #[test]
    fn a_held_55_hz_tone_gains_harmonics_and_keeps_its_fundamental() {
        let input = tone(55.0, 0.5, 3 * 44_100);
        let clean = settled(&input);
        let fundamental = amplitude(clean, 55.0);
        // A pure tone has no harmonics to speak of.
        for harmonic in [110.0, 165.0] {
            assert!(db(amplitude(clean, harmonic) / fundamental) < -100.0);
        }
        // Parallel harmonics: the focus at 110 Hz, twice the note, keeps
        // the fundamental clean. Symmetric gives the 3rd, not the 2nd.
        let symmetric = Drive {
            amount: 24.0,
            character: 0.0,
            mix: 100.0,
            focus: 110.0,
            ..on()
        };
        let output = drive(&symmetric, &input);
        let output = settled(&output);
        let kept = db(amplitude(output, 55.0) / fundamental);
        assert!(kept.abs() < 0.75, "fundamental {kept:.2} dB");
        let second = db(amplitude(output, 110.0) / fundamental);
        let third = db(amplitude(output, 165.0) / fundamental);
        assert!(second < -60.0, "2nd {second:.1} dB");
        assert!((third + 10.7).abs() < 1.5, "3rd {third:.1} dB");
        // One-sided, the 2nd leads. It sits on the focus, 3 dB down from
        // the -11 dB that the curve makes.
        let even = Drive {
            character: 100.0,
            ..symmetric
        };
        let output = drive(&even, &input);
        let output = settled(&output);
        let kept = db(amplitude(output, 55.0) / fundamental);
        assert!(kept.abs() < 0.75, "fundamental {kept:.2} dB");
        let second = db(amplitude(output, 110.0) / fundamental);
        let third = db(amplitude(output, 165.0) / fundamental);
        assert!((second + 13.5).abs() < 2.0, "2nd {second:.1} dB");
        assert!(
            second > third + 6.0,
            "2nd {second:.1} dB, 3rd {third:.1} dB"
        );
        // Half the mix adds half the harmonics: 6 dB less.
        let half = drive(
            &Drive {
                mix: 50.0,
                ..symmetric
            },
            &input,
        );
        let third = db(amplitude(settled(&half), 165.0) / fundamental);
        assert!((third + 16.7).abs() < 1.5, "3rd {third:.1} dB");
    }

    #[test]
    fn the_drive_keeps_the_track_power() {
        for (hz, peak) in [(49.0, 0.5), (87.0, 0.2), (440.0, 0.7)] {
            let input = tone(hz, peak, 3 * 44_100);
            let before = power_db(settled(&input));
            for (amount, character, mix, focus) in [
                (12.0, 25.0, 50.0, 100.0),
                (24.0, 0.0, 100.0, 20.0),
                (24.0, 100.0, 100.0, 150.0),
                (18.0, 60.0, 30.0, 400.0),
            ] {
                let settings = Drive {
                    amount,
                    character,
                    mix,
                    focus,
                    ..on()
                };
                let after = power_db(settled(&drive(&settings, &input)));
                assert!(
                    (after - before).abs() < 0.2,
                    "{hz} Hz {settings:?}: {:.2} dB",
                    after - before
                );
                // The output trim moves the level by hand.
                let trimmed = power_db(settled(&drive(
                    &Drive {
                        output: -6.0,
                        ..settings
                    },
                    &input,
                )));
                assert!((trimmed - after + 6.0).abs() < 0.01, "{trimmed:.2}");
            }
        }
    }

    #[test]
    fn the_result_carries_no_offset() {
        let input = tone(55.0, 0.8, 3 * 44_100);
        let output = drive(
            &Drive {
                amount: 24.0,
                character: 100.0,
                mix: 100.0,
                focus: 20.0,
                ..on()
            },
            &input,
        );
        let output = settled(&output);
        let left: Vec<f64> = output.iter().step_by(2).map(|x| f64::from(*x)).collect();
        let mean = left.iter().sum::<f64>() / left.len() as f64;
        assert!(mean.abs() < 1e-4, "offset {mean:e}");
        // Without the blocker the one-sided curve's offset would be about a
        // third of the peak.
        let peak = left.iter().fold(0.0_f64, |peak, x| peak.max(x.abs()));
        assert!(peak > 0.3, "{peak}");
    }

    #[test]
    fn high_notes_driven_hard_do_not_alias() {
        // 3 kHz and 5 kHz at 1 dB under full scale, driven as hard as the
        // drive goes, with each character. Their harmonics are multiples of
        // the note; an alias would land between them. Plain four-times
        // oversampling leaves the 33rd harmonic of 5 kHz at -45 dB, folded
        // to 11.4 kHz.
        for hz in [3000.0, 5000.0] {
            let input = tone(hz, 0.9, 3 * 44_100);
            for character in [0.0, 50.0, 100.0] {
                let output = drive(
                    &Drive {
                        amount: 24.0,
                        character,
                        mix: 100.0,
                        focus: 20.0,
                        ..on()
                    },
                    &input,
                );
                let output = settled(&output);
                let fundamental = amplitude(output, hz);
                // Every 100 Hz from 100 Hz to 20 kHz that is not a harmonic.
                let worst = (1..=200)
                    .map(|step| f64::from(step) * 100.0)
                    .filter(|frequency| (frequency / hz).fract().abs() > 1e-9)
                    .map(|frequency| db(amplitude(output, frequency) / fundamental))
                    .fold(f64::NEG_INFINITY, f64::max);
                assert!(worst < -65.0, "{hz} Hz {character}%: alias {worst:.1} dB");
                // The harmonics are there.
                let third = db(amplitude(output, 3.0 * hz) / fundamental);
                assert!(third > -35.0, "{hz} Hz {character}%: 3rd {third:.1} dB");
            }
        }
    }

    #[test]
    fn the_gains_follow_level_jumps_without_pumping() {
        // A 50 Hz bass, whose cycle is 882 frames, that jumps 12 dB up and
        // then 24 dB down at the start of a cycle.
        let mut input = tone(50.0, 0.125, 44_100);
        input.extend(tone(50.0, 0.5, 44_100));
        input.extend(tone(50.0, 0.03, 44_100));
        let output = drive(
            &Drive {
                amount: 24.0,
                mix: 100.0,
                focus: 100.0,
                ..on()
            },
            &input,
        );
        // Cycle by cycle, the result keeps the clean level as it moves. The
        // gains glide: within 100 ms of a jump the level settles, and on the
        // way it moves by at most a few dB, without a click.
        let changes: Vec<f64> = (0..150)
            .map(|cycle| {
                let frames = cycle * 882 * 2..(cycle + 1) * 882 * 2;
                power_db(&output[frames.clone()]) - power_db(&input[frames])
            })
            .collect();
        for (cycle, change) in changes.iter().enumerate() {
            let settling = [50, 100]
                .iter()
                .any(|jump| (*jump..jump + 5).contains(&cycle));
            let bound = if settling { 3.0 } else { 0.5 };
            assert!(change.abs() < bound, "cycle {cycle}: {changes:.2?} dB");
        }
    }

    #[test]
    fn nearby_settings_sound_nearby() {
        // Each control, swept in 48 steps across its range, changes the
        // harmonics and the level a little at a time, with no jump.
        let input = tone(50.0, 0.5, 44_100);
        for param in DriveParam::ALL {
            let range = param.range();
            let mut sweep = Vec::new();
            for index in 0..=48 {
                let mut settings = Drive {
                    focus: 100.0,
                    ..on()
                };
                let position = f64::from(index) / 48.0;
                let value = if range.logarithmic {
                    f64::from(range.low) * f64::from(range.high / range.low).powf(position)
                } else {
                    f64::from(range.low) + f64::from(range.high - range.low) * position
                };
                param.set(&mut settings, value as f32);
                let output = drive(&settings, &input);
                // The last half second: 25 whole cycles.
                let output = &output[44_100..];
                let total = 10.0_f64.powf(power_db(output) / 10.0);
                let fundamental = amplitude(output, 50.0).powi(2) / 2.0;
                let harmonics = (total - fundamental).max(0.0).sqrt();
                sweep.push((value, power_db(output), harmonics));
            }
            let most = sweep.iter().fold(0.0_f64, |most, step| most.max(step.2));
            for pair in sweep.windows(2) {
                let [(_, level, harmonics), (value, next_level, next)] = [pair[0], pair[1]];
                assert!(
                    (next - harmonics).abs() <= 0.15 * most,
                    "{param:?} at {value:.1}: harmonics {harmonics:.4} to {next:.4}"
                );
                assert!(
                    (next_level - level).abs() < 0.75,
                    "{param:?} at {value:.1}: {level:.2} to {next_level:.2} dB"
                );
            }
        }
    }
}
