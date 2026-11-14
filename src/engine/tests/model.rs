use super::*;

#[test]
fn a_section_lasts_until_the_next_one_starts() {
    let section = |bar: u32, name: &str| Section {
        start_tick: bar * TICKS_PER_BAR,
        name: name.into(),
    };
    let mut project = Project::default();
    assert_eq!(project.section_span(0), None);
    project.sections = vec![
        section(4, "Intro"),
        section(12, "Drift"),
        section(20, "Outro"),
    ];
    project.clips.push(Clip {
        id: 1,
        sample_id: 1,
        track: 0,
        start_tick: 0,
        length_ticks: 24 * TICKS_PER_BAR,
        offset_frames: 0,
        gain: 1.0,
        fades: Fades::default(),
        pitch_cents: 0,
        reverse: false,
    });
    // Before the first section, the lead-in is its own span.
    assert_eq!(project.section_span(0), Some([0, 4 * TICKS_PER_BAR]));
    assert_eq!(
        project.section_span(4 * TICKS_PER_BAR),
        Some([4 * TICKS_PER_BAR, 12 * TICKS_PER_BAR])
    );
    assert_eq!(
        project.section_span(15 * TICKS_PER_BAR + 7),
        Some([12 * TICKS_PER_BAR, 20 * TICKS_PER_BAR])
    );
    // The last section runs to the end of the song.
    assert_eq!(
        project.section_span(21 * TICKS_PER_BAR),
        Some([20 * TICKS_PER_BAR, 24 * TICKS_PER_BAR])
    );
    assert_eq!(project.validate(), Ok(()));
    // A short final section ends with its audio, without adding silence.
    project.clips[0].length_ticks = 20 * TICKS_PER_BAR + TICKS_PER_BAR / 2;
    assert_eq!(
        project.section_span(20 * TICKS_PER_BAR),
        Some([20 * TICKS_PER_BAR, project.clips[0].length_ticks])
    );
    // A planning bookmark at or beyond the song end gets one bounded bar.
    for end in [19 * TICKS_PER_BAR, 20 * TICKS_PER_BAR] {
        project.clips[0].length_ticks = end;
        assert_eq!(
            project.section_span(20 * TICKS_PER_BAR),
            Some([20 * TICKS_PER_BAR, 21 * TICKS_PER_BAR])
        );
    }
    project.sections.swap(0, 1);
    assert!(
        project
            .validate()
            .unwrap_err()
            .contains("each after the one before")
    );
    project.sections = vec![section(0, &"x".repeat(MAX_SECTION_NAME + 1))];
    assert!(
        project
            .validate()
            .unwrap_err()
            .contains("1 to 48 characters")
    );
}

#[test]
fn missing_sample_group_reads_the_key_of_each_importer() {
    let mut missing = MissingSample::native(
        crate::products::Product::D2,
        7,
        "Lead".into(),
        140.0,
        256,
        false,
        "The sample is unavailable.".into(),
    )
    .unwrap();
    assert_eq!(missing.group_name(), None);
    for (key, group) in [
        ("category", "Bass"),
        ("Group", "Drum"),
        ("group_name", "Loop"),
    ] {
        missing.metadata.insert(key.into(), group.into());
        assert_eq!(missing.group_name(), Some(group));
    }
    missing.metadata.insert("group_name".into(), " ".into());
    assert_eq!(missing.group_name(), Some("Drum"));
}

#[test]
fn sample_conflict_choices_persist_validate_and_prune_after_clip_deletion() {
    let source = ImportedSample {
        id: 1_000_000,
        name: "Lead".into(),
        group_name: "MixJam".into(),
        path: "A/lead.wav".into(),
        source_bpm: 120.0,
        source_sample_rate: Some(44_100),
        origin: None,
    };
    let mut project = Project {
        imported_samples: vec![source].into(),
        sample_conflicts: vec![SampleConflict {
            reference: "Old/lead.wav".into(),
            sample_ids: vec![1_000_000],
            candidates: vec!["A/lead.wav".into(), "B/lead.wav".into()],
            selected: 0,
            confirmed: false,
            source_spans: BTreeMap::from([(1_000_000, 256)]),
        }],
        ..Project::default()
    };
    project.validate().unwrap();
    let bytes = serde_json::to_vec(&project).unwrap();
    let reopened: Project = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(reopened, project);
    let mut old = serde_json::to_value(&project).unwrap();
    old.as_object_mut().unwrap().remove("sample_conflicts");
    assert!(
        serde_json::from_value::<Project>(old)
            .unwrap()
            .sample_conflicts
            .is_empty()
    );
    for case in 0..6 {
        let mut invalid = project.clone();
        let conflict = &mut invalid.sample_conflicts[0];
        match case {
            0 => conflict.selected = 2,
            1 => conflict.sample_ids.push(1_000_000),
            2 => conflict.candidates[1] = conflict.candidates[0].clone(),
            3 => conflict.sample_ids[0] = 1_000_099,
            4 => {
                conflict.source_spans.insert(1_000_001, 256);
            }
            _ => {
                conflict.source_spans.insert(1_000_000, 0);
            }
        }
        assert!(invalid.validate().is_err(), "case {case}");
    }
    // Deleting the last placement is valid while its import metadata remains.
    project.validate().unwrap();
    project.prune_sample_conflicts();
    assert!(project.sample_conflicts.is_empty());
}

