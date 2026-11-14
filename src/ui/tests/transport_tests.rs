//! Top bar and transport: section toggles and names, transport buttons, song name,
//! position readout, tempo and the score toolbar.

use super::*;

#[test]
fn transport_section_toggle_fits_beside_the_rail_at_supported_widths() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        for _ in 0..2 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        // The rail with names is on the left; the transport has an
        // 18-point inner margin.
        let content_left = rail_width(true) + 18.0;
        let content_right = size.x - 18.0;
        for name in ["Project BPM", "Zoom", "Follow", "Sections"] {
            let rect = effects_control_rect(&output, name);
            assert!(
                rect.left() >= content_left && rect.right() <= content_right,
                "{name} must fit beside the rail at {size:?}: {rect:?}, \
                 {content_left}..{content_right}"
            );
        }
    }
}

#[test]
fn sections_after_the_audio_remain_reachable_on_the_timeline() {
    let (ctx, mut app) = fixture();
    let start_tick = 128 * TICKS_PER_BAR;
    let end_tick = app.session.project().end_tick();
    app.session
        .edit(EditCommand::SetSections(vec![Section {
            start_tick,
            name: "Later".into(),
        }]))
        .unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.session.cursor_tick(), 0);
    assert_eq!(app.session.project().end_tick(), end_tick);
    assert!(app.score.rect.unwrap().width() > 128.0 * app.score.bar_width);
}

#[test]
fn section_drafts_end_when_the_bookmarks_or_project_change() {
    let (ctx, mut app) = fixture();
    let root =
        std::env::temp_dir().join(format!("mixjam-section-options-ui-{}", std::process::id()));
    app.session = app.session.with_settings_root(root.clone());
    app.sections.bookmarks = true;
    app.add_section(0);
    app.sections.menu = (Some(0), 0);
    app.clear_selection();
    assert!(app.sections.rename.is_none());
    assert_eq!(app.sections.menu, (None, 0));

    app.add_section(0);
    app.history_step(false);
    assert!(app.sections.rename.is_none());

    app.add_section(0);
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    app.sections.rename.as_mut().unwrap().name = "Unfinished".into();
    app.session.remember_section_bookmarks(false).unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.sections.rename.is_none());
    app.session.remember_section_bookmarks(true).unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.session.project().sections[0].name, "Section 1");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn section_replacement_does_not_commit_an_earlier_name_draft() {
    let (ctx, mut app, port) = agent_fixture();
    app.sections.bookmarks = true;
    app.add_section(0);
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    app.sections.rename.as_mut().unwrap().name = "Old draft".into();
    let reply = agent(
        &ctx,
        &mut app,
        &port,
        &[
            "set_sections",
            r#"sections=[{"tick":0,"name":"Replacement"}]"#,
        ],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().sections[0].name, "Replacement");
    assert!(app.sections.rename.is_none());
}

#[test]
fn a_new_section_opens_its_name_for_editing() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.sections.bookmarks = true;
    let key = |key: egui::Key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    app.add_section(4 * TICKS_PER_BAR);
    app.add_section(0);
    let names: Vec<_> = app
        .session
        .project()
        .sections
        .iter()
        .map(|section| (section.start_tick, section.name.as_str()))
        .collect();
    assert_eq!(names, [(0, "Section 2"), (4 * TICKS_PER_BAR, "Section 1")]);
    // The section added last is the one being named.
    assert_eq!(app.sections.rename.as_ref().unwrap().start_tick, 0);
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    accessible_node(&output, "Section name");
    app.sections.rename.as_mut().unwrap().name = "Intro ".into();
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Enter)],
        egui::Modifiers::NONE,
    );
    assert!(app.sections.rename.is_none());
    assert_eq!(app.session.project().sections[0].name, "Intro");
    // Escape keeps the earlier name, and a section that exists is renamed, not added.
    app.add_section(4 * TICKS_PER_BAR);
    assert_eq!(app.session.project().sections.len(), 2);
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    app.sections.rename.as_mut().unwrap().name = "Drift".into();
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Escape)],
        egui::Modifiers::NONE,
    );
    assert!(app.sections.rename.is_none());
    assert_eq!(app.session.project().sections[1].name, "Section 1");
}

