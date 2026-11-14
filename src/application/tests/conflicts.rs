//! Missing sample references, ambiguous sample choices, and their resolution.
use super::*;
use crate::application::references::same_missing_reference;

#[test]
fn identical_missing_paths_include_windows_verbatim_spellings() {
    let reference = |path: &str| MissingReference::External {
        path: path.into(),
        origin: None,
    };
    assert!(same_missing_reference(
        &reference(r"C:\Lost\Lead.wav"),
        &reference(r"\\?\c:\lost\lead.WAV")
    ));
    assert!(same_missing_reference(
        &reference(r"\\server\share\Lead.wav"),
        &reference(r"\\?\UNC\SERVER\SHARE\lead.wav")
    ));
    assert!(!same_missing_reference(
        &reference(r"C:\Lost\Lead.wav"),
        &reference(r"C:\Other\Lead.wav")
    ));
}

#[test]
fn conflict_switch_updates_all_instances_and_preserves_musical_trims_with_history() {
    let (mut session, root) = conflict_fixture();
    let original = session.project().clone();
    assert_eq!(session.sample_conflicts()[0].selected, 0);
    assert!(!session.sample_conflicts()[0].confirmed);
    assert!(
        audio::render_project(session.project(), &session.library)
            .unwrap()
            .samples
            .iter()
            .any(|sample| *sample > 0.0)
    );
    let revision = session.revision;
    session.resolve_sample_conflict(0, 1).unwrap();
    assert!(session.unsaved());
    assert!(session.revision > revision);
    assert!(session.render_dirty);
    let chosen = &session.sample_conflicts()[0];
    assert_eq!(chosen.selected, 1);
    assert!(chosen.confirmed);
    for source in &session.project().imported_samples {
        assert_eq!(source.path, chosen.candidates[1]);
        assert_eq!(source.source_sample_rate, Some(44_100));
        assert_eq!(
            source.source_bpm,
            if source.id == 1_000_000 { 120.0 } else { 240.0 }
        );
    }
    for (before, after) in original.clips.iter().zip(&session.project().clips) {
        assert_eq!(after.offset_frames, 44_100);
        let mut expected = before.clone();
        expected.offset_frames = 44_100;
        assert_eq!(after, &expected);
    }
    assert!(
        audio::render_project(session.project(), &session.library)
            .unwrap()
            .samples
            .iter()
            .any(|sample| *sample < 0.0)
    );
    let changed = session.project().clone();
    let encoded = serde_json::to_vec(&changed).unwrap();
    assert_eq!(
        serde_json::from_slice::<Project>(&encoded).unwrap(),
        changed
    );
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &original);
    assert!(session.history_step(true).unwrap());
    assert_eq!(session.project(), &changed);
    session.resolve_sample_conflict(0, 0).unwrap();
    assert_eq!(session.project().clips, original.clips);
    for path in &original.sample_conflicts[0].candidates {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn t1_conflict_switch_preserves_decoder_on_return_and_missing_source_recovery() {
    let (base, root) = conflict_fixture();
    let mut project = base.project().clone();
    let candidates = project.sample_conflicts[0].candidates.clone();
    // Distinct stereo lanes and source rates exercise the T1 WAV decoder.
    for (path, rate, left, right) in [
        (&candidates[0], 22_050, 0.25, -0.75),
        (&candidates[1], 44_100, -0.5, 0.125),
    ] {
        Pcm {
            sample_rate: rate,
            channels: 2,
            samples: [left, right].repeat(rate as usize),
        }
        .write_wav(path)
        .unwrap();
    }
    project.sample_conflicts[0].source_spans.clear();
    for source in &mut project.imported_samples {
        source.group_name = "T1 sounds".into();
        source.source_bpm = 140.0;
        source.source_sample_rate = Some(44_100);
        source.origin = Some(Product::T1);
    }
    let mut library = Library::empty();
    library.apply_imports(&project.imported_samples).unwrap();
    let mut session = Session::headless(Arc::new(library), project.clone()).unwrap();
    let original = audio::render_project(session.project(), &session.library).unwrap();
    assert!(original.samples.iter().any(|sample| *sample != 0.0));
    session.resolve_sample_conflict(0, 1).unwrap();
    assert!(
        session
            .project()
            .imported_samples
            .iter()
            .all(|source| source.origin == Some(Product::T1))
    );
    let alternative = audio::render_project(session.project(), &session.library).unwrap();
    assert_ne!(alternative.samples, original.samples);
    session.resolve_sample_conflict(0, 0).unwrap();
    assert_eq!(session.project().clips, project.clips);
    let returned = audio::render_project(session.project(), &session.library).unwrap();
    assert_eq!(returned.sample_rate, original.sample_rate);
    assert_eq!(returned.channels, original.channels);
    assert_eq!(returned.samples, original.samples);

    let mut missing_project = session.project().clone();
    let unavailable = missing_project.imported_samples.remove(0);
    missing_project
        .missing_samples
        .push(MissingSample::external(
            &unavailable,
            256,
            "Unavailable source.".into(),
        ));
    let mut missing_library = Library::empty();
    missing_library
        .apply_imports(&missing_project.imported_samples)
        .unwrap();
    missing_library.reserve_missing_samples(&missing_project.missing_samples);
    let mut missing_session =
        Session::headless(Arc::new(missing_library), missing_project).unwrap();
    missing_session.resolve_sample_conflict(0, 1).unwrap();
    assert!(missing_session.missing_samples().is_empty());
    assert!(
        missing_session
            .project()
            .imported_samples
            .iter()
            .all(|source| source.origin == Some(Product::T1))
    );
    for path in &candidates {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn deleting_conflict_placements_prunes_choices_and_spans_with_history() {
    let (mut session, root) = conflict_fixture();
    let original = session.project().clone();
    session.delete(2).unwrap();
    assert_eq!(session.sample_conflicts().len(), 1);
    assert_eq!(session.sample_conflicts()[0].sample_ids, vec![1_000_000]);
    assert_eq!(
        session.sample_conflicts()[0].source_spans,
        [(1_000_000, 256)].into()
    );
    assert!(!session.sample_conflicts()[0].confirmed);
    let partial = session.project().clone();
    session.edit(EditCommand::DeleteClips(vec![1, 3])).unwrap();
    assert!(session.project().clips.is_empty());
    assert!(session.sample_conflicts().is_empty());
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &partial);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &original);
    assert!(session.history_step(true).unwrap());
    assert_eq!(session.project(), &partial);
    assert!(session.history_step(true).unwrap());
    assert!(session.sample_conflicts().is_empty());
    for path in &original.sample_conflicts[0].candidates {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn invalid_conflict_choice_is_atomic_and_accepting_defaults_is_undoable() {
    let (mut session, root) = conflict_fixture();
    let original = session.project().clone();
    let revision = session.revision;
    assert!(session.resolve_sample_conflict(9, 0).is_err());
    assert!(session.resolve_sample_conflict(0, 9).is_err());
    std::fs::write(&original.sample_conflicts[0].candidates[1], b"invalid WAV").unwrap();
    assert!(session.resolve_sample_conflict(0, 1).is_err());
    assert_eq!(session.project(), &original);
    assert_eq!(session.revision, revision);
    assert!(!session.unsaved());
    assert!(!session.history_step(false).unwrap());
    session.confirm_sample_conflicts().unwrap();
    assert!(session.sample_conflicts()[0].confirmed);
    assert_eq!(session.sample_conflicts()[0].selected, 0);
    assert_eq!(session.project().clips, original.clips);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &original);
    assert!(session.history_step(true).unwrap());
    assert!(session.sample_conflicts()[0].confirmed);
    for path in &original.sample_conflicts[0].candidates {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn conflict_choice_restores_native_missing_source_as_import_without_copying() {
    let (base, root) = conflict_fixture();
    let mut missing = MissingSample::native(
        Product::T2,
        42,
        "Lead".into(),
        140.0,
        256,
        false,
        "Missing source.".into(),
    )
    .unwrap();
    missing.sample_rate = 22_050;
    let candidates = base.sample_conflicts()[0].candidates.clone();
    let project = Project {
        missing_samples: vec![missing],
        sample_conflicts: vec![SampleConflict {
            reference: "Native lead".into(),
            sample_ids: vec![42],
            candidates: candidates.clone(),
            selected: 0,
            confirmed: false,
            source_spans: Default::default(),
        }],
        clips: vec![Clip {
            id: 1,
            sample_id: 42,
            track: 0,
            start_tick: 17,
            length_ticks: 256,
            offset_frames: 11_025,
            gain: 0.75,
            fades: Default::default(),
            pitch_cents: 0,
            reverse: false,
        }],
        ..Project::default()
    };
    let mut session = Session::headless(Arc::new(Library::empty()), project.clone()).unwrap();
    session.resolve_sample_conflict(0, 1).unwrap();
    assert!(session.missing_samples().is_empty());
    let imported = &session.project().imported_samples[0];
    assert!(imported.id >= crate::products::IMPORTED_ID_START);
    assert_eq!(imported.path, candidates[1]);
    assert_eq!(imported.source_bpm, 140.0);
    assert_eq!(imported.origin, None);
    assert_eq!(session.sample_conflicts()[0].sample_ids, vec![imported.id]);
    assert_eq!(session.project().clips[0].sample_id, imported.id);
    assert_eq!(session.project().clips[0].offset_frames, 22_050);
    assert!(session.history_step(false).unwrap());
    assert_eq!(session.project(), &project);
    assert!(session.history_step(true).unwrap());
    for path in &candidates {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn manual_resolution_is_atomic_updates_all_exact_references_and_supports_history() {
    let root = manual_fixture_dir();
    let imported = crate::engine::ImportedSample {
        id: 1_000_000,
        name: "Missing lead".into(),
        group_name: "Leads".into(),
        path: PathBuf::from("C:\\Old\\Leads\\lead.wav"),
        source_bpm: 120.0,
        source_sample_rate: None,
        origin: None,
    };
    let first = MissingSample::external(&imported, 512, "File is missing.".into());
    let mut second = first.clone();
    second.id += 1;
    second.reference = MissingReference::External {
        path: PathBuf::from("c:/old/leads/LEAD.wav"),
        origin: Some(Product::D3),
    };
    let mut unrelated = first.clone();
    unrelated.id += 2;
    unrelated.reference = MissingReference::External {
        path: PathBuf::from("C:\\Other\\lead.wav"),
        origin: None,
    };
    let mut project = Project {
        missing_samples: vec![first, second, unrelated],
        ..Project::default()
    };
    for (index, (sample_id, track)) in [
        (1_000_000, 0),
        (1_000_000, 4),
        (1_000_001, 7),
        (1_000_002, 9),
    ]
    .into_iter()
    .enumerate()
    {
        project.clips.push(Clip {
            id: index as u64 + 1,
            sample_id,
            track,
            start_tick: 17,
            length_ticks: 256,
            offset_frames: 4410,
            gain: 0.75,
            fades: Default::default(),
            pitch_cents: 0,
            reverse: false,
        });
    }
    let mut session = Session::headless(Arc::new(Library::empty()), project.clone())
        .unwrap()
        .with_settings_root(root.join("settings"));
    let revision = session.revision;
    let bad = root.join("bad.wav");
    std::fs::write(&bad, b"not audio").unwrap();
    assert!(session.resolve_missing_sample(1_000_000, &bad).is_err());
    assert_eq!(session.project(), &project);
    assert_eq!(session.revision, revision);
    assert!(!session.unsaved());
    assert!(!session.history_step(false).unwrap());

    let wave = root.join("chosen.wav");
    Pcm {
        sample_rate: 22_050,
        channels: 1,
        samples: vec![0.5; 22_050],
    }
    .write_wav(&wave)
    .unwrap();
    session.resolve_missing_sample(1_000_000, &wave).unwrap();
    assert_eq!(session.missing_samples().len(), 1);
    assert_eq!(session.missing_samples()[0].id, 1_000_002);
    assert!(session.unsaved());
    assert!(session.revision > revision);
    for (index, clip) in session.project().clips.iter().enumerate() {
        assert_eq!(clip.start_tick, 17);
        assert_eq!(clip.length_ticks, 256);
        assert_eq!(clip.gain, 0.75);
        assert_eq!(clip.track, project.clips[index].track);
        assert_eq!(clip.offset_frames, if index < 3 { 2205 } else { 4410 });
    }
    let resolved = session.project().clone();
    assert!(
        audio::render_project(&resolved, &session.library)
            .unwrap()
            .samples
            .iter()
            .any(|sample| *sample != 0.0)
    );
    session.history_step(false).unwrap();
    assert_eq!(session.project(), &project);
    assert!(
        audio::render_project(session.project(), &session.library)
            .unwrap()
            .samples
            .iter()
            .all(|sample| *sample == 0.0)
    );
    session.history_step(true).unwrap();
    assert_eq!(session.project(), &resolved);
    std::fs::remove_file(&resolved.imported_samples[0].path).unwrap();
    std::fs::remove_file(wave).unwrap();
    std::fs::remove_file(bad).unwrap();
    std::fs::remove_dir(root.join("settings/manual-samples")).unwrap();
    std::fs::remove_dir(root.join("settings")).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn manual_resolution_uses_saved_rate_and_actual_mixjam_wave_duration() {
    let root = manual_fixture_dir();
    let wave = root.join("chosen.wav");
    Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.5; 88_200],
    }
    .write_wav(&wave)
    .unwrap();
    let imported = crate::engine::ImportedSample {
        id: 1_000_000,
        name: "Missing lead".into(),
        group_name: "Lead".into(),
        path: PathBuf::from("lost.wav"),
        source_bpm: 140.0,
        source_sample_rate: Some(22_050),
        origin: None,
    };
    let mut missing = MissingSample::external(&imported, 256, "Missing audio.".into());
    missing
        .metadata
        .insert("Playback timing policy".into(), "mixjam-span".into());
    missing
        .metadata
        .insert("durationSeconds".into(), "99".into());
    let project = Project {
        missing_samples: vec![missing],
        clips: vec![Clip {
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
    let mut session = Session::headless(Arc::new(Library::empty()), project)
        .unwrap()
        .with_settings_root(root.join("settings"));
    session.resolve_missing_sample(imported.id, &wave).unwrap();
    assert_eq!(session.project().clips[0].offset_frames, 25_725);
    assert_eq!(session.project().imported_samples[0].source_bpm, 120.0);
    assert_eq!(
        session.project().imported_samples[0].source_sample_rate,
        Some(44_100)
    );
    std::fs::remove_file(&session.project().imported_samples[0].path).unwrap();
    std::fs::remove_file(wave).unwrap();
    std::fs::remove_dir(root.join("settings/manual-samples")).unwrap();
    std::fs::remove_dir(root.join("settings")).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn manual_native_resolution_uses_imported_identity_without_factory_identity() {
    let root = manual_fixture_dir();
    let mut notices = crate::catalog::Catalogs::open(None).missing_notices([(Product::T2, 42)]);
    assert_eq!(notices.len(), 1);
    assert!(notices[0].starts_with("At import,"));
    assert!(notices[0].contains("If sounds remain missing"));
    notices.push("Unrelated import detail.".into());
    let missing = MissingSample::native(
        Product::T2,
        42,
        "Native lead".into(),
        140.0,
        256,
        false,
        "Missing library.".into(),
    )
    .unwrap();
    let mut project = Project {
        missing_samples: vec![missing],
        source_notices: notices.clone(),
        ..Project::default()
    };
    project.clips.push(Clip {
        id: 1,
        sample_id: 42,
        track: 0,
        start_tick: 0,
        length_ticks: 256,
        offset_frames: 0,
        gain: 1.0,
        fades: Default::default(),
        pitch_cents: 0,
        reverse: false,
    });
    let wave = root.join("chosen.wav");
    Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.5; 1000],
    }
    .write_wav(&wave)
    .unwrap();
    let mut session = Session::headless(Arc::new(Library::empty()), project)
        .unwrap()
        .with_settings_root(root.join("settings"));
    session.resolve_missing_sample(42, &wave).unwrap();
    assert!(session.missing_samples().is_empty());
    assert!(session.project().clips[0].sample_id >= crate::products::IMPORTED_ID_START);
    assert!(session.library.sample_by_id(42).is_none());
    assert_eq!(session.library.verified_count(), 0);
    assert_eq!(session.project().source_notices, notices);

    assert!(session.history_step(false).unwrap());
    assert_eq!(session.missing_samples().len(), 1);
    assert_eq!(session.project().source_notices, notices);
    assert!(session.history_step(true).unwrap());
    assert!(session.missing_samples().is_empty());
    assert_eq!(session.project().source_notices, notices);

    let song = root.join("resolved.mixjam");
    crate::project_io::save(session.project(), &song).unwrap();
    let (reopened, _) = crate::project_io::open(&song, &Library::empty()).unwrap();
    assert!(reopened.missing_samples.is_empty());
    assert_eq!(reopened.source_notices, notices);
    std::fs::remove_file(song).unwrap();
    std::fs::remove_file(&session.project().imported_samples[0].path).unwrap();
    std::fs::remove_file(wave).unwrap();
    std::fs::remove_dir(root.join("settings/manual-samples")).unwrap();
    std::fs::remove_dir(root.join("settings")).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn manual_choice_survives_same_basename_palette_and_original_file_loss() {
    let root = manual_fixture_dir();
    let selected = root.join("selected");
    let outside = root.join("outside");
    std::fs::create_dir_all(&selected).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    for (folder, value) in [(&selected, -0.5), (&outside, 0.5)] {
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![value; 1000],
        }
        .write_wav(&folder.join("lead.wav"))
        .unwrap();
    }
    let library = Library::open_wav_folder(&selected, 140.0).unwrap();
    let missing = MissingSample::native(
        Product::T2,
        42,
        "Native lead".into(),
        140.0,
        256,
        false,
        "Missing source.".into(),
    )
    .unwrap();
    let project = Project {
        missing_samples: vec![missing],
        clips: vec![Clip {
            id: 1,
            sample_id: 42,
            track: 0,
            start_tick: 0,
            length_ticks: 256,
            offset_frames: 0,
            gain: 1.0,
            fades: Default::default(),
            pitch_cents: 0,
            reverse: false,
        }],
        ..Project::default()
    };
    let mut session = Session::headless(Arc::new(library.clone()), project)
        .unwrap()
        .with_settings_root(root.join("settings"));
    session
        .resolve_missing_sample(42, &outside.join("lead.wav"))
        .unwrap();
    let retained = session.project().imported_samples[0].path.clone();
    assert_eq!(
        std::fs::read(&retained).unwrap(),
        std::fs::read(outside.join("lead.wav")).unwrap()
    );
    std::fs::remove_file(outside.join("lead.wav")).unwrap();
    let saved = root.join("song.mixjam");
    session.save(&saved).unwrap();
    let (reopened, ready) = crate::project_io::open(&saved, &library).unwrap();
    assert_eq!(reopened.imported_samples[0].path, retained);
    assert!(reopened.missing_samples.is_empty());
    let rendered = audio::render_project(&reopened, &ready).unwrap();
    assert!(rendered.samples.iter().any(|sample| *sample > 0.0));
    assert!(rendered.samples.iter().all(|sample| *sample >= 0.0));
    std::fs::remove_file(retained).unwrap();
    std::fs::remove_file(saved).unwrap();
    std::fs::remove_file(selected.join("lead.wav")).unwrap();
    std::fs::remove_dir(selected).unwrap();
    std::fs::remove_dir(outside).unwrap();
    std::fs::remove_dir(root.join("settings/manual-samples")).unwrap();
    std::fs::remove_dir(root.join("settings")).unwrap();
    std::fs::remove_dir(root).unwrap();
}
