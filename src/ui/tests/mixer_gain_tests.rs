//! Mixer gain, pan and polarity: dB labels, silence, numeric entry, tapers and routes.

use super::*;

#[test]
fn mixer_db_gain_labels_do_not_change_saved_amplitude_precision() {
    let (ctx, mut app) = fixture();
    let mut project = app.session.project().clone();
    project.tracks[0].gain = 0.7533;
    project.tracks[0].polarity_inverted = true;
    project.tracks[1].gain = 0.3333;
    project.master_gain = 0.7777;
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for label in ["-2.5 dB", "-9.5 dB", "-2.2 dB"] {
        text_rect(&output, label);
    }
    assert_eq!(app.session.project().tracks[0].gain, 0.7533);
    assert!(app.session.project().tracks[0].polarity_inverted);
    assert_eq!(app.session.project().tracks[1].gain, 0.3333);
    assert_eq!(app.session.project().master_gain, 0.7777);
    assert!(!app.session.unsaved());

    // A passive display or focus change must not round-trip amplitude
    // through its rounded dB label and change the saved sound.
    focus_control(&ctx, &mut app, &output, "Track 2 gain value");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    // The edit text has one decimal, like the label (0.3333 is -9.54 dB).
    assert!(
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| node.value() == Some("-9.5")),
        "The focused gain value reads -9.5."
    );
    focus_control(&ctx, &mut app, &output, "Master gain value");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Track 1 left output");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.session.project().tracks[1].gain, 0.3333);
    assert_eq!(app.session.project().master_gain, 0.7777);
    assert!(!app.session.unsaved());

    // Explicitly entering a rounded dB value is a real amplitude edit.
    focus_control(&ctx, &mut app, &output, "Track 2 gain value");
    frame(
        &ctx,
        &mut app,
        vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("-9.5".into()),
        ],
        egui::Modifiers::COMMAND,
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!((app.session.project().tracks[1].gain - 10.0_f32.powf(-9.5 / 20.0)).abs() < 0.000_001);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (_, gain) = accessible_node(&output, "Track 2 gain");
    assert!((gain.numeric_value().unwrap() + 9.5).abs() < 0.001);
    assert_eq!(app.session.project().tracks[0].gain, 0.7533);
    assert!(app.session.project().tracks[0].polarity_inverted);
    assert!(!app.session.project().tracks[1].polarity_inverted);
    assert_eq!(app.session.project().master_gain, 0.7777);
}

#[test]
fn mixer_increasing_silent_and_very_quiet_gain_does_not_stick_or_silence() {
    let (ctx, mut app) = fixture();
    let mut project = app.session.project().clone();
    project.tracks[0].gain = 0.0;
    project.tracks[1].gain = 0.000_001;
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Track 1 gain value");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        app.session.project().tracks[0].gain > 0.0,
        "Increasing the numeric gain from silence must produce a positive gain."
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Track 2 gain");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        app.session.project().tracks[1].gain > 0.000_001,
        "Increasing a gain below -96 dB must increase it instead of silencing it."
    );
}

#[test]
fn mixer_numeric_silence_origin_preserves_focus_and_exact_entries() {
    for initial_gain in [0.0, 10.0_f32.powf(-96.0 / 20.0), 0.000_001] {
        let (ctx, mut app) = fixture();
        let mut project = app.session.project().clone();
        project.tracks[0].gain = initial_gain;
        app.session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            project,
        )
        .unwrap();
        ctx.enable_accesskit();
        app.boards.module = BottomModule::Mixer;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 gain value");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 left output");
        assert_eq!(
            app.session.project().tracks[0].gain.to_bits(),
            initial_gain.to_bits()
        );
        assert!(!app.session.unsaved());

        for entry in ["-inf", "-120", "-inf", "-96", "-inf"] {
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            focus_control(&ctx, &mut app, &output, "Track 1 gain value");
            frame(
                &ctx,
                &mut app,
                vec![
                    key_event(egui::Key::A, egui::Modifiers::COMMAND),
                    egui::Event::Text(entry.into()),
                ],
                egui::Modifiers::COMMAND,
            );
            frame(
                &ctx,
                &mut app,
                vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
                egui::Modifiers::NONE,
            );
            let expected = if entry == "-inf" {
                0.0
            } else {
                mixjam_techno::application::db_to_amplitude(entry.parse::<f32>().unwrap())
            };
            assert_eq!(
                app.session.project().tracks[0].gain,
                expected,
                "Explicit {entry} must retain its meaning."
            );
        }
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 gain value");
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::ArrowDown, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        assert_eq!(
            app.session.project().tracks[0].gain,
            0.0,
            "Decreasing a silent numeric control must keep silence."
        );
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        focus_control(&ctx, &mut app, &output, "Track 1 left output");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            app.session.project().tracks[0].gain,
            0.0,
            "Blurring after a decrease from silence must not commit a finite draft."
        );
        let rect = effects_control_rect(&output, "Track 1 gain value");
        let pos = rect.center();
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    pressed: true,
                    button: egui::PointerButton::Primary,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
        );
        let pos = pos + Vec2::new(24.0, 0.0);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(pos)],
            egui::Modifiers::NONE,
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerButton {
                pos,
                pressed: false,
                button: egui::PointerButton::Primary,
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        );
        assert!(
            app.session.project().tracks[0].gain > 0.0,
            "Dragging the numeric control up from silence must raise gain."
        );
    }
}

