//! The board's own undo and redo, apart from the song's, and the Listen key.
//! The history's rules are tested in `synth::edit_tests`; these tests check
//! that the board routes keys and gestures to it.

use super::*;

/// The note the undo tests edit.
const NOTE: &str = "Step 1, +12 semitones";

/// One frame that presses `key` with the pointer at `at`, then an idle frame.
fn press_over(
    ctx: &egui::Context,
    app: &mut MixJamApp,
    at: Pos2,
    key: egui::Key,
    modifiers: egui::Modifiers,
) {
    frame(
        ctx,
        app,
        vec![egui::Event::PointerMoved(at), key_event(key, modifiers)],
        modifiers,
    );
    settle(ctx, app, 1);
}

#[test]
fn step_synth_board_undo_takes_back_bar_actions_and_listen_has_its_own_key() {
    let (ctx, mut app) = board_fixture();
    app.selection.sample = Some(1);
    app.step_synth.state.steps[0] = Step::note(5);
    let output = settle(&ctx, &mut app, 4);
    // Copy names the bar it writes to.
    text_rect(&output, "Copy to 2");
    let before = app.step_synth.state.clone();
    let output = activate_control(&ctx, &mut app, &output, "Copy to 2");
    assert_eq!(app.step_synth.state.steps[16], Step::note(5));
    let output = activate_control(&ctx, &mut app, &output, "Clear bar");
    assert_eq!(app.step_synth.state.steps[0], Step::default());
    assert!(
        accessible_node(&output, "Clear bar").1.is_disabled(),
        "An empty bar has nothing to clear."
    );
    // Ctrl+Z over the board takes back Clear, then Copy; Ctrl+Y redoes.
    let board = step_synth_board_rect(&output).center();
    press_over(
        &ctx,
        &mut app,
        board,
        egui::Key::Z,
        egui::Modifiers::COMMAND,
    );
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("Undid Clear bar."),
        "The status line names what undo took back."
    );
    // The toolbar's undo key does the same as Ctrl+Z.
    let output = settle(&ctx, &mut app, 1);
    assert!(
        !accessible_node(&output, "Redo pattern edit")
            .1
            .is_disabled()
    );
    let output = activate_control(&ctx, &mut app, &output, "Undo pattern edit");
    assert_eq!(app.step_synth.state, before, "Undo restores the notes.");
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("Undid Copy bar.")
    );
    assert!(
        accessible_node(&output, "Undo pattern edit")
            .1
            .is_disabled(),
        "With nothing left to undo, the key is disabled."
    );
    press_over(
        &ctx,
        &mut app,
        board,
        egui::Key::Y,
        egui::Modifiers::COMMAND,
    );
    assert_eq!(app.step_synth.state.steps[16], Step::note(5));
    assert_eq!(app.step_synth.state.steps[0], Step::note(5));
    press_over(
        &ctx,
        &mut app,
        board,
        egui::Key::Z,
        egui::Modifiers::COMMAND,
    );
    press_over(
        &ctx,
        &mut app,
        board,
        egui::Key::Z,
        egui::Modifiers::COMMAND,
    );
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("No Step synth edit to undo.")
    );
    // L runs Listen from the keyboard while the board shows.
    app.step_synth.feedback.message = None;
    press_over(&ctx, &mut app, board, egui::Key::L, egui::Modifiers::NONE);
    assert!(
        app.step_synth.feedback.message.is_some(),
        "L must run Listen."
    );
    assert_eq!(
        commands::Command::StepSynthListen.shortcut(),
        Some("L"),
        "The command list shows Listen's key."
    );
}

#[test]
fn step_synth_undo_restores_an_edit_on_the_very_next_frame() {
    for pointer_edit in [false, true] {
        let (ctx, mut app) = board_fixture();
        app.step_synth.state.steps.fill(Step::default());
        let output = settle(&ctx, &mut app, 3);
        let at = step_synth_control_rect(&output, NOTE).center();
        if pointer_edit {
            click(&ctx, &mut app, at, egui::Modifiers::NONE);
        } else {
            focus_control(&ctx, &mut app, &output, NOTE);
            press(&ctx, &mut app, egui::Key::Enter, egui::Modifiers::NONE);
        }
        assert_eq!(app.step_synth.state.steps[0], Step::note(12));
        // No idle frame may be required between an edit and its undo.
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(at),
                key_event(egui::Key::Z, egui::Modifiers::COMMAND),
            ],
            egui::Modifiers::COMMAND,
        );
        assert_eq!(
            app.step_synth.state.steps[0],
            Step::default(),
            "The newest edit must already be available to undo: pointer={pointer_edit}."
        );
    }
}

