//! Import the recovered T1 profile as editable common-project clips.
use crate::{
    engine::{
        Clip, ImportedSample, MissingReference, MissingSample, Project, TICKS_PER_BAR,
        normalize_import,
    },
    library::Library,
    products::{IMPORTED_ID_START, Product},
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
    path::{Component, Path, PathBuf},
};

const PACKED_BYTE_LIMIT: usize = 64 * 1024 * 1024;
const WAV_BYTE_LIMIT: usize = 128 * 1024 * 1024;
const SOURCE_BPM: f64 = 180.0;
const DEFAULT_KIT_REGIONS: [(u16, u16); 3] = [(4400, 4999), (5000, 5624), (5625, 6299)];

pub fn load(path: &Path, library: &Library) -> Result<Project, String> {
    let bytes = super::song_file::read_song_file(path)?;
    let mix = Mix::parse(&bytes).map_err(|error| format!("Cannot read the T1 MIX: {error}"))?;
    let root = library.source_root(Product::T1);
    let native = root
        .filter(|_| !library.is_wav_product(Product::T1))
        .map(|root| {
            legacy_format::library::Library::load(root)
                .map(|native| (root, native))
                .map_err(|error| format!("Cannot read the T1 sample catalogs: {error}"))
        })
        .and_then(Result::ok);
    convert(&mix, path, library, |native_id| {
        let global_id = Product::T1.global_sample_id(u32::from(native_id))?;
        let sample = library
            .sample_by_id(global_id)
            .ok_or_else(|| library.describe_missing(Product::T1, u32::from(native_id)))?;
        if sample.source_bpm != SOURCE_BPM || (native.is_some() && sample.sample_rate != 44_100) {
            return Err(format!(
                "T1 sample {native_id} has incompatible source timing in the active library."
            ));
        }
        let bars = if let Some((root, native)) = &native {
            let metadata = native
                .sample(native_id)
                .ok_or_else(|| format!("T1 sample {native_id} is not in the installed catalog."))?;
            let source_path = root.join("\x50XD").join(&metadata.path);
            let source_bytes = read_bounded(&source_path, PACKED_BYTE_LIMIT, "T1 sample")?;
            FileSample::from_bytes(source_path, &source_bytes)
                .map_err(|error| format!("Cannot use T1 sample {native_id}: {error}"))?;
            metadata
                .nominal_bars
                .ok_or_else(|| format!("T1 sample {native_id} has no recovered bar length."))?
        } else {
            sample.bars
        };
        Ok((global_id, bars))
    })
}