#[test]
fn transport_controls_do_not_move_when_position_digits_change() {
    let (ctx, mut app) = fixture();
    let size = Vec2::new(960.0, 720.0);
    let mut positions = None;
    for bar in [1, 21, 111, 888, 999, 1000, 4096] {
        app.session
            .seek((bar - 1) * TICKS_PER_BAR + 3 * TICKS_PER_BAR / 4);
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let current = ["BPM", "Zoom", "Follow"].map(|label| text_rect(&output, label));
        if let Some(expected) = positions {
            assert_eq!(current, expected);
        }
        positions = Some(current);
        assert!(
            current[2].right() <= size.x - 18.0,
            "Follow must fit in the score toolbar."
        );
        let tick = (bar - 1) * TICKS_PER_BAR + 3 * TICKS_PER_BAR / 4;
        let seconds =
            (tick as f64 * 240.0 / app.session.project().bpm / TICKS_PER_BAR as f64) as u32;
        let label = format!("{:02} : {:02}", seconds / 60, seconds % 60);
        // The position readout sits in the 64-point top bar, before the
        // tempo.
        let time = text_rect(&output, &label);
        assert!(
            time.bottom() <= 64.0 && time.right() < current[0].left(),
            "{label}: {time:?}"
        );
    }
}

#[test]
fn score_toolbar_controls_fit_with_the_sounds_panel_open() {
    each_theme(|theme| {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        for track in 2..5 {
            app.session.place(1, track, 0).unwrap();
        }
        app.open_sounds_panel();
        for size in [
            Vec2::new(960.0, 720.0),
            Vec2::new(1024.0, 768.0),
            Vec2::new(1440.0, 1000.0),
        ] {
            for _ in 0..3 {
                frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let panel =
                egui::containers::panel::PanelState::load(&ctx, egui::Id::new("sounds-panel"))
                    .unwrap()
                    .rect;
            let controls = [
                "Automation lane",
                "Follow",
                "Sections",
                "Zoom",
                "Maximise the score",
            ]
            .map(|name| effects_control_rect(&output, name));
            for rect in controls {
                assert!(
                    rect.left() >= panel.right() + 18.0
                        && rect.right() <= size.x - inspector::INSPECTOR_STRIP_WIDTH - 18.0 + 0.5,
                    "{theme:?}, {size:?}: {rect:?} must fit beside the Sounds panel."
                );
                assert!(
                    rect.bottom() <= app.score.rect.unwrap().top(),
                    "The toolbar must stay above the lanes."
                );
            }
            for pair in controls.windows(2) {
                assert!(pair[0].right() <= pair[1].left(), "{pair:?}");
            }
            let lanes = app.score.drop_rect.unwrap();
            assert!(lanes.height() >= 150.0, "{theme:?}, {size:?}: {lanes:?}");
        }
    });
}

/// Replace the fixture's song with one of the given name.
fn retitle(app: &mut MixJamApp, title: &str) {
    let mut project = app.session.project().clone();
    project.title = title.into();
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
}

/// The song name's painted text: its bounds and galley.
fn song_name_text(output: &egui::FullOutput, title: &str) -> (Rect, Arc<egui::Galley>) {
    let text = title_text(title);
    // The top bar's; the Inspector's song page shows it too.
    let mut names = output.shapes.iter().filter_map(|shape| match &shape.shape {
        egui::Shape::Text(shape)
            if shape.galley.job.text == text && shape.pos.y < TOP_BAR_HEIGHT =>
        {
            Some((
                Rect::from_min_size(shape.pos, shape.galley.size()),
                shape.galley.clone(),
            ))
        }
        _ => None,
    });
    let name = names
        .next()
        .unwrap_or_else(|| panic!("Missing song name: {text}"));
    assert!(
        names.next().is_none(),
        "The song name is painted once: {text}"
    );
    name
}

#[test]
fn the_song_name_stays_clear_of_the_transport_at_every_width() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    for title in ["All 64 tracks", "Night Shift", "Fades and automation"] {
        for width in (960..=1440).step_by(16) {
            let size = Vec2::new(width as f32, 768.0);
            retitle(&mut app, title);
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let (name, _) = song_name_text(&output, title);
            let first = effects_control_rect(&output, TRANSPORT_BUTTONS[0]);
            assert!(
                name.right() <= first.left() - 4.0,
                "{title} at {width}: {name:?} runs into {first:?}"
            );
        }
    }
}

