//! Step synth pattern files: source recovery, pattern paths, feedback and pane focus.

use super::*;

#[test]
fn step_synth_file_source_can_recover_a_missing_palette_source_and_reports_read_errors_locally() {
    let (ctx, mut app) = missing_fixture();
    ctx.enable_accesskit();
    app.select_clip(1, false);
    app.boards.module = BottomModule::StepSynth;
    let draft = app.step_synth.state.clone();
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        step_synth_accessible_node(&output, "Listen sequence")
            .1
            .is_disabled()
    );
    step_synth_accessible_node(&output, "Import details");
    let absent_source = std::env::temp_dir()
        .join(format!(
            "mixjam-ui-step-synth-absent-{}",
            std::process::id()
        ))
        .join("source.wav");
    assert!(!absent_source.exists());
    app.finish_path(FileRequest::SynthSource, Some(absent_source.clone()));
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.step_synth.file.as_ref(), Some(&absent_source));
    assert!(
        !step_synth_accessible_node(&output, "Listen sequence")
            .1
            .is_disabled(),
        "An explicit file must bypass the missing palette source."
    );
    let output = activate_control(&ctx, &mut app, &output, "Listen sequence");
    assert!(app.status_error && app.step_synth.feedback.error);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some(app.status.as_str())
    );
    assert_eq!(app.step_synth.state, draft);
    let output = activate_control(&ctx, &mut app, &output, "Use palette sound");
    assert!(app.step_synth.file.is_none());
    assert!(app.step_synth.feedback.message.is_none());
    assert!(!app.step_synth.feedback.error);
    assert!(
        step_synth_accessible_node(&output, "Listen sequence")
            .1
            .is_disabled()
    );
    activate_control(&ctx, &mut app, &output, "Import details");
    assert!(app.sample_import.details_open);
    assert_eq!(app.step_synth.state, draft);
}

