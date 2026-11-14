//! Opening a song transactionally: reading it, resolving its sounds against the library, and its notices.
use super::{data_root, has_song_extension, reference_samples_root};
use crate::{
    engine::{ImportedSample, MissingReference, MissingSample, Project, TICKS_PER_BAR},
    library::Library,
    products::Product,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

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
        let notices = catalog_notices(&project, &candidate);
        project.source_notices.extend(notices);
        (project, false)
    } else if has_song_extension(path) {
        crate::mixjam::load_song(path, &candidate)?
    } else {
        (Project::load(path)?, false)
    };
    let (project, library) = resolve(project, &candidate)?;
    Ok((project, library, song))
}

/// Name the products whose catalogs lack native samples that an imported song
/// needs. An original installation that serves a product identifies its sounds
/// without a catalog, so its missing samples are absent from the installation.
pub(super) fn catalog_notices(project: &Project, library: &Library) -> Vec<String> {
    crate::catalog::catalogs().missing_notices(project.missing_samples.iter().filter_map(
        |sample| match sample.reference {
            MissingReference::Native { product, native_id }
                if !library.has_original_provider(product) =>
            {
                Some((product, native_id))
            }
            _ => None,
        },
    ))
}

/// Resolve a loaded song's sounds against a copy of the library. The song's
/// saved providers are registered first, then its imports and missing samples
/// are restored, and any native sound the library cannot read becomes missing.
pub fn resolve(mut project: Project, library: &Library) -> Result<(Project, Library), String> {
    project.validate()?;
    project.prune_sample_conflicts();
    let mut candidate = library.clone();
    let used = prune_unused_samples(&mut project);
    let saved_supplements = register_saved_providers(&project, &mut candidate, &used);
    let checked_imports = {
        let mut restoration = Restoration {
            candidate: &candidate,
            references: crate::mixjam::ReferenceResolver::new(&candidate),
            checked_imports: HashMap::new(),
        };
        restore_imports(&mut project, library, &mut restoration)?;
        restore_missing_samples(&mut project, &mut restoration)?;
        restoration.checked_imports
    };
    // Keep the selected folder's palette. Saved imports retain their IDs; unused
    // palette entries receive fresh IDs if the saved project uses those numbers.
    candidate.apply_checked_imports(&project.imported_samples, &checked_imports)?;
    candidate.reserve_missing_samples(&project.missing_samples);
    project
        .imported_samples
        .extend(candidate.restore_wav_palette(library)?);
    report_unreadable_natives(&mut project, &candidate)?;
    project.libraries = merged_bindings(&project, &candidate, &saved_supplements);
    project.validate()?;
    Ok((project, candidate))
}

/// What the restoration of a song's samples shares: the library copy with the
/// song's providers, the resolver of song references, and each import that reads.
struct Restoration<'a> {
    candidate: &'a Library,
    references: crate::mixjam::ReferenceResolver<'a>,
    checked_imports: HashMap<u32, crate::library::Sample>,
}

/// Drop imported and missing samples that no clip uses, and return the used IDs.
fn prune_unused_samples(project: &mut Project) -> HashSet<u32> {
    let used: HashSet<_> = project.clips.iter().map(|clip| clip.sample_id).collect();
    project
        .imported_samples
        .retain(|sample| used.contains(&sample.id));
    project
        .missing_samples
        .retain(|sample| used.contains(&sample.id));
    used
}

/// Register the providers saved in the song where the library lacks them.
/// Returns the products whose saved folder supplements an original provider;
/// the song keeps those bindings.
fn register_saved_providers(
    project: &Project,
    candidate: &mut Library,
    used: &HashSet<u32>,
) -> HashSet<Product> {
    let mut saved_supplements = HashSet::new();
    for binding in project.libraries.clone() {
        if candidate.has_original_provider(binding.product) {
            let needed: HashSet<_> = used
                .iter()
                .copied()
                .filter(|id| {
                    Product::sample_key(*id).is_some_and(|(product, _)| product == binding.product)
                        && candidate.sample_by_id(*id).is_none()
                })
                .collect();
            if !needed.is_empty() {
                // Preserve the song's saved folder even when it is temporarily
                // unavailable, so a later open can try it again.
                saved_supplements.insert(binding.product);
                if let Ok(root) = fs::canonicalize(&binding.root)
                    && candidate.source_root(binding.product) != Some(root.as_path())
                    && !crate::catalog::has_native_catalog(binding.product, &root)
                {
                    let mut saved = Library::empty();
                    if saved
                        .register(binding.product, &root, &reference_samples_root())
                        .is_ok()
                        && saved.is_wav_product(binding.product)
                    {
                        candidate.supplement_loaded_ids(binding.product, &saved, &needed);
                    }
                }
            }
            continue;
        }
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
                provided.supplement_loaded(binding.product, candidate);
                *candidate = provided;
            }
        }
    }
    saved_supplements
}

/// Relink and check each imported sample. An import that cannot be read
/// becomes a missing sample.
fn restore_imports(
    project: &mut Project,
    library: &Library,
    restoration: &mut Restoration<'_>,
) -> Result<(), String> {
    let imports = std::mem::take(&mut project.imported_samples);
    for imported in imports {
        let mut resolved = imported.clone();
        let selected = conflict_selection(project, imported.id);
        let rebased = rebase_asset(
            &mut resolved,
            restoration.candidate,
            &mut restoration.references,
            selected.as_deref(),
        );
        if let Ok(paths) = &rebased {
            crate::library::record_wav_conflict(
                project,
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
                    rescale_import_trims(project, resolved.id, previous_rate, ready.sample_rate)?;
                }
                resolved.source_sample_rate = Some(ready.sample_rate);
                restoration.checked_imports.insert(resolved.id, ready);
                project.imported_samples.push(resolved);
            }
            Err(reason) => {
                let missing = missing_import(project, library, &imported, reason);
                project.missing_samples.push(missing);
            }
        }
    }
    Ok(())
}