/// Resolve each native source once, then use the recovered scheduler unchanged.
fn convert(
    mix: &Mix,
    mix_path: &Path,
    library: &Library,
    mut resolve_native: impl FnMut(u16) -> Result<(u32, u32), String>,
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
    let title = footer.title.byte_view();
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
    let catalog = crate::catalog::wave_metadata(Product::T1)?;
    let mut sources = HashMap::new();
    for record in &footer.imports {
        let saved_path = import_reference_path(&record.path, base)?;
        let resolved =
            resolve_import_path(&record.path, base, library).and_then(|(path, candidates)| {
                let bytes = read_bounded(&path, WAV_BYTE_LIMIT, "T1 imported WAV")?;
                FileSample::from_wave(PathBuf::from("import.wav"), &bytes).map_err(|error| {
                    format!(
                        "Cannot use T1 imported sample {} at {}: {error}",
                        record.source_id,
                        path.display()
                    )
                })?;
                let duration = legacy_format::playback::wave_duration_bytes(&bytes)
                    .map_err(|error| format!("Cannot read T1 imported WAV timing: {error}"))?;
                let bars = imports::imported_nominal_bars(duration)
                    .map_err(|error| format!("Cannot schedule T1 imported WAV: {error}"))?;
                Ok((path, bars, candidates))
            });
        let id = IMPORTED_ID_START
            .checked_add((project.imported_samples.len() + project.missing_samples.len()) as u32)
            .ok_or("The imported sample ID limit was reached.")?;
        let name = record
            .labels
            .iter()
            .map(|text| crate::library::decode_1252(&text.0))
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_owned();
        let name = if name.is_empty() {
            saved_path.file_stem().map_or_else(
                || format!("Sample {}", record.source_id),
                |stem| stem.to_string_lossy().into_owned(),
            )
        } else {
            name
        };
        let imported = ImportedSample {
            id,
            name,
            group_name: crate::library::decode_1252(&record.category.0),
            path: saved_path,
            source_bpm: SOURCE_BPM,
            source_sample_rate: Some(44_100),
            origin: Some(Product::T1),
        };
        let bars = match resolved {
            Ok((path, bars, candidates)) => {
                crate::library::record_wav_conflict(
                    &mut project,
                    &crate::library::decode_1252(&record.path.0),
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
                for (key, value) in [
                    ("source_id", record.source_id.to_string()),
                    ("label_1", crate::library::decode_1252(&record.labels[0].0)),
                    ("label_2", crate::library::decode_1252(&record.labels[1].0)),
                    ("saved_path", crate::library::decode_1252(&record.path.0)),
                    ("category", crate::library::decode_1252(&record.category.0)),
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
            Err("The saved T1 expansion kit name or ID range cannot be matched to the active catalog.".into())
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
                        catalog
                            .iter()
                            .find(|sample| sample.native_id == u32::from(event.sample_id))
                    },
                )?;
                if let Some(kit) = unresolved_kit {
                    missing.id = IMPORTED_ID_START
                        .checked_add(
                            (project.imported_samples.len() + project.missing_samples.len()) as u32,
                        )
                        .ok_or("The imported sample ID limit was reached.")?;
                    let name = crate::library::decode_1252(&kit.name.0);
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
        });
    }
    normalize_import(project)
}

fn inferred_bars(mix: &Mix, source_id: u16) -> u32 {
    mix.events()
        .filter(|event| event.sample_id == source_id)
        .map(|event| {
            mix.events()
                .find(|next| next.track == event.track && next.row > event.row)
                .map_or(1, |next| (next.row - event.row) as u32)
        })
        .max()
        .unwrap_or(1)
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
    let reference = crate::library::decode_1252(&raw.0).replace('\\', "/");
    let path = PathBuf::from(&reference);
    let path = if path.is_absolute() {
        path
    } else {
        if path.has_root()
            || path
                .components()
                .any(|part| matches!(part, Component::Prefix(_)))
        {
            return Err(
                "The T1 imported sample needs an absolute drive or a relative project path.".into(),
            );
        }
        let base = if base.is_absolute() {
            base.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|error| format!("Cannot resolve the T1 project folder: {error}"))?
                .join(base)
        };
        base.join(path)
    };
    Ok(path)
}