#[test]
fn song_name_and_position_readout_share_the_top_bar() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let long = "Long song name ".repeat(6);
    for size in [Vec2::new(960.0, 720.0), Vec2::new(1440.0, 1000.0)] {
        for title in ["Song", long.trim_end()] {
            retitle(&mut app, title);
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let context = format!("{size:?} {title}");
            let (name, _) = song_name_text(&output, title);
            let time = accessible_label(&output, "00 : 00");
            // One 64-point bar holds the brand, the name and the transport.
            assert!(name.bottom() <= 64.0 && time.bottom() <= 64.0, "{context}");
            let buttons = TRANSPORT_BUTTONS.map(|button| effects_control_rect(&output, button));
            assert!(
                name.left() > text_rect(&output, "TECHNO").right()
                    && name.right() <= buttons[0].left() - 4.0,
                "The song name follows the brand and leads the transport: {context}"
            );
            // Record, its track, the readout, the tempo, then the actions.
            let track = effects_control_rect(&output, "Recording track");
            let bpm = effects_control_rect(&output, "Project BPM");
            let undo = effects_control_rect(&output, "Undo");
            assert!(
                buttons[7].right() <= track.left()
                    && track.right() <= time.left()
                    && time.right() <= bpm.left()
                    && bpm.right() <= undo.left(),
                "{context}: {track:?} {time:?} {bpm:?} {undo:?}"
            );
            let help = effects_control_rect(&output, "Help");
            assert!(help.right() <= size.x - 18.0 + 0.5, "{context}: {help:?}");
        }
    }
}

#[test]
fn position_readout_keeps_its_place_in_the_top_bar_and_shows_bar_and_beat_when_wide() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    for width in [960.0, 1024.0, 1280.0, 1440.0, 1920.0] {
        let size = Vec2::new(width, 900.0);
        // The top bar narrows its controls below 1240 points of content.
        let compact = width - 36.0 < 1240.0;
        for title in ["Song", "Reality (Vocal Guide, Proposals)"] {
            retitle(&mut app, title);
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let readout = accessible_label(&output, "00 : 00");
            let expected = if compact {
                Vec2::new(76.0, 40.0)
            } else {
                Vec2::new(112.0, 42.0)
            };
            assert_eq!(readout.size(), expected, "{width}");
            assert!((readout.center().y - 32.0).abs() <= 0.5, "{readout:?}");
            let digits = text_rect(&output, "00 : 00");
            assert!(
                (digits.center().x - readout.center().x).abs() <= 0.5,
                "{digits:?}"
            );
            let beside = output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Text(text) => text.galley.job.text == "1 . 1",
                _ => false,
            });
            assert_eq!(beside, !compact, "{width}: bar and beat beside the time");
        }
    }
}

