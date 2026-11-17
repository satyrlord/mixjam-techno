//! Mix view: lane headers widened into mixer rows, wheel input, scrolling and drafts.

use super::*;

#[test]
fn mix_view_captions_stay_whole_at_both_scrolled_edges() {
    each_theme(|_| {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        let strip = Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(250.0, 26.0));
        for (offset, expected) in [
            (0.0, Some("TRACK")),
            (15.0, Some("OUT")),
            (250.0, None),
            (mix_view::max_offset(strip.width()), Some("EQ")),
        ] {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 100.0))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        mix_view::captions(ui, strip, offset);
                    });
                },
            );
            if let Some(expected) = expected {
                text_rect(&output, expected);
            }
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    let bounds = Rect::from_min_size(text.pos, text.galley.size());
                    assert!(
                        shape.clip_rect.contains_rect(bounds),
                        "Caption {} is clipped at offset {offset}: {bounds:?}, {:?}",
                        text.galley.job.text,
                        shape.clip_rect,
                    );
                }
            }
        }
    });
}

/// The colour of `label` painted from inside `rows`.
fn text_color(output: &egui::FullOutput, label: &str, rows: Rect) -> egui::Color32 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == label && rows.contains(text.pos) => {
                Some(text.galley.job.sections[0].format.color)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing text: {label}"))
}

#[test]
fn mix_view_rows_carry_every_mixer_control_under_the_strip_names() {
    each_theme(|theme| {
        let (_ctx, app, output) = mix_view_fixture(Vec2::new(1440.0, 1000.0));
        assert!(app.mix.shown);
        assert_ne!(app.boards.module, BottomModule::Mixer, "{theme:?}");
        // M1: one row per used track, with its sounds.
        for track in 1..=2 {
            let name = format!("Track {track}");
            for suffix in [
                "gain",
                "gain value",
                "pan",
                "mute",
                "solo",
                "left output",
                "right output",
                "polarity invert",
                "EQ",
                "send FX 1",
                "send FX 2",
                "send FX 3",
                "send FX 4",
            ] {
                accessible_node(&output, &format!("{name} {suffix}"));
            }
            accessible_node(&output, &format!("Reset {name} gain to unity"));
            accessible_node(&output, &format!("Center {name} pan"));
            accessible_node(&output, &format!("Record on track {track}"));
        }
        assert!(
            output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .all(|(_, node)| node.label() != Some("Track 3 gain")),
            "An empty lane has no mixer row."
        );
        accessible_label(&output, "good grasp 2");
        // M2: the level's range is the strip's.
        let (_, gain) = accessible_node(&output, "Track 1 gain");
        assert_eq!(gain.role(), egui::accesskit::Role::Slider);
        assert_eq!(gain.max_numeric_value(), Some(12.0));
        assert!(gain.min_numeric_value().is_some_and(|db| db <= -60.0));
        // M11 to M13: Master, the returns, and Export tracks.
        for name in [
            "Master gain",
            "Master gain value",
            "Master bypass",
            "Export tracks",
        ] {
            accessible_node(&output, name);
        }
        text_rect(&output, "Stereo output");
        for bus in 1..=4 {
            for name in [
                format!("FX {bus} return level"),
                format!("FX {bus} power"),
                format!("FX {bus} limiter"),
                format!("Edit FX {bus}"),
            ] {
                accessible_node(&output, &name);
            }
        }
        text_rect(&output, "Return 100%");
        for caption in ["TRACK", "OUT", "LEVEL", "PAN", "FX 1", "FX 4", "EQ"] {
            text_rect(&output, caption);
        }
        // M15: the rows carry the strips' material and ink.
        // The rows' left parts, so the clips' names are left out.
        let left = |name: &str, up: f32| {
            let control = effects_control_rect(&output, name);
            Rect::from_min_max(
                Pos2::new(0.0, control.top() - up),
                Pos2::new(control.left(), control.bottom() + up),
            )
        };
        let row = left("Track 1 gain", 10.0);
        let master = left("Master gain", 16.0);
        assert_eq!(
            text_color(&output, "Stereo output", master),
            roles().strip_text
        );
        assert_eq!(text_color(&output, "good grasp 2", row), roles().strip_text);
    });
}

