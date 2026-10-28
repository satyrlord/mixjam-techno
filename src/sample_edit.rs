//! New sample editing algorithms behind the recovered manual's operations.
//! The original resampler, time stretcher, and tempo detector are not recovered.
use crate::audio::Pcm;

pub fn trim(input: &Pcm, start_frame: u64, end_frame: u64) -> Result<Pcm, String> {
    input.validate()?;
    if start_frame >= end_frame || end_frame > input.frames() {
        return Err("Select a nonempty trim range within the sample.".into());
    }
    let channels = input.channels as usize;
    Ok(Pcm {
        sample_rate: input.sample_rate,
        channels: input.channels,
        samples: input.samples[start_frame as usize * channels..end_frame as usize * channels]
            .to_vec(),
    })
}

/// An edited sound is saved as a WAV and read again, so it has the WAV limit.
fn fits_a_wav(frames: usize, channels: usize) -> bool {
    frames
        .checked_mul(channels)
        .is_some_and(|samples| samples <= crate::audio::MAX_WAV_SAMPLES)
}

/// Change sample pitch and duration together through linear interpolation.
pub fn pitch_shift(input: &Pcm, semitones: f64) -> Result<Pcm, String> {
    input.validate()?;
    if !semitones.is_finite() || !(-48.0..=48.0).contains(&semitones) {
        return Err("Pitch must be from -48 to 48 semitones.".into());
    }
    if input.frames() == 0 {
        return Err("Choose a nonempty sample.".into());
    }
    if semitones == 0.0 {
        return Ok(input.clone());
    }
    let step = 2.0f64.powf(semitones / 12.0);
    let frames = (input.frames() as f64 / step).round().max(1.0) as usize;
    let channels = input.channels as usize;
    if !fits_a_wav(frames, channels) {
        return Err("The edited sample is too large.".into());
    }
    let source_frames = input.frames() as usize;
    let mut samples = vec![0.0; frames * channels];
    for frame in 0..frames {
        let position = (frame as f64 * step).min((source_frames - 1) as f64);
        let first = position.floor() as usize;
        let next = (first + 1).min(source_frames - 1);
        for channel in 0..channels {
            let a = input.samples[first * channels + channel];
            let b = input.samples[next * channels + channel];
            samples[frame * channels + channel] = a + (b - a) * (position - first as f64) as f32;
        }
    }
    Ok(Pcm {
        sample_rate: input.sample_rate,
        channels: input.channels,
        samples,
    })
}

/// Pitch-preserving overlap-add with waveform-aligned grain selection.
/// Tempo values are supplied by the caller. This does not detect source tempo.
/// Transients and large stretch ratios can produce audible artifacts.
pub fn time_stretch(input: &Pcm, source_bpm: f64, target_bpm: f64) -> Result<Pcm, String> {
    input.validate()?;
    if [source_bpm, target_bpm]
        .iter()
        .any(|bpm| !bpm.is_finite() || !(7.5..=1200.0).contains(bpm))
    {
        return Err("Source and target tempo must be from 7.5 to 1200 BPM.".into());
    }
    let source_frames = input.frames() as usize;
    if source_frames < 8 {
        return Err("The sample is too short for time stretching.".into());
    }
    if source_bpm == target_bpm {
        return Ok(input.clone());
    }
    stretch(input, source_bpm / target_bpm)
}

