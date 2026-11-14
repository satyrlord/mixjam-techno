//! Preserved audio files: staging and publishing the WAV assets that an import
//! keeps beside the project, and restoring a missing imported sample from them.

use super::waves::{d2_source_wave, wave_info};
use crate::{
    compat::drums,
    engine::{ImportedSample, MissingReference, MissingSample},
    library::Library,
    products::Product,
};
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// The audio that one song import keeps pending, in bytes: as much as the
/// song file itself may hold.
pub(super) const MAX_PENDING_ASSET_BYTES: usize = super::super::song_file::MIX_BYTE_LIMIT;

pub(super) static NEXT_ASSET_TEMP_ID: AtomicU64 = AtomicU64::new(0);

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

pub(super) fn encode_float_wave(pcm: &crate::audio::Pcm) -> Result<Vec<u8>, String> {
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

pub(super) fn queue_asset<'a>(
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

pub(super) fn preserve_wave(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.try_exists().map_err(|error| error.to_string())? {
        return verify_existing_asset(path, bytes);
    }
    let temporary = stage_asset(path, bytes)?;
    publish_staged_asset(&temporary, path, bytes)
}

pub(super) fn stage_asset(path: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
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
pub(super) fn publish_staged_asset(
    temporary: &Path,
    path: &Path,
    bytes: &[u8],
) -> Result<(), String> {
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

pub(super) fn publish_asset_directly(path: &Path, bytes: &[u8]) -> Result<(), String> {
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
