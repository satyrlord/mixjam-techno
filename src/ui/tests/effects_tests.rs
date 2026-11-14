//! Effects tool window: skin, switches, knobs, saving and missing audio.

use super::*;

#[test]
fn effects_disabled_actions_explain_themselves_on_hover() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
    app.effects.open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for label in ["Listen", "Save sound"] {
        assert!(accessible_node(&output, label).1.is_disabled());
        let center = effects_control_rect(&output, label).center();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(center)],
            egui::Modifiers::NONE,
        );
        let mut shown = false;
        for _ in 0..4 {
            shown |= frame(&ctx, &mut app, vec![], egui::Modifiers::NONE)
                .shapes
                .iter()
                .any(|shape| {
                    matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "Select an available sound \
                                                                    to process it.")
                });
        }
        assert!(
            shown,
            "Disabled {label} must show why it is unavailable on hover."
        );
    }
}

#[test]
fn effects_compact_skin_fits_the_minimum_window_and_collapses_without_losing_settings() {
    for density in themed_densities() {
        let (ctx, mut app) = fixture();
        ctx.set_pixels_per_point(density);
        ctx.enable_accesskit();
        ctx.style_mut(|style| style.animation_time = 0.0);
        app.selection.sample = Some(1);
        app.effects.open = true;
        app.effects.state.filter = Some(crate::effects::Filter {
            cutoff_hz: 4567.0,
            ..Default::default()
        });
        app.effects.state.echo = Some(Default::default());
        app.effects.state.robotize = Some(Default::default());
        app.effects.state.dirty = 0.375;
        let before = app.effects.state.clone();
        let size = Vec2::new(960.0, 720.0);
        let viewport = Rect::from_min_size(Pos2::ZERO, size);
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let window = effects_control_rect(&output, "EFFECT STUDIO");
        let panel = window;
        assert!(
            viewport.contains_rect(window),
            "The complete module must fit."
        );
        let footer = effects_control_rect(&output, "Effect Studio status");
        assert!(window.contains_rect(footer));
        assert_folded_assets(&ctx, &output, density, "Red");
        assert_eq!(
            accessible_node(&output, "Source sound").1.value(),
            Some("good grasp 2")
        );
        for name in [
            "Source sound",
            "Cutoff",
            "Resonance",
            "Overdrive",
            "Filter volume",
            "Delay (beats)",
            "Feedback",
            "Echo volume",
            "Color",
            "Robotize volume",
            "Dirty distortion",
            "Noise distortion",
            "Output volume",
            "Listen",
            "Save sound",
            "Reset",
        ] {
            let bounds = effects_control_rect(&output, name);
            assert!(
                panel.contains_rect(bounds),
                "{name} must fit the compact skin."
            );
            assert!(
                !bounds.intersects(footer),
                "{name} must not overlap status."
            );
        }
        for text in [
            "Cutoff",
            "Resonance",
            "Overdrive",
            "Filter volume",
            "Delay (beats)",
            "Feedback",
            "Echo volume",
            "Color",
            "Robotize volume",
            "Dirty distortion",
            "Noise distortion",
            "Output volume",
            "Listen",
            "Save sound",
            "Reset",
        ] {
            assert!(
                panel.contains_rect(text_rect(&output, text)),
                "{text} must remain readable."
            );
        }
        for name in [
            "Cutoff",
            "Resonance",
            "Overdrive",
            "Filter volume",
            "Delay (beats)",
            "Feedback",
            "Echo volume",
            "Color",
            "Robotize volume",
            "Dirty distortion",
            "Noise distortion",
            "Output volume",
        ] {
            let slot = effects_control_rect(&output, &format!("{name} value"));
            // Only the window's own text, painted from its title on: the
            // arrangement's track numbers can lie beneath the slot.
            let first = output
                .shapes
                .iter()
                .position(|shape| match &shape.shape {
                    egui::Shape::Text(text) => text.galley.job.text == "EFFECT STUDIO",
                    _ => false,
                })
                .expect("The window paints its title.");
            let number = output.shapes[first..]
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text.parse::<f64>().is_ok() => {
                        let bounds = text.galley.mesh_bounds.translate(text.pos.to_vec2());
                        slot.contains(bounds.center()).then_some(bounds)
                    }
                    _ => None,
                })
                .reduce(|bounds, next| bounds.union(next))
                .unwrap_or_else(|| panic!("Missing numeric glyphs for {name}."));
            let caption = effects_text_mesh_bounds(&output, name);
            let gap = caption.top() - number.bottom();
            assert!(
                gap >= 4.0,
                "{name} needs 4 logical pixels between number and caption glyphs; measured {gap:.3}, number={number:?}, caption={caption:?}."
            );
        }
        for (section, controls) in [
            ("Robotize", ["Color", "Robotize volume"].as_slice()),
            (
                "Distortion / output",
                ["Dirty distortion", "Noise distortion", "Output volume"].as_slice(),
            ),
        ] {
            let heading = effects_text_mesh_bounds(&output, section);
            for name in controls {
                let knob = effects_control_rect(&output, name).expand(1.0);
                // Aero and Vintage print tick marks; Studio draws a
                // value arc.
                let mut marks = 0;
                for shape in &output.shapes {
                    let bounds = match &shape.shape {
                        egui::Shape::LineSegment { points, stroke }
                            if stroke.width <= 1.1
                                && points[0].distance(points[1]) <= 4.0
                                && points.iter().all(|point| knob.contains(*point)) =>
                        {
                            Rect::from_two_pos(points[0], points[1]).expand(stroke.width / 2.0)
                        }
                        egui::Shape::Path(path)
                            if path.points.len() > 2
                                && path.points.iter().all(|point| knob.contains(*point)) =>
                        {
                            path.visual_bounding_rect()
                        }
                        _ => continue,
                    };
                    marks += 1;
                    assert!(
                        !heading.intersects(bounds),
                        "{section} heading glyphs must not touch {name} scale marks: heading={heading:?}, mark={bounds:?}."
                    );
                }
                assert!(marks > 0, "{name} must have a native scale to check.");
            }
        }
        assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == "Help")));
        focus_control(&ctx, &mut app, &output, "Collapse Effect Studio");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let collapsed = effects_control_rect(&output, "EFFECT STUDIO");
        assert_eq!(collapsed.min, window.min, "Collapse must stay in place.");
        assert!((collapsed.height() - 44.0).abs() < 0.1);
        assert_eq!(collapsed.width(), window.width());
        for hidden in ["Source sound", "Cutoff", "Listen", "Effect Studio status"] {
            assert!(
                !output
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some(hidden)),
                "{hidden} must hide when collapsed."
            );
        }
        focus_control(&ctx, &mut app, &output, "Expand Effect Studio");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(effects_control_rect(&output, "EFFECT STUDIO"), window);
        assert_eq!(
            app.effects.state, before,
            "Collapse must preserve processing settings."
        );
        focus_control(&ctx, &mut app, &output, "Effect Studio help");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(
            app.help.effects,
            "The Help icon must support keyboard activation."
        );
        app.help.effects = false;
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Collapse Effect Studio");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Close Effect Studio");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(!app.effects.open, "The collapsed module must still close.");
        assert_eq!(app.effects.state, before);
    }
}

