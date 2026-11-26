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
    const BOOSTER_KNOBS: [&str; 4] = [
        "Booster compressor drive",
        "Booster compressor speed",
        "Booster compressor gain",
        "Booster width",
    ];
    for _ in themes_in_turn() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        ctx.style_mut(|style| style.animation_time = 0.0);
        let size = Vec2::new(960.0, 720.0);
        // Open the tool once the minimum window is in place.
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        app.boards.module = BottomModule::Mastering;
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let window = effects_control_rect(&output, "Mastering board");
        assert!(
            Rect::from_min_size(Pos2::ZERO, size).contains_rect(window),
            "The board must fit the minimum window."
        );
        assert_folded_assets(&ctx, &output, "Indigo");
        // The input and output units and the pedals stay in view however
        // far the row scrolls.
        for name in ["Trim", "Trim value", "Clear OVER"] {
            assert!(
                window.contains_rect(effects_control_rect(&output, name)),
                "{name} must fit."
            );
        }
        let first = effects_control_rect(&output, "Drag Booster EQ");
        assert!(
            window.contains_rect(Rect::from_min_size(
                first.min,
                Vec2::new(first.width(), 228.0)
            )),
            "The pedals must fit."
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
                "{label} must fit."
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
        assert!(
            !accessible_node(&output, "Edit Booster EQ").1.is_disabled(),
            "Deep Edit trigger starts enabled."
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
            Rect::from_min_size(grip.min, Vec2::new(grip.width() + 68.0, 228.0))
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
            &[],
            &BOOSTER_KNOBS[..3],
            &BOOSTER_KNOBS[3..],
            &["Soft clip amount", "Soft clip ceiling"],
            &["Tube drive", "Tube mix"],
            &["Trim EQ high-pass", "Trim EQ mud", "Trim EQ harsh"],
            &[
                "Bus compressor threshold",
                "Bus compressor ratio",
                "Bus compressor attack",
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
                        "{dial}."
                    );
                } else {
                    parts.push(dial.to_string());
                    parts.push(format!("{dial} value"));
                }
            }
            for part in parts {
                assert!(
                    bounds.contains_rect(effects_control_rect(&output, &part)),
                    "{part} must keep clear of the {name} pedal's edge."
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
        let labels: Vec<&str> = screen(&output)
            .nodes()
            .filter_map(|node| node.node().label())
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
            "The Limiter must come into view."
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
            "A focused switch must scroll into view."
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
            "2 of 13 pedals on, trim -0.1 dB. Export removes 4.8 ms of delay.",
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
            vectorscope: [(0.0, 0.0); mixjam_techno::application::VECTORSCOPE_POINTS],
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
fn a_mastering_preset_after_a_knob_turn_is_its_own_undo_step() {
    use mixjam_techno::application::presets::PRESETS;
    let (context, mut app, port) = agent_fixture();
    app.boards.module = BottomModule::Mastering;
    let hard = PRESETS
        .iter()
        .find(|preset| preset.name == "Hard Techno")
        .unwrap()
        .chain;
    let mut turned = hard;
    turned.limiter.gain = hard.limiter.gain + 2.0;
    app.session
        .edit_discrete(EditCommand::SetMaster(turned))
        .unwrap();
    // A knob turn that the preset then takes back: the board differs from
    // the preset in that one knob only.
    let mut turned_more = turned;
    turned_more.limiter.gain = hard.limiter.gain + 3.0;
    app.session
        .edit(EditCommand::SetMaster(turned_more))
        .unwrap();
    let reply = agent(
        &context,
        &mut app,
        &port,
        &["set", "target=Mastering preset", "value=Hard Techno"],
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(app.session.project().master, hard);
    assert!(app.session.history_step(false).unwrap());
    assert_eq!(
        app.session.project().master,
        turned_more,
        "One undo takes back the preset only, not the knob turn before it."
    );
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
                    vectorscope: [(0.0, 0.0); mixjam_techno::application::VECTORSCOPE_POINTS],
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
                assert!(
                    master::show_deep_edit(
                        ctx,
                        &mut true,
                        &mut PedalKind::Limiter,
                        &chain,
                        &metering,
                        generation.get(),
                    )
                    .chain
                    .is_none()
                );
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
    let (target, deep_over) = accessible_node(&output, "Deep edit Clear OVER");
    assert_eq!(deep_over.description(), Some("OVER: detected."));
    render(
        reading,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target,
                data: None,
            },
        )],
    );
    let output = render(reading, vec![]);
    assert_eq!(
        accessible_node(&output, "Clear OVER").1.description(),
        Some("OVER: clear.")
    );
}

