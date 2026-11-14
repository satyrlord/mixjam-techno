//! Mastering board: pedal switches and knobs, presets, meters, the over latch and the
//! bypass switches.

use super::*;

#[test]
fn mastering_switches_and_knobs_edit_the_pedalboard_with_undo() {
    use mixjam_techno::application::PedalKind::*;
    const NAMES: [&str; 13] = [
        "Booster EQ",
        "Booster compressor",
        "Booster wide",
        "Soft clip",
        "Tube",
        "Trim EQ",
        "Bus compressor",
        "Maximizer",
        "Lift EQ",
        "Tape",
        "Stereo imaging",
        "Multiband",
        "Limiter",
    ];
    const BOOSTER_KNOBS: [&str; 14] = [
        "Booster EQ 60 Hz",
        "Booster EQ 80 Hz",
        "Booster EQ 160 Hz",
        "Booster EQ 320 Hz",
        "Booster EQ 640 Hz",
        "Booster EQ 1100 Hz",
        "Booster EQ 2200 Hz",
        "Booster EQ 4500 Hz",
        "Booster EQ 9000 Hz",
        "Booster EQ 12000 Hz",
        "Booster compressor drive",
        "Booster compressor speed",
        "Booster compressor gain",
        "Booster width",
    ];
    for density in themed_densities() {
        let (ctx, mut app) = fixture();
        ctx.set_pixels_per_point(density);
        ctx.enable_accesskit();
        ctx.style_mut(|style| style.animation_time = 0.0);
        let size = Vec2::new(960.0, 720.0);
        // The first frame after a density change still reports the old
        // screen; open the tool once the minimum window is in place.
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        app.boards.module = BottomModule::Mastering;
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let window = effects_control_rect(&output, "Mastering board");
        assert!(
            Rect::from_min_size(Pos2::ZERO, size).contains_rect(window),
            "The board must fit the minimum window at {density}x."
        );
        assert_folded_assets(&ctx, &output, density, "Indigo");
        // The input and output units and the pedals stay in view however
        // far the row scrolls. The minimum window scrolls the strip below.
        for name in ["Trim", "Trim value", "Clear OVER"] {
            assert!(
                window.contains_rect(effects_control_rect(&output, name)),
                "{name} must fit at {density}x."
            );
        }
        let first = effects_control_rect(&output, "Drag Booster EQ");
        assert!(
            window.contains_rect(Rect::from_min_size(
                first.min,
                Vec2::new(first.width(), 304.0)
            )),
            "The pedals must fit at {density}x."
        );
        for name in NAMES {
            effects_control_rect(&output, &format!("Show {name}"));
        }
        for label in [
            "Input level: no mix playing",
            "Output loudness: momentary - LUFS, short-term - LUFS, integrated - LUFS, true peak - dBTP",
        ] {
            assert!(
                window.contains_rect(accessible_label(&output, label)),
                "{label} must fit at {density}x."
            );
        }
        // Every pedal starts off, and its knobs wait for its switch.
        for name in
            BOOSTER_KNOBS
                .iter()
                .chain(&["Soft clip amount", "Limiter ceiling", "Tape speed"])
        {
            assert!(
                accessible_node(&output, name).1.is_disabled(),
                "{name} starts disabled."
            );
        }
        assert!(!accessible_node(&output, "Trim").1.is_disabled());
        assert_eq!(
            accessible_node(&output, "Booster EQ 1100 Hz value")
                .1
                .value(),
            Some("+0.0"),
            "EQ values read in decibels."
        );
        assert_eq!(
            accessible_node(&output, "Limiter ceiling value").1.value(),
            Some("-1.0 dB")
        );
        assert_eq!(
            accessible_node(&output, "Bus compressor ratio value")
                .1
                .value(),
            Some("2.0:1")
        );
        assert_eq!(
            accessible_node(&output, "Tape speed value").1.value(),
            Some("30 ips")
        );
        text_rect(
            &output,
            "Every pedal is off and the trim is at 0 dB: the mix plays unprocessed.",
        );

        // The row runs left to right in signal order between the units,
        // and every control keeps clear of its pedal's beveled edge.
        // A pedal's header holds its drag grip; the limiter's says that it
        // always runs last.
        let pedal = |output: &egui::FullOutput, name: &str| {
            let header = if name == "Limiter" {
                "Limiter always runs last".to_owned()
            } else {
                format!("Drag {name}")
            };
            let grip = effects_control_rect(output, &header);
            Rect::from_min_size(grip.min, Vec2::new(grip.width() + 30.0, 304.0))
        };
        let input = effects_control_rect(&output, "Trim");
        let outlet = effects_control_rect(&output, "Clear OVER");
        let pedals = NAMES.map(|name| pedal(&output, name));
        assert!(
            input.right() < pedals[0].left() && pedals[12].left() > outlet.left(),
            "The row scrolls between the units, and its end starts out of view."
        );
        assert!(
            pedals
                .windows(2)
                .all(|pair| pair[0].right() < pair[1].left())
        );
        let dials: [&[&str]; 13] = [
            &BOOSTER_KNOBS[..10],
            &BOOSTER_KNOBS[10..13],
            &BOOSTER_KNOBS[13..],
            &["Soft clip amount", "Soft clip ceiling"],
            &["Tube drive", "Tube mix"],
            &["Trim EQ high-pass", "Trim EQ mud", "Trim EQ harsh"],
            &[
                "Bus compressor threshold",
                "Bus compressor ratio",
                "Bus compressor attack",
                "Bus compressor release",
                "Bus compressor gain reduction 0.0 dB",
            ],
            &["Maximizer boost"],
            &["Lift EQ low", "Lift EQ air"],
            &["Tape drive", "Tape speed"],
            &["Stereo imaging width", "Stereo imaging mono below"],
            &["Multiband low", "Multiband mid", "Multiband high"],
            &[
                "Limiter gain",
                "Limiter ceiling",
                "Limiter gain reduction 0.0 dB",
            ],
        ];
        for (name, dials) in NAMES.into_iter().zip(dials) {
            let bounds = pedal(&output, name).shrink(4.0);
            let mut parts = vec![name.to_string()];
            if name != "Limiter" {
                parts.extend([format!("Move {name} left"), format!("Move {name} right")]);
            }
            for dial in dials {
                if dial.contains("reduction") {
                    assert!(
                        bounds.contains_rect(accessible_label(&output, dial)),
                        "{dial} at {density}x."
                    );
                } else {
                    parts.push(dial.to_string());
                    parts.push(format!("{dial} value"));
                }
            }
            for part in parts {
                assert!(
                    bounds.contains_rect(effects_control_rect(&output, &part)),
                    "{part} must keep clear of the {name} pedal's edge at {density}x."
                );
            }
        }
        assert!(
            accessible_node(&output, "Move Booster EQ left")
                .1
                .is_disabled()
        );
        // Nothing passes the limiter, which has no move buttons or grip.
        assert!(
            accessible_node(&output, "Move Multiband right")
                .1
                .is_disabled()
        );
        assert!(
            !accessible_node(&output, "Move Multiband left")
                .1
                .is_disabled()
        );
        let labels: Vec<&str> = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .filter_map(|(_, node)| node.label())
            .collect();
        for absent in ["Move Limiter left", "Move Limiter right", "Drag Limiter"] {
            assert!(!labels.contains(&absent), "{absent} must not exist.");
        }

        // A strip chip brings its pedal into view; so does keyboard focus.
        focus_control(&ctx, &mut app, &output, "Show Limiter");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        // From here the window has its default size, and the board follows it.
        let window = effects_control_rect(&output, "Mastering board");
        assert!(
            window.contains_rect(pedal(&output, "Limiter")),
            "The Limiter must come into view at {density}x."
        );
        assert!(!window.contains_rect(pedal(&output, "Booster EQ")));
        // A pedal dropped past the limiter stays before it.
        let grip = effects_control_rect(&output, "Drag Multiband").center();
        let past = pedal(&output, "Limiter").right_center() + Vec2::new(6.0, 0.0);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(grip)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, grip, true, egui::Modifiers::NONE);
        for step in 1..=6 {
            let at = grip + (past - grip) * (step as f32 / 6.0);
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(at)],
                egui::Modifiers::NONE,
            );
        }
        pointer(&ctx, &mut app, past, false, egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.session.project().master.order, PedalKind::ALL);
        assert!(
            !app.session.unsaved(),
            "A drop that moves nothing is no edit."
        );
        focus_control(&ctx, &mut app, &output, "Booster wide");
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            window.contains_rect(effects_control_rect(&output, "Booster wide")),
            "A focused switch must scroll into view at {density}x."
        );

        // Arrow buttons and dragging both move a pedal; each move is one undo step.
        focus_control(&ctx, &mut app, &output, "Move Booster wide left");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(
            app.session.project().master.order[..3],
            [BoosterEq, BoosterWide, BoosterCompressor]
        );
        focus_control(&ctx, &mut app, &output, "Show Booster EQ");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let grip = effects_control_rect(&output, "Drag Booster EQ").center();
        let past = pedal(&output, "Booster wide").right_center() + Vec2::new(4.0, 0.0);
        assert!(window.contains(past));
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(grip)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, grip, true, egui::Modifiers::NONE);
        for step in 1..=8 {
            let at = grip + (past - grip) * (step as f32 / 8.0);
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(at)],
                egui::Modifiers::NONE,
            );
        }
        assert_eq!(
            app.session.project().master.order[..3],
            [BoosterEq, BoosterWide, BoosterCompressor],
            "Dragging previews the place; only the drop moves the pedal."
        );
        pointer(&ctx, &mut app, past, false, egui::Modifiers::NONE);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            app.session.project().master.order[..3],
            [BoosterWide, BoosterEq, BoosterCompressor]
        );
        assert_eq!(app.session.project().master.order[3..], PedalKind::ALL[3..]);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(pedal(&output, "Booster wide").right() < pedal(&output, "Booster EQ").left());
        assert!(app.session.history_step(false).unwrap());
        assert_eq!(
            app.session.project().master.order[..3],
            [BoosterEq, BoosterWide, BoosterCompressor]
        );
        assert!(app.session.history_step(false).unwrap());
        assert_eq!(app.session.project().master.order, PedalKind::ALL);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);

        focus_control(&ctx, &mut app, &output, "Booster compressor");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(app.session.project().master.booster_compressor.on);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            !accessible_node(&output, "Booster compressor drive")
                .1
                .is_disabled()
        );
        assert!(accessible_node(&output, "Booster width").1.is_disabled());
        // The keys are one gesture; frames drawn under load must not count
        // as the user's pause. The double-click is a press, so it starts
        // a step of its own.
        app.session.hold_gestures();
        focus_control(&ctx, &mut app, &output, "Booster compressor drive");
        for (key, modifiers, expected) in [
            (egui::Key::ArrowUp, egui::Modifiers::NONE, 501),
            (egui::Key::ArrowUp, egui::Modifiers::SHIFT, 502),
            (egui::Key::End, egui::Modifiers::NONE, 1000),
            (egui::Key::Home, egui::Modifiers::NONE, 0),
        ] {
            frame(&ctx, &mut app, vec![key_event(key, modifiers)], modifiers);
            assert_eq!(
                app.session.project().master.booster_compressor.drive,
                expected
            );
        }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let center = effects_control_rect(&output, "Booster compressor drive").center();
        click(&ctx, &mut app, center, egui::Modifiers::NONE);
        click(&ctx, &mut app, center, egui::Modifiers::NONE);
        assert_eq!(
            app.session.project().master.booster_compressor.drive,
            500,
            "Double-click must restore the default."
        );
        assert!(app.session.unsaved());

        // The reset undoes, then the key gesture as one step, then the switch.
        assert!(app.session.history_step(false).unwrap());
        assert_eq!(app.session.project().master.booster_compressor.drive, 0);
        assert!(app.session.history_step(false).unwrap());
        assert_eq!(app.session.project().master.booster_compressor.drive, 500);
        assert!(app.session.project().master.booster_compressor.on);
        assert!(app.session.history_step(false).unwrap());
        assert!(!app.session.project().master.is_active());

        // The mastering pedals: the limiter's knobs step in tenths of a
        // decibel, the tape speed switches between its two speeds, and
        // the trim changes the status line.
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Limiter");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(app.session.project().master.limiter.on);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Limiter ceiling");
        for (key, modifiers, expected) in [
            (egui::Key::ArrowUp, egui::Modifiers::NONE, -0.9),
            (egui::Key::ArrowDown, egui::Modifiers::SHIFT, -1.0),
            (egui::Key::End, egui::Modifiers::NONE, 0.0),
        ] {
            frame(&ctx, &mut app, vec![key_event(key, modifiers)], modifiers);
            assert_eq!(app.session.project().master.limiter.ceiling, expected);
        }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            accessible_node(&output, "Limiter ceiling value").1.value(),
            Some("0.0 dB")
        );
        assert!(window.contains_rect(accessible_label(&output, "Limiter gain reduction 0.0 dB")));
        focus_control(&ctx, &mut app, &output, "Tape");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Tape speed");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(app.session.project().master.tape.speed, 15);
        focus_control(&ctx, &mut app, &output, "Trim");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(app.session.project().master.trim, -0.1);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(
            &output,
            "2 of 13 pedals on, trim -0.1 dB. Latency 4.8 ms, which exports remove.",
        );
        for _ in 0..5 {
            assert!(app.session.history_step(false).unwrap());
        }
        assert_eq!(app.session.project().master, MasterChain::default());

        focus_control(&ctx, &mut app, &output, "Mastering help");
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert!(
            app.help.master,
            "The Help icon must support keyboard activation."
        );
    }
}

