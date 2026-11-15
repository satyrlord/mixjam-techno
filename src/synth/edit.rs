//! Pattern edits: what the Step synth board does to a pattern's notes, apart
//! from how the board draws them or reads its input. Each operation keeps the
//! pattern valid, so a front end may call them in any order.
use super::{
    BAR_STEPS, LENGTH_RANGE, NOTE_RANGE, STEPS, Step, StepSynthState, Trig, VELOCITY_RANGE,
};

/// The bars a pattern stores.
pub const BARS: usize = STEPS / BAR_STEPS;

/// True when the step starts a note.
pub fn is_note(step: &Step) -> bool {
    matches!(step.trig, Trig::Note(_))
}

/// The steps that start a note, in order.
pub fn note_steps(steps: &[Step]) -> Vec<usize> {
    (0..steps.len().min(STEPS))
        .filter(|index| is_note(&steps[*index]))
        .collect()
}

/// For each step, the note that sounds there: its first step and pitch.
pub fn covers(steps: &[Step]) -> [Option<(usize, i8)>; STEPS] {
    let mut covers = [None; STEPS];
    let mut current = None;
    for (index, step) in steps.iter().enumerate().take(STEPS) {
        current = match step.trig {
            Trig::Note(note) => Some((index, note)),
            Trig::Tie => current,
            Trig::Rest => None,
        };
        covers[index] = current;
    }
    covers
}

/// The last step of the note that starts at `head`.
pub fn note_end(steps: &[Step], head: usize) -> usize {
    let mut end = head;
    while end + 1 < steps.len() && steps[end + 1].trig == Trig::Tie {
        end += 1;
    }
    end
}

/// The pitch midway between the lowest and highest note that sounds within
/// the pattern's length, or `None` when no note sounds there.
pub fn pitch_middle(state: &StepSynthState) -> Option<i32> {
    let pitches = covers(&state.steps)
        .iter()
        .take(usize::from(state.length))
        .flatten()
        .map(|(_, pitch)| i32::from(*pitch))
        .collect::<Vec<_>>();
    match (pitches.iter().min(), pitches.iter().max()) {
        (Some(low), Some(high)) => Some((low + high) / 2),
        _ => None,
    }
}

/// One edit of the pitch grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoteEdit {
    /// Put a note at a step. It replaces what the step held.
    Place { step: usize, note: i8 },
    /// Clear the note that starts at a step, with its ties.
    Clear(usize),
    /// Turn the accent of the note that starts at a step on or off.
    Accent(usize),
    /// Turn the slide of the note that starts at a step on or off.
    Slide(usize),
    /// Tie the note at `head` through `end`, from the steps as they were
    /// when the drag began. The tie replaces the notes it covers.
    Tie {
        snapshot: Vec<Step>,
        head: usize,
        end: usize,
    },
}

impl NoteEdit {
    pub fn apply(self, state: &mut StepSynthState) {
        match self {
            Self::Place { step, note } => state.steps[step].trig = Trig::Note(note),
            Self::Clear(head) => clear_note(state, head),
            Self::Accent(head) => state.steps[head].accent ^= true,
            Self::Slide(head) => state.steps[head].slide ^= true,
            Self::Tie {
                snapshot,
                head,
                end,
            } => tie_to(state, &snapshot, head, end),
        }
    }
}

/// Clear the note that starts at `head`, with its ties.
fn clear_note(state: &mut StepSynthState, head: usize) {
    let end = note_end(&state.steps, head);
    state.steps[head] = Step::default();
    for step in &mut state.steps[head + 1..=end] {
        step.trig = Trig::Rest;
    }
}

/// Tie the note at `head` through `end`, from `snapshot`. The note's old ties
/// become rests first; the remaining ties of a note it covers become rests.
fn tie_to(state: &mut StepSynthState, snapshot: &[Step], head: usize, end: usize) {
    state.steps.copy_from_slice(snapshot);
    let old_end = note_end(snapshot, head);
    for step in &mut state.steps[head + 1..=old_end] {
        step.trig = Trig::Rest;
    }
    for step in &mut state.steps[head + 1..=end] {
        step.trig = Trig::Tie;
    }
    let mut after = end + 1;
    while after < STEPS && state.steps[after].trig == Trig::Tie {
        state.steps[after].trig = Trig::Rest;
        after += 1;
    }
}

/// The steps of a bar.
fn bar_steps(bar: usize) -> std::ops::Range<usize> {
    bar * BAR_STEPS..(bar + 1) * BAR_STEPS
}

/// The bar that Copy bar writes to, or `None` for the last bar.
pub fn copy_target(bar: usize) -> Option<usize> {
    (bar + 1 < BARS).then_some(bar + 1)
}

