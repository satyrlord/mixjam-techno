//! The channel EQ: an equaliser on every track, between its clips and its
//! automation.
//!
//! Five bands run in order: a low cut (a 12 dB/octave Butterworth
//! high-pass), a low shelf, a peak, a high shelf, and a high cut (a 12
//! dB/octave Butterworth low-pass). The low cut at 20 Hz and the high cut at
//! 20 kHz are off, not filters at those frequencies, and a band at 0 dB is
//! off too. A track's EQ is off until the user switches it on, and a track
//! whose EQ is off mixes exactly as it did before the EQ existed.
use crate::master::{
    KnobRange,
    dsp::{Biquad, Coefficients},
    range, ratio_range,
};
use serde::{Deserialize, Serialize};
use std::f64::consts::FRAC_1_SQRT_2;

/// One track's EQ settings. The EQ keeps them while it is off.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChannelEq {
    pub on: bool,
    /// The low cut's frequency in Hz. At its lowest it is off.
    pub low_cut: f32,
    pub low_hz: f32,
    pub low_db: f32,
    pub mid_hz: f32,
    pub mid_db: f32,
    pub mid_q: f32,
    pub high_hz: f32,
    pub high_db: f32,
    /// The high cut's frequency in Hz. At its highest it is off.
    pub high_cut: f32,
}

impl Default for ChannelEq {
    fn default() -> Self {
        let mut eq = Self {
            on: false,
            low_cut: 0.0,
            low_hz: 0.0,
            low_db: 0.0,
            mid_hz: 0.0,
            mid_db: 0.0,
            mid_q: 0.0,
            high_hz: 0.0,
            high_db: 0.0,
            high_cut: 0.0,
        };
        for param in EqParam::ALL {
            param.set(&mut eq, param.range().default);
        }
        eq
    }
}

/// A continuous control of the channel EQ.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EqParam {
    LowCut,
    LowHz,
    LowDb,
    MidHz,
    MidDb,
    MidQ,
    HighHz,
    HighDb,
    HighCut,
}

/// The cut and shelf frequencies at which a band does nothing.
const LOW_CUT_OFF: f32 = 20.0;
const HIGH_CUT_OFF: f32 = 20_000.0;
/// The slope of both shelves: the steepest that does not overshoot.
const SHELF_SLOPE: f64 = 1.0;

impl EqParam {
    /// In signal order, each band's frequency before its gain.
    pub const ALL: [Self; 9] = [
        Self::LowCut,
        Self::LowHz,
        Self::LowDb,
        Self::MidHz,
        Self::MidDb,
        Self::MidQ,
        Self::HighHz,
        Self::HighDb,
        Self::HighCut,
    ];

    pub const fn range(self) -> KnobRange {
        match self {
            Self::LowCut => ratio_range(LOW_CUT_OFF, 2000.0, LOW_CUT_OFF, 1.0),
            Self::LowHz => ratio_range(40.0, 500.0, 100.0, 1.0),
            Self::MidHz => ratio_range(150.0, 10_000.0, 1000.0, 1.0),
            Self::HighHz => ratio_range(1500.0, 16_000.0, 8000.0, 10.0),
            Self::HighCut => ratio_range(1000.0, HIGH_CUT_OFF, HIGH_CUT_OFF, 10.0),
            Self::LowDb | Self::MidDb | Self::HighDb => range(-18.0, 18.0, 0.0, 0.1),
            Self::MidQ => ratio_range(0.3, 8.0, 1.0, 0.1),
        }
    }