#[test]
fn opening_mastering_preserves_saved_values_between_dial_steps() {
    use mixjam_techno::application::Knob;

    for on in [false, true] {
        let (ctx, _) = fixture();
        let mut chain = MasterChain::default();
        for kind in PedalKind::ALL {
            chain.set_on(kind, on);
        }
        for knob in Knob::all() {
            if knob.pedal().is_some_and(PedalKind::is_booster) {
                continue;
            }
            let range = knob.range();
            knob.set(&mut chain, range.low + range.step / 3.0);
        }
        chain.validate().unwrap();
        let metering = master::Metering {
            reduced: false,
            reading: MeterReading::default(),
            playing: false,
        };
        for _ in 0..3 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
                    ..Default::default()
                },
                |ctx| {
                    assert!(
                        matches!(board(ctx, &chain, &metering), master::Action::None),
                        "Showing a saved board must not change values between dial steps, \
                         even when its pedals are {on}."
                    );
                },
            );
        }
    }
}

#[test]
fn mastering_value_windows_keep_saved_values_and_apply_typed_values_on_entry() {
    let (ctx, mut app, output) = mastering_fixture(|master| {
        master.trim = 1.04;
        master.limiter.on = true;
    });
    // Focusing a value window and leaving it is no edit, even where the
    // window shows the saved value rounded.
    focus_control(&ctx, &mut app, &output, "Trim value");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Trim");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.session.project().master.trim, 1.04);
    assert!(
        !app.session.unsaved(),
        "Focus alone must not edit the board."
    );

    // A typed value applies once, on entry, not at each keystroke.
    focus_control(&ctx, &mut app, &output, "Limiter gain value");
    frame(
        &ctx,
        &mut app,
        vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("1".into()),
        ],
        egui::Modifiers::COMMAND,
    );
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Text("0".into())],
        egui::Modifiers::NONE,
    );
    assert_eq!(
        app.session.project().master.limiter.gain,
        4.0,
        "Partial entries must not reach the board."
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().master.limiter.gain, 10.0);
    assert!(app.session.history_step(false).unwrap());
    assert_eq!(app.session.project().master.limiter.gain, 4.0);
}

