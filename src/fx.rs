//! Send/return effects: four FX buses that the lanes feed through their
//! sends.
//!
//! Each lane sends a share of its sound, after its fader and pan, to each
//! bus. A bus holds one effect, the Echoform Delay, the Aetherform Reverb,
//! or the Emberform Drive, or nothing. The effect renders only its wet
//! sound. Then come the bus's return level and its safety stage, a
//! lookahead limiter or, for the drive, an instant ceiling, and the four
//! returns join the dry mix before the master pedalboard.
//!
//! The effects are this application's own designs. The delay's and the
//! reverb's controls, ranges and presets follow the return modules of the
//! MixJam Electron app, whose v7 projects saved them; that app had no drive.
mod adaa;
mod delay;
mod distortion;
pub(crate) mod migration;
pub(crate) mod oversample;
mod parts;
mod processor;
mod reverb;
mod settings;

pub(crate) use adaa::sample_transfer_curve;
pub(crate) use delay::sample_delay_taps;
pub use reverb::ReverbProfile;
pub(crate) use reverb::sample_reverb_profile;

/// Build the shared tables before a processor reaches the audio thread.
pub(crate) fn prepare_tables() {
    parts::warm_sinc_table();
    reverb::prepare_tables();
}
pub use oversample::RenderQuality;
pub use parts::return_limiter_ceiling_dbfs;
pub(crate) use processor::{LOOKAHEAD, TAIL_LIMIT};
pub use processor::{Lanes, ReturnLanes, ReturnsProcessor};
pub use settings::{
    Aetherform, DelayCharacter, DelayParam, DistortionParam, DistortionShape, Division, Echoform,
    Emberform, Range, ReverbCharacter, ReverbParam, SHIMMER_INTERVALS, SpaceModel, TimeMode,
};

use serde::{Deserialize, Serialize};

/// The number of FX buses, and of sends on each lane.
pub const BUSES: usize = 4;

// Buses ----------------------------------------------------------------------------------

/// The effect in a bus.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Module {
    /// No effect: the bus returns silence, so a send cannot double the dry sound.
    #[default]
    Empty,
    EchoformDelay(Echoform),
    AetherformReverb(Aetherform),
    EmberformDrive(Emberform),
}
impl Module {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Empty => "Empty",
            Self::EchoformDelay(_) => "Echoform Delay",
            Self::AetherformReverb(_) => "Aetherform Reverb",
            Self::EmberformDrive(_) => "Emberform Drive",
        }
    }
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
    /// Whether the effect rings on after its input stops, so Clear tail
    /// has something to cut: Delay and Reverb do, Drive and Empty do not.
    pub fn has_tail(&self) -> bool {
        matches!(self, Self::EchoformDelay(_) | Self::AetherformReverb(_))
    }
    /// The ducker's key: 0 is the bus's own input, 1 to 64 a track.
    pub fn duck_key(&self) -> u8 {
        match self {
            Self::Empty => 0,
            Self::EchoformDelay(delay) => delay.duck_key,
            Self::AetherformReverb(reverb) => reverb.duck_key,
            Self::EmberformDrive(drive) => drive.duck_key,
        }
    }
    /// Whether the ducker can push the effect down: a nonzero duck amount.
    pub fn ducks(&self) -> bool {
        let amount = match self {
            Self::Empty => 0.0,
            Self::EchoformDelay(delay) => delay.duck_amount,
            Self::AetherformReverb(reverb) => reverb.duck_amount_percent,
            Self::EmberformDrive(drive) => drive.duck_amount_percent,
        };
        amount > 0.0
    }
    /// The effect's factory presets and the return level each sets, in
    /// percent. An empty bus has none.
    pub fn presets(&self) -> &'static [(&'static str, f32)] {
        match self {
            Self::Empty => &[],
            Self::EchoformDelay(_) => &Echoform::PRESETS,
            Self::AetherformReverb(_) => &Aetherform::PRESETS,
            Self::EmberformDrive(_) => &Emberform::PRESETS,
        }
    }
    /// The same effect with a preset's settings and this effect's duck key,
    /// or `None` for an unknown name or an empty bus.
    fn with_preset(&self, name: &str) -> Option<Self> {
        Some(match self {
            Self::Empty => return None,
            Self::EchoformDelay(delay) => Self::EchoformDelay(Echoform {
                duck_key: delay.duck_key,
                ..Echoform::preset(name)?
            }),
            Self::AetherformReverb(reverb) => Self::AetherformReverb(Aetherform {
                duck_key: reverb.duck_key,
                ..Aetherform::preset(name)?
            }),
            Self::EmberformDrive(drive) => Self::EmberformDrive(Emberform {
                duck_key: drive.duck_key,
                ..Emberform::preset(name)?
            }),
        })
    }
}

/// One FX bus: its effect, its input switch, its return level, and its
/// safety limiter.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FxBus {
    pub module: Module,
    /// Off stops new input; the tail rings out.
    pub powered: bool,
    /// The return's amplitude, from 0 to 1.
    pub return_level: f32,
    /// Hold the return under -1 dBFS.
    pub limiter: bool,
}
impl Default for FxBus {
    fn default() -> Self {
        Self {
            module: Module::Empty,
            powered: true,
            return_level: 1.0,
            limiter: true,
        }
    }
}
impl FxBus {
    /// Whether the bus has an effect to process. Its state keeps running
    /// behind a zero return level, just as it does during live playback.
    pub fn is_active(&self) -> bool {
        !self.module.is_empty()
    }
    /// Load a factory preset of the bus's effect. It sets every control but
    /// the duck key, and the return level. An unknown name or an empty bus
    /// changes nothing and gives false.
    pub fn load_preset(&mut self, name: &str) -> bool {
        let level = self
            .module
            .presets()
            .iter()
            .find(|(preset, _)| *preset == name);
        let (Some(&(_, mix)), Some(module)) = (level, self.module.with_preset(name)) else {
            return false;
        };
        self.module = module;
        self.return_level = mix / 100.0;
        true
    }
    /// The factory preset whose settings and return level the bus has, if
    /// any. The duck key does not count.
    pub fn matching_preset(&self) -> Option<&'static str> {
        let level = (self.return_level * 100.0).round();
        self.module
            .presets()
            .iter()
            .find(|&&(name, mix)| {
                mix == level && self.module.with_preset(name) == Some(self.module)
            })
            .map(|&(name, _)| name)
    }
    fn validate(&self) -> Result<(), String> {
        if !(self.return_level.is_finite() && (0.0..=1.0).contains(&self.return_level)) {
            return Err("An FX return level must be from 0 to 1.".into());
        }
        match &self.module {
            Module::Empty => Ok(()),
            Module::EchoformDelay(delay) => delay.validate(),
            Module::AetherformReverb(reverb) => reverb.validate(),
            Module::EmberformDrive(drive) => drive.validate(),
        }
    }
}

/// The four FX buses of a song.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FxBuses {
    pub buses: [FxBus; BUSES],
}
impl FxBuses {
    /// Whether any bus has an effect to process, including silent returns.
    pub fn is_active(&self) -> bool {
        self.buses.iter().any(FxBus::is_active)
    }
    pub fn validate(&self) -> Result<(), String> {
        self.buses.iter().try_for_each(FxBus::validate)
    }
}

#[cfg(test)]
mod tests;