    /// The control's name after its track's, such as "low cut".
    pub fn name(self) -> &'static str {
        match self {
            Self::LowCut => "low cut",
            Self::LowHz => "low frequency",
            Self::LowDb => "low gain",
            Self::MidHz => "mid frequency",
            Self::MidDb => "mid gain",
            Self::MidQ => "mid Q",
            Self::HighHz => "high frequency",
            Self::HighDb => "high gain",
            Self::HighCut => "high cut",
        }
    }

    pub fn get(self, eq: &ChannelEq) -> f32 {
        match self {
            Self::LowCut => eq.low_cut,
            Self::LowHz => eq.low_hz,
            Self::LowDb => eq.low_db,
            Self::MidHz => eq.mid_hz,
            Self::MidDb => eq.mid_db,
            Self::MidQ => eq.mid_q,
            Self::HighHz => eq.high_hz,
            Self::HighDb => eq.high_db,
            Self::HighCut => eq.high_cut,
        }
    }

    /// Set the control, held to its range.
    pub fn set(self, eq: &mut ChannelEq, value: f32) {
        let range = self.range();
        let value = if value.is_nan() {
            range.default
        } else {
            value.clamp(range.low, range.high)
        };
        match self {
            Self::LowCut => eq.low_cut = value,
            Self::LowHz => eq.low_hz = value,
            Self::LowDb => eq.low_db = value,
            Self::MidHz => eq.mid_hz = value,
            Self::MidDb => eq.mid_db = value,
            Self::MidQ => eq.mid_q = value,
            Self::HighHz => eq.high_hz = value,
            Self::HighDb => eq.high_db = value,
            Self::HighCut => eq.high_cut = value,
        }
    }
}

impl ChannelEq {
    pub fn validate(&self) -> Result<(), String> {
        if EqParam::ALL.into_iter().any(|param| {
            let (value, range) = (param.get(self), param.range());
            !(value.is_finite() && (range.low..=range.high).contains(&value))
        }) {
            return Err("A channel EQ setting is out of range.".into());
        }
        Ok(())
    }

    /// The settings with every band and the switch as a new track has them.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The bands that change the sound, in signal order.
    fn bands(&self) -> impl Iterator<Item = Coefficients> {
        let hz = |value: f32| f64::from(value);
        [
            (self.low_cut > LOW_CUT_OFF)
                .then(|| Coefficients::high_pass(hz(self.low_cut), FRAC_1_SQRT_2)),
            (self.low_db != 0.0)
                .then(|| Coefficients::low_shelf(hz(self.low_hz), SHELF_SLOPE, hz(self.low_db))),
            (self.mid_db != 0.0)
                .then(|| Coefficients::peaking(hz(self.mid_hz), hz(self.mid_q), hz(self.mid_db))),
            (self.high_db != 0.0)
                .then(|| Coefficients::high_shelf(hz(self.high_hz), SHELF_SLOPE, hz(self.high_db))),
            (self.high_cut < HIGH_CUT_OFF)
                .then(|| Coefficients::low_pass(hz(self.high_cut), FRAC_1_SQRT_2)),
        ]
        .into_iter()
        .flatten()
    }

    /// The change in level at a frequency, in dB, with the EQ on.
    pub fn response_db(&self, hz: f64) -> f64 {
        self.bands()
            .map(|band| 20.0 * band.magnitude(hz).log10())
            .sum()
    }
}

/// The filters of an EQ that is on, designed once when a mix is prepared.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Curve {
    bands: [Coefficients; 5],
    count: usize,
}

impl Curve {
    /// `None` for an EQ that is off. An EQ that is on with every band off
    /// keeps an empty curve, which passes the sound unchanged.
    pub(crate) fn of(eq: &ChannelEq) -> Option<Self> {
        if !eq.on {
            return None;
        }
        let mut curve = Self {
            bands: [Coefficients::FLAT; 5],
            count: 0,
        };
        for band in eq.bands() {
            curve.bands[curve.count] = band;
            curve.count += 1;
        }
        Some(curve)
    }
}

/// The history of one track's EQ filters. Each render reader owns its own.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Filters([Biquad; 5]);

impl Default for Filters {
    fn default() -> Self {
        Self([Biquad::FLAT; 5])
    }
}