#[test]
fn mixer_retains_legacy_gain_four_until_an_explicit_db_edit() {
    let (ctx, mut app) = fixture();
    let mut project = app.session.project().clone();
    project.tracks[0].gain = 4.0;
    project.tracks[0].polarity_inverted = true;
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "+12.0 dB");
    assert_eq!(app.session.project().tracks[0].gain, 4.0);
    assert!(!app.session.unsaved());
    focus_control(&ctx, &mut app, &output, "Track 1 gain value");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Track 1 left output");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.session.project().tracks[0].gain,
        4.0,
        "Focusing a legacy gain must preserve its exact amplitude above +12 dB."
    );
    assert!(app.session.project().tracks[0].polarity_inverted);
    assert!(!app.session.unsaved());

    focus_control(&ctx, &mut app, &output, "Track 1 gain value");
    frame(
        &ctx,
        &mut app,
        vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("+12".into()),
        ],
        egui::Modifiers::COMMAND,
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        (app.session.project().tracks[0].gain - 10.0_f32.powf(12.0 / 20.0)).abs() < 0.000_001,
        "An explicit +12 dB edit must use the new fader ceiling rather than legacy amplitude four."
    );
    assert!(app.session.project().tracks[0].polarity_inverted);
    assert!(app.session.unsaved());
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].gain, 4.0);
    assert!(app.session.project().tracks[0].polarity_inverted);
}

