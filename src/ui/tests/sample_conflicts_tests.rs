//! Sample conflicts and missing media: the conflict modal, missing clips and missing
//! sample details.

use super::*;

#[test]
fn sample_conflict_modal_shows_paths_and_switches_every_instance_without_closing() {
    let (context, mut app, dir) = sample_conflicts_fixture("switch");
    let before = app.session.project().clips.clone();
    let candidates = app.session.sample_conflicts()[0].candidates.clone();
    let label = format!(
        "Use match 2\n{}",
        display_paths(&candidates[1].to_string_lossy())
    );
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "SAMPLE CONFLICTS");
    text_rect(&output, "3 instances · 2 matches");
    let (_, control) = accessible_node(&output, &label);
    assert_eq!(control.role(), egui::accesskit::Role::RadioButton);
    text_rect(
        &output,
        &format!(
            "Selected match 1\n{}",
            display_paths(&candidates[0].to_string_lossy())
        ),
    );
    let choice = text_rect(&output, &label);
    click(&context, &mut app, choice.center(), egui::Modifiers::NONE);
    assert_eq!(app.session.sample_conflicts()[0].selected, 1);
    assert!(
        app.session
            .project()
            .imported_samples
            .iter()
            .all(|sample| sample.path == candidates[1])
    );
    assert_eq!(app.session.project().clips, before);
    assert!(
        app.sample_conflicts_open(),
        "Choices must remain available until the user closes the modal."
    );
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let first = text_rect(
        &output,
        &format!(
            "Use match 1\n{}",
            display_paths(&candidates[0].to_string_lossy())
        ),
    );
    click(&context, &mut app, first.center(), egui::Modifiers::NONE);
    assert_eq!(app.session.sample_conflicts()[0].selected, 0);
    assert!(
        app.session
            .project()
            .imported_samples
            .iter()
            .all(|sample| sample.path == candidates[0])
    );
    for path in candidates {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn escape_hides_unconfirmed_sample_conflicts_without_an_edit_until_reopened() {
    let (context, mut app, dir) = sample_conflicts_fixture("escape");
    let before = app.session.project().clone();
    assert!(app.sample_conflicts_open());
    frame(
        &context,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(!app.sample_conflicts_open());
    assert_eq!(app.session.project(), &before);
    assert!(!app.session.unsaved());
    assert!(!app.session.history_step(false).unwrap());
    assert!(!app.session.sample_conflicts()[0].confirmed);
    assert!(app.status.contains("not confirmed"));
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        !output.shapes.iter().any(|shape| {
            matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "SAMPLE CONFLICTS"
            )
        }),
        "A dismissed resolver must stay hidden until it is reopened."
    );
    let reopen = text_rect(&output, "Sample conflicts");
    click(&context, &mut app, reopen.center(), egui::Modifiers::NONE);
    assert!(app.sample_conflicts_open());
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let keep = text_rect(&output, "Keep choices");
    click(&context, &mut app, keep.center(), egui::Modifiers::NONE);
    assert!(app.session.sample_conflicts()[0].confirmed);
    assert!(app.session.unsaved());
    for path in &app.session.sample_conflicts()[0].candidates {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn sample_conflict_confirmation_stays_closed_and_status_can_reopen_choices() {
    let (context, mut app, dir) = sample_conflicts_fixture("confirm");
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let keep = text_rect(&output, "Keep choices");
    click(&context, &mut app, keep.center(), egui::Modifiers::NONE);
    assert!(!app.sample_conflicts_open());
    assert!(
        app.session
            .sample_conflicts()
            .iter()
            .all(|conflict| conflict.confirmed)
    );
    for _ in 0..3 {
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    }
    assert!(!app.sample_conflicts_open());
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let reopen = text_rect(&output, "Sample conflicts");
    click(&context, &mut app, reopen.center(), egui::Modifiers::NONE);
    assert!(app.sample_conflicts_open());
    frame(
        &context,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(!app.sample_conflicts_open());
    let reopened_project: Project =
        serde_json::from_str(&serde_json::to_string(app.session.project()).unwrap()).unwrap();
    let session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        reopened_project,
    )
    .unwrap();
    let reopened = MixJamApp::from_session(session, "Reopened".into(), false);
    assert!(
        !reopened.sample_conflicts_open(),
        "Confirmed choices must not warn again after reopening."
    );
    for path in &app.session.sample_conflicts()[0].candidates {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn sample_conflict_modal_keeps_actions_and_wrapped_paths_inside_supported_viewports() {
    let (context, mut app, dir) = sample_conflicts_fixture("layout");
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        for _ in 0..3 {
            frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&context, &mut app, vec![], egui::Modifiers::NONE, size);
        let viewport = Rect::from_min_size(Pos2::ZERO, size);
        for label in ["SAMPLE CONFLICTS", "Keep choices", "Play song"] {
            assert!(
                viewport.contains_rect(text_rect(&output, label)),
                "{label} lies outside {size:?}"
            );
        }
        for (index, path) in app.session.sample_conflicts()[0]
            .candidates
            .iter()
            .enumerate()
        {
            let label = format!(
                "{} match {}\n{}",
                if index == 0 { "Selected" } else { "Use" },
                index + 1,
                display_paths(&path.to_string_lossy())
            );
            assert!(viewport.contains_rect(text_rect(&output, &label)));
        }
    }
    for path in &app.session.sample_conflicts()[0].candidates {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn missing_clip_keeps_full_span_label_selection_and_history() {
    let (context, mut app) = missing_fixture();
    // The Mixer strips print the clip name as well.
    app.boards.module = BottomModule::Mastering;
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let label = text_rect(&output, "Missing: Lost lead");
    let timeline = app.score.rect.unwrap();
    let clip = app.session.project().clips[0].clone();
    let rect = app.clip_rect(&clip, timeline);
    assert!((rect.width() - (4.0 * app.score.bar_width - 1.0)).abs() < 0.01);
    assert!((rect.left() - timeline.left() - app.score.bar_width).abs() < 0.01);
    assert!(rect.contains(label.center()));
    click(&context, &mut app, rect.center(), egui::Modifiers::NONE);
    assert_eq!(app.selection.clip, Some(clip.id));
    assert_eq!(app.source_name(), "Missing: Lost lead");
    app.preview(clip.sample_id);
    assert!(
        app.status
            .contains("Cannot preview missing sample Lost lead")
    );
    app.delete_selection();
    assert!(app.session.project().clips.is_empty());
    app.history_step(false);
    assert_eq!(app.session.project().clips[0], clip);
    assert_eq!(app.session.missing_samples().len(), 1);
}

#[test]
fn missing_clip_context_menu_offers_manual_resolution_without_changing_project() {
    let (context, mut app) = missing_fixture();
    context.enable_accesskit();
    let before = app.session.project().clone();
    let pos = app
        .clip_rect(&before.clips[0], app.score.rect.unwrap())
        .center();
    for pressed in [true, false] {
        frame(
            &context,
            &mut app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
        );
    }
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "Resolve manually");
    accessible_node(&output, "Resolve manually");
    let id = before.clips[0].sample_id;
    assert_eq!(app.clip_menu.missing_sample, Some(id));
    assert_eq!(app.session.project(), &before);

    let dir = std::env::temp_dir().join(format!("mixjam-ui-resolve-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let placeholder =
        Session::headless(Arc::new(Library::fixture(vec![])), Project::default()).unwrap();
    app.session =
        std::mem::replace(&mut app.session, placeholder).with_settings_root(dir.join("settings"));
    let status = app.status.clone();
    app.finish_path(FileRequest::ResolveMissing(id), None);
    assert_eq!(app.session.project(), &before);
    assert_eq!(app.clip_menu.missing_sample, Some(id));
    assert_eq!(app.status, status);
    let bad = dir.join("bad.wav");
    std::fs::write(&bad, b"not audio").unwrap();
    app.resolve_missing_sample_with(id, &bad);
    assert_eq!(app.session.project(), &before);
    assert!(app.status_error);
    let chosen = dir.join("chosen.wav");
    Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.5; 44_100],
    }
    .write_wav(&chosen)
    .unwrap();
    app.resolve_missing_sample_with(999, &chosen);
    assert_eq!(app.session.project(), &before);
    app.resolve_missing_sample_with(id, &chosen);
    assert!(app.session.missing_samples().is_empty());
    assert_eq!(app.clip_menu.missing_sample, None);
    let resolved = &app.session.project().clips[0];
    assert_eq!(
        (
            resolved.track,
            resolved.start_tick,
            resolved.length_ticks,
            resolved.gain
        ),
        (2, TICKS_PER_BAR, 4 * TICKS_PER_BAR, 0.75)
    );
    assert!(app.session.history_step(false).unwrap());
    assert_eq!(app.session.project(), &before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn missing_metadata_keeps_original_reference_raw_fields_and_length_confidence() {
    let (_, app) = missing_fixture();
    let mut missing = app.session.missing_samples()[0].clone();
    missing
        .metadata
        .insert("Original code".into(), "LOST42".into());
    missing
        .metadata
        .insert("Saved raw flags".into(), "0x0017".into());
    let details = missing_sample_details(&missing);
    for value in [
        "Missing: Lost lead",
        "Playback: silence",
        "Native sample ID: 42",
        "132 BPM",
        "1024 ticks",
        "known",
        "Original code: LOST42",
        "Saved raw flags: 0x0017",
        "The original audio file is absent.",
    ] {
        assert!(
            details.contains(value),
            "Missing retained metadata: {value}"
        );
    }
    missing.length_inferred = true;
    missing.reference = MissingReference::External {
        path: PathBuf::from(r"C:\old sounds\Lost lead.wav"),
        origin: Some(Product::D2),
    };
    let details = missing_sample_details(&missing);
    assert!(details.contains(r"Original path: C:\old sounds\Lost lead.wav"));
    assert!(details.contains("inferred"));
    missing.reference = MissingReference::Generated {
        product: Product::D2,
        kind: "Drum".into(),
        data: serde_json::json!({"pattern": [1, 0, 1]}),
    };
    assert!(missing_sample_details(&missing).contains("\"pattern\":[1,0,1]"));
    let clip = missing_clip_details(&app.session.project().clips[0]);
    assert!(clip.contains("Track 3") && clip.contains("150 frames") && clip.contains("gain 0.75"));
}

#[test]
fn saved_source_settings_are_listed_in_import_details() {
    let (context, mut app) = fixture();
    let mut project = app.session.project().clone();
    for (key, value) in [("BoostCompressorLED", "1"), ("BoostStereoWide", "21348")] {
        project.source_settings.insert(key.into(), value.into());
    }
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    // The settings alone make Import details available.
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let button = text_rect(&output, "Import details");
    click(&context, &mut app, button.center(), egui::Modifiers::NONE);
    assert!(app.sample_import.details_open);
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let header = text_rect(&output, "Saved settings (2)");
    click(&context, &mut app, header.center(), egui::Modifiers::NONE);
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "BoostCompressorLED: 1\nBoostStereoWide: 21348");
}

#[test]
fn missing_sample_details_are_accessible_and_processing_explains_silence() {
    let (context, mut app) = missing_fixture();
    context.enable_accesskit();
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    accessible_node(&output, "Import details");
    text_rect(&output, "1 missing · silent");
    let button = text_rect(&output, "Import details");
    click(&context, &mut app, button.center(), egui::Modifiers::NONE);
    assert!(app.sample_import.details_open);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    assert!(output.shapes.iter().any(|shape| matches!(
        &shape.shape,
        egui::Shape::Text(text) if text.galley.job.text.contains("Native sample ID: 42")
    )));
    app.sample_import.details_open = false;
    app.select_clip(1, false);
    app.sample_edit.open = true;
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    for label in ["Listen edit", "Save edited sound"] {
        assert!(
            accessible_node(&output, label).1.is_disabled(),
            "Missing source actions remain visible and explain why they are unavailable."
        );
    }
    accessible_node(&output, "Import details");
}
