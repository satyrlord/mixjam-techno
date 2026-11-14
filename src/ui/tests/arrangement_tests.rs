//! Arrangement view: viewport, lane heights, scrolling, snapping, zoom and frame cost.

use super::*;

#[test]
fn mouse_wheel_over_the_zoom_slider_steps_the_zoom_within_its_range() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let bounds = accessible_node(&output, "Zoom").1.bounds().unwrap();
    let position = Pos2::new(
        (bounds.x0 + bounds.x1) as f32 / 2.0,
        (bounds.y0 + bounds.y1) as f32 / 2.0,
    );
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(position)],
        egui::Modifiers::NONE,
    );
    let wheel = |ctx: &egui::Context, app: &mut MixJamApp, y: f32| {
        frame(
            ctx,
            app,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, y),
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        );
    };
    let before = app.score.zoom;
    wheel(&ctx, &mut app, 20.0);
    assert!(
        (app.score.zoom - before * 1.1).abs() < 1e-4,
        "One wheel step up must zoom in by a tenth."
    );
    for _ in 0..5 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    assert!(
        (app.score.zoom - before * 1.1).abs() < 1e-4,
        "A wheel event acts once."
    );
    wheel(&ctx, &mut app, -20.0);
    assert!(
        (app.score.zoom - before).abs() < 1e-4,
        "One step down undoes it."
    );
    for _ in 0..40 {
        wheel(&ctx, &mut app, 20.0);
    }
    assert_eq!(
        app.score.zoom, 2.0,
        "The wheel stops at the slider's maximum."
    );
    for _ in 0..40 {
        wheel(&ctx, &mut app, -20.0);
    }
    assert_eq!(
        app.score.zoom, 0.55,
        "The wheel stops at the slider's minimum."
    );
}

#[test]
fn follow_keeps_playhead_visible_and_off_preserves_manual_viewport() {
    let (ctx, mut app) = fixture();
    for track in 2..TRACK_COUNT {
        app.session.place(1, track, 0).unwrap();
    }
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let visible = app.score.drop_rect.unwrap();
    frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(visible.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -1000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        egui::Modifiers::NONE,
    );
    // Let egui's wheel smoothing settle before checking the other axis.
    for _ in 0..60 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let vertical = app.score.rect.unwrap().top();
    let project_before = app.session.project().clone();
    let mut first_follow_left = None;
    for (tick, zoom) in [
        (40 * TICKS_PER_BAR, 1.0),
        (41 * TICKS_PER_BAR, 1.0),
        (41 * TICKS_PER_BAR, 2.0),
        (0, 2.0),
    ] {
        app.score.zoom = zoom;
        app.session.seek(tick);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let canvas = app.score.rect.unwrap();
        let ruler = app.score.ruler_rect.unwrap();
        let x = canvas.left() + tick as f32 / TICKS_PER_BAR as f32 * app.score.bar_width;
        assert!(
            x >= ruler.left() - 1.0 && x <= ruler.right(),
            "tick {tick}: {x}, {ruler:?}"
        );
        assert!(
            (canvas.top() - vertical).abs() < 1.0,
            "Follow must retain the vertical offset: {} -> {} at tick {tick}.",
            vertical,
            canvas.top()
        );
        let left = canvas.left();
        if tick == 40 * TICKS_PER_BAR {
            first_follow_left = Some(left);
        } else if tick == 41 * TICKS_PER_BAR && zoom == 1.0 {
            assert_eq!(
                Some(left),
                first_follow_left,
                "Follow must not scroll on every tick."
            );
        }
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.score.rect.unwrap().left(), left);
    }
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let toggle = text_rect(&output, "Follow").center();
    click(&ctx, &mut app, toggle, egui::Modifiers::NONE);
    assert!(!app.score.follow_playhead);
    let left = app.score.rect.unwrap().left();
    app.session.seek(50 * TICKS_PER_BAR);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.score.rect.unwrap().left(), left);
    click(&ctx, &mut app, toggle, egui::Modifiers::NONE);
    assert!(app.score.follow_playhead);
    assert!(app.score.rect.unwrap().left() < left);
    // Panning while paused does not fight the Follow setting.
    let visible = app.score.drop_rect.unwrap();
    frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(visible.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(300.0, 0.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        egui::Modifiers::NONE,
    );
    for _ in 0..60 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let panned = app.score.rect.unwrap().left();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.score.rect.unwrap().left(), panned);
    assert_eq!(app.session.project(), &project_before);
}

