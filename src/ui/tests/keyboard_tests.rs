//! Keyboard focus and shortcuts: tab order, focus scrolling and shortcuts beside
//! focused controls.

use super::*;

/// The accessible name of the focused control, if any.
fn focused_label(output: &egui::FullOutput) -> Option<String> {
    let update = output.platform_output.accesskit_update.as_ref()?;
    update
        .nodes
        .iter()
        .find(|(id, _)| *id == update.focus)
        .and_then(|(_, node)| node.label().map(str::to_owned))
}

#[test]
fn tab_walks_the_rail_top_to_bottom_and_reaches_the_panel_switches_last() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let first = TRANSPORT_BUTTONS[0];
    focus_control(&ctx, &mut app, &output, first);
    let mut order = vec![first.to_owned()];
    for _ in 0..400 {
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let Some(label) = focused_label(&output) else {
            continue;
        };
        if label == first {
            break;
        }
        order.push(label);
    }
    // The top bar, left to right: the transport, the recording track, the
    // tempo, then Help (Undo and Redo are disabled in the fixture).
    let top_bar: Vec<&str> = TRANSPORT_BUTTONS
        .into_iter()
        .chain(["Recording track", "Project BPM", "Help"])
        .collect();
    assert_eq!(
        &order[..top_bar.len()],
        top_bar.as_slice(),
        "The top bar comes first: {order:?}"
    );
    let rail = [
        "Shrink the tool rail to icons",
        "New",
        "Open",
        "Save",
        "Save as",
        "Import",
        "Export",
        "Erase",
        "Delete",
        "Mixer",
        "Mastering",
        "FX returns",
        "Effects",
        "Step synth",
        "Edit",
        "Stretch",
        "Album Review",
        "Options",
        "All commands",
    ];
    let after = top_bar.len();
    assert_eq!(
        &order[after..after + rail.len()],
        rail.as_slice(),
        "The rail follows the top bar, top to bottom: {order:?}"
    );
    assert_eq!(
        order.get(after + rail.len()).map(String::as_str),
        Some("Mixer tab"),
        "The lower panel follows the rail: {order:?}"
    );
    // The song overview leads the arrangement, then the score toolbar,
    // left to right.
    let toolbar = [
        "Song overview",
        "Hide the song overview",
        "Arrange view",
        "Mix view",
        "Automation lane",
        "Follow",
        "Sections",
        "Zoom",
        "Maximise the score",
        "Arrangement lanes",
    ];
    let at = order
        .iter()
        .position(|name| name == toolbar[0])
        .unwrap_or_else(|| panic!("The score toolbar is reachable: {order:?}"));
    assert_eq!(
        &order[at..at + toolbar.len()],
        toolbar.as_slice(),
        "{order:?}"
    );
    let tools = order.iter().position(|name| name == "Tools");
    let boards = order.iter().position(|name| name == "Boards");
    let sounds = order.iter().position(|name| name == "Sounds");
    let overview = order.iter().position(|name| name == "Overview");
    let inspector = order.iter().position(|name| name == "Inspector");
    assert!(
        tools.is_some()
            && boards == tools.map(|at| at + 1)
            && sounds == tools.map(|at| at + 2)
            && overview == tools.map(|at| at + 3)
            && inspector == tools.map(|at| at + 4),
        "The panel switches come together: {order:?}"
    );
    assert!(
        inspector.is_some_and(|at| at + 2 >= order.len()),
        "Only the version link may follow the panel switches: {order:?}"
    );
}

#[test]
fn tab_scrolls_rail_controls_into_view_in_short_windows() {
    each_theme(|_| {
        for size in [Vec2::new(960.0, 720.0), Vec2::new(1024.0, 768.0)] {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            ctx.style_mut(|style| {
                style.scroll_animation = egui::style::ScrollAnimation::none();
            });
            let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            let target = accessible_node(&output, "Shrink the tool rail to icons").0;
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
            let mut reached_last = false;
            for _ in 0..18 {
                frame_at_size(
                    &ctx,
                    &mut app,
                    vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
                    egui::Modifiers::NONE,
                    size,
                );
                // Tab resolves focus after drawing. The next frame requests
                // scrolling, and the following frame paints that offset.
                frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
                let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
                let label = focused_label(&output).expect("A rail control must have focus.");
                let rect = effects_control_rect(&output, &label);
                let options = effects_control_rect(&output, "Options");
                assert!(
                    rect.top() >= 64.0 && rect.bottom() < options.top(),
                    "Focused {label} must be visible at {size:?}: {rect:?}."
                );
                if label == "Album Review" {
                    reached_last = true;
                    break;
                }
            }
            assert!(reached_last);
        }
    });
}

