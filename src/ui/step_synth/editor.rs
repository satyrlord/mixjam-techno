//! The board's editing view: what the grid shows, the selected steps, and
//! the board's undo. None of it is saved with the pattern.
use super::*;

#[derive(Clone, Debug, Default)]
pub(in super::super) struct Editor {
    /// Octaves the 25 pitch rows are shifted by, within `OCTAVES`.
    pub octave: i8,
    /// The first and last selected step. Lane edits apply to all of them.
    pub selection: Option<(usize, usize)>,
    /// The step shared by the pitch cursor, ruler, and value lanes.
    pub(super) step: usize,
    seed: u64,
    history: pattern::PatternHistory,
    /// The board's rectangle last frame, where Ctrl+Z undoes its edits.
    board: Option<Rect>,
    /// True once the grid has scrolled to the notes. A load, Randomise, or
    /// new source clears it, so the grid opens where the music is.
    centred: bool,
    /// What the board's last edit did, for the status line.
    notice: Option<String>,
}

impl Editor {
    /// Record an edit made since the last observation. Every change while a
    /// pointer is held is one undo step, as is each change without one.
    /// `board` is true after the board's controls drew; a change before
    /// them came from elsewhere, such as a load, and centres the grid.
    pub(super) fn track(&mut self, state: &StepSynthState, held: bool, board: bool) {
        if self.history.track(state, held, board) {
            self.centred = false;
        }
    }

    /// Name the edit this frame makes, for the undo message.
    pub(in super::super) fn edit(&mut self, what: &'static str) {
        self.history.name(what);
    }

    /// Name the edit this frame makes, and tell the status line what it did.
    pub(super) fn edit_saying(&mut self, what: &'static str, notice: String) {
        self.edit(what);
        self.notice = Some(notice);
    }

    /// What the board's last edit did, once, for the status line.
    pub(in super::super) fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }

    /// Note where the board shows this frame.
    pub(super) fn show_at(&mut self, area: Rect) {
        self.board = Some(area);
    }

    /// Scroll the grid to the notes when it next shows.
    pub(in super::super) fn recentre(&mut self) {
        self.centred = false;
    }

    /// The board's rectangle if it showed since the last call. The keyboard
    /// takes it each frame before the board draws.
    pub(in super::super) fn take_board(&mut self) -> Option<Rect> {
        self.board.take()
    }

    /// True when Ctrl+Z (or, with `redo`, Ctrl+Y) has an edit to restore.
    pub(in super::super) fn can_undo(&self, redo: bool) -> bool {
        self.history.can_undo(redo)
    }

    /// Take back the board's last edit, or with `redo` put it back. Returns
    /// what the edit was, such as "Clear bar".
    pub(in super::super) fn undo(
        &mut self,
        state: &mut StepSynthState,
        redo: bool,
    ) -> Option<&'static str> {
        let what = self.history.undo(state, redo)?;
        self.selection = None;
        Some(what)
    }

    /// Once after a recentre, move the view to the octave nearest the middle
    /// of the notes when it is out of view. Returns the pitch row to scroll
    /// to, or `None` when the grid has already centred.
    pub(super) fn centre_on_notes(&mut self, state: &StepSynthState) -> Option<i8> {
        if self.centred {
            return None;
        }
        self.centred = true;
        let middle = pattern::pitch_middle(state).unwrap_or(12 * i32::from(self.octave));
        if !shown_pitches(self.octave).contains(&middle) {
            self.octave =
                ((middle as f32 / 12.0).round() as i8).clamp(*OCTAVES.start(), *OCTAVES.end());
        }
        let shown = shown_pitches(self.octave);
        Some(middle.clamp(*shown.start(), *shown.end()) as i8)
    }

    /// Write a new line in the scale over the pattern's length.
    pub(super) fn randomise(&mut self, state: &mut StepSynthState) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |time| time.as_nanos() as u64);
        let line = pattern::Line {
            scale: state.scale,
            root: state.root,
            octave: self.octave,
        };
        self.seed = pattern::randomise(state, line, self.seed ^ now);
        self.selection = None;
        self.edit_saying(
            "Randomise",
            format!(
                "Wrote a new {} line in {}. Ctrl+Z takes it back.",
                state.scale.label(),
                pattern::ROOTS[usize::from(state.root % 12)]
            ),
        );
        self.recentre();
    }

    /// True when `step` is in the selected range.
    pub(super) fn selected(&self, step: usize) -> bool {
        pattern::in_selection(self.selection, step)
    }
}

/// The pitches the rows show at an octave.
fn shown_pitches(octave: i8) -> std::ops::RangeInclusive<i32> {
    i32::from(-ROWS + 12 * octave)..=i32::from(ROWS + 12 * octave)
}
