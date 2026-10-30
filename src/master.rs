//! The master pedalboard: processing of the whole mix after the tracks are
//! summed, for playback and export alike.
//!
//! The mix enters through the input trim, passes the pedals that are on in
//! the board's order, and leaves for the output. The limiter is always the
//! last pedal, so nothing after it can raise the peaks it holds; the user can
//! put the others in any order. Two kinds of pedal share the board:
//! - The Booster pedals are the master effects of the D3, D4, and T3
//!   products (see `booster`). They match the original library sample for sample and work on
//!   16-bit samples, as the original does.
//! - The mastering pedals are this application's own designs (see
//!   `pedals`). They work on floats. Those with a curve or a lookahead delay
//!   the sound; export removes the delay, and playback starts that much
//!   further into the mix.
//!
//! A board with every pedal off and the trim at 0 dB leaves the mix
//! untouched, and so does a bypassed board, whatever its settings: Bypass
//! skips the trim and every pedal, and the pedals keep their switches.
//! During playback, switching or moving a pedal, or the bypass, crossfades
//! from the old board to the new one, and meters read the input after the
//! trim and the output after the last pedal (see `meters`).
mod booster;
pub(crate) mod dsp;
mod meters;
mod pedals;
pub mod presets;

pub use meters::{MeterReading, OVER_DBTP};

use booster::clamp16;
use dsp::{Smoothed, db_to_gain};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, AtomicU64, Ordering},
};

/// Knob positions of the Booster pedals run from 0 to this value, as the
/// library's setters take them.
pub const KNOB_MAX: u16 = 1000;
/// Equaliser bands run from 0 to this value; half of it is flat.
pub const EQ_BAND_MAX: u8 = 100;
pub const EQ_FREQUENCIES: [f64; 10] = [
    60.0, 80.0, 160.0, 320.0, 640.0, 1100.0, 2200.0, 4500.0, 9000.0, 12000.0,
];

// Settings ------------------------------------------------------------------------

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

/// One pedal of the board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PedalKind {
    BoosterEq,
    BoosterCompressor,
    BoosterWide,
    SoftClip,
    Tube,
    TrimEq,
    BusCompressor,
    Maximizer,
    LiftEq,
    Tape,
    Imaging,
    Multiband,
    Limiter,
}
impl PedalKind {
    /// Every pedal in the board's first order: the Booster pedals in the
    /// original's order, then the mastering pedals, with the limiter last.
    pub const ALL: [Self; 13] = [
        Self::BoosterEq,
        Self::BoosterCompressor,
        Self::BoosterWide,
        Self::SoftClip,
        Self::Tube,
        Self::TrimEq,
        Self::BusCompressor,
        Self::Maximizer,
        Self::LiftEq,
        Self::Tape,
        Self::Imaging,
        Self::Multiband,
        Self::Limiter,
    ];
    #[cfg(test)]
    pub(crate) fn is_booster(self) -> bool {
        matches!(
            self,
            Self::BoosterEq | Self::BoosterCompressor | Self::BoosterWide
        )
    }
    /// Whether the pedal can change places. The limiter stays last.
    pub fn movable(self) -> bool {
        self != Self::Limiter
    }
    /// The pedal's knobs, in panel order.
    pub fn knobs(self) -> &'static [Knob] {
        use Knob::*;
        match self {
            Self::BoosterEq => &[
                BoosterBand(0),
                BoosterBand(1),
                BoosterBand(2),
                BoosterBand(3),
                BoosterBand(4),
                BoosterBand(5),
                BoosterBand(6),
                BoosterBand(7),
                BoosterBand(8),
                BoosterBand(9),
            ],
            Self::BoosterCompressor => &[BoosterDrive, BoosterSpeed, BoosterGain, BoosterKey],
            Self::BoosterWide => &[BoosterSpread],
            Self::SoftClip => &[SoftClipAmount, SoftClipCeiling],
            Self::Tube => &[TubeDrive, TubeMix],
            Self::TrimEq => &[TrimEqHighPass, TrimEqMud, TrimEqHarsh],
            Self::BusCompressor => &[
                CompressorThreshold,
                CompressorRatio,
                CompressorAttack,
                CompressorRelease,
                CompressorKey,
            ],
            Self::Maximizer => &[MaximizerBoost],
            Self::LiftEq => &[LiftLow, LiftAir],
            Self::Tape => &[TapeDrive],
            Self::Imaging => &[ImagingWidth, ImagingMonoBelow],
            Self::Multiband => &[MultibandLow, MultibandMid, MultibandHigh, MultibandKey],
            Self::Limiter => &[LimiterGain, LimiterCeiling, LimiterKey],
        }
    }
    /// The pedals with a key: the dynamics pedals.
    pub const KEYED: [Self; 4] = [
        Self::BoosterCompressor,
        Self::BusCompressor,
        Self::Multiband,
        Self::Limiter,
    ];
    /// The frames by which the pedal delays the sound while it is on.
    pub fn latency(self) -> usize {
        match self {
            Self::SoftClip => pedals::SoftClip::LATENCY,
            Self::Tube => pedals::Tube::LATENCY,
            Self::Maximizer => pedals::Maximizer::LATENCY,
            Self::Multiband => pedals::Multiband::LATENCY,
            Self::Tape => pedals::Tape::LATENCY,
            Self::Limiter => pedals::Limiter::LATENCY,
            _ => 0,
        }
    }
}

/// A continuous setting of the board: the input trim or a pedal's knob.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Knob {
    Trim,
    BoosterBand(usize),
    BoosterDrive,
    BoosterSpeed,
    BoosterGain,
    BoosterSpread,
    SoftClipAmount,
    SoftClipCeiling,
    TubeDrive,
    TubeMix,
    TrimEqHighPass,
    TrimEqMud,
    TrimEqHarsh,
    CompressorThreshold,
    CompressorRatio,
    CompressorAttack,
    CompressorRelease,
    MaximizerBoost,
    LiftLow,
    LiftAir,
    TapeDrive,
    ImagingWidth,
    ImagingMonoBelow,
    MultibandLow,
    MultibandMid,
    MultibandHigh,
    LimiterGain,
    LimiterCeiling,
    /// A dynamics pedal's key: 0 is its own input, 1 to 64 a track.
    BoosterKey,
    CompressorKey,
    MultibandKey,
    LimiterKey,
}

/// A knob's range in its unit, its default, and the step it moves in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnobRange {
    pub low: f32,
    pub high: f32,
    pub default: f32,
    pub step: f32,
    /// The dial spreads the range by ratio rather than by difference.
    pub logarithmic: bool,
}
pub(crate) const fn range(low: f32, high: f32, default: f32, step: f32) -> KnobRange {
    KnobRange {
        low,
        high,
        default,
        step,
        logarithmic: false,
    }
}
pub(crate) const fn ratio_range(low: f32, high: f32, default: f32, step: f32) -> KnobRange {
    KnobRange {
        logarithmic: true,
        ..range(low, high, default, step)
    }
}