fn resolve_import_path(
    raw: &ByteText,
    base: &Path,
    library: &Library,
) -> Result<(PathBuf, Vec<PathBuf>), String> {
    let path = import_reference_path(raw, base)?;
    let reference = crate::library::decode_1252(&raw.0).replace('\\', "/");
    match crate::preservation::resolve_asset(&path) {
        Ok(found) if found.is_file() => Ok((found, Vec::new())),
        _ => {
            let candidates = library.find_wav_candidates(&reference)?;
            let path = candidates.first().cloned().ok_or_else(|| {
                format!("WAV sample {reference} is missing from the selected folder.")
            })?;
            Ok((path, candidates))
        }
    }
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
mod tests {
    use super::*;
    use legacy_format::imports::{ImportedSample as NativeImport, KitRegion};
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn text(value: &str) -> ByteText {
        ByteText::from_text(value).unwrap()
    }

    fn footer(mix: &mut Mix, routes: [u16; 8]) {
        mix.trailer = MixFooter::new(text("\x52ave \x65Jay 1.01"), routes)
            .to_bytes(mix)
            .unwrap();
    }

    fn resolve(native_id: u16) -> Result<(u32, u32), String> {
        match native_id {
            731 | 732 | 4400 => Ok((Product::T1.global_sample_id(u32::from(native_id))?, 2)),
            _ => Err(format!("Unknown synthetic T1 sample {native_id}.")),
        }
    }

    #[test]
    fn keeps_offsets_native_duration_caps_routes_and_editable_clips() {
        let mut mix = Mix::default();
        mix.set_cell(0, 0, 731, 4).unwrap();
        mix.set_cell(1, 0, 732, 12).unwrap();
        mix.set_cell(4, 7, 4400, 0).unwrap();
        footer(&mut mix, [0, 1, 2, 3, 0, 0, 0, 3]);
        let project =
            convert(&mix, Path::new("synthetic.mix"), &Library::empty(), resolve).unwrap();
        assert_eq!(project.title, "synthetic");
        assert!(project.author.is_empty());
        assert_eq!(project.bpm, 180.0);
        assert_eq!(project.pitch_ratio, 1.0);
        assert_eq!(project.tracks.len(), 64);
        assert_eq!(project.clips.len(), 3);
        assert_eq!(project.clips[0].sample_id, 100_731);
        assert_eq!(
            (project.clips[0].start_tick, project.clips[0].length_ticks),
            (64, 384)
        );
        assert_eq!(
            (project.clips[1].start_tick, project.clips[1].length_ticks),
            (448, 576)
        );
        assert_eq!(project.clips[2].track, 7);
        assert_eq!(
            (project.clips[2].start_tick, project.clips[2].length_ticks),
            (1024, 512)
        );
        assert!(
            project
                .clips
                .iter()
                .all(|clip| clip.offset_frames == 0 && clip.gain == 1.0)
        );
        assert!(project.tracks[1].left_enabled && !project.tracks[1].right_enabled);
        assert!(!project.tracks[2].left_enabled && project.tracks[2].right_enabled);
        assert!(!project.tracks[3].left_enabled && !project.tracks[3].right_enabled);
        assert!(!project.tracks[7].left_enabled && !project.tracks[7].right_enabled);
        assert!(
            project.tracks[8..]
                .iter()
                .all(|track| track.left_enabled && track.right_enabled)
        );
        project.validate().unwrap();
    }

    #[test]
    fn preserves_unresolved_but_rejects_invalid_offset_and_nonplayable_placements() {
        let mut mix = Mix::default();
        mix.set_cell(0, 0, 733, 0).unwrap();
        footer(&mut mix, [0; 8]);
        let project = convert(&mix, Path::new("test.mix"), &Library::empty(), resolve).unwrap();
        assert_eq!(project.clips.len(), 1);
        assert_eq!(project.missing_samples.len(), 1);
        assert_eq!(
            project.missing_samples[0].id,
            Product::T1.global_sample_id(733).unwrap()
        );
        assert!(project.missing_samples[0].reason.contains("733"));
        mix.set_cell(0, 0, 731, 16).unwrap();
        footer(&mut mix, [0; 8]);
        assert!(
            convert(&mix, Path::new("test.mix"), &Library::empty(), resolve)
                .unwrap_err()
                .contains("offset")
        );
        mix.set_cell(0, 0, 731, 0).unwrap();
        mix.set_cell(350, 0, 731, 0).unwrap();
        footer(&mut mix, [0; 8]);
        assert!(
            convert(&mix, Path::new("test.mix"), &Library::empty(), resolve)
                .unwrap_err()
                .contains("guard row")
        );
        mix.set_cell(350, 0, 0, 0).unwrap();
        mix.set_cell(0, 0, 65535, 0).unwrap();
        assert!(
            convert(&mix, Path::new("test.mix"), &Library::empty(), resolve)
                .unwrap_err()
                .contains("signed sample ID")
        );
    }

    #[test]
    fn rejects_truncated_footer_and_preserves_unresolved_kit_mapping() {
        let mut mix = Mix::default();
        mix.set_cell(0, 0, 4400, 0).unwrap();
        footer(&mut mix, [0; 8]);
        mix.trailer.pop();
        assert!(convert(&mix, Path::new("test.mix"), &Library::empty(), resolve).is_err());
        let mut saved = MixFooter::new(text("Custom title"), [0; 8]);
        saved.kit_regions = [(4300, 4999), (5000, 5624), (5625, 6299)]
            .map(|(first_id, last_id)| KitRegion {
                first_id,
                last_id,
                name: text(""),
            })
            .to_vec();
        mix.trailer = saved.to_bytes(&mix).unwrap();
        let remapped = convert(&mix, Path::new("test.mix"), &Library::empty(), resolve).unwrap();
        assert_eq!(remapped.clips.len(), 1);
        assert_eq!(remapped.missing_samples.len(), 1);
        assert_eq!(remapped.missing_samples[0].metadata["kit_first_id"], "4300");
        assert!(matches!(&remapped.missing_samples[0].reference,
            MissingReference::Generated { kind, .. } if kind == "unresolved-kit"));
        assert!(remapped.missing_samples[0].length_inferred);
        saved.kit_regions[0].first_id = 4400;
        mix.trailer = saved.to_bytes(&mix).unwrap();
        assert_eq!(
            convert(&mix, Path::new("test.mix"), &Library::empty(), resolve)
                .unwrap()
                .title,
            "Custom title"
        );
        saved.kit_regions[0].name = text("Another kit");
        mix.trailer = saved.to_bytes(&mix).unwrap();
        let project = convert(&mix, Path::new("test.mix"), &Library::empty(), resolve).unwrap();
        let missing = &project.missing_samples[0];
        assert_eq!(missing.metadata["kit_name"], "Another kit");
        assert!(
            matches!(&missing.reference, MissingReference::Generated { kind, .. } if kind == "unresolved-kit")
        );
        assert!(missing.id >= IMPORTED_ID_START);
        assert!(missing.length_inferred);
    }

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "t1-adapter-{}-{nonce}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn wav_only_provider_imports_native_ids_without_original_catalog_or_packed() {
        let fixture = Fixture::new();
        let bytes = legacy_format::wav::PcmWave {
            sample_rate: 22_050,
            channels: 1,
            samples: vec![1000; 29_400],
        }
        .encode()
        .unwrap();
        fs::write(fixture.0.join("r1test.wav"), bytes).unwrap();
        let metadata = crate::catalog::WaveMetadata {
            native_id: 731,
            code: "r1test.\x70xd".into(),
            name: "Synthetic sample".into(),
            group_name: "Test".into(),
            bars: 2,
            source_bpm: 180.0,
            frame_count: 58_800,
            pcm_digest: None,
        };
        let mut library = Library::empty();
        library.import_wav_folder(&fixture.0, 180.0).unwrap();
        library
            .register_wav_product(Product::T1, &fixture.0, &[metadata])
            .unwrap();
        let mut mix = Mix::default();
        mix.set_cell(0, 0, 731, 0).unwrap();
        footer(&mut mix, [0; 8]);
        let path = fixture.0.join("song.mix");
        fs::write(&path, mix.to_bytes()).unwrap();
        let project = load(&path, &library).unwrap();
        assert_eq!(project.clips[0].sample_id, 100_731);
        // Catalog bars can differ from physical audio length.
        assert_eq!(project.clips[0].length_ticks, 512);
        assert_eq!(library.read_pcm(100_731).unwrap().frames(), 29_400);
        assert!(!fixture.0.join("\x50XD").exists());
    }

    #[test]
    fn moved_external_wavs_select_first_and_retain_all_conflict_candidates() {
        let fixture = Fixture::new();
        let bytes = legacy_format::wav::PcmWave {
            sample_rate: 22_050,
            channels: 2,
            samples: vec![1000; 58_800],
        }
        .encode()
        .unwrap();
        let wave_path = fixture.0.join("take.wav");
        fs::write(&wave_path, &bytes).unwrap();
        let mut library = Library::empty();
        library.import_wav_folder(&fixture.0, 180.0).unwrap();
        let mut mix = Mix::default();
        mix.set_cell(0, 0, 1880, 0).unwrap();
        mix.set_cell(4, 1, 1880, 0).unwrap();
        let mut saved = MixFooter::new(text("Moved song"), [0; 8]);
        saved.imports.push(NativeImport {
            source_id: 1880,
            labels: [text("Recorded"), text("sample")],
            path: text("missing-folder/take.wav"),
            category: text("voice"),
        });
        mix.trailer = saved.to_bytes(&mix).unwrap();
        let song_path = fixture.0.join("song.mix");
        fs::write(&song_path, mix.to_bytes()).unwrap();
        let project = load(&song_path, &library).unwrap();
        assert_eq!(fs::read(&project.imported_samples[0].path).unwrap(), bytes);
        let duplicate_folder = fixture.0.join("other");
        fs::create_dir(&duplicate_folder).unwrap();
        fs::write(duplicate_folder.join("take.wav"), &bytes).unwrap();
        library.import_wav_folder(&duplicate_folder, 180.0).unwrap();
        let project = load(&song_path, &library).unwrap();
        assert!(project.missing_samples.is_empty());
        assert_eq!(project.clips.len(), 2);
        let conflict = &project.sample_conflicts[0];
        assert_eq!(conflict.reference, "missing-folder/take.wav");
        assert_eq!(conflict.candidates.len(), 2);
        assert_eq!(conflict.selected, 0);
        assert!(!conflict.confirmed);
        assert_eq!(project.imported_samples[0].path, conflict.candidates[0]);
        assert_eq!(conflict.sample_ids, vec![project.imported_samples[0].id]);
        assert!(
            project
                .clips
                .iter()
                .all(|clip| clip.sample_id == conflict.sample_ids[0])
        );
    }

    #[test]
    fn preserves_relative_wav_references_distinct_ids_and_native_origin() {
        let fixture = Fixture::new();
        let wave_path = fixture.0.join("take.wav");
        let bytes = legacy_format::wav::PcmWave {
            sample_rate: 22_050,
            channels: 2,
            samples: vec![1000; 58_800],
        }
        .encode()
        .unwrap();
        fs::write(&wave_path, &bytes).unwrap();
        let mut mix = Mix::default();
        mix.set_cell(0, 0, 1880, 0).unwrap();
        mix.set_cell(2, 1, 1881, 8).unwrap();
        let mut saved = MixFooter::new(text("Recorded song"), [0; 8]);
        for (source_id, label) in [(1880, "first"), (1881, "second")] {
            saved.imports.push(NativeImport {
                source_id,
                labels: [text("Recorded"), text(label)],
                path: text("take.wav"),
                category: text("voice"),
            });
        }
        mix.trailer = saved.to_bytes(&mix).unwrap();
        let project = convert(
            &mix,
            &fixture.0.join("song.mix"),
            &Library::empty(),
            resolve,
        )
        .unwrap();
        assert_eq!(project.imported_samples.len(), 2);
        assert_eq!(project.imported_samples[0].id, IMPORTED_ID_START);
        assert_eq!(project.imported_samples[1].id, IMPORTED_ID_START + 1);
        assert_eq!(project.imported_samples[1].name, "Recorded second");
        for imported in &project.imported_samples {
            assert_eq!(imported.path, wave_path);
            assert_eq!(imported.origin, Some(Product::T1));
            assert_eq!(imported.source_bpm, 180.0);
        }
        assert_eq!(project.clips[0].sample_id, IMPORTED_ID_START);
        assert_eq!(project.clips[0].length_ticks, 256);
        assert_eq!(project.clips[1].sample_id, IMPORTED_ID_START + 1);
        assert_eq!(project.clips[1].start_tick, 640);
        assert_eq!(project.clips[1].length_ticks, 384);
        assert_eq!(fs::read(&wave_path).unwrap(), bytes);
        fs::remove_file(&wave_path).unwrap();
        let missing = convert(
            &mix,
            &fixture.0.join("song.mix"),
            &Library::empty(),
            resolve,
        )
        .unwrap();
        assert!(missing.imported_samples.is_empty());
        assert_eq!(missing.missing_samples.len(), 2);
        assert_eq!(missing.clips.len(), 2);
        assert_eq!(missing.missing_samples[0].id, IMPORTED_ID_START);
        assert_eq!(missing.missing_samples[1].id, IMPORTED_ID_START + 1);
        assert_eq!(missing.missing_samples[1].name, "Recorded second");
        assert_eq!(missing.missing_samples[1].metadata["label_1"], "Recorded");
        assert_eq!(missing.missing_samples[1].metadata["label_2"], "second");
        assert_eq!(missing.missing_samples[1].metadata["category"], "voice");
        assert_eq!(
            missing.missing_samples[1].metadata["saved_path"],
            "take.wav"
        );
        assert!(
            missing
                .missing_samples
                .iter()
                .all(|sample| sample.length_inferred)
        );
        assert_eq!(missing.clips[1].start_tick, 640);
        assert_eq!(missing.clips[1].length_ticks, 384);
        let encoded = serde_json::to_string(&missing).unwrap();
        let reopened: Project = serde_json::from_str(&encoded).unwrap();
        reopened.validate().unwrap();
        assert_eq!(
            serde_json::to_value(reopened).unwrap(),
            serde_json::to_value(missing).unwrap()
        );
        assert!(!wave_path.exists());
    }

    #[test]
    fn empty_library_preserves_catalog_lengths_and_all_native_placements() {
        let fixture = Fixture::new();
        let sample = crate::catalog::wave_metadata(Product::T1)
            .unwrap()
            .into_iter()
            .find(|sample| sample.native_id < 1880 && sample.bars > 1)
            .unwrap();
        let mut mix = Mix::default();
        mix.set_cell(0, 0, sample.native_id as u16, 0).unwrap();
        mix.set_cell(12, 2, sample.native_id as u16, 4).unwrap();
        footer(&mut mix, [0; 8]);
        let path = fixture.0.join("missing-native.mix");
        fs::write(&path, mix.to_bytes()).unwrap();
        let project = load(&path, &Library::empty()).unwrap();
        assert_eq!(project.clips.len(), 2);
        assert_eq!(project.missing_samples.len(), 1);
        let missing = &project.missing_samples[0];
        assert!(!missing.length_inferred);
        assert_eq!(missing.nominal_ticks, sample.bars * TICKS_PER_BAR);
        assert_eq!(missing.name, sample.name);
        assert_eq!(missing.metadata["code"], sample.code);
        assert_eq!(project.clips[0].length_ticks, sample.bars * TICKS_PER_BAR);
        assert_eq!(project.clips[1].start_tick, 12 * TICKS_PER_BAR + 64);
    }

    #[test]
    fn unknown_sample_infers_gap_without_dropping_later_placements() {
        let mut mix = Mix::default();
        mix.set_cell(0, 0, 6400, 0).unwrap();
        mix.set_cell(7, 0, 731, 0).unwrap();
        mix.set_cell(20, 1, 6400, 0).unwrap();
        footer(&mut mix, [0; 8]);
        let project = convert(&mix, Path::new("unknown.mix"), &Library::empty(), resolve).unwrap();
        assert_eq!(project.clips.len(), 3);
        assert_eq!(project.clips[0].length_ticks, 7 * TICKS_PER_BAR);
        assert_eq!(project.clips[2].length_ticks, 7 * TICKS_PER_BAR);
        assert_eq!(project.missing_samples[0].nominal_ticks, 7 * TICKS_PER_BAR);
        assert!(project.missing_samples[0].length_inferred);
    }

    #[test]
    #[ignore = "Set MIXJAM_TECHNO_APP_ROOT to the installed T1 folder."]
    fn all_fifteen_installed_demos_keep_every_placement_and_resolve_native_ids() {
        let root = PathBuf::from(
            std::env::var_os("MIXJAM_TECHNO_APP_ROOT").expect("Set MIXJAM_TECHNO_APP_ROOT"),
        );
        let library = Library::open_t1(&root).unwrap();
        let mut mixes = 0;
        let mut placements = 0;
        for entry in fs::read_dir(root.join("MIX")).unwrap() {
            let path = entry.unwrap().path();
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mix"))
            {
                continue;
            }
            let original = fs::read(&path).unwrap();
            let native = Mix::parse(&original).unwrap();
            let project =
                load(&path, &library).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert_eq!(
                project.clips.len(),
                native.events().count(),
                "{}",
                path.display()
            );
            assert_eq!(project.tracks.len(), 64);
            assert_eq!(project.bpm, 180.0);
            assert_eq!(project.pitch_ratio, 1.0);
            for clip in &project.clips {
                assert!(library.sample_by_id(clip.sample_id).is_some());
                assert_eq!(Product::sample_key(clip.sample_id).unwrap().0, Product::T1);
            }
            assert_eq!(fs::read(&path).unwrap(), original);
            project.validate().unwrap();
            placements += project.clips.len();
            mixes += 1;
        }
        assert_eq!(mixes, 15);
        assert_eq!(placements, 4852);
    }
}
