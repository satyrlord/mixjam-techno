//! Options window tests through its accessible controls, grouped by what they exercise.
//! The drivers that both groups use are defined here.

use super::options::*;
use super::*;
use crate::library::Library;
use mixjam_techno::application::options::AppOptions;

mod pages_tests;
mod window_tests;

fn fixture() -> (egui::Context, MixJamApp, PathBuf) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "mixjam-options-ui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let session = Session::headless(Arc::new(Library::empty()), Project::default())
        .unwrap()
        .with_settings_root(root.clone());
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    configure_style(&ctx);
    let mut app = MixJamApp::from_session(session, "Options fixture".into(), false);
    app.options = None;
    app.capture.path = None;
    (ctx, app, root)
}

fn frame(
    ctx: &egui::Context,
    app: &mut MixJamApp,
    size: Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            focused: true,
            events,
            ..Default::default()
        },
        |ctx| app.draw(ctx),
    )
}

fn node<'a>(output: &'a egui::FullOutput, name: &str) -> &'a egui::accesskit::Node {
    &output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(name))
        .unwrap_or_else(|| panic!("Missing accessible control: {name}"))
        .1
}

fn has_text(output: &egui::FullOutput, text: &str) -> bool {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .any(|(_, node)| node.value() == Some(text) || node.label() == Some(text))
}

fn click(ctx: &egui::Context, app: &mut MixJamApp, name: &str) {
    let size = Vec2::new(1024.0, 768.0);
    let output = frame(ctx, app, size, vec![]);
    let rect = node(&output, name).bounds().unwrap();
    let pos = Pos2::new(
        ((rect.x0 + rect.x1) / 2.0) as f32,
        ((rect.y0 + rect.y1) / 2.0) as f32,
    );
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            size,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}
