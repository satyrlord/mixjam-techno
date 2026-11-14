//! Clips: the clip menu, moving and copying, reversal and waveform drawing.

use super::*;

#[test]
fn clip_menu_previews_the_clip_it_was_opened_on() {
    let (ctx, mut app) = missing_fixture();
    let mut project = app.session.project().clone();
    let other = MissingSample::native(
        Product::T2,
        43,
        "Other pad".into(),
        132.0,
        4 * TICKS_PER_BAR,
        false,
        "The original audio file is absent.".into(),
    )
    .unwrap();
    project.clips.push(Clip {
        id: 2,
        sample_id: other.id,
        track: 3,
        start_tick: TICKS_PER_BAR,
        length_ticks: 4 * TICKS_PER_BAR,
        offset_frames: 0,
        gain: 1.0,
        fades: Default::default(),
        pitch_cents: 0,
        reverse: false,
    });
    project.missing_samples.push(other);
    app.session = Session::headless(Arc::new(Library::fixture(vec![])), project).unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    // Both clips are selected; the first one is the primary clip.
    app.selection.clips = [1, 2].into_iter().collect();
    app.selection.clip = Some(1);
    let clip = app.session.project().clips[1].clone();
    let pos = app.clip_rect(&clip, app.score.rect.unwrap()).center();
    for pressed in [true, false] {
        frame(
            &ctx,
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
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.selection.clips.len(),
        2,
        "A right-click inside a selection keeps it."
    );
    let preview = text_rect(&output, "Preview").center();
    click(&ctx, &mut app, preview, egui::Modifiers::NONE);
    assert!(
        app.status.contains("Other pad"),
        "Preview must act on the clip under the menu: {}",
        app.status
    );
}

#[test]
fn focused_lanes_choose_move_duplicate_and_delete_clips_by_key() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    focus_control(&ctx, &mut app, &output, "Arrangement lanes");
    // egui holds arrow keys for a control from its second focused frame.
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let key = |app: &mut MixJamApp, key, modifiers| {
        frame(&ctx, app, vec![key_event(key, modifiers)], modifiers);
        frame(&ctx, app, vec![], egui::Modifiers::NONE);
    };
    key(&mut app, egui::Key::ArrowRight, egui::Modifiers::NONE);
    let first = app
        .session
        .project()
        .clips
        .iter()
        .min_by_key(|clip| (clip.track, clip.start_tick))
        .unwrap()
        .clone();
    assert_eq!(
        app.selection.clip,
        Some(first.id),
        "An arrow key chooses the first clip."
    );
    assert!(!app.selection.place_on_click);
    let count = app.session.project().clips.len();
    key(&mut app, egui::Key::D, egui::Modifiers::COMMAND);
    assert_eq!(
        app.session.project().clips.len(),
        count + 1,
        "Ctrl+D duplicates the selection."
    );
    let copy = app
        .session
        .project()
        .clips
        .iter()
        .find(|clip| Some(clip.id) == app.selection.clip)
        .unwrap()
        .clone();
    assert_eq!(copy.track, first.track);
    assert!(copy.start_tick >= first.start_tick + first.length_ticks);
    assert_eq!(
        copy.start_tick % TICKS_PER_BAR,
        first.start_tick % TICKS_PER_BAR
    );
    key(&mut app, egui::Key::ArrowRight, egui::Modifiers::ALT);
    let moved = app
        .session
        .project()
        .clips
        .iter()
        .find(|clip| Some(clip.id) == app.selection.clip)
        .unwrap()
        .clone();
    assert_eq!(
        moved.start_tick,
        copy.start_tick + TICKS_PER_BAR,
        "Alt+Right moves the clip one bar."
    );
    key(&mut app, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(
        app.session.project().clips.len(),
        count,
        "Delete removes the selected clip."
    );
    key(&mut app, egui::Key::A, egui::Modifiers::COMMAND);
    assert_eq!(
        app.selection.clips.len(),
        count,
        "Ctrl+A selects every clip."
    );
}

#[test]
fn clip_menu_shortening_preserves_fade_in_before_fade_out() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let clip = app.session.project().clips[0].clone();
    let mut fades = clip.fades;
    fades.fade_in.ticks = 128;
    fades.fade_out.ticks = 128;
    app.session
        .edit(EditCommand::SetClip {
            id: clip.id,
            start_tick: clip.start_tick,
            gain: clip.gain,
            offset_frames: clip.offset_frames,
            length_ticks: clip.length_ticks,
            fades,
            pitch_cents: clip.pitch_cents,
            reverse: false,
        })
        .unwrap();
    open_clip_menu(&ctx, &mut app, clip.id);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (target, _) = accessible_node(&output, "Clip length (bars)");
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::SetValue,
                target,
                data: Some(egui::accesskit::ActionData::NumericValue(0.5)),
            },
        )],
        egui::Modifiers::NONE,
    );
    let clip = &app.session.project().clips[0];
    assert_eq!(clip.length_ticks, 128);
    assert_eq!(
        (clip.fades.fade_in.ticks, clip.fades.fade_out.ticks),
        (128, 0)
    );
}