#[test]
fn imaging_scope_and_telemetry_show_one_correlation_with_imaging_off() {
    let (ctx, _) = fixture();
    ctx.enable_accesskit();
    let chain = MasterChain::default();
    assert!(!chain.is_on(PedalKind::Imaging));
    let metering = master::Metering {
        reduced: false,
        reading: MeterReading {
            run: 1,
            stereo_correlation: 0.78,
            ..MeterReading::default()
        },
        vectorscope: [(0.3, 0.2); mixjam_techno::application::VECTORSCOPE_POINTS],
        playing: true,
    };
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
                ..Default::default()
            },
            |ctx| {
                master::show_deep_edit(
                    ctx,
                    &mut true,
                    &mut PedalKind::Imaging,
                    &chain,
                    &metering,
                    0,
                );
            },
        );
    }
    // The meters measure after the whole board, so a bypassed Imaging
    // pedal does not make the scope's bar disagree with the telemetry.
    accessible_label(&output, "Correlation: +0.78");
    assert_eq!(
        accessible_node(&output, "Stereo imaging scope").1.value(),
        Some("Correlation +0.78, Mono below 120 Hz")
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
        multiband_reduction: [2.1, 1.4, 0.9],
        limiter_reduction: 2.6,
        stereo_correlation: 0.0,
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
        vectorscope: [(0.0, 0.0); mixjam_techno::application::VECTORSCOPE_POINTS],
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
        vectorscope: [(0.0, 0.0); mixjam_techno::application::VECTORSCOPE_POINTS],
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
    chain.soft_clip.on = true;
    app.session.edit(EditCommand::SetMaster(chain)).unwrap();
    let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for _ in 0..3 {
        output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    for (name, modifiers) in [
        ("Trim", egui::Modifiers::NONE),
        ("Soft clip amount", egui::Modifiers::NONE),
        ("Trim", egui::Modifiers::SHIFT),
        ("Soft clip amount", egui::Modifiers::SHIFT),
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

#[test]
fn reduced_meter_display_holds_values_and_retains_peak_events() {
    let mut display = view::MeterDisplay::default();
    let scope = [(0.0, 0.0); mixjam_techno::application::VECTORSCOPE_POINTS];
    let mut sample = |reading, now| display.sample(reading, scope, true, now).0;
    let mut reading = MeterReading {
        run: 1,
        input_vu: -4.0,
        ..MeterReading::default()
    };
    assert_eq!(sample(reading, 0.0).input_vu, -4.0);
    reading.input_vu = -2.0;
    reading.input_peak_counts = [1, 0];
    reading.overs = 1;
    let held = sample(reading, 0.02);
    assert_eq!(held.input_vu, -4.0);
    assert_eq!(held.overs, 0);
    reading.input_vu = -6.0;
    let shown = sample(reading, 0.07);
    assert_eq!(shown.input_vu, -6.0);
    assert_eq!(shown.overs, 1);
    assert_eq!(shown.input_peaks, [true, false]);
    assert_eq!(sample(reading, 0.09), shown);
    assert!(!sample(reading, 0.14).input_peaks[0]);
    reading.run = 2;
    reading.input_peak_counts = [0, 0];
    reading.overs = 0;
    let restarted = sample(reading, 0.15);
    assert_eq!(restarted.input_peaks, [false, false]);
    assert_eq!(restarted.overs, 0);
}

#[test]
fn reduced_meter_display_holds_the_vectorscope_with_its_reading() {
    let points = |value: f32| [(value, -value); mixjam_techno::application::VECTORSCOPE_POINTS];
    let reading = |correlation| MeterReading {
        run: 1,
        stereo_correlation: correlation,
        ..MeterReading::default()
    };
    let mut display = view::MeterDisplay::default();
    assert_eq!(
        display.sample(reading(0.1), points(0.1), true, 0.0),
        (reading(0.1), points(0.1))
    );
    // Within 1/15 s, a reduced display holds the points with the reading.
    let (held, scope) = display.sample(reading(0.2), points(0.2), true, 0.02);
    assert_eq!(held.stereo_correlation, 0.1);
    assert_eq!(scope, points(0.1));
    let (shown, scope) = display.sample(reading(0.3), points(0.3), true, 0.07);
    assert_eq!(shown.stereo_correlation, 0.3);
    assert_eq!(scope, points(0.3));
    // A normal display shows every frame's points.
    let (_, scope) = display.sample(reading(0.4), points(0.4), false, 0.08);
    assert_eq!(scope, points(0.4));
}

#[test]
fn reduced_meter_needle_does_not_interpolate_between_display_samples() {
    let ctx = egui::Context::default();
    let id = egui::Id::new("reduced-needle-test");
    for (index, target) in [0.2_f32, 0.8, 0.4, 0.9].into_iter().enumerate() {
        let _ = ctx.run(
            egui::RawInput {
                time: Some(index as f64 * 0.02),
                ..Default::default()
            },
            |ctx| {
                assert_eq!(master::display_needle(ctx, id, target, true), target);
            },
        );
    }
    let reduced = master::Metering {
        reading: MeterReading::default(),
        vectorscope: [(0.0, 0.0); mixjam_techno::application::VECTORSCOPE_POINTS],
        playing: true,
        reduced: true,
    };
    assert!(reduced.repaint_delay().as_secs_f64() >= 1.0 / 15.0);
    let normal = master::Metering {
        reduced: false,
        ..reduced
    };
    assert_eq!(normal.repaint_delay(), Duration::from_millis(33));
}

#[test]
fn mastering_compact_board_fits_960_by_720_with_no_overlap_across_themes() {
    let size = Vec2::new(960.0, 720.0);
    for _ in themes_in_turn() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        ctx.style_mut(|style| style.animation_time = 0.0);
        app.boards.module = BottomModule::Mastering;
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let board = effects_control_rect(&output, "Mastering board");
        let trim = effects_control_rect(&output, "Trim");
        let row = effects_control_rect(&output, "Drag Booster EQ");
        let clear_over = effects_control_rect(&output, "Clear OVER");
        let chip = effects_control_rect(&output, "Show Limiter");
        assert!(board.contains_rect(trim), "Trim must fit inside board.");
        assert!(
            board.contains_rect(clear_over),
            "Output Clear OVER must fit."
        );
        assert!(
            board.contains_rect(chip),
            "Signal chip strip must fit at 960x720."
        );
        assert!(
            trim.right() < row.left(),
            "Input and pedals must not overlap."
        );
        assert!(
            row.right() < clear_over.left(),
            "Pedals and Output must not overlap."
        );
    }
}

#[test]
fn mastering_deep_edit_opens_edits_eq_and_saturation_with_undo() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    click_named(&ctx, &mut app, "Edit Booster EQ");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.master_deep_edit, Some(PedalKind::BoosterEq));
    assert_eq!(
        accessible_node(&output, "Deep edit Booster EQ 1100 Hz value")
            .1
            .value(),
        Some("+0.0")
    );
    click_named(&ctx, &mut app, "Deep edit pedal power");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.session.project().master.booster_eq.on);
    focus_control(&ctx, &mut app, &output, "Deep edit Booster EQ 60 Hz");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(app.session.project().master.booster_eq.bands[0] > 50);

    click_named(&ctx, &mut app, "Deep edit select Tape");
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.master_deep_edit, Some(PedalKind::Tape));
    click_named(&ctx, &mut app, "Deep edit pedal power");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.session.project().master.tape.on);
    focus_control(&ctx, &mut app, &output, "Deep edit Tape drive");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::End, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().master.tape.drive, 10.0);
    assert!(app.session.history_step(false).unwrap());
    assert_eq!(
        app.session.project().master.tape.drive,
        MasterChain::default().tape.drive
    );
}

