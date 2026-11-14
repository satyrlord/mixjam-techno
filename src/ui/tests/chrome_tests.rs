//! Window chrome: version link, status notices, uppercase titles, wrapped help text
//! and the tool status line.

use super::*;

#[test]
fn app_version_stays_at_the_bottom_right_with_long_status_and_notices() {
    let (context, mut app) = fixture();
    let mut project = app.session.project().clone();
    project.source_notices.push("Imported source notice".into());
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    app.status = "A long status message with a saved sample path. ".repeat(100);
    for (width, height) in [(1440.0, 1000.0), (1024.0, 768.0), (960.0, 720.0)] {
        let size = Vec2::new(width, height);
        frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        let bounds = |label: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        let rect = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(
                            shape.clip_rect.contains_rect(rect),
                            "Clipped {label}: {rect:?} in {:?}",
                            shape.clip_rect
                        );
                        Some(rect)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing status label: {label}"))
        };
        let version = bounds(&format!("v{}", mixjam_techno::APP_VERSION));
        // The status bar spans the window, under the rail.
        assert!((version.right() - (width - 18.0)).abs() <= 1.0);
        assert!(version.top() >= height - 30.0);
        assert!(version.bottom() <= height);
        assert!(bounds(&app.status).right() < version.left());
        assert!(bounds("Import details").right() < version.left());
    }
}

#[test]
fn status_notices_stay_clear_of_panel_switches() {
    each_theme(|theme| {
        let (context, mut app, root) = sample_conflicts_fixture("status-panel-switches");
        let (_, missing) = missing_fixture();
        let mut project = app.session.project().clone();
        project.missing_samples = missing.session.project().missing_samples.clone();
        project
            .clips
            .push(missing.session.project().clips[0].clone());
        project.clips.last_mut().unwrap().id = 99;
        let mut library = Library::fixture(vec![]);
        library.apply_imports(&project.imported_samples).unwrap();
        app.session = Session::headless(Arc::new(library), project)
            .unwrap()
            .with_settings_root(root.join("settings"));
        let mut options = app.session.options().clone();
        options.theme = theme;
        options.sample_playback_quality = mixjam_techno::application::options::SampleQuality::Low;
        app.session
            .apply_options(options, app.session.options_revision(), false)
            .unwrap();
        app.conflicts.open = false;
        app.conflicts.dismissed = true;
        for size in [
            Vec2::new(960.0, 720.0),
            Vec2::new(1024.0, 768.0),
            Vec2::new(1440.0, 1000.0),
        ] {
            frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
            let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
            let tools = effects_control_rect(&output, "Tools");
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    let rect = text.galley.rect.translate(text.pos.to_vec2());
                    if rect.top() >= size.y - 30.0
                        && text.galley.job.text != "Panels"
                        && rect.left() < tools.left()
                    {
                        assert!(
                            rect.intersect(shape.clip_rect).right() < tools.left(),
                            "Status text overlaps Tools at {size:?}: {:?} {rect:?}.",
                            text.galley.job.text
                        );
                    }
                }
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    });
}

#[test]
fn version_link_opens_the_repository_through_the_system_browser() {
    let (context, mut app) = fixture();
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let version = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.galley.job.text == format!("v{}", mixjam_techno::APP_VERSION) =>
            {
                assert!(
                    text.galley
                        .job
                        .sections
                        .iter()
                        .all(|section| section.format.color == roles().accent)
                );
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            _ => None,
        })
        .expect("The version link must be visible.");
    pointer(
        &context,
        &mut app,
        version.center(),
        true,
        egui::Modifiers::NONE,
    );
    let output = frame(
        &context,
        &mut app,
        vec![egui::Event::PointerButton {
            pos: version.center(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        egui::Modifiers::NONE,
    );
    assert!(
        output
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(
                command,
                egui::OutputCommand::OpenUrl(url)
                    if url.url == "https://github.com/satyrlord/mixjam-techno" && url.new_tab
            ))
    );
}

#[test]
fn titles_are_uppercase_without_changing_saved_text_or_body_font() {
    let (context, mut app) = fixture();
    let mut project = app.session.project().clone();
    project.title = "Știință și ritm".into();
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    app.set_layout(|layout| layout.sounds = true);
    app.options = Some(options::OptionsDraft::new(
        &app.session,
        options::Page::Libraries,
    ));
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    // The product name keeps its brand face in every theme; other titles
    // take the theme's heading face.
    for (label, family) in [
        ("MIXJAM", theme::brand_family()),
        ("TECHNO", theme::brand_family()),
        ("STIINTA SI RITM", theme::heading_family()),
        ("ARRANGEMENT", theme::heading_family()),
        ("OPTIONS", theme::heading_family()),
    ] {
        let text = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => Some(text),
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing title: {label}"));
        assert!(
            text.galley
                .job
                .sections
                .iter()
                .all(|section| section.format.font_id.family == family)
        );
    }
    let body = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == "Find a sound" => Some(text),
            _ => None,
        })
        .expect("The sound search must keep its body font.");
    assert!(
        body.galley
            .job
            .sections
            .iter()
            .all(|section| { section.format.font_id.family == egui::FontFamily::Proportional })
    );
    assert_eq!(app.session.project().title, "Știință și ritm");
    assert_eq!(app.session.library().samples[0].name, "good grasp 2");
}

