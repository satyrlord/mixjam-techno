//! Transactional project opening and local installation registrations.
use crate::{
    engine::{ImportedSample, MissingReference, MissingSample, Project, TICKS_PER_BAR},
    library::Library,
    products::{LibraryBinding, Product},
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub fn data_root() -> PathBuf {
    std::env::var_os("MIXJAM_TECHNO_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("MixJam Techno"))
        })
        .unwrap_or_else(|| std::env::temp_dir().join("mixjam-techno"))
}

/// Retain an explicit manual choice independently of later palette path matches.
pub(crate) fn retain_manual_wave(path: &Path, root: &Path) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    const MAX_BYTES: u64 = 128 * 1024 * 1024;
    let source = fs::File::open(path).map_err(|error| error.to_string())?;
    if source.metadata().map_err(|error| error.to_string())?.len() > MAX_BYTES {
        return Err("The selected WAV exceeds the 128 MiB limit.".into());
    }
    let mut source = source.take(MAX_BYTES + 1);
    let folder = root.join("manual-samples");
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    static WRITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let temporary = folder.join(format!(
        ".{}-{stamp}-{}.tmp",
        std::process::id(),
        WRITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let result = (|| {
        let mut target = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
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
            if copied > MAX_BYTES {
                return Err("The selected WAV exceeds the 128 MiB limit.".into());
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

/// The library folders that the sound browser merges, in order, and the
/// fallback tempo of their scans. Stored in `sample-folder.json`.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct WaveFolderConfig {
    pub roots: Vec<PathBuf>,
    pub source_bpm: f64,
}

const MAX_WAVE_CONFIG_BYTES: u64 = 16 * 1024;

/// `sample-folder.json` as written by every version: earlier versions wrote
/// one `root`, later ones a `roots` list.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredWaveConfig {
    root: Option<PathBuf>,
    roots: Option<Vec<PathBuf>>,
    source_bpm: f64,
}

pub fn load_wave_config() -> Result<Option<WaveFolderConfig>, String> {
    load_wave_config_in(&data_root())
}

/// Read the library folders. A file with the earlier single `root` reads as
/// a list of that one folder; the next save writes the list.
pub fn load_wave_config_in(data_root: &Path) -> Result<Option<WaveFolderConfig>, String> {
    let path = data_root.join("sample-folder.json");
    if !path.exists() {
        return Ok(None);
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_WAVE_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_WAVE_CONFIG_BYTES {
        return Err("The sample folder settings exceed the size limit.".into());
    }
    let stored: StoredWaveConfig = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Cannot read sample folder settings: {e}"))?;
    let roots = match (stored.root, stored.roots) {
        (Some(root), None) => vec![root],
        (None, Some(roots)) => roots,
        _ => {
            return Err(
                "The sample folder settings need either one root or a list of roots.".into(),
            );
        }
    };
    if roots.iter().any(|root| !root.is_absolute())
        || !stored.source_bpm.is_finite()
        || stored.source_bpm <= 0.0
    {
        return Err(
            "The sample folder settings need absolute paths and a positive source tempo.".into(),
        );
    }
    crate::library::check_library_folders(&roots)
        .map_err(|error| format!("Cannot use the sample folder settings: {error}"))?;
    Ok(Some(WaveFolderConfig {
        roots,
        source_bpm: stored.source_bpm,
    }))
}

/// The probe cache of earlier folder scans. A missing, oversized, or
/// unreadable cache is an empty one: scans then decode every file again.
pub fn load_scan_cache(data_root: &Path) -> crate::library::ScanCache {
    let path = data_root.join("wav-scan-cache.json");
    let Ok(file) = fs::File::open(path) else {
        return Default::default();
    };
    let mut bytes = Vec::new();
    // About 260 bytes an entry: room for every entry that the cache keeps.
    const LIMIT: u64 = 1024 * 1024 * 1024;
    if file.take(LIMIT + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > LIMIT {
        return Default::default();
    }
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save_scan_cache(data_root: &Path, cache: &crate::library::ScanCache) -> Result<(), String> {
    let bytes = serde_json::to_vec(cache).map_err(|e| e.to_string())?;
    fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    crate::atomic_file::write(&data_root.join("wav-scan-cache.json"), &bytes)
}

/// Open library folders through the probe cache in `data_root`. The probes
/// of a failed or cancelled scan are kept for the next one; a cache that
/// cannot be written only costs the next scan time.
pub fn open_wav_folders_cached(
    data_root: &Path,
    roots: &[PathBuf],
    source_bpm: f64,
    progress: Option<&crate::library::ScanProgress>,
) -> Result<Library, String> {
    let mut cache = load_scan_cache(data_root);
    let result = Library::open_wav_folders_with(roots, source_bpm, progress, Some(&mut cache));
    if cache.has_changes() {
        let _ = save_scan_cache(data_root, &cache);
    }
    result
}

/// Save the library folders, in order. A folder that exists is saved as its
/// canonical path; one that is unavailable now keeps its absolute path.
pub fn save_wave_config_in(
    data_root: &Path,
    roots: &[PathBuf],
    source_bpm: f64,
) -> Result<(), String> {
    if !source_bpm.is_finite() || source_bpm <= 0.0 {
        return Err("The source tempo must be finite and positive.".into());
    }
    crate::library::check_library_folders(roots)?;
    let roots = roots
        .iter()
        .map(|root| {
            fs::canonicalize(root)
                .or_else(|_| std::path::absolute(root))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    crate::library::check_library_folders(&roots)?;
    let bytes = serde_json::to_vec_pretty(&WaveFolderConfig { roots, source_bpm })
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_WAVE_CONFIG_BYTES {
        return Err("The sample folder settings exceed the size limit.".into());
    }
    fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    crate::atomic_file::write(&data_root.join("sample-folder.json"), &bytes)
}

/// The folder of the song last chosen in a file dialog, while that folder exists.
pub fn load_song_folder(data_root: &Path) -> Option<PathBuf> {
    let file = fs::File::open(data_root.join("song-folder.json")).ok()?;
    let mut bytes = Vec::new();
    file.take(1024 * 1024).read_to_end(&mut bytes).ok()?;
    let folder: PathBuf = serde_json::from_slice(&bytes).ok()?;
    folder.is_dir().then_some(folder)
}

pub fn save_song_folder(data_root: &Path, folder: &Path) -> Result<(), String> {
    fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(folder).map_err(|e| e.to_string())?;
    crate::atomic_file::write(&data_root.join("song-folder.json"), &bytes)
}

/// An optional folder of extracted WAVs with a `metadata.json`: `samples`
/// beside the executable, or the folder that `MIXJAM_TECHNO_REFERENCE_SAMPLES`
/// names. A provider registers without it.
pub fn reference_samples_root() -> PathBuf {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("samples")))
        .unwrap_or_default();
    [
        Some(beside.clone()),
        std::env::var_os("MIXJAM_TECHNO_REFERENCE_SAMPLES").map(PathBuf::from),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.join("metadata.json").is_file())
    .unwrap_or(beside)
}

pub fn load_library_config() -> Result<Vec<LibraryBinding>, String> {
    const LIMIT: u64 = 1024 * 1024;
    let file = match fs::File::open(data_root().join("libraries.json")) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Cannot read the library settings: {error}")),
    };
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read the library settings: {error}"))?;
    if bytes.len() as u64 > LIMIT {
        return Err("The library settings exceed the 1 MiB limit.".into());
    }
    let bindings: Vec<LibraryBinding> = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Cannot read the library settings: {e}"))?;
    validate_library_config(&bindings)?;
    Ok(bindings)
}

fn validate_library_config(bindings: &[LibraryBinding]) -> Result<(), String> {
    let mut products = HashSet::new();
    if bindings.len() > Product::ALL.len()
        || bindings
            .iter()
            .any(|b| !products.insert(b.product) || !b.root.is_absolute())
    {
        return Err("Library settings require distinct products and absolute folder paths.".into());
    }
    Ok(())
}

pub fn save_library_config(bindings: &[LibraryBinding]) -> Result<(), String> {
    validate_library_config(bindings)?;
    fs::create_dir_all(data_root()).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(bindings).map_err(|e| e.to_string())?;
    crate::atomic_file::write(&data_root().join("libraries.json"), &bytes)
}

/// Original installations for the ignored fixture tests. The application has no default provider.
#[cfg(test)]
pub(crate) fn default_root(product: Product) -> Option<PathBuf> {
    let installations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("local-data/installations");
    let media = std::env::var_os("MIXJAM_TECHNO_MEDIA_ROOT").map(PathBuf::from);
    let (local, setup) = match product {
        Product::D1 => ("\x64ance1", "\x65jay_\x64ance1_setup"),
        Product::D2 => ("\x64ance2", "\x65jay_\x64ance2_setup"),
        Product::D3 => ("\x64ance3", "\x65jay_\x64ance3_setup"),
        Product::D4 => ("\x64ance4", "\x65jay_\x64ance4_setup"),
        Product::H1 => ("\x68ouse", "\x65jay_\x68ouse_setup"),
        Product::T1 => ("\x72ave", "\x65jay_\x74echno1_setup"),
        Product::T2 => ("\x74echno2", "\x65jay_\x54echno2_setup"),
        Product::T3 => ("\x74echno3", "\x65jay_\x74echno3_setup"),
    };
    [
        Some(installations.join(local)),
        media.map(|media| media.join(setup)),
    ]
    .into_iter()
    .flatten()
    .find(|root| root.is_dir())
}

pub fn open(path: &Path, library: &Library) -> Result<(Project, Library), String> {
    open_song(path, library).map(|(project, library, _)| (project, library))
}

/// Open a song, a `.mix` or v7 `.mixjam` import, or an earlier JSON project.
/// The flag says whether Save can replace the file: a `.mixjam` song of format
/// version 8 or later. Imports and earlier JSON projects need a new `.mixjam` path.
pub fn open_song(path: &Path, library: &Library) -> Result<(Project, Library, bool), String> {
    let candidate = library.clone();
    let (project, song) = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mix"))
    {
        let product = detect_product(path)?;
        let mut project = match product {
            Product::T2 => crate::legacy_mix::load(path, &candidate)?,
            Product::T1 => crate::compat::t1::load(path, &candidate)?,
            Product::D1 => crate::compat::d1::load(path, &candidate)?,
            _ => crate::compat::chunks::load(path, product, &candidate)?,
        };
        project.libraries = candidate.bindings().to_vec();
        (project, false)
    } else if has_song_extension(path) {
        crate::mixjam::load_song(path, &candidate)?
    } else {
        (Project::load(path)?, false)
    };
    let (project, library) = resolve(project, &candidate)?;
    Ok((project, library, song))
}

pub fn resolve(mut project: Project, library: &Library) -> Result<(Project, Library), String> {
    project.validate()?;
    project.prune_sample_conflicts();
    let mut candidate = library.clone();
    let used: HashSet<_> = project.clips.iter().map(|clip| clip.sample_id).collect();
    project
        .imported_samples
        .retain(|sample| used.contains(&sample.id));
    project
        .missing_samples
        .retain(|sample| used.contains(&sample.id));
    for binding in project.libraries.clone() {
        // A current provider of the whole product takes precedence over the
        // folder saved in the song. A sample folder without the product, or
        // with only part of it, does not hide the saved folder.
        let sparse = candidate.is_sparse_wav_product(binding.product);
        if candidate.source_root(binding.product).is_some() && !sparse {
            continue;
        }
        if !project.clips.iter().any(|clip| {
            Product::sample_key(clip.sample_id)
                .is_some_and(|(product, _)| product == binding.product)
        }) {
            continue;
        }
        if let Ok(root) = fs::canonicalize(&binding.root) {
            // Unavailable providers do not prevent opening the arrangement.
            if !sparse {
                let _ = candidate.register(binding.product, &root, &reference_samples_root());
                continue;
            }
            // The partial copy stays when the saved folder cannot replace it.
            let mut provided = candidate.clone();
            provided.remove_product(binding.product);
            if provided
                .register(binding.product, &root, &reference_samples_root())
                .is_ok()
            {
                // The saved provider may itself be partial. Keep identities
                // supplied by the selected folders that it cannot replace.
                provided.supplement_loaded(binding.product, &candidate);
                candidate = provided;
            }
        }
    }
    let mut mixjam_references = crate::mixjam::ReferenceResolver::new(&candidate);
    let mut checked_imports = HashMap::new();
    let imports = std::mem::take(&mut project.imported_samples);
    for imported in imports {
        let mut resolved = imported.clone();
        let selected = conflict_selection(&project, imported.id);
        let rebased = rebase_asset(
            &mut resolved,
            &candidate,
            &mut mixjam_references,
            selected.as_deref(),
        );
        if let Ok(paths) = &rebased {
            crate::library::record_wav_conflict(
                &mut project,
                &imported.path.to_string_lossy(),
                imported.id,
                paths.clone(),
            );
        }
        match rebased.and_then(|_| Library::check_import(&resolved)) {
            Ok(ready) => {
                if let Some(previous_rate) = resolved.source_sample_rate
                    && previous_rate != ready.sample_rate
                {
                    for clip in project
                        .clips
                        .iter_mut()
                        .filter(|clip| clip.sample_id == resolved.id)
                    {
                        let offset = (f64::from(clip.offset_frames) * f64::from(ready.sample_rate)
                            / f64::from(previous_rate))
                        .round();
                        if offset > f64::from(u32::MAX) {
                            return Err(
                                "The restored sample trim exceeds the supported frame range."
                                    .into(),
                            );
                        }
                        clip.offset_frames = offset as u32;
                    }
                }
                resolved.source_sample_rate = Some(ready.sample_rate);
                checked_imports.insert(resolved.id, ready);
                project.imported_samples.push(resolved);
            }
            Err(reason) => {
                let known = library
                    .sample_by_id(imported.id)
                    .filter(|sample| sample.wav_path.as_deref() == Some(imported.path.as_path()));
                let (ticks, rate, inferred) = sample_timing(
                    &project,
                    known,
                    imported.id,
                    imported.source_bpm,
                    imported.source_sample_rate,
                );
                let mut missing = MissingSample::external(&imported, ticks, reason);
                missing.sample_rate = rate;
                missing.length_inferred = inferred;
                if known.is_none() && imported.source_sample_rate.is_none() {
                    missing
                        .metadata
                        .insert("Trim rate policy".into(), "preserve-frames".into());
                    missing.metadata.insert(
                        "Sample rate confidence".into(),
                        "The original frame rate is unknown. The 44100 Hz estimate only sets the \
                         displayed duration; resolution preserves the original frame offset."
                            .into(),
                    );
                }
                project.missing_samples.push(missing);
            }
        }
    }
    let missing = std::mem::take(&mut project.missing_samples);
    for mut sample in missing {
        let selected = conflict_selection(&project, sample.id);
        let result = match &sample.reference {
            MissingReference::Native { .. } => {
                if candidate
                    .sample_by_id(sample.id)
                    .is_some_and(|ready| ready.source_bpm != sample.source_bpm)
                {
                    Err("The library sample tempo does not match the retained source.".into())
                } else {
                    candidate
                        .read_pcm(sample.id)
                        .map(|pcm| (None, pcm.sample_rate))
                }
            }
            MissingReference::External { path, origin } => {
                let restored = crate::compat::chunks::restore_missing(&sample, &candidate);
                restored.and_then(|restored| {
                    let mut imported = restored.unwrap_or_else(|| ImportedSample {
                        id: sample.id,
                        name: sample.name.clone(),
                        group_name: sample
                            .metadata
                            .get("group_name")
                            .cloned()
                            .unwrap_or_else(|| "Wave".into()),
                        path: path.clone(),
                        source_bpm: sample.source_bpm,
                        source_sample_rate: None,
                        origin: *origin,
                    });
                    let paths = rebase_asset(
                        &mut imported,
                        &candidate,
                        &mut mixjam_references,
                        selected.as_deref(),
                    )?;
                    crate::library::record_wav_conflict(
                        &mut project,
                        &path.to_string_lossy(),
                        sample.id,
                        paths,
                    );
                    if sample
                        .metadata
                        .get("Playback timing policy")
                        .is_some_and(|policy| policy == "mixjam-span")
                        && let Some(conflict) = project
                            .sample_conflicts
                            .iter_mut()
                            .find(|conflict| conflict.sample_ids.contains(&sample.id))
                    {
                        conflict
                            .source_spans
                            .insert(sample.id, sample.nominal_ticks);
                    }
                    let ready = Library::check_import(&imported)?;
                    imported.source_bpm =
                        sample.resolved_source_bpm(ready.frame_count, ready.sample_rate)?;
                    imported.source_sample_rate = Some(ready.sample_rate);
                    let rate = ready.sample_rate;
                    checked_imports.insert(imported.id, ready);
                    Ok((Some(imported), rate))
                })
            }
            MissingReference::Generated { .. } => {
                crate::compat::chunks::restore_missing(&sample, &candidate).and_then(|imported| {
                    let mut imported = imported.ok_or_else(|| sample.reason.clone())?;
                    let ready = Library::check_import(&imported)?;
                    imported.source_bpm =
                        sample.resolved_source_bpm(ready.frame_count, ready.sample_rate)?;
                    imported.source_sample_rate = Some(ready.sample_rate);
                    let rate = ready.sample_rate;
                    checked_imports.insert(imported.id, ready);
                    Ok((Some(imported), rate))
                })
            }
        };
        match result {
            Ok((imported, rate)) => {
                let resolved_bpm = imported
                    .as_ref()
                    .map_or(sample.source_bpm, |source| source.source_bpm);
                for clip in project
                    .clips
                    .iter_mut()
                    .filter(|clip| clip.sample_id == sample.id)
                {
                    let offset = if clip.offset_frames == 0
                        || sample
                            .metadata
                            .get("Trim rate policy")
                            .is_some_and(|policy| policy == "preserve-frames")
                    {
                        f64::from(clip.offset_frames)
                    } else {
                        (f64::from(clip.offset_frames) * f64::from(rate)
                            / f64::from(sample.sample_rate)
                            * (sample.source_bpm / resolved_bpm))
                            .round()
                    };
                    if !offset.is_finite() || !(0.0..=f64::from(u32::MAX)).contains(&offset) {
                        return Err(
                            "The restored sample trim exceeds the supported frame range.".into(),
                        );
                    }
                    clip.offset_frames = offset as u32;
                }
                if let Some(imported) = imported {
                    project.imported_samples.push(imported);
                }
            }
            Err(reason) => {
                // Keep detailed importer diagnostics when this provider has no source.
                if !matches!(sample.reference, MissingReference::Native { .. }) {
                    sample.reason = reason;
                }
                project.missing_samples.push(sample);
            }
        }
    }
    // Keep the selected folder's palette. Saved imports retain their IDs; unused
    // palette entries receive fresh IDs if the saved project uses those numbers.
    candidate.apply_checked_imports(&project.imported_samples, &checked_imports)?;
    candidate.reserve_missing_samples(&project.missing_samples);
    project
        .imported_samples
        .extend(candidate.restore_wav_palette(library)?);
    let used: std::collections::BTreeSet<_> =
        project.clips.iter().map(|clip| clip.sample_id).collect();
    for id in used {
        if project.missing_sample(id).is_some() || id >= crate::products::IMPORTED_ID_START {
            continue;
        }
        if let Err(reason) = candidate.read_pcm(id) {
            let (product, native_id) =
                Product::sample_key(id).ok_or("The native sample ID has no product identity.")?;
            let catalog = crate::catalog::bundled_wave_metadata(product)?;
            let entry = catalog.iter().find(|entry| entry.native_id == native_id);
            let (ticks, rate, inferred) =
                sample_timing(&project, candidate.sample_by_id(id), id, project.bpm, None);
            let mut missing = MissingSample::native(
                product,
                native_id,
                entry.map_or_else(
                    || format!("{} sample {native_id}", product.label()),
                    |entry| entry.name.clone(),
                ),
                entry.map_or(project.bpm, |entry| entry.source_bpm),
                entry.map_or(ticks, |entry| {
                    entry.bars.saturating_mul(TICKS_PER_BAR).max(1)
                }),
                entry.is_none() && inferred,
                reason,
            )?;
            missing.sample_rate = rate;
            if let Some(entry) = entry {
                missing.metadata.insert("code".into(), entry.code.clone());
                missing
                    .metadata
                    .insert("group_name".into(), entry.group_name.clone());
                missing
                    .metadata
                    .insert("frame_count".into(), entry.frame_count.to_string());
            }
            project.missing_samples.push(missing);
        }
    }
    let mut bindings = candidate.bindings().to_vec();
    for binding in &project.libraries {
        if !bindings
            .iter()
            .any(|current| current.product == binding.product)
            && project.missing_samples.iter().any(|sample| {
                matches!(sample.reference,
                MissingReference::Native { product, .. } if product == binding.product)
            })
        {
            bindings.push(binding.clone());
        }
    }
    project.libraries = bindings;
    project.validate()?;
    Ok((project, candidate))
}

fn sample_timing(
    project: &Project,
    known: Option<&crate::library::Sample>,
    id: u32,
    source_bpm: f64,
    known_rate: Option<u32>,
) -> (u32, u32, bool) {
    if let Some(sample) = known {
        let ticks = (f64::from(sample.frame_count) / f64::from(sample.sample_rate)
            * sample.source_bpm
            / 240.0
            * f64::from(TICKS_PER_BAR))
        .ceil();
        return (
            (ticks as u32).clamp(1, crate::engine::MAX_TICKS),
            known_rate.unwrap_or(sample.sample_rate),
            false,
        );
    }
    let rate = known_rate.unwrap_or(44_100);
    let ticks = project
        .clips
        .iter()
        .filter(|clip| clip.sample_id == id)
        .map(|clip| {
            let trim_ticks = (f64::from(clip.offset_frames) / f64::from(rate) * source_bpm / 240.0
                * f64::from(TICKS_PER_BAR))
            .ceil() as u32;
            trim_ticks.saturating_add(clip.length_ticks)
        })
        .max()
        .unwrap_or(TICKS_PER_BAR)
        .clamp(1, crate::engine::MAX_TICKS);
    (ticks, rate, true)
}

pub fn detect_product(path: &Path) -> Result<Product, String> {
    let mut header = [0; 4];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut header))
        .map_err(|e| format!("Cannot read the song header: {e}"))?;
    let signature = u32::from_le_bytes(header);
    match signature {
        0x889 => Ok(Product::T2),
        0xa19 => Ok(Product::D2),
        0x2571 => Ok(Product::D3),
        0x2572 => Ok(Product::T3),
        0x2d41 => Ok(Product::D4),
        0x11d6 => Ok(Product::H1),
        _ => match u16::from_le_bytes(header[..2].try_into().unwrap()) {
            0xa06 => Ok(Product::D1),
            0xa07 => Ok(Product::T1),
            _ => Err("This song format is not supported.".into()),
        },
    }
}

fn conflict_selection(project: &Project, sample_id: u32) -> Option<PathBuf> {
    project
        .sample_conflicts
        .iter()
        .find(|conflict| conflict.sample_ids.contains(&sample_id))
        .map(|conflict| conflict.candidates[conflict.selected].clone())
}

/// A selected WAV folder relinks imports by path suffix. An import that the
/// folder lacks keeps its own file when that file exists; a missing file is
/// looked up in the library.
fn rebase_asset(
    imported: &mut ImportedSample,
    library: &Library,
    mixjam_references: &mut crate::mixjam::ReferenceResolver<'_>,
    selected: Option<&Path>,
) -> Result<Vec<PathBuf>, String> {
    let data_root = data_root();
    let canonical_data_root = fs::canonicalize(&data_root).ok();
    let is_retained = imported.path.starts_with(&data_root)
        || canonical_data_root
            .as_ref()
            .is_some_and(|root| imported.path.starts_with(root));
    let is_manual = imported.path.starts_with(data_root.join("manual-samples"))
        || canonical_data_root
            .as_ref()
            .is_some_and(|root| imported.path.starts_with(root.join("manual-samples")));
    if is_manual {
        // A missing retained manual choice stays missing. A similarly named palette
        // entry is not evidence that the user chose that sound.
        return Ok(Vec::new());
    }
    if let Some(selected) = selected {
        imported.path = selected.to_path_buf();
        return Ok(Vec::new());
    }
    let reference = imported.path.to_string_lossy().into_owned();
    if imported.group_name == "MixJam" {
        let paths = mixjam_references.resolve_candidates(&reference)?;
        imported.path = paths[0].clone();
        return Ok(paths);
    }
    if !library.wav_roots().is_empty() && !is_retained {
        let paths = library.find_wav_candidates(&reference)?;
        if let Some(path) = paths.first() {
            imported.path = path.clone();
            return Ok(paths);
        }
    }
    if !imported.path.is_file() && !library.samples.is_empty() {
        let paths = library.find_wav_candidates(&reference)?;
        imported.path = paths.first().cloned().ok_or_else(|| {
            format!("WAV sample {reference} is missing from the selected folder.")
        })?;
        return Ok(paths);
    }
    Ok(Vec::new())
}

fn has_song_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mixjam"))
}

/// [`save`] with each product provider recorded as the folder that holds the
/// song's files in this library. See [`Library::song_binding`].
pub fn save_with_library(project: &Project, library: &Library, path: &Path) -> Result<(), String> {
    save_with_library_in(project, library, path, &data_root())
}

fn save_with_library_in(
    project: &Project,
    library: &Library,
    path: &Path,
    storage: &Path,
) -> Result<(), String> {
    project.validate()?;
    if !has_song_extension(path) {
        return Err("Save the song with the .mixjam extension.".into());
    }
    let used: HashSet<_> = project.clips.iter().map(|clip| clip.sample_id).collect();
    let mut saved = project.clone();
    saved.libraries = saved
        .libraries
        .iter()
        .map(|binding| library.song_binding(binding, &used))
        .collect();
    for binding in &mut saved.libraries {
        let files: Vec<_> = used
            .iter()
            .filter(|id| {
                Product::sample_key(**id).is_some_and(|(product, _)| product == binding.product)
                    && project.missing_sample(**id).is_none()
            })
            .filter_map(|id| {
                let sample = library.sample_by_id(*id)?;
                Some((*id, sample.wav_path.as_deref()?))
            })
            .collect();
        if files
            .iter()
            .any(|(_, file)| !file.starts_with(&binding.root))
        {
            binding.root = retain_native_provider(binding.product, files, storage)?;
        }
    }
    save(&saved, path)
}

/// A product split across volumes has no single folder that a format-18
/// binding can name. Retain its used WAV bytes together, and verify their
/// native identities before the song can refer to that folder.
fn retain_native_provider(
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
        let metadata = crate::catalog::bundled_wave_metadata(product)?;
        let mut digest = Sha256::new();
        digest.update(product.slug().as_bytes());
        let mut expected = Vec::with_capacity(files.len());
        for (id, source) in files {
            let (_, native_id) = Product::sample_key(id).ok_or("Invalid native sample ID.")?;
            let entry = metadata
                .iter()
                .find(|entry| entry.native_id == native_id)
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

fn bounded_provider_wave(path: &Path) -> Result<Vec<u8>, String> {
    const MAX_BYTES: u64 = 128 * 1024 * 1024;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("The provider WAV exceeds the 128 MiB limit.".into());
    }
    Ok(bytes)
}

fn provider_wave_audio(path: &Path) -> Result<(hound::WavSpec, u32, [u8; 32]), String> {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    let (spec, frames) = crate::audio::visit_wav_samples(path, |_, sample| {
        digest.update(sample.to_bits().to_le_bytes());
    })?;
    Ok((spec, frames, digest.finalize().into()))
}

/// A saved song references its used audio, not every sound in the browser palette.
pub fn save(project: &Project, path: &Path) -> Result<(), String> {
    if !has_song_extension(path) {
        return Err("Save the song with the .mixjam extension.".into());
    }
    let mut saved = project.clone();
    saved.prune_sample_conflicts();
    for imported in &mut saved.imported_samples {
        if imported.source_sample_rate.is_none()
            && let Ok(ready) = Library::check_import(imported)
        {
            imported.source_sample_rate = Some(ready.sample_rate);
        }
    }
    let used: HashSet<_> = saved.clips.iter().map(|clip| clip.sample_id).collect();
    saved
        .imported_samples
        .retain(|sample| used.contains(&sample.id));
    saved
        .missing_samples
        .retain(|sample| used.contains(&sample.id));
    saved.libraries.retain(|binding| {
        saved.clips.iter().any(|clip| {
            Product::sample_key(clip.sample_id)
                .is_some_and(|(product, _)| product == binding.product)
        })
    });
    crate::mixjam::save(&saved, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_audio_identity_ignores_riff_metadata() {
        let root =
            std::env::temp_dir().join(format!("mixjam-provider-wave-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let first = root.join("first.wav");
        let second = root.join("second.wav");
        crate::audio::Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.25; 300],
        }
        .write_wav(&first)
        .unwrap();
        let bytes = fs::read(&first).unwrap();
        let mut tagged = bytes.clone();
        tagged.extend_from_slice(b"JUNK\x04\x00\x00\x00test");
        let riff_size = (tagged.len() as u32 - 8).to_le_bytes();
        tagged[4..8].copy_from_slice(&riff_size);
        fs::write(&second, &tagged).unwrap();
        assert_ne!(bytes, tagged);
        assert_eq!(
            provider_wave_audio(&first).unwrap(),
            provider_wave_audio(&second).unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retained_native_provider_preserves_ids_audio_and_existing_song_on_failure() {
        let root =
            std::env::temp_dir().join(format!("mixjam-retained-provider-{}", std::process::id()));
        let metadata = crate::catalog::bundled_wave_metadata(Product::D1).unwrap();
        let mut sources = Vec::new();
        let mut editor = crate::engine::Editor::new(Project::default());
        for (index, entry) in metadata.iter().take(2).enumerate() {
            let folder = root.join(format!("volume-{index}"));
            fs::create_dir_all(&folder).unwrap();
            let code = entry.code.replace('\\', "/");
            let path = folder
                .join(Path::new(&code).file_stem().unwrap())
                .with_extension("wav");
            crate::audio::Pcm {
                sample_rate: 22_050,
                channels: 2,
                samples: vec![0.25 * (index + 1) as f32; entry.frame_count as usize],
            }
            .write_wav(&path)
            .unwrap();
            let id = Product::D1.global_sample_id(entry.native_id).unwrap();
            editor.place(id, index, 0, 256).unwrap();
            sources.push((id, path));
        }
        let folders = [root.join("volume-0"), root.join("volume-1")];
        let library = Library::open_wav_folders_with(&folders, 140.0, None, None).unwrap();
        let mut project = editor.project().clone();
        // Inject a binding which cannot cover the loaded files, the same
        // condition that different Windows volume roots produce.
        project.libraries = vec![LibraryBinding {
            product: Product::D1,
            root: root.join("old-provider"),
        }];
        project.clips[0].offset_frames = 3;
        project.clips[0].gain = 0.7;
        let original = project.clone();
        let song = root.join("saved.mixjam");
        let storage = root.join("data");
        save_with_library_in(&project, &library, &song, &storage).unwrap();
        let (opened, ready) = open(&song, &Library::empty()).unwrap();
        assert!(opened.missing_samples.is_empty());
        assert_eq!(opened.clips, original.clips);
        assert_eq!(project, original);
        for (id, path) in &sources {
            let expected = crate::audio::read_wav(path).unwrap();
            let actual = ready.read_pcm(*id).unwrap();
            assert_eq!(actual.samples, expected.samples);
            assert_eq!(actual.sample_rate, expected.sample_rate);
            assert_eq!(actual.channels, expected.channels);
        }
        let retained = opened.libraries[0].root.clone();
        assert!(retained.starts_with(fs::canonicalize(&storage).unwrap()));
        save_with_library_in(&project, &library, &song, &storage).unwrap();
        let (again, _) = open(&song, &Library::empty()).unwrap();
        assert_eq!(again.libraries[0].root, retained);
        assert_eq!(fs::read_dir(retained.parent().unwrap()).unwrap().count(), 1);
        let previous_song = fs::read(&song).unwrap();
        let cached = ready
            .sample_by_id(sources[0].0)
            .unwrap()
            .wav_path
            .as_ref()
            .unwrap();
        let mut corrupt = crate::audio::read_wav(cached).unwrap();
        corrupt.samples.fill(0.0);
        corrupt.write_wav(cached).unwrap();
        assert!(save_with_library_in(&project, &library, &song, &storage).is_err());
        assert_eq!(fs::read(&song).unwrap(), previous_song);
        assert_eq!(fs::read_dir(retained.parent().unwrap()).unwrap().count(), 1);
        assert_eq!(project, original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_headless_open_reuses_and_extends_the_scan_cache() {
        let root = std::env::temp_dir().join(format!("mixjam-cached-open-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (samples, data) = (root.join("Samples"), root.join("data"));
        fs::create_dir_all(samples.join("Loop")).unwrap();
        for path in [
            samples.join("one.wav"),
            samples.join("Loop").join("two.wav"),
        ] {
            crate::audio::Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![0.25; 441],
            }
            .write_wav(&path)
            .unwrap();
        }
        let first =
            open_wav_folders_cached(&data, std::slice::from_ref(&samples), 140.0, None).unwrap();
        assert_eq!(first.samples.len(), 2);
        assert_eq!(load_scan_cache(&data).len(), 2);
        // Bytes that cannot be decoded, at the size and time the cache knows:
        // only the cached probe can still read the file.
        let path = samples.join("one.wav");
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let length = fs::metadata(&path).unwrap().len() as usize;
        fs::write(&path, vec![0u8; length]).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        let cached =
            open_wav_folders_cached(&data, std::slice::from_ref(&samples), 140.0, None).unwrap();
        assert_eq!(cached.samples.len(), 2);
        assert!(cached.skipped_waves().is_empty());
        let fresh = Library::open_wav_folder(&samples, 140.0).unwrap();
        assert_eq!(fresh.samples.len(), 1);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn reopening_refreshes_changed_wave_metadata_at_the_same_path() {
        let root = std::env::temp_dir().join(format!("mixjam-refresh-wave-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("take.wav");
        let mut pcm = crate::audio::Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.25; 441],
        };
        pcm.write_wav(&path).unwrap();
        let mut library = Library::empty();
        let imported = library
            .import_wav_with_tempo(&path, None, "Wave", 140.0)
            .unwrap();
        let project = Project {
            imported_samples: vec![imported.clone()].into(),
            clips: vec![crate::engine::Clip {
                id: 1,
                sample_id: imported.id,
                track: 0,
                start_tick: 0,
                length_ticks: 256,
                offset_frames: 0,
                gain: 1.0,
                fades: Default::default(),
                pitch_cents: 0,
            }],
            ..Project::default()
        };
        pcm.channels = 2;
        pcm.samples = vec![0.5; 1_764];
        pcm.write_wav(&path).unwrap();
        let (_, refreshed) = resolve(project, &library).unwrap();
        let sample = refreshed.sample_by_id(imported.id).unwrap();
        assert_eq!(sample.frame_count, 882);
        assert_eq!(sample.channels, 2);
        assert_eq!(sample.digest, None);
        assert_eq!(sample.length, fs::metadata(&path).unwrap().len());
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn song_folder_is_remembered_until_the_folder_disappears() {
        let root = std::env::temp_dir().join(format!("mixjam-song-folder-{}", std::process::id()));
        let settings = root.join("settings");
        let songs = root.join("songs");
        fs::create_dir_all(&songs).unwrap();
        assert_eq!(load_song_folder(&settings), None);
        save_song_folder(&settings, &songs).unwrap();
        assert_eq!(load_song_folder(&settings), Some(songs.clone()));
        fs::remove_dir(&songs).unwrap();
        assert_eq!(load_song_folder(&settings), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_earlier_single_folder_setting_reads_as_a_list_and_saves_as_one() {
        let root =
            std::env::temp_dir().join(format!("mixjam-folder-config-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let settings = root.join("settings");
        let (first, second) = (root.join("First"), root.join("Second"));
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        fs::create_dir_all(&settings).unwrap();
        let file = settings.join("sample-folder.json");
        assert_eq!(load_wave_config_in(&settings), Ok(None));
        // The file that earlier versions wrote: one root.
        fs::write(
            &file,
            serde_json::to_vec(&serde_json::json!({"root": first, "source_bpm": 160.0})).unwrap(),
        )
        .unwrap();
        assert_eq!(
            load_wave_config_in(&settings),
            Ok(Some(WaveFolderConfig {
                roots: vec![first.clone()],
                source_bpm: 160.0,
            }))
        );
        // A save writes the list. A folder that is offline keeps its path.
        let offline = root.join("Offline");
        save_wave_config_in(
            &settings,
            &[first.clone(), offline.clone(), second.clone()],
            150.0,
        )
        .unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert!(
            saved.get("root").is_none() && saved["roots"].is_array(),
            "{saved}"
        );
        assert_eq!(
            load_wave_config_in(&settings).unwrap().unwrap(),
            WaveFolderConfig {
                roots: vec![
                    fs::canonicalize(&first).unwrap(),
                    offline,
                    fs::canonicalize(&second).unwrap()
                ],
                source_bpm: 150.0,
            }
        );
        // Ten folders are refused, in a save and in a file.
        let ten: Vec<_> = (0..10)
            .map(|index| root.join(format!("L{index}")))
            .collect();
        assert!(save_wave_config_in(&settings, &ten, 140.0).is_err());
        fs::write(
            &file,
            serde_json::to_vec(&serde_json::json!({"roots": ten, "source_bpm": 140.0})).unwrap(),
        )
        .unwrap();
        assert!(
            load_wave_config_in(&settings)
                .unwrap_err()
                .contains("up to 9")
        );
        fs::write(
            &file,
            serde_json::to_vec(
                &serde_json::json!({"root": first, "roots": [second], "source_bpm": 140.0}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(load_wave_config_in(&settings).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn oversized_folder_settings_do_not_replace_a_readable_list() {
        let root =
            std::env::temp_dir().join(format!("mixjam-folder-config-size-{}", std::process::id()));
        let settings = root.join("settings");
        let roots = |depth| {
            (0..crate::library::MAX_LIBRARY_FOLDERS)
                .map(|index| {
                    let mut path = root.join(format!("Offline{index}"));
                    for _ in 0..depth {
                        path.push("a".repeat(80));
                    }
                    path
                })
                .collect::<Vec<_>>()
        };
        let accepted = roots(10);
        save_wave_config_in(&settings, &accepted, 140.0).unwrap();
        let expected = WaveFolderConfig {
            roots: accepted,
            source_bpm: 140.0,
        };
        assert_eq!(load_wave_config_in(&settings), Ok(Some(expected.clone())));
        let file = settings.join("sample-folder.json");
        let before = fs::read(&file).unwrap();

        let error = save_wave_config_in(&settings, &roots(30), 160.0).unwrap_err();
        assert!(error.contains("size limit"), "{error}");
        assert_eq!(fs::read(&file).unwrap(), before);
        assert_eq!(load_wave_config_in(&settings), Ok(Some(expected)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_song_relinks_a_file_moved_into_another_library_folder() {
        let root =
            std::env::temp_dir().join(format!("mixjam-moved-library-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (first, second) = (root.join("First"), root.join("Second"));
        fs::create_dir_all(first.join("Drums")).unwrap();
        fs::create_dir_all(second.join("Pack/Loops")).unwrap();
        write_wave(&first.join("Drums/snare.wav"));
        let moved = second.join("Pack/Loops/kick.wav");
        write_wave(&moved);
        let library =
            Library::open_wav_folders_with(&[first.clone(), second.clone()], 140.0, None, None)
                .unwrap();
        let mut imported = library
            .imported_samples()
            .into_iter()
            .find(|sample| sample.name == "kick")
            .unwrap();
        // The song names the file where it was before the pack moved from
        // a subfolder of the first library to the second library.
        imported.path = first.join("Moved pack/Pack/Loops/kick.wav");
        let project = Project {
            imported_samples: vec![imported.clone()].into(),
            clips: vec![crate::engine::Clip {
                id: 1,
                sample_id: imported.id,
                track: 0,
                start_tick: 0,
                length_ticks: 1,
                offset_frames: 0,
                gain: 1.0,
                fades: Default::default(),
                pitch_cents: 0,
            }],
            ..Project::default()
        };
        let song = root.join("song.mixjam");
        save(&project, &song).unwrap();
        let (opened, _, _) = open_song(&song, &library).unwrap();
        assert!(opened.missing_samples.is_empty());
        let clip_source = opened
            .imported_samples
            .iter()
            .find(|sample| sample.id == opened.clips[0].sample_id)
            .unwrap();
        assert_eq!(clip_source.path, fs::canonicalize(&moved).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn manual_wave_retention_rejects_oversized_files_before_creating_staging_files() {
        let root = std::env::temp_dir().join(format!("mixjam-manual-size-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let wave = root.join("oversized.wav");
        let file = fs::File::create(&wave).unwrap();
        file.set_len(128 * 1024 * 1024 + 1).unwrap();
        drop(file);
        let cache = root.join("cache");
        assert!(
            retain_manual_wave(&wave, &cache)
                .unwrap_err()
                .contains("128 MiB")
        );
        assert!(!cache.exists());
        fs::remove_file(wave).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn saved_rate_survives_lost_audio_and_autoresolution_preserves_trim_time() {
        let root = std::env::temp_dir().join(format!("mixjam-lost-rate-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let original = root.join("lead.wav");
        crate::audio::Pcm {
            sample_rate: 22_050,
            channels: 1,
            samples: vec![0.5; 22_050],
        }
        .write_wav(&original)
        .unwrap();
        let mut library = Library::empty();
        let imported = library
            .import_wav_with_tempo(&original, None, "Lead", 140.0)
            .unwrap();
        assert_eq!(imported.source_sample_rate, Some(22_050));
        let project = Project {
            imported_samples: vec![imported.clone()].into(),
            clips: vec![crate::engine::Clip {
                id: 1,
                sample_id: imported.id,
                track: 0,
                start_tick: 0,
                length_ticks: 256,
                offset_frames: 11_025,
                gain: 1.0,
                fades: Default::default(),
                pitch_cents: 0,
            }],
            ..Project::default()
        };
        let song = root.join("song.mixjam");
        save(&project, &song).unwrap();
        assert_eq!(
            crate::mixjam::load_song(&song, &Library::empty())
                .unwrap()
                .0
                .imported_samples[0]
                .source_sample_rate,
            Some(22_050)
        );
        fs::remove_file(&original).unwrap();
        let (missing, _) = open(&song, &Library::empty()).unwrap();
        assert_eq!(missing.missing_samples[0].sample_rate, 22_050);
        let moved = root.join("moved");
        fs::create_dir_all(&moved).unwrap();
        crate::audio::Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 44_100],
        }
        .write_wav(&moved.join("lead.wav"))
        .unwrap();
        let ready = Library::open_wav_folder(&moved, 140.0).unwrap();
        let (direct, _) = open(&song, &ready).unwrap();
        assert_eq!(direct.clips[0].offset_frames, 22_050);
        let (resolved, _) = resolve(missing.clone(), &ready).unwrap();
        assert_eq!(resolved.clips[0].offset_frames, 22_050);
        assert_eq!(
            resolved.imported_samples[0].source_sample_rate,
            Some(44_100)
        );
        let mut legacy = project;
        legacy.imported_samples[0].source_sample_rate = None;
        let (legacy_missing, _) = resolve(legacy, &Library::empty()).unwrap();
        assert_eq!(
            legacy_missing.missing_samples[0].metadata["Trim rate policy"],
            "preserve-frames"
        );
        let (legacy_resolved, _) = resolve(legacy_missing, &ready).unwrap();
        assert_eq!(legacy_resolved.clips[0].offset_frames, 11_025);
        fs::remove_file(song).unwrap();
        fs::remove_file(moved.join("lead.wav")).unwrap();
        fs::remove_dir(moved).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn mixjam_placeholder_uses_actual_wave_duration_for_saved_musical_span() {
        let root =
            std::env::temp_dir().join(format!("mixjam-placeholder-tempo-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let wave = root.join("lead.wav");
        crate::audio::Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 88_200],
        }
        .write_wav(&wave)
        .unwrap();
        let imported = ImportedSample {
            id: 1_000_000,
            name: "Lead".into(),
            group_name: "Wave".into(),
            path: PathBuf::from("lost/lead.wav"),
            source_bpm: 140.0,
            source_sample_rate: None,
            origin: None,
        };
        let mut missing = MissingSample::external(&imported, 256, "Missing file.".into());
        missing
            .metadata
            .insert("Playback timing policy".into(), "mixjam-span".into());
        missing
            .metadata
            .insert("durationSeconds".into(), "99".into());
        let project = Project {
            missing_samples: vec![missing],
            clips: vec![crate::engine::Clip {
                id: 1,
                sample_id: imported.id,
                track: 0,
                start_tick: 0,
                length_ticks: 256,
                offset_frames: 37_800,
                gain: 1.0,
                fades: Default::default(),
                pitch_cents: 0,
            }],
            ..Project::default()
        };
        let library = Library::open_wav_folder(&root, 140.0).unwrap();
        let (resolved, _) = resolve(project, &library).unwrap();
        assert_eq!(resolved.imported_samples[0].source_bpm, 120.0);
        assert_eq!(resolved.clips[0].offset_frames, 44_100);
        fs::remove_file(wave).unwrap();
        fs::remove_dir(root).unwrap();
    }

    fn write_wave(path: &Path) {
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..128 {
            writer.write_sample(500i16).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn wav_only_project_relinks_and_renders_without_saved_installations() {
        let root = std::env::temp_dir().join(format!(
            "mixjam-techno-standalone-open-{}",
            std::process::id()
        ));
        let waves = root.join("moved/Drums");
        fs::create_dir_all(&waves).unwrap();
        fs::create_dir_all(root.join("old/Drums")).unwrap();
        write_wave(&waves.join("my-own-kick.wav"));
        write_wave(&root.join("old/Drums/my-own-kick.wav"));
        let library = Library::open_wav_folder(&root.join("moved"), 140.0).unwrap();
        let mut imported = library.imported_samples().remove(0);
        imported.path = root.join("old/Drums/my-own-kick.wav");
        let mut project = Project {
            imported_samples: vec![imported.clone()].into(),
            libraries: vec![LibraryBinding {
                product: Product::T2,
                root: root.join("absent-installation"),
            }],
            ..Project::default()
        };
        let mut unused = imported.clone();
        unused.id += 1;
        unused.path = root.join("absent-unused.wav");
        project.imported_samples.push(unused);
        project.clips.push(crate::engine::Clip {
            id: 1,
            sample_id: imported.id,
            track: 0,
            start_tick: 0,
            length_ticks: 1,
            offset_frames: 0,
            gain: 1.0,
            fades: Default::default(),
            pitch_cents: 0,
        });
        let file = root.join("song.json");
        project.save(&file).unwrap();
        let (opened, ready) = open(&file, &library).unwrap();
        assert_eq!(
            opened.imported_samples[0].path,
            fs::canonicalize(waves.join("my-own-kick.wav")).unwrap()
        );
        assert!(opened.libraries.is_empty());
        let audio = crate::audio::render_project(&opened, &ready).unwrap();
        assert!(audio.samples.iter().any(|sample| *sample != 0.0));
        assert!(save(&opened, &file).unwrap_err().contains(".mixjam"));
        let song = root.join("song.mixjam");
        save(&opened, &song).unwrap();
        assert_eq!(
            crate::mixjam::load_song(&song, &Library::empty())
                .unwrap()
                .0
                .imported_samples
                .len(),
            1
        );
        // The one read that opens a file also reports whether Save can replace it.
        let (reopened, _, replaceable) = open_song(&song, &library).unwrap();
        // The untitled project opens under its file name.
        assert_eq!(
            reopened,
            Project {
                title: "song".into(),
                ..opened
            }
        );
        assert!(replaceable);
        assert!(!open_song(&file, &library).unwrap().2);
        fs::remove_file(song).unwrap();
        fs::remove_file(file).unwrap();
        fs::remove_file(waves.join("my-own-kick.wav")).unwrap();
        fs::remove_file(root.join("old/Drums/my-own-kick.wav")).unwrap();
        fs::remove_dir(root.join("old/Drums")).unwrap();
        fs::remove_dir(root.join("old")).unwrap();
        fs::remove_dir(waves).unwrap();
        fs::remove_dir(root.join("moved")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn ambiguous_selected_folder_selects_first_and_preserves_conflict_choices() {
        let root = std::env::temp_dir().join(format!(
            "mixjam-techno-ambiguous-open-{}",
            std::process::id()
        ));
        let selected = root.join("selected");
        let old = root.join("old");
        for folder in [selected.join("a"), selected.join("b"), old.clone()] {
            fs::create_dir_all(&folder).unwrap();
            write_wave(&folder.join("take.wav"));
        }
        let library = Library::open_wav_folder(&selected, 140.0).unwrap();
        let original_palette = library.imported_samples();
        let imported = crate::engine::ImportedSample {
            id: crate::products::IMPORTED_ID_START + 7,
            name: "Take".into(),
            group_name: "Wave".into(),
            path: fs::canonicalize(old.join("take.wav")).unwrap(),
            source_bpm: 140.0,
            source_sample_rate: None,
            origin: None,
        };
        let mut project = Project {
            imported_samples: vec![imported.clone()].into(),
            ..Project::default()
        };
        project.clips.push(crate::engine::Clip {
            id: 1,
            sample_id: imported.id,
            track: 0,
            start_tick: 0,
            length_ticks: 1,
            offset_frames: 0,
            gain: 1.0,
            fades: Default::default(),
            pitch_cents: 0,
        });
        let file = root.join("song.json");
        project.save(&file).unwrap();
        let saved_bytes = fs::read(&file).unwrap();

        let (opened, ready) = open(&file, &library).unwrap();
        assert!(opened.missing_samples.is_empty());
        let conflict = &opened.sample_conflicts[0];
        assert_eq!(conflict.sample_ids, vec![imported.id]);
        assert_eq!(
            conflict.candidates,
            vec![
                fs::canonicalize(selected.join("a/take.wav")).unwrap(),
                fs::canonicalize(selected.join("b/take.wav")).unwrap()
            ]
        );
        assert_eq!(conflict.selected, 0);
        assert!(!conflict.confirmed);
        assert_eq!(opened.clips, project.clips);
        assert_eq!(
            ready.sample_by_id(imported.id).unwrap().wav_path.as_ref(),
            Some(&conflict.candidates[0])
        );
        assert!(
            crate::audio::render_project(&opened, &ready)
                .unwrap()
                .samples
                .iter()
                .any(|sample| *sample != 0.0)
        );
        assert_eq!(fs::read(&file).unwrap(), saved_bytes);
        assert_eq!(Project::load(&file).unwrap(), project);
        assert_eq!(library.imported_samples(), original_palette);
        assert!(library.sample_by_id(imported.id).is_none());
        assert!(imported.path.is_file());

        let mut chosen = opened.clone();
        chosen.sample_conflicts[0].selected = 1;
        chosen.sample_conflicts[0].confirmed = true;
        chosen
            .imported_samples
            .iter_mut()
            .find(|sample| sample.id == imported.id)
            .unwrap()
            .path = conflict.candidates[1].clone();
        let chosen_file = root.join("chosen.mixjam");
        save(&chosen, &chosen_file).unwrap();
        let (reopened, _) = open(&chosen_file, &library).unwrap();
        assert_eq!(reopened.sample_conflicts, chosen.sample_conflicts);
        assert_eq!(
            reopened
                .imported_samples
                .iter()
                .find(|sample| sample.id == imported.id)
                .unwrap()
                .path,
            conflict.candidates[1]
        );
        fs::remove_file(chosen_file).unwrap();

        fs::remove_file(file).unwrap();
        for folder in [selected.join("a"), selected.join("b"), old] {
            fs::remove_file(folder.join("take.wav")).unwrap();
            fs::remove_dir(folder).unwrap();
        }
        fs::remove_dir(selected).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn selected_folder_keeps_existing_imports_that_it_lacks() {
        let root =
            std::env::temp_dir().join(format!("mixjam-techno-keep-import-{}", std::process::id()));
        let waves = root.join("folder");
        let elsewhere = root.join("elsewhere");
        fs::create_dir_all(&waves).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        write_wave(&waves.join("kick.wav"));
        write_wave(&elsewhere.join("take.wav"));
        let library = Library::open_wav_folder(&waves, 140.0).unwrap();
        let take = fs::canonicalize(elsewhere.join("take.wav")).unwrap();
        let imported = crate::engine::ImportedSample {
            id: crate::products::IMPORTED_ID_START + 7,
            name: "Take".into(),
            group_name: "Wave".into(),
            path: take.clone(),
            source_bpm: 140.0,
            source_sample_rate: None,
            origin: None,
        };
        let mut project = Project {
            imported_samples: vec![imported.clone()].into(),
            ..Project::default()
        };
        project.clips.push(crate::engine::Clip {
            id: 1,
            sample_id: imported.id,
            track: 0,
            start_tick: 0,
            length_ticks: 1,
            offset_frames: 0,
            gain: 1.0,
            fades: Default::default(),
            pitch_cents: 0,
        });
        let (opened, ready) = resolve(project.clone(), &library).unwrap();
        assert_eq!(opened.imported_samples[0].path, take);
        assert_eq!(
            ready.sample_by_id(imported.id).unwrap().wav_path.as_deref(),
            Some(take.as_path())
        );
        fs::remove_file(&take).unwrap();
        let (opened, _) = resolve(project, &library).unwrap();
        let error = &opened.missing_sample(imported.id).unwrap().reason;
        assert!(
            error.contains("missing from the selected folder"),
            "{error}"
        );
        fs::remove_file(waves.join("kick.wav")).unwrap();
        fs::remove_dir(waves).unwrap();
        fs::remove_dir(elsewhere).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn typed_product_headers_reject_truncation_and_unknown_formats() {
        let root =
            std::env::temp_dir().join(format!("mixjam-techno-header-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("test.mix");
        for (signature, product) in [
            (0x889u32, Product::T2),
            (0xa19, Product::D2),
            (0x2571, Product::D3),
            (0x2572, Product::T3),
            (0x2d41, Product::D4),
            (0x11d6, Product::H1),
            (0xa06, Product::D1),
            (0xa07, Product::T1),
        ] {
            fs::write(&path, signature.to_le_bytes()).unwrap();
            assert_eq!(detect_product(&path).unwrap(), product);
        }
        fs::write(&path, [7, 10]).unwrap();
        assert!(detect_product(&path).is_err());
        fs::write(&path, [0xff; 4]).unwrap();
        assert!(detect_product(&path).is_err());
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn missing_native_audio_opens_without_changing_catalog_or_project_file() {
        let root = std::env::temp_dir().join(format!("mixjam-techno-open-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("song.json");
        let mut project = Project::default();
        project.clips.push(crate::engine::Clip {
            id: 1,
            sample_id: 999,
            track: 63,
            start_tick: 0,
            length_ticks: 256,
            offset_frames: 0,
            gain: 1.0,
            fades: Default::default(),
            pitch_cents: 0,
        });
        project.save(&path).unwrap();
        let before = fs::read(&path).unwrap();
        let library = Library::empty();
        let (opened, _) = open(&path, &library).unwrap();
        assert_eq!(opened.clips, project.clips);
        assert!(opened.missing_sample(999).is_some());
        assert!(library.samples.is_empty());
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn missing_metadata_survives_save_reopen_and_resolution_preserves_trim_time() {
        let root =
            std::env::temp_dir().join(format!("mixjam-missing-roundtrip-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let imported = ImportedSample {
            id: crate::products::IMPORTED_ID_START,
            name: "Original lead".into(),
            group_name: "User leads".into(),
            path: root.join("old/lead.wav"),
            source_bpm: 120.0,
            source_sample_rate: None,
            origin: None,
        };
        let mut missing =
            MissingSample::external(&imported, 1024, "Original file is missing.".into());
        missing.length_inferred = false;
        missing
            .metadata
            .insert("original_label_2".into(), "Saved second label".into());
        let mut project = Project {
            missing_samples: vec![missing.clone()],
            ..Project::default()
        };
        project.clips.push(crate::engine::Clip {
            id: 9,
            sample_id: imported.id,
            track: 5,
            start_tick: 17,
            length_ticks: 256,
            offset_frames: 4410,
            gain: 0.75,
            fades: Default::default(),
            pitch_cents: 0,
        });
        let path = root.join("song.mixjam");
        save(&project, &path).unwrap();
        let (opened, empty) = open(&path, &Library::empty()).unwrap();
        assert_eq!(opened.clips, project.clips);
        let retained = opened.missing_sample(imported.id).unwrap();
        assert_eq!(retained.metadata, missing.metadata);
        assert_eq!(retained.reference, missing.reference);
        assert_eq!(retained.nominal_ticks, 1024);
        assert!(!retained.length_inferred);
        let silent = crate::audio::render_project(&opened, &empty).unwrap();
        assert_eq!(silent.frames(), 80_621);
        assert!(silent.samples.iter().all(|sample| *sample == 0.0));
        save(&opened, &path).unwrap();
        assert_eq!(open(&path, &Library::empty()).unwrap().0, opened);

        let waves = root.join("moved");
        fs::create_dir_all(&waves).unwrap();
        crate::audio::Pcm {
            sample_rate: 22_050,
            channels: 1,
            samples: vec![0.25; 22_050],
        }
        .write_wav(&waves.join("lead.wav"))
        .unwrap();
        let library = Library::open_wav_folder(&waves, 120.0).unwrap();
        let (resolved, ready) = resolve(opened, &library).unwrap();
        assert!(resolved.missing_samples.is_empty());
        assert_eq!(resolved.clips[0].offset_frames, 2205);
        assert_eq!(resolved.clips[0].start_tick, 17);
        assert_eq!(resolved.clips[0].length_ticks, 256);
        assert_eq!(resolved.imported_samples[0].name, "Original lead");
        assert!(
            crate::audio::render_project(&resolved, &ready)
                .unwrap()
                .samples
                .iter()
                .any(|sample| *sample != 0.0)
        );
        fs::remove_file(path).unwrap();
        fs::remove_file(waves.join("lead.wav")).unwrap();
        fs::remove_dir(waves).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn lost_import_reserves_its_identity_and_retains_available_library_metadata() {
        let root =
            std::env::temp_dir().join(format!("mixjam-missing-identity-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let wave = root.join("lost.wav");
        write_wave(&wave);
        let mut library = Library::empty();
        let imported = library
            .import_wav_with_tempo(&wave, Some("Lost sound"), "Original group", 160.0)
            .unwrap();
        let mut project = Project {
            imported_samples: vec![imported.clone()].into(),
            ..Project::default()
        };
        project.clips.push(crate::engine::Clip {
            id: 1,
            sample_id: imported.id,
            track: 0,
            start_tick: 0,
            length_ticks: 8,
            offset_frames: 32,
            gain: 1.0,
            fades: Default::default(),
            pitch_cents: 0,
        });
        fs::remove_file(&wave).unwrap();
        let (opened, mut ready) = resolve(project.clone(), &library).unwrap();
        let missing = opened.missing_sample(imported.id).unwrap();
        assert_eq!(missing.name, "Lost sound");
        assert_eq!(missing.metadata["group_name"], "Original group");
        assert_eq!(missing.source_bpm, 160.0);
        assert_eq!(missing.sample_rate, 44_100);
        assert_eq!(opened.clips, project.clips);
        assert!(
            opened
                .imported_samples
                .iter()
                .all(|sample| sample.id != imported.id)
        );
        let other = root.join("new.wav");
        write_wave(&other);
        let new_sample = ready
            .import_wav_with_tempo(&other, None, "Wave", 140.0)
            .unwrap();
        assert_ne!(new_sample.id, missing.id);
        fs::remove_file(other).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
