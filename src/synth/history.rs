//! The Step synth board's own undo, apart from the song's. It watches the
//! whole pattern (notes, length, settings, effects, scale, and source) and
//! makes each edit one step: every change while a pointer is held, or each
//! change without one.
use super::StepSynthState;

/// The whole pattern: what undo restores.
type Snapshot = StepSynthState;

#[derive(Clone, Debug, Default)]
pub struct PatternHistory {
    /// The last observed pattern.
    seen: Option<Snapshot>,
    /// The pattern before the edit in progress, and what the edit is. A
    /// gesture ends when the pointer is released.
    gesture: Option<(Snapshot, &'static str)>,
    /// What the edit made this frame is, for the undo message.
    what: Option<&'static str>,
    /// Snapshot to undo to and to redo to, oldest first, with what changed them.
    history: Vec<(Snapshot, &'static str)>,
    future: Vec<(Snapshot, &'static str)>,
}

impl PatternHistory {
    /// Record an edit made since the last observation. `held` is true while
    /// a pointer is down. `board` is true after the board's controls drew;
    /// a change before them came from elsewhere, such as a load. Returns true
    /// for such an outside change.
    pub fn track(&mut self, state: &StepSynthState, held: bool, board: bool) -> bool {
        let changed = self.seen.as_ref().is_some_and(|seen| seen != state);
        let mut outside = false;
        if changed && self.gesture.is_none() {
            let notes = self
                .seen
                .as_ref()
                .is_some_and(|seen| seen.steps != state.steps);
            let what = match self.what {
                Some(what) => what,
                None if board && notes => "the note edit",
                None if board => "the setting change",
                None => {
                    outside = true;
                    "the pattern change"
                }
            };
            self.gesture = self.seen.take().map(|before| (before, what));
        }
        self.what = None;
        if changed || self.seen.is_none() {
            self.seen = Some(state.clone());
        }
        if !held
            && let Some(before) = self.gesture.take()
            && before.0 != *state
        {
            self.history.push(before);
            self.future.clear();
        }
        outside
    }

    /// Name the edit this frame makes, such as "Clear bar", for the undo
    /// message.
    pub fn name(&mut self, what: &'static str) {
        self.what = Some(what);
    }

    /// True when undo (or, with `redo`, redo) has an edit to restore.
    pub fn can_undo(&self, redo: bool) -> bool {
        if redo {
            !self.future.is_empty()
        } else {
            !self.history.is_empty()
                || self.gesture.as_ref().is_some_and(|(before, _)| {
                    self.seen.as_ref().is_some_and(|seen| seen != before)
                })
        }
    }

    /// Take back the last edit, or with `redo` put it back. Returns what the
    /// edit was.
    pub fn undo(&mut self, state: &mut StepSynthState, redo: bool) -> Option<&'static str> {
        // An edit not yet recorded is the newest one.
        if let Some(before) = self.gesture.take()
            && before.0 != *state
        {
            self.history.push(before);
            self.future.clear();
        }
        let (from, to) = if redo {
            (&mut self.future, &mut self.history)
        } else {
            (&mut self.history, &mut self.future)
        };
        let (before, what) = from.pop()?;
        to.push((std::mem::replace(state, before), what));
        self.seen = Some(state.clone());
        Some(what)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gesture_that_returns_to_its_start_does_not_add_an_undo_step() {
        for release in [false, true] {
            let mut history = PatternHistory::default();
            let mut state = StepSynthState::default();
            history.track(&state, false, true);
            state.length = 32;
            history.track(&state, true, true);
            state.length = 16;
            history.track(&state, !release, true);
            assert!(!history.can_undo(false));
        }
    }

    #[test]
    fn a_gesture_that_returns_to_its_start_preserves_redo() {
        for release in [false, true] {
            let mut history = PatternHistory::default();
            let mut state = StepSynthState::default();
            history.track(&state, false, true);
            state.length = 32;
            history.name("Length");
            history.track(&state, false, true);
            assert_eq!(history.undo(&mut state, false), Some("Length"));
            state.length = 24;
            history.track(&state, true, true);
            state.length = 16;
            history.track(&state, !release, true);
            assert_eq!(history.undo(&mut state, true), Some("Length"));
            assert_eq!(state.length, 32);
        }
    }
}