#[test]
fn mixer_decibels_use_amplitude_and_keep_legacy_headroom() {
    assert_eq!(amplitude_to_db(0.0), f32::NEG_INFINITY);
    assert_eq!(db_to_amplitude(f32::NEG_INFINITY), 0.0);
    assert_eq!(amplitude_to_db(1.0), 0.0);
    assert_eq!(db_to_amplitude(0.0), 1.0);
    assert!((db_to_amplitude(-6.0) - 0.501_187_2).abs() < 1e-6);
    assert!((amplitude_to_db(0.5) + 6.020_6).abs() < 1e-5);
    assert_eq!(db_to_amplitude(MAX_MIXER_GAIN_DB), 3.981_071_7);
    for db in [-90.0, -60.0, -24.0, -12.0, -3.0, 0.0, 6.0, 12.0] {
        assert!((amplitude_to_db(db_to_amplitude(db)) - db).abs() < 1e-5);
    }
    let mut project = Project::default();
    project.tracks[0].gain = 4.0;
    project.tracks[0].polarity_inverted = true;
    project.master_gain = db_to_amplitude(MAX_MIXER_GAIN_DB);
    project.validate().unwrap();
    assert!(amplitude_to_db(project.tracks[0].gain) > MAX_MIXER_GAIN_DB);
    project.tracks[0].gain = -0.5;
    assert!(project.validate().is_err());
    project.tracks[0].gain = 4.01;
    assert!(project.validate().is_err());
    project.tracks[0].gain = 0.0;
    project.master_gain = 4.01;
    assert!(project.validate().is_err());
}

#[test]
fn placeholder_validation_rejects_conflicting_identities_and_invalid_metadata() {
    let imported = ImportedSample {
        id: 1_000_000,
        name: "Lead".into(),
        group_name: "Wave".into(),
        path: PathBuf::from("missing.wav"),
        source_bpm: 140.0,
        source_sample_rate: None,
        origin: None,
    };
    let missing = MissingSample::external(&imported, 256, "File is missing.".into());
    let mut project = Project {
        missing_samples: vec![missing.clone()],
        ..Project::default()
    };
    project.clips.push(Clip {
        id: 1,
        sample_id: imported.id,
        track: 0,
        start_tick: 0,
        length_ticks: 256,
        offset_frames: 0,
        gain: 1.0,
        fades: Default::default(),
        pitch_cents: 0,
        reverse: false,
    });
    project.validate().unwrap();
    assert_eq!(
        Project::parse_json(&serde_json::to_vec(&project).unwrap()).unwrap(),
        project
    );
    project.imported_samples.push(imported);
    assert!(project.validate().is_err());
    project.imported_samples.clear();
    project.missing_samples.push(missing.clone());
    assert!(project.validate().is_err());
    project.missing_samples.pop();
    project.missing_samples[0].sample_rate = 0;
    assert!(project.validate().is_err());
    project.missing_samples[0] = missing.clone();
    project.missing_samples[0].nominal_ticks = 0;
    assert!(project.validate().is_err());
    project.missing_samples[0] = missing;
    project.missing_samples[0].reference = MissingReference::Native {
        product: crate::products::Product::T2,
        native_id: 7,
    };
    assert!(project.validate().is_err());
}

#[test]
fn optional_source_and_notice_fields_keep_deserialization_defaults() {
    let mut value = serde_json::to_value(Project::default()).unwrap();
    value.as_object_mut().unwrap().remove("import_notices");
    value.as_object_mut().unwrap().remove("imported_samples");
    value.as_object_mut().unwrap().remove("libraries");
    assert_eq!(
        Project::parse_json(&serde_json::to_vec(&value).unwrap()).unwrap(),
        Project::default()
    );
    value.as_object_mut().unwrap().remove("schema_version");
    value["tracks"].as_array_mut().unwrap().truncate(16);
    let migrated = Project::parse_json(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(migrated.imported_samples.is_empty());
    assert!(migrated.import_notices.is_empty());
    assert!(migrated.libraries.is_empty());
}

#[test]
fn library_bindings_require_unique_products_and_nonempty_roots() {
    use crate::products::{LibraryBinding, Product};
    let mut project = Project {
        libraries: vec![
            LibraryBinding {
                product: Product::T2,
                root: PathBuf::from("t2-library"),
            },
            LibraryBinding {
                product: Product::T1,
                root: PathBuf::from("t1-library"),
            },
        ],
        imported_samples: vec![ImportedSample {
            id: 1_000_000,
            name: "Original WAV".into(),
            group_name: "Imported".into(),
            path: PathBuf::from("original.wav"),
            source_bpm: 180.0,
            source_sample_rate: None,
            origin: Some(Product::T1),
        }]
        .into(),
        ..Project::default()
    };
    project.validate().unwrap();
    let reopened = Project::parse_json(&serde_json::to_vec(&project).unwrap()).unwrap();
    assert_eq!(reopened, project);
    for origin in Product::ALL {
        project.imported_samples[0].origin = Some(origin);
        project.validate().unwrap();
    }
    project.libraries[1].product = Product::T2;
    assert!(project.validate().is_err());
    project.libraries[1].product = Product::T1;
    project.libraries[1].root.clear();
    assert!(project.validate().is_err());
}
