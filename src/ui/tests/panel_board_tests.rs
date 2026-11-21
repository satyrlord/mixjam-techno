//! The lower panel's boards from the rail: board buttons, panel switches, folding,
//! and names that stay unique across boards and views.

use super::*;

#[test]
fn the_rail_mastering_and_step_synth_buttons_show_their_board_and_fold_the_lower_panel() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let rail = |output: &egui::FullOutput, name: &str| {
        screen(output)
            .nodes()
            .find(|node| {
                node.role() == egui::accesskit::Role::Button && node.node().label() == Some(name)
            })
            .map(|node| (node.id(), node.node().toggled()))
            .unwrap_or_else(|| panic!("Missing rail button {name}"))
    };
    for (name, module) in [
        ("Mastering", BottomModule::Mastering),
        ("Step synth", BottomModule::StepSynth),
    ] {
        app.boards.module = BottomModule::Mixer;
        app.set_layout(|layout| layout.boards = true);
        let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(rail(&output, name).1, Some(egui::accesskit::Toggled::False));
        for (shown, boards) in [(true, true), (false, false), (true, true)] {
            let target = rail(&output, name).0;
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
            output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
            assert_eq!(app.boards.module, module, "{name} keeps its board chosen.");
            assert_eq!(app.session.layout().boards, boards, "{name}");
            assert_eq!(app.board_shown(module), shown, "{name}");
            assert_eq!(
                rail(&output, name).1,
                Some(if shown {
                    egui::accesskit::Toggled::True
                } else {
                    egui::accesskit::Toggled::False
                }),
                "The {name} highlight follows its board."
            );
        }
        // The lower panel's title names the board it shows.
        assert!(shows_text(&output, &view::title_text(module.label())));
    }
    assert!(
        !app.session.unsaved(),
        "Rail navigation must not edit the project."
    );
}

#[test]
fn panel_switches_show_names_fold_the_lower_panel_and_keep_every_tool() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let mut output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let selected = |output: &egui::FullOutput, name: &str| {
        accessible_node(output, name).1.toggled() == Some(egui::accesskit::Toggled::True)
    };
    assert!(selected(&output, "Tools"));
    assert!(selected(&output, "Boards"));
    assert_eq!(
        effects_control_rect(&output, "New").size(),
        rail_button_size(RailForm::Names)
    );
    // The status-bar switch and the rail's own switch both change the names.
    output = click_named(&ctx, &mut app, "Tools");
    assert!(!app.session.layout().tool_names);
    assert!(!selected(&output, "Tools"));
    assert_eq!(
        effects_control_rect(&output, "New").size(),
        rail_button_size(RailForm::Icons)
    );
    output = click_named(&ctx, &mut app, "Show the tool names");
    assert!(app.session.layout().tool_names);
    accessible_node(&output, "Shrink the tool rail to icons");
    // Folded, the lower panel leaves nothing behind and the rail keeps
    // every tool; the Mixer rail button opens it again.
    assert!(shows_text(&output, "MIXER"));
    output = click_named(&ctx, &mut app, "Boards");
    assert!(!app.session.layout().boards);
    assert!(!selected(&output, "Boards"));
    assert!(
        !shows_text(&output, "MIXER"),
        "Folded, the lower panel's title row goes with it."
    );
    for icon in RailIcon::ALL {
        if !matches!(icon, RailIcon::Undo | RailIcon::Redo | RailIcon::Help) {
            accessible_node(&output, icon.label());
        }
    }
    click_named(&ctx, &mut app, "Mixer");
    assert!(app.session.layout().boards);
    assert_eq!(app.boards.module, BottomModule::Mixer);
}

#[test]
fn folded_boards_reopen_from_their_rail_button_and_a_second_press_folds_them() {
    each_theme(|_| {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        for (name, module) in [
            ("Mastering", BottomModule::Mastering),
            ("FX returns", BottomModule::FxReturns),
            ("Step synth", BottomModule::StepSynth),
        ] {
            click_named(&ctx, &mut app, name);
            assert_eq!(app.boards.module, module);
            click_named(&ctx, &mut app, "Boards");
            assert!(!app.session.layout().boards);
            click_named(&ctx, &mut app, name);
            assert!(app.session.layout().boards, "Reopen the folded {name}.");
            assert_eq!(app.boards.module, module, "Restore the folded {name}.");
            click_named(&ctx, &mut app, name);
            // A second press folds the lower panel (R6, R7).
            assert!(!app.session.layout().boards, "Fold {name} again.");
            assert_eq!(app.boards.module, module);
            click_named(&ctx, &mut app, "Boards");
        }
    });
}