#[test]
fn lane_heights_keep_reversed_clip_names_clear_of_the_marker() {
    use mixjam_techno::application::options::LaneHeight;
    each_theme(|theme| {
        let (ctx, mut app) = fixture();
        let root = std::env::temp_dir().join(format!(
            "mixjam-lane-reverse-ui-{}-{}",
            std::process::id(),
            theme.label()
        ));
        let mut samples = app.session.library().samples.to_vec();
        samples[0].name = "A long sample name that needs an ellipsis".into();
        let mut project = app.session.project().clone();
        project.clips[0].reverse = true;
        app.session = Session::headless(Arc::new(Library::fixture(samples)), project)
            .unwrap()
            .with_settings_root(root.clone());
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
            let text = |label: &str| {
                output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text)
                            if text.galley.job.text == label && clip.contains(text.pos) =>
                        {
                            Some(
                                text.galley
                                    .mesh_bounds
                                    .translate(text.pos.to_vec2())
                                    .intersect(shape.clip_rect),
                            )
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("Missing clip text: {label}"))
            };
            let name = text("A long sample name that needs an ellipsis");
            let reverse = text("rev");
            assert!(name.is_positive() && reverse.is_positive());
            assert!(
                !name.intersects(reverse),
                "{theme:?} {lane_height:?}: name {name:?} overlaps rev {reverse:?}"
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    });
}

#[test]
fn lane_heights_keep_the_new_track_drop_hint_visible() {
    use mixjam_techno::application::options::LaneHeight;
    each_theme(|theme| {
        let (ctx, mut app) = fixture();
        let root = std::env::temp_dir().join(format!(
            "mixjam-lane-drop-ui-{}-{}",
            std::process::id(),
            theme.label()
        ));
        app.session = app.session.with_settings_root(root.clone());
        for lane_height in LaneHeight::ALL {
            let mut options = app.session.options().clone();
            options.theme = theme;
            options.lane_height = lane_height;
            app.session
                .apply_options(options, app.session.options_revision(), false)
                .unwrap();
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            let hint = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "Drop a sound here" => {
                        Some((
                            text.galley.mesh_bounds.translate(text.pos.to_vec2()),
                            shape.clip_rect,
                        ))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{theme:?} {lane_height:?}: missing drop hint"));
            assert!(
                hint.1.contains_rect(hint.0),
                "The hint must not be clipped."
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    });
}

#[test]
fn fine_snap_and_two_line_labels_preserve_unicode_boundaries() {
    let (_, app) = fixture();
    let rect = app.score.rect.unwrap();
    let x = rect.left() + app.score.bar_width * 1.59;
    assert_eq!(app.position_tick(x, rect, false), 256);
    assert_eq!(app.position_tick(x, rect, true), 400);
    let ctx = egui::Context::default();
    configure_style(&ctx);
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        let rows = |text: &str, width: f32, size: f32, height: f32| {
            let job = elided(text, size, roles().text, width, clip_label_rows(height));
            ctx.fonts_mut(|fonts| fonts.layout_job(job))
                .rows
                .iter()
                .map(|row| row.text())
                .collect::<Vec<_>>()
        };
        let short = rows("Closed hats", 50.0, 12.0, 34.0);
        assert_eq!(
            short.len(),
            1,
            "Short clip rows must keep the text on one visible line."
        );
        assert!(
            short[0].ends_with('…') && short[0].starts_with("Closed"),
            "A shortened name must show its ellipsis on the visible line: {short:?}"
        );
        assert_eq!(rows("Low orbit", 120.0, 12.0, 34.0), ["Low orbit"]);
        let tall = rows("good grasp 2", 54.0, 11.0, 44.0);
        assert_eq!(tall.len(), 2, "Tall clips use two lines: {tall:?}");
        assert!(tall[0].starts_with("good") && tall[1].starts_with("grasp"));
        assert_eq!(rows("électronique", 10.0, 11.0, 44.0).len(), 2);
        let long = rows("one two three four five six seven", 54.0, 11.0, 44.0);
        assert_eq!(long.len(), 2);
        assert!(long[1].ends_with('…'));
    });
}

#[test]
fn scrolling_reaches_track_64_and_keeps_labels_and_ruler_interactive() {
    let (ctx, mut app) = fixture();
    for track in 2..TRACK_COUNT {
        app.session.place(1, track, 0).unwrap();
    }
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.score.tracks.len(), 64);
    assert!(app.score.add_track.is_none());
    let visible = app.score.drop_rect.unwrap();
    frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(visible.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -2500.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        egui::Modifiers::NONE,
    );
    for _ in 0..5 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let canvas = app.score.rect.unwrap();
    let visible = app.score.drop_rect.unwrap();
    let row_y = canvas.top() + 63.5 * app.score.row_height;
    assert!(visible.contains(Pos2::new(visible.center().x, row_y)));
    click(
        &ctx,
        &mut app,
        Pos2::new(header_x(&ctx, visible, lanes::NUMBER_X), row_y),
        egui::Modifiers::NONE,
    );
    assert_eq!(
        app.recording.track, 63,
        "Track labels must share the lane scroll offset."
    );
    click(
        &ctx,
        &mut app,
        Pos2::new(header_x(&ctx, visible, lanes::SWITCH_X[2]), row_y),
        egui::Modifiers::NONE,
    );
    assert!(
        !app.session.project().tracks[63].left_enabled,
        "route click at {row_y}; current canvas {:?}, visible {:?}, row {}; disabled tracks {:?}",
        app.score.rect,
        app.score.drop_rect,
        app.score.row_height,
        app.session
            .project()
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| !track.left_enabled)
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
    );
    assert!(app.session.project().tracks[0].left_enabled);
    let ruler = app.score.ruler_rect.unwrap();
    let seek_x = ruler.left() + 2.5 * app.score.bar_width;
    click(
        &ctx,
        &mut app,
        Pos2::new(seek_x, ruler.center().y),
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.cursor_tick(), 2 * TICKS_PER_BAR);
    app.selection.sample = Some(1);
    click(
        &ctx,
        &mut app,
        Pos2::new(seek_x, row_y),
        egui::Modifiers::NONE,
    );
    assert!(
        app.session
            .project()
            .clips
            .iter()
            .any(|clip| clip.track == 63 && clip.start_tick == 2 * TICKS_PER_BAR)
    );
    let ruler_before = app.score.ruler_rect.unwrap();
    let visible = app.score.drop_rect.unwrap();
    frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(visible.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(-400.0, 0.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        egui::Modifiers::NONE,
    );
    for _ in 0..5 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let ruler = app.score.ruler_rect.unwrap();
    let canvas = app.score.rect.unwrap();
    assert!(canvas.left() < ruler.left() - 100.0);
    assert_eq!(
        ruler.left(),
        ruler_before.left(),
        "Ruler viewport stays fixed while bars scroll."
    );
    let pos = Pos2::new(ruler.left() + 30.0, ruler.center().y);
    click(&ctx, &mut app, pos, egui::Modifiers::NONE);
    let expected = app.position_tick(pos.x, app.score.rect.unwrap(), false);
    assert_eq!(app.session.cursor_tick(), expected);
}