#[test]
fn clip_menu_controls_fit_the_minimum_window() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let size = Vec2::new(960.0, 720.0);
    for _ in 0..3 {
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    // The menu opens at the pointer; the lowest clip leaves it the least room.
    let clip = app.session.project().clips[1].clone();
    let pos = app.clip_rect(&clip, app.score.rect.unwrap()).right_bottom() - Vec2::splat(4.0);
    for pressed in [true, false] {
        frame_at_size(
            &ctx,
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
            size,
        );
    }
    let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    for name in [
        "Clip gain",
        "Clip trim start (frames)",
        "Clip length (bars)",
        "Clip fade in (beats)",
        "Clip fade in curve",
        "Clip fade out (beats)",
        "Clip fade out curve",
        "Clip pitch (semitones)",
        "Clip pitch (cents)",
        "Clip reverse",
    ] {
        let bounds = accessible_node(&output, name).1.bounds().unwrap();
        assert!(bounds.x1 <= f64::from(size.x), "{name}: {bounds:?}");
        assert!(bounds.y1 <= f64::from(size.y), "{name}: {bounds:?}");
    }
}

#[test]
fn shift_f10_opens_the_selected_clips_menu() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    // After a click on a clip nothing has focus, as for Delete.
    app.select_clip(2, false);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let shift_f10 = |ctx: &egui::Context, app: &mut MixJamApp| {
        frame(
            ctx,
            app,
            vec![key_event(egui::Key::F10, egui::Modifiers::SHIFT)],
            egui::Modifiers::SHIFT,
        );
        frame(ctx, app, vec![], egui::Modifiers::NONE)
    };
    let menu = shift_f10(&ctx, &mut app);
    assert_eq!(app.clip_menu.clip, Some(2));
    accessible_node(&menu, "Clip gain");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    // The focused lanes take the key themselves.
    app.clip_menu.clip = None;
    focus_control(&ctx, &mut app, &output, "Arrangement lanes");
    let menu = shift_f10(&ctx, &mut app);
    assert_eq!(app.clip_menu.clip, Some(2));
    accessible_node(&menu, "Clip gain");
    accessible_node(&menu, "Clip pitch (cents)");
}