/// Whether `label` is painted in the track rows, above the Master row.
fn painted(output: &egui::FullOutput, label: &str) -> bool {
    let rows = effects_control_rect(output, "Track 1 gain").top() - 8.0
        ..effects_control_rect(output, "Master gain").top() - 8.0;
    output.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text)
            if text.galley.job.text == label && rows.contains(&text.pos.y))
    })
}

#[test]
fn mix_view_rows_at_their_defaults_stay_quiet_and_name_what_differs() {
    let (ctx, mut app, output) = mix_view_fixture(Vec2::new(1440.0, 1000.0));
    // A plain row: no reset labels and no state.
    for quiet in ["Unity", "Center"] {
        assert!(!painted(&output, quiet), "{quiet} shows on a plain row.");
    }
    // The resets stay reachable and named while hidden.
    accessible_node(&output, "Reset Track 1 gain to unity");
    accessible_node(&output, "Center Track 1 pan");
    // A moved value shows its state, not its reset; a send shows its amount.
    let mut tracks = app.session.project().tracks.clone();
    tracks[0].gain = 0.5;
    tracks[0].pan = -0.25;
    tracks[0].sends[1] = 0.35;
    let master_gain = app.session.project().master_gain;
    app.session
        .edit(EditCommand::SetMixer {
            tracks,
            master_gain,
        })
        .unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for shown in ["-6.0 dB", "25% left", "35"] {
        assert!(painted(&output, shown), "{shown} is missing.");
    }
    for quiet in ["Unity", "Center"] {
        assert!(!painted(&output, quiet), "{quiet} shows at rest.");
    }
}

#[test]
fn mix_view_names_a_track_by_its_first_sound_and_counts_the_rest() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let mut second = app.session.library().samples[0].clone();
    second.id = 2;
    second.name = "deep chord 4".into();
    let mut samples = app.session.library().samples.to_vec();
    samples.push(second);
    let mut editor = crate::engine::Editor::new(Project::default());
    // The later sound is placed first, so the order is by start, not by id.
    editor
        .place(1, 0, 8 * TICKS_PER_BAR, TICKS_PER_BAR)
        .unwrap();
    editor.place(2, 0, 0, TICKS_PER_BAR).unwrap();
    app.session =
        Session::headless(Arc::new(Library::fixture(samples)), editor.into_project()).unwrap();
    app.set_mix_view(true);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    accessible_label(&output, "deep chord 4 +1");
}

#[test]
fn the_first_opening_of_the_mix_view_names_its_shortcuts_once() {
    let (_ctx, mut app) = fixture();
    app.set_mix_view(true);
    assert_eq!(app.status, mix_view::FIRST_OPEN_HINT);
    app.set_mix_view(false);
    app.message("Other");
    app.set_mix_view(true);
    assert_eq!(app.status, "Other");
}

#[test]
fn mix_view_is_its_own_mode_with_the_whole_song_in_the_lanes() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    assert!(app.session.layout().boards);
    click_named(&ctx, &mut app, "Mix view");
    // The rows take the whole score: the lower panel folds.
    assert!(app.mix.shown && !app.session.layout().boards);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    // The lanes fit the song, so the score does not scroll sideways, and
    // Zoom rests with its name.
    let view = app.score.view.unwrap();
    assert_eq!(view.offset, 0.0);
    assert!(view.bar_width * 16.0 <= view.visible + 0.5, "{view:?}");
    let (_, zoom) = accessible_node(&output, "Zoom");
    assert!(zoom.is_disabled());
    // Leaving opens the lower panel again.
    click_named(&ctx, &mut app, "Arrange view");
    assert!(!app.mix.shown && app.session.layout().boards);
    // A panel opened or folded in the Mix view stays as it is.
    click_named(&ctx, &mut app, "Mix view");
    app.set_layout(|layout| layout.boards = true);
    app.set_layout(|layout| layout.boards = false);
    click_named(&ctx, &mut app, "Arrange view");
    assert!(!app.session.layout().boards);
}

