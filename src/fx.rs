//! Send/return effects: four FX buses that the lanes feed through their
//! sends.
//!
//! Each lane sends a share of its sound, after its fader and pan, to each
//! bus. A bus holds one effect, the Echoform Delay or the Aetherform Reverb,
//! or nothing. The effect renders only its wet sound. Then come the bus's
//! return level and a fixed safety limiter, and the four returns join the
//! dry mix before the master pedalboard.
//!
//! The effects are this application's own designs. Their controls, ranges
//! and presets follow the return modules of the MixJam Electron app, whose
//! v7 projects saved them.
mod delay;
mod parts;
mod processor;
mod reverb;

pub use processor::ReturnsProcessor;
pub(crate) use processor::{LOOKAHEAD, TAIL_LIMIT};

use serde::{Deserialize, Serialize};

/// The number of FX buses, and of sends on each lane.
pub const BUSES: usize = 4;

// Delay settings ----------------------------------------------------------------------

/// A delay time as a note length: straight, dotted (one and a half), or
/// triplet (two thirds).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Division {
    #[serde(rename = "1/1")]
    Whole,
    #[serde(rename = "1/1.")]
    WholeDotted,
    #[serde(rename = "1/1T")]
    WholeTriplet,
    #[serde(rename = "1/2")]
    Half,
    #[serde(rename = "1/2.")]
    HalfDotted,
    #[serde(rename = "1/2T")]
    HalfTriplet,
    #[serde(rename = "1/4")]
    Quarter,
    #[serde(rename = "1/4.")]
    QuarterDotted,
    #[serde(rename = "1/4T")]
    QuarterTriplet,
    #[serde(rename = "1/8")]
    Eighth,
    #[serde(rename = "1/8.")]
    EighthDotted,
    #[serde(rename = "1/8T")]
    EighthTriplet,
    #[serde(rename = "1/16")]
    Sixteenth,
    #[serde(rename = "1/16.")]
    SixteenthDotted,
    #[serde(rename = "1/16T")]
    SixteenthTriplet,
}
impl Division {
    pub const ALL: [Self; 15] = [
        Self::Whole,
        Self::WholeDotted,
        Self::WholeTriplet,
        Self::Half,
        Self::HalfDotted,
        Self::HalfTriplet,
        Self::Quarter,
        Self::QuarterDotted,
        Self::QuarterTriplet,
        Self::Eighth,
        Self::EighthDotted,
        Self::EighthTriplet,
        Self::Sixteenth,
        Self::SixteenthDotted,
        Self::SixteenthTriplet,
    ];
    pub fn label(self) -> &'static str {
        [
            "1/1", "1/1.", "1/1T", "1/2", "1/2.", "1/2T", "1/4", "1/4.", "1/4T", "1/8", "1/8.",
            "1/8T", "1/16", "1/16.", "1/16T",
        ][self as usize]
    }
    /// The length in quarter notes: a straight 1/N lasts 4/N of them.
    pub fn quarters(self) -> f64 {
        let index = self as usize;
        let straight = 4.0 / f64::from(1u32 << (index / 3));
        straight * [1.0, 1.5, 2.0 / 3.0][index % 3]
    }
    pub fn seconds(self, bpm: f64) -> f64 {
        self.quarters() * 60.0 / bpm
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeMode {
    /// Note lengths at the song's tempo.
    Sync,
    /// Times in milliseconds.
    Free,
}

/// How the repeats change as they circulate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DelayCharacter {
    /// Clean repeats; a time change crossfades between two read heads.
    Digital,
    /// Mild saturation and softening; times glide.
    Analog,
    /// Stronger, asymmetric saturation, high loss, wow and flutter; times
    /// glide slowly.
    Tape,
}

