//! Counted timeline profiles from the installed Gen 2 and Gen 3 products.
//! Unsupported generated records cause an error before a project is returned.
use super::{drums, settings, song_file::read_song_file};
use crate::{
    engine::{
        Clip, ImportedSample, MAX_CLIPS, MAX_TICKS, MissingReference, MissingSample, Project,
        TICKS_PER_BAR, TRACK_COUNT, Track, normalize_import,
    },
    library::{Library, decode_1252},
    products::{IMPORTED_ID_START, Product},
};
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_EMBEDDED_WAVES: usize = 100_000;
const MAX_PENDING_ASSET_BYTES: usize = super::song_file::MIX_BYTE_LIMIT;
const SENTINEL: [u8; 4] = [0xa9, 0x7a, 0x8a, 0x01];
static NEXT_ASSET_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
#[path = "chunks/import_tests.rs"]
mod import_tests;

pub fn load(path: &Path, product: Product, library: &Library) -> Result<Project, String> {
    if product == Product::T2 {
        return crate::legacy_mix::load(path, library);
    }
    let bytes = read_song_file(path)?;
    let header = read_header(&bytes, product)?;
    let raw = read_timeline(&bytes, header.after_mixer, product)?;
    build_project(path, &bytes, product, library, header, raw)
}

fn read_external_wave(path: &Path) -> Result<Vec<u8>, String> {
    const MAX_WAVE_BYTES: usize = 64 * 1024 * 1024;
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > MAX_WAVE_BYTES as u64 {
        return Err("The external WAV file exceeds the 64 MiB limit.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_WAVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_WAVE_BYTES {
        return Err("The external WAV file exceeds the 64 MiB limit.".into());
    }
    Ok(bytes)
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
            .filter(|end| *end <= self.bytes.len())
            .ok_or("The native song record is truncated.")?;
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn block(&mut self) -> Result<&'a [u8], String> {
        let length = self.u16()? as usize;
        self.take(length)
    }
}

pub(crate) struct Header {
    pub(crate) title: String,
    pub(crate) author: String,
    pub(crate) bpm: f64,
    pub(crate) effective_bpm: f64,
    pub(crate) tracks: Vec<Track>,
    pub(crate) source_notices: Vec<String>,
    pub(crate) settings: settings::Saved,
    pub(crate) after_mixer: usize,
}

fn profile(product: Product) -> Result<(u32, usize, usize), String> {
    match product {
        Product::D2 => Ok((0xa19, 16, 4)),
        Product::T3 => Ok((0x2572, 32, 8)),
        Product::D3 => Ok((0x2571, 32, 8)),
        Product::H1 => Ok((0x11d6, 24, 8)),
        Product::D4 => Ok((0x2d41, 48, 8)),
        _ => Err("This product uses a different native song reader.".into()),
    }
}

fn read_header(bytes: &[u8], product: Product) -> Result<Header, String> {
    let (app_id, track_count, track_bytes) = profile(product)?;
    // These native mixer arrays include the generated-audio lane after the
    // ordinary sample lanes. Timeline admission still uses the core count.
    let mixer_count =
        track_count + usize::from(matches!(product, Product::D3 | Product::D4 | Product::T3));
    let mut reader = Reader { bytes, offset: 0 };
    if reader.u32()? != app_id {
        return Err("The native song belongs to a different product or profile.".into());
    }
    reader.u32()?;
    let bpm = f64::from(reader.u16()?);
    let effective_bpm = f64::from(reader.u32()?);
    if bpm <= 0.0 || effective_bpm <= 0.0 {
        return Err("The native song has invalid tempo data.".into());
    }
    if mixer_count > TRACK_COUNT {
        return Err("The native song declares more than 64 tracks.".into());
    }
    let author = first_text(reader.block()?);
    let title_block = reader.block()?;
    let title = first_text(title_block);
    let settings = settings::parse(title_block);
    let mut tracks = vec![Track::default(); TRACK_COUNT];
    let mut native_switches = Vec::new();
    let mut source_notices = Vec::new();
    for track in &mut tracks[..mixer_count] {
        let control = reader.u16()?;
        let gain = reader.u16()?;
        let signed_gain = match product {
            Product::D2 => native_fader_gain(gain, 1.25),
            _ => native_fader_gain(gain, 1.33),
        };
        track.set_legacy_gain(signed_gain);
        if track_bytes == 4 {
            if control > 3 {
                return Err("The native track has invalid output routing.".into());
            }
            track.left_enabled = matches!(control, 0 | 1);
            track.right_enabled = matches!(control, 0 | 2);
        } else {
            track.pan = decode_pan(control)?;
            track.left_enabled = track.pan < 1.0;
            track.right_enabled = track.pan > -1.0;
            native_switches.push((reader.u16()?, reader.u16()?));
        }
    }
    // The native dispatcher first excludes non-solo lanes when any saved solo
    // switch is 1, then excludes mute=1 lanes. A muted solo lane stays silent.
    let any_solo = native_switches.iter().any(|(_, solo)| *solo == 1);
    for (index, (mute, solo)) in native_switches.into_iter().enumerate() {
        let audible = mute != 1 && (!any_solo || solo != 0);
        tracks[index].left_enabled &= audible;
        tracks[index].right_enabled &= audible;
        if mute != 0 || solo != 0 {
            source_notices.push(format!(
                "Native track {}: mute={mute}, solo={solo}. Saved mute and solo settings were applied to the L/R outputs.",
                index + 1
            ));
        }
    }
    Ok(Header {
        title,
        author,
        bpm,
        effective_bpm,
        tracks,
        source_notices,
        settings,
        after_mixer: reader.offset,
    })
}

fn first_text(bytes: &[u8]) -> String {
    decode_1252(bytes.split(|byte| *byte == 0).next().unwrap_or_default())
        .trim_end_matches('\u{1}')
        .to_string()
}

// AMitte in the original D3, D4, T3, and House engines snaps the end and
// center regions, then uses Q8 balance factors with denominator 128 on both sides.
fn decode_pan(value: u16) -> Result<f32, String> {
    if value > 255 {
        return Err("The native track pan exceeds its supported range.".into());
    }
    Ok(match value {
        0..=14 => -1.0,
        117..=139 => 0.0,
        241..=255 => 1.0,
        _ => (f32::from(value) - 128.0) / 128.0,
    })
}

#[derive(Debug, PartialEq)]
pub(crate) enum Reference {
    Native(u16),
    Wave(WaveReference),
    Drum(drums::Pattern),
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct WaveReference {
    pub(crate) path: String,
    pub(crate) declared_ticks: Option<u32>,
    pub(crate) metadata: BTreeMap<String, String>,
}
#[derive(Debug, PartialEq)]
pub(crate) struct Event {
    pub(crate) track: usize,
    pub(crate) position: u32,
    pub(crate) left_trim: u32,
    pub(crate) right_trim: u32,
    pub(crate) gain: f32,
    pub(crate) reference: Reference,
}

pub(crate) struct Timeline {
    pub(crate) start: usize,
    pub(crate) events: Vec<Event>,
}

fn read_timeline(bytes: &[u8], lower_bound: usize, product: Product) -> Result<Timeline, String> {
    let last_marker = bytes
        .windows(4)
        .rposition(|value| value == SENTINEL)
        .filter(|offset| *offset >= lower_bound)
        .ok_or("The native song has no timeline end marker.")?;
    let mut result = None;
    let mut last_error = None;
    let mut payload_ranges: Vec<_> = if product == Product::D2 {
        Vec::new()
    } else {
        embedded_wave_records(bytes)?
            .into_iter()
            .map(|source| source.range)
            .collect()
    };
    let mut payload_index = 0;
    // Audio payloads can contain the marker bytes. Match a complete counted
    // timeline to its own length and marker, not the last marker in the file.
    // Each rejected candidate can cost a full timeline parse, so a file made
    // of candidates must not be searched without limit.
    const MAX_REJECTED_CANDIDATES: usize = 4096;
    let mut rejected = 0;
    let mut cursor = lower_bound;
    while cursor < last_marker.saturating_sub(4) {
        while payload_index < payload_ranges.len() && payload_ranges[payload_index].end <= cursor {
            payload_index += 1;
        }
        if let Some(range) = payload_ranges.get(payload_index)
            && range.contains(&cursor)
        {
            cursor = range.end;
            continue;
        }
        let offset = cursor;
        cursor += 1;
        let length = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let Some(marker) = offset.checked_add(length).filter(|end| *end <= last_marker) else {
            continue;
        };
        if length < 9
            || bytes.get(marker..marker + 4) != Some(&SENTINEL)
            || bytes.get(offset + 4) != Some(&1)
        {
            continue;
        }
        match read_chunk(&bytes[offset + 4..marker], product) {
            Ok(events) => {
                if product == Product::D2 {
                    match d2_embedded_sources(bytes, offset) {
                        Ok(sources) => {
                            payload_ranges.extend(sources.into_iter().map(|source| source.range))
                        }
                        Err(error) => {
                            last_error = Some(error);
                            continue;
                        }
                    }
                }
                if result.is_some() {
                    return Err("The native song has ambiguous timeline data.".into());
                }
                result = Some(Timeline {
                    start: offset,
                    events,
                });
            }
            Err(error) => {
                last_error = Some(error);
                rejected += 1;
                if rejected > MAX_REJECTED_CANDIDATES {
                    return Err("The native song has too many invalid timeline records.".into());
                }
            }
        }
    }
    result.ok_or_else(|| {
        last_error.unwrap_or_else(|| "The native counted timeline profile is not supported.".into())
    })
}

fn read_chunk(bytes: &[u8], product: Product) -> Result<Vec<Event>, String> {
    let core_tracks = profile(product)?.1;
    let maximum_lane = core_tracks
        + usize::from(matches!(
            product,
            Product::D2 | Product::D3 | Product::D4 | Product::H1
        ));
    let mut reader = Reader { bytes, offset: 0 };
    let mut events = Vec::new();
    let mut previous_lane = 0;
    while reader.offset < bytes.len() {
        if reader.offset + 1 == bytes.len() && bytes[reader.offset] == 1 {
            reader.offset += 1;
            break;
        }
        let lane = usize::from(reader.u8()?);
        let count = reader.u32()? as usize;
        if lane <= previous_lane
            || lane > maximum_lane
            || count > MAX_CLIPS
            || count > bytes.len().saturating_sub(reader.offset) / 7
        {
            return Err("The native timeline has an invalid lane or event count.".into());
        }
        previous_lane = lane;
        if lane > TRACK_COUNT && count > 0 {
            return Err("The native song uses more than 64 tracks.".into());
        }
        for _ in 0..count {
            if events.len() >= MAX_CLIPS {
                return Err("The native song has too many events.".into());
            }
            let class = reader.u8()?;
            let position = reader.i32()?;
            let sample = reader.u16()?;
            if class == 2
                && matches!(product, Product::D2 | Product::D3 | Product::D4)
                && lane == core_tracks + 1
            {
                if position >= 0 {
                    return Err("The native drum pattern uses unsupported trim encoding.".into());
                }
                let (pattern, consumed) =
                    drums::parse_pattern(&reader.bytes[reader.offset..], sample, product)?;
                reader.take(consumed)?;
                events.push(Event {
                    track: lane - 1,
                    position: native_position(position)?,
                    left_trim: 0,
                    right_trim: 0,
                    gain: 1.0,
                    reference: Reference::Drum(pattern),
                });
                continue;
            }
            let mut event = event_tuple(lane - 1, class, position, sample, &mut reader, product)?;
            if class == 1 {
                let mut reference = read_wave_extension(&mut reader, lane, product)?;
                reference
                    .metadata
                    .insert("Saved sample ID".into(), sample.to_string());
                event.reference = Reference::Wave(reference);
            }
            events.push(event);
        }
    }
    if previous_lane == 0 || reader.offset != bytes.len() {
        return Err("The native timeline is incomplete.".into());
    }
    Ok(events)
}

fn event_tuple(
    track: usize,
    class: u8,
    position: i32,
    sample: u16,
    reader: &mut Reader<'_>,
    product: Product,
) -> Result<Event, String> {
    if class == 2 || class == 6 {
        return Err(format!(
            "{} track {} contains an unsupported generated pattern (source {}).",
            product.label(),
            track + 1,
            class
        ));
    }
    if class != 0
        && class != 3
        && !(class == 1
            && matches!(
                product,
                Product::D2 | Product::D3 | Product::D4 | Product::T3 | Product::H1
            ))
    {
        return Err(format!(
            "{} track {} uses unsupported source {}.",
            product.label(),
            track + 1,
            class
        ));
    }
    let native_position = native_position(position)?;
    let (left_trim, right_trim, gain) = if position >= 0 {
        let left = reader.u32()?;
        let right = reader.u32()?;
        let gain = (32767.0 + f32::from(reader.i16()?)) / 32767.0;
        if !(0.0..=2.0).contains(&gain) {
            return Err("The native clip has invalid volume data.".into());
        }
        (native_tick_count(left)?, native_tick_count(right)?, gain)
    } else {
        (0, 0, 1.0)
    };
    Ok(Event {
        track,
        position: native_position,
        left_trim,
        right_trim,
        gain,
        reference: Reference::Native(sample),
    })
}

// Native RWaveParam faders use signed input and truncate to signed Q16 gain.
fn native_fader_gain(stored: u16, scale: f64) -> f32 {
    let value = f64::from(stored as i16) / 32767.0 * scale;
    let value = if value > 1.0 {
        value * value * value
    } else {
        value
    };
    ((value * 65536.0).trunc() / 65536.0) as f32
}

fn native_position(position: i32) -> Result<u32, String> {
    let position = if position < 0 {
        -i64::from(position) - 1
    } else {
        i64::from(position)
    };
    native_tick_count(
        u32::try_from(position).map_err(|_| "The native position exceeds the timeline.")?,
    )
}

fn native_tick_count(value: u32) -> Result<u32, String> {
    // All admitted counted profiles have native duration writers with 128 units per bar.
    value
        .checked_mul(2)
        .filter(|value| *value <= MAX_TICKS)
        .ok_or_else(|| "The native position or trim exceeds the timeline.".into())
}

fn read_wave_extension(
    reader: &mut Reader<'_>,
    lane: usize,
    product: Product,
) -> Result<WaveReference, String> {
    let mut metadata = BTreeMap::new();
    let raw = reader.block()?;
    if !(12..=260).contains(&raw.len()) {
        return Err("The native WAV path has an unsupported length.".into());
    }
    let path = first_text(raw);
    if product == Product::D2 && !raw.ends_with(&[0, 1]) {
        return Err("The D2 imported-audio path is not framed correctly.".into());
    }
    let packed = product == Product::D2 && path.to_ascii_lowercase().ends_with(".pxd");
    if path.as_bytes().get(1) != Some(&b':')
        || !(packed || path.to_ascii_lowercase().ends_with(".wav"))
    {
        return Err("The native audio reference is invalid.".into());
    }
    if matches!(product, Product::D2 | Product::D3) && raw.ends_with(&[0, 1]) {
        // D2 and D3 save two labels followed by eight bytes of source metadata.
        // A metadata value can look like a text length, including an empty label.
        for index in 0..2 {
            let label = reader.block()?;
            if !(2..=262).contains(&label.len()) || !label.ends_with(&[0, 1]) {
                return Err("The native imported-audio label is invalid.".into());
            }
            metadata.insert(format!("Native label {}", index + 1), first_text(label));
        }
        metadata.insert("Native metadata word 1".into(), reader.u16()?.to_string());
        // The original extent calculation multiplies this stored bar count by
        // 128 native units. PCM can end before the nominal tile does.
        let bars = reader.u32()?;
        metadata.insert("Declared bars".into(), bars.to_string());
        let declared_ticks = bars
            .checked_mul(TICKS_PER_BAR)
            .filter(|ticks| (1..=MAX_TICKS).contains(ticks))
            .ok_or("The native imported-audio span exceeds the supported timeline.")?;
        metadata.insert("Native metadata word 2".into(), reader.u16()?.to_string());
        return Ok(WaveReference {
            path,
            declared_ticks: Some(declared_ticks),
            metadata,
        });
    }
    // Known path extensions can carry up to three additional text tokens.
    for index in 0..3 {
        let saved = reader.offset;
        if saved + 2 > reader.bytes.len() {
            break;
        }
        let length =
            u16::from_le_bytes(reader.bytes[saved..saved + 2].try_into().unwrap()) as usize;
        if !(4..=260).contains(&length) || saved + 2 + length > reader.bytes.len() {
            break;
        }
        let token = &reader.bytes[saved + 2..saved + 2 + length];
        let text = token.split(|byte| *byte == 0).next().unwrap_or_default();
        if text.is_empty() || text.iter().any(|byte| !matches!(*byte, 9 | 32..=126)) {
            break;
        }
        metadata.insert(format!("Native label {}", index + 1), first_text(token));
        reader.offset += 2 + length;
    }
    let tail = &reader.bytes[reader.offset..];
    if tail.starts_with(&[2, 0, 0, 1]) {
        metadata.insert(
            "Native extension bytes".into(),
            format!("{:02x?}", reader.take(12)?),
        );
    } else if tail.len() >= 8 && matches!(u16::from_le_bytes(tail[..2].try_into().unwrap()), 6 | 12)
    {
        metadata.insert(
            "Native extension bytes".into(),
            format!("{:02x?}", reader.take(8)?),
        );
    } else if tail.len() >= 6 {
        let kind = u16::from_le_bytes(tail[..2].try_into().unwrap());
        let count = u32::from_le_bytes(tail[2..6].try_into().unwrap());
        if (1..=32).contains(&kind) && (1..=10_000).contains(&count) {
            return Err(format!(
                "{} track {} contains an unsupported WAV continuation list.",
                product.label(),
                lane
            ));
        }
    }
    Ok(WaveReference {
        path,
        declared_ticks: None,
        metadata,
    })
}

struct Wave<'a> {
    source_path: String,
    bytes: Cow<'a, [u8]>,
    frames: u32,
    sample_rate: u32,
}

