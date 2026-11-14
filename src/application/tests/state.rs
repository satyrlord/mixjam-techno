//! Session state: library changes, change tokens, the render worker, and playback captures.
use super::*;
use crate::application::render_worker::{RenderRequest, RenderResult};
use std::sync::mpsc;

#[test]
#[ignore = "opens the physical audio output for silence"]
fn idle_audio_keeps_polling_for_capture_notices() {
    let mut session = fixture();
    assert_eq!(session.poll(), None);
    session.player = Some(Player::new().expect("opens the physical audio output"));
    assert_eq!(session.poll(), Some(250));
}

#[test]
fn a_capture_without_audio_output_says_why_and_describes_the_transport() {
    let mut session = fixture();
    session.cursor_tick = 64 * 256 + 17;
    let error = session.save_playback_capture("test").unwrap_err();
    assert!(error.contains("Audio output is unavailable"), "{error}");
    assert_eq!(
        session.take_notice(),
        Some((format!("Cannot save an audio capture: {error}"), true))
    );
    let context = session.capture_context();
    assert_eq!(context["transport"]["bar"], 65);
    assert_eq!(context["transport"]["tick_in_bar"], 17);
    assert_eq!(context["transport"]["playing_mix"], false);
    assert!(context["master"].is_object() && context["options"].is_object());
}

#[test]
fn recording_blocks_sample_folder_changes_before_scanning_or_replacing_state() {
    let mut app = fixture();
    app.editor
        .transact(|project| project.title = "Before recording".into())
        .unwrap();
    app.cursor_tick = TICKS_PER_BAR;
    app.recorder = Some(crate::recording::Recorder::fixture());
    let project = app.editor.project().clone();
    let library = app.library.clone();
    let revision = app.revision;
    let unsaved = app.unsaved;

    // The recording guard must run before a scan of any path starts.
    let error = app.start_wave_scan(&[PathBuf::new()], 140.0).unwrap_err();

    assert_eq!(error, "Stop recording before you change the sample folder.");
    assert_eq!(*app.editor.project(), project);
    assert!(Arc::ptr_eq(&app.library, &library));
    assert_eq!(app.revision, revision);
    assert_eq!(app.unsaved, unsaved);
    assert_eq!(app.cursor_tick, TICKS_PER_BAR);
    assert!(app.recorded_take.is_none());
    assert_eq!(app.recorder.as_ref().unwrap().status().captured_frames, 2);
    assert!(
        app.editor.undo(),
        "The blocked action must retain edit history."
    );
    assert_eq!(
        app.recorder.take().unwrap().finish().unwrap().samples,
        vec![0.25, -0.25]
    );
}

#[test]
fn provider_changes_preserve_an_active_recording_or_folder_scan() {
    for recording in [true, false] {
        let mut app = fixture();
        app.edit(EditCommand::SetTempo(160.0)).unwrap();
        let progress = Arc::new(crate::library::ScanProgress::default());
        let (_sender, receiver) = mpsc::channel();
        let expected = if recording {
            app.recorder = Some(crate::recording::Recorder::fixture());
            "Stop recording before you change the sample libraries."
        } else {
            app.wave_scan = Some(WaveScan {
                roots: vec![PathBuf::from("pending-library")],
                source_bpm: 140.0,
                progress: Arc::clone(&progress),
                receiver,
            });
            app.open_after_scan(Path::new("queued.mixjam")).unwrap();
            "Wait for the sample folder scan to finish before you change the sample libraries."
        };
        let project = app.project().clone();
        let library = Arc::clone(&app.library);
        let token = app.project_change_token();
        let pending = app.pending_open.clone();
        // Invalid paths and a provider in use establish that busy-state
        // admission happens before provider reads, checks, or settings writes.
        assert_eq!(
            app.add_library(Product::T2, Path::new("")).unwrap_err(),
            expected
        );
        assert_eq!(app.remove_library(Product::T2).unwrap_err(), expected);
        assert_eq!(app.project(), &project);
        assert!(Arc::ptr_eq(&app.library, &library));
        assert_eq!(app.project_change_token(), token);
        assert_eq!(app.pending_open, pending);
        assert!(app.unsaved());
        assert!(app.has_history_step(false));
        assert!(!progress.is_cancelled());
        if recording {
            assert_eq!(
                app.recorder.take().unwrap().finish().unwrap().samples,
                vec![0.25, -0.25]
            );
        } else {
            assert!(Arc::ptr_eq(
                &app.wave_scan.as_ref().unwrap().progress,
                &progress
            ));
        }
    }
}

