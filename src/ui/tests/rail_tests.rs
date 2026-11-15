//! Tool rail and the lower panel's boards: switches, toggles, icons, grouping
//! and rail state.

use super::*;

#[test]
fn the_rail_mastering_and_step_synth_buttons_show_their_board_and_fold_the_lower_panel() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    let rail = |output: &egui::FullOutput, name: &str| {
        output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, node)| {
                node.role() == egui::accesskit::Role::Button && node.label() == Some(name)
            })
            .map(|(id, node)| (*id, node.toggled()))
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
            output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .find(|(_, node)| {
                    node.role() == egui::accesskit::Role::Button
                        && node.label() == Some(module.label())
                })
                .map(|(id, node)| {
                    let bounds = node.bounds().unwrap();
                    (
                        *id,
                        Rect::from_min_max(
                            Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
                            Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
                        ),
                    )
                })
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
            let painted = output.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text)
                    if text.galley.job.text == title
                        && !text.galley.elided
                        && panel.contains(text.pos))
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
            let nodes = &output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes;
            assert!(
                !nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some("Show Mixer")
                        || node.label() == Some("Bottom module")),
                "The rail replaces the former module selector and swap buttons."
            );
            let shown = |name: &str| nodes.iter().any(|(_, node)| node.label() == Some(name));
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
fn rail_groups_remain_ordered_separated_and_reachable_in_both_states() {
    for _ in themes_in_turn() {
        for names in [true, false] {
            for size in [
                Vec2::new(960.0, 720.0),
                Vec2::new(1024.0, 768.0),
                Vec2::new(1440.0, 1000.0),
            ] {
                check_rail_state(names, size);
            }
        }
    }
}

