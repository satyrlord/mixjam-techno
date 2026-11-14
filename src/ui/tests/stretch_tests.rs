//! Stretch tool window controls: knobs, rotaries, layout, targets and wheel input.

use super::*;

#[test]
fn stretch_hover_does_not_change_tempo_and_wheel_input_changes_it_once() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.selection.sample = Some(1);
    app.stretch.open = true;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for label in ["Original BPM knob", "Target BPM"] {
        let modifiers = if label == "Target BPM" {
            egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::NONE
        };
        let bounds = accessible_node(&output, label).1.bounds().unwrap();
        let position = Pos2::new(
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        );
        let before = (app.stretch.source_bpm, app.stretch.target_bpm);
        for _ in 0..12 {
            output = frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(position)],
                egui::Modifiers::NONE,
            );
            assert_eq!(
                (app.stretch.source_bpm, app.stretch.target_bpm),
                before,
                "Hovering {label} must not change its value."
            );
        }
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, 20.0),
                modifiers,
            }],
            modifiers,
        );
        let expected = if label == "Original BPM knob" {
            (141.0, 140.0)
        } else {
            (141.0, 160.0)
        };
        assert_eq!((app.stretch.source_bpm, app.stretch.target_bpm), expected);
        for _ in 0..20 {
            output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_eq!(
                (app.stretch.source_bpm, app.stretch.target_bpm),
                expected,
                "A wheel event must not keep changing {label} on later frames."
            );
        }
    }
}

#[test]
fn stretch_knob_drag_tracks_total_motion_and_holds_when_pointer_stops() {
    let density = 1.0;
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.selection.sample = Some(1);
    app.stretch.open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let label = "Original BPM knob";
    let bounds = accessible_node(&output, label).1.bounds().unwrap();
    let origin = Pos2::new(
        (bounds.x0 + bounds.x1) as f32 / 2.0,
        (bounds.y0 + bounds.y1) as f32 / 2.0,
    );
    let scale = 150.0 / 270.0;
    // Pause and reverse within the same gesture, then start a new one.
    for offsets in [
        [12.0, 24.0, 36.0, 24.0, 0.0],
        [-12.0, -24.0, -36.0, -24.0, 0.0],
    ] {
        pointer(&ctx, &mut app, origin, true, egui::Modifiers::NONE);
        for offset in offsets {
            let position = origin + Vec2::new(offset * scale, 0.0);
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(position)],
                egui::Modifiers::NONE,
            );
            let expected = 140.0 + f64::from(offset);
            assert_eq!(
                app.stretch.source_bpm, expected,
                "{label} at {density}x, offset {offset}"
            );
            for _ in 0..3 {
                frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
                assert_eq!(
                    app.stretch.source_bpm, expected,
                    "Holding {label} still must retain its value."
                );
            }
        }
        pointer(&ctx, &mut app, origin, false, egui::Modifiers::NONE);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.stretch.source_bpm, 140.0);
    }

    // The target slider snaps to the nearest notch during a drag.
    let bounds = accessible_node(&output, "Target BPM").1.bounds().unwrap();
    let notch = |index: usize| {
        let (left, right) = (bounds.x0 as f32 + 8.0, bounds.x1 as f32 - 8.0);
        Pos2::new(
            left + (right - left) * index as f32 / 6.0,
            bounds.y0 as f32 + 10.0,
        )
    };
    pointer(&ctx, &mut app, notch(4), true, egui::Modifiers::NONE);
    for (position, expected) in [
        (notch(6), 180.0),
        (notch(0), 120.0),
        (notch(2) + Vec2::new(3.0, 0.0), 130.0),
        (notch(4) - Vec2::new(4.0, 0.0), 140.0),
    ] {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(position)],
            egui::Modifiers::NONE,
        );
        assert_eq!(app.stretch.target_bpm, expected, "Target BPM at {density}x");
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            app.stretch.target_bpm, expected,
            "Holding the slider still must retain its value."
        );
    }
    pointer(&ctx, &mut app, notch(4), false, egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.stretch.target_bpm, 140.0);
}

