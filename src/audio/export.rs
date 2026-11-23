//! Rendering a prepared mix: to memory, to a WAV file, or to stems.
use super::{
    Frames, Mix, Pcm, RenderState, float_to_i16,
    schedule::{Phase, Schedule},
};
use crate::{
    ducker::Keys,
    fx::{BUSES, FxBuses, ReturnsProcessor},
    master::{MasterChain, MasterProcessor},
};
use std::path::{Path, PathBuf};

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

const NO_MEMORY: &str = "There is not enough memory to hold this song. Export it instead.";

pub const NO_TRACK_SOUNDS: &str = "No track sounds: add clips, or check Mute, Solo, and L/R.";

impl Mix {
    /// The most frames that a stereo PCM16 WAV file holds. Both the data
    /// chunk and the RIFF size are 32-bit, and the RIFF payload also has 36
    /// bytes of headers. This is the format's limit, about 6.7 hours at the
    /// mix rate; a song may be shorter and always plays.
    pub(super) const MAX_WAV_FRAMES: u64 = (u32::MAX as u64 - 36) / 4;
    /// The block that export mixes at a time during the song.
    pub(super) const EXPORT_BLOCK: u64 = 65_536;
    /// Tails go in short blocks, so an export ends soon after they die.
    /// Export, stem export, and playback share the `Schedule` that asks
    /// whether the buses still ring once a tail block, so they end at the
    /// same frame.
    pub(super) const TAIL_BLOCK: u64 = 1024;

    /// An empty stereo sample buffer with room for `frames`, or an error if
    /// the memory is not there. The buffer is reserved whole, so a long song
    /// does not copy itself again and again as it grows.
    pub(super) fn reserve_pcm(frames: u64) -> Result<Vec<f32>, String> {
        let samples = usize::try_from(frames)
            .ok()
            .and_then(|frames| frames.checked_mul(2))
            .ok_or(NO_MEMORY)?;
        let mut buffer = Vec::new();
        buffer.try_reserve_exact(samples).map_err(|_| NO_MEMORY)?;
        Ok(buffer)
    }

    fn check_export_quality(&self) -> Result<(), String> {
        if self.quality != crate::playback_quality::PlaybackQuality::FULL
            || self.render_quality != crate::fx::RenderQuality::Offline
        {
            return Err("Prepare the song at full quality before export.".into());
        }
        Ok(())
    }

    /// The whole mix in memory, through the FX buses and the master
    /// pedalboard. Long songs play and export block by block through `read_block`
    /// and `write_wav`, which need no room for the whole song. Pedals that
    /// delay the sound run on past the end, and the delay is cut from the
    /// start, so the result lines up with the song; the buses' tails ring on
    /// after it. A song has no length limit here: the samples are reserved up
    /// front, and a machine without the memory gets a plain error.
    pub fn to_pcm(&self, master: &MasterChain, fx: &FxBuses) -> Result<Pcm, String> {
        self.check_export_quality()?;
        let mut samples = Self::reserve_pcm(self.frames)?;
        self.render(master, fx, |block| {
            samples.try_reserve(block.len()).map_err(|_| NO_MEMORY)?;
            samples.extend_from_slice(block);
            Ok(())
        })?;
        Ok(Pcm {
            sample_rate: Self::SAMPLE_RATE,
            channels: 2,
            samples,
        })
    }