impl Filters {
    /// Filter interleaved stereo frames in place.
    pub(crate) fn process(&mut self, frames: &mut [f32], curve: &Curve) {
        for (filter, coefficients) in self.0.iter_mut().zip(&curve.bands[..curve.count]) {
            filter.set(*coefficients);
            for frame in frames.as_chunks_mut::<2>().0 {
                for (channel, sample) in frame.iter_mut().enumerate() {
                    *sample = filter.process(channel, f64::from(*sample)) as f32;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|frame| {
                let x = 0.5 * (std::f32::consts::TAU * hz * frame as f32 / 44_100.0).sin();
                [x, x]
            })
            .collect()
    }

    /// The level change of a tone through the EQ, in dB, after it settles.
    fn gain_db(eq: &ChannelEq, hz: f32) -> f64 {
        let input = tone(hz, 44_100);
        let mut output = input.clone();
        Filters::default().process(&mut output, &Curve::of(eq).unwrap());
        let rms = |samples: &[f32]| {
            (samples[22_050..]
                .iter()
                .map(|x| f64::from(*x).powi(2))
                .sum::<f64>()
                / (samples.len() - 22_050) as f64)
                .sqrt()
        };
        20.0 * (rms(&output) / rms(&input)).log10()
    }

    #[test]
    fn defaults_are_off_and_flat_and_ranges_reject_bad_values() {
        let eq = ChannelEq::default();
        assert!(!eq.on && eq.is_default());
        assert_eq!((eq.low_cut, eq.high_cut), (20.0, 20_000.0));
        assert_eq!((eq.low_hz, eq.mid_hz, eq.high_hz), (100.0, 1000.0, 8000.0));
        assert_eq!(eq.mid_q, 1.0);
        assert!(Curve::of(&eq).is_none());
        let on = ChannelEq { on: true, ..eq };
        assert_eq!(Curve::of(&on).unwrap().count, 0);
        assert_eq!(on.response_db(1000.0), 0.0);
        eq.validate().unwrap();
        for param in EqParam::ALL {
            let range = param.range();
            for bad in [range.low - 1.0, range.high + 1.0, f32::NAN, f32::INFINITY] {
                let mut changed = eq;
                match param {
                    EqParam::LowCut => changed.low_cut = bad,
                    EqParam::LowHz => changed.low_hz = bad,
                    EqParam::LowDb => changed.low_db = bad,
                    EqParam::MidHz => changed.mid_hz = bad,
                    EqParam::MidDb => changed.mid_db = bad,
                    EqParam::MidQ => changed.mid_q = bad,
                    EqParam::HighHz => changed.high_hz = bad,
                    EqParam::HighDb => changed.high_db = bad,
                    EqParam::HighCut => changed.high_cut = bad,
                }
                assert!(changed.validate().is_err(), "{param:?} {bad}");
            }
            let mut held = eq;
            param.set(&mut held, range.high * 10.0);
            assert_eq!(param.get(&held), range.high);
        }
    }

    #[test]
    fn a_high_shelf_tames_the_top_and_leaves_the_low_end() {
        let eq = ChannelEq {
            on: true,
            high_db: -12.0,
            ..ChannelEq::default()
        };
        // Half an octave above the shelf's frequency, its midpoint, the
        // shelf has nearly reached its gain.
        let top = gain_db(&eq, 12_000.0);
        assert!((top + 12.0).abs() < 1.5, "12 kHz: {top:.2} dB");
        let low = gain_db(&eq, 200.0);
        assert!(low.abs() < 0.5, "200 Hz: {low:.2} dB");
        assert!((eq.response_db(12_000.0) - top).abs() < 0.3);
    }

    #[test]
    fn the_cuts_remove_what_lies_beyond_them() {
        let eq = ChannelEq {
            on: true,
            low_cut: 200.0,
            high_cut: 2000.0,
            ..ChannelEq::default()
        };
        assert!(gain_db(&eq, 50.0) < -20.0);
        assert!(gain_db(&eq, 12_000.0) < -20.0);
        assert!(gain_db(&eq, 630.0).abs() < 1.0);
        // A 12 dB/octave Butterworth cut is 3 dB down at its frequency.
        assert!((eq.response_db(200.0) + 3.0).abs() < 0.2);
    }

    #[test]
    fn the_peak_and_low_shelf_reach_their_gain() {
        let eq = ChannelEq {
            on: true,
            low_db: 6.0,
            low_hz: 100.0,
            mid_db: -9.0,
            mid_hz: 3000.0,
            mid_q: 2.0,
            ..ChannelEq::default()
        };
        assert!((gain_db(&eq, 30.0) - 6.0).abs() < 0.5);
        assert!((gain_db(&eq, 3000.0) + 9.0).abs() < 0.5);
        assert!(gain_db(&eq, 800.0).abs() < 1.0);
    }
}
