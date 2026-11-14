//! Stretch tool window states: busy, preview, saved and error feedback in the footer.

use super::*;

#[test]
fn stretch_idle_status_returns_after_a_preview_and_marks_unheard_settings() {
    for (audition, original, expected) in [
        (
            None,
            160.0,
            "Duration +33%. Pitch preservation is approximate. Listen before saving.",
        ),
        (
            Some((1, 160.0, 120.0)),
            160.0,
            "Duration +33%. Pitch preservation is approximate.",
        ),
        (
            Some((1, 160.0, 120.0)),
            150.0,
            "Duration +25%. Pitch preservation is approximate. Listen before saving.",
        ),
        (
            Some((1, 160.0, 120.0)),
            120.0,
            "Same tempo. Duration is unchanged.",
        ),
    ] {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        let (mut original, mut target, mut open) = (original, 120.0, true);
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    stretch::show(
                        ctx,
                        &mut open,
                        stretch::Source {
                            name: "Sound",
                            id: Some(1),
                            samples: &[],
                            catalog_revision: 0,
                            missing: false,
                            bpm: 160.0,
                        },
                        stretch::Feedback {
                            processing: false,
                            listening: false,
                            saved: false,
                            audition,
                            message: None,
                            error: false,
                        },
                        &mut original,
                        &mut target,
                    );
                },
            );
        }
        text_rect(&output, expected);
    }
}

#[test]
fn stretch_panel_exposes_busy_preview_saved_and_error_states() {
    for (processing, listening, saved, message, error) in [
        (true, false, false, Some("Processing sound..."), false),
        (false, true, false, None, false),
        (false, false, true, Some("Saved result.wav."), false),
        (
            false,
            false,
            false,
            Some("Audio output is unavailable."),
            true,
        ),
    ] {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        ctx.style_mut(|style| style.animation_time = 0.0);
        ctx.enable_accesskit();
        let mut original = 160.0;
        let mut target = 120.0;
        let mut open = true;
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    stretch::show(
                        ctx,
                        &mut open,
                        stretch::Source {
                            name: "Sound",
                            id: Some(1),
                            samples: &[],
                            catalog_revision: 0,
                            missing: false,
                            bpm: 160.0,
                        },
                        stretch::Feedback {
                            processing,
                            listening,
                            saved,
                            audition: Some((1, 160.0, 120.0)),
                            message,
                            error,
                        },
                        &mut original,
                        &mut target,
                    );
                },
            );
        }
        let action_name = if listening {
            "Stop listening"
        } else {
            "Listen to stretched sound"
        };
        assert_eq!(
            accessible_node(&output, action_name).1.is_disabled(),
            processing
        );
        assert_eq!(
            accessible_node(&output, "Save stretched sound")
                .1
                .is_disabled(),
            processing
        );
        if let Some(message) = message {
            text_rect(&output, message);
        }
        if saved {
            text_rect(&output, "Saved");
        }
        if processing {
            text_rect(&output, "Processing...");
        }
        if listening {
            text_rect(&output, "Stop listening");
        }
    }
}

