//! Rendering a prepared mix: to memory, to a WAV file, or to stems.
use super::{Frames, Mix, Pcm, RenderState, float_to_i16};
use crate::{
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

impl Mix {
    /// The most frames that a stereo PCM16 WAV file holds. Both the data
    /// chunk and the RIFF size are 32-bit, and the RIFF payload also has 36
    /// bytes of headers. This is the format's limit, about 6.7 hours at the
    /// mix rate; a song may be shorter and always plays.
    pub(super) const MAX_WAV_FRAMES: u64 = (u32::MAX as u64 - 36) / 4;
    /// The block that export mixes at a time during the song.
    pub(super) const EXPORT_BLOCK: u64 = 65_536;
    /// Tails go in short blocks, so an export ends soon after they die.
    /// Playback asks whether the buses still ring once a tail block too, so
    /// it ends at the same frame.
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
        if self.quality != crate::playback_quality::PlaybackQuality::FULL {
            return Err("Prepare the song at full quality before export.".into());
        }
        Ok(())
    }

    /// The whole mix in memory, through the FX buses and the master
    /// pedalboard. Long songs play and export block by block through `read`
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
                        returns.process_at(
                            samples,
                            sending.then_some(&*sends),
                            self.key_block(&state, count),
                            self.lanes(position),
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
                    returns.process_at(
                        samples,
                        None,
                        crate::ducker::Keys::NONE,
                        self.lanes(position),
                    );
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
    /// fader, pan, ducker, and
    /// outputs but without its sends, and the return of each bus in
    /// `returning_buses`. Every file runs from the song's start until the
    /// buses' tails die, as the mix export does. If the summed mix does not
    /// clip, the files sum to the mix with the master pedalboard bypassed.
    /// An out-of-range part rejects before any file is replaced. Each file
    /// is named after `song`.
    pub fn write_stems(&self, folder: &Path, song: &str, fx: &FxBuses) -> Result<Stems, String> {
        self.check_export_quality()?;
        let tracks = self.sounding_tracks();
        if tracks.is_empty() {
            return Err("No track sounds: add clips, or check Mute, Solo, and L/R.".into());
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
                    returns.split(
                        wet,
                        Some(sends),
                        self.key_block(&states[0], count),
                        self.lanes(position),
                    );
                }
            } else {
                dry.fill(0.0);
                for index in 0..tracks.len() {
                    write(index, dry)?;
                }
                if let Some(returns) = &mut returns {
                    returns.split(wet, None, crate::ducker::Keys::NONE, self.lanes(position));
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