/// The Echoform Delay: a stereo delay with filters and saturation inside
/// its feedback loop, so each repeat is darker and rougher than the last.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Echoform {
    pub mode: TimeMode,
    pub division_l: Division,
    pub division_r: Division,
    /// Free times, in milliseconds. Each mode keeps its own times.
    pub time_ms_l: f32,
    pub time_ms_r: f32,
    /// Loop gain in percent; above 100 the loop's soft limiter holds it.
    pub feedback: f32,
    /// Repeats cross to the other side, and the input enters on the left.
    pub ping_pong: bool,
    /// Width of the repeats in percent: 0 is mono, 100 unchanged.
    pub width: f32,
    /// The loop's high-pass and low-pass corners, in Hz.
    pub low_cut: f32,
    pub high_cut: f32,
    /// Delay-time modulation: rate in Hz and peak deviation in milliseconds.
    pub mod_rate: f32,
    pub mod_depth: f32,
    pub character: DelayCharacter,
    /// Saturation of the input before the delay, in percent.
    pub drive: f32,
    /// How far loud input pushes the repeats down, in percent, and how
    /// fast they come back, in milliseconds.
    pub duck_amount: f32,
    pub duck_release: f32,
    /// The level of the repeats, in dB.
    pub output_db: f32,
    /// Silence the output while the loop keeps running.
    pub bypass: bool,
}

/// A continuous control of the Echoform Delay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DelayParam {
    TimeL,
    TimeR,
    Feedback,
    Width,
    LowCut,
    HighCut,
    ModRate,
    ModDepth,
    Drive,
    DuckAmount,
    DuckRelease,
    Output,
}

/// A continuous control's range, default, and step. A logarithmic control
/// moves by ratios.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub low: f32,
    pub high: f32,
    pub default: f32,
    pub step: f32,
    pub log: bool,
}
const fn range(low: f32, high: f32, default: f32, step: f32) -> Range {
    Range {
        low,
        high,
        default,
        step,
        log: false,
    }
}
const fn log_range(low: f32, high: f32, default: f32, step: f32) -> Range {
    Range {
        low,
        high,
        default,
        step,
        log: true,
    }
}
fn held(value: f32, range: Range) -> f32 {
    if value.is_nan() {
        range.default
    } else {
        value.clamp(range.low, range.high)
    }
}

impl DelayParam {
    pub const ALL: [Self; 12] = [
        Self::TimeL,
        Self::TimeR,
        Self::Feedback,
        Self::Width,
        Self::LowCut,
        Self::HighCut,
        Self::ModRate,
        Self::ModDepth,
        Self::Drive,
        Self::DuckAmount,
        Self::DuckRelease,
        Self::Output,
    ];
    pub fn range(self) -> Range {
        let defaults = Echoform::default();
        match self {
            Self::TimeL => log_range(1.0, 2000.0, defaults.time_ms_l, 1.0),
            Self::TimeR => log_range(1.0, 2000.0, defaults.time_ms_r, 1.0),
            Self::Feedback => range(0.0, 110.0, defaults.feedback, 1.0),
            Self::Width => range(0.0, 200.0, defaults.width, 1.0),
            Self::LowCut => log_range(20.0, 2000.0, defaults.low_cut, 1.0),
            Self::HighCut => log_range(1000.0, 20_000.0, defaults.high_cut, 10.0),
            Self::ModRate => log_range(0.05, 8.0, defaults.mod_rate, 0.01),
            Self::ModDepth => range(0.0, 20.0, defaults.mod_depth, 0.1),
            Self::Drive => range(0.0, 100.0, defaults.drive, 1.0),
            Self::DuckAmount => range(0.0, 100.0, defaults.duck_amount, 1.0),
            Self::DuckRelease => log_range(50.0, 2500.0, defaults.duck_release, 1.0),
            Self::Output => range(-24.0, 12.0, defaults.output_db, 0.1),
        }
    }
    pub fn get(self, delay: &Echoform) -> f32 {
        match self {
            Self::TimeL => delay.time_ms_l,
            Self::TimeR => delay.time_ms_r,
            Self::Feedback => delay.feedback,
            Self::Width => delay.width,
            Self::LowCut => delay.low_cut,
            Self::HighCut => delay.high_cut,
            Self::ModRate => delay.mod_rate,
            Self::ModDepth => delay.mod_depth,
            Self::Drive => delay.drive,
            Self::DuckAmount => delay.duck_amount,
            Self::DuckRelease => delay.duck_release,
            Self::Output => delay.output_db,
        }
    }
    /// Set the control, held to its range.
    pub fn set(self, delay: &mut Echoform, value: f32) {
        let value = held(value, self.range());
        *match self {
            Self::TimeL => &mut delay.time_ms_l,
            Self::TimeR => &mut delay.time_ms_r,
            Self::Feedback => &mut delay.feedback,
            Self::Width => &mut delay.width,
            Self::LowCut => &mut delay.low_cut,
            Self::HighCut => &mut delay.high_cut,
            Self::ModRate => &mut delay.mod_rate,
            Self::ModDepth => &mut delay.mod_depth,
            Self::Drive => &mut delay.drive,
            Self::DuckAmount => &mut delay.duck_amount,
            Self::DuckRelease => &mut delay.duck_release,
            Self::Output => &mut delay.output_db,
        } = value;
    }
}

