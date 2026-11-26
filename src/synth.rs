//! Step synth patterns and their render. The step model (note, tie, or rest,
//! with accent and slide), gate, swing, resampling, source looping, and DSP
//! are the application's own policies.
use crate::{
    audio::Pcm,
    effects::{self, EffectsState},
};
use serde::{Deserialize, Serialize};
use std::{ops::RangeInclusive, path::PathBuf};

pub mod edit;
#[cfg(test)]
mod edit_tests;
pub mod history;

/// The steps a pattern stores: four bars of sixteenths.
pub const STEPS: usize = 64;
/// Sixteenth steps in a bar.
pub const BAR_STEPS: usize = 16;
/// Semitones from the source pitch that a note may take.
pub const NOTE_RANGE: RangeInclusive<i8> = -48..=48;
/// Steps the pattern repeats over.
pub const LENGTH_RANGE: RangeInclusive<u8> = 1..=64;
/// How much of a note's last step it sounds for, in percent, when it does not
/// slide into the next note.
pub const GATE_RANGE: RangeInclusive<u8> = 5..=100;
/// Where the second sixteenth of each eighth starts, in percent of the eighth:
/// 50 is straight, about 67 a triplet feel, and 75 a dotted feel.
pub const SWING_RANGE: RangeInclusive<u8> = 50..=75;
/// A note's level in percent.
pub const VELOCITY_RANGE: RangeInclusive<u8> = 1..=100;
/// The boost an accented note gets.
pub const ACCENT_DB_RANGE: RangeInclusive<f64> = 0.0..=12.0;
/// How long a slide takes to reach the next note's pitch.
pub const SLIDE_MS_RANGE: RangeInclusive<f64> = 1.0..=1000.0;
/// Where each note starts to read the source, in ms from its beginning.
pub const START_MS_RANGE: RangeInclusive<f64> = 0.0..=2000.0;
/// The fade at the end of every note that does not slide, so a cut never
/// clicks.
const END_FADE_SECONDS: f64 = 0.003;

/// What a step does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trig {
    /// Silence: a note sounding before it ends at the gate.
    #[default]
    Rest,
    /// A new note, in semitones from the source pitch. It restarts the source
    /// unless the note before it slides into it.
    Note(i8),
    /// The note before it keeps sounding through this step.
    Tie,
}

/// One sixteenth step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    #[serde(default, skip_serializing_if = "is_rest")]
    pub trig: Trig,
    #[serde(default = "full_velocity", skip_serializing_if = "is_full_velocity")]
    pub velocity: u8,
    /// The note plays louder by the pattern's accent level.
    #[serde(default, skip_serializing_if = "is_false")]
    pub accent: bool,
    /// The note glides into the next note without restarting the source.
    #[serde(default, skip_serializing_if = "is_false")]
    pub slide: bool,
}
fn is_rest(trig: &Trig) -> bool {
    *trig == Trig::Rest
}
fn full_velocity() -> u8 {
    *VELOCITY_RANGE.end()
}
fn is_full_velocity(velocity: &u8) -> bool {
    *velocity == full_velocity()
}
fn is_false(value: &bool) -> bool {
    !*value
}
impl Default for Step {
    fn default() -> Self {
        Self {
            trig: Trig::Rest,
            velocity: full_velocity(),
            accent: false,
            slide: false,
        }
    }
}
impl Step {
    pub fn note(note: i8) -> Self {
        Self {
            trig: Trig::Note(note),
            ..Self::default()
        }
    }
    pub fn tie() -> Self {
        Self {
            trig: Trig::Tie,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "serde_json::Value")]
pub struct StepSynthState {
    pub bpm: f64,
    /// The pattern repeats over its first `length` steps. Steps after it keep
    /// their contents.
    pub length: u8,
    /// Always `STEPS` long.
    pub steps: Vec<Step>,
    pub gate: u8,
    pub swing: u8,
    pub accent_db: f64,
    pub slide_ms: f64,
    /// Optional source looping is a reconstruction setting. Original loop points
    /// and generator sample envelopes have not been recovered.
    pub loop_source: bool,
    pub effects: EffectsState,
    /// The scale that shades the pitch rows and that Randomise writes in.
    pub scale: edit::Scale,
    /// The scale's root in semitones above the source pitch, 0 to 11.
    pub root: u8,
    /// Where each note starts to read the source, in ms. A sound with
    /// silence before its first hit plays on the beat with the silence cut.
    pub start_ms: f64,
    /// The source file the pattern plays, or `None` for the sound selected
    /// in the palette.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<PathBuf>,
}
impl Default for StepSynthState {
    fn default() -> Self {
        let mut steps = vec![Step::default(); STEPS];
        steps[0] = Step::note(0);
        Self {
            bpm: 140.0,
            length: BAR_STEPS as u8,
            steps,
            gate: 100,
            swing: 50,
            accent_db: 6.0,
            slide_ms: 60.0,
            loop_source: false,
            effects: EffectsState::default(),
            scale: edit::Scale::default(),
            root: 0,
            start_ms: 0.0,
            source: None,
        }
    }
}

/// The saved form, which the loader checks field by field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    bpm: f64,
    length: u8,
    steps: Vec<Step>,
    gate: u8,
    swing: u8,
    accent_db: f64,
    slide_ms: f64,
    loop_source: bool,
    effects: EffectsState,
    // Patterns saved before these fields keep the defaults.
    #[serde(default)]
    scale: edit::Scale,
    #[serde(default)]
    root: u8,
    #[serde(default)]
    start_ms: f64,
    #[serde(default)]
    source: Option<PathBuf>,
}

