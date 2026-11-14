//! Workspace layout: maximised score, panel switches, divider and starting panels.

use super::*;

#[test]
fn lower_panel_divider_resizes_retains_height_and_preserves_arrangement() {
    let (ctx, mut app) = fixture();
    for track in 2..TRACK_COUNT {
        app.session.place(1, track, 0).unwrap();
    }
    let base = app.session.library().samples[0].clone();
    let samples = (1..=128)
        .map(|id| {
            let mut sample = base.clone();
            sample.id = id;
            sample.name = format!("Resizable sound {id}");
            sample
        })
        .collect();
    app.session = Session::headless(
        Arc::new(Library::fixture(samples)),
        app.session.project().clone(),
    )
    .unwrap();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let panel = || {
        egui::containers::panel::PanelState::load(&ctx, egui::Id::new("sounds"))
            .unwrap()
            .rect
    };
    let before = panel();
    let arrangement_before = app.score.drop_rect.unwrap().height();
    let start = Pos2::new(before.center().x, before.top() + 1.0);
    let target = start - Vec2::new(0.0, 150.0);
    pointer(&ctx, &mut app, start, true, egui::Modifiers::NONE);
    for _ in 0..2 {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::NONE,
        );
    }
    pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let expanded = panel();
    assert!(
        expanded.height() > before.height() + 100.0,
        "{before:?} -> {expanded:?}"
    );
    assert!(app.score.drop_rect.unwrap().height() < arrangement_before);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        panel(),
        expanded,
        "The divider must keep its height after release."
    );
    // Shrinking the window clamps a previous large panel to preserve the lanes.
    for _ in 0..2 {
        frame_at_size(
            &ctx,
            &mut app,
            vec![],
            egui::Modifiers::NONE,
            Vec2::new(960.0, 720.0),
        );
    }
    // The largest minimum any window size gives a module.
    let shared_minimum = BottomModule::ALL
        .into_iter()
        .map(|module| module.minimum_height(f32::INFINITY))
        .fold(0.0_f32, f32::max);
    assert!(
        panel().height() <= shared_minimum,
        "The minimum viewport must clamp to the shared module height limit."
    );
    assert!(app.score.drop_rect.unwrap().height() >= 80.0);
    assert!(!app.session.unsaved());
}

#[test]
fn the_divider_stops_where_the_sounds_panel_keeps_its_height() {
    let (ctx, mut app) = fixture();
    app.set_layout(|layout| {
        layout.sounds = true;
        layout.boards = true;
    });
    app.open_sounds_panel();
    app.set_layout(|layout| layout.boards = true);
    app.resolve_room();
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.panels.room.sounds && app.panels.room.boards);
    let panel = || {
        egui::containers::panel::PanelState::load(&ctx, egui::Id::new("sounds"))
            .unwrap()
            .rect
    };
    let before = panel();
    let start = Pos2::new(before.center().x, before.top() + 1.0);
    let target = start - Vec2::new(0.0, 500.0);
    pointer(&ctx, &mut app, start, true, egui::Modifiers::NONE);
    for _ in 0..3 {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::NONE,
        );
        assert!(app.panels.room.sounds && app.panels.room.boards, "mid-drag");
    }
    pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.panels.room.sounds && app.panels.room.boards);
    assert!(panel().height() > before.height() + 100.0, "{:?}", panel());
    let id = egui::Id::new("sounds-panel");
    let sounds = egui::containers::panel::PanelState::load(&ctx, id)
        .unwrap()
        .rect;
    assert!(sounds.height() >= room::SOUNDS_MIN_HEIGHT, "{sounds:?}");
}

