use super::options::*;
use super::*;
use crate::library::Library;

fn fixture() -> (egui::Context, MixJamApp, PathBuf) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "mixjam-options-ui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let session = Session::headless(Arc::new(Library::empty()), Project::default())
        .unwrap()
        .with_settings_root(root.clone());
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    configure_style(&ctx);
    let mut app = MixJamApp::from_session(session, "Options fixture".into(), false);
    app.options = None;
    app.capture_path = None;
    (ctx, app, root)
}
fn frame(
    ctx: &egui::Context,
    app: &mut MixJamApp,
    size: Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            focused: true,
            events,
            ..Default::default()
        },
        |ctx| app.draw(ctx),
    )
}
fn node<'a>(output: &'a egui::FullOutput, name: &str) -> &'a egui::accesskit::Node {
    &output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(name))
        .unwrap_or_else(|| panic!("Missing accessible control: {name}"))
        .1
}
fn click(ctx: &egui::Context, app: &mut MixJamApp, name: &str) {
    let size = Vec2::new(1024.0, 768.0);
    let output = frame(ctx, app, size, vec![]);
    let rect = node(&output, name).bounds().unwrap();
    let pos = Pos2::new(
        ((rect.x0 + rect.x1) / 2.0) as f32,
        ((rect.y0 + rect.y1) / 2.0) as f32,
    );
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            size,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}
fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn focus(ctx: &egui::Context, app: &mut MixJamApp, name: &str) {
    let size = Vec2::new(1024.0, 768.0);
    let output = frame(ctx, app, size, vec![]);
    let id = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(name))
        .unwrap()
        .0;
    frame(
        ctx,
        app,
        size,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target: id,
                data: None,
            },
        )],
    );
}

#[test]
fn options_keyboard_edits_applies_restores_and_cancels() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    frame(&ctx, &mut app, size, vec![]);
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Comma, egui::Modifiers::COMMAND)],
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
    );
    assert_eq!(app.options.as_ref().unwrap().page, Page::Workspace);
    // Tab traverses the page selector before the option controls.
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    let output = frame(&ctx, &mut app, size, vec![]);
    let focus_id = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .focus;
    assert_eq!(
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(id, _)| *id == focus_id)
            .and_then(|(_, node)| node.label()),
        Some("Files")
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    let output = frame(&ctx, &mut app, size, vec![]);
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .find(|(id, _)| *id == tree.focus)
            .and_then(|(_, node)| node.label()),
        Some("Sample libraries")
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Space, egui::Modifiers::NONE)],
    );
    assert!(!app.options.as_ref().unwrap().values.follow_on_startup);
    assert!(app.session.options().follow_on_startup);
    focus(&ctx, &mut app, "Apply");
    let wrapped = frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    let tree = wrapped.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .find(|(id, _)| *id == tree.focus)
            .and_then(|(_, node)| node.label()),
        Some("Collapse Options")
    );
    let reversed = frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Tab, egui::Modifiers::SHIFT)],
    );
    let tree = reversed.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .find(|(id, _)| *id == tree.focus)
            .and_then(|(_, node)| node.label()),
        Some("Apply")
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    assert!(!app.session.options().follow_on_startup);
    focus(&ctx, &mut app, "Restore options defaults");
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Space, egui::Modifiers::NONE)],
    );
    assert!(app.options.as_ref().unwrap().values.follow_on_startup);
    assert!(!app.session.options().follow_on_startup);
    focus(&ctx, &mut app, "Apply");
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    assert!(app.session.options().follow_on_startup);
    focus(&ctx, &mut app, "Cancel");
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Space, egui::Modifiers::NONE)],
    );
    assert!(app.options.is_none());
    assert!(!app.session.has_history_step(false));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn options_escape_respects_a_higher_utility_window() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    app.open_options(&ctx);
    frame(&ctx, &mut app, size, vec![]);
    app.show_import_details = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, size, vec![]);
    }
    assert!(dialog::top_window(&ctx, "Import details"));
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(!app.show_import_details);
    assert!(app.options.is_some());
    if root.exists() {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn options_actions_and_page_navigation_fit_supported_windows() {
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        for page in [Page::Audio, Page::Workspace, Page::Files, Page::Libraries] {
            let (ctx, mut app, root) = fixture();
            app.options = Some(OptionsDraft::new(&app.session, page));
            let mut output = frame(&ctx, &mut app, size, vec![]);
            for _ in 0..2 {
                output = frame(&ctx, &mut app, size, vec![]);
            }
            for name in [
                "Audio",
                "Workspace",
                "Files",
                "Sample libraries",
                "Apply",
                "Cancel",
                "Restore options defaults",
            ] {
                let bounds = node(&output, name).bounds().unwrap();
                assert!(
                    bounds.x0 >= 0.0
                        && bounds.y0 >= 0.0
                        && bounds.x1 <= f64::from(size.x)
                        && bounds.y1 <= f64::from(size.y),
                    "{name} is outside {size:?}: {bounds:?}"
                );
            }
            assert!(node(&output, "Apply").bounds().unwrap().y0 > f64::from(size.y) / 2.0);
            if root.exists() {
                std::fs::remove_dir_all(root).unwrap();
            }
        }
    }
}

