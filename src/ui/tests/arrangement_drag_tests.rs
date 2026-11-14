//! Arrangement pointer editing: selection, group drags, copies, drops and click
//! placement.

use super::*;

#[test]
fn native_canvas_shift_selection_group_drag_and_delete_are_wired() {
    let (ctx, mut app) = fixture();
    let canvas = app.score.rect.unwrap();
    let first = app
        .clip_rect(&app.session.project().clips[0], canvas)
        .center()
        + GRIP;
    let second = app
        .clip_rect(&app.session.project().clips[1], canvas)
        .center()
        + GRIP;
    click(&ctx, &mut app, first, egui::Modifiers::NONE);
    click(&ctx, &mut app, second, egui::Modifiers::SHIFT);
    assert_eq!(
        app.selection.clips.len(),
        2,
        "Shift must add a second clip."
    );
    let target = first + Vec2::new(app.score.bar_width, 0.0);
    pointer(&ctx, &mut app, first, true, egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(target)],
        egui::Modifiers::NONE,
    );
    pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
    assert_eq!(app.session.project().clips[0].start_tick, TICKS_PER_BAR);
    assert_eq!(app.session.project().clips[1].start_tick, TICKS_PER_BAR * 2);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        egui::Modifiers::NONE,
    );
    assert!(
        app.session.project().clips.is_empty(),
        "Delete must remove the selected group."
    );
    app.history_step(false);
    assert_eq!(app.session.project().clips.len(), 2);
}

#[test]
fn a_short_drag_keeps_the_bar_and_an_erased_clip_leaves_the_selection() {
    let (ctx, mut app) = fixture();
    let canvas = app.score.rect.unwrap();
    let bar = app.score.bar_width;
    let first = app
        .clip_rect(&app.session.project().clips[0], canvas)
        .center()
        + GRIP;
    let second = app
        .clip_rect(&app.session.project().clips[1], canvas)
        .center()
        + GRIP;
    let starts = |app: &MixJamApp| {
        app.session
            .project()
            .clips
            .iter()
            .map(|clip| clip.start_tick)
            .collect::<Vec<_>>()
    };
    let before = starts(&app);
    // The nearest bar decides, to the left as well as to the right.
    for (distance, bars) in [(-0.3, 0), (0.3, 0), (0.7, 1), (-0.7, -1)] {
        let from = app
            .clip_rect(&app.session.project().clips[1], canvas)
            .center()
            + GRIP;
        let target = from + Vec2::new(bar * distance, 0.0);
        pointer(&ctx, &mut app, from, true, egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
        let expected = before[1] as i64 + bars * i64::from(TICKS_PER_BAR);
        assert_eq!(
            i64::from(app.session.project().clips[1].start_tick),
            expected,
            "A drag of {distance} bars must move the clip {bars} bars."
        );
        if bars != 0 {
            app.history_step(false);
        }
    }
    assert_eq!(starts(&app), before);
    click(&ctx, &mut app, first, egui::Modifiers::NONE);
    click(&ctx, &mut app, second, egui::Modifiers::SHIFT);
    let erased = app.session.project().clips[0].id;
    app.score_tools.erase = true;
    click(&ctx, &mut app, first, egui::Modifiers::NONE);
    app.score_tools.erase = false;
    assert!(!app.selection.clips.contains(&erased));
    app.delete_selection();
    assert!(
        app.session.project().clips.is_empty(),
        "Delete must still remove the rest of the selection."
    );
}

#[test]
fn native_canvas_alt_drag_copies_group_and_undo_restores_once() {
    let (ctx, mut app) = fixture();
    let canvas = app.score.rect.unwrap();
    let first = app
        .clip_rect(&app.session.project().clips[0], canvas)
        .center();
    app.selection.clips = [1, 2].into_iter().collect();
    app.selection.clip = Some(1);
    let target = first + Vec2::new(app.score.bar_width * 3.0, 0.0);
    pointer(&ctx, &mut app, first, true, egui::Modifiers::ALT);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(target)],
        egui::Modifiers::ALT,
    );
    pointer(&ctx, &mut app, target, false, egui::Modifiers::ALT);
    assert_eq!(app.session.project().clips.len(), 4);
    assert_eq!(app.selection.clips.len(), 2);
    app.history_step(false);
    assert_eq!(app.session.project().clips.len(), 2);
}

