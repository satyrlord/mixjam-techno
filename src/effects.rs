//! Deterministic DSP reconstruction of the controls described by the installed
//! Effect Studio manual. These are new algorithms, not recovered original DSP.
use crate::audio::Pcm;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub cutoff_hz: f32,
    pub resonance: f32,
    pub drive: f32,
    pub wet: f32,
}
impl Default for Filter {
    fn default() -> Self {
        Self {
            cutoff_hz: 2000.0,
            resonance: 0.0,
            drive: 0.0,
            wet: 1.0,
        }
    }
}

/// The shortest echo delay: a sixteenth of a beat. A shorter delay sounds
/// as a comb filter, not as an echo.
pub const MIN_ECHO_BEATS: f32 = 0.0625;

/// The shortest delay that files from before `MIN_ECHO_BEATS` can hold.
/// Such a delay opens at `MIN_ECHO_BEATS`.
const OLD_MIN_ECHO_BEATS: f32 = 0.001;

/// Read an echo delay. A delay from an older file that is shorter than
/// `MIN_ECHO_BEATS` becomes `MIN_ECHO_BEATS`; other values stay as they are,
/// for `EffectsState::validate`.
fn echo_delay<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    Ok(if (OLD_MIN_ECHO_BEATS..MIN_ECHO_BEATS).contains(&value) {
        MIN_ECHO_BEATS
    } else {
        value
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Echo {
    /// Four beats are one bar. The original manual permits up to half a bar.
    #[serde(deserialize_with = "echo_delay")]
    pub delay_beats: f32,
    pub feedback: f32,
    pub wet: f32,
}
impl Default for Echo {
    fn default() -> Self {
        Self {
            delay_beats: 0.5,
            feedback: 0.4,
            wet: 0.35,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Robotize {
    pub frequency_hz: f32,
    pub wet: f32,
}
impl Default for Robotize {
    fn default() -> Self {
        Self {
            frequency_hz: 80.0,
            wet: 0.5,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectsState {
    pub filter: Option<Filter>,
    pub echo: Option<Echo>,
    pub robotize: Option<Robotize>,
    pub dirty: f32,
    pub noise: f32,
    pub volume: f32,
}
impl Default for EffectsState {
    fn default() -> Self {
        Self {
            filter: None,
            echo: None,
            robotize: None,
            dirty: 0.0,
            noise: 0.0,
            volume: 1.0,
        }
    }
}

impl EffectsState {
    pub fn validate(&self) -> Result<(), String> {
        let unit = |x: f32| x.is_finite() && (0.0..=1.0).contains(&x);
        if !unit(self.dirty)
            || !unit(self.noise)
            || !self.volume.is_finite()
            || !(0.0..=2.0).contains(&self.volume)
        {
            return Err("Effect volume or distortion is outside its permitted range.".into());
        }
        // The upper bound is above every usable cutoff: `process` limits the
        // cutoff to 0.45 of the sample rate, which is at most 172.8 kHz.
        if let Some(f) = &self.filter
            && (!f.cutoff_hz.is_finite()
                || !(20.0..=180000.0).contains(&f.cutoff_hz)
                || !unit(f.resonance)
                || !unit(f.drive)
                || !unit(f.wet))
        {
            return Err("Filter values are outside their permitted ranges.".into());
        }
        if let Some(e) = &self.echo
            && (!e.delay_beats.is_finite()
                || !(MIN_ECHO_BEATS..=2.0).contains(&e.delay_beats)
                || !e.feedback.is_finite()
                || !(0.0..=0.95).contains(&e.feedback)
                || !unit(e.wet))
        {
            return Err("Echo delay, feedback, or volume is outside its permitted range.".into());
        }
        if let Some(r) = &self.robotize
            && (!r.frequency_hz.is_finite()
                || !(1.0..=20000.0).contains(&r.frequency_hz)
                || !unit(r.wet))
        {
            return Err("Robotize color or volume is outside its permitted range.".into());
        }
        Ok(())
    }
}

/// Preserve the input duration and channel count. Echo tails are cut at the
/// sample boundary. No effect state is shared between calls or channels.
pub fn process(input: &Pcm, state: &EffectsState, bpm: f64) -> Result<Pcm, String> {
    input.validate()?;
    let output = process_validated(input, state, bpm, 1.0, false)?;
    output.validate()?;
    Ok(output)
}

/// A normalized source with the synth's maximum 12 dB accent stays below
/// four times full scale. Keep that headroom through the effects and the
/// output gain. When the result peaks above full scale, turn all of it down
/// to full scale, so accents and resonance keep their shape and never
/// hard-clip. Returns the result and the gain that it got, at most 1.
///
/// A pattern plays as a loop, so the filter and echo tails at its end wrap
/// into its start: the result is the loop as it sounds on every repeat.
pub(crate) fn process_synth(
    input: &Pcm,
    state: &EffectsState,
    bpm: f64,
) -> Result<(Pcm, f32), String> {
    input.validate_with_peak_limit(4.0)?;
    let mut output = process_validated(input, state, bpm, f32::MAX, true)?;
    let peak = output
        .samples
        .iter()
        .fold(0.0_f32, |peak, x| peak.max(x.abs()));
    let gain = if peak > 1.0 { 1.0 / peak } else { 1.0 };
    if gain < 1.0 {
        for sample in &mut output.samples {
            *sample = (*sample * gain).clamp(-1.0, 1.0);
        }
    }
    output.validate()?;
    Ok((output, gain))
}

/// The effects, with the output held within `ceiling`. With `looped`, the
/// input is one period of a loop: the filter and the echo start in the state
/// that the end of the loop leaves them in, so their tails wrap into the
/// start. The caller validates the result.
fn process_validated(
    input: &Pcm,
    state: &EffectsState,
    bpm: f64,
    ceiling: f32,
    looped: bool,
) -> Result<Pcm, String> {
    state.validate()?;
    if !bpm.is_finite() || !(7.5..=1200.0).contains(&bpm) {
        return Err("Source tempo must be from 7.5 to 1200 BPM.".into());
    }
    // The result is saved as a WAV, so it can be no longer than one holds.
    if input.samples.len() > crate::audio::MAX_WAV_SAMPLES {
        return Err("The sample is too long for a WAV file.".into());
    }
    let mut output = input.clone();
    let channels = input.channels as usize;
    let frames = input.frames() as usize;
    for channel in 0..channels {
        let mut noise_state = 0x6d2b79f5u32.wrapping_add(channel as u32);
        for frame in 0..frames {
            let index = frame * channels + channel;
            if state.noise > 0.0 {
                noise_state ^= noise_state << 13;
                noise_state ^= noise_state >> 17;
                noise_state ^= noise_state << 5;
                let noise = noise_state as f64 / u32::MAX as f64 * 2.0 - 1.0;
                output.samples[index] += output.samples[index].abs() * noise as f32 * state.noise;
            }
        }
        if let Some(filter) = &state.filter
            && filter.wet > 0.0
        {
            let omega = 2.0
                * std::f64::consts::PI
                * (filter.cutoff_hz as f64).min(input.sample_rate as f64 * 0.45)
                / input.sample_rate as f64;
            let cosine = omega.cos();
            let q = 0.5 + 15.0 * filter.resonance as f64;
            let alpha = omega.sin() / (2.0 * q);
            let a0 = 1.0 + alpha;
            let b0 = (1.0 - cosine) / 2.0 / a0;
            let b1 = (1.0 - cosine) / a0;
            let b2 = b0;
            let a1 = -2.0 * cosine / a0;
            let a2 = (1.0 - alpha) / a0;
            let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
            let drive = 1.0 + 20.0 * filter.drive as f64;
            // Low cutoffs with high resonance can ring across several
            // repeats. Let that initial state decay before writing the loop.
            let warm = filter_warm_passes(looped, frames, a2);
            for pass in 0..=warm {
                for frame in 0..frames {
                    let index = frame * channels + channel;
                    let dry = output.samples[index] as f64;
                    let x = if filter.drive > 0.0 {
                        (dry * drive).tanh() / drive.tanh()
                    } else {
                        dry
                    };
                    let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
                    // The synth turns down the whole render later. Keep
                    // resonant peaks intact until that stage. Effect Studio
                    // retains its existing bounded filter state.
                    let y = if looped { y } else { y.clamp(-8.0, 8.0) };
                    x2 = x1;
                    x1 = x;
                    y2 = y1;
                    y1 = y;
                    if pass == warm {
                        output.samples[index] = (dry + (y - dry) * filter.wet as f64) as f32;
                    }
                }
            }
        }
        if let Some(echo) = &state.echo
            && echo.wet > 0.0
        {
            let delay_frames = (input.sample_rate as f64 * 60.0 / bpm * echo.delay_beats as f64)
                .round()
                .max(1.0) as usize;
            let mut delay = vec![0.0; delay_frames];
            let mut at = 0;
            // A loop first fills the delay line as the loop's earlier repeats
            // leave it, until their echoes are below -100 dB.
            for _ in 0..echo_warm_passes(looped, echo.feedback, frames, delay_frames) {
                for frame in 0..frames {
                    let dry = output.samples[frame * channels + channel];
                    delay[at] = dry + delay[at] * echo.feedback;
                    at = (at + 1) % delay_frames;
                }
            }
            for frame in 0..frames {
                let index = frame * channels + channel;
                let dry = output.samples[index];
                let delayed = delay[at];
                delay[at] = dry + delayed * echo.feedback;
                at = (at + 1) % delay_frames;
                output.samples[index] = dry + (delayed - dry) * echo.wet;
            }
        }
        for frame in 0..frames {
            let index = frame * channels + channel;
            let mut value = output.samples[index];
            if let Some(robot) = &state.robotize {
                let frequency = (robot.frequency_hz as f64).min(input.sample_rate as f64 * 0.45);
                let carrier = (2.0 * std::f64::consts::PI * frequency * frame as f64
                    / input.sample_rate as f64)
                    .sin() as f32;
                value += (value * carrier - value) * robot.wet;
            }
            if state.dirty > 0.0 {
                let drive = 1.0 + 40.0 * state.dirty;
                value = (value * drive).tanh() / drive.tanh();
            }
            output.samples[index] = (value * state.volume).clamp(-ceiling, ceiling);
        }
    }
    Ok(output)
}

/// The low-pass poles have radius `sqrt(a2)` for the supported Q >= 0.5.
/// Run whole repeats until their initial state has decayed below -140 dB.
/// A synth render is at least a bar, so even the lowest cutoff and highest
/// resonance need only a few passes. Bound tiny internal test buffers too.
fn filter_warm_passes(looped: bool, frames: usize, a2: f64) -> usize {
    if !looped || frames == 0 {
        return 0;
    }
    let decay = 0.5 * a2.max(f64::MIN_POSITIVE).ln() * frames as f64;
    ((1e-7_f64.ln() / decay).ceil() as usize).clamp(1, 256)
}

/// How many times a loop of `frames` runs through the echo before the
/// result, so that the echoes of its earlier repeats are in the delay line.
/// Each repeat passes the delay `frames / delay_frames` times, and each pass
/// scales the echo by `feedback`. A pattern is at least four beats and the
/// delay at most two, so a repeat passes the delay at least twice.
fn echo_warm_passes(looped: bool, feedback: f32, frames: usize, delay_frames: usize) -> usize {
    if !looped || frames == 0 {
        return 0;
    }
    if feedback <= 0.0 {
        return 1;
    }
    let trips = (frames as f64 / delay_frames as f64).max(1.0);
    let per_pass = f64::from(feedback).ln() * trips;
    // -100 dB. The feedback is at most 0.95, so this is at most 225 passes.
    ((1e-5_f64.ln() / per_pass).ceil() as usize).clamp(1, 256)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_looped_resonant_filter_settles_before_the_render() {
        for amplitude in [0.125, 4.0] {
            check_looped_resonant_filter(amplitude);
        }
    }

    #[test]
    fn synth_filter_headroom_preserves_the_normalized_waveform() {
        let input = Pcm {
            sample_rate: 8000,
            channels: 1,
            samples: (0..16000)
                .map(|frame| {
                    (0.125 * (2.0 * std::f64::consts::PI * 600.0 * f64::from(frame) / 8000.0).sin())
                        as f32
                })
                .collect(),
        };
        let state = EffectsState {
            filter: Some(Filter {
                cutoff_hz: 600.0,
                resonance: 1.0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let (quiet, quiet_gain) = process_synth(&input, &state, 120.0).unwrap();
        let loud = Pcm {
            samples: input.samples.iter().map(|sample| sample * 32.0).collect(),
            ..input
        };
        let (loud, loud_gain) = process_synth(&loud, &state, 120.0).unwrap();
        assert!(quiet_gain < 1.0 && loud_gain < quiet_gain);
        let error = quiet
            .samples
            .iter()
            .zip(loud.samples)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            error < 1e-5,
            "Filter headroom changes the normalized waveform: {error}"
        );
    }

    fn check_looped_resonant_filter(amplitude: f64) {
        let sample_rate = 8000;
        let frames = (f64::from(sample_rate) * 60.0 / 180.0 * 4.0).round() as usize;
        let mut samples = vec![0.0; frames];
        // Excite the slowest supported filter near the end of a short loop.
        // One earlier repeat does not let this resonance settle.
        for (index, sample) in samples[frames - 667..].iter_mut().enumerate() {
            *sample = (amplitude
                * (2.0 * std::f64::consts::PI * 20.0 * index as f64 / f64::from(sample_rate)).sin())
                as f32;
        }
        let input = Pcm {
            sample_rate,
            channels: 1,
            samples,
        };
        let state = EffectsState {
            filter: Some(Filter {
                cutoff_hz: 20.0,
                resonance: 1.0,
                ..Default::default()
            }),
            ..Default::default()
        };
        let (once, _) = process_synth(&input, &state, 180.0).unwrap();
        let repeated = Pcm {
            samples: input.samples.repeat(8),
            ..input
        };
        let (settled, _) = process_synth(&repeated, &state, 180.0).unwrap();
        let error = once
            .samples
            .iter()
            .zip(&settled.samples[7 * frames..])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            error < 1e-5,
            "The loop retains a filter transient at amplitude {amplitude}: {error}"
        );
    }

    #[test]
    fn a_looped_echo_wraps_its_tail_into_the_start() {
        // 120 BPM at 1000 Hz: a beat is 500 frames. A two-beat loop with one
        // click near its end, and an echo of half a beat.
        let mut samples = vec![0.0; 1000];
        samples[900] = 1.0;
        let input = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples,
        };
        let state = EffectsState {
            echo: Some(Echo {
                delay_beats: 0.5,
                feedback: 0.5,
                wet: 0.5,
            }),
            ..Default::default()
        };
        let cut = process(&input, &state, 120.0).unwrap();
        assert!(
            cut.samples[..900].iter().all(|x| *x == 0.0),
            "An Effect Studio sample cuts its tail."
        );
        let (looped, _) = process_synth(&input, &state, 120.0).unwrap();
        // The echo of the click, 250 frames on, wraps to frame 150.
        assert!(
            // It holds the echoes of every earlier repeat: 0.5 / (1 - 0.5^4).
            (looped.samples[150] - 0.5 / (1.0 - 0.5_f32.powi(4))).abs() < 1e-3,
            "{}",
            looped.samples[150]
        );
        // The second echo, of the earlier repeats too, adds at frame 400.
        assert!(looped.samples[400] > 0.2, "{}", looped.samples[400]);
        // Each repeat ends as the one before: the loop is seamless.
        let (twice, _) = process_synth(
            &Pcm {
                samples: [input.samples.clone(), input.samples.clone()].concat(),
                ..input.clone()
            },
            &state,
            120.0,
        )
        .unwrap();
        for (a, b) in looped.samples.iter().zip(&twice.samples[1000..]) {
            assert!((a - b).abs() < 1e-4, "{a} {b}");
        }
    }

    #[test]
    fn an_older_echo_delay_opens_at_the_minimum() {
        let echo: Echo =
            serde_json::from_str(r#"{"delay_beats":0.01,"feedback":0.4,"wet":0.35}"#).unwrap();
        assert_eq!(echo.delay_beats, MIN_ECHO_BEATS);
        let echo: Echo =
            serde_json::from_str(r#"{"delay_beats":0.25,"feedback":0.4,"wet":0.35}"#).unwrap();
        assert_eq!(echo.delay_beats, 0.25);
        let state = EffectsState {
            echo: Some(Echo {
                delay_beats: 0.03,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(
            state.validate().is_err(),
            "A new delay under the minimum is refused."
        );
    }

    #[test]
    fn synth_headroom_is_bounded_and_does_not_relax_public_pcm() {
        let mut source = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![4.0],
        };
        let state = EffectsState {
            volume: 0.2,
            ..Default::default()
        };
        assert!(source.validate().is_err());
        assert!(process(&source, &state, 120.0).is_err());
        let (output, gain) = process_synth(&source, &state, 120.0).unwrap();
        assert_eq!((output.samples, gain), (vec![0.8], 1.0));
        let loud = EffectsState {
            volume: 0.5,
            ..Default::default()
        };
        let (output, gain) = process_synth(&source, &loud, 120.0).unwrap();
        assert_eq!(
            output.samples,
            vec![1.0],
            "A loud result is turned down, not clipped."
        );
        assert_eq!(gain, 0.5);
        for invalid in [4.01, f32::INFINITY, f32::NAN] {
            source.samples[0] = invalid;
            assert!(process_synth(&source, &state, 120.0).is_err());
        }
    }

    #[test]
    fn bypass_preserves_audio_and_each_call_has_fresh_state() {
        let source = Pcm {
            sample_rate: 1000,
            channels: 2,
            samples: vec![0.25, -0.5, 0.0, 0.75],
        };
        assert_eq!(
            process(&source, &EffectsState::default(), 140.0)
                .unwrap()
                .samples,
            source.samples
        );
        let state = EffectsState {
            noise: 0.8,
            ..Default::default()
        };
        assert_eq!(
            process(&source, &state, 140.0).unwrap().samples,
            process(&source, &state, 140.0).unwrap().samples
        );
    }
    #[test]
    fn echo_has_measured_delay_feedback_and_channel_isolation() {
        let mut source = Pcm {
            sample_rate: 1000,
            channels: 2,
            samples: vec![0.0; 1400],
        };
        source.samples[0] = 1.0;
        let state = EffectsState {
            echo: Some(Echo {
                delay_beats: 0.5,
                feedback: 0.5,
                wet: 1.0,
            }),
            ..Default::default()
        };
        let output = process(&source, &state, 120.0).unwrap();
        assert_eq!(output.samples[0], 0.0);
        assert_eq!(output.samples[250 * 2], 1.0);
        assert_eq!(output.samples[500 * 2], 0.5);
        assert!(output.samples.iter().skip(1).step_by(2).all(|x| *x == 0.0));
    }
    #[test]
    fn resonant_filter_and_distortion_remain_finite_and_bounded() {
        let source = Pcm {
            sample_rate: 44100,
            channels: 1,
            samples: vec![1.0; 20000],
        };
        let state = EffectsState {
            filter: Some(Filter {
                cutoff_hz: 20.0,
                resonance: 1.0,
                drive: 1.0,
                wet: 1.0,
            }),
            dirty: 1.0,
            noise: 1.0,
            volume: 2.0,
            ..Default::default()
        };
        assert!(
            process(&source, &state, 140.0)
                .unwrap()
                .samples
                .iter()
                .all(|x| x.is_finite() && x.abs() <= 1.0)
        );
        let invalid = EffectsState {
            echo: Some(Echo {
                feedback: 1.0,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(process(&source, &invalid, 140.0).is_err());
    }
}
