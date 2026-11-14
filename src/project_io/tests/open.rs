//! Opening songs: relinking, resolving sounds, and the saved timing of missing sources.
use super::*;
use crate::{
    engine::{ImportedSample, MissingSample, Project},
    library::Library,
    products::{LibraryBinding, Product},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[test]
fn a_song_relinks_a_file_moved_into_another_library_folder() {
    let root = std::env::temp_dir().join(format!("mixjam-moved-library-{}", std::process::id()));
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
            reverse: false,
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
            reverse: false,
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
            reverse: false,
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
        reverse: false,
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
        reverse: false,
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
        reverse: false,
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
    let root = std::env::temp_dir().join(format!("mixjam-techno-header-{}", std::process::id()));
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
        reverse: false,
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
    let mut missing = MissingSample::external(&imported, 1024, "Original file is missing.".into());
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
        reverse: false,
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
    let root = std::env::temp_dir().join(format!("mixjam-missing-identity-{}", std::process::id()));
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
        reverse: false,
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