#[test]
fn stretch_opens_with_source_tempo_and_accepts_accessible_adjustments() {
    let (ctx, mut app) = fixture();
    let mut sample = app.session.library().samples[0].clone();
    sample.source_bpm = 160.0;
    app.session = Session::headless(
        Arc::new(Library::fixture(vec![sample])),
        app.session.project().clone(),
    )
    .unwrap();
    app.selection.sample = Some(1);
    ctx.enable_accesskit();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let bounds = accessible_node(&output, "Stretch").1.bounds().unwrap();
    click(
        &ctx,
        &mut app,
        Pos2::new(
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        ),
        egui::Modifiers::NONE,
    );
    assert!(app.stretch.open);
    assert_eq!(app.stretch.source_bpm, 160.0);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (node_id, node) = accessible_node(&output, "Target BPM");
    assert_eq!(node.min_numeric_value(), Some(120.0));
    assert_eq!(node.max_numeric_value(), Some(180.0));
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::SetValue,
                target: node_id,
                data: Some(egui::accesskit::ActionData::NumericValue(155.0)),
            },
        )],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.stretch.target_bpm, 160.0);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Decrement,
                target: node_id,
                data: None,
            },
        )],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.stretch.target_bpm, 140.0);
}

#[test]
fn stretch_artwork_uses_one_texel_per_physical_pixel() {
    for density in themed_densities() {
        let (ctx, mut app) = fixture();
        ctx.set_pixels_per_point(density);
        app.stretch.open = true;
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_folded_assets(&ctx, &output, density, "Marine");
    }
}

#[test]
fn stretch_header_controls_use_corner_positions_and_a_complete_collapsed_skin() {
    let density = 1.0;
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.stretch.open = true;
    app.stretch.source_bpm = 160.0;
    app.stretch.target_bpm = 120.0;
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let expanded = effects_control_rect(&output, "TIME STRETCHER");
    for (name, x) in [
        ("Collapse Time Stretcher", 24.0),
        ("Close Time Stretcher", expanded.width() - 24.0),
    ] {
        assert!(
            (effects_control_rect(&output, name).center().x - expanded.left() - x).abs()
                <= 1.0 / density
        );
    }
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.text == "Help")));
    focus_control(&ctx, &mut app, &output, "Collapse Time Stretcher");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let collapsed = effects_control_rect(&output, "TIME STRETCHER");
    assert_eq!(collapsed.min, expanded.min);
    assert_eq!(collapsed.width(), expanded.width());
    assert!((collapsed.height() - 44.0).abs() < 0.1);
    for hidden in [
        "Source sound",
        "Listen to stretched sound",
        "Time Stretcher status",
    ] {
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
    focus_control(&ctx, &mut app, &output, "Expand Time Stretcher");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(effects_control_rect(&output, "TIME STRETCHER"), expanded);
    assert_eq!(
        (app.stretch.source_bpm, app.stretch.target_bpm),
        (160.0, 120.0)
    );
    focus_control(&ctx, &mut app, &output, "Time Stretcher help");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        app.help.stretch,
        "The help icon must support keyboard activation."
    );
    app.help.stretch = false;
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new(("hardware-collapsed", "Time Stretcher")),
            true,
        )
    });
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Close Time Stretcher");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(!app.stretch.open, "Close must work in the collapsed state.");
}

