use crate::{
    engine::{Project, SOURCE_BPM},
    library::Library,
};
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};

#[derive(Clone, Debug)]
pub struct Pcm {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}
impl Pcm {
    pub fn frames(&self) -> u64 {
        self.samples.len() as u64 / u64::from(self.channels.max(1))
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.sample_rate == 0
            || self.sample_rate > 384_000
            || !(1..=2).contains(&self.channels)
            || !self.samples.len().is_multiple_of(self.channels as usize)
            || self.samples.iter().any(|x| !x.is_finite() || x.abs() > 1.0)
        {
            return Err(
                "The audio must contain normalized finite mono or stereo PCM samples.".into(),
            );
        }
        Ok(())
    }
    pub fn write_wav(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let spec = hound::WavSpec {
            channels: self.channels,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
        for &s in &self.samples {
            writer
                .write_sample(float_to_i16(s))
                .map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())
    }
}
fn float_to_i16(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * 32768.0)
        .round()
        .clamp(-32768.0, 32767.0) as i16
}

pub fn read_wav(path: &Path) -> Result<Pcm, String> {
    let mut samples = Vec::new();
    let (spec, _) = visit_wav_samples(path, |_, sample| samples.push(sample))?;
    Ok(Pcm {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        samples,
    })
}

/// Validate and normalize WAV samples without retaining decoded audio.
pub(crate) fn visit_wav_samples(
    path: &Path,
    mut visit: impl FnMut(hound::WavSpec, f32),
) -> Result<(hound::WavSpec, u32), String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let spec = reader.spec();
    if !(1..=2).contains(&spec.channels)
        || spec.sample_rate == 0
        || spec.sample_rate > 384_000
        || reader.len() > 64_000_000
    {
        return Err(
            "The WAV must contain mono or stereo audio with a supported sample rate and size."
                .into(),
        );
    }
    let mut count = 0_u32;
    let mut accept = |value: Result<f32, hound::Error>| -> Result<(), String> {
        let value = value.map_err(|e| e.to_string())?;
        if !value.is_finite() || value.abs() > 1.0 {
            return Err(
                "The audio must contain normalized finite mono or stereo PCM samples.".into(),
            );
        }
        visit(spec, value);
        count += 1;
        Ok(())
    };
    match spec.sample_format {
        hound::SampleFormat::Float if spec.bits_per_sample == 32 => {
            for value in reader.samples::<f32>() {
                accept(value.map(|v| if v.is_finite() { v.clamp(-1.0, 1.0) } else { v }))?;
            }
        }
        hound::SampleFormat::Int if (8..=32).contains(&spec.bits_per_sample) => {
            let divisor = (1_u64 << (spec.bits_per_sample - 1)) as f32;
            for value in reader.samples::<i32>() {
                accept(value.map(|v| v as f32 / divisor))?;
            }
        }
        _ => return Err("This WAV encoding is not supported.".into()),
    };
    if !count.is_multiple_of(u32::from(spec.channels)) {
        return Err("The audio must contain normalized finite mono or stereo PCM samples.".into());
    }
    Ok((spec, count / u32::from(spec.channels)))
}

pub fn render_project(project: &Project, library: &Library) -> Result<Pcm, String> {
    project.validate()?;
    let mut cache = HashMap::new();
    let missing: std::collections::HashSet<_> = project
        .missing_samples
        .iter()
        .map(|sample| sample.id)
        .collect();
    for clip in &project.clips {
        if missing.contains(&clip.sample_id) {
            continue;
        }
        if let std::collections::hash_map::Entry::Vacant(e) = cache.entry(clip.sample_id) {
            e.insert(library.read_pcm(clip.sample_id)?);
        }
    }
    let source_tempos = project
        .clips
        .iter()
        .map(|c| {
            (
                c.sample_id,
                library
                    .sample_by_id(c.sample_id)
                    .map_or(SOURCE_BPM, |s| s.source_bpm),
            )
        })
        .collect();
    render_with_tempos(project, &cache, &source_tempos)
}

