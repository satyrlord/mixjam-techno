//! Keyboard focus and shortcuts: focus outlines, list keys, focus scrolling and
//! shortcuts beside focused controls.

use super::*;

#[test]
fn focused_sound_list_shows_its_focus_without_a_current_sound() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    with_more_sounds(&mut app);
    app.open_sounds_panel();
    app.selection.sample = None;
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let marked = |output: &egui::FullOutput| {
        let list = effects_control_rect(output, "Sound list");
        output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(mark)
                if mark.stroke.color == roles().accent
                    && mark.stroke.width == 2.0
                    && list.expand(1.0).contains_rect(mark.rect))
        })
    };
    assert!(!marked(&output), "An unfocused list has no focus mark.");
    focus_control(&ctx, &mut app, &output, "Sound list");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(focused_label(&output), Some("Sound list"));
    assert!(
        marked(&output),
        "The focused list shows where the focus is before a sound is chosen."
    );
}

#[test]
fn short_lists_show_whole_and_do_not_scroll() {
    type Setup = fn(&egui::Context, &mut MixJamApp);
    let sounds: Setup = |_, app| {
        with_more_sounds(app);
        app.open_sounds_panel();
    };
    let step_synth: Setup = |_, app| app.boards.module = BottomModule::StepSynth;
    let providers: Setup = |ctx, app| {
        app.open_sample_libraries(ctx);
        frame(ctx, app, vec![], egui::Modifiers::NONE);
        click_named(ctx, app, "Optional product providers");
    };
    for (list, setup) in [
        ("Library filter", sounds),
        ("Scale", step_synth),
        ("Scale root", step_synth),
        ("Provider product", providers),
    ] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        setup(&ctx, &mut app);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        click_named(&ctx, &mut app, list);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let menu = effects_control_rect(&output, "Menu");
        assert!(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0)).contains_rect(menu),
            "The {list} list stays in the window: {menu:?}."
        );
        // egui adds the scroll parts only when the rows do not fit.
        let scrolls = screen(&output).nodes().any(|node| {
            node.node()
                .label()
                .is_some_and(|label| label.starts_with("Menu ") && label.contains("scroll"))
        });
        assert!(!scrolls, "The {list} list must show all its rows.");
    }
}

#[test]
fn focused_menu_surface_shows_a_focus_outline() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Automation lane");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    // The outline goes round the whole surface, inside its edge.
    let marked = |output: &egui::FullOutput| {
        let menu = effects_control_rect(output, "Menu");
        output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(mark)
                if mark.stroke.color == roles().accent
                    && mark.stroke.width == 2.0
                    && menu.contains_rect(mark.rect)
                    && mark.rect.expand(2.0).contains_rect(menu))
        })
    };
    let tab = |app: &mut MixJamApp| {
        frame(
            &ctx,
            app,
            vec![key_event(egui::Key::Tab, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, app, vec![], egui::Modifiers::NONE)
    };
    assert!(!marked(&output), "An unfocused menu has no focus mark.");
    let output = tab(&mut app);
    assert_eq!(focused_label(&output), Some("Menu"));
    assert!(marked(&output), "The focused menu surface shows the focus.");
    let output = tab(&mut app);
    assert_eq!(focused_label(&output), Some("Off"));
    assert!(!marked(&output), "The mark goes when the focus moves on.");
}

#[test]
fn sound_list_keys_choose_sounds_and_enter_places_the_current_one() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    with_more_sounds(&mut app);
    app.open_sounds_panel();
    app.selection.sample = None;
    app.recording.track = 3;
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Sound list");
    // The repaint that the list asks for when it gains focus.
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let press = |app: &mut MixJamApp, key: egui::Key| {
        frame(
            &ctx,
            app,
            vec![key_event(key, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, app, vec![], egui::Modifiers::NONE);
    };
    let (order, _) = app.palette_view();
    let id_at = |app: &MixJamApp, row: usize| app.session.library().samples[order[row]].id;
    press(&mut app, egui::Key::ArrowDown);
    assert_eq!(
        app.selection.sample,
        Some(id_at(&app, 0)),
        "Down starts at the first sound."
    );
    press(&mut app, egui::Key::ArrowDown);
    assert_eq!(app.selection.sample, Some(id_at(&app, 1)));
    press(&mut app, egui::Key::End);
    assert_eq!(app.selection.sample, Some(id_at(&app, order.len() - 1)));
    press(&mut app, egui::Key::Home);
    assert_eq!(app.selection.sample, Some(id_at(&app, 0)));
    let clips = app.session.project().clips.len();
    press(&mut app, egui::Key::Enter);
    assert_eq!(
        app.session.project().clips.len(),
        clips + 1,
        "Enter places the current sound."
    );
}

#[test]
fn delete_removes_the_selected_clips_while_the_sound_list_has_focus() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    with_more_sounds(&mut app);
    app.open_sounds_panel();
    let clip = app.session.project().clips[0].id;
    app.selection.clip = Some(clip);
    app.selection.clips = [clip].into_iter().collect();
    let count = app.session.project().clips.len();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Sound list");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(focused_label(&output), Some("Sound list"));
    let sound = app.selection.sample;
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Delete, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.session.project().clips.len(),
        count - 1,
        "Delete removes the selected clip while the list has focus."
    );
    assert!(app.selection.clips.is_empty());
    assert_eq!(app.selection.sample, sound, "The current sound stays.");
    assert_eq!(
        focused_label(&output),
        Some("Sound list"),
        "The list keeps the focus."
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
                let rect = effects_control_rect(&output, label);
                // The rail ends at the 30-point status bar.
                assert!(
                    rect.top() >= 64.0 && rect.bottom() <= size.y - 30.0,
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
    let cells = screen(&output)
        .nodes()
        .filter(|node| {
            node.node().label().is_some_and(|label| {
                label.starts_with("Step ")
                    && (label.ends_with("semitones") || label.ends_with("semitone"))
            })
        })
        .count();
    // 25 pitch rows over one, two, or four bars, as the board's width allows.
    assert!(
        [400, 800, 1600].contains(&cells),
        "Every cell in view stays available to assistive technology: {cells}"
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
        Step::note(1),
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
