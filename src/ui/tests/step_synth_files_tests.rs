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
        step_synth_accessible_node(&output, "Listen")
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
    assert_eq!(app.step_synth.state.source.as_ref(), Some(&absent_source));
    assert!(
        !step_synth_accessible_node(&output, "Listen")
            .1
            .is_disabled(),
        "An explicit file must bypass the missing palette source."
    );
    let output = activate_control(&ctx, &mut app, &output, "Listen");
    assert!(app.status_error);
    assert_eq!(
        app.step_synth.feedback.message.as_deref(),
        Some(app.status.as_str())
    );
    // The file is part of the pattern now; a failed Listen changes nothing else.
    assert_eq!(
        StepSynthState {
            source: None,
            ..app.step_synth.state.clone()
        },
        draft
    );
    let output = activate_control(&ctx, &mut app, &output, "Palette sound");
    assert!(app.step_synth.state.source.is_none());
    assert!(app.step_synth.feedback.message.is_none());
    assert_eq!(app.status, DEFAULT_STATUS);
    assert!(!app.status_error);
    assert!(
        step_synth_accessible_node(&output, "Listen")
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
    app.step_synth.state.length = 12;
    app.step_synth.state.steps[1] = Step {
        velocity: 30,
        accent: true,
        slide: true,
        ..Step::note(-48)
    };
    app.step_synth.state.steps[2] = Step::tie();
    app.step_synth.state.steps[63] = Step::note(48);
    app.step_synth.state.gate = 25;
    app.step_synth.state.swing = 62;
    app.step_synth.state.accent_db = 9.0;
    app.step_synth.state.slide_ms = 120.0;
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
    assert!(!app.status_error);
    assert_eq!(app.step_synth.state, saved);
    assert!(app.step_synth.state.source.is_none());
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
fn step_synth_status_bar_announces_full_messages_and_labels_errors() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::StepSynth;
    app.selection.sample = Some(1);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for error in [false, true] {
        let message = "The Step synth result must remain available in full. ".repeat(12);
        app.step_synth_say(&message, error);
        let output = frame_at_size(
            &ctx,
            &mut app,
            vec![],
            egui::Modifiers::NONE,
            Vec2::new(960.0, 720.0),
        );
        let spoken = if error {
            format!("Error: {message}")
        } else {
            message
        };
        assert!(
            output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .any(
                    |(_, node)| [node.label(), node.value()].contains(&Some(spoken.as_str()))
                        && node.live() == Some(egui::accesskit::Live::Polite)
                ),
            "The full message must be a polite live status, with errors labelled: {spoken}"
        );
    }
}

fn hide_step_synth(app: &mut MixJamApp, mode: &str) {
    match mode {
        "folded" => app.set_layout(|layout| layout.boards = false),
        "other board" => app.boards.module = BottomModule::Mixer,
        "maximised" => app.set_layout(|layout| layout.maximised = true),
        _ => unreachable!(),
    }
}

#[test]
fn step_synth_hidden_board_drops_the_old_palette_sources_status() {
    for mode in ["folded", "other board", "maximised"] {
        for error in [false, true] {
            let (ctx, mut app) = missing_fixture();
            app.boards.module = BottomModule::StepSynth;
            app.selection.sample = Some(1);
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            app.step_synth_say("A result for the old source.", error);
            hide_step_synth(&mut app, mode);
            app.select_clip(1, false);
            assert_ne!(app.step_synth.feedback.source, app.selection.sample);
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_eq!(app.status, DEFAULT_STATUS, "Board state: {mode}");
            assert!(!app.status_error, "Board state: {mode}");
            assert!(app.step_synth.feedback.message.is_none());
        }
    }
}

