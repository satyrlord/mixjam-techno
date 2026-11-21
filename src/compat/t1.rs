//! Import the recovered T1 profile as editable common-project clips.
use crate::{
    engine::{
        Clip, ImportedSample, MissingReference, MissingSample, Project, TICKS_PER_BAR,
        normalize_import,
    },
    library::{Library, decode_1252},
    products::Product,
};
use legacy_format::{
    imports::{self, ByteText, MixFooter},
    mix::Mix,
    playback::FileSample,
    timeline,
};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const PACKED_BYTE_LIMIT: usize = legacy_format::packed::MAX_PACKED_BYTES as usize;
/// The original loader holds a file's byte length in a signed 32-bit value.
const WAV_BYTE_LIMIT: usize = legacy_format::playback::NATIVE_FILE_BYTES as usize;
const SOURCE_BPM: f64 = 180.0;
const DEFAULT_KIT_REGIONS: [(u16, u16); 3] = [(4400, 4999), (5000, 5624), (5625, 6299)];

pub fn load(path: &Path, library: &Library) -> Result<Project, String> {
    let bytes = super::song_file::read_song_file(path)?;
    let mix = Mix::parse(&bytes).map_err(|error| format!("Cannot read the T1 MIX: {error}"))?;
    let root = library.source_root(Product::T1);
    let mut native_notice = None;
    let native = root
        .filter(|_| !library.is_wav_product(Product::T1))
        // Without readable catalogs, spans come from the registered samples.
        .and_then(|root| match legacy_format::library::Library::load(root) {
            Ok(native) => Some((root, native)),
            Err(error) => {
                native_notice = Some(format!(
                    "Cannot read the installed {} sample catalog: {error}",
                    Product::T1.label()
                ));
                None
            }
        });
    let installed = library.has_original_provider(Product::T1);
    let mut project = convert(&mix, path, library, |native_id| {
        let global_id = Product::T1.global_sample_id(u32::from(native_id))?;
        let sample = library.sample_by_id(global_id).ok_or_else(|| {
            library.describe_missing_with(Product::T1, u32::from(native_id), installed)
        })?;
        if sample.source_bpm != SOURCE_BPM
            || (native.is_some() && sample.sample_rate != legacy_format::NATIVE_SAMPLE_RATE)
        {
            return Err(format!(
                "T1 sample {native_id} has incompatible source timing in the active library."
            ));
        }
        let bars = if let Some((root, native)) = &native {
            let metadata = native
                .sample(native_id)
                .ok_or_else(|| format!("T1 sample {native_id} is not in the installed catalog."))?;
            let catalog_path = Path::new("\x50XD").join(&metadata.path);
            let source_bytes =
                read_bounded(&root.join(&catalog_path), PACKED_BYTE_LIMIT, "T1 sample")?;
            // The original's path range applies to the path inside the product,
            // not to the folder where this copy is kept.
            FileSample::from_bytes(catalog_path, &source_bytes)
                .map_err(|error| format!("Cannot use T1 sample {native_id}: {error}"))?;
            metadata
                .nominal_bars
                .ok_or_else(|| format!("T1 sample {native_id} has no recovered bar length."))?
        } else {
            sample.bars
        };
        Ok((global_id, bars))
    })?;
    if let Some(notice) = native_notice {
        project.source_notices.push(notice);
    }
    Ok(project)
}

/// Resolve each native source once, then use the recovered scheduler unchanged.
fn convert(
    mix: &Mix,
    mix_path: &Path,
    library: &Library,
    resolve_native: impl FnMut(u16) -> Result<(u32, u32), String>,
) -> Result<Project, String> {
    convert_with_catalog(mix, mix_path, library, resolve_native, |native_id| {
        crate::catalog::identity(Product::T1, native_id)
    })
}

