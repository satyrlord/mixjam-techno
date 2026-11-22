//! The Aetherform Reverb's settings, controls, and presets.
use super::range::{
    CommonRanges, Range, high_cut_range, is_own, log_range, low_cut_range, range, width_range,
};
use serde::{Deserialize, Serialize};

/// The shape of the space: its delay lengths and early reflections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpaceModel {
    Room,
    Hall,
    Plate,
    Chamber,
}
impl SpaceModel {
    pub const ALL: [Self; 4] = [Self::Room, Self::Hall, Self::Plate, Self::Chamber];
    pub fn label(self) -> &'static str {
        match self {
            Self::Room => "Room",
            Self::Hall => "Hall",
            Self::Plate => "Plate",
            Self::Chamber => "Chamber",
        }
    }
}

/// How the tail sounds as it decays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReverbCharacter {
    /// Neutral.
    Natural,
    /// Soft saturation, extra damping, and a slow wander.
    Vintage,
    /// A soft onset and a tail that opens up, with slower, wider motion.
    Bloom,
}

/// The shimmer's pitch steps in semitones: a fifth, one octave, an octave
/// and a fifth, and two octaves.
pub const SHIMMER_INTERVALS: [u8; 4] = [7, 12, 19, 24];

/// The Aetherform Reverb: early reflections and a sixteen-line feedback
/// delay network, with a pitch-shifted shimmer branch.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Aetherform {
    pub space_model: SpaceModel,
    pub pre_delay_ms: f32,
    /// The time the tail takes to fall by 60 dB in the midband.
    pub decay_seconds: f32,
    /// The size of the space in percent. It does not change the decay.
    pub size_percent: f32,
    pub character: ReverbCharacter,
    /// Saturation of the input before the reverb, in percent.
    pub drive_percent: f32,
    pub width_percent: f32,
    /// From early reflections only (0) to the late tail only (100).
    pub late_balance_percent: f32,
    pub low_cut_hz: f32,
    pub high_cut_hz: f32,
    /// How fast reflections blur into a smooth field.
    pub diffusion_percent: f32,
    /// How many reflections there are, and how full the tail is.
    pub density_percent: f32,
    pub early_reflections_enabled: bool,
    pub mod_rate_hz: f32,
    pub mod_depth_percent: f32,
    pub shimmer_enabled: bool,
    /// Kept while the shimmer is off.
    pub shimmer_amount_percent: f32,
    pub shimmer_interval_semitones: u8,
    pub duck_amount_percent: f32,
    pub duck_release_ms: f32,
    /// What the ducking follows: 0 is the bus's own input, 1 to 64 a
    /// track as it sounds after its fader.
    #[serde(default, skip_serializing_if = "is_own")]
    pub duck_key: u8,
    /// Sidechain/self ducking threshold in dBFS.
    pub duck_threshold_db: f32,
    /// Low-frequency decay multiplier relative to midband decay.
    pub bass_mult: f32,
    /// High-frequency decay damping multiplier inside the tank.
    pub treble_mult: f32,
    pub output_db: f32,
    pub bypass: bool,
}