/// Move the trims of an import's clips from its saved rate to its current rate.
fn rescale_import_trims(
    project: &mut Project,
    sample_id: u32,
    previous_rate: u32,
    rate: u32,
) -> Result<(), String> {
    for clip in project
        .clips
        .iter_mut()
        .filter(|clip| clip.sample_id == sample_id)
    {
        let offset =
            (f64::from(clip.offset_frames) * f64::from(rate) / f64::from(previous_rate)).round();
        if offset > f64::from(u32::MAX) {
            return Err("The restored sample trim exceeds the supported frame range.".into());
        }
        clip.offset_frames = offset as u32;
    }
    Ok(())
}

/// The missing sample that stands in for an import that cannot be read.
fn missing_import(
    project: &Project,
    library: &Library,
    imported: &ImportedSample,
    reason: String,
) -> MissingSample {
    let known = library
        .sample_by_id(imported.id)
        .filter(|sample| sample.wav_path.as_deref() == Some(imported.path.as_path()));
    let (ticks, rate, inferred) = sample_timing(
        project,
        known,
        imported.id,
        imported.source_bpm,
        imported.source_sample_rate,
    );
    let mut missing = MissingSample::external(imported, ticks, reason);
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
    missing
}

/// Try to restore each missing sample from the library. A restored sample
/// rescales its clips' trims; the others stay missing.
fn restore_missing_samples(
    project: &mut Project,
    restoration: &mut Restoration<'_>,
) -> Result<(), String> {
    let candidate = restoration.candidate;
    let missing = std::mem::take(&mut project.missing_samples);
    for mut sample in missing {
        let selected = conflict_selection(project, sample.id);
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
                let restored = crate::compat::chunks::restore_missing(&sample, candidate);
                restored.and_then(|restored| {
                    restore_external(
                        project,
                        &sample,
                        restored,
                        (path, *origin),
                        restoration,
                        selected.as_deref(),
                    )
                })
            }
            MissingReference::Generated { .. } => {
                crate::compat::chunks::restore_missing(&sample, candidate).and_then(|imported| {
                    let mut imported = imported.ok_or_else(|| sample.reason.clone())?;
                    let ready = Library::check_import(&imported)?;
                    imported.source_bpm =
                        sample.resolved_source_bpm(ready.frame_count, ready.sample_rate)?;
                    imported.source_sample_rate = Some(ready.sample_rate);
                    let rate = ready.sample_rate;
                    restoration.checked_imports.insert(imported.id, ready);
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
                    clip.offset_frames = crate::engine::rescale_trim(
                        clip.offset_frames,
                        sample.preserves_trim_frames(),
                        (sample.sample_rate, sample.source_bpm),
                        (rate, resolved_bpm),
                    )
                    .ok_or("The restored sample trim exceeds the supported frame range.")?;
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
    Ok(())
}

/// Restore a missing external sample: relink its file, record any path
/// conflict, and check that it reads.
fn restore_external(
    project: &mut Project,
    sample: &MissingSample,
    restored: Option<ImportedSample>,
    (path, origin): (&PathBuf, Option<Product>),
    restoration: &mut Restoration<'_>,
    selected: Option<&Path>,
) -> Result<(Option<ImportedSample>, u32), String> {
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
        origin,
    });
    let paths = rebase_asset(
        &mut imported,
        restoration.candidate,
        &mut restoration.references,
        selected,
    )?;
    crate::library::record_wav_conflict(project, &path.to_string_lossy(), sample.id, paths);
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
    imported.source_bpm = sample.resolved_source_bpm(ready.frame_count, ready.sample_rate)?;
    imported.source_sample_rate = Some(ready.sample_rate);
    let rate = ready.sample_rate;
    restoration.checked_imports.insert(imported.id, ready);
    Ok((Some(imported), rate))
}

/// Mark each used native sound that the library cannot read as missing.
fn report_unreadable_natives(project: &mut Project, candidate: &Library) -> Result<(), String> {
    let used: std::collections::BTreeSet<_> =
        project.clips.iter().map(|clip| clip.sample_id).collect();
    for id in used {
        if project.missing_sample(id).is_some() || id >= crate::products::IMPORTED_ID_START {
            continue;
        }
        if let Err(reason) = candidate.read_pcm(id) {
            let (product, native_id) =
                Product::sample_key(id).ok_or("The native sample ID has no product identity.")?;
            let entry = crate::catalog::identity(product, native_id);
            let (ticks, rate, inferred) =
                sample_timing(project, candidate.sample_by_id(id), id, project.bpm, None);
            let mut missing = MissingSample::native(
                product,
                native_id,
                entry.map_or_else(
                    || format!("{} sample {native_id}", product.label()),
                    |entry| entry.display_name(product),
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
    Ok(())
}

/// The library bindings the song keeps: the library's own, with each saved
/// supplement folder, and each saved folder of a product that still has
/// missing native samples.
fn merged_bindings(
    project: &Project,
    candidate: &Library,
    saved_supplements: &HashSet<Product>,
) -> Vec<crate::products::LibraryBinding> {
    let mut bindings = candidate.bindings().to_vec();
    for binding in &project.libraries {
        if saved_supplements.contains(&binding.product)
            && let Some(current) = bindings
                .iter_mut()
                .find(|current| current.product == binding.product)
        {
            *current = binding.clone();
            continue;
        }
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
    bindings
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
    let rate = known_rate.unwrap_or(legacy_format::NATIVE_SAMPLE_RATE);
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
