//! Song overview bar: focus, navigation, folding and drawing.

use super::*;

/// An app whose song is `project`, laid out at 1440x1000.
fn overview_fixture(project: Project) -> (egui::Context, MixJamApp) {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        project,
    )
    .unwrap();
    app.session.set_theme_fixture(theme::current());
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    (ctx, app)
}

/// The clip lines the song overview paints: one mesh inside its strip.
fn overview_mesh(output: &egui::FullOutput, strip: Rect) -> Option<egui::Mesh> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Mesh(mesh) if strip.contains_rect(shape.clip_rect) => {
            Some(mesh.as_ref().clone())
        }
        _ => None,
    })
}

#[test]
fn the_song_overview_keeps_focus_for_repeated_arrow_navigation() {
    let mut editor = Editor::new(Project::default());
    editor
        .place(1, 0, 200 * TICKS_PER_BAR, TICKS_PER_BAR)
        .unwrap();
    let (ctx, mut app) = overview_fixture(editor.into_project());
    let strip = app.overview.rect.unwrap();
    click(
        &ctx,
        &mut app,
        Pos2::new(strip.left() + 12.0, strip.center().y),
        egui::Modifiers::NONE,
    );
    let focus = ctx
        .memory(|memory| memory.focused())
        .expect("overview focus");
    for key in [
        egui::Key::ArrowRight,
        egui::Key::ArrowRight,
        egui::Key::ArrowLeft,
        egui::Key::ArrowLeft,
    ] {
        let before = app.score.view.unwrap();
        frame(
            &ctx,
            &mut app,
            vec![key_event(key, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
        );
        let after = app.score.view.unwrap();
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(focus));
        let step = if key == egui::Key::ArrowRight {
            1.0
        } else {
            -1.0
        };
        assert!((after.offset - before.offset - step * before.visible).abs() < 1.0);
    }
}

#[test]
fn the_song_overview_moves_the_score_view_and_never_edits_the_song() {
    let mut editor = Editor::new(Project::default());
    editor.place(1, 0, 0, TICKS_PER_BAR).unwrap();
    editor
        .place(1, 1, 200 * TICKS_PER_BAR, TICKS_PER_BAR)
        .unwrap();
    let (ctx, mut app) = overview_fixture(editor.into_project());
    let before = app.session.project().clone();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    accessible_node(&output, "Song overview");
    accessible_node(&output, "Hide the song overview");
    let strip = app.overview.rect.unwrap();
    assert_eq!(strip.height(), overview::OVERVIEW_HEIGHT);
    // It sits above the score toolbar.
    assert!(strip.bottom() < effects_control_rect(&output, "Follow").top());
    assert_eq!(app.score.view.unwrap().offset, 0.0);

    // A click near its end moves the view there.
    let at = Pos2::new(strip.right() - 60.0, strip.center().y);
    click(&ctx, &mut app, at, egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let view = app.score.view.unwrap();
    let span = overview::overview_span(app.session.project(), 0, view.ticks());
    let tick = (at.x - strip.left() - 12.0) / (strip.width() - 24.0) * span;
    assert!(view.offset > 0.0);
    assert!(
        (view.start_tick()..=view.start_tick() + view.ticks()).contains(&tick),
        "{view:?} {tick}"
    );
    // Home and End go to the start and the end.
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Home, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(app.score.view.unwrap().offset, 0.0);
    frame(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::End, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
    );
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let view = app.score.view.unwrap();
    assert!(view.start_tick() + view.ticks() >= 201.0 * TICKS_PER_BAR as f32);
    // A drag follows the pointer.
    let start = Pos2::new(strip.left() + 40.0, strip.center().y);
    pointer(&ctx, &mut app, start, true, egui::Modifiers::NONE);
    for step in 1..=4 {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(
                start + Vec2::new(step as f32 * 60.0, 0.0),
            )],
            egui::Modifiers::NONE,
        );
    }
    let dragged = app.score.view.unwrap().offset;
    pointer(
        &ctx,
        &mut app,
        start + Vec2::new(240.0, 0.0),
        false,
        egui::Modifiers::NONE,
    );
    assert!(dragged > 0.0);
    // Nothing of the song changed, and the playhead stayed put.
    assert_eq!(app.session.project(), &before);
    assert!(!app.session.unsaved());
    assert_eq!(app.session.cursor_tick(), 0);
}

#[test]
fn the_song_overview_folds_to_a_named_bar_and_its_status_switch_brings_it_back() {
    let (ctx, mut app) = overview_fixture(Project::default());
    let toggled = |output: &egui::FullOutput| {
        accessible_node(output, "Overview").1.toggled() == Some(egui::accesskit::Toggled::True)
    };
    let output = click_named(&ctx, &mut app, "Hide the song overview");
    assert!(!app.session.layout().overview);
    assert!(!toggled(&output));
    assert_eq!(
        app.overview.rect.unwrap().height(),
        overview::OVERVIEW_BAR_HEIGHT
    );
    text_rect(&output, "SONG OVERVIEW");
    // A click anywhere on the bar opens it.
    let bar = app.overview.rect.unwrap();
    click(
        &ctx,
        &mut app,
        bar.center() + Vec2::new(200.0, 0.0),
        egui::Modifiers::NONE,
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.session.layout().overview);
    assert!(toggled(&output));
    // The status-bar switch folds and opens it.
    click_named(&ctx, &mut app, "Overview");
    assert!(!app.session.layout().overview);
    accessible_node(
        &frame(&ctx, &mut app, vec![], egui::Modifiers::NONE),
        "Show the song overview",
    );
    click_named(&ctx, &mut app, "Overview");
    assert!(app.session.layout().overview);
    // It belongs to the score, so a maximised score keeps it.
    click_named(&ctx, &mut app, "Maximise the score");
    assert!(app.session.layout().maximised);
    assert_eq!(
        app.overview.rect.unwrap().height(),
        overview::OVERVIEW_HEIGHT
    );
}