/// Change pitch by `cents` and keep the length: stretch the sound by the pitch
/// ratio, then resample it back to its length. A sound too short to stretch
/// keeps its pitch.
pub fn repitch(input: &Pcm, cents: i32) -> Result<Pcm, String> {
    input.validate()?;
    if !(-crate::engine::MAX_PITCH_CENTS..=crate::engine::MAX_PITCH_CENTS).contains(&cents) {
        return Err("Pitch must be from -24 to 24 semitones.".into());
    }
    let frames = input.frames() as usize;
    if cents == 0 || frames < 8 {
        return Ok(input.clone());
    }
    let ratio = 2.0f64.powf(f64::from(cents) / 1200.0);
    let stretched = stretch(input, ratio)?;
    let channels = input.channels as usize;
    let last = stretched.frames() as usize - 1;
    let step = stretched.frames() as f64 / frames as f64;
    let at = |frame: isize, channel: usize| {
        stretched.samples[frame.clamp(0, last as isize) as usize * channels + channel]
    };
    let mut samples = Vec::with_capacity(frames * channels);
    for frame in 0..frames {
        let position = frame as f64 * step;
        let index = position.floor() as isize;
        let t = (position - index as f64) as f32;
        for channel in 0..channels {
            // Catmull-Rom interpolation through the four nearest frames.
            let [a, b, c, d] = [-1, 0, 1, 2].map(|offset| at(index + offset, channel));
            let value = b + 0.5
                * t
                * (c - a + t * (2.0 * a - 5.0 * b + 4.0 * c - d + t * (3.0 * (b - c) + d - a)));
            samples.push(value.clamp(-1.0, 1.0));
        }
    }
    Ok(Pcm {
        sample_rate: input.sample_rate,
        channels: input.channels,
        samples,
    })
}

/// The split between the stretcher's two bands, in hertz.
const BAND_SPLIT_HZ: f64 = 150.0;

/// Make `input` `ratio` times as long and keep its pitch.
///
/// A grain must span a period of the tone it carries, or the overlap-add
/// follows the waveform slowly and changes the pitch with the length. Short
/// grains keep transients tight but miss periods longer than about 10 ms, so
/// the sound is split at 150 Hz: the bass band gets grains long enough for
/// tones down to about 30 Hz, the rest keeps the short grains, and the bands
/// sum back to the whole sound.
fn stretch(input: &Pcm, ratio: f64) -> Result<Pcm, String> {
    let source_frames = input.frames() as usize;
    let frames = (source_frames as f64 * ratio).round().max(1.0) as usize;
    let channels = input.channels as usize;
    if !fits_a_wav(frames, channels) {
        return Err("The stretched sample is too large.".into());
    }
    let rate = input.sample_rate as usize;
    let low = zero_phase_low_pass(input, BAND_SPLIT_HZ);
    let high: Vec<f64> = input
        .samples
        .iter()
        .zip(&low)
        .map(|(sample, low)| f64::from(*sample) - low)
        .collect();
    let low = overlap_add(&low, channels, frames, ratio, rate / 15, rate / 60);
    let high = overlap_add(&high, channels, frames, ratio, rate / 50, rate / 200);
    let samples = low
        .iter()
        .zip(&high)
        .map(|(low, high)| (low + high).clamp(-1.0, 1.0) as f32)
        .collect();
    let result = Pcm {
        sample_rate: input.sample_rate,
        channels: input.channels,
        samples,
    };
    result.validate()?;
    Ok(result)
}

/// A second-order Butterworth low-pass run forward and backward, so the band
/// keeps its phase and the rest of the sound is the input minus this band.
fn zero_phase_low_pass(input: &Pcm, cutoff: f64) -> Vec<f64> {
    let channels = input.channels as usize;
    let frames = input.frames() as usize;
    let omega = 2.0 * std::f64::consts::PI * cutoff.min(f64::from(input.sample_rate) * 0.45)
        / f64::from(input.sample_rate);
    let alpha = omega.sin() / std::f64::consts::SQRT_2;
    let cos = omega.cos();
    let a0 = 1.0 + alpha;
    let b0 = (1.0 - cos) / 2.0 / a0;
    let b1 = (1.0 - cos) / a0;
    let a1 = -2.0 * cos / a0;
    let a2 = (1.0 - alpha) / a0;
    let mut band: Vec<f64> = input
        .samples
        .iter()
        .map(|sample| f64::from(*sample))
        .collect();
    for channel in 0..channels {
        for backward in [false, true] {
            let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
            for step in 0..frames {
                let frame = if backward { frames - 1 - step } else { step };
                let index = frame * channels + channel;
                let x = band[index];
                let y = b0 * x + b1 * x1 + b0 * x2 - a1 * y1 - a2 * y2;
                (x2, x1, y2, y1) = (x1, x, y1, y);
                band[index] = y;
            }
        }
    }
    band
}

