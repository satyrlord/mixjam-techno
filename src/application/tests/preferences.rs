//! Options, layout, the song folder, and the sample quality of playback and export.
use super::*;
use crate::application::render_worker::RenderResult;
use std::sync::mpsc;

#[test]
fn layout_is_saved_beside_the_options_and_read_by_the_next_session() {
    let root = manual_fixture_dir();
    let mut session = fixture().with_settings_root(root.clone());
    assert_eq!(session.layout(), crate::options::WorkspaceLayout::default());
    let layout = crate::options::WorkspaceLayout {
        tool_names: false,
        boards: false,
        maximised: true,
        sounds: true,
        overview: false,
        inspector: false,
    };
    session.set_layout(layout).unwrap();
    assert_eq!(crate::options::load_layout(&root), layout);
    // The Options window's revision and file are untouched.
    assert_eq!(session.options_revision(), 0);
    assert!(!root.join("options.json").exists());
    assert_eq!(fixture().with_settings_root(root.clone()).layout(), layout);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn options_apply_is_atomic_and_independent_of_song_history() {
    let root = manual_fixture_dir();
    let mut session = fixture().with_settings_root(root.clone());
    let project = session.project().clone();
    let token = session.project_change_token();
    let mut draft = session.options().clone();
    draft.show_section_bookmarks = false;
    draft.follow_on_startup = false;
    assert!(session.apply_options(draft.clone(), 0, false).unwrap());
    assert_eq!(session.project(), &project);
    assert_eq!(session.project_change_token(), token);
    assert!(!session.unsaved());
    assert!(!session.has_history_step(false));
    assert_eq!(crate::options::load(&root).values, draft);
    let bytes = std::fs::read(root.join("options.json")).unwrap();
    assert!(!session.apply_options(draft.clone(), 1, false).unwrap());
    assert_eq!(std::fs::read(root.join("options.json")).unwrap(), bytes);
    assert_eq!(session.options_revision(), 1);
    let mut stale = draft.clone();
    stale.show_tooltips = false;
    session.remember_section_bookmarks(true).unwrap();
    assert!(
        session
            .apply_options(stale, 1, false)
            .unwrap_err()
            .contains("Reload")
    );
    assert!(session.options().show_section_bookmarks);
    // A failed atomic publication must not adopt any part of the draft.
    let before = session.options().clone();
    let revision = session.options_revision();
    std::fs::remove_file(root.join("options.json")).unwrap();
    std::fs::create_dir(root.join("options.json")).unwrap();
    draft.show_tooltips = false;
    assert!(session.apply_options(draft, revision, false).is_err());
    assert_eq!(session.options(), &before);
    assert_eq!(session.options_revision(), revision);
    assert_eq!(session.project(), &project);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_options_require_explicit_restore_and_future_options_block_writes() {
    let root = manual_fixture_dir();
    for (bytes, future) in [("broken", false), ("{\"schemaVersion\":999}", true)] {
        std::fs::write(root.join("options.json"), bytes).unwrap();
        let mut session = fixture().with_settings_root(root.clone());
        assert!(session.options_error().is_some());
        let mut draft = session.options().clone();
        draft.show_tooltips = false;
        assert!(session.apply_options(draft, 0, false).is_err());
        let restored = session.apply_options(crate::options::AppOptions::default(), 0, true);
        if future {
            assert!(restored.is_err());
            assert_eq!(
                std::fs::read_to_string(root.join("options.json")).unwrap(),
                bytes
            );
        } else {
            assert!(restored.unwrap());
            assert!(session.options_error().is_none());
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn options_write_failure_retains_existing_file_and_active_values() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = manual_fixture_dir();
    let mut session = fixture().with_settings_root(root.clone());
    let mut values = session.options().clone();
    values.show_tooltips = false;
    session.apply_options(values, 0, false).unwrap();
    let path = root.join("options.json");
    let before = std::fs::read(&path).unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let active = session.options().clone();
    let mut draft = active.clone();
    draft.preview_level_db = -12.0;
    assert!(session.apply_options(draft, 1, false).is_err());
    assert_eq!(session.options(), &active);
    assert_eq!(session.options_revision(), 1);
    drop(held);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audio_options_wait_for_transport_but_workspace_changes_do_not() {
    let root = manual_fixture_dir();
    let mut session = fixture().with_settings_root(root.clone());
    session.wants_playback = true;
    let mut draft = session.options().clone();
    draft.preview_muted = true;
    assert!(
        session
            .apply_options(draft, 0, false)
            .unwrap_err()
            .contains("Stop playback")
    );
    let mut draft = session.options().clone();
    draft.meter_display_rate = crate::options::MeterDisplayRate::Reduced;
    assert!(session.apply_options(draft, 0, false).unwrap());
    session.wants_playback = false;
    session.recorder = Some(crate::recording::Recorder::fixture());
    let mut draft = session.options().clone();
    draft.drive_playback_oversampling = crate::options::Oversampling::Off;
    assert!(session.apply_options(draft.clone(), 1, false).is_err());
    session.recorder.take();
    let project = session.project().clone();
    assert!(session.apply_options(draft, 1, false).unwrap());
    assert_eq!(session.project(), &project);
    assert!(!session.unsaved());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_theme_applies_during_playback_without_touching_song_history_or_render() {
    let root = manual_fixture_dir();
    let mut session = fixture().with_settings_root(root.clone());
    session.render_dirty = false;
    session.wants_playback = true;
    let (project, revision, unsaved, rendered) = (
        session.project().clone(),
        session.revision,
        session.unsaved(),
        session.render.is_some(),
    );
    for theme in [
        crate::options::Theme::Vintage,
        crate::options::Theme::Aero,
        crate::options::Theme::Studio,
    ] {
        let mut draft = session.options().clone();
        draft.theme = theme;
        let expected = session.options_revision();
        assert!(session.apply_options(draft, expected, false).unwrap());
        assert_eq!(session.options().theme, theme);
        assert!(session.wants_playback);
        assert_eq!(session.project(), &project);
        assert_eq!(session.revision, revision);
        assert!(!session.render_dirty);
        assert_eq!(session.render.is_some(), rendered);
        assert_eq!(session.unsaved(), unsaved);
        assert!(!session.has_history_step(false));
        let saved = crate::options::load(&root);
        assert!(saved.error.is_none());
        assert_eq!(saved.values.theme, theme);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn quality_apply_rejects_old_preparation_and_exports_use_full_quality() {
    let root = manual_fixture_dir();
    let wave = root.join("source.wav");
    Pcm {
        sample_rate: 44100,
        channels: 2,
        samples: (0..8820)
            .map(|i| ((i / 2) as f32 * 0.07).sin() * 0.4)
            .collect(),
    }
    .write_wav(&wave)
    .unwrap();
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default())
        .unwrap()
        .with_settings_root(root.clone());
    let source = session.register_file(&wave, "Wave", 140.0).unwrap();
    session.place(source, 0, 0).unwrap();
    let mut tracks = session.project().tracks.clone();
    tracks[0].drive.on = true;
    tracks[0].drive.amount = 24.0;
    session
        .edit(EditCommand::SetMixer {
            tracks,
            master_gain: 1.0,
        })
        .unwrap();
    let old_revision = session.revision;
    let full = Arc::new(audio::prepare_project(session.project(), &session.library).unwrap());
    let (sender, _requests) = mpsc::channel();
    let (results, receiver) = mpsc::channel();
    session.render_worker = RenderWorker {
        sender,
        receiver,
        in_flight: true,
        current_revision: Some(old_revision),
        pending: None,
    };
    session.render = Some(full.clone());
    session.render_dirty = false;
    let mut draft = session.options().clone();
    draft.drive_playback_oversampling = crate::options::Oversampling::Off;
    draft.master_playback_oversampling = crate::options::Oversampling::Off;
    session.apply_options(draft, 0, false).unwrap();
    results
        .send(RenderResult {
            revision: old_revision,
            mix: Ok(full.clone()),
        })
        .unwrap();
    session.poll_render();
    assert!(session.render.is_none());
    assert!(session.render_dirty);
    session.render = Some(Arc::new(
        audio::prepare_project_with_quality(
            session.project(),
            &session.library,
            session.options.playback_quality(),
        )
        .unwrap(),
    ));
    session.render_dirty = false;
    assert_eq!(
        session.current_mix().unwrap().quality(),
        crate::playback_quality::PlaybackQuality::FULL
    );
    let path = root.join("full.wav");
    session.export(&path).unwrap();
    let expected = root.join("reference.wav");
    full.write_wav(&expected, &session.project().master, &session.project().fx)
        .unwrap();
    assert_eq!(
        std::fs::read(path).unwrap(),
        std::fs::read(expected).unwrap()
    );
    let async_path = root.join("async.wav");
    session.start_export(&async_path).unwrap();
    let mut draft = session.options().clone();
    draft.preview_muted = true;
    session.apply_options(draft, 1, false).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while session.is_exporting() {
        session.poll_export();
        assert!(
            std::time::Instant::now() < deadline,
            "Export did not finish."
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        std::fs::read(async_path).unwrap(),
        std::fs::read(root.join("reference.wav")).unwrap()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn fixed_song_folder_falls_back_without_forgetting_configured_path() {
    let root = manual_fixture_dir();
    let fixed = root.join("fixed");
    std::fs::create_dir(&fixed).unwrap();
    let mut session = fixture().with_settings_root(root.clone());
    session.remember_song_folder(&root.join("song.mixjam"));
    let mut draft = session.options().clone();
    draft.song_folder_mode = crate::options::SongFolderMode::Fixed;
    draft.fixed_song_folder = Some(fixed.clone());
    session.apply_options(draft, 0, false).unwrap();
    assert_eq!(session.song_folder(), Some(fixed.clone()));
    std::fs::remove_dir(&fixed).unwrap();
    assert_eq!(session.song_folder(), Some(root.clone()));
    assert_eq!(session.options().fixed_song_folder, Some(fixed));
    assert!(session.song_folder_warning().is_some());
    session.remember_section_bookmarks(false).unwrap();
    assert!(!session.options().show_section_bookmarks);
    let revision = session.options_revision();
    assert!(
        !session
            .apply_options(session.options().clone(), revision, false)
            .unwrap()
    );
    std::fs::remove_dir_all(root).unwrap();
}