#[test]
fn title_text_transliterates_accents_ligatures_and_combining_marks() {
    assert_eq!(title_text("Șș Şş Țț Ţţ Ââ Ăă Îî"), "SS SS TT TT AA AA II");
    assert_eq!(
        title_text("S\u{326}tiint\u{326}a\u{306} i\u{302}n ra\u{302}ve"),
        "STIINTA IN RAVE"
    );
    assert_eq!(
        title_text("Æther Øresund Łódź Straße"),
        "AETHER ORESUND LODZ STRASSE"
    );
    assert_eq!(title_text("Mix 01 / A-B + 2.5 *"), "MIX 01 / A-B + 2.5 *");
}

#[test]
fn help_instructions_wrap_readably_and_bottom_guidance_remains_reachable() {
    const PLACE: &str = "Drag to a track, or select a sound and click a track. Up to 64 tracks.";
    let visible = |output: &egui::FullOutput, label: &str, viewport: Rect| {
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == label => {
                let bounds = Rect::from_min_size(text.pos, text.galley.size());
                viewport.contains_rect(bounds) && shape.clip_rect.contains_rect(bounds)
            }
            _ => false,
        })
    };
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        let (context, mut app) = fixture();
        app.help.main = true;
        for _ in 0..3 {
            frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let mut output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        let viewport = Rect::from_min_size(Pos2::ZERO, size);
        let instruction = text_rect(&output, PLACE);
        const ACTIONS: [&str; 11] = [
            "Place sound",
            "Preview",
            "Move / copy",
            "Select group",
            "Fine snap",
            "Delete",
            "Seek / loop",
            "Play / pause",
            "Undo / redo",
            "Open / save",
            "Export",
        ];
        let mut seen_actions = BTreeSet::new();
        let inspect_actions = |output: &egui::FullOutput, seen: &mut BTreeSet<String>| {
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape
                    && ACTIONS.contains(&text.galley.job.text.as_str())
                {
                    let action = &text.galley.job.text;
                    assert_eq!(
                        text.galley.rows.len(),
                        1,
                        "Help action {action:?} must stay on one line at {size:?}."
                    );
                    assert!(
                        (action.chars().count() as f32 * 3.0..=160.0)
                            .contains(&text.galley.size().x),
                        "Help action {action:?} needs a readable column at {size:?}: {:?}.",
                        text.galley.size()
                    );
                    seen.insert(action.clone());
                }
            }
        };
        inspect_actions(&output, &mut seen_actions);
        assert!(
            instruction.width() > 200.0,
            "Instruction column is too narrow at {size:?}: {instruction:?}"
        );
        assert!(
            instruction.height() < 80.0,
            "Instruction wraps excessively at {size:?}: {instruction:?}"
        );
        assert!(visible(&output, PLACE, viewport));
        for label in [
            "Mixer: gain, pan, L/R outputs, Mute, Solo, and master gain. Both outputs off leaves a track with no output.",
            "Tab focuses controls. Arrow keys adjust sliders. Space toggles M, S, L, and R.",
        ] {
            // Later sections follow this guidance, so scroll toward it in
            // steps and stop when it appears.
            let mut steps = 0;
            while !visible(&output, label, viewport) && steps < 60 {
                steps += 1;
                frame_at_size(
                    &context,
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(instruction.center()),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: Vec2::new(0.0, -40.0),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    egui::Modifiers::NONE,
                    size,
                );
                for _ in 0..12 {
                    output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
                }
                inspect_actions(&output, &mut seen_actions);
            }
            assert!(
                visible(&output, label, viewport),
                "Help guidance is unreachable at {size:?}: {label}"
            );
        }
        for action in ACTIONS {
            assert!(
                seen_actions.contains(action),
                "Help action {action:?} must remain reachable at {size:?}."
            );
        }
    }
}

#[test]
fn tool_status_text_scrolls_inside_a_short_footer() {
    each_theme(|_| {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        ctx.enable_accesskit();
        ctx.style_mut(|style| style.scroll_animation = egui::style::ScrollAnimation::none());
        let rect = Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(250.0, 18.0));
        let message = "Missing audio. Choose a source file or open Import details.";
        let render = |events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 100.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        controls::status_text(
                            ui,
                            rect,
                            "short-status",
                            message,
                            12.5,
                            false,
                            message,
                        );
                    });
                },
            )
        };
        for _ in 0..3 {
            render(vec![]);
        }
        let before = render(vec![egui::Event::PointerMoved(rect.center())]);
        accessible_label(&before, message);
        let top = |output: &egui::FullOutput| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == message => Some(text.pos.y),
                    _ => None,
                })
                .unwrap()
        };
        render(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -200.0),
            modifiers: egui::Modifiers::NONE,
        }]);
        for _ in 0..30 {
            render(vec![]);
        }
        let after = render(vec![]);
        assert!(
            top(&after) < top(&before),
            "The last line must scroll into the short footer."
        );
        accessible_label(&after, message);
    });
}

#[test]
fn persisted_import_notices_are_visible_at_startup_and_keep_details() {
    let (_, app) = fixture();
    let mut project = app.session.project().clone();
    project.import_notices.push(ImportNotice {
        source_bpm: 146.0,
        target_bpm: 140.0,
    });
    let mut reopened = MixJamApp::from_session(
        Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            project,
        )
        .unwrap(),
        "Ready.".into(),
        false,
    );
    assert!(reopened.status.contains("146 BPM changed to 140 BPM"));
    reopened.message("Ready to edit.");
    assert_eq!(reopened.session.project().import_notices.len(), 1);
    reopened.set_startup_messages(&["One library folder is unavailable.".into()]);
    assert!(reopened.status_error);
    assert!(
        reopened
            .status
            .contains("One library folder is unavailable")
    );
}