#[test]
fn mix_view_keeps_sections_after_the_audio_visible_on_the_ruler() {
    let size = Vec2::new(960.0, 720.0);
    // Include the last valid bar: one spare bar is too narrow to hold its name.
    for start_tick in [64 * TICKS_PER_BAR, 4095 * TICKS_PER_BAR] {
        let (ctx, mut app, _) = mix_view_fixture(size);
        app.sections.bookmarks = true;
        app.session
            .edit(EditCommand::SetSections(vec![Section {
                start_tick,
                name: "Later".into(),
            }]))
            .unwrap();
        for _ in 0..3 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let ruler = app.score.ruler_rect.unwrap();
        let name = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "Later"
                {
                    let name = Rect::from_min_size(text.pos, text.galley.size());
                    assert!(
                        shape
                            .clip_rect
                            .intersect(ruler)
                            .expand(0.5)
                            .contains_rect(name),
                        "The full section name must fit at tick {start_tick}: {name:?}, {ruler:?}"
                    );
                    Some(name)
                } else {
                    None
                }
            })
            .expect("The last section paints its name.");
        let pos = name.center();
        for pressed in [true, false] {
            frame_at_size(
                &ctx,
                &mut app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                egui::Modifiers::NONE,
                size,
            );
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        assert_eq!(app.sections.menu.0, Some(start_tick));
        accessible_node(&output, "Rename section");
    }
}

#[test]
fn mix_view_long_song_ruler_numbers_have_room_between_them() {
    each_theme(|_| {
        for width in [960.0, 1024.0, 1248.0, 1280.0, 1440.0] {
            let size = Vec2::new(width, 720.0);
            let (ctx, mut app, _) = mix_view_fixture(size);
            app.session
                .edit(EditCommand::SetSections(vec![Section {
                    start_tick: 4095 * TICKS_PER_BAR,
                    name: "Later".into(),
                }]))
                .unwrap();
            for _ in 0..3 {
                frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let ruler = app.score.ruler_rect.unwrap();
            let mut numbers: Vec<Rect> = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape
                        && text.galley.job.text.chars().all(|c| c.is_ascii_digit())
                    {
                        let rect = Rect::from_min_size(text.pos, text.galley.size());
                        if rect.intersect(shape.clip_rect).intersects(ruler) {
                            return Some(rect);
                        }
                    }
                    None
                })
                .collect();
            numbers.sort_by(|a, b| a.left().total_cmp(&b.left()));
            assert!(numbers.len() >= 2, "The ruler must still show bar numbers.");
            for pair in numbers.windows(2) {
                assert!(pair[0].right() + 2.0 <= pair[1].left(), "{pair:?}");
            }
        }
    });
}