#[test]
fn mastering_presets_set_every_pedal_keep_the_trim_and_undo_in_one_step() {
    use mixjam_techno::application::presets::PRESETS;
    let (context, mut app, port) = agent_fixture();
    app.boards.module = BottomModule::Mastering;
    let mut booster = MasterChain::default();
    booster.booster_eq.on = true;
    // The mix was too loud, so its trim is down 9 dB.
    booster.trim = -9.0;
    booster.move_pedal(0, 6);
    app.session.edit(EditCommand::SetMaster(booster)).unwrap();
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["set", "target=Mastering preset", "value=Hard Techno"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    let hard = PRESETS
        .iter()
        .find(|preset| preset.name == "Hard Techno")
        .unwrap();
    let chain = app.session.project().master;
    assert_eq!(
        chain,
        MasterChain {
            trim: -9.0,
            ..hard.chain
        }
    );
    assert!(
        !chain.booster_eq.on && chain.order == PedalKind::ALL,
        "A preset sets every pedal."
    );
    assert_eq!(
        chain.trim, -9.0,
        "A preset must keep the trim that suits the mix."
    );
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "Hard Techno");
    assert!(app.session.history_step(false).unwrap());
    assert_eq!(
        app.session.project().master,
        booster,
        "One undo restores the board before the preset."
    );
    assert!(app.session.history_step(true).unwrap());
    // Any edit leaves the preset; the selector then reads Custom.
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["set", "target=Limiter gain value", "value=6"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(app.session.project().master.limiter.gain, 6.0);
    let output = frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "Custom");
}

