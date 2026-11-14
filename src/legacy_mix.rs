//! Reader for the 0x889 project profile found in factory and custom songs.
//! Unknown preamble data is kept outside the audio arrangement decoder. A match
//! must contain all sixteen numbered, counted track lists and the exact trailer.
#[cfg(test)]
mod tests;

use crate::{
    compat::chunks::{self, Event, Header, Reference, Timeline, WaveReference},
    engine::{MAX_CLIPS, MAX_TICKS, Project, TICKS_PER_BAR, Track},
    library::{Library, decode_1252},
    products::Product,
};
use std::path::Path;

const TRAILER: &[u8] = &[17, 0, 0, 0, 0, 0xa9, 0x7a, 0x8a, 1];

/// The product labels that custom 0x889 files save: the product's own, the
/// earlier one, or none. The reader compares the trimmed label with these.
const OBSERVED_LABELS: [&str; 3] = ["\x54echno \x65Jay 2", "\x54echno \x65Jay", ""];

pub fn load(path: &Path, library: &Library) -> Result<Project, String> {
    load_named_bytes(
        path,
        &crate::compat::song_file::read_song_file(path)?,
        library,
    )
}

#[cfg(test)]
pub(crate) fn load_bytes(bytes: &[u8], library: &Library) -> Result<Project, String> {
    load_named_bytes(Path::new("original.mix"), bytes, library)
}

fn load_named_bytes(path: &Path, bytes: &[u8], library: &Library) -> Result<Project, String> {
    let mut reader = Reader { bytes, offset: 0 };
    if reader.u32()? != 0x889 {
        return Err("This original mix version is not supported.".into());
    }
    let _unknown_header = reader.u32()?;
    let source_bpm = reader.u16()?;
    // Original writer 0x508add stores four-byte effective BPM from 0x526748.
    // Pitch handler 0x46cf7e derives that value from output rate and source BPM.
    let playback_bpm = reader.u32()?;
    if source_bpm != 140 || !(30..=300).contains(&playback_bpm) {
        return Err("The original mix has unsupported tempo data.".into());
    }
    let author = reader
        .string()?
        .split('\0')
        .next()
        .unwrap_or_default()
        .to_string();
    let title = reader.string()?;
    let mut tracks = Vec::with_capacity(16);
    for _ in 0..16 {
        let routing = reader.u16()?;
        let volume = reader.u16()?;
        if routing > 3 || volume > 32767 {
            return Err("The original mix has unsupported mixer data.".into());
        }
        tracks.push(Track {
            gain: volume as f32 / 32767.0,
            polarity_inverted: false,
            pan: 0.0,
            left_enabled: matches!(routing, 0 | 1),
            right_enabled: matches!(routing, 0 | 2),
            ..Track::default()
        });
    }
    // The installed profile has ten further bytes, followed by a product name,
    // sample-kit declarations and drum instrument settings. Those controls are
    // not yet reconstructed. The track lists are independently self-delimiting.
    reader.take(10)?;
    let product = reader.string()?;
    // Custom 0x889 files also save the earlier label or an empty label. Admit
    // only those observed labels, then require the entire counted arrangement
    // and exact trailer before resolving any sample or writing imported audio.
    if !OBSERVED_LABELS.contains(&product.trim()) {
        return Err("The mix belongs to a different product.".into());
    }
    let search_start = reader.offset;
    let search_end = bytes
        .len()
        .saturating_sub(16 * 5 + TRAILER.len())
        .min(search_start + 4096);
    let mut arrangement = None;
    let payload_ranges = chunks::embedded_wave_ranges(bytes)?;
    let mut payload_index = 0;
    // Preamble bytes can resemble a track list. Resolve audio only after a
    // unique complete structural parse, independently of available samples.
    for start in search_start..=search_end {
        while payload_index < payload_ranges.len() && payload_ranges[payload_index].end <= start {
            payload_index += 1;
        }
        if payload_ranges
            .get(payload_index)
            .is_some_and(|range| range.contains(&start))
        {
            continue;
        }
        if bytes.get(start) != Some(&1) {
            continue;
        }
        if let Ok(events) = parse_arrangement(bytes, start) {
            if arrangement.is_some() {
                return Err("The original mix has ambiguous arrangement data.".into());
            }
            arrangement = Some(Timeline { start, events });
        }
    }
    let timeline = arrangement.ok_or(
        "Cannot decode the original mix arrangement. The file is damaged or uses an unsupported \
         profile.",
    )?;
    let header = Header {
        title,
        author,
        bpm: source_bpm as f64,
        effective_bpm: playback_bpm as f64,
        tracks,
        source_notices: Vec::new(),
        settings: Default::default(),
        after_mixer: search_start,
    };
    chunks::build_project(path, bytes, Product::T2, library, header, timeline)
}

