//! Modern input capture with the original manual's two-bar count-in.
use crate::audio::Pcm;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

pub fn count_in(bpm: f64) -> Result<Pcm, String> {
    if !bpm.is_finite() || !(7.5..=1200.0).contains(&bpm) {
        return Err("Effective tempo must be from 7.5 to 1200 BPM.".into());
    }
    let sample_rate = 44_100;
    let beat_frames = sample_rate as f64 * 60.0 / bpm;
    let mut samples = vec![0.0; (beat_frames * 8.0).round() as usize];
    for beat in 0..8 {
        let start = (beat as f64 * beat_frames).round() as usize;
        let frequency = if beat % 4 == 0 { 2200.0 } else { 1100.0 };
        for frame in 0..441 {
            if let Some(out) = samples.get_mut(start + frame) {
                let time = frame as f64 / sample_rate as f64;
                *out = (0.3
                    * (-time * 400.0).exp()
                    * (std::f64::consts::TAU * frequency * time).sin())
                    as f32;
            }
        }
    }
    Ok(Pcm {
        sample_rate,
        channels: 1,
        samples,
    })
}

struct Buffer {
    channels: usize,
    skip: u64,
    limit: usize,
    samples: Vec<f32>,
    error: Option<String>,
}
impl Buffer {
    fn push(&mut self, data: &[f32]) {
        for frame in data.chunks_exact(self.channels) {
            if self.skip > 0 {
                self.skip -= 1;
                continue;
            }
            if self.samples.len() >= self.limit {
                break;
            }
            let value = frame.iter().copied().sum::<f32>() / self.channels as f32;
            self.samples.push(if value.is_finite() {
                value.clamp(-1.0, 1.0)
            } else {
                0.0
            });
        }
    }
}
pub struct RecordingStatus {
    pub count_in_frames_remaining: u64,
    pub captured_frames: u64,
    pub sample_rate: u32,
    pub limit_reached: bool,
    pub error: Option<String>,
}
pub struct Recorder {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<Buffer>>,
    sample_rate: u32,
}
impl Recorder {
    #[cfg(test)]
    pub(crate) fn fixture() -> Self {
        Self {
            stream: None,
            buffer: Arc::new(Mutex::new(Buffer {
                channels: 1,
                skip: 0,
                limit: 44_100,
                samples: vec![0.25, -0.25],
                error: None,
            })),
            sample_rate: 44_100,
        }
    }

    pub fn start(bpm: f64) -> Result<Self, String> {
        if !bpm.is_finite() || !(7.5..=1200.0).contains(&bpm) {
            return Err("Effective tempo must be from 7.5 to 1200 BPM.".into());
        }
        let device = cpal::default_host()
            .default_input_device()
            .ok_or("No audio input device is available.")?;
        let supported = device
            .default_input_config()
            .map_err(|e| format!("Cannot read the audio input settings: {e}"))?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        if config.channels == 0
            || config.channels > 32
            || config.sample_rate.0 == 0
            || config.sample_rate.0 > 384000
        {
            return Err("The audio input settings are not supported.".into());
        }
        let sample_rate = config.sample_rate.0;
        let buffer = Arc::new(Mutex::new(Buffer {
            channels: config.channels as usize,
            skip: (sample_rate as f64 * 480.0 / bpm).round() as u64,
            limit: sample_rate as usize * 600,
            samples: Vec::new(),
            error: None,
        }));
        let capture = buffer.clone();
        let errors = buffer.clone();
        let on_error = move |error: cpal::StreamError| {
            if let Ok(mut b) = errors.lock() {
                b.error = Some(format!("Audio input failed: {error}"));
            }
        };
        let stream = match format {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _| {
                    if let Ok(mut b) = capture.lock() {
                        b.push(data);
                    }
                },
                on_error,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config,
                move |data: &[i16], _| {
                    let converted: Vec<_> = data.iter().map(|x| *x as f32 / 32768.0).collect();
                    if let Ok(mut b) = capture.lock() {
                        b.push(&converted);
                    }
                },
                on_error,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_input_stream(
                &config,
                move |data: &[u16], _| {
                    let converted: Vec<_> = data
                        .iter()
                        .map(|x| (*x as f32 - 32768.0) / 32768.0)
                        .collect();
                    if let Ok(mut b) = capture.lock() {
                        b.push(&converted);
                    }
                },
                on_error,
                None,
            ),
            _ => return Err("This audio input sample format is not supported.".into()),
        }
        .map_err(|e| format!("Cannot open the audio input: {e}"))?;
        stream
            .play()
            .map_err(|e| format!("Cannot start the audio input: {e}"))?;
        Ok(Self {
            stream: Some(stream),
            buffer,
            sample_rate,
        })
    }
    pub fn status(&self) -> RecordingStatus {
        match self.buffer.lock() {
            Ok(b) => RecordingStatus {
                count_in_frames_remaining: b.skip,
                captured_frames: b.samples.len() as u64,
                sample_rate: self.sample_rate,
                limit_reached: b.samples.len() >= b.limit,
                error: b.error.clone(),
            },
            Err(_) => RecordingStatus {
                count_in_frames_remaining: 0,
                captured_frames: 0,
                sample_rate: self.sample_rate,
                limit_reached: false,
                error: Some("The recording buffer is unavailable.".into()),
            },
        }
    }
    pub fn finish(mut self) -> Result<Pcm, String> {
        drop(self.stream.take());
        let mut b = self
            .buffer
            .lock()
            .map_err(|_| "The recording buffer is unavailable.")?;
        if let Some(e) = &b.error {
            return Err(e.clone());
        }
        if b.samples.is_empty() {
            return Err("No audio was recorded after the count-in.".into());
        }
        Ok(Pcm {
            sample_rate: self.sample_rate,
            channels: 1,
            samples: std::mem::take(&mut b.samples),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn count_in_discards_exact_frames_and_capture_is_bounded() {
        let mut b = Buffer {
            channels: 2,
            skip: 2,
            limit: 2,
            samples: Vec::new(),
            error: None,
        };
        b.push(&[1.0, 1.0, 0.5, 0.5, 0.2, 0.6, 0.0, 0.2, 1.0, 1.0]);
        assert_eq!(b.skip, 0);
        assert_eq!(b.samples, vec![0.4, 0.1]);
    }
    #[test]
    fn metronome_has_eight_beats_and_two_bars() {
        let pcm = count_in(120.0).unwrap();
        assert_eq!(pcm.frames(), 176400);
        for beat in 0..8 {
            let start = beat * 22050;
            assert!(
                pcm.samples[start..start + 441]
                    .iter()
                    .any(|x| x.abs() > 0.01)
            );
            assert!(
                pcm.samples[start + 441..start + 22050]
                    .iter()
                    .all(|x| *x == 0.0)
            );
        }
    }
}
