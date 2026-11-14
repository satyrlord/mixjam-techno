//! Mastering board interaction: dial and pedal drags, pedal order, and fitting the
//! board into short windows.

use super::*;

#[test]
fn mastering_dial_held_still_mid_drag_keeps_its_step() {
    let (ctx, mut app, _) = mastering_fixture(|master| master.bus_compressor.on = true);
    app.boards.master_reveal = Some(PedalKind::BusCompressor);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let window = effects_control_rect(&output, "Mastering board");
    let dial = effects_control_rect(&output, "Bus compressor threshold").center();
    assert!(window.contains(dial));
    // The threshold steps by 0.5 dB, and the dial turns 0.2 dB a point.
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(dial)],
        egui::Modifiers::NONE,
    );
    pointer(&ctx, &mut app, dial, true, egui::Modifiers::NONE);
    for step in 1..=12 {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(
                dial + Vec2::new(step as f32, 0.0),
            )],
            egui::Modifiers::NONE,
        );
    }
    let held = app.session.project().master.bus_compressor.threshold;
    assert!(held > -16.0 && (held * 2.0).fract() == 0.0, "{held}");
    for _ in 0..6 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            app.session.project().master.bus_compressor.threshold,
            held,
            "A dial held still must not flip between the steps around the pointer."
        );
    }
    pointer(
        &ctx,
        &mut app,
        dial + Vec2::new(12.0, 0.0),
        false,
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().master.bus_compressor.threshold, held);
}

#[test]
fn new_mix_cancels_a_mastering_pedal_drag_before_release() {
    for release_with_new in [false, true] {
        let (ctx, mut app, output) = mastering_fixture(|_| {});
        let grip = effects_control_rect(&output, "Drag Booster EQ").center();
        let drop = effects_control_rect(&output, "Drag Booster compressor").right_center();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(grip)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, grip, true, egui::Modifiers::NONE);
        for step in 1..=6 {
            let at = grip + (drop - grip) * (step as f32 / 6.0);
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(at)],
                egui::Modifiers::NONE,
            );
        }
        assert!(ctx.dragged_id().is_some(), "The pedal drag must be active.");
        let mut events = vec![key_event(egui::Key::N, egui::Modifiers::COMMAND)];
        if release_with_new {
            events.push(egui::Event::PointerButton {
                pos: drop,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::COMMAND,
            });
        }
        frame(&ctx, &mut app, events, egui::Modifiers::COMMAND);
        assert!(
            app.session.project().clips.is_empty(),
            "New must replace the previous song."
        );
        assert!(!app.session.unsaved());
        if !release_with_new {
            pointer(&ctx, &mut app, drop, false, egui::Modifiers::NONE);
        }
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            app.session.project().master.order,
            PedalKind::ALL,
            "Releasing the previous song's pedal must not edit the new song."
        );
        assert!(!app.session.unsaved());
        assert!(
            !app.session.history_step(false).unwrap(),
            "The new song must have no undo entry."
        );
    }
}

#[test]
fn mastering_pedal_drag_ends_with_the_window() {
    let (ctx, mut app, output) = mastering_fixture(|_| {});
    let grip = effects_control_rect(&output, "Drag Booster wide").center();
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(grip)],
        egui::Modifiers::NONE,
    );
    pointer(&ctx, &mut app, grip, true, egui::Modifiers::NONE);
    for step in 1..=6 {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(
                grip - Vec2::new(10.0 * step as f32, 0.0),
            )],
            egui::Modifiers::NONE,
        );
    }
    // The window closes mid-drag, and the button lets go elsewhere.
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let away = Pos2::new(4.0, 500.0);
    pointer(&ctx, &mut app, away, false, egui::Modifiers::NONE);
    app.boards.module = BottomModule::Mastering;
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    click(&ctx, &mut app, away, egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.session.project().master.order,
        PedalKind::ALL,
        "A later click must not drop a drag that ended."
    );
    assert!(!app.session.unsaved());
}

