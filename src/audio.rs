use crate::{
    automation::{Envelope, Filter},
    engine::{FadeCurve, Project, SOURCE_BPM},
    eq::{Curve, Filters},
    fx::{BUSES, FxBuses, ReturnsProcessor},
    library::Library,
    master::{MasterChain, MasterControl, MasterProcessor},
};
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::SyncSender,
    },
    time::Duration,
};

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

/// Frames at the end of a sound that can be a stray.
const STRAY_END_FRAMES: usize = 4;

/// Near silence, in normalized units: the 4 frames before a stray run.
const STRAY_SILENCE: f32 = 0.001;

/// Hide a stray end: many original product samples are silent or quiet and
/// then end on one or two frames that jump to a large value, which clicks
/// wherever the sample ends or loops. Per channel, a run of at most four
/// final frames is stray when all its frames rise above four times the peak
/// of the ten milliseconds before the last four frames, or above twice that
/// peak when the four frames just before the run are near silence, and above
/// 1% of full scale in both cases. It takes the value of the frame before
/// the run. A single last frame is also stray when its step from the frame
/// before is over four times the largest step in that window (and over 5% of
/// full scale). Real tails, such as a sound cut while loud, are as loud and
/// as jumpy as the audio before them and stay. The frame count never changes.
/// Playback and render read audio through this. Sample identity must not.
pub(crate) fn hide_stray_end(pcm: &mut Pcm) {
    let channels = usize::from(pcm.channels.max(1));
    let frames = pcm.samples.len() / channels;
    let window = (0.010 * f64::from(pcm.sample_rate)).round() as usize;
    if frames < window + STRAY_END_FRAMES + 1 || frames < 2 * STRAY_END_FRAMES {
        return;
    }
    for channel in 0..channels {
        let at = |frame: usize| frame * channels + channel;
        let peak = (frames - STRAY_END_FRAMES - window..frames - STRAY_END_FRAMES)
            .map(|frame| pcm.samples[at(frame)].abs())
            .fold(0.0, f32::max);
        let above = |threshold: f32| {
            (1..=STRAY_END_FRAMES)
                .take_while(|back| pcm.samples[at(frames - back)].abs() > threshold)
                .count()
        };
        let mut run = above((4.0 * peak).max(0.01));
        let loose = above((2.0 * peak).max(0.01));
        for candidate in (run + 1..=loose).rev() {
            let silent = (frames - candidate - STRAY_END_FRAMES..frames - candidate)
                .all(|frame| pcm.samples[at(frame)].abs() <= STRAY_SILENCE);
            if silent {
                run = candidate;
                break;
            }
        }
        if run == 0 {
            // A lone last frame that steps far more than anything in the
            // window steps, whatever the tail before it was doing.
            let steps = (frames - STRAY_END_FRAMES - window + 1..frames - STRAY_END_FRAMES)
                .map(|frame| (pcm.samples[at(frame)] - pcm.samples[at(frame - 1)]).abs())
                .fold(0.0, f32::max);
            let last = (pcm.samples[at(frames - 1)] - pcm.samples[at(frames - 2)]).abs();
            if last > (4.0 * steps).max(0.05) {
                run = 1;
            }
        }
        if run > 0 {
            let hold = pcm.samples[at(frames - run - 1)];
            for frame in frames - run..frames {
                pcm.samples[at(frame)] = hold;
            }
        }
    }
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
    chains: [ChainState; crate::engine::TRACK_COUNT],
    /// The key tracks' own filter histories, apart from their mixed ones.
    keys: [ChainState; crate::engine::MAX_KEYS],
    /// Each chain's ducker, in the order of the mix's chains.
    duckers: [crate::ducker::State; crate::engine::TRACK_COUNT],
    /// The last block's key tracks for the master pedals and the FX buses,
    /// as `Keys::audio` holds them.
    key_audio: Vec<f32>,
    next_frame: Option<u64>,
    /// Each track's glide to its level, as the last block left it.
    levels: [Ramp; crate::engine::TRACK_COUNT],
    /// The glide of the master gain inside those levels, in both channels.
    master_gain: Ramp,
    /// Playback's live levels. Without them the mix plays the levels it was
    /// prepared with.
    live: Option<Arc<MasterControl>>,
    /// The loop that playback wraps through: a read from its start right
    /// after its end is a loop pass.
    loop_region: Option<[u64; 2]>,
    compressor_epoch: Option<u64>,
}

/// How far behind the frame where playback kept a compressor's history a
/// new playback start may begin and still carry on from it. Playback renders
/// ahead of what it has played by up to a block and the master pedals'
/// delay. The cursor that a new start begins at is at most one tick (345
/// frames at 120 BPM) and, after a pause, one window update (about 30 ms)
/// behind what has played. Together that is less than 3300 frames.
const CARRY_FRAMES: u64 = 4096;

/// The frames over which a track's level glides to a moved fader, pan, or
/// master gain during playback: about 23 ms, so the move does not click.
const GLIDE_FRAMES: u64 = 1024;

/// Each track's left and right level: fader, polarity, pan, and the master gain.
pub(crate) fn track_levels(project: &Project) -> [[f32; 2]; crate::engine::TRACK_COUNT] {
    std::array::from_fn(|index| {
        let track = &project.tracks[index];
        let gain = track.effective_gain() * project.master_gain;
        [
            gain * (1.0 - track.pan.max(0.0)),
            gain * (1.0 + track.pan.min(0.0)),
        ]
    })
}

/// A track's left and right level: a straight glide over `GLIDE_FRAMES`
/// from the level it had reached when its target last changed, so a moved
/// fader does not click. The glide runs on across playback's blocks.
#[derive(Clone, Copy)]
struct Ramp {
    /// The first frame of the glide, which already moves one step.
    first_frame: u64,
    frames: f32,
    from: [f32; 2],
    to: [f32; 2],
}

impl Ramp {
    const UNITY: Self = Self::still([1.0; 2]);

    const fn still(level: [f32; 2]) -> Self {
        Self {
            first_frame: 0,
            frames: 1.0,
            from: level,
            to: level,
        }
    }

    fn at(&self, frame: u64, channel: usize) -> f32 {
        let (from, to) = (self.from[channel], self.to[channel]);
        if from == to {
            return to;
        }
        let along = ((frame - self.first_frame) as f32 + 1.0) / self.frames;
        if along >= 1.0 {
            return to;
        }
        from + (to - from) * along
    }

    /// Carry the glide into a block from `first_frame`. A new target starts
    /// a new glide from the level reached before that frame; a playback that
    /// does not continue from the last block starts at the target.
    fn advance(&mut self, first_frame: u64, continues: bool, target: [f32; 2]) {
        if !continues {
            *self = Self::still(target);
        } else if target != self.to {
            let reached = if first_frame > self.first_frame {
                [0, 1].map(|channel| self.at(first_frame - 1, channel))
            } else {
                self.from
            };
            *self = Self {
                first_frame,
                frames: GLIDE_FRAMES as f32,
                from: reached,
                to: target,
            };
        } else if first_frame.saturating_sub(self.first_frame) as f32 >= self.frames {
            // The glide is over: hold the target.
            self.from = self.to;
        }
    }
}

/// The histories of one track's chain: its EQ, its compressor, and its
/// automation filter.
#[derive(Clone, Copy, Default)]
struct ChainState {
    eq: Filters,
    compressor: crate::compressor::State,
    filter: Filter,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            chains: [ChainState::default(); crate::engine::TRACK_COUNT],
            keys: [ChainState::default(); crate::engine::MAX_KEYS],
            duckers: [crate::ducker::State::default(); crate::engine::TRACK_COUNT],
            key_audio: Vec::new(),
            next_frame: None,
            levels: [Ramp::still([0.0; 2]); crate::engine::TRACK_COUNT],
            master_gain: Ramp::still([0.0; 2]),
            live: None,
            loop_region: None,
            compressor_epoch: None,
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
    /// Read a block with history private to this playback or export. A mix
    /// fills `out` with the dry stereo mix, not yet held to full scale: the
    /// master processor holds it after the board's trim. With `sends` it
    /// fills `sends` with the FX buses' feeds, `2 * BUSES` interleaved
    /// channels a frame. False, with `sends` untouched, when nothing in the
    /// block sends.
    fn read_block(
        &self,
        first_frame: u64,
        out: &mut [f32],
        _sends: Option<&mut [f32]>,
        _state: &mut RenderState,
    ) -> bool {
        self.read(first_frame, out);
        false
    }
    /// The key tracks of the first `frames` frames that the last
    /// `read_block` with `state` read, for the master pedals and the FX
    /// buses.
    fn keys<'a>(&'a self, _state: &'a RenderState, _frames: usize) -> crate::ducker::Keys<'a> {
        crate::ducker::Keys::NONE
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
#[derive(Clone)]
struct Voice {
    track: usize,
    source: Arc<Pcm>,
    start: u64,
    end: u64,
    offset_frames: u32,
    step: f64,
    /// The clip gain for the left and right outputs; `None` for a disabled
    /// output. The track's level comes in at mix time.
    channel_gains: [Option<f32>; 2],
    /// The track's sends, applied after its level.
    sends: [f32; BUSES],
    /// Fade lengths in frames. The fade out ends at `sound_end`, the clip's
    /// end or the earlier end of its source audio.
    fade_in: u64,
    fade_out: u64,
    curves: [FadeCurve; 2],
    sound_end: u64,
}

impl Voice {
    /// Add the voice at `level` to `out`, stereo frames from `first_frame`,
    /// and to `sends` when given.
    fn add(&self, first_frame: u64, out: &mut [f32], mut sends: Option<&mut [f32]>, level: &Ramp) {
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
                let value =
                    (a + (b - a) * fraction) * channel_gain * fade * level.at(frame, channel);
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

/// A track with its EQ or compressor on, a width other than 100%, with
/// automation, with its ducker on, or that keys a ducker. Its voices mix
/// into a short buffer that its EQ, compressor, width, filter, volume,
/// ducker, level, and sends then shape, in that order.
struct TrackChain {
    track: usize,
    voices: Vec<Voice>,
    eq: Option<Curve>,
    compressor: Option<crate::compressor::Run>,
    /// The side's gain: the width as a fraction of 100%.
    width: Option<f32>,
    volume: Option<Envelope>,
    sends: [SendLevel; BUSES],
    filter: Option<Envelope>,
    /// The ducker and the slot of its key among the mix's keys.
    ducker: Option<(crate::ducker::Run, usize)>,
}

impl TrackChain {
    /// The stack buffer used to sum voices before frame-by-frame automation.
    const CHUNK: usize = 128;

    fn new(index: usize, track: &crate::engine::Track, frames_per_tick: f64) -> Self {
        let automation = &track.automation;
        Self {
            track: index,
            voices: Vec::new(),
            eq: Curve::of(&track.eq),
            compressor: track
                .compressor
                .on
                .then(|| crate::compressor::Run::new(&track.compressor)),
            width: track.widens().then_some(track.width / 100.0),
            volume: Envelope::new(&automation.volume, frames_per_tick),
            sends: std::array::from_fn(|bus| {
                Envelope::new(&automation.sends[bus], frames_per_tick)
                    .map_or(SendLevel::Fixed(track.sends[bus]), SendLevel::Moving)
            }),
            filter: Envelope::new(&automation.filter, frames_per_tick),
            ducker: None,
        }
    }

    /// Whether the track ever sends to a bus.
    fn feeds(&self, bus: usize) -> bool {
        match &self.sends[bus] {
            SendLevel::Fixed(level) => *level > 0.0,
            SendLevel::Moving(envelope) => envelope.any_positive(),
        }
    }

    /// Fill `buffer`, stereo frames from `at`, with the track as it sounds:
    /// its voices through its EQ, compressor, width, filter, volume, ducker,
    /// and level. `duck` holds the key's level at each frame and the
    /// ducker's history.
    fn shape(
        &self,
        at: u64,
        buffer: &mut [f32],
        state: &mut ChainState,
        level: &Ramp,
        mut duck: Option<(&[f32], &mut crate::ducker::State)>,
    ) {
        buffer.fill(0.0);
        for voice in &self.voices {
            voice.add(at, buffer, None, &Ramp::UNITY);
        }
        if let Some(curve) = &self.eq {
            state.eq.process(buffer, curve);
        }
        if let Some(compressor) = &self.compressor {
            compressor.process(buffer, &mut state.compressor);
        }
        if let Some(width) = self.width {
            widen(buffer, width);
        }
        for index in 0..buffer.len() / 2 {
            let frame = at + index as u64;
            if let Some(envelope) = &self.filter {
                state.filter.process(
                    &mut buffer[index * 2..index * 2 + 2],
                    envelope.at(frame),
                    Mix::SAMPLE_RATE as f32,
                );
            }
            let mut gain = self.volume.as_ref().map_or(1.0, |envelope| {
                crate::automation::volume_gain(envelope.at(frame))
            });
            if let (Some((run, _)), Some((keys, history))) = (&self.ducker, duck.as_mut()) {
                gain *= run.gain(keys[index], history);
            }
            for channel in 0..2 {
                buffer[index * 2 + channel] =
                    buffer[index * 2 + channel] * gain * level.at(frame, channel);
            }
        }
    }

    /// Add `count` frames of the track, from `at`, to `out` and `sends` at
    /// frame `offset` of the block. Runs on the audio thread: the buffer is
    /// on the stack.
    #[allow(clippy::too_many_arguments)]
    fn add(
        &self,
        at: u64,
        offset: usize,
        count: usize,
        out: &mut [f32],
        sends: Option<&mut [f32]>,
        state: &mut ChainState,
        level: &Ramp,
        duck: Option<(&[f32], &mut crate::ducker::State)>,
    ) {
        let mut buffer = [0.0_f32; Self::CHUNK * 2];
        let buffer = &mut buffer[..count * 2];
        self.shape(at, buffer, state, level, duck);
        for (sum, value) in out[offset * 2..(offset + count) * 2]
            .iter_mut()
            .zip(buffer.iter())
        {
            *sum += value;
        }
        let Some(sends) = sends else {
            return;
        };
        for (bus, send) in self.sends.iter().enumerate() {
            for index in 0..count {
                let level = send.at(at + index as u64);
                for channel in 0..2 {
                    sends[(offset + index) * 2 * BUSES + bus * 2 + channel] +=
                        buffer[index * 2 + channel] * level;
                }
            }
        }
    }
}

/// Scale the side of interleaved stereo frames by `width`, 1 for no change,
/// and keep the mid: 0 folds the frames to mono.
fn widen(frames: &mut [f32], width: f32) {
    for frame in frames.as_chunks_mut::<2>().0 {
        let mid = 0.5 * (frame[0] + frame[1]);
        let side = 0.5 * (frame[0] - frame[1]) * width;
        *frame = [mid + side, mid - side];
    }
}

/// A project prepared for rendering. It holds the decoded sources and mixes
/// any range of the song on demand, so playback and export need no buffer of
/// the whole song and a song can be as long as the timeline.
pub struct Mix {
    frames: u64,
    voices: Vec<Voice>,
    /// The tracks with their EQ or compressor on, a width other than 100%,
    /// with automation, or with a ducker on or that key one, which hold
    /// their own voices.
    chains: Vec<TrackChain>,
    /// The key tracks of the duckers, the master pedals, and the FX buses,
    /// in key slot order, each in a chain of its own that only measures it.
    keys: Vec<TrackChain>,
    /// The track of each key slot, and whether the master pedals or the FX
    /// buses listen to any, so that the mix keeps their audio.
    key_tracks: Vec<usize>,
    side: bool,
    /// The track levels the mix was prepared with.
    levels: [[f32; 2]; crate::engine::TRACK_COUNT],
    /// The master gain inside those levels. A track ducker hears its key
    /// without it, so the master gain does not change how far it ducks.
    master_gain: f32,
    bpm: f64,
    /// Whether any voice sends to each FX bus, and to any.
    fed: [bool; BUSES],
    sends: bool,
}

/// The number of files that `Mix::write_stems` wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stems {
    pub tracks: usize,
    pub returns: usize,
}

impl std::fmt::Display for Stems {
    /// The counts as a status line names them, such as "2 tracks and 1 FX return".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = |count: usize, one: &str| {
            if count == 1 {
                format!("1 {one}")
            } else {
                format!("{count} {one}s")
            }
        };
        write!(
            f,
            "{} and {}",
            count(self.tracks, "track"),
            count(self.returns, "FX return")
        )
    }
}

impl Mix {
    pub const SAMPLE_RATE: u32 = 44_100;
    /// The longest mix that `to_pcm` holds in memory: about 12 minutes.
    const MAX_PCM_FRAMES: u64 = 32_000_000;
    /// The block that export mixes at a time during the song.
    const EXPORT_BLOCK: u64 = 65_536;
    /// Tails go in short blocks, so an export ends soon after they die.
    /// Playback asks whether the buses still ring once a tail block too, so
    /// it ends at the same frame.
    const TAIL_BLOCK: u64 = 1024;

