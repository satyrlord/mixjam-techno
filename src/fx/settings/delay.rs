//! The Echoform Delay's settings, controls, and presets.
use super::range::{
    CommonRanges, Range, high_cut_range, is_own, log_range, low_cut_range, range, width_range,
};
use serde::{Deserialize, Serialize};

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
    /// What the ducking follows: 0 is the bus's own input, 1 to 64 a
    /// track as it sounds after its fader.
    #[serde(default, skip_serializing_if = "is_own")]
    pub duck_key: u8,
    /// Sidechain/self ducking threshold in dBFS.
    pub duck_threshold_db: f32,
    /// Link left and right time/division controls.
    pub link_times: bool,
    /// Feedback diffusion smear in percent.
    pub smear_percent: f32,
    /// Resonance of the loop HP/LP filters in percent.
    pub filter_resonance: f32,
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
    FilterResonance,
    Smear,
    ModRate,
    ModDepth,
    Drive,
    DuckThreshold,
    DuckAmount,
    DuckRelease,
    DuckKey,
    Output,
}

impl DelayParam {
    pub const ALL: [Self; 16] = [
        Self::TimeL,
        Self::TimeR,
        Self::Feedback,
        Self::Width,
        Self::LowCut,
        Self::HighCut,
        Self::FilterResonance,
        Self::Smear,
        Self::ModRate,
        Self::ModDepth,
        Self::Drive,
        Self::DuckThreshold,
        Self::DuckAmount,
        Self::DuckRelease,
        Self::DuckKey,
        Self::Output,
    ];
    const RANGES: [Range; 16] = {
        let d = Echoform::DEFAULT;
        CommonRanges::new(
            d.duck_threshold_db,
            d.duck_amount,
            d.duck_release,
            d.output_db,
        )
        .table([
            (
                Self::TimeL as usize,
                log_range(1.0, 2000.0, d.time_ms_l, 1.0),
            ),
            (
                Self::TimeR as usize,
                log_range(1.0, 2000.0, d.time_ms_r, 1.0),
            ),
            (Self::Feedback as usize, range(0.0, 110.0, d.feedback, 1.0)),
            (Self::Width as usize, width_range(d.width)),
            (Self::LowCut as usize, low_cut_range(d.low_cut)),
            (Self::HighCut as usize, high_cut_range(d.high_cut)),
            (
                Self::FilterResonance as usize,
                range(0.0, 100.0, d.filter_resonance, 1.0),
            ),
            (
                Self::Smear as usize,
                range(0.0, 100.0, d.smear_percent, 1.0),
            ),
            (
                Self::ModRate as usize,
                log_range(0.05, 8.0, d.mod_rate, 0.01),
            ),
            (Self::ModDepth as usize, range(0.0, 20.0, d.mod_depth, 0.1)),
            (Self::Drive as usize, range(0.0, 100.0, d.drive, 1.0)),
        ])
    };
    pub const fn range(self) -> Range {
        Self::RANGES[self as usize]
    }
    pub fn get(self, delay: &Echoform) -> f32 {
        match self {
            Self::TimeL => delay.time_ms_l,
            Self::TimeR => delay.time_ms_r,
            Self::Feedback => delay.feedback,
            Self::Width => delay.width,
            Self::LowCut => delay.low_cut,
            Self::HighCut => delay.high_cut,
            Self::FilterResonance => delay.filter_resonance,
            Self::Smear => delay.smear_percent,
            Self::ModRate => delay.mod_rate,
            Self::ModDepth => delay.mod_depth,
            Self::Drive => delay.drive,
            Self::DuckThreshold => delay.duck_threshold_db,
            Self::DuckAmount => delay.duck_amount,
            Self::DuckRelease => delay.duck_release,
            Self::DuckKey => f32::from(delay.duck_key),
            Self::Output => delay.output_db,
        }
    }
    /// Set the control, held to its range.
    pub fn set(self, delay: &mut Echoform, value: f32) {
        let value = self.range().hold(value);
        if self == Self::DuckKey {
            delay.duck_key = value.round() as u8;
            return;
        }
        if delay.link_times && matches!(self, Self::TimeL | Self::TimeR) {
            delay.time_ms_l = value;
            delay.time_ms_r = value;
            return;
        }
        *match self {
            Self::TimeL => &mut delay.time_ms_l,
            Self::TimeR => &mut delay.time_ms_r,
            Self::Feedback => &mut delay.feedback,
            Self::Width => &mut delay.width,
            Self::LowCut => &mut delay.low_cut,
            Self::HighCut => &mut delay.high_cut,
            Self::FilterResonance => &mut delay.filter_resonance,
            Self::Smear => &mut delay.smear_percent,
            Self::ModRate => &mut delay.mod_rate,
            Self::ModDepth => &mut delay.mod_depth,
            Self::Drive => &mut delay.drive,
            Self::DuckThreshold => &mut delay.duck_threshold_db,
            Self::DuckAmount => &mut delay.duck_amount,
            Self::DuckRelease => &mut delay.duck_release,
            Self::DuckKey => unreachable!(),
            Self::Output => &mut delay.output_db,
        } = value;
    }
}