/// A continuous control of the Aetherform Reverb.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReverbParam {
    PreDelay,
    Decay,
    BassMult,
    TrebleMult,
    Size,
    Drive,
    Width,
    LateBalance,
    LowCut,
    HighCut,
    Diffusion,
    Density,
    ModRate,
    ModDepth,
    ShimmerAmount,
    DuckThreshold,
    DuckAmount,
    DuckRelease,
    DuckKey,
    Output,
}
impl ReverbParam {
    pub const ALL: [Self; 20] = [
        Self::PreDelay,
        Self::Decay,
        Self::BassMult,
        Self::TrebleMult,
        Self::Size,
        Self::Drive,
        Self::Width,
        Self::LateBalance,
        Self::LowCut,
        Self::HighCut,
        Self::Diffusion,
        Self::Density,
        Self::ModRate,
        Self::ModDepth,
        Self::ShimmerAmount,
        Self::DuckThreshold,
        Self::DuckAmount,
        Self::DuckRelease,
        Self::DuckKey,
        Self::Output,
    ];
    const RANGES: [Range; 20] = {
        let d = Aetherform::DEFAULT;
        CommonRanges::new(
            d.duck_threshold_db,
            d.duck_amount_percent,
            d.duck_release_ms,
            d.output_db,
        )
        .table([
            (
                Self::PreDelay as usize,
                range(0.0, 250.0, d.pre_delay_ms, 1.0),
            ),
            (
                Self::Decay as usize,
                log_range(0.2, 30.0, d.decay_seconds, 0.01),
            ),
            (Self::BassMult as usize, range(0.25, 2.5, d.bass_mult, 0.05)),
            (
                Self::TrebleMult as usize,
                range(0.2, 1.5, d.treble_mult, 0.05),
            ),
            (Self::Size as usize, range(5.0, 100.0, d.size_percent, 1.0)),
            (
                Self::Drive as usize,
                range(0.0, 100.0, d.drive_percent, 1.0),
            ),
            (Self::Width as usize, width_range(d.width_percent)),
            (
                Self::LateBalance as usize,
                range(0.0, 100.0, d.late_balance_percent, 1.0),
            ),
            (Self::LowCut as usize, low_cut_range(d.low_cut_hz)),
            (Self::HighCut as usize, high_cut_range(d.high_cut_hz)),
            (
                Self::Diffusion as usize,
                range(0.0, 100.0, d.diffusion_percent, 1.0),
            ),
            (
                Self::Density as usize,
                range(0.0, 100.0, d.density_percent, 1.0),
            ),
            (
                Self::ModRate as usize,
                log_range(0.05, 3.0, d.mod_rate_hz, 0.01),
            ),
            (
                Self::ModDepth as usize,
                range(0.0, 100.0, d.mod_depth_percent, 1.0),
            ),
            (
                Self::ShimmerAmount as usize,
                range(0.0, 100.0, d.shimmer_amount_percent, 1.0),
            ),
        ])
    };
    pub const fn range(self) -> Range {
        Self::RANGES[self as usize]
    }
    pub fn get(self, reverb: &Aetherform) -> f32 {
        match self {
            Self::PreDelay => reverb.pre_delay_ms,
            Self::Decay => reverb.decay_seconds,
            Self::BassMult => reverb.bass_mult,
            Self::TrebleMult => reverb.treble_mult,
            Self::Size => reverb.size_percent,
            Self::Drive => reverb.drive_percent,
            Self::Width => reverb.width_percent,
            Self::LateBalance => reverb.late_balance_percent,
            Self::LowCut => reverb.low_cut_hz,
            Self::HighCut => reverb.high_cut_hz,
            Self::Diffusion => reverb.diffusion_percent,
            Self::Density => reverb.density_percent,
            Self::ModRate => reverb.mod_rate_hz,
            Self::ModDepth => reverb.mod_depth_percent,
            Self::ShimmerAmount => reverb.shimmer_amount_percent,
            Self::DuckThreshold => reverb.duck_threshold_db,
            Self::DuckAmount => reverb.duck_amount_percent,
            Self::DuckRelease => reverb.duck_release_ms,
            Self::DuckKey => f32::from(reverb.duck_key),
            Self::Output => reverb.output_db,
        }
    }
    /// Set the control, held to its range.
    pub fn set(self, reverb: &mut Aetherform, value: f32) {
        let value = self.range().hold(value);
        if self == Self::DuckKey {
            reverb.duck_key = value.round() as u8;
            return;
        }
        *match self {
            Self::PreDelay => &mut reverb.pre_delay_ms,
            Self::Decay => &mut reverb.decay_seconds,
            Self::BassMult => &mut reverb.bass_mult,
            Self::TrebleMult => &mut reverb.treble_mult,
            Self::Size => &mut reverb.size_percent,
            Self::Drive => &mut reverb.drive_percent,
            Self::Width => &mut reverb.width_percent,
            Self::LateBalance => &mut reverb.late_balance_percent,
            Self::LowCut => &mut reverb.low_cut_hz,
            Self::HighCut => &mut reverb.high_cut_hz,
            Self::Diffusion => &mut reverb.diffusion_percent,
            Self::Density => &mut reverb.density_percent,
            Self::ModRate => &mut reverb.mod_rate_hz,
            Self::ModDepth => &mut reverb.mod_depth_percent,
            Self::ShimmerAmount => &mut reverb.shimmer_amount_percent,
            Self::DuckThreshold => &mut reverb.duck_threshold_db,
            Self::DuckAmount => &mut reverb.duck_amount_percent,
            Self::DuckRelease => &mut reverb.duck_release_ms,
            Self::DuckKey => unreachable!(),
            Self::Output => &mut reverb.output_db,
        } = value;
    }
}