#[test]
fn musical_tools_collapsed_help_icons_preserve_processing_drafts() {
    for (tool, title, collapse, help, expand, collapse_id, height) in [
        (
            "stretch",
            "TIME STRETCHER",
            "Collapse Time Stretcher",
            "Time Stretcher help",
            "Expand Time Stretcher",
            "Time Stretcher",
            44.0,
        ),
        (
            "effects",
            "EFFECT STUDIO",
            "Collapse Effect Studio",
            "Effect Studio help",
            "Expand Effect Studio",
            "Effect Studio",
            44.0,
        ),
    ] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        ctx.style_mut(|style| style.animation_time = 0.0);
        app.selection.sample = Some(1);
        app.stretch.source_bpm = 160.0;
        app.stretch.target_bpm = 120.0;
        app.effects.state.echo = Some(Default::default());
        app.effects.state.dirty = 0.375;
        app.step_synth.state.steps[19] = Some(-7);
        app.step_synth.state.active_bars = [true, false, true, false];
        app.step_synth.state.bpm = 160.0;
        match tool {
            "stretch" => app.stretch.open = true,
            "effects" => app.effects.open = true,
            _ => unreachable!(),
        }
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let before = (
            app.effects.state.clone(),
            app.step_synth.state.clone(),
            app.stretch.source_bpm,
            app.stretch.target_bpm,
            app.source_id(),
            app.session.project().clone(),
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, collapse);
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
        let window = effects_control_rect(&output, title);
        assert!(
            (window.height() - height).abs() < 0.1,
            "{title} must be collapsed."
        );
        assert!(
            window.contains_rect(effects_control_rect(&output, help)),
            "The collapsed header must keep {help} available."
        );
        assert!(!accessible_node(&output, help).1.is_disabled());
        focus_control(&ctx, &mut app, &output, help);
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
        accessible_node(&output, &title_text(help));
        assert!(match tool {
            "stretch" => app.stretch.open && app.help.stretch,
            "effects" => app.effects.open && app.help.effects,
            _ => unreachable!(),
        });
        assert!(dialog::is_collapsed(&ctx, collapse_id));
        accessible_node(&output, expand);
        focus_control(&ctx, &mut app, &output, &format!("Close {help}"));
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            (effects_control_rect(&output, title).height() - height).abs() < 0.1,
            "Closing {help} must return to the collapsed tool."
        );
        assert_eq!(
            (
                app.effects.state.clone(),
                app.step_synth.state.clone(),
                app.stretch.source_bpm,
                app.stretch.target_bpm,
                app.source_id(),
                app.session.project().clone()
            ),
            before,
            "Opening and closing {help} must preserve the source, all processing drafts, and song."
        );
        assert!(!app.session.is_stretching());
    }
}

#[test]
fn stretch_rotaries_accept_keyboard_input_and_keep_supported_target_tempos() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.selection.sample = Some(1);
    app.stretch.open = true;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Original BPM knob");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.stretch.source_bpm, 141.0);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Target BPM");
    let focused_knob = ctx.memory(|memory| memory.focused());
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowRight, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.stretch.target_bpm, 160.0);
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        focused_knob,
        "Arrow keys must keep focus on the target slider."
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::End, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.stretch.target_bpm, 180.0);
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(!app.stretch.open);
}

#[test]
fn stretch_escape_closes_an_open_list_or_a_window_above_before_the_tool() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    // Test frames share one instant; a closing window must not keep fading.
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.selection.sample = Some(1);
    app.stretch.open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let bounds = accessible_node(&output, "Source sound").1.bounds().unwrap();
    click(
        &ctx,
        &mut app,
        Pos2::new(
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        ),
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(egui::Popup::is_any_open(&ctx));
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        app.stretch.open,
        "Esc must close the source list, not the tool."
    );
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    assert!(!egui::Popup::is_any_open(&ctx));
    // The tool's own Help opens above it and takes Esc first.
    app.help.stretch = true;
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        app.stretch.open,
        "Esc must not close the tool under another window."
    );
    app.help.stretch = false;
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(!app.stretch.open);
}

#[test]
fn stretch_actions_fit_supported_windows_and_require_an_available_source() {
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.stretch.open = true;
        app.selection.sample = Some(1);
        for _ in 0..3 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        for text in [
            "TIME STRETCHER",
            "Original BPM",
            "Target BPM",
            "Listen",
            "Save stretched sound",
            "Same tempo. Duration is unchanged.",
        ] {
            let text_bounds = text_rect(&output, text);
            assert!(
                Rect::from_min_size(Pos2::ZERO, size).contains_rect(text_bounds),
                "{text} outside {size:?}: {text_bounds:?}"
            );
        }
        for label in ["Listen to stretched sound", "Save stretched sound"] {
            assert!(!accessible_node(&output, label).1.is_disabled());
        }
        app.selection.sample = None;
        app.selection.clip = None;
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        for label in ["Listen to stretched sound", "Save stretched sound"] {
            assert!(accessible_node(&output, label).1.is_disabled());
        }
    }
}