impl Default for Echoform {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Echoform {
    /// The Wide Tape Echo preset.
    const DEFAULT: Self = Self {
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
        duck_key: 0,
        duck_threshold_db: -24.0,
        link_times: false,
        smear_percent: 0.0,
        filter_resonance: 0.0,
        output_db: -1.5,
        bypass: false,
    };
    /// The factory presets and the return level each sets, in percent.
    pub const PRESETS: [(&'static str, f32); 6] = [
        ("Wide Tape Echo", 82.0),
        ("Clean Slap", 72.0),
        ("Dotted Motion", 78.0),
        ("Dub Feedback", 100.0),
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
                smear_percent: 12.0,
                filter_resonance: 10.0,
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
                // Loud, long repeats like a hardware tape echo: the first ones
                // answer a few dB under the dry sound and the tail rings for
                // about 10 s; the drive keeps the feedback bounded.
                feedback: 90.0,
                width: 136.0,
                low_cut: 180.0,
                high_cut: 6000.0,
                mod_rate: 0.24,
                mod_depth: 7.6,
                character: Tape,
                drive: 32.0,
                smear_percent: 24.0,
                filter_resonance: 38.0,
                duck_amount: 4.0,
                duck_release: 980.0,
                output_db: 0.0,
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
                smear_percent: 45.0,
                filter_resonance: 18.0,
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
        let [left, right] = match self.mode {
            TimeMode::Sync => [self.division_l.seconds(bpm), self.division_r.seconds(bpm)],
            TimeMode::Free => [
                f64::from(self.time_ms_l) / 1000.0,
                f64::from(self.time_ms_r) / 1000.0,
            ],
        };
        [left, if self.link_times { left } else { right }]
    }
    pub(in crate::fx) fn validate(&self) -> Result<(), String> {
        if !DelayParam::ALL
            .into_iter()
            .all(|param| param.range().contains(param.get(self)))
        {
            return Err("An Echoform Delay setting is out of range.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_delay_mirrors_edits_and_uses_left_in_both_time_modes() {
        let mut delay = Echoform {
            link_times: true,
            time_ms_l: 80.0,
            time_ms_r: 160.0,
            division_l: Division::Eighth,
            division_r: Division::Quarter,
            mode: TimeMode::Sync,
            ..Echoform::default()
        };
        assert_eq!(delay.seconds(120.0), [0.25, 0.25]);
        delay.mode = TimeMode::Free;
        assert_eq!(delay.seconds(120.0), [0.08, 0.08]);
        DelayParam::TimeL.set(&mut delay, 119.0);
        assert_eq!([delay.time_ms_l, delay.time_ms_r], [119.0, 119.0]);
        DelayParam::TimeR.set(&mut delay, 237.0);
        assert_eq!([delay.time_ms_l, delay.time_ms_r], [237.0, 237.0]);
        delay.link_times = false;
        DelayParam::TimeR.set(&mut delay, 119.0);
        assert_eq!(delay.seconds(120.0), [0.237, 0.119]);
    }
}
