use crate::{
    engine::{Project, SOURCE_BPM},
    library::Library,
    master::{MasterChain, MasterControl, MasterProcessor},
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
        // A failed export must not truncate a WAV that it would replace.
        crate::atomic_file::write_with(path, |file| {
            let mut writer = hound::WavWriter::new(std::io::BufWriter::new(file), spec)
                .map_err(|e| e.to_string())?;
            for &s in &self.samples {
                writer
                    .write_sample(float_to_i16(s))
                    .map_err(|e| e.to_string())?;
            }
            writer.finalize().map_err(|e| e.to_string())
        })
    }
}
pub(crate) fn float_to_i16(x: f32) -> i16 {
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

/// The largest WAV that the library reads, in samples of all channels.
pub(crate) const MAX_WAV_SAMPLES: usize = 64_000_000;

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
        || reader.len() as usize > MAX_WAV_SAMPLES
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

/// Audio that a player reads in blocks: decoded PCM or a mix made on demand.
pub trait Frames: Send + Sync {
    fn channels(&self) -> u16;
    fn sample_rate(&self) -> u32;
    fn frames(&self) -> u64;
    /// Fill `out` with interleaved samples from `first_frame` onward.
    /// Samples past the end are silent.
    fn read(&self, first_frame: u64, out: &mut [f32]);
}

impl Frames for Pcm {
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn frames(&self) -> u64 {
        Pcm::frames(self)
    }
    fn read(&self, first_frame: u64, out: &mut [f32]) {
        let start = (first_frame as usize)
            .saturating_mul(self.channels as usize)
            .min(self.samples.len());
        let available = (self.samples.len() - start).min(out.len());
        out[..available].copy_from_slice(&self.samples[start..start + available]);
        out[available..].fill(0.0);
    }
}

/// One clip, resolved to output frames and its decoded source.
struct Voice {
    source: Arc<Pcm>,
    start: u64,
    end: u64,
    offset_frames: u32,
    step: f64,
    /// Left and right gains; `None` for a disabled output.
    channel_gains: [Option<f32>; 2],
}

/// A project prepared for rendering. It holds the decoded sources and mixes
/// any range of the song on demand, so playback and export need no buffer of
/// the whole song and a song can be as long as the timeline.
pub struct Mix {
    frames: u64,
    voices: Vec<Voice>,
}

impl Mix {
    pub const SAMPLE_RATE: u32 = 44_100;
    /// The longest mix that `to_pcm` holds in memory: about 12 minutes.
    const MAX_PCM_FRAMES: u64 = 32_000_000;

    fn new(
        project: &Project,
        samples: &HashMap<u32, Arc<Pcm>>,
        source_tempos: &HashMap<u32, f64>,
    ) -> Result<Self, String> {
        project.validate()?;
        let rate = Self::SAMPLE_RATE;
        let frames_per_tick = project.frames_per_tick(rate);
        let frames = (project.end_tick() as f64 * frames_per_tick).ceil() as u64;
        let missing: std::collections::HashSet<_> = project
            .missing_samples
            .iter()
            .map(|sample| sample.id)
            .collect();
        // Many clips share one source: check each source's audio once.
        let mut validated = std::collections::HashSet::new();
        let mut voices = Vec::new();
        let any_solo = project.any_solo();
        for clip in &project.clips {
            if missing.contains(&clip.sample_id) {
                continue;
            }
            let source = samples
                .get(&clip.sample_id)
                .ok_or_else(|| format!("Sample {} is missing.", clip.sample_id))?;
            if validated.insert(clip.sample_id) {
                source.validate()?;
            }
            let track = &project.tracks[clip.track];
            if !track.audible(any_solo) || !track.left_enabled && !track.right_enabled {
                continue;
            }
            let start = (clip.start_tick as f64 * frames_per_tick).round() as u64;
            let end = (((clip.start_tick + clip.length_ticks) as f64 * frames_per_tick).round()
                as u64)
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
            let gain = clip.gain * track.effective_gain() * project.master_gain;
            // Pan and channel enables depend only on the track: resolve them once per clip.
            voices.push(Voice {
                source: Arc::clone(source),
                start,
                end,
                offset_frames: clip.offset_frames,
                step,
                channel_gains: [
                    track
                        .left_enabled
                        .then(|| gain * (1.0 - track.pan.max(0.0))),
                    track
                        .right_enabled
                        .then(|| gain * (1.0 + track.pan.min(0.0))),
                ],
            });
        }
        Ok(Self { frames, voices })
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// The whole mix in memory, through the master pedalboard. Long songs play
    /// and export through `read` and `write_wav`, which have no length limit.
    /// Pedals that delay the sound run on past the end, and the delay is
    /// cut from the start, so the result lines up with the song.
    pub fn to_pcm(&self, master: &MasterChain) -> Result<Pcm, String> {
        if self.frames > Self::MAX_PCM_FRAMES {
            return Err("This song is too long to hold in memory. Play or export it.".into());
        }
        let mut processor = MasterProcessor::new(master);
        let latency = processor.latency();
        let mut samples = vec![0.0; (self.frames as usize + latency) * 2];
        self.read(0, &mut samples);
        processor.process(&mut samples);
        samples.drain(..latency * 2);
        Ok(Pcm {
            sample_rate: Self::SAMPLE_RATE,
            channels: 2,
            samples,
        })
    }

    /// Write stereo PCM16 through the master pedalboard, mixing one block at a time.
    pub fn write_wav(&self, path: &Path, master: &MasterChain) -> Result<(), String> {
        const BLOCK_FRAMES: u64 = 65_536;
        // A WAV header counts its data bytes in 32 bits.
        if self.frames > u64::from(u32::MAX) / 4 {
            return Err("This song is too long for a WAV file.".into());
        }
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: Self::SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        crate::atomic_file::write_with(path, |file| {
            let mut writer = hound::WavWriter::new(std::io::BufWriter::new(file), spec)
                .map_err(|e| e.to_string())?;
            let mut block = vec![0.0; BLOCK_FRAMES as usize * 2];
            let mut master = MasterProcessor::new(master);
            // The pedals run on past the end by their delay, which is cut
            // from the start, as in `to_pcm`.
            let latency = master.latency() as u64;
            let mut frame = 0;
            while frame < self.frames + latency {
                let count = (self.frames + latency - frame).min(BLOCK_FRAMES) as usize;
                self.read(frame, &mut block[..count * 2]);
                master.process(&mut block[..count * 2]);
                let skip = latency.saturating_sub(frame).min(count as u64) as usize;
                for &sample in &block[skip * 2..count * 2] {
                    writer
                        .write_sample(float_to_i16(sample))
                        .map_err(|e| e.to_string())?;
                }
                frame += count as u64;
            }
            writer.finalize().map_err(|e| e.to_string())
        })
    }
}

impl Frames for Mix {
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        Self::SAMPLE_RATE
    }
    fn frames(&self) -> u64 {
        self.frames
    }
    /// Runs on the audio thread during playback: no allocation and no locks.
    fn read(&self, first_frame: u64, out: &mut [f32]) {
        out.fill(0.0);
        let block_end = first_frame + (out.len() / 2) as u64;
        for voice in &self.voices {
            let from = voice.start.max(first_frame);
            let to = voice.end.min(block_end);
            if from >= to {
                continue;
            }
            let source = &*voice.source;
            let source_frames = source.frames() as usize;
            let source_channels = source.channels as usize;
            for frame in from..to {
                let pos = voice.offset_frames as f64 + (frame - voice.start) as f64 * voice.step;
                let index = pos.floor() as usize;
                if index >= source_frames {
                    break;
                }
                let next = (index + 1).min(source_frames - 1);
                let fraction = (pos - index as f64) as f32;
                let out_frame = (frame - first_frame) as usize * 2;
                for (channel, channel_gain) in voice.channel_gains.iter().enumerate() {
                    let Some(channel_gain) = channel_gain else {
                        continue;
                    };
                    let ch = if source_channels == 1 { 0 } else { channel };
                    let a = source.samples[index * source_channels + ch];
                    let b = source.samples[next * source_channels + ch];
                    out[out_frame + channel] += (a + (b - a) * fraction) * channel_gain;
                }
            }
        }
        // Saturation is the reconstruction policy. Original mixer rounding and resampling remain to be measured.
        for x in out {
            *x = x.clamp(-1.0, 1.0);
        }
    }
}

/// Render a whole project in memory. Use `prepare_project` for a song of any length.
pub fn render_project(project: &Project, library: &Library) -> Result<Pcm, String> {
    prepare_project(project, library)?.to_pcm(&project.master)
}

/// Decode a project's sources and prepare its mix.
pub fn prepare_project(project: &Project, library: &Library) -> Result<Mix, String> {
    prepare_project_reusing(project, library, &mut HashMap::new())
}

/// Prepare a mix with the sources that an earlier call for the same library
/// decoded. `decoded` then holds exactly the sources of this project, so its
/// size follows the song. The caller clears it when the library changes.
pub(crate) fn prepare_project_reusing(
    project: &Project,
    library: &Library,
    decoded: &mut HashMap<u32, Arc<Pcm>>,
) -> Result<Mix, String> {
    project.validate()?;
    let missing: std::collections::HashSet<_> = project
        .missing_samples
        .iter()
        .map(|sample| sample.id)
        .collect();
    let used: std::collections::HashSet<_> = project
        .clips
        .iter()
        .map(|clip| clip.sample_id)
        .filter(|id| !missing.contains(id))
        .collect();
    decoded.retain(|id, _| used.contains(id));
    for id in used {
        if let std::collections::hash_map::Entry::Vacant(e) = decoded.entry(id) {
            e.insert(Arc::new(library.read_pcm(id)?));
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
    Mix::new(project, decoded, &source_tempos)
}

#[cfg(test)]
pub(crate) fn mix_with_samples(
    project: &Project,
    samples: &HashMap<u32, Pcm>,
) -> Result<Mix, String> {
    let source_tempos = project
        .imported_samples
        .iter()
        .map(|s| (s.id, s.source_bpm))
        .collect();
    let samples = samples
        .iter()
        .map(|(id, pcm)| (*id, Arc::new(pcm.clone())))
        .collect();
    Mix::new(project, &samples, &source_tempos)
}

#[cfg(test)]
pub(crate) fn render_with_samples(
    project: &Project,
    samples: &HashMap<u32, Pcm>,
) -> Result<Pcm, String> {
    mix_with_samples(project, samples)?.to_pcm(&project.master)
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

/// Reads audio in blocks for the output device, looping when asked to.
struct PlaybackSource {
    audio: Arc<dyn Frames>,
    channels: usize,
    start: u64,
    /// The next frame to read into the block.
    frame: u64,
    /// The frame after the last one played before stopping or looping.
    end: u64,
    /// The frame where a loop resumes.
    loop_start: Option<u64>,
    block: Vec<f32>,
    /// The next sample in `block`, and the count of valid samples in it.
    cursor: usize,
    filled: usize,
    /// The master pedalboard of a stereo mix. Its state runs on through loops.
    master: Option<MasterProcessor>,
    /// Frames of the pedals' delay still to drop at the start, so the song
    /// starts on time, and still to play out after the end.
    lead: usize,
    tail: usize,
}
impl PlaybackSource {
    /// About 23 ms, so the master meters update more than 30 times a second.
    const BLOCK_FRAMES: usize = 1024;

    fn new(
        audio: Arc<dyn Frames>,
        start: u64,
        loop_region: Option<[u64; 2]>,
        master: Option<Arc<MasterControl>>,
    ) -> Self {
        let channels = audio.channels() as usize;
        let master = master.filter(|_| channels == 2).map(MasterProcessor::live);
        let latency = master.as_ref().map_or(0, MasterProcessor::latency);
        Self {
            channels,
            start,
            frame: start,
            end: loop_region.map_or(audio.frames(), |[_, b]| b),
            loop_start: loop_region.map(|[a, _]| a),
            block: vec![0.0; Self::BLOCK_FRAMES * channels],
            cursor: 0,
            filled: 0,
            master,
            lead: latency,
            tail: latency,
            audio,
        }
    }
    /// Read and process the next block that has sound to play. False when
    /// nothing is left.
    fn fill(&mut self) -> bool {
        loop {
            if self.frame >= self.end {
                if let Some(start) = self.loop_start {
                    self.frame = start;
                } else if self.tail > 0 {
                    let count = self.tail.min(Self::BLOCK_FRAMES);
                    self.tail -= count;
                    self.filled = count * self.channels;
                    self.block[..self.filled].fill(0.0);
                    if self.process() {
                        return true;
                    }
                    continue;
                } else {
                    if let Some(mut master) = self.master.take() {
                        master.finish();
                    }
                    return false;
                }
            }
            let count = ((self.end - self.frame) as usize).min(Self::BLOCK_FRAMES);
            if count == 0 {
                return false;
            }
            self.filled = count * self.channels;
            self.audio.read(self.frame, &mut self.block[..self.filled]);
            self.frame += count as u64;
            if self.process() {
                return true;
            }
        }
    }
    /// Run the block through the pedals and drop what remains of their
    /// delay. True when the block still has samples to play.
    fn process(&mut self) -> bool {
        if let Some(master) = &mut self.master {
            master.process(&mut self.block[..self.filled]);
        }
        let skip = self.lead.min(self.filled / self.channels);
        self.lead -= skip;
        self.cursor = skip * self.channels;
        self.cursor < self.filled
    }
}
impl Iterator for PlaybackSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.cursor == self.filled && !self.fill() {
            return None;
        }
        let value = self.block[self.cursor];
        self.cursor += 1;
        Some(value)
    }
}
impl Source for PlaybackSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels as u16
    }
    fn sample_rate(&self) -> u32 {
        self.audio.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        if self.loop_start.is_some() {
            None
        } else {
            Some(Duration::from_secs_f64(
                self.audio.frames().saturating_sub(self.start) as f64
                    / self.audio.sample_rate() as f64,
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
    /// Play validated audio. Callers check decoded PCM before they pass it.
    /// A mix plays through `master`, whose settings can change as it plays.
    pub fn play(
        &mut self,
        audio: Arc<dyn Frames>,
        start_frame: u64,
        loop_region: Option<[u64; 2]>,
        master: Option<Arc<MasterControl>>,
    ) -> Result<(), String> {
        let total = audio.frames();
        if !(1..=2).contains(&audio.channels())
            || audio.sample_rate() == 0
            || audio.sample_rate() > 384_000
        {
            return Err(
                "The audio must contain normalized finite mono or stereo PCM samples.".into(),
            );
        }
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
        let sample_rate = audio.sample_rate();
        sink.append(PlaybackSource::new(audio, start, loop_region, master));
        if let Some(old) = self.sink.take() {
            old.stop();
        }
        self.start_frame = start;
        self.total_frames = total;
        self.sample_rate = sample_rate;
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
    fn mixer_attenuation_and_polarity_are_independent_of_pan_and_routing() {
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        project.tracks[0].pan = 0.25;
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44100,
                channels: 2,
                samples: vec![0.125, -0.25, 0.25, -0.125],
            },
        )]);
        for amplitude in [
            0.0,
            crate::engine::db_to_amplitude(-6.0),
            1.0,
            crate::engine::db_to_amplitude(crate::engine::MAX_MIXER_GAIN_DB),
        ] {
            project.tracks[0].gain = amplitude;
            project.tracks[0].polarity_inverted = false;
            let mut normal = [0.0; 4];
            Frames::read(
                &mix_with_samples(&project, &samples).unwrap(),
                0,
                &mut normal,
            );
            assert!((normal[0] - 0.125 * 0.5 * amplitude * 0.75).abs() < 1e-7);
            assert!((normal[1] - -0.25 * 0.5 * amplitude).abs() < 1e-7);
            project.tracks[0].polarity_inverted = true;
            let mut reversed = [0.0; 4];
            Frames::read(
                &mix_with_samples(&project, &samples).unwrap(),
                0,
                &mut reversed,
            );
            for (normal, reversed) in normal.into_iter().zip(reversed) {
                assert_eq!(reversed, -normal);
            }
        }
        project.tracks[0].gain = 1.0;
        project.tracks[0].right_enabled = false;
        let mut routed = [0.0; 4];
        Frames::read(
            &mix_with_samples(&project, &samples).unwrap(),
            0,
            &mut routed,
        );
        assert!(routed[0] < 0.0);
        assert_eq!([routed[1], routed[3]], [0.0, 0.0]);
    }

    #[test]
    fn solo_plays_only_soloed_tracks_and_mute_wins_over_solo() {
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        // Tracks 1 to 3 each play the same sound.
        for track in 1..3 {
            let mut clip = project.clips[0].clone();
            clip.id = 10 + track as u64;
            clip.track = track;
            project.clips.push(clip);
        }
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44100,
                channels: 1,
                samples: vec![0.25; 4],
            },
        )]);
        let one = render_with_samples(&project, &samples).unwrap().samples[0] / 3.0;
        for (muted, solo, playing) in [
            ([false; 3], [false; 3], 3.0),
            ([true, false, false], [false; 3], 2.0),
            ([false; 3], [false, true, false], 1.0),
            ([false; 3], [true, true, false], 2.0),
            ([true, false, false], [true, true, false], 1.0),
            ([true, false, false], [true, false, false], 0.0),
        ] {
            for track in 0..3 {
                project.tracks[track].muted = muted[track];
                project.tracks[track].solo = solo[track];
            }
            let rendered = render_with_samples(&project, &samples).unwrap();
            assert!(
                (rendered.samples[0] - one * playing).abs() < 1e-7,
                "{muted:?} {solo:?}"
            );
        }
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
    fn a_mix_reads_any_block_like_the_whole_render_and_has_no_length_limit() {
        let source = Pcm {
            sample_rate: 44_100,
            channels: 2,
            samples: (0..44_100 * 2)
                .map(|index| ((index % 199) as f32 / 199.0) - 0.5)
                .collect(),
        };
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        editor.place(1, 3, 100, 300).unwrap();
        editor.place(1, 63, 700, 256).unwrap();
        let samples = HashMap::from([(1, source.clone())]);
        let mix = mix_with_samples(editor.project(), &samples).unwrap();
        let whole = mix.to_pcm(&editor.project().master).unwrap();
        // Blocks of any size and position give the same samples, and silence past the end.
        for block_frames in [1, 7, 2048, 65_536] {
            let mut pieces = Vec::new();
            let mut frame = 0;
            while frame < mix.frames() {
                let mut block = vec![1.0; block_frames * 2];
                Frames::read(&mix, frame, &mut block);
                pieces.extend(block);
                frame += block_frames as u64;
            }
            assert_eq!(pieces[..whole.samples.len()], whole.samples[..]);
            assert!(pieces[whole.samples.len()..].iter().all(|x| *x == 0.0));
        }
        // The last bar of the timeline: about 2 hours at 140 BPM.
        let mut long = crate::engine::Editor::new(Project::default());
        let last_bar = crate::engine::MAX_TICKS - 256;
        long.place(1, 0, last_bar, 256).unwrap();
        let mix = mix_with_samples(long.project(), &samples).unwrap();
        assert!(mix.frames() > 300_000_000);
        assert!(mix.to_pcm(&long.project().master).is_err());
        let first = (f64::from(last_bar) * long.project().frames_per_tick(44_100)).round() as u64;
        let mut block = vec![0.0; 2048 * 2];
        Frames::read(&mix, first, &mut block);
        let gain = long.project().master_gain;
        assert_eq!(
            block[..8],
            source.samples[..8]
                .iter()
                .map(|x| x * gain)
                .collect::<Vec<_>>()[..]
        );
        let mut before = vec![1.0; 64];
        Frames::read(&mix, first - 32, &mut before);
        assert!(before.iter().all(|x| *x == 0.0));
    }
    #[test]
    fn playback_meters_include_the_final_output_frame() {
        let mut samples = vec![0.0; 1_024 * 2];
        samples[1_023 * 2] = 1.0;
        let audio: Arc<dyn Frames> = Arc::new(Pcm {
            sample_rate: 44_100,
            channels: 2,
            samples: samples.clone(),
        });
        let control = MasterControl::new(&MasterChain::default());
        let played: Vec<_> = PlaybackSource::new(audio, 0, None, Some(control.clone())).collect();
        assert_eq!(played, samples);
        let reading = control.meters();
        assert!(reading.true_peak_max >= 0.0, "{reading:?}");
        assert!(reading.overs > 0, "{reading:?}");
        assert_eq!(reading.integrated, f32::NEG_INFINITY);
    }

    #[test]
    fn live_pedal_switches_keep_the_timeline_and_final_frame() {
        for initially_on in [false, true] {
            let mut samples = vec![0.0; 10_000 * 2];
            for frame in [0, 7_000, 9_999] {
                samples[frame * 2] = 0.25;
                samples[frame * 2 + 1] = -0.25;
            }
            let audio: Arc<dyn Frames> = Arc::new(Pcm {
                sample_rate: 44_100,
                channels: 2,
                samples: samples.clone(),
            });
            let mut chain = MasterChain::default();
            chain.limiter.on = initially_on;
            chain.limiter.gain = 0.0;
            let control = MasterControl::new(&chain);
            let mut source = PlaybackSource::new(audio, 0, None, Some(control.clone()));
            let mut played: Vec<_> = source.by_ref().take(2_048 * 2).collect();
            chain.limiter.on = !initially_on;
            control.set(&chain);
            played.extend(source);
            assert_eq!(played.len(), samples.len());
            for (index, (played, expected)) in played.iter().zip(&samples).enumerate() {
                assert_eq!(
                    played, expected,
                    "Sample {index}; limiter initially on: {initially_on}"
                );
            }
        }
    }

    #[test]
    fn neutral_live_switches_crossfade_aligned_audio() {
        let samples: Vec<_> = (0..10_000)
            .flat_map(|frame| {
                let sample = (frame as f32 * 0.13).sin() * 0.25;
                [sample, -sample]
            })
            .collect();
        for initially_on in [false, true] {
            let audio: Arc<dyn Frames> = Arc::new(Pcm {
                sample_rate: 44_100,
                channels: 2,
                samples: samples.clone(),
            });
            let mut chain = MasterChain::default();
            chain.limiter.on = initially_on;
            chain.limiter.gain = 0.0;
            let control = MasterControl::new(&chain);
            let mut source = PlaybackSource::new(audio, 0, None, Some(control.clone()));
            let mut played: Vec<_> = source.by_ref().take(2_048 * 2).collect();
            chain.limiter.on = !initially_on;
            control.set(&chain);
            played.extend(source);
            assert_eq!(played.len(), samples.len());
            for (index, (played, expected)) in played.iter().zip(&samples).enumerate() {
                assert_eq!(
                    played, expected,
                    "Sample {index}; limiter initially on: {initially_on}"
                );
            }
        }
    }

    #[test]
    fn playback_and_export_run_the_same_master_pedalboard() {
        let source = Pcm {
            sample_rate: 44_100,
            channels: 2,
            samples: (0..20_000)
                .map(|i| ((i as f32) * 0.037).sin() * if i % 2 == 0 { 0.6 } else { 0.3 })
                .collect(),
        };
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        let mut project = editor.project().clone();
        let samples = HashMap::from([(1, source)]);
        let dry = render_with_samples(&project, &samples).unwrap();
        project.master.ejay_compressor.on = true;
        project.master.ejay_wide.on = true;
        project.master.ejay_eq.on = true;
        project.master.ejay_eq.bands[0] = 90;
        let exported = render_with_samples(&project, &samples).unwrap();
        assert_ne!(exported.samples, dry.samples);

        let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
        let played: Vec<f32> = PlaybackSource::new(
            mix.clone(),
            0,
            None,
            Some(MasterControl::new(&project.master)),
        )
        .collect();
        assert_eq!(played, exported.samples);
        // Without a control, as for a tool preview, the mix plays dry.
        let preview: Vec<f32> = PlaybackSource::new(mix, 0, None, None).collect();
        assert_eq!(preview, dry.samples);

        // Pedals that delay the sound still line up with the song: a limiter
        // with nothing to limit gives back the dry mix exactly.
        let mut delayed = editor.project().clone();
        delayed.master.limiter.on = true;
        delayed.master.limiter.gain = 0.0;
        assert_eq!(
            render_with_samples(&delayed, &samples).unwrap().samples,
            dry.samples
        );
        delayed.master.tube.on = true;
        delayed.master.limiter.gain = 6.0;
        let exported = render_with_samples(&delayed, &samples).unwrap();
        assert_eq!(exported.samples.len(), dry.samples.len());
        let mix = mix_with_samples(&delayed, &samples).unwrap();
        let path = std::env::temp_dir().join(format!("mixjam-master-{}.wav", std::process::id()));
        mix.write_wav(&path, &delayed.master).unwrap();
        let written = read_wav(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(written.samples.len(), exported.samples.len());
        assert!(
            written
                .samples
                .iter()
                .zip(&exported.samples)
                .all(|(a, b)| float_to_i16(*a) == float_to_i16(*b))
        );
        let played: Vec<f32> = PlaybackSource::new(
            Arc::new(mix),
            0,
            None,
            Some(MasterControl::new(&delayed.master)),
        )
        .collect();
        assert_eq!(played, exported.samples);
    }

    #[test]
    fn loop_source_keeps_channels_aligned_and_preserves_intro() {
        let pcm = Arc::new(Pcm {
            sample_rate: 44100,
            channels: 2,
            samples: vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7],
        });
        let source = PlaybackSource::new(pcm, 0, Some([1, 3]), None);
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
