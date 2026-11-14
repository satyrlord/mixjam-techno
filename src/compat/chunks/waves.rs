//! The audio that a song carries or names: embedded wave records, the
//! external and packed sources they resolve to, and the stretched copies that
//! some products store.

use super::{
    Reader,
    assets::{encode_float_wave, queue_asset},
    layout::{SENTINEL, WaveReference, first_text},
};
use crate::{
    engine::{MAX_TICKS, TICKS_PER_BAR},
    library::Library,
    products::Product,
};
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

/// How many embedded wave records a song may hold. The songs are read from
/// at most `MIX_BYTE_LIMIT` bytes and each record takes more than a few bytes,
/// so a real song is far below this; the bound stops a damaged count from
/// reserving memory for records that are not there.
const MAX_EMBEDDED_WAVES: usize = 100_000;

/// An external audio file's bytes. A WAV file cannot be longer than the
/// format's limit, so the read stops there and reports a longer file. A
/// packed file is checked against its own limit when it is decoded.
pub(super) fn read_external_wave(path: &Path) -> Result<Vec<u8>, String> {
    const MAX_BYTES: u64 = crate::wav::MAX_WAV_FILE_BYTES;
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > MAX_BYTES {
        return Err("The external audio file is larger than 4 GiB.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("The external audio file is larger than 4 GiB.".into());
    }
    Ok(bytes)
}

pub(super) fn read_wave_extension(
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
    // House saves this layout too when the path is framed and two nonempty
    // framed labels follow, as in the album song "Salon": the file name, then
    // its original path. Its 12-byte `02 00 00 01` extension starts like an
    // empty label, so empty labels do not select this layout.
    let framed_labels_follow = || {
        let mut peek = Reader {
            bytes: reader.bytes,
            offset: reader.offset,
        };
        (0..2).all(|_| {
            peek.block()
                .is_ok_and(|label| (3..=262).contains(&label.len()) && label.ends_with(&[0, 1]))
        }) && peek.take(8).is_ok()
    };
    // T3 saves this layout for every imported WAV, as D3 does: the album
    // songs "Infinitrip" and "Storm" save `0c 00 09 00 00 00 00 00` and
    // `0c 00 20 00 00 00 00 00` after the labels. Read as an optional text
    // token, a bar count of 9 (a tab) or 32 to 126 (printable) consumed six
    // bytes of the next event, whose sample ID's high byte (39 or 41) then
    // read as an unsupported source. D4 saves it too: all 282 records in
    // seven saved songs.
    let labelled = match product {
        Product::D2 | Product::D3 | Product::D4 | Product::T3 => true,
        Product::H1 => framed_labels_follow(),
        _ => false,
    };
    if labelled && raw.ends_with(&[0, 1]) {
        // D2, D3, D4, and T3 save two labels followed by eight bytes of source metadata.
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

pub(super) struct Wave<'a> {
    pub(super) source_path: String,
    pub(super) bytes: Cow<'a, [u8]>,
    pub(super) frames: u32,
    pub(super) sample_rate: u32,
}

#[derive(Clone, Debug)]
pub(super) struct RetainedWave {
    pub(super) path: PathBuf,
    pub(super) frames: u32,
    pub(super) sample_rate: u32,
}

pub(super) fn retain_wave<'a>(
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

pub(super) fn wave_info(bytes: &[u8]) -> Result<(u32, u32), String> {
    let mut reader = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|error| format!("The imported WAV is invalid: {error}"))?;
    let spec = reader.spec();
    if !matches!(spec.channels, 1 | 2)
        || spec.sample_rate == 0
        || spec.sample_rate > 384000
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

pub(super) fn embedded_wave_records(bytes: &[u8]) -> Result<Vec<EmbeddedSource<'_>>, String> {
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

/// Framed WAV payloads are audio, even when their bytes resemble a timeline.
pub(crate) fn embedded_wave_ranges(bytes: &[u8]) -> Result<Vec<std::ops::Range<usize>>, String> {
    Ok(embedded_wave_records(bytes)?
        .into_iter()
        .map(|source| source.range)
        .collect())
}

pub(super) fn embedded_waves(bytes: &[u8]) -> Result<Vec<Wave<'_>>, String> {
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

pub(super) fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

pub(super) fn source_wave<'a>(
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

pub(super) struct StretchedCopy {
    pub(super) wave: Wave<'static>,
    pub(super) name: String,
    pub(super) original: serde_json::Value,
    pub(super) target_bpm: f64,
}

/// The T2 editor's Time Stretcher (editor strings `TimeStretchOrginal`,
/// `TimeStretchZiel`, `TIMESAV`) saves its result as `MIXWAVES\timeNNN.wav`.
/// The song labels such a copy with its original: label 1 is the file name,
/// label 2 its folder followed by `[time]`, as in `R1LA037.WAV` and
/// `F:\samples\Rave\LOOP\-1[time]`. When the copy is gone, the original
/// library sound is stretched from its tempo to fill its saved bars at the
/// song tempo, keeping its pitch. The original stretch algorithm is not
/// recovered; MixJam's own stretcher stands in for it. A name that matches no
/// library sound, or several, leaves the copy missing.
pub(super) fn stretched_copy(
    metadata: &BTreeMap<String, String>,
    library: &Library,
    song_bpm: f64,
    declared_ticks: Option<u32>,
) -> Result<Option<StretchedCopy>, String> {
    let (Some(file), Some(folder)) = (
        metadata.get("Native label 1"),
        metadata.get("Native label 2"),
    ) else {
        return Ok(None);
    };
    let Some((stem, _)) = file
        .rsplit_once('.')
        .filter(|(_, extension)| extension.eq_ignore_ascii_case("wav"))
    else {
        return Ok(None);
    };
    if !folder.to_ascii_lowercase().ends_with("[time]") || stem.is_empty() {
        return Ok(None);
    }
    // Library codes keep the original file name, as in `bn/r1la037.pxd`.
    let code_stem = |code: &str| {
        let name = basename(code);
        name.rsplit_once('.')
            .map_or(name, |(stem, _)| stem)
            .to_string()
    };
    let mut matches = library
        .samples
        .iter()
        .filter(|sample| code_stem(&sample.code).eq_ignore_ascii_case(stem));
    let (Some(sample), None) = (matches.next(), matches.next()) else {
        return Ok(None);
    };
    let bars = declared_ticks.map_or(sample.bars, |ticks| ticks / TICKS_PER_BAR);
    if sample.bars == 0 || bars == 0 {
        return Ok(None);
    }
    let target_bpm = song_bpm * f64::from(sample.bars) / f64::from(bars);
    let pcm = library.read_original_pcm(sample.id)?;
    let stretched = crate::sample_edit::time_stretch(&pcm, sample.source_bpm, target_bpm)?;
    let frames =
        u32::try_from(stretched.frames()).map_err(|_| "The stretched sound is too long.")?;
    Ok(Some(StretchedCopy {
        wave: Wave {
            source_path: file.clone(),
            bytes: Cow::Owned(encode_float_wave(&stretched)?),
            frames,
            sample_rate: stretched.sample_rate,
        },
        name: format!("{} {target_bpm} BPM", sample.name),
        original: serde_json::json!({
            "id": sample.id, "code": &sample.code, "name": &sample.name,
            "bars": sample.bars, "source_bpm": sample.source_bpm,
        }),
        target_bpm,
    }))
}

pub(super) fn normalize_saved_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

pub(super) struct EmbeddedSource<'a> {
    pub(super) path: String,
    pub(super) bytes: &'a [u8],
    pub(super) range: std::ops::Range<usize>,
}

pub(super) fn d2_embedded_sources(
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

pub(super) fn packed_wave(path: &str, bytes: &[u8]) -> Result<Wave<'static>, String> {
    if bytes.len() as u64 > legacy_format::packed::MAX_PACKED_BYTES || !bytes.starts_with(b"tPxD") {
        return Err("The imported PXD header or file size is invalid.".into());
    }
    let decoded = legacy_format::packed::decode(bytes, legacy_format::packed::DecodeOptions::FILE)
        .map_err(|error| format!("Cannot decode imported PXD {path}: {error}"))?;
    let samples = decoded.samples.as_slice();
    // A packed file holds at most 64 MiB, which decodes to a fraction of what a
    // WAV file can hold, so the decoded audio always fits.
    if samples.is_empty() {
        return Err("The imported PXD has no audio.".into());
    }
    // Retain the source's exact PCM16 values.
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 1,
                sample_rate: legacy_format::NATIVE_SAMPLE_RATE,
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
        sample_rate: legacy_format::NATIVE_SAMPLE_RATE,
        bytes: Cow::Owned(cursor.into_inner()),
    })
}

pub(super) fn d2_source_wave<'a>(
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
    let pcm = library.read_original_pcm(sample.id)?;
    Ok(Wave {
        source_path: path.into(),
        frames: u32::try_from(pcm.frames()).map_err(|_| "The imported PXD is too long.")?,
        sample_rate: pcm.sample_rate,
        bytes: Cow::Owned(encode_float_wave(&pcm)?),
    })
}