/// The rail's order, targets, artwork, groups and reach in one layout.
fn check_rail_state(names: bool, size: Vec2) {
    let actions = [
        RailIcon::New,
        RailIcon::Open,
        RailIcon::Save,
        RailIcon::SaveAs,
        RailIcon::Import,
        RailIcon::Export,
        RailIcon::Erase,
        RailIcon::Delete,
        RailIcon::Mixer,
        RailIcon::Master,
        RailIcon::FxReturns,
        RailIcon::StepSynth,
        RailIcon::Edit,
        RailIcon::Stretch,
        RailIcon::Effects,
        RailIcon::AlbumReview,
    ];
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.set_layout(|layout| layout.tool_names = names);
    let viewport = Rect::from_min_size(Pos2::ZERO, size);
    // The rail runs from the 64-point top bar to the 30-point status bar.
    let rail = Rect::from_min_max(
        Pos2::new(0.0, 64.0),
        Pos2::new(rail_width(names), size.y - 30.0),
    );
    // Names keep their headings only where every tool fits under them; a
    // shorter window tightens the targets and turns headings into separators.
    let form = if !names {
        RailForm::Icons
    } else if size.y >= 1000.0 {
        RailForm::Names
    } else {
        RailForm::Compact
    };
    let target = rail_button_size(form);
    let state = format!("{form:?}, {size:?}");
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let icons = app.rail_icons.as_ref().unwrap().as_ref().unwrap().clone();
    let bounds: Vec<_> = actions
        .iter()
        .map(|icon| {
            let rect = effects_control_rect(&output, icon.label());
            assert!(
                (rect.size() - target).abs().max_elem() < 0.01,
                "{} must keep the common {target:?} target ({state}).",
                icon.label()
            );
            assert!(
                (rect.center().x - rail_width(names) / 2.0).abs() < 0.6,
                "{} must be centered in the rail ({state}).",
                icon.label()
            );
            let (clip, image) = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == icons.texture_id(*icon) => {
                        Some((shape.clip_rect, mesh.calc_bounds()))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing rail artwork: {}", icon.label()));
            assert!(
                rect.contains_rect(image),
                "{} artwork must stay inside its target ({state}).",
                icon.label()
            );
            // In view: inside the rail's 6-point margins.
            if rail.shrink(6.0).contains_rect(rect) {
                assert!(
                    clip.contains_rect(image),
                    "{} artwork must be fully visible ({state}).",
                    icon.label()
                );
            }
            assert!(
                (image.width() - 20.0).abs() < 0.01 && (image.height() - 20.0).abs() < 0.01,
                "{} artwork must use the prepared size without stretching.",
                icon.label()
            );
            rect
        })
        .collect();
    // Tools may touch: their faces, not gaps, separate them.
    for pair in bounds.windows(2) {
        assert!(
            pair[0].bottom() <= pair[1].top() + 0.01,
            "File, clip and tool actions keep their order without overlap ({state})."
        );
    }
    // Each group starts under a heading with full names, and under a
    // separator otherwise.
    for (before, after, heading) in [
        (5, 6, "Clips"),
        (7, 8, "Mix"),
        (11, 12, "Sound"),
        (14, 15, "Library"),
    ] {
        let gap = Rect::from_min_max(
            Pos2::new(bounds[before].left(), bounds[before].bottom()),
            Pos2::new(bounds[after].right(), bounds[after].top()),
        );
        if form.headings() {
            let title = accessible_label(&output, heading);
            assert!(
                gap.contains_rect(title),
                "The {heading} heading must sit between its groups ({state})."
            );
            continue;
        }
        let has_separator = output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::LineSegment { points, stroke } => {
                stroke.width > 0.0
                    && (points[0].y - points[1].y).abs() < 0.01
                    && (points[1].x - points[0].x).abs() >= 32.0
                    && gap.contains(points[0])
                    && gap.contains(points[1])
            }
            _ => false,
        });
        assert!(
            has_separator,
            "Group boundaries need a visible separator ({state})."
        );
    }
    // The microphone and its track selector moved to the top bar.
    for label in [RailIcon::Record.label(), "Recording track"] {
        let rect = effects_control_rect(&output, label);
        assert!(
            rect.bottom() <= 64.0 && !rail.intersects(rect),
            "{label} sits in the top bar, not the rail ({state}): {rect:?}."
        );
    }
    // Options and All commands lead the 30-point status bar, under the rail.
    for label in ["Options", "All commands"] {
        let rect = effects_control_rect(&output, label);
        assert!(
            rect.top() >= size.y - 30.0 && rect.left() >= 18.0 && rect.right() <= 18.0 + 64.0,
            "{label} leads the status bar ({state}): {rect:?}."
        );
    }
    let fits = |rect: Rect| rail.shrink(6.0).contains_rect(rect);
    // Icons alone, and the compact form, fit every tool from a 768-point
    // window up.
    if !names || size.y >= 768.0 {
        for rect in &bounds {
            assert!(
                viewport.contains_rect(*rect) && fits(*rect),
                "Every tool must be in view without scrolling ({state}): {rect:?}."
            );
        }
    } else {
        // Too short for every name: the wheel brings the rest
        // of the rail into view.
        for _ in 0..3 {
            frame_at_size(
                &ctx,
                &mut app,
                vec![
                    egui::Event::PointerMoved(Pos2::new(rail_width(names) / 2.0, rail.center().y)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(0.0, -400.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                egui::Modifiers::NONE,
                size,
            );
        }
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let label = RailIcon::AlbumReview.label();
        let rect = effects_control_rect(&output, label);
        assert!(
            fits(rect),
            "{label} must scroll into view ({state}): {rect:?}."
        );
    }
    // Studio and Aero paint the rail in code; Vintage crops its artwork.
    if roles().material == theme::Material::Metal {
        for (part, expected) in [
            ("grain", [256, 256]),
            ("part-6", [12, 96]),
            ("part-7", [12, 96]),
        ] {
            let name = format!("folded-metal-Utility-{part}");
            let (texture, pixels) = ctx
                .tex_manager()
                .read()
                .allocated()
                .find(|(_, meta)| meta.name == name)
                .map(|(id, meta)| (*id, meta.size))
                .expect("The rail must load its shared material finish.");
            assert_eq!(
                pixels, expected,
                "The rail must load the prepared material."
            );
            let mut painted = false;
            for shape in &output.shapes {
                let egui::Shape::Mesh(mesh) = &shape.shape else {
                    continue;
                };
                if mesh.texture_id != texture {
                    continue;
                }
                painted = true;
                for vertex in &mesh.vertices {
                    for coordinate in [vertex.pos.x, vertex.pos.y] {
                        assert!(
                            (coordinate - coordinate.round()).abs() < 0.001,
                            "Rail material must align to physical pixels."
                        );
                    }
                }
                for triangle in mesh.indices.as_chunks::<3>().0 {
                    for edge in 0..3 {
                        let a = mesh.vertices[triangle[edge] as usize];
                        let b = mesh.vertices[triangle[(edge + 1) % 3] as usize];
                        let destination = (b.pos - a.pos).abs();
                        let source =
                            (b.uv - a.uv).abs() * Vec2::new(pixels[0] as f32, pixels[1] as f32);
                        assert!(
                            (destination - source).abs().max_elem() < 0.001,
                            "The narrow rail must crop its material without stretching texels."
                        );
                    }
                }
            }
            assert!(painted, "The rail must paint its {part} material.");
        }
    }
}

#[test]
fn rail_names_share_one_size_and_fit_their_targets() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.set_layout(|layout| layout.tool_names = true);
    let size = Vec2::new(1440.0, 1000.0);
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
    }
    let tools: Vec<(&str, &str)> = commands::RAIL
        .iter()
        .flat_map(|(_, tools)| tools.iter().filter_map(|command| command.icon()))
        .map(|icon| (icon.label(), icon.rail_name()))
        .collect();
    for (label, name) in tools {
        let target = effects_control_rect(&output, label);
        let text = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.job.text == name && target.contains(text.pos) =>
                {
                    Some(text)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{label} must print its rail name {name:?}."));
        for section in &text.galley.job.sections {
            assert_eq!(
                section.format.font_id.size, 10.5,
                "{label}: every rail name uses one size."
            );
        }
        let bounds = text.visual_bounding_rect();
        assert!(
            bounds.left() >= target.left() + 4.0 && bounds.right() <= target.right() - 4.0,
            "{label}: the name {name:?} must clear its target's edges: {bounds:?} in {target:?}."
        );
    }
}

#[test]
fn the_names_switch_stays_in_the_rail_clear_of_the_first_title_and_tool() {
    each_theme(|_| {
        for names in [true, false] {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            app.set_layout(|layout| layout.tool_names = names);
            let size = Vec2::new(1440.0, 1000.0);
            let mut output = egui::FullOutput::default();
            for _ in 0..3 {
                output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            let rail = Rect::from_min_max(
                Pos2::new(0.0, 64.0),
                Pos2::new(rail_width(names), size.y - 30.0),
            );
            let switch = effects_control_rect(
                &output,
                if names {
                    "Shrink the tool rail to icons"
                } else {
                    "Show the tool names"
                },
            );
            let new = effects_control_rect(&output, "New");
            assert!(
                rail.contains_rect(switch) && switch.bottom() <= new.top() + 0.01,
                "The switch sits in the rail, above the first tool (names {names}): \
                 {switch:?} over {new:?}."
            );
            if !names {
                continue;
            }
            // With names, it shares the first heading's row, at the right.
            let title = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if text.galley.job.text == "FILE" && rail.contains(text.pos) =>
                    {
                        Some(text.visual_bounding_rect())
                    }
                    _ => None,
                })
                .expect("The first heading must be painted.");
            assert!(
                (switch.right() - new.right()).abs() < 0.01
                    && switch.left() > title.right()
                    && switch.y_range().contains(title.center().y),
                "The switch ends the File heading's row, clear of its title: \
                 {switch:?} beside {title:?}."
            );
        }
    });
}