#[test]
fn stretch_source_names_stay_bounded_and_search_selects_the_matching_sound() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let mut sample = app.session.library().samples[0].clone();
    sample.name = format!("{} distinctive ending.wav", "Long source name ".repeat(40));
    let mut other = sample.clone();
    other.id = 2;
    other.name = "Unique searchable sound".into();
    other.source_bpm = 160.0;
    app.session = Session::headless(
        Arc::new(Library::fixture(vec![sample, other])),
        app.session.project().clone(),
    )
    .unwrap();
    app.selection.sample = Some(1);
    app.stretch.open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let bounds = accessible_node(&output, "Source sound").1.bounds().unwrap();
    let window = accessible_node(&output, "TIME STRETCHER")
        .1
        .bounds()
        .unwrap();
    assert!(bounds.x0 >= window.x0 && bounds.x1 <= window.x1);
    assert!(
        bounds.x1 - bounds.x0 <= 620.0,
        "Long source label expanded the slot: {bounds:?}"
    );
    click(
        &ctx,
        &mut app,
        Pos2::new(
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        ),
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Find source sound");
    let output = frame(
        &ctx,
        &mut app,
        vec![egui::Event::Text("unique".into())],
        egui::Modifiers::NONE,
    );
    let (_, node) = accessible_node(&output, "Unique searchable sound");
    let bounds = node.bounds().unwrap();
    assert!(bounds.x1 - bounds.x0 <= 620.0);
    click(
        &ctx,
        &mut app,
        Pos2::new(
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        ),
        egui::Modifiers::NONE,
    );
    assert_eq!(app.selection.sample, Some(2));
    assert_eq!(app.stretch.source_bpm, 160.0);
    assert!(!egui::Popup::is_any_open(&ctx));
}

#[test]
fn stretch_source_search_refreshes_after_same_size_library_scan() {
    let dir =
        std::env::temp_dir().join(format!("mixjam-stretch-source-scan-{}", std::process::id()));
    let old = dir.join("old");
    let new = dir.join("new");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir_all(&new).unwrap();
    for (folder, names) in [
        (&old, ["a needle.wav", "b other.wav"]),
        (&new, ["a other.wav", "b needle.wav"]),
    ] {
        for name in names {
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![0.25; 1000],
            }
            .write_wav(&folder.join(name))
            .unwrap();
        }
    }
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.session = Session::headless(
        Arc::new(Library::open_wav_folder(&old, 140.0).unwrap()),
        Project::default(),
    )
    .unwrap()
    .with_settings_root(dir.join("settings"));
    app.stretch.open = true;
    app.options = None;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let bounds = accessible_node(&output, "Source sound").1.bounds().unwrap();
    click(
        &ctx,
        &mut app,
        Pos2::new(
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        ),
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Find source sound");
    let output = frame(
        &ctx,
        &mut app,
        vec![egui::Event::Text("needle".into())],
        egui::Modifiers::NONE,
    );
    accessible_node(&output, "a needle");
    let revision = app.session.catalog_revision();
    app.session.start_wave_scan(&[new], 140.0).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while app.session.wave_scan().is_some() {
        assert!(std::time::Instant::now() < deadline);
        app.poll_application(&ctx);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.session.catalog_revision() > revision);
    assert_eq!(app.session.library().samples.len(), 2);
    assert!(egui::Popup::is_any_open(&ctx));
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    std::fs::remove_dir_all(dir).unwrap();
    accessible_node(&output, "b needle");
    assert!(
        !output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("a other")),
        "The open source search must use the replacement library."
    );
}

