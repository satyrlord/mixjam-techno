//! Sample libraries: folders, scans, adoption, catalog import and removal.

use super::*;

#[test]
fn imported_catalog_tracks_history_atomically_and_keeps_source_tempo() {
    let (_, mut app) = fixture();
    app.sounds.product = Some(Product::T1);
    let path = std::env::temp_dir().join(format!(
        "mixjam-techno-ui-import-{}.wav",
        std::process::id()
    ));
    let pcm = Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.25; 1000],
    };
    pcm.write_wav(&path).unwrap();
    let id = app.register_file(&path, "Wave", 90.0).unwrap();
    assert_eq!(
        app.session.library().sample_by_id(id).unwrap().source_bpm,
        90.0
    );
    assert_eq!(app.session.project().imported_samples.len(), 1);
    assert!(app.sounds.product.is_none());
    assert!(
        app.palette_indices()
            .iter()
            .any(|&index| app.session.library().samples[index].id == id)
    );
    app.history_step(false);
    assert!(app.session.library().sample_by_id(id).is_none());
    app.history_step(true);
    assert!(app.session.library().sample_by_id(id).is_some());
    app.history_step(false);
    std::fs::remove_file(&path).unwrap();
    let before = app.session.project().clone();
    app.history_step(true);
    assert_eq!(
        app.session.project(),
        &before,
        "A missing imported file must reject redo atomically."
    );
    assert!(app.session.library().sample_by_id(id).is_none());
    assert!(app.status_error);
}

#[test]
fn failed_open_and_in_use_library_removal_preserve_the_active_project() {
    let (_, mut app) = fixture();
    let previous = app.session.project().clone();
    let library = app.session.library().samples.as_ptr();
    app.open_path(Path::new("missing-ui-fixture.invalid"));
    assert!(app.status_error);
    assert_eq!(app.session.project(), &previous);
    assert!(std::ptr::eq(
        library,
        app.session.library().samples.as_ptr()
    ));
    app.remove_library(Product::T2);
    assert!(app.status_error);
    assert!(app.status.contains("This mix uses"));
    assert_eq!(app.session.project(), &previous);
    assert!(std::ptr::eq(
        library,
        app.session.library().samples.as_ptr()
    ));
}