#[test]
fn mastering_move_buttons_follow_their_pedal_and_keep_focus_at_the_end() {
    let (ctx, mut app, output) = mastering_fixture(|_| {});
    let window = effects_control_rect(&output, "Mastering board");
    // The grips are for the pointer; the arrows move pedals from the keyboard.
    assert!(
        !accessible_node(&output, "Drag Soft clip")
            .1
            .supports_action(egui::accesskit::Action::Focus),
        "A drag grip must not take a Tab stop."
    );
    focus_control(&ctx, &mut app, &output, "Move Soft clip right");
    let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for place in 4..=11 {
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            app.session.project().master.order[place],
            PedalKind::SoftClip
        );
        let followed = if place == 11 {
            "Move Soft clip left"
        } else {
            "Move Soft clip right"
        };
        assert!(
            window.contains_rect(effects_control_rect(&output, followed)),
            "{followed} must stay in view at place {place}."
        );
    }
    let focus = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .focus;
    assert!(
        accessible_node(&output, "Move Soft clip right")
            .1
            .is_disabled()
    );
    assert_eq!(
        focus,
        accessible_node(&output, "Move Soft clip left").0,
        "Focus passes to the other arrow when the pressed one turns off."
    );
}

#[test]
fn mastering_shows_its_whole_board_from_1024_by_768() {
    for size in [Vec2::new(1024.0, 768.0), Vec2::new(1440.0, 1000.0)] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.boards.module = BottomModule::Mastering;
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let board = effects_control_rect(&output, "Mastering board");
        assert!(
            Rect::from_min_size(Pos2::ZERO, size).contains_rect(board),
            "{size:?}"
        );
        for chip in ["Show Booster EQ", "Show Limiter"] {
            assert!(
                board.contains_rect(effects_control_rect(&output, chip)),
                "{chip} at {size:?}"
            );
        }
        // The toolbar holds the preset, the board's state, and its help.
        let toolbar = effects_control_rect(&output, "Step synth tab");
        let preset = effects_control_rect(&output, "Mastering preset");
        let status = text_rect(
            &output,
            "Every pedal is off and the trim is at 0 dB: the mix plays unprocessed.",
        );
        let help = effects_control_rect(&output, "Mastering help");
        assert!(
            toolbar.right() < preset.left() && preset.right() < status.left(),
            "{size:?}"
        );
        assert!(
            status.right() <= help.left() && help.bottom() <= board.top(),
            "{size:?}"
        );
        text_rect(&output, "Choose a preset");
    }
}

#[test]
fn a_mastering_chip_brings_its_whole_pedal_into_a_short_section() {
    let size = Vec2::new(960.0, 720.0);
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    let mut output = egui::FullOutput::default();
    for _ in 0..4 {
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let board = effects_control_rect(&output, "Mastering board");
    assert!(
        !board.contains_rect(effects_control_rect(&output, "Show Limiter")),
        "The minimum window scrolls the strip below the pedals."
    );
    // The wheel scrolls the board down to the chips; choosing one scrolls
    // the board back up to its whole pedal.
    let inside = board.center();
    frame_at_size(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(inside)],
        egui::Modifiers::NONE,
        size,
    );
    for _ in 0..3 {
        output = frame_at_size(
            &ctx,
            &mut app,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -60.0),
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
            size,
        );
    }
    // Smooth scrolling spends the wheel over the following frames.
    for _ in 0..30 {
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let chip = effects_control_rect(&output, "Show Limiter");
    assert!(
        board.contains_rect(chip),
        "The wheel must reach the strip: {chip:?} in {board:?}."
    );
    for pressed in [true, false] {
        frame_at_size(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(chip.center()),
                egui::Event::PointerButton {
                    pos: chip.center(),
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
            size,
        );
    }
    for _ in 0..3 {
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let grip = effects_control_rect(&output, "Limiter always runs last");
    assert!(
        board.contains_rect(Rect::from_min_size(
            grip.min,
            Vec2::new(grip.width(), 304.0)
        )),
        "The whole Limiter must come into view: {grip:?} in {board:?}."
    );
    // Tab focus scrolls the board down to a chip; Enter on it scrolls back up.
    let before = accessible_node(&output, "Show Multiband").0;
    frame_at_size(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target: before,
                data: None,
            },
        )],
        egui::Modifiers::NONE,
        size,
    );
    frame_at_size(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        size,
    );
    for _ in 0..2 {
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    assert!(
        board.contains_rect(effects_control_rect(&output, "Show Limiter")),
        "Tab must reveal the chip."
    );
    frame_at_size(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        size,
    );
    for _ in 0..3 {
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let grip = effects_control_rect(&output, "Limiter always runs last");
    assert!(
        board.contains_rect(Rect::from_min_size(
            grip.min,
            Vec2::new(grip.width(), 304.0)
        )),
        "Enter on a chip must bring the whole pedal into view: {grip:?} in {board:?}."
    );
}
