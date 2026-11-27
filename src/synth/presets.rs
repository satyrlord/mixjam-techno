//! Curated Step Synth presets that set Voice + Effects while preserving user notes.
use super::{OscWaveform, SourceMode, StepSynthState};
use crate::effects::{
    DistortionShape, Echo, EffectsState, Filter, FilterMode, Modulation, ModulationMode, Reverb,
    ReverbSpace, Robotize, RobotizeMode,
};

#[derive(Clone, Debug)]
pub struct StepSynthPreset {
    pub name: &'static str,
    pub source_mode: SourceMode,
    pub osc_waveform: OscWaveform,
    pub osc_sub: f32,
    pub osc_octave: i8,
    pub amp_decay_ms: f64,
    pub filter_env_amount: f32,
    pub filter_env_decay_ms: f64,
    pub accent_db: f64,
    pub slide_ms: f64,
    pub effects: EffectsState,
}

impl StepSynthPreset {
    pub fn apply(&self, state: &mut StepSynthState) {
        state.source_mode = self.source_mode;
        state.osc_waveform = self.osc_waveform;
        state.osc_sub = self.osc_sub;
        state.osc_octave = self.osc_octave;
        state.amp_decay_ms = self.amp_decay_ms;
        state.filter_env_amount = self.filter_env_amount;
        state.filter_env_decay_ms = self.filter_env_decay_ms;
        state.accent_db = self.accent_db;
        state.slide_ms = self.slide_ms;
        state.effects = self.effects.clone();
    }

    fn matches(&self, state: &StepSynthState) -> bool {
        let discrete = (
            self.source_mode,
            self.osc_waveform,
            self.osc_octave,
            &self.effects,
        ) == (
            state.source_mode,
            state.osc_waveform,
            state.osc_octave,
            &state.effects,
        );
        let deltas = [
            f64::from((state.osc_sub - self.osc_sub).abs()),
            (state.amp_decay_ms - self.amp_decay_ms).abs(),
            f64::from((state.filter_env_amount - self.filter_env_amount).abs()),
            (state.filter_env_decay_ms - self.filter_env_decay_ms).abs(),
            (state.accent_db - self.accent_db).abs(),
            (state.slide_ms - self.slide_ms).abs(),
        ];
        discrete && deltas.into_iter().all(|d| d < 1e-3)
    }
}

pub fn matching_step_synth_preset(state: &StepSynthState) -> Option<&'static str> {
    step_synth_presets()
        .iter()
        .find(|p| p.matches(state))
        .map(|p| p.name)
}