#[test]
fn mix_view_edits_levels_pan_outputs_and_polarity_with_one_undo_each() {
    let (ctx, mut app, output) = mix_view_fixture(Vec2::new(1440.0, 1000.0));
    let press = |ctx: &egui::Context, app: &mut MixJamApp, name: &str, key: egui::Key| {
        let output = frame(ctx, app, vec![], egui::Modifiers::NONE);
        focus_control(ctx, app, &output, name);
        frame(
            ctx,
            app,
            vec![key_event(key, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(ctx, app, vec![], egui::Modifiers::NONE)
    };
    // M2: exact dB entry, one undo step.
    focus_control(&ctx, &mut app, &output, "Track 1 gain value");
    frame(
        &ctx,
        &mut app,
        vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("-6.0".into()),
        ],
        egui::Modifiers::COMMAND,
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let half = 10.0_f32.powf(-6.0 / 20.0);
    assert!((app.session.project().tracks[0].gain - half).abs() < 0.000_001);
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].gain, 1.0);
    app.history_step(true);
    assert!((app.session.project().tracks[0].gain - half).abs() < 0.000_001);
    // M3: Unity.
    press(
        &ctx,
        &mut app,
        "Reset Track 1 gain to unity",
        egui::Key::Space,
    );
    assert_eq!(app.session.project().tracks[0].gain, 1.0);
    // M4: arrows move the pan, and Center returns it.
    press(&ctx, &mut app, "Track 1 pan", egui::Key::ArrowRight);
    assert!(app.session.project().tracks[0].pan > 0.0);
    press(&ctx, &mut app, "Center Track 1 pan", egui::Key::Space);
    assert_eq!(app.session.project().tracks[0].pan, 0.0);
    // M5: polarity, lit while inverted.
    let output = press(&ctx, &mut app, "Track 1 polarity invert", egui::Key::Space);
    assert!(app.session.project().tracks[0].polarity_inverted);
    assert_eq!(
        accessible_node(&output, "Track 1 polarity invert")
            .1
            .toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    // M6 and M7: the outputs and switches, and the state they leave.
    press(&ctx, &mut app, "Track 2 solo", egui::Key::Space);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    // One solo leaves the other rows dimmed, without a column of its name.
    assert!(!output.shapes.iter().any(|shape| matches!(
        &shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "Solo elsewhere"
    )));
    press(&ctx, &mut app, "Track 1 left output", egui::Key::Space);
    let output = press(&ctx, &mut app, "Track 1 right output", egui::Key::Space);
    assert!(!app.session.project().tracks[0].left_enabled);
    text_rect(&output, "No output");
    let output = press(&ctx, &mut app, "Track 1 mute", egui::Key::Space);
    assert!(app.session.project().tracks[0].muted);
    text_rect(&output, "Muted");
    app.history_step(false);
    assert!(
        !app.session.project().tracks[0].muted,
        "Each edit is one step."
    );
    assert!(!app.session.project().tracks[0].right_enabled);
    // M11: the Master level.
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Master gain");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowDown, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(app.session.project().master_gain < 1.0);
}

/// Whether the pan slider `name` has the automation ring: a thin outline
/// just outside it.
fn pan_ringed(output: &egui::FullOutput, name: &str) -> bool {
    let slider = effects_control_rect(output, name);
    output.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Rect(ring)
            if ring.stroke.width == 1.0
                && ring.rect.contains_rect(slider)
                && ring.rect.width() <= slider.width() + 6.0
                && ring.rect.height() <= slider.height() + 6.0)
    })
}

#[test]
fn mix_view_eq_uses_the_new_row_when_switching_to_an_earlier_track() {
    let (ctx, mut app, _) = mix_view_fixture(Vec2::new(1440.0, 1000.0));
    let output = click_named(&ctx, &mut app, "Track 2 EQ");
    assert_eq!(app.eq_track, Some(1));
    let target = accessible_node(&output, "Track 1 EQ").0;
    let row = effects_control_rect(&output, "Track 1 pan");
    // Check the selection frame: the shell keeps its first position, so
    // correcting the anchor in a later frame cannot repair its placement.
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target,
                data: None,
            },
        )],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.eq_track, Some(0));
    assert!((app.mix.eq_row.unwrap().center().y - row.center().y).abs() < 0.5);
}

