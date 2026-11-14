//! The settings of the pedals: each pedal's switch and knob values, as a
//! song saves them.
use super::Knob;
use serde::{Deserialize, Serialize};

/// Knob positions of the Booster pedals run from 0 to this value, as the
/// library's setters take them.
pub const KNOB_MAX: u16 = 1000;
/// Equaliser bands run from 0 to this value; half of it is flat.
pub const EQ_BAND_MAX: u8 = 100;
pub const EQ_FREQUENCIES: [f64; 10] = [
    60.0, 80.0, 160.0, 320.0, 640.0, 1100.0, 2200.0, 4500.0, 9000.0, 12000.0,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BoosterEq {
    pub on: bool,
    pub bands: [u8; 10],
}
impl Default for BoosterEq {
    fn default() -> Self {
        Self {
            on: false,
            bands: [EQ_BAND_MAX / 2; 10],
        }
    }
}

/// Whether a pedal's key is its own input. Such a key is not saved.
fn is_own(key: &u8) -> bool {
    *key == 0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BoosterCompressor {
    pub on: bool,
    pub drive: u16,
    pub speed: u16,
    pub gain: u16,
    /// What the level follows: 0 is the pedal's input, 1 to 64 a track.
    #[serde(default, skip_serializing_if = "is_own")]
    pub key: u8,
}
impl Default for BoosterCompressor {
    fn default() -> Self {
        Self {
            on: false,
            drive: KNOB_MAX / 2,
            speed: KNOB_MAX / 2,
            gain: KNOB_MAX / 2,
            key: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BoosterWide {
    pub on: bool,
    pub spread: u16,
}
impl Default for BoosterWide {
    fn default() -> Self {
        Self {
            on: false,
            spread: KNOB_MAX / 2,
        }
    }
}

/// Rounds peaks off below a ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SoftClip {
    pub on: bool,
    /// How far below the ceiling the curve starts to bend, in dB.
    pub amount: f32,
    /// The level the output approaches, in dBFS.
    pub ceiling: f32,
}
impl Default for SoftClip {
    fn default() -> Self {
        Self {
            on: false,
            amount: Knob::SoftClipAmount.range().default,
            ceiling: Knob::SoftClipCeiling.range().default,
        }
    }
}

/// Asymmetric saturation with mostly even harmonics.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Tube {
    pub on: bool,
    /// From 0 to 10.
    pub drive: f32,
    /// The saturated share of the output, in percent.
    pub mix: f32,
}
impl Default for Tube {
    fn default() -> Self {
        Self {
            on: false,
            drive: Knob::TubeDrive.range().default,
            mix: Knob::TubeMix.range().default,
        }
    }
}

/// Cleanup: a high-pass, and narrow cuts for mud and harshness.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TrimEq {
    pub on: bool,
    /// The high-pass corner, in Hz.
    pub high_pass: f32,
    /// Cuts at 250 Hz and 3.5 kHz, in dB.
    pub mud: f32,
    pub harsh: f32,
}
impl Default for TrimEq {
    fn default() -> Self {
        Self {
            on: false,
            high_pass: Knob::TrimEqHighPass.range().default,
            mud: Knob::TrimEqMud.range().default,
            harsh: Knob::TrimEqHarsh.range().default,
        }
    }
}

/// A stereo-linked compressor for the whole mix.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BusCompressor {
    pub on: bool,
    /// In dBFS.
    pub threshold: f32,
    /// Input decibels over the threshold per output decibel.
    pub ratio: f32,
    /// In milliseconds.
    pub attack: f32,
    pub release: f32,
    /// What the detector follows: 0 is the pedal's input, 1 to 64 a track.
    #[serde(default, skip_serializing_if = "is_own")]
    pub key: u8,
}
impl Default for BusCompressor {
    fn default() -> Self {
        Self {
            on: false,
            threshold: Knob::CompressorThreshold.range().default,
            ratio: Knob::CompressorRatio.range().default,
            attack: Knob::CompressorAttack.range().default,
            release: Knob::CompressorRelease.range().default,
            key: 0,
        }
    }
}

/// Loudness from a soft clipper with a fixed ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Maximizer {
    pub on: bool,
    /// In percent; each percent drives the clipper a quarter decibel.
    pub boost: f32,
}
impl Default for Maximizer {
    fn default() -> Self {
        Self {
            on: false,
            boost: Knob::MaximizerBoost.range().default,
        }
    }
}

/// Wide shelves for low-end weight and treble air.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LiftEq {
    pub on: bool,
    /// Shelf gains at 90 Hz and 12 kHz, in dB.
    pub low: f32,
    pub air: f32,
}
impl Default for LiftEq {
    fn default() -> Self {
        Self {
            on: false,
            low: Knob::LiftLow.range().default,
            air: Knob::LiftAir.range().default,
        }
    }
}

/// Tape saturation with its head bump and treble roll-off.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Tape {
    pub on: bool,
    /// From 0 to 10.
    pub drive: f32,
    /// Inches per second: 15 or 30.
    pub speed: u8,
}
impl Default for Tape {
    fn default() -> Self {
        Self {
            on: false,
            drive: Knob::TapeDrive.range().default,
            speed: 30,
        }
    }
}

/// Mid-side width, with the lows kept mono.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Imaging {
    pub on: bool,
    /// The side level, in percent.
    pub width: f32,
    /// The side is removed below this frequency, in Hz.
    pub mono_below: f32,
}
impl Default for Imaging {
    fn default() -> Self {
        Self {
            on: false,
            width: Knob::ImagingWidth.range().default,
            mono_below: Knob::ImagingMonoBelow.range().default,
        }
    }
}

/// A three-band peak limiter with one amount per band: each band is driven
/// into a ceiling above its own recent peak, which lifts the sound under the
/// peaks, and the sum never peaks above the mix's recent peak.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Multiband {
    pub on: bool,
    /// Amounts below 200 Hz, from 200 Hz to 3 kHz, and above, in percent.
    pub low: f32,
    pub mid: f32,
    pub high: f32,
    /// 0, or a track from 1 to 64 whose same band each band also holds
    /// under its ceiling, so the band gives way while the key sounds.
    #[serde(default, skip_serializing_if = "is_own")]
    pub key: u8,
}
impl Default for Multiband {
    fn default() -> Self {
        Self {
            on: false,
            low: Knob::MultibandLow.range().default,
            mid: Knob::MultibandMid.range().default,
            high: Knob::MultibandHigh.range().default,
            key: 0,
        }
    }
}

/// A true-peak brickwall limiter.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Limiter {
    pub on: bool,
    /// Gain before limiting, in dB.
    pub gain: f32,
    /// The highest true peak out, in dBTP.
    pub ceiling: f32,
    /// 0, or a track from 1 to 64 that the limiter also holds under the
    /// ceiling, so the mix gives way while the key peaks.
    #[serde(default, skip_serializing_if = "is_own")]
    pub key: u8,
}
impl Default for Limiter {
    fn default() -> Self {
        Self {
            on: false,
            gain: Knob::LimiterGain.range().default,
            ceiling: Knob::LimiterCeiling.range().default,
            key: 0,
        }
    }
}
