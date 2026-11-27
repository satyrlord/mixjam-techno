//! The compact board in a short lower panel: every control stays on the
//! board and reachable, and panes, scrolling, and folding keep the draft.

use super::*;

/// Wheel the settings column until `label` is on the board, then check that
/// its `text` has clear space inside the button.
fn scroll_to_button(ctx: &egui::Context, app: &mut MixJamApp, size: Vec2, label: &str, text: &str) {
    let mut output = settle_at(ctx, app, 1, size);
    for _ in 0..12 {
        let board = step_synth_board_rect(&output);
        let control = step_synth_control_rect(&output, label);
        if board.contains_rect(control) {
            break;
        }
        let down = control.bottom() > board.bottom();
        frame_at_size(
            ctx,
            app,
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
        output = settle_at(ctx, app, 1, size);
    }
    let glyphs = step_synth_text_mesh_bounds(&output, text);
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

/// Press the rail's Step synth button, which folds or opens the board.
fn press_rail(ctx: &egui::Context, app: &mut MixJamApp, output: &egui::FullOutput, size: Vec2) {
    frame_at_size(
        ctx,
        app,
        vec![access_request(
            step_synth_rail_button(output),
            egui::accesskit::Action::Click,
        )],
        egui::Modifiers::NONE,
        size,
    );
}

#[test]
fn step_synth_loop_source_label_fits_its_clip_at_supported_sizes() {
    for theme in themes_in_turn() {
        for size in [
            Vec2::new(1440.0, 1000.0),
            Vec2::new(1024.0, 768.0),
            Vec2::new(960.0, 720.0),
        ] {
            let (ctx, mut app) = board_fixture();
            no_animation(&ctx);
            app.selection.sample = Some(1);
            let output = settle_at(&ctx, &mut app, 4, size);
            let label = screen(&output)
                .find("Loop source")
                .map(|text| {
                    let shape = text.shape();
                    (
                        shape.galley.mesh_bounds.translate(shape.pos.to_vec2()),
                        text.clip(),
                    )
                })
                .expect("The board must paint its Loop source label.");
            assert!(
                label.1.contains_rect(label.0),
                "Loop source text {:?} must fit clip {:?}: {theme:?}, {size:?}.",
                label.0,
                label.1
            );
            let toggle = step_synth_control_rect(&output, "Loop source");
            let start = step_synth_control_rect(&output, "Start value");
            assert!(
                !toggle.intersects(start),
                "Loop source must not overlap the Start value control: {theme:?}, {size:?}."
            );
        }
    }
}

#[test]
fn step_synth_compact_layout_preserves_the_draft_and_keeps_both_panes_reachable() {
    for _ in themes_in_turn() {
        let size = Vec2::new(960.0, 720.0);
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        no_animation(&ctx);
        settle_at(&ctx, &mut app, 2, size);
        app.selection.sample = Some(1);
        app.boards.module = BottomModule::StepSynth;
        app.step_synth.state.steps[63] = Step::note(-48);
        app.step_synth.state.effects.echo.enabled = true;
        let before = app.step_synth.state.clone();
        let viewport = Rect::from_min_size(Pos2::ZERO, size);
        let output = settle_at(&ctx, &mut app, 4, size);
        let window = step_synth_board_rect(&output);
        assert_folded_assets(&ctx, &output, "Olive");
        assert!(viewport.contains_rect(window));
        // The pane switch sits in the top row, above the board.
        let notes = step_synth_control_rect(&output, "Notes pane");
        assert!(
            viewport.contains_rect(notes) && notes.bottom() <= window.top(),
            "Notes pane {notes:?} must sit in the top row above {window:?}"
        );
        assert!(
            !has_control(&output, "Step synth status"),
            "The board has no status line of its own; the status bar has it."
        );
        assert_eq!(
            accessible_node(&output, "Step synth source sound")
                .1
                .value(),
            Some("good grasp 2")
        );
        for label in [
            "Source file…",
            "Palette sound",
            "View bar 1",
            "View bar 4",
            "Length",
            "Gate",
            "Loop source",
            "Step synth BPM",
            "Clear bar",
            "Listen",
            "Save sound",
            "Save pattern",
            "Load pattern",
        ] {
            let bounds = step_synth_control_rect(&output, label);
            assert!(
                bounds.left() >= window.left() && bounds.top() >= window.top(),
                "{label} must start on the board."
            );
            assert!(
                !bounds.intersects(notes),
                "{label} must not overlap the board's top row."
            );
        }
        // A short lower panel scrolls the settings column: the wheel over
        // the control's column brings it in.
        for (label, text) in [
            ("Source file…", "Source file…"),
            ("Palette sound", "Palette sound"),
            ("Clear bar", "Clear bar"),
            ("Save sound", "Save sound"),
            ("Save pattern", "Save"),
            ("Load pattern", "Load"),
        ] {
            scroll_to_button(&ctx, &mut app, size, label, text);
        }
        // A narrow grid shows Listen as its icon alone, before the bars.
        let listen = step_synth_control_rect(&output, "Listen");
        assert!(
            listen.width() <= 40.0
                && listen.right() < step_synth_control_rect(&output, "View bar 1").left(),
            "Listen {listen:?} must fold to its icon and lead the bar strip."
        );
        let caption = step_synth_text_mesh_bounds(&output, "Length");
        assert!(
            step_synth_control_rect(&output, "Length").left() - caption.right() >= 4.0,
            "The Length caption needs clear space before its well."
        );
        let caption = step_synth_text_mesh_bounds(&output, "Gate");
        assert!(
            caption.left() >= step_synth_control_rect(&output, "Gate").right()
                && caption.bottom() <= step_synth_control_rect(&output, "Gate value").top(),
            "A dial's name sits beside its knob, over its value."
        );
        // The scale group sits between the bars and the bar actions, in gaps.
        let scale = step_synth_control_rect(&output, "Scale");
        let randomise = step_synth_control_rect(&output, "Randomise");
        let bar = step_synth_control_rect(&output, "View bar 4");
        let copy = step_synth_control_rect(&output, "Copy to 2");
        assert!(
            scale.left() - bar.right() >= 12.0 && copy.left() - randomise.right() >= 12.0,
            "The scale group {scale:?}..{randomise:?} needs a gap on both sides: {bar:?}, {copy:?}."
        );
        assert!(
            randomise.width() <= 32.0,
            "A narrow strip shows Randomise as its icon."
        );
        let bottom_note = "Step 16, -12 semitones";
        frame_at_size(
            &ctx,
            &mut app,
            vec![access_request(
                step_synth_accessible_node(&output, bottom_note).0,
                egui::accesskit::Action::Focus,
            )],
            egui::Modifiers::NONE,
            size,
        );
        let output = settle_at(&ctx, &mut app, 5, size);
        let cell = step_synth_control_rect(&output, bottom_note);
        assert!(
            step_synth_board_rect(&output).contains_rect(cell),
            "Keyboard focus must scroll the lowest pitch into view."
        );
        assert!(!cell.intersects(step_synth_control_rect(&output, "Load pattern")));
        let output = activate_control(&ctx, &mut app, &output, "Effects pane");
        assert!(app.step_synth.effects_expanded);
        assert_eq!(
            accessible_node(&output, "Effects pane").1.toggled(),
            Some(egui::accesskit::Toggled::True)
        );
        assert_eq!(
            accessible_node(&output, "Notes pane").1.toggled(),
            Some(egui::accesskit::Toggled::False)
        );
        // Listen keeps its place over the effects.
        step_synth_control_rect(&output, "Listen");
        text_rect(&output, "Echo");
        let echo = step_synth_control_rect(&output, "Echo");
        assert!(
            output.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Rect(track) if echo.contains_rect(track.rect)
                    && track.fill == controls::Palette::Olive.accent()
            )),
            "Enabled pattern effects must retain the board's olive accent."
        );
        let row_y = step_synth_control_rect(&output, "Cutoff").center().y;
        for name in ["Cutoff", "Resonance", "Overdrive", "Filter volume"] {
            let dial = step_synth_control_rect(&output, name);
            let value = step_synth_control_rect(&output, &format!("{name} value"));
            assert!(
                (dial.center().y - row_y).abs() < 0.1,
                "Shared effect dials must align in a row: {name} {dial:?}, row {row_y}."
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
        press_rail(&ctx, &mut app, &output, size);
        let output = settle_at(&ctx, &mut app, 1, size);
        assert!(
            !app.session.layout().boards,
            "A second rail press folds the board."
        );
        // Folded, the lower panel goes with its top row.
        for hidden in ["Step synth source sound", "Notes pane", "Load pattern"] {
            assert!(
                !has_control(&output, hidden),
                "{hidden} must hide when folded."
            );
        }
        press_rail(&ctx, &mut app, &output, size);
        let output = settle_at(&ctx, &mut app, 3, size);
        assert!(app.board_shown(BottomModule::StepSynth));
        assert!(
            app.step_synth.effects_expanded,
            "Folding must preserve the chosen pane."
        );
        assert_eq!(
            app.step_synth.state, before,
            "Pane changes, scrolling, and folding must preserve the draft."
        );
        let output = activate_control(&ctx, &mut app, &output, "Notes pane");
        assert!(!app.step_synth.effects_expanded);
        accessible_node(&output, "Step 16, -12 semitones");
        activate_control(&ctx, &mut app, &output, "Step synth help");
        assert!(app.help.step_synth);
        assert_eq!(app.step_synth.state, before);
    }
}

#[test]
fn focusing_a_rack_control_scrolls_it_into_the_short_step_synth_pane() {
    let (ctx, mut app) = board_fixture();
    no_animation(&ctx);
    app.step_synth.effects_expanded = true;
    app.step_synth.state.effects.filter.enabled = true;
    let size = Vec2::new(960.0, 720.0);
    let mut output = settle_at(&ctx, &mut app, 4, size);
    for label in [
        "Output volume",
        "Filter",
        "Distortion shape Fuzz",
        "Cutoff value",
    ] {
        let node = step_synth_accessible_node(&output, label).0;
        frame_at_size(
            &ctx,
            &mut app,
            vec![access_request(node, egui::accesskit::Action::Focus)],
            egui::Modifiers::NONE,
            size,
        );
        output = settle_at(&ctx, &mut app, 4, size);
        let control = step_synth_control_rect(&output, label);
        let board = step_synth_board_rect(&output);
        assert!(
            board.contains_rect(control),
            "Focused {label} {control:?} must scroll inside {board:?}."
        );
    }
}

#[test]
fn voice_envelope_controls_scroll_into_view_and_stay_valid() {
    let (ctx, mut app) = board_fixture();
    no_animation(&ctx);
    let size = Vec2::new(960.0, 720.0);
    let mut output = settle_at(&ctx, &mut app, 4, size);
    for label in ["Amp decay", "Filter env", "Env decay"] {
        let node = step_synth_accessible_node(&output, label).0;
        frame_at_size(
            &ctx,
            &mut app,
            vec![access_request(node, egui::accesskit::Action::Focus)],
            egui::Modifiers::NONE,
            size,
        );
        output = settle_at(&ctx, &mut app, 4, size);
        let control = step_synth_control_rect(&output, label);
        let board = step_synth_board_rect(&output);
        assert!(
            board.contains_rect(control),
            "Focused {label} {control:?} must scroll inside {board:?}."
        );
    }
    assert!(app.step_synth.state.validate().is_ok());
}
