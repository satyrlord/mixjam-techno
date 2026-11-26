//! Mixer: strips, summaries, master visibility, switches, wheel input and FX sends.

use super::*;

#[test]
fn mixer_master_stays_visible_while_scrolling_all_tracks_at_supported_sizes() {
    for size in [
        Vec2::new(960.0, 720.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(1440.0, 1000.0),
    ] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.boards.module = BottomModule::Mixer;
        for track in 2..TRACK_COUNT {
            app.session.place(1, track, 0).unwrap();
        }
        for _ in 0..12 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let master = text_rect(&output, "Master");
        assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(master));
        let bank = text_rect(&output, "Track 1");
        frame_at_size(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(bank.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(-20_000.0, 0.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
            size,
        );
        for _ in 0..60 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        assert_eq!(text_rect(&output, "Master"), master, "Window: {size:?}");
        let last = text_rect(&output, "Track 64");
        assert!(last.right() < master.left(), "Window: {size:?}; {last:?}");
        assert!(
            screen(&output)
                .texts()
                .any(|text| text.label() == "Track 64" && text.clip().contains_rect(last))
        );
        let bounds = accessible_node(&output, "Master gain").1.bounds().unwrap();
        let pos = Pos2::new((bounds.x0 + bounds.x1) as f32 / 2.0, bounds.y0 as f32 + 2.0);
        for pressed in [true, false] {
            frame_at_size(
                &ctx,
                &mut app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                egui::Modifiers::NONE,
                size,
            );
        }
        assert!(app.session.project().master_gain > 1.5, "Window: {size:?}");
    }
}

#[test]
fn arrangement_and_mixer_switches_mirror_states_edits_and_history() {
    fn assert_switches(output: &egui::FullOutput, track: usize, state: &Track) {
        for switch in TrackSwitch::ALL {
            let enabled = switch.get(state);
            for label in [
                format!("Arrangement track {track} {}", switch.name()),
                format!("Track {track} {}", switch.name()),
            ] {
                let node = screen(output).node(&label);
                assert_eq!(
                    node.node().toggled(),
                    Some(if enabled {
                        egui::accesskit::Toggled::True
                    } else {
                        egui::accesskit::Toggled::False
                    }),
                    "{label}"
                );
                let rect = node.rect();
                let painted = screen(output)
                    .texts()
                    .find(|painted| {
                        painted.label() == switch.letter() && rect.contains(painted.rect().min)
                    })
                    .unwrap_or_else(|| panic!("Missing painted state for {label}"));
                assert_eq!(
                    painted.format().color,
                    if enabled {
                        roles().accent
                    } else {
                        roles().muted
                    },
                    "{label}"
                );
            }
        }
    }
    let (ctx, mut app) = fixture();
    let mut project = app.session.project().clone();
    project.clips[1].track = 63;
    project.tracks[0].pan = -0.75;
    project.tracks[63].pan = 0.5;
    project.tracks[63].gain = 1.25;
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    app.boards.module = BottomModule::Mixer;
    ctx.enable_accesskit();
    for (left, right, muted, solo) in [
        (true, true, false, false),
        (true, false, true, false),
        (false, false, false, true),
        (false, true, true, true),
    ] {
        for (switch, on) in TrackSwitch::ALL.into_iter().zip([muted, solo, left, right]) {
            app.session
                .edit(EditCommand::SetSwitch {
                    track: 0,
                    switch,
                    on,
                })
                .unwrap();
        }
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let expected = Track {
            left_enabled: left,
            right_enabled: right,
            muted,
            solo,
            ..app.session.project().tracks[0].clone()
        };
        assert_switches(&output, 1, &expected);
        assert_switches(&output, 64, &app.session.project().tracks[63].clone());
    }
    for (label, track, switch) in [
        ("Arrangement track 1 left output", 0, TrackSwitch::Left),
        ("Track 64 right output", 63, TrackSwitch::Right),
        ("Arrangement track 1 mute", 0, TrackSwitch::Mute),
        ("Track 64 solo", 63, TrackSwitch::Solo),
    ] {
        let before = app.session.project().tracks[track].clone();
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let bounds = accessible_node(&output, label).1.bounds().unwrap();
        click(
            &ctx,
            &mut app,
            Pos2::new(
                (bounds.x0 + bounds.x1) as f32 * 0.5,
                (bounds.y0 + bounds.y1) as f32 * 0.5,
            ),
            egui::Modifiers::NONE,
        );
        let changed = app.session.project().tracks[track].clone();
        let mut expected = before.clone();
        *switch.get_mut(&mut expected) = !switch.get(&before);
        assert_eq!(changed, expected, "{label}");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_switches(&output, track + 1, &changed);
        app.history_step(false);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_switches(&output, track + 1, &before);
        app.history_step(true);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_switches(&output, track + 1, &changed);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Arrangement track 64 left output");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Space, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(!app.session.project().tracks[63].left_enabled);
    assert_switches(&output, 64, &app.session.project().tracks[63].clone());
    assert_eq!(app.session.project().tracks[0].pan, -0.75);
    assert_eq!(app.session.project().tracks[63].pan, 0.5);
}

#[test]
fn mixer_status_names_why_a_track_is_silent() {
    let (ctx, mut app) = fixture();
    let mut project = app.session.project().clone();
    project.clips[1].track = 1;
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    app.boards.module = BottomModule::Mixer;
    // Track 1 shows each cause in turn; the order is Muted, No output, Solo elsewhere.
    for (switches, expected) in [
        (vec![(1, TrackSwitch::Solo, true)], "Solo elsewhere"),
        (
            vec![
                (0, TrackSwitch::Left, false),
                (0, TrackSwitch::Right, false),
            ],
            "No output",
        ),
        (vec![(0, TrackSwitch::Mute, true)], "Muted"),
    ] {
        for (track, switch, on) in switches {
            app.session
                .edit(EditCommand::SetSwitch { track, switch, on })
                .unwrap();
        }
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        text_rect(&output, expected);
    }
}

#[test]
fn muted_and_solo_silenced_lanes_dim_their_track_numbers() {
    fn number_color(output: &egui::FullOutput, track: usize) -> egui::Color32 {
        let rect = screen(output)
            .node(&format!("Record on track {track}"))
            .rect();
        screen(output)
            .texts()
            .find(|text| text.label() == track.to_string() && rect.contains(text.rect().min))
            .map(|text| text.format().color)
            .unwrap_or_else(|| panic!("Missing track number {track}"))
    }
    let (ctx, mut app) = fixture();
    let mut project = app.session.project().clone();
    project.clips[1].track = 1;
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    ctx.enable_accesskit();
    for (track, switch, expected) in [
        (0, TrackSwitch::Mute, [roles().muted, roles().text]),
        (1, TrackSwitch::Solo, [roles().muted, roles().text]),
        (0, TrackSwitch::Mute, [roles().muted, roles().text]),
        (1, TrackSwitch::Solo, [roles().text, roles().text]),
    ] {
        let on = !switch.get(&app.session.project().tracks[track]);
        app.session
            .edit(EditCommand::SetSwitch { track, switch, on })
            .unwrap();
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            [number_color(&output, 1), number_color(&output, 2)],
            expected
        );
    }
}

#[test]
fn mixer_yields_its_extra_height_only_in_a_short_window() {
    // The shared layout gives the same heights in every theme.
    each_theme(|theme| {
        for (size, minimum) in [
            (Vec2::new(1440.0, 1000.0), 400.0),
            (Vec2::new(960.0, 720.0), 370.0),
        ] {
            let (ctx, mut app) = fixture();
            app.boards.module = BottomModule::Mixer;
            for track in 2..5 {
                app.session.place(1, track, 0).unwrap();
            }
            for _ in 0..3 {
                frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let panel = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("sounds"))
                .unwrap()
                .rect;
            assert!(
                panel.height() >= minimum - 0.5,
                "{theme:?} {size:?}: {panel:?}"
            );
            let lanes = app.score.drop_rect.unwrap().height();
            assert!(lanes >= 150.0, "{theme:?} {size:?}: {lanes}");
        }
    });
}

#[test]
fn a_soloed_empty_track_stays_visible_so_its_solo_can_be_cleared() {
    let (ctx, mut app) = fixture();
    app.session
        .edit(EditCommand::SetSwitch {
            track: 40,
            switch: TrackSwitch::Solo,
            on: true,
        })
        .unwrap();
    assert!(app.session.project().occupied_tracks().contains(&40));
    ctx.enable_accesskit();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let bounds = accessible_node(&output, "Arrangement track 41 solo")
        .1
        .bounds()
        .unwrap();
    click(
        &ctx,
        &mut app,
        Pos2::new(
            (bounds.x0 + bounds.x1) as f32 * 0.5,
            (bounds.y0 + bounds.y1) as f32 * 0.5,
        ),
        egui::Modifiers::NONE,
    );
    assert!(!app.session.project().any_solo());
    assert!(!app.session.project().occupied_tracks().contains(&40));
}

#[test]
fn mixer_summaries_and_control_identity_follow_sparse_project_tracks() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let mut samples = app.session.library().samples.to_vec();
    let mut second = samples[0].clone();
    second.id = 2;
    second.name = "Second sound".into();
    samples.push(second);
    let mut project = app.session.project().clone();
    project.clips[1].track = 63;
    app.session = Session::headless(Arc::new(Library::fixture(samples)), project).unwrap();
    app.session.place(2, 0, TICKS_PER_BAR * 2).unwrap();
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "good grasp 2 +1");
    text_rect(&output, "good grasp 2");
    let track64 = accessible_node(&output, "Track 64 gain").0;
    let track1_ids = app
        .session
        .project()
        .clips
        .iter()
        .filter(|clip| clip.track == 0)
        .map(|clip| clip.id)
        .collect();
    app.session
        .edit(EditCommand::DeleteClips(track1_ids))
        .unwrap();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(accessible_node(&output, "Track 64 gain").0, track64);
    focus_control(&ctx, &mut app, &output, "Track 64 gain");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowUp, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(app.session.project().tracks[63].gain > 1.0);
    assert_eq!(app.session.project().tracks[0].gain, 1.0);
    app.session.place(1, 0, 0).unwrap();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(accessible_node(&output, "Track 64 gain").0, track64);
    text_rect(&output, "Track 1");
    text_rect(&output, "Track 64");
}