#[test]
fn options_gear_focuses_one_window_and_preserves_the_draft() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    frame(&ctx, &mut app, size, vec![]);
    click(&ctx, &mut app, "Options");
    app.options.as_mut().unwrap().values.follow_on_startup = false;
    click(&ctx, &mut app, "Files");
    click(&ctx, &mut app, "Go to sample libraries");
    assert!(app.sample_libraries_open());
    assert!(!app.options.as_ref().unwrap().values.follow_on_startup);
    assert!(app.session.options().follow_on_startup);
    click(&ctx, &mut app, "Options");
    assert!(app.sample_libraries_open());
    assert!(!app.options.as_ref().unwrap().values.follow_on_startup);
    let output = frame(&ctx, &mut app, size, vec![]);
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .filter(|(_, node)| node.role() == egui::accesskit::Role::Dialog
                && node.label() == Some("OPTIONS"))
            .count(),
        1
    );
    assert!(
        !tree
            .nodes
            .iter()
            .any(|(_, node)| node.role() == egui::accesskit::Role::Dialog
                && node.label() == Some("SAMPLE LIBRARIES"))
    );
    click(&ctx, &mut app, "Collapse Options");
    assert!(dialog::is_collapsed(&ctx, "Options"));
    click(&ctx, &mut app, "Options");
    assert!(!dialog::is_collapsed(&ctx, "Options"));
    assert!(!app.options.as_ref().unwrap().values.follow_on_startup);
    click(&ctx, &mut app, "Cancel");
    assert!(app.options.is_none());
    let output = frame(&ctx, &mut app, size, vec![]);
    assert_eq!(
        node(&output, "Options").toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    if root.exists() {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn options_keyboard_navigates_all_four_pages_without_changing_preferences() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    app.open_options(&ctx);
    frame(&ctx, &mut app, size, vec![]);
    for page in [Page::Workspace, Page::Files, Page::Libraries, Page::Audio] {
        frame(
            &ctx,
            &mut app,
            size,
            vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
        );
        assert_eq!(app.options.as_ref().unwrap().page, page);
    }
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::ArrowUp, egui::Modifiers::NONE)],
    );
    assert_eq!(app.options.as_ref().unwrap().page, Page::Libraries);
    assert_eq!(app.options.as_ref().unwrap().values, *app.session.options());
    if root.exists() {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn closing_options_keeps_library_scan_and_its_adopted_folders() {
    let (ctx, mut app, root) = fixture();
    let folder = root.join("samples");
    std::fs::create_dir_all(&folder).unwrap();
    Pcm {
        sample_rate: 44100,
        channels: 1,
        samples: vec![0.25; 441],
    }
    .write_wav(&folder.join("sound.wav"))
    .unwrap();
    app.open_sample_libraries(&ctx);
    app.options.as_mut().unwrap().values.follow_on_startup = false;
    // Let the native Area finish its initial sizing before pointer input.
    for _ in 0..3 {
        frame(&ctx, &mut app, Vec2::new(1024.0, 768.0), vec![]);
    }
    assert!(app.sample_libraries_open());
    app.try_scan_library_folders(vec![folder.clone()], 140.0)
        .unwrap();
    click(&ctx, &mut app, "Cancel");
    assert!(app.options.is_none());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app.session.wave_scan().is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll_application(&ctx);
        std::thread::yield_now();
    }
    app.poll_application(&ctx);
    assert_eq!(
        app.session.library().wav_roots(),
        &[std::fs::canonicalize(folder).unwrap()]
    );
    assert_eq!(app.session.library().samples.len(), 1);
    assert!(app.session.options().follow_on_startup);
    assert!(!app.session.has_history_step(false));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn options_keyboard_cancel_restores_focus_and_keeps_draft_out_of_song() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    let output = frame(&ctx, &mut app, size, vec![]);
    let follow = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some("Follow"))
        .unwrap()
        .0;
    frame(
        &ctx,
        &mut app,
        size,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target: follow,
                data: None,
            },
        )],
    );
    let previous_focus = ctx.memory(|memory| memory.focused());
    assert!(previous_focus.is_some());
    let project = app.session.project().clone();
    let revision = app.session.options_revision();
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Comma, egui::Modifiers::COMMAND)],
    );
    assert!(app.options.is_some());
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
    );
    assert_eq!(app.options.as_ref().unwrap().page, Page::Workspace);
    app.options.as_mut().unwrap().values.follow_on_startup = false;
    frame(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(app.options.is_none());
    assert_eq!(ctx.memory(|memory| memory.focused()), previous_focus);
    assert!(app.session.options().follow_on_startup);
    assert_eq!(app.session.options_revision(), revision);
    assert_eq!(app.session.project(), &project);
    assert!(!app.session.has_history_step(false));
    assert!(!root.join("options.json").exists());
}

