//! Automation lanes: point editing on the pan, repeat, gate, pitch and return lanes.

use super::*;

#[test]
fn automation_drag_adds_and_moves_a_point_in_one_undo_step() {
    use mixjam_techno::application::automation::Lane;
    let (ctx, mut app) = fixture();
    app.score_tools.automation_lane = Some(Lane::Volume);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let canvas = app.score.rect.unwrap();
    let start = Pos2::new(
        canvas.left() + app.score.bar_width,
        canvas.top() + app.score.row_height / 2.0,
    );
    let end = start + Vec2::new(app.score.bar_width, -6.0);
    pointer(&ctx, &mut app, start, true, egui::Modifiers::NONE);
    for step in 1..=4 {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(
                start + (end - start) * step as f32 / 4.0,
            )],
            egui::Modifiers::NONE,
        );
    }
    pointer(&ctx, &mut app, end, false, egui::Modifiers::NONE);
    assert_eq!(app.session.project().tracks[0].automation.volume.len(), 1);
    app.history_step(false);
    assert!(app.session.project().tracks[0].automation.volume.is_empty());
    app.history_step(true);
    assert_eq!(app.session.project().tracks[0].automation.volume.len(), 1);
    app.gestures.automation_drag = Some((0, 0));
    app.clear_selection();
    assert!(app.gestures.automation_drag.is_none());
}

#[test]
fn the_pan_lane_takes_points_from_left_to_right_and_the_mixer_names_it() {
    use mixjam_techno::application::automation::Lane;
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.session
        .edit(EditCommand::SetMixer {
            tracks: {
                let mut tracks = app.session.project().tracks.clone();
                tracks[0].pan = 0.5;
                tracks
            },
            master_gain: app.session.project().master_gain,
        })
        .unwrap();
    app.score_tools.automation_lane = Some(Lane::Pan);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    accessible_node(&output, "Automation lane");
    text_rect(&output, "Pan");
    let canvas = app.score.rect.unwrap();
    let row = app.score.row_height;
    // The middle of the row is the center, the top hard right, and the
    // bottom hard left, 4 points in from the row's edges.
    for (bar, y, value) in [
        (1.0, row / 2.0 + 0.25, 0.0),
        (2.0, 4.0, 1.0),
        (3.0, row - 4.0, -1.0),
    ] {
        let pos = Pos2::new(canvas.left() + app.score.bar_width * bar, canvas.top() + y);
        click(&ctx, &mut app, pos, egui::Modifiers::NONE);
        // Let the double-click time pass before the next click.
        for _ in 0..20 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let points = &app.session.project().tracks[0].automation.pan;
        let point = points
            .iter()
            .find(|point| point.tick == bar as u32 * TICKS_PER_BAR)
            .unwrap_or_else(|| panic!("{points:?}"));
        assert_eq!(point.value, value, "bar {bar}");
    }
    assert_eq!(
        app.session.project().tracks[0]
            .automation
            .pan
            .iter()
            .map(|point| Lane::Pan.format(point.value))
            .collect::<Vec<_>>(),
        ["C", "R 100", "L 100"]
    );
    // The knob keeps its value, and the mixer says the lane moves the pan.
    assert_eq!(app.session.project().tracks[0].pan, 0.5);
    app.score_tools.automation_lane = None;
    app.boards.module = BottomModule::Mixer;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "Pan automated");
    // Undo removes the last point; the other two remain.
    app.history_step(false);
    assert_eq!(app.session.project().tracks[0].automation.pan.len(), 2);
}

