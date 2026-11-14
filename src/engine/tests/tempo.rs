use super::*;

#[test]
fn recovered_mixjam_source_tempos_survive_json_without_float_drift() {
    for tempo in [
        124.999_999_999_999_99,
        151.964_162_646_450_75,
        113.562_231_759_656_65,
    ] {
        let source = ImportedSample {
            id: 1_000_000,
            name: "Recovered source".into(),
            group_name: "MixJam".into(),
            path: "Genre/source.wav".into(),
            source_bpm: tempo,
            source_sample_rate: Some(44100),
            origin: None,
        };
        let mut project = Project::default();
        project
            .missing_samples
            .push(MissingSample::external(&source, 256, "Unavailable".into()));
        let mut ready = source;
        ready.id += 1;
        project.imported_samples.push(ready);
        project.validate().unwrap();
        let bytes = serde_json::to_vec(&project).unwrap();
        let reopened: Project = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(reopened, project);
        assert_eq!(serde_json::to_vec(&reopened).unwrap(), bytes);
    }
}

#[test]
fn sounds_play_in_the_octave_nearest_the_song_tempo() {
    for (source, song, expected) in [
        (90.0, 180.0, 180.0),
        (90.0, 160.0, 180.0),
        (90.0, 120.0, 90.0),
        (80.0, 160.0, 160.0),
        (45.0, 180.0, 180.0),
        (180.0, 120.0, 90.0),
        (180.0, 130.0, 180.0),
        (140.0, 120.0, 140.0),
        (140.0, 180.0, 140.0),
        (360.0, 180.0, 180.0),
    ] {
        assert_eq!(playback_bpm(source, song), expected, "{source} in {song}");
    }
    assert!(playback_bpm(f64::NAN, 140.0).is_nan());
    assert_eq!(playback_bpm(-1.0, 140.0), -1.0);
}

#[test]
fn exact_source_tempos_need_adapter_provenance_or_a_saved_span_policy() {
    let source = ImportedSample {
        id: 1_000_000,
        name: "Loop".into(),
        group_name: "MixJam".into(),
        path: "loop.wav".into(),
        source_bpm: 240.0,
        source_sample_rate: Some(44_100),
        origin: None,
    };
    let mut project = Project {
        imported_samples: vec![source.clone()].into(),
        ..Project::default()
    };
    assert_eq!(project.sample_playback_bpm(source.id, 240.0), 120.0);
    project
        .source_notices
        .push("MixJam v7 source controls: {}".into());
    assert_eq!(project.sample_playback_bpm(source.id, 240.0), 240.0);
    project.imported_samples[0].group_name = "Wave".into();
    assert_eq!(project.sample_playback_bpm(source.id, 240.0), 120.0);
    project.source_notices.clear();
    project.imported_samples.clear();
    let mut missing = MissingSample::external(&source, 256, "Missing audio.".into());
    missing
        .metadata
        .insert("Playback timing policy".into(), "mixjam-span".into());
    project.missing_samples.push(missing);
    assert_eq!(project.sample_playback_bpm(source.id, 240.0), 240.0);
    project.missing_samples.clear();
    project.sample_conflicts.push(SampleConflict {
        reference: "loop.wav".into(),
        sample_ids: vec![source.id],
        candidates: vec!["one/loop.wav".into(), "two/loop.wav".into()],
        selected: 0,
        confirmed: false,
        source_spans: BTreeMap::from([(source.id, 256)]),
    });
    assert_eq!(project.sample_playback_bpm(source.id, 240.0), 240.0);
}

#[test]
fn nearest_project_tempo_uses_lower_midpoint_ties() {
    for (source, target) in [
        (0.001, 120.0),
        (120.0, 120.0),
        (122.5, 120.0),
        (122.501, 125.0),
        (127.5, 125.0),
        (130.0, 130.0),
        (132.501, 135.0),
        (137.5, 135.0),
        (137.501, 140.0),
        (150.0, 140.0),
        (150.001, 160.0),
        (170.0, 160.0),
        (170.001, 180.0),
        (180.0, 180.0),
        (f64::MAX, 180.0),
    ] {
        assert_eq!(nearest_supported_bpm(source).unwrap(), target);
    }
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(nearest_supported_bpm(invalid).is_err());
    }
}

#[test]
fn current_project_contract_keeps_source_sample_tempo_independent() {
    let mut project = Project::default();
    for bpm in SUPPORTED_BPMS {
        project.bpm = bpm;
        project.validate().unwrap();
    }
    project.bpm = 146.0;
    assert!(project.validate().is_err());
    project.bpm = 140.0;
    project.pitch_ratio = 1.01;
    assert!(project.validate().is_err());
    project.pitch_ratio = 1.0;
    project.imported_samples.push(ImportedSample {
        id: 1_000_000,
        name: "Source tempo".into(),
        group_name: "Imported".into(),
        path: PathBuf::from("source.wav"),
        source_bpm: 0.001,
        source_sample_rate: None,
        origin: None,
    });
    project.validate().unwrap();
    project.imported_samples[0].source_bpm = 2_000.0;
    project.validate().unwrap();
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        project.imported_samples[0].source_bpm = invalid;
        assert!(project.validate().is_err());
    }
}

#[test]
fn import_folds_legacy_pitch_once_and_records_only_tempo_changes() {
    let original = Project {
        pitch_ratio: 146.0 / 140.0,
        ..Project::default()
    };
    let normalized = normalize_import(original).unwrap();
    assert_eq!(normalized.bpm, 140.0);
    assert_eq!(normalized.pitch_ratio, 1.0);
    assert_eq!(normalized.import_notices.len(), 1);
    assert!((normalized.import_notices[0].source_bpm - 146.0).abs() < 1e-12);
    assert_eq!(normalized.import_notices[0].target_bpm, 140.0);
    assert_eq!(normalize_import(normalized.clone()).unwrap(), normalized);
    let supported_effective_tempo = Project {
        bpm: 120.0,
        pitch_ratio: 1.5,
        ..Project::default()
    };
    let normalized = normalize_import(supported_effective_tempo).unwrap();
    assert_eq!(normalized.bpm, 180.0);
    assert_eq!(normalized.pitch_ratio, 1.0);
    assert!(normalized.import_notices.is_empty());
    let overflowed_effective_tempo = Project {
        bpm: f64::MAX,
        pitch_ratio: 2.0,
        ..Project::default()
    };
    assert!(normalize_import(overflowed_effective_tempo).is_err());
}

#[test]
fn a_trim_follows_its_source_unless_the_original_keeps_frames() {
    // Twice the rate and half the tempo: the same musical point is four
    // times as many frames in.
    assert_eq!(
        rescale_trim(1000, false, (22_050, 140.0), (44_100, 70.0)),
        Some(4000)
    );
    assert_eq!(
        rescale_trim(1000, true, (22_050, 140.0), (44_100, 70.0)),
        Some(1000)
    );
    assert_eq!(rescale_trim(0, false, (1, 1.0), (384_000, 1.0)), Some(0));
    assert_eq!(
        rescale_trim(u32::MAX, false, (44_100, 120.0), (88_200, 120.0)),
        None
    );
}