/// Copy a bar onto the next one, replacing it. Returns false, and changes
/// nothing, for the last bar.
pub fn copy_bar(state: &mut StepSynthState, bar: usize) -> bool {
    let Some(target) = copy_target(bar) else {
        return false;
    };
    let source = state.steps[bar_steps(bar)].to_vec();
    state.steps[bar_steps(target)].copy_from_slice(&source);
    true
}

/// Copy the bar to every other bar. Returns false when the pattern is
/// already four copies of it.
pub fn fill_bars(state: &mut StepSynthState, bar: usize) -> bool {
    let source = state.steps[bar_steps(bar)].to_vec();
    let mut changed = false;
    for target in (0..BARS).filter(|target| *target != bar) {
        let steps = &mut state.steps[bar_steps(target)];
        changed |= *steps != source[..];
        steps.copy_from_slice(&source);
    }
    changed
}

/// Reset every step of the pattern to a default rest. The length, the
/// settings, and the effects stay.
pub fn clear_pattern(state: &mut StepSynthState) {
    state.steps.fill(Step::default());
}

/// True when every step of the pattern is a default rest.
pub fn pattern_is_empty(state: &StepSynthState) -> bool {
    state.steps.iter().all(|step| *step == Step::default())
}

/// True when every step of the bar is a default rest.
pub fn bar_is_empty(state: &StepSynthState, bar: usize) -> bool {
    state.steps[bar_steps(bar)]
        .iter()
        .all(|step| *step == Step::default())
}

/// Reset every step of the bar to a default rest.
pub fn clear_bar(state: &mut StepSynthState, bar: usize) {
    state.steps[bar_steps(bar)].fill(Step::default());
}

/// How much of a bar the pattern's length plays, from 0 to 1.
pub fn bar_in_loop(length: u8, bar: usize) -> f32 {
    usize::from(length)
        .saturating_sub(bar * BAR_STEPS)
        .min(BAR_STEPS) as f32
        / BAR_STEPS as f32
}

/// A length clamped to the lengths a pattern may have.
pub fn clamp_length(length: i64) -> u8 {
    length.clamp(
        i64::from(*LENGTH_RANGE.start()),
        i64::from(*LENGTH_RANGE.end()),
    ) as u8
}

/// The length doubled, at most the longest length.
pub fn doubled_length(length: u8) -> u8 {
    clamp_length(i64::from(length) * 2)
}

/// Double the pattern's length. When the new steps hold only rests, they
/// get a copy of the loop, so the pattern plays as before, twice; steps that
/// already hold notes, such as those that Halve length hid, show again as
/// they are. Returns false at the longest length.
pub fn double_length(state: &mut StepSynthState) -> bool {
    let (old, new) = (
        usize::from(state.length),
        usize::from(doubled_length(state.length)),
    );
    if new == old {
        return false;
    }
    if state.steps[old..new]
        .iter()
        .all(|step| *step == Step::default())
    {
        let copy = state.steps[..new - old].to_vec();
        state.steps[old..new].copy_from_slice(&copy);
    }
    state.length = new as u8;
    true
}

/// The length halved, at least one step.
pub fn halved_length(length: u8) -> u8 {
    clamp_length(i64::from(length) / 2)
}

/// One of the value lanes under the pitch grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Accent,
    Slide,
    Velocity,
}

impl Lane {
    pub const ALL: [Self; 3] = [Self::Accent, Self::Slide, Self::Velocity];

    pub fn label(self) -> &'static str {
        match self {
            Self::Accent => "Accent",
            Self::Slide => "Slide",
            Self::Velocity => "Velocity",
        }
    }

    /// True for the on and off lanes, Accent and Slide.
    pub fn is_flag(self) -> bool {
        self != Self::Velocity
    }

    /// The step's value in this lane: 0 or 1 for a flag, else its level.
    pub fn value(self, step: &Step) -> u8 {
        match self {
            Self::Accent => u8::from(step.accent),
            Self::Slide => u8::from(step.slide),
            Self::Velocity => step.velocity,
        }
    }

    /// The values this lane takes.
    pub fn range(self) -> std::ops::RangeInclusive<i32> {
        if self.is_flag() {
            0..=1
        } else {
            i32::from(*VELOCITY_RANGE.start())..=i32::from(*VELOCITY_RANGE.end())
        }
    }
}

/// True when `step` is in the selected range.
pub fn in_selection(selection: Option<(usize, usize)>, step: usize) -> bool {
    selection.is_some_and(|(from, to)| (from..=to).contains(&step))
}

/// The steps a lane edit at `step` changes: every selected note when the
/// step is in the selection, else the step alone.
pub fn lane_targets(
    state: &StepSynthState,
    selection: Option<(usize, usize)>,
    step: usize,
) -> Vec<usize> {
    if in_selection(selection, step) {
        note_steps(&state.steps)
            .into_iter()
            .filter(|target| in_selection(selection, *target))
            .collect()
    } else {
        vec![step]
    }
}

