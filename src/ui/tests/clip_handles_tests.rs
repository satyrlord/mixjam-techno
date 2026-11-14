//! Clip handles: fade, gain and edge handles, and automation points edited from the
//! arrangement.

use super::*;

#[test]
fn clip_fade_handles_and_placement_keep_imported_saved_span_timing() {
    let (ctx, mut app) = fixture();
    let mut sample = app.session.library().samples[0].clone();
    sample.id = 1_000_000;
    sample.source_bpm = 240.0;
    sample.frame_count = 44_100;
    sample.group_name = "MixJam".into();
    let mut project = app.session.project().clone();
    for clip in &mut project.clips {
        clip.sample_id = sample.id;
    }
    project
        .source_notices
        .push("MixJam v7 source controls: {}".into());
    project
        .imported_samples
        .push(crate::engine::ImportedSample {
            id: sample.id,
            name: sample.name.clone(),
            group_name: sample.group_name.clone(),
            path: PathBuf::from("saved-span.wav"),
            source_bpm: sample.source_bpm,
            source_sample_rate: Some(sample.sample_rate),
            origin: None,
        });
    app.session = Session::headless(Arc::new(Library::fixture(vec![sample])), project).unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.session.placement_ticks(1_000_000), Some(TICKS_PER_BAR));
    let lanes = app.score.rect.unwrap();
    let clip = &app.session.project().clips[0];
    let rect = app.clip_rect(clip, lanes);
    let handle = rect.right_top() + Vec2::new(-4.5, 5.5);
    assert_eq!(
        app.clip_handle_at(handle, lanes),
        Some((clip.id, ClipHandle::FadeOut))
    );
}

#[test]
fn clip_edges_fades_and_gain_line_reshape_the_clip_in_one_undo_step_each() {
    let (ctx, mut app) = fixture();
    let lanes = app.score.rect.unwrap();
    let bar = app.score.bar_width;
    let rect =
        |app: &MixJamApp, index: usize| app.clip_rect(&app.session.project().clips[index], lanes);
    let before = app.session.project().clone();

    // The right edge snaps to the bar it is dropped near.
    let first = rect(&app, 0);
    let edge = Pos2::new(first.right() - 2.0, first.center().y + 8.0);
    drag(
        &ctx,
        &mut app,
        edge,
        edge + Vec2::new(bar, 0.0),
        egui::Modifiers::NONE,
    );
    assert_eq!(
        app.session.project().clips[0].length_ticks,
        2 * TICKS_PER_BAR
    );
    assert!(app.gestures.handle_drag.is_none());

    // The fade in handle starts in the top left corner; fades snap to sixteenths.
    let first = rect(&app, 0);
    let corner = first.left_top() + Vec2::new(3.0, 4.0);
    drag(
        &ctx,
        &mut app,
        corner,
        Pos2::new(first.left() + bar * 0.52, corner.y),
        egui::Modifiers::NONE,
    );
    assert_eq!(app.session.project().clips[0].fades.fade_in.ticks, 128);

    // The left edge, on the fine grid with Ctrl, trims the start and
    // keeps the sound where it plays.
    let second = rect(&app, 1);
    let edge = Pos2::new(second.left() + 1.0, second.center().y + 8.0);
    drag(
        &ctx,
        &mut app,
        edge,
        edge + Vec2::new(bar / 2.0, 0.0),
        egui::Modifiers::CTRL,
    );
    let clip = &app.session.project().clips[1];
    assert_eq!(
        (clip.start_tick, clip.length_ticks, clip.offset_frames),
        (TICKS_PER_BAR * 3 / 2, TICKS_PER_BAR / 2, 37800)
    );

    // The handle on the gain line, in the middle of the clip, moves the
    // gain in whole dB steps: here three steps down from unity.
    let first = rect(&app, 0);
    let handle = gain_handle(first, app.score.drop_rect.unwrap(), 1.0);
    assert_eq!(handle.y, first.center().y, "Unity is at half height.");
    drag(
        &ctx,
        &mut app,
        handle,
        handle + Vec2::new(0.0, 3.0 * 4.0 + 1.0),
        egui::Modifiers::NONE,
    );
    let gain = app.session.project().clips[0].gain;
    assert!((amplitude_to_db(gain) + 3.0).abs() < 0.001, "{gain}");

    for _ in 0..4 {
        app.history_step(false);
    }
    assert_eq!(
        app.session.project(),
        &before,
        "Each drag is one undo step."
    );
}