#[test]
fn song_name_is_never_clipped_and_every_transport_button_fits() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let very_long = "A very long working title for the second take of the dawn mix with \
                     extra strings and a choir";
    let absurd = "Absurd ".repeat(40);
    let titles = [
        "Song",
        "Reality (Vocal Guide, Proposals)",
        very_long,
        absurd.trim_end(),
    ];
    for size in [Vec2::new(960.0, 720.0), Vec2::new(1440.0, 1000.0)] {
        let narrowest = size.x == 960.0;
        let right = size.x - 18.0;
        for title in titles {
            retitle(&mut app, title);
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            // The name and the transport follow the brand, inside the top
            // bar's 18-point margins.
            let left = text_rect(&output, "TECHNO").right();
            let (name, galley) = song_name_text(&output, title);
            let font = galley.job.sections[0].format.font_id.size;
            let context = format!("{size:?} {title}: {name:?} at {font} points");
            assert!(!galley.elided, "{context}");
            // The whole name stays in the 48-point row inside the top bar.
            assert!(name.left() >= left - 0.5, "{context}");
            assert!(
                name.top() >= 8.0 - 0.5 && name.bottom() <= 56.0 + 0.5,
                "{context}"
            );
            let buttons = TRANSPORT_BUTTONS.map(|button| effects_control_rect(&output, button));
            assert!(name.right() <= buttons[0].left() - 4.0, "{context}");
            for pair in buttons.windows(2) {
                assert!(pair[0].right() <= pair[1].left(), "{context}");
            }
            // Buttons narrow only in a compact window, and only for a name
            // that would otherwise lose its full size.
            let side = buttons[0].width();
            for button in buttons {
                assert_eq!(button.size(), Vec2::splat(side), "{context}");
                assert!(
                    button.left() >= left && button.right() <= right,
                    "{context}"
                );
            }
            // One top bar holds the brand and the transport, so the
            // narrowest window narrows the buttons for any name.
            if !narrowest {
                assert_eq!(side, 42.0, "{context}");
            } else {
                assert!(side >= 34.0, "{context}");
            }
            let bpm = effects_control_rect(&output, "Project BPM");
            assert!(buttons[7].right() <= bpm.left(), "{context}");
            for control in ["Follow", "Sections", "Maximise the score", "Help"] {
                let control = effects_control_rect(&output, control);
                assert!(control.right() <= right + 0.5, "{context}: {control:?}");
            }
            // Real names keep at least the 9-point floor; a short name and
            // the owner's song keep the full 15-point size where they fit.
            if title != absurd.trim_end() {
                assert!(font >= 9.0, "{context}");
            }
            if title == "Song" || (!narrowest && title.len() < 40) {
                assert_eq!((font, galley.rows.len()), (15.0, 1), "{context}");
            }
            assert!(galley.rows.len() <= 2 || font <= 9.0, "{context}");
        }
    }
}

#[test]
fn song_name_has_no_tooltip_and_its_mark_does_not_move_the_transport() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
    let size = Vec2::new(960.0, 720.0);
    let title = "Reality (Vocal Guide, Proposals)";
    retitle(&mut app, title);
    // A tooltip adds text to the frame.
    let texts = |output: &egui::FullOutput| {
        output
            .shapes
            .iter()
            .filter(|shape| matches!(shape.shape, egui::Shape::Text(_)))
            .count()
    };
    let hover = |app: &mut MixJamApp, at: Pos2| {
        frame_at_size(
            &ctx,
            app,
            vec![egui::Event::PointerMoved(at)],
            egui::Modifiers::NONE,
            size,
        );
        frame_at_size(&ctx, app, vec![], egui::Modifiers::NONE, size)
    };
    let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    let (name, _) = song_name_text(&output, title);
    let resting = texts(&output);
    // The readout's tooltip shows that hovering opens tooltips here.
    let readout = accessible_label(&output, "00 : 00");
    let hovered = hover(&mut app, readout.center());
    assert!(texts(&hovered) > resting);
    assert!(hovered.shapes.iter().any(|shape| matches!(
        &shape.shape,
        egui::Shape::Text(text) if text.galley.job.text.contains("minutes : seconds")
    )));
    let output = hover(&mut app, name.center());
    assert_eq!(
        texts(&output),
        resting,
        "The whole name shows; it needs no tooltip."
    );
    song_name_text(&output, title);

    // The slot keeps room for the unsaved mark: the first edit moves nothing.
    let controls = |output: &egui::FullOutput| {
        TRANSPORT_BUTTONS
            .map(|button| effects_control_rect(output, button))
            .into_iter()
            .chain(["Project BPM", "Follow", "Sections"].map(|c| effects_control_rect(output, c)))
            .collect::<Vec<_>>()
    };
    let saved = controls(&output);
    app.session.edit(EditCommand::SetTempo(160.0)).unwrap();
    assert!(app.session.unsaved());
    let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    let (marked, galley) = song_name_text(&output, &format!("{title} *"));
    assert!(!galley.elided);
    assert!(marked.right() <= saved[0].left() - 4.0, "{marked:?}");
    assert_eq!(controls(&output), saved);
}