#[test]
fn rail_board_buttons_keep_the_panel_and_sound_state_across_input_methods() {
    fn action(action: egui::accesskit::Action, target: egui::accesskit::NodeId) -> egui::Event {
        egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action,
            target,
            data: None,
        })
    }
    fn pointer_events(pos: Pos2, pressed: bool) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                pressed,
                button: egui::PointerButton::Primary,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }
    for size in [
        Vec2::new(1440.0, 1000.0),
        Vec2::new(1024.0, 768.0),
        Vec2::new(960.0, 720.0),
    ] {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        // Start from the tallest board: a shorter one keeps the panel,
        // while a taller one grows it to show the whole board.
        app.boards.module = BottomModule::StepSynth;
        app.sounds.search = "grasp".into();
        app.sounds.group = Some("Loop".into());
        app.sounds.sort = PaletteSort::Name;
        app.selection.sample = Some(1);
        for _ in 0..12 {
            frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        }
        let mut output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        // The rail's board buttons, found by their role: board controls
        // can share a name.
        let rail_button = |output: &egui::FullOutput, module: BottomModule| {
            screen(output)
                .nodes()
                .find(|node| {
                    node.role() == egui::accesskit::Role::Button
                        && node.node().label() == Some(module.label())
                })
                .map(|node| (node.id(), node.rect()))
                .unwrap_or_else(|| panic!("Missing rail button {module:?}"))
        };
        let panel = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("sounds"))
            .unwrap()
            .rect;
        assert!(panel.height() >= 370.0);
        accessible_node(&output, "Step synth BPM");

        // Choose Mixer with the pointer, Mastering with focus and Enter,
        // then the others with the accessibility actions used by agents.
        for (method, module) in [
            (0, BottomModule::Mixer),
            (1, BottomModule::Mastering),
            (2, BottomModule::FxReturns),
            (0, BottomModule::StepSynth),
            (2, BottomModule::Mixer),
        ] {
            let (tab_id, tab) = rail_button(&output, module);
            match method {
                0 => {
                    frame_at_size(
                        &ctx,
                        &mut app,
                        pointer_events(tab.center(), true),
                        egui::Modifiers::NONE,
                        size,
                    );
                    frame_at_size(
                        &ctx,
                        &mut app,
                        pointer_events(tab.center(), false),
                        egui::Modifiers::NONE,
                        size,
                    );
                }
                1 => {
                    frame_at_size(
                        &ctx,
                        &mut app,
                        vec![action(egui::accesskit::Action::Focus, tab_id)],
                        egui::Modifiers::NONE,
                        size,
                    );
                    frame_at_size(
                        &ctx,
                        &mut app,
                        vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
                        egui::Modifiers::NONE,
                        size,
                    );
                }
                _ => {
                    frame_at_size(
                        &ctx,
                        &mut app,
                        vec![action(egui::accesskit::Action::Click, tab_id)],
                        egui::Modifiers::NONE,
                        size,
                    );
                }
            }
            for _ in 0..6 {
                output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            assert_eq!(
                app.boards.module, module,
                "Method {method}, viewport {size:?}."
            );
            let title = view::title_text(module.label());
            let painted = screen(&output).texts().any(|text| {
                text.label() == title
                    && !text.shape().galley.elided
                    && panel.contains(text.rect().min)
            });
            assert!(painted, "The lower panel's title must name {title} whole.");
            assert_eq!(
                egui::containers::panel::PanelState::load(&ctx, egui::Id::new("sounds"))
                    .unwrap()
                    .rect,
                panel,
                "Choosing a board must preserve the existing panel size."
            );
            assert_eq!(app.sounds.search, "grasp");
            assert_eq!(app.sounds.group.as_deref(), Some("Loop"));
            assert_eq!(app.sounds.product, None);
            assert_eq!(app.sounds.sort, PaletteSort::Name);
            assert_eq!(app.selection.sample, Some(1));
            assert!(
                screen(&output).find_node("Show Mixer").is_none()
                    && screen(&output).find_node("Bottom module").is_none(),
                "The rail replaces the former module selector and swap buttons."
            );
            let shown = |name: &str| screen(&output).find_node(name).is_some();
            let expected = match module {
                BottomModule::Mixer => "Master gain",
                BottomModule::Mastering => "Trim",
                BottomModule::FxReturns => "FX returns board",
                BottomModule::StepSynth => "Step synth BPM",
            };
            assert!(shown(expected), "{expected} must show with {module:?}.");
        }
        assert!(
            !app.session.unsaved(),
            "Module navigation must not edit the project."
        );
    }
}

#[test]
fn no_two_controls_share_a_name_with_any_board_or_view() {
    let size = Vec2::new(1600.0, 1300.0);
    let (ctx, mut app, _) = mix_view_fixture(size);
    // The names that more than one control carries on screen.
    let repeated = |app: &mut MixJamApp| {
        frame_at_size(&ctx, app, vec![], egui::Modifiers::NONE, size);
        let output = frame_at_size(&ctx, app, vec![], egui::Modifiers::NONE, size);
        let nodes = agent::nodes(output.platform_output.accesskit_update.as_ref().unwrap());
        let mut seen = BTreeSet::new();
        nodes
            .into_iter()
            .filter(|node| {
                !node.label.is_empty() && !matches!(node.role, "label" | "window" | "other")
            })
            .filter(|node| !seen.insert(node.label.clone()))
            .map(|node| node.label)
            .collect::<BTreeSet<_>>()
    };
    for module in [
        BottomModule::Mastering,
        BottomModule::FxReturns,
        BottomModule::StepSynth,
    ] {
        app.boards.module = module;
        app.set_layout(|layout| layout.boards = true);
        app.mix.shown = true;
        app.resolve_room();
        let with_mix_view = repeated(&mut app);
        assert!(app.mix.shown && app.board_shown(module), "{module:?}");
        app.mix.shown = false;
        let without = repeated(&mut app);
        assert!(app.board_shown(module), "{module:?}");
        assert!(
            with_mix_view.is_empty() && without.is_empty(),
            "{module:?}: repeated names {with_mix_view:?} with the Mix view, {without:?} without"
        );
    }
}