pub fn render_with_samples(project: &Project, samples: &HashMap<u32, Pcm>) -> Result<Pcm, String> {
    let source_tempos = project
        .imported_samples
        .iter()
        .map(|s| (s.id, s.source_bpm))
        .collect();
    render_with_tempos(project, samples, &source_tempos)
}
fn render_with_tempos(
    project: &Project,
    samples: &HashMap<u32, Pcm>,
    source_tempos: &HashMap<u32, f64>,
) -> Result<Pcm, String> {
    project.validate()?;
    let rate = 44_100;
    let frames_per_tick = project.frames_per_tick(rate);
    let frames = (project.end_tick() as f64 * frames_per_tick).ceil() as usize;
    if frames > 32_000_000 {
        return Err(
            "This song is too long for in-memory playback. Export a shorter section.".into(),
        );
    }
    let mut out = vec![0.0_f32; frames * 2];
    let missing: std::collections::HashSet<_> = project
        .missing_samples
        .iter()
        .map(|sample| sample.id)
        .collect();
    for clip in &project.clips {
        if missing.contains(&clip.sample_id) {
            continue;
        }
        let source = samples
            .get(&clip.sample_id)
            .ok_or_else(|| format!("Sample {} is missing.", clip.sample_id))?;
        source.validate()?;
        let track = &project.tracks[clip.track];
        if !track.left_enabled && !track.right_enabled {
            continue;
        }
        let start = (clip.start_tick as f64 * frames_per_tick).round() as usize;
        let end = (((clip.start_tick + clip.length_ticks) as f64 * frames_per_tick).round()
            as usize)
            .min(frames);
        let source_bpm = source_tempos
            .get(&clip.sample_id)
            .copied()
            .unwrap_or(SOURCE_BPM);
        if !source_bpm.is_finite() || source_bpm <= 0.0 {
            return Err("A sample has an invalid source tempo.".into());
        }
        let step = source.sample_rate as f64 / rate as f64 * project.bpm / source_bpm;
        if !step.is_finite() || step <= 0.0 {
            return Err("The source tempo cannot be represented for sample playback.".into());
        }
        let gain = clip.gain * track.gain * project.master_gain;
        // Pan and channel enables depend only on the track: resolve them once per clip.
        let channel_gains = [
            track
                .left_enabled
                .then(|| gain * (1.0 - track.pan.max(0.0))),
            track
                .right_enabled
                .then(|| gain * (1.0 + track.pan.min(0.0))),
        ];
        for frame in start..end {
            let pos = clip.offset_frames as f64 + (frame - start) as f64 * step;
            let index = pos.floor() as usize;
            if index >= source.frames() as usize {
                break;
            }
            let next = (index + 1).min(source.frames() as usize - 1);
            let fraction = (pos - index as f64) as f32;
            for (channel, channel_gain) in channel_gains.iter().enumerate() {
                let Some(channel_gain) = channel_gain else {
                    continue;
                };
                let ch = if source.channels == 1 { 0 } else { channel };
                let a = source.samples[index * source.channels as usize + ch];
                let b = source.samples[next * source.channels as usize + ch];
                out[frame * 2 + channel] += (a + (b - a) * fraction) * channel_gain;
            }
        }
    }
    // Saturation is the reconstruction policy. Original mixer rounding and resampling remain to be measured.
    for x in &mut out {
        *x = x.clamp(-1.0, 1.0);
    }
    Ok(Pcm {
        sample_rate: rate,
        channels: 2,
        samples: out,
    })
}