    fn build_returns(&self, fx: &FxBuses) -> (Option<ReturnsProcessor>, usize) {
        let returns = (self.sends && fx.is_active()).then(|| ReturnsProcessor::new(fx, self.bpm));
        let latency = returns
            .as_ref()
            .map_or(0, ReturnsProcessor::latency_samples);
        (returns, latency)
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
        let (mut returns, fx_latency) = self.build_returns(fx);
        let block_frames = Self::EXPORT_BLOCK as usize;
        let mut key_delay = KeyDelay::new(
            self.delayed_key_stride(master, fx_latency),
            fx_latency,
            block_frames,
        );
        let mut master = MasterProcessor::new(master);
        // Both FX oversampling/limiting and master pedals run past the end by their delay,
        // which is trimmed from the start so the export aligns with song frame 0.
        let latency = fx_latency + master.latency();
        let mut schedule = Schedule::new(block_frames, latency);
        let mut block = vec![0.0; block_frames * 2];
        let mut sends = vec![0.0; block_frames * 2 * BUSES];
        // Frames fed into the board so far.
        let mut position = 0;
        let mut state = RenderState::default();
        while let Some((phase, count)) = schedule.next(self.frames.saturating_sub(position), || {
            returns.as_ref().is_some_and(ReturnsProcessor::ringing)
        }) {
            let samples = &mut block[..count * 2];
            let sends = &mut sends[..count * 2 * BUSES];
            let key_frames = self.feed_block(
                returns.as_mut(),
                phase,
                position,
                samples,
                sends,
                &mut state,
            );
            let keys = self.key_block(&state, key_frames);
            master.process_keyed(samples, key_delay.delay(keys, &self.key_tracks, count));
            let skip = (latency as u64).saturating_sub(position).min(count as u64) as usize;
            write(&samples[skip * 2..])?;
            position += count as u64;
        }
        Ok(())
    }

    /// The samples in one frame of the master's delayed keys. When the buses
    /// delay the sound, the export delays the master's key audio by as much,
    /// so that keyed pedals hear their key in step. Zero when no key needs it.
    fn delayed_key_stride(&self, master: &MasterChain, fx_latency: usize) -> usize {
        if fx_latency > 0 && self.side && master.key_tracks().next().is_some() {
            2 * self.key_tracks.len()
        } else {
            0
        }
    }

    /// Fill `samples` with one block of the export through the buses: the
    /// song in the song phase, silence in the tail and the flush. Gives the
    /// frames of key audio that the block read.
    fn feed_block(
        &self,
        returns: Option<&mut ReturnsProcessor>,
        phase: Phase,
        position: u64,
        samples: &mut [f32],
        sends: &mut [f32],
        state: &mut RenderState,
    ) -> usize {
        let count = samples.len() / 2;
        match (phase, returns) {
            (Phase::Song, Some(returns)) => {
                let sending = self.read_block(position, samples, Some(sends), state);
                returns.process_at(
                    samples,
                    sending.then_some(&*sends),
                    self.key_block(state, count),
                    self.lanes(position),
                );
                count
            }
            (Phase::Song, None) => {
                self.read_block(position, samples, None, state);
                count
            }
            (Phase::Tail { .. } | Phase::Flush, returns) => {
                samples.fill(0.0);
                if let Some(returns) = returns {
                    returns.process_at(samples, None, Keys::NONE, self.lanes(position));
                }
                0
            }
        }
    }