#[test]
fn mix_view_marks_automation_and_opens_the_eq_and_the_returns() {
    let (context, mut app, port) = agent_fixture();
    context.enable_accesskit();
    app.set_mix_view(true);
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    assert!(!pan_ringed(&output, "Track 1 pan"));
    for words in [
        &["automate", "track=1", "lane=pan", "points=[[1,-1],[2,1]]"][..],
        &[
            "automate",
            "track=1",
            "lane=send1",
            "points=[[1,0],[2,0.5]]",
        ],
        &["automate", "bus=2", "lane=return", "points=[[1,0],[2,0.5]]"],
    ] {
        let reply = send(&mut app, words);
        assert_eq!(reply["ok"], true, "{words:?}: {reply}");
    }
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    // M4 and M12: the automation marks.
    text_rect(&output, "Pan automated");
    text_rect(&output, "Return automated");
    // M9: an automated send is ringed.
    let knob = effects_control_rect(&output, "Track 1 send FX 1");
    let ringed = |rect: Rect| {
        output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Circle(circle)
                if circle.center.distance(rect.center()) < 0.5
                    && (circle.radius - (rect.width() / 2.0 + 2.0)).abs() < 0.5)
        })
    };
    assert!(ringed(knob));
    assert!(!ringed(effects_control_rect(&output, "Track 1 send FX 2")));
    // U8: so is an automated pan slider, by the same shared painter.
    assert!(pan_ringed(&output, "Track 1 pan"));
    // M8: EQ is lit while the compressor is on, and opens the EQ window.
    assert_eq!(
        accessible_node(&output, "Track 1 EQ").1.toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    let mut tracks = app.session.project().tracks.clone();
    tracks[0].compressor.on = true;
    let master_gain = app.session.project().master_gain;
    app.session
        .edit(EditCommand::SetMixer {
            tracks,
            master_gain,
        })
        .unwrap();
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        accessible_node(&output, "Track 1 EQ").1.toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    send(&mut app, &["click", "target=Track 1 EQ"]);
    assert_eq!(app.eq_track, Some(0));
    app.eq_track = None;
    // M12: the returns' switches and Edit.
    let limiter = app.session.project().fx.buses[2].limiter;
    send(&mut app, &["click", "target=FX 3 limiter"]);
    assert_ne!(app.session.project().fx.buses[2].limiter, limiter);
    send(&mut app, &["set", "target=FX 3 return level", "value=0.25"]);
    assert!((app.session.project().fx.buses[2].return_level - 0.25).abs() < 0.0001);
    send(&mut app, &["click", "target=Edit FX 3"]);
    assert_eq!(app.boards.module, BottomModule::FxReturns);
    assert!(app.session.layout().boards);
    assert!(app.mix.shown, "Edit opens FX returns below the Mix view.");
    // M11: Master bypass is Bypass all.
    send(&mut app, &["click", "target=Master bypass"]);
    assert!(app.session.project().master.bypass);
    // M13: Export tracks asks for a folder.
    let reply = send(&mut app, &["click", "target=Export tracks"]);
    assert_eq!(reply["modal"], "Choose path", "{reply}");
    let state = send(&mut app, &["state"]);
    assert_eq!(state["path_prompt"]["title"], "Export tracks", "{state}");
}

#[test]
fn mix_view_wheel_moves_the_control_under_the_pointer_and_never_the_pan() {
    let (ctx, mut app, output) = mix_view_fixture(Vec2::new(1440.0, 1000.0));
    let wheel = |ctx: &egui::Context, app: &mut MixJamApp, at: Pos2, up: bool| {
        frame(
            ctx,
            app,
            vec![egui::Event::PointerMoved(at)],
            egui::Modifiers::NONE,
        );
        frame(
            ctx,
            app,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: Vec2::new(0.0, if up { 1.0 } else { -1.0 }),
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        );
        for _ in 0..10 {
            frame(ctx, app, vec![], egui::Modifiers::NONE);
        }
    };
    wheel(
        &ctx,
        &mut app,
        effects_control_rect(&output, "Track 1 gain").center(),
        true,
    );
    let db = amplitude_to_db(app.session.project().tracks[0].gain);
    assert!((db - 0.5).abs() < 0.001, "One notch is 0.5 dB, once: {db}");
    wheel(
        &ctx,
        &mut app,
        effects_control_rect(&output, "Track 1 send FX 1").center(),
        true,
    );
    assert!((app.session.project().tracks[0].sends[0] - 0.05).abs() < 0.0001);
    wheel(
        &ctx,
        &mut app,
        effects_control_rect(&output, "Track 1 pan").center(),
        false,
    );
    assert_eq!(app.session.project().tracks[0].pan, 0.0);
    wheel(
        &ctx,
        &mut app,
        effects_control_rect(&output, "FX 1 return level").center(),
        false,
    );
    assert!((app.session.project().fx.buses[0].return_level - 0.95).abs() < 0.0001);
    let before = app.session.project().master_gain;
    wheel(
        &ctx,
        &mut app,
        effects_control_rect(&output, "Master gain").center(),
        false,
    );
    assert!(app.session.project().master_gain < before);
}