#[test]
fn mixer_gain_pan_and_routes_support_accessibility_and_keyboard_input() {
    let (ctx, mut app) = fixture();
    let mut project = app.session.project().clone();
    project.tracks[1].gain = 0.3333;
    project.master_gain = 0.7777;
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (_, gain) = accessible_node(&output, "Track 1 gain");
    assert_eq!(gain.role(), egui::accesskit::Role::Slider);
    assert_eq!(gain.numeric_value(), Some(0.0));
    assert!(gain.min_numeric_value().is_some_and(|db| db <= -60.0));
    assert_eq!(gain.max_numeric_value(), Some(12.0));
    accessible_node(&output, "Track 2 gain");
    accessible_node(&output, "Master gain");
    let (_, left) = accessible_node(&output, "Track 1 left output");
    assert_eq!(left.role(), egui::accesskit::Role::CheckBox);
    assert_eq!(left.toggled(), Some(egui::accesskit::Toggled::True));
    focus_control(&ctx, &mut app, &output, "Track 1 left output");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(!app.session.project().tracks[0].left_enabled);
    assert!(app.session.project().tracks[1].left_enabled);
    assert_eq!(
        accessible_node(&output, "Track 1 left output").1.toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    focus_control(&ctx, &mut app, &output, "Track 1 right output");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(!app.session.project().tracks[0].right_enabled);
    text_rect(&output, "No output");

    focus_control(&ctx, &mut app, &output, "Track 1 gain value");
    frame(
        &ctx,
        &mut app,
        vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("-6.0".into()),
        ],
        egui::Modifiers::COMMAND,
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!((app.session.project().tracks[0].gain - 10.0_f32.powf(-6.0 / 20.0)).abs() < 0.000_001);
    assert!(
        !app.session.project().tracks[0].polarity_inverted,
        "A negative dB value attenuates without reversing polarity."
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Track 1 polarity invert");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(app.session.project().tracks[0].polarity_inverted);
    assert!((app.session.project().tracks[0].gain - 10.0_f32.powf(-6.0 / 20.0)).abs() < 0.000_001);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        accessible_node(&output, "Track 1 polarity invert")
            .1
            .toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    focus_control(&ctx, &mut app, &output, "Reset Track 1 gain to unity");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().tracks[0].gain, 1.0);
    assert!(
        app.session.project().tracks[0].polarity_inverted,
        "Unity changes magnitude without changing polarity."
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Track 1 gain");
    for (key, modifiers, expected_db) in [
        (egui::Key::ArrowUp, egui::Modifiers::SHIFT, 0.1_f32),
        (egui::Key::ArrowDown, egui::Modifiers::SHIFT, 0.0),
        (egui::Key::ArrowDown, egui::Modifiers::NONE, -0.5),
        (egui::Key::Home, egui::Modifiers::NONE, f32::NEG_INFINITY),
        (
            egui::Key::ArrowDown,
            egui::Modifiers::NONE,
            f32::NEG_INFINITY,
        ),
        (
            egui::Key::ArrowDown,
            egui::Modifiers::SHIFT,
            f32::NEG_INFINITY,
        ),
        (egui::Key::End, egui::Modifiers::NONE, 12.0),
        (egui::Key::ArrowUp, egui::Modifiers::NONE, 12.0),
    ] {
        frame(&ctx, &mut app, vec![key_event(key, modifiers)], modifiers);
        let expected_gain = 10.0_f32.powf(expected_db / 20.0);
        assert!(
            (app.session.project().tracks[0].gain - expected_gain).abs() < 0.000_001,
            "{key:?} with {modifiers:?} must set {expected_db} dB; lowering silence must keep exact zero."
        );
        assert!(
            app.session.project().tracks[0].polarity_inverted,
            "Gain rail keys must leave polarity unchanged."
        );
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Track 1 pan");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowRight, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(app.session.project().tracks[0].pan > 0.0);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Center Track 1 pan");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().tracks[0].pan, 0.0);
    assert_eq!(app.session.project().tracks[1].gain, 0.3333);
    assert_eq!(app.session.project().master_gain, 0.7777);
}

#[test]
fn mixer_polarity_survives_silence_history_and_project_save() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Track 1 gain value");
    frame(
        &ctx,
        &mut app,
        vec![
            key_event(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("-inf".into()),
        ],
        egui::Modifiers::COMMAND,
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().tracks[0].gain, 0.0);
    assert!(!app.session.project().tracks[0].polarity_inverted);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Track 1 polarity invert");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().tracks[0].gain, 0.0);
    assert!(
        app.session.project().tracks[0].polarity_inverted,
        "Polarity must remain a real independent setting at silence."
    );
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].gain, 0.0);
    assert!(!app.session.project().tracks[0].polarity_inverted);
    app.history_step(true);
    assert_eq!(app.session.project().tracks[0].gain, 0.0);
    assert!(app.session.project().tracks[0].polarity_inverted);

    let path = std::env::temp_dir().join(format!(
        "mixjam-ui-mixer-polarity-{}.json",
        std::process::id()
    ));
    app.session.project().save(&path).unwrap();
    let loaded = Project::load(&path).unwrap();
    assert_eq!(loaded.tracks[0].gain, 0.0);
    assert!(loaded.tracks[0].polarity_inverted);
    std::fs::remove_file(path).unwrap();

    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Reset Track 1 gain to unity");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().tracks[0].gain, 1.0);
    assert!(app.session.project().tracks[0].polarity_inverted);
    assert_eq!(app.session.project().tracks[0].effective_gain(), -1.0);
    assert_eq!(app.session.project().tracks[1].gain, 1.0);
    assert!(!app.session.project().tracks[1].polarity_inverted);
}

#[test]
fn mixer_gain_taper_reaches_silence_and_favors_musically_useful_levels() {
    assert_eq!(mixer::fraction_gain(0.0), 0.0);
    assert_eq!(mixer::gain_fraction(0.0), 0.0);
    assert!((20.0 * mixer::fraction_gain(1.0).log10() - 12.0).abs() < 0.000_01);
    let useful_travel = mixer::gain_fraction(10.0_f32.powf(12.0 / 20.0))
        - mixer::gain_fraction(10.0_f32.powf(-24.0 / 20.0));
    assert!(
        useful_travel >= 0.65,
        "Most travel must serve -24 through +12 dB rather than near-silent levels."
    );
    assert!(
        mixer::gain_fraction(10.0_f32.powf(-60.0 / 20.0)) <= 0.12,
        "The near-silent range must use only a short part of the rail."
    );
    let mut previous = 0.0;
    for position in 1..=1000 {
        let fraction = position as f32 / 1000.0;
        let gain = mixer::fraction_gain(fraction);
        assert!(
            gain > previous,
            "Increasing the fader must always increase amplitude."
        );
        assert!((mixer::gain_fraction(gain) - fraction).abs() < 0.000_01);
        previous = gain;
    }
    for gain in [0.000_000_01, 0.001, 0.01, 0.1, 0.5, 1.0, 2.0] {
        let restored = mixer::fraction_gain(mixer::gain_fraction(gain));
        assert!(
            (restored - gain).abs() <= gain * 0.000_01,
            "The rail must round-trip small positive amplitudes without an artificial silence floor."
        );
    }
}

#[test]
fn a_focused_send_knob_leaves_space_to_the_transport() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Track 1 send FX 1");
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let focused = ctx.memory(|memory| memory.focused());
    assert!(focused.is_some());
    assert_eq!(controls::arrow_key_focus(&ctx), focused);
}
