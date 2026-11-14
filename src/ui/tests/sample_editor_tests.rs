//! Sample editor: readiness, native fields, compact layout and trim.

use super::*;

#[test]
fn sample_editor_readiness_keeps_actions_visible_and_offers_missing_source_recovery() {
    for (available, missing, frames, start, end) in [
        (false, false, 0, 0, 0),
        (false, true, 0, 0, 0),
        (true, false, 0, 0, 0),
        (true, false, 32, 0, 32),
        (true, false, 32, 17, 17),
        (true, false, 32, 18, 17),
        (true, false, 32, 31, 32),
    ] {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        ctx.enable_accesskit();
        ctx.style_mut(|style| {
            style.interaction.tooltip_delay = 0.0;
            style.animation_time = 0.0;
        });
        let ready = available && !missing && frames > 0 && start < end && end <= frames;
        let range = format!("{start}..{end}");
        let (mut open, mut start, mut end, mut pitch) = (true, start, end, 0.0);
        let mut render = |events| {
            let mut action = sample_editor::Action::None;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    action = sample_editor::show(
                        ctx,
                        &mut open,
                        sample_editor::Source {
                            name: "Source",
                            frames,
                            sample_rate: 1000,
                            available,
                            missing,
                        },
                        &mut start,
                        &mut end,
                        &mut pitch,
                    );
                },
            );
            (output, action)
        };
        for _ in 0..3 {
            render(vec![]);
        }
        let (mut output, _) = render(vec![]);
        for label in ["Listen edit", "Save edited sound"] {
            assert_eq!(
                !accessible_node(&output, label).1.is_disabled(),
                ready,
                "{label}: available={available}, missing={missing}, frames={frames}, {range}"
            );
        }
        accessible_node(&output, "Sample editor status");
        let actions = if missing {
            assert_full_recovery_caption(&output);
            vec![("Import details", sample_editor::Action::Details)]
        } else if ready {
            vec![
                ("Listen edit", sample_editor::Action::Listen),
                ("Save edited sound", sample_editor::Action::Save),
            ]
        } else {
            vec![]
        };
        for (label, expected) in actions {
            let target = accessible_node(&output, label).0;
            render(vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::Focus,
                    target,
                    data: None,
                },
            )]);
            let (_, action) = render(vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)]);
            assert_eq!(
                std::mem::discriminant(&action),
                std::mem::discriminant(&expected),
                "{label} must return its action without performing audio or file work in the view."
            );
            (output, _) = render(vec![]);
        }
        if !ready {
            let reason = if missing {
                "This sound is missing and plays silence. Open Import details to inspect or resolve it."
            } else if !available || frames == 0 {
                "Select a nonempty sound or clip before editing."
            } else {
                "End frame must be greater than Start frame and within the source."
            };
            let copies = |output: &egui::FullOutput| {
                output
                    .shapes
                    .iter()
                    .filter(|shape| {
                        matches!(&shape.shape, egui::Shape::Text(text)
                    if text.galley.job.text == reason
                        && shape.clip_rect.contains_rect(text.visual_bounding_rect()))
                    })
                    .count()
            };
            let footer_copies = copies(&output);
            assert!(
                footer_copies > 0,
                "The status footer must explain unavailable processing."
            );
            for label in ["Listen edit", "Save edited sound"] {
                let center = effects_control_rect(&output, label).center();
                render(vec![egui::Event::PointerMoved(center)]);
                let mut shown = false;
                for _ in 0..4 {
                    let (hovered, action) = render(vec![]);
                    assert!(matches!(action, sample_editor::Action::None));
                    shown |= copies(&hovered) > footer_copies;
                }
                assert!(
                    shown,
                    "Disabled {label} must explain why it is unavailable on hover."
                );
            }
        }
    }
}