#[test]
fn effects_switches_discard_settings_and_disable_only_their_own_controls() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.effects.open = true;
    app.effects.state = EffectsState {
        filter: Some(crate::effects::Filter {
            cutoff_hz: 4567.0,
            ..Default::default()
        }),
        echo: Some(crate::effects::Echo {
            feedback: 0.7,
            ..Default::default()
        }),
        robotize: Some(crate::effects::Robotize {
            frequency_hz: 333.0,
            ..Default::default()
        }),
        dirty: 0.25,
        noise: 0.125,
        volume: 0.75,
    };
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for name in ["Listen", "Save sound"] {
        assert!(
            accessible_node(&output, name).1.is_disabled(),
            "Processing needs a selected source."
        );
    }
    app.selection.sample = Some(1);
    for (switch, knob) in [
        ("Filter", "Cutoff"),
        ("Echo", "Feedback"),
        ("Robotize", "Color"),
    ] {
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(!accessible_node(&output, knob).1.is_disabled());
        focus_control(&ctx, &mut app, &output, switch);
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let (target, node) = accessible_node(&output, knob);
        assert!(node.is_disabled());
        assert!(
            accessible_node(&output, &format!("{knob} value"))
                .1
                .is_disabled()
        );
        let before = app.effects.state.clone();
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::SetValue,
                    target,
                    data: Some(egui::accesskit::ActionData::NumericValue(0.9)),
                },
            )],
            egui::Modifiers::NONE,
        );
        assert_eq!(
            app.effects.state, before,
            "A disabled control must not change effect state."
        );
        for name in [
            "Dirty distortion",
            "Noise distortion",
            "Output volume",
            "Listen",
            "Save sound",
            "Reset",
        ] {
            assert!(
                !accessible_node(&output, name).1.is_disabled(),
                "{name} must remain available."
            );
        }
        focus_control(&ctx, &mut app, &output, switch);
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
    }
    assert_eq!(app.effects.state.filter, Some(Default::default()));
    assert_eq!(app.effects.state.echo, Some(Default::default()));
    assert_eq!(app.effects.state.robotize, Some(Default::default()));
    assert_eq!(
        (
            app.effects.state.dirty,
            app.effects.state.noise,
            app.effects.state.volume
        ),
        (0.25, 0.125, 0.75)
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Reset");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.effects.state, EffectsState::default());
    assert_eq!(
        app.effects.feedback.message.as_deref(),
        Some("Effects reset. Filter, Echo, and Robotize are off.")
    );
}