#[derive(Clone, Debug)]
struct RetainedWave {
    path: PathBuf,
    frames: u32,
    sample_rate: u32,
}

fn retain_wave<'a>(
    wave: Wave<'a>,
    pending: &mut Vec<(PathBuf, Cow<'a, [u8]>)>,
    total_bytes: &mut usize,
) -> Result<RetainedWave, String> {
    let hash = format!("{:x}", Sha256::digest(&wave.bytes));
    let path = crate::project_io::data_root()
        .join("imported-audio")
        .join(format!("{hash}.wav"));
    if !pending.iter().any(|(existing, _)| *existing == path) {
        queue_asset(pending, total_bytes, path.clone(), wave.bytes)?;
    }
    Ok(RetainedWave {
        path,
        frames: wave.frames,
        sample_rate: wave.sample_rate,
    })
}

fn wave_info(bytes: &[u8]) -> Result<(u32, u32), String> {
    let mut reader = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|error| format!("The imported WAV is invalid: {error}"))?;
    let spec = reader.spec();
    if !matches!(spec.channels, 1 | 2)
        || spec.sample_rate == 0
        || spec.sample_rate > 384000
        || reader.len() > 64_000_000
        || !reader.len().is_multiple_of(u32::from(spec.channels))
        || !matches!(
            (spec.sample_format, spec.bits_per_sample),
            (hound::SampleFormat::Int, 1..=32) | (hound::SampleFormat::Float, 32)
        )
    {
        return Err("The imported WAV format is not supported.".into());
    }
    let frames = reader.duration();
    if frames == 0 {
        return Err("The imported WAV has no audio frames.".into());
    }
    match spec.sample_format {
        hound::SampleFormat::Int => {
            for sample in reader.samples::<i32>() {
                sample.map_err(|error| error.to_string())?;
            }
        }
        hound::SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                if !sample.map_err(|error| error.to_string())?.is_finite() {
                    return Err("The imported WAV has nonfinite samples.".into());
                }
            }
        }
    }
    Ok((frames, spec.sample_rate))
}

fn embedded_wave_records(bytes: &[u8]) -> Result<Vec<EmbeddedSource<'_>>, String> {
    let mut sources = Vec::new();
    // A record's audio payload cannot hold further records. Without this
    // bound, nested headers would each be decoded as a complete WAV.
    let mut payload_end = 0;
    for riff in 0..bytes.len().saturating_sub(12) {
        if riff < payload_end
            || &bytes[riff..riff + 4] != b"RIFF"
            || &bytes[riff + 8..riff + 12] != b"WAVE"
        {
            continue;
        }
        let size = u32::from_le_bytes(bytes[riff + 4..riff + 8].try_into().unwrap()) as usize + 8;
        if size < 44 || riff.checked_add(size).is_none_or(|end| end > bytes.len()) {
            continue;
        }
        // Every record ends with its marker and the WAV size, just before the
        // WAV. Check those fixed bytes once before searching for the path.
        if riff < 6
            || bytes[riff - 6..riff - 4] != [0, 1]
            || u32::from_le_bytes(bytes[riff - 4..riff].try_into().unwrap()) as usize != size
        {
            continue;
        }
        let mut source_path = None;
        for start in riff.saturating_sub(270)..riff.saturating_sub(6) {
            let length = u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap()) as usize;
            if !(4..=262).contains(&length) {
                continue;
            }
            for includes_marker in [true, false] {
                let marker = start + 2 + length - if includes_marker { 2 } else { 0 };
                if marker + 6 != riff || bytes.get(marker..marker + 2) != Some(&[0, 1]) {
                    continue;
                }
                if u32::from_le_bytes(bytes[marker + 2..marker + 6].try_into().unwrap()) as usize
                    != size
                {
                    continue;
                }
                let path = first_text(&bytes[start + 2..marker]);
                if path.as_bytes().get(1) == Some(&b':')
                    && path.to_ascii_lowercase().ends_with(".wav")
                {
                    source_path = Some(path);
                    break;
                }
            }
            if source_path.is_some() {
                break;
            }
        }
        if let Some(source_path) = source_path {
            let wav = &bytes[riff..riff + size];
            if sources.len() >= MAX_EMBEDDED_WAVES {
                return Err("The native song has too many embedded WAV records.".into());
            }
            sources.push(EmbeddedSource {
                path: source_path,
                bytes: wav,
                range: riff..riff + size,
            });
            payload_end = riff + size;
        }
    }
    Ok(sources)
}

fn embedded_waves(bytes: &[u8]) -> Result<Vec<Wave<'_>>, String> {
    embedded_wave_records(bytes)?
        .into_iter()
        .map(|source| {
            let (frames, sample_rate) = wave_info(source.bytes)?;
            Ok(Wave {
                source_path: source.path,
                bytes: Cow::Borrowed(source.bytes),
                frames,
                sample_rate,
            })
        })
        .collect()
}

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn source_wave<'a>(
    path: &str,
    embedded: &'a [Wave<'_>],
    library: &Library,
) -> Result<Wave<'a>, String> {
    let normalized = normalize_saved_path(path);
    let matches: Vec<_> = embedded
        .iter()
        .filter(|wave| normalize_saved_path(&wave.source_path) == normalized)
        .collect();
    if let Some(first) = matches.first() {
        if matches.iter().any(|other| other.bytes != first.bytes) {
            return Err("The native WAV reference matches different embedded audio files.".into());
        }
        return Ok(Wave {
            source_path: path.into(),
            bytes: Cow::Borrowed(&first.bytes),
            frames: first.frames,
            sample_rate: first.sample_rate,
        });
    }
    let file = if Path::new(path).is_file() {
        PathBuf::from(path)
    } else {
        library.resolve_wav_reference(path)?
    };
    if !file.is_file() {
        return Err(format!(
            "The native song needs WAV audio that is missing: {}.",
            basename(path)
        ));
    }
    let bytes = read_external_wave(&file)?;
    let (frames, sample_rate) = wave_info(&bytes)?;
    Ok(Wave {
        source_path: path.into(),
        bytes: Cow::Owned(bytes),
        frames,
        sample_rate,
    })
}

fn normalize_saved_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

struct EmbeddedSource<'a> {
    path: String,
    bytes: &'a [u8],
    range: std::ops::Range<usize>,
}

fn d2_embedded_sources(
    bytes: &[u8],
    timeline_start: usize,
) -> Result<Vec<EmbeddedSource<'_>>, String> {
    let mut timeline = Reader {
        bytes,
        offset: timeline_start,
    };
    let length = timeline.u32()? as usize;
    let marker = timeline_start
        .checked_add(length)
        .ok_or("The D2 timeline length is invalid.")?;
    let mut reader = Reader {
        bytes,
        offset: marker,
    };
    if reader.take(4)? != SENTINEL {
        return Err("The D2 timeline end marker is invalid.".into());
    }
    if reader.offset == bytes.len() {
        return Ok(Vec::new());
    }
    let count = reader.u32()? as usize;
    if count > MAX_EMBEDDED_WAVES || count > bytes.len().saturating_sub(reader.offset) / 12 {
        return Err("The D2 embedded-audio count is invalid.".into());
    }
    let mut sources = Vec::new();
    for _ in 0..count {
        let raw = reader.block()?;
        if !(4..=262).contains(&raw.len()) || !raw.ends_with(&[0, 1]) {
            return Err("The D2 embedded-audio path is invalid.".into());
        }
        let path = first_text(raw);
        let suffix = path.to_ascii_lowercase();
        if path.as_bytes().get(1) != Some(&b':')
            || !(suffix.ends_with(".wav") || suffix.ends_with(".pxd"))
        {
            return Err("The D2 embedded-audio path is invalid.".into());
        }
        let length = reader.u32()? as usize;
        if length == 0 {
            return Err("The D2 embedded audio is empty.".into());
        }
        let start = reader.offset;
        sources.push(EmbeddedSource {
            path,
            bytes: reader.take(length)?,
            range: start..reader.offset,
        });
    }
    if reader.offset != bytes.len() {
        return Err("The D2 embedded-audio footer has unexpected trailing data.".into());
    }
    Ok(sources)
}

