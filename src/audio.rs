use crate::{
    automation::{Envelope, Filter},
    engine::{FadeCurve, Project, SOURCE_BPM},
    fx::{BUSES, FxBuses, ReturnsProcessor},
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

/// Stateful processing belongs to a reader, never to the shared audio data.
pub struct RenderState {
    filters: [Filter; crate::engine::TRACK_COUNT],
    next_frame: Option<u64>,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            filters: [Filter::default(); crate::engine::TRACK_COUNT],
            next_frame: None,
        }
    }
}

/// Audio that a player reads in blocks: decoded PCM or a mix made on demand.
pub trait Frames: Send + Sync {
    fn channels(&self) -> u16;
    fn sample_rate(&self) -> u32;
    fn frames(&self) -> u64;
    /// Fill `out` with interleaved samples from `first_frame` onward.
    /// Samples past the end are silent.
    fn read(&self, first_frame: u64, out: &mut [f32]);
    /// The tempo of a song mix, whose sends feed the FX buses.
    fn tempo(&self) -> Option<f64> {
        None
    }
    /// Fill `out` with the dry stereo mix, not yet held to full scale, and
    /// `sends` with the FX buses' feeds, `2 * BUSES` interleaved channels a
    /// frame. False, with `sends` untouched, when nothing in the block sends.
    fn read_sends(&self, first_frame: u64, out: &mut [f32], _sends: &mut [f32]) -> bool {
        self.read(first_frame, out);
        false
    }
    /// Read a block with history private to this playback or export.
    fn read_block(
        &self,
        first_frame: u64,
        out: &mut [f32],
        sends: Option<&mut [f32]>,
        _state: &mut RenderState,
    ) -> bool {
        match sends {
            Some(sends) => self.read_sends(first_frame, out, sends),
            None => {
                self.read(first_frame, out);
                false
            }
        }
    }
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
    /// The track's sends, applied after its gains.
    sends: [f32; BUSES],
    /// Fade lengths in frames. The fade out ends at `sound_end`, the clip's
    /// end or the earlier end of its source audio.
    fade_in: u64,
    fade_out: u64,
    curves: [FadeCurve; 2],
    sound_end: u64,
}

impl Voice {
    /// Add the voice to `out`, stereo frames from `first_frame`, and to
    /// `sends` when given.
    fn add(&self, first_frame: u64, out: &mut [f32], mut sends: Option<&mut [f32]>) {
        let block_end = first_frame + (out.len() / 2) as u64;
        let from = self.start.max(first_frame);
        let to = self.end.min(block_end);
        if from >= to {
            return;
        }
        let source = &*self.source;
        let source_frames = source.frames() as usize;
        let source_channels = source.channels as usize;
        let sending = sends.is_some() && self.sends.iter().any(|send| *send > 0.0);
        for frame in from..to {
            let pos = self.offset_frames as f64 + (frame - self.start) as f64 * self.step;
            let index = pos.floor() as usize;
            if index >= source_frames {
                break;
            }
            let next = (index + 1).min(source_frames - 1);
            let fraction = (pos - index as f64) as f32;
            let at = (frame - first_frame) as usize;
            let fade = self.fade(frame);
            for (channel, channel_gain) in self.channel_gains.iter().enumerate() {
                let Some(channel_gain) = channel_gain else {
                    continue;
                };
                let ch = if source_channels == 1 { 0 } else { channel };
                let a = source.samples[index * source_channels + ch];
                let b = source.samples[next * source_channels + ch];
                let value = (a + (b - a) * fraction) * channel_gain * fade;
                out[at * 2 + channel] += value;
                if sending && let Some(sends) = sends.as_deref_mut() {
                    for (bus, send) in self.sends.iter().enumerate() {
                        sends[at * 2 * BUSES + bus * 2 + channel] += value * send;
                    }
                }
            }
        }
    }

    /// The fade gain at an output frame within the clip.
    fn fade(&self, frame: u64) -> f32 {
        let mut gain = 1.0;
        let since = frame - self.start;
        if since < self.fade_in {
            gain = self.curves[0].gain((since as f32 + 0.5) / self.fade_in as f32);
        }
        let left = self.sound_end.saturating_sub(frame);
        if left <= self.fade_out {
            gain *= self.curves[1].gain((left as f32 - 0.5) / self.fade_out as f32);
        }
        gain
    }
}

/// A track send: the knob's fixed amount, or its automation lane.
enum SendLevel {
    Fixed(f32),
    Moving(Envelope),
}

