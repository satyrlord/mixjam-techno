//! The grid, the step ruler, and the value lanes from the keyboard and the
//! pointer.

use super::*;

#[test]
fn step_synth_keyboard_selection_edits_flags_and_velocity_in_the_value_lanes() {
    let (ctx, mut app) = board_fixture();
    app.step_synth.state.steps[0] = Step {
        velocity: 40,
        ..Step::note(0)
    };
    app.step_synth.state.steps[1] = Step {
        velocity: 70,
        ..Step::note(7)
    };
    let output = settle(&ctx, &mut app, 4);
    focus_control(&ctx, &mut app, &output, "Step ruler");
    settle(&ctx, &mut app, 1);
    press(
        &ctx,
        &mut app,
        egui::Key::ArrowRight,
        egui::Modifiers::SHIFT,
    );
    let output = settle(&ctx, &mut app, 1);
    assert_eq!(app.step_synth.grid.selection, Some((0, 1)));
    for label in ["Accent lane", "Slide lane"] {
        focus_control(&ctx, &mut app, &output, label);
        settle(&ctx, &mut app, 1);
        press(&ctx, &mut app, egui::Key::Enter, egui::Modifiers::NONE);
        settle(&ctx, &mut app, 1);
    }
    assert!(
        app.step_synth.state.steps[..2]
            .iter()
            .all(|step| step.accent && step.slide)
    );
    focus_control(&ctx, &mut app, &output, "Velocity lane");
    settle(&ctx, &mut app, 1);
    press(&ctx, &mut app, egui::Key::ArrowDown, egui::Modifiers::NONE);
    settle(&ctx, &mut app, 1);
    assert_eq!(app.step_synth.state.steps[0].velocity, 39);
    assert_eq!(app.step_synth.state.steps[1].velocity, 69);
}