#[test]
fn mix_view_keeps_master_and_returns_in_view_while_the_tracks_scroll() {
    let size = Vec2::new(1024.0, 768.0);
    let (ctx, mut app, _) = mix_view_fixture(size);
    for track in 2..24 {
        app.session.place(1, track, 0).unwrap();
    }
    for _ in 0..4 {
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    let master = effects_control_rect(&output, "Master gain");
    let returns = effects_control_rect(&output, "FX 4 return level");
    let window = Rect::from_min_size(Pos2::ZERO, size);
    assert!(window.contains_rect(master) && window.contains_rect(returns));
    let row = effects_control_rect(&output, "Track 1 gain");
    frame_at_size(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(Pos2::new(row.left() - 40.0, row.center().y)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -20_000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        egui::Modifiers::NONE,
        size,
    );
    for _ in 0..60 {
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    assert_eq!(effects_control_rect(&output, "Master gain"), master);
    assert_eq!(effects_control_rect(&output, "FX 4 return level"), returns);
    let last = effects_control_rect(&output, "Track 24 gain");
    assert!(last.bottom() <= master.top(), "{last:?} {master:?}");
}

#[test]
fn mix_view_dense_viewport_clips_at_the_last_whole_row() {
    let size = Vec2::new(960.0, 720.0);
    let (ctx, mut app, _) = mix_view_fixture(size);
    for track in 2..24 {
        app.session.place(1, track, 0).unwrap();
    }
    for _ in 0..4 {
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let rows = app.score.drop_rect.unwrap().height() / app.score.row_height;
    assert!((rows - rows.round()).abs() < 0.001, "{rows} visible rows");
}

#[test]
fn mix_view_keeps_five_track_rows_above_master_and_returns_in_a_short_window() {
    for size in [Vec2::new(960.0, 720.0), Vec2::new(1024.0, 768.0)] {
        let (ctx, mut app, _) = mix_view_fixture(size);
        for track in 2..12 {
            app.session.place(1, track, 0).unwrap();
        }
        for _ in 0..4 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        assert!(
            !app.panels.room.boards,
            "The Mix view folds the lower panel."
        );
        let master = effects_control_rect(&output, "Master gain");
        let window = Rect::from_min_size(Pos2::ZERO, size);
        assert!(window.contains_rect(effects_control_rect(&output, "FX 1 return level")));
        for track in 1..=5 {
            let row = effects_control_rect(&output, &format!("Track {track} gain"));
            assert!(
                row.bottom() <= master.top(),
                "Track {track} shows above Master at {size:?}: {row:?} {master:?}"
            );
        }
    }
}

#[test]
fn mix_view_columns_scroll_sideways_and_follow_keyboard_focus_in_a_narrow_window() {
    let size = Vec2::new(960.0, 720.0);
    let (ctx, mut app, output) = mix_view_fixture(size);
    accessible_node(&output, "Mix view columns");
    assert_eq!(app.mix.scroll, 0.0);
    let settle = |ctx: &egui::Context, app: &mut MixJamApp| {
        for _ in 0..20 {
            frame_at_size(ctx, app, vec![], egui::Modifiers::NONE, size);
        }
        frame_at_size(ctx, app, vec![], egui::Modifiers::NONE, size)
    };
    // A control reached with the keyboard scrolls into view.
    let (target, _) = accessible_node(&output, "Track 1 EQ");
    frame_at_size(
        &ctx,
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
    let output = settle(&ctx, &mut app);
    assert!(app.mix.scroll > 0.0);
    let eq = effects_control_rect(&output, "Track 1 EQ");
    let (_, node) = accessible_node(&output, "Track 1 EQ");
    let bounds = node.bounds().unwrap();
    assert!(
        (bounds.x1 as f32 - eq.right()).abs() < 0.5,
        "The focused EQ is whole in view: {bounds:?} {eq:?}"
    );
    // Shift and the wheel over a row scroll the columns back.
    let scrolled = app.mix.scroll;
    // Between the last send and EQ, over no control.
    let gap = Pos2::new(eq.left() - 3.0, eq.center().y);
    frame_at_size(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(gap),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, 400.0),
                modifiers: egui::Modifiers::SHIFT,
            },
        ],
        egui::Modifiers::SHIFT,
        size,
    );
    settle(&ctx, &mut app);
    assert!(app.mix.scroll < scrolled, "{} {scrolled}", app.mix.scroll);
}

#[test]
fn mix_view_track_rows_take_horizontal_wheel_before_the_score() {
    let size = Vec2::new(960.0, 720.0);
    let (ctx, mut app, output) = mix_view_fixture(size);
    let row = effects_control_rect(&output, "Track 1 gain");
    let at = Pos2::new(app.score.rect.unwrap().left() - 400.0, row.center().y);
    let before = app.score.view.unwrap().offset;
    frame_at_size(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -400.0),
                modifiers: egui::Modifiers::SHIFT,
            },
        ],
        egui::Modifiers::SHIFT,
        size,
    );
    for _ in 0..30 {
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    assert!(
        app.mix.scroll > 0.0,
        "The track columns must take the wheel."
    );
    assert_eq!(app.score.view.unwrap().offset, before);
    let scrolled = app.mix.scroll;
    // The same header owns the opposite direction, and the score, which
    // fits the whole song, stays put even when asked to move.
    app.overview.target = Some(32.0 * TICKS_PER_BAR as f32);
    frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    let before = app.score.view.unwrap().offset;
    assert_eq!(before, 0.0);
    frame_at_size(
        &ctx,
        &mut app,
        vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, 400.0),
            modifiers: egui::Modifiers::SHIFT,
        }],
        egui::Modifiers::SHIFT,
        size,
    );
    for _ in 0..30 {
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    assert!(app.mix.scroll < scrolled);
    assert_eq!(app.score.view.unwrap().offset, before);
}