#[test]
fn sparse_tracks_keep_model_ids_for_drag_copy_selection_and_mixer() {
    let (ctx, mut app) = fixture();
    let mut project = app.session.project().clone();
    project.clips[1].track = 63;
    project.tracks[31].gain = 1.25;
    project.tracks[31].pan = 0.4;
    project.tracks[31].left_enabled = false;
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    let hidden = app.session.project().tracks[31].clone();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.score.tracks, vec![0, 63, 1]);
    assert_eq!(app.score.add_track, Some(1));
    let canvas = app.score.rect.unwrap();
    let first = app
        .clip_rect(&app.session.project().clips[0], canvas)
        .center()
        + GRIP;
    let second = app
        .clip_rect(&app.session.project().clips[1], canvas)
        .center()
        + GRIP;
    assert!((second.y - first.y - app.score.row_height).abs() < 0.1);
    let target = Pos2::new(first.x + 3.0 * app.score.bar_width, second.y);
    click(&ctx, &mut app, first, egui::Modifiers::NONE);
    pointer(&ctx, &mut app, first, true, egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(target)],
        egui::Modifiers::NONE,
    );
    pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
    let moved = app
        .session
        .project()
        .clips
        .iter()
        .find(|clip| clip.id == 1)
        .unwrap();
    assert_eq!(moved.track, 63);
    assert_eq!(moved.start_tick, 3 * TICKS_PER_BAR);
    app.history_step(false);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let canvas = app.score.rect.unwrap();
    let first = app
        .clip_rect(&app.session.project().clips[0], canvas)
        .center()
        + GRIP;
    let second = app
        .clip_rect(&app.session.project().clips[1], canvas)
        .center()
        + GRIP;
    let target = Pos2::new(first.x + 3.0 * app.score.bar_width, second.y);
    click(&ctx, &mut app, first, egui::Modifiers::NONE);
    pointer(&ctx, &mut app, first, true, egui::Modifiers::ALT);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(target)],
        egui::Modifiers::ALT,
    );
    pointer(&ctx, &mut app, target, false, egui::Modifiers::ALT);
    assert_eq!(app.session.project().clips.len(), 3);
    assert_eq!(app.session.project().clips[0].track, 0);
    assert!(
        app.session
            .project()
            .clips
            .iter()
            .any(|clip| clip.track == 63 && clip.start_tick == 3 * TICKS_PER_BAR)
    );
    app.history_step(false);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let canvas = app.score.rect.unwrap();
    let origin = canvas.min + Vec2::splat(1.0);
    let target = Pos2::new(
        canvas.left() + 2.5 * app.score.bar_width,
        canvas.top() + 2.0 * app.score.row_height - 1.0,
    );
    pointer(&ctx, &mut app, origin, true, egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(target)],
        egui::Modifiers::NONE,
    );
    pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
    assert_eq!(app.selection.clips, [1, 2].into_iter().collect());
    let visible = app.score.drop_rect.unwrap();
    let second_y = canvas.top() + 1.5 * app.score.row_height;
    click(
        &ctx,
        &mut app,
        Pos2::new(header_x(&ctx, visible, lanes::SWITCH_X[2]), second_y),
        egui::Modifiers::NONE,
    );
    assert!(!app.session.project().tracks[63].left_enabled);
    assert!(app.session.project().tracks[1].left_enabled);
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 350.0))),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.mixer(ui));
        },
    );
    let labels: BTreeSet<_> = output
        .shapes
        .iter()
        .filter_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape {
                Some(text.galley.job.text.as_str())
            } else {
                None
            }
        })
        .collect();
    assert!(labels.contains("Track 1") && labels.contains("Track 64"));
    assert!(!labels.contains("Track 2") && !labels.contains("Track 32"));
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let canvas = app.score.rect.unwrap();
    let add = Pos2::new(
        canvas.left() + 3.5 * app.score.bar_width,
        canvas.top() + 2.5 * app.score.row_height,
    );
    // Choosing a sound in the palette arms click placement.
    app.selection.sample = Some(1);
    app.selection.place_on_click = true;
    click(&ctx, &mut app, add, egui::Modifiers::NONE);
    assert!(
        app.session
            .project()
            .clips
            .iter()
            .any(|clip| clip.track == 1)
    );
    assert_eq!(
        app.session
            .project()
            .clips
            .iter()
            .find(|clip| clip.id == 2)
            .unwrap()
            .track,
        63
    );
    assert_eq!(app.session.project().tracks[31], hidden);
    assert_eq!(app.session.project().tracks.len(), 64);
}

