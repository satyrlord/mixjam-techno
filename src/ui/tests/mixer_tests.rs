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
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text)
                if text.galley.job.text == "Track 64" && shape.clip_rect.contains_rect(last))
        }));
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
                let (_, node) = accessible_node(output, &label);
                assert_eq!(
                    node.toggled(),
                    Some(if enabled {
                        egui::accesskit::Toggled::True
                    } else {
                        egui::accesskit::Toggled::False
                    }),
                    "{label}"
                );
                let bounds = node.bounds().unwrap();
                let rect = Rect::from_min_max(
                    Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
                    Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
                );
                let painted = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(painted)
                            if painted.galley.job.text == switch.letter()
                                && rect.contains(painted.pos) =>
                        {
                            Some(painted)
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("Missing painted state for {label}"));
                assert_eq!(
                    painted.galley.job.sections[0].format.color,
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
        let bounds = accessible_node(output, &format!("Record on track {track}"))
            .1
            .bounds()
            .unwrap();
        let rect = Rect::from_min_max(
            Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
            Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
        );
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.job.text == track.to_string() && rect.contains(text.pos) =>
                {
                    Some(text.galley.job.sections[0].format.color)
                }
                _ => None,
            })
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
    assert!(occupied_tracks(app.session.project()).contains(&40));
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
    assert!(!occupied_tracks(app.session.project()).contains(&40));
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
    text_rect(&output, "2 sounds");
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
    for density in themed_densities() {
        let (ctx, mut app) = fixture();
        ctx.set_pixels_per_point(density);
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
                "{track} gain controls must fit at {density}x."
            );
            assert!(
                gain.height() >= 86.0,
                "The minimum-window rail must remain usable at {density}x."
            );
            assert!(
                gain.height() - 32.0 >= 68.0,
                "The physical rail must retain at least 68 points of travel at {density}x."
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
                        // Aero's gradient mesh ends half a point inside its
                        // rim and carries a half-pixel feather.
                        egui::Shape::Mesh(mesh) if roles().material == theme::Material::Plastic => {
                            mesh.calc_bounds().expand(0.5 - 0.5 / density)
                        }
                        egui::Shape::Mesh(mesh) => mesh.calc_bounds(),
                        egui::Shape::Rect(rect) if rect.fill == roles().cap => rect.rect,
                        _ => return None,
                    };
                    ((bounds.width() - 24.0).abs() <= 1.0 / density
                        && (bounds.height() - height).abs() <= 1.0 / density
                        && (bounds.center().x - gain.center().x).abs() <= 1.0 / density
                        && gain.expand(1.0 / density).contains(bounds.center()))
                    .then_some(bounds)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "Missing {track} fader cap at {density}x in {}.",
                        theme::current().label()
                    )
                });
            assert!(
                cap.top() - value.bottom() >= 4.0 - 1.0 / density,
                "The fader cap must clear the dB readout even at +12 dB."
            );
            assert!(
                gain.expand(1.0 / density).contains_rect(cap),
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
                    "{track} lower controls must fit at {density}x."
                );
            }
            assert!(
                (eq.top() - mute.top()).abs() <= 1.0 / density
                    && eq.right() <= mute.left()
                    && eq.bottom() <= polarity.top()
                    && (eq.center().x - polarity.center().x).abs() <= 1.0 / density,
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
                    "{track} send FX {bus} must clear the fader cap at {density}x."
                );
                assert!(
                    !send.intersects(pan),
                    "{track} send FX {bus} must clear pan at {density}x."
                );
                let gain = effects_control_rect(&output, &format!("{track} gain"));
                assert!(
                    !send.intersects(gain),
                    "{track} send FX {bus} must clear the gain hit area at {density}x."
                );
            }
            assert!(
                pan.top() - cap.bottom() >= 4.0 - 1.0 / density,
                "The fader cap must clear pan even at silence."
            );
            assert!(
                center.top() >= pan.bottom() - 1.0 / density && center.top() <= pan.bottom() + 4.0,
                "Center must attach just below its pan rail without covering the slider."
            );
            assert!(
                (pan.center().x - center.center().x).abs() <= 1.0 / density,
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
            "Master bypass must fit under the master rail at {density}x."
        );
    }
}

#[test]
fn empty_mixer_keeps_guidance_master_and_action_errors_visible() {
    let (ctx, mut app) = fixture();
    app.new_mix();
    app.boards.module = BottomModule::Mixer;
    assert_eq!(DEFAULT_STATUS, "Ready");
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
        "FX 1 effect",
        "FX 1 duck key",
        "FX 2 preset",
        "FX 2 duck key",
        "FX 2 space",
        "FX 2 shimmer",
        "Clear FX 2 tail",
        "FX 2 bypass",
        "FX 4 limiter",
    ] {
        accessible_node(&output, name);
    }
    assert!(
        accessible_node(&output, "FX 2 shimmer amount")
            .1
            .is_disabled(),
        "the amount waits for Shimmer"
    );
    set(&ctx, &mut app, &output, "FX 2 decay", 5.0);
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
    set(&ctx, &mut app, &output, "FX 2 duck key", 4.0);
    let Module::AetherformReverb(reverb) = app.session.project().fx.buses[1].module else {
        panic!("FX 2 holds the reverb")
    };
    assert_eq!(reverb.duck_key, 4);
}