impl SendLevel {
    fn at(&self, frame: u64) -> f32 {
        match self {
            Self::Fixed(level) => *level,
            Self::Moving(envelope) => envelope.at(frame),
        }
    }
}

/// A track with automation. Its voices mix into a short buffer that its
/// filter, volume, and sends then shape, in that order.
struct AutomatedTrack {
    voices: Vec<Voice>,
    volume: Option<Envelope>,
    sends: [SendLevel; BUSES],
    filter: Option<Envelope>,
}

impl AutomatedTrack {
    /// The stack buffer used to sum voices before frame-by-frame automation.
    const CHUNK: usize = 128;

    fn new(track: &crate::engine::Track, frames_per_tick: f64) -> Self {
        let automation = &track.automation;
        Self {
            voices: Vec::new(),
            volume: Envelope::new(&automation.volume, frames_per_tick),
            sends: std::array::from_fn(|bus| {
                Envelope::new(&automation.sends[bus], frames_per_tick)
                    .map_or(SendLevel::Fixed(track.sends[bus]), SendLevel::Moving)
            }),
            filter: Envelope::new(&automation.filter, frames_per_tick),
        }
    }

    fn sends(&self) -> bool {
        self.sends.iter().any(|send| match send {
            SendLevel::Fixed(level) => *level > 0.0,
            SendLevel::Moving(envelope) => envelope.any_positive(),
        })
    }

    /// Add the track to `out` and `sends`. Runs on the audio thread: the
    /// buffer is on the stack.
    fn add(
        &self,
        first_frame: u64,
        out: &mut [f32],
        mut sends: Option<&mut [f32]>,
        filter: &mut Filter,
    ) {
        let frames = out.len() / 2;
        let mut buffer = [0.0_f32; Self::CHUNK * 2];
        for chunk in (0..frames).step_by(Self::CHUNK) {
            let count = Self::CHUNK.min(frames - chunk);
            let at = first_frame + chunk as u64;
            let buffer = &mut buffer[..count * 2];
            buffer.fill(0.0);
            for voice in &self.voices {
                voice.add(at, buffer, None);
            }
            for index in 0..count {
                let frame = at + index as u64;
                if let Some(envelope) = &self.filter {
                    filter.process(
                        &mut buffer[index * 2..index * 2 + 2],
                        envelope.at(frame),
                        Mix::SAMPLE_RATE as f32,
                    );
                }
                let gain = self.volume.as_ref().map_or(1.0, |envelope| {
                    crate::automation::volume_gain(envelope.at(frame))
                });
                for channel in 0..2 {
                    let value = buffer[index * 2 + channel] * gain;
                    buffer[index * 2 + channel] = value;
                    out[(chunk + index) * 2 + channel] += value;
                }
            }
            let Some(sends) = sends.as_deref_mut() else {
                continue;
            };
            for (bus, send) in self.sends.iter().enumerate() {
                for index in 0..count {
                    let level = send.at(at + index as u64);
                    for channel in 0..2 {
                        sends[(chunk + index) * 2 * BUSES + bus * 2 + channel] +=
                            buffer[index * 2 + channel] * level;
                    }
                }
            }
        }
    }
}

/// A project prepared for rendering. It holds the decoded sources and mixes
/// any range of the song on demand, so playback and export need no buffer of
/// the whole song and a song can be as long as the timeline.
pub struct Mix {
    frames: u64,
    voices: Vec<Voice>,
    /// The tracks with automation, which hold their own voices.
    automated: Vec<AutomatedTrack>,
    bpm: f64,
    /// Whether any voice sends to an FX bus.
    sends: bool,
}

impl Mix {
    pub const SAMPLE_RATE: u32 = 44_100;
    /// The longest mix that `to_pcm` holds in memory: about 12 minutes.
    const MAX_PCM_FRAMES: u64 = 32_000_000;