#[test]
fn mixer_compact_rails_and_inline_resets_fit_in_every_theme() {
    for _ in themes_in_turn() {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        let mut project = app.session.project().clone();
        project.tracks[0].gain = 10.0_f32.powf(12.0 / 20.0);
        project.tracks[1].gain = 0.0;
        app.session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            project,
        )
        .unwrap();
        app.session.set_theme_fixture(theme::current());
        app.boards.module = BottomModule::Mixer;
        let size = Vec2::new(960.0, 720.0);
        let viewport = Rect::from_min_size(Pos2::ZERO, size);
        for _ in 0..12 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let mut caps = Vec::new();
        for track in ["Track 1", "Track 2", "Master"] {
            let gain = effects_control_rect(&output, &format!("{track} gain"));
            let value = effects_control_rect(&output, &format!("{track} gain value"));
            let unity = effects_control_rect(&output, &format!("Reset {track} gain to unity"));
            assert!(
                viewport.contains_rect(gain)
                    && viewport.contains_rect(value)
                    && viewport.contains_rect(unity),
                "{track} gain controls must fit."
            );
            assert!(
                gain.height() >= 86.0,
                "The minimum-window rail must remain usable."
            );
            assert!(
                gain.height() - 32.0 >= 68.0,
                "The physical rail must retain at least 68 points of travel."
            );
            assert!(
                value.bottom() <= gain.top(),
                "The dB readout must have room above its rail."
            );
            // Vintage's bitmap cap, Aero's molded cap, and Studio's flat
            // cap all sit inside the same 24 by 32 point envelope.
            let height = match roles().material {
                theme::Material::Metal => 32.0,
                theme::Material::Plastic => 24.0,
                theme::Material::Flat => 16.0,
            };
            let cap = output
                .shapes
                .iter()
                .find_map(|shape| {
                    let bounds = match &shape.shape {
                        egui::Shape::Mesh(mesh) => mesh.calc_bounds(),
                        egui::Shape::Rect(rect) if rect.fill == roles().cap => rect.rect,
                        _ => return None,
                    };
                    ((bounds.width() - 24.0).abs() <= 1.0
                        && (bounds.height() - height).abs() <= 1.0
                        && (bounds.center().x - gain.center().x).abs() <= 1.0
                        && gain.expand(1.0).contains(bounds.center()))
                    .then_some(bounds)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "Missing {track} fader cap in {}.",
                        options::labels::theme(theme::current())
                    )
                });
            assert!(
                cap.top() - value.bottom() >= 3.0,
                "The fader cap must clear the dB readout even at +12 dB."
            );
            assert!(
                gain.expand(1.0).contains_rect(cap),
                "The whole fader cap must remain draggable at either endpoint."
            );
            caps.push((track, cap));
            assert!(
                gain.y_range().contains(unity.center().y),
                "Unity must sit directly beside the gain rail."
            );
            assert!(
                unity.width() <= 48.0 && unity.height() <= 20.0,
                "Unity must be a small rail button."
            );
        }
        for track in ["Track 1", "Track 2"] {
            let pan = effects_control_rect(&output, &format!("{track} pan"));
            let center = effects_control_rect(&output, &format!("Center {track} pan"));
            let polarity = effects_control_rect(&output, &format!("{track} polarity invert"));
            let left = effects_control_rect(&output, &format!("{track} left output"));
            let right = effects_control_rect(&output, &format!("{track} right output"));
            let mute = effects_control_rect(&output, &format!("{track} mute"));
            let solo = effects_control_rect(&output, &format!("{track} solo"));
            let eq = effects_control_rect(&output, &format!("{track} EQ"));
            for rect in [pan, center, polarity, left, right, mute, solo, eq] {
                assert!(
                    viewport.contains_rect(rect),
                    "{track} lower controls must fit."
                );
            }
            assert!(
                (eq.top() - mute.top()).abs() <= 1.0
                    && eq.right() <= mute.left()
                    && eq.bottom() <= polarity.top()
                    && (eq.center().x - polarity.center().x).abs() <= 1.0,
                "EQ starts the row of Mute and Solo, above polarity."
            );
            for rect in [mute, solo] {
                assert!(
                    rect.top() >= center.bottom() && rect.bottom() <= left.top(),
                    "Mute and Solo sit in their own row between Center and L/R."
                );
            }
            let cap = caps.iter().find(|(name, _)| *name == track).unwrap().1;
            for bus in 1..=4 {
                let send = effects_control_rect(&output, &format!("{track} send FX {bus}"));
                assert!(
                    !send.intersects(cap),
                    "{track} send FX {bus} must clear the fader cap."
                );
                assert!(
                    !send.intersects(pan),
                    "{track} send FX {bus} must clear pan."
                );
                let gain = effects_control_rect(&output, &format!("{track} gain"));
                assert!(
                    !send.intersects(gain),
                    "{track} send FX {bus} must clear the gain hit area."
                );
            }
            assert!(
                pan.top() - cap.bottom() >= 3.0,
                "The fader cap must clear pan even at silence."
            );
            assert!(
                center.top() >= pan.bottom() - 1.0 && center.top() <= pan.bottom() + 4.0,
                "Center must attach just below its pan rail without covering the slider."
            );
            assert!(
                (pan.center().x - center.center().x).abs() <= 1.0,
                "Center must mark the center of the pan rail."
            );
            assert!(center.width() <= 44.0 && center.height() <= 18.0);
            assert!(!polarity.intersects(pan) && !polarity.intersects(center));
            assert!(!polarity.intersects(left) && !polarity.intersects(right));
            assert_eq!(
                accessible_node(&output, &format!("{track} polarity invert"))
                    .1
                    .role(),
                egui::accesskit::Role::CheckBox
            );
        }
        let bypass = effects_control_rect(&output, "Master bypass");
        let master = effects_control_rect(&output, "Master gain");
        assert!(
            viewport.contains_rect(bypass)
                && bypass.top() > master.bottom()
                && bypass.width() <= 108.0,
            "Master bypass must fit under the master rail."
        );
    }
}