pub fn step_synth_presets() -> Vec<StepSynthPreset> {
    vec![
        StepSynthPreset {
            name: "Clean Sample",
            source_mode: SourceMode::Sample,
            osc_waveform: OscWaveform::Saw,
            osc_sub: 0.0,
            osc_octave: 0,
            amp_decay_ms: 0.0,
            filter_env_amount: 0.0,
            filter_env_decay_ms: 220.0,
            accent_db: 6.0,
            slide_ms: 60.0,
            effects: EffectsState::default(),
        },
        StepSynthPreset {
            name: "303 Acid Saw",
            source_mode: SourceMode::Synth,
            osc_waveform: OscWaveform::Saw,
            osc_sub: 0.0,
            osc_octave: 0,
            amp_decay_ms: 380.0,
            filter_env_amount: 0.75,
            filter_env_decay_ms: 210.0,
            accent_db: 8.0,
            slide_ms: 65.0,
            effects: EffectsState {
                filter: Filter {
                    enabled: true,
                    mode: FilterMode::Lp24,
                    cutoff_hz: 320.0,
                    resonance: 0.76,
                    drive: 0.35,
                    wet: 1.0,
                },
                dist_shape: DistortionShape::Acid,
                dirty: 0.20,
                ..EffectsState::default()
            },
        },
        StepSynthPreset {
            name: "303 Hollow Square",
            source_mode: SourceMode::Synth,
            osc_waveform: OscWaveform::Square,
            osc_sub: 0.25,
            osc_octave: 0,
            amp_decay_ms: 300.0,
            filter_env_amount: 0.80,
            filter_env_decay_ms: 180.0,
            accent_db: 7.5,
            slide_ms: 60.0,
            effects: EffectsState {
                filter: Filter {
                    enabled: true,
                    mode: FilterMode::Lp24,
                    cutoff_hz: 280.0,
                    resonance: 0.72,
                    drive: 0.25,
                    wet: 1.0,
                },
                ..EffectsState::default()
            },
        },
        StepSynthPreset {
            name: "Sub Punch Bass",
            source_mode: SourceMode::Synth,
            osc_waveform: OscWaveform::Triangle,
            osc_sub: 0.65,
            osc_octave: 0,
            amp_decay_ms: 260.0,
            filter_env_amount: 0.35,
            filter_env_decay_ms: 160.0,
            accent_db: 6.0,
            slide_ms: 45.0,
            effects: EffectsState {
                filter: Filter {
                    enabled: true,
                    mode: FilterMode::Lp12,
                    cutoff_hz: 240.0,
                    resonance: 0.30,
                    drive: 0.15,
                    wet: 1.0,
                },
                dist_shape: DistortionShape::Warm,
                dirty: 0.30,
                ..EffectsState::default()
            },
        },
        StepSynthPreset {
            name: "Detuned Chorus Lead",
            source_mode: SourceMode::Synth,
            osc_waveform: OscWaveform::Saw,
            osc_sub: 0.15,
            osc_octave: 1,
            amp_decay_ms: 650.0,
            filter_env_amount: 0.40,
            filter_env_decay_ms: 340.0,
            accent_db: 5.0,
            slide_ms: 80.0,
            effects: EffectsState {
                filter: Filter {
                    enabled: true,
                    mode: FilterMode::Lp12,
                    cutoff_hz: 3200.0,
                    resonance: 0.25,
                    drive: 0.0,
                    wet: 1.0,
                },
                modulation: Modulation {
                    enabled: true,
                    mode: ModulationMode::Chorus,
                    rate_hz: 0.75,
                    depth: 0.70,
                    feedback: 0.25,
                    wet: 0.50,
                },
                echo: Echo {
                    enabled: true,
                    delay_beats: 0.375,
                    feedback: 0.45,
                    tone: 0.45,
                    ping_pong: true,
                    wet: 0.28,
                },
                reverb: Reverb {
                    enabled: true,
                    space: ReverbSpace::Hall,
                    decay_s: 2.4,
                    damping: 0.45,
                    wet: 0.25,
                },
                ..EffectsState::default()
            },
        },
        StepSynthPreset {
            name: "Dub Chord Stab",
            source_mode: SourceMode::Sample,
            osc_waveform: OscWaveform::Saw,
            osc_sub: 0.0,
            osc_octave: 0,
            amp_decay_ms: 0.0,
            filter_env_amount: 0.45,
            filter_env_decay_ms: 240.0,
            accent_db: 6.0,
            slide_ms: 60.0,
            effects: EffectsState {
                filter: Filter {
                    enabled: true,
                    mode: FilterMode::Lp12,
                    cutoff_hz: 1800.0,
                    resonance: 0.45,
                    drive: 0.15,
                    wet: 1.0,
                },
                echo: Echo {
                    enabled: true,
                    delay_beats: 0.375,
                    feedback: 0.60,
                    tone: 0.55,
                    ping_pong: true,
                    wet: 0.42,
                },
                reverb: Reverb {
                    enabled: true,
                    space: ReverbSpace::Plate,
                    decay_s: 2.0,
                    damping: 0.40,
                    wet: 0.28,
                },
                ..EffectsState::default()
            },
        },
        StepSynthPreset {
            name: "Pluck Percussion",
            source_mode: SourceMode::Synth,
            osc_waveform: OscWaveform::Triangle,
            osc_sub: 0.0,
            osc_octave: 1,
            amp_decay_ms: 140.0,
            filter_env_amount: 0.55,
            filter_env_decay_ms: 120.0,
            accent_db: 6.0,
            slide_ms: 35.0,
            effects: EffectsState {
                filter: Filter {
                    enabled: true,
                    mode: FilterMode::Lp12,
                    cutoff_hz: 620.0,
                    resonance: 0.20,
                    drive: 0.0,
                    wet: 1.0,
                },
                reverb: Reverb {
                    enabled: true,
                    space: ReverbSpace::Room,
                    decay_s: 0.9,
                    damping: 0.35,
                    wet: 0.22,
                },
                ..EffectsState::default()
            },
        },
        StepSynthPreset {
            name: "Industrial Crush",
            source_mode: SourceMode::Sample,
            osc_waveform: OscWaveform::Saw,
            osc_sub: 0.0,
            osc_octave: 0,
            amp_decay_ms: 0.0,
            filter_env_amount: 0.0,
            filter_env_decay_ms: 220.0,
            accent_db: 6.0,
            slide_ms: 60.0,
            effects: EffectsState {
                robotize: Robotize {
                    enabled: true,
                    mode: RobotizeMode::FreqShift,
                    frequency_hz: 180.0,
                    wet: 0.55,
                },
                echo: Echo {
                    enabled: true,
                    delay_beats: 0.25,
                    feedback: 0.40,
                    tone: 0.50,
                    ping_pong: true,
                    wet: 0.26,
                },
                dist_shape: DistortionShape::Crush,
                dirty: 0.55,
                ..EffectsState::default()
            },
        },
    ]
}
