//! Manual-derived StepSynth sequencer state. Resampling, source looping,
//! and DSP are explicit reconstruction policies, not original engine parity.
use crate::{
    audio::Pcm,
    effects::{self, EffectsState},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepSynthState {
    pub bpm: f64,
    pub active_bars: [bool; 4],
    /// One optional note per sixteenth step; zero is the source's original pitch.
    pub steps: Vec<Option<i8>>,
    pub cut_sixteenth: bool,
    /// Optional source looping is a reconstruction setting. Original loop points
    /// and generator sample envelopes have not been recovered.
    pub loop_source: bool,
    pub effects: EffectsState,
}
impl Default for StepSynthState {
    fn default() -> Self {
        let mut steps = vec![None; 64];
        steps[0] = Some(0);
        Self {
            bpm: 140.0,
            active_bars: [true, false, false, false],
            steps,
            cut_sixteenth: false,
            loop_source: false,
            effects: EffectsState::default(),
        }
    }
}
impl StepSynthState {
    pub fn validate(&self) -> Result<(), String> {
        if !self.bpm.is_finite() || !(30.0..=300.0).contains(&self.bpm) {
            return Err("Tempo must be from 30 to 300 BPM.".into());
        }
        if self.steps.len() != 64
            || self
                .steps
                .iter()
                .flatten()
                .any(|note| !(-48..=48).contains(note))
        {
            return Err(
                "The generator must have 64 steps with notes from -48 to 48 semitones.".into(),
            );
        }
        if !self.active_bars.iter().any(|active| *active) {
            return Err("Select at least one generator bar.".into());
        }
        self.effects.validate()
    }
}

pub fn render(source: &Pcm, state: &StepSynthState) -> Result<Pcm, String> {
    source.validate()?;
    state.validate()?;
    if source.frames() == 0 || source.frames() > 64_000_000 {
        return Err("Choose a nonempty sample with a supported size.".into());
    }
    let notes: Vec<_> = state
        .active_bars
        .iter()
        .enumerate()
        .filter(|(_, active)| **active)
        .flat_map(|(bar, _)| state.steps[bar * 16..bar * 16 + 16].iter().copied())
        .collect();
    let frames_per_step = source.sample_rate as f64 * 60.0 / state.bpm / 4.0;
    let total_frames = (notes.len() as f64 * frames_per_step).round() as usize;
    if total_frames > 32_000_000 {
        return Err("The generated sequence is too large.".into());
    }
    let mut samples = vec![0.0; total_frames];
    let source_frames = source.frames() as usize;
    let channel_count = source.channels as usize;
    let mono = |frame: usize| {
        source.samples[frame * channel_count..(frame + 1) * channel_count]
            .iter()
            .sum::<f32>()
            / channel_count as f32
    };
    for (step, note) in notes.iter().enumerate() {
        let Some(note) = note else {
            continue;
        };
        let next_note = if state.cut_sixteenth {
            step + 1
        } else {
            notes
                .iter()
                .enumerate()
                .skip(step + 1)
                .find(|(_, note)| note.is_some())
                .map_or(notes.len(), |(index, _)| index)
        };
        let start = (step as f64 * frames_per_step).round() as usize;
        let end = (next_note as f64 * frames_per_step).round() as usize;
        let pitch = 2.0f64.powf(*note as f64 / 12.0);
        for (index, output) in samples[start..end.min(total_frames)].iter_mut().enumerate() {
            let mut position = index as f64 * pitch;
            if state.loop_source {
                position %= source_frames as f64;
            }
            if position >= source_frames as f64 {
                break;
            }
            let first = position.floor() as usize;
            let next = if state.loop_source {
                (first + 1) % source_frames
            } else {
                (first + 1).min(source_frames - 1)
            };
            let a = mono(first);
            *output = a + (mono(next) - a) * (position - first as f64) as f32;
        }
    }
    effects::process(
        &Pcm {
            sample_rate: source.sample_rate,
            channels: 1,
            samples,
        },
        &state.effects,
        state.bpm,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_bars_concatenate_and_cut_mode_gates_one_sixteenth() {
        let source = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.5; 4000],
        };
        let mut state = StepSynthState {
            bpm: 120.0,
            active_bars: [true, false, true, false],
            cut_sixteenth: true,
            ..Default::default()
        };
        state.steps[32] = Some(0);
        let output = render(&source, &state).unwrap();
        assert_eq!(output.frames(), 4000);
        assert!(output.samples[..125].iter().all(|x| *x == 0.5));
        assert!(output.samples[125..2000].iter().all(|x| *x == 0.0));
        assert!(output.samples[2000..2125].iter().all(|x| *x == 0.5));
    }
    #[test]
    fn next_note_retriggers_monophonic_source_and_octave_doubles_speed() {
        let source = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: (0..1000).map(|i| i as f32 / 1000.0).collect(),
        };
        let mut state = StepSynthState {
            bpm: 120.0,
            ..Default::default()
        };
        state.steps[1] = Some(12);
        let output = render(&source, &state).unwrap();
        assert_eq!(output.samples[124], 0.124);
        assert_eq!(output.samples[125], 0.0);
        assert_eq!(output.samples[126], 0.002);
        assert_eq!(output.samples[625], 0.0);
    }
    #[test]
    fn serialized_state_roundtrips_and_invalid_notes_are_rejected() {
        let state = StepSynthState::default();
        let saved = serde_json::to_vec(&state).unwrap();
        assert_eq!(
            serde_json::from_slice::<StepSynthState>(&saved).unwrap(),
            state
        );
        let mut bad = state;
        bad.steps[0] = Some(49);
        assert!(bad.validate().is_err());
    }
}
