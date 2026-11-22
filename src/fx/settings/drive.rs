//! The Emberform Drive's settings, controls, and presets.
use super::range::{
    CommonRanges, Range, high_cut_range, is_own, log_range, low_cut_range, range, width_range,
};
use serde::{Deserialize, Serialize};

/// The curve that the Emberform Drive bends its input through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistortionShape {
    /// A soft curve whose negative half bends later: warm, even harmonics.
    Tube,
    /// A symmetric soft curve with a softened top: thick and round.
    Tape,
    /// A cubic soft clip with a firm knee: the crunch of a pedal.
    Overdrive,
    /// An exponential clip after 12 dB of extra gain: dense and buzzing.
    Fuzz,
    /// A straight cut at full scale: hard, bright, and aggressive.
    HardClip,
    /// A sine fold: louder input folds back over itself into bell-like overtones.
    Foldback,
    /// A full-wave rectifier: the sound an octave up, with grit.
    Rectifier,
    /// Fewer bits and a lower sample rate: steps and aliasing on purpose.
    Bitcrush,
}
impl DistortionShape {
    pub const ALL: [Self; 8] = [
        Self::Tube,
        Self::Tape,
        Self::Overdrive,
        Self::Fuzz,
        Self::HardClip,
        Self::Foldback,
        Self::Rectifier,
        Self::Bitcrush,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Tube => "Tube",
            Self::Tape => "Tape",
            Self::Overdrive => "Overdrive",
            Self::Fuzz => "Fuzz",
            Self::HardClip => "Hard clip",
            Self::Foldback => "Foldback",
            Self::Rectifier => "Rectifier",
            Self::Bitcrush => "Bitcrush",
        }
    }
}

/// The Emberform Drive: a parallel distortion with eight curves, a low cut
/// before the curve and a high cut after it. Its result is held at the
/// level of what it hears, so the drive changes the tone, not the level.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Emberform {
    pub shape: DistortionShape,
    /// The gain into the curve, in dB.
    pub drive_db: f32,
    /// An offset into the curve that bends one half of the wave more than
    /// the other, in percent: more even harmonics.
    pub asymmetry_percent: f32,
    /// The high-pass before the curve, in Hz.
    pub low_cut_hz: f32,
    /// The low-pass after the curve, in Hz.
    pub high_cut_hz: f32,
    /// The crusher's bit depth, from 1 to 16.
    pub bits: u8,
    /// The crusher's sample rate, in Hz.
    pub crush_rate_hz: f32,
    pub width_percent: f32,
    pub duck_amount_percent: f32,
    pub duck_release_ms: f32,
    /// What the ducking follows: 0 is the bus's own input, 1 to 64 a
    /// track as it sounds after its fader.
    #[serde(default, skip_serializing_if = "is_own")]
    pub duck_key: u8,
    /// Sidechain/self ducking threshold in dBFS.
    pub duck_threshold_db: f32,
    /// Hold the output near the dry level so drive shapes tone without level jumps.
    pub level_match: bool,
    pub output_db: f32,
    pub bypass: bool,
}

