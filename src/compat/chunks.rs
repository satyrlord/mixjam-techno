//! Counted timeline profiles from the installed Gen 2 and Gen 3 products.
//! Unsupported generated records cause an error before a project is returned.
use super::drums;
use crate::{
    engine::{
        Clip, ImportedSample, MAX_TICKS, Project, TICKS_PER_BAR, TRACK_COUNT, Track,
        normalize_import,
    },
    library::{Library, decode_1252},
    products::{IMPORTED_ID_START, Product},
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_MIX_BYTES: usize = 64 * 1024 * 1024;
const MAX_EVENTS: usize = 100_000;
const MAX_PENDING_ASSET_BYTES: usize = 256 * 1024 * 1024;
const SENTINEL: [u8; 4] = [0xa9, 0x7a, 0x8a, 0x01];
static NEXT_ASSET_TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub fn load(path: &Path, product: Product, library: &Library) -> Result<Project, String> {
    if product == Product::T2 {
        return crate::legacy_mix::load(path, library);
    }
    let bytes = read_bounded(path)?;
    let header = read_header(&bytes, product)?;
    let raw = read_timeline(&bytes, header.after_mixer, product)?;
    build_project(path, &bytes, product, library, header, raw)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > MAX_MIX_BYTES as u64 {
        return Err("The native song file exceeds the 64 MiB limit.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_MIX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_MIX_BYTES {
        return Err("The native song file exceeds the 64 MiB limit.".into());
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

struct Header {
    title: String,
    author: String,
    bpm: f64,
    effective_bpm: f64,
    tracks: Vec<Track>,
    after_mixer: usize,
}

fn profile(product: Product) -> Result<(u32, usize, usize), String> {
    match product {
        Product::T2 => Ok((0x889, 16, 4)),
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
    if track_count > TRACK_COUNT {
        return Err("The native song declares more than 64 tracks.".into());
    }
    let author = first_text(reader.block()?);
    let title = first_text(reader.block()?);
    let mut tracks = vec![Track::default(); TRACK_COUNT];
    for track in &mut tracks[..track_count] {
        let control = reader.u16()?;
        let gain = reader.u16()?;
        if gain > 32767 && !matches!(product, Product::T3 | Product::D2) {
            return Err("The native track volume exceeds its supported range.".into());
        }
        track.gain = match product {
            Product::T3 => native_fader_gain(gain, 1.33),
            Product::D2 => native_fader_gain(gain, 1.25),
            _ => f32::from(gain) / 32767.0,
        };
        if track_bytes == 4 {
            if control > 3 {
                return Err("The native track has invalid output routing.".into());
            }
            track.left_enabled = matches!(control, 0 | 1);
            track.right_enabled = matches!(control, 0 | 2);
        } else {
            track.pan = decode_pan(control)?;
            reader.u32()?;
        }
    }
    Ok(Header {
        title,
        author,
        bpm,
        effective_bpm,
        tracks,
        after_mixer: reader.offset,
    })
}

fn first_text(bytes: &[u8]) -> String {
    decode_1252(bytes.split(|byte| *byte == 0).next().unwrap_or_default())
        .trim_end_matches('\u{1}')
        .to_string()
}

// Stored center 128 is neutral. Each side reaches its corresponding endpoint.
fn decode_pan(value: u16) -> Result<f32, String> {
    if value > 255 {
        return Err("The native track pan exceeds its supported range.".into());
    }
    let signed = f32::from(value) - 128.0;
    Ok(signed / if value <= 128 { 128.0 } else { 127.0 })
}

#[derive(Debug, PartialEq)]
enum Reference {
    Native(u16),
    Wave(String),
    Drum(drums::Pattern),
}
#[derive(Debug, PartialEq)]
struct Event {
    track: usize,
    position: u32,
    left_trim: u32,
    right_trim: u32,
    gain: f32,
    reference: Reference,
}

struct Timeline {
    start: usize,
    events: Vec<Event>,
}

fn read_timeline(bytes: &[u8], lower_bound: usize, product: Product) -> Result<Timeline, String> {
    let marker = bytes
        .windows(4)
        .rposition(|value| value == SENTINEL)
        .filter(|offset| *offset >= lower_bound)
        .ok_or("The native song has no timeline end marker.")?;
    let mut result = None;
    let mut last_error = None;
    for offset in lower_bound..marker.saturating_sub(4) {
        let length = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        if length < 9
            || offset.checked_add(length) != Some(marker)
            || bytes.get(offset + 4) != Some(&1)
        {
            continue;
        }
        match read_chunk(&bytes[offset + 4..marker], product) {
            Ok(events) => {
                if result.is_some() {
                    return Err("The native song has ambiguous timeline data.".into());
                }
                result = Some(Timeline {
                    start: offset,
                    events,
                });
            }
            Err(error) => last_error = Some(error),
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
            || count > MAX_EVENTS
            || count > bytes.len().saturating_sub(reader.offset) / 7
        {
            return Err("The native timeline has an invalid lane or event count.".into());
        }
        previous_lane = lane;
        if lane > TRACK_COUNT && count > 0 {
            return Err("The native song uses more than 64 tracks.".into());
        }
        for _ in 0..count {
            if events.len() >= MAX_EVENTS {
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
                event.reference = Reference::Wave(read_wave_extension(&mut reader, lane, product)?);
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
                Product::D3 | Product::D4 | Product::T3 | Product::H1
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
) -> Result<String, String> {
    let raw = reader.block()?;
    if !(12..=260).contains(&raw.len()) {
        return Err("The native WAV path has an unsupported length.".into());
    }
    let path = first_text(raw);
    if path.as_bytes().get(1) != Some(&b':') || !path.to_ascii_lowercase().ends_with(".wav") {
        return Err("The native WAV reference is invalid.".into());
    }
    // Known path extensions can carry up to three additional text tokens.
    for _ in 0..3 {
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
        reader.offset += 2 + length;
    }
    let tail = &reader.bytes[reader.offset..];
    if tail.starts_with(&[2, 0, 0, 1]) {
        reader.take(12)?;
    } else if tail.len() >= 8 && matches!(u16::from_le_bytes(tail[..2].try_into().unwrap()), 6 | 12)
    {
        reader.take(8)?;
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
    Ok(path)
}

struct Wave {
    source_path: String,
    bytes: Vec<u8>,
    frames: u32,
    sample_rate: u32,
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

fn embedded_waves(bytes: &[u8]) -> Result<Vec<Wave>, String> {
    let mut waves = Vec::new();
    for riff in 0..bytes.len().saturating_sub(12) {
        if &bytes[riff..riff + 4] != b"RIFF" || &bytes[riff + 8..riff + 12] != b"WAVE" {
            continue;
        }
        let size = u32::from_le_bytes(bytes[riff + 4..riff + 8].try_into().unwrap()) as usize + 8;
        if size < 44 || riff.checked_add(size).is_none_or(|end| end > bytes.len()) {
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
            let (frames, sample_rate) = wave_info(wav)?;
            waves.push(Wave {
                source_path,
                bytes: wav.to_vec(),
                frames,
                sample_rate,
            });
        }
    }
    Ok(waves)
}

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn source_wave(path: &str, embedded: &[Wave], library: &Library) -> Result<Wave, String> {
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
            bytes: first.bytes.clone(),
            frames: first.frames,
            sample_rate: first.sample_rate,
        });
    }
    let file = match crate::preservation::resolve_asset(Path::new(path)) {
        Ok(file) if file.is_file() => file,
        _ => library.resolve_wav_reference(path)?,
    };
    if !file.is_file() {
        return Err(format!(
            "The native song needs WAV audio that is missing: {}.",
            basename(path)
        ));
    }
    let bytes = read_bounded(&file)?;
    let (frames, sample_rate) = wave_info(&bytes)?;
    Ok(Wave {
        source_path: path.into(),
        bytes,
        frames,
        sample_rate,
    })
}

fn normalize_saved_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn build_project(
    path: &Path,
    bytes: &[u8],
    product: Product,
    library: &Library,
    header: Header,
    timeline: Timeline,
) -> Result<Project, String> {
    let events = timeline.events;
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
    let prepared_kit = kit
        .as_ref()
        .map(|kit| {
            drums::prepare_kit(
                kit,
                library,
                product,
                events.iter().filter_map(|event| match &event.reference {
                    Reference::Drum(pattern) => Some(pattern),
                    _ => None,
                }),
            )
        })
        .transpose()?;
    let embedded = if events
        .iter()
        .any(|event| matches!(event.reference, Reference::Wave(_)))
    {
        embedded_waves(bytes)?
    } else {
        Vec::new()
    };
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
        libraries: library.bindings().to_vec(),
        ..Project::default()
    };
    let mut pending_waves: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    let mut pending_bytes = 0;
    let mut drum_cache: HashMap<drums::Pattern, (u32, u32)> = HashMap::new();
    for event in events {
        let (sample_id, frames, rate, source_bpm, declared_ticks) = match event.reference {
            Reference::Native(native_id) => {
                let id = product.global_sample_id(u32::from(native_id))?;
                let sample = library.sample_by_id(id).ok_or_else(|| {
                    format!(
                        "{} native sample {} is missing from its library.",
                        product.label(),
                        native_id
                    )
                })?;
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
            }
            Reference::Wave(source_path) => {
                let wave = source_wave(&source_path, &embedded, library)?;
                let hash = format!("{:x}", Sha256::digest(&wave.bytes));
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
                    let id = IMPORTED_ID_START
                        .checked_add(
                            u32::try_from(project.imported_samples.len())
                                .map_err(|_| "The imported sample limit was reached.")?,
                        )
                        .ok_or("The imported sample limit was reached.")?;
                    project.imported_samples.push(ImportedSample {
                        id,
                        name: basename(&source_path).into(),
                        group_name: "Imported".into(),
                        path: destination.clone(),
                        source_bpm: header.bpm,
                        origin: Some(product),
                    });
                    queue_asset(
                        &mut pending_waves,
                        &mut pending_bytes,
                        destination,
                        wave.bytes,
                    )?;
                    id
                };
                (id, wave.frames, wave.sample_rate, header.bpm, None)
            }
            Reference::Drum(pattern) => {
                let (id, frames) = if let Some(cached) = drum_cache.get(&pattern) {
                    *cached
                } else {
                    let pcm = drums::render(
                        prepared_kit
                            .as_ref()
                            .ok_or("The native drum kit is missing.")?,
                        &pattern,
                        header.bpm,
                    )?;
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
                        let id = IMPORTED_ID_START
                            + u32::try_from(project.imported_samples.len())
                                .map_err(|_| "The imported sample limit was reached.")?;
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
                        "source_mix_sha256": format!("{:x}", Sha256::digest(bytes)),
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
                };
                (id, frames, 44100, header.bpm, None)
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

fn queue_asset(
    pending: &mut Vec<(PathBuf, Vec<u8>)>,
    total_bytes: &mut usize,
    path: PathBuf,
    bytes: Vec<u8>,
) -> Result<(), String> {
    let total = total_bytes
        .checked_add(bytes.len())
        .filter(|total| *total <= MAX_PENDING_ASSET_BYTES)
        .ok_or("The native song needs more than 256 MiB of imported assets.")?;
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
        let (app_id, count, width) = profile(product).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&app_id.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&bpm.to_le_bytes());
        bytes.extend_from_slice(&effective.to_le_bytes());
        text_block(&mut bytes, b"Composer\0#SKKENNUNG#:0000000\0\x01");
        text_block(&mut bytes, b"Fixture title\0Mixer text");
        for _ in 0..count {
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
        let root = std::env::temp_dir().join(format!(
            "counted-wav-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
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
            let gain = match product {
                Product::T3 => native_fader_gain(16384, 1.33),
                Product::D2 => native_fader_gain(16384, 1.25),
                _ => 16384.0 / 32767.0,
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
            let size_offset = 2 + path.len() + 2;
            bytes[size_offset] ^= 1;
            assert!(embedded_waves(&bytes).unwrap().is_empty());
        }
        assert!(embedded_waves(&wave).unwrap().is_empty());
    }

    #[test]
    fn preserved_audio_never_overwrites_an_existing_file() {
        let directory = std::env::temp_dir().join(format!(
            "mixjam-techno-chunk-audio-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
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
                reference: Reference::Wave(source_path.into()),
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
            origin: Some(Product::D2),
        };
        crate::engine::validate_import_metadata(&sample).unwrap();
        let project = Project {
            imported_samples: vec![sample],
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
            bytes: bytes.clone(),
            frames: 4,
            sample_rate: 44100,
        };
        let wave =
            source_wave("c:/ORIGINAL/kick.wav", &[same(), same()], &Library::empty()).unwrap();
        assert_eq!(wave.bytes, bytes);
        let different = Wave {
            source_path: "C:/original/KICK.wav".into(),
            bytes: b"different".to_vec(),
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
        let directory = std::env::temp_dir().join(format!(
            "mixjam-techno-wave-identity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
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
        for entry in fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mix"))
            {
                continue;
            }
            songs += 1;
            let bytes = read_bounded(&path).unwrap();
            let header = read_header(&bytes, Product::D2).unwrap();
            let timeline = read_timeline(&bytes, header.after_mixer, Product::D2).unwrap();
            let kit =
                drums::parse_kit(&bytes, header.after_mixer, timeline.start, Product::D2).unwrap();
            assert_eq!(kit.pads.len(), 10);
            assert_eq!(kit.source_end + 18, timeline.start);
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
                    || path.file_name().unwrap() == ".mix"
                {
                    continue;
                }
                let bytes = read_bounded(&path).unwrap();
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
            (Product::D2, "D2/MIX", (13, 13, 0)),
            (Product::D3, "MIX", (17, 17, 0)),
            (Product::D4, "Mix", (12, 12, 0)),
            (Product::H1, "Mix", (14, 14, 0)),
            (Product::T3, "Mix", (20, 20, 0)),
        ];
        for (product, mix_folder, expected) in cases {
            let installation = crate::project_io::default_root(product)
                .expect("The installed song fixtures are missing.");
            let folder = installation.join(mix_folder);
            if product == Product::D4 {
                let placeholder = folder.join(".mix");
                assert_eq!(fs::metadata(&placeholder).unwrap().len(), 2);
                assert!(read_header(&read_bounded(&placeholder).unwrap(), product).is_err());
            }
            let mut library = Library::empty();
            library
                .register(product, &installation, Path::new(""))
                .unwrap();
            let mut parsed = 0;
            let mut malformed = 0;
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
                let bytes = read_bounded(&path).unwrap();
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
                    Err(_)
                        if product == Product::D4
                            && path.file_name().is_some_and(|name| name == ".mix")
                            && bytes.len() == 2 =>
                    {
                        malformed += 1;
                    }
                    Err(error) => failures.push(format!("{}: {error}", path.display())),
                }
            }
            eprintln!(
                "{}: {parsed} complete projects, {malformed} explicit malformed rejections",
                product.label()
            );
            assert!(failures.is_empty(), "{}", failures.join("\n"));
            assert_eq!((parsed, malformed), (expected.1, expected.2));
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
