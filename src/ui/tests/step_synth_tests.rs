//! Step synth board: pattern editing, compact layout, source readiness and options.

use super::*;

fn step_synth_board_rect(output: &egui::FullOutput) -> Rect {
    let bounds = step_synth_board_node(output).1.bounds().unwrap();
    Rect::from_min_max(
        Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
        Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
    )
}

/// The rail's Step synth button, which opens and folds the board.
fn step_synth_rail_button(output: &egui::FullOutput) -> egui::accesskit::NodeId {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role() == egui::accesskit::Role::Button && node.label() == Some("Step synth")
        })
        .map(|(id, _)| *id)
        .expect("The rail must have Step synth.")
}

fn step_synth_text_mesh_bounds(output: &egui::FullOutput, label: &str) -> Rect {
    // Glyphs that the board clips, whether or not they are scrolled into view.
    let window = step_synth_board_rect(output);
    output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Text(shape) if shape.galley.job.text == label => {
                let bounds = shape.galley.mesh_bounds.translate(shape.pos.to_vec2());
                window.contains_rect(clipped.clip_rect).then_some(bounds)
            }
            _ => None,
        })
        .reduce(|bounds, next| bounds.union(next))
        .unwrap_or_else(|| panic!("Missing Step synth glyphs: {label}"))
}

#[test]
fn step_synth_keyboard_selection_edits_flags_and_velocity_in_the_value_lanes() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::StepSynth;
    app.step_synth.state.steps[0] = Step {
        velocity: 40,
        ..Step::note(0)
    };
    app.step_synth.state.steps[1] = Step {
        velocity: 70,
        ..Step::note(7)
    };
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Step ruler");
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowRight, egui::Modifiers::SHIFT)],
        egui::Modifiers::SHIFT,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.step_synth.grid.selection, Some((0, 1)));
    for label in ["Accent lane", "Slide lane"] {
        focus_control(&ctx, &mut app, &output, label);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    assert!(
        app.step_synth.state.steps[..2]
            .iter()
            .all(|step| step.accent && step.slide)
    );
    focus_control(&ctx, &mut app, &output, "Velocity lane");
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowDown, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.step_synth.state.steps[0].velocity, 39);
    assert_eq!(app.step_synth.state.steps[1].velocity, 69);
}

#[test]
fn step_synth_tab_leaves_the_grid_without_stopping_on_its_cells() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::StepSynth;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Step 1, +0 semitones");
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for _ in 0..4 {
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let update = output.platform_output.accesskit_update.as_ref().unwrap();
        assert!(
            !update.nodes.iter().any(|(id, node)| {
                *id == update.focus
                    && node.label().is_some_and(|label| {
                        label.starts_with("Step ") && label.ends_with("semitones")
                    })
            }),
            "The grid's cells must not become individual Tab stops."
        );
        if update.focus == accessible_node(&output, "Accent lane").0 {
            return;
        }
    }
    panic!("Tab must leave the pitch grid and its scroll bar for the Accent lane.");
}