#[test]
fn rail_tool_faces_and_active_marks_follow_the_themes_rail_keys_role() {
    each_theme(|_| {
        let (ctx, mut app) = fixture();
        ctx.enable_accesskit();
        click_named(&ctx, &mut app, "Erase");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(
            accessible_node(&output, "Erase").1.toggled(),
            Some(egui::accesskit::Toggled::True)
        );
        // The rectangles painted inside one tool's target.
        let painted = |target: Rect| -> Vec<&egui::epaint::RectShape> {
            output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if target.expand(0.01).contains_rect(rect.rect) => {
                        Some(rect)
                    }
                    _ => None,
                })
                .collect()
        };
        let keys = roles().rail_keys;
        let idle = effects_control_rect(&output, "New");
        assert_eq!(
            !painted(idle).is_empty(),
            keys,
            "An idle tool rests on a face only where the theme gives the rail keys."
        );
        let active = effects_control_rect(&output, "Erase");
        let marks = painted(active);
        let outline = marks
            .iter()
            .any(|mark| mark.fill == Color32::TRANSPARENT && mark.stroke.color == roles().accent);
        // Bare, the bar sits on the target's edge; on a key, inside its border.
        let inset = if keys { 2.0 } else { 0.0 };
        let bar = marks.iter().any(|mark| {
            mark.fill == roles().accent
                && (mark.rect.width() - 2.0).abs() < 0.01
                && (mark.rect.left() - active.left() - inset).abs() < 0.01
        });
        assert!(
            bar && !outline,
            "The active tool carries the accent bar in every theme; an accent \
             outline means focus alone."
        );
        assert!(
            marks.len() > 1,
            "The active tool keeps its face under the mark."
        );
    });
}