#[test]
fn empty_mixer_keeps_guidance_master_and_action_errors_visible() {
    let (ctx, mut app) = fixture();
    app.new_mix();
    app.boards.module = BottomModule::Mixer;
    assert!(DEFAULT_STATUS.starts_with("Space plays the song."));
    let (status, status_error) = startup_status(&mut app.session);
    assert_eq!((status.as_str(), status_error), (DEFAULT_STATUS, false));
    app.status = status;
    app.status_error = status_error;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "NO TRACKS");
    text_rect(&output, "Place a sound in the arrangement.");
    text_rect(&output, "Master");
    text_rect(&output, DEFAULT_STATUS);
    app.error("Cannot open the selected sample.");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "Cannot open the selected sample.");
    assert!(app.status_error);
    assert_eq!(app.status, "Cannot open the selected sample.");
}

#[test]
fn mixer_wheel_moves_the_fader_and_send_under_the_pointer_once_and_never_the_pan() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let wheel = |ctx: &egui::Context, app: &mut MixJamApp, at: Pos2, up: bool| {
        frame(
            ctx,
            app,
            vec![egui::Event::PointerMoved(at)],
            egui::Modifiers::NONE,
        );
        frame(
            ctx,
            app,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: Vec2::new(0.0, if up { 1.0 } else { -1.0 }),
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        );
        for _ in 0..10 {
            frame(ctx, app, vec![], egui::Modifiers::NONE);
        }
    };
    let rail = effects_control_rect(&output, "Track 1 gain");
    wheel(&ctx, &mut app, rail.center(), true);
    let db = amplitude_to_db(app.session.project().tracks[0].gain);
    assert!((db - 0.5).abs() < 0.001, "One notch is 0.5 dB, once: {db}");
    let send = effects_control_rect(&output, "Track 1 send FX 1");
    wheel(&ctx, &mut app, send.center(), true);
    assert!((app.session.project().tracks[0].sends[0] - 0.05).abs() < 0.0001);
    let pan = effects_control_rect(&output, "Track 1 pan");
    wheel(&ctx, &mut app, pan.center(), false);
    assert_eq!(
        app.session.project().tracks[0].pan,
        0.0,
        "The pan leaves the wheel to the view, so a scroll never moves it."
    );
    let master = effects_control_rect(&output, "Master gain");
    let before = app.session.project().master_gain;
    wheel(&ctx, &mut app, master.center(), false);
    assert!(
        app.session.project().master_gain < before,
        "The master fader takes the wheel."
    );
}

#[test]
fn mixer_preserves_unedited_send_and_return_precision() {
    let (ctx, mut app) = fixture();
    let mut project = app.session.project().clone();
    project.tracks[0].sends[0] = 0.12345;
    project.fx.buses[0].return_level = 0.76543;
    app.session = Session::headless(Arc::new(Library::fixture(vec![])), project.clone()).unwrap();
    app.boards.module = BottomModule::Mixer;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    assert_eq!(
        app.session.project().tracks[0].sends,
        project.tracks[0].sends
    );
    assert_eq!(app.session.project().fx, project.fx);
}

#[test]
fn mixer_sends_and_the_effects_module_edit_the_fx_buses() {
    use mixjam_techno::application::fx::{Aetherform, Echoform, Module};
    let set = |ctx: &egui::Context,
               app: &mut MixJamApp,
               output: &egui::FullOutput,
               name: &str,
               value: f64| {
        let (target, _) = accessible_node(output, name);
        frame(
            ctx,
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
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    set(&ctx, &mut app, &output, "Track 1 send FX 2", 0.4);
    assert_eq!(app.session.project().tracks[0].sends, [0.0, 0.4, 0.0, 0.0]);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(!accessible_node(&output, "FX 1 power").1.is_disabled());
    set(&ctx, &mut app, &output, "FX 2 return level", 0.5);
    assert_eq!(app.session.project().fx.buses[1].return_level, 0.5);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Edit FX 2");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert_eq!(app.boards.module, BottomModule::FxReturns);

    let mut buses = app.session.project().fx;
    buses.buses[0].module = Module::EchoformDelay(Echoform::default());
    buses.buses[1].module = Module::AetherformReverb(Aetherform::default());
    app.session.edit(EditCommand::SetEffects(buses)).unwrap();
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for name in [
        "Clear all FX tails",
        "FX 1 effect",
        "FX 1 solo",
        "Deep edit FX 1",
        "FX 2 preset",
        "Clear FX 2 tail",
        "FX 4 limiter",
    ] {
        accessible_node(&output, name);
    }
    app.fx_deep_edit = Some(1);
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for name in [
        "Deep edit bus 1",
        "Deep edit bus 2",
        "Deep edit duck key",
        "Deep edit space Room",
        "Deep edit shimmer",
        "Deep edit power",
        "Deep edit toggle ducker",
        "Deep edit reset effect",
    ] {
        accessible_node(&output, name);
    }
    assert!(
        accessible_node(&output, "Deep edit shimmer amount")
            .1
            .is_disabled(),
        "the amount waits for Shimmer"
    );
    set(&ctx, &mut app, &output, "Deep edit decay", 5.0);
    let Module::AetherformReverb(reverb) = app.session.project().fx.buses[1].module else {
        panic!("FX 2 holds the reverb")
    };
    assert_eq!(reverb.decay_seconds, 5.0);
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    let Module::AetherformReverb(reverb) = app.session.project().fx.buses[1].module else {
        panic!("FX 2 holds the reverb")
    };
    assert_eq!(reverb.decay_seconds, 2.8, "Undo restores the decay.");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    set(&ctx, &mut app, &output, "Deep edit duck key", 4.0);
    let Module::AetherformReverb(reverb) = app.session.project().fx.buses[1].module else {
        panic!("FX 2 holds the reverb")
    };
    assert_eq!(reverb.duck_key, 4);
    app.fx_deep_edit = None;
}

#[test]
fn fx_deep_edit_tab_changes_the_control_target() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    load_reverbs(&mut app, &[0, 1]);
    let output = click_named(&ctx, &mut app, "Deep edit FX 1");
    let old_power = accessible_node(&output, "Deep edit power").0;
    let output = click_named(&ctx, &mut app, "Deep edit bus 2");
    assert_eq!(app.fx_deep_edit, Some(1));
    assert_ne!(accessible_node(&output, "Deep edit power").0, old_power);

    click_named(&ctx, &mut app, "Deep edit power");
    let buses = &app.session.project().fx.buses;
    assert!(buses[0].powered);
    assert!(!buses[1].powered);
}

#[test]
fn fx_deep_edit_tabs_keep_focus_across_arrow_steps() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    load_reverbs(&mut app, &[0, 1, 2, 3]);
    let output = click_named(&ctx, &mut app, "Deep edit FX 1");
    focus_control(&ctx, &mut app, &output, "Deep edit bus 1");
    let none = egui::Modifiers::NONE;
    for expected in [1, 2] {
        press(&ctx, &mut app, egui::Key::ArrowRight, none);
        let output = frame(&ctx, &mut app, vec![], none);
        assert_eq!(app.fx_deep_edit, Some(expected));
        let label = format!("Deep edit bus {}", expected + 1);
        assert_eq!(focused_label(&output), Some(label.as_str()));
    }
}