#[test]
fn effects_knob_input_is_stable_bounded_and_accessible() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.effects.open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let origin = effects_control_rect(&output, "Output volume").center();
    for _ in 0..12 {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(origin)],
            egui::Modifiers::NONE,
        );
        assert_eq!(
            app.effects.state.volume, 1.0,
            "Hover must not change a knob."
        );
    }
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, 20.0),
            modifiers: egui::Modifiers::NONE,
        }],
        egui::Modifiers::NONE,
    );
    assert!((app.effects.state.volume - 1.01).abs() < 0.0001);
    for _ in 0..20 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            (app.effects.state.volume - 1.01).abs() < 0.0001,
            "One wheel event must apply once."
        );
    }
    pointer(&ctx, &mut app, origin, true, egui::Modifiers::NONE);
    for offset in [15.0, 30.0, 15.0, 0.0] {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(origin + Vec2::new(offset, 0.0))],
            egui::Modifiers::NONE,
        );
        let expected = 1.01 + offset / 75.0;
        assert!(
            (app.effects.state.volume - expected).abs() < 0.0001,
            "Drag must track motion and reversal."
        );
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert!(
                (app.effects.state.volume - expected).abs() < 0.0001,
                "A stopped pointer must hold its value."
            );
        }
    }
    pointer(&ctx, &mut app, origin, false, egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (target, node) = accessible_node(&output, "Output volume");
    assert_eq!(node.min_numeric_value(), Some(0.0));
    assert_eq!(node.max_numeric_value(), Some(2.0));
    focus_control(&ctx, &mut app, &output, "Output volume");
    for (key, modifiers, expected) in [
        (egui::Key::Home, egui::Modifiers::NONE, 0.0),
        (egui::Key::ArrowUp, egui::Modifiers::NONE, 0.01),
        (egui::Key::ArrowUp, egui::Modifiers::SHIFT, 0.011),
        (egui::Key::End, egui::Modifiers::NONE, 2.0),
        (egui::Key::ArrowRight, egui::Modifiers::NONE, 2.0),
    ] {
        frame(&ctx, &mut app, vec![key_event(key, modifiers)], modifiers);
        assert!(
            (app.effects.state.volume - expected).abs() < 0.0001,
            "Keyboard adjustment must respect the range."
        );
    }
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::SetValue,
                target,
                data: Some(egui::accesskit::ActionData::NumericValue(50.0)),
            },
        )],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.effects.state.volume, 2.0);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Decrement,
                target,
                data: None,
            },
        )],
        egui::Modifiers::NONE,
    );
    assert!((app.effects.state.volume - 1.99).abs() < 0.0001);
    click(&ctx, &mut app, origin, egui::Modifiers::NONE);
    click(&ctx, &mut app, origin, egui::Modifiers::NONE);
    assert_eq!(
        app.effects.state.volume, 1.0,
        "Double-click must restore the control default."
    );
}