#[test]
fn project_change_tokens_track_live_edits_and_project_replacement() {
    let mut app = fixture();
    let initial = app.project_change_token();
    app.edit(EditCommand::SetTempo(app.project().bpm)).unwrap();
    assert_eq!(app.project_change_token(), initial);
    let master = MasterChain {
        bypass: true,
        ..app.project().master
    };
    app.edit(EditCommand::SetMaster(master)).unwrap();
    let edited = app.project_change_token();
    assert_eq!(
        edited.0, initial.0,
        "A live board edit keeps the prepared mix."
    );
    assert_ne!(edited, initial);
    app.history_step(false).unwrap();
    let restored = app.project_change_token();
    assert_ne!(restored, initial);
    assert_ne!(restored, edited);
    app.new_mix().unwrap();
    assert_ne!(app.project_change_token(), restored);
}

#[test]
fn render_worker_keeps_only_the_latest_pending_revision_and_cancels_it() {
    let app = fixture();
    let (sender, requests) = mpsc::channel();
    let (results, receiver) = mpsc::channel();
    let mut worker = RenderWorker {
        sender,
        receiver,
        in_flight: false,
        current_revision: None,
        pending: None,
    };
    let request = |revision| RenderRequest {
        revision,
        quality: crate::playback_quality::PlaybackQuality::FULL,
        project: app.editor.project().clone(),
        library: app.library.clone(),
    };
    worker.request(request(1)).unwrap();
    worker.request(request(2)).unwrap();
    worker.request(request(3)).unwrap();
    assert_eq!(requests.try_recv().unwrap().revision, 1);
    assert!(requests.try_recv().is_err());
    results
        .send(RenderResult {
            revision: 1,
            mix: Err("Fixture result".into()),
        })
        .unwrap();
    assert_eq!(worker.poll().unwrap().revision, 1);
    assert_eq!(
        requests.try_recv().unwrap().revision,
        3,
        "Intermediate fader revisions must not fill the worker queue."
    );
    worker.request(request(4)).unwrap();
    worker.cancel_pending();
    results
        .send(RenderResult {
            revision: 3,
            mix: Err("Fixture result".into()),
        })
        .unwrap();
    worker.poll().unwrap();
    assert!(requests.try_recv().is_err());
    assert!(!worker.in_flight);
}