/// A continuous control of the Emberform Drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DistortionParam {
    Drive,
    Asymmetry,
    LowCut,
    HighCut,
    Bits,
    CrushRate,
    Width,
    DuckThreshold,
    DuckAmount,
    DuckRelease,
    DuckKey,
    Output,
}
impl DistortionParam {
    pub const ALL: [Self; 12] = [
        Self::Drive,
        Self::Asymmetry,
        Self::LowCut,
        Self::HighCut,
        Self::Bits,
        Self::CrushRate,
        Self::Width,
        Self::DuckThreshold,
        Self::DuckAmount,
        Self::DuckRelease,
        Self::DuckKey,
        Self::Output,
    ];
    const RANGES: [Range; 12] = {
        let d = Emberform::DEFAULT;
        CommonRanges::new(
            d.duck_threshold_db,
            d.duck_amount_percent,
            d.duck_release_ms,
            d.output_db,
        )
        .table([
            (Self::Drive as usize, range(0.0, 48.0, d.drive_db, 0.5)),
            (
                Self::Asymmetry as usize,
                range(0.0, 100.0, d.asymmetry_percent, 1.0),
            ),
            (Self::LowCut as usize, low_cut_range(d.low_cut_hz)),
            (Self::HighCut as usize, high_cut_range(d.high_cut_hz)),
            (Self::Bits as usize, range(1.0, 16.0, d.bits as f32, 1.0)),
            (
                Self::CrushRate as usize,
                log_range(500.0, 44_100.0, d.crush_rate_hz, 10.0),
            ),
            (Self::Width as usize, width_range(d.width_percent)),
        ])
    };
    pub const fn range(self) -> Range {
        Self::RANGES[self as usize]
    }
    pub fn get(self, drive: &Emberform) -> f32 {
        match self {
            Self::Drive => drive.drive_db,
            Self::Asymmetry => drive.asymmetry_percent,
            Self::LowCut => drive.low_cut_hz,
            Self::HighCut => drive.high_cut_hz,
            Self::Bits => f32::from(drive.bits),
            Self::CrushRate => drive.crush_rate_hz,
            Self::Width => drive.width_percent,
            Self::DuckThreshold => drive.duck_threshold_db,
            Self::DuckAmount => drive.duck_amount_percent,
            Self::DuckRelease => drive.duck_release_ms,
            Self::DuckKey => f32::from(drive.duck_key),
            Self::Output => drive.output_db,
        }
    }
    /// Set the control, held to its range.
    pub fn set(self, drive: &mut Emberform, value: f32) {
        let value = self.range().hold(value);
        match self {
            Self::Bits => drive.bits = value.round() as u8,
            Self::DuckKey => drive.duck_key = value.round() as u8,
            Self::Drive => drive.drive_db = value,
            Self::Asymmetry => drive.asymmetry_percent = value,
            Self::LowCut => drive.low_cut_hz = value,
            Self::HighCut => drive.high_cut_hz = value,
            Self::CrushRate => drive.crush_rate_hz = value,
            Self::Width => drive.width_percent = value,
            Self::DuckThreshold => drive.duck_threshold_db = value,
            Self::DuckAmount => drive.duck_amount_percent = value,
            Self::DuckRelease => drive.duck_release_ms = value,
            Self::Output => drive.output_db = value,
        }
    }
}

