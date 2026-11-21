use super::*;
use crate::products::IMPORTED_ID_START;
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
    let project = convert(&mix, Path::new("synthetic.mix"), &Library::empty(), resolve).unwrap();
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
fn title_uses_the_same_windows_code_page_as_import_labels() {
    let mut mix = Mix::default();
    mix.set_cell(0, 0, 731, 0).unwrap();
    mix.trailer = MixFooter::new(ByteText(b"Night \x96 \x93Live\x94".to_vec()), [0; 8])
        .to_bytes(&mix)
        .unwrap();
    let project = convert(&mix, Path::new("title.mix"), &Library::empty(), resolve).unwrap();
    assert_eq!(project.title, "Night \u{2013} \u{201c}Live\u{201d}");
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
        label_key: crate::catalog::label_key("Synthetic sample"),
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
fn a_bad_saved_path_makes_one_placeholder_and_never_reaches_the_file_system() {
    let paths = [
        r"\\attacker\share\a.wav",
        "//attacker/share/b.wav",
        r"\\.\pipe\c.wav",
        r"\\?\UNC\attacker\share\d.wav",
        r"\root-relative.wav",
        "bad\u{7}byte.wav",
        r"C:\music\COM1",
        "nul",
    ];
    let mut mix = Mix::default();
    let mut saved = MixFooter::new(text("Hostile song"), [0; 8]);
    for (index, path) in paths.iter().enumerate() {
        let id = 1880 + index as u16;
        mix.set_cell(index * 2, 0, id, 0).unwrap();
        saved.imports.push(NativeImport {
            source_id: id,
            labels: [text("Recorded"), text("take")],
            path: text(path),
            category: text("voice"),
        });
    }
    mix.trailer = saved.to_bytes(&mix).unwrap();
    let project = convert(
        &mix,
        Path::new("d:/songs/hostile.mix"),
        &Library::empty(),
        resolve,
    )
    .unwrap();
    assert!(project.imported_samples.is_empty());
    assert_eq!(project.missing_samples.len(), paths.len());
    assert_eq!(project.clips.len(), paths.len());
    for (missing, path) in project.missing_samples.iter().zip(paths) {
        // A rejected path never becomes a "missing file" or "cannot open" reason.
        assert!(
            missing.reason.contains("needs an absolute drive")
                || missing.reason.contains("control byte"),
            "{path}: {}",
            missing.reason
        );
        assert_eq!(missing.metadata["saved_path"], path);
    }
}

#[test]
fn empty_saved_paths_make_distinct_missing_sources() {
    let mut mix = Mix::default();
    mix.set_cell(0, 0, 1880, 0).unwrap();
    mix.set_cell(2, 0, 1881, 0).unwrap();
    // The writer cannot make this fixture because it requires a path.
    // The title and both import records use empty wrapped text fields.
    let empty_text = [2, 0, 0, 1];
    mix.trailer.extend([8, 10, 1, 0]);
    mix.trailer.extend(empty_text);
    mix.trailer.extend([2, 0]);
    for _ in 0..2 {
        mix.trailer.extend([8, 0]);
        for _ in 0..4 {
            mix.trailer.extend(empty_text);
        }
    }
    mix.trailer.extend([8, 0]);
    mix.trailer.extend([0; 16]);

    let project = convert(&mix, Path::new("song.mix"), &Library::empty(), resolve).unwrap();
    assert!(project.imported_samples.is_empty());
    assert_eq!(project.clips.len(), 2);
    assert_eq!(project.missing_samples.len(), 2);
    for (clip, (missing, source)) in project
        .clips
        .iter()
        .zip(project.missing_samples.iter().zip([1880, 1881]))
    {
        assert_eq!(clip.sample_id, missing.id);
        assert_eq!(
            super::super::rejected_import_path(&missing.reference),
            Some(("", Some(source)))
        );
    }
}

#[cfg(windows)]
#[test]
fn a_rejected_path_stays_missing_on_open_and_reopen() {
    let fixture = Fixture::new();
    let wave_path = fixture.0.join("take.wav");
    let bytes = legacy_format::wav::PcmWave {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![1000; 44_100],
    }
    .encode()
    .unwrap();
    fs::write(&wave_path, bytes).unwrap();
    let library = Library::open_wav_folder(&fixture.0, 180.0).unwrap();

    let mut mix = Mix::default();
    mix.set_cell(0, 0, 1880, 0).unwrap();
    let mut saved = MixFooter::new(text("Saved song"), [0; 8]);
    let reference = r"\take.wav";
    saved.imports.push(NativeImport {
        source_id: 1880,
        labels: [text("Recorded"), text("sample")],
        path: text(reference),
        category: text("voice"),
    });
    mix.trailer = saved.to_bytes(&mix).unwrap();
    let mix_path = fixture.0.join("song.mix");
    fs::write(&mix_path, mix.to_bytes()).unwrap();

    let (opened, _) = crate::project_io::open(&mix_path, &library).unwrap();
    let id = opened.clips[0].sample_id;
    let missing = opened
        .missing_sample(id)
        .expect("the rejected path stays missing");
    assert_eq!(missing.metadata["saved_path"], reference);
    assert!(missing.reason.contains("needs an absolute drive"));

    let song_path = fixture.0.join("song.mixjam");
    crate::project_io::save(&opened, &song_path).unwrap();
    let (reopened, library) = crate::project_io::open(&song_path, &library).unwrap();
    assert_eq!(reopened.missing_sample(id), Some(missing));

    let mut session = crate::application::Session::headless(std::sync::Arc::new(library), reopened)
        .unwrap()
        .with_settings_root(fixture.0.join("settings"));
    session.resolve_missing_sample(id, &wave_path).unwrap();
    assert!(session.missing_sample(id).is_none());
    assert_eq!(session.project().clips[0].sample_id, id);
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
    let sample = crate::catalog::WaveMetadata {
        native_id: 731,
        code: "fixture.pxd".into(),
        label_key: crate::catalog::label_key("Fixture"),
        group_name: "Bass".into(),
        bars: 2,
        source_bpm: 180.0,
        frame_count: 44_100,
        pcm_digest: None,
    };
    let mut mix = Mix::default();
    mix.set_cell(0, 0, sample.native_id as u16, 0).unwrap();
    mix.set_cell(12, 2, sample.native_id as u16, 4).unwrap();
    footer(&mut mix, [0; 8]);
    let path = fixture.0.join("missing-native.mix");
    fs::write(&path, mix.to_bytes()).unwrap();
    let bytes = fs::read(&path).unwrap();
    let parsed = Mix::parse(&bytes).unwrap();
    let project = convert_with_catalog(
        &parsed,
        &path,
        &Library::empty(),
        |_| Err("Audio unavailable.".into()),
        |id| (id == sample.native_id).then_some(&sample),
    )
    .unwrap();
    assert_eq!(project.clips.len(), 2);
    assert_eq!(project.missing_samples.len(), 1);
    let missing = &project.missing_samples[0];
    assert!(!missing.length_inferred);
    assert_eq!(missing.nominal_ticks, sample.bars * TICKS_PER_BAR);
    assert_eq!(missing.name, sample.display_name(Product::T1));
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
fn rejected_import_path_with_blank_or_control_stem_becomes_a_valid_placeholder() {
    let mut mix = Mix::default();
    let mut saved = MixFooter::new(text("Blank stem"), [0; 8]);
    for (slot, (source_id, raw_path)) in [
        (1880, "\t.wav"),
        (1881, "   .wav"),
        (1882, "bad\x07byte.wav"),
    ]
    .into_iter()
    .enumerate()
    {
        mix.set_cell(slot, 0, source_id, 0).unwrap();
        saved.imports.push(NativeImport {
            source_id,
            labels: [text(""), text("")],
            path: text(raw_path),
            category: text("voice"),
        });
    }
    mix.trailer = saved.to_bytes(&mix).unwrap();
    let project = convert(
        &mix,
        Path::new("blank-stem.mix"),
        &Library::empty(),
        resolve,
    )
    .expect("rejected paths with blank or control stems must make placeholders");
    project.validate().unwrap();
    assert_eq!(project.missing_samples.len(), 3);
    assert_eq!(project.missing_samples[0].name, "Sample 1880");
    assert_eq!(project.missing_samples[1].name, "Sample 1881");
    assert_eq!(project.missing_samples[2].name, "Sample 1882");
}

#[test]
fn failed_native_library_load_keeps_notice_in_opened_project() {
    let fixture = Fixture::new();
    let mut library = Library::empty();
    // Point the T1 installation binding to a folder lacking its sample catalog.
    library.bind_source_root_for_test(Product::T1, fixture.0.clone());
    let mut mix = Mix::default();
    mix.set_cell(0, 0, 731, 0).unwrap();
    footer(&mut mix, [0; 8]);
    let song_path = fixture.0.join("broken-root.mix");
    fs::write(&song_path, mix.to_bytes()).unwrap();
    let project = load(&song_path, &library).unwrap();
    let expected = legacy_format::library::Library::load(&fixture.0)
        .unwrap_err()
        .to_string();
    assert!(
        project
            .source_notices
            .iter()
            .any(|notice| notice.contains(&expected)),
        "expected error {expected:?} in {:?}",
        project.source_notices
    );
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