#[test]
fn mastering_deep_edit_tape_speed_dial_matches_its_compact_neighbours() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    click_named(&ctx, &mut app, "Edit Booster EQ");
    click_named(&ctx, &mut app, "Deep edit select Limiter");
    // Off Mastering pedals allow pre-dialing in Deep Edit without switching on.
    assert!(!app.session.project().master.is_on(PedalKind::Limiter));
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        !accessible_node(&output, "Deep edit Limiter gain")
            .1
            .is_disabled()
    );
    focus_control(&ctx, &mut app, &output, "Deep edit Limiter gain");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_ne!(
        app.session.project().master.limiter.gain,
        MasterChain::default().limiter.gain
    );
    assert!(!app.session.project().master.is_on(PedalKind::Limiter));
    assert!(app.session.history_step(false).unwrap());

    click_named(&ctx, &mut app, "Deep edit select Tape");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.master_deep_edit, Some(PedalKind::Tape));
    let speed = effects_control_rect(&output, "Deep edit Tape speed value");
    let drive = effects_control_rect(&output, "Deep edit Tape drive value");
    assert_eq!(speed.width(), drive.width());
    // The label under each dial in the grid has the same letter size.
    let label_height = |label: &str, column: Rect| {
        screen(&output)
            .texts()
            .find_map(|text| {
                let bounds = text.rect();
                (text.label() == label && (bounds.center().x - column.center().x).abs() < 1.0)
                    .then_some(bounds.height())
            })
            .unwrap_or_else(|| panic!("Missing Deep Edit label: {label}"))
    };
    assert_eq!(label_height("Speed", speed), label_height("Drive", drive));
}