#[test]
fn the_repeat_lane_snaps_to_its_steps_and_draws_them_as_steps() {
    use mixjam_techno::application::automation::Lane;
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.score_tools.automation_lane = Some(Lane::Repeat);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    accessible_node(&output, "Automation lane");
    text_rect(&output, "Repeat");
    let canvas = app.score.rect.unwrap();
    let row = app.score.row_height;
    // Six even steps from Off at the bottom to 1/64 at the top, 4 points
    // in from the row's edges. A click between two steps takes the
    // nearer one.
    let height = |along: f32| 4.0 + (row - 8.0) * (1.0 - along);
    for (bar, along, value) in [
        (1.0, 0.45, 8.0),
        (1.25, 1.0, 64.0),
        (1.5, 0.62, 16.0),
        (2.0, 0.0, 0.0),
    ] {
        let pos = Pos2::new(
            canvas.left() + app.score.bar_width * bar,
            canvas.top() + height(along),
        );
        click(&ctx, &mut app, pos, egui::Modifiers::NONE);
        // Let the double-click time pass before the next click.
        for _ in 0..20 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let points = &app.session.project().tracks[0].automation.repeat;
        let tick = (bar * TICKS_PER_BAR as f32) as u32;
        let point = points
            .iter()
            .find(|point| point.tick == tick)
            .unwrap_or_else(|| panic!("{points:?}"));
        assert_eq!(point.value, value, "bar {bar}");
    }
    let points = &app.session.project().tracks[0].automation.repeat;
    assert_eq!(
        points
            .iter()
            .map(|point| Lane::Repeat.format(point.value))
            .collect::<Vec<_>>(),
        ["1/8", "1/64", "1/16", "Off"]
    );
    // The lane is drawn as steps: off from the left edge, then each value
    // held flat until the next point, where the line rises or falls
    // straight up or down.
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let x = |bar: f32| canvas.left() + app.score.bar_width * bar;
    let y = |along: f32| canvas.top() + height(along);
    let expected = [
        (canvas.left(), y(0.0)),
        (x(1.0), y(0.0)),
        (x(1.0), y(0.4)),
        (x(1.25), y(0.4)),
        (x(1.25), y(1.0)),
        (x(1.5), y(1.0)),
        (x(1.5), y(0.6)),
        (x(2.0), y(0.6)),
        (x(2.0), y(0.0)),
    ];
    let drawn =
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Path(path) => {
                path.points.len() == expected.len() + 1
                    && path.points.iter().zip(&expected).all(|(point, (x, y))| {
                        (point.x - x).abs() < 0.01 && (point.y - y).abs() < 0.01
                    })
            }
            _ => false,
        });
    assert!(drawn, "The Repeat lane must be drawn as steps.");
}

#[test]
fn the_repeat_gate_lane_snaps_to_five_percent_and_steps_down_from_full() {
    use mixjam_techno::application::automation::Lane;
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.score_tools.automation_lane = Some(Lane::RepeatGate);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "Repeat gate");
    let canvas = app.score.rect.unwrap();
    let row = app.score.row_height;
    // 5% at the bottom of the row and 100% at the top, 4 points in.
    let height = |percent: f32| 4.0 + (row - 8.0) * (1.0 - (percent - 5.0) / 95.0);
    for (bar, aim, value) in [(1.0, 26.0, 25.0), (1.5, 63.0, 65.0), (2.0, 99.0, 100.0)] {
        let pos = Pos2::new(
            canvas.left() + app.score.bar_width * bar,
            canvas.top() + height(aim),
        );
        click(&ctx, &mut app, pos, egui::Modifiers::NONE);
        for _ in 0..20 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let points = &app.session.project().tracks[0].automation.repeat_gate;
        let tick = (bar * TICKS_PER_BAR as f32) as u32;
        let point = points
            .iter()
            .find(|point| point.tick == tick)
            .unwrap_or_else(|| panic!("{points:?}"));
        assert_eq!(point.value, value, "bar {bar}");
    }
    let points = &app.session.project().tracks[0].automation.repeat_gate;
    assert_eq!(
        points
            .iter()
            .map(|point| Lane::RepeatGate.format(point.value))
            .collect::<Vec<_>>(),
        ["25%", "65%", "100%"]
    );
    // The line starts at 100% on the left, as the gate is full before
    // its first point, and steps from there.
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let x = |bar: f32| canvas.left() + app.score.bar_width * bar;
    let y = |percent: f32| canvas.top() + height(percent);
    let expected = [
        (canvas.left(), y(100.0)),
        (x(1.0), y(100.0)),
        (x(1.0), y(25.0)),
        (x(1.5), y(25.0)),
        (x(1.5), y(65.0)),
        (x(2.0), y(65.0)),
        (x(2.0), y(100.0)),
    ];
    let drawn =
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Path(path) => {
                path.points.len() == expected.len() + 1
                    && path.points.iter().zip(&expected).all(|(point, (x, y))| {
                        (point.x - x).abs() < 0.01 && (point.y - y).abs() < 0.01
                    })
            }
            _ => false,
        });
    assert!(
        drawn,
        "The Repeat gate lane must be drawn as steps from 100%."
    );
}