#[test]
fn mastering_over_latch_exposes_its_state_and_clears_by_keyboard() {
    let (ctx, _) = fixture();
    ctx.enable_accesskit();
    let chain = MasterChain::default();
    let mut reading = MeterReading {
        run: 1,
        ..MeterReading::default()
    };
    let generation = std::cell::Cell::new(0_u64);
    let render = |reading, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                let metering = master::Metering {
                    reduced: false,
                    reading,
                    playing: true,
                };
                let action = egui::CentralPanel::default()
                    .show(ctx, |ui| {
                        ui.push_id(generation.get(), |ui| {
                            master::show(ui, &chain, &metering, None)
                        })
                        .inner
                    })
                    .inner;
                assert!(matches!(action, master::Action::None));
            },
        )
    };
    for _ in 0..3 {
        render(reading, vec![]);
    }
    let output = render(reading, vec![]);
    assert_eq!(
        accessible_node(&output, "Clear OVER").1.description(),
        Some("OVER: clear.")
    );
    reading.overs = 1;
    let output = render(reading, vec![]);
    assert_eq!(
        accessible_node(&output, "Clear OVER").1.description(),
        Some("OVER: detected.")
    );
    let target = accessible_node(&output, "Clear OVER").0;
    render(
        reading,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target,
                data: None,
            },
        )],
    );
    render(
        reading,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    let output = render(reading, vec![]);
    assert_eq!(
        accessible_node(&output, "Clear OVER").1.description(),
        Some("OVER: clear.")
    );
    generation.set(1);
    let output = render(reading, vec![]);
    assert_eq!(
        accessible_node(&output, "Clear OVER").1.description(),
        Some("OVER: clear."),
        "Resetting project controls must preserve an acknowledged meter reading."
    );
    reading.overs = 2;
    let output = render(reading, vec![]);
    assert_eq!(
        accessible_node(&output, "Clear OVER").1.description(),
        Some("OVER: detected.")
    );
}

