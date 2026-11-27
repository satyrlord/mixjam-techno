//! Curated built-in presets for Effect Studio.
use super::{
    DistortionShape, Echo, EffectsState, Filter, FilterMode, Modulation, ModulationMode, Reverb,
    ReverbSpace, Robotize, RobotizeMode, TailMode,
};

/// A named Effect Studio configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectPreset {
    pub name: &'static str,
    pub state: EffectsState,
}

/// Curated factory presets covering clean analog polish, acid, dub, space, and lo-fi.
pub fn effect_presets() -> &'static [EffectPreset] {
    static PRESETS: std::sync::OnceLock<[EffectPreset; 8]> = std::sync::OnceLock::new();
    PRESETS.get_or_init(|| {
        [
            EffectPreset {
                name: "Clean / Init",
                state: EffectsState::default(),
            },
            EffectPreset {
                name: "Warm Analog Polish",
                state: EffectsState {
                    filter: Filter {
                        enabled: true,
                        mode: FilterMode::Lp12,
                        cutoff_hz: 16000.0,
                        resonance: 0.05,
                        drive: 0.15,
                        wet: 1.0,
                    },
                    modulation: Modulation {
                        enabled: true,
                        mode: ModulationMode::Chorus,
                        rate_hz: 0.45,
                        depth: 0.25,
                        feedback: 0.1,
                        wet: 0.18,
                    },
                    dist_shape: DistortionShape::Warm,
                    dirty: 0.15,
                    ..EffectsState::default()
                },
            },
            EffectPreset {
                name: "Acid Squelch",
                state: EffectsState {
                    filter: Filter {
                        enabled: true,
                        mode: FilterMode::Lp24,
                        cutoff_hz: 720.0,
                        resonance: 0.72,
                        drive: 0.35,
                        wet: 1.0,
                    },
                    dist_shape: DistortionShape::Acid,
                    dirty: 0.25,
                    volume: 1.05,
                    ..EffectsState::default()
                },
            },
            EffectPreset {
                name: "Dub Tape Echo",
                state: EffectsState {
                    filter: Filter {
                        enabled: true,
                        mode: FilterMode::Lp12,
                        cutoff_hz: 4500.0,
                        resonance: 0.22,
                        drive: 0.15,
                        wet: 1.0,
                    },
                    echo: Echo {
                        enabled: true,
                        delay_beats: 0.75,
                        feedback: 0.62,
                        tone: 0.55,
                        ping_pong: true,
                        wet: 0.45,
                    },
                    tail_mode: TailMode::RingOut,
                    ..EffectsState::default()
                },
            },
            EffectPreset {
                name: "Dark Warehouse Hall",
                state: EffectsState {
                    modulation: Modulation {
                        enabled: true,
                        mode: ModulationMode::Chorus,
                        rate_hz: 0.3,
                        depth: 0.45,
                        feedback: 0.2,
                        wet: 0.35,
                    },
                    reverb: Reverb {
                        enabled: true,
                        space: ReverbSpace::Hall,
                        decay_s: 3.2,
                        damping: 0.6,
                        wet: 0.38,
                    },
                    tail_mode: TailMode::RingOut,
                    ..EffectsState::default()
                },
            },
            EffectPreset {
                name: "Metallic Ring",
                state: EffectsState {
                    modulation: Modulation {
                        enabled: true,
                        mode: ModulationMode::Phaser,
                        rate_hz: 0.3,
                        depth: 0.7,
                        feedback: 0.45,
                        wet: 0.45,
                    },
                    echo: Echo {
                        enabled: true,
                        delay_beats: 0.5,
                        feedback: 0.4,
                        tone: 0.25,
                        ping_pong: true,
                        wet: 0.3,
                    },
                    robotize: Robotize {
                        enabled: true,
                        mode: RobotizeMode::RingMod,
                        frequency_hz: 220.0,
                        wet: 0.65,
                    },
                    ..EffectsState::default()
                },
            },
            EffectPreset {
                name: "Lo-Fi Cassette",
                state: EffectsState {
                    filter: Filter {
                        enabled: true,
                        mode: FilterMode::Lp12,
                        cutoff_hz: 6500.0,
                        resonance: 0.15,
                        drive: 0.25,
                        wet: 1.0,
                    },
                    modulation: Modulation {
                        enabled: true,
                        mode: ModulationMode::Chorus,
                        rate_hz: 1.4,
                        depth: 0.35,
                        feedback: 0.0,
                        wet: 0.25,
                    },
                    dist_shape: DistortionShape::Crush,
                    dirty: 0.28,
                    noise: 0.08,
                    ..EffectsState::default()
                },
            },
            EffectPreset {
                name: "Loop Slapback",
                state: EffectsState {
                    echo: Echo {
                        enabled: true,
                        delay_beats: 0.25,
                        feedback: 0.45,
                        tone: 0.3,
                        ping_pong: false,
                        wet: 0.35,
                    },
                    dist_shape: DistortionShape::Warm,
                    dirty: 0.18,
                    tail_mode: TailMode::WrapLoop,
                    ..EffectsState::default()
                },
            },
        ]
    })
}

/// Name of the built-in Effect Studio preset matching `state`, if any.
pub fn matching_effect_preset(state: &EffectsState) -> Option<&'static str> {
    effect_presets()
        .iter()
        .find(|preset| &preset.state == state)
        .map(|preset| preset.name)
}