#[test]
fn step_synth_pointer_note_keeps_arrow_and_enter_navigation_in_the_grid() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::StepSynth;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    click(
        &ctx,
        &mut app,
        step_synth_control_rect(&output, "Step 1, +12 semitones").center(),
        egui::Modifiers::NONE,
    );
    assert_eq!(app.step_synth.state.steps[0], Step::note(12));
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for key in [egui::Key::ArrowRight, egui::Key::Enter] {
        frame(
            &ctx,
            &mut app,
            vec![key_event(key, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    assert_eq!(
        app.step_synth.state.steps[1],
        Step::note(12),
        "A pointer edit must leave focus on the grid for keyboard edits."
    );
}

#[test]
fn step_synth_edits_one_note_per_step_and_clears_only_the_viewed_bar() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| {
        style.animation_time = 0.0;
        style.scroll_animation = egui::style::ScrollAnimation::none();
    });
    app.selection.sample = Some(1);
    app.boards.module = BottomModule::StepSynth;
    app.step_synth.state.steps[1] = Step::note(-36);
    app.step_synth.state.steps[16] = Step::note(-48);
    app.step_synth.state.steps[48] = Step::note(48);
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = activate_control(&ctx, &mut app, &output, "Step 1, +12 semitones");
    assert_eq!(
        app.step_synth.state.steps[0],
        Step::note(12),
        "A new pitch must replace the existing note."
    );
    let output = activate_control(&ctx, &mut app, &output, "Step 1, +12 semitones");
    assert_eq!(
        app.step_synth.state.steps[0],
        Step::default(),
        "Activating the selected note must clear it."
    );
    let output = activate_control(&ctx, &mut app, &output, "Step 1, +11 semitones");
    let output = activate_control(&ctx, &mut app, &output, "View bar 2");
    assert_eq!(app.step_synth.bar, 1);
    let output = activate_control(&ctx, &mut app, &output, "Step 21, +12 semitones");
    assert_eq!(app.step_synth.state.steps[20], Step::note(12));
    let before_clear = app.step_synth.state.clone();
    let output = activate_control(&ctx, &mut app, &output, "Clear bar");
    assert!(
        app.step_synth.state.steps[16..32]
            .iter()
            .all(|step| *step == Step::default())
    );
    assert_eq!(&app.step_synth.state.steps[..16], &before_clear.steps[..16]);
    assert_eq!(&app.step_synth.state.steps[32..], &before_clear.steps[32..]);
    let output = activate_control(&ctx, &mut app, &output, "View bar 4");
    assert_eq!(
        app.step_synth.state.steps[48],
        Step::note(48),
        "Viewing a bar must retain loaded pitches outside the visible grid."
    );
    activate_control(&ctx, &mut app, &output, "View bar 1");
    assert_eq!(app.step_synth.state.steps[0], Step::note(11));
    assert_eq!(app.step_synth.state.steps[1], Step::note(-36));
    assert_eq!(
        app.step_synth.state.length, before_clear.length,
        "Viewing a bar must not change the pattern length."
    );
}

#[test]
fn step_synth_compact_layout_preserves_the_draft_and_keeps_both_panes_reachable() {
    for _ in themes_in_turn() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        ctx.style_mut(|style| {
            style.animation_time = 0.0;
            style.scroll_animation = egui::style::ScrollAnimation::none();
        });
        for _ in 0..2 {
            frame_at_size(
                &ctx,
                &mut app,
                vec![],
                egui::Modifiers::NONE,
                Vec2::new(960.0, 720.0),
            );
        }
        app.selection.sample = Some(1);
        app.boards.module = BottomModule::StepSynth;
        app.step_synth.state.steps[63] = Step::note(-48);
        app.step_synth.state.effects.echo = Some(Default::default());
        let before = app.step_synth.state.clone();
        let size = Vec2::new(960.0, 720.0);
        let viewport = Rect::from_min_size(Pos2::ZERO, size);
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let window = step_synth_board_rect(&output);
        assert_folded_assets(&ctx, &output, "Olive");
        assert!(viewport.contains_rect(window));
        // The status line sits in the top row, above the board.
        let footer = step_synth_control_rect(&output, "Step synth status");
        assert!(
            viewport.contains_rect(footer) && footer.bottom() <= window.top(),
            "Status {footer:?} must sit in the top row above {window:?}"
        );
        assert_eq!(
            accessible_node(&output, "Step synth source sound")
                .1
                .value(),
            Some("good grasp 2")
        );
        for label in [
            "Choose source file",
            "Use palette sound",
            "View bar 1",
            "View bar 4",
            "Length",
            "Gate",
            "Loop source",
            "Step synth BPM",
            "Clear bar",
            "Listen sequence",
            "Save sequence",
            "Sequence effects",
            "Save pattern",
            "Load pattern",
        ] {
            let bounds = step_synth_control_rect(&output, label);
            assert!(
                bounds.left() >= window.left() && bounds.top() >= window.top(),
                "{label} must start on the board."
            );
            assert!(
                !bounds.intersects(footer),
                "{label} must not overlap the status footer."
            );
        }
        for label in [
            "Choose source file",
            "Use palette sound",
            "Clear bar",
            "Listen sequence",
            "Save sequence",
            "Sequence effects",
            "Save pattern",
            "Load pattern",
        ] {
            // A short lower panel scrolls the settings column: the wheel
            // over the control's column brings it in.
            let mut output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            for _ in 0..12 {
                let board = step_synth_board_rect(&output);
                let control = step_synth_control_rect(&output, label);
                if board.contains_rect(control) {
                    break;
                }
                let down = control.bottom() > board.bottom();
                frame_at_size(
                    &ctx,
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(Pos2::new(control.center().x, board.center().y)),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: Vec2::new(0.0, if down { -80.0 } else { 80.0 }),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    egui::Modifiers::NONE,
                    size,
                );
                output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let glyphs = step_synth_text_mesh_bounds(&output, label);
            let button = step_synth_control_rect(&output, label);
            assert!(
                step_synth_board_rect(&output).contains_rect(button),
                "{label} must scroll into view."
            );
            assert!(
                button.shrink(2.0).contains_rect(glyphs),
                "{label} must have clear space inside its button."
            );
        }
        let caption = step_synth_text_mesh_bounds(&output, "Length");
        assert!(
            step_synth_control_rect(&output, "Length").left() - caption.right() >= 4.0,
            "The Length caption needs clear space before its well."
        );
        let caption = step_synth_text_mesh_bounds(&output, "Gate");
        assert!(
            caption.bottom() <= step_synth_control_rect(&output, "Gate").top(),
            "A dial's name sits above it."
        );
        assert!(
            step_synth_text_mesh_bounds(&output, "Length").left()
                - step_synth_control_rect(&output, "View bar 4").right()
                >= 12.0,
            "Length needs clear space after the View bar buttons."
        );
        let bottom_note = "Step 16, -12 semitones";
        frame_at_size(
            &ctx,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Focus,
                    target: step_synth_accessible_node(&output, bottom_note).0,
                    data: None,
                },
            )],
            egui::Modifiers::NONE,
            size,
        );
        for _ in 0..4 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let cell = step_synth_control_rect(&output, bottom_note);
        assert!(
            step_synth_board_rect(&output).contains_rect(cell),
            "Keyboard focus must scroll the lowest pitch into view."
        );
        assert!(!cell.intersects(step_synth_control_rect(&output, "Load pattern")));
        let output = activate_control(&ctx, &mut app, &output, "Sequence effects");
        assert!(app.step_synth.effects_expanded);
        accessible_node(&output, "Note grid");
        text_rect(&output, "Sequence effects");
        text_rect(&output, "Echo");
        let echo = step_synth_control_rect(&output, "Echo");
        assert!(
            output.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Circle(circle) if echo.contains(circle.center)
                    && circle.fill == controls::Palette::Olive.accent()
            )),
            "Enabled sequence effects must retain the board's olive accent."
        );
        let row_y = step_synth_control_rect(&output, "Cutoff").center().y;
        for name in ["Cutoff", "Resonance", "Overdrive", "Filter volume"] {
            let dial = step_synth_control_rect(&output, name);
            let value = step_synth_control_rect(&output, &format!("{name} value"));
            assert!(
                (dial.center().y - row_y).abs() < 0.1,
                "Shared effect dials must align in a row."
            );
            assert!(
                value.top() > dial.bottom(),
                "Numeric value must sit below its dial."
            );
            assert!(
                (value.center().x - dial.center().x).abs() < 2.0,
                "Numeric value must align below {name}."
            );
        }
        // The rail folds the lower panel; the board's controls hide and
        // come back with the chosen pane and the draft.
        let rail = step_synth_rail_button(&output);
        frame_at_size(
            &ctx,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Click,
                    target: rail,
                    data: None,
                },
            )],
            egui::Modifiers::NONE,
            size,
        );
        let mut output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        assert!(
            !app.session.layout().boards,
            "A second rail press folds the board."
        );
        // Folded, the lower panel goes with its top row and status line.
        for hidden in [
            "Step synth status",
            "Step synth source sound",
            "Note grid",
            "Load pattern",
        ] {
            assert!(
                !output
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some(hidden)),
                "{hidden} must hide when folded."
            );
        }
        let rail = step_synth_rail_button(&output);
        frame_at_size(
            &ctx,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Click,
                    target: rail,
                    data: None,
                },
            )],
            egui::Modifiers::NONE,
            size,
        );
        for _ in 0..3 {
            output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        assert!(app.board_shown(BottomModule::StepSynth));
        assert!(
            app.step_synth.effects_expanded,
            "Folding must preserve the chosen pane."
        );
        assert_eq!(
            app.step_synth.state, before,
            "Pane changes, scrolling, and folding must preserve the draft."
        );
        let output = activate_control(&ctx, &mut app, &output, "Note grid");
        assert!(!app.step_synth.effects_expanded);
        accessible_node(&output, "Step 16, -12 semitones");
        activate_control(&ctx, &mut app, &output, "Step synth help");
        assert!(app.help.step_synth);
        assert_eq!(app.step_synth.state, before);
    }
}