    /// `samples` holds each source at each pitch that a clip plays it at, as
    /// `load_sources` checked it.
    fn new(
        project: &Project,
        samples: &HashMap<SourceKey, Arc<Pcm>>,
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
        let mut voices = Vec::new();
        let mut chains = Vec::new();
        let mut chain_index = [None; crate::engine::TRACK_COUNT];
        let any_solo = project.any_solo();
        // A ducker works when its own track sounds; its key may be silent.
        let ducking = |index: usize| {
            let track = &project.tracks[index];
            track.ducker.on
                && track.audible(any_solo)
                && (track.left_enabled || track.right_enabled)
        };
        let mut keys: Vec<TrackChain> = Vec::new();
        let side_keys = project.side_keys();
        let ducker_keys = (0..crate::engine::TRACK_COUNT)
            .filter(|&index| ducking(index))
            .map(|index| project.tracks[index].ducker.key_track());
        for key in ducker_keys.chain(side_keys.iter().copied()) {
            if !keys.iter().any(|chain| chain.track == key) {
                keys.push(TrackChain::new(key, &project.tracks[key], frames_per_tick));
            }
        }
        for clip in &project.clips {
            if missing.contains(&clip.sample_id) {
                continue;
            }
            let source = samples
                .get(&(clip.sample_id, clip.pitch_cents))
                .ok_or_else(|| format!("Sample {} is missing.", clip.sample_id))?;
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
            let step = source.sample_rate as f64 / rate as f64 * project.bpm
                / project.sample_playback_bpm(clip.sample_id, source_bpm);
            if !step.is_finite() || step <= 0.0 {
                return Err("The source tempo cannot be represented for sample playback.".into());
            }
            let frames_of = |ticks: u32| (f64::from(ticks) * frames_per_tick).round() as u64;
            let source_left =
                (source.frames() as f64 - f64::from(clip.offset_frames)).max(0.0) / step;
            // Channel enables depend only on the track: resolve them once per clip.
            let voice = Voice {
                track: clip.track,
                source: Arc::clone(source),
                start,
                end,
                offset_frames: clip.offset_frames,
                step,
                channel_gains: [
                    track.left_enabled.then_some(clip.gain),
                    track.right_enabled.then_some(clip.gain),
                ],
                sends: track.sends,
                fade_in: frames_of(clip.fades.fade_in.ticks),
                fade_out: frames_of(clip.fades.fade_out.ticks),
                curves: [clip.fades.fade_in.curve, clip.fades.fade_out.curve],
                sound_end: end.min(start + source_left.ceil() as u64),
            };
            let keyed = keys.iter().position(|chain| chain.track == clip.track);
            if let Some(slot) = keyed {
                keys[slot].voices.push(voice.clone());
            }
            // A track whose EQ and compressor are off, at 100% width, that
            // has no automation, and that neither ducks nor keys a ducker
            // mixes its voices straight into the sum, as before the EQ
            // existed.
            if track.automation.is_empty()
                && !track.eq.on
                && !track.compressor.on
                && !track.widens()
                && !ducking(clip.track)
                && keyed.is_none()
            {
                voices.push(voice);
                continue;
            }
            let index = *chain_index[clip.track].get_or_insert_with(|| {
                let mut chain = TrackChain::new(clip.track, track, frames_per_tick);
                if ducking(clip.track) {
                    let key = track.ducker.key_track();
                    let slot = keys.iter().position(|chain| chain.track == key);
                    chain.ducker = slot
                        .map(|slot| (crate::ducker::Run::new(&track.ducker, rate as f32), slot));
                }
                chains.push(chain);
                chains.len() - 1
            });
            chains[index].voices.push(voice);
        }
        let fed = std::array::from_fn(|bus| {
            voices.iter().any(|voice| voice.sends[bus] > 0.0)
                || chains.iter().any(|chain| chain.feeds(bus))
        });
        Ok(Self {
            frames,
            voices,
            chains,
            key_tracks: keys.iter().map(|chain| chain.track).collect(),
            side: !side_keys.is_empty(),
            keys,
            levels: track_levels(project),
            master_gain: project.master_gain,
            bpm: project.bpm,
            fed,
            sends: fed.contains(&true),
        })
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Whether the mix was prepared with the project's track levels.
    /// Playback reads them live; an export needs them in the mix.
    pub fn has_levels(&self, project: &Project) -> bool {
        self.levels == track_levels(project)
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
        const BLOCK_FRAMES: u64 = Mix::EXPORT_BLOCK;
        const TAIL_BLOCK: u64 = Mix::TAIL_BLOCK;
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
            let keyed = if position < self.frames {
                match &mut returns {
                    Some(returns) => {
                        let sends = &mut sends[..count * 2 * BUSES];
                        let sending = self.read_block(position, samples, Some(sends), &mut state);
                        returns.process_keyed(
                            samples,
                            sending.then_some(&*sends),
                            self.key_block(&state, count),
                        );
                    }
                    None => {
                        self.read_block(position, samples, None, &mut state);
                    }
                }
                count
            } else {
                samples.fill(0.0);
                if let Some(returns) = returns.as_mut().filter(|_| ringing) {
                    returns.process(samples, None);
                    tail += count as u64;
                }
                0
            };
            master.process_keyed(samples, self.key_block(&state, keyed));
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

    /// The tracks that sound in this mix, in order: audible tracks with clips
    /// whose audio is available.
    fn sounding_tracks(&self) -> Vec<usize> {
        let mut tracks: Vec<usize> = self
            .voices
            .iter()
            .map(|voice| voice.track)
            .chain(self.chains.iter().map(|chain| chain.track))
            .collect();
        tracks.sort_unstable();
        tracks.dedup();
        tracks
    }

    /// The FX buses with a return in this mix: those with an effect and
    /// their input on that a track sends to.
    fn returning_buses(&self, fx: &FxBuses) -> Vec<usize> {
        (0..BUSES)
            .filter(|&bus| self.fed[bus] && fx.buses[bus].powered && fx.buses[bus].is_active())
            .collect()
    }

    /// Write the premaster in parts to stereo PCM16 files in `folder`: each
    /// sounding track alone, after its EQ, compressor, width, automation,
    /// fader, pan, ducker, and
    /// outputs but without its sends, and the return of each bus in
    /// `returning_buses`. Every file runs from the song's start until the
    /// buses' tails die, as the mix export does. If the summed mix does not
    /// clip, the files sum to the mix with the master pedalboard bypassed.
    /// An out-of-range part rejects before any file is replaced. Each file
    /// is named after `song`.
    pub fn write_stems(&self, folder: &Path, song: &str, fx: &FxBuses) -> Result<Stems, String> {
        let tracks = self.sounding_tracks();
        if tracks.is_empty() {
            return Err("No track sounds: add clips, or check Mute, Solo, and L/R.".into());
        }
        let buses = self.returning_buses(fx);
        let tail = if buses.is_empty() {
            0
        } else {
            crate::fx::TAIL_LIMIT
        };
        if self.frames + tail > (u64::from(u32::MAX) - 36) / 4 {
            return Err("This song is too long for a WAV file.".into());
        }
        let song = file_name_part(song);
        let paths: Vec<PathBuf> = tracks
            .iter()
            .map(|track| format!("{song} - Track {:02}.wav", track + 1))
            .chain(
                buses
                    .iter()
                    .map(|bus| format!("{song} - FX {} return.wav", bus + 1)),
            )
            .map(|name| folder.join(name))
            .collect();
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: Self::SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        crate::atomic_file::write_many(&paths, |files| {
            let mut writers = files
                .iter_mut()
                .map(|file| hound::WavWriter::new(std::io::BufWriter::new(file), spec))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            self.render_stems(fx, &tracks, &buses, |index, block| {
                if let Some(sample) = block
                    .iter()
                    .find(|sample| !sample.is_finite() || sample.abs() > 1.0)
                {
                    let (part, level) = if index < tracks.len() {
                        (format!("Track {}", tracks[index] + 1), "track gain")
                    } else {
                        (
                            format!("FX {} return", buses[index - tracks.len()] + 1),
                            "FX return level",
                        )
                    };
                    return Err(if sample.is_finite() {
                        format!(
                            "{part} exceeds the 16-bit WAV range. Lower the {level} \
                             or master gain, then export again."
                        )
                    } else {
                        format!(
                            "{part} contains invalid audio. Check its source and effects, \
                             then export again."
                        )
                    });
                }
                block.iter().try_for_each(|&sample| {
                    writers[index]
                        .write_sample(float_to_i16(sample))
                        .map_err(|e| e.to_string())
                })
            })?;
            writers
                .into_iter()
                .try_for_each(|writer| writer.finalize().map_err(|e| e.to_string()))
        })?;
        Ok(Stems {
            tracks: tracks.len(),
            returns: buses.len(),
        })
    }

    /// Render the parts of `write_stems` block by block, in the blocks of
    /// `render`, and hand each to `write` with its part: the index of its
    /// track in `tracks`, then of its bus in `buses` after them.
    fn render_stems(
        &self,
        fx: &FxBuses,
        tracks: &[usize],
        buses: &[usize],
        mut write: impl FnMut(usize, &[f32]) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut returns =
            (self.sends && fx.is_active()).then(|| ReturnsProcessor::new(fx, self.bpm));
        let block_frames = Self::EXPORT_BLOCK as usize;
        let mut dry = vec![0.0; block_frames * 2];
        let mut feed = vec![0.0; block_frames * 2 * BUSES];
        let mut sends = vec![0.0; block_frames * 2 * BUSES];
        let mut wet = vec![0.0; block_frames * 2 * BUSES];
        let mut part = vec![0.0; block_frames * 2];
        // Each track keeps its own filter histories, as a playback would.
        let mut states: Vec<RenderState> = tracks.iter().map(|_| RenderState::default()).collect();
        let (mut position, mut tail) = (0, 0);
        loop {
            let ringing = returns.as_ref().is_some_and(ReturnsProcessor::ringing)
                && tail < crate::fx::TAIL_LIMIT;
            let count = if position < self.frames {
                (self.frames - position).min(Self::EXPORT_BLOCK)
            } else if ringing {
                (crate::fx::TAIL_LIMIT - tail).min(Self::TAIL_BLOCK)
            } else {
                return Ok(());
            } as usize;
            let dry = &mut dry[..count * 2];
            let wet = &mut wet[..count * 2 * BUSES];
            if position < self.frames {
                let sends = &mut sends[..count * 2 * BUSES];
                sends.fill(0.0);
                for (index, (&track, state)) in tracks.iter().zip(&mut states).enumerate() {
                    if returns.is_some() {
                        let feed = &mut feed[..count * 2 * BUSES];
                        self.mix(position, dry, Some(feed), state, Some(track));
                        for (sum, value) in sends.iter_mut().zip(feed.iter()) {
                            *sum += value;
                        }
                    } else {
                        self.mix(position, dry, None, state, Some(track));
                    }
                    write(index, dry)?;
                }
                if let Some(returns) = &mut returns {
                    returns.split(wet, Some(sends), self.key_block(&states[0], count));
                }
            } else {
                dry.fill(0.0);
                for index in 0..tracks.len() {
                    write(index, dry)?;
                }
                if let Some(returns) = &mut returns {
                    returns.split(wet, None, crate::ducker::Keys::NONE);
                    tail += count as u64;
                }
            }
            let part = &mut part[..count * 2];
            for (offset, &bus) in buses.iter().enumerate() {
                for (frame, out) in part.as_chunks_mut::<2>().0.iter_mut().enumerate() {
                    let at = frame * 2 * BUSES + bus * 2;
                    *out = [wet[at], wet[at + 1]];
                }
                write(tracks.len() + offset, part)?;
            }
            position += count as u64;
        }
    }

    /// The key tracks that the master pedals and the FX buses listen to, for
    /// the first `frames` frames of the block that `state` last mixed. None
    /// when nothing listens.
    fn key_block<'a>(&'a self, state: &'a RenderState, frames: usize) -> crate::ducker::Keys<'a> {
        if !self.side || frames == 0 {
            return crate::ducker::Keys::NONE;
        }
        crate::ducker::Keys {
            tracks: &self.key_tracks,
            audio: &state.key_audio[..frames * 2 * self.key_tracks.len()],
        }
    }

    /// Mix voices into `out`, and into `sends` when given and any voice sends.
    /// `only` mixes one track alone. Runs on the audio thread during
    /// playback: no allocation and no locks.
    fn mix(
        &self,
        first_frame: u64,
        out: &mut [f32],
        mut sends: Option<&mut [f32]>,
        state: &mut RenderState,
        only: Option<usize>,
    ) {
        if state.compressor_epoch.is_none() {
            state.compressor_epoch = state.live.as_ref().map(|live| live.compressor_epoch());
        }
        let continues = state.next_frame == Some(first_frame);
        if !continues {
            // Playback's compressors carry on where playback continues: a
            // loop pass keeps the history from the loop's end, and a new
            // playback just behind where the last one kept its history,
            // after a pause or for a mix prepared again such as for each
            // step of a compressor dial, starts from that history. Starting
            // without reduction would play the track up to its full
            // reduction louder until the attack caught up. Any other start,
            // and the song's start, which follows silence as in export,
            // begins from silence like the other histories. A key chain
            // carries its track's history, so the key matches the track.
            let carries = first_frame > 0;
            let wraps = state
                .loop_region
                .is_some_and(|[start, end]| first_frame == start && state.next_frame == Some(end));
            for history in state.chains.iter_mut().chain(&mut state.keys) {
                *history = ChainState {
                    compressor: if wraps {
                        history.compressor
                    } else {
                        Default::default()
                    },
                    ..ChainState::default()
                };
            }
            state.duckers.fill(crate::ducker::State::default());
            if carries
                && !wraps
                && let Some(live) = &state.live
            {
                for (chain, history) in (self.chains.iter().zip(&mut state.chains))
                    .chain(self.keys.iter().zip(&mut state.keys))
                {
                    if chain.compressor.is_some()
                        && let Some((kept, at)) = live.kept_compressor(chain.track)
                        && (first_frame..=first_frame + CARRY_FRAMES).contains(&at)
                    {
                        history.compressor = kept;
                    }
                }
            }
        }
        let (targets, master_gain) = state
            .live
            .as_ref()
            .and_then(|live| live.levels())
            .unwrap_or_else(|| {
                if state.next_frame.is_some() {
                    (state.levels.map(|ramp| ramp.to), state.master_gain.to[0])
                } else {
                    (self.levels, self.master_gain)
                }
            });
        for (ramp, target) in state.levels.iter_mut().zip(targets) {
            ramp.advance(first_frame, continues, target);
        }
        let levels = state.levels;
        out.fill(0.0);
        if let Some(sends) = sends.as_deref_mut() {
            sends.fill(0.0);
        }
        let included = |track: usize| only.is_none_or(|only| only == track);
        for voice in self.voices.iter().filter(|voice| included(voice.track)) {
            voice.add(first_frame, out, sends.as_deref_mut(), &levels[voice.track]);
        }
        // Chunk by chunk, so that each ducker reads its key over the same
        // frames. The keys are measured whether or not `only` mixes them.
        // A ducker's key level leaves out the master gain; the key audio for
        // the master pedals and the FX buses keeps it, as their input does.
        // The master gain glides as the track levels do, so the key stays
        // free of it while a live master gain moves.
        state
            .master_gain
            .advance(first_frame, continues, [master_gain; 2]);
        let mastered = state.master_gain;
        let frames = out.len() / 2;
        let stride = 2 * self.keys.len();
        if self.side && state.key_audio.len() < frames * stride {
            state.key_audio.resize(frames * stride, 0.0);
        }
        for offset in (0..frames).step_by(TrackChain::CHUNK) {
            let count = TrackChain::CHUNK.min(frames - offset);
            let at = first_frame + offset as u64;
            let mut keys = [[0.0_f32; TrackChain::CHUNK]; crate::engine::MAX_KEYS];
            for (slot, ((chain, history), key)) in self
                .keys
                .iter()
                .zip(&mut state.keys)
                .zip(&mut keys)
                .enumerate()
            {
                let mut buffer = [0.0_f32; TrackChain::CHUNK * 2];
                let buffer = &mut buffer[..count * 2];
                chain.shape(at, buffer, history, &levels[chain.track], None);
                for (index, (level, &[left, right])) in
                    key.iter_mut().zip(buffer.as_chunks::<2>().0).enumerate()
                {
                    let master_gain = mastered.at(at + index as u64, 0);
                    *level = if master_gain > 0.0 {
                        left.abs().max(right.abs()) / master_gain
                    } else {
                        0.0
                    };
                    if self.side {
                        let at = (offset + index) * stride + 2 * slot;
                        state.key_audio[at..at + 2].copy_from_slice(&[left, right]);
                    }
                }
            }
            for ((chain, history), ducker) in self
                .chains
                .iter()
                .zip(&mut state.chains)
                .zip(&mut state.duckers)
            {
                if included(chain.track) {
                    let duck = chain.ducker.map(|(_, slot)| (&keys[slot][..count], ducker));
                    chain.add(
                        at,
                        offset,
                        count,
                        out,
                        sends.as_deref_mut(),
                        history,
                        &levels[chain.track],
                        duck,
                    );
                }
            }
        }
        let next_frame = first_frame + frames as u64;
        if let Some(live) = &state.live {
            for (chain, history) in self.chains.iter().zip(&state.chains) {
                if chain.compressor.is_some() && included(chain.track) {
                    live.keep_compressor(
                        chain.track,
                        history.compressor,
                        next_frame,
                        state.compressor_epoch.unwrap(),
                    );
                }
            }
        }
        state.next_frame = Some(next_frame);
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
    /// A read that starts the filters from silence and holds the mix to full
    /// scale, as no board follows it. Playback and export call `read_block`
    /// with their own `RenderState` instead.
    fn read(&self, first_frame: u64, out: &mut [f32]) {
        self.read_block(first_frame, out, None, &mut RenderState::default());
        hold_to_full_scale(out);
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
            None,
        );
        self.sends && want_sends
    }
    fn keys<'a>(&'a self, state: &'a RenderState, frames: usize) -> crate::ducker::Keys<'a> {
        self.key_block(state, frames)
    }
    fn tempo(&self) -> Option<f64> {
        Some(self.bpm)
    }
}

/// Hold every sample to full scale. Saturation is the reconstruction
/// policy; the original mixer's rounding and resampling remain to be
/// measured. The master processor does this after the board's trim.
pub(crate) fn hold_to_full_scale(samples: &mut [f32]) {
    for sample in samples {
        *sample = sample.clamp(-1.0, 1.0);
    }
}

/// A song title made safe for a file name: characters that Windows does
/// not allow in one become underscores.
fn file_name_part(title: &str) -> String {
    let name: String = title
        .chars()
        .map(|c| {
            if c.is_control() || r#"<>:"/\|?*"#.contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_end_matches('.');
    if name.is_empty() {
        crate::engine::DEFAULT_TITLE.into()
    } else {
        name.into()
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
/// clips play. Entries already there are reused. Each source that is not
/// there is read and checked once, and each pitched copy is made from the
/// plain source. Both steps share out the sources among threads.
fn load_sources(
    project: &Project,
    decoded: &mut HashMap<SourceKey, Arc<Pcm>>,
    read: impl Fn(u32) -> Result<Pcm, String> + Sync,
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
    let new: Vec<SourceKey> = used
        .into_iter()
        .filter(|key| !decoded.contains_key(key))
        .collect();
    let mut unread: Vec<u32> = new
        .iter()
        .map(|&(id, _)| id)
        .filter(|id| !decoded.contains_key(&(*id, 0)))
        .collect();
    unread.dedup();
    let sources = in_parallel(&unread, |&id| {
        let source = read(id)?;
        source.validate()?;
        Ok(Arc::new(source))
    });
    let mut plains = HashMap::new();
    let mut error = None;
    for (id, source) in unread.into_iter().zip(sources) {
        match source {
            Ok(source) => {
                plains.insert(id, source);
            }
            Err(message) => {
                error.get_or_insert(message);
            }
        }
    }
    // Pitch the longest sources first, so that the last thread to finish
    // does not start a long one late.
    let mut pitched: Vec<(SourceKey, Arc<Pcm>)> = new
        .iter()
        .filter(|&&(_, cents)| cents != 0)
        .filter_map(|&(id, cents)| {
            let plain = plains.get(&id).or_else(|| decoded.get(&(id, 0)))?;
            Some(((id, cents), Arc::clone(plain)))
        })
        .collect();
    pitched.sort_by_key(|(_, plain)| std::cmp::Reverse(plain.samples.len()));
    let copies = in_parallel(&pitched, |((_, cents), plain)| {
        crate::sample_edit::repitch(plain, *cents).map(Arc::new)
    });
    for ((key, _), copy) in pitched.into_iter().zip(copies) {
        match copy {
            Ok(copy) => {
                decoded.insert(key, copy);
            }
            Err(message) => {
                error.get_or_insert(message);
            }
        }
    }
    for (id, plain) in plains {
        if new.binary_search(&(id, 0)).is_ok() {
            decoded.insert((id, 0), plain);
        }
    }
    error.map_or(Ok(()), Err)
}

/// `work` applied to each item, in the items' order. The items are shared
/// out among all but one of the processor's cores, so that playback keeps
/// one while a mix is prepared.
fn in_parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(1, |count| count.get().saturating_sub(1))
        .clamp(1, items.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<R>>> =
        items.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    let result = work(item);
                    *results[index].lock().unwrap() = Some(result);
                }
            });
        }
    });
    results
        .into_iter()
        .map(|result| {
            result
                .into_inner()
                .unwrap()
                .expect("every item has a result")
        })
        .collect()
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
    current: Option<Current>,
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
    /// The frames of a whole block: `BLOCK_FRAMES` but in tests.
    block_frames: usize,
    /// The next sample in `block`, and the count of valid samples in it.
    cursor: usize,
    filled: usize,
    /// The master pedalboard of a stereo mix. Its state runs on through loops.
    master: Option<MasterProcessor>,
    /// The FX buses of a song mix, with the block of their feeds. Their
    /// tails ring on after the song, as in export.
    returns: Option<ReturnsProcessor>,
    sends: Vec<f32>,
    /// Frames of the buses' tail played after the end, and of them those
    /// left of the current `Mix::TAIL_BLOCK`.
    ringing: u64,
    ringing_block: u64,
    /// Frames of the pedals' delay still to drop at the start, so the song
    /// starts on time, and still to play out after the end.
    lead: usize,
    tail: usize,
    /// Frames of the block that the key tracks cover: none after the song.
    keyed: usize,
    /// Whether the thread that reads this source has been asked to run at
    /// an audio thread's priority.
    raised: bool,
}
impl PlaybackSource {
    /// About 5.8 ms. The device's callback, every 10 ms or so, renders the
    /// blocks it needs inside its own period, so a block's work must be a
    /// small part of the period: a callback then renders one or two blocks.
    /// The level glides, the meters, and the tails keep their own timing
    /// (`GLIDE_FRAMES`, `MasterProcessor`, `Mix::TAIL_BLOCK`).
    const BLOCK_FRAMES: usize = 256;