#[test]
fn arrangement_rows_keep_their_height_and_more_room_shows_more_rows() {
    let (ctx, mut app) = fixture();
    for track in 2..20 {
        app.session.place(1, track, 0).unwrap();
    }
    let mut seen = Vec::new();
    for size in [Vec2::new(1280.0, 720.0), Vec2::new(1280.0, 1440.0)] {
        for _ in 0..3 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        seen.push((app.score.row_height, app.score.drop_rect.unwrap().height()));
    }
    assert_eq!(seen[0].0, seen[1].0, "Rows keep one height: {seen:?}");
    assert!(
        seen[1].1 > seen[0].1 + seen[0].0,
        "A taller window shows more rows: {seen:?}"
    );
}

/// A 64-track song of 256 bars whose eight sounds all have peaks.
fn long_song_fixture(
    lane_height: mixjam_techno::application::options::LaneHeight,
    root: &Path,
) -> MixJamApp {
    use mixjam_techno::application::Peaks;
    let samples: Vec<Sample> = (1..=8)
        .map(|id| Sample {
            id,
            code: format!("S{id}"),
            name: format!("long song sound {id}"),
            bars: 1 << (id % 3),
            group_name: ["Loop", "Drum", "Bass", "Sequence"][id as usize % 4].into(),
            offset: 0,
            length: 0,
            frame_count: 75600 << (id % 3),
            sample_rate: 44100,
            channels: 1,
            source_bpm: 140.0,
            wav_path: None,
            digest: None,
        })
        .collect();
    let mut editor = Editor::new(Project::default());
    for track in 0..TRACK_COUNT {
        let mut bar = 0;
        while bar < 256 {
            let id = (track as u32 + bar) % 8 + 1;
            let bars = 1 << (id % 3);
            editor
                .place(id, track, bar * TICKS_PER_BAR, bars * TICKS_PER_BAR)
                .unwrap();
            bar += bars;
        }
    }
    let mut project = editor.into_project();
    for (index, clip) in project.clips.iter_mut().enumerate() {
        clip.reverse = index % 5 == 0;
        if index % 3 == 0 {
            clip.fades.fade_in.ticks = TICKS_PER_BAR / 4;
            clip.fades.fade_out.ticks = TICKS_PER_BAR / 2;
        }
    }
    let library = Arc::new(Library::fixture(samples.clone()));
    let mut app = MixJamApp::from_session(
        Session::headless(library, project)
            .unwrap()
            .with_settings_root(root.to_path_buf()),
        "UI fixture".into(),
        false,
    );
    app.capture.path = None;
    app.session.set_theme_fixture(theme::current());
    let mut options = app.session.options().clone();
    options.lane_height = lane_height;
    app.session
        .apply_options(options, app.session.options_revision(), false)
        .unwrap();
    for sample in &samples {
        // A decaying sine, so every column has a different height.
        let frames = sample.frame_count as usize;
        let pcm = Pcm {
            sample_rate: 44100,
            channels: 1,
            samples: (0..frames)
                .map(|frame| {
                    let t = frame as f32 / 44100.0;
                    (t * 220.0 * std::f32::consts::TAU).sin() * (-(t % 0.43) * 6.0).exp()
                })
                .collect(),
        };
        app.session
            .set_sample_peaks(sample.id, Arc::new(Peaks::from_pcm(&pcm)));
    }
    app
}

