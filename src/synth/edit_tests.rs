//! Pattern edits and the board's undo, without a screen.
use super::edit::*;
use super::history::PatternHistory;
use super::*;

/// A pattern of rests, a bar long.
fn empty() -> StepSynthState {
    StepSynthState {
        steps: vec![Step::default(); STEPS],
        ..Default::default()
    }
}

/// A pattern with a note at each `(step, pitch)`.
fn with_notes(notes: &[(usize, i8)]) -> StepSynthState {
    let mut state = empty();
    for (step, pitch) in notes {
        state.steps[*step] = Step::note(*pitch);
    }
    state
}

fn trigs(state: &StepSynthState, steps: std::ops::Range<usize>) -> Vec<Trig> {
    state.steps[steps].iter().map(|step| step.trig).collect()
}

#[test]
fn covers_follow_each_note_through_its_ties() {
    let mut state = with_notes(&[(0, 3), (4, -2)]);
    state.steps[1] = Step::tie();
    state.steps[2] = Step::tie();
    let covers = covers(&state.steps);
    assert_eq!(
        &covers[..5],
        &[
            Some((0, 3)),
            Some((0, 3)),
            Some((0, 3)),
            None,
            Some((4, -2))
        ]
    );
    assert_eq!(note_end(&state.steps, 0), 2);
    assert_eq!(note_end(&state.steps, 4), 4);
    assert_eq!(note_steps(&state.steps), vec![0, 4]);
}

#[test]
fn note_edits_place_clear_and_flag_a_note() {
    let mut state = with_notes(&[(0, 0)]);
    state.steps[1] = Step::tie();
    NoteEdit::Accent(0).apply(&mut state);
    NoteEdit::Slide(0).apply(&mut state);
    assert!(state.steps[0].accent && state.steps[0].slide);
    NoteEdit::Place { step: 0, note: 7 }.apply(&mut state);
    assert_eq!(
        state.steps[0].trig,
        Trig::Note(7),
        "A new pitch replaces the note."
    );
    NoteEdit::Clear(0).apply(&mut state);
    assert_eq!(state.steps[0], Step::default());
    assert_eq!(
        state.steps[1].trig,
        Trig::Rest,
        "Clearing a note clears its ties."
    );
}

#[test]
fn a_tie_extends_or_shortens_from_the_drag_snapshot_and_replaces_covered_notes() {
    let mut state = with_notes(&[(0, 0), (3, 5)]);
    state.steps[4] = Step::tie();
    let snapshot = state.steps.clone();
    NoteEdit::Tie {
        snapshot: snapshot.clone(),
        head: 0,
        end: 3,
    }
    .apply(&mut state);
    assert_eq!(
        trigs(&state, 0..6),
        vec![
            Trig::Note(0),
            Trig::Tie,
            Trig::Tie,
            Trig::Tie,
            Trig::Rest,
            Trig::Rest,
        ]
    );
    // Dragging back shortens the note from the same snapshot.
    NoteEdit::Tie {
        snapshot,
        head: 0,
        end: 1,
    }
    .apply(&mut state);
    assert_eq!(
        trigs(&state, 0..5),
        vec![
            Trig::Note(0),
            Trig::Tie,
            Trig::Rest,
            Trig::Note(5),
            Trig::Tie,
        ]
    );
    assert!(state.validate().is_ok());
}

#[test]
fn bar_actions_copy_forward_and_clear_one_bar() {
    let mut state = with_notes(&[(0, 5), (20, 1)]);
    assert_eq!(copy_target(0), Some(1));
    assert!(copy_bar(&mut state, 0));
    assert_eq!(state.steps[16], Step::note(5));
    assert_eq!(
        state.steps[20],
        Step::default(),
        "Copy replaces the target bar."
    );
    let before = state.clone();
    assert_eq!(copy_target(BARS - 1), None);
    assert!(
        !copy_bar(&mut state, BARS - 1),
        "The last bar has no bar after it."
    );
    assert_eq!(state, before);
    assert!(!bar_is_empty(&state, 1));
    clear_bar(&mut state, 1);
    assert!(bar_is_empty(&state, 1));
    assert_eq!(
        state.steps[0],
        Step::note(5),
        "Clear bar keeps the other bars."
    );
}

#[test]
fn lengths_stay_in_range_and_mark_the_bars_in_the_loop() {
    assert_eq!(doubled_length(40), 64);
    assert_eq!(doubled_length(8), 16);
    assert_eq!(halved_length(1), 1);
    assert_eq!(halved_length(9), 4);
    assert_eq!(clamp_length(-5), 1);
    assert_eq!(clamp_length(500), 64);
    assert_eq!(bar_in_loop(24, 0), 1.0);
    assert_eq!(bar_in_loop(24, 1), 0.5);
    assert_eq!(bar_in_loop(24, 2), 0.0);
}

#[test]
fn double_length_repeats_the_loop_into_empty_steps_only() {
    let mut state = with_notes(&[(0, 0), (5, 7)]);
    state.length = 12;
    assert!(double_length(&mut state));
    assert_eq!(state.length, 24);
    assert_eq!(state.steps[12], state.steps[0]);
    assert_eq!(state.steps[17], state.steps[5]);
    // At 40 steps only the first 24 fit before the longest length.
    state.length = 40;
    assert!(double_length(&mut state));
    assert_eq!(state.length, 64);
    assert_eq!(state.steps[40], state.steps[0]);
    assert!(!double_length(&mut state), "64 is the longest length.");
    // Notes that Halve length hid come back unchanged.
    let mut state = with_notes(&[(0, 0), (20, 9)]);
    state.length = 16;
    assert!(double_length(&mut state));
    assert_eq!(state.steps[16], Step::default());
    assert_eq!(state.steps[20], Step::note(9));
}