    fn new(
        audio: Arc<dyn Frames>,
        start: u64,
        loop_region: Option<[u64; 2]>,
        master: Option<Arc<MasterControl>>,
    ) -> Self {
        Self::with_block(audio, start, loop_region, master, Self::BLOCK_FRAMES)
    }

    /// A source that renders blocks of `block_frames` frames.
    fn with_block(
        audio: Arc<dyn Frames>,
        start: u64,
        loop_region: Option<[u64; 2]>,
        master: Option<Arc<MasterControl>>,
        block_frames: usize,
    ) -> Self {
        let channels = audio.channels() as usize;
        let master = master.filter(|_| channels == 2);
        let live = master.clone();
        let returns = master
            .as_ref()
            .zip(audio.tempo())
            .map(|(control, bpm)| ReturnsProcessor::live(Arc::clone(control), bpm));
        let master = master.map(MasterProcessor::live);
        let latency = master.as_ref().map_or(0, MasterProcessor::latency);
        let sends = if returns.is_some() {
            vec![0.0; block_frames * 2 * BUSES]
        } else {
            Vec::new()
        };
        Self {
            state: RenderState {
                compressor_epoch: live.as_ref().map(|live| live.compressor_epoch()),
                live,
                loop_region,
                // Allocate key scratch before the device reads this source.
                // A short first block must also leave room for a full block.
                key_audio: if audio.tempo().is_some() {
                    vec![0.0; block_frames * 2 * crate::engine::MAX_KEYS]
                } else {
                    Vec::new()
                },
                ..RenderState::default()
            },
            returns,
            sends,
            ringing: 0,
            ringing_block: 0,
            channels,
            start,
            frame: start,
            end: loop_region.map_or(audio.frames(), |[_, b]| b),
            loop_start: loop_region.map(|[a, _]| a),
            block: vec![0.0; block_frames * channels],
            block_frames,
            cursor: 0,
            filled: 0,
            master,
            lead: latency,
            tail: latency,
            keyed: 0,
            raised: false,
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
                } else if let Some(returns) = self.returns.as_mut().filter(|returns| {
                    // As in export, whether the buses still ring is asked
                    // once a tail block, so playback ends at its frame.
                    self.ringing_block > 0
                        || returns.ringing() && self.ringing < crate::fx::TAIL_LIMIT
                }) {
                    if self.ringing_block == 0 {
                        self.ringing_block =
                            Mix::TAIL_BLOCK.min(crate::fx::TAIL_LIMIT - self.ringing);
                    }
                    let count = self.block_frames.min(self.ringing_block as usize);
                    self.ringing += count as u64;
                    self.ringing_block -= count as u64;
                    self.filled = count * self.channels;
                    self.keyed = 0;
                    self.block[..self.filled].fill(0.0);
                    returns.process(&mut self.block[..self.filled], None);
                    if self.process() {
                        return true;
                    }
                    continue;
                } else if self.tail > 0 {
                    let count = self.tail.min(self.block_frames);
                    self.tail -= count;
                    self.filled = count * self.channels;
                    self.keyed = 0;
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
            let count = ((self.end - self.frame) as usize).min(self.block_frames);
            if count == 0 {
                return false;
            }
            self.filled = count * self.channels;
            self.keyed = count;
            match &mut self.returns {
                Some(returns) => {
                    let sends = &mut self.sends[..count * 2 * BUSES];
                    let sending = self.audio.read_block(
                        self.frame,
                        &mut self.block[..self.filled],
                        Some(sends),
                        &mut self.state,
                    );
                    returns.process_keyed(
                        &mut self.block[..self.filled],
                        sending.then_some(&*sends),
                        self.audio.keys(&self.state, count),
                    );
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
        match &mut self.master {
            Some(master) => master.process_keyed(
                &mut self.block[..self.filled],
                self.audio.keys(&self.state, self.keyed),
            ),
            None => hold_to_full_scale(&mut self.block[..self.filled]),
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
        if self.cursor == self.filled {
            if !self.raised {
                self.raised = true;
                raise_priority_once();
            }
            if !self.fill() {
                return None;
            }
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

/// What a sink plays: a playback source that ends before its next block
/// once `stop` is set, so a replaced playback renders nothing beside the
/// one that replaces it. When the sink lets go of it, on the audio thread,
/// the source goes to the retired playback thread to be freed: a replaced
/// mix can hold hundreds of megabytes of samples, and its pedals and buses
/// megabytes more.
struct Playing {
    /// `None` only while it is dropped.
    source: Option<Box<PlaybackSource>>,
    stop: Arc<AtomicBool>,
}

impl Playing {
    /// Made on the window's thread, which also starts the retired playback
    /// thread here if it has not started yet.
    fn new(source: PlaybackSource) -> Self {
        retired_playback();
        Self {
            source: Some(Box::new(source)),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Iterator for Playing {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let source = self.source.as_mut()?;
        if source.cursor == source.filled && self.stop.load(Ordering::Relaxed) {
            return None;
        }
        source.next()
    }
}

impl Source for Playing {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.source.as_ref().map_or(2, |source| source.channels())
    }
    fn sample_rate(&self) -> u32 {
        self.source
            .as_ref()
            .map_or(Mix::SAMPLE_RATE, |source| source.sample_rate())
    }
    fn total_duration(&self) -> Option<Duration> {
        self.source
            .as_ref()
            .and_then(|source| source.total_duration())
    }
}

impl Drop for Playing {
    fn drop(&mut self) {
        if let Some(source) = self.source.take() {
            retire(source);
        }
    }
}

/// A playback on the device: its sink, and the switch that ends its source.
struct Current {
    sink: Sink,
    stop: Arc<AtomicBool>,
}

impl Current {
    /// Play `source` on `sink` in place of `old`. The old source ends
    /// before the new one can start, so no device callback renders both.
    fn start(sink: Sink, source: PlaybackSource, old: Option<Current>) -> Self {
        let playing = Playing::new(source);
        let stop = Arc::clone(&playing.stop);
        if let Some(old) = old {
            old.end();
        }
        sink.append(playing);
        Self { sink, stop }
    }

    fn end(self) {
        self.stop.store(true, Ordering::Relaxed);
        self.sink.stop();
    }
}

/// Retired playback that waits to be freed: a few replacements at most.
const RETIRED_QUEUE: usize = 16;
static RETIRED: OnceLock<SyncSender<Box<dyn Send>>> = OnceLock::new();

/// The queue to the thread that frees retired playback, started on first use.
fn retired_playback() -> &'static SyncSender<Box<dyn Send>> {
    RETIRED.get_or_init(|| {
        let (queue, retired) = std::sync::mpsc::sync_channel::<Box<dyn Send>>(RETIRED_QUEUE);
        // If the thread cannot start, the queue is closed and `retire` frees
        // in place, as before.
        let _ = std::thread::Builder::new()
            .name("retired playback".into())
            .spawn(move || retired.into_iter().for_each(drop));
        queue
    })
}

/// Hand `retired` to the retired playback thread. It never waits: the
/// bounded queue holds it without allocating, and only a full or closed
/// queue frees it here.
fn retire(retired: Box<dyn Send>) {
    if let Some(queue) = RETIRED.get() {
        let _ = queue.try_send(retired);
    }
}

/// The error of the last failed request for an audio thread's priority, or 0.
static PRIORITY_ERROR: AtomicU32 = AtomicU32::new(0);

#[cfg(test)]
thread_local! {
    /// Test threads count their priority requests and send them to the
    /// system only when a test asks, so tests do not run at an audio
    /// thread's priority.
    static PRIORITY_REQUESTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SYSTEM_PRIORITY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Ask the system to run the calling thread, the device's, as an audio
/// thread, once per thread. cpal 0.15.3 asks too, but passes a thread id
/// where its request needs a handle, so the system refuses it.
fn raise_priority_once() {
    thread_local! {
        static RAISED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if RAISED.with(|raised| raised.replace(true)) {
        return;
    }
    #[cfg(test)]
    {
        PRIORITY_REQUESTS.with(|count| count.set(count.get() + 1));
        if !SYSTEM_PRIORITY.with(std::cell::Cell::get) {
            return;
        }
    }
    if let Err(error) = raise_thread_priority() {
        PRIORITY_ERROR.store(error.max(1), Ordering::Relaxed);
    }
}

/// The multimedia class scheduler's "Pro Audio" task, or else the highest
/// priority of the process's class. The thread keeps it until it ends.
#[cfg(windows)]
fn raise_thread_priority() -> Result<(), u32> {
    use windows::Win32::System::Threading::{
        AvSetMmThreadCharacteristicsW, GetCurrentThread, SetThreadPriority,
        THREAD_PRIORITY_TIME_CRITICAL,
    };
    let mut task = 0;
    // SAFETY: the task name is a static wide string, and the pseudo handle
    // names the calling thread.
    unsafe {
        if AvSetMmThreadCharacteristicsW(windows::core::w!("Pro Audio"), &mut task).is_ok() {
            return Ok(());
        }
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL)
            .map_err(|error| error.code().0 as u32)
    }
}

#[cfg(not(windows))]
fn raise_thread_priority() -> Result<(), u32> {
    Ok(())
}

/// Why playback runs at a normal thread priority, if the system refused it
/// an audio thread's. Playback goes on either way.
pub fn priority_failure() -> Option<String> {
    let error = PRIORITY_ERROR.load(Ordering::Relaxed);
    (error != 0).then(|| {
        format!(
            "Playback could not raise its thread's priority (error 0x{error:08X}). \
             Audio may break up while the computer is busy."
        )
    })
}

impl Player {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| format!("Cannot open the audio output: {e}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            current: None,
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
            return Err("The audio must be mono or stereo with a supported sample rate.".into());
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
        let source = PlaybackSource::new(audio, start, loop_region, master);
        self.current = Some(Current::start(sink, source, self.current.take()));
        self.start_frame = start;
        self.total_frames = total;
        self.sample_rate = sample_rate;
        self.loop_region = loop_region;
        Ok(())
    }
    pub fn pause(&self) {
        if let Some(current) = &self.current {
            current.sink.pause();
        }
    }
    pub fn stop(&mut self) {
        if let Some(current) = self.current.take() {
            current.end();
        }
        self.start_frame = 0;
        self.loop_region = None;
    }
    pub fn is_playing(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|current| !current.sink.empty() && !current.sink.is_paused())
    }
    pub fn position_frames(&self) -> u64 {
        let elapsed = self.current.as_ref().map_or(0, |Current { sink: s, .. }| {
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
mod realtime_bench;

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
            chains: Vec::new(),
            keys: Vec::new(),
            key_tracks: Vec::new(),
            side: false,
            levels: [[1.0; 2]; crate::engine::TRACK_COUNT],
            master_gain: 1.0,
            bpm: 140.0,
            fed: [false; BUSES],
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
    fn playback_glides_to_live_track_levels_and_export_keeps_the_prepared_ones() {
        for chained in [false, true] {
            let mut project = song();
            project.clips[0].start_tick = 0;
            project.clips[0].offset_frames = 0;
            // A track with automation mixes through its chain.
            if chained {
                project.tracks[0].automation.volume = vec![crate::automation::Point {
                    tick: 0,
                    value: 0.0,
                }];
            }
            let source = Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![0.8; 75_600],
            };
            let mix = mix_with_samples(&project, &HashMap::from([(5, source)])).unwrap();
            let live = MasterControl::new(&project.master);
            live.set_levels(&project);
            let mut state = RenderState {
                live: Some(Arc::clone(&live)),
                ..RenderState::default()
            };
            let mut block = vec![0.0; 2048];
            mix.read_block(0, &mut block, None, &mut state);
            assert!(
                (block[2046] - 0.4).abs() < 1e-3,
                "chained {chained}: {}",
                block[2046]
            );
            project.tracks[0].gain = 0.25;
            live.set_levels(&project);
            mix.read_block(1024, &mut block, None, &mut state);
            let left: Vec<f32> = block.iter().step_by(2).copied().collect();
            let jump = left
                .windows(2)
                .map(|pair| (pair[1] - pair[0]).abs())
                .fold(0.0, f32::max);
            assert!(
                jump < 1e-3,
                "chained {chained}: the level must glide, not step: {jump}"
            );
            assert!(
                (left[0] - 0.4).abs() < 2e-3,
                "chained {chained}: {}",
                left[0]
            );
            assert!(
                (left[1023] - 0.1).abs() < 1e-3,
                "chained {chained}: {}",
                left[1023]
            );
            mix.read_block(2048, &mut block, None, &mut state);
            assert!((block[0] - 0.1).abs() < 1e-3);
            // Export reads the levels the mix was prepared with.
            assert!(!mix.has_levels(&project));
            mix.read_block(2048, &mut block, None, &mut RenderState::default());
            assert!((block[0] - 0.4).abs() < 1e-3);
        }
    }

    #[test]
    fn export_during_playback_does_not_change_filter_history() {
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
    fn automation_reaches_breakpoints_inside_a_render_chunk() {
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
        mix.read_block(0, &mut out, Some(&mut sends), &mut RenderState::default());
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
    fn a_filter_lane_crossing_zero_renders_without_a_click() {
        let mix_of = |lane: [(u32, f32); 2]| {
            let mut project = song();
            project.clips[0].start_tick = 0;
            project.clips[0].offset_frames = 0;
            project.tracks[0].automation.filter = lane
                .map(|(tick, value)| crate::automation::Point { tick, value })
                .into();
            let samples = HashMap::from([(
                5,
                Pcm {
                    sample_rate: 44_100,
                    channels: 1,
                    samples: (0..20_000)
                        .map(|i| {
                            let t = i as f32 / 44_100.0;
                            0.4 * ((std::f32::consts::TAU * 100.0 * t).sin()
                                + (std::f32::consts::TAU * 3000.0 * t).sin())
                        })
                        .collect(),
                },
            )]);
            let mix = mix_with_samples(&project, &samples).unwrap();
            let mut out = vec![0.0; 14_000 * 2];
            mix.read_block(0, &mut out, None, &mut RenderState::default());
            let left: Vec<f32> = out.iter().step_by(2).copied().collect();
            // The 20 ms around the middle of the move between the two points.
            left[4_000..6_500]
                .windows(2)
                .map(|pair| (pair[1] - pair[0]).abs())
                .fold(0.0, f32::max)
        };
        let crossing = mix_of([(8, -0.3), (9, 0.3)]);
        let low = mix_of([(8, -0.3), (9, -0.001)]);
        let high = mix_of([(8, 0.001), (9, 0.3)]);
        assert!(
            crossing <= low.max(high) + 0.03,
            "crossing step {crossing} against {low} and {high}"
        );
    }

    #[test]
    fn a_stray_end_is_hidden_and_a_real_tail_is_not() {
        // Tails of original product samples, 16-bit, after 10 ms of the given
        // lead-in. The frame count never changes.
        let tail = |lead: &dyn Fn(usize) -> i16, tail: &[i16]| {
            let mut pcm = Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: (0..600)
                    .map(lead)
                    .chain(tail.iter().copied())
                    .map(|x| f32::from(x) / 32768.0)
                    .collect(),
            };
            let before = pcm.samples.clone();
            hide_stray_end(&mut pcm);
            assert_eq!(pcm.samples.len(), before.len());
            let end = |samples: &[f32]| -> Vec<i16> {
                samples[samples.len() - tail.len()..]
                    .iter()
                    .map(|x| (x * 32768.0).round() as i16)
                    .collect()
            };
            (end(&before), end(&pcm.samples), before == pcm.samples)
        };
        let silent = |_: usize| 0;
        let loud = |i: usize| (((i * 7) % 13) as i16 - 6) * 3_000;
        let (_, fixed, _) = tail(&silent, &[0, 0, 0, 0, 0, 0, 0, 9282]);
        assert_eq!(fixed, [0, 0, 0, 0, 0, 0, 0, 0]);
        let (_, fixed, _) = tail(&silent, &[0, 0, 0, 0, 0, 0, 0, 399]);
        assert_eq!(fixed, [0; 8]);
        let (_, fixed, _) = tail(&|_| -4, &[-4, -4, -4, -3, -4, -4, -4, 1729]);
        assert_eq!(fixed, [-4, -4, -4, -3, -4, -4, -4, -4]);
        let (_, fixed, _) = tail(&silent, &[0, 0, 0, 0, 0, 0, -3853, 4756]);
        assert_eq!(fixed, [0; 8]);
        // A quiet sound that dies away to silence and ends on one frame
        // between two and four times its peak.
        let quiet = |i: usize| (((i * 7) % 13) as i16 - 6) * 500;
        let (_, fixed, _) = tail(&quiet, &[0, 0, 0, 0, 0, 0, 0, 9282]);
        assert_eq!(fixed, [0; 8]);
        let (_, fixed, _) = tail(&quiet, &[0, 0, 0, 2, 0, 0, 0, 9282]);
        assert_eq!(fixed, [0, 0, 0, 2, 0, 0, 0, 0]);
        // The same sound that is still sounding at its end is a real tail.
        let (_, kept, unchanged) =
            tail(&quiet, &[2000, -2500, 1800, -2200, 2400, -1900, 2100, 7000]);
        assert!(unchanged, "became {kept:?}");
        // A sound still sounding at its end, a smooth tail, then a single
        // frame that jumps (hold-on-005-r).
        let smooth = |i: usize| ((i % 200) as i16 - 100).abs() * 14 + 1066;
        let (_, fixed, _) = tail(&smooth, &[1644, 1648, 1494, 1208, 922, 692, 562, 9478]);
        assert_eq!(fixed, [1644, 1648, 1494, 1208, 922, 692, 562, 562]);
        // The loud real tails also stay with a lead that continues from their start,
        // so the lone-jump rule is what must keep them.
        for real in [
            [-5338, 4359, 3207, -10247, 15196, -20289, 23170, -21643],
            [816, 2006, 10203, 18743, 23341, 23767, 16673, 3815],
        ] {
            let start = real[0];
            let (_, kept, unchanged) = tail(&|_| start, &real);
            assert!(unchanged, "{real:?} became {kept:?}");
        }
        for real in [
            [-5338, 4359, 3207, -10247, 15196, -20289, 23170, -21643],
            [816, 2006, 10203, 18743, 23341, 23767, 16673, 3815],
            [-518, -490, 162, 865, 3077, 4644, 8113, 9916],
        ] {
            let (_, kept, unchanged) = tail(&loud, &real);
            assert!(unchanged, "{real:?} became {kept:?}");
        }
    }

    #[test]
    fn a_stray_end_is_hidden_in_one_channel_only() {
        let mut samples = vec![0.0; 2 * 600];
        for frame in 0..600 {
            samples[frame * 2] = -4.0 / 32768.0;
        }
        let last = samples.len() - 2;
        samples[last] = 1729.0 / 32768.0;
        samples[last + 1] = 0.0;
        let mut pcm = Pcm {
            sample_rate: 44_100,
            channels: 2,
            samples,
        };
        let before = pcm.samples.clone();
        hide_stray_end(&mut pcm);
        assert_eq!(pcm.samples.len(), before.len());
        assert_eq!(pcm.samples[last], -4.0 / 32768.0);
        assert_eq!(pcm.samples[..last], before[..last]);
        assert_eq!(pcm.samples[last + 1], before[last + 1]);
    }

    #[test]
    fn a_short_sound_keeps_its_end() {
        let mut pcm = Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.5],
        };
        let before = pcm.samples.clone();
        hide_stray_end(&mut pcm);
        assert_eq!(pcm.samples, before);
    }

    #[test]
    fn dry_wav_admission_keeps_the_full_riff_capacity() {
        let mix = Mix {
            frames: (u64::from(u32::MAX) - 36) / 4,
            voices: Vec::new(),
            chains: Vec::new(),
            keys: Vec::new(),
            key_tracks: Vec::new(),
            side: false,
            levels: [[1.0; 2]; crate::engine::TRACK_COUNT],
            master_gain: 1.0,
            bpm: 140.0,
            fed: [false; BUSES],
            sends: false,
        };
        let error = mix
            .write_wav(Path::new(""), &MasterChain::default(), &FxBuses::default())
            .unwrap_err();
        assert_ne!(error, "This song is too long for a WAV file.");
    }

    #[test]
    fn playback_and_export_stop_at_the_same_tail_limit() {
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
    fn a_new_preparation_reuses_the_sources_and_their_pitched_copies() {
        // Sound 5 plays at two pitches and never as it is; sound 6 as it is.
        let mut project = song();
        project.clips[0].pitch_cents = 300;
        for (id, sample_id, pitch_cents) in [(2, 5, -300), (3, 6, 0)] {
            project.clips.push(Clip {
                id,
                sample_id,
                pitch_cents,
                ..project.clips[0].clone()
            });
        }
        let reads = std::sync::Mutex::new(Vec::new());
        let read = |id| {
            reads.lock().unwrap().push(id);
            Ok(Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: (0..20_000)
                    .map(|frame| (frame as f32 * 0.05).sin() * 0.5)
                    .collect(),
            })
        };
        let mut decoded = HashMap::new();
        load_sources(&project, &mut decoded, read).unwrap();
        let mut first = reads.lock().unwrap().clone();
        first.sort_unstable();
        assert_eq!(
            first,
            [5, 6],
            "Each source is read once, at any number of pitches."
        );
        let mut keys: Vec<_> = decoded.keys().copied().collect();
        keys.sort_unstable();
        assert_eq!(keys, [(5, -300), (5, 300), (6, 0)]);
        let pitched = Arc::clone(&decoded[&(5, 300)]);

        // A gain edit reads and pitches nothing again.
        project.clips[0].gain = 0.25;
        reads.lock().unwrap().clear();
        load_sources(&project, &mut decoded, read).unwrap();
        assert!(reads.lock().unwrap().is_empty());
        assert!(Arc::ptr_eq(&pitched, &decoded[&(5, 300)]));

        // A source that is not normalized PCM is refused when it is read.
        project.clips[2].sample_id = 7;
        let refused = load_sources(&project, &mut decoded, |_| {
            Ok(Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![f32::NAN; 100],
            })
        });
        assert!(refused.is_err());
        assert!(!decoded.contains_key(&(7, 0)));
        assert!(Arc::ptr_eq(&pitched, &decoded[&(5, 300)]));
    }

    #[test]
    fn work_shared_among_threads_comes_back_in_order() {
        let items: Vec<u64> = (0..200).collect();
        assert_eq!(
            in_parallel(&items, |item| item * item),
            items.iter().map(|item| item * item).collect::<Vec<_>>()
        );
        assert!(in_parallel(&[] as &[u64], |item| *item).is_empty());
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
            let sending = mix.read_block(
                from,
                &mut out,
                Some(&mut feeds),
                &mut RenderState::default(),
            );
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
        project.master.booster_compressor.on = true;
        project.master.booster_wide.on = true;
        project.master.booster_eq.on = true;
        project.master.booster_eq.bands[0] = 90;
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
    fn a_half_time_loop_plays_at_its_own_pitch() {
        let mut p = song();
        p.bpm = 180.0;
        p.clips[0].offset_frames = 0;
        p.clips[0].sample_id = 1_000_000;
        p.imported_samples.push(crate::engine::ImportedSample {
            id: 1_000_000,
            name: "Half-time loop".into(),
            group_name: "Wave".into(),
            path: "loop.wav".into(),
            source_bpm: 90.0,
            source_sample_rate: Some(44100),
            origin: None,
        });
        let pcm = Pcm {
            sample_rate: 44100,
            channels: 1,
            samples: vec![0.1, 0.2, 0.3],
        };
        let out = render_with_samples(&p, &HashMap::from([(1_000_000, pcm)])).unwrap();
        let index = (p.frames_per_tick(44100) * 256.0).round() as usize * 2;
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

    /// A mono tone of `hz` at half scale, one second long.
    fn tone(hz: f32) -> Pcm {
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: (0..44_100)
                .map(|i| 0.5 * (std::f32::consts::TAU * hz * i as f32 / 44_100.0).sin())
                .collect(),
        }
    }

    /// The RMS level of the left channel between two frames, in dB.
    fn level_db(samples: &[f32], frames: std::ops::Range<usize>) -> f64 {
        let sum: f64 = frames
            .clone()
            .map(|frame| f64::from(samples[frame * 2]).powi(2))
            .sum();
        10.0 * (sum / frames.len() as f64).log10()
    }

    #[test]
    fn a_track_whose_eq_is_off_renders_exactly_as_before() {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        editor.place(2, 1, 0, 256).unwrap();
        let mut project = editor.project().clone();
        project.tracks[1].automation.volume = vec![
            crate::automation::Point {
                tick: 0,
                value: -12.0,
            },
            crate::automation::Point {
                tick: 256,
                value: 0.0,
            },
        ];
        let samples = HashMap::from([(1, tone(220.0)), (2, tone(5000.0))]);
        let before = render_with_samples(&project, &samples).unwrap();
        // Settings that the EQ keeps while it is off change nothing, on a
        // plain track and on an automated one.
        for track in &mut project.tracks[..2] {
            track.eq = crate::eq::ChannelEq {
                low_cut: 400.0,
                high_db: -18.0,
                mid_db: 9.0,
                ..Default::default()
            };
        }
        assert!(render_with_samples(&project, &samples).unwrap().samples == before.samples);
        // Switched on with every band flat, it passes the sound unchanged.
        for track in &mut project.tracks[..2] {
            track.eq = crate::eq::ChannelEq {
                on: true,
                ..Default::default()
            };
        }
        let flat = render_with_samples(&project, &samples).unwrap();
        assert!(
            flat.samples
                .iter()
                .zip(&before.samples)
                .all(|(a, b)| (a - b).abs() < 1e-6)
        );
    }

    #[test]
    fn the_channel_eq_shapes_its_track_before_the_automation() {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        editor.place(2, 1, 0, 256).unwrap();
        let mut project = editor.project().clone();
        let samples = HashMap::from([(1, tone(12_000.0)), (2, tone(200.0))]);
        let solo = |project: &Project, track: usize| {
            let mut alone = project.clone();
            alone.tracks[track].solo = true;
            render_with_samples(&alone, &samples).unwrap().samples
        };
        // The tones last one second of the one-bar clips.
        let frames = 11_025..44_100;
        let open = [0, 1].map(|track| level_db(&solo(&project, track), frames.clone()));
        // A high shelf at 8 kHz lowers 12 kHz by about its gain; 200 Hz stays.
        for track in &mut project.tracks[..2] {
            track.eq = crate::eq::ChannelEq {
                on: true,
                high_db: -12.0,
                ..Default::default()
            };
        }
        let top = level_db(&solo(&project, 0), frames.clone()) - open[0];
        assert!((top + 12.0).abs() < 1.5, "12 kHz: {top:.2} dB");
        let low = level_db(&solo(&project, 1), frames.clone()) - open[1];
        assert!(low.abs() < 0.5, "200 Hz: {low:.2} dB");
        // A low cut at 200 Hz removes 50 Hz, on a track with automation too.
        let samples = HashMap::from([(1, tone(50.0)), (2, tone(50.0))]);
        let mut project = editor.project().clone();
        project.tracks[1].automation.filter = vec![crate::automation::Point {
            tick: 0,
            value: 0.0,
        }];
        let open = level_db(
            &render_with_samples(&project, &samples).unwrap().samples,
            frames.clone(),
        );
        for track in &mut project.tracks[..2] {
            track.eq = crate::eq::ChannelEq {
                on: true,
                low_cut: 200.0,
                ..Default::default()
            };
        }
        let cut = level_db(
            &render_with_samples(&project, &samples).unwrap().samples,
            frames,
        );
        assert!(cut - open < -20.0, "50 Hz: {:.2} dB", cut - open);
    }

    #[test]
    fn a_lane_compressor_works_before_the_fader_and_off_changes_nothing() {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        let mut project = editor.project().clone();
        let samples = HashMap::from([(1, tone(220.0))]);
        let render = |project: &Project| render_with_samples(project, &samples).unwrap().samples;
        let frames = 11_025..44_100;
        let before = render(&project);
        let open = level_db(&before, frames.clone());
        // Settings that the compressor keeps while it is off change nothing.
        project.tracks[0].compressor = crate::compressor::Compressor {
            threshold: -30.0,
            ratio: 4.0,
            attack: 1.0,
            ..Default::default()
        };
        assert!(render(&project) == before);
        // The tone's mean power is 9 dB under full scale: 21 dB over the
        // threshold at 4:1 comes down by about 15.75 dB.
        project.tracks[0].compressor.on = true;
        let compressed = level_db(&render(&project), frames.clone());
        let change = compressed - open;
        assert!((change + 15.75).abs() < 1.0, "{change:.2} dB");
        // The fader follows the compressor, so it does not change how hard
        // the compressor works: 12 dB down on the fader is 12 dB down.
        project.tracks[0].gain = crate::engine::db_to_amplitude(-12.0);
        let lower = level_db(&render(&project), frames.clone()) - compressed;
        assert!((lower + 12.0).abs() < 0.05, "{lower:.3} dB");
        // The makeup gain raises the result.
        project.tracks[0].compressor.makeup = 6.0;
        let made_up = level_db(&render(&project), frames) - compressed;
        assert!((made_up + 6.0).abs() < 0.05, "{made_up:.3} dB");
    }

    /// One second of a stereo source: `left` and `right` of the frame.
    fn stereo(left: impl Fn(usize) -> f32, right: impl Fn(usize) -> f32) -> Pcm {
        Pcm {
            sample_rate: 44_100,
            channels: 2,
            samples: (0..44_100).flat_map(|i| [left(i), right(i)]).collect(),
        }
    }

    #[test]
    fn the_width_folds_or_spreads_a_track_before_its_pan() {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        let mut project = editor.project().clone();
        let wave = |i: usize| 0.25 * (std::f32::consts::TAU * 330.0 * i as f32 / 44_100.0).sin();
        // An anti-phase source: all side, no mid.
        let samples = HashMap::from([(1, stereo(wave, |i| -wave(i)))]);
        let render = |project: &Project| render_with_samples(project, &samples).unwrap().samples;
        let before = render(&project);
        project.tracks[0].width = 100.0;
        assert!(render(&project) == before, "100% leaves the track as it is");
        let peak = |samples: &[f32]| samples.iter().fold(0.0_f32, |peak, x| peak.max(x.abs()));
        project.tracks[0].width = 0.0;
        assert!(peak(&render(&project)) < 1e-6, "0% folds the side away");
        project.tracks[0].width = 200.0;
        let wide = render(&project);
        assert!(
            wide.iter()
                .zip(&before)
                .all(|(wide, open)| (wide - 2.0 * open).abs() < 1e-6),
            "200% doubles the side"
        );
        // A track on one output has no side: the width leaves it alone.
        project.tracks[0].right_enabled = false;
        project.tracks[0].width = 100.0;
        let one = render(&project);
        project.tracks[0].width = 0.0;
        assert!(render(&project) == one);
        // The width works before the pan: a left-only source folded to mono
        // and panned hard left keeps half its level on the left, through
        // the master gain, and none on the right.
        let samples = HashMap::from([(1, stereo(wave, |_| 0.0))]);
        let mut project = editor.project().clone();
        project.tracks[0].width = 0.0;
        project.tracks[0].pan = -1.0;
        let panned = render_with_samples(&project, &samples).unwrap().samples;
        let (left, right): (Vec<f32>, Vec<f32>) = panned
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[left, right]| (left, right))
            .unzip();
        assert!(peak(&right) < 1e-6);
        let half = 0.125 * project.master_gain;
        assert!((peak(&left) - half).abs() < 1e-3, "{}", peak(&left));
    }

    #[test]
    fn the_second_peak_cuts_its_line_in_the_mix() {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        let mut project = editor.project().clone();
        let samples = HashMap::from([(1, tone(191.0))]);
        let frames = 11_025..44_100;
        let level = |project: &Project| {
            level_db(
                &render_with_samples(project, &samples).unwrap().samples,
                frames.clone(),
            )
        };
        let open = level(&project);
        project.tracks[0].eq = crate::eq::ChannelEq {
            on: true,
            mid_db: 6.0,
            mid_hz: 2000.0,
            peak2_hz: 191.0,
            peak2_db: -12.0,
            peak2_q: 8.0,
            ..Default::default()
        };
        let cut = level(&project) - open;
        assert!((cut + 12.0).abs() < 0.5, "{cut:.2} dB");
    }

    #[test]
    fn a_ducker_lowers_its_track_while_its_key_sounds_and_only_then() {
        // Track 1 holds a tone from the start; track 2's key enters halfway
        // through the bar, at frame 37,800, while the tone still sounds.
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        editor.place(2, 1, 128, 128).unwrap();
        let mut project = editor.project().clone();
        let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0))]);
        let alone = |project: &Project, track: usize| {
            let mix = mix_with_samples(project, &samples).unwrap();
            let mut out = vec![0.0; mix.frames() as usize * 2];
            mix.mix(0, &mut out, None, &mut RenderState::default(), Some(track));
            out
        };
        let open = alone(&project, 0);
        let key = alone(&project, 1);
        project.tracks[0].ducker = crate::ducker::Ducker {
            on: true,
            key: 2,
            threshold: -40.0,
            depth: 12.0,
            attack: 0.1,
            release: 20.0,
        };
        let ducked = alone(&project, 0);
        // Before the key: unchanged. With the key 34 dB over the threshold:
        // the depth, 12 dB down.
        assert_eq!(ducked[..37_800 * 2], open[..37_800 * 2]);
        let down = level_db(&ducked, 39_000..44_000) - level_db(&open, 39_000..44_000);
        assert!((down + 12.0).abs() < 0.2, "{down:.2} dB");
        // The key sounds as before: a key is measured, not changed.
        assert_eq!(alone(&project, 1), key);
        // A muted key does not duck.
        project.tracks[1].muted = true;
        assert_eq!(alone(&project, 0), open);
    }

    #[test]
    fn the_master_gain_does_not_change_how_far_a_ducker_ducks() {
        // The key sits a few dB over the threshold, inside the depth, where
        // any change in its level would change the ducking.
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        editor.place(2, 1, 128, 128).unwrap();
        let mut project = editor.project().clone();
        project.tracks[0].ducker = crate::ducker::Ducker {
            on: true,
            key: 2,
            threshold: -12.0,
            depth: 24.0,
            attack: 0.1,
            release: 20.0,
        };
        let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0))]);
        let ducked = |project: &Project| {
            let mix = mix_with_samples(project, &samples).unwrap();
            let mut out = vec![0.0; mix.frames() as usize * 2];
            mix.mix(0, &mut out, None, &mut RenderState::default(), Some(0));
            level_db(&out, 39_000..44_000)
        };
        project.master_gain = 1.0;
        let full = ducked(&project);
        project.master_gain = 0.5;
        let half = ducked(&project);
        // Half the gain is 6.02 dB down, and nothing more.
        assert!((half - full + 6.0206).abs() < 0.01, "{:.3} dB", half - full);
    }