#[test]
fn mix_view_gain_drafts_cannot_change_a_replacement_song() {
    for name in ["Master gain value", "Track 1 gain value"] {
        let (ctx, mut app, output) = mix_view_fixture(Vec2::new(1440.0, 1000.0));
        focus_control(&ctx, &mut app, &output, name);
        frame(
            &ctx,
            &mut app,
            vec![
                key_event(egui::Key::A, egui::Modifiers::COMMAND),
                egui::Event::Text("+6".into()),
            ],
            egui::Modifiers::COMMAND,
        );
        app.new_mix();
        app.session.place(1, 0, 0).unwrap();
        let replacement = app.session.project().clone();
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(app.session.project() == &replacement, "{name}");
    }
}

#[test]
fn mix_view_held_controls_cannot_change_a_replacement_song() {
    for name in ["Master gain", "Track 1 gain", "FX 1 return level"] {
        let (ctx, mut app, output) = mix_view_fixture(Vec2::new(1440.0, 1000.0));
        let start = effects_control_rect(&output, name).center();
        pointer(&ctx, &mut app, start, true, egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(start + Vec2::new(-12.0, 12.0))],
            egui::Modifiers::NONE,
        );
        app.new_mix();
        app.session.place(1, 0, 0).unwrap();
        let replacement = app.session.project().clone();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(start + Vec2::new(-30.0, 30.0))],
            egui::Modifiers::NONE,
        );
        assert!(app.session.project() == &replacement, "{name}");
    }
}

#[test]
fn an_empty_mix_view_says_no_tracks_and_keeps_master_and_returns() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let library = Arc::new(Library::fixture(app.session.library().samples.to_vec()));
    app.session = Session::headless(library, Project::default()).unwrap();
    app.set_mix_view(true);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "NO TRACKS");
    text_rect(&output, "Place a sound in the arrangement.");
    assert!(!has_control(&output, "Track 1 gain"));
    for name in ["Master gain", "FX 1 return level", "Export tracks"] {
        assert!(has_control(&output, name), "{name} stays in an empty song.");
    }
}

#[test]
fn mix_view_and_the_mixer_board_never_show_at_once() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    click_named(&ctx, &mut app, "Mix view");
    assert!(app.mix.shown);
    assert_eq!(app.boards.module, BottomModule::Mastering);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        accessible_node(&output, "Mix view").1.toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    // The rail's Mixer returns the score to Arrange and shows the board.
    click_named(&ctx, &mut app, "Mixer");
    assert!(
        !app.mix.shown && app.board_shown(BottomModule::Mixer),
        "The rail's Mixer returns the score to Arrange."
    );
    // Clicked again, it folds the lower panel and leaves the score alone.
    click_named(&ctx, &mut app, "Mixer");
    assert!(!app.mix.shown && !app.panels.room.boards);
    // The command list's Mix view toggles the score's view.
    app.run_command(commands::Command::MixView, &ctx);
    assert!(app.mix.shown);
    app.run_command(commands::Command::MixView, &ctx);
    assert!(!app.mix.shown);
    click_named(&ctx, &mut app, "Mix view");
    click_named(&ctx, &mut app, "Arrange view");
    assert!(!app.mix.shown);
}