/// Patterns saved before ties: one optional note per step, Active bar
/// switches, and the 1/16 cut.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Version1 {
    bpm: f64,
    active_bars: [bool; 4],
    steps: Vec<Option<i8>>,
    cut_sixteenth: bool,
    loop_source: bool,
    effects: EffectsState,
}

impl TryFrom<serde_json::Value> for StepSynthState {
    type Error = String;
    fn try_from(value: serde_json::Value) -> Result<Self, String> {
        let old = value
            .as_object()
            .is_some_and(|fields| fields.contains_key("active_bars"));
        if old {
            let old: Version1 = serde_json::from_value(value).map_err(|error| error.to_string())?;
            return Self::from_version1(old);
        }
        let stored: Stored = serde_json::from_value(value).map_err(|error| error.to_string())?;
        Ok(Self {
            bpm: stored.bpm,
            length: stored.length,
            steps: stored.steps,
            gate: stored.gate,
            swing: stored.swing,
            accent_db: stored.accent_db,
            slide_ms: stored.slide_ms,
            loop_source: stored.loop_source,
            effects: stored.effects,
            scale: stored.scale,
            root: stored.root,
            start_ms: stored.start_ms,
            source: stored.source,
        })
    }
}

impl StepSynthState {
    /// Retain an older pattern's note timing and contents. The Active bars
    /// come first, in order, and set the length; the other bars follow, so no
    /// note is lost. Rendering also applies the new 3 ms note-end fade.
    /// Without the 1/16 cut a note held through empty steps to the next note,
    /// so those steps become ties.
    fn from_version1(old: Version1) -> Result<Self, String> {
        if old.steps.len() != STEPS {
            return Err(format!("The generator must have {STEPS} steps."));
        }
        if !old.active_bars.iter().any(|active| *active) {
            return Err("Switch on at least one Active bar.".into());
        }
        let bars = |active: bool| {
            (0..4)
                .filter(move |bar| old.active_bars[*bar] == active)
                .collect::<Vec<_>>()
        };
        let (active, inactive) = (bars(true), bars(false));
        let length = (active.len() * BAR_STEPS) as u8;
        let mut steps = Vec::with_capacity(STEPS);
        for part in [active, inactive] {
            let mut sounding = false;
            for bar in part {
                for note in &old.steps[bar * BAR_STEPS..(bar + 1) * BAR_STEPS] {
                    steps.push(match note {
                        Some(note) => {
                            sounding = true;
                            Step::note(*note)
                        }
                        None if sounding && !old.cut_sixteenth => Step::tie(),
                        None => Step::default(),
                    });
                }
            }
        }
        Ok(Self {
            bpm: old.bpm,
            length,
            steps,
            loop_source: old.loop_source,
            effects: old.effects,
            ..Self::default()
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.bpm.is_finite() || !(30.0..=300.0).contains(&self.bpm) {
            return Err("Tempo must be from 30 to 300 BPM.".into());
        }
        if self.steps.len() != STEPS
            || self
                .steps
                .iter()
                .any(|step| matches!(step.trig, Trig::Note(note) if !NOTE_RANGE.contains(&note)))
        {
            return Err(format!(
                "The generator must have {STEPS} steps with notes from -48 to 48 semitones."
            ));
        }
        if self
            .steps
            .iter()
            .any(|step| !VELOCITY_RANGE.contains(&step.velocity))
        {
            return Err("Each step's velocity must be from 1 to 100.".into());
        }
        if !LENGTH_RANGE.contains(&self.length) {
            return Err("The pattern length must be from 1 to 64 steps.".into());
        }
        if !GATE_RANGE.contains(&self.gate) {
            return Err("The gate must be from 5 to 100 percent.".into());
        }
        if !SWING_RANGE.contains(&self.swing) {
            return Err("Swing must be from 50 to 75 percent.".into());
        }
        if !self.accent_db.is_finite() || !ACCENT_DB_RANGE.contains(&self.accent_db) {
            return Err("The accent level must be from 0 to 12 dB.".into());
        }
        if !self.slide_ms.is_finite() || !SLIDE_MS_RANGE.contains(&self.slide_ms) {
            return Err("The slide time must be from 1 to 1000 ms.".into());
        }
        if self.root > 11 {
            return Err("The scale root must be from 0 to 11 semitones.".into());
        }
        if !self.start_ms.is_finite() || !START_MS_RANGE.contains(&self.start_ms) {
            return Err("The source start must be from 0 to 2000 ms.".into());
        }
        self.effects.validate()
    }

    /// The steps one render covers: the pattern repeats until it lines up with
    /// the bar again, so the WAV loops seamlessly. Twelve steps give 48.
    pub fn sequence_steps(&self) -> usize {
        let length = usize::from(self.length.max(1));
        length / gcd(length, BAR_STEPS) * BAR_STEPS
    }

    fn step_at(&self, index: usize) -> Step {
        self.steps[index % usize::from(self.length.max(1))]
    }

    /// When step `index` of the sequence starts, in steps. Swing delays every
    /// second sixteenth.
    fn step_time(&self, index: usize) -> f64 {
        let swing = f64::from(self.swing.saturating_sub(50)) / 50.0;
        index as f64 + if index % 2 == 1 { swing } else { 0.0 }
    }

    /// Every note the sequence plays, in order.
    pub fn notes(&self) -> Vec<Note> {
        let total = self.sequence_steps();
        let gate = f64::from(self.gate) / 100.0;
        let accent = 10f64.powf(self.accent_db / 20.0);
        let mut notes = Vec::new();
        let mut slide_in = false;
        let mut index = 0;
        while index < total {
            let step = self.step_at(index);
            let Trig::Note(note) = step.trig else {
                slide_in = false;
                index += 1;
                continue;
            };
            let mut next = index + 1;
            while next < total && self.step_at(next).trig == Trig::Tie {
                next += 1;
            }
            let legato =
                step.slide && next < total && matches!(self.step_at(next).trig, Trig::Note(_));
            let last = self.step_time(next - 1);
            notes.push(Note {
                start: self.step_time(index),
                end: if legato {
                    self.step_time(next)
                } else {
                    last + gate * (self.step_time(next) - last)
                },
                note,
                gain: f64::from(step.velocity) / 100.0 * if step.accent { accent } else { 1.0 },
                slide_in,
            });
            slide_in = legato;
            index = next;
        }
        notes
    }
}

/// A note of the rendered sequence. Times are in steps from the start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    pub start: f64,
    pub end: f64,
    pub note: i8,
    pub gain: f64,
    /// The note before it glides into this one without restarting the source.
    pub slide_in: bool,
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Where Save sound keeps a sound's pattern: beside its WAV, as
/// `Name.pattern.json` for `Name.wav`, in the format Load pattern reads. The
/// arrangement draws a clip of that sound as notes.
pub fn pattern_beside(wav: &std::path::Path) -> std::path::PathBuf {
    wav.with_extension("pattern.json")
}

pub fn render(source: &Pcm, state: &StepSynthState) -> Result<Pcm, String> {
    render_measured(source, state).map(|(pcm, _)| pcm)
}

/// Render the pattern, and say by how many dB the render was turned down to
/// stay within full scale: 0 when it fits. Accents, velocity, and the effects
/// can raise a normalized source above full scale; the whole render is then
/// turned down, so no note hard-clips and the accents keep their contrast.
pub fn render_measured(source: &Pcm, state: &StepSynthState) -> Result<(Pcm, f64), String> {
    source.validate()?;
    state.validate()?;
    if source.frames() == 0 {
        return Err("Choose a nonempty sample.".into());
    }
    let sample_rate = f64::from(source.sample_rate);
    let frames_per_step = sample_rate * 60.0 / state.bpm / 4.0;
    let frame = |steps: f64| (steps * frames_per_step).round() as usize;
    // The longest sequence, 63 steps repeated to 1,008, lasts about 8 minutes
    // at 30 BPM. That fits a WAV file at any supported rate.
    let total_frames = frame(state.sequence_steps() as f64);
    let mut samples = vec![0.0; total_frames];
    let source_frames = source.frames() as usize;
    let channel_count = usize::from(source.channels);
    let mono = |frame: usize| {
        source.samples[frame * channel_count..(frame + 1) * channel_count]
            .iter()
            .sum::<f32>()
            / channel_count as f32
    };
    // Each note reads the source from the start; a looped source repeats
    // from there too.
    let start_frame = (state.start_ms / 1000.0 * sample_rate).round();
    if start_frame >= source_frames as f64 {
        return Err(format!(
            "The source start, {:.0} ms, is past the end of the source.",
            state.start_ms
        ));
    }
    let loop_frames = source_frames as f64 - start_frame;
    let fade_frames = (END_FADE_SECONDS * sample_rate).round().max(1.0);
    let slide_frames = (state.slide_ms / 1000.0 * sample_rate).max(1.0);
    let notes = state.notes();
    let mut first = 0;
    while first < notes.len() {
        // A phrase is a note and the notes it slides into: the source plays
        // on through it without restarting.
        let mut after = first + 1;
        while after < notes.len() && notes[after].slide_in {
            after += 1;
        }
        let phrase = &notes[first..after];
        first = after;
        let start = frame(phrase[0].start);
        let end = frame(phrase[phrase.len() - 1].end).min(total_frames);
        let mut written_end = start.min(end);
        let mut position = start_frame;
        let mut current = 0;
        let mut slide_from = (f64::from(phrase[0].note), phrase[0].gain);
        let interpolate = |from: (f64, f64), to: Note, t: f64| {
            (
                from.0 + (f64::from(to.note) - from.0) * t,
                from.1 + (to.gain - from.1) * t,
            )
        };
        for (index, output) in (start..end).zip(&mut samples[start.min(end)..end]) {
            while current + 1 < phrase.len() && index >= frame(phrase[current + 1].start) {
                let before = phrase[current];
                let t = ((frame(phrase[current + 1].start) - frame(before.start)) as f64
                    / slide_frames)
                    .min(1.0);
                // Retarget an unfinished glide from its current pitch and
                // level, rather than jumping to the previous target.
                slide_from = if current == 0 {
                    (f64::from(before.note), before.gain)
                } else {
                    interpolate(slide_from, before, t)
                };
                current += 1;
            }
            let note = phrase[current];
            let (semitones, gain) = if current > 0 {
                let t = ((index - frame(note.start)) as f64 / slide_frames).min(1.0);
                interpolate(slide_from, note, t)
            } else {
                (f64::from(note.note), note.gain)
            };
            if state.loop_source && position >= source_frames as f64 {
                position = start_frame + (position - start_frame) % loop_frames;
            }
            if position >= source_frames as f64 {
                break;
            }
            let first = position.floor() as usize;
            let next = if state.loop_source && first + 1 >= source_frames {
                start_frame as usize
            } else if state.loop_source {
                first + 1
            } else {
                (first + 1).min(source_frames - 1)
            };
            let a = mono(first);
            let value = a + (mono(next) - a) * (position - first as f64) as f32;
            *output = value * gain as f32;
            written_end = index + 1;
            position += 2f64.powf(semitones / 12.0);
        }
        // The source can run out before the gate or the end of a slide
        // phrase. Fade where audio actually ends in either case.
        let fade_start = written_end
            .saturating_sub(fade_frames as usize)
            .max(start.min(end));
        for (index, output) in (fade_start..written_end).zip(&mut samples[fade_start..written_end])
        {
            *output *= ((written_end - index) as f64 / fade_frames) as f32;
        }
    }
    let (pcm, gain) = effects::process_synth(
        &Pcm {
            sample_rate: source.sample_rate,
            channels: 1,
            samples,
        },
        &state.effects,
        state.bpm,
    )?;
    Ok((pcm, -20.0 * f64::from(gain).log10()))
}

/// Where the source's first hit starts, in whole ms for the Start setting:
/// the first frame above a twentieth of the peak, less 1 ms so the attack
/// stays whole. A silent source gives 0.
pub fn first_hit(source: &Pcm) -> f64 {
    let channels = usize::from(source.channels.max(1));
    let peak = source
        .samples
        .iter()
        .fold(0.0_f32, |peak, x| peak.max(x.abs()));
    if peak <= 0.0 || source.sample_rate == 0 {
        return 0.0;
    }
    let Some(index) = source.samples.iter().position(|x| x.abs() > peak / 20.0) else {
        return 0.0;
    };
    let ms = (index / channels) as f64 * 1000.0 / f64::from(source.sample_rate) - 1.0;
    ms.floor()
        .clamp(*START_MS_RANGE.start(), *START_MS_RANGE.end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_source() -> Pcm {
        Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.5; 4000],
        }
    }

    fn rest_pattern() -> StepSynthState {
        StepSynthState {
            bpm: 120.0,
            steps: vec![Step::default(); STEPS],
            ..Default::default()
        }
    }

    #[test]
    fn a_note_sounds_for_the_gate_of_its_step_and_fades_out() {
        // 120 BPM at 1000 Hz: 125 frames per step, a 3-frame end fade.
        let mut state = rest_pattern();
        state.steps[0] = Step::note(0);
        state.gate = 50;
        let output = render(&flat_source(), &state).unwrap();
        assert_eq!(output.frames(), 2000);
        assert!(output.samples[..61].iter().all(|x| *x == 0.5));
        assert!(output.samples[61] < 0.5 && output.samples[62] > 0.0);
        assert!(output.samples[63..].iter().all(|x| *x == 0.0));
    }

    #[test]
    fn source_exhaustion_fades_at_the_actual_end_including_slides() {
        for (pitch, slide) in [(0, false), (12, false), (0, true)] {
            let source = Pcm {
                sample_rate: 1000,
                channels: 1,
                samples: vec![0.5; if slide { 200 } else { 100 }],
            };
            let mut state = rest_pattern();
            state.steps[0] = Step {
                slide,
                ..Step::note(pitch)
            };
            if slide {
                state.steps[1] = Step::note(12);
            }
            let output = render(&source, &state).unwrap();
            let end = output.samples.iter().rposition(|x| *x != 0.0).unwrap() + 1;
            assert!(end < if slide { 250 } else { 125 });
            assert_eq!(output.samples[end - 3], 0.5);
            assert!((output.samples[end - 2] - 1.0 / 3.0).abs() < 1e-6);
            assert!((output.samples[end - 1] - 1.0 / 6.0).abs() < 1e-6);
            assert!(output.samples[end..].iter().all(|x| *x == 0.0));

            state.loop_source = true;
            let output = render(&source, &state).unwrap();
            let scheduled_end = if slide { 250 } else { 125 };
            assert_eq!(output.samples[scheduled_end - 3], 0.5);
            assert!(output.samples[scheduled_end - 1] > 0.0);
            assert_eq!(output.samples[scheduled_end], 0.0);
        }
    }

    #[test]
    fn ties_hold_a_note_and_a_new_note_restarts_the_source() {
        let source = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: (0..1000).map(|i| i as f32 / 1000.0).collect(),
        };
        let mut state = rest_pattern();
        state.steps[0] = Step::note(0);
        state.steps[1] = Step::tie();
        state.steps[2] = Step::note(12);
        for step in &mut state.steps[3..8] {
            *step = Step::tie();
        }
        let output = render(&source, &state).unwrap();
        // The tie keeps reading: frame 200 is 0.2, not a restart.
        assert_eq!(output.samples[200], 0.2);
        assert_eq!(output.samples[250], 0.0);
        assert_eq!(output.samples[251], 0.002);
        // An octave up reads twice as fast and runs out after 500 frames.
        assert!(output.samples[747] > 0.99);
        assert!(output.samples[749] > 0.0 && output.samples[749] < 0.34);
        assert_eq!(output.samples[750], 0.0);
    }