    /// `samples` holds each source at each pitch that a clip plays it at.
    fn new(
        project: &Project,
        samples: &HashMap<(u32, i32), Arc<Pcm>>,
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
        let mut automated = Vec::new();
        let mut automated_index = [None; crate::engine::TRACK_COUNT];
        let any_solo = project.any_solo();
        for clip in &project.clips {
            if missing.contains(&clip.sample_id) {
                continue;
            }
            let source = samples
                .get(&(clip.sample_id, clip.pitch_cents))
                .ok_or_else(|| format!("Sample {} is missing.", clip.sample_id))?;
            if validated.insert((clip.sample_id, clip.pitch_cents)) {
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
            let frames_of = |ticks: u32| (f64::from(ticks) * frames_per_tick).round() as u64;
            let source_left =
                (source.frames() as f64 - f64::from(clip.offset_frames)).max(0.0) / step;
            // Pan and channel enables depend only on the track: resolve them once per clip.
            let voice = Voice {
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
                sends: track.sends,
                fade_in: frames_of(clip.fades.fade_in.ticks),
                fade_out: frames_of(clip.fades.fade_out.ticks),
                curves: [clip.fades.fade_in.curve, clip.fades.fade_out.curve],
                sound_end: end.min(start + source_left.ceil() as u64),
            };
            if track.automation.is_empty() {
                voices.push(voice);
                continue;
            }
            let index = *automated_index[clip.track].get_or_insert_with(|| {
                automated.push(AutomatedTrack::new(track, frames_per_tick));
                automated.len() - 1
            });
            automated[index].voices.push(voice);
        }
        let sends = voices
            .iter()
            .any(|voice| voice.sends.iter().any(|send| *send > 0.0))
            || automated.iter().any(AutomatedTrack::sends);
        Ok(Self {
            frames,
            voices,
            automated,
            bpm: project.bpm,
            sends,
        })
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// The whole mix in memory, through the FX buses and the master
    /// pedalboard. Long songs play and export through `read` and
    /// `write_wav`, which have no length limit. Pedals that delay the sound
    /// run on past the end, and the delay is cut from the start, so the
    /// result lines up with the song; the buses' tails ring on after it.
    pub fn to_pcm(&self, master: &MasterChain, fx: &FxBuses) -> Result<Pcm, String> {
        if self.frames > Self::MAX_PCM_FRAMES {
            return Err("This song is too long to hold in memory. Play or export it.".into());
        }
        let mut samples = Vec::new();
        self.render(master, fx, |block| {
            samples.extend_from_slice(block);
            Ok(())
        })?;
        Ok(Pcm {
            sample_rate: Self::SAMPLE_RATE,
            channels: 2,
            samples,
        })
    }

    /// Mix the song block by block through the buses and the board and hand
    /// each finished block to `write`. The output starts with the song's
    /// first frame and ends after the buses' tails, at most `TAIL_LIMIT`
    /// frames past the song.
    fn render(
        &self,
        master: &MasterChain,
        fx: &FxBuses,
        mut write: impl FnMut(&[f32]) -> Result<(), String>,
    ) -> Result<(), String> {
        const BLOCK_FRAMES: u64 = 65_536;
        /// Tails go in playback's short blocks, so the output ends soon after
        /// they die, at the same frame as playback's.
        const TAIL_BLOCK: u64 = PlaybackSource::BLOCK_FRAMES as u64;
        let mut returns =
            (self.sends && fx.is_active()).then(|| ReturnsProcessor::new(fx, self.bpm));
        let mut master = MasterProcessor::new(master);
        // The pedals run on past the end by their delay, which is cut
        // from the start.
        let latency = master.latency() as u64;
        let mut block = vec![0.0; BLOCK_FRAMES as usize * 2];
        let mut sends = vec![0.0; BLOCK_FRAMES as usize * 2 * BUSES];
        // Frames fed into the board so far, and of them the tail's.
        let (mut position, mut tail) = (0, 0);
        let mut state = RenderState::default();
        loop {
            let ringing = returns.as_ref().is_some_and(ReturnsProcessor::ringing)
                && tail < crate::fx::TAIL_LIMIT;
            let count = if position < self.frames {
                (self.frames - position).min(BLOCK_FRAMES)
            } else if ringing {
                (crate::fx::TAIL_LIMIT - tail).min(TAIL_BLOCK)
            } else if position < self.frames + tail + latency {
                (self.frames + tail + latency - position).min(BLOCK_FRAMES)
            } else {
                return Ok(());
            } as usize;
            let samples = &mut block[..count * 2];
            if position < self.frames {
                match &mut returns {
                    Some(returns) => {
                        let sends = &mut sends[..count * 2 * BUSES];
                        let sending = self.read_block(position, samples, Some(sends), &mut state);
                        returns.process(samples, sending.then_some(&*sends));
                    }
                    None => {
                        self.read_block(position, samples, None, &mut state);
                    }
                }
            } else {
                samples.fill(0.0);
                if let Some(returns) = returns.as_mut().filter(|_| ringing) {
                    returns.process(samples, None);
                    tail += count as u64;
                }
            }
            master.process(samples);
            let skip = latency.saturating_sub(position).min(count as u64) as usize;
            write(&samples[skip * 2..])?;
            position += count as u64;
        }
    }

    /// Write stereo PCM16 through the FX buses and the master pedalboard,
    /// mixing one block at a time.
    pub fn write_wav(&self, path: &Path, master: &MasterChain, fx: &FxBuses) -> Result<(), String> {
        // Both the data chunk and the RIFF size are 32-bit. Stereo PCM16
        // uses four bytes per frame plus 36 bytes in the RIFF payload.
        let tail = if self.sends && fx.is_active() {
            crate::fx::TAIL_LIMIT
        } else {
            0
        };
        if self.frames + tail > (u64::from(u32::MAX) - 36) / 4 {
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
            self.render(master, fx, |block| {
                block.iter().try_for_each(|&sample| {
                    writer
                        .write_sample(float_to_i16(sample))
                        .map_err(|e| e.to_string())
                })
            })?;
            writer.finalize().map_err(|e| e.to_string())
        })
    }

    /// Mix voices into `out`, and into `sends` when given and any voice sends.
    /// Runs on the audio thread during playback: no allocation and no locks.
    fn mix(
        &self,
        first_frame: u64,
        out: &mut [f32],
        mut sends: Option<&mut [f32]>,
        state: &mut RenderState,
    ) {
        if state.next_frame != Some(first_frame) {
            state.filters.fill(Filter::default());
        }
        out.fill(0.0);
        if let Some(sends) = sends.as_deref_mut() {
            sends.fill(0.0);
        }
        for voice in &self.voices {
            voice.add(first_frame, out, sends.as_deref_mut());
        }
        for (track, filter) in self.automated.iter().zip(&mut state.filters) {
            track.add(first_frame, out, sends.as_deref_mut(), filter);
        }
        state.next_frame = Some(first_frame + (out.len() / 2) as u64);
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
        self.read_block(first_frame, out, None, &mut RenderState::default());
    }
    fn read_block(
        &self,
        first_frame: u64,
        out: &mut [f32],
        sends: Option<&mut [f32]>,
        state: &mut RenderState,
    ) -> bool {
        let want_sends = sends.is_some();
        self.mix(
            first_frame,
            out,
            if self.sends { sends } else { None },
            state,
        );
        // Saturation is the reconstruction policy. Original mixer rounding and resampling remain to be measured.
        if !want_sends {
            for x in out {
                *x = x.clamp(-1.0, 1.0);
            }
        }
        self.sends && want_sends
    }
    fn tempo(&self) -> Option<f64> {
        Some(self.bpm)
    }
    fn read_sends(&self, first_frame: u64, out: &mut [f32], sends: &mut [f32]) -> bool {
        self.read_block(first_frame, out, Some(sends), &mut RenderState::default())
    }
}

/// Render a whole project in memory. Use `prepare_project` for a song of any length.
pub fn render_project(project: &Project, library: &Library) -> Result<Pcm, String> {
    prepare_project(project, library)?.to_pcm(&project.master, &project.fx)
}

/// Decode a project's sources and prepare its mix.
pub fn prepare_project(project: &Project, library: &Library) -> Result<Mix, String> {
    prepare_project_reusing(project, library, &mut HashMap::new())
}

/// A decoded source at one clip pitch in cents. Pitch 0 is the source as it is.
pub(crate) type SourceKey = (u32, i32);

/// Fill `decoded` with exactly the sources and pitches that the project's
/// clips play. Entries already there are reused; a pitched copy is made from
/// the plain source.
fn load_sources(
    project: &Project,
    decoded: &mut HashMap<SourceKey, Arc<Pcm>>,
    mut read: impl FnMut(u32) -> Result<Pcm, String>,
) -> Result<(), String> {
    let missing: std::collections::HashSet<_> = project
        .missing_samples
        .iter()
        .map(|sample| sample.id)
        .collect();
    let used: std::collections::BTreeSet<SourceKey> = project
        .clips
        .iter()
        .map(|clip| (clip.sample_id, clip.pitch_cents))
        .filter(|(id, _)| !missing.contains(id))
        .collect();
    decoded.retain(|key, _| used.contains(key));
    for &(id, cents) in &used {
        if decoded.contains_key(&(id, cents)) {
            continue;
        }
        let plain = match decoded.get(&(id, 0)) {
            Some(plain) => Arc::clone(plain),
            None => Arc::new(read(id)?),
        };
        let source = if cents == 0 {
            plain
        } else {
            Arc::new(crate::sample_edit::repitch(&plain, cents)?)
        };
        decoded.insert((id, cents), source);
    }
    Ok(())
}

/// Prepare a mix with the sources that an earlier call for the same library
/// decoded. `decoded` then holds exactly the sources of this project, so its
/// size follows the song. The caller clears it when the library changes.
pub(crate) fn prepare_project_reusing(
    project: &Project,
    library: &Library,
    decoded: &mut HashMap<SourceKey, Arc<Pcm>>,
) -> Result<Mix, String> {
    project.validate()?;
    load_sources(project, decoded, |id| library.read_pcm(id))?;
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
    let mut decoded = HashMap::new();
    load_sources(project, &mut decoded, |id| {
        samples
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("Sample {id} is missing."))
    })?;
    Mix::new(project, &decoded, &source_tempos)
}

