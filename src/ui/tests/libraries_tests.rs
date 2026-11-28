//! Sample libraries: folders, scans, adoption, catalog import and removal.

use super::*;

#[test]
fn imported_catalog_tracks_history_atomically_and_keeps_source_tempo() {
    let (_, mut app) = fixture();
    app.sounds.product = Some(Product::T1);
    let folder = crate::test_folder::TestFolder::new("mixjam-techno-ui-import");
    let path = folder.join("import.wav");
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
    let dir = crate::test_folder::TestFolder::new("mixjam-ui-folders");
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
        app.libraries.open = true;
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
}

#[test]
fn folder_tempo_list_scrolls_to_provider_controls_at_compact_size() {
    let dir = crate::test_folder::TestFolder::new("mixjam-ui-tempos");
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
    app.libraries.open = true;
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
    let dir = crate::test_folder::TestFolder::new("mixjam-ui-scan");
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
}

#[test]
fn startup_scan_closes_sample_libraries_only_when_it_fills_the_library() {
    let dir = crate::test_folder::TestFolder::new("mixjam-ui-startup");
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
}

#[test]
fn adoption_status_and_import_notice_survive_the_next_application_poll() {
    let (context, mut app) = fixture();
    let folder = crate::test_folder::TestFolder::new("mixjam-ui-adoption");
    let path = folder.join("adoption.json");
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
}

/// Wait for the library scan to finish and take its result.
fn finish_scan(ctx: &egui::Context, app: &mut MixJamApp) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app.session.wave_scan().is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll_application(ctx);
        std::thread::yield_now();
    }
    app.poll_application(ctx);
}

/// A one-sound folder at `folder`.
fn sound_folder(folder: &std::path::Path) {
    std::fs::create_dir_all(folder).unwrap();
    Pcm {
        sample_rate: 44100,
        channels: 1,
        samples: vec![0.25; 441],
    }
    .write_wav(&folder.join("sound.wav"))
    .unwrap();
}

#[test]
fn closing_sample_libraries_keeps_its_scan_and_the_adopted_folders() {
    let (ctx, mut app, root) = isolated_fixture();
    let folder = root.join("samples");
    sound_folder(&folder);
    app.open_sample_libraries(&ctx);
    // Let the native Area finish its initial sizing before input.
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    assert!(app.sample_libraries_open());
    app.try_scan_library_folders(vec![folder.clone()], 140.0)
        .unwrap();
    click_named(&ctx, &mut app, "Close Sample libraries");
    assert!(!app.sample_libraries_open());
    finish_scan(&ctx, &mut app);
    assert_eq!(
        app.session.library().wav_roots(),
        &[std::fs::canonicalize(folder).unwrap()]
    );
    assert_eq!(app.session.library().samples.len(), 1);
    assert!(!app.session.has_history_step(false));
    remove_settings(&root);
}

