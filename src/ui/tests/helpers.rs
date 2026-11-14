//! Drivers shared by the themed UI test modules: the app fixture, frame runners, pointer
//! and keyboard events, and accessible-node lookups.

use super::*;

pub(super) fn fixture() -> (egui::Context, MixJamApp) {
    let context = egui::Context::default();
    configure_style(&context);
    let sample = Sample {
        id: 1,
        code: "FIXTURE".into(),
        name: "good grasp 2".into(),
        bars: 1,
        group_name: "Loop".into(),
        offset: 0,
        length: 0,
        frame_count: 75600,
        sample_rate: 44100,
        channels: 1,
        source_bpm: 140.0,
        wav_path: None,
        digest: None,
    };
    let library = Arc::new(Library::fixture(vec![sample]));
    let mut editor = Editor::new(Project::default());
    editor.place(1, 0, 0, TICKS_PER_BAR).unwrap();
    editor.place(1, 1, TICKS_PER_BAR, TICKS_PER_BAR).unwrap();
    let mut app = MixJamApp::from_session(
        Session::headless(library, editor.into_project()).unwrap(),
        "UI fixture".into(),
        false,
    );
    app.capture.path = None;
    // The fixture paints in the thread's theme; see `each_theme`.
    app.session.set_theme_fixture(theme::current());
    // A default launch folds the lower panel and opens the Sounds panel
    // (Starting module Sounds); the fixture keeps the stored defaults,
    // as most tests use a board.
    app.set_layout(|layout| {
        layout.boards = true;
        layout.sounds = false;
        layout.inspector = false;
    });
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    frame(&context, &mut app, vec![], egui::Modifiers::NONE);
    (context, app)
}

/// Run `body` once in each theme, then return the thread to Studio. The
/// body's fixtures carry the theme into their options, so their frames
/// keep painting it.
pub(super) fn each_theme(mut body: impl FnMut(theme::Theme)) {
    for theme in theme::Theme::ALL {
        theme::paint_as(theme);
        body(theme);
        assert_eq!(
            theme::current(),
            theme,
            "the body's frames changed the theme"
        );
    }
    theme::paint_as(theme::Theme::Studio);
}

/// The 100% display density once per theme. Taking one sets its theme
/// on this test's thread, and the loop body's fixture then adopts it.
/// Each new item, and the end of the loop, first checks that the previous
/// body's frames kept its theme, as `each_theme` does. The end returns the
/// thread to Studio.
pub(super) fn themed_densities() -> impl Iterator<Item = f32> {
    let previous = std::rc::Rc::new(std::cell::Cell::new(None));
    let check = {
        let previous = previous.clone();
        move || {
            if let Some(earlier) = previous.get() {
                assert_eq!(
                    theme::current(),
                    earlier,
                    "the body's frames changed the theme"
                );
            }
        }
    };
    let end = check.clone();
    theme::Theme::ALL
        .into_iter()
        .map(|theme| (theme, 1.0))
        .map(move |(theme, density)| {
            check();
            previous.set(Some(theme));
            theme::paint_as(theme);
            density
        })
        .chain(std::iter::from_fn(move || {
            end();
            theme::paint_as(theme::Theme::Studio);
            None
        }))
}

pub(super) fn effects_control_rect(output: &egui::FullOutput, name: &str) -> Rect {
    let bounds = accessible_node(output, name).1.bounds().unwrap();
    Rect::from_min_max(
        Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
        Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
    )
}

/// Click a control through accessibility, as the agent channel does.
pub(super) fn click_named(
    ctx: &egui::Context,
    app: &mut MixJamApp,
    name: &str,
) -> egui::FullOutput {
    let output = frame(ctx, app, vec![], egui::Modifiers::NONE);
    // A board and its toolbar row carry the board's name; click the control.
    let target = output
        .platform_output
        .accesskit_update
        .as_ref()
        .and_then(|update| {
            update.nodes.iter().find(|(_, node)| {
                node.label() == Some(name)
                    && !matches!(
                        node.role(),
                        egui::accesskit::Role::Toolbar | egui::accesskit::Role::TabPanel
                    )
            })
        })
        .map_or_else(|| accessible_node(&output, name).0, |(id, _)| *id);
    frame(
        ctx,
        app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target,
                data: None,
            },
        )],
        egui::Modifiers::NONE,
    );
    frame(ctx, app, vec![], egui::Modifiers::NONE)
}

