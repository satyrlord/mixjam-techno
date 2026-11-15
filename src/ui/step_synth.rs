//! Step synth board. The settings sit in a fixed column at the left; the grid
//! column fills the rest of the lower panel and shows as many bars as keep
//! the steps wide enough. Shared hardware controls own presentation.
//!
//! The board only draws and reads input. What an edit does to the pattern,
//! and the board's undo, live in `application::pattern`, which the board
//! calls through the `pattern` name; a change of look stays in these files.
//!
//! - `editor`: the board's view state, such as the octave and selection.
//! - `layout`: the board's measures and where the steps in view sit.
//! - `keys` and `parts`: the board's keys, dials, wells, and lettering.
//! - `settings`, `bar_strip`, `grid`, `notes`, `lanes`, and `toolbar`: the
//!   board's areas, from the left column to the top row.
use super::*;
use mixjam_techno::application::pattern;

#[path = "step_synth/bar_strip.rs"]
mod bar_strip;
#[path = "step_synth/editor.rs"]
mod editor;
#[path = "step_synth/grid.rs"]
mod grid;
#[path = "step_synth/keys.rs"]
mod keys;
#[path = "step_synth/lanes.rs"]
mod lanes;
#[path = "step_synth/layout.rs"]
mod layout;
#[path = "step_synth/notes.rs"]
mod notes;
#[path = "step_synth/parts.rs"]
mod parts;
#[path = "step_synth/settings.rs"]
mod settings;
#[path = "step_synth/toolbar.rs"]
mod toolbar;
use self::lanes::*;
use bar_strip::*;
pub(super) use editor::Editor;
use grid::*;
use keys::*;
pub(super) use layout::MODULE_HEIGHT;
use layout::*;
use notes::*;
use parts::*;
use settings::*;
pub(super) use toolbar::toolbar;

pub(super) struct Source<'a> {
    pub name: &'a str,
    pub available: bool,
    pub missing: bool,
    pub explicit_file: bool,
    /// The source's length in ms, when it can be read. Start stays inside it.
    pub length_ms: Option<f64>,
}

impl Source<'_> {
    /// True when the palette's sound is missing and no file replaces it.
    fn missing_audio(&self) -> bool {
        self.missing && !self.explicit_file
    }

    /// True when Listen and Save sound have audio to play.
    fn playable(&self) -> bool {
        self.available && !self.missing_audio()
    }
}

pub(super) struct Feedback<'a> {
    pub message: Option<&'a str>,
    pub error: bool,
}

pub(super) enum Action {
    None,
    ChooseSource,
    PaletteSource,
    FindStart,
    Listen,
    Stop,
    SaveSound,
    SavePattern,
    LoadPattern,
    Details,
    Undo,
    Redo,
}

/// What the board's undo and redo keys can restore, and their icons.
pub(super) struct History<'a> {
    pub undo: bool,
    pub redo: bool,
    pub icons: Option<&'a RailIcons>,
}

/// Show the Step synth board in the space left in `ui`. The board is a tab
/// panel named Step synth, so agents can scope to it by that name.
/// `playing` is the pattern step Listen is playing. `icons` draws the
/// pattern file keys' icons; without them the keys show text only.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    ui: &mut egui::Ui,
    source: Source<'_>,
    state: &mut StepSynthState,
    view_bar: &mut usize,
    show_effects: bool,
    editor: &mut Editor,
    playing: Option<usize>,
    icons: Option<&RailIcons>,
) -> Action {
    let ctx = ui.ctx().clone();
    let area = ui.available_rect_before_wrap();
    ui.advance_cursor_after_rect(area);
    editor.show_at(area);
    // Adopt external changes before drawing, but close a pointer gesture only
    // after this frame's controls have made their final edits.
    editor.track(state, true, false);
    let available = source.playable();
    let mut board = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("step-synth-board")
            .max_rect(area),
    );
    dialog::hardware_style(&mut board, controls::Palette::Olive);
    board.set_clip_rect(area.intersect(ui.clip_rect()));
    controls::panel(&board, area);
    ctx.accesskit_node_builder(board.unique_id(), |node| {
        node.set_role(egui::accesskit::Role::TabPanel);
        node.set_label("Step synth");
        node.set_bounds(egui::accesskit::Rect {
            x0: area.left().into(),
            y0: area.top().into(),
            x1: area.right().into(),
            y1: area.bottom().into(),
        });
    });
    let inner = area.shrink(BOARD_INSET + 8.0);
    let settings_rect = Rect::from_min_size(inner.min, Vec2::new(SETTINGS_WIDTH, inner.height()));
    let grid_rect = Rect::from_min_max(
        Pos2::new(settings_rect.right() + 8.0, inner.top()),
        inner.max,
    );
    controls::inset(&board, settings_rect);
    controls::inset(&board, grid_rect);
    let action = settings(
        &mut board,
        settings_rect.shrink(6.0),
        source,
        state,
        editor,
        icons,
    );
    let listen = if show_effects {
        effects_pane(
            &mut board,
            grid_rect.shrink(8.0),
            state,
            editor,
            available,
            playing,
        )
    } else {
        grid_column(
            &mut board,
            grid_rect.shrink(8.0),
            state,
            view_bar,
            editor,
            playing,
            available,
        )
    };
    editor.track(state, ctx.input(|input| input.pointer.any_down()), true);
    if matches!(action, Action::None) {
        listen
    } else {
        action
    }
}
