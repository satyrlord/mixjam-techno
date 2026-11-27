//! The source states, and the settings column's options: what stays
//! available without audio, and which action each key returns.

use super::*;

/// The toolbar and the board alone in a window, without the app, for a
/// source in one state.
struct Standalone {
    ctx: egui::Context,
    state: StepSynthState,
    view_bar: usize,
    effects: bool,
    available: bool,
    missing: bool,
    explicit_file: bool,
}

impl Standalone {
    fn new(available: bool, missing: bool, explicit_file: bool) -> Self {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        ctx.enable_accesskit();
        Self {
            ctx,
            state: StepSynthState::default(),
            view_bar: 0,
            effects: false,
            available,
            missing,
            explicit_file,
        }
    }

    /// One frame with `events`. Returns the output and the action the board
    /// or Import details returned.
    fn render(&mut self, events: Vec<egui::Event>) -> (egui::FullOutput, step_synth::Action) {
        let mut action = step_synth::Action::None;
        let Self {
            ctx,
            state,
            view_bar,
            effects,
            available,
            missing,
            explicit_file,
        } = self;
        let source = |synth_mode| step_synth::Source {
            name: "Source",
            available: *available,
            missing: *missing,
            explicit_file: *explicit_file,
            synth_mode,
            length_ms: None,
        };
        let is_synth = |s: &StepSynthState| {
            s.source_mode == mixjam_techno::application::pattern::SourceMode::Synth
        };
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                focused: true,
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    // Import details sits in the top row above the board.
                    let synth_mode = is_synth(state);
                    let bar = ui
                        .horizontal(|ui| {
                            step_synth::toolbar(
                                ui,
                                800.0,
                                source(synth_mode),
                                effects,
                                step_synth::History {
                                    undo: false,
                                    redo: false,
                                    icons: None,
                                },
                            )
                        })
                        .inner;
                    let synth_mode = is_synth(state);
                    action = step_synth::show(
                        ui,
                        source(synth_mode),
                        state,
                        view_bar,
                        *effects,
                        &mut Default::default(),
                        None,
                        None,
                    );
                    if matches!(bar, step_synth::Action::Details) {
                        action = bar;
                    }
                });
            },
        );
        (output, action)
    }
}

#[test]
fn step_synth_source_readiness_disables_audio_actions_but_keeps_draft_and_recovery_available() {
    for (available, missing, explicit_file) in [
        (false, false, false),
        (false, true, false),
        (true, false, false),
        (true, true, true),
    ] {
        let mut board = Standalone::new(available, missing, explicit_file);
        for _ in 0..3 {
            board.render(vec![]);
        }
        let (mut output, _) = board.render(vec![]);
        let ready = available && (!missing || explicit_file);
        for label in ["Listen", "Save sound"] {
            assert_eq!(
                !accessible_node(&output, label).1.is_disabled(),
                ready,
                "{label}: available={available}, missing={missing}, file={explicit_file}"
            );
        }
        for label in [
            "Source file…",
            "Palette sound",
            "Save pattern",
            "Load pattern",
            "Length",
            "Gate",
            "Loop source",
            "Step synth BPM",
            "Step 1, +12 semitones",
        ] {
            assert!(
                !accessible_node(&output, label).1.is_disabled(),
                "{label} must remain available for editing or recovery."
            );
        }
        let mut actions = vec![
            ("Source file…", step_synth::Action::ChooseSource),
            ("Palette sound", step_synth::Action::PaletteSource),
            ("Save pattern", step_synth::Action::SavePattern),
            ("Load pattern", step_synth::Action::LoadPattern),
        ];
        if missing && !explicit_file {
            assert_full_recovery_caption(&output);
            actions.push(("Import details", step_synth::Action::Details));
        }
        if ready {
            actions.push(("Listen", step_synth::Action::Listen));
            actions.push(("Save sound", step_synth::Action::SaveSound));
        }
        for (label, expected) in actions {
            let target = accessible_node(&output, label).0;
            board.render(vec![access_request(target, egui::accesskit::Action::Focus)]);
            let (_, action) =
                board.render(vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)]);
            assert_eq!(
                std::mem::discriminant(&action),
                std::mem::discriminant(&expected),
                "{label} must return its Session action without performing audio or file work in the view."
            );
            (output, _) = board.render(vec![]);
        }
    }
}