#[test]
fn step_synth_source_readiness_disables_audio_actions_but_keeps_draft_and_recovery_available() {
    for (available, missing, explicit_file) in [
        (false, false, false),
        (false, true, false),
        (true, false, false),
        (true, true, true),
    ] {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        ctx.enable_accesskit();
        let mut state = StepSynthState::default();
        let (mut view_bar, mut effects) = (0, false);
        let mut render = |events| {
            let mut action = step_synth::Action::None;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let source = || step_synth::Source {
                            name: "Source",
                            available,
                            missing,
                            explicit_file,
                        };
                        // The status line and Import details sit in the
                        // top row above the board.
                        let bar = ui
                            .horizontal(|ui| {
                                step_synth::toolbar(
                                    ui,
                                    800.0,
                                    source(),
                                    step_synth::Feedback {
                                        message: None,
                                        error: false,
                                    },
                                    effects,
                                )
                            })
                            .inner;
                        action = step_synth::show(
                            ui,
                            source(),
                            &mut state,
                            &mut view_bar,
                            &mut effects,
                            &mut Default::default(),
                            None,
                        );
                        if matches!(bar, step_synth::Action::Details) {
                            action = bar;
                        }
                    });
                },
            );
            (output, action)
        };
        for _ in 0..3 {
            render(vec![]);
        }
        let (mut output, _) = render(vec![]);
        let ready = available && (!missing || explicit_file);
        for label in ["Listen sequence", "Save sequence"] {
            assert_eq!(
                !accessible_node(&output, label).1.is_disabled(),
                ready,
                "{label}: available={available}, missing={missing}, file={explicit_file}"
            );
        }
        for label in [
            "Choose source file",
            "Use palette sound",
            "Save pattern",
            "Load pattern",
            "Length",
            "Gate",
            "Loop source",
            "Step synth BPM",
            "Step 1, +12 semitones",
        ] {
            assert!(
                !accessible_node(&output, label).1.is_disabled(),
                "{label} must remain available for editing or recovery."
            );
        }
        let mut actions = vec![
            ("Choose source file", step_synth::Action::ChooseSource),
            ("Use palette sound", step_synth::Action::PaletteSource),
            ("Save pattern", step_synth::Action::SavePattern),
            ("Load pattern", step_synth::Action::LoadPattern),
        ];
        if missing && !explicit_file {
            assert_full_recovery_caption(&output);
            actions.push(("Import details", step_synth::Action::Details));
        }
        if ready {
            actions.push(("Listen sequence", step_synth::Action::Listen));
            actions.push(("Save sequence", step_synth::Action::SaveSequence));
        }
        for (label, expected) in actions {
            let target = accessible_node(&output, label).0;
            render(vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Focus,
                    target,
                    data: None,
                },
            )]);
            let (_, action) = render(vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)]);
            assert_eq!(
                std::mem::discriminant(&action),
                std::mem::discriminant(&expected),
                "{label} must return its Session action without performing audio or file work in the view."
            );
            (output, _) = render(vec![]);
        }
    }
}