/// The Wide Tape Echo preset.
impl Default for Echoform {
    fn default() -> Self {
        Self {
            mode: TimeMode::Sync,
            division_l: Division::Quarter,
            division_r: Division::EighthDotted,
            time_ms_l: 420.0,
            time_ms_r: 610.0,
            feedback: 68.0,
            ping_pong: true,
            width: 142.0,
            low_cut: 160.0,
            high_cut: 7800.0,
            mod_rate: 0.38,
            mod_depth: 5.4,
            character: DelayCharacter::Tape,
            drive: 0.0,
            duck_amount: 34.0,
            duck_release: 620.0,
            output_db: -1.5,
            bypass: false,
        }
    }
}

impl Echoform {
    /// The factory presets and the return level each sets, in percent.
    pub const PRESETS: [(&'static str, f32); 6] = [
        ("Wide Tape Echo", 82.0),
        ("Clean Slap", 72.0),
        ("Dotted Motion", 78.0),
        ("Dub Feedback", 90.0),
        ("Ducked Eighths", 86.0),
        ("Endless Wash", 96.0),
    ];
    /// A preset's settings, or `None` for an unknown name.
    pub fn preset(name: &str) -> Option<Self> {
        use DelayCharacter::{Analog, Digital, Tape};
        use Division::*;
        let base = Self::default();
        Some(match name {
            "Wide Tape Echo" => base,
            "Clean Slap" => Self {
                mode: TimeMode::Free,
                division_l: Sixteenth,
                division_r: Sixteenth,
                time_ms_l: 96.0,
                time_ms_r: 124.0,
                feedback: 18.0,
                ping_pong: false,
                width: 118.0,
                low_cut: 90.0,
                high_cut: 17_200.0,
                mod_rate: 0.12,
                mod_depth: 0.6,
                character: Digital,
                duck_amount: 12.0,
                duck_release: 180.0,
                output_db: -2.2,
                ..base
            },
            "Dotted Motion" => Self {
                division_l: EighthDotted,
                division_r: QuarterTriplet,
                time_ms_l: 430.0,
                time_ms_r: 350.0,
                feedback: 54.0,
                width: 168.0,
                low_cut: 240.0,
                high_cut: 9600.0,
                mod_rate: 0.74,
                mod_depth: 2.8,
                character: Analog,
                duck_amount: 46.0,
                duck_release: 480.0,
                output_db: -1.8,
                ..base
            },
            "Dub Feedback" => Self {
                division_l: QuarterDotted,
                division_r: HalfTriplet,
                time_ms_l: 680.0,
                time_ms_r: 870.0,
                feedback: 94.0,
                width: 136.0,
                low_cut: 310.0,
                high_cut: 4400.0,
                mod_rate: 0.24,
                mod_depth: 7.6,
                character: Tape,
                drive: 32.0,
                duck_amount: 18.0,
                duck_release: 980.0,
                output_db: -4.1,
                ..base
            },
            "Ducked Eighths" => Self {
                division_l: Eighth,
                division_r: EighthDotted,
                time_ms_l: 234.0,
                time_ms_r: 351.0,
                feedback: 47.0,
                width: 152.0,
                low_cut: 190.0,
                high_cut: 11_200.0,
                mod_rate: 0.18,
                mod_depth: 1.2,
                character: Digital,
                duck_amount: 72.0,
                duck_release: 340.0,
                output_db: -1.0,
                ..base
            },
            "Endless Wash" => Self {
                mode: TimeMode::Free,
                division_l: Half,
                division_r: HalfDotted,
                time_ms_l: 980.0,
                time_ms_r: 1330.0,
                feedback: 110.0,
                width: 188.0,
                low_cut: 420.0,
                high_cut: 5200.0,
                mod_rate: 0.11,
                mod_depth: 9.8,
                character: Tape,
                duck_amount: 6.0,
                duck_release: 1900.0,
                output_db: -7.5,
                ..base
            },
            _ => return None,
        })
    }
    /// The left and right delay times in seconds at a tempo.
    pub fn seconds(&self, bpm: f64) -> [f64; 2] {
        match self.mode {
            TimeMode::Sync => [self.division_l.seconds(bpm), self.division_r.seconds(bpm)],
            TimeMode::Free => [
                f64::from(self.time_ms_l) / 1000.0,
                f64::from(self.time_ms_r) / 1000.0,
            ],
        }
    }
    fn validate(&self) -> Result<(), String> {
        if DelayParam::ALL.into_iter().any(|param| {
            let (value, range) = (param.get(self), param.range());
            !(value.is_finite() && (range.low..=range.high).contains(&value))
        }) {
            return Err("An Echoform Delay setting is out of range.".into());
        }
        Ok(())
    }
}

// Reverb settings ----------------------------------------------------------------------

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

/// The Aetherform Reverb: early reflections and an eight-line feedback
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
    pub output_db: f32,
    pub bypass: bool,
}