/// Waveform-aligned overlap-add of interleaved `samples` into `frames` frames:
/// grains of `window` frames, each placed where it best continues the output
/// within `search` frames of its nominal position.
fn overlap_add(
    samples: &[f64],
    channels: usize,
    frames: usize,
    ratio: f64,
    window: usize,
    search: usize,
) -> Vec<f64> {
    let source_frames = samples.len() / channels;
    let window = window.max(32).min(source_frames);
    let hop = (window / 2).max(1);
    let search = search.max(1);
    let mut output = vec![0.0f64; frames * channels];
    let mut weights = vec![0.0f64; frames];
    let window_weights: Vec<_> = (0..window)
        .map(|index| {
            (std::f64::consts::PI * (index as f64 + 0.5) / window as f64)
                .sin()
                .powi(2)
                .max(0.000001)
        })
        .collect();
    for destination in (0..frames).step_by(hop) {
        let nominal = (destination as f64 / ratio).round() as usize;
        let nominal = nominal.min(source_frames - window);
        let overlap = window.min(frames - destination).min(hop);
        let mut best = nominal;
        if destination != 0 && overlap > 4 {
            let first = nominal.saturating_sub(search);
            let last = (nominal + search).min(source_frames - window);
            let correlation = |candidate: usize| {
                let (mut dot, mut a_power, mut b_power) = (0.0, 0.0, 0.0);
                for index in (0..overlap).step_by(8) {
                    let destination_frame = destination + index;
                    if weights[destination_frame] == 0.0 {
                        continue;
                    }
                    for channel in 0..channels {
                        let a = output[destination_frame * channels + channel]
                            / weights[destination_frame];
                        let b = samples[(candidate + index) * channels + channel];
                        dot += a * b;
                        a_power += a * a;
                        b_power += b * b;
                    }
                }
                let magnitude = (a_power * b_power).sqrt();
                if magnitude > 1e-12 {
                    dot / magnitude
                } else {
                    0.0
                }
            };
            let mut best_score = correlation(nominal);
            for candidate in (first..=last).step_by(4) {
                let score = correlation(candidate);
                if score > best_score {
                    best_score = score;
                    best = candidate;
                }
            }
            let refine_start = best.saturating_sub(3).max(first);
            let refine_end = (best + 3).min(last);
            for candidate in refine_start..=refine_end {
                let score = correlation(candidate);
                if score > best_score {
                    best_score = score;
                    best = candidate;
                }
            }
        }
        for index in 0..window.min(frames - destination) {
            let weight = window_weights[index];
            weights[destination + index] += weight;
            for channel in 0..channels {
                output[(destination + index) * channels + channel] +=
                    samples[(best + index) * channels + channel] * weight;
            }
        }
    }
    output
        .iter()
        .enumerate()
        .map(|(index, sample)| sample / weights[index / channels])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_edited_sound_can_be_as_large_as_an_importable_wav() {
        assert!(fits_a_wav(64_000_000, 1));
        assert!(!fits_a_wav(64_000_001, 1));
        assert!(fits_a_wav(32_000_000, 2));
        assert!(!fits_a_wav(32_000_001, 2));
        assert!(!fits_a_wav(usize::MAX, 2));
    }
    fn sine(frames: usize) -> Pcm {
        tone(440.0, frames)
    }
    fn tone(hertz: f32, frames: usize) -> Pcm {
        Pcm {
            sample_rate: 8000,
            channels: 1,
            samples: (0..frames)
                .map(|frame| {
                    (2.0 * std::f32::consts::PI * hertz * frame as f32 / 8000.0).sin() * 0.5
                })
                .collect(),
        }
    }
    fn frequency(pcm: &Pcm) -> f64 {
        let start = pcm.samples.len() / 4;
        let end = pcm.samples.len() * 3 / 4;
        let crossings = pcm.samples[start..end]
            .windows(2)
            .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
            .count();
        crossings as f64 * pcm.sample_rate as f64 / (end - start) as f64
    }
    #[test]
    fn trim_preserves_stereo_frames_and_rejects_invalid_ranges() {
        let input = Pcm {
            sample_rate: 8000,
            channels: 2,
            samples: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
        };
        assert_eq!(
            trim(&input, 1, 3).unwrap().samples,
            vec![0.3, 0.4, 0.5, 0.6]
        );
        assert!(trim(&input, 2, 2).is_err());
        assert!(trim(&input, 0, 4).is_err());
    }
    #[test]
    fn octave_resampling_halves_duration_and_doubles_frequency() {
        let input = sine(8000);
        let output = pitch_shift(&input, 12.0).unwrap();
        assert_eq!(output.frames(), 4000);
        assert!((frequency(&output) - 880.0).abs() < 8.0);
        assert_eq!(pitch_shift(&input, 0.0).unwrap().samples, input.samples);
    }
    #[test]
    fn stretching_changes_duration_and_keeps_tone_pitch() {
        let input = sine(8000);
        for (target, expected_frames) in [(70.0, 16000), (210.0, 5333)] {
            let output = time_stretch(&input, 140.0, target).unwrap();
            assert_eq!(output.frames(), expected_frames);
            assert!(
                (frequency(&output) - 440.0).abs() < 8.0,
                "{}",
                frequency(&output)
            );
        }
    }
    #[test]
    fn repitching_moves_the_tone_and_keeps_the_length() {
        let input = sine(8000);
        for (cents, expected) in [(1200, 880.0), (-1200, 220.0), (700, 659.26), (-35, 431.2)] {
            let output = repitch(&input, cents).unwrap();
            assert_eq!(output.frames(), 8000);
            let measured = frequency(&output);
            assert!((measured - expected).abs() < 3.0, "{cents}: {measured}");
        }
        assert_eq!(repitch(&input, 0).unwrap().samples, input.samples);
        assert!(repitch(&input, 2401).is_err());
    }
    #[test]
    fn bass_tones_move_with_the_pitch_and_keep_it_through_a_stretch() {
        // A grain shorter than the period would leave these tones where they were.
        for (hertz, cents, expected) in
            [(50.0, 219, 56.68), (45.0, -300, 37.84), (40.0, 100, 42.38)]
        {
            let output = repitch(&tone(hertz, 32000), cents).unwrap();
            let measured = frequency(&output);
            assert!(
                (measured - expected).abs() < 1.0,
                "{hertz} Hz {cents}: {measured}"
            );
        }
        let output = time_stretch(&tone(50.0, 32000), 140.0, 100.0).unwrap();
        assert!(
            (frequency(&output) - 50.0).abs() < 1.0,
            "{}",
            frequency(&output)
        );
    }
    #[test]
    fn stretcher_keeps_stereo_channels_isolated_and_rejects_unknown_tempo() {
        let mono = sine(4000);
        let input = Pcm {
            sample_rate: mono.sample_rate,
            channels: 2,
            samples: mono
                .samples
                .iter()
                .flat_map(|sample| [*sample, 0.0])
                .collect(),
        };
        let output = time_stretch(&input, 140.0, 100.0).unwrap();
        assert!(
            output
                .samples
                .iter()
                .skip(1)
                .step_by(2)
                .all(|sample| *sample == 0.0)
        );
        assert!(time_stretch(&input, f64::NAN, 140.0).is_err());
    }
}