    /// Every exported file: stereo PCM16 at the mix rate.
    const EXPORT_SPEC: hound::WavSpec = hound::WavSpec {
        channels: 2,
        sample_rate: Self::SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    /// Refuse a file whose audio, with `tail` frames after the song, does
    /// not fit a WAV: both the data chunk and the RIFF size are 32-bit, and
    /// stereo PCM16 uses four bytes per frame plus 36 bytes of RIFF payload.
    fn check_wav_length(&self, tail: u64) -> Result<(), String> {
        if self.frames + tail > Self::MAX_WAV_FRAMES {
            return Err("This song is too long for a WAV file.".into());
        }
        Ok(())
    }

    /// Write stereo PCM16 through the FX buses and the master pedalboard,
    /// mixing one block at a time.
    pub fn write_wav(&self, path: &Path, master: &MasterChain, fx: &FxBuses) -> Result<(), String> {
        self.check_export_quality()?;
        self.check_wav_length(if self.sends && fx.is_active() {
            crate::fx::TAIL_LIMIT
        } else {
            0
        })?;
        crate::atomic_file::write_with(path, |file| {
            let mut writer =
                hound::WavWriter::new(std::io::BufWriter::new(file), Self::EXPORT_SPEC)
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
    pub(super) fn sounding_tracks(&self) -> Vec<usize> {
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
    pub(super) fn returning_buses(&self, fx: &FxBuses) -> Vec<usize> {
        (0..BUSES)
            .filter(|&bus| self.fed[bus] && fx.buses[bus].powered && fx.buses[bus].is_active())
            .collect()
    }

    /// Write the premaster in parts to stereo PCM16 files in `folder`: each
    /// sounding track alone, after its EQ, compressor, width, automation,
    /// fader, pan, ducker, and outputs but without its sends, and the return
    /// of each bus in `returning_buses`. Every file runs from the song's start until the
    /// buses' tails die, as the mix export does. If the summed mix does not
    /// clip, the files sum to the mix with the master pedalboard bypassed.
    /// An out-of-range part rejects before any file is replaced. Each file
    /// is named after `song`.
    pub fn write_stems(&self, folder: &Path, song: &str, fx: &FxBuses) -> Result<Stems, String> {
        self.check_export_quality()?;
        let tracks = self.sounding_tracks();
        if tracks.is_empty() {
            return Err(NO_TRACK_SOUNDS.into());
        }
        let buses = self.returning_buses(fx);
        self.check_wav_length(if buses.is_empty() {
            0
        } else {
            crate::fx::TAIL_LIMIT
        })?;
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
        crate::atomic_file::write_many(&paths, |files| {
            let mut writers = files
                .iter_mut()
                .map(|file| hound::WavWriter::new(std::io::BufWriter::new(file), Self::EXPORT_SPEC))
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
    pub(super) fn render_stems(
        &self,
        fx: &FxBuses,
        tracks: &[usize],
        buses: &[usize],
        mut write: impl FnMut(usize, &[f32]) -> Result<(), String>,
    ) -> Result<(), String> {
        let (mut returns, fx_latency) = self.build_returns(fx);
        let block_frames = Self::EXPORT_BLOCK as usize;
        let mut dry = vec![0.0; block_frames * 2];
        let mut feed = vec![0.0; block_frames * 2 * BUSES];
        let mut sends = vec![0.0; block_frames * 2 * BUSES];
        let mut wet = vec![0.0; block_frames * 2 * BUSES];
        let mut part = vec![0.0; block_frames * 2];
        // Each track keeps its own filter histories, as a playback would.
        let mut parts: Vec<(usize, RenderState)> = tracks
            .iter()
            .map(|&track| (track, RenderState::default()))
            .collect();
        // Track stems carry zero delay; FX stems flush `fx_latency` and trim the same at start.
        let mut schedule = Schedule::new(block_frames, fx_latency);
        let mut position = 0;
        while let Some((phase, count)) = schedule.next(self.frames.saturating_sub(position), || {
            returns.as_ref().is_some_and(ReturnsProcessor::ringing)
        }) {
            let dry = &mut dry[..count * 2];
            let wet = &mut wet[..count * 2 * BUSES];
            let sends = &mut sends[..count * 2 * BUSES];
            let feed = returns
                .is_some()
                .then(|| (&mut feed[..count * 2 * BUSES], &mut *sends));
            let sending =
                self.write_dry_parts(phase, position, &mut parts, dry, feed, &mut write)?;
            if let Some(returns) = &mut returns {
                let sends = sending.then_some(&*sends);
                self.split_returns(returns, wet, sends, &parts[0].1, position);
            }
            let skip_wet = (fx_latency as u64)
                .saturating_sub(position)
                .min(count as u64) as usize;
            let wet = &wet[skip_wet * 2 * BUSES..];
            write_return_parts(wet, buses, &mut part, tracks.len(), &mut write)?;
            position += count as u64;
        }
        Ok(())
    }

    /// Hand each track's part of one block to `write`, as `write_track_blocks`
    /// gives it in the song and as silence in the tail. The flush has no
    /// track parts, because the tracks carry no delay. Gives whether the
    /// block feeds the buses.
    fn write_dry_parts(
        &self,
        phase: Phase,
        position: u64,
        parts: &mut [(usize, RenderState)],
        dry: &mut [f32],
        feed: Option<(&mut [f32], &mut [f32])>,
        write: &mut impl FnMut(usize, &[f32]) -> Result<(), String>,
    ) -> Result<bool, String> {
        match phase {
            Phase::Song => {
                self.write_track_blocks(position, parts, dry, feed, write)?;
                Ok(true)
            }
            Phase::Tail { .. } => {
                dry.fill(0.0);
                for index in 0..parts.len() {
                    write(index, dry)?;
                }
                Ok(false)
            }
            Phase::Flush => Ok(false),
        }
    }

    /// Split one block of the buses' returns into `wet`. With `sends`, the
    /// song feeds the buses, and the buses read the key audio of `key_state`.
    fn split_returns(
        &self,
        returns: &mut ReturnsProcessor,
        wet: &mut [f32],
        sends: Option<&[f32]>,
        key_state: &RenderState,
        position: u64,
    ) {
        let keys = match sends {
            Some(_) => self.key_block(key_state, wet.len() / (2 * BUSES)),
            None => Keys::NONE,
        };
        returns.split(wet, sends, keys, self.lanes(position));
    }

    /// Mix each track's block of the song alone into `dry`, and hand it to
    /// `write` as its part. With `feed`, which holds a scratch buffer and the
    /// block's sends, the tracks' sends add up in the sends.
    fn write_track_blocks(
        &self,
        position: u64,
        parts: &mut [(usize, RenderState)],
        dry: &mut [f32],
        mut feed: Option<(&mut [f32], &mut [f32])>,
        write: &mut impl FnMut(usize, &[f32]) -> Result<(), String>,
    ) -> Result<(), String> {
        if let Some((_, sends)) = &mut feed {
            sends.fill(0.0);
        }
        for (index, (track, state)) in parts.iter_mut().enumerate() {
            let track = *track;
            // The first track's state keeps the key audio that the buses read.
            let side = index == 0;
            match &mut feed {
                Some((scratch, sends)) => {
                    self.mix_keyed(position, dry, Some(scratch), state, Some(track), side);
                    for (sum, value) in sends.iter_mut().zip(scratch.iter()) {
                        *sum += value;
                    }
                }
                None => self.mix_keyed(position, dry, None, state, Some(track), side),
            }
            write(index, dry)?;
        }
        Ok(())
    }
}

/// Hand the return of each bus in `buses` to `write` as parts `first`,
/// `first + 1`, and so on. `wet` holds the returns of all buses, frame by
/// frame; `part` is the scratch buffer for one part.
fn write_return_parts(
    wet: &[f32],
    buses: &[usize],
    part: &mut [f32],
    first: usize,
    write: &mut impl FnMut(usize, &[f32]) -> Result<(), String>,
) -> Result<(), String> {
    let frames = wet.len() / (2 * BUSES);
    if frames == 0 {
        return Ok(());
    }
    let part = &mut part[..frames * 2];
    for (offset, &bus) in buses.iter().enumerate() {
        for (out, frame) in part
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(wet.as_chunks::<{ 2 * BUSES }>().0)
        {
            *out = [frame[bus * 2], frame[bus * 2 + 1]];
        }
        write(first + offset, part)?;
    }
    Ok(())
}

/// The master's key audio in an export, delayed by the buses' latency.
struct KeyDelay {
    /// The samples in one frame of keys, or 0 when the keys need no delay.
    stride: usize,
    /// The last `latency` frames of keys, written in a circle at `write`.
    ring: Vec<f32>,
    write: usize,
    /// One block of delayed keys.
    delayed: Vec<f32>,
}

impl KeyDelay {
    fn new(stride: usize, latency: usize, block_frames: usize) -> Self {
        Self {
            stride,
            ring: vec![0.0; latency * stride],
            write: 0,
            delayed: vec![0.0; block_frames * stride],
        }
    }

    /// The keys of a block of `count` frames, delayed. Key frames that the
    /// block does not have, as in the tail, enter the delay as silence.
    fn delay<'a>(&'a mut self, keys: Keys<'a>, tracks: &'a [usize], count: usize) -> Keys<'a> {
        let stride = self.stride;
        if stride == 0 {
            return keys;
        }
        let delayed = &mut self.delayed[..count * stride];
        let mut source = keys.audio.chunks_exact(stride);
        for out in delayed.chunks_exact_mut(stride) {
            let slot = &mut self.ring[self.write..self.write + stride];
            out.copy_from_slice(slot);
            match source.next() {
                Some(input) => slot.copy_from_slice(input),
                None => slot.fill(0.0),
            }
            self.write = (self.write + stride) % self.ring.len();
        }
        Keys {
            tracks,
            audio: delayed,
        }
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