fn convert_with_catalog<'a>(
    mix: &Mix,
    mix_path: &Path,
    library: &Library,
    mut resolve_native: impl FnMut(u16) -> Result<(u32, u32), String>,
    identity: impl Fn(u32) -> Option<&'a crate::catalog::WaveMetadata>,
) -> Result<Project, String> {
    if let Some(event) = mix
        .events()
        .find(|event| event.row >= legacy_format::editing::EDITABLE_ROWS)
    {
        return Err(format!(
            "The T1 MIX has sample {} in its nonplayable guard row. Remove that placement before import.",
            event.sample_id
        ));
    }
    let footer = MixFooter::parse(mix)
        .map_err(|error| format!("Cannot read the T1 MIX sample references: {error}"))?;
    let title = decode_1252(&footer.title.0);
    let title = if title.is_empty() || title == "\x52ave \x65Jay 1.01" {
        mix_path.file_stem().map_or_else(
            || "Untitled".into(),
            |name| name.to_string_lossy().into_owned(),
        )
    } else {
        title
    };
    let mut project = Project {
        title,
        bpm: SOURCE_BPM,
        master_gain: 1.0,
        ..Project::default()
    };
    for (track, route) in project.tracks.iter_mut().zip(footer.routes) {
        track.left_enabled = matches!(route, 0 | 1);
        track.right_enabled = matches!(route, 0 | 2);
    }

    let base = mix_path.parent().unwrap_or_else(|| Path::new("."));
    let mut sources = HashMap::new();
    for record in &footer.imports {
        let saved = import_reference_path(&record.path, base);
        let rejected = saved.is_err();
        let (saved_path, resolved) = match saved {
            Ok(saved) => {
                let resolved = resolve_import_path(&saved, &record.path, library).and_then(
                    |(path, candidates)| {
                        let bytes = read_bounded(&path, WAV_BYTE_LIMIT, "T1 imported WAV")?;
                        legacy_format::playback::check_wave(Path::new("import.wav"), &bytes)
                            .map_err(|error| {
                                format!(
                                    "Cannot use T1 imported sample {} at {}: {error}",
                                    record.source_id,
                                    path.display()
                                )
                            })?;
                        let duration = legacy_format::playback::wave_duration_bytes(&bytes)
                            .map_err(|error| {
                                format!("Cannot read T1 imported WAV timing: {error}")
                            })?;
                        let bars = imports::imported_nominal_bars(duration)
                            .map_err(|error| format!("Cannot schedule T1 imported WAV: {error}"))?;
                        Ok((path, bars, candidates))
                    },
                );
                (saved, resolved)
            }
            // A bad saved path makes one placeholder, not a failed import.
            Err(reason) => (PathBuf::from(decode_1252(&record.path.0)), Err(reason)),
        };
        let id = project.next_import_id()?;
        let name = record
            .labels
            .iter()
            .map(|text| decode_1252(&text.0))
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_owned();
        let name = if name.is_empty() {
            saved_path
                .file_stem()
                .map(|stem| stem.to_string_lossy().trim().to_owned())
                .filter(|stem| !stem.is_empty() && !stem.chars().any(char::is_control))
                .unwrap_or_else(|| format!("Sample {}", record.source_id))
        } else {
            name
        };
        let imported = ImportedSample {
            id,
            name,
            group_name: decode_1252(&record.category.0),
            path: saved_path,
            source_bpm: SOURCE_BPM,
            source_sample_rate: Some(legacy_format::NATIVE_SAMPLE_RATE),
            origin: Some(Product::T1),
        };
        let bars = match resolved {
            Ok((path, bars, candidates)) => {
                crate::library::record_wav_conflict(
                    &mut project,
                    &decode_1252(&record.path.0),
                    id,
                    candidates,
                );
                project
                    .imported_samples
                    .push(ImportedSample { path, ..imported });
                bars
            }
            Err(reason) => {
                let bars = inferred_bars(mix, record.source_id);
                let mut missing = MissingSample::external(&imported, bars * TICKS_PER_BAR, reason);
                let raw_path = decode_1252(&record.path.0);
                if rejected {
                    missing.reference = super::import_support::rejected_reference(
                        Product::T1,
                        &raw_path,
                        record.source_id,
                    );
                }
                for (key, value) in [
                    ("source_id", record.source_id.to_string()),
                    ("label_1", decode_1252(&record.labels[0].0)),
                    ("label_2", decode_1252(&record.labels[1].0)),
                    ("saved_path", raw_path),
                    ("category", decode_1252(&record.category.0)),
                ] {
                    missing.metadata.insert(key.into(), value);
                }
                project.missing_samples.push(missing);
                bars
            }
        };
        sources.insert(record.source_id, (id, bars));
    }
    for event in mix.events() {
        if sources.contains_key(&event.sample_id) {
            continue;
        }
        let unresolved_kit = footer
            .kit_regions
            .iter()
            .enumerate()
            .find(|(index, region)| {
                let (first, last) = DEFAULT_KIT_REGIONS[*index];
                (region.first_id..=region.last_id).contains(&event.sample_id)
                    && (!region.name.0.is_empty()
                        || region.first_id != first
                        || region.last_id != last)
            })
            .map(|(_, region)| region);
        let resolved = if unresolved_kit.is_some() {
            Err(
                "The saved T1 expansion kit name or ID range cannot be matched to the active \
                 catalog."
                    .into(),
            )
        } else {
            resolve_native(event.sample_id)
        };
        let source = match resolved {
            Ok(source) => source,
            Err(reason) => {
                let (mut missing, bars, _) = super::d1::native_placeholder(
                    Product::T1,
                    event.sample_id,
                    inferred_bars(mix, event.sample_id),
                    reason,
                    if unresolved_kit.is_some() {
                        None
                    } else {
                        identity(u32::from(event.sample_id))
                    },
                )?;
                if let Some(kit) = unresolved_kit {
                    missing.id = project.next_import_id()?;
                    let name = decode_1252(&kit.name.0);
                    missing.reference = MissingReference::Generated {
                        product: Product::T1,
                        kind: "unresolved-kit".into(),
                        data: serde_json::json!({"native_id": event.sample_id, "first_id": kit.first_id, "last_id": kit.last_id, "name": name}),
                    };
                    missing.metadata.insert("kit_name".into(), name);
                    missing
                        .metadata
                        .insert("source_id".into(), event.sample_id.to_string());
                    missing
                        .metadata
                        .insert("kit_first_id".into(), kit.first_id.to_string());
                    missing
                        .metadata
                        .insert("kit_last_id".into(), kit.last_id.to_string());
                }
                let id = missing.id;
                project.missing_samples.push(missing);
                (id, bars)
            }
        };
        sources.insert(event.sample_id, source);
    }
    let scheduled = timeline::schedule(mix, |native_id| {
        sources.get(&native_id).map(|(_, bars)| *bars)
    })
    .map_err(|error| format!("Cannot schedule the T1 MIX: {error}"))?;
    if scheduled.len() != mix.events().count() {
        return Err(
            "The T1 MIX contains placements that the recovered scheduler cannot play.".into(),
        );
    }
    for event in scheduled {
        let (sample_id, _) = sources
            .get(&event.sample_id)
            .ok_or("A scheduled T1 sample has no resolved source.")?;
        project.clips.push(Clip {
            id: project.clips.len() as u64 + 1,
            sample_id: *sample_id,
            track: event.track,
            start_tick: frame_to_tick(event.start_frame)?,
            length_ticks: frame_to_tick(event.limit_frames)?,
            offset_frames: 0,
            gain: 1.0,
            fades: Default::default(),
            pitch_cents: 0,
            reverse: false,
        });
    }
    normalize_import(project)
}