#[test]
fn step_synth_pattern_paths_keep_the_draft_on_cancel_or_error_and_restore_complete_state() {
    let (ctx, mut app) = fixture();
    let dir = std::env::temp_dir().join(format!(
        "mixjam-ui-step-synth-pattern-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    app.boards.module = BottomModule::StepSynth;
    app.step_synth.state.bpm = 160.0;
    app.step_synth.state.active_bars = [true, false, true, false];
    app.step_synth.state.steps[1] = Some(-48);
    app.step_synth.state.steps[63] = Some(48);
    app.step_synth.state.cut_sixteenth = true;
    app.step_synth.state.loop_source = true;
    app.step_synth.state.effects.dirty = 0.25;
    let saved = app.step_synth.state.clone();
    app.finish_path(FileRequest::SavePattern, None);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("Pattern save cancelled.")
    );
    app.finish_path(FileRequest::LoadPattern, None);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("Pattern load cancelled.")
    );
    app.finish_path(FileRequest::SynthSource, None);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("Source selection cancelled.")
    );
    assert!(!app.step_synth.feedback.error);
    assert_eq!(app.step_synth.state, saved);
    assert!(app.step_synth.file.is_none());
    let pattern = dir.join("sequence.json");
    app.finish_path(FileRequest::SavePattern, Some(pattern.clone()));
    assert!(!app.status_error);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("Saved Step synth pattern.")
    );
    assert_eq!(app.session.load_pattern(&pattern).unwrap(), saved);
    app.step_synth.state = StepSynthState::default();
    app.finish_path(FileRequest::LoadPattern, Some(pattern));
    assert!(!app.status_error);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("Loaded Step synth pattern.")
    );
    assert_eq!(
        app.step_synth.state, saved,
        "Loading must restore notes, active bars, tempo, source options, and effects."
    );
    let broken = dir.join("broken.json");
    std::fs::write(&broken, b"{ not a pattern }").unwrap();
    app.finish_path(FileRequest::LoadPattern, Some(broken));
    assert!(app.status_error);
    assert!(app.step_synth.feedback.error);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some(app.status.as_str())
    );
    assert_eq!(
        app.step_synth.state, saved,
        "A failed load must retain the current editable draft."
    );
    app.finish_path(
        FileRequest::SavePattern,
        Some(dir.join("absent-parent/sequence.json")),
    );
    assert!(app.status_error);
    assert!(app.step_synth.feedback.error);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some(app.status.as_str())
    );
    assert_eq!(app.step_synth.state, saved);
    assert_eq!(app.selection.sample, None);
    assert_eq!(app.step_synth.bar, 0);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.step_synth.state, saved);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn step_synth_feedback_is_dropped_when_the_source_changes_but_kept_for_its_own_results() {
    let (ctx, mut app) = missing_fixture();
    ctx.enable_accesskit();
    let dir = std::env::temp_dir().join(format!(
        "mixjam-ui-step-synth-feedback-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    app.selection.sample = Some(1);
    app.boards.module = BottomModule::StepSynth;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    app.finish_path(FileRequest::LoadPattern, None);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("Pattern load cancelled."),
        "Feedback must remain while the source is unchanged."
    );
    app.select_clip(1, false);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        app.step_synth.feedback.message.is_none(),
        "A new palette source must drop the previous source's feedback."
    );
    assert!(
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| [node.label(), node.value()].contains(&Some(
                "Missing audio. Choose a source file or open Import details."
            ))),
        "The footer must return to the missing-audio guidance."
    );
    app.finish_path(FileRequest::SynthSource, Some(dir.join("source.wav")));
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("Selected a source file. Listen before saving.")
    );
    app.selection.sample = Some(1);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("Selected a source file. Listen before saving."),
        "Palette selection must not affect feedback while an explicit file is the source."
    );
    app.step_synth.file = None;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    app.finish_path(
        FileRequest::SaveSound {
            pcm: Pcm {
                sample_rate: 1000,
                channels: 1,
                samples: vec![0.25; 1000],
            },
            name: "sequence".into(),
            group: "Step synth",
            source_bpm: 140.0,
            stretch: None,
        },
        Some(dir.join("sequence.wav")),
    );
    assert_ne!(
        app.selection.sample,
        Some(1),
        "Saving must select the new Step synth sound."
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some("Saved sequence.wav to Step synth."),
        "Selecting the saved sound must keep the save confirmation."
    );
    assert_eq!(
        app.session
            .load_pattern(&dir.join("sequence.pattern.json"))
            .unwrap()
            .steps,
        app.step_synth.state.steps,
        "Save sequence keeps the pattern beside the WAV."
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn step_synth_sequence_keeps_a_pattern_save_error_in_its_feedback() {
    let (ctx, mut app) = fixture();
    let dir =
        std::env::temp_dir().join(format!("mixjam-step-pattern-error-{}", std::process::id()));
    let wav = dir.join("sequence.wav");
    let pattern = mixjam_techno::application::pattern_beside(&wav);
    std::fs::create_dir_all(&pattern).unwrap();
    app.boards.module = BottomModule::StepSynth;
    app.finish_path(
        FileRequest::SaveSound {
            pcm: Pcm {
                sample_rate: 1000,
                channels: 1,
                samples: vec![0.25; 1000],
            },
            name: "sequence".into(),
            group: "Step synth",
            source_bpm: 140.0,
            stretch: None,
        },
        Some(wav.clone()),
    );
    assert!(!wav.exists(), "A pattern failure must not publish the WAV.");
    assert!(app.selection.sample.is_none());
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.status_error && app.step_synth.feedback.error);
    assert_eq!(
        app.step_synth.feedback.message,
        Some(format!("Cannot save the sequence: {}", app.status))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn step_synth_pane_switch_keeps_keyboard_focus_and_disabled_actions_explain_themselves() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
    app.selection.sample = Some(1);
    app.boards.module = BottomModule::StepSynth;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = activate_control(&ctx, &mut app, &output, "Sequence effects");
    assert!(app.step_synth.effects_expanded);
    assert_eq!(
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .focus,
        step_synth_accessible_node(&output, "Note grid").0,
        "Switching panes from the keyboard must keep focus on the pane button."
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        !app.step_synth.effects_expanded,
        "A second Enter must return to the note grid."
    );
    assert_eq!(
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .focus,
        step_synth_accessible_node(&output, "Sequence effects").0
    );
    app.step_synth.state.active_bars.fill(false);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let listen = step_synth_control_rect(&output, "Listen sequence");
    // The footer shows the same sentence, so count the tooltip as an added copy.
    let copies = |output: &egui::FullOutput| {
        output
            .shapes
            .iter()
            .filter(|shape| {
                matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "Switch on at least one Active bar.")
            })
            .count()
    };
    let before = copies(&output);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(listen.center())],
        egui::Modifiers::NONE,
    );
    let mut hovered = before;
    for _ in 0..4 {
        hovered = hovered.max(copies(&frame(
            &ctx,
            &mut app,
            vec![],
            egui::Modifiers::NONE,
        )));
    }
    assert!(
        hovered > before,
        "A disabled audio action must show why it is unavailable on hover."
    );
}
