//! Sound files that a song keeps beside the data folder: manual choices and native provider folders.
use super::reference_samples_root;
use crate::{library::Library, products::Product};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

/// Retain an explicit manual choice independently of later palette path matches.
/// The file may be as long as a WAV can be, which is the format's limit.
pub(crate) fn retain_manual_wave(path: &Path, root: &Path) -> Result<PathBuf, String> {
    retain_manual_wave_within(path, root, crate::wav::MAX_WAV_FILE_BYTES)
}

/// `retain_manual_wave` for a file of at most `max_bytes`.
pub(super) fn retain_manual_wave_within(
    path: &Path,
    root: &Path,
    max_bytes: u64,
) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let source = fs::File::open(path).map_err(|error| error.to_string())?;
    if source.metadata().map_err(|error| error.to_string())?.len() > max_bytes {
        return Err("The selected WAV is larger than 4 GiB.".into());
    }
    let mut source = source.take(max_bytes.saturating_add(1));
    let folder = root.join("manual-samples");
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let (temporary, mut target) = crate::atomic_file::sibling(&folder.join("manual"), "tmp")?;
    let result = (|| {
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut copied = 0u64;
        loop {
            let count = source
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            copied += count as u64;
            if copied > max_bytes {
                return Err("The selected WAV is larger than 4 GiB.".into());
            }
            target
                .write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
            hash.update(&buffer[..count]);
        }
        target.sync_all().map_err(|error| error.to_string())?;
        drop(target);
        // Validate the retained bytes, including changes made during the source copy.
        crate::audio::visit_wav_samples(&temporary, |_, _| {})?;
        let digest = hash.finalize();
        let retained = folder.join(format!("{digest:x}.wav"));
        if retained.exists() {
            let mut existing = fs::File::open(&retained).map_err(|error| error.to_string())?;
            let mut existing_hash = Sha256::new();
            loop {
                let count = existing
                    .read(&mut buffer)
                    .map_err(|error| error.to_string())?;
                if count == 0 {
                    break;
                }
                existing_hash.update(&buffer[..count]);
            }
            if existing_hash.finalize() != digest {
                return Err(
                    "The retained manual sample has changed. Restore that file before you \
                     continue."
                        .into(),
                );
            }
        } else {
            fs::rename(&temporary, &retained).map_err(|error| error.to_string())?;
        }
        fs::canonicalize(retained).map_err(|error| error.to_string())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(temporary);
    }
    result
}

/// A product split across volumes has no single folder that a format-18
/// binding can name. Retain its used WAV bytes together, and verify their
/// native identities before the song can refer to that folder.
pub(super) fn retain_native_provider(
    product: Product,
    mut files: Vec<(u32, &Path)>,
    storage: &Path,
) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);
    struct KeptWave {
        id: u32,
        relative: PathBuf,
        hash: [u8; 32],
        pcm: [u8; 32],
        bpm: f64,
        spec: hound::WavSpec,
        frames: u32,
    }

    let parent = storage.join("native-providers").join(product.slug());
    fs::create_dir_all(&parent).map_err(|error| error.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let stage = parent.join(format!(
        ".stage-{}-{stamp}-{}",
        std::process::id(),
        NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&stage).map_err(|error| error.to_string())?;
    let result = (|| {
        files.sort_by_key(|(id, _)| *id);
        let mut digest = Sha256::new();
        digest.update(product.slug().as_bytes());
        let mut expected = Vec::with_capacity(files.len());
        for (id, source) in files {
            let (_, native_id) = Product::sample_key(id).ok_or("Invalid native sample ID.")?;
            let entry = crate::catalog::identity(product, native_id)
                .ok_or("The native sample has no catalog identity.")?;
            let code = entry.code.replace('\\', "/");
            let name = Path::new(&code)
                .file_stem()
                .filter(|name| !name.is_empty())
                .ok_or("The native sample has no catalog file name.")?;
            let relative = PathBuf::from(id.to_string())
                .join(name)
                .with_extension("wav");
            let destination = stage.join(&relative);
            fs::create_dir_all(destination.parent().unwrap()).map_err(|error| error.to_string())?;
            let bytes = bounded_provider_wave(source)?;
            let hash: [u8; 32] = Sha256::digest(&bytes).into();
            crate::atomic_file::write_new(&destination, &bytes)?;
            let (spec, frames, pcm) = provider_wave_audio(&destination)?;
            digest.update(id.to_le_bytes());
            let name = relative.to_string_lossy();
            digest.update((name.len() as u64).to_le_bytes());
            digest.update(name.as_bytes());
            digest.update(hash);
            expected.push(KeptWave {
                id,
                relative,
                hash,
                pcm,
                bpm: entry.source_bpm,
                spec,
                frames,
            });
        }
        let check = |root: &Path| -> Result<(), String> {
            let mut retained = Library::empty();
            retained.register(product, root, &reference_samples_root())?;
            for wave in &expected {
                let id = wave.id;
                let exact: [u8; 32] =
                    Sha256::digest(bounded_provider_wave(&root.join(&wave.relative))?).into();
                if exact != wave.hash {
                    return Err(format!("The retained bytes for sample {id} do not match."));
                }
                let sample = retained
                    .sample_by_id(id)
                    .ok_or_else(|| format!("The retained provider cannot resolve sample {id}."))?;
                if sample.source_bpm != wave.bpm
                    || sample.sample_rate != wave.spec.sample_rate
                    || sample.channels != wave.spec.channels
                    || sample.frame_count != wave.frames
                {
                    return Err(format!(
                        "The retained timing for sample {id} does not match."
                    ));
                }
                let file = sample
                    .wav_path
                    .as_deref()
                    .ok_or("The retained WAV has no path.")?;
                // Catalog aliases can choose another byte container with
                // identical PCM. Preserve each copy, but judge the mapping
                // by the audio it plays and the timing checked above.
                if provider_wave_audio(file)?.2 != wave.pcm {
                    return Err(format!(
                        "The retained audio for sample {id} does not match."
                    ));
                }
            }
            Ok(())
        };
        check(&stage)?;
        let target = parent.join(format!("{:x}", digest.finalize()));
        if !target.exists()
            && let Err(error) = fs::rename(&stage, &target)
            && !target.is_dir()
        {
            return Err(error.to_string());
        }
        // A prior save or another writer can already have this content. Do
        // not replace it; verify it before reusing its stable path.
        check(&target)?;
        fs::canonicalize(target).map_err(|error| error.to_string())
    })();
    if stage.exists() {
        let _ = fs::remove_dir_all(stage);
    }
    result
}

/// A provider WAV's bytes. A WAV file cannot be longer than the format's
/// limit, so the read stops there and reports a longer file.
fn bounded_provider_wave(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(crate::wav::MAX_WAV_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > crate::wav::MAX_WAV_FILE_BYTES {
        return Err("The provider WAV is larger than 4 GiB.".into());
    }
    Ok(bytes)
}

pub(super) fn provider_wave_audio(path: &Path) -> Result<(hound::WavSpec, u32, [u8; 32]), String> {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    let (spec, frames) = crate::audio::visit_wav_samples(path, |_, sample| {
        digest.update(sample.to_bits().to_le_bytes());
    })?;
    Ok((spec, frames, digest.finalize().into()))
}