#[test]
fn a_short_rail_marks_each_edge_that_hides_tools() {
    each_theme(|_| {
        // The engraved edge: a dark line across the scrolling view, 6 points
        // in from each side. With names on, the rail has no other such line.
        let width = rail_width(true);
        let edges = |output: &egui::FullOutput| -> Vec<f32> {
            output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::LineSegment { points, stroke }
                        if stroke.color == roles().ink
                            && (points[0].y - points[1].y).abs() < 0.01
                            && (points[0].x - 12.0).abs() < 0.01
                            && (points[1].x - (width - 12.0)).abs() < 0.01 =>
                    {
                        Some(points[0].y)
                    }
                    _ => None,
                })
                .collect()
        };
        // Where the scrolling view stops drawing its tools: the widest reach
        // of the clip rectangles inside the rail panel's margins.
        let drawn = |output: &egui::FullOutput, foot: f32| -> (f32, f32) {
            output
                .shapes
                .iter()
                .map(|shape| shape.clip_rect)
                .filter(|clip| clip.right() <= width + 0.01 && clip.bottom() < foot)
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(top, bottom), clip| {
                    (top.min(clip.top()), bottom.max(clip.bottom()))
                })
        };
        let draw = |size: Vec2| {
            let (ctx, mut app) = fixture();
            ctx.enable_accesskit();
            app.set_layout(|layout| layout.tool_names = true);
            let mut output = egui::FullOutput::default();
            for _ in 0..3 {
                output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
            }
            (ctx, app, output)
        };
        let (_, _, output) = draw(Vec2::new(1440.0, 1000.0));
        assert_eq!(
            edges(&output),
            Vec::<f32>::new(),
            "A rail that shows every tool marks no edge."
        );

        let size = Vec2::new(960.0, 720.0);
        let (ctx, mut app, output) = draw(size);
        // The rail ends at the 30-point status bar.
        let foot = size.y - 30.0;
        let hidden = effects_control_rect(&output, RailIcon::AlbumReview.label());
        let marks = edges(&output);
        assert!(
            marks.len() == 1
                && marks[0] < foot
                && marks[0] > foot - 16.0
                && hidden.bottom() > marks[0],
            "Tools below the view need the lower edge only: {marks:?} above {foot}."
        );
        let (_, bottom) = drawn(&output, foot - 0.5);
        assert!(
            (0.0..=2.0).contains(&(bottom - marks[0])),
            "The lower edge ends the tools, not crosses them: {} against {bottom}.",
            marks[0]
        );
        for _ in 0..3 {
            frame_at_size(
                &ctx,
                &mut app,
                vec![
                    egui::Event::PointerMoved(Pos2::new(width / 2.0, size.y / 2.0)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(0.0, -400.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                egui::Modifiers::NONE,
                size,
            );
        }
        let output = frame_at_size(&ctx, &mut app, vec![], egui::Modifiers::NONE, size);
        let first = effects_control_rect(&output, "New");
        let marks = edges(&output);
        assert!(
            marks.len() == 1 && marks[0] < 64.0 + 8.0 && first.top() < marks[0],
            "Scrolled to the end, tools above the view need the upper edge only: {marks:?}."
        );
        let (top, _) = drawn(&output, foot - 0.5);
        assert!(
            (0.0..=2.0).contains(&(marks[0] - top)),
            "The upper edge ends the tools, not crosses them: {} against {top}.",
            marks[0]
        );
    });
}