#[test]
fn lane_edits_apply_to_the_selected_notes_and_keep_level_differences() {
    let mut state = with_notes(&[(0, 0), (1, 2), (3, 4)]);
    state.steps[0].velocity = 40;
    state.steps[1].velocity = 70;
    let selection = Some((0, 2));
    assert_eq!(lane_targets(&state, selection, 1), vec![0, 1]);
    assert_eq!(lane_targets(&state, selection, 3), vec![3]);
    assert!(lane_mixed(&state, Lane::Velocity, selection));
    assert!(!lane_mixed(&state, Lane::Accent, selection));
    let targets = lane_targets(&state, selection, 0);
    set_lane(&mut state, Lane::Velocity, &targets, Some(40), 39);
    assert_eq!((state.steps[0].velocity, state.steps[1].velocity), (39, 69));
    set_lane(&mut state, Lane::Accent, &targets, Some(0), 1);
    assert!(state.steps[0].accent && state.steps[1].accent && !state.steps[3].accent);
    // A level never leaves its range, and rests take no lane value.
    set_lane(&mut state, Lane::Velocity, &[0, 2], Some(1), 100);
    assert_eq!(state.steps[0].velocity, 100);
    assert_eq!(state.steps[2], Step::default());
    // Without a start level, every target takes the same level.
    set_lane(&mut state, Lane::Velocity, &targets, None, 55);
    assert_eq!((state.steps[0].velocity, state.steps[1].velocity), (55, 55));
}

#[test]
fn fill_copies_a_bar_everywhere_and_clear_empties_the_pattern() {
    let mut state = with_notes(&[(17, 3), (20, -2)]);
    state.steps[18] = Step::tie();
    assert!(fill_bars(&mut state, 1));
    for bar in 0..BARS {
        let at = bar * BAR_STEPS;
        assert_eq!(state.steps[at + 1], Step::note(3));
        assert_eq!(state.steps[at + 2], Step::tie());
        assert_eq!(state.steps[at + 4], Step::note(-2));
    }
    assert!(!fill_bars(&mut state, 2));
    state.length = 12;
    clear_pattern(&mut state);
    assert!(pattern_is_empty(&state));
    assert_eq!(state.length, 12);
}

#[test]
fn scales_wrap_around_their_root_in_every_octave() {
    // C minor: the root C is 3 semitones above the source pitch A.
    for note in [3, 15, -9, 5, 6] {
        assert!(Scale::Minor.contains(3, note), "{note}");
    }
    assert!(!Scale::Minor.contains(3, 4));
    assert!(Scale::ALL.iter().all(|scale| scale.contains(0, 0)));
    assert_eq!(ROOTS[3], "C");
}

#[test]
fn randomise_writes_a_valid_line_in_the_scale_over_the_length_only() {
    let line = Line {
        scale: Scale::MinorPentatonic,
        root: 0,
        octave: -1,
    };
    let mut state = with_notes(&[(40, 9)]);
    state.length = 24;
    let mut again = state.clone();
    let next = randomise(&mut state, line, 7);
    randomise(&mut again, line, 7);
    assert_eq!(state, again, "The same seed writes the same line.");
    assert_ne!(next, 7);
    assert!(state.validate().is_ok());
    assert!(is_note(&state.steps[0]), "A line starts with a note.");
    for step in &state.steps[..24] {
        if let Trig::Note(note) = step.trig {
            assert!(line.scale.contains(line.root, note) && (-19..=0).contains(&note));
        }
    }
    assert_eq!(
        state.steps[40],
        Step::note(9),
        "Steps after the length keep their notes."
    );
}

#[test]
fn the_pitch_middle_counts_only_notes_within_the_length() {
    let mut state = with_notes(&[(0, -26), (8, -19), (20, 30)]);
    state.length = 16;
    assert_eq!(pitch_middle(&state), Some(-22));
    state.steps.fill(Step::default());
    assert_eq!(pitch_middle(&state), None);
}

#[test]
fn history_makes_a_held_gesture_one_named_step_and_redoes_it() {
    let mut history = PatternHistory::default();
    let mut state = with_notes(&[(0, 0)]);
    let before = state.clone();
    assert!(!history.track(&state, false, true));
    // Three frames of a drag, with the pointer held.
    for end in 1..4 {
        state.steps[end] = Step::tie();
        history.name("Tie");
        history.track(&state, true, true);
    }
    history.track(&state, false, true);
    let tied = state.clone();
    assert!(history.can_undo(false) && !history.can_undo(true));
    assert_eq!(history.undo(&mut state, false), Some("Tie"));
    assert_eq!(state, before);
    assert!(!history.can_undo(false), "The whole drag is one step.");
    assert_eq!(history.undo(&mut state, true), Some("Tie"));
    assert_eq!(state, tied);
    assert_eq!(history.undo(&mut state, true), None);
}

#[test]
fn history_reports_an_outside_change_and_undoes_an_unrecorded_edit() {
    let mut history = PatternHistory::default();
    let mut state = empty();
    history.track(&state, false, true);
    state.steps[0] = Step::note(4);
    assert!(
        history.track(&state, true, false),
        "A load is an outside change."
    );
    history.track(&state, false, true);
    // An edit made this frame is undone before the next observation.
    state.length = 32;
    history.name("the length change");
    history.track(&state, true, true);
    assert_eq!(history.undo(&mut state, false), Some("the length change"));
    assert_eq!(state.length, 16);
    assert_eq!(history.undo(&mut state, false), Some("the pattern change"));
    assert_eq!(state, empty());
}