#[test]
fn mastering_deep_edit_does_not_carry_a_trim_drag_into_a_new_mix() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    let output = click_named(&ctx, &mut app, "Edit Booster EQ");
    let dial = effects_control_rect(&output, "Deep edit Trim").center();
    let none = egui::Modifiers::NONE;
    pointer(&ctx, &mut app, dial, true, none);
    for rise in [15.0, 30.0] {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(dial - Vec2::new(0.0, rise))],
            none,
        );
    }
    assert_ne!(app.session.project().master.trim, 0.0);

    app.new_mix();
    let end = dial - Vec2::new(0.0, 60.0);
    frame(&ctx, &mut app, vec![egui::Event::PointerMoved(end)], none);
    pointer(&ctx, &mut app, end, false, none);
    assert_eq!(app.session.project().master.trim, 0.0);
}

#[test]
fn mastering_deep_edit_keeps_telemetry_clear_and_does_not_resize() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    click_named(&ctx, &mut app, "Edit Booster EQ");
    let output = click_named(&ctx, &mut app, "Deep edit select Bus compressor");
    let peak = accessible_label(&output, "True peak: - dBTP");
    let over = effects_control_rect(&output, "Deep edit Clear OVER");
    assert!(peak.bottom() < over.top());
    let window = dialog::window_rect(&ctx, master::DEEP_EDIT_TITLE).unwrap();
    let corner = window.right_bottom() - Vec2::splat(2.0);
    let none = egui::Modifiers::NONE;
    drag(
        &ctx,
        &mut app,
        corner,
        corner - Vec2::new(160.0, 160.0),
        none,
    );
    let output = frame(&ctx, &mut app, vec![], none);
    let after = dialog::window_rect(&ctx, master::DEEP_EDIT_TITLE).unwrap();
    assert_eq!(after.size(), window.size());
    assert_fixed_window_holds_controls(&ctx, &output, master::DEEP_EDIT_TITLE, "Deep edit");
}

#[test]
fn mastering_deep_edit_shows_every_pedal_whole() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    let output = click_named(&ctx, &mut app, "Edit Booster EQ");
    let pedals: Vec<String> = screen(&output)
        .nodes()
        .filter_map(|node| node.node().label())
        .filter(|label| label.starts_with("Deep edit select "))
        .map(str::to_owned)
        .collect();
    assert_eq!(pedals.len(), PedalKind::ALL.len());
    let none = egui::Modifiers::NONE;
    for pedal in pedals {
        click_named(&ctx, &mut app, &pedal);
        frame(&ctx, &mut app, vec![], none);
        let output = frame(&ctx, &mut app, vec![], none);
        assert_fixed_window_holds_controls(&ctx, &output, master::DEEP_EDIT_TITLE, "Deep edit");
    }
}

#[test]
fn mastering_help_opens_beside_the_board_after_deep_edit_closes() {
    let place = |open_deep_edit: bool| {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        ctx.style_mut(|style| style.animation_time = 0.0);
        app.boards.module = BottomModule::Mastering;
        let none = egui::Modifiers::NONE;
        frame(&ctx, &mut app, vec![], none);
        if open_deep_edit {
            click_named(&ctx, &mut app, "Edit Booster EQ");
            assert!(dialog::window_rect(&ctx, master::DEEP_EDIT_TITLE).is_some());
            app.master_deep_edit = None;
            frame(&ctx, &mut app, vec![], none);
        }
        app.help.open_tool(help::Topic::Mastering);
        frame(&ctx, &mut app, vec![], none);
        frame(&ctx, &mut app, vec![], none);
        dialog::window_rect(&ctx, "Mastering help").unwrap()
    };
    assert_eq!(place(true), place(false));
}