impl Knob {
    /// Every knob: the trim, then each pedal's knobs in the first order.
    pub fn all() -> impl Iterator<Item = Self> {
        std::iter::once(Self::Trim).chain(
            PedalKind::ALL
                .into_iter()
                .flat_map(|kind| kind.knobs().iter().copied()),
        )
    }
    pub const fn range(self) -> KnobRange {
        match self {
            Self::Trim => range(-24.0, 24.0, 0.0, 0.1),
            Self::BoosterBand(_) => range(0.0, EQ_BAND_MAX as f32, (EQ_BAND_MAX / 2) as f32, 1.0),
            Self::BoosterDrive | Self::BoosterSpeed | Self::BoosterGain | Self::BoosterSpread => {
                range(0.0, KNOB_MAX as f32, (KNOB_MAX / 2) as f32, 1.0)
            }
            Self::SoftClipAmount => range(0.0, 6.0, 1.5, 0.1),
            Self::SoftClipCeiling => range(-6.0, 0.0, -0.5, 0.1),
            Self::TubeDrive => range(0.0, 10.0, 2.5, 0.1),
            Self::TubeMix => range(0.0, 100.0, 100.0, 1.0),
            Self::TrimEqHighPass => ratio_range(10.0, 40.0, 20.0, 1.0),
            Self::TrimEqMud => range(-3.0, 0.0, -1.5, 0.1),
            Self::TrimEqHarsh => range(-3.0, 0.0, -1.0, 0.1),
            Self::CompressorThreshold => range(-30.0, 0.0, -16.0, 0.5),
            Self::CompressorRatio => ratio_range(1.5, 10.0, 2.0, 0.1),
            Self::CompressorAttack => ratio_range(0.1, 100.0, 10.0, 0.1),
            Self::CompressorRelease => ratio_range(50.0, 1200.0, 300.0, 10.0),
            Self::MaximizerBoost => range(0.0, 25.0, 10.0, 1.0),
            Self::LiftLow | Self::LiftAir => range(0.0, 2.0, 1.0, 0.1),
            Self::TapeDrive => range(0.0, 10.0, 2.0, 0.1),
            Self::ImagingWidth => range(60.0, 140.0, 105.0, 1.0),
            Self::ImagingMonoBelow => ratio_range(60.0, 300.0, 120.0, 5.0),
            Self::MultibandLow | Self::MultibandHigh => range(0.0, 100.0, 20.0, 1.0),
            Self::MultibandMid => range(0.0, 100.0, 15.0, 1.0),
            Self::LimiterGain => range(0.0, 12.0, 4.0, 0.1),
            Self::LimiterCeiling => range(-3.0, 0.0, -1.0, 0.1),
            Self::BoosterKey | Self::CompressorKey | Self::MultibandKey | Self::LimiterKey => {
                range(0.0, crate::engine::TRACK_COUNT as f32, 0.0, 1.0)
            }
        }
    }
    /// Whether the knob picks a dynamics pedal's key.
    pub fn is_key(self) -> bool {
        matches!(
            self,
            Self::BoosterKey | Self::CompressorKey | Self::MultibandKey | Self::LimiterKey
        )
    }
    /// The pedal the knob belongs to; the trim belongs to the board.
    pub fn pedal(self) -> Option<PedalKind> {
        PedalKind::ALL
            .into_iter()
            .find(|kind| kind.knobs().contains(&self))
    }
    pub fn get(self, chain: &MasterChain) -> f32 {
        match self {
            Self::Trim => chain.trim,
            Self::BoosterBand(band) => f32::from(chain.booster_eq.bands[band]),
            Self::BoosterDrive => f32::from(chain.booster_compressor.drive),
            Self::BoosterSpeed => f32::from(chain.booster_compressor.speed),
            Self::BoosterGain => f32::from(chain.booster_compressor.gain),
            Self::BoosterSpread => f32::from(chain.booster_wide.spread),
            Self::SoftClipAmount => chain.soft_clip.amount,
            Self::SoftClipCeiling => chain.soft_clip.ceiling,
            Self::TubeDrive => chain.tube.drive,
            Self::TubeMix => chain.tube.mix,
            Self::TrimEqHighPass => chain.trim_eq.high_pass,
            Self::TrimEqMud => chain.trim_eq.mud,
            Self::TrimEqHarsh => chain.trim_eq.harsh,
            Self::CompressorThreshold => chain.bus_compressor.threshold,
            Self::CompressorRatio => chain.bus_compressor.ratio,
            Self::CompressorAttack => chain.bus_compressor.attack,
            Self::CompressorRelease => chain.bus_compressor.release,
            Self::MaximizerBoost => chain.maximizer.boost,
            Self::LiftLow => chain.lift_eq.low,
            Self::LiftAir => chain.lift_eq.air,
            Self::TapeDrive => chain.tape.drive,
            Self::ImagingWidth => chain.imaging.width,
            Self::ImagingMonoBelow => chain.imaging.mono_below,
            Self::MultibandLow => chain.multiband.low,
            Self::MultibandMid => chain.multiband.mid,
            Self::MultibandHigh => chain.multiband.high,
            Self::LimiterGain => chain.limiter.gain,
            Self::LimiterCeiling => chain.limiter.ceiling,
            Self::BoosterKey => f32::from(chain.booster_compressor.key),
            Self::CompressorKey => f32::from(chain.bus_compressor.key),
            Self::MultibandKey => f32::from(chain.multiband.key),
            Self::LimiterKey => f32::from(chain.limiter.key),
        }
    }
    /// Set the knob, held to its range. The Booster knobs take whole positions.
    pub fn set(self, chain: &mut MasterChain, value: f32) {
        let range = self.range();
        let value = if value.is_nan() {
            range.default
        } else {
            value.clamp(range.low, range.high)
        };
        let whole = value.round() as u16;
        match self {
            Self::Trim => chain.trim = value,
            Self::BoosterBand(band) => chain.booster_eq.bands[band] = whole as u8,
            Self::BoosterDrive => chain.booster_compressor.drive = whole,
            Self::BoosterSpeed => chain.booster_compressor.speed = whole,
            Self::BoosterGain => chain.booster_compressor.gain = whole,
            Self::BoosterSpread => chain.booster_wide.spread = whole,
            Self::SoftClipAmount => chain.soft_clip.amount = value,
            Self::SoftClipCeiling => chain.soft_clip.ceiling = value,
            Self::TubeDrive => chain.tube.drive = value,
            Self::TubeMix => chain.tube.mix = value,
            Self::TrimEqHighPass => chain.trim_eq.high_pass = value,
            Self::TrimEqMud => chain.trim_eq.mud = value,
            Self::TrimEqHarsh => chain.trim_eq.harsh = value,
            Self::CompressorThreshold => chain.bus_compressor.threshold = value,
            Self::CompressorRatio => chain.bus_compressor.ratio = value,
            Self::CompressorAttack => chain.bus_compressor.attack = value,
            Self::CompressorRelease => chain.bus_compressor.release = value,
            Self::MaximizerBoost => chain.maximizer.boost = value,
            Self::LiftLow => chain.lift_eq.low = value,
            Self::LiftAir => chain.lift_eq.air = value,
            Self::TapeDrive => chain.tape.drive = value,
            Self::ImagingWidth => chain.imaging.width = value,
            Self::ImagingMonoBelow => chain.imaging.mono_below = value,
            Self::MultibandLow => chain.multiband.low = value,
            Self::MultibandMid => chain.multiband.mid = value,
            Self::MultibandHigh => chain.multiband.high = value,
            Self::LimiterGain => chain.limiter.gain = value,
            Self::LimiterCeiling => chain.limiter.ceiling = value,
            Self::BoosterKey => chain.booster_compressor.key = whole as u8,
            Self::CompressorKey => chain.bus_compressor.key = whole as u8,
            Self::MultibandKey => chain.multiband.key = whole as u8,
            Self::LimiterKey => chain.limiter.key = whole as u8,
        }
    }
}