#[test]
fn empty_project_record_chooser_can_create_track_64_without_empty_rows() {
    let (ctx, mut app) = fixture();
    app.new_mix();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.score.tracks, vec![0]);
    assert_eq!(app.score.add_track, Some(0));
    assert!(occupied_tracks(app.session.project()).is_empty());
    assert!((app.score.rect.unwrap().height() - app.score.row_height).abs() < 0.1);
    let menu_ctx = egui::Context::default();
    configure_style(&menu_ctx);
    let mut selected = 0;
    let mut menu = |events: Vec<egui::Event>| {
        let mut rect = Rect::NOTHING;
        let output = menu_ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    rect = recording_destination(ui, &mut selected, &[]).rect;
                });
            },
        );
        (output, rect)
    };
    let (_, rect) = menu(vec![]);
    let mouse = |pos, pressed| {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    menu(mouse(rect.center(), true));
    menu(mouse(rect.center(), false));
    let (output, _) = menu(vec![]);
    let first = output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.job.text == "New track 1"
            {
                Some(Rect::from_min_size(text.pos, text.galley.size()).center())
            } else {
                None
            }
        })
        .unwrap();
    menu(vec![
        egui::Event::PointerMoved(first),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -5000.0),
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    for _ in 0..5 {
        menu(vec![]);
    }
    let (output, _) = menu(vec![]);
    let last = output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.job.text == "New track 64"
            {
                let pos = Rect::from_min_size(text.pos, text.galley.size()).center();
                shape.clip_rect.contains(pos).then_some(pos)
            } else {
                None
            }
        })
        .expect("The recording chooser must reach unused track 64.");
    menu(vec![egui::Event::PointerMoved(last)]);
    menu(mouse(last, true));
    menu(mouse(last, false));
    assert_eq!(selected, 63);
    app.recording.track = selected;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.score.tracks, vec![63]);
    assert_eq!(app.score.add_track, Some(63));
    let canvas = app.score.rect.unwrap();
    app.selection.sample = Some(1);
    let drop = canvas.min + Vec2::new(30.0, app.score.row_height / 2.0);
    click(&ctx, &mut app, drop, egui::Modifiers::NONE);
    assert_eq!(app.session.project().clips.len(), 1);
    assert_eq!(app.session.project().clips[0].track, 63);
    assert_eq!(app.session.project().tracks.len(), 64);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(occupied_tracks(app.session.project()), vec![63]);
    assert!(app.score.tracks.len() <= 2);
}

#[test]
fn dropping_a_group_below_the_add_target_cancels_without_deleting_or_history() {
    let (ctx, mut app) = fixture();
    app.selection.clips = [1, 2].into_iter().collect();
    app.selection.clip = Some(1);
    let before = app.session.project().clone();
    let was_unsaved = app.session.unsaved();
    let canvas = app.score.rect.unwrap();
    let first = app
        .clip_rect(&app.session.project().clips[0], canvas)
        .center()
        + GRIP;
    let below = Pos2::new(first.x, canvas.bottom() + 15.0);
    pointer(&ctx, &mut app, first, true, egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(below)],
        egui::Modifiers::NONE,
    );
    pointer(&ctx, &mut app, below, false, egui::Modifiers::NONE);
    assert_eq!(app.session.project(), &before);
    assert_eq!(app.selection.clips, [1, 2].into_iter().collect());
    assert_eq!(app.selection.clip, Some(1));
    assert_eq!(app.session.unsaved(), was_unsaved);
    assert!(app.status.contains("Move canceled"));
    assert!(
        !app.session.history_step(false).unwrap(),
        "Canceled movement must not add an edit-history entry."
    );
}