#[test]
fn step_synth_scrolled_grid_focus_keeps_undo_when_the_pointer_leaves_the_board() {
    let (ctx, mut app) = board_fixture();
    app.step_synth.state.steps.fill(Step::default());
    let size = Vec2::new(960.0, 720.0);
    let output = settle_at(&ctx, &mut app, 3, size);
    let target = accessible_node(&output, NOTE).0;
    frame_at_size(
        &ctx,
        &mut app,
        vec![access_request(target, egui::accesskit::Action::Focus)],
        egui::Modifiers::NONE,
        size,
    );
    frame_at_size(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        size,
    );
    assert_eq!(app.step_synth.state.steps[0], Step::note(12));
    // Let the edit reach history so that this checks focus routing alone.
    settle_at(&ctx, &mut app, 1, size);
    frame_at_size(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(Pos2::new(900.0, 100.0)),
            key_event(egui::Key::Z, egui::Modifiers::COMMAND),
        ],
        egui::Modifiers::COMMAND,
        size,
    );
    assert_eq!(
        app.step_synth.state.steps[0],
        Step::default(),
        "The scrolled grid keeps focus, so Ctrl+Z must undo its note edit."
    );
}

#[test]
fn step_synth_multiframe_tie_drag_is_one_undo_step() {
    let (ctx, mut app) = board_fixture();
    app.step_synth.state.steps.fill(Step::default());
    app.step_synth.state.steps[0] = Step::note(12);
    let before = app.step_synth.state.clone();
    let output = settle(&ctx, &mut app, 3);
    let from = step_synth_control_rect(&output, NOTE).center();
    let to = step_synth_control_rect(&output, "Step 6, +12 semitones").center();
    // The driver moves through four frames with the pointer held.
    drag(&ctx, &mut app, from, to, egui::Modifiers::NONE);
    assert_eq!(app.step_synth.state.steps[0], Step::note(12));
    assert!(
        app.step_synth.state.steps[1..6]
            .iter()
            .all(|step| step.trig == Trig::Tie),
        "The pointer drag must extend the note through step 6."
    );
    let extended = app.step_synth.state.clone();
    press(&ctx, &mut app, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(app.step_synth.state, before);
    assert!(
        !app.step_synth.grid.can_undo(false),
        "A multi-frame pointer gesture must create exactly one undo step."
    );
    press(&ctx, &mut app, egui::Key::Y, egui::Modifiers::COMMAND);
    assert_eq!(
        app.step_synth.state, extended,
        "Redo must restore the whole tie gesture."
    );
}

#[test]
fn step_synth_source_undo_redo_preserves_start_and_loop_after_redraw() {
    let dir = crate::test_folder::TestFolder::new("mixjam-step-source-history");
    let first = dir.join("first.wav");
    let second = dir.join("second.wav");
    for (path, silence) in [(&first, 100), (&second, 200)] {
        let mut samples = vec![0.0; 2000];
        samples[silence..].fill(0.25);
        Pcm {
            sample_rate: 1000,
            channels: 1,
            samples,
        }
        .write_wav(path)
        .unwrap();
    }
    for load in [false, true] {
        let (ctx, mut app) = board_fixture();
        app.step_synth.state.source = Some(first.clone());
        app.step_synth.state.start_ms = 37.0;
        app.step_synth.state.loop_source = true;
        settle(&ctx, &mut app, 3);
        let before = app.step_synth.state.clone();
        if load {
            let pattern = dir.join("second.pattern.json");
            let next = StepSynthState {
                source: Some(second.clone()),
                start_ms: 123.0,
                loop_source: true,
                ..before.clone()
            };
            app.session.save_pattern(&pattern, &next).unwrap();
            app.finish_path(FileRequest::LoadPattern, Some(pattern));
        } else {
            app.finish_path(FileRequest::SynthSource, Some(second.clone()));
        }
        settle(&ctx, &mut app, 3);
        let after = app.step_synth.state.clone();
        assert_ne!(after.source, before.source);
        app.step_synth_undo(false);
        settle(&ctx, &mut app, 3);
        assert_eq!(
            app.step_synth.state, before,
            "Undo must keep the restored source settings after redraw: load={load}."
        );
        app.step_synth_undo(true);
        settle(&ctx, &mut app, 3);
        assert_eq!(
            app.step_synth.state, after,
            "Redo must keep the restored source settings after redraw: load={load}."
        );
    }
}

#[test]
fn step_synth_adopts_replacement_audio_with_the_same_palette_id() {
    let (ctx, mut app, dir) = sample_conflicts_fixture("step-source-revision");
    app.session.confirm_sample_conflicts().unwrap();
    app.conflicts.open = false;
    app.selection.sample = Some(1_000_000);
    app.boards.module = BottomModule::StepSynth;
    settle(&ctx, &mut app, 3);
    assert_eq!(app.step_synth.adopted.as_ref().unwrap().1, Some(2000.0));
    app.step_synth.state.start_ms = 500.0;
    app.step_synth.state.loop_source = true;
    settle(&ctx, &mut app, 2);

    let path = app.session.sample_conflicts()[0].candidates[1].clone();
    let mut samples = vec![0.0; 800];
    samples[50..].fill(0.25);
    let replacement = Pcm {
        sample_rate: 1000,
        channels: 1,
        samples,
    };
    let start = mixjam_techno::application::pattern::first_hit(&replacement);
    replacement.write_wav(&path).unwrap();
    let revision = app.session.catalog_revision();
    app.session.resolve_sample_conflict(0, 1).unwrap();
    assert!(app.session.catalog_revision() > revision);
    settle(&ctx, &mut app, 3);
    assert_eq!(app.source_id(), Some(1_000_000));
    assert_eq!(
        app.step_synth.adopted.as_ref().unwrap().1,
        Some(800.0),
        "A catalog replacement must refresh the cached source length."
    );
    assert_eq!(app.step_synth.state.start_ms, start);
    assert!(!app.step_synth.state.loop_source);
    app.step_synth.state.start_ms = 123.0;
    app.step_synth.state.loop_source = true;
    settle(&ctx, &mut app, 2);
    let before_save = app.step_synth.state.clone();
    app.finish_path(
        FileRequest::SaveSound {
            pcm: replacement,
            name: "Saved pattern".into(),
            group: "Step synth",
            source_bpm: before_save.bpm,
            stretch: None,
        },
        Some(dir.join("saved.wav")),
    );
    assert!(!app.status_error, "{}", app.status);
    settle(&ctx, &mut app, 3);
    assert_eq!(
        app.step_synth.state, before_save,
        "Saving a sound replaces the catalog but must keep the source settings."
    );
}

#[test]
fn step_synth_keeps_settings_when_the_same_audio_moves() {
    let (ctx, mut app, _dir) = sample_conflicts_fixture("step-source-rebound");
    app.session.confirm_sample_conflicts().unwrap();
    app.conflicts.open = false;
    app.selection.sample = Some(1_000_000);
    app.boards.module = BottomModule::StepSynth;
    // Set the draft before the board observes it for the first time.
    app.step_synth.state.start_ms = 500.0;
    app.step_synth.state.loop_source = true;
    settle(&ctx, &mut app, 3);
    let before = app.step_synth.state.clone();
    assert!(!app.step_synth.grid.can_undo(false));

    let candidates = &app.session.sample_conflicts()[0].candidates;
    std::fs::copy(&candidates[0], &candidates[1]).unwrap();
    app.session.resolve_sample_conflict(0, 1).unwrap();
    settle(&ctx, &mut app, 3);

    assert_eq!(app.source_id(), Some(1_000_000));
    assert_eq!(app.step_synth.state, before);
    assert!(!app.step_synth.grid.can_undo(false));
}

#[test]
fn step_synth_uses_playable_audio_to_set_source_length() {
    let (ctx, mut app, _dir) = sample_conflicts_fixture("step-source-read-failed");
    app.session.confirm_sample_conflicts().unwrap();
    app.conflicts.open = false;
    let path = &app.session.sample_conflicts()[0].candidates[0];
    std::fs::remove_file(path).unwrap();
    app.selection.sample = Some(1_000_000);
    app.boards.module = BottomModule::StepSynth;
    settle(&ctx, &mut app, 3);

    assert_eq!(app.source_id(), Some(1_000_000));
    assert_eq!(app.step_synth.adopted.as_ref().unwrap().1, None);
}