#[test]
fn fx_deep_edit_takes_the_new_bus_height_on_the_frame_of_the_tab_click() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    // Bus 1 has a reverb and an open ducker; bus 2 is empty, so its
    // workspace is shorter.
    load_reverbs(&mut app, &[0]);
    click_named(&ctx, &mut app, "Deep edit FX 1");
    let none = egui::Modifiers::NONE;
    frame(&ctx, &mut app, vec![], none);
    let output = frame(&ctx, &mut app, vec![], none);
    let full = dialog::window_rect(&ctx, "FX deep edit").unwrap();
    // Only the click frame: the next frame would hide a lag.
    let click = egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
        action: egui::accesskit::Action::Click,
        target: accessible_node(&output, "Deep edit bus 2").0,
        data: None,
    });
    frame(&ctx, &mut app, vec![click], none);
    assert_eq!(app.fx_deep_edit, Some(1));
    let clicked = dialog::window_rect(&ctx, "FX deep edit").unwrap();
    frame(&ctx, &mut app, vec![], none);
    let settled = dialog::window_rect(&ctx, "FX deep edit").unwrap();
    assert!(settled.height() < full.height(), "{settled:?} {full:?}");
    assert_eq!(
        clicked, settled,
        "The click frame draws the new bus at its own height."
    );
}

#[test]
fn fx_deep_edit_is_listed_as_an_agent_window() {
    let (ctx, mut app, port) = agent_fixture();
    app.boards.module = BottomModule::FxReturns;
    load_reverbs(&mut app, &[0]);
    let opened = agent(&ctx, &mut app, &port, &["click", "target=Deep edit FX 1"]);
    assert_eq!(opened["windows"], serde_json::json!(["FX deep edit"]));
    let closed = agent(
        &ctx,
        &mut app,
        &port,
        &["click", "target=Close FX deep edit"],
    );
    assert_eq!(closed["windows"], serde_json::json!([]));
}

#[test]
fn fx_linked_delay_edits_both_channels_and_undoes_one_free_time_gesture() {
    use mixjam_techno::application::fx::{Division, Echoform, Module};
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.session.hold_gestures();
    app.boards.module = BottomModule::FxReturns;
    let mut buses = app.session.project().fx;
    buses.buses[0].module = Module::EchoformDelay(Echoform::default());
    app.session.edit(EditCommand::SetEffects(buses)).unwrap();

    choose(&ctx, &mut app, "FX 1 left division", "1/2");
    click_named(&ctx, &mut app, "Deep edit FX 1");
    let output = click_named(&ctx, &mut app, "Deep edit link times");
    // In Sync the time dials step note lengths; the right one follows the
    // left while linked.
    for name in ["Deep edit right division", "Deep edit right division dial"] {
        assert!(accessible_node(&output, name).1.is_disabled());
    }
    assert!(
        !accessible_node(&output, "Deep edit left division dial")
            .1
            .is_disabled()
    );
    choose(&ctx, &mut app, "Deep edit left division", "1/2.");
    let delay = |app: &MixJamApp| {
        let Module::EchoformDelay(delay) = app.session.project().fx.buses[0].module else {
            panic!("FX 1 holds the delay");
        };
        delay
    };
    assert_eq!(delay(&app).division_l, Division::HalfDotted);
    assert_eq!(delay(&app).division_r, Division::HalfDotted);

    let output = click_named(&ctx, &mut app, "Deep edit sync");
    assert!(
        accessible_node(&output, "Deep edit right time")
            .1
            .is_disabled()
    );
    let initial = delay(&app);
    let (target, _) = accessible_node(&output, "Deep edit left time");
    for value in [800.0, 900.0] {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::SetValue,
                    target,
                    data: Some(egui::accesskit::ActionData::NumericValue(value)),
                },
            )],
            egui::Modifiers::NONE,
        );
        let current = delay(&app);
        assert_eq!([current.time_ms_l, current.time_ms_r], [value as f32; 2]);
    }
    let command = egui::Modifiers::COMMAND;
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Z, command)],
        command,
    );
    assert_eq!(delay(&app), initial);
}

#[test]
fn fx_delay_deep_edit_controls_and_scope_cover_all_bays() {
    use mixjam_techno::application::fx::{DelayCharacter, Echoform, Module};
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    let mut buses = app.session.project().fx;
    buses.buses[0].module = Module::EchoformDelay(Echoform::default());
    app.session.edit(EditCommand::SetEffects(buses)).unwrap();

    let output = click_named(&ctx, &mut app, "Deep edit FX 1");
    assert!(screen(&output).shows("Tap & Decay"));
    assert!(screen(&output).shows("L/R PING"));

    let (target, _) = accessible_node(&output, "Deep edit ping-pong");
    let output = frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target,
                data: None,
            },
        )],
        egui::Modifiers::NONE,
    );
    assert!(screen(&output).shows("L/R DUAL"));
    for (name, val) in [
        ("Deep edit smear", 36.0),
        ("Deep edit filter resonance", 44.0),
        ("Deep edit drive", 25.0),
        ("Deep edit duck amount", 48.0),
    ] {
        let (target, _) = accessible_node(&output, name);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::SetValue,
                    target,
                    data: Some(egui::accesskit::ActionData::NumericValue(val)),
                },
            )],
            egui::Modifiers::NONE,
        );
    }
    click_named(&ctx, &mut app, "Deep edit delay character Analog");
    let Module::EchoformDelay(delay) = app.session.project().fx.buses[0].module else {
        panic!("FX 1 holds Echoform Delay");
    };
    assert!(!delay.ping_pong);
    assert_eq!(delay.smear_percent, 36.0);
    assert_eq!(delay.filter_resonance, 44.0);
    assert_eq!(delay.drive, 25.0);
    assert_eq!(delay.duck_amount, 48.0);
    assert_eq!(delay.character, DelayCharacter::Analog);
}