#[test]
fn step_synth_options_apply_to_the_draft_without_starting_transport() {
    let (ctx, mut app) = board_fixture();
    app.selection.sample = Some(1);
    let output = settle(&ctx, &mut app, 4);
    app.session.take_notice();
    let output = activate_control(&ctx, &mut app, &output, "Loop source");
    assert!(app.step_synth.state.loop_source);
    for (name, key, check) in [
        ("Length", egui::Key::End, 64.0),
        ("Gate", egui::Key::Home, 5.0),
        ("Step synth BPM", egui::Key::End, 180.0),
    ] {
        focus_control(&ctx, &mut app, &output, name);
        press(&ctx, &mut app, key, egui::Modifiers::NONE);
        settle(&ctx, &mut app, 1);
        let state = &app.step_synth.state;
        let value = match name {
            "Length" => f64::from(state.length),
            "Gate" => f64::from(state.gate),
            _ => state.bpm,
        };
        assert_eq!(value, check, "{name}");
    }
    let output = settle(&ctx, &mut app, 1);
    for label in ["Listen", "Save sound"] {
        assert!(!accessible_node(&output, label).1.is_disabled());
    }
    assert_eq!(
        app.step_synth.bar, 0,
        "The pattern length and the viewed bar must stay independent."
    );
    assert!(app.step_synth.state.validate().is_ok());
    // Without an audio device, a Play request leaves this notice.
    assert!(
        !app.session
            .take_notice()
            .is_some_and(|(message, _)| message.contains("Audio output is unavailable")),
        "Step synth options must not start the transport."
    );
}

#[test]
fn the_idle_hint_asks_for_a_source_only_in_sample_mode() {
    let source = |synth_mode| step_synth::Source {
        name: "Source",
        available: false,
        missing: false,
        explicit_file: false,
        synth_mode,
        length_ms: None,
    };
    assert_eq!(
        step_synth::hint(&source(false), false),
        "Choose a source file or select a sound in the palette."
    );
    assert_eq!(
        step_synth::hint(&source(true), false),
        "Click places or clears a note. Drag a note's end to hold it."
    );
    assert_eq!(
        step_synth::hint(&source(true), true),
        "Adjust the pattern effects. Notes returns to the grid."
    );
}

#[test]
fn standalone_synth_mode_enables_listen_and_save_without_sample_and_keeps_notes() {
    use mixjam_techno::application::pattern::{OscWaveform, SourceMode, step_synth_presets};
    let (ctx, mut app) = board_fixture();
    app.selection.sample = None;
    app.step_synth.state.steps[2] = Step::note(7);
    let placed_notes = app.step_synth.state.steps.clone();
    let output = settle(&ctx, &mut app, 4);
    assert!(accessible_node(&output, "Listen").1.is_disabled());
    assert!(accessible_node(&output, "Save sound").1.is_disabled());

    let output = activate_control(&ctx, &mut app, &output, "Source mode Synth");
    assert_eq!(app.step_synth.state.source_mode, SourceMode::Synth);
    assert!(!accessible_node(&output, "Listen").1.is_disabled());
    assert!(!accessible_node(&output, "Save sound").1.is_disabled());
    let output = activate_control(&ctx, &mut app, &output, "Oscillator Square");
    assert_eq!(app.step_synth.state.osc_waveform, OscWaveform::Square);
    let output = activate_control(&ctx, &mut app, &output, "Synth octave up");
    assert_eq!(app.step_synth.state.osc_octave, 1);
    let _output = activate_control(&ctx, &mut app, &output, "Synth octave down");
    assert_eq!(app.step_synth.state.osc_octave, 0);
    app.step_synth.state.osc_octave = 2;
    let output = settle(&ctx, &mut app, 2);
    let up = accessible_node(&output, "Synth octave up").1;
    assert!(up.is_disabled());
    assert_eq!(
        up.description(),
        Some("The oscillator is at its highest octave.")
    );
    assert_eq!(
        accessible_node(&output, "Synth octave down")
            .1
            .description(),
        Some("Play the oscillator one octave lower.")
    );
    app.step_synth.state.osc_octave = 0;

    let acid = step_synth_presets()
        .into_iter()
        .find(|p| p.name == "303 Acid Saw")
        .expect("303 Acid Saw preset");
    acid.apply(&mut app.step_synth.state);
    assert_eq!(app.step_synth.state.steps, placed_notes);
    assert!(app.step_synth.state.validate().is_ok());
    assert!(
        app.session
            .step_synth_pcm(None, &app.step_synth.state)
            .is_ok()
    );
}