#[test]
fn sample_editor_native_fields_slider_and_steppers_edit_only_the_draft() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.selection.sample = Some(1);
    app.sample_edit.open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let project = app.session.project().clone();
    let unsaved = app.session.unsaved();
    let set_value = |app: &mut MixJamApp, label: &str, value: f64| {
        let output = frame(&ctx, app, vec![], egui::Modifiers::NONE);
        let target = accessible_node(&output, label).0;
        frame(
            &ctx,
            app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::SetValue,
                    target,
                    data: Some(egui::accesskit::ActionData::NumericValue(value)),
                },
            )],
            egui::Modifiers::NONE,
        );
    };
    let activate = |app: &mut MixJamApp, label: &str, modifiers: egui::Modifiers| {
        let output = frame(&ctx, app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, app, &output, label);
        frame(
            &ctx,
            app,
            vec![key_event(egui::Key::Enter, modifiers)],
            modifiers,
        );
    };
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for (label, min, max) in [
        ("Start frame", 0.0, 75599.0),
        ("End frame", 1.0, 75600.0),
        ("Pitch value", -24.0, 24.0),
        ("Pitch (semitones)", -24.0, 24.0),
    ] {
        let node = accessible_node(&output, label).1;
        assert_eq!(node.min_numeric_value(), Some(min), "{label}");
        assert_eq!(node.max_numeric_value(), Some(max), "{label}");
    }
    set_value(&mut app, "Start frame", 17.0);
    set_value(&mut app, "End frame", 19.0);
    assert_eq!((app.sample_edit.start, app.sample_edit.end), (17, 19));
    activate(&mut app, "Increase End frame", egui::Modifiers::NONE);
    assert_eq!(app.sample_edit.end, 20);
    activate(&mut app, "Decrease Start frame", egui::Modifiers::SHIFT);
    assert_eq!(
        app.sample_edit.start, 0,
        "Shift frame stepping clamps at the source boundary."
    );
    activate(&mut app, "Increase Start frame", egui::Modifiers::NONE);
    assert_eq!(app.sample_edit.start, 1);
    set_value(&mut app, "Start frame", 100000.0);
    set_value(&mut app, "End frame", 0.0);
    assert_eq!((app.sample_edit.start, app.sample_edit.end), (75599, 1));
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for label in ["Listen edit", "Save edited sound"] {
        assert!(
            accessible_node(&output, label).1.is_disabled(),
            "A reversed trim must remain editable without allowing audio processing."
        );
    }
    set_value(&mut app, "Start frame", 0.0);
    set_value(&mut app, "End frame", 75600.0);
    set_value(&mut app, "Pitch value", 3.5);
    assert_eq!(app.sample_edit.pitch, 3.5);
    activate(&mut app, "Increase Pitch", egui::Modifiers::NONE);
    assert!((app.sample_edit.pitch - 3.6).abs() < 0.0001);
    activate(&mut app, "Decrease Pitch", egui::Modifiers::SHIFT);
    assert!((app.sample_edit.pitch - 2.6).abs() < 0.0001);
    set_value(&mut app, "Pitch (semitones)", -12.0);
    assert_eq!(app.sample_edit.pitch, -12.0);
    set_value(&mut app, "Pitch value", 100.0);
    assert_eq!(app.sample_edit.pitch, 24.0);
    activate(&mut app, "Increase Pitch", egui::Modifiers::NONE);
    assert_eq!(app.sample_edit.pitch, 24.0);
    set_value(&mut app, "Pitch value", -100.0);
    assert_eq!(app.sample_edit.pitch, -24.0);
    activate(&mut app, "Decrease Pitch", egui::Modifiers::NONE);
    assert_eq!(app.sample_edit.pitch, -24.0);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Pitch value");
    frame(
        &ctx,
        &mut app,
        vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("-4.5678".into()),
        ],
        egui::Modifiers::COMMAND,
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(
        app.sample_edit.pitch, -4.5678,
        "The amber readout supports exact keyboard entry."
    );
    assert_eq!(app.session.project(), &project);
    assert_eq!(app.session.unsaved(), unsaved);
}