#[test]
fn fx_card_clear_tail_is_off_for_an_effect_without_a_tail() {
    use mixjam_techno::application::fx::{Echoform, Emberform, Module};
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    let mut buses = app.session.project().fx;
    buses.buses[0].module = Module::EmberformDrive(Emberform::default());
    buses.buses[1].module = Module::EchoformDelay(Echoform::default());
    app.session.edit(EditCommand::SetEffects(buses)).unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(accessible_node(&output, "Clear FX 1 tail").1.is_disabled());
    assert!(!accessible_node(&output, "Clear FX 2 tail").1.is_disabled());
}

#[test]
fn fx_key_strip_keeps_focus_across_arrow_steps() {
    use mixjam_techno::application::fx::{DelayCharacter, Echoform, Module};
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    let mut buses = app.session.project().fx;
    buses.buses[0].module = Module::EchoformDelay(Echoform {
        character: DelayCharacter::Digital,
        ..Echoform::default()
    });
    app.session.edit(EditCommand::SetEffects(buses)).unwrap();
    let output = click_named(&ctx, &mut app, "Deep edit FX 1");
    focus_control(&ctx, &mut app, &output, "Deep edit delay character Digital");
    let none = egui::Modifiers::NONE;
    for (character, label) in [
        (DelayCharacter::Analog, "Deep edit delay character Analog"),
        (DelayCharacter::Tape, "Deep edit delay character Tape"),
    ] {
        press(&ctx, &mut app, egui::Key::ArrowRight, none);
        let output = frame(&ctx, &mut app, vec![], none);
        let Module::EchoformDelay(delay) = app.session.project().fx.buses[0].module else {
            panic!("FX 1 holds Echoform Delay");
        };
        assert_eq!(delay.character, character);
        assert_eq!(focused_label(&output), Some(label));
    }
}

#[test]
fn fx_reverb_deep_edit_controls_and_scope_cover_all_bays() {
    use mixjam_techno::application::fx::{Aetherform, Module, ReverbCharacter, SpaceModel};
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    let mut buses = app.session.project().fx;
    buses.buses[0].module = Module::AetherformReverb(Aetherform::default());
    app.session.edit(EditCommand::SetEffects(buses)).unwrap();

    let output = click_named(&ctx, &mut app, "Deep edit FX 1");
    assert!(screen(&output).shows("Decay & Space"));
    assert!(screen(&output).shows("CHAMBER · VINTAGE"));
    assert!(
        accessible_node(&output, "Deep edit shimmer amount")
            .1
            .is_disabled()
    );
    let output = click_named(&ctx, &mut app, "Deep edit shimmer");
    assert!(screen(&output).shows("CHAMBER · VINTAGE +12"));
    assert!(
        !accessible_node(&output, "Deep edit shimmer amount")
            .1
            .is_disabled()
    );
    for (name, value) in [
        ("Deep edit bass multiplier", 1.4),
        ("Deep edit treble multiplier", 0.6),
        ("Deep edit pre-delay", 40.0),
    ] {
        let (target, _) = accessible_node(&output, name);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::SetValue,
                    target,
                    data: Some(egui::accesskit::ActionData::NumericValue(value)),
                },
            )],
            egui::Modifiers::NONE,
        );
    }
    click_named(&ctx, &mut app, "Deep edit space Hall");
    click_named(&ctx, &mut app, "Deep edit reverb character Bloom");
    let Module::AetherformReverb(reverb) = app.session.project().fx.buses[0].module else {
        panic!("FX 1 holds Aetherform Reverb");
    };
    assert!(reverb.shimmer_enabled);
    assert_eq!(reverb.bass_mult, 1.4);
    assert_eq!(reverb.treble_mult, 0.6);
    assert_eq!(reverb.pre_delay_ms, 40.0);
    assert_eq!(reverb.space_model, SpaceModel::Hall);
    assert_eq!(reverb.character, ReverbCharacter::Bloom);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let readout = accessible_node(&output, "Decay & Space").1.value().unwrap();
    assert!(readout.starts_with("Decay & Space."), "{readout}");
    assert!(readout.contains("Pre-delay 40 ms"), "{readout}");
    assert!(readout.contains("Bass"), "{readout}");
    assert!(readout.contains("Middle"), "{readout}");
    assert!(readout.contains("Treble"), "{readout}");
}

#[test]
fn fx_deep_edit_at_960x720_clears_transport_and_toggles_ducker() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    load_reverbs(&mut app, &[0]);
    click_named(&ctx, &mut app, "Deep edit FX 1");
    let small = Vec2::new(960.0, 720.0);
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, small);
    }
    let win = dialog::window_rect(&ctx, fx::DEEP_EDIT_TITLE).unwrap();
    assert!(
        win.top() >= 68.0,
        "FX Deep Edit must clear transport bar: {win:?}"
    );
    assert!(
        win.bottom() <= 708.0,
        "FX Deep Edit must fit 960x720: {win:?}"
    );
    assert!(
        screen(&output).find_node("Deep edit duck key").is_none(),
        "Ducker dials must start folded on 960x720"
    );
    let toggle = effects_control_rect(&output, "Deep edit toggle ducker").center();
    frame_at_size(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(toggle),
            egui::Event::PointerButton {
                pos: toggle,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: toggle,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        egui::Modifiers::NONE,
        small,
    );
    let expanded = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, small);
    accessible_node(&expanded, "Deep edit duck key");
}

