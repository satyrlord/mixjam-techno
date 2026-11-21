//! Album Review window: the cover grid with its search, filters, and sort,
//! and the album page with the overview, links, and track timelines. The
//! reviews and the cover cache stay in the application layer.
use super::*;
#[path = "album_review/chart.rs"]
mod chart;
#[path = "album_review/grid.rs"]
mod grid;
#[path = "album_review/page.rs"]
mod page;
#[path = "album_review/state.rs"]
mod state;
use grid::grid;
use page::page;
pub(super) use state::{AlbumReviewState, capture_state};

pub(super) const TITLE: &str = "Album Review";
const CARD: Vec2 = Vec2::new(150.0, 214.0);
const COVER: f32 = 138.0;
const PAGE_COVER: f32 = 220.0;

pub(super) fn show(ctx: &egui::Context, open: &mut bool, state: &mut AlbumReviewState) {
    if !*open {
        return;
    }
    state.poll(ctx);
    let viewport = ctx.viewport_rect();
    dialog::window(TITLE)
        .open(open)
        .default_width((viewport.width() - 160.0).clamp(560.0, 980.0))
        .default_height((viewport.height() - 200.0).clamp(300.0, 660.0))
        .max_height((viewport.height() - 80.0).max(240.0))
        .constrain_to(viewport.shrink(12.0))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            if state.drawn_page != state.page {
                ui.scroll_to_cursor_animation(
                    Some(egui::Align::TOP),
                    egui::style::ScrollAnimation::none(),
                );
                state.drawn_page.clone_from(&state.page);
            }
            match state.page.clone() {
                Some(id) => page(ui, state, &id),
                None => grid(ui, state),
            }
        });
}

#[cfg(test)]
#[path = "album_review_state_tests.rs"]
mod tests;
