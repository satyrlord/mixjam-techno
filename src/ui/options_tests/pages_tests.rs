//! Options pages: sample libraries, files, audio, display, hints, theme and lane height
//! settings.

use super::*;

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
    click(&ctx, &mut app, "Done");
    click(&ctx, &mut app, "Discard changes");
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
fn sample_libraries_page_has_one_done_action_and_a_plain_fallback_tempo() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    app.open_sample_libraries(&ctx);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..2 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    for name in [
        "Apply",
        "Cancel",
        "Restore options defaults",
        "Fallback BPM",
    ] {
        assert!(
            !tree
                .nodes
                .iter()
                .any(|(_, node)| node.label() == Some(name)),
            "{name} is on the Sample libraries page"
        );
    }
    node(&output, "Tempo for files without one");
    assert!(has_text(
        &output,
        "Applies to the next Add library folder or Scan again."
    ));
    // Done closes a clean draft at once.
    click(&ctx, &mut app, "Done");
    assert!(app.options.is_none());
    if root.exists() {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn files_page_shows_the_fixed_folder_only_in_fixed_mode_and_the_window_fits_it() {
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1440.0, 1000.0);
    app.options = Some(OptionsDraft::new(&app.session, Page::Files));
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    for name in [
        "Fixed song folder",
        "Choose fixed song folder",
        "Go to sample libraries",
    ] {
        assert!(
            !tree
                .nodes
                .iter()
                .any(|(_, node)| node.label() == Some(name)),
            "{name} is shown in Last used mode"
        );
    }
    // The window ends below its page instead of keeping a fixed tall body.
    let window = tree
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role() == egui::accesskit::Role::Dialog && node.label() == Some("OPTIONS")
        })
        .unwrap()
        .1
        .bounds()
        .unwrap();
    let apply = node(&output, "Apply").bounds().unwrap();
    assert!(window.y1 - window.y0 < 400.0, "{window:?}");
    assert!(window.y1 - apply.y1 < 40.0, "{window:?} {apply:?}");
    app.options.as_mut().unwrap().values.song_folder_mode =
        mixjam_techno::application::options::SongFolderMode::Fixed;
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    node(&output, "Fixed song folder");
    node(&output, "Choose fixed song folder");
    if root.exists() {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn options_section_titles_are_uppercase_and_clear_the_bay_top() {
    for (page, title) in [
        (Page::Audio, "PLAYBACK QUALITY"),
        (Page::Workspace, "WORKSPACE"),
        (Page::Files, "SONG FOLDERS"),
        (Page::Libraries, "SAMPLE LIBRARIES"),
    ] {
        let (ctx, mut app, root) = fixture();
        let size = Vec2::new(960.0, 720.0);
        app.options = Some(OptionsDraft::new(&app.session, page));
        let mut output = frame(&ctx, &mut app, size, vec![]);
        for _ in 0..3 {
            output = frame(&ctx, &mut app, size, vec![]);
        }
        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
        let heading = tree
            .nodes
            .iter()
            .find(|(_, node)| node.value() == Some(title) || node.label() == Some(title))
            .unwrap_or_else(|| panic!("Missing section title {title}"))
            .1
            .bounds()
            .unwrap();
        // The page selector starts at the top of the bay.
        let top = node(&output, "Audio").bounds().unwrap().y0;
        assert!(heading.y0 >= top + 3.0, "{title}: {heading:?} {top}");
        if root.exists() {
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn long_library_paths_wrap_in_full_without_a_hover() {
    let (ctx, mut app, root) = fixture();
    let folder = root
        .join("a library folder with a deliberately long name for wrapping checks")
        .join("another nested folder whose name keeps the path wider than the options page");
    std::fs::create_dir_all(&folder).unwrap();
    Pcm {
        sample_rate: 44100,
        channels: 1,
        samples: vec![0.25; 441],
    }
    .write_wav(&folder.join("sound.wav"))
    .unwrap();
    app.try_scan_library_folders(vec![folder.clone()], 140.0)
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app.session.wave_scan().is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll_application(&ctx);
        std::thread::yield_now();
    }
    app.poll_application(&ctx);
    app.open_sample_libraries(&ctx);
    let size = Vec2::new(960.0, 720.0);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let path = display_paths(&app.session.library().wav_roots()[0].to_string_lossy());
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    let label = tree
        .nodes
        .iter()
        .find(|(_, node)| node.value() == Some(path.as_str()))
        .expect("The full library path is shown")
        .1
        .bounds()
        .unwrap();
    // More than one line, inside the window.
    assert!(label.y1 - label.y0 > 24.0, "{label:?}");
    assert!(label.x1 <= f64::from(size.x), "{label:?}");
    std::fs::remove_dir_all(root).unwrap();
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

/// Whether the frame paints `text`.
fn shows(output: &egui::FullOutput, text: &str) -> bool {
    output.shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::Text(painted) => painted.galley.job.text == text,
        _ => false,
    })
}

/// Choose `value` in the combo box `name`, as a person does: open it, then
/// click the item that it adds.
fn choose(ctx: &egui::Context, app: &mut MixJamApp, name: &str, value: &str) {
    let size = Vec2::new(1024.0, 768.0);
    let before = frame(ctx, app, size, vec![]);
    let known: Vec<_> = before
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .map(|(id, _)| *id)
        .collect();
    click(ctx, app, name);
    let output = frame(ctx, app, size, vec![]);
    let rect = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(id, node)| node.label() == Some(value) && !known.contains(id))
        .unwrap_or_else(|| panic!("{name} offers no {value}"))
        .1
        .bounds()
        .unwrap();
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

#[test]
fn sample_playback_quality_applies_restores_cancels_and_flags_reduced_playback() {
    use mixjam_techno::application::options::SampleQuality;
    const REDUCED: &str = "Playback quality reduced; exports use full quality";
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    app.open_options(&ctx);
    let output = frame(&ctx, &mut app, size, vec![]);
    assert_eq!(
        node(&output, "Sample playback quality").value(),
        Some("High")
    );
    assert_eq!(
        app.session.options().sample_playback_quality,
        SampleQuality::High
    );
    choose(&ctx, &mut app, "Sample playback quality", "Low");
    let draft = |app: &MixJamApp| app.options.as_ref().unwrap().values.sample_playback_quality;
    assert_eq!(draft(&app), SampleQuality::Low);
    // Only Apply changes the active preference.
    assert_eq!(
        app.session.options().sample_playback_quality,
        SampleQuality::High
    );
    let output = frame(&ctx, &mut app, size, vec![]);
    assert_eq!(
        node(&output, "Sample playback quality").value(),
        Some("Low")
    );
    assert!(shows(&output, REDUCED));
    click(&ctx, &mut app, "Apply");
    assert_eq!(
        app.session.options().sample_playback_quality,
        SampleQuality::Low
    );
    assert!(app.session.options().quality_reduced());
    let saved = std::fs::read_to_string(root.join("options.json")).unwrap();
    assert!(
        saved.contains("\"samplePlaybackQuality\": \"low\""),
        "{saved}"
    );
    // Restore changes the draft back to High; Cancel then discards it.
    click(&ctx, &mut app, "Restore options defaults");
    assert_eq!(draft(&app), SampleQuality::High);
    click(&ctx, &mut app, "Cancel");
    // The restored draft is an unapplied change, so Cancel asks first.
    click(&ctx, &mut app, "Discard changes");
    assert!(app.options.is_none());
    assert_eq!(
        app.session.options().sample_playback_quality,
        SampleQuality::Low
    );
    // The main window's status flags the reduced playback too.
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(shows(&output, REDUCED));
    // Restore and Apply return to High, and the flag goes.
    app.open_options(&ctx);
    click(&ctx, &mut app, "Restore options defaults");
    click(&ctx, &mut app, "Apply");
    assert_eq!(
        app.session.options().sample_playback_quality,
        SampleQuality::High
    );
    assert!(!app.session.options().quality_reduced());
    click(&ctx, &mut app, "Cancel");
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(!shows(&output, REDUCED));
    assert!(!app.session.has_history_step(false));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn theme_applies_at_once_restores_and_cancels_without_touching_the_song() {
    use mixjam_techno::application::options::Theme;
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    let project = app.session.project().clone();
    app.open_options(&ctx);
    click(&ctx, &mut app, "Workspace");
    let output = frame(&ctx, &mut app, size, vec![]);
    assert_eq!(node(&output, "Theme").value(), Some("Studio"));
    assert_eq!(theme::current(), Theme::Studio);
    choose(&ctx, &mut app, "Theme", "Aero");
    // A draft does not repaint the application.
    frame(&ctx, &mut app, size, vec![]);
    assert_eq!(theme::current(), Theme::Studio);
    assert_eq!(
        ctx.style().visuals.panel_fill,
        theme::of(Theme::Studio).canvas
    );
    click(&ctx, &mut app, "Apply");
    frame(&ctx, &mut app, size, vec![]);
    assert_eq!(app.session.options().theme, Theme::Aero);
    assert_eq!(theme::current(), Theme::Aero);
    assert_eq!(
        ctx.style().visuals.panel_fill,
        theme::of(Theme::Aero).canvas
    );
    let saved = std::fs::read_to_string(root.join("options.json")).unwrap();
    assert!(saved.contains("\"theme\": \"aero\""), "{saved}");
    // Restore puts Studio in the draft; Cancel discards it and Aero stays.
    click(&ctx, &mut app, "Restore options defaults");
    assert_eq!(app.options.as_ref().unwrap().values.theme, Theme::Studio);
    click(&ctx, &mut app, "Cancel");
    click(&ctx, &mut app, "Discard changes");
    frame(&ctx, &mut app, size, vec![]);
    assert!(app.options.is_none());
    assert_eq!(theme::current(), Theme::Aero);
    assert_eq!(app.session.project(), &project);
    assert!(!app.session.unsaved());
    assert!(!app.session.has_history_step(false));
    theme::paint_as(Theme::Studio);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn lane_height_applies_at_once_without_touching_the_song() {
    use mixjam_techno::application::options::LaneHeight;
    let (ctx, mut app, root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    let project = app.session.project().clone();
    app.open_options(&ctx);
    click(&ctx, &mut app, "Workspace");
    let output = frame(&ctx, &mut app, size, vec![]);
    assert_eq!(app.score.row_height, 30.0);
    assert_eq!(node(&output, "Lane height").value(), Some("30 px"));
    for (choice, height, saved_as) in [
        ("20 px", LaneHeight::Px20, "20px"),
        ("40 px", LaneHeight::Px40, "40px"),
    ] {
        choose(&ctx, &mut app, "Lane height", choice);
        // A draft does not resize the rows.
        frame(&ctx, &mut app, size, vec![]);
        assert_eq!(app.score.row_height, 30.0);
        click(&ctx, &mut app, "Apply");
        frame(&ctx, &mut app, size, vec![]);
        assert_eq!(app.session.options().lane_height, height);
        assert_eq!(app.score.row_height, height.points());
        let saved = std::fs::read_to_string(root.join("options.json")).unwrap();
        assert!(
            saved.contains(&format!("\"laneHeight\": \"{saved_as}\"")),
            "{saved}"
        );
        // Put the draft's starting point back at 30 px for the next choice.
        choose(&ctx, &mut app, "Lane height", "30 px");
        click(&ctx, &mut app, "Apply");
        frame(&ctx, &mut app, size, vec![]);
        assert_eq!(app.score.row_height, 30.0);
    }
    assert_eq!(app.session.project(), &project);
    assert!(!app.session.unsaved());
    assert!(!app.session.has_history_step(false));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn audio_device_details_stay_closed_until_opened() {
    let (ctx, mut app, _root) = fixture();
    let size = Vec2::new(1024.0, 768.0);
    app.open_options(&ctx);
    let status = app.session.buffer_status();
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(has_text(&output, "Device details"));
    assert!(
        !has_text(&output, &status),
        "the diagnostics show before Device details opens"
    );
    click(&ctx, &mut app, "Device details");
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(
        has_text(&output, &status),
        "Device details does not show the diagnostics"
    );
    // Opening the disclosure is not an option change.
    assert!(
        app.options
            .as_ref()
            .is_some_and(|draft| draft.values == *app.session.options())
    );
}
