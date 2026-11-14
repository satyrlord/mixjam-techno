//! Room for the panels around the score. The Sounds panel and the Inspector
//! share the score's width; the Sounds panel and the lower panel's board share
//! its height. When a pair does not fit, the panel opened last shows and the
//! other folds to its strip or toolbar row. The saved layout keeps every
//! choice, so a larger window shows them all again.
use super::inspector::INSPECTOR_WIDTH;
use super::sounds::SOUNDS_PANEL_WIDTH;
use super::*;
use mixjam_techno::application::options::WorkspaceLayout;

/// The narrowest score that the two side panels leave between them.
pub(super) const SCORE_MIN_WIDTH: f32 = 520.0;
/// The Sounds panel's least height above an open board: its search,
/// filters, chips, and sort, and three sound rows.
pub(super) const SOUNDS_MIN_HEIGHT: f32 = 330.0;

/// A panel that can fold to make room for another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Panel {
    Sounds,
    Inspector,
    Boards,
}

/// The panels the score shows this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Room {
    pub sounds: bool,
    pub inspector: bool,
    pub boards: bool,
}

/// The space the panels share, as last laid out: the width beside the tool
/// rail, the height between the top bar and the status bar, and the open
/// board's height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Space {
    pub width: f32,
    pub height: f32,
    pub board: f32,
}

impl Default for Space {
    /// The default window, before its first frame.
    fn default() -> Self {
        Self {
            width: 1356.0,
            height: 900.0,
            board: 290.0,
        }
    }
}

/// The folding panels: which show this frame, the space they were chosen
/// for, and the order they were opened in.
pub(super) struct Panels {
    /// The panels that show this frame, and the space they were chosen for.
    pub(super) room: Room,
    pub(super) space: Space,
    /// The folding panels, from the one opened last.
    pub(super) order: [Panel; 3],
    /// The Inspector was opened this session, so a narrow window shows it.
    pub(super) inspector_opened: bool,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            room: Room::default(),
            space: Space::default(),
            order: [Panel::Boards, Panel::Sounds, Panel::Inspector],
            inspector_opened: false,
        }
    }
}

/// Which panels show. `order` lists the panels from the one opened last.
/// The Inspector shows beside an open Sounds panel only where both fit; in a
/// narrower window it starts folded until it is opened (`inspector_opened`).
pub(super) fn resolve(
    layout: WorkspaceLayout,
    space: Space,
    order: [Panel; 3],
    inspector_opened: bool,
) -> Room {
    let mut room = Room::default();
    if layout.maximised {
        return room;
    }
    let side_by_side = space.width - SOUNDS_PANEL_WIDTH - INSPECTOR_WIDTH >= SCORE_MIN_WIDTH;
    let stacked = space.height - space.board >= SOUNDS_MIN_HEIGHT;
    for panel in order {
        match panel {
            Panel::Sounds => {
                room.sounds =
                    layout.sounds && (side_by_side || !room.inspector) && (stacked || !room.boards);
            }
            Panel::Inspector => {
                room.inspector =
                    layout.inspector && (side_by_side || (inspector_opened && !room.sounds));
            }
            Panel::Boards => room.boards = layout.boards && (stacked || !room.sounds),
        }
    }
    room
}

impl MixJamApp {
    /// Measure the space the panels share. Call after the top bar and the
    /// status bar are laid out, before the rail and the panels.
    pub(super) fn measure_room(&mut self, ctx: &egui::Context) {
        let layout = self.session.layout();
        let available = ctx.available_rect();
        let rail = if layout.maximised {
            0.0
        } else {
            rail_width(layout.tool_names)
        };
        // The open board's height: as last resized, and at least its board's
        // minimum.
        let board = egui::containers::panel::PanelState::load(ctx, egui::Id::new("sounds"))
            .map_or_else(
                || (ctx.content_rect().height() * 0.28).clamp(220.0, 290.0),
                |state| state.rect.height(),
            )
            .max(self.boards.module.minimum_height(available.height()));
        let space = Space {
            width: available.width() - rail,
            height: available.height(),
            board,
        };
        if space != self.panels.space {
            self.panels.space = space;
            self.resolve_room();
        }
    }

    /// Decide again which panels show, after the layout or the space changed.
    pub(super) fn resolve_room(&mut self) {
        self.panels.room = resolve(
            self.session.layout(),
            self.panels.space,
            self.panels.order,
            self.panels.inspector_opened,
        );
    }

    /// `panel` was opened: it takes the room it shares first.
    pub(super) fn claim(&mut self, panel: Panel) {
        let mut order = vec![panel];
        order.extend(
            self.panels
                .order
                .into_iter()
                .filter(|other| *other != panel),
        );
        self.panels.order = [order[0], order[1], order[2]];
        if panel == Panel::Inspector {
            self.panels.inspector_opened = true;
        }
        self.resolve_room();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_ORDER: [Panel; 3] = [Panel::Boards, Panel::Sounds, Panel::Inspector];

    fn layout(sounds: bool, inspector: bool, boards: bool) -> WorkspaceLayout {
        WorkspaceLayout {
            sounds,
            inspector,
            boards,
            ..WorkspaceLayout::default()
        }
    }

    #[test]
    fn panels_that_fit_all_show_and_a_narrow_window_starts_with_the_inspector_folded() {
        let wide = Space {
            width: 1356.0,
            height: 914.0,
            board: 290.0,
        };
        let room = resolve(layout(true, true, true), wide, DEFAULT_ORDER, false);
        assert_eq!(
            room,
            Room {
                sounds: true,
                inspector: true,
                boards: true
            }
        );
        // 960 wide: the score would keep 356 points between both panels.
        let narrow = Space {
            width: 876.0,
            ..wide
        };
        let room = resolve(layout(false, true, true), narrow, DEFAULT_ORDER, false);
        assert!(!room.inspector, "unopened, it starts folded");
        let room = resolve(layout(false, true, true), narrow, DEFAULT_ORDER, true);
        assert!(room.inspector, "opened, it shows beside the Sounds strip");
        let room = resolve(layout(true, true, true), narrow, DEFAULT_ORDER, true);
        assert!(room.sounds && !room.inspector);
        let order = [Panel::Inspector, Panel::Boards, Panel::Sounds];
        let room = resolve(layout(true, true, true), narrow, order, true);
        assert!(room.inspector && !room.sounds, "the one opened last wins");
        assert!(room.boards);
    }

    #[test]
    fn a_short_window_gives_the_sounds_list_or_the_board_the_height() {
        let short = Space {
            width: 876.0,
            height: 634.0,
            board: 400.0,
        };
        let room = resolve(layout(true, false, true), short, DEFAULT_ORDER, false);
        assert!(room.boards && !room.sounds);
        let order = [Panel::Sounds, Panel::Boards, Panel::Inspector];
        let room = resolve(layout(true, false, true), short, order, false);
        assert!(room.sounds && !room.boards);
        // A board short enough leaves the list three rows.
        let room = resolve(
            layout(true, false, true),
            Space {
                board: 290.0,
                ..short
            },
            DEFAULT_ORDER,
            false,
        );
        assert!(room.sounds && room.boards);
        let maximised = WorkspaceLayout {
            maximised: true,
            ..layout(true, true, true)
        };
        assert_eq!(
            resolve(maximised, short, DEFAULT_ORDER, true),
            Room::default()
        );
    }
}