fn packed_wave(path: &str, bytes: &[u8]) -> Result<Wave<'static>, String> {
    if bytes.len() > 64 * 1024 * 1024 || !bytes.starts_with(b"tPxD") {
        return Err("The imported PXD header or file size is invalid.".into());
    }
    let decoded = legacy_format::source::SampleSource::from_packed_file_audio(bytes)
        .map_err(|error| format!("Cannot decode imported PXD {path}: {error}"))?;
    let samples = decoded.samples();
    if samples.is_empty() || samples.len() > 64_000_000 {
        return Err("The imported PXD exceeds the supported decoded audio size.".into());
    }
    // Retain the source's exact PCM16 values. This also keeps the largest
    // supported decoded PXD within the WAV provider's 128 MiB file limit.
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 1,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|error| error.to_string())?;
        for &sample in samples {
            writer
                .write_sample(sample)
                .map_err(|error| error.to_string())?;
        }
        writer.finalize().map_err(|error| error.to_string())?;
    }
    Ok(Wave {
        source_path: path.into(),
        frames: samples.len() as u32,
        sample_rate: 44100,
        bytes: Cow::Owned(cursor.into_inner()),
    })
}

fn d2_source_wave<'a>(
    path: &str,
    embedded: &'a [EmbeddedSource<'_>],
    library: &Library,
) -> Result<Wave<'a>, String> {
    let normalized = normalize_saved_path(path);
    let matches: Vec<_> = embedded
        .iter()
        .filter(|source| normalize_saved_path(&source.path) == normalized)
        .collect();
    if let Some(first) = matches.first() {
        if matches.iter().any(|other| other.bytes != first.bytes) {
            return Err("The native audio reference matches different embedded files.".into());
        }
        return if normalized.ends_with(".pxd") {
            packed_wave(path, first.bytes)
        } else {
            let (frames, sample_rate) = wave_info(first.bytes)?;
            Ok(Wave {
                source_path: path.into(),
                bytes: Cow::Borrowed(first.bytes),
                frames,
                sample_rate,
            })
        };
    }
    if !normalized.ends_with(".pxd") {
        return source_wave(path, &[], library);
    }
    if Path::new(path).is_file() {
        return packed_wave(path, &read_external_wave(Path::new(path))?);
    }
    // Older standalone PXD references retain the catalog's relative folder/code.
    // Resolve that identity from selected providers; never use an arbitrary basename.
    let matches: Vec<_> = library
        .samples
        .iter()
        .filter(|sample| {
            if !matches!(Product::sample_key(sample.id), Some((Product::D1, _))) {
                return false;
            }
            let code = normalize_saved_path(&sample.code);
            normalized == code || normalized.ends_with(&format!("/{code}"))
        })
        .collect();
    if matches.len() != 1 {
        return Err(format!(
            "Imported PXD {path} is missing or has no unique catalog identity."
        ));
    }
    let sample = matches[0];
    let pcm = library.read_pcm(sample.id)?;
    Ok(Wave {
        source_path: path.into(),
        frames: u32::try_from(pcm.frames()).map_err(|_| "The imported PXD is too long.")?,
        sample_rate: pcm.sample_rate,
        bytes: Cow::Owned(encode_float_wave(&pcm)?),
    })
}

fn conflicting_placement_metadata(
    conflicts: &BTreeMap<(u32, u32, u32), usize>,
) -> BTreeMap<String, String> {
    // Clip positions remain in the arrangement. Group repeated trim tuples and
    // split large tables to stay within the per-value metadata limit.
    let header = "Saved span, left trim, right trim (ticks), occurrence count:\n";
    let mut result = BTreeMap::new();
    let mut value = header.to_string();
    let mut part = 1;
    for ((span, left, right), count) in conflicts {
        let row = format!("{span}, {left}, {right}, {count}\n");
        if value.len() + row.len() > 60_000 {
            result.insert(format!("Saved conflicting placements {part}"), value);
            part += 1;
            value = header.to_string();
        }
        value.push_str(&row);
    }
    let key = if part == 1 {
        "Saved conflicting placements".into()
    } else {
        format!("Saved conflicting placements {part}")
    };
    result.insert(key, value);
    result
}