#[test]
fn step_synth_tab_leaves_the_grid_without_stopping_on_its_cells() {
    let (ctx, mut app) = board_fixture();
    let output = settle(&ctx, &mut app, 4);
    focus_control(&ctx, &mut app, &output, "Step 1, +0 semitones");
    settle(&ctx, &mut app, 1);
    for _ in 0..4 {
        press(&ctx, &mut app, egui::Key::Tab, egui::Modifiers::NONE);
        let output = settle(&ctx, &mut app, 1);
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
    let (ctx, mut app) = board_fixture();
    let output = settle(&ctx, &mut app, 4);
    click(
        &ctx,
        &mut app,
        step_synth_control_rect(&output, "Step 1, +12 semitones").center(),
        egui::Modifiers::NONE,
    );
    assert_eq!(app.step_synth.state.steps[0], Step::note(12));
    settle(&ctx, &mut app, 1);
    for key in [egui::Key::ArrowRight, egui::Key::Enter] {
        press(&ctx, &mut app, key, egui::Modifiers::NONE);
        settle(&ctx, &mut app, 1);
    }
    assert_eq!(
        app.step_synth.state.steps[1],
        Step::note(12),
        "A pointer edit must leave focus on the grid for keyboard edits."
    );
}

#[test]
fn step_synth_edits_one_note_per_step_and_clears_only_the_viewed_bar() {
    let (ctx, mut app) = board_fixture();
    no_animation(&ctx);
    app.selection.sample = Some(1);
    // Notes above and below the rows; their middle keeps the octave at 0.
    app.step_synth.state.steps.fill(Step::default());
    app.step_synth.state.steps[1] = Step::note(-36);
    app.step_synth.state.steps[2] = Step::note(36);
    app.step_synth.state.steps[16] = Step::note(-48);
    app.step_synth.state.steps[48] = Step::note(48);
    let output = settle(&ctx, &mut app, 4);
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
fn step_synth_grid_opens_on_the_notes() {
    let (ctx, mut app) = board_fixture();
    no_animation(&ctx);
    app.selection.sample = Some(1);
    app.step_synth.state.steps.fill(Step::default());
    for (step, note) in [(0, -26), (4, -22), (8, -19)] {
        app.step_synth.state.steps[step] = Step::note(note);
    }
    let output = settle(&ctx, &mut app, 4);
    assert_eq!(
        app.step_synth.grid.octave, -2,
        "The grid opens on the octave nearest the notes."
    );
    let board = step_synth_board_rect(&output);
    for label in ["Step 1, -26 semitones", "Step 9, -19 semitones"] {
        assert!(
            board.contains_rect(step_synth_control_rect(&output, label)),
            "{label} must be in view when the board opens."
        );
    }
}

/// Each drag runs again with the primary button, which must edit: that
/// shows the drag is on the control.
#[test]
fn step_synth_secondary_drags_edit_nothing() {
    let (ctx, mut app) = board_fixture();
    app.step_synth.state.steps.fill(Step::default());
    app.step_synth.state.steps[0] = Step::note(12);
    app.step_synth.state.length = 8;
    let before = app.step_synth.state.clone();
    let output = settle(&ctx, &mut app, 4);
    let rect = |label: &str| step_synth_control_rect(&output, label);
    let note = rect("Step 1, +12 semitones").center();
    let sixth = rect("Step 6, +12 semitones").center();
    let ruler = rect("Step ruler").center().y;
    // The end marker sits on the boundary after the last step.
    let end = (rect("Step 8, +12 semitones").right() + rect("Step 9, +12 semitones").left()) / 2.0;
    let length = rect("Length").center();
    let accent = Pos2::new(note.x, rect("Accent lane").center().y);
    let velocity = Pos2::new(note.x, rect("Velocity lane").center().y);
    let drags = [
        ("a note's tie", note, sixth),
        (
            "the step ruler",
            Pos2::new(note.x, ruler),
            Pos2::new(sixth.x, ruler),
        ),
        (
            "the end marker",
            Pos2::new(end, ruler),
            Pos2::new(sixth.x, ruler),
        ),
        ("the Length well", length, length + Vec2::new(30.0, 0.0)),
        ("the Accent lane", accent, accent),
        (
            "the Velocity lane",
            velocity,
            velocity - Vec2::new(0.0, 6.0),
        ),
    ];
    let mut edited = Vec::new();
    for mode in [0, 1, 2, 3] {
        for (name, from, to) in drags {
            match mode {
                0 => secondary_drag(&ctx, &mut app, from, to),
                1 => {
                    button_at(&ctx, &mut app, from, egui::PointerButton::Secondary, true);
                    frame(
                        &ctx,
                        &mut app,
                        vec![egui::Event::PointerMoved(to)],
                        egui::Modifiers::NONE,
                    );
                    pointer(&ctx, &mut app, to, true, egui::Modifiers::NONE);
                    pointer(&ctx, &mut app, to, false, egui::Modifiers::NONE);
                    button_at(&ctx, &mut app, to, egui::PointerButton::Secondary, false);
                }
                2 => secondary_then_primary_drag(&ctx, &mut app, from, to),
                _ => drag(&ctx, &mut app, from, to, egui::Modifiers::NONE),
            }
            let changed = app.step_synth.state != before || app.step_synth.grid.selection.is_some();
            if mode == 3 {
                assert!(changed, "The primary button must edit {name}.");
            } else if changed {
                edited.push((mode, name));
            }
            app.step_synth.state = before.clone();
            app.step_synth.grid.selection = None;
            settle(&ctx, &mut app, 1);
        }
        assert!(edited.is_empty(), "A secondary drag edited {edited:?}.");
    }
}

#[test]
fn step_synth_edits_continue_after_an_unrelated_release() {
    for control in ["tie", "ruler", "length", "end marker", "accent", "velocity"] {
        let (ctx, mut app) = board_fixture();
        app.step_synth.state.steps.fill(Step::default());
        app.step_synth.state.steps[0] = Step::note(12);
        if matches!(control, "accent" | "velocity") {
            app.step_synth.state.steps[1..6].fill(Step::note(12));
        }
        app.step_synth.state.length = 8;
        let output = settle(&ctx, &mut app, 4);
        let at = |step: usize| {
            step_synth_control_rect(&output, &format!("Step {step}, +12 semitones")).center()
        };
        let ruler = step_synth_control_rect(&output, "Step ruler").center().y;
        let length = step_synth_control_rect(&output, "Length").center();
        let accent = step_synth_control_rect(&output, "Accent lane").center().y;
        let velocity = step_synth_control_rect(&output, "Velocity lane").center().y;
        let (from, middle, to) = match control {
            "tie" => (at(1), at(3), at(6)),
            "ruler" => (
                Pos2::new(at(1).x, ruler),
                Pos2::new(at(3).x, ruler),
                Pos2::new(at(6).x, ruler),
            ),
            "length" => (
                length,
                length + Vec2::new(12.0, 0.0),
                length + Vec2::new(30.0, 0.0),
            ),
            "accent" => (
                Pos2::new(at(1).x, accent),
                Pos2::new(at(3).x, accent),
                Pos2::new(at(6).x, accent),
            ),
            "velocity" => (
                Pos2::new(at(1).x, velocity),
                Pos2::new(at(3).x, velocity),
                Pos2::new(at(6).x, velocity + 4.0),
            ),
            _ => (
                Pos2::new((at(8).x + at(9).x) / 2.0, ruler),
                Pos2::new((at(9).x + at(10).x) / 2.0, ruler),
                Pos2::new((at(11).x + at(12).x) / 2.0, ruler),
            ),
        };
        let none = egui::Modifiers::NONE;
        pointer(&ctx, &mut app, from, true, none);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(middle)],
            none,
        );
        let middle_length = app.step_synth.state.length;
        for pressed in [true, false] {
            button_at(
                &ctx,
                &mut app,
                middle,
                egui::PointerButton::Secondary,
                pressed,
            );
        }
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(to)], none);
        if control == "ruler" {
            button_at(&ctx, &mut app, to, egui::PointerButton::Secondary, true);
        }
        pointer(&ctx, &mut app, to, false, none);
        if control == "ruler" {
            button_at(&ctx, &mut app, to, egui::PointerButton::Secondary, false);
        }
        match control {
            "tie" => assert_eq!(app.step_synth.state.steps[5].trig, Trig::Tie),
            "ruler" => assert_eq!(app.step_synth.grid.selection, Some((0, 5))),
            "accent" => assert!(
                app.step_synth.state.steps[..6]
                    .iter()
                    .all(|step| step.accent)
            ),
            "velocity" => assert!(
                app.step_synth.state.steps[5].velocity < app.step_synth.state.steps[2].velocity
            ),
            _ => assert!(app.step_synth.state.length > middle_length, "{control}"),
        }
    }
}