#[test]
fn options_apply_cancel_restore_and_stale_reload_share_one_active_value() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    app.open_options(&ctx);
    frame(&ctx, &mut app, size, vec![]);
    click(&ctx, &mut app, "Workspace");
    click(&ctx, &mut app, "Show section bookmarks");
    assert!(app.session.options().show_section_bookmarks);
    click(&ctx, &mut app, "Apply");
    assert!(!app.session.options().show_section_bookmarks);
    assert!(!app.section_bookmarks);
    assert!(app.options.is_some());
    click(&ctx, &mut app, "Restore options defaults");
    assert!(!app.session.options().show_section_bookmarks);
    app.session.remember_section_bookmarks(true).unwrap();
    let old_revision = app.options.as_ref().unwrap().revision;
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(node(&output, "Apply").is_disabled());
    assert_eq!(app.options.as_ref().unwrap().revision, old_revision);
    assert!(app.options.as_ref().unwrap().restored);
    click(&ctx, &mut app, "Reload active options");
    assert_eq!(
        app.options.as_ref().unwrap().revision,
        app.session.options_revision()
    );
    click(&ctx, &mut app, "Cancel");
    assert!(app.options.is_none());
    assert!(app.session.options().show_section_bookmarks);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn options_folder_picker_cancel_retains_draft_and_selection_is_not_apply() {
    let (ctx, mut app, root) = fixture();
    app.open_options(&ctx);
    let before = app.options.as_ref().unwrap().values.clone();
    app.finish_path(FileRequest::OptionsFolder, None);
    assert_eq!(app.options.as_ref().unwrap().values, before);
    app.finish_path(FileRequest::OptionsFolder, Some(root.clone()));
    assert_eq!(
        app.options.as_ref().unwrap().values.fixed_song_folder,
        Some(root.clone())
    );
    assert_eq!(app.session.options(), &before);
    assert!(!root.join("options.json").exists());
}