#[test]
fn effects_logarithmic_knobs_step_along_the_dial() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.effects.open = true;
    app.effects.state.filter = Some(Default::default());
    app.effects.state.robotize = Some(crate::effects::Robotize {
        frequency_hz: 1.0,
        ..Default::default()
    });
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (target, _) = accessible_node(&output, "Cutoff");
    focus_control(&ctx, &mut app, &output, "Cutoff");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(
        app.effects.state.filter.as_ref().unwrap().cutoff_hz,
        2143.0,
        "A key must turn Cutoff 1% of its dial."
    );
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Decrement,
                target,
                data: None,
            },
        )],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.effects.state.filter.as_ref().unwrap().cutoff_hz, 2000.0);
    for key in [egui::Key::End, egui::Key::ArrowUp] {
        frame(
            &ctx,
            &mut app,
            vec![key_event(key, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
    }
    assert_eq!(
        app.effects.state.filter.as_ref().unwrap().cutoff_hz,
        20000.0
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Color");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(
        app.effects.state.robotize.as_ref().unwrap().frequency_hz,
        2.0,
        "Rounding must not hold the low end in place."
    );
}

#[test]
fn effects_missing_audio_keeps_processing_disabled_and_details_reachable() {
    let (ctx, mut app) = missing_fixture();
    ctx.enable_accesskit();
    app.select_clip(1, false);
    app.effects.open = true;
    for _ in 0..3 {
        frame_at_size(
            &ctx,
            &mut app,
            vec![],
            egui::Modifiers::NONE,
            Vec2::new(960.0, 720.0),
        );
    }
    let output = frame_at_size(
        &ctx,
        &mut app,
        vec![],
        egui::Modifiers::NONE,
        Vec2::new(960.0, 720.0),
    );
    for name in ["Listen", "Save sound"] {
        assert!(accessible_node(&output, name).1.is_disabled());
    }
    // The status bar has its own Import details; Effect Studio's sits in
    // the window's status line.
    let status = effects_control_rect(&output, "Effect Studio status");
    assert!(
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .filter(|(_, node)| node.label() == Some("Import details"))
            .filter_map(|(_, node)| node.bounds())
            .any(|bounds| status.contains_rect(Rect::from_min_max(
                Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
                Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
            )))
    );
    assert_full_recovery_caption(&output);
    text_rect(
        &output,
        "Missing audio. Open Import details to resolve the sound.",
    );
    focus_control(&ctx, &mut app, &output, "Import details");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(app.sample_import.details_open);
}

#[test]
fn effects_save_feedback_preserves_source_tempo_and_selects_the_saved_effect() {
    let (ctx, mut app) = fixture();
    let dir = std::env::temp_dir().join(format!("mixjam-ui-effects-save-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut sample = app.session.library().samples[0].clone();
    sample.source_bpm = 160.0;
    app.session = Session::headless(
        Arc::new(Library::fixture(vec![sample])),
        app.session.project().clone(),
    )
    .unwrap()
    .with_settings_root(dir.join("settings"));
    app.selection.sample = Some(1);
    app.effects.open = true;
    app.effects.state.dirty = 0.25;
    let before = app.effects.state.clone();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let source_bpm = app.source_bpm();
    let request = || FileRequest::SaveSound {
        pcm: Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.25; 1000],
        },
        name: "effect result".into(),
        group: "Effect",
        source_bpm,
        stretch: None,
    };
    app.finish_path(request(), None);
    assert_eq!(
        app.effects.feedback.message.as_deref(),
        Some("Save cancelled.")
    );
    assert!(!app.effects.feedback.error);
    assert_eq!(app.selection.sample, Some(1));
    let path = dir.join("result.wav");
    app.finish_path(request(), Some(path.clone()));
    let saved = app
        .selection
        .sample
        .expect("Saving must select the new Effect sound.");
    assert_ne!(saved, 1);
    assert_eq!(app.sounds.group.as_deref(), Some("Effect"));
    assert_eq!(app.source_bpm(), 160.0);
    assert_eq!(app.effects.state, before);
    assert_eq!(app.effects.feedback.source, Some(saved));
    assert_eq!(
        app.effects.feedback.message.as_deref(),
        Some("Saved result.wav to Effect.")
    );
    assert!(!app.effects.feedback.error);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "Saved result.wav to Effect.");
    assert_eq!(crate::audio::read_wav(&path).unwrap().frames(), 1000);
    app.finish_path(request(), Some(dir.join("absent-parent/result.wav")));
    assert!(app.effects.feedback.error);
    assert!(
        app.effects
            .feedback
            .message
            .as_deref()
            .unwrap()
            .starts_with("Cannot save the sound:")
    );
    assert_eq!(app.selection.sample, Some(saved));
    assert_eq!(
        app.session.library().samples.len(),
        2,
        "A failed save must not add another source."
    );
    assert_eq!(app.effects.state, before);
    app.selection.sample = Some(1);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.effects.feedback.source, Some(1));
    assert!(
        app.effects.feedback.message.is_none(),
        "Changing source must clear the previous save receipt."
    );
    std::fs::remove_dir_all(dir).unwrap();
}