#[test]
fn a_group_drag_onto_a_track_without_a_row_names_it_and_cancels() {
    let (ctx, mut app) = fixture();
    // Tracks 1 and 2 hold the clips and track 3 is the Add row; track 4
    // has no row.
    assert_eq!(app.score.tracks, vec![0, 1, 2]);
    app.selection.clips = [1, 2].into_iter().collect();
    app.selection.clip = Some(1);
    let before = app.session.project().clone();
    let was_unsaved = app.session.unsaved();
    let canvas = app.score.rect.unwrap();
    let row = Vec2::new(0.0, app.score.row_height);
    let grip = |app: &MixJamApp, index: usize| {
        app.clip_rect(&app.session.project().clips[index], canvas)
            .center()
            + GRIP
    };
    let hover = |ctx: &egui::Context, app: &mut MixJamApp, to: Pos2| {
        frame(
            ctx,
            app,
            vec![egui::Event::PointerMoved(to)],
            egui::Modifiers::NONE,
        );
        frame(ctx, app, vec![], egui::Modifiers::NONE)
    };
    let shows = |output: &egui::FullOutput, label: &str| {
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Text(text) => text.galley.job.text == label,
            _ => false,
        })
    };
    let first = grip(&app, 0);
    pointer(&ctx, &mut app, first, true, egui::Modifiers::NONE);
    // One row down, the second clip lands on the Add row: both have ghosts.
    let output = hover(&ctx, &mut app, first + row);
    let item = app
        .gestures
        .drag
        .as_ref()
        .expect("The press must start a clip drag.");
    assert_eq!(app.drop_ghosts(item, first + row, false).len(), 2);
    assert!(!shows(&output, "Track 4 has no row"));
    // Two rows down, the second clip would land on track 4.
    let output = hover(&ctx, &mut app, first + row * 2.0);
    let item = app.gestures.drag.as_ref().unwrap();
    assert!(
        app.drop_ghosts(item, first + row * 2.0, false).is_empty(),
        "A drop that does nothing must not show ghosts."
    );
    assert!(
        shows(&output, "Track 4 has no row"),
        "The drag label must name the track without a row."
    );
    pointer(
        &ctx,
        &mut app,
        first + row * 2.0,
        false,
        egui::Modifiers::NONE,
    );
    assert_eq!(
        app.session.project(),
        &before,
        "No clip may land on a track the arrangement does not show."
    );
    assert_eq!(app.session.unsaved(), was_unsaved);
    assert_eq!(app.selection.clips, [1, 2].into_iter().collect());
    assert!(
        app.status.contains("Move canceled. Track 4 has no row."),
        "{}",
        app.status
    );
    // One row up from track 2, the first clip would leave the timeline.
    let second = grip(&app, 1);
    pointer(&ctx, &mut app, second, true, egui::Modifiers::NONE);
    let output = hover(&ctx, &mut app, second - row);
    let item = app.gestures.drag.as_ref().unwrap();
    assert!(app.drop_ghosts(item, second - row, false).is_empty());
    assert!(shows(&output, "Outside the timeline"));
    pointer(&ctx, &mut app, second - row, false, egui::Modifiers::NONE);
    assert_eq!(app.session.project(), &before);
    assert!(
        !app.session.history_step(false).unwrap(),
        "Canceled movement must not add an edit-history entry."
    );
    // A group may still fill the Add row.
    drag(&ctx, &mut app, first, first + row, egui::Modifiers::NONE);
    let tracks: Vec<_> = app
        .session
        .project()
        .clips
        .iter()
        .map(|clip| clip.track)
        .collect();
    assert_eq!(tracks, vec![1, 2]);
}

#[test]
fn choosing_a_clip_ends_click_placement_until_a_sound_is_chosen_again() {
    let (ctx, mut app) = fixture();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let canvas = app.score.rect.unwrap();
    let clip = app.session.project().clips[0].clone();
    let on_clip = app.clip_rect(&clip, canvas).center();
    let count = app.session.project().clips.len();
    click(&ctx, &mut app, on_clip, egui::Modifiers::NONE);
    assert_eq!(app.selection.clip, Some(clip.id));
    assert!(!app.selection.place_on_click);
    let empty = (0..20)
        .map(|bar| {
            Pos2::new(
                canvas.left() + (bar as f32 + 0.5) * app.score.bar_width,
                on_clip.y,
            )
        })
        .rev()
        .find(|pos| {
            // Inside the score, clear of its margin and scroll bar.
            pos.x < 1440.0 - 40.0
                && !app
                    .session
                    .project()
                    .clips
                    .iter()
                    .any(|clip| app.clip_rect(clip, canvas).contains(*pos))
        })
        .unwrap();
    click(&ctx, &mut app, empty, egui::Modifiers::NONE);
    assert_eq!(
        app.session.project().clips.len(),
        count,
        "A click on an empty lane after choosing a clip must only clear the selection."
    );
    assert!(app.selection.clips.is_empty());
    app.selection.place_on_click = true;
    click(&ctx, &mut app, empty, egui::Modifiers::NONE);
    assert_eq!(
        app.session.project().clips.len(),
        count + 1,
        "A sound chosen in the palette is placed by a click."
    );
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Escape, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    assert!(
        !app.selection.place_on_click,
        "Esc must end click placement."
    );
}