#[test]
fn sample_libraries_window_acts_at_once_with_a_plain_fallback_tempo() {
    let (ctx, mut app, root) = isolated_fixture();
    app.open_sample_libraries(&ctx);
    let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for _ in 0..2 {
        output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    // No draft, so no Apply, Cancel, Restore, or Done.
    for name in [
        "Apply",
        "Cancel",
        "Restore all defaults",
        "Done",
        "Fallback BPM",
    ] {
        assert!(!has_control(&output, name), "{name} is in Sample libraries");
    }
    assert!(has_control(&output, "Tempo for files without one"));
    assert!(shows_text(&output, "Changes take effect right away."));
    assert!(shows_text(
        &output,
        "Applies to the next Add library folder or Scan again."
    ));
    assert!(app.options.is_none(), "Sample libraries opens alone");
    click_named(&ctx, &mut app, "Close Sample libraries");
    assert!(!app.sample_libraries_open());
    remove_settings(&root);
}

#[test]
fn long_library_paths_wrap_in_full_without_a_hover() {
    let (ctx, mut app, root) = isolated_fixture();
    let folder = root
        .join("a library folder with a deliberately long name for wrapping checks")
        .join("another nested folder whose name keeps the path wider than the window");
    sound_folder(&folder);
    app.try_scan_library_folders(vec![folder.clone()], 140.0)
        .unwrap();
    finish_scan(&ctx, &mut app);
    app.open_sample_libraries(&ctx);
    let size = Vec2::new(960.0, 720.0);
    let mut output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    for _ in 0..3 {
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let path = display_paths(&app.session.library().wav_roots()[0].to_string_lossy());
    let label = screen(&output)
        .nodes()
        .find(|node| node.value() == Some(path.as_str()))
        .expect("The full library path is shown")
        .rect();
    // More than one line, inside the window.
    assert!(label.height() > 24.0, "{label:?}");
    assert!(label.right() <= size.x, "{label:?}");
    remove_settings(&root);
}

#[test]
fn sample_libraries_opens_beside_options_when_room_and_stays_inside_compact_windows() {
    for size in [
        Vec2::new(1440.0, 1000.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(960.0, 720.0),
    ] {
        let (ctx, mut app, root) = isolated_fixture();
        // First paint the startup window without Options, so its centered
        // geometry is cached before the Files page opens it again.
        app.open_sample_libraries(&ctx);
        for _ in 0..4 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        frame_at_size(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
            size,
        );
        assert!(!app.sample_libraries_open());
        app.options = Some(options::OptionsWindow::new(
            &app.session,
            options::Page::Files,
        ));
        for _ in 0..4 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        if size.x == 1440.0 {
            // Move Options to the left so both windows fit side by side.
            let before = dialog::window_rect(&ctx, options::labels::WINDOW).unwrap();
            let start = Pos2::new(before.center().x, before.top() + 22.0);
            pointer(&ctx, &mut app, start, true, egui::Modifiers::NONE);
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(start - Vec2::new(8.0, 0.0))],
                egui::Modifiers::NONE,
            );
            let end = start - Vec2::new(before.left(), 0.0);
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(end)],
                egui::Modifiers::NONE,
            );
            pointer(&ctx, &mut app, end, false, egui::Modifiers::NONE);
        }
        app.open_sample_libraries(&ctx);
        for _ in 0..4 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let options = dialog::window_rect(&ctx, options::labels::WINDOW).unwrap();
        let libraries = dialog::window_rect(&ctx, libraries::TITLE).unwrap();
        let viewport = Rect::from_min_size(Pos2::ZERO, size).shrink(8.0);
        assert!(viewport.contains_rect(libraries), "{size}: {libraries:?}");
        if size.x == 1440.0 {
            assert!(
                viewport.right() - options.right() >= libraries.width() + 8.0,
                "The fixture must leave room beside Options: {options:?}"
            );
            assert!(
                !options.intersects(libraries),
                "Sample libraries must open beside Options: {options:?} / {libraries:?}"
            );
            // Repeated activation expands and focuses the existing window;
            // it must preserve the person's position and size.
            let start = Pos2::new(libraries.center().x, libraries.top() + 22.0);
            pointer(&ctx, &mut app, start, true, egui::Modifiers::NONE);
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(start + Vec2::new(8.0, 0.0))],
                egui::Modifiers::NONE,
            );
            let end = start + Vec2::new(32.0, 0.0);
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(end)],
                egui::Modifiers::NONE,
            );
            pointer(&ctx, &mut app, end, false, egui::Modifiers::NONE);
            let moved = dialog::window_rect(&ctx, libraries::TITLE).unwrap();
            assert!(moved.left() > libraries.left(), "The window must move.");
            app.open_sample_libraries(&ctx);
            for _ in 0..3 {
                frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            }
            assert_eq!(dialog::window_rect(&ctx, libraries::TITLE), Some(moved));
        }
        remove_settings(&root);
    }
}

#[test]
fn scan_labels_format_the_path_and_agent_status_keeps_its_spelling() {
    let (ctx, mut app, root) = isolated_fixture();
    let folder = root.join("samples");
    sound_folder(&folder);
    let path = std::fs::canonicalize(folder).unwrap();
    app.open_sample_libraries(&ctx);
    app.try_scan_library_folders(vec![path.clone()], 140.0)
        .unwrap();
    assert_eq!(app.status, format!("Scanning {}…", path.display()));
    // Draw the window before the next application poll adopts the scan.
    let render = |app: &mut MixJamApp| {
        ctx.run(egui::RawInput::default(), |ctx| {
            app.sample_libraries_window(ctx)
        })
    };
    render(&mut app);
    let output = render(&mut app);
    let label = format!("Scanning {}", display_paths(&path.to_string_lossy()));
    accessible_label(&output, &label);
    finish_scan(&ctx, &mut app);
    remove_settings(&root);
    assert_eq!(
        actions::scan_subject(&[PathBuf::from("a"), PathBuf::from("b")]),
        "2 library folders"
    );
}

#[test]
fn register_file_keeps_original_folder_category() {
    let dir = reserve_settings_folder();
    let drum_dir = dir.join("Drum");
    std::fs::create_dir_all(&drum_dir).unwrap();
    let kick = drum_dir.join("kick.wav");
    Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.25; 44_100],
    }
    .write_wav(&kick)
    .unwrap();
    let library = Library::open_wav_folder(&dir, 140.0).unwrap();
    let project = Project {
        libraries: library.bindings().to_vec(),
        imported_samples: library.imported_samples().into(),
        ..Project::default()
    };
    let session = Session::headless(Arc::new(library), project).unwrap();
    let mut app = MixJamApp::from_session(session, String::new(), false);
    let id = app.register_file(&kick, "Wave", 140.0).unwrap();
    assert!(
        app.session
            .library()
            .samples
            .iter()
            .any(|sample| sample.group_name == "Drum" && sample.name == "kick"),
        "Original Drum sample must stay in Drum."
    );
    assert_eq!(
        app.session.library().sample_by_id(id).unwrap().group_name,
        "Wave"
    );
    app.history_step(false);
    assert!(app.session.library().sample_by_id(id).is_none());
    assert!(
        app.session
            .library()
            .samples
            .iter()
            .any(|sample| sample.group_name == "Drum" && sample.name == "kick"),
        "Undoing the Wave import must preserve the Drum library sample."
    );
}