#[test]
fn reduced_meter_display_holds_values_and_retains_peak_events() {
    let mut display = MeterDisplay::default();
    let mut reading = MeterReading {
        run: 1,
        input_vu: -4.0,
        ..MeterReading::default()
    };
    assert_eq!(display.sample(reading, true, 0.0).input_vu, -4.0);
    reading.input_vu = -2.0;
    reading.input_peak_counts = [1, 0];
    reading.overs = 1;
    let held = display.sample(reading, true, 0.02);
    assert_eq!(held.input_vu, -4.0);
    assert_eq!(held.overs, 0);
    reading.input_vu = -6.0;
    let shown = display.sample(reading, true, 0.07);
    assert_eq!(shown.input_vu, -6.0);
    assert_eq!(shown.overs, 1);
    assert_eq!(shown.input_peaks, [true, false]);
    assert_eq!(display.sample(reading, true, 0.09), shown);
    assert!(!display.sample(reading, true, 0.14).input_peaks[0]);
    reading.run = 2;
    reading.input_peak_counts = [0, 0];
    reading.overs = 0;
    let restarted = display.sample(reading, true, 0.15);
    assert_eq!(restarted.input_peaks, [false, false]);
    assert_eq!(restarted.overs, 0);
}

#[test]
fn reduced_meter_needle_does_not_interpolate_between_display_samples() {
    let ctx = egui::Context::default();
    let id = egui::Id::new("reduced-needle-test");
    for (index, target) in [0.2_f32, 0.8, 0.4, 0.9].into_iter().enumerate() {
        let _ = ctx.run(
            egui::RawInput {
                time: Some(index as f64 * 0.02),
                ..Default::default()
            },
            |ctx| {
                assert_eq!(master::display_needle(ctx, id, target, true), target);
            },
        );
    }
    let reduced = master::Metering {
        reading: MeterReading::default(),
        playing: true,
        reduced: true,
    };
    assert!(reduced.repaint_delay().as_secs_f64() >= 1.0 / 15.0);
    let normal = master::Metering {
        reduced: false,
        ..reduced
    };
    assert_eq!(normal.repaint_delay(), Duration::from_millis(33));
}

#[test]
fn hiding_tooltips_keeps_accessible_control_descriptions() {
    let (ctx, mut app, root) = fixture();
    let mut values = app.session.options().clone();
    values.show_tooltips = false;
    app.session
        .apply_options(values, app.session.options_revision(), false)
        .unwrap();
    let output = frame(&ctx, &mut app, Vec2::new(1024.0, 768.0), vec![]);
    assert_eq!(
        node(&output, "Follow").description(),
        Some("Keep the playhead in view during playback and seeking.")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn hidden_pointer_hints_keep_help_without_opening_tooltips() {
    for show in [true, false] {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        ctx.style_mut(|style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("show-hover-hints"), show));
        let mut output = egui::FullOutput::default();
        for frame in 0..4 {
            output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
                    time: Some(frame as f64),
                    events: vec![egui::Event::PointerMoved(Pos2::new(20.0, 20.0))],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.add_sized([200.0, 50.0], egui::Button::new("Pointer control"))
                            .hover_hint_at_pointer("Pointer help text");
                    });
                },
            );
        }
        assert_eq!(
            node(&output, "Pointer control").description(),
            Some("Pointer help text")
        );
        let tooltip = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| {
                node.role() == egui::accesskit::Role::Label
                    && node.value() == Some("Pointer help text")
            });
        assert_eq!(tooltip, show);
    }
}

#[test]
fn hover_hints_preserve_explicit_accessible_state() {
    for show in [false, true] {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("show-hover-hints"), show));
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let response = ui.button("State control");
                ctx.accesskit_node_builder(response.id, |node| {
                    node.set_description("OVER: detected.")
                });
                response.hover_hint("Long hover instructions.");
                let pointer = ui.button("Pointer state control");
                ctx.accesskit_node_builder(pointer.id, |node| {
                    node.set_description("Current value.")
                });
                pointer.hover_hint_at_pointer("Long pointer instructions.");
            });
        });
        assert_eq!(
            node(&output, "State control").description(),
            Some("OVER: detected.")
        );
        assert_eq!(
            node(&output, "Pointer state control").description(),
            Some("Current value.")
        );
    }
}