#[test]
fn fx_deep_edit_active_clears_bypass_resets_effect_and_exposes_scope_nodes() {
    use mixjam_techno::application::fx::{Division, Echoform, Module};

    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    let mut buses = app.session.project().fx;
    let delay = Echoform {
        bypass: true,
        mode: mixjam_techno::application::fx::TimeMode::Sync,
        division_l: Division::Whole,
        feedback: 85.0,
        ..Default::default()
    };
    buses.buses[0].module = Module::EchoformDelay(delay);
    buses.buses[0].powered = false;
    app.session.edit(EditCommand::SetEffects(buses)).unwrap();

    click_named(&ctx, &mut app, "Deep edit FX 1");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    accessible_node(&output, "Tap & Decay");
    accessible_node(&output, "Deep edit tail status");

    click_named(&ctx, &mut app, "Deep edit power");
    let bus = app.session.project().fx.buses[0];
    let Module::EchoformDelay(d) = bus.module else {
        panic!("FX 1 holds Echoform Delay");
    };
    assert!(bus.powered && !d.bypass, "Active must clear legacy bypass");

    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let dial_pos = effects_control_rect(&output, "Deep edit left division dial").center();
    click(&ctx, &mut app, dial_pos, egui::Modifiers::NONE);
    click(&ctx, &mut app, dial_pos, egui::Modifiers::NONE);
    let Module::EchoformDelay(d) = app.session.project().fx.buses[0].module else {
        panic!("FX 1 holds Echoform Delay");
    };
    assert_eq!(d.division_l, Echoform::default().division_l);

    click_named(&ctx, &mut app, "Deep edit reset effect");
    let Module::EchoformDelay(d) = app.session.project().fx.buses[0].module else {
        panic!("FX 1 holds Echoform Delay");
    };
    assert_eq!(d.feedback, Echoform::default().feedback);
    assert!(app.session.project().fx.buses[0].powered);
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
    );
    let Module::EchoformDelay(d) = app.session.project().fx.buses[0].module else {
        panic!("FX 1 holds Echoform Delay");
    };
    assert_eq!(d.feedback, 85.0);
}
#[test]
fn fx_meters_expose_their_values_to_assistive_technology() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    load_reverbs(&mut app, &[0]);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let readout = accessible_node(&output, "FX 1 return meters")
        .1
        .value()
        .unwrap();
    assert!(
        readout.contains("Left peak silent, RMS silent"),
        "{readout}"
    );
    assert!(readout.contains("Ducker reduction 0.0 dB"), "{readout}");
    assert!(readout.contains("Limiter reduction 0.0 dB"), "{readout}");
    assert!(readout.contains("Tail idle"), "{readout}");
    let output = click_named(&ctx, &mut app, "Deep edit FX 1");
    assert_eq!(
        accessible_node(&output, "Deep edit return meters")
            .1
            .value(),
        Some(readout)
    );
}

#[test]
fn fx_empty_bus_cannot_solo_in_the_rack_or_deep_edit() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    let output = click_named(&ctx, &mut app, "FX 1 solo");
    assert!(accessible_node(&output, "FX 1 solo").1.is_disabled());
    assert!(!app.session.fx_solo(0));
    // An empty bus has nothing to feed, limit, or edit, and shows no meter.
    for name in ["FX 1 power", "FX 1 limiter", "Deep edit FX 1"] {
        assert!(accessible_node(&output, name).1.is_disabled(), "{name}");
    }
    assert!(
        screen(&output)
            .nodes()
            .all(|node| node.node().label() != Some("FX 1 return meters"))
    );
    app.fx_deep_edit = Some(0);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for name in ["Deep edit solo", "Deep edit power", "Deep edit limiter"] {
        assert!(accessible_node(&output, name).1.is_disabled(), "{name}");
    }
    accessible_node(&output, "Deep edit Aetherform Reverb");
}

#[test]
fn fx_empty_bus_loads_an_effect_from_its_key() {
    use mixjam_techno::application::fx::Module;
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    click_named(&ctx, &mut app, "FX 2 Echoform Delay");
    let bus = app.session.project().fx.buses[1];
    assert!(matches!(bus.module, Module::EchoformDelay(_)));
    let (_, mix) = bus.module.presets()[0];
    assert_eq!(bus.return_level, mix / 100.0);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(!accessible_node(&output, "Deep edit FX 2").1.is_disabled());
}

#[test]
fn fx_deep_edit_shows_every_effect_whole_and_does_not_resize() {
    use mixjam_techno::application::fx::Module;
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.boards.module = BottomModule::FxReturns;
    let none = egui::Modifiers::NONE;
    for module in [
        Module::Empty,
        Module::EchoformDelay(Default::default()),
        Module::AetherformReverb(Default::default()),
        Module::EmberformDrive(Default::default()),
    ] {
        let mut fx = app.session.project().fx;
        fx.buses[0].module = module;
        app.session.edit(EditCommand::SetEffects(fx)).unwrap();
        app.fx_deep_edit = Some(0);
        frame(&ctx, &mut app, vec![], none);
        let output = frame(&ctx, &mut app, vec![], none);
        assert_fixed_window_holds_controls(&ctx, &output, fx::DEEP_EDIT_TITLE, "Deep edit");
    }
    let window = dialog::window_rect(&ctx, fx::DEEP_EDIT_TITLE).unwrap();
    let corner = window.right_bottom() - Vec2::splat(2.0);
    drag(
        &ctx,
        &mut app,
        corner,
        corner + Vec2::new(120.0, 120.0),
        none,
    );
    frame(&ctx, &mut app, vec![], none);
    let after = dialog::window_rect(&ctx, fx::DEEP_EDIT_TITLE).unwrap();
    assert_eq!(after.size(), window.size());
}

/// Put an Aetherform Reverb on each of `buses`.
fn load_reverbs(app: &mut MixJamApp, buses: &[usize]) {
    use mixjam_techno::application::fx::Module;
    let mut fx = app.session.project().fx;
    for &bus in buses {
        fx.buses[bus].module = Module::AetherformReverb(Default::default());
    }
    app.session.edit(EditCommand::SetEffects(fx)).unwrap();
}

#[test]
fn fx_rack_shows_all_four_cards_at_the_smallest_window_size() {
    use mixjam_techno::application::fx::{Division, Echoform, Module};
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    let mut buses = app.session.project().fx;
    buses.buses[0].module = Module::EchoformDelay(Echoform {
        division_l: Division::SixteenthTriplet,
        ..Echoform::default()
    });
    app.session.edit(EditCommand::SetEffects(buses)).unwrap();
    let size = Vec2::new(960.0, 720.0);
    let mut output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    for _ in 0..3 {
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let edits: Vec<_> = screen(&output)
        .texts()
        .filter(|text| text.label() == "Edit" && text.rect().center().x > 100.0)
        .collect();
    assert_eq!(edits.len(), 4);
    assert!(
        edits
            .into_iter()
            .all(|text| text.whole() && !text.shape().galley.elided)
    );
    for label in ["1/16T", "Custom", "Delay"] {
        assert!(
            output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text)
                if text.galley.job.text == label && !text.galley.elided)
            }),
            "{label} must fit in its selector"
        );
    }
    assert_eq!(
        accessible_node(&output, "FX 1 effect").1.value(),
        Some("Echoform Delay")
    );
}

/// The meter's D lane shows only while the bus's ducker can act.
#[test]
fn fx_meter_shows_the_ducker_lane_only_while_the_bus_ducks() {
    use mixjam_techno::application::fx::{Echoform, Module};
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    let mut buses = app.session.project().fx;
    for (bus, duck_amount) in [(0, 0.0), (1, 30.0)] {
        buses.buses[bus].module = Module::EchoformDelay(Echoform {
            duck_amount,
            ..Echoform::default()
        });
    }
    app.session.edit(EditCommand::SetEffects(buses)).unwrap();
    let size = Vec2::new(1440.0, 1000.0);
    frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    let s = screen(&output);
    let meter = |bus: usize| s.within(s.node(&format!("FX {bus} return meters")).rect());
    assert!(!meter(1).shows("D"), "a bus without ducking hides D");
    assert!(meter(2).shows("D"), "a ducking bus shows D");
}