#[test]
fn the_pitch_lane_snaps_to_semitones_and_rests_at_zero_before_its_first_point() {
    use mixjam_techno::application::automation::Lane;
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    choose_automation_lane(&ctx, &mut app, "Pitch");
    assert_eq!(
        (app.score_tools.automation_lane, app.score_tools.return_lane),
        (Some(Lane::Pitch), None)
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    text_rect(&output, "Pitch");
    let canvas = app.score.rect.unwrap();
    let row = app.score.row_height;
    // -4800 cents at the bottom of the row and 2400 at the top, 4 points in.
    let height = |cents: f32| 4.0 + (row - 8.0) * (1.0 - (cents + 4800.0) / 7200.0);
    for (bar, aim, value) in [
        (1.0, -2390.0, -2400.0),
        (1.5, -4800.0, -4800.0),
        (2.0, 40.0, 0.0),
    ] {
        let pos = Pos2::new(
            canvas.left() + app.score.bar_width * bar,
            canvas.top() + height(aim),
        );
        click(&ctx, &mut app, pos, egui::Modifiers::NONE);
        for _ in 0..20 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let points = &app.session.project().tracks[0].automation.pitch;
        let tick = (bar * TICKS_PER_BAR as f32) as u32;
        let point = points
            .iter()
            .find(|point| point.tick == tick)
            .unwrap_or_else(|| panic!("{points:?}"));
        assert_eq!(point.value, value, "bar {bar}");
    }
    let points = &app.session.project().tracks[0].automation.pitch;
    assert_eq!(
        points
            .iter()
            .map(|point| Lane::Pitch.format(point.value))
            .collect::<Vec<_>>(),
        ["-24 st", "-48 st", "0 st"]
    );
    // The line rests at 0 on the left, jumps to the first point, and
    // moves in straight lines from there.
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let x = |bar: f32| canvas.left() + app.score.bar_width * bar;
    let y = |cents: f32| canvas.top() + height(cents);
    let expected = [
        (canvas.left(), y(0.0)),
        (x(1.0), y(0.0)),
        (x(1.0), y(-2400.0)),
        (x(1.5), y(-4800.0)),
        (x(2.0), y(0.0)),
    ];
    let drawn =
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Path(path) => {
                path.points.len() == expected.len() + 1
                    && path.points.iter().zip(&expected).all(|(point, (x, y))| {
                        (point.x - x).abs() < 0.01 && (point.y - y).abs() < 0.01
                    })
            }
            _ => false,
        });
    assert!(
        drawn,
        "The Pitch lane must rest at 0 before its first point."
    );
}

#[test]
fn a_return_lane_spans_the_arrangement_and_edits_its_bus_in_one_undo_step() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.score_tools.return_lane = Some(1);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let canvas = app.score.rect.unwrap();
    let row = view::return_row(canvas, app.score.drop_rect.unwrap());
    let bar_width = app.score.bar_width;
    let at = |bar: f32, value: f32| {
        Pos2::new(canvas.left() + bar_width * bar, view::return_y(value, row))
    };
    // Clicks add points, at the value of their height.
    click(&ctx, &mut app, at(1.0, 0.25), egui::Modifiers::NONE);
    for _ in 0..20 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    click(&ctx, &mut app, at(2.0, 1.5), egui::Modifiers::NONE);
    for _ in 0..20 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let points = |app: &MixJamApp| -> Vec<(u32, f32)> {
        app.session.project().return_automation[1]
            .iter()
            .map(|point| (point.tick, point.value))
            .collect()
    };
    assert_eq!(points(&app), [(256, 0.25), (512, 1.5)]);
    // Near the full return the pointer holds 100%.
    click(&ctx, &mut app, at(3.0, 1.02), egui::Modifiers::NONE);
    for _ in 0..20 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    assert_eq!(points(&app)[2], (768, 1.0));
    // A drag moves a point and is one undo step; the other lanes keep
    // theirs, and no track lane changes.
    drag(
        &ctx,
        &mut app,
        at(2.0, 1.5),
        at(2.5, 2.0),
        egui::Modifiers::NONE,
    );
    for _ in 0..20 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    assert_eq!(points(&app)[1], (640, 2.0));
    assert!(app.session.project().return_automation[0].is_empty());
    assert!(
        app.session
            .project()
            .tracks
            .iter()
            .all(|track| track.automation.is_empty())
    );
    app.history_step(false);
    assert_eq!(points(&app)[1], (512, 1.5));
    // A right-click removes a point; clips do not move meanwhile.
    let clips = app.session.project().clips.clone();
    let pos = at(1.0, 0.25);
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
    assert_eq!(points(&app), [(512, 1.5), (768, 1.0)]);
    assert_eq!(app.session.project().clips, clips);
}
