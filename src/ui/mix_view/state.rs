//! The Mix view's state, and how it opens and closes.
use super::*;

/// The Mix view: whether the score shows it, and its column scroll.
#[derive(Default)]
pub(in super::super) struct MixViewState {
    /// The score shows the Mix view's mixer rows instead of the lane headers.
    /// It never shows with the Mixer board.
    pub(in super::super) shown: bool,
    /// How far the Mix view's columns are scrolled to the left.
    pub(in super::super) scroll: f32,
    /// The Mix view control last scrolled into view for keyboard focus.
    pub(in super::super) revealed: Option<egui::Id>,
    /// The Mix view folded the lower panel when it opened; leaving it opens
    /// the panel again unless the lower panel was opened or folded since.
    pub(in super::super) restore_boards: bool,
    /// The Mix view's first opening in this session named its shortcuts in
    /// the status bar.
    pub(in super::super) hinted: bool,
    /// The part of the row, from its number through Pan, of the track whose
    /// channel EQ is open or was just pressed. The EQ window first opens
    /// beside it, so the row's name, level, and pan stay in view.
    pub(in super::super) eq_row: Option<Rect>,
}

/// The status line of the Mix view's first opening in a session: the
/// shortcuts that its quiet rows do not show.
pub(in super::super) const FIRST_OPEN_HINT: &str = "Mix view: double-click a fader or knob to \
     reset it. Shift and the wheel scroll the columns.";

impl MixJamApp {
    /// Show the score's Mix view or its Arrange view. The Mix view and the
    /// Mixer board never show at once, so the lower panel leaves Mixer for
    /// Mastering. The Mix view is its own mode: it folds the lower panel so
    /// its rows take the whole score, and leaving it opens the panel again
    /// unless the panel was opened or folded in between.
    pub(in super::super) fn set_mix_view(&mut self, on: bool) {
        if on && self.boards.module == BottomModule::Mixer {
            self.boards.module = BottomModule::Mastering;
        }
        if on == self.mix.shown {
            return;
        }
        self.mix.shown = on;
        if on && !std::mem::replace(&mut self.mix.hinted, true) {
            self.message(FIRST_OPEN_HINT);
        }
        if on {
            let open = self.session.layout().boards;
            if open {
                self.set_layout(|layout| layout.boards = false);
            }
            self.mix.restore_boards = open;
        } else if std::mem::take(&mut self.mix.restore_boards) && !self.session.layout().boards {
            self.set_layout(|layout| layout.boards = true);
        }
    }

    /// The height the score keeps below its lanes: the Mix view's Master and
    /// returns rows, which stay in view while the tracks scroll.
    pub(in super::super) fn score_block_height(&self) -> f32 {
        if self.mix.shown { BLOCK_HEIGHT } else { 0.0 }
    }
}