#[test]
fn a_step_synth_sound_saved_with_its_pattern_draws_notes_instead_of_a_waveform() {
    let dir = std::env::temp_dir().join(format!(
        "mixjam-step-notes-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("Lead.wav");
    let sample = |id, wav_path| Sample {
        id,
        code: String::new(),
        name: "Lead".into(),
        bars: 1,
        group_name: "Step synth".into(),
        offset: 0,
        length: 0,
        frame_count: 75_600,
        sample_rate: 44_100,
        channels: 1,
        source_bpm: 140.0,
        wav_path,
        digest: None,
    };
    let library = Arc::new(Library::fixture(vec![
        sample(1, Some(wav.clone())),
        sample(2, None),
    ]));
    let mut editor = Editor::new(Project::default());
    editor.place(1, 0, 0, TICKS_PER_BAR).unwrap();
    editor.place(2, 1, 0, TICKS_PER_BAR).unwrap();
    let mut app = MixJamApp::from_session(
        Session::headless(library, editor.into_project()).unwrap(),
        "UI fixture".into(),
        false,
    );
    app.capture.path = None;
    let ctx = egui::Context::default();
    configure_style(&ctx);
    let pattern = mixjam_techno::application::pattern_beside(&wav);
    assert_eq!(pattern, dir.join("Lead.pattern.json"));
    let mut state = StepSynthState {
        bpm: 120.0,
        ..StepSynthState::default()
    };
    state.steps.fill(None);
    state.steps[4] = Some(7);
    app.session.save_pattern(&pattern, &state).unwrap();
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    assert!(
        app.clip_paint
            .step_patterns
            .get(&1)
            .is_some_and(Option::is_some),
        "The sound with a pattern beside it draws notes."
    );
    assert_eq!(
        app.clip_paint.step_patterns.get(&2).map(Option::is_some),
        Some(false),
        "A sound without one keeps its waveform."
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let clip = app.clip_rect(&app.session.project().clips[0], app.score.rect.unwrap());
    let category = group_color("Step synth");
    let tones = [0.55, 0.82].map(|keep| {
        Color32::from_rgb(
            (f32::from(category.r()) * keep).round() as u8,
            (f32::from(category.g()) * keep).round() as u8,
            (f32::from(category.b()) * keep).round() as u8,
        )
    });
    let note = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if tones.contains(&rect.fill) && clip.contains(rect.rect.center()) =>
            {
                Some(rect.rect)
            }
            _ => None,
        })
        .expect("The note must be painted inside the clip.");
    // The WAV was rendered at 120 BPM, even if its catalog tempo is 140.
    let expected_tick = (44_100.0 * 60.0 / 120.0) / (44_100.0 * 240.0 / 140.0 / 256.0);
    let expected_left = clip.left() + expected_tick * app.score.bar_width / 256.0;
    assert!(
        (note.left() - expected_left).abs() < 0.01,
        "{note:?} vs {expected_left}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_clip_menu_reverses_a_clip_and_marks_it() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    open_clip_menu(&ctx, &mut app, 1);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (target, node) = accessible_node(&output, "Clip reverse");
    assert_eq!(node.toggled(), Some(egui::accesskit::Toggled::False));
    frame(
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
    assert!(app.session.project().clips[0].reverse);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let (_, node) = accessible_node(&output, "Clip reverse");
    assert_eq!(node.toggled(), Some(egui::accesskit::Toggled::True));
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "rev");
    // The switch is one undo step of its own.
    app.history_step(false);
    assert!(!app.session.project().clips[0].reverse);
    app.history_step(true);
    assert!(app.session.project().clips[0].reverse);
}

/// Give the fixture's sound peaks: a 220 Hz tone that decays each beat.
fn give_fixture_peaks(app: &MixJamApp) {
    use mixjam_techno::application::Peaks;
    let pcm = Pcm {
        sample_rate: 44100,
        channels: 1,
        samples: (0..75600)
            .map(|frame| {
                let t = frame as f32 / 44100.0;
                (t * 220.0 * std::f32::consts::TAU).sin() * (-(t % 0.43) * 6.0).exp()
            })
            .collect(),
    };
    app.session
        .set_sample_peaks(1, Arc::new(Peaks::from_pcm(&pcm)));
}

/// The waveform meshes painted inside `clip`: many vertices of one color,
/// unlike a clip's fill, its gloss, or its marks.
fn wave_meshes(output: &egui::FullOutput, clip: Rect) -> Vec<Rect> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh)
                if mesh.vertices.len() >= 40
                    && mesh
                        .vertices
                        .iter()
                        .all(|vertex| vertex.color == mesh.vertices[0].color) =>
            {
                let bounds = mesh.calc_bounds();
                clip.expand(0.5).contains_rect(bounds).then_some(bounds)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn clips_paint_their_waveform_below_the_name_at_every_lane_height() {
    use mixjam_techno::application::options::LaneHeight;
    each_theme(|theme| {
        let (ctx, mut app) = fixture();
        let root = std::env::temp_dir().join(format!(
            "mixjam-clip-wave-ui-{}-{}",
            std::process::id(),
            theme.label()
        ));
        let project = app.session.project().clone();
        app.session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            project,
        )
        .unwrap()
        .with_settings_root(root.clone());
        give_fixture_peaks(&app);
        for lane_height in LaneHeight::ALL {
            let mut options = app.session.options().clone();
            options.theme = theme;
            options.lane_height = lane_height;
            app.session
                .apply_options(options, app.session.options_revision(), false)
                .unwrap();
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let clip = app.clip_rect(&app.session.project().clips[0], app.score.rect.unwrap());
            let waves = wave_meshes(&output, clip);
            assert_eq!(waves.len(), 1, "{theme:?} {lane_height:?}");
            let wave = waves[0];
            // One bar of sound fills the one-bar clip.
            assert!(wave.width() > clip.width() - 6.0, "{wave:?} in {clip:?}");
            let name = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if text.galley.job.text == "good grasp 2" && clip.contains(text.pos) =>
                    {
                        Some(text.galley.rect.translate(text.pos.to_vec2()))
                    }
                    _ => None,
                })
                .unwrap();
            if lane_height != LaneHeight::Px20 {
                assert!(
                    wave.top() >= name.bottom() - 0.5,
                    "{theme:?} {lane_height:?}: waveform {wave:?} under name {name:?}"
                );
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    });
}

#[test]
fn clip_waveforms_are_cached_per_clip_and_zoom() {
    let (ctx, mut app) = fixture();
    give_fixture_peaks(&app);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.clip_paint.waves.len(), 2);
    let first = app.session.project().clips[0].id;
    let key = app.clip_paint.waves[&first].key;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        app.clip_paint.waves[&first].key == key,
        "A still frame reuses the waveform."
    );
    app.score.zoom = 1.5;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        app.clip_paint.waves[&first].key != key,
        "A new zoom reads it again."
    );
    let second = app.session.project().clips[1].id;
    app.session.delete(second).unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        app.clip_paint.waves.len(),
        1,
        "A deleted clip's waveform is dropped."
    );
}
