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

/// A fixture with an empty library and a settings folder of its own, so a
/// test can apply options or adopt library folders without touching another
/// test's files. Pass the folder to `remove_settings` at the end.
pub(super) fn isolated_fixture() -> (egui::Context, MixJamApp, PathBuf) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = settings_fixture_dir(&std::env::temp_dir(), &NEXT);
    let session = Session::headless(Arc::new(Library::empty()), Project::default())
        .unwrap()
        .with_settings_root(root.clone());
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    configure_style(&ctx);
    let mut app = MixJamApp::from_session(session, "Isolated fixture".into(), false);
    // An empty library opens Sample libraries at launch; the tests open
    // what they use.
    app.options = None;
    app.libraries.open = false;
    app.libraries.close_after_scan = false;
    app.capture.path = None;
    (ctx, app, root)
}

fn settings_fixture_dir(root: &std::path::Path, next: &std::sync::atomic::AtomicU64) -> PathBuf {
    loop {
        let path = root.join(format!(
            "mixjam-settings-ui-{}-{}",
            std::process::id(),
            next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => return path,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!(
                "Cannot create test settings folder {}: {error}",
                path.display()
            ),
        }
    }
}

#[test]
fn settings_fixtures_preserve_existing_directories() {
    use std::sync::atomic::AtomicU64;
    let base = std::env::temp_dir().join(format!(
        "mixjam-settings-reservation-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&base).unwrap();
    let stale = base.join(format!("mixjam-settings-ui-{}-0", std::process::id()));
    std::fs::create_dir(&stale).unwrap();
    std::fs::write(stale.join("options.json"), b"existing settings").unwrap();
    let fresh = settings_fixture_dir(&base, &AtomicU64::new(0));
    assert_ne!(fresh, stale);
    assert!(std::fs::read_dir(fresh).unwrap().next().is_none());
    assert_eq!(
        std::fs::read(stale.join("options.json")).unwrap(),
        b"existing settings"
    );
    std::fs::remove_dir_all(base).unwrap();
}

/// Remove an `isolated_fixture`'s settings folder, if anything was saved.
pub(super) fn remove_settings(root: &std::path::Path) {
    if root.exists() {
        std::fs::remove_dir_all(root).unwrap();
    }
}

/// Choose `value` in the choice box named `name`, as a person does: open
/// it, then click the item that it adds.
pub(super) fn choose(ctx: &egui::Context, app: &mut MixJamApp, name: &str, value: &str) {
    let before = frame(ctx, app, vec![], egui::Modifiers::NONE);
    let known: Vec<_> = screen(&before).nodes().map(|node| node.id()).collect();
    click_named(ctx, app, name);
    let output = frame(ctx, app, vec![], egui::Modifiers::NONE);
    let item = screen(&output)
        .nodes()
        .find(|node| node.node().label() == Some(value) && !known.contains(&node.id()))
        .unwrap_or_else(|| panic!("{name} offers no {value}"))
        .rect();
    // A pointer click, so the list closes as it does for a person.
    click(ctx, app, item.center(), egui::Modifiers::NONE);
    frame(ctx, app, vec![], egui::Modifiers::NONE);
}

/// The label of the control that has keyboard focus in `output`.
pub(super) fn focused_label(output: &egui::FullOutput) -> Option<&str> {
    let tree = output.platform_output.accesskit_update.as_ref()?;
    tree.nodes
        .iter()
        .find(|(id, _)| *id == tree.focus)
        .and_then(|(_, node)| node.label())
}

/// Press `key` once and return the frame that handled it.
pub(super) fn press(
    ctx: &egui::Context,
    app: &mut MixJamApp,
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> egui::FullOutput {
    frame(
        ctx,
        app,
        vec![key_event(key, modifiers)],
        egui::Modifiers::NONE,
    )
}

/// Each theme in turn. Taking one sets its theme on this test's
/// thread, and the loop body's fixture then adopts it.
/// Each new item, and the end of the loop, first checks that the previous
/// body's frames kept its theme, as `each_theme` does. The end returns the
/// thread to Studio.
pub(super) fn themes_in_turn() -> impl Iterator<Item = theme::Theme> {
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
        .inspect(move |&theme| {
            check();
            previous.set(Some(theme));
            theme::paint_as(theme);
        })
        .chain(std::iter::from_fn(move || {
            end();
            theme::paint_as(theme::Theme::Studio);
            None
        }))
}

pub(super) fn effects_control_rect(output: &egui::FullOutput, name: &str) -> Rect {
    screen(output).node(name).rect()
}

/// Click a control through accessibility, as the agent channel does.
pub(super) fn click_named(
    ctx: &egui::Context,
    app: &mut MixJamApp,
    name: &str,
) -> egui::FullOutput {
    let output = frame(ctx, app, vec![], egui::Modifiers::NONE);
    // A board and its toolbar row carry the board's name; click the control.
    let target = screen(&output)
        .nodes()
        .find(|node| {
            node.node().label() == Some(name)
                && !matches!(
                    node.role(),
                    egui::accesskit::Role::Toolbar | egui::accesskit::Role::TabPanel
                )
        })
        .map_or_else(|| accessible_node(&output, name).0, |node| node.id());
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

/// The colour a text shape's glyphs are drawn in: its override, else its
/// first section's colour, with a placeholder resolved to the fallback.
pub(super) fn drawn_text_color(text: &egui::epaint::TextShape) -> egui::Color32 {
    text.override_text_color.unwrap_or_else(|| {
        let color = text.galley.job.sections[0].format.color;
        if color == egui::Color32::PLACEHOLDER {
            text.fallback_color
        } else {
            color
        }
    })
}

pub(super) fn text_rect(output: &egui::FullOutput, label: &str) -> Rect {
    screen(output).text(label).rect()
}

pub(super) fn accessible_node<'a>(
    output: &'a egui::FullOutput,
    label: &str,
) -> (egui::accesskit::NodeId, &'a egui::accesskit::Node) {
    let node = screen(output).node(label);
    (node.id(), node.node())
}

/// The bounds of a painted label, whose text egui stores as its value.
pub(super) fn accessible_label(output: &egui::FullOutput, text: &str) -> Rect {
    screen(output).label(text).rect()
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
    screen(output).find_node(name).is_some()
}

/// How many accessible nodes carry this name.
pub(super) fn named_count(output: &egui::FullOutput, name: &str) -> usize {
    screen(output).named_count(name)
}

/// Whether a frame painted `text` anywhere.
pub(super) fn shows_text(output: &egui::FullOutput, text: &str) -> bool {
    screen(output).shows(text)
}