#[test]
fn step_synth_options_apply_to_the_draft_without_starting_transport() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.selection.sample = Some(1);
    app.boards.module = BottomModule::StepSynth;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    app.session.take_notice();
    let output = activate_control(&ctx, &mut app, &output, "Loop source");
    assert!(app.step_synth.state.loop_source);
    for (name, key, check) in [
        ("Length", egui::Key::End, 64.0),
        ("Gate", egui::Key::Home, 5.0),
        ("Step synth BPM", egui::Key::End, 180.0),
    ] {
        focus_control(&ctx, &mut app, &output, name);
        frame(
            &ctx,
            &mut app,
            vec![key_event(key, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let state = &app.step_synth.state;
        let value = match name {
            "Length" => f64::from(state.length),
            "Gate" => f64::from(state.gate),
            _ => state.bpm,
        };
        assert_eq!(value, check, "{name}");
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for label in ["Listen sequence", "Save sequence"] {
        assert!(!accessible_node(&output, label).1.is_disabled());
    }
    assert_eq!(
        app.step_synth.bar, 0,
        "The pattern length and the viewed bar must stay independent."
    );
    assert!(app.step_synth.state.validate().is_ok());
    // Without an audio device, a Play request leaves this notice.
    assert!(
        !app.session
            .take_notice()
            .is_some_and(|(message, _)| message.contains("Audio output is unavailable")),
        "Step synth options must not start the transport."
    );
}