    #[test]
    fn a_tiny_master_gain_does_not_poison_the_ducker_after_the_key_stops() {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        editor.place(2, 1, 0, 256).unwrap();
        let mut project = editor.project().clone();
        project.master_gain = 1e-39;
        project.tracks[0].ducker = crate::ducker::Ducker {
            on: true,
            key: 2,
            threshold: -12.0,
            depth: 24.0,
            attack: 0.1,
            release: 20.0,
        };
        let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0))]);
        let mix = mix_with_samples(&project, &samples).unwrap();
        let live = MasterControl::new(&project.master);
        live.set_levels(&project);
        let mut state = RenderState {
            live: Some(live.clone()),
            ..Default::default()
        };
        let mut block = vec![0.0; 2048];
        mix.mix(0, &mut block, None, &mut state, Some(0));
        project.master_gain = 1.0;
        project.tracks[1].gain = 0.0;
        live.set_levels(&project);
        let mut after = vec![0.0; 32_768 * 2];
        for (index, part) in after.chunks_mut(2048).enumerate() {
            mix.mix(1024 * (index + 1) as u64, part, None, &mut state, Some(0));
        }
        project.tracks[0].ducker.on = false;
        let mut expected = vec![0.0; after.len()];
        mix_with_samples(&project, &samples).unwrap().mix(
            1024,
            &mut expected,
            None,
            &mut RenderState::default(),
            Some(0),
        );
        let difference = level_db(&after, 28_672..32_768) - level_db(&expected, 28_672..32_768);
        assert!(
            difference.abs() < 0.1,
            "Ducker stayed reduced: {difference} dB"
        );
    }

    #[test]
    fn playback_hands_a_live_master_gain_to_a_ducker_key_without_a_step() {
        // A master-gain edit during playback stores new levels without a
        // new mix: the key must take the live gain, glided as the levels
        // glide, and duck as an export at the new gain does.
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        editor.place(2, 1, 0, 256).unwrap();
        let mut project = editor.project().clone();
        project.tracks[0].ducker = crate::ducker::Ducker {
            on: true,
            key: 2,
            threshold: -12.0,
            depth: 24.0,
            attack: 0.1,
            release: 20.0,
        };
        let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0))]);
        project.master_gain = 1.0;
        let mix = mix_with_samples(&project, &samples).unwrap();
        let live = MasterControl::new(&project.master);
        live.set_levels(&project);
        let mut state = RenderState {
            live: Some(Arc::clone(&live)),
            ..RenderState::default()
        };
        let mut played = vec![0.0; 3 * 2048];
        mix.read_block(0, &mut played[..2048], None, &mut state);
        project.master_gain = 0.5;
        live.set_levels(&project);
        mix.read_block(1024, &mut played[2048..4096], None, &mut state);
        mix.read_block(2048, &mut played[4096..], None, &mut state);
        let mut expected = vec![0.0; played.len()];
        mix_with_samples(&project, &samples).unwrap().read_block(
            0,
            &mut expected,
            None,
            &mut RenderState::default(),
        );
        // Playback is the export at 0.5 times the gain it plays at over the
        // export's: 2, then a glide to 1 over the second block, then 1.
        for frame in 0..3072 {
            let along = (frame as f32 - 1023.0).clamp(0.0, 1024.0) / 1024.0;
            let scale = 2.0 - along;
            for channel in 0..2 {
                let at = 2 * frame + channel;
                assert!(
                    (played[at] - scale * expected[at]).abs() < 1e-4,
                    "frame {frame}: played {}, expected {}",
                    played[at],
                    scale * expected[at]
                );
            }
        }
    }

    /// The first frame of bar 3, 11 264 frames in: far from bar 1, and late
    /// enough in the bar for the export's compressor to have settled.
    fn inside_bar_3(project: &Project) -> usize {
        (512.0 * project.frames_per_tick(Mix::SAMPLE_RATE)).round() as usize + 11_264
    }

    #[test]
    fn review_a_loop_at_zero_keeps_compression_and_off_on_discards_it() {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        let mut project = editor.project().clone();
        project.tracks[0].compressor.on = true;
        project.tracks[0].compressor.threshold = -30.0;
        let samples = HashMap::from([(1, tone(220.0))]);
        let mix = mix_with_samples(&project, &samples).unwrap();
        let live = MasterControl::new(&project.master);
        live.set_levels(&project);
        let mut state = RenderState {
            live: Some(live.clone()),
            loop_region: Some([0, 22_528]),
            ..Default::default()
        };
        let mut warm = vec![0.0; 22_528 * 2];
        mix.read_block(0, &mut warm, None, &mut state);
        let mut looped = vec![0.0; 1024 * 2];
        mix.read_block(0, &mut looped, None, &mut state);
        let difference = level_db(&looped, 0..256) - level_db(&warm, 0..256);
        assert!(difference < -6.0, "Loop attack restarted: {difference} dB");
        assert!(live.kept_compressor(0).is_some());
        project.tracks[0].compressor.on = false;
        live.set_levels(&project);
        project.tracks[0].compressor.on = true;
        live.set_levels(&project);
        assert!(
            live.kept_compressor(0).is_none(),
            "Off/on retained old reduction"
        );
    }

    #[test]
    fn playback_compressors_carry_on_only_where_playback_continues() {
        // Bar 1 holds a tone that the compressor holds about 15.75 dB down,
        // with the default 10 ms attack. Bar 3 holds it 40 dB lower, under
        // the threshold and its knee.
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        editor.place(1, 0, 512, 256).unwrap();
        let mut project = editor.project().clone();
        let quiet_clip = project.clips.iter_mut().find(|clip| clip.start_tick == 512);
        quiet_clip.unwrap().gain = 0.01;
        project.tracks[0].compressor = crate::compressor::Compressor {
            on: true,
            threshold: -30.0,
            ratio: 4.0,
            ..Default::default()
        };
        let samples = HashMap::from([(1, tone(220.0))]);
        let mix = mix_with_samples(&project, &samples).unwrap();
        let quiet = inside_bar_3(&project);
        let mut export = vec![0.0; (quiet + 2048) * 2];
        mix.read_block(0, &mut export, None, &mut RenderState::default());
        let live = MasterControl::new(&project.master);
        live.set_levels(&project);
        let playback = |loop_region: Option<[u64; 2]>| RenderState {
            live: Some(Arc::clone(&live)),
            loop_region,
            ..RenderState::default()
        };
        // The first 6 ms of a block against the export's same frames.
        let onset = |first: usize, block: &[f32]| {
            level_db(block, 0..256) - level_db(&export[first * 2..], 0..256)
        };
        let mut state = playback(Some([11_264, 22_528]));
        let mut block = vec![0.0; 2048];
        for first in (0..22_528).step_by(1024) {
            mix.read_block(first as u64, &mut block, None, &mut state);
        }
        // A mix prepared again, such as for a compressor dial's step, plays
        // through a new source with a new state from the playhead, which is
        // behind the frames that playback has rendered.
        mix.read_block(21_000, &mut block, None, &mut playback(None));
        let resumed = onset(21_000, &block);
        assert!(resumed.abs() < 0.5, "new mix: {resumed:.2} dB");
        // A loop pass, from the loop's end back to its start.
        mix.read_block(11_264, &mut block, None, &mut state);
        let looped = onset(11_264, &block);
        assert!(looped.abs() < 0.5, "loop pass: {looped:.2} dB");
        // A start far from where playback kept the history, here in the
        // quiet bar, begins from silence, as export has it there. The loud
        // bar's history would hold the track about 15 dB too low.
        mix.read_block(quiet as u64, &mut block, None, &mut playback(None));
        let far = onset(quiet, &block);
        assert!(far.abs() < 0.5, "far start: {far:.2} dB");
        // In the loud bar a far start begins without reduction, louder
        // until the attack catches up, as before playback kept histories.
        mix.read_block(5_000, &mut block, None, &mut playback(None));
        let fresh = onset(5_000, &block);
        assert!(fresh > 3.0, "fresh start: {fresh:.2} dB");
        // Playback from the song's start begins from silence, as export does.
        let mut from_start = vec![0.0; 2048];
        mix.read_block(0, &mut from_start, None, &mut playback(None));
        assert!(from_start[..] == export[..2048]);
    }

    #[test]
    fn a_compressed_key_carries_its_tracks_history_and_starts_fresh_elsewhere() {
        // Track 3 keys track 1's ducker and is compressed: about 15.75 dB
        // down in bar 1, and untouched in bar 3, where it plays 30 dB lower.
        // The ducker works inside its depth in both bars, so any change in
        // the key's level changes how far track 1 ducks.
        let mut editor = crate::engine::Editor::new(Project::default());
        for start in [0, 512] {
            editor.place(1, 0, start, 256).unwrap();
            editor.place(2, 2, start, 256).unwrap();
        }
        let mut project = editor.project().clone();
        let quiet_key = project
            .clips
            .iter_mut()
            .find(|clip| clip.track == 2 && clip.start_tick == 512);
        quiet_key.unwrap().gain = 0.03;
        project.tracks[2].compressor = crate::compressor::Compressor {
            on: true,
            threshold: -30.0,
            ratio: 4.0,
            ..Default::default()
        };
        project.tracks[0].ducker = crate::ducker::Ducker {
            on: true,
            key: 3,
            threshold: -42.0,
            depth: 24.0,
            attack: 0.1,
            release: 20.0,
        };
        let samples = HashMap::from([(1, tone(220.0)), (2, tone(1000.0))]);
        let mix = mix_with_samples(&project, &samples).unwrap();
        // The mix prepared again has its chains in another order, so the
        // kept history must follow the track, not the chain's place.
        let mut reordered = project.clone();
        reordered
            .clips
            .sort_by_key(|clip| std::cmp::Reverse(clip.track));
        let again = mix_with_samples(&reordered, &samples).unwrap();
        let order = |mix: &Mix| {
            mix.chains
                .iter()
                .map(|chain| chain.track)
                .collect::<Vec<_>>()
        };
        assert_eq!((order(&mix), order(&again)), (vec![0, 2], vec![2, 0]));
        // Track 1 alone, as export plays it.
        let quiet = inside_bar_3(&project);
        let mut export = vec![0.0; (quiet + 2048) * 2];
        again.mix(0, &mut export, None, &mut RenderState::default(), Some(0));
        // 12 ms of a block after its first 128 frames, against the export's
        // same frames. The ducker's own history starts from silence, so its
        // attack takes those first frames whatever the key does.
        let onset = |first: usize, block: &[f32]| {
            level_db(block, 128..640) - level_db(&export[first * 2..], 128..640)
        };
        let live = MasterControl::new(&project.master);
        live.set_levels(&project);
        let playback = || RenderState {
            live: Some(Arc::clone(&live)),
            ..RenderState::default()
        };
        let mut state = playback();
        let mut block = vec![0.0; 2048];
        for first in (0..22_528).step_by(1024) {
            mix.read_block(first as u64, &mut block, None, &mut state);
        }
        // From the playhead, the key chain carries the key track's history,
        // so track 1 ducks as in export rather than deeper.
        again.mix(21_000, &mut block, None, &mut playback(), Some(0));
        let resumed = onset(21_000, &block);
        assert!(resumed.abs() < 0.5, "new mix: {resumed:.2} dB");
        // A start in the quiet bar begins fresh. A key carried from the loud
        // bar would sit about 15 dB low, and track 1 would duck that much
        // less: louder than export.
        again.mix(quiet as u64, &mut block, None, &mut playback(), Some(0));
        let far = onset(quiet, &block);
        assert!(far.abs() < 0.5, "far start: {far:.2} dB");
    }

    #[test]
    fn playback_key_audio_is_allocated_before_the_first_read() {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        let mut project = editor.project().clone();
        project.master.bus_compressor.on = true;
        project.master.bus_compressor.key = 2;
        let samples = HashMap::from([(1, tone(220.0))]);
        let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
        let mut playback =
            PlaybackSource::new(mix, 0, None, Some(MasterControl::new(&project.master)));
        let capacity = playback.state.key_audio.capacity();
        let allocation = playback.state.key_audio.as_ptr();
        assert!(playback.fill());
        assert_eq!(playback.state.key_audio.capacity(), capacity);
        assert_eq!(playback.state.key_audio.as_ptr(), allocation);
    }

    #[test]
    fn a_keyed_bus_compressor_lowers_the_mix_while_its_key_track_sounds() {
        // Track 2's key enters halfway through the bar, at frame 37,800.
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        editor.place(2, 1, 128, 128).unwrap();
        let mut project = editor.project().clone();
        let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0))]);
        // At a 0 dBFS threshold the mix never reaches the knee.
        project.master.bus_compressor = crate::master::BusCompressor {
            on: true,
            threshold: 0.0,
            ratio: 10.0,
            attack: 0.1,
            release: 50.0,
            key: 0,
        };
        let render = |project: &Project| render_with_samples(project, &samples).unwrap().samples;
        let open = render(&project);
        project.master.bus_compressor.threshold = -30.0;
        project.master.bus_compressor.key = 2;
        let keyed = render(&project);
        // Before the key the mix is untouched; with it, the whole mix ducks.
        assert_eq!(keyed[..37_800 * 2], open[..37_800 * 2]);
        let down = level_db(&keyed, 39_000..44_000) - level_db(&open, 39_000..44_000);
        assert!(down < -12.0, "{down:.2} dB");
        // Playback hears the same key.
        let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
        let played: Vec<f32> =
            PlaybackSource::new(mix, 0, None, Some(MasterControl::new(&project.master))).collect();
        let played_down = level_db(&played, 39_000..44_000) - level_db(&open, 39_000..44_000);
        assert!(
            (played_down - down).abs() < 0.5,
            "{played_down:.2} against {down:.2} dB"
        );
        // A muted key is silent, so nothing ducks.
        project.tracks[1].muted = true;
        let muted = render(&project);
        project.master.bus_compressor.key = 0;
        project.master.bus_compressor.threshold = 0.0;
        assert_eq!(muted, render(&project));
    }

    #[test]
    fn bypass_skips_the_trim_and_pedals_for_export_and_playback() {
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
        project.master.trim = -9.0;
        project.master.tube.on = true;
        project.master.limiter.on = true;
        project.master.booster_compressor.on = true;
        let mastered = render_with_samples(&project, &samples).unwrap();
        assert_ne!(mastered.samples, dry.samples);
        project.master.bypass = true;
        assert!(!project.master.is_active());
        assert_eq!(project.master.latency(), 0);
        let bypassed = render_with_samples(&project, &samples).unwrap();
        assert_eq!(bypassed.samples, dry.samples);
        let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
        let played: Vec<f32> = PlaybackSource::new(
            mix.clone(),
            0,
            None,
            Some(MasterControl::new(&project.master)),
        )
        .collect();
        assert_eq!(played, dry.samples);

        // Bypassing during playback crossfades to the dry mix and keeps the
        // song on the timeline.
        project.master.bypass = false;
        let control = MasterControl::new(&project.master);
        let mut playback = PlaybackSource::new(mix, 0, None, Some(Arc::clone(&control)));
        let mut live: Vec<f32> = playback.by_ref().take(8192).collect();
        control.set(&MasterChain {
            bypass: true,
            ..project.master
        });
        live.extend(playback);
        assert_eq!(live.len(), dry.samples.len());
        // The switch lands at frame 4096. The new board fills its delay and
        // the 30 ms crossfade runs, while the trim glides to unity; by frame
        // 8192 the dry mix plays exactly.
        assert_ne!(live[..2 * 4096], dry.samples[..2 * 4096]);
        assert_eq!(live[2 * 8192..], dry.samples[2 * 8192..]);
    }

    /// A failed stem render must keep every earlier export, including files
    /// whose new samples were staged before the failure.
    fn rejected_stems_keep_saved_files(project: &Project, mix: &Mix, part: &str) {
        let folder = std::env::temp_dir().join(format!(
            "mixjam-stems-rejected-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let names: Vec<String> = mix
            .sounding_tracks()
            .iter()
            .map(|track| format!("{} - Track {:02}.wav", project.title, track + 1))
            .chain(
                mix.returning_buses(&project.fx)
                    .iter()
                    .map(|bus| format!("{} - FX {} return.wav", project.title, bus + 1)),
            )
            .collect();
        for name in &names {
            std::fs::write(folder.join(name), name.as_bytes()).unwrap();
        }
        let result = mix.write_stems(&folder, &project.title, &project.fx);
        let preserved = names
            .iter()
            .all(|name| std::fs::read(folder.join(name)).unwrap() == name.as_bytes());
        let entries = std::fs::read_dir(&folder).unwrap().count();
        std::fs::remove_dir_all(&folder).unwrap();
        let error = result.expect_err("An over-range part must not become a clipped stem.");
        assert!(error.contains(part), "{error}");
        assert!(error.contains("Lower"), "{error}");
        assert!(preserved, "Every saved stem must remain unchanged.");
        assert_eq!(entries, names.len(), "No staging files may remain.");
    }

    #[test]
    fn over_range_track_stems_reject_even_when_the_mix_does_not_clip() {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        editor.place(1, 1, 0, 256).unwrap();
        let mut project = editor.project().clone();
        project.master_gain = 1.0;
        project.tracks[0].polarity_inverted = true;
        project.tracks[1].gain = 2.0;
        let samples = HashMap::from([(
            1,
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![0.75; 4410],
            },
        )]);
        let mix = mix_with_samples(&project, &samples).unwrap();
        let whole = mix.to_pcm(&project.master, &project.fx).unwrap();
        assert_eq!(&whole.samples[..2], &[0.75, 0.75]);
        rejected_stems_keep_saved_files(&project, &mix, "Track 2");
    }

    #[test]
    fn over_range_fx_return_stems_reject_and_keep_saved_files() {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        let mut project = editor.project().clone();
        project.tracks[0].sends[2] = 1.0;
        project.fx.buses[2].limiter = false;
        project.fx.buses[2].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
            mode: crate::fx::TimeMode::Free,
            time_ms_l: 20.0,
            time_ms_r: 20.0,
            feedback: 0.0,
            ping_pong: false,
            width: 100.0,
            low_cut: 20.0,
            high_cut: 20_000.0,
            mod_depth: 0.0,
            character: crate::fx::DelayCharacter::Digital,
            duck_amount: 0.0,
            output_db: 12.0,
            ..Default::default()
        });
        let mix = mix_with_samples(&project, &HashMap::from([(1, tone(1000.0))])).unwrap();
        rejected_stems_keep_saved_files(&project, &mix, "FX 3 return");
    }

    #[test]
    fn exported_tracks_and_returns_sum_to_the_mix_without_the_board() {
        let mut editor = crate::engine::Editor::new(Project::default());
        for (sample, track) in [(1, 0), (2, 2), (3, 4), (1, 6)] {
            editor.place(sample, track, 0, 256).unwrap();
        }
        let mut project = editor.project().clone();
        project.title = "Reality: take 2".into();
        let samples = HashMap::from([(1, tone(110.0)), (2, tone(3000.0)), (3, tone(700.0))]);
        project.master_gain = 0.5;
        project.tracks[0].gain = 0.6;
        project.tracks[0].pan = -0.4;
        project.tracks[0].sends[1] = 0.5;
        project.tracks[2].eq = crate::eq::ChannelEq {
            on: true,
            high_db: -6.0,
            ..Default::default()
        };
        project.tracks[2].sends[1] = 0.3;
        project.tracks[4].automation.volume = vec![
            crate::automation::Point {
                tick: 0,
                value: -20.0,
            },
            crate::automation::Point {
                tick: 256,
                value: 0.0,
            },
        ];
        project.tracks[4].right_enabled = false;
        project.tracks[4].sends[3] = 0.4;
        // A muted track and an unpowered bus have no file.
        project.tracks[6].muted = true;
        project.fx.buses[1].module = crate::fx::Module::AetherformReverb(
            crate::fx::Aetherform::preset("Dark Hall").unwrap(),
        );
        project.fx.buses[3].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
            mode: crate::fx::TimeMode::Free,
            time_ms_l: 300.0,
            time_ms_r: 300.0,
            feedback: 30.0,
            ..crate::fx::Echoform::default()
        });
        project.fx.buses[2].module = project.fx.buses[3].module;
        project.fx.buses[2].powered = false;
        // The board is left out of the parts, whatever it holds.
        project.master.trim = 6.0;
        project.master.limiter.on = true;

        let mix = mix_with_samples(&project, &samples).unwrap();
        let folder = std::env::temp_dir().join(format!("mixjam-stems-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let written = mix
            .write_stems(&folder, &project.title, &project.fx)
            .unwrap();
        assert_eq!(
            written,
            Stems {
                tracks: 3,
                returns: 2
            }
        );
        let mut names: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "Reality_ take 2 - FX 2 return.wav",
                "Reality_ take 2 - FX 4 return.wav",
                "Reality_ take 2 - Track 01.wav",
                "Reality_ take 2 - Track 03.wav",
                "Reality_ take 2 - Track 05.wav",
            ]
        );
        let parts: Vec<Pcm> = names
            .iter()
            .map(|name| read_wav(&folder.join(name)).unwrap())
            .collect();
        let whole = folder.join("mix.wav");
        let bypassed = MasterChain {
            bypass: true,
            ..project.master
        };
        mix.write_wav(&whole, &bypassed, &project.fx).unwrap();
        let whole = read_wav(&whole).unwrap();
        std::fs::remove_dir_all(&folder).unwrap();
        assert!(whole.frames() > mix.frames() + 44_100, "the tails ring on");
        for part in &parts {
            assert_eq!(part.samples.len(), whole.samples.len());
            assert!(part.samples.iter().any(|sample| sample.abs() > 0.01));
        }
        // Each file rounds to 16 bits once; so does the mix.
        let step = 1.0 / 32768.0;
        let worst = (0..whole.samples.len())
            .map(|at| {
                let sum: f32 = parts.iter().map(|part| part.samples[at]).sum();
                (sum - whole.samples[at]).abs()
            })
            .fold(0.0_f32, f32::max);
        assert!(
            worst <= (parts.len() + 1) as f32 * step,
            "the parts differ from the mix by {} steps",
            worst / step
        );
        // Track 5 plays on the left only.
        let track5 = &parts[4];
        assert!(track5.samples.iter().skip(1).step_by(2).all(|s| *s == 0.0));
    }

    /// A song that runs every part of playback: a lane with its EQ,
    /// compressor, and a ducker keyed by another lane, a narrowed lane with
    /// filter automation, a lane with volume automation, sends to a delay
    /// whose tail dies and to a reverb, and master pedals, one of them keyed.
    fn busy_song() -> (Project, HashMap<u32, Pcm>) {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        editor.place(2, 1, 64, 192).unwrap();
        editor.place(3, 2, 128, 128).unwrap();
        let mut project = editor.project().clone();
        let points = |points: &[(u32, f32)]| -> Vec<crate::automation::Point> {
            points
                .iter()
                .map(|&(tick, value)| crate::automation::Point { tick, value })
                .collect()
        };
        project.master_gain = 0.6;
        let lane = &mut project.tracks[0];
        lane.eq = crate::eq::ChannelEq {
            on: true,
            high_db: -6.0,
            ..Default::default()
        };
        lane.compressor = crate::compressor::Compressor {
            on: true,
            threshold: -24.0,
            ratio: 4.0,
            ..Default::default()
        };
        lane.ducker = crate::ducker::Ducker {
            on: true,
            key: 2,
            threshold: -24.0,
            depth: 12.0,
            attack: 1.0,
            release: 50.0,
        };
        lane.sends[0] = 0.4;
        project.tracks[1].width = 50.0;
        project.tracks[1].automation.filter = points(&[(0, -0.5), (256, 0.3)]);
        project.tracks[1].sends[1] = 0.5;
        project.tracks[2].automation.volume = points(&[(128, -12.0), (256, 0.0)]);
        project.fx.buses[0].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
            mode: crate::fx::TimeMode::Free,
            time_ms_l: 120.0,
            time_ms_r: 180.0,
            feedback: 40.0,
            ..crate::fx::Echoform::default()
        });
        project.fx.buses[1].module = crate::fx::Module::AetherformReverb(crate::fx::Aetherform {
            decay_seconds: 0.5,
            ..crate::fx::Aetherform::default()
        });
        project.master.bus_compressor = crate::master::BusCompressor {
            on: true,
            threshold: -20.0,
            ratio: 4.0,
            attack: 5.0,
            release: 100.0,
            key: 3,
        };
        project.master.multiband.on = true;
        project.master.limiter.on = true;
        let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0)), (3, tone(880.0))]);
        (project, samples)
    }

    /// Play `project` from its start in blocks of `block` frames. With
    /// `edit`, the live controls take the edited project once the source
    /// has rendered `at` frames, a multiple of `block`.
    fn play_in_blocks(
        mix: &Arc<Mix>,
        project: &Project,
        block: usize,
        edit: Option<(usize, &Project)>,
    ) -> Vec<f32> {
        let control = MasterControl::new(&project.master);
        control.set_returns(&project.fx);
        control.set_levels(project);
        let audio: Arc<dyn Frames> = mix.clone();
        let mut source = PlaybackSource::with_block(audio, 0, None, Some(control.clone()), block);
        let mut played = Vec::new();
        if let Some((at, edited)) = edit {
            let lead = source.lead;
            played.extend(source.by_ref().take((at - lead) * 2));
            assert_eq!((source.frame, source.cursor), (at as u64, source.filled));
            control.set(&edited.master);
            control.set_returns(&edited.fx);
            control.set_levels(edited);
        }
        played.extend(source);
        played
    }

    #[test]
    fn playback_does_not_depend_on_its_block_size() {
        let (project, samples) = busy_song();
        let mix = Arc::new(mix_with_samples(&project, &samples).unwrap());
        let exported = mix.to_pcm(&project.master, &project.fx).unwrap().samples;
        // The buses ring on for several tail blocks after the song.
        let tail = exported.len() / 2 - mix.frames() as usize;
        assert!(tail > 8 * Mix::TAIL_BLOCK as usize, "{tail}");
        for block in [PlaybackSource::BLOCK_FRAMES, 100] {
            let played = play_in_blocks(&mix, &project, block, None);
            assert_eq!(played.len(), exported.len(), "{block}-frame blocks");
            assert!(
                played == exported,
                "{block}-frame blocks differ from export"
            );
        }
        // Live edits take effect at the same frame whatever the block size,
        // a moved fader glides over the same frames, and a switched pedal
        // crossfades over them.
        let mut edited = project.clone();
        edited.tracks[0].gain = 0.3;
        edited.tracks[1].pan = -0.5;
        edited.master_gain = 0.8;
        edited.master.tape.on = true;
        edited.fx.buses[0].return_level = 0.5;
        let at = 25_600;
        let whole = play_in_blocks(&mix, &project, 1024, Some((at, &edited)));
        assert!(whole != exported);
        let short = play_in_blocks(&mix, &project, 100, Some((at, &edited)));
        assert!(short == whole, "100-frame blocks differ after an edit");
    }

    /// Audio that counts its reads and says on which thread it is freed.
    struct Probe {
        reads: Arc<std::sync::atomic::AtomicUsize>,
        freed: std::sync::mpsc::Sender<std::thread::ThreadId>,
    }

    impl Frames for Probe {
        fn channels(&self) -> u16 {
            2
        }
        fn sample_rate(&self) -> u32 {
            44_100
        }
        fn frames(&self) -> u64 {
            1_000_000
        }
        fn read(&self, _first_frame: u64, out: &mut [f32]) {
            self.reads.fetch_add(1, Ordering::Relaxed);
            out.fill(0.25);
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            let _ = self.freed.send(std::thread::current().id());
        }
    }

    fn probe() -> (
        Playing,
        Arc<std::sync::atomic::AtomicUsize>,
        std::sync::mpsc::Receiver<std::thread::ThreadId>,
    ) {
        let reads = Arc::default();
        let (freed, freeing) = std::sync::mpsc::channel();
        let audio = Arc::new(Probe {
            reads: Arc::clone(&reads),
            freed,
        });
        let playing = Playing::new(PlaybackSource::new(audio, 0, None, None));
        (playing, reads, freeing)
    }

    #[test]
    fn a_stopped_playback_plays_out_its_block_and_renders_no_other() {
        let (mut playing, reads, _freeing) = probe();
        let block = PlaybackSource::BLOCK_FRAMES * 2;
        assert_eq!(playing.by_ref().take(block + 10).count(), block + 10);
        assert_eq!(reads.load(Ordering::Relaxed), 2);
        playing.stop.store(true, Ordering::Relaxed);
        assert_eq!(playing.by_ref().count(), block - 10);
        assert_eq!(reads.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn retired_playback_is_freed_on_another_thread() {
        let (mut playing, _reads, freeing) = probe();
        assert!(playing.next().is_some());
        drop(playing);
        let thread = freeing
            .recv_timeout(Duration::from_secs(10))
            .expect("the audio is freed");
        assert_ne!(thread, std::thread::current().id());
    }

    #[test]
    fn playback_asks_for_an_audio_threads_priority_once_per_thread() {
        std::thread::spawn(|| {
            SYSTEM_PRIORITY.with(|system| system.set(true));
            let requests = || PRIORITY_REQUESTS.with(std::cell::Cell::get);
            let audio: Arc<dyn Frames> = Arc::new(Pcm {
                sample_rate: 44_100,
                channels: 2,
                samples: vec![0.0; 4096],
            });
            let first = PlaybackSource::new(Arc::clone(&audio), 0, None, None);
            let second = PlaybackSource::new(audio, 0, None, None);
            assert_eq!(requests(), 0, "made before the device reads");
            #[cfg(windows)]
            // SAFETY: the pseudo handle names the calling thread.
            let priority = || unsafe {
                windows::Win32::System::Threading::GetThreadPriority(
                    windows::Win32::System::Threading::GetCurrentThread(),
                )
            };
            #[cfg(windows)]
            let normal = priority();
            assert_eq!(first.count() + second.count(), 2 * 4096);
            assert_eq!(requests(), 1);
            assert_eq!(priority_failure(), None);
            #[cfg(windows)]
            assert!(priority() > normal, "{} after {normal}", priority());
        })
        .join()
        .unwrap();
    }
}