pub struct Player {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Option<Sink>,
    start_frame: u64,
    total_frames: u64,
    sample_rate: u32,
    loop_region: Option<[u64; 2]>,
}
struct PlaybackSource {
    pcm: Arc<Pcm>,
    index: usize,
    start: u64,
    loop_region: Option<[u64; 2]>,
}
impl Iterator for PlaybackSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let channels = self.pcm.channels as usize;
        let end = self.loop_region.map_or(self.pcm.frames(), |[_, b]| b) as usize * channels;
        if self.index >= end {
            if let Some([a, _]) = self.loop_region {
                self.index = a as usize * channels;
            } else {
                return None;
            }
        }
        let value = self.pcm.samples.get(self.index).copied()?;
        self.index += 1;
        Some(value)
    }
}
impl Source for PlaybackSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.pcm.channels
    }
    fn sample_rate(&self) -> u32 {
        self.pcm.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        if self.loop_region.is_some() {
            None
        } else {
            Some(Duration::from_secs_f64(
                (self.pcm.frames() - self.start) as f64 / self.pcm.sample_rate as f64,
            ))
        }
    }
}
impl Player {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| format!("Cannot open the audio output: {e}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            sink: None,
            start_frame: 0,
            total_frames: 0,
            sample_rate: 44100,
            loop_region: None,
        })
    }
    pub fn play(
        &mut self,
        pcm: Arc<Pcm>,
        start_frame: u64,
        loop_region: Option<[u64; 2]>,
    ) -> Result<(), String> {
        pcm.validate()?;
        let total = pcm.frames();
        if total == 0 {
            return Err("There is no audio to play.".into());
        }
        if let Some([a, b]) = loop_region
            && (a >= b || b > total)
        {
            return Err("The audio loop is outside the song.".into());
        }
        let start = if let Some([a, b]) = loop_region {
            if start_frame >= b { a } else { start_frame }
        } else {
            start_frame.min(total)
        };
        let sink = Sink::try_new(&self.handle).map_err(|e| e.to_string())?;
        let channels = pcm.channels as usize;
        sink.append(PlaybackSource {
            pcm: pcm.clone(),
            index: start as usize * channels,
            start,
            loop_region,
        });
        if let Some(old) = self.sink.take() {
            old.stop();
        }
        self.start_frame = start;
        self.total_frames = total;
        self.sample_rate = pcm.sample_rate;
        self.loop_region = loop_region;
        self.sink = Some(sink);
        Ok(())
    }
    pub fn pause(&self) {
        if let Some(sink) = &self.sink {
            sink.pause();
        }
    }
    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.start_frame = 0;
        self.loop_region = None;
    }
    pub fn is_playing(&self) -> bool {
        self.sink
            .as_ref()
            .is_some_and(|s| !s.empty() && !s.is_paused())
    }
    pub fn position_frames(&self) -> u64 {
        let elapsed = self.sink.as_ref().map_or(0, |s| {
            (s.get_pos().as_secs_f64() * self.sample_rate as f64) as u64
        });
        let pos = self.start_frame.saturating_add(elapsed);
        if let Some([a, b]) = self.loop_region {
            if pos >= b {
                a + (pos - b) % (b - a)
            } else {
                pos
            }
        } else {
            pos.min(self.total_frames)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Clip, TICKS_PER_BAR};
    fn song() -> Project {
        Project {
            master_gain: 1.0,
            clips: vec![Clip {
                id: 1,
                sample_id: 5,
                track: 0,
                start_tick: TICKS_PER_BAR,
                length_ticks: TICKS_PER_BAR,
                offset_frames: 1,
                gain: 0.5,
            }],
            ..Project::default()
        }
    }
    #[test]
    fn explicit_missing_source_is_silent_even_when_cached_audio_has_the_same_id() {
        let mut project = song();
        project.missing_samples.push(
            crate::engine::MissingSample::native(
                crate::products::Product::T2,
                5,
                "Missing sound".into(),
                140.0,
                TICKS_PER_BAR,
                false,
                "The referenced sound is unavailable.".into(),
            )
            .unwrap(),
        );
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![0.5; 75600],
            },
        )]);
        let rendered = render_with_samples(&project, &samples).unwrap();
        assert_eq!(rendered.frames(), 151200);
        assert!(rendered.samples.iter().all(|sample| *sample == 0.0));
        project.missing_samples.clear();
        assert!(render_with_samples(&project, &HashMap::new()).is_err());
    }

    #[test]
    fn last_track_preserves_scheduled_offset_gain_and_channel_routing() {
        let mut p = song();
        p.clips[0].track = 63;
        p.tracks[63].right_enabled = false;
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44100,
                channels: 1,
                samples: vec![0.0, 0.5, 1.0, -1.0],
            },
        )]);
        let out = render_with_samples(&p, &samples).unwrap();
        assert_eq!(out.frames(), 151200);
        assert_eq!(out.samples[151198], 0.0);
        assert_eq!(
            &out.samples[151200..151206],
            &[0.25, 0.0, 0.5, 0.0, -0.5, 0.0]
        );
    }
    #[test]
    fn tempo_and_pitch_use_one_clock() {
        let mut p = song();
        p.bpm = 280.0;
        p.pitch_ratio = 2.0;
        let p = crate::engine::normalize_import(p).unwrap();
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44100,
                channels: 1,
                samples: vec![1.0; 75600],
            },
        )]);
        assert_eq!(p.bpm, 180.0);
        assert_eq!(p.pitch_ratio, 1.0);
        assert_eq!(render_with_samples(&p, &samples).unwrap().frames(), 117600);
    }
    #[test]
    fn clipping_and_float_pcm_boundaries() {
        assert_eq!(float_to_i16(-1.0), -32768);
        assert_eq!(float_to_i16(1.0), 32767);
        let mut p = song();
        p.clips[0].start_tick = 0;
        p.clips[0].offset_frames = 0;
        p.clips[0].gain = 2.0;
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44100,
                channels: 1,
                samples: vec![1.0],
            },
        )]);
        assert_eq!(
            &render_with_samples(&p, &samples).unwrap().samples[..2],
            &[1.0, 1.0]
        );
    }
    #[test]
    fn loop_source_keeps_channels_aligned_and_preserves_intro() {
        let pcm = Arc::new(Pcm {
            sample_rate: 44100,
            channels: 2,
            samples: vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7],
        });
        let source = PlaybackSource {
            pcm,
            index: 0,
            start: 0,
            loop_region: Some([1, 3]),
        };
        assert_eq!(
            source.take(14).collect::<Vec<_>>(),
            vec![
                0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.2, 0.3, 0.4, 0.5, 0.2, 0.3, 0.4, 0.5
            ]
        );
    }
    #[test]
    fn captured_source_tempo_is_not_applied_twice() {
        let mut p = song();
        p.bpm = 180.0;
        p.clips[0].offset_frames = 0;
        p.clips[0].sample_id = 1_000_000;
        p.imported_samples.push(crate::engine::ImportedSample {
            id: 1_000_000,
            name: "Recorded take".into(),
            group_name: "Wave".into(),
            path: "take.wav".into(),
            source_bpm: 180.0,
            source_sample_rate: Some(44100),
            origin: None,
        });
        let pcm = Pcm {
            sample_rate: 44100,
            channels: 1,
            samples: vec![0.1, 0.2, 0.3],
        };
        let out = render_with_samples(&p, &HashMap::from([(1_000_000, pcm)])).unwrap();
        let start = p.frames_per_tick(44100) * 256.0;
        let index = start.round() as usize * 2;
        assert_eq!(
            &out.samples[index..index + 6],
            &[0.05, 0.05, 0.1, 0.1, 0.15, 0.15]
        );
    }
    #[test]
    fn factual_source_tempos_do_not_change_the_discrete_project_clock() {
        for bpm in [0.001, 2000.0] {
            let mut project = song();
            project.clips[0].sample_id = 1_000_000;
            project.clips[0].start_tick = 0;
            project.clips[0].offset_frames = 0;
            project
                .imported_samples
                .push(crate::engine::ImportedSample {
                    id: 1_000_000,
                    name: "Source".into(),
                    group_name: "Wave".into(),
                    path: "source.wav".into(),
                    source_bpm: bpm,
                    source_sample_rate: Some(44100),
                    origin: None,
                });
            let samples = HashMap::from([(
                1_000_000,
                Pcm {
                    sample_rate: 44100,
                    channels: 1,
                    samples: vec![0.5; 32],
                },
            )]);
            let pcm = render_with_samples(&project, &samples).unwrap();
            pcm.validate().unwrap();
            assert_eq!(pcm.frames(), 75600);
            assert!(pcm.samples[0] > 0.0);
            project.imported_samples[0].source_bpm = f64::MIN_POSITIVE;
            assert!(render_with_samples(&project, &samples).is_err());
        }
    }
    #[test]
    fn extreme_float_wav_is_normalized_before_interpolation() {
        let path =
            std::env::temp_dir().join(format!("mixjam-techno-float-{}.wav", std::process::id()));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut wav = hound::WavWriter::create(&path, spec).unwrap();
        wav.write_sample(f32::MAX).unwrap();
        wav.write_sample(-f32::MAX).unwrap();
        wav.finalize().unwrap();
        let pcm = read_wav(&path).unwrap();
        assert_eq!(pcm.samples, vec![1.0, -1.0]);
        let mut p = song();
        p.clips[0].offset_frames = 0;
        p.bpm = 120.0;
        assert!(
            render_with_samples(&p, &HashMap::from([(5, pcm)]))
                .unwrap()
                .samples
                .iter()
                .all(|x| x.is_finite())
        );
        std::fs::remove_file(path).unwrap();
    }
}