    #[test]
    fn a_slide_glides_into_the_next_note_without_restarting() {
        let source = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: (0..1000).map(|i| i as f32 / 1000.0).collect(),
        };
        let mut state = rest_pattern();
        state.steps[0] = Step {
            slide: true,
            ..Step::note(0)
        };
        state.steps[1] = Step::note(12);
        state.slide_ms = 100.0;
        let notes = state.notes();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].end, 1.0);
        assert!(notes[1].slide_in);
        let output = render(&source, &state).unwrap();
        // No restart at frame 125, and halfway through the glide the source
        // reads faster than at the original pitch.
        assert!(output.samples[125] > 0.12);
        let step = output.samples[176] - output.samples[175];
        assert!(step > 0.0013 && step < 0.002, "{step}");
    }

    #[test]
    fn unfinished_slides_retarget_the_current_pitch_and_level() {
        let source = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: (0..10000).map(|i| i as f32 / 10000.0).collect(),
        };
        let mut state = rest_pattern();
        state.slide_ms = 1000.0;
        for (index, note) in [0, 12, 24].into_iter().enumerate() {
            state.steps[index] = Step {
                slide: index < 2,
                ..Step::note(note)
            };
        }
        let output = render(&source, &state).unwrap();
        // The first glide has reached only 1.5 semitones when the third
        // note starts at frame 250. Its next source step must continue there.
        let increment = output.samples[251] - output.samples[250];
        let expected = 2f32.powf(1.5 / 12.0) / 10000.0;
        assert!((increment - expected).abs() < 1e-8, "{increment}");

        state.steps[0].velocity = 20;
        state.steps[1].velocity = 100;
        state.steps[2].velocity = 40;
        let output = render(&flat_source(), &state).unwrap();
        // The level has moved one eighth of the way from 0.2 to 1.0.
        assert!((output.samples[250] - 0.15).abs() < 1e-6);
        assert!((output.samples[250] - output.samples[249]).abs() < 0.001);
    }

    #[test]
    fn accent_and_velocity_set_the_level() {
        let mut state = rest_pattern();
        state.steps[0] = Step {
            velocity: 50,
            ..Step::note(0)
        };
        state.steps[1] = Step {
            accent: true,
            ..Step::note(0)
        };
        state.accent_db = 6.0;
        let output = render(&flat_source(), &state).unwrap();
        assert_eq!(output.samples[10], 0.25);
        assert!((output.samples[135] - 0.5 * 10f32.powf(0.3)).abs() < 1e-6);
    }

    #[test]
    fn a_loud_render_is_turned_down_whole_instead_of_clipping() {
        // A full-scale source with a 6 dB accent peaks at twice full scale.
        let mut source = flat_source();
        source.samples.fill(1.0);
        let mut state = rest_pattern();
        state.steps[0] = Step::note(0);
        state.steps[1] = Step {
            accent: true,
            ..Step::note(0)
        };
        state.accent_db = 6.0;
        let (output, turned_down) = render_measured(&source, &state).unwrap();
        let accent = 10f64.powf(0.3);
        assert!(
            (turned_down - 20.0 * accent.log10()).abs() < 1e-4,
            "{turned_down}"
        );
        // The accented note reaches full scale; the plain one keeps the
        // accent's contrast below it.
        assert!((output.samples[135] - 1.0).abs() < 1e-6);
        assert!((output.samples[10] - (1.0 / accent) as f32).abs() < 1e-5);
        let full_scale = output.samples.iter().filter(|x| x.abs() >= 1.0).count();
        assert!(
            full_scale < 125,
            "Only the accented step reaches full scale."
        );

        state.steps[1].accent = false;
        let (output, turned_down) = render_measured(&source, &state).unwrap();
        assert_eq!(turned_down, 0.0);
        assert_eq!(output.samples[10], 1.0);
    }

    #[test]
    fn full_accent_keeps_headroom_until_the_effects_output_gain() {
        let mut source = flat_source();
        source.samples.fill(0.8);
        let mut state = rest_pattern();
        state.steps[0] = Step {
            accent: true,
            ..Step::note(0)
        };
        state.accent_db = 12.0;
        state.effects.volume = 0.2;
        let output = render(&source, &state).unwrap();
        let expected = 0.8 * 10f32.powf(12.0 / 20.0) * 0.2;
        assert!((output.samples[10] - expected).abs() < 1e-6);
        output.validate().unwrap();

        state.effects.volume = 1.0;
        let output = render(&source, &state).unwrap();
        assert_eq!(output.samples[10], 1.0);
        output.validate().unwrap();
        source.samples[0] = 1.0;
        assert!(render(&source, &state).is_ok());
        source.samples[0] = 1.01;
        assert!(render(&source, &state).is_err());
    }

    #[test]
    fn swing_delays_every_second_sixteenth() {
        let mut state = rest_pattern();
        state.steps[1] = Step::note(0);
        state.swing = 75;
        let notes = state.notes();
        assert_eq!((notes[0].start, notes[0].end), (1.5, 2.0));
        let output = render(&flat_source(), &state).unwrap();
        assert_eq!(output.samples[186], 0.0);
        assert_eq!(output.samples[188], 0.5);
    }

    #[test]
    fn an_odd_length_renders_until_it_lines_up_with_the_bar() {
        let mut state = rest_pattern();
        state.length = 12;
        state.steps[0] = Step::note(0);
        state.steps[20] = Step::note(5);
        assert_eq!(state.sequence_steps(), 48);
        let starts: Vec<f64> = state.notes().iter().map(|note| note.start).collect();
        assert_eq!(starts, vec![0.0, 12.0, 24.0, 36.0]);
        assert_eq!(render(&flat_source(), &state).unwrap().frames(), 6000);
        state.length = 63;
        assert_eq!(state.sequence_steps(), 1008);
        state.length = 64;
        assert_eq!(state.sequence_steps(), 64);
    }

    #[test]
    fn old_patterns_preserve_note_timing_and_bar_contents() {
        let mut steps = vec![None; 64];
        steps[0] = Some(0);
        steps[4] = Some(7);
        steps[32] = Some(-3);
        steps[48] = Some(12);
        let old = serde_json::json!({
            "bpm": 140.0,
            "active_bars": [true, false, true, false],
            "steps": steps,
            "cut_sixteenth": false,
            "loop_source": true,
            "effects": EffectsState::default(),
        });
        let state: StepSynthState = serde_json::from_value(old.clone()).unwrap();
        state.validate().unwrap();
        assert_eq!(state.length, 32);
        assert!(state.loop_source);
        // Bar 1, then bar 3, then the inactive bars 2 and 4.
        assert_eq!(state.steps[0], Step::note(0));
        assert_eq!(state.steps[1], Step::tie());
        assert_eq!(state.steps[4], Step::note(7));
        assert_eq!(state.steps[16], Step::note(-3));
        assert_eq!(state.steps[31], Step::tie());
        assert_eq!(state.steps[32], Step::default());
        assert_eq!(state.steps[48], Step::note(12));
        assert_eq!(state.steps[63], Step::tie());
        let notes = state.notes();
        assert_eq!(
            notes
                .iter()
                .map(|note| (note.start, note.end, note.note))
                .collect::<Vec<_>>(),
            vec![(0.0, 4.0, 0), (4.0, 16.0, 7), (16.0, 32.0, -3)]
        );

        let mut cut = old;
        cut["cut_sixteenth"] = true.into();
        let state: StepSynthState = serde_json::from_value(cut).unwrap();
        assert_eq!(state.steps[1], Step::default());
        assert_eq!(state.notes()[0].end, 1.0);
    }

    #[test]
    fn old_patterns_without_an_active_bar_stay_invalid() {
        let old = serde_json::json!({
            "bpm": 140.0,
            "active_bars": vec![false; 4],
            "steps": vec![Some(0); STEPS],
            "cut_sixteenth": false,
            "loop_source": false,
            "effects": EffectsState::default(),
        });
        let error = serde_json::from_value::<StepSynthState>(old).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Switch on at least one Active bar.")
        );
    }

    #[test]
    fn serialized_state_roundtrips_and_invalid_values_are_rejected() {
        let mut state = StepSynthState::default();
        state.steps[3] = Step {
            accent: true,
            slide: true,
            velocity: 40,
            ..Step::note(-5)
        };
        state.steps[4] = Step::tie();
        let saved = serde_json::to_vec(&state).unwrap();
        assert_eq!(
            serde_json::from_slice::<StepSynthState>(&saved).unwrap(),
            state
        );
        let text = String::from_utf8(saved).unwrap();
        assert!(text.contains(r#"{"trig":"tie"}"#), "{text}");
        assert!(
            text.contains("{},{}"),
            "A rest saves as an empty step: {text}"
        );

        for change in [
            (|s: &mut StepSynthState| s.steps[0] = Step::note(49)) as fn(&mut StepSynthState),
            |s| s.steps[0].velocity = 0,
            |s| s.length = 0,
            |s| s.length = 65,
            |s| s.gate = 4,
            |s| s.swing = 76,
            |s| s.accent_db = 13.0,
            |s| s.slide_ms = 0.5,
            |s| s.root = 12,
            |s| s.start_ms = -1.0,
            |s| s.start_ms = 2001.0,
            |s| {
                s.steps.pop();
            },
        ] {
            let mut bad = state.clone();
            change(&mut bad);
            assert!(bad.validate().is_err());
        }
        let mut unknown = serde_json::to_value(&state).unwrap();
        unknown["extra"] = 1.into();
        assert!(serde_json::from_value::<StepSynthState>(unknown).is_err());

        // The scale, root, start, and source save with the pattern; a
        // pattern saved before them loads with the defaults.
        state.scale = edit::Scale::Dorian;
        state.root = 5;
        state.start_ms = 12.0;
        state.source = Some(PathBuf::from("kick.wav"));
        let saved = serde_json::to_value(&state).unwrap();
        assert_eq!(saved["scale"], "dorian");
        assert_eq!(
            serde_json::from_value::<StepSynthState>(saved.clone()).unwrap(),
            state
        );
        let mut old = saved;
        for field in ["scale", "root", "start_ms", "source"] {
            old.as_object_mut().unwrap().remove(field);
        }
        let old = serde_json::from_value::<StepSynthState>(old).unwrap();
        assert_eq!(
            (old.scale, old.root, old.start_ms, old.source),
            (edit::Scale::Minor, 0, 0.0, None)
        );
        let text = serde_json::to_string(&StepSynthState::default()).unwrap();
        assert!(!text.contains("\"source\""), "{text}");
    }

    #[test]
    fn the_start_skips_the_source_head_and_loops_from_it() {
        let source = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: (0..100).map(|i| i as f32 / 100.0).collect(),
        };
        let mut state = rest_pattern();
        state.steps[0] = Step::note(0);
        state.gate = 100;
        state.start_ms = 40.0;
        let output = render(&source, &state).unwrap();
        assert_eq!(output.samples[0], 0.4);
        // Without a loop the 60 frames after the start run out.
        assert_eq!(output.samples[60], 0.0);
        state.loop_source = true;
        let output = render(&source, &state).unwrap();
        assert_eq!(output.samples[59], 0.99);
        assert_eq!(output.samples[60], 0.4);
        state.start_ms = 100.0;
        assert!(
            render(&source, &state)
                .unwrap_err()
                .contains("past the end")
        );
    }

    #[test]
    fn the_first_hit_is_found_after_leading_silence() {
        let mut samples = vec![0.0; 2 * 1000];
        samples[2 * 30] = 0.01;
        samples[2 * 50 + 1] = -0.8;
        let source = Pcm {
            sample_rate: 1000,
            channels: 2,
            samples,
        };
        assert_eq!(first_hit(&source), 49.0);
        let silent = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.0; 10],
        };
        assert_eq!(first_hit(&silent), 0.0);
    }
}
