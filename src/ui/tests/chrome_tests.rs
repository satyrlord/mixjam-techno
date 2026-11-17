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
            let text = screen(&output)
                .find(label)
                .unwrap_or_else(|| panic!("Missing status label: {label}"));
            let rect = text.bounds();
            assert!(
                text.clip().contains_rect(rect),
                "Clipped {label}: {rect:?} in {:?}",
                text.clip()
            );
            rect
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
            for text in screen(&output).texts() {
                let rect = text.bounds();
                if rect.top() >= size.y - 30.0
                    && text.label() != "PANELS"
                    && rect.left() < tools.left()
                {
                    assert!(
                        text.visible().right() < tools.left(),
                        "Status text overlaps Tools at {size:?}: {:?} {rect:?}.",
                        text.label()
                    );
                }
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    });
}

#[test]
fn an_error_keeps_its_room_beside_every_notice() {
    let (context, mut app, root) = sample_conflicts_fixture("status-error-room");
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
    options.sample_playback_quality = mixjam_techno::application::options::SampleQuality::Low;
    app.session
        .apply_options(options, app.session.options_revision(), false)
        .unwrap();
    app.conflicts.open = false;
    app.conflicts.dismissed = true;
    let message = "Cannot load the pattern broken.json. It is not a Step synth pattern                    (line 1, column 3).";
    app.error(message);
    let size = Vec2::new(960.0, 720.0);
    frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
    let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
    let view = screen(&output);
    // The notices take their shortest forms first.
    for long in ["Sample conflicts", "Import details", "Reduced quality"] {
        assert!(!view.shows(long), "{long} keeps its long form.");
    }
    let error = view
        .find(message)
        .expect("The error must show beside the notices.");
    // Then the error fills the line up to the panel switches. It loses only
    // the gap before them and the end that its ellipsis cuts. The version
    // link's width moves the switches, so the room is measured, not fixed.
    let room = view.text("PANELS").rect().left() - error.rect().left();
    let width = error.visible().width();
    assert!(
        width >= room - 20.0,
        "The error leaves room unused: {width} of {room}"
    );
    assert!(
        width >= 150.0,
        "The notices shorten before the error loses its room: {width}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn version_link_opens_the_repository_through_the_system_browser() {
    let (context, mut app) = fixture();
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let version = screen(&output)
        .find(&format!("v{}", mixjam_techno::APP_VERSION))
        .expect("The version link must be visible.");
    assert!(
        version
            .shape()
            .galley
            .job
            .sections
            .iter()
            .all(|section| section.format.color == roles().muted),
        "The version link is muted at rest; the accent marks hover and focus."
    );
    let version = version.bounds();
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
        let text = screen(&output)
            .find(label)
            .unwrap_or_else(|| panic!("Missing title: {label}"));
        assert!(
            text.shape()
                .galley
                .job
                .sections
                .iter()
                .all(|section| section.format.font_id.family == family)
        );
    }
    let body = screen(&output)
        .find("Find a sound")
        .expect("The sound search must keep its body font.");
    assert!(
        body.shape()
            .galley
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

/// The bounds of `label` drawn inside the window `identity`.
fn window_text(
    output: &egui::FullOutput,
    context: &egui::Context,
    identity: &str,
    label: &str,
) -> Option<Rect> {
    let window = dialog::window_rect(context, identity)?;
    screen(output).texts().find_map(|text| {
        let bounds = text.rect();
        (text.label() == label && window.contains_rect(bounds) && text.whole()).then_some(bounds)
    })
}

/// The number of lines `label` takes where it is drawn.
fn text_rows(output: &egui::FullOutput, label: &str) -> usize {
    screen(output).find(label).map_or(0, |text| text.rows())
}

fn release(context: &egui::Context, app: &mut MixJamApp, pos: Pos2, size: Vec2) {
    for pressed in [true, false] {
        frame_at_size(
            context,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
            size,
        );
    }
}

#[test]
fn help_lists_its_pages_and_each_page_reads_and_scrolls() {
    const LAST: &str = "Every file is as long as the mix export. The files sum to the mix with the board bypassed when that sum does not clip.";
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        let (context, mut app) = fixture();
        context.style_mut(|style| style.scroll_animation = egui::style::ScrollAnimation::none());
        app.help.main = true;
        for _ in 0..3 {
            frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        let window = dialog::window_rect(&context, "Help").unwrap();
        assert!(
            window.height() >= 420.0 && window.width() >= 700.0,
            "Help must open roomy at {size:?}: {window:?}"
        );
        for topic in help::Topic::ALL {
            assert!(
                window_text(&output, &context, "Help", topic.name()).is_some(),
                "The page list must show {:?} at {size:?}.",
                topic.name()
            );
        }
        // The shortcut sheet: one action to a line, its keys beside it.
        for (action, key) in [
            ("Play / pause", "Space"),
            ("Undo", "Ctrl+Z"),
            ("All commands", "Ctrl+K"),
        ] {
            let action_rect = window_text(&output, &context, "Help", action)
                .unwrap_or_else(|| panic!("Missing action {action} at {size:?}"));
            let key_rect = window_text(&output, &context, "Help", key)
                .unwrap_or_else(|| panic!("Missing key {key} at {size:?}"));
            assert_eq!(text_rows(&output, action), 1, "{action} wraps at {size:?}");
            assert!(
                key_rect.left() > action_rect.right()
                    && (key_rect.center().y - action_rect.center().y).abs() < 2.5,
                "{key} must sit beside {action} at {size:?}: {key_rect:?} / {action_rect:?}"
            );
        }
        // Choose a page from the list.
        let mixer = window_text(&output, &context, "Help", "Mixer").unwrap();
        release(&context, &mut app, mixer.center(), size);
        assert_eq!(app.help.topic, help::Topic::Mixer);
        let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        let title =
            window_text(&output, &context, "Help", "MIXER").expect("The page opens at its title.");
        let level = window_text(&output, &context, "Help", "LEVEL AND PAN").unwrap();
        assert!(level.top() > title.bottom());
        // End reaches the last line of the page; Home returns to its title.
        let page = Pos2::new(level.center().x, level.bottom() + 40.0);
        let key = |context: &egui::Context, app: &mut MixJamApp, key| {
            frame_at_size(
                context,
                app,
                vec![
                    egui::Event::PointerMoved(page),
                    key_event(key, egui::Modifiers::NONE),
                ],
                egui::Modifiers::NONE,
                size,
            );
            frame_at_size(context, app, vec![], egui::Modifiers::NONE, size)
        };
        let output = key(&context, &mut app, egui::Key::End);
        assert!(
            window_text(&output, &context, "Help", LAST).is_some(),
            "End must reach the end of the Mixer page at {size:?}."
        );
        let output = key(&context, &mut app, egui::Key::Home);
        assert!(window_text(&output, &context, "Help", "MIXER").is_some());
        // Page Down moves by a page, not to the end.
        let output = key(&context, &mut app, egui::Key::PageDown);
        assert!(window_text(&output, &context, "Help", "MIXER").is_none());
        // Another page opens at its top.
        app.help.topic = help::Topic::Recording;
        frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        assert!(window_text(&output, &context, "Help", "RECORDING").is_some());
    }
}

#[test]
fn help_page_list_answers_the_arrow_keys_and_wraps_in_a_narrow_window() {
    let (context, mut app) = fixture();
    app.help.main = true;
    let size = Vec2::new(1440.0, 1000.0);
    for _ in 0..3 {
        frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
    let shortcuts = window_text(&output, &context, "Help", "Shortcuts").unwrap();
    release(&context, &mut app, shortcuts.center(), size);
    assert!(
        context.memory(|memory| memory.focused()).is_some(),
        "A click on a page entry gives it the focus."
    );
    // The entry keeps the arrows from the frame after it gains the focus.
    frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
    for (key, topic) in [
        (egui::Key::ArrowDown, help::Topic::Arrange),
        (egui::Key::ArrowDown, help::Topic::Automation),
        (egui::Key::ArrowUp, help::Topic::Arrange),
    ] {
        frame_at_size(
            &context,
            &mut app,
            vec![key_event(key, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
            size,
        );
        frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        assert_eq!(app.help.topic, topic);
    }
    // A narrow Help puts its list above the page.
    dialog::forget_geometry(&context, "Help");
    let narrow = Vec2::new(520.0, 760.0);
    for _ in 0..3 {
        frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, narrow);
    }
    let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, narrow);
    let shortcuts = window_text(&output, &context, "Help", "Shortcuts").unwrap();
    let title = window_text(&output, &context, "Help", "ARRANGE").unwrap();
    assert!(
        title.top() > shortcuts.bottom(),
        "{title:?} / {shortcuts:?}"
    );
}

#[test]
fn every_help_page_has_text_and_every_tool_has_its_page() {
    for topic in help::Topic::ALL {
        let text = topic.text();
        assert!(text.len() >= 3, "{:?} needs a page", topic.name());
        for line in text {
            assert!(
                line.chars().count() <= 300,
                "{:?} has a line too long to scan: {line}",
                topic.name()
            );
            assert!(
                !line.contains("D3") && !line.contains("T3"),
                "{:?} names a product: {line}",
                topic.name()
            );
        }
    }
    // Every tool of the rail with a board or a window has a page.
    for tool in [
        "Mixer",
        "Mastering",
        "FX returns",
        "Step synth",
        "Time Stretcher",
        "Effect Studio",
        "Sample editor",
        "Album Review",
    ] {
        assert!(
            help::Topic::ALL.iter().any(|topic| topic.name() == tool),
            "{tool} needs a Help page"
        );
    }
    // A tool's own Help window takes the tool's palette.
    for topic in help::Topic::ALL {
        if topic.window().is_some() {
            assert!(topic.palette().is_some(), "{:?}", topic.name());
        }
    }
    let mastering = help::Topic::Mastering.text();
    for term in ["dBFS", "dBTP", "LUFS", "VU"] {
        assert!(mastering.contains(&term), "Mastering must explain {term}");
    }
}

/// Paint the real Help shell at a user-selected size without the workspace.
fn short_help_frame(
    context: &egui::Context,
    topic: &mut help::Topic,
    size: Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    context.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
            events,
            focused: true,
            ..Default::default()
        },
        |ctx| {
            dialog::window("Help")
                .default_width(size.x)
                .max_height(size.y)
                .scroll(false)
                .show(ctx, |ui| help::manual(ui, topic));
        },
    )
}

#[test]
fn help_keyboard_selection_stays_visible_in_a_short_list() {
    for size in [Vec2::new(720.0, 300.0), Vec2::new(300.0, 300.0)] {
        let (context, _) = fixture();
        context.style_mut(|style| style.scroll_animation = egui::style::ScrollAnimation::none());
        let mut topic = help::Topic::Shortcuts;
        for _ in 0..3 {
            short_help_frame(&context, &mut topic, size, vec![]);
        }
        context.memory_mut(|memory| {
            memory.request_focus(egui::Id::new(("help-page-entry", "Shortcuts")));
        });
        short_help_frame(&context, &mut topic, size, vec![]);
        for expected in help::Topic::ALL.into_iter().skip(1) {
            short_help_frame(
                &context,
                &mut topic,
                size,
                vec![key_event(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            );
            let mut release = key_event(egui::Key::ArrowDown, egui::Modifiers::NONE);
            if let egui::Event::Key { pressed, .. } = &mut release {
                *pressed = false;
            }
            short_help_frame(&context, &mut topic, size, vec![release]);
            let output = short_help_frame(&context, &mut topic, size, vec![]);
            assert_eq!(topic, expected);
            assert!(
                window_text(&output, &context, "Help", expected.name()).is_some(),
                "The selected topic {expected:?} must stay visible at {size:?}."
            );
        }
    }
}

#[test]
fn help_page_remains_visible_at_minimum_window_size() {
    let (context, _) = fixture();
    let mut topic = help::Topic::Shortcuts;
    let size = Vec2::new(300.0, 160.0);
    for _ in 0..3 {
        short_help_frame(&context, &mut topic, size, vec![]);
    }
    let output = short_help_frame(&context, &mut topic, size, vec![]);
    assert!(
        window_text(&output, &context, "Help", "SHORTCUTS").is_some(),
        "The compact topic list must leave room for the page."
    );
}

#[test]
fn help_topic_controls_do_not_conflict_with_workspace_controls() {
    let (context, mut app) = fixture();
    context.enable_accesskit();
    app.help.main = true;
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let nodes = agent::nodes(output.platform_output.accesskit_update.as_ref().unwrap());
    let controls = |name: &str| {
        nodes
            .iter()
            .filter(|node| {
                node.label == name && node.enabled && node.role != "label" && node.role != "window"
            })
            .count()
    };
    assert_eq!(
        controls("Mixer"),
        1,
        "The Mixer command name must stay unique."
    );
    for topic in help::Topic::ALL {
        assert_eq!(controls(&format!("Help page: {}", topic.name())), 1);
    }
}

#[test]
fn tool_help_scroll_keys_work_with_focus_in_its_header() {
    let (context, mut app) = fixture();
    context.enable_accesskit();
    context.style_mut(|style| style.scroll_animation = egui::style::ScrollAnimation::none());
    app.help.master = true;
    for _ in 0..3 {
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&context, &mut app, &output, "Close Mastering help");
    frame(
        &context,
        &mut app,
        vec![key_event(egui::Key::End, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        window_text(
            &output,
            &context,
            "Mastering help",
            "Changes apply while the song plays, and Undo restores them."
        )
        .is_some(),
        "End must reach the page end when a Help header control has focus."
    );
}

#[test]
fn f1_opens_and_closes_help() {
    let (context, mut app) = fixture();
    // A press, then its release, so the next press is not a repeat.
    let f1 = |context: &egui::Context, app: &mut MixJamApp| {
        frame(
            context,
            app,
            vec![key_event(egui::Key::F1, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let mut release = key_event(egui::Key::F1, egui::Modifiers::NONE);
        if let egui::Event::Key { pressed, .. } = &mut release {
            *pressed = false;
        }
        frame(context, app, vec![release], egui::Modifiers::NONE);
    };
    f1(&context, &mut app);
    assert!(app.help.main);
    f1(&context, &mut app);
    assert!(!app.help.main);
}

#[test]
fn tool_help_opens_beside_its_tool() {
    let (context, mut app) = fixture();
    let size = Vec2::new(1440.0, 1000.0);
    app.selection.sample = Some(app.session.library().samples[0].id);
    app.stretch.open = true;
    for _ in 0..3 {
        frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    app.help.open_tool(help::Topic::Stretch);
    for _ in 0..3 {
        frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let tool = dialog::window_rect(&context, "Time Stretcher").unwrap();
    let page = dialog::window_rect(&context, "Time Stretcher help").unwrap();
    assert!(
        !tool.shrink(1.0).intersects(page),
        "Time Stretcher help must not cover the tool: {tool:?} / {page:?}"
    );
    // A board's Help opens above the lower panel.
    app.help.open_tool(help::Topic::Mastering);
    for _ in 0..3 {
        frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let board = context
        .data(|data| data.get_temp::<Rect>(egui::Id::new("help-board-rect")))
        .unwrap();
    let page = dialog::window_rect(&context, "Mastering help").unwrap();
    assert!(page.bottom() <= board.top() + 1.0, "{page:?} / {board:?}");
}

#[test]
fn tool_help_stays_clear_of_its_tool_at_compact_sizes() {
    for size in [Vec2::new(960.0, 720.0), Vec2::new(1024.0, 768.0)] {
        let (context, mut app) = fixture();
        context.enable_accesskit();
        context.style_mut(|style| style.scroll_animation = egui::style::ScrollAnimation::none());
        app.selection.sample = Some(app.session.library().samples[0].id);
        app.stretch.open = true;
        for _ in 0..3 {
            frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        app.help.open_tool(help::Topic::Stretch);
        for _ in 0..3 {
            frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let tool = dialog::window_rect(&context, "Time Stretcher").unwrap();
        let page = dialog::window_rect(&context, "Time Stretcher help").unwrap();
        assert!(
            !tool.intersects(page),
            "Help must use the free space above the tool at {size:?}: {tool:?} / {page:?}"
        );
        assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(page));
        let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        let (target, _) = accessible_node(&output, "Close Time Stretcher help");
        frame_at_size(
            &context,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Focus,
                    target,
                    data: None,
                },
            )],
            egui::Modifiers::NONE,
            size,
        );
        frame_at_size(
            &context,
            &mut app,
            vec![key_event(egui::Key::End, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
            size,
        );
        let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        assert!(
            window_text(
                &output,
                &context,
                "Time Stretcher help",
                "Esc closes the tool. It first closes an open list or cancels a value being typed."
            )
            .is_some(),
            "The short Help page must scroll to its last paragraph at {size:?}."
        );
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
        let top = |output: &egui::FullOutput| screen(output).find(message).unwrap().rect().min.y;
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
