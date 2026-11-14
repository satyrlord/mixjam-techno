//! Saving and exporting songs, sounds, and sequences without a window.
use super::*;

#[test]
fn save_names_the_song_after_its_file_and_keeps_an_imported_title() {
    let dir = fixture_dir();
    let library = Arc::new(Library::empty());
    let mut session = Session::headless(library.clone(), Project::default()).unwrap();
    session.save(&dir.join("First.mixjam")).unwrap();
    assert_eq!(session.project().title, "First");
    // Save as follows the new name.
    session.save(&dir.join("Second.mixjam")).unwrap();
    assert_eq!(session.project().title, "Second");
    session.open(&dir.join("First.mixjam")).unwrap();
    assert_eq!(session.project().title, "First");
    // An earlier save kept the default title; the file names it on open.
    crate::project_io::save(&Project::default(), &dir.join("Old song.mixjam")).unwrap();
    session.open(&dir.join("Old song.mixjam")).unwrap();
    assert_eq!(session.project().title, "Old song");

    let mut imported = Session::headless(
        library,
        Project {
            title: "Night drive".into(),
            ..Project::default()
        },
    )
    .unwrap();
    imported.save(&dir.join("copy.mixjam")).unwrap();
    assert_eq!(imported.project().title, "Night drive");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn track_export_writes_each_track_on_another_thread_and_reports_the_counts() {
    let dir = fixture_dir();
    let wave = dir.join("source.wav");
    Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.25; 1000],
    }
    .write_wav(&wave)
    .unwrap();
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let source = session.register_file(&wave, "Wave", 140.0).unwrap();
    session.place(source, 0, 0).unwrap();
    session.place(source, 9, 0).unwrap();
    let folder = dir.join("tracks");
    std::fs::create_dir_all(&folder).unwrap();
    assert_eq!(
        session.export_tracks(&folder).unwrap(),
        format!(
            "Exported 2 tracks and 0 FX returns to {}.",
            folder.display()
        )
    );
    session.start_track_export(&folder).unwrap();
    assert!(session.is_exporting());
    assert!(session.start_export(&dir.join("mix.wav")).is_err());
    while session.is_exporting() {
        session.poll();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        session.take_notice(),
        Some((
            format!(
                "Exported 2 tracks and 0 FX returns to {}.",
                folder.display()
            ),
            false
        ))
    );
    let title = &session.project().title;
    for track in ["01", "10"] {
        let part = audio::read_wav(&folder.join(format!("{title} - Track {track}.wav")));
        assert!(
            part.unwrap()
                .samples
                .iter()
                .any(|sample| sample.abs() > 0.1)
        );
    }
    // A folder that is gone fails with a reason, as the mix export does.
    session.start_track_export(&dir.join("gone")).unwrap();
    while session.is_exporting() {
        session.poll();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let (notice, error) = session.take_notice().unwrap();
    assert!(
        error && notice.starts_with("Cannot export tracks: "),
        "{notice}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn headless_session_import_save_reopen_render_and_processing_work_without_gui() {
    let dir = fixture_dir();
    let wave = dir.join("source.wav");
    let project = dir.join("song.mixjam");
    Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.25; 1000],
    }
    .write_wav(&wave)
    .unwrap();
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let source = session.register_file(&wave, "Wave", 90.0).unwrap();
    session.place(source, 63, 0).unwrap();
    session.edit(EditCommand::SetTempo(160.0)).unwrap();
    session.save(&project).unwrap();
    let expected = session.project().clone();
    session.new_mix().unwrap();
    session.open(&project).unwrap();
    assert_eq!(session.project(), &expected);
    assert_eq!(
        session.library().sample_by_id(source).unwrap().source_bpm,
        90.0
    );
    session.export(&dir.join("export.wav")).unwrap();
    let rendered = audio::read_wav(&dir.join("export.wav")).unwrap();
    assert!(rendered.samples.iter().any(|sample| sample.abs() > 0.1));
    // The background export writes the same audio and reports through `poll`.
    let background = dir.join("background.wav");
    session.start_export(&background).unwrap();
    assert!(session.start_export(&background).is_err());
    while session.is_exporting() {
        session.poll();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        session.take_notice(),
        Some((format!("Exported {}.", background.display()), false))
    );
    assert_eq!(
        audio::read_wav(&background).unwrap().samples,
        rendered.samples
    );
    // A render reuses decoded sources and drops those no longer used.
    let mut decoded = std::collections::HashMap::new();
    let first =
        audio::prepare_project_reusing(session.project(), &session.library, &mut decoded).unwrap();
    assert_eq!(decoded.keys().copied().collect::<Vec<_>>(), [(source, 0)]);
    std::fs::remove_file(&wave).unwrap();
    let again =
        audio::prepare_project_reusing(session.project(), &session.library, &mut decoded).unwrap();
    let (master, fx) = (&session.project().master, &session.project().fx);
    assert_eq!(
        again.to_pcm(master, fx).unwrap().samples,
        first.to_pcm(master, fx).unwrap().samples
    );
    let empty = Project::default();
    audio::prepare_project_reusing(&empty, &session.library, &mut decoded).unwrap();
    assert!(decoded.is_empty());
    Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.25; 1000],
    }
    .write_wav(&wave)
    .unwrap();
    assert_eq!(
        session
            .edited_pcm(Some(source), 10, 20, 0.0)
            .unwrap()
            .frames(),
        10
    );
    assert_eq!(
        session
            .processed_pcm(Some(source), &EffectsState::default())
            .unwrap()
            .frames(),
        1000
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn saving_a_sequence_publishes_its_wav_and_pattern_before_registration() {
    let dir = fixture_dir();
    let path = dir.join("sequence.wav");
    let pcm = Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.5; 1000],
    };
    let state = StepSynthState {
        bpm: 160.0,
        ..StepSynthState::default()
    };
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let id = session.save_sequence(&pcm, &path, &state).unwrap();
    assert_eq!(audio::read_wav(&path).unwrap().samples, pcm.samples);
    assert_eq!(session.load_pattern(&pattern_beside(&path)).unwrap(), state);
    let sample = session.library().sample_by_id(id).unwrap();
    assert_eq!(sample.group_name, "Step synth");
    assert_eq!(sample.source_bpm, 160.0);
    assert!(
        session
            .project()
            .imported_samples
            .iter()
            .any(|s| s.id == id)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_blocked_sequence_pattern_preserves_the_wav_and_session() {
    let dir = fixture_dir();
    let path = dir.join("sequence.wav");
    std::fs::write(&path, b"existing WAV").unwrap();
    std::fs::create_dir(pattern_beside(&path)).unwrap();
    let pcm = Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.5; 1000],
    };
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let before = session.project().clone();
    let revision = session.catalog_revision();
    assert!(
        session
            .save_sequence(&pcm, &path, &StepSynthState::default())
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"existing WAV");
    assert_eq!(session.project(), &before);
    assert_eq!(session.catalog_revision(), revision);
    assert!(!session.has_history_step(false));
    assert!(!session.unsaved());
    assert!(session.library().samples.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(windows)]
