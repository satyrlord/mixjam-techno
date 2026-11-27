//! Step synth patterns and their render. The step model (note, tie, or rest,
//! with accent and slide), gate, swing, resampling, source looping, and DSP
//! are the application's own policies.
use crate::{audio::Pcm, effects::EffectsState};
use serde::{Deserialize, Serialize};
use std::{ops::RangeInclusive, path::PathBuf};

pub mod edit;
#[cfg(test)]
mod edit_tests;
pub mod history;
pub mod osc;
pub mod presets;
mod render;
#[cfg(test)]
mod tests;

pub use presets::{StepSynthPreset, matching_step_synth_preset, step_synth_presets};

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
/// Base octave shift for built-in oscillator synthesis around C2.
pub const OSC_OCTAVE_RANGE: RangeInclusive<i8> = -2..=2;
/// Exponential amp decay range in ms (`0.0` = Off / full gate).
pub const AMP_DECAY_MS_RANGE: RangeInclusive<f64> = 15.0..=2000.0;
/// Filter envelope decay range in ms.
pub const FILTER_ENV_DECAY_MS_RANGE: RangeInclusive<f64> = 20.0..=2000.0;
/// Default filter envelope decay time in ms.
pub const DEFAULT_FILTER_ENV_DECAY_MS: f64 = 220.0;
/// Sample rate used for standalone oscillator rendering.
pub const SYNTH_SAMPLE_RATE: u32 = crate::audio::Mix::SAMPLE_RATE;
/// The fade at the end of every note that does not slide, so a cut never
/// clicks.
const END_FADE_SECONDS: f64 = 0.003;

/// Whether Step Synth triggers a WAV sample or its built-in anti-aliased oscillator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceMode {
    #[default]
    Sample,
    Synth,
}

/// Primary waveform in standalone Synth mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OscWaveform {
    #[default]
    Saw,
    Square,
    Triangle,
}

fn is_default_source_mode(v: &SourceMode) -> bool {
    *v == SourceMode::Sample
}
fn is_default_osc_waveform(v: &OscWaveform) -> bool {
    *v == OscWaveform::Saw
}
fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}
fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}
fn is_zero_i8(v: &i8) -> bool {
    *v == 0
}
fn default_filter_env_decay_ms() -> f64 {
    DEFAULT_FILTER_ENV_DECAY_MS
}
fn is_default_filter_env_decay(v: &f64) -> bool {
    (*v - DEFAULT_FILTER_ENV_DECAY_MS).abs() < 1e-9
}

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
    #[serde(default, skip_serializing_if = "is_default_source_mode")]
    pub source_mode: SourceMode,
    #[serde(default, skip_serializing_if = "is_default_osc_waveform")]
    pub osc_waveform: OscWaveform,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub osc_sub: f32,
    #[serde(default, skip_serializing_if = "is_zero_i8")]
    pub osc_octave: i8,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub amp_decay_ms: f64,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub filter_env_amount: f32,
    #[serde(
        default = "default_filter_env_decay_ms",
        skip_serializing_if = "is_default_filter_env_decay"
    )]
    pub filter_env_decay_ms: f64,
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
            source_mode: SourceMode::default(),
            osc_waveform: OscWaveform::default(),
            osc_sub: 0.0,
            osc_octave: 0,
            amp_decay_ms: 0.0,
            filter_env_amount: 0.0,
            filter_env_decay_ms: DEFAULT_FILTER_ENV_DECAY_MS,
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
    #[serde(default)]
    source_mode: SourceMode,
    #[serde(default)]
    osc_waveform: OscWaveform,
    #[serde(default)]
    osc_sub: f32,
    #[serde(default)]
    osc_octave: i8,
    #[serde(default)]
    amp_decay_ms: f64,
    #[serde(default)]
    filter_env_amount: f32,
    #[serde(default = "default_filter_env_decay_ms")]
    filter_env_decay_ms: f64,
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
            source_mode: stored.source_mode,
            osc_waveform: stored.osc_waveform,
            osc_sub: stored.osc_sub,
            osc_octave: stored.osc_octave,
            amp_decay_ms: stored.amp_decay_ms,
            filter_env_amount: stored.filter_env_amount,
            filter_env_decay_ms: stored.filter_env_decay_ms,
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

    fn validate_steps(&self) -> Result<(), String> {
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
        Ok(())
    }

    fn validate_osc(&self) -> Result<(), String> {
        if !self.osc_sub.is_finite() || !(0.0..=1.0).contains(&self.osc_sub) {
            return Err("Sub oscillator level must be from 0 to 100 percent.".into());
        }
        if !OSC_OCTAVE_RANGE.contains(&self.osc_octave) {
            return Err("Oscillator octave must be from -2 to +2.".into());
        }
        Ok(())
    }

    fn validate_envelopes(&self) -> Result<(), String> {
        let amp_ok = self.amp_decay_ms == 0.0 || AMP_DECAY_MS_RANGE.contains(&self.amp_decay_ms);
        if !self.amp_decay_ms.is_finite() || !amp_ok {
            return Err("Amp decay must be Off (0) or from 15 to 2000 ms.".into());
        }
        if !self.filter_env_amount.is_finite() || !(-1.0..=1.0).contains(&self.filter_env_amount) {
            return Err("Filter envelope depth must be from -100 to +100 percent.".into());
        }
        if !self.filter_env_decay_ms.is_finite()
            || !FILTER_ENV_DECAY_MS_RANGE.contains(&self.filter_env_decay_ms)
        {
            return Err("Filter envelope decay must be from 20 to 2000 ms.".into());
        }
        Ok(())
    }

    fn validate_rhythm(&self) -> Result<(), String> {
        if !self.bpm.is_finite() || !(30.0..=300.0).contains(&self.bpm) {
            return Err("Tempo must be from 30 to 300 BPM.".into());
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
        Ok(())
    }

    fn validate_expression(&self) -> Result<(), String> {
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
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_rhythm()?;
        self.validate_steps()?;
        self.validate_expression()?;
        self.validate_osc()?;
        self.validate_envelopes()?;
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
                accent: step.accent,
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
    pub accent: bool,
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
    if state.source_mode == SourceMode::Synth {
        source.validate()?;
        return render::render_measured(None, state, source.sample_rate.max(1));
    }
    source.validate()?;
    if source.frames() == 0 {
        return Err("Choose a nonempty sample.".into());
    }
    render::render_measured(Some(source), state, source.sample_rate)
}

/// Render a pattern in standalone `SourceMode::Synth` at the mix sample rate.
pub fn render_standalone(state: &StepSynthState) -> Result<Pcm, String> {
    render_standalone_measured(state).map(|(pcm, _)| pcm)
}

/// Render a standalone synth pattern and return the turned-down attenuation in dB.
pub fn render_standalone_measured(state: &StepSynthState) -> Result<(Pcm, f64), String> {
    if state.source_mode != SourceMode::Synth {
        return Err("Standalone synth rendering requires Synth source mode.".into());
    }
    render::render_measured(None, state, SYNTH_SAMPLE_RATE)
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