#[test]
fn project_tempo_slider_drag_is_one_undo_step() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    // Simulated frames can take longer than the user's gesture under load.
    app.session.hold_gestures();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let bounds = accessible_node(&output, "Project BPM").1.bounds().unwrap();
    let notch = |index: usize| {
        let (left, right) = (bounds.x0 as f32 + 8.0, bounds.x1 as f32 - 32.0);
        Pos2::new(
            left + (right - left) * index as f32 / 6.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        )
    };
    assert_eq!(app.session.project().bpm, 140.0);
    assert!(!app.session.unsaved());
    pointer(&ctx, &mut app, notch(4), true, egui::Modifiers::NONE);
    for (index, expected) in [(3, 135.0), (2, 130.0), (1, 125.0)] {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(notch(index))],
            egui::Modifiers::NONE,
        );
        assert_eq!(app.session.project().bpm, expected);
    }
    pointer(&ctx, &mut app, notch(1), false, egui::Modifiers::NONE);
    assert!(app.session.unsaved());
    assert!(app.session.history_step(false).unwrap());
    assert_eq!(
        app.session.project().bpm,
        140.0,
        "Undo must restore the tempo before the complete drag."
    );
    assert!(!app.session.history_step(false).unwrap());
    assert!(app.session.history_step(true).unwrap());
    assert_eq!(app.session.project().bpm, 125.0);
}

#[test]
fn native_tempo_slider_and_pan_controls_accept_user_input() {
    let ctx = egui::Context::default();
    configure_style(&ctx);
    let bpm = std::cell::Cell::new(140.0);
    let mut pan = 0.0;
    let mut draw = |events: Vec<egui::Event>| {
        let mut tempo_rect = Rect::NOTHING;
        let mut pan_rect = Rect::NOTHING;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut value = bpm.get();
                    tempo_rect = bpm_picker(ui, "Fixture tempo", &mut value, 150.0).rect;
                    bpm.set(value);
                    pan_rect = pan_control(ui, "Track 1", &mut pan, false).rect;
                });
            },
        );
        (output, tempo_rect, pan_rect)
    };
    let (_, tempo, _) = draw(vec![]);
    let mouse = |pos, pressed| {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    // Seven even notches between the cap's end positions; the value
    // reads in the last 24 points.
    let notch = |index: usize| {
        let (left, right) = (tempo.left() + 8.0, tempo.right() - 32.0);
        Pos2::new(left + (right - left) * index as f32 / 6.0, tempo.center().y)
    };
    for (index, expected) in [(3, 135.0), (1, 125.0), (6, 180.0)] {
        draw(mouse(notch(index), true));
        draw(mouse(notch(index), false));
        assert_eq!(bpm.get(), expected, "notch {index}");
    }
    // Dragging snaps to the nearest notch; the arrow keys then step.
    draw(mouse(notch(0), true));
    draw(vec![egui::Event::PointerMoved(
        notch(2) + Vec2::new(4.0, 0.0),
    )]);
    draw(mouse(notch(2) + Vec2::new(4.0, 0.0), false));
    assert_eq!(bpm.get(), 130.0);
    let key = |key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    draw(vec![key(egui::Key::ArrowRight)]);
    assert_eq!(bpm.get(), 135.0);
    draw(vec![key(egui::Key::End)]);
    assert_eq!(bpm.get(), 180.0);
    draw(vec![key(egui::Key::ArrowLeft)]);
    assert_eq!(bpm.get(), 160.0);
    let (_, _, pan_rect) = draw(vec![]);
    draw(mouse(
        Pos2::new(pan_rect.right() - 2.0, pan_rect.center().y),
        true,
    ));
    draw(mouse(
        Pos2::new(pan_rect.right() - 2.0, pan_rect.center().y),
        false,
    ));
    assert_eq!(bpm.get(), 160.0);
    assert!(pan > 0.95 && pan <= 1.0);
}