#[test]
fn sample_libraries_name_each_library_folder_control() {
    let dir = std::env::temp_dir().join(format!("mixjam-ui-folders-{}", std::process::id()));
    let roots = [dir.join("First"), dir.join("Second")];
    for root in &roots {
        std::fs::create_dir_all(root).unwrap();
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.25; 441],
        }
        .write_wav(&root.join("sound.wav"))
        .unwrap();
    }
    let shown = |roots: &[PathBuf], check: &dyn Fn(&egui::FullOutput)| {
        let library = Library::open_wav_folders_with(roots, 140.0, None, None).unwrap();
        let session = Session::headless(Arc::new(library), Project::default()).unwrap();
        let mut app = MixJamApp::from_session(session, String::new(), false);
        app.options = Some(options::OptionsDraft::new(
            &app.session,
            options::Page::Libraries,
        ));
        let context = egui::Context::default();
        configure_style(&context);
        context.enable_accesskit();
        for _ in 0..3 {
            frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        }
        check(&frame(&context, &mut app, vec![], egui::Modifiers::NONE));
    };
    shown(&roots, &|output| {
        for name in [
            "Add library folder",
            "Scan again",
            "Remove library 1",
            "Remove library 2",
        ] {
            assert!(!accessible_node(output, name).1.is_disabled(), "{name}");
        }
    });
    // The list keeps at least one folder.
    shown(&roots[..1], &|output| {
        assert!(accessible_node(output, "Remove library 1").1.is_disabled());
    });
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn folder_tempo_list_scrolls_to_provider_controls_at_compact_size() {
    let dir = std::env::temp_dir().join(format!("mixjam-ui-tempos-{}", std::process::id()));
    for index in 0..60 {
        let folder = dir.join(format!("Folder {index:02}"));
        std::fs::create_dir_all(&folder).unwrap();
        for name in ["a", "b", "c"] {
            Pcm {
                sample_rate: 100,
                channels: 1,
                samples: vec![0.25; 200],
            }
            .write_wav(&folder.join(format!("{name}.wav")))
            .unwrap();
        }
    }
    let library = Library::open_wav_folder(&dir, 120.0).unwrap();
    assert_eq!(library.folder_tempos().len(), 60);
    let session = Session::headless(Arc::new(library), Project::default()).unwrap();
    let mut app = MixJamApp::from_session(session, String::new(), false);
    app.options = Some(options::OptionsDraft::new(
        &app.session,
        options::Page::Libraries,
    ));
    let context = egui::Context::default();
    configure_style(&context);
    let size = Vec2::new(960.0, 720.0);
    for _ in 0..3 {
        frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
    let position = text_rect(&output, "Folder tempos (60)").center();
    for pressed in [true, false] {
        frame_at_size(
            &context,
            &mut app,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
            size,
        );
    }
    for _ in 0..20 {
        frame_at_size(
            &context,
            &mut app,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -500.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
            size,
        );
    }
    let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
    let provider = text_rect(&output, "Optional product providers");
    assert!(
        Rect::from_min_size(Pos2::ZERO, size).contains_rect(provider),
        "Provider controls must remain reachable after expanding a large tempo list: {provider:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn new_mix_cancels_a_sample_drag_before_its_pointer_release() {
    let (context, mut app) = fixture();
    app.new_mix();
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let target = app.score.drop_rect.unwrap().center();
    pointer(&context, &mut app, target, true, egui::Modifiers::NONE);
    app.gestures.drag = Some(DragItem::Sample(1));
    app.request_action(PendingAction::New, &context);
    pointer(&context, &mut app, target, false, egui::Modifiers::NONE);
    assert!(app.session.project().clips.is_empty());
    assert!(!app.session.unsaved());
    assert!(app.gestures.drag.is_none());
}

#[test]
fn scan_adoption_clears_selection_and_drag_from_the_previous_palette() {
    let dir = std::env::temp_dir().join(format!("mixjam-ui-scan-{}", std::process::id()));
    let old = dir.join("old");
    let new = dir.join("new");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir_all(&new).unwrap();
    for (folder, value) in [(&old, 0.25), (&new, 0.5)] {
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![value; 1000],
        }
        .write_wav(&folder.join("sound.wav"))
        .unwrap();
    }
    let library = Library::open_wav_folder(&old, 140.0).unwrap();
    let source = library.samples[0].id;
    let session = Session::headless(Arc::new(library), Project::default())
        .unwrap()
        .with_settings_root(dir.join("settings"));
    let mut app = MixJamApp::from_session(session, String::new(), false);
    let context = egui::Context::default();
    app.scan_library_folders(vec![new.clone()]);
    app.selection.sample = Some(source);
    app.gestures.drag = Some(DragItem::Sample(source));
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while app.session.wave_scan().is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll_application(&context);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(app.session.library().samples[0].id, source);
    assert!(
        app.selection.sample.is_none(),
        "The old ID now identifies another sound."
    );
    assert!(
        app.gestures.drag.is_none(),
        "An in-flight drag must not place the replacement sound."
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn startup_scan_closes_sample_libraries_only_when_it_fills_the_library() {
    let dir = std::env::temp_dir().join(format!("mixjam-ui-startup-{}", std::process::id()));
    let full = dir.join("full");
    let empty = dir.join("empty");
    std::fs::create_dir_all(&full).unwrap();
    std::fs::create_dir_all(&empty).unwrap();
    Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.25; 1000],
    }
    .write_wav(&full.join("sound.wav"))
    .unwrap();
    let context = egui::Context::default();
    let startup = |root: &Path, manual: bool| {
        let mut session = Session::headless(Arc::new(Library::empty()), Project::default())
            .unwrap()
            .with_settings_root(dir.join("settings"));
        session
            .start_wave_scan(&[root.to_path_buf()], 140.0)
            .unwrap();
        let mut app = MixJamApp::from_session(session, String::new(), false);
        assert!(app.sample_libraries_open());
        if manual {
            app.scan_library_folders(vec![root.to_path_buf()]);
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while app.session.wave_scan().is_some() {
            assert!(std::time::Instant::now() < deadline);
            app.poll_application(&context);
            std::thread::sleep(Duration::from_millis(5));
        }
        app.poll_application(&context);
        app.sample_libraries_open()
    };
    assert!(
        !startup(&full, false),
        "The startup scan filled the library."
    );
    assert!(
        startup(&empty, false),
        "An empty library still needs a folder."
    );
    assert!(
        startup(&full, true),
        "The user chose this folder in the window."
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn adoption_status_and_import_notice_survive_the_next_application_poll() {
    let (context, mut app) = fixture();
    let path = std::env::temp_dir().join(format!("mixjam-ui-adoption-{}.json", std::process::id()));
    let mut project = Project::default();
    project.import_notices.push(ImportNotice {
        source_bpm: 146.0,
        target_bpm: 140.0,
    });
    project.save(&path).unwrap();
    app.open_path(&path);
    let status = app.status.clone();
    assert!(status.contains("Opened") && status.contains("146 BPM changed to 140 BPM"));
    app.poll_application(&context);
    assert_eq!(app.status, status);
    app.new_mix();
    let status = app.status.clone();
    app.poll_application(&context);
    assert_eq!(app.status, status);
    std::fs::remove_file(path).unwrap();
}