#[test]
fn sample_editor_compact_layout_uses_complete_assets_and_preserves_collapsed_edits() {
    let size = Vec2::new(960.0, 720.0);
    let viewport = Rect::from_min_size(Pos2::ZERO, size);
    for density in themed_densities() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        ctx.set_pixels_per_point(density);
        ctx.style_mut(|style| style.animation_time = 0.0);
        app.selection.sample = Some(1);
        app.sample_edit.open = true;
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        app.sample_edit.start = 123;
        app.sample_edit.end = 34567;
        app.sample_edit.pitch = -3.56789;
        let before = (
            app.sample_edit.start,
            app.sample_edit.end,
            app.sample_edit.pitch,
        );
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let window = effects_control_rect(&output, "SAMPLE EDITOR");
        let panel = window;
        assert_folded_assets(&ctx, &output, density, "Granite");
        assert!(viewport.contains_rect(window));
        let footer = effects_control_rect(&output, "Sample editor status");
        assert!(window.contains_rect(footer));
        let controls = [
            "Start frame",
            "End frame",
            "Pitch (semitones)",
            "Pitch value",
            "Listen edit",
            "Save edited sound",
            "Collapse Sample editor",
            "Close Sample editor",
        ];
        let bounds = controls.map(|label| effects_control_rect(&output, label));
        for (label, bounds) in controls.iter().zip(bounds) {
            assert!(panel.contains_rect(bounds), "{label} must fit the skin.");
            assert!(!bounds.intersects(footer), "{label} must not touch status.");
        }
        let start_value = effects_control_rect(&output, "Start frame");
        let end_value = effects_control_rect(&output, "End frame");
        assert!(
            (start_value.width() - end_value.width()).abs() < 0.1,
            "Frame fields must share a width."
        );
        assert!(
            (start_value.center().y - end_value.center().y).abs() < 0.1,
            "Frame values must share a baseline."
        );
        for name in ["Start frame", "End frame", "Pitch value"] {
            let value = accessible_node(&output, name).1.value().unwrap();
            let container = effects_control_rect(&output, name);
            let glyph = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if text.galley.job.text == value
                            && container.contains(text.visual_bounding_rect().center()) =>
                    {
                        Some(text.visual_bounding_rect())
                    }
                    _ => None,
                })
                .expect("Numeric field must paint its value inside its own container.");
            let offset = (glyph.center() - container.center()).abs() * density;
            assert!(
                offset.x <= 1.0 && offset.y <= 1.0,
                "{name} glyph offset {offset:?}"
            );
        }
        let pitch_value = effects_control_rect(&output, "Pitch value");
        let pitch_rail = effects_control_rect(&output, "Pitch (semitones)");
        assert!(
            (pitch_value.center().y - pitch_rail.center().y).abs() < 0.6,
            "Pitch rail and value must align vertically."
        );
        for (index, first) in bounds.iter().enumerate() {
            for (second, name) in bounds.iter().zip(controls).skip(index + 1) {
                assert!(
                    !first.intersects(*second),
                    "Controls must not overlap: {} and {name}.",
                    controls[index]
                );
            }
        }
        for label in [
            "SAMPLE EDITOR",
            "Start frame",
            "End frame",
            "Pitch (semitones)",
            "Listen edit",
            "Save edited sound",
        ] {
            assert!(
                panel.contains_rect(text_rect(&output, label)),
                "{label} must remain readable inside the panel."
            );
        }
        for label in ["Listen edit", "Save edited sound"] {
            assert!(
                effects_control_rect(&output, label)
                    .shrink(2.0)
                    .contains_rect(effects_text_mesh_bounds(&output, label)),
                "{label} needs clear space inside its button."
            );
        }
        focus_control(&ctx, &mut app, &output, "Collapse Sample editor");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        for _ in 0..3 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let collapsed = effects_control_rect(&output, "SAMPLE EDITOR");
        assert_eq!(collapsed.min, window.min);
        assert_eq!(collapsed.width(), window.width());
        assert!((collapsed.height() - 44.0).abs() < 0.1);
        for label in [
            "Start frame",
            "End frame",
            "Pitch value",
            "Listen edit",
            "Sample editor status",
        ] {
            assert!(
                !output
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some(label)),
                "{label} must hide while collapsed."
            );
        }
        assert_eq!(
            (
                app.sample_edit.start,
                app.sample_edit.end,
                app.sample_edit.pitch
            ),
            before
        );
        focus_control(&ctx, &mut app, &output, "Expand Sample editor");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        for _ in 0..3 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        assert_eq!(effects_control_rect(&output, "SAMPLE EDITOR"), window);
        assert_eq!(
            (
                app.sample_edit.start,
                app.sample_edit.end,
                app.sample_edit.pitch
            ),
            before
        );
        focus_control(&ctx, &mut app, &output, "Close Sample editor");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(!app.sample_edit.open);
        assert_eq!(
            (
                app.sample_edit.start,
                app.sample_edit.end,
                app.sample_edit.pitch
            ),
            before
        );
    }
}