#[test]
fn a_silent_deep_edit_bay_names_its_reason_beside_its_title() {
    // The motif art fills the right of the header: the Tube vents and the
    // Booster wide blocks. The status must not paint over it.
    for (kind, bypass, title) in [
        (PedalKind::Tube, false, "Tube · Pedal off"),
        (
            PedalKind::BoosterWide,
            true,
            "Booster wide · Chain bypassed",
        ),
    ] {
        let (ctx, mut app) = fixture();
        app.boards.module = BottomModule::Mastering;
        let chain = MasterChain {
            bypass,
            ..MasterChain::default()
        };
        app.session.edit(EditCommand::SetMaster(chain)).unwrap();
        app.master_deep_edit = Some(kind);
        let none = egui::Modifiers::NONE;
        frame(&ctx, &mut app, vec![], none);
        let output = frame(&ctx, &mut app, vec![], none);
        let screen = screen(&output);
        assert!(screen.shows(title), "{title}");
        for tag in ["Pedal off", "Chain bypassed"] {
            assert!(!screen.shows(tag), "{tag} paints apart from the title.");
        }
    }
}

#[test]
fn mastering_deep_edit_chips_keep_focus_across_arrow_steps() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    let output = click_named(&ctx, &mut app, "Edit Booster EQ");
    // The chip in signal slot `slot`, counted from 0, by its "Slot N of M" value.
    let chip = |slot: usize| -> String {
        let value = format!("Slot {} of ", slot + 1);
        screen(&output)
            .nodes()
            .find(|node| {
                node.node()
                    .label()
                    .is_some_and(|label| label.starts_with("Deep edit select "))
                    && node.node().value().is_some_and(|v| v.starts_with(&value))
            })
            .and_then(|node| node.node().label())
            .map(str::to_owned)
            .unwrap()
    };
    let chips: Vec<String> = (0..3).map(chip).collect();
    let order = app.session.project().master.order;
    focus_control(&ctx, &mut app, &output, &chips[0]);
    let none = egui::Modifiers::NONE;
    for step in [1, 2] {
        press(&ctx, &mut app, egui::Key::ArrowRight, none);
        let output = frame(&ctx, &mut app, vec![], none);
        assert_eq!(app.master_deep_edit, Some(order[step]));
        assert_eq!(focused_label(&output), Some(chips[step].as_str()));
    }
}

#[test]
fn mastering_strip_double_click_opens_the_chosen_pedal() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    let none = egui::Modifiers::NONE;
    frame(&ctx, &mut app, vec![], none);
    let output = frame(&ctx, &mut app, vec![], none);
    let chip = effects_control_rect(&output, "Show Limiter").center();
    click(&ctx, &mut app, chip, none);
    assert!(app.master_deep_edit.is_none());
    click(&ctx, &mut app, chip, none);
    assert_eq!(app.master_deep_edit, Some(PedalKind::Limiter));
}

#[test]
fn mastering_deep_edit_reset_preset_value_and_transfer_scope_work() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::Mastering;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = click_named(&ctx, &mut app, "Edit Soft clip");
    accessible_node(&output, "Soft clip transfer curve");
    assert!(
        accessible_node(&output, "Deep edit preset")
            .1
            .value()
            .is_some(),
        "Deep edit preset must expose selected value"
    );

    click_named(&ctx, &mut app, "Deep edit pedal power");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Deep edit Soft clip amount");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::End, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let max_amount = app.session.project().master.soft_clip.amount;
    assert_ne!(max_amount, MasterChain::default().soft_clip.amount);

    click_named(&ctx, &mut app, "Deep edit reset pedal");
    assert_eq!(
        app.session.project().master.soft_clip.amount,
        MasterChain::default().soft_clip.amount
    );
    assert!(app.session.project().master.soft_clip.on);
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    assert_eq!(app.session.project().master.soft_clip.amount, max_amount);
}
