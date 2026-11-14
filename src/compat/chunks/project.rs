//! Assembling a project from a read header and timeline: each event becomes a
//! clip on its lane, with its sound resolved from the library or the song's
//! own audio.

use super::{
    assets::{encode_float_wave, preserve_wave, queue_asset},
    layout::{Header, Reference, Timeline, WaveReference},
    waves::{
        RetainedWave, basename, d2_embedded_sources, d2_source_wave, embedded_waves,
        normalize_saved_path, retain_wave, source_wave, stretched_copy,
    },
};
use crate::{
    compat::{drums, settings},
    engine::{
        Clip, ImportedSample, MAX_TICKS, MissingReference, MissingSample, Project, TICKS_PER_BAR,
        normalize_import,
    },
    library::Library,
    products::Product,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};

pub(super) fn conflicting_placement_metadata(
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

/// The product whose catalog a native ID in a `product` song names. The
/// song's own product comes first. Its loader also reads earlier catalogs:
/// D3 the D2 IDs, D4 the D3 and D2 IDs (D4 songs save drum pads at D3 IDs
/// under their D3 catalog names, tagged `[D3]`), and T2 the D2 kind-2
/// catalog, the contiguous IDs 1..=1957. Do not search other products by ID;
/// equal numbers can identify unrelated sounds.
pub(in crate::compat) fn native_source(
    product: Product,
    native_id: u32,
    library: &Library,
) -> Result<Product, String> {
    if library
        .sample_by_id(product.global_sample_id(native_id)?)
        .is_some()
    {
        return Ok(product);
    }
    Ok(match (product, native_id) {
        (Product::D3 | Product::D4, 1..=12999) | (Product::T2, 1..=1957) => Product::D2,
        (Product::D4, 13000..=18299) => Product::D3,
        _ => product,
    })
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
        Reference::Kit(id) => Some(format!("kit:{id}")),
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
    let (master, _) = settings::booster(product, &header.settings);
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
        master,
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
    // Saved paths whose missing Time Stretcher copy was rebuilt, with its name.
    let mut stretched_names: HashMap<String, String> = HashMap::new();
    for event in events {
        let inferred_ticks = source_key(&event.reference)
            .and_then(|key| inferred_sources.get(&key).copied())
            .unwrap_or(TICKS_PER_BAR);
        let (sample_id, frames, rate, source_bpm, declared_ticks) = match event.reference {
            reference @ (Reference::Native(_) | Reference::Kit(_)) => {
                let (native_id, kit) = match reference {
                    Reference::Native(id) => (u32::from(id), false),
                    Reference::Kit(id) => (u32::from(id), true),
                    _ => unreachable!(),
                };
                let source_product = if kit {
                    Product::D2
                } else {
                    native_source(product, native_id, library)?
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
                    let metadata = crate::catalog::identity(source_product, native_id);
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
                                |entry| entry.display_name(source_product),
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
                        legacy_format::NATIVE_SAMPLE_RATE,
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
                            let wave = match wave {
                                Err(reason) if product == Product::T2 => {
                                    match stretched_copy(
                                        &metadata,
                                        library,
                                        header.bpm,
                                        declared_ticks,
                                    )? {
                                        Some(copy) => {
                                            let provenance = serde_json::json!({
                                                "schema_version": 1, "product": product,
                                                "source_mix_name": path.file_name().unwrap_or_default().to_string_lossy(),
                                                "source_mix_sha256": mix_hash.get_or_insert_with(|| format!("{:x}", Sha256::digest(bytes))),
                                                "saved_path": &source_path, "saved_labels": [metadata.get("Native label 1"), metadata.get("Native label 2")],
                                                "original": copy.original, "target_bpm": copy.target_bpm,
                                                "stretcher": "sample_edit::time_stretch",
                                                "rendered_audio_sha256": format!("{:x}", Sha256::digest(&copy.wave.bytes)),
                                            });
                                            let provenance = serde_json::to_vec_pretty(&provenance)
                                                .map_err(|error| error.to_string())?;
                                            let destination = crate::project_io::data_root()
                                                .join("imported-audio")
                                                .join(format!(
                                                    "{:x}.wav",
                                                    Sha256::digest(&copy.wave.bytes)
                                                ));
                                            let provenance_hash =
                                                format!("{:x}", Sha256::digest(&provenance));
                                            queue_asset(
                                                &mut pending_waves,
                                                &mut pending_bytes,
                                                destination.with_extension(format!(
                                                    "{provenance_hash}.json"
                                                )),
                                                provenance,
                                            )?;
                                            stretched_names.insert(key.path.clone(), copy.name);
                                            Ok(copy.wave)
                                        }
                                        None => Err(reason),
                                    }
                                }
                                other => other,
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
                            let id = project.next_import_id()?;
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
                                missing.metadata.insert(
                                    "Length confidence".into(),
                                    "Saved trims exceed a saved source span. The minimum \
                                     compatible whole-bar source length is inferred; valid saved \
                                     placement spans are retained."
                                        .into(),
                                );
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
                            wave_cache
                                .insert(key.clone(), (id, 0, legacy_format::NATIVE_SAMPLE_RATE));
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
                            let id = project.next_import_id()?;
                            project.imported_samples.push(ImportedSample {
                                id,
                                name: stretched_names
                                    .get(&key.path)
                                    .cloned()
                                    .unwrap_or_else(|| basename(&source_path).into()),
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
                    let label = format!("{} {}", pattern.group, pattern.variant)
                        .trim()
                        .to_string();
                    let name = if label.is_empty() {
                        format!("Drum pattern {}", pattern.native_id)
                    } else {
                        label
                    };
                    if let Err(reason) = pcm {
                        let id = project.next_import_id()?;
                        project.missing_samples.push(MissingSample {
                            id,
                            name,
                            source_bpm: header.bpm,
                            sample_rate: legacy_format::NATIVE_SAMPLE_RATE,
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
                            let id = project.next_import_id()?;
                            project.imported_samples.push(ImportedSample {
                                id,
                                name,
                                group_name: "Groove".into(),
                                path: destination.clone(),
                                source_bpm: header.bpm,
                                source_sample_rate: Some(legacy_format::NATIVE_SAMPLE_RATE),
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
                (
                    id,
                    frames,
                    legacy_format::NATIVE_SAMPLE_RATE,
                    header.bpm,
                    Some(TICKS_PER_BAR),
                )
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
            fades: Default::default(),
            pitch_cents: 0,
            reverse: false,
        });
    }
    let project = normalize_import(project)?;
    for (path, bytes) in pending_waves {
        preserve_wave(&path, &bytes)?;
    }
    Ok(project)
}