#[test]
fn step_synth_hidden_source_changes_keep_unrelated_and_file_notices() {
    for mode in ["folded", "other board", "maximised"] {
        for error in [false, true] {
            let (ctx, mut app) = missing_fixture();
            app.boards.module = BottomModule::StepSynth;
            app.selection.sample = Some(1);
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            app.step_synth_say("A result for the old source.", false);
            let notice = "An unrelated application notice.";
            if error {
                app.error(notice);
            } else {
                app.message(notice);
            }
            hide_step_synth(&mut app, mode);
            app.select_clip(1, false);
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_eq!(app.status, notice, "Board state: {mode}");
            assert_eq!(app.status_error, error, "Board state: {mode}");
            assert!(app.step_synth.feedback.message.is_none());

            app.finish_path(
                FileRequest::SynthSource,
                Some(std::env::temp_dir().join("step-synth-explicit-source.wav")),
            );
            let file_notice = app.status.clone();
            app.selection.sample = Some(1);
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_eq!(app.status, file_notice, "Board state: {mode}");
            assert_eq!(app.step_synth.feedback.message, Some(file_notice));
        }
    }
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
    assert_eq!(
        app.status, DEFAULT_STATUS,
        "A new palette source must take the old message off the status line."
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
        "The status bar must return to the missing-audio guidance."
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
    app.step_synth.state.source = None;
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
    // The saved sound joins the palette; the palette source stays selected.
    assert_eq!(
        app.selection.sample,
        Some(1),
        "Saving must keep the palette source: {}",
        app.status
    );
    assert!(
        app.session
            .library()
            .samples
            .iter()
            .any(|sample| sample.group_name == "Step synth" && sample.name == "sequence"),
        "Saving must add the sound to Step synth."
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
        "Save sound keeps the pattern beside the WAV."
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
    assert!(app.status_error);
    assert!(app.status.starts_with("Cannot save the sound: "));
    assert_eq!(app.step_synth.feedback.message, Some(app.status.clone()));
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
    let focus = |output: &egui::FullOutput| {
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .focus
    };
    let output = activate_control(&ctx, &mut app, &output, "Effects pane");
    assert!(app.step_synth.effects_expanded);
    assert_eq!(
        focus(&output),
        step_synth_accessible_node(&output, "Effects pane").0,
        "Switching panes from the keyboard must keep focus on the switch."
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        app.step_synth.effects_expanded,
        "Pressing the chosen side again keeps its pane."
    );
    let output = activate_control(&ctx, &mut app, &output, "Notes pane");
    assert!(
        !app.step_synth.effects_expanded,
        "Notes must return to the note grid."
    );
    assert_eq!(
        focus(&output),
        step_synth_accessible_node(&output, "Notes pane").0
    );
    app.selection.sample = None;
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let listen = step_synth_control_rect(&output, "Listen");
    // Count the tooltip as an added copy of the sentence.
    let copies = |output: &egui::FullOutput| {
        output
            .shapes
            .iter()
            .filter(|shape| {
                matches!(
                    &shape.shape,
                    egui::Shape::Text(text)
                        if text.galley.job.text
                            == "Choose an available source file or palette sound."
                )
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

#[test]
fn step_synth_listen_key_works_with_focus_on_the_pane_toolbar() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.selection.sample = Some(1);
    app.boards.module = BottomModule::StepSynth;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for pane in ["Effects pane", "Notes pane"] {
        activate_control(&ctx, &mut app, &output, pane);
        app.step_synth.feedback.message = None;
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::L, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        // A second Listen press must follow a release; egui marks another
        // press of a held key as a repeat, which this toggle ignores.
        output = frame(
            &ctx,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::L,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        );
        assert!(
            app.step_synth.feedback.message.is_some(),
            "L must run Listen while {pane} has focus."
        );
    }
}

#[test]
fn step_synth_shortcuts_do_not_claim_an_overlapping_window() {
    for focused in [false, true] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.selection.sample = Some(1);
        app.boards.module = BottomModule::StepSynth;
        app.step_synth.state.steps[0] = Step::note(5);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = activate_control(&ctx, &mut app, &output, "Copy to 2");
        let bounds = step_synth_board_node(&output).1.bounds().unwrap();
        let at = Pos2::new(
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        );
        if let Some(id) = ctx.memory(|memory| memory.focused()) {
            ctx.memory_mut(|memory| memory.surrender_focus(id));
        }
        let overlay_frame = |app: &mut MixJamApp, events, modifiers| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
                    events,
                    modifiers,
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    app.draw(ctx);
                    egui::Area::new(egui::Id::new("overlapping-tool"))
                        .order(egui::Order::Middle)
                        .fixed_pos(at - Vec2::new(20.0, 10.0))
                        .show(ctx, |ui| {
                            let response = ui.button("Other tool");
                            if focused {
                                response.request_focus();
                            }
                        });
                },
            )
        };
        for _ in 0..3 {
            overlay_frame(
                &mut app,
                vec![egui::Event::PointerMoved(at)],
                egui::Modifiers::NONE,
            );
        }
        app.step_synth.feedback.message = None;
        if focused {
            overlay_frame(
                &mut app,
                vec![key_event(egui::Key::L, egui::Modifiers::NONE)],
                egui::Modifiers::NONE,
            );
            assert!(
                app.step_synth.feedback.message.is_none(),
                "A focused floating tool must keep L from the board below it."
            );
        }
        overlay_frame(
            &mut app,
            vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
            egui::Modifiers::COMMAND,
        );
        assert_eq!(
            app.step_synth.state.steps[16],
            Step::note(5),
            "An overlapping tool must not route Ctrl+Z to the board: focus={focused}."
        );
    }
}
