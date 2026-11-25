//! Tool rail: grouping, order, names, faces, edge marks, toggles, icons and rail
//! state.

use super::*;

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
    // All themes paint the rail procedurally in code.
    assert!(
        !ctx.tex_manager()
            .read()
            .allocated()
            .any(|(_, meta)| meta.name.starts_with("folded-metal-")),
        "The rail must paint in code without legacy Folded Metal textures."
    );
    if roles().material == theme::Material::Metal {
        assert!(
            output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == egui::TextureId::default() => {
                    let bounds = mesh.calc_bounds();
                    bounds.width() > 50.0 && bounds.height() > 300.0
                }
                _ => false,
            }),
            "The Vintage rail must paint its procedural modular gradient."
        );
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
        let text = screen(&output)
            .texts()
            .find(|text| text.label() == name && target.contains(text.rect().min))
            .unwrap_or_else(|| panic!("{label} must print its rail name {name:?}."));
        for section in &text.shape().galley.job.sections {
            assert_eq!(
                section.format.font_id.size, 10.5,
                "{label}: every rail name uses one size."
            );
        }
        let bounds = text.shape().visual_bounding_rect();
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
            let title = screen(&output)
                .texts()
                .find(|text| text.label() == "FILE" && rail.contains(text.rect().min))
                .expect("The first heading must be painted.")
                .shape()
                .visual_bounding_rect();
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
        assert!(screen(&output).find_node(&title_text(window)).is_none());
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
        !screen(&output).texts().any(|text| {
            let label = text.label();
            text.rect().min.x < rail_width(false)
                && text.rect().min.y > 66.0
                && (RailIcon::ALL.iter().any(|icon| icon.label() == label)
                    || matches!(label, "MIX" | "Record"))
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
        screen(&hovered)
            .texts()
            .any(|text| text.label().starts_with("Mixer")),
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
        screen(&hovered_undo)
            .texts()
            .any(|text| text.label().starts_with("Undo")),
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
    assert!(screen(&output).node("Mixer").toggled());
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