#[cfg(test)]
pub(crate) fn render_with_samples(
    project: &Project,
    samples: &HashMap<u32, Pcm>,
) -> Result<Pcm, String> {
    mix_with_samples(project, samples)?.to_pcm(&project.master, &project.fx)
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
    state: RenderState,
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
    /// The FX buses of a song mix, with the block of their feeds. Their
    /// tails ring on after the song, as in export.
    returns: Option<ReturnsProcessor>,
    sends: Vec<f32>,
    /// Frames of the buses' tail played after the end.
    ringing: u64,
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
        let master = master.filter(|_| channels == 2);
        let returns = master
            .as_ref()
            .zip(audio.tempo())
            .map(|(control, bpm)| ReturnsProcessor::live(Arc::clone(control), bpm));
        let master = master.map(MasterProcessor::live);
        let latency = master.as_ref().map_or(0, MasterProcessor::latency);
        let sends = if returns.is_some() {
            vec![0.0; Self::BLOCK_FRAMES * 2 * BUSES]
        } else {
            Vec::new()
        };
        Self {
            state: RenderState::default(),
            returns,
            sends,
            ringing: 0,
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
                } else if let Some(returns) = self
                    .returns
                    .as_mut()
                    .filter(|returns| returns.ringing() && self.ringing < crate::fx::TAIL_LIMIT)
                {
                    let count =
                        Self::BLOCK_FRAMES.min((crate::fx::TAIL_LIMIT - self.ringing) as usize);
                    self.ringing += count as u64;
                    self.filled = count * self.channels;
                    self.block[..self.filled].fill(0.0);
                    returns.process(&mut self.block[..self.filled], None);
                    if self.process() {
                        return true;
                    }
                    continue;
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
            match &mut self.returns {
                Some(returns) => {
                    let sends = &mut self.sends[..count * 2 * BUSES];
                    let sending = self.audio.read_block(
                        self.frame,
                        &mut self.block[..self.filled],
                        Some(sends),
                        &mut self.state,
                    );
                    returns.process(&mut self.block[..self.filled], sending.then_some(&*sends));
                }
                None => {
                    self.audio.read_block(
                        self.frame,
                        &mut self.block[..self.filled],
                        None,
                        &mut self.state,
                    );
                }
            }
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
    #[test]
    fn wav_export_rejects_riff_size_overflow_before_opening_output() {
        // The data fits u32, but adding the 36-byte RIFF payload header does not.
        let mix = Mix {
            frames: (u64::from(u32::MAX) - 36) / 4 + 1,
            voices: Vec::new(),
            automated: Vec::new(),
            bpm: 140.0,
            sends: false,
        };
        // An invalid destination proves that admission runs before file creation,
        // without writing a multi-gigabyte fixture on a broken implementation.
        let error = mix
            .write_wav(Path::new(""), &MasterChain::default(), &FxBuses::default())
            .unwrap_err();
        assert_eq!(error, "This song is too long for a WAV file.");
    }

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
                fades: Default::default(),
                pitch_cents: 0,
            }],
            ..Project::default()
        }
    }

    #[test]
    fn review_export_during_playback_does_not_change_filter_history() {
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        project.tracks[0].automation.filter = vec![crate::automation::Point {
            tick: 0,
            value: -0.8,
        }];
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![0.5; 75_600],
            },
        )]);
        let mix = Arc::new(mix_with_samples(&project, &samples).unwrap());
        let baseline: Vec<_> = PlaybackSource::new(mix.clone(), 0, None, None).collect();
        let mut playback = PlaybackSource::new(mix.clone(), 0, None, None);
        let mut actual: Vec<_> = playback.by_ref().take(2048).collect();
        mix.to_pcm(&project.master, &project.fx).unwrap();
        actual.extend(playback);
        assert!(
            actual == baseline,
            "export changed the playing filter history"
        );
    }

    #[test]
    fn review_automation_reaches_breakpoints_inside_a_render_chunk() {
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        project.tracks[0].automation.sends[0] = [(0, 0.0), (1, 1.0), (2, 0.0)]
            .map(|(tick, value)| crate::automation::Point { tick, value })
            .into();
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![1.0; 1000],
            },
        )]);
        let mix = mix_with_samples(&project, &samples).unwrap();
        let mut out = vec![0.0; 1024 * 2];
        let mut sends = vec![0.0; 1024 * 2 * BUSES];
        mix.read_sends(0, &mut out, &mut sends);
        assert_eq!(sends[295 * 2 * BUSES], 0.5);
    }

    #[test]
    fn automation_readers_are_independent_of_block_sizes_and_reset_after_seek() {
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        project.tracks[0].automation.filter = [(0, -0.9), (8, -0.2), (16, 0.4)]
            .map(|(tick, value)| crate::automation::Point { tick, value })
            .into();
        project.tracks[0].automation.volume = [(0, -20.0), (1, 0.0), (5, -6.0)]
            .map(|(tick, value)| crate::automation::Point { tick, value })
            .into();
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: (0..6000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect(),
            },
        )]);
        let mix = mix_with_samples(&project, &samples).unwrap();
        let mut expected = vec![0.0; 12_000];
        mix.read_block(0, &mut expected, None, &mut RenderState::default());
        let mut state = RenderState::default();
        let mut actual = vec![0.0; expected.len()];
        let mut start = 0;
        for size in [1, 131, 79, 2049, 3, 997].into_iter().cycle() {
            let end = (start + size).min(actual.len() / 2);
            mix.read_block(
                start as u64,
                &mut actual[start * 2..end * 2],
                None,
                &mut state,
            );
            start = end;
            if start == actual.len() / 2 {
                break;
            }
        }
        assert!(
            actual == expected,
            "block sizes changed the automation samples"
        );
        let mut seeked = [0.0; 512];
        let mut fresh = [0.0; 512];
        mix.read_block(1000, &mut seeked, None, &mut state);
        mix.read_block(1000, &mut fresh, None, &mut RenderState::default());
        assert_eq!(seeked, fresh);
    }

    #[test]
    fn review_dry_wav_admission_keeps_the_full_riff_capacity() {
        let mix = Mix {
            frames: (u64::from(u32::MAX) - 36) / 4,
            voices: Vec::new(),
            automated: Vec::new(),
            bpm: 140.0,
            sends: false,
        };
        let error = mix
            .write_wav(Path::new(""), &MasterChain::default(), &FxBuses::default())
            .unwrap_err();
        assert_ne!(error, "This song is too long for a WAV file.");
    }

    #[test]
    fn review_playback_and_export_stop_at_the_same_tail_limit() {
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        project.tracks[0].sends[0] = 1.0;
        project.fx.buses[0].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
            mode: crate::fx::TimeMode::Free,
            time_ms_l: 10.0,
            time_ms_r: 10.0,
            feedback: 110.0,
            character: crate::fx::DelayCharacter::Digital,
            mod_depth: 0.0,
            duck_amount: 0.0,
            ..crate::fx::Echoform::default()
        });
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: (0..4410).map(|i| (i as f32 * 0.05).sin() * 0.5).collect(),
            },
        )]);
        let mix = Arc::new(mix_with_samples(&project, &samples).unwrap());
        let exported = mix.to_pcm(&project.master, &project.fx).unwrap();
        let control = MasterControl::new(&project.master);
        control.set_returns(&project.fx);
        let played: Vec<_> = PlaybackSource::new(mix, 0, None, Some(control)).collect();
        assert_eq!(played.len(), exported.samples.len());
        assert!(played == exported.samples, "playback differs from export");
    }

    #[test]
    fn zero_level_returns_have_identical_playback_and_export_duration() {
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        project.tracks[0].sends[0] = 1.0;
        project.fx.buses[0].return_level = 0.0;
        project.fx.buses[0].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
            feedback: 0.0,
            mod_depth: 0.0,
            ..crate::fx::Echoform::default()
        });
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![0.5; 100],
            },
        )]);
        let mix = Arc::new(mix_with_samples(&project, &samples).unwrap());
        let exported = mix.to_pcm(&project.master, &project.fx).unwrap();
        let control = MasterControl::new(&project.master);
        control.set_returns(&project.fx);
        let played: Vec<_> = PlaybackSource::new(mix, 0, None, Some(control)).collect();
        assert_eq!(played.len(), exported.samples.len());
        assert!(played == exported.samples);
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
    fn clip_fades_shape_the_clip_and_the_fade_out_ends_with_its_sound() {
        use crate::engine::{Fade, FadeCurve, Fades};
        // One bar of sound in a two-bar clip: 75,600 frames at 140 BPM.
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        project.clips[0].length_ticks = 2 * TICKS_PER_BAR;
        project.clips[0].fades = Fades {
            fade_in: Fade {
                ticks: 64,
                curve: FadeCurve::Linear,
            },
            fade_out: Fade {
                ticks: 64,
                curve: FadeCurve::Exponential,
            },
        };
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![0.5; 75_600],
            },
        )]);
        let at = |project: &Project, frame: u64| {
            let mut out = [0.0; 2];
            Frames::read(
                &mix_with_samples(project, &samples).unwrap(),
                frame,
                &mut out,
            );
            out[0]
        };
        // A quarter note is 18,900 frames.
        assert!(at(&project, 0) < 1e-4);
        assert!((at(&project, 9_450) - 0.125).abs() < 1e-4);
        assert_eq!(at(&project, 30_000), 0.25);
        let middle = 0.25 * (1000.0_f32.sqrt() - 1.0) / 999.0;
        assert!((at(&project, 75_600 - 9_450) - middle).abs() < 1e-4);
        assert!(at(&project, 75_599) < 1e-4);
        assert_eq!(at(&project, 75_600), 0.0);
        // A clip shorter than its sound fades out to the clip's end.
        project.clips[0].length_ticks = 3 * TICKS_PER_BAR / 4;
        project.clips[0].fades.fade_out.curve = FadeCurve::Linear;
        assert!((at(&project, 56_700 - 9_450) - 0.125).abs() < 1e-4);
        assert!(at(&project, 56_699) < 1e-4);
        assert_eq!(at(&project, 28_350), 0.25);
    }

    #[test]
    fn a_pitched_clip_plays_higher_in_the_same_time() {
        // One bar of a 441 Hz tone at 140 BPM, played an octave up.
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        project.clips[0].pitch_cents = 1200;
        let tone = Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: (0..75_600)
                .map(|frame| (std::f32::consts::TAU * 441.0 * frame as f32 / 44_100.0).sin() * 0.5)
                .collect(),
        };
        let rendered = render_with_samples(&project, &HashMap::from([(5, tone)])).unwrap();
        let left: Vec<f32> = rendered.samples.iter().step_by(2).copied().collect();
        let crossings = |range: std::ops::Range<usize>| {
            left[range.clone()]
                .windows(2)
                .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
                .count() as f64
                * 44_100.0
                / range.len() as f64
        };
        let measured = crossings(10_000..60_000);
        assert!((measured - 882.0).abs() < 4.0, "{measured}");
        // The sound still fills its bar and stops where it did.
        assert!(left[70_000..75_000].iter().any(|sample| sample.abs() > 0.2));
        assert!(left[75_600..].iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn automation_moves_the_volume_filter_and_sends_of_its_track() {
        use crate::automation::Point;
        let point = |tick, value| Point { tick, value };
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        let tone = |hz: f32| Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: (0..75_600)
                .map(|frame| (std::f32::consts::TAU * hz * frame as f32 / 44_100.0).sin())
                .collect(),
        };
        let level = |project: &Project, hz: f32, from: u64, sends: bool| {
            let mix = mix_with_samples(project, &HashMap::from([(5, tone(hz))])).unwrap();
            let mut out = vec![0.0; 2048];
            let mut feeds = vec![0.0; 1024 * 2 * BUSES];
            let sending = mix.read_sends(from, &mut out, &mut feeds);
            let rms = |values: Vec<f32>| {
                (values.iter().map(|x| x * x).sum::<f32>() / values.len() as f32).sqrt()
            };
            if sends {
                assert!(sending);
                rms(feeds
                    .iter()
                    .skip(2 * 2)
                    .step_by(2 * BUSES)
                    .copied()
                    .collect())
            } else {
                rms(out.iter().step_by(2).copied().collect())
            }
        };
        let plain = level(&project, 440.0, 30_000, false);
        let later = level(&project, 440.0, 40_000, false);
        // Volume rises from silence to unity over the first half bar.
        project.tracks[0].automation.volume = vec![point(0, -60.0), point(128, 0.0)];
        assert!(level(&project, 440.0, 0, false) < plain * 0.01);
        let halfway = level(&project, 440.0, 18_900 - 512, false);
        assert!((halfway / plain - 0.0316).abs() < 0.01, "{halfway}");
        assert!((level(&project, 440.0, 40_000, false) - later).abs() < 1e-6);
        // The low-pass removes a high tone and keeps a low one.
        project.tracks[0].automation.volume.clear();
        project.tracks[0].automation.filter = vec![point(0, -0.75)];
        assert!(level(&project, 5000.0, 30_000, false) < plain * 0.02);
        assert!(level(&project, 60.0, 30_000, false) > plain * 0.8);
        // A send lane replaces the knob of its bus only.
        project.tracks[0].automation.filter.clear();
        project.tracks[0].sends[2] = 0.0;
        project.tracks[0].automation.sends[2] = vec![point(0, 0.5)];
        assert!((level(&project, 440.0, 30_000, true) - plain * 0.5).abs() < 1e-4);
    }

    #[test]
    fn fade_curves_rise_from_silence_to_full() {
        use crate::engine::{FadeCurve, Fades};
        for curve in FadeCurve::ALL {
            assert_eq!(curve.gain(0.0), 0.0);
            assert!((curve.gain(1.0) - 1.0).abs() < 1e-6);
            let steps: Vec<_> = (0..=100).map(|t| curve.gain(t as f32 / 100.0)).collect();
            assert!(steps.windows(2).all(|pair| pair[1] > pair[0]), "{curve:?}");
        }
        assert!((FadeCurve::EqualPower.gain(0.5).powi(2) - 0.5).abs() < 1e-6);
        let mut fades = Fades::default();
        fades.fade_in.ticks = 300;
        fades.fade_out.ticks = 100;
        let fitted = fades.fit(256);
        assert_eq!((fitted.fade_in.ticks, fitted.fade_out.ticks), (256, 0));
        let fitted = fades.fit(350);
        assert_eq!((fitted.fade_in.ticks, fitted.fade_out.ticks), (300, 50));
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
        let whole = mix
            .to_pcm(&editor.project().master, &editor.project().fx)
            .unwrap();
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
        assert!(
            mix.to_pcm(&long.project().master, &long.project().fx)
                .is_err()
        );
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
    fn sends_feed_the_fx_buses_whose_tails_ring_past_the_song_in_export_and_playback() {
        let source = Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: (0..4410).map(|i| ((i as f32) * 0.05).sin() * 0.5).collect(),
        };
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        let mut project = editor.project().clone();
        let samples = HashMap::from([(1, source)]);
        let dry = render_with_samples(&project, &samples).unwrap();
        // A bus with an effect but no sends changes nothing.
        project.fx.buses[0].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
            mode: crate::fx::TimeMode::Free,
            time_ms_l: 500.0,
            time_ms_r: 500.0,
            feedback: 40.0,
            duck_amount: 0.0,
            ..crate::fx::Echoform::default()
        });
        assert_eq!(
            render_with_samples(&project, &samples).unwrap().samples,
            dry.samples
        );
        project.tracks[0].sends[0] = 0.8;
        let wet = render_with_samples(&project, &samples).unwrap();
        // The song is one bar at 140 BPM; the echoes ring on after it.
        assert!(wet.samples.len() > dry.samples.len() + 44_100);
        let echo = &wet.samples[2 * 22_050..2 * 26_460];
        assert!(
            echo.iter().any(|v| v.abs() > 0.05),
            "the first echo is half a second in"
        );
        let end = &wet.samples[wet.samples.len() - 2048..];
        assert!(
            end.iter().all(|v| v.abs() < 1e-3),
            "the tail dies out before the end"
        );

        let control = MasterControl::new(&project.master);
        control.set_returns(&project.fx);
        let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
        let played: Vec<f32> = PlaybackSource::new(mix, 0, None, Some(control)).collect();
        assert_eq!(played, wet.samples);
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
        mix.write_wav(&path, &delayed.master, &delayed.fx).unwrap();
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