fn parse_arrangement(bytes: &[u8], start: usize) -> Result<Vec<Event>, String> {
    let mut reader = Reader {
        bytes,
        offset: start,
    };
    let mut events = Vec::new();
    for track in 0..16 {
        if reader.u8()? as usize != track + 1 {
            return Err("Invalid track marker.".into());
        }
        let count = reader.u32()? as usize;
        if count > MAX_CLIPS - events.len()
            || count > reader.bytes.len().saturating_sub(reader.offset) / 7
        {
            return Err("Invalid clip count.".into());
        }
        for _ in 0..count {
            // Source 0 is a product sound, 1 an imported WAV, and 3 a Sample
            // Kit sound. Early releases saved kit sounds by their D2 index ID
            // (kit archives 4-6).
            let source = reader.u8()?;
            if !matches!(source, 0 | 1 | 3) {
                return Err("Unsupported clip source.".into());
            }
            let stored_position = reader.i32()?;
            let sample_id = reader.u16()?;
            let native_position = if stored_position < 0 {
                !stored_position as u32
            } else {
                stored_position as u32
            };
            // Native duration writer 0x4fd1d5 divides stored positions by 128.
            // Common projects use 256 ticks per bar, so convert exactly once.
            let start_tick = native_position
                .checked_mul(2)
                .filter(|value| *value <= MAX_TICKS)
                .ok_or("Invalid native clip position.")?;
            let (left_trim, right_trim, gain_delta) = if stored_position < 0 {
                (0, 0, 0)
            } else {
                (reader.u32()?, reader.u32()?, reader.i16()?)
            };
            let left_trim = left_trim
                .checked_mul(2)
                .filter(|value| *value <= MAX_TICKS)
                .ok_or("Invalid native clip trim.")?;
            let right_trim = right_trim
                .checked_mul(2)
                .filter(|value| *value <= MAX_TICKS)
                .ok_or("Invalid native clip trim.")?;
            let reference = match source {
                1 => {
                    let mut reference = read_wave_extension(&mut reader)?;
                    reference
                        .metadata
                        .insert("Saved sample ID".into(), sample_id.to_string());
                    Reference::Wave(reference)
                }
                3 => Reference::Kit(sample_id),
                _ => Reference::Native(sample_id),
            };
            events.push(Event {
                reference,
                track,
                position: start_tick,
                left_trim,
                right_trim,
                gain: (32767i32 + gain_delta as i32) as f32 / 32767.0,
            });
        }
    }
    if reader.take(TRAILER.len())? != TRAILER {
        return Err("Invalid mix trailer.".into());
    }
    if reader.offset != reader.bytes.len() {
        read_wave_appendix(&mut reader)?;
    }
    Ok(events)
}

/// Early releases append the song's imported WAV files
/// after the trailer, with a count even when it is zero. Each record is the
/// saved path, the WAV size and the whole RIFF file; the block ends the file.
/// The shared embedded-WAV scan resolves the audio, so this only checks that
/// the block is complete.
fn read_wave_appendix(reader: &mut Reader<'_>) -> Result<(), String> {
    let count = reader.u32()? as usize;
    if count > reader.bytes.len().saturating_sub(reader.offset) / 52 {
        return Err("Invalid embedded WAV count.".into());
    }
    for _ in 0..count {
        if !is_saved_wave_path(&reader.string()?) {
            return Err("Invalid embedded WAV path.".into());
        }
        let size = reader.u32()? as usize;
        let wav = reader.take(size)?;
        if size < 44
            || &wav[..4] != b"RIFF"
            || &wav[8..12] != b"WAVE"
            || u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize + 8 != size
        {
            return Err("Invalid embedded WAV.".into());
        }
    }
    if reader.offset != reader.bytes.len() {
        return Err("Invalid data after the embedded WAV files.".into());
    }
    Ok(())
}

/// The form in which the original saves a WAV path: a drive path that ends in
/// .wav and fits its 260-byte field.
fn is_saved_wave_path(path: &str) -> bool {
    path.len() <= 260
        && path.as_bytes().get(1) == Some(&b':')
        && path.to_ascii_lowercase().ends_with(".wav")
        && !path.contains('\0')
}

fn read_wave_extension(reader: &mut Reader<'_>) -> Result<WaveReference, String> {
    let mut metadata = std::collections::BTreeMap::new();
    // Original writer 0x50A5B5, 0x50A602 and 0x50A650 writes three strings.
    // 0x50A6A0..0x50A741 then writes u16/u32/u16 metadata. The middle
    // value is the nominal bar count: 0x42FDDE..0x42FDE1 multiplies it by 128.
    let path = reader.string()?;
    if !is_saved_wave_path(&path) {
        return Err("The T2 WAV reference is invalid.".into());
    }
    for index in 0..2 {
        let label = reader.string()?;
        if label.len() > 260 {
            return Err("The T2 WAV label is too long.".into());
        }
        metadata.insert(format!("Native label {}", index + 1), label);
    }
    metadata.insert("Native metadata word 1".into(), reader.u16()?.to_string());
    let bars = reader.u32()?;
    metadata.insert("Declared bars".into(), bars.to_string());
    let declared_ticks = bars
        .checked_mul(TICKS_PER_BAR)
        .filter(|ticks| (1..=MAX_TICKS).contains(ticks))
        .ok_or("The T2 WAV span exceeds the supported timeline.")?;
    metadata.insert("Native metadata word 2".into(), reader.u16()?.to_string());
    Ok(WaveReference {
        path,
        declared_ticks: Some(declared_ticks),
        metadata,
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or("Invalid mix length.")?;
        let data = self
            .bytes
            .get(self.offset..end)
            .ok_or("The original mix is truncated.")?;
        self.offset = end;
        Ok(data)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String, String> {
        let size = self.u16()? as usize;
        let bytes = self.take(size)?;
        let text = bytes
            .strip_suffix(&[0, 1])
            .ok_or("The mix has an unsupported text field.")?;
        Ok(decode_1252(text))
    }
}