#[test]
fn audio_capture_shortcut_works_in_a_modal_and_does_not_repeat() {
    let (ctx, mut app) = fixture();
    app.files.pending_action = Some(PendingAction::New);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.blocking_modal(&ctx), Some("Save your changes?"));
    app.session.take_notice();
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::F12, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let (message, error) = app
        .session
        .take_notice()
        .expect("F12 must reach the session.");
    assert!(error && message.contains("Audio output is unavailable"));
    assert_eq!(app.blocking_modal(&ctx), Some("Save your changes?"));
    let mut repeat = key_event(egui::Key::F12, egui::Modifiers::NONE);
    if let egui::Event::Key { repeat, .. } = &mut repeat {
        *repeat = true;
    }
    frame(&ctx, &mut app, vec![repeat], egui::Modifiers::NONE);
    assert!(app.session.take_notice().is_none());
}

#[test]
fn focused_controls_keep_command_shortcuts_and_only_typing_blocks_them() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.set_layout(|layout| layout.sounds = true);
    let before = app.session.project().bpm;
    let other = if before == 160.0 { 140.0 } else { 160.0 };
    app.session.edit(EditCommand::SetTempo(other)).unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Stop");
    assert!(ctx.memory(|memory| memory.focused()).is_some());
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    assert_eq!(
        app.session.project().bpm,
        before,
        "Undo must work while a button has keyboard focus."
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Y, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    assert_eq!(app.session.project().bpm, other, "Ctrl+Y must redo.");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Find a sound");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    assert_eq!(
        app.session.project().bpm,
        other,
        "A text field that is being typed in keeps Ctrl+Z."
    );
}

#[test]
fn escape_ends_erase_while_its_rail_button_has_focus() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, RailIcon::Erase.label());
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        app.score_tools.erase,
        "Space on the focused Erase button starts erasing."
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    // egui releases focus on Esc before the workspace reads the key.
    assert!(
        !app.score_tools.erase,
        "Esc must end erasing, as the Erase tooltip says."
    );
}

#[test]
fn step_grid_is_one_tab_stop_with_arrow_key_navigation() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.selection.sample = Some(1);
    app.boards.module = BottomModule::StepSynth;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let nodes = &output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes;
    assert_eq!(
        nodes
            .iter()
            .filter(|(_, node)| {
                node.label()
                    .is_some_and(|label| label.starts_with("Step ") && label.ends_with("semitones"))
            })
            .count(),
        400,
        "Every cell stays available to assistive technology."
    );
    focus_control(&ctx, &mut app, &output, "Step 1, +0 semitones");
    // egui holds arrow keys for a control from its second focused frame.
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for key in [
        egui::Key::ArrowRight,
        egui::Key::ArrowRight,
        egui::Key::ArrowUp,
    ] {
        frame(
            &ctx,
            &mut app,
            vec![key_event(key, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.step_synth.state.steps[2],
        Some(1),
        "Arrow keys move the cell cursor and Enter sets its note."
    );
    assert!(
        app.board_shown(BottomModule::StepSynth),
        "Arrow keys must stay inside the grid."
    );
}

#[test]
fn mixer_pointer_gain_edit_keeps_transport_and_history_shortcuts() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let rail = effects_control_rect(&output, "Track 1 gain");
    click(
        &ctx,
        &mut app,
        rail.center_bottom() - Vec2::new(0.0, 20.0),
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        app.session.project().tracks[0].gain < 1.0,
        "The rail click must lower the gain."
    );
    // A pointer edit leaves keyboard focus unchanged.
    assert!(
        !ctx.wants_keyboard_input(),
        "A pointer edit must not leave the rail holding keyboard focus."
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    assert_eq!(
        app.session.project().tracks[0].gain,
        1.0,
        "Undo must work directly after a pointer gain edit."
    );
}