/// The input trim, every pedal with its switch, and the order the signal
/// passes through them. A pedal that is off keeps its settings and passes
/// the signal on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MasterChain {
    /// Skip the whole board, trim included, for playback and export. The
    /// pedals keep their settings and switches.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bypass: bool,
    /// The gain before the first pedal, in dB.
    pub trim: f32,
    /// Each pedal once, from the mixer's output to the master output,
    /// ending with the limiter.
    pub order: [PedalKind; 13],
    pub booster_eq: BoosterEq,
    pub booster_compressor: BoosterCompressor,
    pub booster_wide: BoosterWide,
    pub soft_clip: SoftClip,
    pub tube: Tube,
    pub trim_eq: TrimEq,
    pub bus_compressor: BusCompressor,
    pub maximizer: Maximizer,
    pub lift_eq: LiftEq,
    pub tape: Tape,
    pub imaging: Imaging,
    pub multiband: Multiband,
    pub limiter: Limiter,
}
impl Default for MasterChain {
    fn default() -> Self {
        Self {
            bypass: false,
            trim: 0.0,
            order: PedalKind::ALL,
            booster_eq: BoosterEq::default(),
            booster_compressor: BoosterCompressor::default(),
            booster_wide: BoosterWide::default(),
            soft_clip: SoftClip::default(),
            tube: Tube::default(),
            trim_eq: TrimEq::default(),
            bus_compressor: BusCompressor::default(),
            maximizer: Maximizer::default(),
            lift_eq: LiftEq::default(),
            tape: Tape::default(),
            imaging: Imaging::default(),
            multiband: Multiband::default(),
            limiter: Limiter::default(),
        }
    }
}
impl MasterChain {
    /// Whether the board changes the mix: it is not bypassed, and a pedal is
    /// on or the trim is not 0 dB.
    pub fn is_active(&self) -> bool {
        !self.bypass
            && (self.trim != 0.0 || PedalKind::ALL.into_iter().any(|kind| self.is_on(kind)))
    }
    /// Whether a pedal processes the mix: it is on and the board is not bypassed.
    pub fn runs(&self, kind: PedalKind) -> bool {
        !self.bypass && self.is_on(kind)
    }
    pub fn is_on(&self, kind: PedalKind) -> bool {
        match kind {
            PedalKind::BoosterEq => self.booster_eq.on,
            PedalKind::BoosterCompressor => self.booster_compressor.on,
            PedalKind::BoosterWide => self.booster_wide.on,
            PedalKind::SoftClip => self.soft_clip.on,
            PedalKind::Tube => self.tube.on,
            PedalKind::TrimEq => self.trim_eq.on,
            PedalKind::BusCompressor => self.bus_compressor.on,
            PedalKind::Maximizer => self.maximizer.on,
            PedalKind::LiftEq => self.lift_eq.on,
            PedalKind::Tape => self.tape.on,
            PedalKind::Imaging => self.imaging.on,
            PedalKind::Multiband => self.multiband.on,
            PedalKind::Limiter => self.limiter.on,
        }
    }
    pub fn set_on(&mut self, kind: PedalKind, on: bool) {
        match kind {
            PedalKind::BoosterEq => self.booster_eq.on = on,
            PedalKind::BoosterCompressor => self.booster_compressor.on = on,
            PedalKind::BoosterWide => self.booster_wide.on = on,
            PedalKind::SoftClip => self.soft_clip.on = on,
            PedalKind::Tube => self.tube.on = on,
            PedalKind::TrimEq => self.trim_eq.on = on,
            PedalKind::BusCompressor => self.bus_compressor.on = on,
            PedalKind::Maximizer => self.maximizer.on = on,
            PedalKind::LiftEq => self.lift_eq.on = on,
            PedalKind::Tape => self.tape.on = on,
            PedalKind::Imaging => self.imaging.on = on,
            PedalKind::Multiband => self.multiband.on = on,
            PedalKind::Limiter => self.limiter.on = on,
        }
    }
    /// A pedal's key: 0, its own input, for a pedal without one.
    pub fn key(&self, kind: PedalKind) -> u8 {
        match kind {
            PedalKind::BoosterCompressor => self.booster_compressor.key,
            PedalKind::BusCompressor => self.bus_compressor.key,
            PedalKind::Multiband => self.multiband.key,
            PedalKind::Limiter => self.limiter.key,
            _ => 0,
        }
    }
    /// The tracks, from 0, that the pedals that run listen to.
    pub fn key_tracks(&self) -> impl Iterator<Item = usize> + '_ {
        PedalKind::KEYED
            .into_iter()
            .filter(|kind| self.runs(*kind))
            .filter_map(|kind| usize::from(self.key(kind)).checked_sub(1))
    }
    /// The frames by which the pedals that run delay the sound.
    pub fn latency(&self) -> usize {
        self.order
            .iter()
            .filter(|kind| self.runs(**kind))
            .map(|kind| kind.latency())
            .sum()
    }
    /// Move the pedal at `from` to position `to`; the pedals between shift
    /// one place. Positions outside the board change nothing.
    /// The limiter stays last, so a move from or to its place changes nothing.
    pub fn move_pedal(&mut self, from: usize, to: usize) {
        let last = self.order.len() - 1;
        if from < last && to < last {
            let kind = self.order[from];
            if from < to {
                self.order.copy_within(from + 1..=to, from);
            } else {
                self.order.copy_within(to..from, to + 1);
            }
            self.order[to] = kind;
        }
    }
    /// The one knob that differs from `other`, when no switch, speed, or
    /// position differs too.
    pub fn changed_knob(&self, other: &Self) -> Option<Knob> {
        if self.bypass != other.bypass
            || self.order != other.order
            || self.tape.speed != other.tape.speed
            || PedalKind::ALL
                .into_iter()
                .any(|kind| self.is_on(kind) != other.is_on(kind))
        {
            return None;
        }
        let mut changed = Knob::all().filter(|knob| knob.get(self) != knob.get(other));
        match (changed.next(), changed.next()) {
            (Some(knob), None) => Some(knob),
            _ => None,
        }
    }
    /// The trim as a gain: unity while the board is bypassed.
    fn trim_gain(&self) -> f64 {
        if self.bypass {
            1.0
        } else {
            db_to_gain(f64::from(self.trim))
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if PedalKind::ALL
            .iter()
            .any(|kind| self.order.iter().filter(|placed| *placed == kind).count() != 1)
        {
            return Err("The master pedal order must name each pedal once.".into());
        }
        if self.order.last() != Some(&PedalKind::Limiter) {
            return Err("The limiter must be the last master pedal.".into());
        }
        if Knob::all().any(|knob| {
            let (value, range) = (knob.get(self), knob.range());
            !(value.is_finite() && (range.low..=range.high).contains(&value))
        }) {
            return Err("A master pedal setting is out of range.".into());
        }
        if ![15, 30].contains(&self.tape.speed) {
            return Err("The tape speed must be 15 or 30 inches per second.".into());
        }
        Ok(())
    }
}

// Live control ------------------------------------------------------------------------