#[test]
fn mastering_meters_show_the_playback_reading() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    let reading = MeterReading {
        run: 1,
        input_vu: -1.5,
        input_peaks: [false, true],
        input_peak_counts: [0, 1],
        compressor_reduction: 1.7,
        limiter_reduction: 2.6,
        momentary: -12.6,
        short_term: -13.4,
        integrated: -13.9,
        true_peak: -2.4,
        true_peak_max: -0.8,
        overs: 1,
    };
    let metering = master::Metering {
        reduced: false,
        reading,
        playing: true,
    };
    let mut chain = MasterChain::default();
    chain.bus_compressor.on = true;
    chain.limiter.on = true;
    let mut action = None;
    let mut output = egui::FullOutput::default();
    // A new window sizes itself before it paints.
    for _ in 0..3 {
        output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
                ..Default::default()
            },
            |ctx| {
                action = Some(matches!(
                    board(ctx, &chain, &metering),
                    master::Action::None
                ));
            },
        );
    }
    assert_eq!(
        action,
        Some(true),
        "Showing readings must not edit the board."
    );
    accessible_label(&output, "Input level -1.5 VU, peaks on the right");
    accessible_label(
        &output,
        "Output loudness: momentary -12.6 LUFS, short-term -13.4 LUFS, integrated -13.9 LUFS, true peak -0.8 dBTP",
    );
    accessible_label(&output, "Bus compressor gain reduction 1.7 dB");
    accessible_label(&output, "Limiter gain reduction 2.6 dB");
    text_rect(&output, "-0.8");
    // When playback stops, the live meters rest; integrated loudness and
    // the true peak of the playback remain.
    let stopped = master::Metering {
        reduced: false,
        reading,
        playing: false,
    };
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        board(ctx, &chain, &stopped);
    });
    accessible_label(&output, "Input level: no mix playing");
    accessible_label(
        &output,
        "Output loudness: momentary - LUFS, short-term - LUFS, integrated -13.9 LUFS, true peak -0.8 dBTP",
    );
    accessible_label(&output, "Limiter gain reduction 0.0 dB");
}

