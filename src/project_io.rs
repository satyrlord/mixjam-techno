//! Transactional project opening and local installation registrations.
use crate::{
    engine::{ImportedSample, MissingReference, MissingSample, Project, TICKS_PER_BAR},
    library::Library,
    products::{LibraryBinding, Product},
};
use std::{
    collections::HashSet,
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
                return Err("The retained manual sample has changed. Restore that file before you continue.".into());
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

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveFolderConfig {
    pub root: PathBuf,
    pub source_bpm: f64,
}

pub fn load_wave_config() -> Result<Option<WaveFolderConfig>, String> {
    let path = data_root().join("sample-folder.json");
    if !path.exists() {
        return Ok(None);
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    const LIMIT: u64 = 16 * 1024;
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("The sample folder settings exceed the size limit.".into());
    }
    let config: WaveFolderConfig = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Cannot read sample folder settings: {e}"))?;
    if !config.root.is_absolute() || !config.source_bpm.is_finite() || config.source_bpm <= 0.0 {
        return Err(
            "The sample folder settings need an absolute path and a positive source tempo.".into(),
        );
    }
    Ok(Some(config))
}

/// The probe cache of earlier folder scans. A missing, oversized, or
/// unreadable cache is an empty one: scans then decode every file again.
pub fn load_scan_cache(data_root: &Path) -> crate::library::ScanCache {
    let path = data_root.join("wav-scan-cache.json");
    let Ok(file) = fs::File::open(path) else {
        return Default::default();
    };
    let mut bytes = Vec::new();
    const LIMIT: u64 = 256 * 1024 * 1024;
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

pub fn save_wave_config_in(data_root: &Path, root: &Path, source_bpm: f64) -> Result<(), String> {
    if !source_bpm.is_finite() || source_bpm <= 0.0 {
        return Err("The source tempo must be finite and positive.".into());
    }
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(&WaveFolderConfig { root, source_bpm })
        .map_err(|e| e.to_string())?;
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
/// The flag says whether Save can replace the file: format version 8 or 9
/// song. Imports and earlier JSON projects need a new `.mixjam` path.
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
        // A selected WAV provider takes precedence over paths saved on another machine.
        if candidate.source_root(binding.product).is_some() || candidate.wav_root().is_some() {
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
            let _ = candidate.register(binding.product, &root, &reference_samples_root());
        }
    }
    let mut mixjam_references = crate::mixjam::ReferenceResolver::new(&candidate);
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
                    missing.metadata.insert("Sample rate confidence".into(), "The original frame rate is unknown. The 44100 Hz estimate only sets the displayed duration; resolution preserves the original frame offset.".into());
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
                    Ok((Some(imported), ready.sample_rate))
                })
            }
            MissingReference::Generated { .. } => {
                crate::compat::chunks::restore_missing(&sample, &candidate).and_then(|imported| {
                    let mut imported = imported.ok_or_else(|| sample.reason.clone())?;
                    let ready = Library::check_import(&imported)?;
                    imported.source_bpm =
                        sample.resolved_source_bpm(ready.frame_count, ready.sample_rate)?;
                    imported.source_sample_rate = Some(ready.sample_rate);
                    Ok((Some(imported), ready.sample_rate))
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
    candidate.apply_imports(&project.imported_samples)?;
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
    if library.wav_root().is_some() && !is_retained {
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
        assert_eq!(reopened, opened);
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