#[test]
fn a_stopped_render_worker_fails_its_render_and_is_replaced() {
    let app = fixture();
    let (sender, requests) = mpsc::channel();
    let (results, receiver) = mpsc::channel::<RenderResult>();
    let mut worker = RenderWorker {
        sender,
        receiver,
        in_flight: false,
        current_revision: None,
        pending: None,
    };
    let request = |revision| RenderRequest {
        revision,
        quality: crate::playback_quality::PlaybackQuality::FULL,
        project: app.editor.project().clone(),
        library: app.library.clone(),
    };
    worker.request(request(1)).unwrap();
    worker.request(request(2)).unwrap();
    // The worker thread ends without a result, as it does after a panic.
    drop((requests, results));
    let failed = worker.poll().unwrap();
    assert_eq!(failed.revision, 1);
    assert!(failed.mix.is_err());
    // The replacement worker renders the request that was waiting.
    let finished = loop {
        if let Some(result) = worker.poll() {
            break result;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(finished.revision, 2);
    assert!(!worker.in_flight);
    assert!(worker.poll().is_none());
}

#[test]
fn render_results_cannot_replace_a_new_project_or_autoplay_after_pause() {
    let mut app = fixture();
    let (sender, _requests) = mpsc::channel();
    let (results, receiver) = mpsc::channel();
    app.render_worker = RenderWorker {
        sender,
        receiver,
        in_flight: false,
        current_revision: None,
        pending: None,
    };
    app.revision = 5;
    app.wants_playback = true;
    app.pause();
    let mix = Arc::new(audio::prepare_project(&Project::default(), &Library::empty()).unwrap());
    results
        .send(RenderResult {
            revision: 5,
            mix: Ok(mix.clone()),
        })
        .unwrap();
    app.poll_render();
    assert!(!app.wants_playback);
    assert!(
        app.render.is_some(),
        "A paused render may be cached without starting an audio device."
    );
    app.new_mix().unwrap();
    results
        .send(RenderResult {
            revision: 5,
            mix: Ok(mix),
        })
        .unwrap();
    app.poll_render();
    assert!(
        app.render.is_none(),
        "A previous project's late result must be discarded."
    );
    assert!(app.render_dirty);
}

#[test]
fn sections_keep_pending_audio_render() {
    for history in [None, Some(false), Some(true)] {
        let mut app = fixture();
        let sections = vec![Section {
            start_tick: 0,
            name: "Intro".into(),
        }];
        if history.is_some() {
            app.edit(EditCommand::SetSections(sections.clone()))
                .unwrap();
            if history == Some(true) {
                app.history_step(false).unwrap();
            }
        }
        let (sender, requests) = mpsc::channel();
        let (results, receiver) = mpsc::channel();
        app.render_worker = RenderWorker {
            sender,
            receiver,
            in_flight: false,
            current_revision: None,
            pending: None,
        };
        app.request_render().unwrap();
        let revision = requests.try_recv().unwrap().revision;
        app.wants_playback = true;
        let token = app.project_change_token();
        match history {
            Some(redo) => assert!(app.history_step(redo).unwrap()),
            None => app.edit(EditCommand::SetSections(sections)).unwrap(),
        }
        assert_ne!(app.project_change_token(), token);
        assert_eq!(app.revision, revision, "Section edits do not change audio.");
        assert_eq!(app.requested_revision, Some(revision));
        // Stop autoplay without using an audio device. The queued result must
        // still be accepted after the section edit and its history steps.
        app.wants_playback = false;
        let mix = Arc::new(audio::prepare_project(&Project::default(), &Library::empty()).unwrap());
        results
            .send(RenderResult {
                revision,
                mix: Ok(mix.clone()),
            })
            .unwrap();
        app.poll_render();
        assert!(Arc::ptr_eq(app.render.as_ref().unwrap(), &mix));
        assert!(!app.render_dirty);
        assert_eq!(app.requested_revision, None);
        assert!(requests.try_recv().is_err());
    }
}

#[test]
fn with_audio_output_each_edit_is_prepared_before_play() {
    let mut app = fixture();
    let (sender, requests) = mpsc::channel();
    let (results, receiver) = mpsc::channel();
    app.render_worker = RenderWorker {
        sender,
        receiver,
        in_flight: false,
        current_revision: None,
        pending: None,
    };
    app.prepares_ahead = true;
    app.place(1, 2, 0).unwrap();
    assert_eq!(requests.try_recv().unwrap().revision, app.revision);
    let mix = Arc::new(audio::prepare_project(&Project::default(), &Library::empty()).unwrap());
    results
        .send(RenderResult {
            revision: app.revision,
            mix: Ok(mix),
        })
        .unwrap();
    app.poll();
    assert!(app.render.is_some() && !app.render_dirty);
    assert!(
        !app.wants_playback,
        "A mix prepared ahead does not start playback."
    );
    assert!(requests.try_recv().is_err());

    // A failure waits for Play, which tries again and reports it.
    app.take_notice();
    app.place(1, 3, 0).unwrap();
    let revision = requests.try_recv().unwrap().revision;
    results
        .send(RenderResult {
            revision,
            mix: Err("Fixture failure".into()),
        })
        .unwrap();
    app.poll();
    app.poll();
    assert!(app.take_notice().is_none());
    assert!(requests.try_recv().is_err());
    assert!(app.render_dirty);
}

#[test]
fn unavailable_library_rejects_undo_atomically_until_the_source_returns() {
    let mut app = fixture();
    let original_library = app.library.clone();
    app.edit(EditCommand::DeleteClips(vec![1, 2])).unwrap();
    let empty_project = app.editor.project().clone();
    assert!(empty_project.clips.is_empty());
    let mut removed = (*app.library).clone();
    removed.remove_product(Product::T2);
    app.library = Arc::new(removed);
    let removed_library = app.library.clone();
    let error = app.history_step(false).unwrap_err();
    assert!(error.contains("sample 1 is unavailable"));
    assert_eq!(*app.editor.project(), empty_project);
    assert!(Arc::ptr_eq(&removed_library, &app.library));
    assert!(
        !app.editor.redo(),
        "A rejected undo must not leave a redo entry."
    );
    app.library = original_library;
    app.history_step(false).unwrap();
    assert_eq!(app.editor.project().clips.len(), 2);
    assert!(app.library.sample_by_id(1).is_some());
    app.history_step(true).unwrap();
    assert!(app.editor.project().clips.is_empty());
}
