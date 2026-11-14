use super::*;

#[test]
fn signed_legacy_json_migrates_polarity_in_all_earlier_schemas() {
    for (schema, slots) in [(0, 16), (1, 32), (2, 64), (3, 64)] {
        let mut value = serde_json::to_value(Project::default()).unwrap();
        if schema == 0 {
            value.as_object_mut().unwrap().remove("schema_version");
        } else {
            value["schema_version"] = schema.into();
        }
        value["tracks"].as_array_mut().unwrap().truncate(slots);
        for track in value["tracks"].as_array_mut().unwrap() {
            track.as_object_mut().unwrap().remove("polarity_inverted");
        }
        value["tracks"][0]["gain"] = (-0.7533).into();
        value["tracks"][1]["gain"] = (-0.0_f32).into();
        value["tracks"][2]["gain"] = 4.0.into();
        let migrated = Project::parse_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
        assert_eq!(migrated.tracks[0].gain, 0.7533);
        assert!(migrated.tracks[0].polarity_inverted);
        assert_eq!(migrated.tracks[0].effective_gain(), -0.7533);
        assert_eq!(migrated.tracks[1].gain, 0.0);
        assert!(migrated.tracks[1].polarity_inverted);
        assert!(migrated.tracks[1].effective_gain().is_sign_negative());
        assert_eq!(migrated.tracks[2].gain, 4.0);
        assert!(!migrated.tracks[2].polarity_inverted);
        assert_eq!(
            Project::parse_json(&serde_json::to_vec(&migrated).unwrap()).unwrap(),
            migrated
        );
        value["tracks"][0]["gain"] = (-2.01).into();
        assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}

#[test]
fn earlier_json_master_keys_keep_the_board_settings_and_order() {
    let mut project = Project::default();
    project.master.booster_eq.on = true;
    project.master.booster_eq.bands[4] = 71;
    project.master.booster_compressor.drive = 731;
    project.master.booster_wide.spread = 217;
    project.master.move_pedal(2, 0);
    let current = serde_json::to_string(&project).unwrap();
    let earlier = current
        .replace("boosterEq", "ejayEq")
        .replace("boosterCompressor", "ejayCompressor")
        .replace("boosterWide", "ejayWide");
    let opened = Project::parse_json(earlier.as_bytes()).unwrap();
    assert_eq!(opened, project);
    assert_eq!(
        Project::parse_json(&serde_json::to_vec(&opened).unwrap()).unwrap(),
        project
    );
    let duplicate = earlier.replace("\"ejayWide\":", "\"boosterWide\":{},\"ejayWide\":");
    assert!(Project::parse_json(duplicate.as_bytes()).is_err());
}

#[test]
fn paired_native_labels_and_signed_track_levels_survive_json() {
    let mut project = Project::default();
    project.tracks[31].set_legacy_gain(-2.0 / 65536.0);
    project.tracks[0].gain = 2.352_630_6;
    project.imported_samples.push(ImportedSample {
        id: 1_000_000,
        name: format!("{} {}", "ă".repeat(200), "Î".repeat(200)),
        group_name: "Wave".into(),
        path: "source.wav".into(),
        source_bpm: 2000.0,
        source_sample_rate: None,
        origin: None,
    });
    project.validate().unwrap();
    let restored: Project = serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
    assert_eq!(restored, project);
    project.tracks[31].gain = -2.01;
    assert!(project.validate().is_err());
    project.tracks[31].gain = 0.0;
    project.imported_samples[0].group_name.clear();
    assert!(project.validate().is_err());
}

#[test]
fn imported_track_shapes_preserve_values_and_pad_to_sixty_four() {
    for source_track_count in [1, 8, 16, 24, 32, 33, 48, 49, 64] {
        let mut project = Project::default();
        project.tracks.truncate(source_track_count);
        project.tracks[0] = Track {
            pan: 0.0,
            gain: 0.3,
            polarity_inverted: false,
            left_enabled: false,
            right_enabled: true,
            muted: true,
            solo: true,
            sends: [0.0; crate::fx::BUSES],
            automation: Default::default(),
            eq: Default::default(),
            drive: Default::default(),
            compressor: Default::default(),
            width: TRACK_WIDTH.default,
            ducker: Default::default(),
        };
        project.clips.push(Clip {
            id: 37,
            sample_id: 13_000,
            track: source_track_count - 1,
            start_tick: 17,
            length_ticks: 31,
            offset_frames: 7,
            gain: 0.5,
            fades: Default::default(),
            pitch_cents: 0,
            reverse: false,
        });
        project.loop_region = Some([3, 999]);
        let original = project.clone();
        let normalized = normalize_import(project).unwrap();
        assert_eq!(normalized.tracks.len(), 64);
        assert_eq!(normalized.tracks[..source_track_count], original.tracks);
        assert!(
            normalized.tracks[source_track_count..]
                .iter()
                .all(|track| *track == Track::default())
        );
        assert_eq!(normalized.clips, original.clips);
        assert_eq!(normalized.loop_region, original.loop_region);
        assert!(normalized.import_notices.is_empty());
        normalized.validate().unwrap();
    }
}

#[test]
fn imported_invalid_shapes_and_missing_source_tracks_are_rejected() {
    for source_track_count in [0, 65, 99] {
        let mut project = Project::default();
        project.tracks.resize(source_track_count, Track::default());
        assert!(normalize_import(project).is_err());
    }
    for source_track_count in [8, 16] {
        let mut project = Project::default();
        project.tracks.truncate(source_track_count);
        project.clips.push(Clip {
            id: 1,
            sample_id: 1,
            track: source_track_count,
            start_tick: 0,
            length_ticks: 256,
            offset_frames: 0,
            gain: 1.0,
            fades: Default::default(),
            pitch_cents: 0,
            reverse: false,
        });
        assert!(normalize_import(project).is_err());
    }
    let project = Project {
        schema_version: PROJECT_SCHEMA_VERSION + 1,
        ..Project::default()
    };
    assert!(normalize_import(project).is_err());
}

#[test]
fn json_rejects_unknown_versions_and_malformed_legacy_shapes() {
    let mut value = serde_json::to_value(Project::default()).unwrap();
    for version in [0, PROJECT_SCHEMA_VERSION + 1, u32::MAX] {
        value["schema_version"] = version.into();
        assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    value.as_object_mut().unwrap().remove("schema_version");
    assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
    value["tracks"].as_array_mut().unwrap().truncate(16);
    value["bpm"] = 301.0.into();
    assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
    value["bpm"] = 140.0.into();
    value["pitch_ratio"] = 4.01.into();
    assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
    let path = std::env::temp_dir().join(format!(
        "mixjam-oversized-project-{}.json",
        std::process::id()
    ));
    let file = fs::File::create(&path).unwrap();
    file.set_len(MAX_PROJECT_BYTES as u64 + 1).unwrap();
    drop(file);
    let result = Project::load(&path);
    fs::remove_file(&path).unwrap();
    assert_eq!(result.unwrap_err(), "The project file is too large.");
}

#[test]
fn version_one_migration_only_expands_track_capacity() {
    let mut original = Project {
        schema_version: 1,
        ..Project::default()
    };
    original.tracks.truncate(32);
    original.tracks[31].pan = -0.75;
    original.clips.push(Clip {
        id: 55,
        sample_id: 731,
        track: 31,
        start_tick: 17,
        length_ticks: 45,
        offset_frames: 9,
        gain: 0.5,
        fades: Default::default(),
        pitch_cents: 0,
        reverse: false,
    });
    original.import_notices.push(ImportNotice {
        source_bpm: 146.0,
        target_bpm: 140.0,
    });
    let mut value = serde_json::to_value(&original).unwrap();
    let migrated = Project::parse_json(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
    assert_eq!(migrated.tracks.len(), 64);
    assert_eq!(migrated.tracks[..32], original.tracks);
    assert_eq!(migrated.clips, original.clips);
    assert_eq!(migrated.import_notices, original.import_notices);
    assert_eq!(migrated.bpm, original.bpm);
    assert_eq!(migrated.pitch_ratio, original.pitch_ratio);
    assert_eq!(
        Project::parse_json(&serde_json::to_vec(&migrated).unwrap()).unwrap(),
        migrated
    );

    value["clips"][0]["track"] = 32.into();
    assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
    value["clips"][0]["track"] = 31.into();
    value["bpm"] = 146.into();
    assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
    value["bpm"] = 140.into();
    value["tracks"].as_array_mut().unwrap().pop();
    assert!(Project::parse_json(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn schema_two_migration_preserves_the_arrangement_and_adds_empty_missing_sources() {
    let mut value = serde_json::to_value(Project::default()).unwrap();
    value["schema_version"] = 2.into();
    value.as_object_mut().unwrap().remove("missing_samples");
    let migrated = Project::parse_json(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(migrated, Project::default());
}

#[test]
fn old_sixteen_track_json_migrates_saves_and_reopens_with_one_notice() {
    let mut old_project = Project {
        title: "Original arrangement".into(),
        author: "Original author".into(),
        master_gain: 0.6,
        pitch_ratio: 146.0 / 140.0,
        loop_region: Some([17, 512]),
        ..Project::default()
    };
    old_project.tracks.truncate(16);
    old_project.tracks[15].gain = 0.25;
    old_project.tracks[15].right_enabled = false;
    old_project.imported_samples.push(ImportedSample {
        id: 1_000_000,
        name: "Imported source".into(),
        group_name: "Imported".into(),
        path: PathBuf::from("original-source.wav"),
        source_bpm: 127.0,
        source_sample_rate: None,
        origin: None,
    });
    old_project.clips.push(Clip {
        id: 19,
        sample_id: 1_000_000,
        track: 15,
        start_tick: 17,
        length_ticks: 31,
        offset_frames: 23,
        gain: 0.4,
        fades: Default::default(),
        pitch_cents: 0,
        reverse: false,
    });
    let mut value = serde_json::to_value(&old_project).unwrap();
    value.as_object_mut().unwrap().remove("schema_version");
    value.as_object_mut().unwrap().remove("import_notices");
    value.as_object_mut().unwrap().remove("libraries");
    value["imported_samples"][0]
        .as_object_mut()
        .unwrap()
        .remove("origin");
    let directory = std::env::temp_dir().join(format!(
        "mixjam-techno-legacy-migration-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let old_path = directory.join("old.json");
    let current_path = directory.join("current.json");
    fs::write(&old_path, serde_json::to_vec(&value).unwrap()).unwrap();
    let migrated = Project::load(&old_path).unwrap();
    assert_eq!(migrated.schema_version, PROJECT_SCHEMA_VERSION);
    assert_eq!(migrated.tracks.len(), 64);
    assert_eq!(migrated.tracks[..16], old_project.tracks);
    assert_eq!(migrated.clips, old_project.clips);
    assert_eq!(migrated.imported_samples, old_project.imported_samples);
    assert_eq!(migrated.loop_region, old_project.loop_region);
    assert_eq!(migrated.title, old_project.title);
    assert_eq!(migrated.author, old_project.author);
    assert_eq!(migrated.master_gain, old_project.master_gain);
    assert_eq!(migrated.bpm, 140.0);
    assert_eq!(migrated.pitch_ratio, 1.0);
    assert_eq!(migrated.import_notices.len(), 1);
    assert!((migrated.import_notices[0].source_bpm - 146.0).abs() < 1e-12);
    migrated.save(&current_path).unwrap();
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(&current_path).unwrap()).unwrap();
    assert_eq!(saved["schema_version"], PROJECT_SCHEMA_VERSION);
    assert_eq!(Project::load(&current_path).unwrap(), migrated);
    fs::remove_file(&old_path).unwrap();
    fs::remove_file(&current_path).unwrap();
    fs::remove_dir(&directory).unwrap();
}

#[test]
fn fractional_original_positions_survive_save() {
    let mut e = Editor::new(Project::default());
    e.place(13000, 0, 17, 31).unwrap();
    let p: Project = serde_json::from_str(&serde_json::to_string(&e.project).unwrap()).unwrap();
    assert_eq!(p, e.project);
    assert!((p.frames_per_tick(44100) * 256.0 - 75600.0).abs() < 1e-8);
}

#[test]
fn save_replaces_the_project_beside_an_existing_temporary_file() {
    let directory = std::env::temp_dir().join(format!("mixjam-techno-save-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("mix.json");
    let temp = directory.join(format!(".mix.json.{}.tmp", std::process::id()));
    fs::write(&temp, b"unrelated existing file").unwrap();
    let p = Project::default();
    p.save(&path).unwrap();
    assert_eq!(fs::read(&temp).unwrap(), b"unrelated existing file");
    assert_eq!(Project::load(&path).unwrap(), p);
    fs::remove_file(&temp).unwrap();
    fs::remove_file(&path).unwrap();
    fs::remove_dir(&directory).unwrap();
}