#[test]
fn clip_copy_drags_at_an_edge_do_not_resize_the_source() {
    for (button, modifiers) in [
        (egui::PointerButton::Primary, egui::Modifiers::ALT),
        (egui::PointerButton::Secondary, egui::Modifiers::NONE),
    ] {
        let (ctx, mut app) = fixture();
        let source = app.session.project().clips[0].clone();
        let rect = app.clip_rect(&source, app.score.rect.unwrap());
        let from = Pos2::new(rect.right() - 2.0, rect.center().y + 8.0);
        let to = from + Vec2::new(2.0 * app.score.bar_width, 0.0);
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton {
                    pos: from,
                    button,
                    pressed: true,
                    modifiers,
                },
            ],
            modifiers,
        );
        for step in 1..=4 {
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(
                    from + (to - from) * step as f32 / 4.0,
                )],
                modifiers,
            );
        }
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerButton {
                pos: to,
                button,
                pressed: false,
                modifiers,
            }],
            modifiers,
        );
        assert_eq!(app.session.project().clips[0], source);
        assert_eq!(app.session.project().clips.len(), 3);
    }
}

#[test]
fn clip_handle_releases_are_separate_undo_steps() {
    let (ctx, mut app) = fixture();
    app.session.hold_gestures();
    let original = app.session.project().clips[0].clone();
    let bar = app.score.bar_width;
    for _ in 0..2 {
        let rect = app.clip_rect(&app.session.project().clips[0], app.score.rect.unwrap());
        let edge = Pos2::new(rect.right() - 2.0, rect.center().y + 8.0);
        drag(
            &ctx,
            &mut app,
            edge,
            edge + Vec2::new(bar, 0.0),
            egui::Modifiers::NONE,
        );
    }
    assert_eq!(
        app.session.project().clips[0].length_ticks,
        3 * TICKS_PER_BAR
    );
    app.history_step(false);
    assert_eq!(
        app.session.project().clips[0].length_ticks,
        2 * TICKS_PER_BAR
    );
    app.history_step(false);
    assert_eq!(app.session.project().clips[0], original);
}

#[test]
fn clip_left_edge_stops_before_its_source_runs_out() {
    let (ctx, mut app) = fixture();
    let clip = app.session.project().clips[0].clone();
    app.session
        .edit(EditCommand::SetClip {
            id: clip.id,
            start_tick: clip.start_tick,
            gain: clip.gain,
            offset_frames: clip.offset_frames,
            length_ticks: 4 * TICKS_PER_BAR,
            fades: clip.fades,
            pitch_cents: clip.pitch_cents,
            reverse: false,
        })
        .unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let rect = app.clip_rect(&app.session.project().clips[0], app.score.rect.unwrap());
    let edge = Pos2::new(rect.left() + 1.0, rect.center().y + 8.0);
    let bar = app.score.bar_width;
    drag(
        &ctx,
        &mut app,
        edge,
        edge + Vec2::new(2.0 * bar, 0.0),
        egui::Modifiers::CTRL,
    );
    let clip = &app.session.project().clips[0];
    assert!(
        clip.start_tick < TICKS_PER_BAR,
        "The source is only one bar long."
    );
    let sample = app.session.library().sample_by_id(clip.sample_id).unwrap();
    let expected = (f64::from(clip.start_tick) * f64::from(sample.sample_rate) * 240.0
        / sample.source_bpm
        / f64::from(TICKS_PER_BAR))
    .round() as u32;
    assert_eq!(
        clip.offset_frames, expected,
        "Trim and start must move together."
    );
}

#[test]
fn clip_gain_handle_preserves_gain_without_vertical_motion() {
    for gain in [0.0, 0.0001, 0.712345] {
        let (ctx, mut app) = fixture();
        let clip = app.session.project().clips[0].clone();
        app.session
            .edit(EditCommand::SetClip {
                id: clip.id,
                start_tick: clip.start_tick,
                gain,
                offset_frames: clip.offset_frames,
                length_ticks: clip.length_ticks,
                fades: clip.fades,
                pitch_cents: clip.pitch_cents,
                reverse: false,
            })
            .unwrap();
        let rect = app.clip_rect(&app.session.project().clips[0], app.score.rect.unwrap());
        let handle = gain_handle(rect, app.score.drop_rect.unwrap(), gain);
        drag(
            &ctx,
            &mut app,
            handle,
            handle + Vec2::new(10.0, 3.0),
            egui::Modifiers::NONE,
        );
        assert_eq!(app.session.project().clips[0].gain, gain);
    }
}

#[test]
fn clip_gain_handle_raises_whole_db_steps_and_ctrl_tenths() {
    for (modifiers, expected) in [
        (egui::Modifiers::NONE, 2.0_f32),
        (egui::Modifiers::CTRL, 0.2),
    ] {
        let (ctx, mut app) = fixture();
        let rect = app.clip_rect(&app.session.project().clips[0], app.score.rect.unwrap());
        let handle = gain_handle(rect, app.score.drop_rect.unwrap(), 1.0);
        assert_eq!(
            app.clip_handle_at(handle, app.score.rect.unwrap())
                .map(|h| h.1),
            Some(ClipHandle::Gain)
        );
        drag(
            &ctx,
            &mut app,
            handle,
            handle - Vec2::new(0.0, 9.0),
            modifiers,
        );
        let db = amplitude_to_db(app.session.project().clips[0].gain);
        assert!((db - expected).abs() < 0.001, "{db} dB with {modifiers:?}");
    }
}

