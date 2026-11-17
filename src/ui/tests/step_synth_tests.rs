//! Step synth board: the tests drive the board through its accessible names.
//! What an edit does to the pattern is tested apart from the screen, in
//! `synth::edit_tests`; these tests check the board's input and layout.
//!
//! - `editing`: the grid, the ruler, and the value lanes from the keyboard
//!   and the pointer.
//! - `layout`: the compact board in a short lower panel.
//! - `readiness`: the source states and the settings column's options.
//! - `undo`: the board's own undo and redo, and the Listen key.
//!
//! The fixtures and finders below are shared by these groups.

use super::*;

mod editing;
mod layout;
mod readiness;
mod undo;

/// The window with the Step synth board open and accessible names on.
fn board_fixture() -> (egui::Context, MixJamApp) {
    let (ctx, mut app) = fixture();
    ctx.enable_accesskit();
    app.boards.module = BottomModule::StepSynth;
    (ctx, app)
}

/// Stop animation and smooth scrolling, so that one frame shows where a
/// scroll lands.
fn no_animation(ctx: &egui::Context) {
    ctx.style_mut(|style| {
        style.animation_time = 0.0;
        style.scroll_animation = egui::style::ScrollAnimation::none();
    });
}

/// Run `frames` idle frames. Returns the output of the last frame.
fn settle(ctx: &egui::Context, app: &mut MixJamApp, frames: usize) -> egui::FullOutput {
    let mut output = egui::FullOutput::default();
    for _ in 0..frames {
        output = frame(ctx, app, vec![], egui::Modifiers::NONE);
    }
    output
}

/// Run `frames` idle frames at the window `size`. Returns the output of the
/// last frame.
fn settle_at(
    ctx: &egui::Context,
    app: &mut MixJamApp,
    frames: usize,
    size: Vec2,
) -> egui::FullOutput {
    let mut output = egui::FullOutput::default();
    for _ in 0..frames {
        output = frame_at_size(ctx, app, vec![], egui::Modifiers::NONE, size);
    }
    output
}

/// One frame that presses `key` with `modifiers` held.
fn press(
    ctx: &egui::Context,
    app: &mut MixJamApp,
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> egui::FullOutput {
    frame(ctx, app, vec![key_event(key, modifiers)], modifiers)
}

/// An assistive request for `action` on the node `target`.
fn access_request(target: egui::accesskit::NodeId, action: egui::accesskit::Action) -> egui::Event {
    egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
        action,
        target,
        data: None,
    })
}

fn step_synth_board_rect(output: &egui::FullOutput) -> Rect {
    let bounds = step_synth_board_node(output).1.bounds().unwrap();
    Rect::from_min_max(
        Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
        Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
    )
}

/// The rail's Step synth button, which opens and folds the board.
fn step_synth_rail_button(output: &egui::FullOutput) -> egui::accesskit::NodeId {
    screen(output)
        .nodes()
        .find(|node| {
            node.role() == egui::accesskit::Role::Button
                && node.node().label() == Some("Step synth")
        })
        .map(|node| node.id())
        .expect("The rail must have Step synth.")
}

fn step_synth_text_mesh_bounds(output: &egui::FullOutput, label: &str) -> Rect {
    // Glyphs that the board clips, whether or not they are scrolled into view.
    let window = step_synth_board_rect(output);
    screen(output)
        .texts()
        .filter(|text| text.label() == label)
        .filter_map(|text| {
            let shape = text.shape();
            let bounds = shape.galley.mesh_bounds.translate(shape.pos.to_vec2());
            window.contains_rect(text.clip()).then_some(bounds)
        })
        .reduce(|bounds, next| bounds.union(next))
        .unwrap_or_else(|| panic!("Missing Step synth glyphs: {label}"))
}