pub(crate) fn build_project(
    path: &Path,
    bytes: &[u8],
    product: Product,
    library: &Library,
    header: Header,
    timeline: Timeline,
) -> Result<Project, String> {
    let events = timeline.events;
    let mut positions: HashMap<usize, Vec<u32>> = HashMap::new();
    for event in &events {
        positions
            .entry(event.track)
            .or_default()
            .push(event.position);
    }
    for starts in positions.values_mut() {
        starts.sort_unstable();
        starts.dedup();
    }
    let source_key = |reference: &Reference| match reference {
        Reference::Native(id) => Some(format!("native:{id}")),
        Reference::Wave(source) => Some(format!("external:{}", normalize_saved_path(&source.path))),
        Reference::Drum(_) => None,
    };
    let mut inferred_sources: HashMap<String, u32> = HashMap::new();
    let mut inconsistent_spans: HashMap<WaveReference, u32> = HashMap::new();
    let mut inconsistent_details: HashMap<WaveReference, BTreeMap<(u32, u32, u32), usize>> =
        HashMap::new();
    for event in &events {
        let starts = &positions[&event.track];
        let next = starts.partition_point(|start| *start <= event.position);
        let guess = starts
            .get(next)
            .map_or(TICKS_PER_BAR, |start| start - event.position)
            .max(TICKS_PER_BAR);
        let span = guess
            .saturating_add(event.left_trim)
            .saturating_add(event.right_trim)
            .min(MAX_TICKS);
        if let Some(key) = source_key(&event.reference) {
            inferred_sources
                .entry(key)
                .and_modify(|value| *value = (*value).max(span))
                .or_insert(span);
        }
        if let Reference::Wave(source) = &event.reference
            && let Some(declared) = source.declared_ticks
        {
            let trims = event.left_trim.saturating_add(event.right_trim);
            if trims >= declared {
                let minimum = trims
                    .saturating_add(1)
                    .div_ceil(TICKS_PER_BAR)
                    .saturating_mul(TICKS_PER_BAR)
                    .min(MAX_TICKS);
                let mut key = source.clone();
                key.path = normalize_saved_path(&key.path);
                *inconsistent_details
                    .entry(key.clone())
                    .or_default()
                    .entry((declared, event.left_trim, event.right_trim))
                    .or_default() += 1;
                inconsistent_spans
                    .entry(key)
                    .and_modify(|span| *span = (*span).max(minimum))
                    .or_insert(minimum);
            }
        }
    }
    let kit = if events
        .iter()
        .any(|event| matches!(event.reference, Reference::Drum(_)))
    {
        Some(drums::parse_kit(
            bytes,
            header.after_mixer,
            timeline.start,
            product,
        )?)
    } else {
        None
    };
    let has_imports = events
        .iter()
        .any(|event| matches!(event.reference, Reference::Wave(_)));
    let d2_embedded = if has_imports && product == Product::D2 {
        d2_embedded_sources(bytes, timeline.start)?
    } else {
        Vec::new()
    };
    let embedded = if has_imports && product != Product::D2 {
        embedded_waves(bytes)?
    } else {
        Vec::new()
    };
    let mut source_notices = header.source_notices;
    source_notices.extend(settings::notices(product, &header.settings, kit.is_some()));
    source_notices.extend(kit.as_ref().and_then(drums::effects_notice));
    let mut project = Project {
        title: if header.title.is_empty() {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into()
        } else {
            header.title
        },
        author: header.author,
        bpm: header.bpm,
        pitch_ratio: header.effective_bpm / header.bpm,
        master_gain: 1.0,
        tracks: header.tracks,
        source_notices,
        source_settings: header.settings.entries,
        libraries: library.bindings().to_vec(),
        ..Project::default()
    };
    let mut pending_waves = Vec::new();
    let mut pending_bytes = 0;
    let mut drum_cache: HashMap<drums::Pattern, (u32, u32)> = HashMap::new();
    // Each generated pattern records the song's hash. Hash the song once.
    let mut mix_hash: Option<String> = None;
    let mut wave_cache = HashMap::new();
    // Events can name one path with different labels. Each path is resolved,
    // decoded, and hashed once; later references reuse that validated source.
    // The cache holds metadata only. Owned audio belongs to the bounded,
    // content-deduplicated pending assets, including when many paths match it.
    let mut resolved_waves: HashMap<String, Result<RetainedWave, String>> = HashMap::new();
    let mut catalogs: HashMap<Product, Vec<crate::catalog::WaveMetadata>> = HashMap::new();
    // D3 keeps the earlier Dance 2 catalog IDs. T2's original loader admits
    // the D2 kind-2 catalog: the contiguous IDs 1..=1957. Do not search other
    // products by ID; identical numbers can identify unrelated sounds.
    for event in events {
        let inferred_ticks = source_key(&event.reference)
            .and_then(|key| inferred_sources.get(&key).copied())
            .unwrap_or(TICKS_PER_BAR);
        let (sample_id, frames, rate, source_bpm, declared_ticks) = match event.reference {
            Reference::Native(native_id) => {
                let native_id = u32::from(native_id);
                let current_id = product.global_sample_id(native_id)?;
                let inherited = matches!(
                    (product, native_id),
                    (Product::D3, 1..=12999) | (Product::T2, 1..=1957)
                );
                let source_product = if library.sample_by_id(current_id).is_none() && inherited {
                    Product::D2
                } else {
                    product
                };
                let id = source_product.global_sample_id(native_id)?;
                if let Some(sample) = library.sample_by_id(id) {
                    (
                        id,
                        sample.frame_count,
                        sample.sample_rate,
                        sample.source_bpm,
                        Some(
                            sample
                                .bars
                                .checked_mul(TICKS_PER_BAR)
                                .ok_or("The native sample span exceeds the timeline.")?,
                        ),
                    )
                } else {
                    if let std::collections::hash_map::Entry::Vacant(entry) =
                        catalogs.entry(source_product)
                    {
                        entry.insert(crate::catalog::wave_metadata(source_product)?);
                    }
                    let metadata = catalogs[&source_product]
                        .iter()
                        .find(|entry| entry.native_id == native_id);
                    let ticks = metadata.map_or(inferred_ticks, |entry| {
                        entry.bars.saturating_mul(TICKS_PER_BAR)
                    });
                    let bpm = metadata.map_or(header.bpm, |entry| entry.source_bpm);
                    if project.missing_sample(id).is_none() {
                        let mut missing = MissingSample::native(
                            source_product,
                            native_id,
                            metadata.map_or_else(
                                || format!("{} sample {native_id}", source_product.label()),
                                |entry| entry.name.clone(),
                            ),
                            bpm,
                            ticks,
                            metadata.is_none(),
                            library.describe_missing(source_product, native_id),
                        )?;
                        if let Some(entry) = metadata {
                            missing.metadata.insert("Code".into(), entry.code.clone());
                            missing
                                .metadata
                                .insert("Group".into(), entry.group_name.clone());
                            missing
                                .metadata
                                .insert("Catalog frames".into(), entry.frame_count.to_string());
                        } else {
                            missing.metadata.insert(
                                "Length inference".into(),
                                "Next placement on the track, with a one-bar minimum.".into(),
                            );
                        }
                        project.missing_samples.push(missing);
                    }
                    (
                        id,
                        metadata.map_or(0, |entry| entry.frame_count),
                        44100,
                        bpm,
                        Some(ticks),
                    )
                }
            }
            Reference::Wave(WaveReference {
                path: source_path,
                declared_ticks,
                metadata,
            }) => {
                let key = WaveReference {
                    path: normalize_saved_path(&source_path),
                    declared_ticks,
                    metadata: metadata.clone(),
                };
                let (id, frames, sample_rate) = if let Some(cached) = wave_cache.get(&key) {
                    *cached
                } else {
                    let wave = match resolved_waves.get(&key.path) {
                        Some(resolved) => resolved.clone(),
                        None => {
                            let wave = if product == Product::D2 {
                                d2_source_wave(&source_path, &d2_embedded, library)
                            } else {
                                source_wave(&source_path, &embedded, library)
                            };
                            let resolved = match wave {
                                Ok(wave) => {
                                    Ok(retain_wave(wave, &mut pending_waves, &mut pending_bytes)?)
                                }
                                Err(error) => Err(error),
                            };
                            resolved_waves.insert(key.path.clone(), resolved.clone());
                            resolved
                        }
                    };
                    let has_embedded = d2_embedded
                        .iter()
                        .any(|source| normalize_saved_path(&source.path) == key.path)
                        || embedded
                            .iter()
                            .any(|source| normalize_saved_path(&source.source_path) == key.path);
                    let candidates = if !has_embedded
                        && !source_path.to_lowercase().ends_with(".pxd")
                        && !Path::new(&source_path).is_file()
                    {
                        library
                            .find_wav_candidates(&source_path)
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    let wave = match wave {
                        Ok(wave) => Some(wave),
                        Err(error) if has_embedded => return Err(error),
                        Err(reason) => {
                            let id = next_import_id(&project)?;
                            let named = [
                                metadata.get("Native label 1"),
                                metadata.get("Native label 2"),
                            ]
                            .into_iter()
                            .flatten()
                            .map(String::as_str)
                            .filter(|text| !text.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ");
                            let imported = ImportedSample {
                                id,
                                name: if named.is_empty() {
                                    basename(&source_path).into()
                                } else {
                                    named
                                },
                                group_name: "Imported".into(),
                                path: PathBuf::from(&source_path),
                                source_bpm: header.bpm,
                                source_sample_rate: None,
                                origin: Some(product),
                            };
                            let mut missing = MissingSample::external(
                                &imported,
                                inconsistent_spans
                                    .get(&key)
                                    .copied()
                                    .or(declared_ticks)
                                    .unwrap_or(inferred_ticks),
                                reason,
                            );
                            missing.length_inferred =
                                declared_ticks.is_none() || inconsistent_spans.contains_key(&key);
                            if inconsistent_spans.contains_key(&key) {
                                missing.metadata.insert("Length confidence".into(), "Saved trims exceed a saved source span. The minimum compatible whole-bar source length is inferred; valid saved placement spans are retained.".into());
                                missing.metadata.extend(conflicting_placement_metadata(
                                    &inconsistent_details[&key],
                                ));
                            }
                            missing.metadata.insert(
                                "Sample rate basis".into(),
                                "Temporary trim reference; original file rate is unavailable."
                                    .into(),
                            );
                            missing.metadata.extend(metadata.clone());
                            project.missing_samples.push(missing);
                            wave_cache.insert(key.clone(), (id, 0, 44100));
                            None
                        }
                    };
                    if let Some(wave) = wave {
                        let destination = wave.path;
                        let id = if let Some(existing) =
                            project.imported_samples.iter().find(|sample| {
                                sample.path == destination
                                    && candidates.len() < 2
                                    && !project
                                        .sample_conflicts
                                        .iter()
                                        .any(|conflict| conflict.sample_ids.contains(&sample.id))
                            }) {
                            existing.id
                        } else {
                            let id = next_import_id(&project)?;
                            project.imported_samples.push(ImportedSample {
                                id,
                                name: basename(&source_path).into(),
                                group_name: "Imported".into(),
                                path: destination.clone(),
                                source_bpm: header.bpm,
                                source_sample_rate: Some(wave.sample_rate),
                                origin: Some(product),
                            });
                            id
                        };
                        crate::library::record_wav_conflict(
                            &mut project,
                            &source_path,
                            id,
                            candidates,
                        );
                        let value = (id, wave.frames, wave.sample_rate);
                        wave_cache.insert(key, value);
                        value
                    } else {
                        wave_cache[&key]
                    }
                };
                let span = match project.missing_sample(id) {
                    Some(missing)
                        if declared_ticks.is_none_or(|ticks| {
                            event.left_trim.saturating_add(event.right_trim) >= ticks
                        }) =>
                    {
                        Some(missing.nominal_ticks)
                    }
                    _ => declared_ticks,
                };
                (id, frames, sample_rate, header.bpm, span)
            }
            Reference::Drum(pattern) => {
                let (id, frames) = if let Some(cached) = drum_cache.get(&pattern) {
                    *cached
                } else {
                    let source_kit = kit.as_ref().ok_or("The native drum kit is missing.")?;
                    let pcm =
                        drums::prepare_kit(source_kit, library, product, std::iter::once(&pattern))
                            .and_then(|prepared| drums::render(&prepared, &pattern, header.bpm));
                    if let Err(reason) = pcm {
                        let id = next_import_id(&project)?;
                        let label = format!("{} {}", pattern.group, pattern.variant)
                            .trim()
                            .to_string();
                        project.missing_samples.push(MissingSample {
                            id,
                            name: if label.is_empty() {
                                format!("Drum pattern {}", pattern.native_id)
                            } else {
                                label
                            },
                            source_bpm: header.bpm,
                            sample_rate: 44100,
                            nominal_ticks: TICKS_PER_BAR,
                            length_inferred: false,
                            reference: MissingReference::Generated {
                                product,
                                kind: "native-drum".into(),
                                data: serde_json::json!({"kit": source_kit, "pattern": pattern}),
                            },
                            reason,
                            metadata: BTreeMap::from([(
                                "Native pattern ID".into(),
                                pattern.native_id.to_string(),
                            )]),
                        });
                        drum_cache.insert(pattern, (id, 0));
                        (id, 0)
                    } else {
                        let pcm = pcm?;
                        let wave = encode_float_wave(&pcm)?;
                        let hash = format!("{:x}", Sha256::digest(&wave));
                        let destination = crate::project_io::data_root()
                            .join("imported-audio")
                            .join(format!("{hash}.wav"));
                        let id = if let Some(existing) = project
                            .imported_samples
                            .iter()
                            .find(|sample| sample.path == destination)
                        {
                            existing.id
                        } else {
                            let id = next_import_id(&project)?;
                            let name = format!("{} {}", pattern.group, pattern.variant)
                                .trim()
                                .to_string();
                            let name = if name.is_empty() {
                                format!("Drum pattern {}", pattern.native_id)
                            } else {
                                name
                            };
                            project.imported_samples.push(ImportedSample {
                                id,
                                name,
                                group_name: "Groove".into(),
                                path: destination.clone(),
                                source_bpm: header.bpm,
                                source_sample_rate: Some(44100),
                                origin: Some(product),
                            });
                            queue_asset(
                                &mut pending_waves,
                                &mut pending_bytes,
                                destination.clone(),
                                wave,
                            )?;
                            id
                        };
                        let provenance = serde_json::json!({
                            "schema_version": 1, "product": product,
                            "source_mix_name": path.file_name().unwrap_or_default().to_string_lossy(),
                            "source_mix_sha256": mix_hash
                                .get_or_insert_with(|| format!("{:x}", Sha256::digest(bytes))),
                            "source_bpm": header.bpm, "native_units_per_bar": 128,
                            "pattern": &pattern, "kit": &kit,
                            "rendered_audio_sha256": hash, "renderer_policy": drums::renderer_policy(product),
                        });
                        let provenance = serde_json::to_vec_pretty(&provenance)
                            .map_err(|error| error.to_string())?;
                        let provenance_hash = format!("{:x}", Sha256::digest(&provenance));
                        queue_asset(
                            &mut pending_waves,
                            &mut pending_bytes,
                            destination.with_extension(format!("{provenance_hash}.json")),
                            provenance,
                        )?;
                        let result = (
                            id,
                            u32::try_from(pcm.frames())
                                .map_err(|_| "The generated drum pattern is too long.")?,
                        );
                        drum_cache.insert(pattern, result);
                        result
                    }
                };
                (id, frames, 44100, header.bpm, Some(TICKS_PER_BAR))
            }
        };
        let measured_ticks = (f64::from(frames) * source_bpm * f64::from(TICKS_PER_BAR)
            / (f64::from(rate) * 240.0))
            .round();
        // Native catalogs keep a whole-bar tile span even when PCM ends sooner.
        let full_ticks = declared_ticks.map_or(measured_ticks, f64::from);
        if !full_ticks.is_finite() || !(1.0..=f64::from(MAX_TICKS)).contains(&full_ticks) {
            return Err("The native sample duration exceeds the timeline.".into());
        }
        let length_ticks = (full_ticks as u32)
            .checked_sub(event.left_trim)
            .and_then(|length| length.checked_sub(event.right_trim))
            .filter(|length| *length > 0)
            .ok_or("The native clip has invalid trim data.")?;
        let offset_frames = (f64::from(event.left_trim) * f64::from(rate) * 240.0
            / (source_bpm * f64::from(TICKS_PER_BAR)))
        .round();
        if !offset_frames.is_finite()
            || offset_frames > f64::from(u32::MAX)
            || (declared_ticks.is_none() && offset_frames > f64::from(frames))
        {
            return Err("The native clip trim exceeds its source audio.".into());
        }
        project.clips.push(Clip {
            id: project.clips.len() as u64 + 1,
            sample_id,
            track: event.track,
            start_tick: event.position,
            length_ticks,
            offset_frames: offset_frames as u32,
            gain: event.gain,
        });
    }
    let project = normalize_import(project)?;
    for (path, bytes) in pending_waves {
        preserve_wave(&path, &bytes)?;
    }
    Ok(project)
}

fn next_import_id(project: &Project) -> Result<u32, String> {
    IMPORTED_ID_START
        .checked_add(
            u32::try_from(project.imported_samples.len() + project.missing_samples.len())
                .map_err(|_| "The imported sample limit was reached.")?,
        )
        .ok_or_else(|| "The imported sample limit was reached.".into())
}

pub(crate) fn restore_missing(
    sample: &MissingSample,
    library: &Library,
) -> Result<Option<ImportedSample>, String> {
    let (bytes, product, group) = match &sample.reference {
        MissingReference::External {
            path,
            origin: Some(Product::D2),
        } if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pxd")) =>
        {
            let wave = d2_source_wave(&path.to_string_lossy(), &[], library)?;
            (wave.bytes.into_owned(), Product::D2, "Imported")
        }
        MissingReference::Generated {
            product,
            kind,
            data,
        } if kind == "native-drum" => {
            let kit: drums::Kit = serde_json::from_value(
                data.get("kit")
                    .cloned()
                    .ok_or("The saved drum kit is missing.")?,
            )
            .map_err(|error| format!("Cannot read the saved drum kit: {error}"))?;
            let pattern: drums::Pattern = serde_json::from_value(
                data.get("pattern")
                    .cloned()
                    .ok_or("The saved drum pattern is missing.")?,
            )
            .map_err(|error| format!("Cannot read the saved drum pattern: {error}"))?;
            let prepared = drums::prepare_kit(&kit, library, *product, std::iter::once(&pattern))?;
            (
                encode_float_wave(&drums::render(&prepared, &pattern, sample.source_bpm)?)?,
                *product,
                "Groove",
            )
        }
        _ => return Ok(None),
    };
    let (_, sample_rate) = wave_info(&bytes)?;
    let destination = crate::project_io::data_root()
        .join("imported-audio")
        .join(format!("{:x}.wav", Sha256::digest(&bytes)));
    preserve_wave(&destination, &bytes)?;
    Ok(Some(ImportedSample {
        id: sample.id,
        name: sample.name.clone(),
        group_name: group.into(),
        path: destination,
        source_bpm: sample.source_bpm,
        source_sample_rate: Some(sample_rate),
        origin: Some(product),
    }))
}

fn encode_float_wave(pcm: &crate::audio::Pcm) -> Result<Vec<u8>, String> {
    pcm.validate()?;
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: pcm.channels,
                sample_rate: pcm.sample_rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .map_err(|error| error.to_string())?;
        for sample in &pcm.samples {
            writer
                .write_sample(*sample)
                .map_err(|error| error.to_string())?;
        }
        writer.finalize().map_err(|error| error.to_string())?;
    }
    Ok(cursor.into_inner())
}

fn queue_asset<'a>(
    pending: &mut Vec<(PathBuf, Cow<'a, [u8]>)>,
    total_bytes: &mut usize,
    path: PathBuf,
    bytes: impl Into<Cow<'a, [u8]>>,
) -> Result<(), String> {
    let bytes = bytes.into();
    let total = total_bytes
        .checked_add(bytes.len())
        .filter(|total| *total <= MAX_PENDING_ASSET_BYTES)
        .ok_or("The native song needs more than 500 MB of imported assets.")?;
    pending.push((path, bytes));
    *total_bytes = total;
    Ok(())
}

fn preserve_wave(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.try_exists().map_err(|error| error.to_string())? {
        return verify_existing_asset(path, bytes);
    }
    let temporary = stage_asset(path, bytes)?;
    publish_staged_asset(&temporary, path, bytes)
}

fn stage_asset(path: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    fs::create_dir_all(path.parent().ok_or("The imported audio path is invalid.")?)
        .map_err(|error| error.to_string())?;
    let name = path
        .file_name()
        .ok_or("The imported asset filename is invalid.")?
        .to_string_lossy();
    let sequence = NEXT_ASSET_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_file_name(format!(
        ".{name}.{}.{timestamp}.{sequence}.tmp",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    drop(file);
    Ok(temporary)
}

// Same-directory hard links publish complete files without replacing an existing path.
// A filesystem without hard links (exFAT, FAT32, some network shares) receives an
// exclusive direct write instead; an existing path is never replaced either way.
fn publish_staged_asset(temporary: &Path, path: &Path, bytes: &[u8]) -> Result<(), String> {
    let result = match fs::hard_link(temporary, path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            verify_existing_asset(path, bytes)
        }
        Err(_) => publish_asset_directly(path, bytes),
    };
    let _ = fs::remove_file(temporary);
    result
}

fn publish_asset_directly(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return verify_existing_asset(path, bytes);
        }
        Err(error) => return Err(format!("Cannot publish the imported asset: {error}")),
    };
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        // An incomplete file at the final path would reject every later retry.
        let _ = fs::remove_file(path);
        return Err(format!("Cannot publish the imported asset: {error}"));
    }
    Ok(())
}