#[test]
fn clip_gain_handle_takes_priority_where_it_covers_a_fade_handle() {
    let (ctx, mut app) = fixture();
    let clip = app.session.project().clips[0].clone();
    let mut fades = clip.fades;
    fades.fade_in.ticks = clip.length_ticks / 2;
    app.session
        .edit(EditCommand::SetClip {
            id: clip.id,
            start_tick: clip.start_tick,
            gain: 2.0,
            offset_frames: clip.offset_frames,
            length_ticks: clip.length_ticks,
            fades,
            pitch_cents: clip.pitch_cents,
            reverse: false,
        })
        .unwrap();
    let rect = app.clip_rect(&app.session.project().clips[0], app.score.rect.unwrap());
    let handle = gain_handle(rect, app.score.drop_rect.unwrap(), 2.0);
    assert_eq!(
        app.clip_handle_at(handle, app.score.rect.unwrap())
            .map(|h| h.1),
        Some(ClipHandle::Gain),
        "The visible gain handle must receive the drag."
    );
    drag(
        &ctx,
        &mut app,
        handle,
        handle + Vec2::new(0.0, 9.0),
        egui::Modifiers::NONE,
    );
    let changed = &app.session.project().clips[0];
    assert!((amplitude_to_db(changed.gain) - (amplitude_to_db(2.0) - 2.0)).abs() < 0.001);
    assert_eq!(changed.fades, fades);
}

#[test]
fn clip_fades_pitch_and_automation_points_edit_from_the_arrangement() {
    use mixjam_techno::application::{FadeCurve, automation::Lane};
    let set = |ctx: &egui::Context, app: &mut MixJamApp, name: &str, value: f64| {
        let output = frame(ctx, app, vec![], egui::Modifiers::NONE);
        let (target, _) = accessible_node(&output, name);
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
    open_clip_menu(&ctx, &mut app, 1);
    set(&ctx, &mut app, "Clip fade in (beats)", 1.0);
    set(&ctx, &mut app, "Clip fade out (beats)", 0.5);
    set(&ctx, &mut app, "Clip pitch (semitones)", -2.0);
    set(&ctx, &mut app, "Clip pitch (cents)", 25.0);
    let clip = &app.session.project().clips[0];
    assert_eq!(
        (clip.fades.fade_in.ticks, clip.fades.fade_out.ticks),
        (64, 32)
    );
    assert_eq!(clip.fades.fade_in.curve, FadeCurve::Linear);
    assert_eq!(clip.pitch_cents, -175);
    // A fade cannot outgrow the clip: one bar is four beats.
    set(&ctx, &mut app, "Clip fade in (beats)", 9.0);
    assert_eq!(app.session.project().clips[0].fades.fade_in.ticks, 224);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    accessible_node(&output, "Clip fade out curve");
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let open = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("Clip gain"));
    assert!(!open, "Escape closes the clip menu.");
    // Each frame is 1/60 s. Let the double-click time of the right-click pass.
    for _ in 0..20 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }

    // With a lane shown, a click adds a point and a right-click removes it.
    app.score_tools.automation_lane = Some(Lane::Volume);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    accessible_node(&output, "Automation lane");
    let canvas = app.score.rect.unwrap();
    let pos = Pos2::new(
        canvas.left() + app.score.bar_width * 2.0,
        canvas.top() + app.score.row_height / 2.0,
    );
    pointer(&ctx, &mut app, pos, true, egui::Modifiers::NONE);
    pointer(&ctx, &mut app, pos, false, egui::Modifiers::NONE);
    let points = app.session.project().tracks[0].automation.volume.clone();
    assert_eq!(points.len(), 1, "{points:?}");
    assert_eq!(points[0].tick, 2 * TICKS_PER_BAR);
    assert!((points[0].value - -27.0).abs() <= 1.0, "{points:?}");
    assert_eq!(
        app.session.project().clips.len(),
        2,
        "A lane click places no clip."
    );
    // Dragging the point moves it a bar later and to the top of the row.
    let top = Pos2::new(pos.x + app.score.bar_width, canvas.top());
    pointer(&ctx, &mut app, pos, true, egui::Modifiers::NONE);
    for step in 1..=4 {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(
                pos + (top - pos) * step as f32 / 4.0,
            )],
            egui::Modifiers::NONE,
        );
    }
    pointer(&ctx, &mut app, top, false, egui::Modifiers::NONE);
    let points = app.session.project().tracks[0].automation.volume.clone();
    assert_eq!(
        (points.len(), points[0].tick, points[0].value),
        (1, 3 * TICKS_PER_BAR, 6.0)
    );
    let point = Pos2::new(top.x, canvas.top() + 4.0);
    frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Secondary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        egui::Modifiers::NONE,
    );
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        egui::Modifiers::NONE,
    );
    assert!(app.session.project().tracks[0].automation.volume.is_empty());
    app.history_step(false);
    assert_eq!(
        app.session.project().tracks[0].automation.volume.len(),
        1,
        "Undo restores the point."
    );
}