#[test]
fn rail_toggles_close_windows_and_keep_highlights_in_sync() {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.selection.sample = Some(1);
    let project = app.session.project().clone();
    for (icon, window) in [
        (RailIcon::Effects, "Effect Studio"),
        (RailIcon::Edit, "Sample editor"),
        (RailIcon::Stretch, "Time Stretcher"),
        (RailIcon::AlbumReview, "Album Review"),
    ] {
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let node = accessible_node(&output, icon.label()).1;
        assert_eq!(node.toggled(), Some(egui::accesskit::Toggled::False));
        let bounds = node.bounds().unwrap();
        let center = Pos2::new(
            (bounds.x0 + bounds.x1) as f32 / 2.0,
            (bounds.y0 + bounds.y1) as f32 / 2.0,
        );
        let icons = app.rail_icons.as_ref().unwrap().as_ref().unwrap().clone();
        let assert_active = |output: &egui::FullOutput, active: bool| {
            let node = accessible_node(output, icon.label()).1;
            assert_eq!(
                node.toggled(),
                Some(if active {
                    egui::accesskit::Toggled::True
                } else {
                    egui::accesskit::Toggled::False
                }),
                "{}",
                icon.label()
            );
            let tint = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == icons.texture_id(icon) => {
                        Some(mesh.vertices[0].color)
                    }
                    _ => None,
                })
                .expect("The rail icon must be painted.");
            // The rail marks its active tool in the workspace accent.
            assert_eq!(
                tint,
                if active {
                    roles().accent
                } else {
                    roles().legend
                }
            );
        };
        click(&ctx, &mut app, center, egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_active(&output, true);
        accessible_node(&output, &title_text(window));
        let collapse = effects_control_rect(&output, &format!("Collapse {window}"));
        click(&ctx, &mut app, collapse.center(), egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(dialog::is_collapsed(&ctx, window));
        assert_active(&output, true);
        click(&ctx, &mut app, center, egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_active(&output, false);
        assert!(
            !output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.label() == Some(title_text(window).as_str()))
        );
        // Closing through the header must also clear the rail state.
        click(&ctx, &mut app, center, egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let close = effects_control_rect(&output, &format!("Close {window}"));
        click(&ctx, &mut app, close.center(), egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_active(&output, false);
    }
    assert_eq!(
        app.session.project(),
        &project,
        "Window toggles must not edit the song."
    );
}

#[test]
fn rail_icons_keep_tooltips_accessible_actions_and_cached_textures() {
    let (ctx, mut app) = fixture();
    app.set_layout(|layout| layout.tool_names = false);
    frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let icons = app.rail_icons.as_ref().unwrap().as_ref().unwrap().clone();
    // The rail paints icons as textured meshes; header buttons paint
    // egui images as textured rectangles. Both carry the texture ID.
    let image_center = |output: &egui::FullOutput, icon: RailIcon| {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == icons.texture_id(icon) => {
                    Some(mesh.calc_bounds().center())
                }
                egui::Shape::Rect(rect)
                    if rect.brush.as_ref().is_some_and(|brush| {
                        brush.fill_texture_id == icons.texture_id(icon) && brush.uv != Rect::ZERO
                    }) =>
                {
                    Some(rect.rect.center())
                }
                _ => None,
            })
            .expect("The view must paint its bundled icon texture.")
    };
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        !output.shapes.iter().any(|shape| {
            if let egui::Shape::Text(text) = &shape.shape {
                let label = text.galley.job.text.as_str();
                text.pos.x < rail_width(false)
                    && text.pos.y > 66.0
                    && (RailIcon::ALL.iter().any(|icon| icon.label() == label)
                        || matches!(label, "MIX" | "Record"))
            } else {
                false
            }
        }),
        "Rail action names must not be painted beside the icons."
    );
    let mixer = image_center(&output, RailIcon::Mixer);
    ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(mixer)],
        egui::Modifiers::NONE,
    );
    let hovered = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        hovered.shapes.iter().any(|shape| {
            matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.job.text.starts_with("Mixer")
            )
        }),
        "The icon must preserve its action name in a hover tooltip."
    );
    ctx.enable_accesskit();
    let undo = image_center(&hovered, RailIcon::Undo);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(undo)],
        egui::Modifiers::NONE,
    );
    // Tooltips wait until the pointer rests after leaving the rail icon.
    for _ in 0..12 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    let hovered_undo = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert!(
        hovered_undo.shapes.iter().any(|shape| {
            matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.job.text.starts_with("Undo")
            )
        }),
        "The header icon must preserve its action name in a hover tooltip."
    );
    for label in ["Undo", "Redo", "Help"] {
        let (_, node) = accessible_node(&hovered_undo, label);
        assert_eq!(node.role(), egui::accesskit::Role::Button, "{label}");
    }
    // From the Mix view, the rail's Mixer shows the Mixer board instead.
    app.set_mix_view(true);
    pointer(&ctx, &mut app, mixer, true, egui::Modifiers::NONE);
    let clicked = frame(
        &ctx,
        &mut app,
        vec![egui::Event::PointerButton {
            pos: mixer,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        egui::Modifiers::NONE,
    );
    assert!(
        !app.mix.shown && app.board_shown(BottomModule::Mixer),
        "The rail's Mixer leaves the Mix view and shows the Mixer board."
    );
    assert!(
        clicked.platform_output.events.iter().any(|event| {
            matches!(
                event,
                egui::output::OutputEvent::Clicked(info)
                    if info.label.as_deref() == Some("Mixer")
            )
        }),
        "The icon must expose its action name to accessibility output."
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        accessible_node(&output, "Mixer").1.toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    click(&ctx, &mut app, mixer, egui::Modifiers::NONE);
    assert!(
        !app.mix.shown && !app.session.layout().boards,
        "A repeated click folds the lower panel."
    );
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    assert_eq!(
        accessible_node(&output, "Mixer").1.toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    for icon in [
        RailIcon::New,
        RailIcon::Open,
        RailIcon::Save,
        RailIcon::SaveAs,
        RailIcon::Import,
        RailIcon::Export,
        RailIcon::Delete,
    ] {
        let node = accessible_node(&output, icon.label()).1;
        assert_eq!(node.role(), egui::accesskit::Role::Button);
        assert_eq!(
            node.toggled(),
            None,
            "{} must remain an action.",
            icon.label()
        );
    }
    let erase = image_center(&output, RailIcon::Erase);
    click(&ctx, &mut app, erase, egui::Modifiers::NONE);
    assert!(app.score_tools.erase);
    app.selection.clips = [1, 2].into_iter().collect();
    app.selection.clip = Some(1);
    let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    let delete = image_center(&output, RailIcon::Delete);
    click(&ctx, &mut app, delete, egui::Modifiers::NONE);
    assert!(app.session.project().clips.is_empty());
    app.history_step(false);
    assert_eq!(app.session.project().clips.len(), 2);
    let cached = app.rail_icons.as_ref().unwrap().as_ref().unwrap();
    assert!(
        Arc::ptr_eq(&icons, cached),
        "Rail textures must be reused across frames and actions."
    );
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