#[test]
fn stretch_target_notches_are_even_and_resets_use_140_and_source_tempo() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let mut sample = app.session.library().samples[0].clone();
    sample.source_bpm = 160.0;
    app.session = Session::headless(
        Arc::new(Library::fixture(vec![sample])),
        app.session.project().clone(),
    )
    .unwrap();
    app.selection.sample = Some(1);
    app.stretch.open = true;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for (index, bpm) in SUPPORTED_BPMS.into_iter().enumerate() {
        app.stretch.target_bpm = bpm;
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let bounds = accessible_node(&output, "Target BPM").1.bounds().unwrap();
        let (left, right) = (bounds.x0 as f32 + 8.0, bounds.x1 as f32 - 8.0);
        let expected = Pos2::new(
            left + (right - left) * index as f32 / 6.0,
            bounds.y0 as f32 + 10.0,
        );
        assert!(
            output.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Circle(circle) if (circle.radius - 7.0).abs() < 0.01
                    && circle.center.distance(expected) < 0.05
            )),
            "No cap at {bpm} BPM"
        );
        let target = Rect::from_min_max(
            Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
            Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
        );
        let caption = format!("{bpm:.0}");
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == caption
                {
                    let rect = Rect::from_min_size(text.pos, text.galley.size());
                    target.contains(rect.center()).then_some(rect)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(labels.len(), 1, "Target BPM must have one {bpm} caption");
        assert!(
            (labels[0].center().x - expected.x).abs() < 0.5,
            "{bpm} BPM must be labelled under its notch"
        );
    }
    app.stretch.source_bpm = 100.0;
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let bounds = accessible_node(&output, "Original BPM knob")
        .1
        .bounds()
        .unwrap();
    let center = Pos2::new(
        (bounds.x0 + bounds.x1) as f32 / 2.0,
        (bounds.y0 + bounds.y1) as f32 / 2.0,
    );
    click(&ctx, &mut app, center, egui::Modifiers::NONE);
    click(&ctx, &mut app, center, egui::Modifiers::NONE);
    assert_eq!(app.stretch.source_bpm, 160.0);
    // Test frames share one instant, so check the target reset in a fresh
    // context; four clicks in one would count as a quadruple click.
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.selection.sample = Some(1);
    app.stretch.open = true;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let bounds = accessible_node(&output, "Target BPM").1.bounds().unwrap();
    let end = Pos2::new(bounds.x1 as f32 - 8.0, bounds.y0 as f32 + 10.0);
    click(&ctx, &mut app, end, egui::Modifiers::NONE);
    click(&ctx, &mut app, end, egui::Modifiers::NONE);
    assert_eq!(
        app.stretch.target_bpm, 140.0,
        "Double-click must reset the target to 140 BPM."
    );
}

#[test]
fn stretch_shift_changes_only_new_drag_motion() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.selection.sample = Some(1);
    app.stretch.open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let bounds = accessible_node(&output, "Original BPM knob")
        .1
        .bounds()
        .unwrap();
    let origin = Pos2::new(
        (bounds.x0 + bounds.x1) as f32 / 2.0,
        (bounds.y0 + bounds.y1) as f32 / 2.0,
    );
    pointer(&ctx, &mut app, origin, true, egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(
            origin + Vec2::new(12.0 * 150.0 / 270.0, 0.0),
        )],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.stretch.source_bpm, 152.0);
    frame(&ctx, &mut app, vec![], egui::Modifiers::SHIFT);
    assert_eq!(app.stretch.source_bpm, 152.0);
    let end = origin + Vec2::new(24.0 * 150.0 / 270.0, 0.0);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(end)],
        egui::Modifiers::SHIFT,
    );
    assert_eq!(app.stretch.source_bpm, 153.0);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.stretch.source_bpm, 153.0);
    pointer(&ctx, &mut app, end, false, egui::Modifiers::NONE);
}

#[test]
fn stretch_action_labels_remain_centered_in_every_playback_state() {
    let density = 1.0;
    for (processing, listening, saved, listen_text) in [
        (false, false, false, "Listen"),
        (false, true, false, "Stop listening"),
        (true, false, false, "Processing..."),
        (false, false, true, "Listen"),
    ] {
        let output = stretch_layout_output(
            density,
            false,
            stretch::Feedback {
                processing,
                listening,
                saved,
                audition: Some((1, 160.0, 120.0)),
                message: None,
                error: false,
            },
        );
        for (accessible_name, visible_text) in [
            (
                if listening {
                    "Stop listening"
                } else {
                    "Listen to stretched sound"
                },
                listen_text,
            ),
            ("Save stretched sound", "Save stretched sound"),
        ] {
            let bounds = accessible_node(&output, accessible_name)
                .1
                .bounds()
                .unwrap();
            let button = Rect::from_min_max(
                Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
                Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
            );
            // Measure visible glyph meshes, including the one-pixel shadow.
            let painted = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == visible_text => {
                        Some(text.visual_bounding_rect())
                    }
                    _ => None,
                })
                .reduce(|bounds, text| bounds.union(text))
                .unwrap_or_else(|| panic!("Missing action label: {visible_text}"));
            assert!(
                button.contains_rect(painted),
                "{visible_text} leaves its button at {density}x: {painted:?} / {button:?}"
            );
            let offset = painted.center() - (button.center() - Vec2::new(0.0, 1.0));
            assert!(
                offset.x.abs() <= 1.0 && offset.y.abs() <= 3.0,
                "Shared button label must stay centered: {offset:?}"
            );
        }
    }
}