#[test]
fn a_short_score_keeps_lane_space_and_restores_the_overview_when_it_fits() {
    each_theme(|theme| {
        // Enough lanes to fill the viewport.
        let mut editor = Editor::new(Project::default());
        for track in 0..8 {
            editor.place(1, track, 0, TICKS_PER_BAR).unwrap();
        }
        let (ctx, mut app) = overview_fixture(editor.into_project());
        app.boards.module = BottomModule::Mixer;
        for size in [
            Vec2::new(960.0, 720.0),
            Vec2::new(1024.0, 768.0),
            Vec2::new(1440.0, 820.0),
            Vec2::new(1440.0, 1000.0),
            Vec2::new(960.0, 720.0),
        ] {
            let mut output = egui::FullOutput::default();
            for _ in 0..3 {
                output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            if size.y < 800.0 {
                assert!(app.overview.rect.is_none(), "{theme:?} {size:?}");
                accessible_node(&output, "Overview");
            } else if size.y < 1000.0 {
                assert_eq!(
                    app.overview.rect.unwrap().height(),
                    overview::OVERVIEW_BAR_HEIGHT,
                    "{theme:?} {size:?}"
                );
                let bar = accessible_label(&output, "Song overview, folded for room");
                // A click there changes nothing.
                for pressed in [true, false] {
                    frame_at_size(
                        &ctx,
                        &mut app,
                        vec![
                            egui::Event::PointerMoved(bar.center()),
                            egui::Event::PointerButton {
                                pos: bar.center(),
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        egui::Modifiers::NONE,
                        size,
                    );
                }
            } else {
                assert_eq!(
                    app.overview.rect.unwrap().height(),
                    overview::OVERVIEW_HEIGHT,
                    "{theme:?} {size:?}"
                );
            }
            assert!(app.session.layout().overview);
            assert!(
                app.score.drop_rect.unwrap().height() >= 150.0,
                "{theme:?} {size:?}"
            );
        }
    });
}

#[test]
fn the_song_overview_draws_an_empty_a_sparse_and_a_full_song() {
    // Empty: no clip lines, and the visible window spans the strip.
    let (ctx, mut app) = overview_fixture(Project::default());
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let strip = app.overview.rect.unwrap();
    assert!(overview_mesh(&output, strip).is_none());
    let view = app.score.view.unwrap();
    assert_eq!(
        overview::overview_span(app.session.project(), 0, view.ticks()),
        view.ticks()
    );
    accessible_node(&output, "Song overview");

    // Sparse, with sections: tracks 1 and 64 get a row each, under the
    // section names.
    let mut editor = Editor::new(Project::default());
    editor.place(1, 0, 0, 4 * TICKS_PER_BAR).unwrap();
    editor
        .place(1, TRACK_COUNT - 1, 8 * TICKS_PER_BAR, 4 * TICKS_PER_BAR)
        .unwrap();
    let mut project = editor.into_project();
    project.sections = vec![
        mixjam_techno::application::Section {
            start_tick: 0,
            name: "Intro".into(),
        },
        mixjam_techno::application::Section {
            start_tick: 8 * TICKS_PER_BAR,
            name: "Drop".into(),
        },
    ];
    let (ctx, mut app) = overview_fixture(project);
    app.sections.bookmarks = true;
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let strip = app.overview.rect.unwrap();
    let mesh = overview_mesh(&output, strip).expect("clip lines");
    assert_eq!(mesh.vertices.len(), 2 * 4);
    let tops: Vec<f32> = mesh.vertices.chunks(4).map(|quad| quad[0].pos.y).collect();
    assert!(tops[0] < tops[1], "{tops:?}");
    assert!(
        tops[0] >= strip.top() + 20.0,
        "Rows sit under the section names."
    );
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| strip.contains(vertex.pos))
    );
    text_rect(&output, "INTRO");
    text_rect(&output, "DROP");

    // Full: one line for each of 64 tracks, each on its own row.
    let mut editor = Editor::new(Project::default());
    for track in 0..TRACK_COUNT {
        editor
            .place(1, track, track as u32 * TICKS_PER_BAR, 2 * TICKS_PER_BAR)
            .unwrap();
    }
    let (ctx, mut app) = overview_fixture(editor.into_project());
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let strip = app.overview.rect.unwrap();
    let mesh = overview_mesh(&output, strip).expect("clip lines");
    assert_eq!(mesh.vertices.len(), TRACK_COUNT * 4);
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| strip.contains(vertex.pos))
    );
    let tops: Vec<f32> = mesh.vertices.chunks(4).map(|quad| quad[0].pos.y).collect();
    assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
}
