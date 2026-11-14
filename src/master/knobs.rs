//! The pedals of the board and their knobs: the pedal kinds, the first
//! order, each knob's range and step, and how a knob reads and writes a
//! chain.
use super::{EQ_BAND_MAX, KNOB_MAX, MasterChain, pedals};
use serde::{Deserialize, Serialize};

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