#[test]
fn collapsed_sample_editor_retains_trim_when_palette_changes_away_and_back() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let a = app.session.library().samples[0].clone();
    let mut b = a.clone();
    b.id = 2;
    b.name = "Other source".into();
    app.session = Session::headless(
        Arc::new(Library::fixture(vec![a, b])),
        app.session.project().clone(),
    )
    .unwrap();
    app.selection.sample = Some(1);
    app.sample_edit.open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    app.sample_edit.start = 100;
    app.sample_edit.end = 1000;
    app.sample_edit.pitch = 3.25;
    let before = (
        app.sample_edit.start,
        app.sample_edit.end,
        app.sample_edit.pitch,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Collapse Sample editor");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    app.selection.sample = Some(2);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    app.selection.sample = Some(1);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        (
            app.sample_edit.start,
            app.sample_edit.end,
            app.sample_edit.pitch
        ),
        before
    );
    focus_control(&ctx, &mut app, &output, "Expand Sample editor");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        (
            app.sample_edit.start,
            app.sample_edit.end,
            app.sample_edit.pitch
        ),
        before
    );
}

#[test]
fn sample_editor_trim_uses_stereo_source_frames_and_an_exclusive_end() {
    let (context, mut app, dir) = sample_conflicts_fixture("sample-editor-trim");
    app.session.confirm_sample_conflicts().unwrap();
    app.conflicts.open = false;
    app.selection.sample = Some(1_000_000);
    app.sample_edit.open = true;
    let path = app.session.sample_conflicts()[0].candidates[0].clone();
    Pcm {
        sample_rate: 1000,
        channels: 2,
        samples: (0..2000)
            .flat_map(|frame| {
                let value = frame as f32 / 4000.0;
                [value, -value]
            })
            .collect(),
    }
    .write_wav(&path)
    .unwrap();
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!((app.sample_edit.start, app.sample_edit.end), (0, 2000));
    app.sample_edit.start = 17;
    app.sample_edit.end = 19;
    app.sample_edit.pitch = 0.0;
    let source = app.session.source_pcm(app.source_id()).unwrap();
    let edited = app.edited_pcm().unwrap();
    assert_eq!(
        (edited.channels, edited.sample_rate, edited.frames()),
        (2, 1000, 2)
    );
    assert_eq!(
        edited.samples,
        source.samples[34..38],
        "Start is inclusive, end is exclusive, and each stereo frame includes both channels."
    );
    app.sample_edit.start = 1999;
    app.sample_edit.end = 2000;
    assert_eq!(
        app.edited_pcm().unwrap().samples,
        source.samples[3998..4000]
    );
    for (start, end) in [(10, 10), (11, 10), (0, 2001)] {
        app.sample_edit.start = start;
        app.sample_edit.end = end;
        assert!(
            app.edited_pcm().is_err(),
            "Invalid trim {start}..{end} must not render."
        );
    }
    app.sample_edit.start = 0;
    app.sample_edit.end = 2000;
    app.sample_edit.pitch = 12.0;
    assert_eq!(
        app.edited_pcm().unwrap().frames(),
        1000,
        "Pitch changes source speed and duration after trimming."
    );
    app.sample_edit.pitch = -12.0;
    assert_eq!(app.edited_pcm().unwrap().frames(), 4000);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sample_editor_resets_its_range_when_a_conflict_replaces_the_same_source_id() {
    let (context, mut app, dir) = sample_conflicts_fixture("edit-range");
    app.session.confirm_sample_conflicts().unwrap();
    app.conflicts.open = false;
    app.selection.sample = Some(1_000_000);
    app.sample_edit.open = true;
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!((app.sample_edit.start, app.sample_edit.end), (0, 2000));
    app.sample_edit.start = 200;
    app.sample_edit.end = 600;
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!((app.sample_edit.start, app.sample_edit.end), (200, 600));

    app.session.resolve_sample_conflict(0, 1).unwrap();
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.source_id(), Some(1_000_000));
    assert_eq!((app.sample_edit.start, app.sample_edit.end), (0, 2000));

    app.sample_edit.start = 300;
    app.sample_edit.end = 800;
    app.history_step(false);
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!((app.sample_edit.start, app.sample_edit.end), (0, 2000));
    std::fs::remove_dir_all(dir).unwrap();
}