#[test]
fn song_length_metadata_does_not_move_the_transport_controls() {
    each_theme(|theme| {
        for width in [960.0, 1440.0] {
            for title in ["A", "Song"] {
                let (ctx, mut app) = fixture();
                ctx.enable_accesskit();
                let mut project = app.session.project().clone();
                project.title = title.into();
                project.clips.clear();
                app.session = Session::headless(
                    Arc::new(Library::fixture(app.session.library().samples.to_vec())),
                    project,
                )
                .unwrap();
                app.session.set_theme_fixture(theme);
                let size = Vec2::new(width, 1000.0);
                let controls = |output: &egui::FullOutput| {
                    TRANSPORT_BUTTONS
                        .map(|button| effects_control_rect(output, button))
                        .into_iter()
                        .chain(
                            ["Recording track", "Project BPM", "Help"]
                                .map(|name| effects_control_rect(output, name)),
                        )
                        .collect::<Vec<_>>()
                };
                for _ in 0..3 {
                    frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
                }
                let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
                let before = controls(&output);
                for bar in [0, 9, 99, 999] {
                    app.session.place(1, 0, bar * TICKS_PER_BAR).unwrap();
                    let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
                    assert_eq!(
                        controls(&output),
                        before,
                        "Song length must not move transport controls: {title}, {width}, {bar}."
                    );
                }
                app.session.edit(EditCommand::SetTempo(160.0)).unwrap();
                let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
                assert_eq!(
                    controls(&output),
                    before,
                    "Tempo changes only the metadata."
                );
            }
        }
    });
}

#[test]
fn top_bar_names_every_control_inside_the_bar_at_every_supported_size() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let controls: Vec<&str> = TRANSPORT_BUTTONS
        .into_iter()
        .chain([
            "MixJam Techno tribal mask",
            RailIcon::Record.label(),
            "Recording track",
            "Project BPM",
            "Undo",
            "Redo",
            "Help",
        ])
        .collect();
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
        Vec2::new(1920.0, 1080.0),
    ] {
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let bar = Rect::from_min_max(Pos2::ZERO, Pos2::new(size.x, 64.0)).expand(0.5);
        let mut rects: Vec<(&str, Rect)> = controls
            .iter()
            .map(|name| (*name, effects_control_rect(&output, name)))
            .collect();
        for text in ["MIXJAM", "TECHNO", "00 : 00"] {
            rects.push((text, accessible_label(&output, text)));
        }
        for (name, rect) in &rects {
            assert!(bar.contains_rect(*rect), "{size:?} {name}: {rect:?}");
        }
        // Left to right, without overlap, below the brand.
        rects.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
        for pair in rects.windows(2) {
            if pair[0].0 == "MIXJAM" || pair[1].0 == "TECHNO" {
                continue;
            }
            assert!(
                pair[0].1.right() <= pair[1].1.left() + 0.5,
                "{size:?}: {} overlaps {}",
                pair[0].0,
                pair[1].0
            );
        }
    }
}

#[test]
fn score_toolbar_controls_fit_with_the_inspector_open() {
    each_theme(|theme| {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.open_sounds_panel();
        app.open_inspector();
        for size in [
            Vec2::new(960.0, 720.0),
            Vec2::new(1024.0, 768.0),
            Vec2::new(1134.0, 768.0),
            Vec2::new(1440.0, 1000.0),
        ] {
            for _ in 0..3 {
                frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let inspector =
                egui::containers::panel::PanelState::load(&ctx, egui::Id::new("inspector"))
                    .unwrap()
                    .rect;
            let controls = [
                "Arrange view",
                "Mix view",
                "Automation lane",
                "Follow",
                "Sections",
                "Zoom",
                "Maximise the score",
            ]
            .map(|name| effects_control_rect(&output, name));
            for rect in controls {
                assert!(
                    rect.right() <= inspector.left() - 18.0 + 0.5,
                    "{theme:?}, {size:?}: {rect:?} must stay inside the score."
                );
            }
            for pair in controls.windows(2) {
                assert!(pair[0].right() <= pair[1].left(), "{pair:?}");
            }
        }
    });
}