#[test]
fn bypass_all_and_master_bypass_are_one_switch_and_track_export_asks_for_a_folder() {
    let (context, mut app, port) = agent_fixture();
    let send = |app: &mut MixJamApp, words: &[&str]| agent(&context, app, &port, words);
    let mut chain = app.session.project().master;
    chain.limiter.on = true;
    app.session.edit(EditCommand::SetMaster(chain)).unwrap();
    app.boards.module = BottomModule::Mastering;
    let reply = send(&mut app, &["set", "target=Bypass all pedals", "value=true"]);
    assert_eq!(reply["ok"], true, "{reply}");
    assert!(app.session.project().master.bypass);
    assert!(app.session.project().master.limiter.on);
    let reply = send(&mut app, &["ui", "all=true", "contains=Bypassed:"]);
    assert_eq!(reply["nodes"].as_array().unwrap().len(), 1, "{reply}");
    let state = send(&mut app, &["state"]);
    assert_eq!(state["song"]["master_bypass"], true, "{state}");
    app.boards.module = BottomModule::Mixer;
    let reply = send(&mut app, &["ui", "contains=Master bypass"]);
    assert_eq!(reply["nodes"][0]["checked"], true, "{reply}");
    send(&mut app, &["click", "target=Master bypass"]);
    assert!(!app.session.project().master.bypass);
    app.history_step(false);
    assert!(app.session.project().master.bypass);

    // A song whose sound has audio, on track 3.
    let dir = std::env::temp_dir().join(format!("mixjam-ui-tracks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wave = dir.join("source.wav");
    Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.25; 4410],
    }
    .write_wav(&wave)
    .unwrap();
    let placeholder =
        Session::headless(Arc::new(Library::fixture(vec![])), Project::default()).unwrap();
    app.session =
        std::mem::replace(&mut app.session, placeholder).with_settings_root(dir.join("settings"));
    app.session.new_mix().unwrap();
    let source = app.session.register_file(&wave, "Wave", 140.0).unwrap();
    app.session.place(source, 2, 0).unwrap();
    let reply = send(&mut app, &["click", "target=Export tracks"]);
    assert_eq!(reply["modal"], "Choose path", "{reply}");
    let state = send(&mut app, &["state"]);
    assert_eq!(state["path_prompt"]["kind"], "folder", "{state}");
    assert_eq!(state["path_prompt"]["title"], "Export tracks", "{state}");
    send(
        &mut app,
        &["set", "target=Path", &format!("value={}", dir.display())],
    );
    let reply = send(&mut app, &["click", "target=Confirm path"]);
    assert_eq!(reply["ok"], true, "{reply}");
    let reply = send(&mut app, &["wait", "idle=true", "timeout_ms=30000"]);
    assert_eq!(reply["ok"], true, "{reply}");
    let state = send(&mut app, &["state"]);
    assert_eq!(state["busy"]["exporting"], false, "{state}");
    assert_eq!(
        state["status"]["text"],
        format!("Exported 1 track and 0 FX returns to {}.", dir.display()),
        "{state}"
    );
    let title = app.session.project().title.clone();
    let part = crate::audio::read_wav(&dir.join(format!("{title} - Track 03.wav"))).unwrap();
    assert!(part.samples.iter().any(|sample| *sample > 0.1));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn mastering_knobs_take_the_wheel_once() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    // A pedal's knobs work while it is on.
    let mut chain = app.session.project().master;
    chain.booster_eq.on = true;
    app.session.edit(EditCommand::SetMaster(chain)).unwrap();
    let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for _ in 0..3 {
        output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    for (name, modifiers) in [
        ("Trim", egui::Modifiers::NONE),
        ("Booster EQ 60 Hz", egui::Modifiers::NONE),
        ("Trim", egui::Modifiers::SHIFT),
        ("Booster EQ 60 Hz", egui::Modifiers::SHIFT),
    ] {
        let knob = effects_control_rect(&output, name).center();
        let before = app.session.project().master;
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(knob)],
            modifiers,
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: Vec2::new(0.0, 1.0),
                modifiers,
            }],
            modifiers,
        );
        let after = app.session.project().master;
        assert_ne!(after, before, "{name} takes the wheel with {modifiers:?}.");
        for _ in 0..10 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        assert_eq!(
            app.session.project().master,
            after,
            "{name}: one notch applies once."
        );
    }
}