/// In Sync mode the left time's dial steps through the note lengths, and its
/// value row shows the one it chose.
#[test]
fn fx_synced_time_dial_steps_the_note_length() {
    use mixjam_techno::application::fx::{Division, Echoform, Module, TimeMode};
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    let mut buses = app.session.project().fx;
    buses.buses[0].module = Module::EchoformDelay(Echoform {
        mode: TimeMode::Sync,
        division_l: Division::ALL[0],
        ..Echoform::default()
    });
    app.session.edit(EditCommand::SetEffects(buses)).unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "FX 1 left division dial");
    press(&ctx, &mut app, egui::Key::ArrowUp, egui::Modifiers::NONE);
    let Module::EchoformDelay(delay) = app.session.project().fx.buses[0].module else {
        panic!("FX 1 keeps its delay");
    };
    assert_eq!(delay.division_l, Division::ALL[1]);
}

/// Each card at the smallest and a large window: the header labels clear
/// every switch, the controls stay inside the card, the preset reads whole,
/// the value windows keep apart in one row, and the meter names its lanes
/// right under the dial labels.
#[test]
fn fx_cards_lay_out_without_collisions_or_dead_space() {
    use mixjam_techno::application::fx::{Aetherform, Echoform, Emberform, Module};
    let sizes = [Vec2::new(960.0, 720.0), Vec2::new(1440.0, 1000.0)];
    for (theme, size) in themes_in_turn().flat_map(|theme| sizes.map(|size| (theme, size))) {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.boards.module = BottomModule::FxReturns;
        let mut buses = app.session.project().fx;
        buses.buses[0].module = Module::EchoformDelay(Echoform::default());
        buses.buses[1].module = Module::AetherformReverb(Aetherform::default());
        buses.buses[2].module = Module::EmberformDrive(Emberform::default());
        buses.buses[3].module = Module::AetherformReverb(Aetherform::default());
        for (bus, preset) in [
            "Wide Tape Echo",
            "Warm Chamber",
            "Warm Tube",
            "Endless Cathedral",
        ]
        .into_iter()
        .enumerate()
        {
            assert!(buses.buses[bus].load_preset(preset));
        }
        app.session.edit(EditCommand::SetEffects(buses)).unwrap();
        // The lower panel opens at its default height, as the app starts.
        ctx.data_mut(|data| {
            data.remove::<egui::containers::panel::PanelState>(egui::Id::new("sounds"))
        });
        let mut output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        for _ in 0..3 {
            output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let s = screen(&output);
        for bus in 1..=4 {
            let rect = |part: &str| s.node(&format!("FX {bus} {part}")).rect();
            let (power, solo, limiter) = (rect("power"), rect("solo"), rect("limiter"));
            let edit = s.node(&format!("Deep edit FX {bus}")).rect();
            let meter = rect("return meters");
            // The bus name opens the header row, left of its In switch.
            let name = s
                .texts()
                .filter(|text| text.label() == format!("FX {bus}"))
                .map(|text| text.rect())
                .find(|name| {
                    solo.y_range().contains(name.center().y) && name.right() < power.left()
                })
                .unwrap_or_else(|| panic!("FX {bus} names its card"));
            let card = Rect::from_x_y_ranges(
                name.left() - 8.0..=edit.right() + 8.0,
                edit.top() - 6.0..=meter.bottom(),
            );
            let at = format!("FX {bus} in {theme:?} at {size:?}, card {card:?}");
            let header = Rect::from_x_y_ranges(card.x_range(), solo.y_range());
            let controls = [power, solo, limiter, edit];
            for text in s.within(header).texts() {
                assert!(
                    !text.shape().galley.elided,
                    "{at}: {:?} is cut off",
                    text.label()
                );
                for control in controls {
                    assert!(
                        control.contains_rect(text.rect())
                            || !text.rect().shrink(0.5).intersects(control),
                        "{at}: {:?} {:?} touches {control:?}",
                        text.label(),
                        text.rect()
                    );
                }
            }
            assert!(s.within(header).shows("Lim"), "{at}: Lim names the limiter");
            for control in [rect("effect"), rect("preset"), meter] {
                assert!(
                    control.left() >= card.left() + 7.5 && control.right() <= card.right() - 7.5,
                    "{at}: {control:?} leaves the card's padding"
                );
            }
            let values: Vec<Rect> = s
                .within(card)
                .nodes()
                .filter(|node| {
                    node.node()
                        .label()
                        .is_some_and(|label| label.starts_with("FX ") && label.ends_with(" value"))
                })
                .map(|node| node.rect())
                .collect();
            let mut row = values.clone();
            if let Some(division) = s.find_node(&format!("FX {bus} left division")) {
                row.push(division.rect());
            }
            row.sort_by(|a, b| a.left().total_cmp(&b.left()));
            assert_eq!(row.len(), 4, "{at}: four controls in the value row");
            for pair in row.windows(2) {
                assert!(
                    pair[1].left() - pair[0].right() >= 4.0,
                    "{at}: value windows {:?} and {:?} crowd",
                    pair[0],
                    pair[1]
                );
                assert!(
                    (pair[1].center().y - pair[0].center().y).abs() < 1.0,
                    "{at}: {:?} and {:?} share one row",
                    pair[0],
                    pair[1]
                );
            }
            // A value keeps clear of its window's frame.
            for window in &row {
                for text in s.within(*window).texts() {
                    assert!(
                        window
                            .shrink2(Vec2::new(2.0, 0.0))
                            .x_range()
                            .contains(text.rect().left())
                            && text.rect().right() <= window.right() - 2.0,
                        "{at}: {:?} {:?} overflows {window:?}",
                        text.label(),
                        text.rect()
                    );
                }
            }
            for lane in ["L", "R"] {
                let letter = s.within(meter).text(lane);
                let size = letter.shape().galley.job.sections[0].format.font_id.size;
                assert!(size >= 8.5, "{at}: lane {lane} is lettered at {size} pt");
            }
            let labels = s.within(card).text("Return").rect();
            assert!(
                meter.top() - labels.bottom() <= 16.0,
                "{at}: {:.0} points of dead space over the meter",
                meter.top() - labels.bottom()
            );
        }
        assert!(
            s.find_node("Effects vertical scroll bar").is_none(),
            "the cards fit the board's height in {theme:?} at {size:?}"
        );
        for preset in ["Wide Tape Echo", "Warm Chamber", "Warm Tube"]
            .into_iter()
            .chain((size.x > 1000.0).then_some("Endless Cathedral"))
        {
            let text = s.text(preset);
            assert!(
                !text.shape().galley.elided && text.whole(),
                "{preset} reads whole at {size:?}"
            );
        }
    }
}

/// Each drag runs again with the primary button, which must move the
/// control: that shows the drag is on the control.
#[test]
fn a_secondary_drag_moves_no_fader_and_no_send() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let rail = effects_control_rect(&output, "Track 1 gain").center();
    let send = effects_control_rect(&output, "Track 1 send FX 1").center();
    let lane = effects_control_rect(&output, "Arrangement track 1 gain").center();
    let drags = [
        ("the Mixer fader", rail, rail + Vec2::new(0.0, 20.0)),
        ("the send knob", send, send + Vec2::new(30.0, 0.0)),
        ("the lane fader", lane, lane - Vec2::new(10.0, 0.0)),
    ];
    let mix = |app: &MixJamApp| {
        let track = &app.session.project().tracks[0];
        (track.gain, track.sends[0])
    };
    let mut moved = Vec::new();
    for add_primary in [false, true] {
        for (name, from, to) in drags {
            let before = mix(&app);
            if add_primary {
                button_at(&ctx, &mut app, from, egui::PointerButton::Secondary, true);
                frame(
                    &ctx,
                    &mut app,
                    vec![egui::Event::PointerMoved(to)],
                    egui::Modifiers::NONE,
                );
                pointer(&ctx, &mut app, to, true, egui::Modifiers::NONE);
                pointer(&ctx, &mut app, to, false, egui::Modifiers::NONE);
                button_at(&ctx, &mut app, to, egui::PointerButton::Secondary, false);
            } else {
                secondary_drag(&ctx, &mut app, from, to);
            }
            if mix(&app) != before {
                moved.push((add_primary, name));
            }
        }
    }
    assert!(moved.is_empty(), "A secondary drag moved {moved:?}.");
    for (name, from, to) in drags {
        let before = mix(&app);
        drag(&ctx, &mut app, from, to, egui::Modifiers::NONE);
        assert_ne!(mix(&app), before, "The primary button must move {name}.");
    }
}