/// The frame time of the 64-track view of a long song: building the frame
/// and tessellating it, as the window does. Run it in release:
/// `cargo test --release --locked arrangement_frame_time -- --ignored --nocapture`.
#[test]
#[ignore = "a timing measurement, not a check"]
fn arrangement_frame_time() {
    use mixjam_techno::application::options::LaneHeight;
    for (lane_height, maximised, zoom) in [
        (LaneHeight::Px30, false, 1.0),
        (LaneHeight::Px20, true, 1.0),
        (LaneHeight::Px20, true, 0.55),
        (LaneHeight::Px40, true, 2.0),
    ] {
        let context = egui::Context::default();
        configure_style(&context);
        let root = std::env::temp_dir().join(format!("mixjam-long-song-{}", std::process::id()));
        let mut app = long_song_fixture(lane_height, &root);
        app.score.zoom = zoom;
        if maximised {
            app.set_layout(|layout| layout.maximised = true);
        }
        let mut times = Vec::new();
        for index in 0..80 {
            let started = std::time::Instant::now();
            // A pointer that moves over the lanes, as while editing.
            let pos = Pos2::new(600.0 + (index % 40) as f32 * 7.0, 400.0);
            let output = frame(
                &context,
                &mut app,
                vec![egui::Event::PointerMoved(pos)],
                egui::Modifiers::NONE,
            );
            let primitives = context.tessellate(output.shapes, output.pixels_per_point);
            std::hint::black_box(primitives);
            if index >= 20 {
                times.push(started.elapsed().as_secs_f64() * 1000.0);
            }
        }
        times.sort_by(f64::total_cmp);
        eprintln!(
            "{lane_height:?} maximised={maximised} zoom={zoom}: median {:.2} ms, \
             p95 {:.2} ms, max {:.2} ms",
            times[times.len() / 2],
            times[times.len() * 95 / 100],
            times[times.len() - 1]
        );
        // The default lane height saves nothing, so the folder may not exist.
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn the_lane_header_names_each_lane_and_its_fader_sets_the_track_gain() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    // The lane is named after its first clip's sound family.
    let name = text_rect(&output, "good grasp");
    let visible = app.score.drop_rect.unwrap();
    assert!(
        name.right() <= visible.left(),
        "The name sits in the lane header."
    );
    for label in [
        "Record on track 1",
        "Arrangement track 1 mute",
        "Arrangement track 1 solo",
        "Arrangement track 1 left output",
        "Arrangement track 1 right output",
        "Arrangement track 1 gain",
    ] {
        accessible_node(&output, label);
    }
    focus_control(&ctx, &mut app, &output, "Arrangement track 1 gain");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::ArrowDown, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let gain = app.session.project().tracks[0].gain;
    assert!(
        (amplitude_to_db(gain) + 0.5).abs() < 0.01,
        "One key step is 0.5 dB: {gain}"
    );
    assert_eq!(app.session.project().tracks[1].gain, 1.0);
    // A click at the fader's left end silences the track.
    let canvas = app.score.rect.unwrap();
    let row_y = canvas.top() + 0.5 * app.score.row_height;
    click(
        &ctx,
        &mut app,
        Pos2::new(header_x(&ctx, visible, lanes::FADER_SPAN.0 + 2.0), row_y),
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().tracks[0].gain, 0.0);
}