/// A continuous control of the Aetherform Reverb.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReverbParam {
    PreDelay,
    Decay,
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
    DuckAmount,
    DuckRelease,
    Output,
}
impl ReverbParam {
    pub const ALL: [Self; 16] = [
        Self::PreDelay,
        Self::Decay,
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
        Self::DuckAmount,
        Self::DuckRelease,
        Self::Output,
    ];
    pub fn range(self) -> Range {
        let d = Aetherform::default();
        match self {
            Self::PreDelay => range(0.0, 250.0, d.pre_delay_ms, 1.0),
            Self::Decay => log_range(0.2, 30.0, d.decay_seconds, 0.01),
            Self::Size => range(5.0, 100.0, d.size_percent, 1.0),
            Self::Drive => range(0.0, 100.0, d.drive_percent, 1.0),
            Self::Width => range(0.0, 200.0, d.width_percent, 1.0),
            Self::LateBalance => range(0.0, 100.0, d.late_balance_percent, 1.0),
            Self::LowCut => log_range(20.0, 2000.0, d.low_cut_hz, 1.0),
            Self::HighCut => log_range(1000.0, 20_000.0, d.high_cut_hz, 10.0),
            Self::Diffusion => range(0.0, 100.0, d.diffusion_percent, 1.0),
            Self::Density => range(0.0, 100.0, d.density_percent, 1.0),
            Self::ModRate => log_range(0.05, 3.0, d.mod_rate_hz, 0.01),
            Self::ModDepth => range(0.0, 100.0, d.mod_depth_percent, 1.0),
            Self::ShimmerAmount => range(0.0, 100.0, d.shimmer_amount_percent, 1.0),
            Self::DuckAmount => range(0.0, 100.0, d.duck_amount_percent, 1.0),
            Self::DuckRelease => log_range(50.0, 2500.0, d.duck_release_ms, 1.0),
            Self::Output => range(-24.0, 12.0, d.output_db, 0.1),
        }
    }
    pub fn get(self, reverb: &Aetherform) -> f32 {
        match self {
            Self::PreDelay => reverb.pre_delay_ms,
            Self::Decay => reverb.decay_seconds,
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
            Self::DuckAmount => reverb.duck_amount_percent,
            Self::DuckRelease => reverb.duck_release_ms,
            Self::Output => reverb.output_db,
        }
    }
    /// Set the control, held to its range.
    pub fn set(self, reverb: &mut Aetherform, value: f32) {
        let value = held(value, self.range());
        *match self {
            Self::PreDelay => &mut reverb.pre_delay_ms,
            Self::Decay => &mut reverb.decay_seconds,
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
            Self::DuckAmount => &mut reverb.duck_amount_percent,
            Self::DuckRelease => &mut reverb.duck_release_ms,
            Self::Output => &mut reverb.output_db,
        } = value;
    }
}

/// The Warm Chamber preset.
impl Default for Aetherform {
    fn default() -> Self {
        Self {
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
            output_db: -1.5,
            bypass: false,
        }
    }
}