fn inferred_bars(mix: &Mix, source_id: u16) -> u32 {
    let events: Vec<_> = mix
        .events()
        .map(|event| (event.row, event.track, event.sample_id))
        .collect();
    super::import_support::inferred_bars(&events, source_id)
}

fn frame_to_tick(frame: u64) -> Result<u32, String> {
    let scaled = frame
        .checked_mul(u64::from(TICKS_PER_BAR))
        .ok_or("The T1 timeline exceeds the supported range.")?;
    if !scaled.is_multiple_of(u64::from(timeline::FRAMES_PER_BAR)) {
        return Err("The T1 timeline cannot be represented at the project tick resolution.".into());
    }
    u32::try_from(scaled / u64::from(timeline::FRAMES_PER_BAR))
        .map_err(|_| "The T1 timeline exceeds the supported range.".into())
}

fn import_reference_path(raw: &ByteText, base: &Path) -> Result<PathBuf, String> {
    if raw.0.is_empty() || raw.0.iter().any(|byte| *byte < 32 || *byte == 127) {
        return Err("The T1 imported sample path is empty or contains a control byte.".into());
    }
    super::import_support::saved_path(
        &decode_1252(&raw.0),
        || {
            if base.is_absolute() {
                Ok(base.to_path_buf())
            } else {
                std::env::current_dir()
                    .map(|dir| dir.join(base))
                    .map_err(|error| format!("Cannot resolve the T1 project folder: {error}"))
            }
        },
        "The T1 imported sample needs an absolute drive or a relative project path.",
    )
}

fn resolve_import_path(
    path: &Path,
    raw: &ByteText,
    library: &Library,
) -> Result<(PathBuf, Vec<PathBuf>), String> {
    let reference = decode_1252(&raw.0).replace('\\', "/");
    super::import_support::find_file(path, &reference, library)
}

fn read_bounded(path: &Path, limit: usize, kind: &str) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("Cannot open {kind} {}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read {kind} {}: {error}", path.display()))?;
    if bytes.len() > limit {
        return Err(format!("The {kind} exceeds its supported file-size limit."));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