#[test]
fn placement_outline_matches_the_length_of_the_placed_clip() {
    let (ctx, mut app) = fixture();
    // A sound outside the product catalogs takes its length from its audio:
    // two bars here, although `bars` says one.
    let id = 900_001;
    assert!(Product::sample_key(id).is_none());
    let sample = Sample {
        id,
        code: "WAVE".into(),
        name: "long take".into(),
        bars: 1,
        group_name: "Loop".into(),
        offset: 0,
        length: 0,
        frame_count: 151_200,
        sample_rate: 44100,
        channels: 1,
        source_bpm: 140.0,
        wav_path: None,
        digest: None,
    };
    app.session =
        Session::headless(Arc::new(Library::fixture(vec![sample])), Project::default()).unwrap();
    app.selection.sample = Some(id);
    app.selection.place_on_click = true;
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let canvas = app.score.rect.unwrap();
    let pos = Pos2::new(
        canvas.left() + 1.5 * app.score.bar_width,
        canvas.top() + 0.5 * app.score.row_height,
    );
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(pos)],
        egui::Modifiers::NONE,
    );
    let output = frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(pos)],
        egui::Modifiers::NONE,
    );
    click(&ctx, &mut app, pos, egui::Modifiers::NONE);
    let clip = app.session.project().clips[0].clone();
    assert_eq!(clip.length_ticks, 2 * TICKS_PER_BAR);
    let width = app.clip_rect(&clip, app.score.rect.unwrap()).width();
    assert!(
        output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Rect(rect) if rect.stroke.color == roles().text
                && (rect.rect.width() - (width - 1.0)).abs() < 0.01)),
        "The outline must show the length of the clip that a click places."
    );
}

#[test]
fn shift_drag_copies_clips_and_a_ghost_shows_where_they_land() {
    let (ctx, mut app) = fixture();
    let lanes = app.score.rect.unwrap();
    let source = app.session.project().clips[0].clone();
    let count = app.session.project().clips.len();
    let rect = app.clip_rect(&source, lanes);
    let from = Pos2::new(rect.left() + rect.width() / 4.0, rect.bottom() - 4.0);
    let to = from + Vec2::new(4.0 * app.score.bar_width, 0.0);
    let shift = egui::Modifiers::SHIFT;
    pointer(&ctx, &mut app, from, true, shift);
    for step in 1..=4 {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(
                from + (to - from) * step as f32 / 4.0,
            )],
            shift,
        );
    }
    let Some(drag) = &app.gestures.drag else {
        panic!("Shift must start a clip drag.");
    };
    assert!(matches!(drag, DragItem::Clip { copy: true, .. }));
    let ghosts = app.drop_ghosts(drag, to, false);
    assert_eq!(ghosts.len(), app.selection.clips.len());
    assert_eq!(
        ghosts[0].0.left(),
        app.lane_rect(
            source.track,
            source.start_tick + 4 * TICKS_PER_BAR,
            1,
            lanes
        )
        .left(),
        "The ghost starts where the drop puts the clip."
    );
    assert!(
        app.drop_ghosts(drag, Pos2::new(-50.0, -50.0), false)
            .is_empty()
    );
    pointer(&ctx, &mut app, to, false, shift);
    let clips = &app.session.project().clips;
    assert_eq!(clips.len(), count + 1, "Shift-drag copies.");
    assert!(clips.contains(&source), "The source stays.");
    assert!(clips.iter().any(
        |clip| clip.start_tick == source.start_tick + 4 * TICKS_PER_BAR
            && clip.sample_id == source.sample_id
    ));
}