#[test]
fn a_locked_sequence_pattern_rolls_back_the_existing_pair_without_session_changes() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = fixture_dir();
    let path = dir.join("sequence.wav");
    let pattern = pattern_beside(&path);
    let mut pcm = Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.5; 1000],
    };
    let mut state = StepSynthState::default();
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    session.save_sequence(&pcm, &path, &state).unwrap();
    let wav_before = std::fs::read(&path).unwrap();
    let pattern_before = std::fs::read(&pattern).unwrap();
    let project_before = session.project().clone();
    let token = session.project_change_token();
    let revision = session.catalog_revision();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&pattern)
        .unwrap();
    pcm.samples.fill(-0.5);
    state.steps[4] = Some(7);
    assert!(session.save_sequence(&pcm, &path, &state).is_err());
    drop(lock);
    assert_eq!(std::fs::read(&path).unwrap(), wav_before);
    assert_eq!(std::fs::read(&pattern).unwrap(), pattern_before);
    assert_eq!(session.project(), &project_before);
    assert_eq!(session.project_change_token(), token);
    assert_eq!(session.catalog_revision(), revision);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_audio_save_metadata_never_replaces_the_selected_file() {
    let dir = fixture_dir();
    let target = dir.join("existing.wav");
    std::fs::write(&target, b"unchanged").unwrap();
    let pcm = Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.5; 10],
    };
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let before = session.project().clone();
    for (group, bpm) in [("Wave", f64::NAN), ("", 140.0)] {
        assert!(session.save_pcm(&pcm, &target, group, bpm).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"unchanged");
        assert_eq!(session.project(), &before);
        assert!(session.library().samples.is_empty());
        assert!(!session.unsaved());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unsaved_mix_and_take_require_a_pending_action_before_replacement() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    assert!(!session.needs_save_confirmation());
    session.edit(EditCommand::SetTempo(160.0)).unwrap();
    assert!(session.needs_save_confirmation());
    session.unsaved = false;
    session.recorded_take = Some(Arc::new(Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.0; 10],
    }));
    assert!(session.needs_save_confirmation());
    assert!(session.has_recorded_take());
    session.discard_recorded_take();
    assert!(!session.needs_save_confirmation());
}