fn verify_existing_asset(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let expected = bytes.len() as u64;
    if file.metadata().map_err(|error| error.to_string())?.len() != expected {
        return Err("An imported asset hash path already contains different data.".into());
    }
    let mut actual = Vec::with_capacity(bytes.len());
    file.take(expected + 1)
        .read_to_end(&mut actual)
        .map_err(|error| error.to_string())?;
    if actual == bytes {
        Ok(())
    } else {
        Err("An imported asset hash path already contains different data.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_block(bytes: &mut Vec<u8>, value: &[u8]) {
        bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
        bytes.extend_from_slice(value);
    }

    fn header_fixture(product: Product, bpm: u16, effective: u32) -> Vec<u8> {
        titled_header_fixture(product, bpm, effective, b"Fixture title\0Mixer text")
    }

    fn titled_header_fixture(product: Product, bpm: u16, effective: u32, title: &[u8]) -> Vec<u8> {
        let (app_id, count, width) = profile(product).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&app_id.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&bpm.to_le_bytes());
        bytes.extend_from_slice(&effective.to_le_bytes());
        text_block(&mut bytes, b"Composer\0#SKKENNUNG#:0000000\0\x01");
        text_block(&mut bytes, title);
        let mixer_count =
            count + usize::from(matches!(product, Product::D3 | Product::D4 | Product::T3));
        for _ in 0..mixer_count {
            bytes.extend_from_slice(&(if width == 4 { 0u16 } else { 128u16 }).to_le_bytes());
            bytes.extend_from_slice(&16384u16.to_le_bytes());
            if width == 8 {
                bytes.extend_from_slice(&0u32.to_le_bytes());
            }
        }
        bytes
    }

    fn lane(bytes: &mut Vec<u8>, number: u8, events: &[(u8, i32, u16)]) {
        bytes.push(number);
        bytes.extend_from_slice(&(events.len() as u32).to_le_bytes());
        for (class, position, sample) in events {
            bytes.push(*class);
            bytes.extend_from_slice(&position.to_le_bytes());
            bytes.extend_from_slice(&sample.to_le_bytes());
            if *position >= 0 {
                bytes.extend_from_slice(&[0; 10]);
            }
        }
    }

    #[test]
    fn counted_profiles_import_from_wav_only_native_providers() {
        let root = asset_test_directory("counted-wav");
        let wave_path = root.join("sample.wav");
        fs::write(&wave_path, wave_fixture()).unwrap();
        let path = root.join("song.mix");
        for product in [
            Product::D2,
            Product::D3,
            Product::D4,
            Product::H1,
            Product::T3,
        ] {
            let mut library = Library::empty();
            library.import_wav_folder(&root, 140.0).unwrap();
            library
                .register_wav_product(
                    product,
                    &root,
                    &[crate::catalog::WaveMetadata {
                        native_id: 174,
                        code: "sample".into(),
                        name: "Sample".into(),
                        group_name: "Test".into(),
                        bars: 2,
                        source_bpm: 140.0,
                        frame_count: 4,
                        pcm_digest: None,
                    }],
                )
                .unwrap();
            let mut bytes = header_fixture(product, 140, 140);
            let mut chunk = Vec::new();
            lane(&mut chunk, 1, &[(0, -129, 174)]);
            bytes.extend_from_slice(&((chunk.len() + 4) as u32).to_le_bytes());
            bytes.extend_from_slice(&chunk);
            bytes.extend_from_slice(&SENTINEL);
            fs::write(&path, bytes).unwrap();
            let project = load(&path, product, &library).unwrap();
            assert_eq!(
                project.clips[0].sample_id,
                product.global_sample_id(174).unwrap()
            );
            assert_eq!(project.clips[0].start_tick, 256);
            assert_eq!(project.clips[0].length_ticks, 512);
        }
        fs::remove_file(path).unwrap();
        fs::remove_file(wave_path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn typed_header_preserves_tempo_and_centered_pan() {
        for product in [Product::D2, Product::D3, Product::T3, Product::H1] {
            let bytes = header_fixture(product, 125, 146);
            let header = read_header(&bytes, product).unwrap();
            assert_eq!(header.title, "Fixture title");
            assert_eq!(header.author, "Composer");
            assert_eq!(header.bpm, 125.0);
            assert_eq!(header.effective_bpm, 146.0);
            assert_eq!(header.tracks.len(), TRACK_COUNT);
            assert_eq!(header.tracks[0].pan, 0.0);
            let gain = if product == Product::D2 {
                40961.0 / 65536.0
            } else {
                43582.0 / 65536.0
            };
            assert_eq!(header.tracks[0].gain, gain);
            let project = build_project(
                Path::new("test.mix"),
                &bytes,
                product,
                &Library::empty(),
                header,
                Timeline {
                    start: bytes.len(),
                    events: Vec::new(),
                },
            )
            .unwrap();
            assert_eq!(project.bpm, 140.0);
            assert_eq!(project.pitch_ratio, 1.0);
            assert_eq!(project.import_notices[0].source_bpm, 146.0);
            assert_eq!(project.import_notices[0].target_bpm, 140.0);
        }
        assert_eq!(decode_pan(0).unwrap(), -1.0);
        assert_eq!(decode_pan(128).unwrap(), 0.0);
        assert_eq!(decode_pan(255).unwrap(), 1.0);
        assert!(decode_pan(256).is_err());
    }

    #[test]
    fn native_mute_solo_and_utility_lane_settings_survive_import_and_json() {
        for product in [Product::D3, Product::D4, Product::T3, Product::H1] {
            let mut bytes = header_fixture(product, 140, 140);
            let core_count = profile(product).unwrap().1;
            let count = core_count + usize::from(product != Product::H1);
            let start = bytes.len() - count * 8;
            // A normal mute, a muted solo, and an unmuted solo on the last saved
            // lane exercise precedence and an empty utility lane's solo switch.
            bytes[start + 4..start + 6].copy_from_slice(&1u16.to_le_bytes());
            let muted = read_header(&bytes, product).unwrap();
            assert!(!muted.tracks[0].left_enabled && !muted.tracks[0].right_enabled);
            assert!(muted.tracks[1].left_enabled && muted.tracks[1].right_enabled);
            bytes[start + 6..start + 8].copy_from_slice(&1u16.to_le_bytes());
            let last = start + (count - 1) * 8;
            bytes[last + 2..last + 4].copy_from_slice(&20000u16.to_le_bytes());
            bytes[last + 6..last + 8].copy_from_slice(&1u16.to_le_bytes());
            let header = read_header(&bytes, product).unwrap();
            assert_eq!(header.after_mixer, bytes.len());
            assert!(!header.tracks[0].left_enabled && !header.tracks[0].right_enabled);
            assert!(!header.tracks[1].left_enabled && !header.tracks[1].right_enabled);
            assert!(
                header.tracks[count - 1].left_enabled && header.tracks[count - 1].right_enabled
            );
            assert!(
                header.tracks[count..]
                    .iter()
                    .all(|track| track == &Track::default())
            );
            let expected_gain = native_fader_gain(20000, 1.33);
            assert_eq!(header.tracks[count - 1].gain, expected_gain);
            let project = build_project(
                Path::new("switches.mix"),
                &bytes,
                product,
                &Library::empty(),
                header,
                Timeline {
                    start: bytes.len(),
                    events: Vec::new(),
                },
            )
            .unwrap();
            let reopened: Project =
                serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
            assert_eq!(reopened.tracks, project.tracks);
            assert_eq!(reopened.source_notices, project.source_notices);
            assert!(
                reopened
                    .source_notices
                    .iter()
                    .any(|notice| notice.contains("mute=1, solo=1"))
            );
        }
    }

    #[test]
    fn saved_settings_after_the_title_survive_import_and_json() {
        let title = b"Song\0BoostEQ#\xb0_#0%\xb0_%BoostCompressorLED#\xb0_#1%\xb0_%BoostCompressorDrive#\xb0_#12080%\xb0_%FX_12#\xb0_#1%\xb0_%DrumEcho#\xb0_#1%\xb0_%\0\x01";
        for product in [Product::D3, Product::D4, Product::T3, Product::H1] {
            let bytes = titled_header_fixture(product, 140, 140, title);
            let header = read_header(&bytes, product).unwrap();
            assert_eq!(header.title, "Song");
            assert_eq!(header.settings.entries.len(), 5);
            let project = build_project(
                Path::new("settings.mix"),
                &bytes,
                product,
                &Library::empty(),
                header,
                Timeline {
                    start: bytes.len(),
                    events: Vec::new(),
                },
            )
            .unwrap();
            assert_eq!(project.title, "Song");
            assert_eq!(project.source_settings["BoostCompressorDrive"], "12080");
            assert_eq!(project.source_settings["BoostEQ"], "0");
            // No drum pattern uses the kit, so only the Booster changes the sound.
            let expected = if product == Product::H1 {
                "House saved 5 settings"
            } else {
                "Saved Booster effects are not applied: compressor."
            };
            assert_eq!(project.source_notices.len(), 1);
            assert!(project.source_notices[0].starts_with(expected));
            let reopened: Project =
                serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
            assert_eq!(reopened.source_settings, project.source_settings);
        }
        // A song without the list has no settings and no notice.
        let bytes = header_fixture(Product::D4, 140, 140);
        let header = read_header(&bytes, Product::D4).unwrap();
        assert_eq!(header.settings, settings::Saved::default());
    }

    #[test]
    fn native_headers_use_original_pan_regions_and_signed_fader_curve() {
        for product in [Product::D3, Product::D4, Product::T3, Product::H1] {
            let mut bytes = header_fixture(product, 140, 140);
            let count = profile(product).unwrap().1 + usize::from(product != Product::H1);
            let start = bytes.len() - count * 8;
            for (pan, expected) in [
                (0, -1.0),
                (14, -1.0),
                (15, -113.0 / 128.0),
                (116, -12.0 / 128.0),
                (117, 0.0),
                (128, 0.0),
                (139, 0.0),
                (140, 12.0 / 128.0),
                (240, 112.0 / 128.0),
                (241, 1.0),
                (255, 1.0),
            ] {
                bytes[start..start + 2].copy_from_slice(&u16::to_le_bytes(pan));
                bytes[start + 2..start + 4].copy_from_slice(&25200u16.to_le_bytes());
                let header = read_header(&bytes, product).unwrap();
                let track = &header.tracks[0];
                assert_eq!(track.pan, expected, "saved pan {pan}");
                assert_eq!(track.left_enabled, expected < 1.0);
                assert_eq!(track.right_enabled, expected > -1.0);
                assert_eq!(track.gain, native_fader_gain(25200, 1.33));
            }
            bytes[start + 2..start + 4].copy_from_slice(&u16::MAX.to_le_bytes());
            let track = &read_header(&bytes, product).unwrap().tracks[0];
            assert!(track.polarity_inverted);
            assert_eq!(track.gain, 2.0 / 65536.0);
            assert_eq!(track.effective_gain(), -2.0 / 65536.0);
            bytes[start..start + 2].copy_from_slice(&256u16.to_le_bytes());
            assert!(read_header(&bytes, product).is_err());
        }
    }

    #[test]
    fn imported_native_mixer_settings_control_rendered_channels_and_levels() {
        for product in [Product::D3, Product::D4, Product::T3, Product::H1] {
            let mut bytes = header_fixture(product, 140, 140);
            let count = profile(product).unwrap().1 + usize::from(product != Product::H1);
            let start = bytes.len() - count * 8;
            for (index, pan) in [0u16, 255, 128].into_iter().enumerate() {
                let offset = start + index * 8;
                bytes[offset..offset + 2].copy_from_slice(&pan.to_le_bytes());
                bytes[offset + 2..offset + 4].copy_from_slice(&25200u16.to_le_bytes());
            }
            bytes[start + 20..start + 22].copy_from_slice(&1u16.to_le_bytes());
            let samples = HashMap::from([(
                1,
                crate::audio::Pcm {
                    sample_rate: 44_100,
                    channels: 1,
                    samples: vec![0.1; 8],
                },
            )]);
            let mut project = Project {
                tracks: read_header(&bytes, product).unwrap().tracks,
                master_gain: 1.0,
                clips: (0..3)
                    .map(|track| Clip {
                        id: track as u64 + 1,
                        sample_id: 1,
                        track,
                        start_tick: 0,
                        length_ticks: 1,
                        offset_frames: 0,
                        gain: if track == 1 { 2.0 } else { 1.0 },
                    })
                    .collect(),
                ..Project::default()
            };
            let output = crate::audio::render_with_samples(&project, &samples).unwrap();
            // 25200 maps to native Q16 coefficient 70133, not 25200/32767.
            assert_eq!(output.samples[0], 0.1 * (70133.0 / 65536.0));
            assert_eq!(output.samples[1], 0.2 * (70133.0 / 65536.0));
            bytes[start + 14..start + 16].copy_from_slice(&1u16.to_le_bytes());
            project.tracks = read_header(&bytes, product).unwrap().tracks;
            let solo = crate::audio::render_with_samples(&project, &samples).unwrap();
            assert_eq!(solo.samples[0], 0.0);
            assert_eq!(solo.samples[1], output.samples[1]);
            bytes[start + 12..start + 14].copy_from_slice(&1u16.to_le_bytes());
            project.tracks = read_header(&bytes, product).unwrap().tracks;
            assert!(
                crate::audio::render_with_samples(&project, &samples)
                    .unwrap()
                    .samples
                    .iter()
                    .all(|sample| *sample == 0.0)
            );
        }
    }

    #[test]
    #[ignore = "Requires the local installed D3, D4, and House song fixtures."]
    fn installed_native_mixer_headers_preserve_artist_settings() {
        let root = crate::project_io::default_root(Product::D3).unwrap();
        let header = read_header(
            &read_song_file(&root.join("MIX/start.mix")).unwrap(),
            Product::D3,
        )
        .unwrap();
        assert_eq!(header.after_mixer, 1319 + 33 * 8);
        assert_eq!(header.tracks[2].gain, 70133.0 / 65536.0);
        assert_eq!(
            (
                header.tracks[2].left_enabled,
                header.tracks[2].right_enabled
            ),
            (true, false)
        );
        assert_eq!(
            (
                header.tracks[3].left_enabled,
                header.tracks[3].right_enabled
            ),
            (false, true)
        );
        assert_eq!(header.tracks[32].gain, 53201.0 / 65536.0);
        assert_eq!(header.tracks[32].pan, 0.0);
        for (product, name, muted) in [
            (Product::H1, "amsterdam.mix", vec![13]),
            (Product::D4, "Deep house monotonic.mix", vec![15, 16]),
        ] {
            let root = crate::project_io::default_root(product).unwrap();
            let header = read_header(
                &read_song_file(&root.join("MIX").join(name)).unwrap(),
                product,
            )
            .unwrap();
            for index in muted {
                assert!(!header.tracks[index].left_enabled && !header.tracks[index].right_enabled);
                assert!(header.tracks[index].gain > 0.0);
            }
        }
    }

    #[test]
    #[ignore = "Requires the local installed D3, D4, T3, and House song fixtures."]
    fn installed_songs_keep_their_saved_settings_lists() {
        // Songs, songs with a list, and songs with the EQ, compressor, and
        // stereo wide switch of the Booster on.
        for (product, expected) in [
            (Product::D3, (17, 14, [0, 0, 0])),
            (Product::D4, (12, 12, [3, 6, 6])),
            (Product::T3, (20, 20, [0, 1, 1])),
            (Product::H1, (14, 14, [0, 0, 0])),
        ] {
            let folder = crate::project_io::default_root(product)
                .unwrap()
                .join("MIX");
            let (mut songs, mut lists, mut switches) = (0, 0, [0; 3]);
            for entry in fs::read_dir(folder).unwrap() {
                let path = entry.unwrap().path();
                if !path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("mix"))
                {
                    continue;
                }
                let saved = read_header(&read_song_file(&path).unwrap(), product)
                    .unwrap()
                    .settings;
                assert_eq!(saved.unread, 0, "{}", path.display());
                songs += 1;
                lists += usize::from(!saved.entries.is_empty());
                for (count, key) in
                    switches
                        .iter_mut()
                        .zip(["BoostEQ", "BoostCompressorLED", "BoostStereoLED"])
                {
                    *count += usize::from(saved.entries.get(key).is_some_and(|v| v == "1"));
                }
            }
            assert_eq!((songs, lists, switches), expected, "{}", product.label());
        }
        let root = crate::project_io::default_root(Product::D4).unwrap();
        let saved = read_header(
            &read_song_file(&root.join("MIX/welmix.mix")).unwrap(),
            Product::D4,
        )
        .unwrap()
        .settings;
        assert_eq!(saved.entries.len(), 93);
        assert_eq!(saved.entries["BoostCompressorDrive"], "12080");
        assert_eq!(saved.entries["BoostStereoWide"], "21348");
        assert_eq!(saved.entries["DrumK_SFX_ECHOTIME"], "5529");
        assert_eq!(saved.entries["FX_12"], "1");
    }

    #[test]
    fn header_rejects_truncation_foreign_identity_and_accepts_forty_eight_tracks() {
        let bytes = header_fixture(Product::T3, 140, 140);
        for end in [0, 3, 13, 16, bytes.len() - 1] {
            assert!(read_header(&bytes[..end], Product::T3).is_err());
        }
        assert!(read_header(&bytes, Product::D3).is_err());
        let oversized = header_fixture(Product::D4, 140, 140);
        assert_eq!(
            read_header(&oversized, Product::D4).unwrap().tracks.len(),
            TRACK_COUNT
        );
        assert!(read_header(&header_fixture(Product::H1, 0, 125), Product::H1).is_err());
    }

    #[test]
    fn counted_events_keep_native_ids_positions_and_complete_trim_fields() {
        let mut bytes = Vec::new();
        lane(&mut bytes, 1, &[(3, -257, 2520)]);
        lane(&mut bytes, 2, &[(0, 128, 2618)]);
        let trim = bytes.len() - 10;
        bytes[trim..trim + 4].copy_from_slice(&7u32.to_le_bytes());
        bytes[trim + 4..trim + 8].copy_from_slice(&11u32.to_le_bytes());
        bytes[trim + 8..].copy_from_slice(&(-1000i16).to_le_bytes());
        let events = read_chunk(&bytes, Product::H1).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].position, 512);
        assert_eq!(events[0].track, 0);
        assert_eq!(events[0].reference, Reference::Native(2520));
        assert_eq!(events[1].position, 256);
        assert_eq!((events[1].left_trim, events[1].right_trim), (14, 22));
        assert_eq!(events[1].gain, 31767.0 / 32767.0);
        assert_ne!(
            Product::H1.global_sample_id(2520).unwrap(),
            Product::D2.global_sample_id(2520).unwrap()
        );
        assert!(read_chunk(&bytes[..bytes.len() - 1], Product::H1).is_err());
    }

    #[test]
    fn utility_records_cannot_return_partial_success_or_overflow_tracks() {
        let mut bytes = Vec::new();
        lane(&mut bytes, 1, &[(0, -1, 174)]);
        lane(&mut bytes, 17, &[(6, -1, 7200)]);
        let error = read_chunk(&bytes, Product::D2).unwrap_err();
        assert!(error.contains("generated pattern"));
        let mut empty_extra = Vec::new();
        lane(&mut empty_extra, 1, &[]);
        lane(&mut empty_extra, 33, &[]);
        assert!(read_chunk(&empty_extra, Product::D3).unwrap().is_empty());
        let mut occupied_extra = Vec::new();
        lane(&mut occupied_extra, 1, &[]);
        lane(&mut occupied_extra, 33, &[(3, -1, 1)]);
        assert_eq!(
            read_chunk(&occupied_extra, Product::D3).unwrap()[0].track,
            32
        );
        let mut overflow = Vec::new();
        lane(&mut overflow, 65, &[(3, -1, 1)]);
        assert!(read_chunk(&overflow, Product::D4).is_err());
        let mut impossible_count = vec![1];
        impossible_count.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_chunk(&impossible_count, Product::T3).is_err());
    }

    fn wave_fixture() -> Vec<u8> {
        wave_fixture_frames(4)
    }

    fn wave_fixture_frames(frames: usize) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(
                &mut cursor,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 44100,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for value in [0i16, 1000, -1000, 0].into_iter().cycle().take(frames) {
                writer.write_sample(value).unwrap();
            }
            writer.finalize().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn embedded_wave_import_keeps_audio_origin_hash_and_saved_reference() {
        let wave = wave_fixture_frames(44100);
        let source_path = b"C:\\old\\MIXWAVES\\take.wav";
        let mut bytes = header_fixture(Product::D3, 140, 146);
        let after_mixer = bytes.len();
        bytes.extend_from_slice(&((source_path.len() + 2) as u16).to_le_bytes());
        bytes.extend_from_slice(source_path);
        bytes.extend_from_slice(&[0, 1]);
        bytes.extend_from_slice(&(wave.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&wave);
        let mut chunk = Vec::new();
        lane(&mut chunk, 1, &[(1, -1, 0)]);
        text_block(&mut chunk, source_path);
        lane(&mut chunk, 2, &[(1, -257, 0)]);
        text_block(&mut chunk, b"c:/OLD/MIXWAVES/TAKE.wav");
        bytes.extend_from_slice(&((chunk.len() + 4) as u32).to_le_bytes());
        bytes.extend_from_slice(&chunk);
        bytes.extend_from_slice(&SENTINEL);
        let destination = crate::project_io::data_root()
            .join("imported-audio")
            .join(format!("{:x}.wav", Sha256::digest(&wave)));
        let existed = destination.exists();
        let events = read_timeline(&bytes, after_mixer, Product::D3).unwrap();
        let project = build_project(
            Path::new("embedded.mix"),
            &bytes,
            Product::D3,
            &Library::empty(),
            read_header(&bytes, Product::D3).unwrap(),
            events,
        )
        .unwrap();
        assert_eq!(project.imported_samples.len(), 1);
        let imported = &project.imported_samples[0];
        assert_eq!(imported.origin, Some(Product::D3));
        assert_eq!(imported.path, destination);
        assert_eq!(imported.source_bpm, 140.0);
        assert_eq!(fs::read(&destination).unwrap(), wave);
        assert_eq!(project.clips[0].sample_id, imported.id);
        assert_eq!(project.clips.len(), 2);
        assert_eq!(project.clips[1].sample_id, imported.id);
        let mut library = Library::empty();
        library.apply_imports(&project.imported_samples).unwrap();
        let pcm = crate::audio::render_project(&project, &library).unwrap();
        assert!(pcm.samples.iter().any(|value| value.abs() > 0.001));
        let saved = std::env::temp_dir().join(format!(
            "mixjam-techno-embedded-project-{}.json",
            std::process::id()
        ));
        assert!(!saved.exists());
        project.save(&saved).unwrap();
        assert_eq!(Project::load(&saved).unwrap(), project);
        fs::remove_file(&saved).unwrap();
        if !existed {
            fs::remove_file(&destination).unwrap();
        }
    }

    #[test]
    fn embedded_riff_requires_a_matching_path_and_size_record() {
        let wave = wave_fixture();
        for includes_marker in [false, true] {
            let path = b"C:\\old\\MIXWAVES\\take.wav";
            let mut bytes = Vec::new();
            bytes.extend_from_slice(
                &((path.len() + usize::from(includes_marker) * 2) as u16).to_le_bytes(),
            );
            bytes.extend_from_slice(path);
            bytes.extend_from_slice(&[0, 1]);
            bytes.extend_from_slice(&(wave.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&wave);
            let found = embedded_waves(&bytes).unwrap();
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].frames, 4);
            assert_eq!(found[0].bytes, wave);
            assert!(matches!(found[0].bytes, Cow::Borrowed(_)));
            let size_offset = 2 + path.len() + 2;
            bytes[size_offset] ^= 1;
            assert!(embedded_waves(&bytes).unwrap().is_empty());
        }
        assert!(embedded_waves(&wave).unwrap().is_empty());
    }

    #[test]
    fn wave_binary_metadata_is_not_an_optional_text_token() {
        for (category, value) in [
            (1u16, 9u32),
            (2, 256),
            (3, 1),
            (4, 1),
            (5, 1),
            (6, 1),
            (9, 3),
            (10, 1),
            (12, 9),
            (12, 45),
        ] {
            let mut chunk = Vec::new();
            lane(&mut chunk, 1, &[(1, -1, 0)]);
            chunk[1..5].copy_from_slice(&2u32.to_le_bytes());
            text_block(&mut chunk, b"C:\\old\\MIXWAVES\\take.wav\0\x01");
            text_block(&mut chunk, b"sample\0\x01");
            text_block(&mut chunk, b"effect\0\x01");
            chunk.extend_from_slice(&category.to_le_bytes());
            chunk.extend_from_slice(&value.to_le_bytes());
            chunk.extend_from_slice(&0u16.to_le_bytes());
            chunk.push(3);
            chunk.extend_from_slice(&(-129i32).to_le_bytes());
            chunk.extend_from_slice(&174u16.to_le_bytes());
            let events = read_chunk(&chunk, Product::D3).unwrap();
            assert_eq!(events.len(), 2);
            assert_eq!(events[1].reference, Reference::Native(174));
            assert_eq!(events[1].position, 256);
        }
    }

    #[test]
    fn d3_wave_labels_allow_empty_and_ansi_text_but_require_complete_metadata() {
        let mut extension = Vec::new();
        text_block(&mut extension, b"C:\\old\\MIXWAVES\\take.wav\0\x01");
        text_block(&mut extension, b"\0\x01");
        text_block(&mut extension, b"caf\xe9\0\x01");
        extension.extend_from_slice(&[2, 0, 0, 1, 0, 0, 0, 0]);
        let mut reader = Reader {
            bytes: &extension,
            offset: 0,
        };
        assert_eq!(
            read_wave_extension(&mut reader, 1, Product::D3)
                .unwrap()
                .path,
            r"C:\old\MIXWAVES\take.wav"
        );
        assert_eq!(reader.offset, extension.len());
        for bars in [0u32, MAX_TICKS / TICKS_PER_BAR + 1, u32::MAX] {
            let mut invalid = extension.clone();
            let span = invalid.len() - 6;
            invalid[span..span + 4].copy_from_slice(&bars.to_le_bytes());
            let mut reader = Reader {
                bytes: &invalid,
                offset: 0,
            };
            assert!(read_wave_extension(&mut reader, 1, Product::D3).is_err());
        }
        for end in 0..extension.len() {
            let mut reader = Reader {
                bytes: &extension[..end],
                offset: 0,
            };
            assert!(read_wave_extension(&mut reader, 1, Product::D3).is_err());
        }
        let label_start = 2 + u16::from_le_bytes(extension[..2].try_into().unwrap()) as usize;
        extension[label_start + 3] = 2;
        let mut reader = Reader {
            bytes: &extension,
            offset: 0,
        };
        assert!(read_wave_extension(&mut reader, 1, Product::D3).is_err());
    }

    #[test]
    fn d3_embedded_wave_uses_saved_bar_span_for_trims_and_repeated_sources() {
        // Waterworld stores four bars for 3.75 bars of audio. Its right trim
        // leaves a quarter-bar clip, not an empty measured-duration clip.
        let wave = wave_fixture_frames(283500);
        let path = b"C:\\old\\MIXWAVES\\Water002.wav";
        let mut bytes = header_fixture(Product::D3, 140, 140);
        text_block(&mut bytes, &[path.as_slice(), &[0, 1]].concat());
        bytes.extend_from_slice(&(wave.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&wave);
        let mut chunk = Vec::new();
        for (track, bars, right) in [(1, 4u32, 480u32), (2, 2, 224), (3, 4, 0)] {
            lane(&mut chunk, track, &[(1, 0, 0)]);
            let trim = chunk.len() - 10;
            chunk[trim + 4..trim + 8].copy_from_slice(&right.to_le_bytes());
            text_block(&mut chunk, &[path.as_slice(), &[0, 1]].concat());
            text_block(&mut chunk, b"Water\0\x01");
            text_block(&mut chunk, b"002\0\x01");
            chunk.extend_from_slice(&10u16.to_le_bytes());
            chunk.extend_from_slice(&bars.to_le_bytes());
            chunk.extend_from_slice(&14185u16.to_le_bytes());
        }
        bytes.extend_from_slice(&((chunk.len() + 4) as u32).to_le_bytes());
        bytes.extend_from_slice(&chunk);
        bytes.extend_from_slice(&SENTINEL);
        let header = read_header(&bytes, Product::D3).unwrap();
        let timeline = read_timeline(&bytes, header.after_mixer, Product::D3).unwrap();
        let destination = crate::project_io::data_root()
            .join("imported-audio")
            .join(format!("{:x}.wav", Sha256::digest(&wave)));
        let existed = destination.exists();
        let mut project = build_project(
            Path::new("span.mix"),
            &bytes,
            Product::D3,
            &Library::empty(),
            header,
            timeline,
        )
        .unwrap();
        assert_eq!(project.imported_samples.len(), 1);
        assert_eq!(
            project
                .clips
                .iter()
                .map(|clip| clip.length_ticks)
                .collect::<Vec<_>>(),
            [64, 64, 1024]
        );
        let mut library = Library::empty();
        library.apply_imports(&project.imported_samples).unwrap();
        project.clips.retain(|clip| clip.track == 2);
        let rendered = crate::audio::render_project(&project, &library).unwrap();
        assert_eq!(rendered.frames(), 302400);
        assert!(
            rendered.samples[..283500 * 2]
                .iter()
                .any(|value| value.abs() > 0.001)
        );
        assert!(
            rendered.samples[283500 * 2..]
                .iter()
                .all(|value| *value == 0.0)
        );
        if !existed {
            fs::remove_file(destination).unwrap();
        }
    }

    #[test]
    fn preserved_audio_never_overwrites_an_existing_file() {
        let directory = asset_test_directory("chunk-audio");
        let path = directory.join("sample.wav");
        let bytes = wave_fixture();
        preserve_wave(&path, &bytes).unwrap();
        preserve_wave(&path, &bytes).unwrap();
        assert!(preserve_wave(&path, b"different").is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&directory).unwrap();
    }

    fn asset_test_directory(label: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "mixjam-techno-{label}-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_ASSET_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        directory
    }

    #[test]
    fn unsupported_wave_rate_is_rejected_before_any_asset_is_published() {
        let mut wave = wave_fixture_frames(44100);
        wave[24..28].copy_from_slice(&705600u32.to_le_bytes());
        wave[28..32].copy_from_slice(&1411200u32.to_le_bytes());
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_le_bytes();
        wave.extend_from_slice(b"JUNK");
        wave.extend_from_slice(&16u32.to_le_bytes());
        wave.extend_from_slice(&nonce);
        let riff_length = (wave.len() - 8) as u32;
        wave[4..8].copy_from_slice(&riff_length.to_le_bytes());
        let reader = hound::WavReader::new(Cursor::new(&wave)).unwrap();
        assert_eq!(reader.spec().sample_rate, 705600);
        assert_eq!(reader.duration(), 44100);
        let hash = format!("{:x}", Sha256::digest(&wave));
        let destination = crate::project_io::data_root()
            .join("imported-audio")
            .join(format!("{hash}.wav"));
        assert!(!destination.exists());
        let source_path = r"C:\old\MIXWAVES\high-rate.wav";
        let mut bytes = header_fixture(Product::D3, 140, 140);
        let header = read_header(&bytes, Product::D3).unwrap();
        bytes.extend_from_slice(&((source_path.len() + 2) as u16).to_le_bytes());
        bytes.extend_from_slice(source_path.as_bytes());
        bytes.extend_from_slice(&[0, 1]);
        bytes.extend_from_slice(&(wave.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&wave);
        let timeline = Timeline {
            start: bytes.len(),
            events: vec![Event {
                track: 0,
                position: 0,
                left_trim: 0,
                right_trim: 0,
                gain: 1.0,
                reference: Reference::Wave(WaveReference {
                    path: source_path.into(),
                    declared_ticks: None,
                    metadata: BTreeMap::new(),
                }),
            }],
        };
        assert!(
            build_project(
                Path::new("high-rate.mix"),
                &bytes,
                Product::D3,
                &Library::empty(),
                header,
                timeline
            )
            .is_err()
        );
        assert!(!destination.exists());
        if let Some(parent) = destination.parent().filter(|parent| parent.is_dir()) {
            assert!(!fs::read_dir(parent).unwrap().any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&format!(".{hash}.wav."))
            }));
        }
    }

    #[test]
    fn wave_preflight_matches_pcm_encodings_counts_and_channel_alignment() {
        let directory = asset_test_directory("wav-preflight");
        for bits in [8, 16, 24, 32] {
            let mut cursor = Cursor::new(Vec::new());
            {
                let mut writer = hound::WavWriter::new(
                    &mut cursor,
                    hound::WavSpec {
                        channels: 1,
                        sample_rate: 44100,
                        bits_per_sample: bits,
                        sample_format: hound::SampleFormat::Int,
                    },
                )
                .unwrap();
                for sample in [0i32, 100, -100, 0] {
                    writer.write_sample(sample).unwrap();
                }
                writer.finalize().unwrap();
            }
            let bytes = cursor.into_inner();
            assert_eq!(wave_info(&bytes).unwrap(), (4, 44100));
            let path = directory.join(format!("pcm{bits}.wav"));
            fs::write(&path, &bytes).unwrap();
            assert_eq!(crate::audio::read_wav(&path).unwrap().frames(), 4);
            fs::remove_file(path).unwrap();
        }
        let mut excessive = wave_fixture();
        let data = excessive
            .windows(4)
            .position(|bytes| bytes == b"data")
            .unwrap();
        excessive[data + 4..data + 8].copy_from_slice(&(64_000_001u32 * 2).to_le_bytes());
        assert!(wave_info(&excessive).is_err());
        let mut unaligned = wave_fixture_frames(3);
        unaligned[22..24].copy_from_slice(&2u16.to_le_bytes());
        unaligned[28..32].copy_from_slice(&176400u32.to_le_bytes());
        unaligned[32..34].copy_from_slice(&4u16.to_le_bytes());
        assert!(wave_info(&unaligned).is_err());
        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn paired_native_labels_survive_shared_project_and_library_validation() {
        let directory = asset_test_directory("paired-labels");
        let path = directory.join("source.wav");
        fs::write(&path, wave_fixture()).unwrap();
        let group = "É".repeat(200);
        let variant = "ß".repeat(200);
        let name = format!("{group} {variant}");
        assert_eq!(name.chars().count(), 401);
        let sample = ImportedSample {
            id: IMPORTED_ID_START,
            name: name.clone(),
            group_name: group.clone(),
            path: path.clone(),
            source_bpm: 140.0,
            source_sample_rate: Some(44100),
            origin: Some(Product::D2),
        };
        crate::engine::validate_import_metadata(&sample).unwrap();
        let project = Project {
            imported_samples: vec![sample].into(),
            ..Project::default()
        };
        project.validate().unwrap();
        let mut library = Library::empty();
        library.apply_imports(&project.imported_samples).unwrap();
        let actual = library.sample_by_id(IMPORTED_ID_START).unwrap();
        assert_eq!(actual.name, name);
        assert_eq!(actual.group_name, group);
        fs::remove_file(path).unwrap();
        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn concurrent_equal_asset_publication_is_atomic_and_idempotent() {
        use std::sync::{Arc, Barrier};
        let directory = asset_test_directory("concurrent-publication");
        let path = Arc::new(directory.join("equal.wav"));
        let bytes = Arc::new(wave_fixture_frames(44100));
        let barrier = Arc::new(Barrier::new(8));
        let writers: Vec<_> = (0..8)
            .map(|_| {
                let path = Arc::clone(&path);
                let bytes = Arc::clone(&bytes);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    preserve_wave(&path, &bytes)
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap().unwrap();
        }
        assert_eq!(fs::read(path.as_ref()).unwrap(), *bytes);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        preserve_wave(&path, &bytes).unwrap();
        assert!(preserve_wave(&path, b"different bytes").is_err());
        assert_eq!(fs::read(path.as_ref()).unwrap(), *bytes);
        fs::remove_file(path.as_ref()).unwrap();
        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn interrupted_or_failed_staging_leaves_final_assets_safe_and_retryable() {
        let directory = asset_test_directory("interrupted-publication");
        let path = directory.join("retry.wav");
        let bytes = wave_fixture();
        let interrupted = stage_asset(&path, &bytes).unwrap();
        assert!(!path.exists());
        fs::write(&interrupted, b"partial prepublication write").unwrap();
        preserve_wave(&path, &bytes).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(
            fs::read(&interrupted).unwrap(),
            b"partial prepublication write"
        );
        let collision = directory.join("collision.json");
        let staged = stage_asset(&collision, b"complete metadata").unwrap();
        fs::write(&collision, b"unrelated existing metadata").unwrap();
        assert!(publish_staged_asset(&staged, &collision, b"complete metadata").is_err());
        assert!(!staged.exists());
        assert_eq!(
            fs::read(&collision).unwrap(),
            b"unrelated existing metadata"
        );
        let absent = directory.join("missing-parent/absent.wav");
        let staged = stage_asset(&path, &bytes).unwrap();
        assert!(publish_staged_asset(&staged, &absent, &bytes).is_err());
        assert!(!absent.exists());
        assert!(!staged.exists());
        fs::remove_file(&path).unwrap();
        fs::remove_file(&interrupted).unwrap();
        fs::remove_file(&collision).unwrap();
        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn direct_publication_never_replaces_an_existing_asset() {
        let directory = asset_test_directory("direct-publication");
        let path = directory.join("direct.wav");
        let bytes = wave_fixture();
        publish_asset_directly(&path, &bytes).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        publish_asset_directly(&path, &bytes).unwrap();
        assert!(publish_asset_directly(&path, b"different bytes").is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let absent = directory.join("missing-parent/absent.wav");
        assert!(publish_asset_directly(&absent, &bytes).is_err());
        assert!(!absent.exists());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn saved_wave_paths_require_full_identity_and_preserve_duplicate_checks() {
        let bytes = wave_fixture();
        let same = || Wave {
            source_path: r"C:\original\Kick.wav".into(),
            bytes: bytes.clone().into(),
            frames: 4,
            sample_rate: 44100,
        };
        let embedded = [same(), same()];
        let wave = source_wave("c:/ORIGINAL/kick.wav", &embedded, &Library::empty()).unwrap();
        assert_eq!(wave.bytes, bytes);
        let different = Wave {
            source_path: "C:/original/KICK.wav".into(),
            bytes: b"different".to_vec().into(),
            frames: 4,
            sample_rate: 44100,
        };
        assert!(
            source_wave(
                r"C:\original\Kick.wav",
                &[same(), different],
                &Library::empty()
            )
            .is_err()
        );
        assert!(source_wave(r"C:\missing-user\Kick.wav", &[same()], &Library::empty()).is_err());
    }

    #[test]
    fn missing_user_wave_does_not_bind_an_unrelated_installation_basename() {
        let directory = asset_test_directory("wave-identity");
        let installation = directory.join("installation/MIXWAVES");
        fs::create_dir_all(&installation).unwrap();
        let unrelated = installation.join("kick.wav");
        fs::write(&unrelated, wave_fixture()).unwrap();
        let missing = directory.join("missing-user/kick.wav");
        assert!(source_wave(&missing.to_string_lossy(), &[], &Library::empty()).is_err());
        // Selecting the WAV folder explicitly makes its unique filename usable
        // after the original user's absolute path has become unavailable.
        let mut library = Library::empty();
        library.import_wav_folder(&installation, 140.0).unwrap();
        let moved = source_wave(&missing.to_string_lossy(), &[], &library).unwrap();
        assert_eq!(moved.bytes, wave_fixture());
        fs::remove_file(&unrelated).unwrap();
        fs::remove_dir(&installation).unwrap();
        fs::remove_dir(directory.join("installation")).unwrap();
        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn native_clock_and_signed_fader_values_keep_the_recovered_contract() {
        assert_eq!(native_position(-129).unwrap(), 256);
        assert_eq!(native_position(8).unwrap(), 16);
        assert!(native_position(i32::MIN).is_err());
        assert!(native_tick_count(MAX_TICKS).is_err());
        assert_eq!(native_fader_gain(65535, 1.33), -2.0 / 65536.0);
        assert_eq!(native_fader_gain(32767, 1.33), 154182.0 / 65536.0);
        let mut pending = Vec::new();
        let mut total = MAX_PENDING_ASSET_BYTES;
        assert!(queue_asset(&mut pending, &mut total, PathBuf::from("unused"), vec![1]).is_err());
        assert!(pending.is_empty());
    }

    #[test]
    #[ignore = "Requires the local installed D2 song fixtures."]
    fn installed_d2_pattern_headers_are_complete_before_audio_conversion() {
        let installation = crate::project_io::default_root(Product::D2)
            .expect("The D2 fixture installation is missing.");
        let folder = installation.join("D2/MIX");
        let mut total_events = 0;
        let mut songs = 0;
        let mut drum_songs = 0;
        let (mut filters, mut echoes) = (0, 0);
        for entry in fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mix"))
            {
                continue;
            }
            songs += 1;
            let bytes = read_song_file(&path).unwrap();
            let header = read_header(&bytes, Product::D2).unwrap();
            assert_eq!(header.settings, settings::Saved::default());
            let timeline = read_timeline(&bytes, header.after_mixer, Product::D2).unwrap();
            let kit =
                drums::parse_kit(&bytes, header.after_mixer, timeline.start, Product::D2).unwrap();
            assert_eq!(kit.pads.len(), 10);
            assert_eq!(kit.source_end + 18, timeline.start);
            let effects = kit.groove_effects.unwrap();
            filters += usize::from(effects.filter_on);
            echoes += usize::from(effects.echo_on);
            assert!(!effects.distortion_on);
            assert!((13_527..=25_250).contains(&effects.volume));
            let patterns: Vec<_> = timeline
                .events
                .iter()
                .filter_map(|event| {
                    if let Reference::Drum(pattern) = &event.reference {
                        Some((event, pattern))
                    } else {
                        None
                    }
                })
                .collect();
            if !patterns.is_empty() {
                drum_songs += 1;
            }
            for (event, pattern) in &patterns {
                assert_eq!(event.track, 16);
                assert!(pattern.native_id >= 7200);
                assert_eq!(event.position % 256, 0);
            }
            total_events += patterns.len();
        }
        assert_eq!((songs, drum_songs, total_events), (13, 8, 449));
        assert_eq!((filters, echoes), (3, 4));
    }

    #[test]
    #[ignore = "Requires the local installed D3/D4 song fixtures."]
    fn installed_sixteen_pad_headers_and_all_patterns_are_complete() {
        for (product, expected_songs, expected_events, expected_unique) in
            [(Product::D3, 1, 23, 3), (Product::D4, 2, 86, 10)]
        {
            let installation = crate::project_io::default_root(product).unwrap();
            let mut songs = 0;
            let mut events = 0;
            let mut unique = std::collections::HashSet::new();
            for entry in fs::read_dir(installation.join("Mix")).unwrap() {
                let path = entry.unwrap().path();
                if !path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("mix"))
                {
                    continue;
                }
                let bytes = read_song_file(&path).unwrap();
                let header = read_header(&bytes, product).unwrap();
                let timeline = read_timeline(&bytes, header.after_mixer, product).unwrap();
                let patterns: Vec<_> = timeline
                    .events
                    .iter()
                    .filter_map(|event| match &event.reference {
                        Reference::Drum(pattern) => Some((event, pattern)),
                        _ => None,
                    })
                    .collect();
                if patterns.is_empty() {
                    continue;
                }
                songs += 1;
                let kit = drums::parse_kit(&bytes, header.after_mixer, timeline.start, product)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                assert_eq!(kit.pads.len(), 16);
                assert_eq!(kit.header_end, Some(timeline.start));
                assert_eq!(kit.raw_header, bytes[kit.source_offset..timeline.start]);
                for pad in &kit.pads {
                    assert_eq!(pad.opaque_native_settings.len(), 10);
                    assert_eq!(pad.source_paths.len(), 2);
                }
                for (event, pattern) in patterns {
                    assert_eq!(event.track, profile(product).unwrap().1);
                    assert_eq!(pattern.masks.len(), 16);
                    assert_eq!(event.position % TICKS_PER_BAR, 0);
                    events += 1;
                    unique.insert((path.clone(), pattern.clone()));
                }
            }
            assert_eq!(
                (songs, events, unique.len()),
                (expected_songs, expected_events, expected_unique)
            );
        }
    }

    #[test]
    #[ignore = "Requires the local installed song fixtures."]
    fn installed_counted_profiles_have_explicit_complete_results() {
        let cases = [
            (Product::D2, "D2/MIX", (13, 13)),
            (Product::D3, "MIX", (17, 17)),
            (Product::D4, "Mix", (12, 12)),
            (Product::H1, "Mix", (14, 14)),
            (Product::T3, "Mix", (20, 20)),
        ];
        for (product, mix_folder, expected) in cases {
            let installation = crate::project_io::default_root(product)
                .expect("The installed song fixtures are missing.");
            let folder = installation.join(mix_folder);
            if product == Product::D4 {
                let placeholder = folder.join(".mix");
                assert_eq!(fs::metadata(&placeholder).unwrap().len(), 2);
                assert!(read_header(&read_song_file(&placeholder).unwrap(), product).is_err());
            }
            let mut library = Library::empty();
            library
                .register(product, &installation, Path::new(""))
                .unwrap();
            let mut parsed = 0;
            let mut drum_audio_projects = 0;
            let mut failures = Vec::new();
            let paths: Vec<_> = fs::read_dir(&folder)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("mix"))
                })
                .collect();
            assert_eq!(paths.len(), expected.0);
            for path in paths {
                let bytes = read_song_file(&path).unwrap();
                let result = read_header(&bytes, product).and_then(|header| {
                    let events = read_timeline(&bytes, header.after_mixer, product)?;
                    build_project(&path, &bytes, product, &library, header, events)
                });
                match result {
                    Ok(project) => {
                        project.validate().unwrap();
                        assert_eq!(project.tracks.len(), TRACK_COUNT);
                        assert_eq!(project.pitch_ratio, 1.0);
                        if product == Product::D3
                            && path
                                .file_stem()
                                .is_some_and(|name| name.eq_ignore_ascii_case("all the way up"))
                        {
                            let sample_id = Product::D3.global_sample_id(13633).unwrap();
                            let trimmed = project
                                .clips
                                .iter()
                                .find(|clip| clip.sample_id == sample_id && clip.start_tick == 3040)
                                .unwrap();
                            assert_eq!(trimmed.length_ticks, 48);
                        }
                        assert!(project.clips.iter().all(|clip| {
                            Product::sample_key(clip.sample_id).map_or_else(
                                || {
                                    project.imported_samples.iter().any(|sample| {
                                        sample.id == clip.sample_id
                                            && sample.origin == Some(product)
                                    })
                                },
                                |(owner, _)| owner == product,
                            )
                        }));
                        let utility_track = profile(product).unwrap().1;
                        if project.clips.iter().any(|clip| {
                            clip.track == utility_track && clip.sample_id >= IMPORTED_ID_START
                        }) {
                            library.apply_imports(&project.imported_samples).unwrap();
                            let drum = project
                                .clips
                                .iter()
                                .find(|clip| {
                                    clip.track == utility_track
                                        && clip.sample_id >= IMPORTED_ID_START
                                })
                                .unwrap()
                                .clone();
                            assert_eq!(drum.length_ticks, TICKS_PER_BAR);
                            let mut representative = project.clone();
                            representative.loop_region = None;
                            representative.clips = vec![Clip {
                                start_tick: 0,
                                ..drum
                            }];
                            let pcm =
                                crate::audio::render_project(&representative, &library).unwrap();
                            pcm.validate().unwrap();
                            assert!(
                                pcm.samples.iter().any(|sample| sample.abs() > 0.0001),
                                "{} drum audio is silent",
                                path.display()
                            );
                            assert_eq!(
                                pcm.frames(),
                                (44100.0 * 240.0 / project.bpm).round() as u64
                            );
                            drum_audio_projects += 1;
                        }
                        if parsed == 0 {
                            let clip = project
                                .clips
                                .iter()
                                .find(|clip| {
                                    let track = &project.tracks[clip.track];
                                    clip.gain > 0.0
                                        && track.gain > 0.0
                                        && (track.left_enabled || track.right_enabled)
                                })
                                .unwrap()
                                .clone();
                            let mut representative = project.clone();
                            representative.loop_region = None;
                            representative.clips = vec![Clip {
                                start_tick: 0,
                                length_ticks: clip.length_ticks.min(TICKS_PER_BAR * 4),
                                ..clip
                            }];
                            let pcm = crate::audio::render_project(&representative, &library)
                                .unwrap_or_else(|error| {
                                    panic!("{} render: {error}", path.display())
                                });
                            pcm.validate().unwrap();
                            assert_eq!(pcm.channels, 2);
                            assert_eq!(pcm.sample_rate, 44100);
                            assert!(pcm.frames() > 0);
                            let peak = pcm
                                .samples
                                .iter()
                                .fold(0.0f32, |peak, value| peak.max(value.abs()));
                            assert!(peak > 0.0001, "{} rendered silence", path.display());
                            eprintln!(
                                "{} representative: {} sample {}, {} stereo frames, peak {peak}",
                                product.label(),
                                path.file_name().unwrap().to_string_lossy(),
                                representative.clips[0].sample_id,
                                pcm.frames()
                            );
                        }
                        parsed += 1;
                    }
                    Err(error) => failures.push(format!("{}: {error}", path.display())),
                }
            }
            eprintln!("{}: {parsed} complete projects", product.label());
            assert!(failures.is_empty(), "{}", failures.join("\n"));
            assert_eq!(parsed, expected.1);
            assert_eq!(
                drum_audio_projects,
                match product {
                    Product::D2 => 8,
                    Product::D3 => 1,
                    Product::D4 => 2,
                    _ => 0,
                }
            );
        }
    }
}