#[test]
fn fx_deep_edit_drive_workspace_edits_parameters_and_renders_across_themes() {
    use mixjam_techno::application::fx::{DistortionShape, Module};
    each_theme(|theme| {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        app.boards.module = BottomModule::FxReturns;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);

        choose(&ctx, &mut app, "FX 1 effect", "Emberform Drive");
        choose(&ctx, &mut app, "FX 1 preset", "Drum Crunch");
        let Module::EmberformDrive(drive) = app.session.project().fx.buses[0].module else {
            panic!("{theme:?}: FX 1 must hold Emberform Drive");
        };
        assert_eq!(drive.shape, DistortionShape::Overdrive);
        assert_eq!(drive.drive_db, 24.0);

        click_named(&ctx, &mut app, "Clear all FX tails");
        assert!(matches!(
            app.session.project().fx.buses[0].module,
            Module::EmberformDrive(_)
        ));

        click_named(&ctx, &mut app, "Deep edit FX 1");
        choose(&ctx, &mut app, "Deep edit drive type", "Bitcrush");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            !accessible_node(&output, "Deep edit bits").1.is_disabled(),
            "{theme:?}: bits must be enabled under Bitcrush"
        );
        assert!(
            !accessible_node(&output, "Deep edit crush rate")
                .1
                .is_disabled(),
            "{theme:?}: crush rate must be enabled under Bitcrush"
        );

        for (control, value) in [("Deep edit bits", 6.0), ("Deep edit drive", 20.0)] {
            let (target, _) = accessible_node(&output, control);
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::AccessKitActionRequest(
                    egui::accesskit::ActionRequest {
                        action: egui::accesskit::Action::SetValue,
                        target,
                        data: Some(egui::accesskit::ActionData::NumericValue(value)),
                    },
                )],
                egui::Modifiers::NONE,
            );
        }
        let Module::EmberformDrive(drive) = app.session.project().fx.buses[0].module else {
            panic!("{theme:?}: FX 1 must still hold Emberform Drive");
        };
        assert_eq!(drive.bits, 6);
        assert_eq!(drive.drive_db, 20.0);

        choose(&ctx, &mut app, "Deep edit drive type", "Tube");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            accessible_node(&output, "Deep edit bits").1.is_disabled(),
            "{theme:?}: bits must disable outside Bitcrush"
        );
        assert!(
            accessible_node(&output, "Deep edit crush rate")
                .1
                .is_disabled(),
            "{theme:?}: crush rate must disable outside Bitcrush"
        );

        click_named(&ctx, &mut app, "Deep edit level match");
        let Module::EmberformDrive(drive) = app.session.project().fx.buses[0].module else {
            panic!("{theme:?}: FX 1 must hold Emberform Drive");
        };
        assert!(!drive.level_match);
        // Clicking the right-hand text label on an FX switch must also toggle it.
        let toggle_rect = effects_control_rect(
            &frame(&ctx, &mut app, vec![], egui::Modifiers::NONE),
            "Deep edit level match",
        );
        assert!(toggle_rect.width() >= 80.0);
        let label_pt = Pos2::new(toggle_rect.right() - 8.0, toggle_rect.center().y);
        click(&ctx, &mut app, label_pt, egui::Modifiers::NONE);
        let Module::EmberformDrive(drive) = app.session.project().fx.buses[0].module else {
            panic!("{theme:?}: FX 1 must still hold Emberform Drive");
        };
        assert!(
            drive.level_match,
            "{theme:?}: clicking switch label text must toggle it"
        );
        click(&ctx, &mut app, label_pt, egui::Modifiers::NONE);

        frame(
            &ctx,
            &mut app,
            vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)],
            egui::Modifiers::COMMAND,
        );
        let Module::EmberformDrive(drive) = app.session.project().fx.buses[0].module else {
            panic!("{theme:?}: FX 1 must still hold Emberform Drive");
        };
        assert!(
            drive.level_match,
            "{theme:?}: Ctrl+Z must restore level match"
        );
    });
}

#[test]
fn fx_delay_division_matches_value_font_and_empty_slot_tiles_lay_out() {
    use mixjam_techno::application::fx::{Division, Echoform, Module, TimeMode};
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::FxReturns;
    let mut buses = app.session.project().fx;
    buses.buses[0].module = Module::EchoformDelay(Echoform {
        mode: TimeMode::Sync,
        link_times: false,
        division_l: Division::Quarter,
        division_r: Division::SixteenthTriplet,
        ..Echoform::default()
    });
    buses.buses[1].module = Module::Empty;
    app.session.edit(EditCommand::SetEffects(buses)).unwrap();

    let large = Vec2::new(1440.0, 1000.0);
    frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, large);
    let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, large);
    let s = screen(&output);
    let effect_rect = s.node("FX 2 effect").rect();
    let tiles = [
        s.node("FX 2 Echoform Delay").rect(),
        s.node("FX 2 Aetherform Reverb").rect(),
        s.node("FX 2 Emberform Drive").rect(),
    ];
    for tile in tiles {
        assert!(
            tile.top() > effect_rect.bottom(),
            "Empty tile {tile:?} must sit below selector {effect_rect:?}"
        );
    }
    for pair in tiles.windows(2) {
        assert!(
            (pair[0].center().y - pair[1].center().y).abs() < 1.0,
            "Wide empty tiles must align in one row: {:?} vs {:?}",
            pair[0],
            pair[1]
        );
        assert!(
            pair[1].left() > pair[0].right(),
            "Wide empty tiles must not overlap: {:?} vs {:?}",
            pair[0],
            pair[1]
        );
    }

    click_named(&ctx, &mut app, "Deep edit FX 1");
    let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, large);
    for label in ["1/4", "1/16T"] {
        let shape = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => Some(text),
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing division label {label}"));
        assert!(!shape.galley.elided, "{label} must not elide");
        let font_size = shape.galley.job.sections[0].format.font_id.size;
        assert!(
            font_size >= 12.5,
            "{label} should match value font size (>= 12.5), got {font_size}"
        );
    }
}