impl Default for Emberform {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Emberform {
    /// The Warm Tube preset.
    const DEFAULT: Self = Self {
        shape: DistortionShape::Tube,
        drive_db: 12.0,
        asymmetry_percent: 20.0,
        low_cut_hz: 90.0,
        high_cut_hz: 12_000.0,
        bits: 8,
        crush_rate_hz: 11_025.0,
        width_percent: 100.0,
        duck_amount_percent: 0.0,
        duck_release_ms: 250.0,
        duck_key: 0,
        duck_threshold_db: -24.0,
        level_match: true,
        output_db: 0.0,
        bypass: false,
    };
    /// The factory presets and the return level each sets, in percent.
    pub const PRESETS: [(&'static str, f32); 12] = [
        ("Warm Tube", 45.0),
        ("Tape Glue", 35.0),
        ("Drum Crunch", 40.0),
        ("Bass Grit", 50.0),
        ("Fuzz Wall", 50.0),
        ("Industrial Clip", 45.0),
        ("Telephone", 55.0),
        ("Octave Up", 35.0),
        ("Wavefolder", 40.0),
        ("Lo-Fi Crush", 45.0),
        ("8-Bit Grit", 40.0),
        ("Pumping Heat", 55.0),
    ];
    /// A preset's settings, or `None` for an unknown name.
    pub fn preset(name: &str) -> Option<Self> {
        use DistortionShape::*;
        let base = Self::default();
        Some(match name {
            // Thickens synths and pads with soft even harmonics.
            "Warm Tube" => base,
            // A gentle, round saturation that pulls a group together.
            "Tape Glue" => Self {
                shape: Tape,
                drive_db: 9.0,
                asymmetry_percent: 10.0,
                low_cut_hz: 30.0,
                high_cut_hz: 15_000.0,
                width_percent: 100.0,
                output_db: 0.0,
                ..base
            },
            // Parallel grit for drums: the hits keep their attack in the dry sound.
            "Drum Crunch" => Self {
                shape: Overdrive,
                drive_db: 24.0,
                asymmetry_percent: 0.0,
                low_cut_hz: 120.0,
                high_cut_hz: 9000.0,
                ..base
            },
            // Harmonics for a bass that small speakers lose; the sub stays clean and mono.
            "Bass Grit" => Self {
                shape: Fuzz,
                drive_db: 15.0,
                asymmetry_percent: 30.0,
                low_cut_hz: 220.0,
                high_cut_hz: 5500.0,
                width_percent: 0.0,
                ..base
            },
            // A dense, wide buzz for leads and stabs.
            "Fuzz Wall" => Self {
                shape: Fuzz,
                drive_db: 30.0,
                asymmetry_percent: 40.0,
                low_cut_hz: 160.0,
                high_cut_hz: 7000.0,
                width_percent: 140.0,
                output_db: -1.0,
                ..base
            },
            // Hard, bright clipping for industrial drums and percussion.
            "Industrial Clip" => Self {
                shape: HardClip,
                drive_db: 30.0,
                asymmetry_percent: 0.0,
                low_cut_hz: 200.0,
                high_cut_hz: 11_000.0,
                output_db: -1.5,
                ..base
            },
            // A narrow, mid-range band of crunch, like a small speaker.
            "Telephone" => Self {
                shape: Overdrive,
                drive_db: 18.0,
                asymmetry_percent: 15.0,
                low_cut_hz: 600.0,
                high_cut_hz: 3200.0,
                width_percent: 0.0,
                ..base
            },
            // An octave over leads and arpeggios.
            "Octave Up" => Self {
                shape: Rectifier,
                drive_db: 6.0,
                asymmetry_percent: 0.0,
                low_cut_hz: 300.0,
                high_cut_hz: 9000.0,
                width_percent: 120.0,
                ..base
            },
            // Metallic, bell-like overtones that move with the level.
            "Wavefolder" => Self {
                shape: Foldback,
                drive_db: 15.0,
                asymmetry_percent: 15.0,
                low_cut_hz: 180.0,
                high_cut_hz: 10_000.0,
                width_percent: 150.0,
                output_db: -1.0,
                ..base
            },
            // Soft steps and a dull top, like an early sampler.
            "Lo-Fi Crush" => Self {
                shape: Bitcrush,
                drive_db: 3.0,
                asymmetry_percent: 0.0,
                low_cut_hz: 60.0,
                high_cut_hz: 8000.0,
                bits: 10,
                crush_rate_hz: 16_000.0,
                ..base
            },
            // Coarse steps and folded aliasing for hats and glitches.
            "8-Bit Grit" => Self {
                shape: Bitcrush,
                drive_db: 6.0,
                asymmetry_percent: 0.0,
                low_cut_hz: 150.0,
                high_cut_hz: 14_000.0,
                bits: 5,
                crush_rate_hz: 6000.0,
                output_db: -2.0,
                ..base
            },
            // Saturation that ducks under its own input and swells between hits.
            "Pumping Heat" => Self {
                shape: Tube,
                drive_db: 18.0,
                asymmetry_percent: 35.0,
                low_cut_hz: 120.0,
                high_cut_hz: 10_000.0,
                width_percent: 130.0,
                duck_amount_percent: 70.0,
                duck_release_ms: 180.0,
                ..base
            },
            _ => return None,
        })
    }
    pub(in crate::fx) fn validate(&self) -> Result<(), String> {
        if !DistortionParam::ALL
            .into_iter()
            .all(|param| param.range().contains(param.get(self)))
        {
            return Err("An Emberform Drive setting is out of range.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_drive_params_roundtrip_clamp_and_validate() {
        let mut drive = Emberform::default();
        for param in DistortionParam::ALL {
            let range = param.range();
            let mid = (range.low + range.high) * 0.5;
            let expected = if matches!(param, DistortionParam::Bits | DistortionParam::DuckKey) {
                mid.round()
            } else {
                mid
            };
            param.set(&mut drive, mid);
            assert!(
                (param.get(&drive) - expected).abs() < 1e-4,
                "{param:?}: {} != {expected}",
                param.get(&drive)
            );
            param.set(&mut drive, range.low - 100.0);
            assert_eq!(param.get(&drive), range.low, "{param:?} lower bound");
            param.set(&mut drive, range.high + 100.0);
            assert_eq!(param.get(&drive), range.high, "{param:?} upper bound");
            param.set(&mut drive, f32::NAN);
            assert_eq!(param.get(&drive), range.default, "{param:?} NaN default");
        }
        DistortionParam::Bits.set(&mut drive, 4.6);
        assert_eq!(drive.bits, 5);
        DistortionParam::DuckKey.set(&mut drive, 3.4);
        assert_eq!(drive.duck_key, 3);

        for (name, _) in Emberform::PRESETS {
            let preset = Emberform::preset(name).expect(name);
            assert!(preset.validate().is_ok(), "preset {name} invalid");
        }
        assert!(Emberform::preset("Does Not Exist").is_none());
        let bad = Emberform {
            drive_db: 999.0,
            ..Emberform::default()
        };
        assert!(bad.validate().is_err());
    }
}