#[test]
fn stretch_status_and_error_messages_are_confined_to_the_footer_below_the_skin() {
    let long_preview = "Listening: 160 to 120 BPM. Settings changed; listen again. ".repeat(20);
    let long_save = format!("Saved {}result.wav.", "long folder name/".repeat(60));
    let density = 1.0;
    for (processing, listening, saved, message, error, missing) in [
        (false, false, false, None, false, false),
        (
            true,
            false,
            false,
            Some("Processing sound..."),
            false,
            false,
        ),
        (
            false,
            true,
            false,
            Some(long_preview.as_str()),
            false,
            false,
        ),
        (false, false, true, Some(long_save.as_str()), false, false),
        (
            false,
            false,
            false,
            Some("Audio output is unavailable."),
            true,
            false,
        ),
        (false, false, false, None, false, true),
    ] {
        let output = stretch_layout_output(
            density,
            missing,
            stretch::Feedback {
                processing,
                listening,
                saved,
                audition: Some((1, 160.0, 120.0)),
                message,
                error,
            },
        );
        let bounds = accessible_node(&output, "Time Stretcher status")
            .1
            .bounds()
            .unwrap();
        let footer = Rect::from_min_max(
            Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
            Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
        );
        let window = effects_control_rect(&output, "TIME STRETCHER");
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0));
        assert!(viewport.contains_rect(window) && window.contains_rect(footer));
        for name in [
            if listening {
                "Stop listening"
            } else {
                "Listen to stretched sound"
            },
            "Save stretched sound",
        ] {
            assert!(
                accessible_node(&output, name).1.bounds().unwrap().y1 < f64::from(footer.top()),
                "Footer overlaps {name}"
            );
        }
        let status = if missing {
            "Missing audio. Open Import details to resolve the sound."
        } else {
            message.unwrap_or("Duration +33%. Pitch preservation is approximate.")
        };
        let painted = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == status => Some((shape, text)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing footer message: {status}"));
        let text_bounds = painted.1.visual_bounding_rect();
        assert!(
            footer.contains_rect(painted.0.clip_rect),
            "Message clip leaves its footer: {:?} / {footer:?}",
            painted.0.clip_rect
        );
        assert!(
            footer.contains_rect(text_bounds.intersect(painted.0.clip_rect)),
            "Visible status leaves its footer"
        );
        if status.len() > 200 {
            assert!(painted.1.galley.rows.len() > 1, "Long messages must wrap");
            assert!(
                text_bounds.bottom() > painted.0.clip_rect.bottom(),
                "Long message must be clipped to its scroll region"
            );
        } else {
            let offset = (text_bounds.center() - painted.0.clip_rect.center()) * density;
            assert!(
                offset.x.abs() <= 1.0 && offset.y.abs() <= 1.0,
                "Footer message is not visibly centered at {density}x: {offset:?}"
            );
        }
        if missing {
            assert_full_recovery_caption(&output);
            let details = accessible_node(&output, "Import details")
                .1
                .bounds()
                .unwrap();
            let details = Rect::from_min_max(
                Pos2::new(details.x0 as f32, details.y0 as f32),
                Pos2::new(details.x1 as f32, details.y1 as f32),
            );
            assert!(
                footer.contains_rect(details),
                "Recovery control leaves its footer"
            );
            assert!((details.center().y - footer.center().y).abs() * density <= 1.0);
            let text = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "Import details" => {
                        Some(text.visual_bounding_rect())
                    }
                    _ => None,
                })
                .unwrap();
            assert!(
                (text.center() - (details.center() - Vec2::new(0.0, 1.0))).length() <= 3.0,
                "Shared button text must be centered in its raised face."
            );
            assert!(
                painted.0.clip_rect.right() < details.left(),
                "Missing audio message overlaps recovery control"
            );
        }
    }
}