/// The screen x of a point `x` from the lane header's left edge, given
/// the visible lanes.
pub(super) fn header_x(context: &egui::Context, visible: Rect, x: f32) -> f32 {
    let spacing = context.style().spacing.item_spacing.x;
    visible.left() - lanes::LANE_HEADER_WIDTH - spacing + x
}

pub(super) fn frame(
    context: &egui::Context,
    app: &mut MixJamApp,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) -> egui::FullOutput {
    frame_at_size(context, app, events, modifiers, Vec2::new(1440.0, 1000.0))
}

pub(super) fn frame_at_size(
    context: &egui::Context,
    app: &mut MixJamApp,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
    size: Vec2,
) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        events,
        modifiers,
        focused: true,
        ..Default::default()
    };
    context.run(input, |ctx| app.draw(ctx))
}

pub(super) fn pointer(
    context: &egui::Context,
    app: &mut MixJamApp,
    pos: Pos2,
    pressed: bool,
    modifiers: egui::Modifiers,
) {
    frame(
        context,
        app,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers,
            },
        ],
        modifiers,
    );
}

pub(super) fn click(
    context: &egui::Context,
    app: &mut MixJamApp,
    pos: Pos2,
    modifiers: egui::Modifiers,
) {
    pointer(context, app, pos, true, modifiers);
    pointer(context, app, pos, false, modifiers);
}

pub(super) fn text_rect(output: &egui::FullOutput, label: &str) -> Rect {
    output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.job.text == label
            {
                Some(Rect::from_min_size(text.pos, text.galley.size()))
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("Missing text: {label}"))
}

pub(super) fn accessible_node<'a>(
    output: &'a egui::FullOutput,
    label: &str,
) -> (egui::accesskit::NodeId, &'a egui::accesskit::Node) {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("Accessibility must be enabled for this test.")
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(label))
        .map(|(id, node)| (*id, node))
        .unwrap_or_else(|| panic!("Missing accessible control: {label}"))
}

/// The bounds of a painted label, whose text egui stores as its value.
pub(super) fn accessible_label(output: &egui::FullOutput, text: &str) -> Rect {
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("Accessibility must be enabled for this test.")
        .nodes
        .iter()
        .find(|(_, node)| node.role() == egui::accesskit::Role::Label && node.value() == Some(text))
        .and_then(|(_, node)| node.bounds())
        .unwrap_or_else(|| panic!("Missing accessible label: {text}"));
    Rect::from_min_max(
        Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
        Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
    )
}

pub(super) fn key_event(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

pub(super) fn focus_control(
    context: &egui::Context,
    app: &mut MixJamApp,
    output: &egui::FullOutput,
    label: &str,
) {
    let (target, _) = accessible_node(output, label);
    frame(
        context,
        app,
        vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Focus,
                target,
                data: None,
            },
        )],
        egui::Modifiers::NONE,
    );
}

pub(super) const TRANSPORT_BUTTONS: [&str; 8] = [
    "Loop the whole song. Drag the bar ruler to loop a range.",
    "Go to start",
    "Back one bar",
    "Stop",
    "Play / pause (Space)",
    "Forward one bar",
    "Go to end",
    "Record into the armed track",
];

/// From a clip's center to a point that moves it: below its gain handle.
pub(super) const GRIP: Vec2 = Vec2::new(0.0, 8.0);

/// Press at `from`, move to `to` over a few frames, and release.
pub(super) fn drag(
    ctx: &egui::Context,
    app: &mut MixJamApp,
    from: Pos2,
    to: Pos2,
    modifiers: egui::Modifiers,
) {
    pointer(ctx, app, from, true, modifiers);
    for step in 1..=4 {
        frame(
            ctx,
            app,
            vec![egui::Event::PointerMoved(
                from + (to - from) * step as f32 / 4.0,
            )],
            modifiers,
        );
    }
    pointer(ctx, app, to, false, modifiers);
}

/// Whether any accessible node carries this name.
pub(super) fn has_control(output: &egui::FullOutput, name: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("Accessibility must be enabled for this test.")
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some(name))
}

/// How many accessible nodes carry this name.
pub(super) fn named_count(output: &egui::FullOutput, name: &str) -> usize {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("Accessibility must be enabled for this test.")
        .nodes
        .iter()
        .filter(|(_, node)| node.label() == Some(name))
        .count()
}

/// Whether a frame painted `text` anywhere.
pub(super) fn shows_text(output: &egui::FullOutput, text: &str) -> bool {
    output.shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::Text(shown) => shown.galley.job.text == text,
        _ => false,
    })
}