/// The link between the window and playback: settings that playback reads
/// while it runs, and the meters it measures. An edit stores new settings;
/// the audio thread takes them at its next block without waiting. The FX
/// buses (see `fx`) take their settings and Clear Tail requests here too,
/// and so do the track levels: each fader, pan, and polarity. Playback
/// keeps its lane compressors' histories here, so a playback started again
/// where the last one was, after a pause or for a mix prepared again,
/// carries on from them.
pub struct MasterControl {
    generation: AtomicU64,
    chain: Mutex<MasterChain>,
    meters: meters::Shared,
    returns_generation: AtomicU64,
    returns: Mutex<crate::fx::FxBuses>,
    tail_clears: [AtomicU64; crate::fx::BUSES],
    /// Each track's two f32 levels packed into one atomic value, so a
    /// reader cannot combine channels from different edits. NaN until set.
    levels: [AtomicU64; crate::engine::TRACK_COUNT],
    /// Serializes publishers and remembers which lane compressors are on.
    level_writer: Mutex<u64>,
    /// Odd while levels and master gain are being published. Audio makes
    /// one bounded snapshot attempt and keeps its last snapshot on overlap.
    level_sequence: AtomicU64,
    /// The master gain inside those levels, as f32 bits. NaN until set.
    /// It is not part of any level pair: a lane ducker's key reads it to
    /// leave the master gain out of its level.
    master_gain: AtomicU32,
    /// Each track's compressor history as playback's last block left it
    /// (`compressor::State::pack`), the frame that block ended at, and its
    /// source epoch, so
    /// that a new playback start just behind that frame carries on from
    /// it. NaN until a playback stores one. Only the audio thread uses them.
    compressors: [[AtomicU64; 3]; crate::engine::TRACK_COUNT],
    compressor_epoch: AtomicU64,
    compressor_reset: AtomicU64,
    compressor_track_reset: [AtomicU64; crate::engine::TRACK_COUNT],
}
impl MasterControl {
    pub fn new(chain: &MasterChain) -> Arc<Self> {
        Arc::new(Self {
            generation: AtomicU64::new(0),
            chain: Mutex::new(*chain),
            meters: meters::Shared::new(),
            returns_generation: AtomicU64::new(0),
            returns: Mutex::new(crate::fx::FxBuses::default()),
            tail_clears: std::array::from_fn(|_| AtomicU64::new(0)),
            levels: std::array::from_fn(|_| {
                let unset = u64::from(f32::NAN.to_bits());
                AtomicU64::new(unset | (unset << 32))
            }),
            master_gain: AtomicU32::new(f32::NAN.to_bits()),
            level_writer: Mutex::new(0),
            level_sequence: AtomicU64::new(0),
            compressors: std::array::from_fn(|_| {
                let unset = u64::from(f32::NAN.to_bits());
                [
                    AtomicU64::new(unset | (unset << 32)),
                    AtomicU64::new(0),
                    AtomicU64::new(0),
                ]
            }),
            compressor_epoch: AtomicU64::new(0),
            compressor_reset: AtomicU64::new(0),
            compressor_track_reset: std::array::from_fn(|_| AtomicU64::new(0)),
        })
    }
    /// Store the project's track levels. Playback glides to them over its
    /// next block, so a fader moves without a click and without a new mix.
    pub fn set_levels(&self, project: &crate::engine::Project) {
        let mut switched = self.level_writer.lock().unwrap_or_else(|e| e.into_inner());
        let enabled = project
            .tracks
            .iter()
            .enumerate()
            .fold(0_u64, |mask, (index, track)| {
                mask | (u64::from(track.compressor.on) << index)
            });
        if *switched != enabled {
            let epoch = self.compressor_epoch.fetch_add(1, Ordering::AcqRel) + 1;
            for (track, reset) in self.compressor_track_reset.iter().enumerate() {
                if (*switched ^ enabled) & (1_u64 << track) != 0 {
                    reset.store(epoch, Ordering::Release);
                }
            }
            *switched = enabled;
        }
        self.level_sequence.fetch_add(1, Ordering::SeqCst);
        for (stored, level) in self.levels.iter().zip(crate::audio::track_levels(project)) {
            let [left, right] = level.map(|value| u64::from(value.to_bits()));
            stored.store(left | (right << 32), Ordering::SeqCst);
        }
        self.master_gain
            .store(project.master_gain.to_bits(), Ordering::SeqCst);
        self.level_sequence.fetch_add(1, Ordering::SeqCst);
    }
    /// All track levels and their master gain from the same publication.
    /// Returns None if unset or a writer overlaps this single attempt.
    pub(crate) fn levels(&self) -> Option<([[f32; 2]; crate::engine::TRACK_COUNT], f32)> {
        let before = self.level_sequence.load(Ordering::SeqCst);
        if before & 1 != 0 {
            return None;
        }
        let levels = std::array::from_fn(|track| {
            let packed = self.levels[track].load(Ordering::SeqCst);
            [packed as u32, (packed >> 32) as u32].map(f32::from_bits)
        });
        let gain = f32::from_bits(self.master_gain.load(Ordering::SeqCst));
        (before == self.level_sequence.load(Ordering::SeqCst) && gain.is_finite())
            .then_some((levels, gain))
    }
    pub(crate) fn compressor_epoch(&self) -> u64 {
        self.compressor_epoch.load(Ordering::Acquire)
    }
    /// Explicit transport discontinuities discard every track's old
    /// reduction. Switch changes use per-track floors in `set_levels`.
    /// Old playback sources cannot publish into the new epoch.
    pub(crate) fn reset_compressors(&self) {
        let epoch = self.compressor_epoch.fetch_add(1, Ordering::AcqRel) + 1;
        self.compressor_reset.store(epoch, Ordering::Release);
    }
    fn compressor_valid(&self, track: usize, epoch: u64) -> bool {
        epoch >= self.compressor_reset.load(Ordering::Acquire)
            && epoch >= self.compressor_track_reset[track].load(Ordering::Acquire)
    }
    /// Keep a track's compressor history at the end of a playback block,
    /// and the frame after that block. Called by the audio thread.
    pub(crate) fn keep_compressor(
        &self,
        track: usize,
        state: crate::compressor::State,
        at: u64,
        epoch: u64,
    ) {
        if !self.compressor_valid(track, epoch) {
            return;
        }
        let [kept, frame, saved_epoch] = &self.compressors[track];
        frame.store(at, Ordering::Relaxed);
        saved_epoch.store(epoch, Ordering::Relaxed);
        kept.store(state.pack(), Ordering::Release);
    }
    /// The compressor history that playback last kept for a track, if any,
    /// with the frame after the block that left it.
    pub(crate) fn kept_compressor(&self, track: usize) -> Option<(crate::compressor::State, u64)> {
        let [kept, frame, saved_epoch] = &self.compressors[track];
        crate::compressor::State::unpack(kept.load(Ordering::Acquire))
            .filter(|_| self.compressor_valid(track, saved_epoch.load(Ordering::Relaxed)))
            .map(|state| (state, frame.load(Ordering::Relaxed)))
    }
    pub fn set_returns(&self, buses: &crate::fx::FxBuses) {
        let mut stored = self.returns.lock().unwrap_or_else(|e| e.into_inner());
        if *stored == *buses {
            return;
        }
        *stored = *buses;
        drop(stored);
        self.returns_generation.fetch_add(1, Ordering::Release);
    }
    pub fn returns(&self) -> crate::fx::FxBuses {
        *self.returns.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Cut the tail of one bus: it fades out over 12 ms and its effect
    /// starts again from silence. This is not a setting and not an edit.
    pub fn clear_tail(&self, bus: usize) {
        if let Some(count) = self.tail_clears.get(bus) {
            count.fetch_add(1, Ordering::Release);
        }
    }
    pub(crate) fn tail_clears(&self, bus: usize) -> u64 {
        self.tail_clears[bus].load(Ordering::Acquire)
    }
    /// New bus settings since `seen`, if the lock is free.
    pub(crate) fn poll_returns(&self, seen: &mut u64) -> Option<crate::fx::FxBuses> {
        let generation = self.returns_generation.load(Ordering::Acquire);
        if generation == *seen {
            return None;
        }
        let buses = *self.returns.try_lock().ok()?;
        *seen = generation;
        Some(buses)
    }
    pub fn set(&self, chain: &MasterChain) {
        let mut stored = self.chain.lock().unwrap_or_else(|e| e.into_inner());
        if *stored == *chain {
            return;
        }
        *stored = *chain;
        drop(stored);
        self.generation.fetch_add(1, Ordering::Release);
    }
    pub fn get(&self) -> MasterChain {
        *self.chain.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// The latest reading of the playback that runs through this control.
    pub fn meters(&self) -> MeterReading {
        self.meters.read()
    }
    /// New settings since `seen`, if the lock is free. A busy lock leaves
    /// `seen` unchanged, so the next block tries again.
    fn poll(&self, seen: &mut u64) -> Option<MasterChain> {
        let generation = self.generation.load(Ordering::Acquire);
        if generation == *seen {
            return None;
        }
        let chain = *self.chain.try_lock().ok()?;
        *seen = generation;
        Some(chain)
    }
}

// Processing ------------------------------------------------------------------------------

/// The crossfade after a pedal is switched or moved during playback: 30 ms.
const FADE: usize = 1323;
/// Playback keeps this delay even as pedals switch, so the timeline and
/// both sides of a crossfade stay aligned. Export uses only active delays.
const LIVE_LATENCY: usize =
    4 * dsp::Oversampler::LATENCY + pedals::Multiband::LATENCY + pedals::Limiter::LATENCY;

/// Runs the board over stereo audio. Its state carries from one block to
/// the next.
pub struct MasterProcessor {
    control: Option<(Arc<MasterControl>, u64)>,
    trim: Smoothed,
    board: Box<Board>,
    /// The board before the last switch or move, while it fades out.
    previous: Box<Board>,
    /// Frames left of the crossfade from `previous` to `board`.
    fade: usize,
    /// Let a rewired board fill its delays before it joins the crossfade.
    warmup: usize,
    meters: Option<Box<meters::Meters>>,
    /// Do not measure the padding that playback removes before its first frame.
    meter_lead: usize,
}
impl MasterProcessor {
    /// Playback and export start with the Booster compressor's smoothed
    /// controls at their targets, every mastering pedal at its settings,
    /// and every other state cleared. The original starts the smoothed
    /// controls at a fixed value when the library loads and never resets
    /// them; here each playback starts as a settled original would.
    pub fn new(chain: &MasterChain) -> Self {
        let mut board = Box::new(Board::new());
        board.apply(chain);
        board.settle();
        Self {
            control: None,
            trim: Smoothed::new(chain.trim_gain()),
            previous: board.clone(),
            board,
            fade: 0,
            warmup: 0,
            meters: None,
            meter_lead: 0,
        }
    }
    /// A processor that takes its settings from `control` as they change
    /// and measures for its meters.
    pub fn live(control: Arc<MasterControl>) -> Self {
        let mut seen = 0;
        let chain = control.poll(&mut seen).unwrap_or_else(|| control.get());
        let mut processor = Self::new(&chain);
        processor.board.live = true;
        processor.previous.live = true;
        processor.meters = Some(Box::new(meters::Meters::new(&control.meters)));
        processor.meter_lead = LIVE_LATENCY;
        processor.control = Some((control, seen));
        processor
    }
    /// The frames by which the output follows the input.
    pub fn latency(&self) -> usize {
        if self.board.live {
            LIVE_LATENCY
        } else {
            self.board.chain.latency()
        }
    }
    fn update(&mut self, chain: &MasterChain) {
        self.trim.set(chain.trim_gain());
        if self.board.rewired(chain) {
            *self.previous = *self.board;
            self.board.padding = [dsp::Delay::ZERO; 2];
            self.fade = FADE;
            self.warmup = LIVE_LATENCY;
        }
        self.board.apply(chain);
    }
    /// Process interleaved stereo samples in place: the trim, then the hold
    /// to full scale, then the pedals. The trim comes first, so a lower trim
    /// keeps a hot mix from clipping.
    pub fn process(&mut self, block: &mut [f32]) {
        self.process_keyed(block, crate::ducker::Keys::NONE);
    }
    /// `process` with the key tracks of the block, which the keyed pedals
    /// listen to after the same trim.
    pub fn process_keyed(&mut self, block: &mut [f32], keys: crate::ducker::Keys) {
        if self.fade == 0
            && let Some((control, seen)) = &mut self.control
            && let Some(chain) = control.poll(seen)
        {
            self.update(&chain);
        }
        let trimmed = !(self.trim.settled() && self.trim.value() == 1.0);
        if !trimmed && !self.board.active && self.fade == 0 && self.meters.is_none() {
            crate::audio::hold_to_full_scale(block);
            return;
        }
        let slots = self.board.key_slots(&keys);
        let previous_slots = self.previous.key_slots(&keys);
        for (frame, [left, right]) in block.as_chunks_mut::<2>().0.iter_mut().enumerate() {
            let (mut l, mut r) = (*left, *right);
            let mut gain = 1.0;
            if trimmed {
                gain = self.trim.next() as f32;
                (l, r) = (l * gain, r * gain);
            }
            if let Some(meters) = &mut self.meters {
                meters.input(l, r);
            }
            let key = |slots: [Option<Option<usize>>; 4]| -> KeyFrame {
                slots.map(|slot| {
                    slot.map(|slot| {
                        let (key_left, key_right) = keys.frame(slot, frame);
                        (key_left * gain, key_right * gain)
                    })
                })
            };
            (l, r) = (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0));
            (l, r) = if self.fade > 0 {
                self.crossfade(l, r, key(slots), key(previous_slots))
            } else if self.board.active || self.board.live {
                self.board.process(l, r, key(slots))
            } else {
                (l, r)
            };
            if self.meter_lead > 0 {
                self.meter_lead -= 1;
            } else if let Some(meters) = &mut self.meters {
                meters.output(l, r);
            }
            (*left, *right) = (l, r);
        }
        if let (Some(meters), Some((control, _))) = (&mut self.meters, &self.control) {
            let board = &mut self.board;
            let compressor = if board.chain.runs(PedalKind::BusCompressor) {
                board.bus_compressor.take_reduction()
            } else {
                0.0
            };
            let limiter = if board.chain.runs(PedalKind::Limiter) {
                board.limiter.take_reduction()
            } else {
                0.0
            };
            meters.publish(&control.meters, compressor, limiter);
        }
    }
    /// Complete delayed peak measurement after the last audible frame.
    /// This does not run the board or add samples to the loudness windows.
    pub(crate) fn finish(&mut self) {
        if let (Some(meters), Some((control, _))) = (&mut self.meters, &self.control) {
            meters.finish(&control.meters);
        }
    }
    /// Both boards on the same input, the old one fading out along a
    /// raised cosine. The two outputs are nearly the same sound, so their
    /// gains sum to one.
    fn crossfade(
        &mut self,
        left: f32,
        right: f32,
        keys: KeyFrame,
        previous_keys: KeyFrame,
    ) -> (f32, f32) {
        let (new_l, new_r) = self.board.process(left, right, keys);
        let (old_l, old_r) = self.previous.process(left, right, previous_keys);
        if self.warmup > 0 {
            self.warmup -= 1;
            return (old_l, old_r);
        }
        let progress = 1.0 - self.fade as f64 / FADE as f64;
        let weight = (0.5 - 0.5 * (std::f64::consts::PI * progress).cos()) as f32;
        self.fade -= 1;
        (
            old_l + (new_l - old_l) * weight,
            old_r + (new_r - old_r) * weight,
        )
    }
}

/// A stereo frame between two pedals.
#[derive(Clone, Copy)]
enum Frame {
    /// Normalized floats, as the mix and the mastering pedals use.
    Audio(f32, f32),
    /// 16-bit samples, as the Booster pedals take them.
    Integers(i32, i32),
    /// 16-bit samples as floats, as the Booster equaliser gives them.
    Floats(f32, f32),
}
impl Frame {
    /// Leaving the Booster pedals, samples are clamped to 16 bits, as at the
    /// end of the original's stereo path.
    fn audio(self) -> (f32, f32) {
        match self {
            Self::Audio(left, right) => (left, right),
            _ => {
                let (left, right) = self.integers();
                (
                    f32::from(clamp16(left)) / 32768.0,
                    f32::from(clamp16(right)) / 32768.0,
                )
            }
        }
    }
    /// An integer becomes a float exactly; a float becomes an integer by
    /// truncation, and a normalized float by rounding to 16 bits.
    fn integers(self) -> (i32, i32) {
        match self {
            Self::Audio(left, right) => (
                i32::from(crate::audio::float_to_i16(left)),
                i32::from(crate::audio::float_to_i16(right)),
            ),
            Self::Integers(left, right) => (left, right),
            Self::Floats(left, right) => (left as i32, right as i32),
        }
    }
    fn floats(self) -> (f32, f32) {
        match self {
            Self::Floats(left, right) => (left, right),
            _ => {
                let (left, right) = self.integers();
                (left as f32, right as f32)
            }
        }
    }
}

/// The state of every pedal and the settings it runs with.
#[derive(Clone, Copy)]
struct Board {
    chain: MasterChain,
    live: bool,
    padding: [dsp::Delay<LIVE_LATENCY>; 2],
    /// Whether any pedal is on.
    active: bool,
    equalizer: booster::Equalizer,
    compressor: booster::Compressor,
    wide: booster::Wide,
    soft_clip: pedals::SoftClip,
    tube: pedals::Tube,
    trim_eq: pedals::TrimEq,
    bus_compressor: pedals::BusCompressor,
    maximizer: pedals::Maximizer,
    lift_eq: pedals::LiftEq,
    tape: pedals::Tape,
    imaging: pedals::Imaging,
    multiband: pedals::Multiband,
    limiter: pedals::Limiter,
    /// Each keyed pedal's key, in `PedalKind::KEYED` order, delayed by the
    /// pedals that run before it, so that it lines up with the sound.
    key_delays: [[dsp::Delay<LIVE_LATENCY>; 2]; 4],
    key_lags: [usize; 4],
}
/// A frame of each keyed pedal's key, in `PedalKind::KEYED` order: `None`
/// for a pedal that listens to its own input.
type KeyFrame = [Option<(f32, f32)>; 4];
impl Board {
    fn new() -> Self {
        Self {
            chain: MasterChain::default(),
            live: false,
            padding: [dsp::Delay::ZERO; 2],
            key_delays: [[dsp::Delay::ZERO; 2]; 4],
            key_lags: [0; 4],
            active: false,
            equalizer: booster::Equalizer::new(),
            compressor: booster::Compressor::at_load(),
            wide: booster::Wide::new(),
            soft_clip: pedals::SoftClip::new(),
            tube: pedals::Tube::new(),
            trim_eq: pedals::TrimEq::new(),
            bus_compressor: pedals::BusCompressor::new(),
            maximizer: pedals::Maximizer::new(),
            lift_eq: pedals::LiftEq::new(),
            tape: pedals::Tape::new(),
            imaging: pedals::Imaging::new(),
            multiband: pedals::Multiband::new(),
            limiter: pedals::Limiter::new(),
        }
    }
    /// Take new settings. A pedal that switches on starts from a clear
    /// state, as the original's enable calls do; other changes keep the
    /// state, and the mastering pedals glide to their new settings.
    fn apply(&mut self, chain: &MasterChain) {
        let old = self.chain;
        // Leaving bypass switches on every pedal that is on.
        let switched_on = |kind| chain.runs(kind) && !old.runs(kind);
        if switched_on(PedalKind::BoosterEq) {
            self.equalizer.clear();
        }
        if chain.booster_eq.bands != old.booster_eq.bands {
            self.equalizer.set(&chain.booster_eq.bands);
        }
        if switched_on(PedalKind::BoosterCompressor) {
            self.compressor.clear();
        }
        let compressor = &chain.booster_compressor;
        self.compressor
            .set(compressor.drive, compressor.speed, compressor.gain);
        if switched_on(PedalKind::BoosterWide) {
            self.wide.clear();
        }
        self.wide.set(chain.booster_wide.spread);

        self.soft_clip.set(&chain.soft_clip);
        self.tube.set(&chain.tube);
        self.trim_eq.set(&chain.trim_eq);
        self.bus_compressor.set(&chain.bus_compressor);
        self.maximizer.set(&chain.maximizer);
        self.lift_eq.set(&chain.lift_eq);
        self.tape.set(&chain.tape);
        self.imaging.set(&chain.imaging);
        self.multiband.set(&chain.multiband);
        self.limiter.set(&chain.limiter);
        for kind in PedalKind::ALL {
            if switched_on(kind) {
                self.clear(kind);
            }
        }
        self.chain = *chain;
        self.active = PedalKind::ALL.into_iter().any(|kind| chain.runs(kind));
        let mut lag = 0;
        for kind in chain.order.into_iter().filter(|kind| chain.runs(*kind)) {
            if let Some(index) = PedalKind::KEYED.iter().position(|keyed| *keyed == kind) {
                self.key_lags[index] = lag;
            }
            lag += kind.latency();
        }
    }
    /// Where each keyed pedal finds its key among `keys`: `None` for its
    /// own input or a pedal that does not run, `Some(None)` for silence.
    fn key_slots(&self, keys: &crate::ducker::Keys) -> [Option<Option<usize>>; 4] {
        PedalKind::KEYED.map(|kind| {
            self.chain
                .runs(kind)
                .then(|| keys.slot(self.chain.key(kind)))
                .flatten()
        })
    }
    /// Clear a mastering pedal's state and move it straight to its settings.
    fn clear(&mut self, kind: PedalKind) {
        match kind {
            PedalKind::SoftClip => self.soft_clip.clear(),
            PedalKind::Tube => self.tube.clear(),
            PedalKind::TrimEq => self.trim_eq.clear(),
            PedalKind::BusCompressor => self.bus_compressor.clear(),
            PedalKind::Maximizer => self.maximizer.clear(),
            PedalKind::LiftEq => self.lift_eq.clear(),
            PedalKind::Tape => self.tape.clear(),
            PedalKind::Imaging => self.imaging.clear(),
            PedalKind::Multiband => self.multiband.clear(),
            PedalKind::Limiter => self.limiter.clear(),
            PedalKind::BoosterEq | PedalKind::BoosterCompressor | PedalKind::BoosterWide => {}
        }
    }
    /// Begin as a board that has run with its settings for a while.
    fn settle(&mut self) {
        self.compressor.settle();
        for kind in PedalKind::ALL {
            self.clear(kind);
        }
    }
    /// Whether new settings switch or move a pedal, or bypass the board.
    fn rewired(&self, chain: &MasterChain) -> bool {
        chain.bypass != self.chain.bypass
            || chain.order != self.chain.order
            || PedalKind::ALL
                .into_iter()
                .any(|kind| chain.is_on(kind) != self.chain.is_on(kind))
    }
    #[inline]
    fn process(&mut self, left: f32, right: f32, keys: KeyFrame) -> (f32, f32) {
        let mut delayed = [None; 4];
        for (index, key) in keys.into_iter().enumerate() {
            // Keep time while a pedal follows its own input too. Otherwise
            // selecting a track again replays key audio from before the switch.
            let (key_left, key_right) = key.unwrap_or((0.0, 0.0));
            let lag = self.key_lags[index];
            let [delay_left, delay_right] = &mut self.key_delays[index];
            let frame = (
                delay_left.push_delayed(key_left, lag),
                delay_right.push_delayed(key_right, lag),
            );
            delayed[index] = key.map(|_| frame);
        }
        let (left, right) = self.run(Frame::Audio(left, right), delayed).audio();
        if self.live {
            let padding = LIVE_LATENCY - self.chain.latency();
            (
                self.padding[0].push_delayed(left, padding),
                self.padding[1].push_delayed(right, padding),
            )
        } else {
            (left, right)
        }
    }
    /// One frame through the pedals that are on, in order. As in
    /// `FX_RenderStereo`, the Booster equaliser works on floats and the other
    /// Booster pedals on integers; the samples change kind only where two
    /// neighbouring pedals differ. In the original's order the Booster pedals
    /// are the library's stereo path.
    /// `keys` holds each keyed pedal's key, already lined up with the frame.
    fn run(&mut self, mut frame: Frame, keys: KeyFrame) -> Frame {
        let [booster_key, compressor_key, multiband_key, limiter_key] = keys;
        for kind in self.chain.order {
            if !self.chain.runs(kind) {
                continue;
            }
            frame = match kind {
                PedalKind::BoosterEq => {
                    let (left, right) = frame.floats();
                    Frame::Floats(
                        self.equalizer.process(0, left),
                        self.equalizer.process(1, right),
                    )
                }
                PedalKind::BoosterCompressor => {
                    let (left, right) = frame.integers();
                    let key = booster_key.map(|(key_left, key_right)| {
                        (
                            i32::from(crate::audio::float_to_i16(key_left)),
                            i32::from(crate::audio::float_to_i16(key_right)),
                        )
                    });
                    let (left, right) = self.compressor.process_keyed(left, right, key);
                    Frame::Integers(left, right)
                }
                PedalKind::BoosterWide => {
                    let (left, right) = frame.integers();
                    let (left, right) = self.wide.process(left, right);
                    Frame::Integers(left, right)
                }
                _ => {
                    let (left, right) = frame.audio();
                    let (left, right) = match kind {
                        PedalKind::SoftClip => self.soft_clip.process(left, right),
                        PedalKind::Tube => self.tube.process(left, right),
                        PedalKind::TrimEq => self.trim_eq.process(left, right),
                        PedalKind::BusCompressor => {
                            self.bus_compressor
                                .process_keyed(left, right, compressor_key)
                        }
                        PedalKind::Maximizer => self.maximizer.process(left, right),
                        PedalKind::LiftEq => self.lift_eq.process(left, right),
                        PedalKind::Tape => self.tape.process(left, right),
                        PedalKind::Imaging => self.imaging.process(left, right),
                        PedalKind::Multiband => {
                            self.multiband.process_keyed(left, right, multiband_key)
                        }
                        _ => self.limiter.process_keyed(left, right, limiter_key),
                    };
                    Frame::Audio(left, right)
                }
            };
        }
        frame
    }
}

#[cfg(test)]
impl Board {
    fn settled(chain: &MasterChain) -> Self {
        let mut board = Self::new();
        board.apply(chain);
        board.settle();
        board
    }
    /// One frame of 16-bit samples through the pedals, then the clamp.
    fn process_integers(&mut self, left: i32, right: i32) -> (i16, i16) {
        let (left, right) = self.run(Frame::Integers(left, right), [None; 4]).integers();
        (clamp16(left), clamp16(right))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressor_switches_invalidate_only_their_track_and_old_sources_cannot_restore_it() {
        let mut project = crate::engine::Project::default();
        project.tracks[0].compressor.on = true;
        project.tracks[1].compressor.on = true;
        let control = MasterControl::new(&project.master);
        control.set_levels(&project);
        let epoch = control.compressor_epoch();
        for track in 0..2 {
            control.keep_compressor(track, Default::default(), 10_000, epoch);
        }
        project.tracks[0].compressor.on = false;
        control.set_levels(&project);
        assert!(control.kept_compressor(0).is_none());
        assert!(control.kept_compressor(1).is_some());
        control.keep_compressor(0, Default::default(), 11_000, epoch);
        assert!(control.kept_compressor(0).is_none());
        control.reset_compressors();
        control.keep_compressor(1, Default::default(), 11_000, epoch);
        assert!(control.kept_compressor(1).is_none());
    }

    #[test]
    fn live_levels_reject_an_incomplete_publication() {
        let project = crate::engine::Project::default();
        let control = MasterControl::new(&project.master);
        control.set_levels(&project);
        control.level_sequence.fetch_add(1, Ordering::SeqCst);
        control
            .master_gain
            .store(0.5_f32.to_bits(), Ordering::SeqCst);
        // The former independent reads accepted this torn publication.
        let packed = control.levels[0].load(Ordering::SeqCst);
        let old_left = f32::from_bits(packed as u32);
        let new_gain = f32::from_bits(control.master_gain.load(Ordering::SeqCst));
        assert_ne!(old_left.abs(), new_gain);
        assert!(control.levels().is_none());
    }

    #[test]
    fn live_stereo_levels_are_published_together() {
        let mut project = crate::engine::Project::default();
        let control = MasterControl::new(&project.master);
        assert_eq!(control.levels(), None);
        control.set_levels(&project);
        let done = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                for _ in 0..100_000 {
                    project.tracks[0].polarity_inverted = !project.tracks[0].polarity_inverted;
                    project.master_gain = if project.tracks[0].polarity_inverted {
                        0.25
                    } else {
                        1.0
                    };
                    control.set_levels(&project);
                }
                done.store(true, std::sync::atomic::Ordering::Release);
            });
            while !done.load(std::sync::atomic::Ordering::Acquire) {
                let Some((levels, master_gain)) = control.levels() else {
                    continue;
                };
                let [left, right] = levels[0];
                assert_eq!(
                    left, right,
                    "A level update must keep stereo polarity coherent."
                );
                assert_eq!(
                    left.abs(),
                    master_gain,
                    "Master gain must match the track snapshot."
                );
            }
        });
    }

    #[test]
    fn playback_meter_ignores_the_delay_removed_from_the_audible_output() {
        let control = MasterControl::new(&MasterChain::default());
        let mut processor = MasterProcessor::live(control.clone());
        let frames = 17_640; // One complete 400 ms loudness block.
        let latency = processor.latency();
        let mut block = vec![0.0; (frames + latency) * 2];
        for frame in 0..frames {
            let value = (frame as f32 * std::f32::consts::TAU * 1000.0 / 44_100.0).sin() * 0.1;
            block[frame * 2..frame * 2 + 2].fill(value);
        }
        for chunk in block.chunks_mut(514) {
            processor.process(chunk);
        }
        let mut reference = meters::Loudness::new();
        for &[left, right] in block[latency * 2..].as_chunks::<2>().0 {
            reference.push(left, right);
        }
        assert!(reference.integrated.is_finite());
        assert!(
            (f64::from(control.meters().integrated) - reference.integrated).abs() < 1e-5,
            "playback {}, audible {}",
            control.meters().integrated,
            reference.integrated
        );
    }

    fn interleaved(frames: &[(f32, f32)]) -> Vec<f32> {
        frames.iter().flat_map(|(l, r)| [*l, *r]).collect()
    }

    fn mastered() -> MasterChain {
        let mut chain = MasterChain::default();
        for kind in PedalKind::ALL.into_iter().filter(|kind| !kind.is_booster()) {
            chain.set_on(kind, true);
        }
        chain
    }

    #[test]
    fn an_inactive_chain_leaves_the_mix_untouched() {
        let mut block = vec![0.123_456_7, -0.987_654_3, 1.0, -1.0];
        let original = block.clone();
        let mut chain = MasterChain::default();
        chain.booster_eq.bands = [100; 10];
        chain.limiter.gain = 12.0;
        MasterProcessor::new(&chain).process(&mut block);
        assert_eq!(block, original);
        assert!(!chain.is_active());

        // Any Booster pedal on gives 16-bit samples, as the original's effects
        // do; the mastering pedals keep full precision.
        let mut wide = chain;
        wide.booster_wide.on = true;
        wide.booster_wide.spread = 0;
        MasterProcessor::new(&wide).process(&mut block);
        assert!(block.iter().all(|v| (v * 32768.0).fract() == 0.0));
        let mut lift = chain;
        lift.lift_eq.on = true;
        let mut quiet = vec![0.123_456_7, -0.087_654_3];
        MasterProcessor::new(&lift).process(&mut quiet);
        assert!(quiet.iter().any(|v| (v * 32768.0).fract() != 0.0));

        // The trim alone changes the level.
        let mut trimmed = chain;
        trimmed.trim = -6.0;
        assert!(trimmed.is_active());
        let mut half = vec![0.5, -0.5];
        MasterProcessor::new(&trimmed).process(&mut half);
        assert!((half[0] - 0.5 * db_to_gain(-6.0) as f32).abs() < 1e-6);
    }

    #[test]
    fn delaying_pedals_add_up_and_others_add_nothing() {
        let mut chain = MasterChain::default();
        assert_eq!(chain.latency(), 0);
        chain.booster_wide.on = true;
        chain.lift_eq.on = true;
        assert_eq!(chain.latency(), 0);
        chain.soft_clip.on = true;
        chain.limiter.on = true;
        assert_eq!(
            MasterProcessor::new(&chain).latency(),
            pedals::SoftClip::LATENCY + pedals::Limiter::LATENCY
        );
        assert_eq!(
            mastered().latency(),
            4 * dsp::Oversampler::LATENCY + pedals::Multiband::LATENCY + pedals::Limiter::LATENCY
        );
    }

    #[test]
    fn returning_to_a_track_key_does_not_replay_its_old_audio() {
        let mut chain = MasterChain::default();
        chain.soft_clip.on = true;
        chain.soft_clip.amount = 0.0;
        chain.bus_compressor = BusCompressor {
            on: true,
            threshold: -30.0,
            ratio: 10.0,
            attack: 0.1,
            release: 50.0,
            key: 2,
        };
        // Another pedal keeps this track in the prepared mix while the
        // compressor switches between its own input and the track.
        chain.limiter.on = true;
        chain.limiter.key = 2;
        let mut board = Box::new(Board::new());
        board.apply(&chain);
        board.settle();
        for _ in 0..4000 {
            board.process(
                0.001,
                0.001,
                [None, Some((0.8, 0.8)), None, Some((0.0, 0.0))],
            );
        }
        chain.bus_compressor.key = 0;
        board.apply(&chain);
        for _ in 0..50_000 {
            board.process(0.001, 0.001, [None, None, None, Some((0.0, 0.0))]);
        }
        let mut own = board.clone();
        chain.bus_compressor.key = 2;
        board.apply(&chain);
        let mut difference = 0.0_f32;
        for _ in 0..2000 {
            let keyed = board.process(
                0.001,
                0.001,
                [None, Some((0.0, 0.0)), None, Some((0.0, 0.0))],
            );
            let expected = own.process(0.001, 0.001, [None, None, None, Some((0.0, 0.0))]);
            difference = difference.max((keyed.0 - expected.0).abs());
        }
        assert!(
            difference < 1e-7,
            "The old key changed the output by {difference}"
        );
    }

    #[test]
    fn switching_and_moving_pedals_while_playing_does_not_click() {
        // A low tone moves at most 0.007 per frame. Switching the limiter
        // on with 12 dB of gain changes its level and delays it, and moving
        // the tube changes the sound again; without a crossfade either
        // would jump.
        let tone: Vec<f32> = (0..44_100)
            .flat_map(|i| {
                let v = (0.5 * (2.0 * std::f64::consts::PI * 100.0 * i as f64 / dsp::RATE).sin())
                    as f32;
                [v, v]
            })
            .collect();
        let mut chain = MasterChain::default();
        chain.tube.on = true;
        chain.tube.drive = 10.0;
        let control = MasterControl::new(&chain);
        let mut processor = MasterProcessor::live(control.clone());
        let mut output = Vec::new();
        for (index, block) in tone.chunks(1024).enumerate() {
            if index == 10 {
                chain.limiter.on = true;
                chain.limiter.gain = 12.0;
                control.set(&chain);
            }
            if index == 20 {
                let from = chain
                    .order
                    .iter()
                    .position(|kind| *kind == PedalKind::Tube)
                    .unwrap();
                chain.move_pedal(from, 11);
                control.set(&chain);
            }
            let mut block = block.to_vec();
            processor.process(&mut block);
            output.extend(block);
        }
        let steps = output
            .chunks(2)
            .zip(output.chunks(2).skip(1))
            .map(|(a, b)| (b[0] - a[0]).abs())
            .fold(0.0_f32, f32::max);
        assert!(steps < 0.03, "A step of {steps}");
        assert_eq!(processor.latency(), LIVE_LATENCY);
    }

    #[test]
    fn playback_measures_the_input_and_output() {
        let program = dsp::program(3 * 44_100);
        let mut chain = mastered();
        chain.trim = -2.0;
        let control = MasterControl::new(&chain);
        assert_eq!(control.meters().run, 0);
        let mut processor = MasterProcessor::live(control.clone());
        let mut audio = interleaved(&program);
        for block in audio.chunks_mut(2048) {
            processor.process(block);
        }
        let reading = control.meters();
        assert_eq!(reading.run, 1);
        // The program sits near -18 dBFS RMS, 0 VU, before the trim.
        assert!((-6.0..1.0).contains(&reading.input_vu), "{reading:?}");
        assert!(reading.momentary.is_finite() && reading.integrated.is_finite());
        assert!(
            reading.true_peak_max <= chain.limiter.ceiling + 1e-3,
            "{reading:?}"
        );
        assert!(reading.compressor_reduction > 0.0 || reading.limiter_reduction > 0.0);
        // Export measures nothing.
        let mut exported = interleaved(&program);
        MasterProcessor::new(&chain).process(&mut exported);
        assert_eq!(control.meters().run, 1);
    }

    #[test]
    fn mastering_pedals_at_their_defaults_raise_loudness_within_the_ceiling() {
        let program = dsp::program(3 * 44_100);
        let measure = |chain: &MasterChain| {
            let control = MasterControl::new(chain);
            let mut processor = MasterProcessor::live(control.clone());
            let mut audio = interleaved(&program);
            for block in audio.chunks_mut(1024) {
                processor.process(block);
            }
            control.meters()
        };
        let dry = measure(&MasterChain::default());
        let wet = measure(&mastered());
        let input_power = program
            .iter()
            .map(|(l, r)| (f64::from(*l).powi(2) + f64::from(*r).powi(2)) / 2.0)
            .sum::<f64>()
            / program.len() as f64;
        let input_db = 10.0 * input_power.log10();
        println!(
            "Input RMS {input_db:.4} dBFS; dry {:.4} LUFS; wet {:.4} LUFS; true peak {:.4} dBTP",
            dry.integrated, wet.integrated, wet.true_peak_max
        );
        assert!((input_db + 18.0).abs() < 0.01);
        assert!((wet.integrated + 11.0).abs() <= 1.0, "{wet:?}");
        assert!(wet.integrated > dry.integrated + 3.0, "{dry:?} {wet:?}");
        assert!(wet.true_peak_max <= -1.0 + 1e-3, "{wet:?}");
    }

    #[test]
    fn knob_gestures_name_the_one_knob_that_moved() {
        let chain = MasterChain::default();
        let mut moved = chain;
        Knob::CompressorAttack.set(&mut moved, 0.3);
        assert_eq!(chain.changed_knob(&moved), Some(Knob::CompressorAttack));
        Knob::BoosterBand(4).set(&mut moved, 70.4);
        assert_eq!(moved.booster_eq.bands[4], 70);
        assert_eq!(chain.changed_knob(&moved), None);
        let mut switched = chain;
        switched.tape.speed = 15;
        assert_eq!(chain.changed_knob(&switched), None);
        Knob::Trim.set(&mut switched, 99.0);
        assert_eq!(switched.trim, 24.0);
        assert_eq!(Knob::LiftAir.pedal(), Some(PedalKind::LiftEq));
        assert_eq!(Knob::Trim.pedal(), None);
        assert_eq!(Knob::all().count(), 41);
        // Every default sits inside its range, on its step.
        for knob in Knob::all() {
            let range = knob.range();
            let value = knob.get(&chain);
            assert_eq!(value, range.default, "{knob:?}");
            let steps = (value - range.low) / range.step;
            assert!((steps - steps.round()).abs() < 1e-3, "{knob:?}");
        }
    }

    #[test]
    fn settings_are_validated() {
        let mut chain = MasterChain::default();
        assert!(chain.validate().is_ok());
        chain.order[1] = PedalKind::BoosterEq;
        assert!(chain.validate().is_err());
        chain.order = PedalKind::ALL;
        chain.booster_eq.bands[3] = 101;
        assert!(chain.validate().is_err());
        chain.booster_eq.bands[3] = 100;
        chain.booster_compressor.speed = 1001;
        assert!(chain.validate().is_err());
        chain.booster_compressor.speed = 1000;
        assert!(chain.validate().is_ok());
        for (knob, value) in [
            (Knob::Trim, 24.5),
            (Knob::CompressorRatio, 1.0),
            (Knob::LimiterCeiling, 0.1),
            (Knob::TubeMix, f32::NAN),
            (Knob::ImagingWidth, f32::INFINITY),
        ] {
            let mut changed = chain;
            match knob {
                Knob::Trim => changed.trim = value,
                Knob::CompressorRatio => changed.bus_compressor.ratio = value,
                Knob::LimiterCeiling => changed.limiter.ceiling = value,
                Knob::TubeMix => changed.tube.mix = value,
                _ => changed.imaging.width = value,
            }
            assert!(changed.validate().is_err(), "{knob:?}");
        }
        chain.tape.speed = 7;
        assert!(chain.validate().is_err());
        chain.tape.speed = 30;
        chain.order.swap(11, 12);
        assert!(
            chain
                .validate()
                .unwrap_err()
                .contains("limiter must be the last")
        );
    }

    #[test]
    fn the_limiter_stays_last() {
        let mut chain = MasterChain::default();
        assert!(!PedalKind::Limiter.movable() && PedalKind::Tape.movable());
        chain.move_pedal(12, 0);
        chain.move_pedal(0, 12);
        assert_eq!(
            chain.order,
            PedalKind::ALL,
            "No move reaches the limiter's place."
        );
        chain.move_pedal(0, 11);
        assert_eq!(
            chain.order[11..],
            [PedalKind::BoosterEq, PedalKind::Limiter]
        );
    }
}