impl Default for Aetherform {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Aetherform {
    /// The Warm Chamber preset.
    const DEFAULT: Self = Self {
        space_model: SpaceModel::Chamber,
        pre_delay_ms: 24.0,
        decay_seconds: 2.8,
        size_percent: 68.0,
        character: ReverbCharacter::Vintage,
        drive_percent: 0.0,
        width_percent: 148.0,
        late_balance_percent: 72.0,
        low_cut_hz: 180.0,
        high_cut_hz: 8600.0,
        diffusion_percent: 78.0,
        density_percent: 84.0,
        early_reflections_enabled: true,
        mod_rate_hz: 0.32,
        mod_depth_percent: 18.0,
        shimmer_enabled: false,
        shimmer_amount_percent: 24.0,
        shimmer_interval_semitones: 12,
        duck_amount_percent: 28.0,
        duck_release_ms: 720.0,
        duck_key: 0,
        duck_threshold_db: -24.0,
        bass_mult: 1.0,
        treble_mult: 1.0,
        output_db: -1.5,
        bypass: false,
    };
    /// The factory presets and the return level each sets, in percent.
    pub const PRESETS: [(&'static str, f32); 7] = [
        ("Warm Chamber", 88.0),
        ("Vocal Plate", 82.0),
        ("Dark Hall", 92.0),
        ("Small Room", 74.0),
        ("Ambient Bloom", 96.0),
        ("Shimmer Cloud", 98.0),
        ("Endless Cathedral", 100.0),
    ];
    /// A preset's settings, or `None` for an unknown name.
    pub fn preset(name: &str) -> Option<Self> {
        use ReverbCharacter::{Bloom, Natural, Vintage};
        use SpaceModel::{Hall, Plate, Room};
        let base = Self::default();
        Some(match name {
            "Warm Chamber" => base,
            "Vocal Plate" => Self {
                space_model: Plate,
                character: Natural,
                pre_delay_ms: 56.0,
                decay_seconds: 1.9,
                size_percent: 52.0,
                width_percent: 132.0,
                late_balance_percent: 62.0,
                low_cut_hz: 220.0,
                high_cut_hz: 12_500.0,
                diffusion_percent: 86.0,
                density_percent: 90.0,
                mod_rate_hz: 0.18,
                mod_depth_percent: 9.0,
                shimmer_amount_percent: 14.0,
                bass_mult: 0.65,
                treble_mult: 1.05,
                duck_amount_percent: 46.0,
                duck_release_ms: 420.0,
                output_db: -2.0,
                ..base
            },
            "Dark Hall" => Self {
                space_model: Hall,
                character: Vintage,
                pre_delay_ms: 38.0,
                decay_seconds: 5.6,
                size_percent: 88.0,
                width_percent: 164.0,
                late_balance_percent: 82.0,
                low_cut_hz: 260.0,
                high_cut_hz: 4800.0,
                diffusion_percent: 70.0,
                density_percent: 76.0,
                mod_rate_hz: 0.21,
                mod_depth_percent: 24.0,
                shimmer_amount_percent: 18.0,
                bass_mult: 1.25,
                treble_mult: 0.7,
                shimmer_interval_semitones: 7,
                duck_amount_percent: 18.0,
                duck_release_ms: 1100.0,
                output_db: -3.5,
                ..base
            },
            "Small Room" => Self {
                space_model: Room,
                character: Natural,
                pre_delay_ms: 8.0,
                decay_seconds: 0.7,
                size_percent: 28.0,
                width_percent: 110.0,
                late_balance_percent: 48.0,
                low_cut_hz: 120.0,
                high_cut_hz: 14_800.0,
                diffusion_percent: 58.0,
                density_percent: 64.0,
                mod_rate_hz: 0.12,
                mod_depth_percent: 4.0,
                shimmer_amount_percent: 0.0,
                bass_mult: 0.75,
                treble_mult: 0.85,
                duck_amount_percent: 12.0,
                duck_release_ms: 220.0,
                output_db: -1.0,
                ..base
            },
            "Ambient Bloom" => Self {
                space_model: Hall,
                character: Bloom,
                pre_delay_ms: 72.0,
                decay_seconds: 9.5,
                size_percent: 96.0,
                width_percent: 188.0,
                late_balance_percent: 91.0,
                low_cut_hz: 340.0,
                high_cut_hz: 7200.0,
                diffusion_percent: 94.0,
                density_percent: 98.0,
                mod_rate_hz: 0.14,
                mod_depth_percent: 42.0,
                shimmer_enabled: true,
                shimmer_amount_percent: 38.0,
                bass_mult: 1.15,
                treble_mult: 0.8,
                duck_amount_percent: 8.0,
                duck_release_ms: 1800.0,
                output_db: -5.2,
                ..base
            },
            "Shimmer Cloud" => Self {
                space_model: Hall,
                character: Bloom,
                pre_delay_ms: 84.0,
                decay_seconds: 12.4,
                size_percent: 100.0,
                width_percent: 196.0,
                late_balance_percent: 94.0,
                low_cut_hz: 460.0,
                high_cut_hz: 9800.0,
                diffusion_percent: 98.0,
                density_percent: 100.0,
                early_reflections_enabled: false,
                mod_rate_hz: 0.1,
                mod_depth_percent: 36.0,
                shimmer_enabled: true,
                shimmer_amount_percent: 72.0,
                bass_mult: 0.6,
                treble_mult: 1.1,
                duck_amount_percent: 4.0,
                duck_release_ms: 2200.0,
                output_db: -6.0,
                ..base
            },
            "Endless Cathedral" => Self {
                space_model: Hall,
                character: Bloom,
                pre_delay_ms: 110.0,
                decay_seconds: 30.0,
                size_percent: 100.0,
                width_percent: 200.0,
                late_balance_percent: 96.0,
                low_cut_hz: 420.0,
                high_cut_hz: 5600.0,
                diffusion_percent: 100.0,
                density_percent: 100.0,
                early_reflections_enabled: false,
                mod_rate_hz: 0.08,
                mod_depth_percent: 55.0,
                shimmer_enabled: true,
                shimmer_amount_percent: 84.0,
                shimmer_interval_semitones: 19,
                bass_mult: 1.35,
                treble_mult: 0.65,
                duck_amount_percent: 0.0,
                duck_release_ms: 2500.0,
                output_db: -7.0,
                ..base
            },
            _ => return None,
        })
    }
    pub(in crate::fx) fn validate(&self) -> Result<(), String> {
        if !SHIMMER_INTERVALS.contains(&self.shimmer_interval_semitones)
            || !ReverbParam::ALL
                .into_iter()
                .all(|param| param.range().contains(param.get(self)))
        {
            return Err("An Aetherform Reverb setting is out of range.".into());
        }
        Ok(())
    }
}