/// True when the selected notes have different values in the lane.
pub fn lane_mixed(state: &StepSynthState, lane: Lane, selection: Option<(usize, usize)>) -> bool {
    let mut values = note_steps(&state.steps)
        .into_iter()
        .filter(|step| in_selection(selection, *step))
        .map(|step| lane.value(&state.steps[step]));
    values
        .next()
        .is_some_and(|first| values.any(|other| other != first))
}

/// Change a lane from `from` to `to` on each target that starts a note. A
/// flag takes `to`; a level moves by the same amount on every target, so the
/// differences between them stay. To give every target the level `to`, pass
/// `None` as `from`.
pub fn set_lane(
    state: &mut StepSynthState,
    lane: Lane,
    targets: &[usize],
    from: Option<u8>,
    to: u8,
) {
    let range = lane.range();
    let to = i32::from(to).clamp(*range.start(), *range.end());
    for target in targets {
        let step = &mut state.steps[*target];
        if !is_note(step) {
            continue;
        }
        match lane {
            Lane::Accent => step.accent = to != 0,
            Lane::Slide => step.slide = to != 0,
            Lane::Velocity => {
                step.velocity = match from {
                    Some(from) => (i32::from(step.velocity) + to - i32::from(from))
                        .clamp(*range.start(), *range.end())
                        as u8,
                    None => to as u8,
                }
            }
        }
    }
}

/// The scale that shades the pitch rows. Notes outside it stay playable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scale {
    Chromatic,
    Major,
    #[default]
    Minor,
    Dorian,
    Phrygian,
    HarmonicMinor,
    MinorPentatonic,
    MajorPentatonic,
}

impl Scale {
    pub const ALL: [Self; 8] = [
        Self::Chromatic,
        Self::Major,
        Self::Minor,
        Self::Dorian,
        Self::Phrygian,
        Self::HarmonicMinor,
        Self::MinorPentatonic,
        Self::MajorPentatonic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Chromatic => "Chromatic",
            Self::Major => "Major",
            Self::Minor => "Minor",
            Self::Dorian => "Dorian",
            Self::Phrygian => "Phrygian",
            Self::HarmonicMinor => "Harmonic minor",
            Self::MinorPentatonic => "Minor pentatonic",
            Self::MajorPentatonic => "Major pentatonic",
        }
    }

    /// The scale's degrees in semitones above its root.
    pub fn degrees(self) -> &'static [i8] {
        match self {
            Self::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            Self::Major => &[0, 2, 4, 5, 7, 9, 11],
            Self::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Self::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Self::MinorPentatonic => &[0, 3, 5, 7, 10],
            Self::MajorPentatonic => &[0, 2, 4, 7, 9],
        }
    }

    /// True when `note`, in semitones from the source pitch, is in the scale
    /// on `root`, in semitones above the source pitch.
    pub fn contains(self, root: u8, note: i8) -> bool {
        let degree = (i32::from(note) - i32::from(root)).rem_euclid(12) as i8;
        self.degrees().contains(&degree)
    }
}

/// Root names, from the source pitch A upward.
pub const ROOTS: [&str; 12] = [
    "A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#",
];

/// Where Randomise writes: the scale, its root, and the octave in view.
#[derive(Clone, Copy, Debug)]
pub struct Line {
    pub scale: Scale,
    pub root: u8,
    pub octave: i8,
}

/// Write a new line over the pattern's length: notes in the scale near the
/// root of the octave, with ties, rests, accents, and slides. The same seed
/// writes the same line. Returns the seed for the next line.
pub fn randomise(state: &mut StepSynthState, line: Line, seed: u64) -> u64 {
    let mut seed = seed;
    let mut next = move || {
        // xorshift64*: enough to vary a line, never for anything else.
        seed ^= seed >> 12;
        seed ^= seed << 25;
        seed ^= seed >> 27;
        seed = seed.max(1);
        (seed.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    };
    let centre = 12 * line.octave + line.root as i8;
    let pool: Vec<i8> = (centre - 7..=centre + 12)
        .filter(|note| NOTE_RANGE.contains(note) && line.scale.contains(line.root, *note))
        .collect();
    let mut sounding = false;
    for index in 0..usize::from(state.length) {
        let roll = next();
        let step = &mut state.steps[index];
        *step = Step::default();
        if index == 0 || roll < 0.45 {
            let pick = (next() * pool.len() as f64) as usize;
            step.trig = Trig::Note(pool.get(pick).copied().unwrap_or(centre));
            step.accent = next() < 0.2;
            step.slide = next() < 0.12;
            step.velocity = 70 + (next() * 31.0) as u8;
            sounding = true;
        } else if sounding && roll < 0.62 {
            step.trig = Trig::Tie;
        } else {
            sounding = false;
        }
    }
    next().to_bits()
}