impl Aetherform {
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
                duck_amount_percent: 0.0,
                duck_release_ms: 2500.0,
                output_db: -7.0,
                ..base
            },
            _ => return None,
        })
    }
    fn validate(&self) -> Result<(), String> {
        if !SHIMMER_INTERVALS.contains(&self.shimmer_interval_semitones)
            || ReverbParam::ALL.into_iter().any(|param| {
                let (value, range) = (param.get(self), param.range());
                !(value.is_finite() && (range.low..=range.high).contains(&value))
            })
        {
            return Err("An Aetherform Reverb setting is out of range.".into());
        }
        Ok(())
    }
}

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
}
impl Module {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Empty => "Empty",
            Self::EchoformDelay(_) => "Echoform Delay",
            Self::AetherformReverb(_) => "Aetherform Reverb",
        }
    }
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
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
    fn validate(&self) -> Result<(), String> {
        if !(self.return_level.is_finite() && (0.0..=1.0).contains(&self.return_level)) {
            return Err("An FX return level must be from 0 to 1.".into());
        }
        match &self.module {
            Module::Empty => Ok(()),
            Module::EchoformDelay(delay) => delay.validate(),
            Module::AetherformReverb(reverb) => reverb.validate(),
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
mod tests {
    use super::*;

    #[test]
    fn divisions_follow_note_lengths() {
        let at = |division: Division| division.seconds(120.0);
        assert_eq!(at(Division::Quarter), 0.5);
        assert_eq!(at(Division::EighthDotted), 0.375);
        assert!((at(Division::QuarterTriplet) - 1.0 / 3.0).abs() < 1e-12);
        assert_eq!(at(Division::Whole), 2.0);
        assert_eq!(at(Division::SixteenthDotted), 0.1875);
        let labels: Vec<_> = Division::ALL.iter().map(|d| d.label()).collect();
        assert_eq!(labels[0], "1/1");
        assert_eq!(labels[14], "1/16T");
        for division in Division::ALL {
            let json = serde_json::to_string(&division).unwrap();
            assert_eq!(json, format!("\"{}\"", division.label()));
        }
    }

    #[test]
    fn every_preset_is_valid_and_the_defaults_are_the_first() {
        assert_eq!(
            Echoform::preset("Wide Tape Echo"),
            Some(Echoform::default())
        );
        assert_eq!(
            Aetherform::preset("Warm Chamber"),
            Some(Aetherform::default())
        );
        for (name, mix) in Echoform::PRESETS {
            Echoform::preset(name).unwrap().validate().unwrap();
            assert!((0.0..=100.0).contains(&mix));
        }
        for (name, mix) in Aetherform::PRESETS {
            Aetherform::preset(name).unwrap().validate().unwrap();
            assert!((0.0..=100.0).contains(&mix));
        }
        assert_eq!(Echoform::preset("Nope"), None);
    }

    #[test]
    fn buses_save_with_the_v7_module_names() {
        let mut buses = FxBuses::default();
        buses.buses[1].module = Module::EchoformDelay(Echoform::default());
        buses.buses[2].module = Module::AetherformReverb(Aetherform::default());
        let json = serde_json::to_value(buses).unwrap();
        assert_eq!(json[0]["module"], serde_json::json!({"type": "empty"}));
        assert_eq!(json[1]["module"]["type"], "echoform-delay");
        assert_eq!(json[1]["module"]["divisionR"], "1/8.");
        assert_eq!(json[2]["module"]["type"], "aetherform-reverb");
        assert_eq!(json[2]["module"]["shimmerIntervalSemitones"], 12);
        assert_eq!(json[0]["returnLevel"], 1.0);
        let back: FxBuses = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(back, buses);
        let mut extra = json;
        extra[1]["module"]["mix"] = serde_json::json!(1);
        assert!(serde_json::from_value::<FxBuses>(extra).is_err());
    }

    #[test]
    fn controls_hold_their_ranges() {
        let mut delay = Echoform::default();
        DelayParam::Feedback.set(&mut delay, 500.0);
        assert_eq!(delay.feedback, 110.0);
        DelayParam::TimeL.set(&mut delay, f32::NAN);
        assert_eq!(delay.time_ms_l, 420.0);
        let mut reverb = Aetherform::default();
        ReverbParam::Decay.set(&mut reverb, 0.0);
        assert_eq!(reverb.decay_seconds, 0.2);
        reverb.shimmer_interval_semitones = 5;
        assert!(reverb.validate().is_err());
        let mut bus = FxBus {
            return_level: 1.5,
            ..FxBus::default()
        };
        assert!(bus.validate().is_err());
        bus.return_level = 0.5;
        bus.validate().unwrap();
    }
}