#[test]
fn stretch_async_errors_are_local_and_changed_or_closed_requests_ask_to_process_again() {
    let (ctx, mut app) = fixture();
    app.selection.sample = Some(1);
    for change in [0, 1, 2] {
        app.stretch.open = true;
        app.start_stretch(StretchPurpose::Save);
        assert!(app.stretch.feedback.pending.is_some());
        if change == 1 {
            app.stretch.target_bpm = 180.0;
        }
        if change == 2 {
            app.stretch.open = false;
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while app.stretch.feedback.pending.is_some() && std::time::Instant::now() < deadline {
            app.poll_application(&ctx);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(app.stretch.feedback.pending.is_none());
        if change == 0 {
            assert!(app.stretch.feedback.error);
            assert!(
                app.stretch
                    .feedback
                    .message
                    .as_ref()
                    .is_some_and(|message| !message.is_empty())
            );
        } else {
            assert!(!app.stretch.feedback.error);
            assert_eq!(
                app.stretch.feedback.message.as_deref(),
                Some("Settings changed. Process the sound again.")
            );
        }
    }
}

#[test]
fn stretch_save_confirmation_tracks_success_cancel_failure_and_output_tempo() {
    let (_, mut app) = fixture();
    let dir = std::env::temp_dir().join(format!("mixjam-ui-stretch-save-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    app.selection.sample = Some(1);
    let settings = StretchSettings {
        source_id: 1,
        source_bpm: 160.0,
        target_bpm: 120.0,
    };
    let request = || FileRequest::SaveSound {
        pcm: Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.25; 1000],
        },
        name: "stretch result".into(),
        group: "Wave",
        source_bpm: settings.target_bpm,
        stretch: Some(settings),
    };
    app.finish_path(request(), Some(dir.join("result.wav")));
    assert!(!app.stretch.feedback.error);
    // A save is a receipt: the tool keeps its source and both tempos.
    assert_eq!(app.selection.sample, Some(1));
    assert_eq!(
        (app.stretch.source_bpm, app.stretch.target_bpm),
        (140.0, 140.0)
    );
    assert_eq!(app.stretch.feedback.saved, Some(settings));
    assert_eq!(
        app.stretch.feedback.message.as_deref(),
        Some("Saved result.wav to Wave.")
    );
    let saved: Vec<_> = app
        .session
        .library()
        .samples
        .iter()
        .filter(|sample| sample.id != 1)
        .map(|sample| sample.id)
        .collect();
    assert_eq!(saved.len(), 1);
    assert_eq!(app.session.source_bpm(Some(saved[0])), 120.0);
    app.finish_path(request(), None);
    assert_eq!(
        app.stretch.feedback.message.as_deref(),
        Some("Save cancelled.")
    );
    assert!(app.stretch.feedback.saved.is_none());
    app.finish_path(request(), Some(dir.join("absent-parent/result.wav")));
    assert!(app.stretch.feedback.error);
    assert!(app.stretch.feedback.saved.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn stretch_stop_and_close_reopen_discard_successful_pending_results() {
    let (ctx, mut app, _port) = agent_fixture();
    let dir = std::env::temp_dir().join(format!("mixjam-ui-stretch-cancel-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("source.wav");
    Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.25; 1000],
    }
    .write_wav(&path)
    .unwrap();
    let source = app.register_file(&path, "Wave", 140.0).unwrap();
    for action in [0, 1, 2] {
        app.selection.sample = Some(source);
        app.stretch.open = true;
        app.start_stretch(if action == 2 {
            StretchPurpose::Save
        } else {
            StretchPurpose::Listen
        });
        if action == 0 {
            app.stop();
        } else {
            app.stretch.open = false;
            app.close_stretch_request();
            app.stretch.open = true;
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while (app.session.is_stretching() || app.stretch.feedback.pending.is_some())
            && std::time::Instant::now() < deadline
        {
            app.poll_application(&ctx);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!app.session.is_stretching());
        assert!(app.stretch.feedback.pending.is_none());
        assert!(
            app.files.path_prompt.is_none(),
            "An abandoned Save must not open a picker."
        );
        assert!(app.stretch.feedback.audition.is_none());
        assert!(
            !app.stretch.feedback.error,
            "An abandoned Listen must not attempt audio output."
        );
        assert_eq!(
            app.stretch.feedback.message.as_deref(),
            Some(if action == 0 {
                "Preview cancelled by another playback action."
            } else {
                "Processing request cancelled."
            })
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn stretch_preview_failure_is_a_local_error_that_begins_with_cannot() {
    let (ctx, mut app, _port) = agent_fixture();
    let dir = std::env::temp_dir().join(format!("mixjam-ui-stretch-listen-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("source.wav");
    Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.25; 1000],
    }
    .write_wav(&path)
    .unwrap();
    app.selection.sample = Some(app.register_file(&path, "Wave", 140.0).unwrap());
    app.stretch.open = true;
    app.start_stretch(StretchPurpose::Listen);
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while app.stretch.feedback.pending.is_some() && std::time::Instant::now() < deadline {
        app.poll_application(&ctx);
        std::thread::sleep(Duration::from_millis(1));
    }
    // The headless session has no audio output.
    assert!(app.stretch.feedback.error);
    assert!(app.stretch.feedback.audition.is_none());
    assert_eq!(
        app.stretch.feedback.message.as_deref(),
        Some("Cannot play the preview: Audio output is unavailable.")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn stretch_recovery_control_clears_action_border_and_context_help_starts_with_stretching() {
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        let (ctx, mut app) = missing_fixture();
        ctx.enable_accesskit();
        app.selection.sample = Some(app.session.project().missing_samples[0].id);
        app.stretch.open = true;
        for _ in 0..3 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let details = accessible_node(&output, "Import details")
            .1
            .bounds()
            .unwrap();
        let save = accessible_node(&output, "Save stretched sound")
            .1
            .bounds()
            .unwrap();
        let footer = accessible_node(&output, "Time Stretcher status")
            .1
            .bounds()
            .unwrap();
        assert!(
            details.y0 > save.y1,
            "Recovery control overlaps the Save border at {size:?}: {details:?} / {save:?}"
        );
        assert!(
            details.x0 >= footer.x0
                && details.x1 <= footer.x1
                && details.y0 >= footer.y0
                && details.y1 <= footer.y1,
            "Recovery control leaves the status footer at {size:?}: {details:?} / {footer:?}"
        );
        let help = accessible_node(&output, "Time Stretcher help")
            .1
            .bounds()
            .unwrap();
        let position = Pos2::new(
            (help.x0 + help.x1) as f32 / 2.0,
            (help.y0 + help.y1) as f32 / 2.0,
        );
        pointer(&ctx, &mut app, position, true, egui::Modifiers::NONE);
        pointer(&ctx, &mut app, position, false, egui::Modifiers::NONE);
        assert!(app.help.stretch);
        for _ in 0..3 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let label = "Original BPM describes the recording. Target BPM sets the result to 120, 125, 130, 135, 140, 160, or 180 BPM.";
        let bounds = text_rect(&output, label);
        assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(bounds));
    }
}