#[test]
fn f_maximises_the_score_and_f_again_restores_the_rail_and_the_lower_panel() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.set_layout(|layout| layout.sounds = true);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(has_control(&output, "New") && has_control(&output, "Mixer tab"));
    let press = |app: &mut MixJamApp| {
        frame(
            &ctx,
            app,
            vec![
                key_event(egui::Key::F, egui::Modifiers::NONE),
                egui::Event::Text("f".into()),
            ],
            egui::Modifiers::NONE,
        );
        // Release it, or the next press reads as a held key's repeat.
        frame(
            &ctx,
            app,
            vec![egui::Event::Key {
                key: egui::Key::F,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        )
    };
    let output = press(&mut app);
    assert!(app.session.layout().maximised);
    assert!(!has_control(&output, "New"), "The rail hides.");
    assert!(!has_control(&output, "Mixer tab"), "The lower panel hides.");
    // The top bar, the score toolbar and the status bar stay.
    for name in ["Play / pause (Space)", "Help", "Follow", "Tools", "Boards"] {
        assert!(has_control(&output, name), "{name} stays.");
    }
    assert_eq!(
        accessible_node(&output, "Maximise the score").1.toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    // The panels' own settings are kept for the restore.
    assert!(app.session.layout().tool_names && app.session.layout().boards);
    let output = press(&mut app);
    assert!(!app.session.layout().maximised);
    assert!(has_control(&output, "New") && has_control(&output, "Mixer tab"));

    // Typing keeps the letter.
    focus_control(&ctx, &mut app, &output, "Find a sound");
    press(&mut app);
    assert!(
        !app.session.layout().maximised,
        "F in a text field is typing."
    );
}

#[test]
fn maximise_button_and_either_panel_switch_restore_the_layout() {
    for (switch, tool_names, boards) in ["Tools", "Boards"].into_iter().flat_map(|switch| {
        [false, true].into_iter().flat_map(move |tool_names| {
            [false, true].map(move |boards| (switch, tool_names, boards))
        })
    }) {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        let original = mixjam_techno::application::options::WorkspaceLayout {
            tool_names,
            boards,
            maximised: false,
            sounds: false,
            overview: true,
            inspector: false,
        };
        app.session.set_layout(original).unwrap();
        let output = click_named(&ctx, &mut app, "Maximise the score");
        assert!(app.session.layout().maximised);
        for name in ["Tools", "Boards"] {
            assert_eq!(
                accessible_node(&output, name).1.toggled(),
                Some(egui::accesskit::Toggled::False),
                "{name} reads off while the score fills the window."
            );
        }
        let output = click_named(&ctx, &mut app, switch);
        let layout = app.session.layout();
        assert!(!layout.maximised, "{switch} restores the layout.");
        assert_eq!(
            layout, original,
            "{switch} keeps the previous panel choices."
        );
        assert!(has_control(&output, "New") && has_control(&output, "Mixer tab"));
        click_named(&ctx, &mut app, "Maximise the score");
        click_named(&ctx, &mut app, "Maximise the score");
        assert!(!app.session.layout().maximised, "The button toggles.");
    }
}

#[test]
fn the_sounds_switch_restores_a_maximised_layout_without_changing_its_panels() {
    for sounds in [false, true] {
        for module in [BottomModule::Mixer, BottomModule::Mastering] {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            app.boards.module = module;
            app.set_layout(|layout| layout.sounds = sounds);
            let original = app.session.layout();
            click_named(&ctx, &mut app, "Maximise the score");
            assert!(app.session.layout().maximised);
            click_named(&ctx, &mut app, "Sounds");
            assert_eq!(app.session.layout(), original);
            assert_eq!(app.boards.module, module);
        }
    }
}

#[test]
fn the_sounds_panel_opens_folds_and_hides_in_a_maximised_score() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        has_control(&output, "Show the Sounds panel"),
        "Folded by default."
    );
    assert_eq!(
        accessible_node(&output, "Sounds").1.toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    click_named(&ctx, &mut app, "Sounds");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(app.session.layout().sounds);
    assert!(has_control(&output, "Fold the Sounds panel"));
    assert!(!has_control(&output, "Show the Sounds panel"));
    for name in [
        "Find a sound",
        "Library filter",
        "Category filter",
        "Sort order",
        "Sort direction",
        "Show All sounds",
        "Show Loop sounds",
        "good grasp 2",
        "Sounds",
    ] {
        assert_eq!(named_count(&output, name), 1, "{name} has one home.");
    }
    // The fold button folds it, and the strip opens it again.
    click_named(&ctx, &mut app, "Fold the Sounds panel");
    assert!(!app.session.layout().sounds);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(has_control(&output, "Show the Sounds panel"));
    assert!(!has_control(&output, "Find a sound"));
    click_named(&ctx, &mut app, "Show the Sounds panel");
    assert!(app.session.layout().sounds);
    click_named(&ctx, &mut app, "Fold the Sounds panel");
    // A maximised score hides the panel and its strip; the switch restores them.
    click_named(&ctx, &mut app, "Sounds");
    click_named(&ctx, &mut app, "Maximise the score");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    for name in [
        "Fold the Sounds panel",
        "Show the Sounds panel",
        "Find a sound",
    ] {
        assert!(
            !has_control(&output, name),
            "{name} hides in a maximised score."
        );
    }
    assert_eq!(
        accessible_node(&output, "Sounds").1.toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    click_named(&ctx, &mut app, "Sounds");
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        !app.session.layout().maximised,
        "The Sounds switch restores the score."
    );
    assert!(has_control(&output, "Fold the Sounds panel"));
}

#[test]
fn starting_with_sounds_opens_the_sounds_panel_and_folds_the_lower_panel() {
    let (_, app) = fixture();
    let library = Arc::new(Library::fixture(app.session.library().samples.to_vec()));
    let session = Session::headless(library, app.session.project().clone()).unwrap();
    assert_eq!(
        session.options().starting_module,
        mixjam_techno::application::options::StartingModule::Sounds
    );
    let app = MixJamApp::from_session(session, "UI fixture".into(), false);
    let layout = app.session.layout();
    assert!(layout.sounds && !layout.boards, "{layout:?}");
    assert_eq!(
        app.boards.module,
        BottomModule::Mixer,
        "Mixer shows when it opens."
    );
    assert!(!app.mix.shown);
}

#[test]
fn starting_with_mastering_or_effects_opens_the_lower_panel_on_its_tab() {
    use mixjam_techno::application::options::StartingModule;
    let (_, app) = fixture();
    for (start, module) in [
        (StartingModule::Mastering, BottomModule::Mastering),
        (StartingModule::Effects, BottomModule::FxReturns),
    ] {
        let name = format!("mixjam-ui-start-{module:?}-{}", std::process::id());
        let root = std::env::temp_dir().join(name);
        let mut session = Session::headless(
            Arc::new(Library::fixture(app.session.library().samples.to_vec())),
            app.session.project().clone(),
        )
        .unwrap()
        .with_settings_root(root.join("settings"));
        // The last session folded the lower panel.
        let mut layout = session.layout();
        layout.boards = false;
        session.set_layout(layout).unwrap();
        let mut options = session.options().clone();
        options.starting_module = start;
        session
            .apply_options(options, session.options_revision(), false)
            .unwrap();
        let app = MixJamApp::from_session(session, "UI fixture".into(), false);
        assert_eq!(app.boards.module, module);
        assert!(
            app.session.layout().boards,
            "{module:?} must open the lower panel."
        );
        assert!(!app.mix.shown);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn starting_with_the_mixer_opens_the_mix_view() {
    let (_, app) = fixture();
    let root = std::env::temp_dir().join(format!("mixjam-ui-start-mix-{}", std::process::id()));
    let mut session = Session::headless(
        Arc::new(Library::fixture(app.session.library().samples.to_vec())),
        app.session.project().clone(),
    )
    .unwrap()
    .with_settings_root(root.join("settings"));
    let mut options = session.options().clone();
    options.starting_module = mixjam_techno::application::options::StartingModule::Mixer;
    session
        .apply_options(options, session.options_revision(), false)
        .unwrap();
    let app = MixJamApp::from_session(session, "UI fixture".into(), false);
    assert!(app.mix.shown);
    assert_eq!(app.boards.module, BottomModule::Mastering);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn small_windows_give_the_sounds_list_room_and_the_panel_opened_last_wins() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    for size in [Vec2::new(960.0, 720.0), Vec2::new(1024.0, 768.0)] {
        app.set_layout(|layout| {
            layout.sounds = false;
            layout.inspector = false;
            layout.boards = true;
        });
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        // Opening the Sounds panel folds the Mixer board to its toolbar
        // row, so the list keeps more than three rows.
        app.open_sounds_panel();
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        assert!(
            app.sounds_panel_open() && !app.panels.room.boards,
            "{size:?}"
        );
        assert!(app.session.layout().boards, "the layout keeps the board");
        let panel = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("sounds-panel"))
            .unwrap()
            .rect;
        assert!(
            panel.height() >= room::SOUNDS_MIN_HEIGHT,
            "{size:?} {panel:?}"
        );
        assert!(has_control(&output, "Mixer tab"));
        // A board tab takes the height back.
        click_named(&ctx, &mut app, "Mixer tab");
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        assert!(
            app.panels.room.boards && !app.sounds_panel_open(),
            "{size:?}"
        );
        // In a narrow window the Inspector starts folded, then takes the
        // width from the Sounds panel when it is opened.
        app.open_sounds_panel();
        app.set_layout(|layout| layout.inspector = true);
        app.panels.inspector_opened = false;
        app.panels.order = [
            room::Panel::Sounds,
            room::Panel::Boards,
            room::Panel::Inspector,
        ];
        app.resolve_room();
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        assert!(!app.panels.room.inspector, "{size:?}");
        app.open_inspector();
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        assert!(
            app.panels.room.inspector && !app.sounds_panel_open(),
            "{size:?}"
        );
        app.open_sounds_panel();
        frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        assert!(
            !app.panels.room.inspector && app.sounds_panel_open(),
            "{size:?}"
        );
    }
    // A large window shows them all.
    app.open_inspector();
    app.set_layout(|layout| layout.boards = true);
    let size = Vec2::new(1600.0, 1200.0);
    frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    assert!(app.panels.room.inspector && app.panels.room.sounds && app.panels.room.boards);
}

#[test]
fn the_inspector_switch_shows_and_folds_it() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    click_named(&ctx, &mut app, "Inspector");
    assert!(app.panels.room.inspector);
    click_named(&ctx, &mut app, "Inspector");
    assert!(!app.panels.room.inspector && !app.session.layout().inspector);
    app.set_layout(|layout| layout.maximised = true);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(!has_control(&output, "Show the Inspector"));
    click_named(&ctx, &mut app, "Inspector");
    assert!(!app.session.layout().maximised);
}
