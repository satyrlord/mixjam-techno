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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Echo {
    /// Four beats are one bar. The original manual permits up to half a bar.
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
                || !(0.001..=2.0).contains(&e.delay_beats)
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
    state.validate()?;
    if !bpm.is_finite() || !(7.5..=1200.0).contains(&bpm) {
        return Err("Source tempo must be from 7.5 to 1200 BPM.".into());
    }
    if input.frames() > 64_000_000 {
        return Err("The sample is too large for effect processing.".into());
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
            for frame in 0..frames {
                let index = frame * channels + channel;
                let dry = output.samples[index] as f64;
                let x = if filter.drive > 0.0 {
                    (dry * drive).tanh() / drive.tanh()
                } else {
                    dry
                };
                let y = (b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2).clamp(-8.0, 8.0);
                x2 = x1;
                x1 = x;
                y2 = y1;
                y1 = y;
                output.samples[index] = (dry + (y - dry) * filter.wet as f64) as f32;
            }
        }
        if let Some(echo) = &state.echo
            && echo.wet > 0.0
        {
            let delay_frames = (input.sample_rate as f64 * 60.0 / bpm * echo.delay_beats as f64)
                .round()
                .max(1.0) as usize;
            let mut delay = vec![0.0; delay_frames];
            for frame in 0..frames {
                let index = frame * channels + channel;
                let dry = output.samples[index];
                let delayed = delay[frame % delay_frames];
                delay[frame % delay_frames] = dry + delayed * echo.feedback;
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
            output.samples[index] = (value * state.volume).clamp(-1.0, 1.0);
        }
    }
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
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
