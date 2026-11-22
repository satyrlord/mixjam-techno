//! The decay curves and early reflections that the Deep Edit scope draws.
use super::{ER_TAPS, HIGH_CUT_DECAY, LINES, TREBLE_DECAY_CORNER, VINTAGE_HIGH_CUT_DECAY};
use crate::fx::{Aetherform, ReverbCharacter, SpaceModel};

/// The points of each decay curve.
const CURVE_POINTS: usize = 32;
/// The shortest and longest time that the scope shows, in seconds.
const HORIZON: (f64, f64) = (0.4, 12.0);

/// What an Aetherform Reverb does over time: its early reflections and the
/// decay of its low, middle, and high bands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReverbProfile {
    /// Time on the scope, from 0 to 1, and signed strength from -1 to 1.
    pub er_taps: [(f32, f32); ER_TAPS],
    pub er_count: usize,
    /// The pre-delay on the same time scale.
    pub pre_delay_norm: f32,
    /// The time that each band takes to fall by 60 dB: low, middle, high.
    pub rt60_bands: [f32; 3],
    /// The level of each band from the pre-delay marker to the right edge,
    /// from -60 dB at 0 to full level at 1.
    pub decay_curves: [[f32; CURVE_POINTS]; 3],
    pub horizon_seconds: f32,
}

/// The seconds that each band takes to fall by 60 dB.
fn band_times(s: &Aetherform) -> [f64; 3] {
    let mid = f64::from(s.decay_seconds);
    let high_cut = f64::from(s.high_cut_hz);
    let share = high_cut_share(s.character == ReverbCharacter::Vintage, high_cut);
    let darkening = 1.0 - (1.0 - share) * (TREBLE_DECAY_CORNER / high_cut).min(1.0);
    [
        mid * f64::from(s.bass_mult),
        mid,
        mid * f64::from(s.treble_mult) * darkening,
    ]
}

/// The level of a tail at `seconds` after its onset, in the scope's scale.
fn level(seconds: f64, rt60: f64, rise: f64) -> f32 {
    if seconds <= 0.0 {
        return 0.0;
    }
    let envelope = (1.0 - (-seconds / rise).exp()) * 10f64.powf(-3.0 * seconds / rt60);
    ((20.0 * envelope.max(1e-6).log10() + 60.0) / 60.0).clamp(0.0, 1.0) as f32
}

/// Sample the profile of `s` for the Deep Edit scope.
pub(crate) fn sample_reverb_profile(s: &Aetherform) -> ReverbProfile {
    let rt60 = band_times(s);
    let pre = f64::from(s.pre_delay_ms) / 1000.0;
    let tail = rt60.iter().copied().fold(0.0, f64::max);
    let horizon = (pre + tail).clamp(HORIZON.0, HORIZON.1);
    let rise = if s.character == ReverbCharacter::Bloom {
        0.14
    } else {
        0.02
    };
    let mut profile = ReverbProfile {
        er_taps: [(0.0, 0.0); ER_TAPS],
        er_count: 0,
        pre_delay_norm: (pre / horizon) as f32,
        rt60_bands: rt60.map(|seconds| seconds as f32),
        decay_curves: [[0.0; CURVE_POINTS]; 3],
        horizon_seconds: horizon as f32,
    };
    if s.early_reflections_enabled {
        let scale = 0.4 + 0.6 * size_factor(f64::from(s.size_percent));
        let count = 4 + (12.0 * f64::from(s.density_percent) / 100.0).round() as usize;
        profile.er_count = count.min(ER_TAPS);
        let times = reflection_times(s.space_model);
        for (k, tap) in profile
            .er_taps
            .iter_mut()
            .enumerate()
            .take(profile.er_count)
        {
            let at = (pre + times[k] * scale / 1000.0) / horizon;
            let sign = if k % 2 == 1 { -1.0 } else { 1.0 };
            *tap = (at as f32, (0.86f64.powi(k as i32) * sign) as f32);
        }
    }
    for (curve, seconds) in profile.decay_curves.iter_mut().zip(rt60) {
        for (p, point) in curve.iter_mut().enumerate() {
            let t = p as f64 / (CURVE_POINTS - 1) as f64 * (horizon - pre);
            *point = level(t, seconds, rise);
        }
    }
    profile
}

/// The share of the decay in which the tail falls by 60 dB at the high cut.
/// A wide-open cut leaves the loop undamped.
pub(super) fn high_cut_share(vintage: bool, high_cut: f64) -> f64 {
    if vintage {
        return VINTAGE_HIGH_CUT_DECAY;
    }
    let open = ((high_cut - 18_000.0) / 2_000.0).clamp(0.0, 1.0);
    HIGH_CUT_DECAY + (1.0 - HIGH_CUT_DECAY) * open
}

/// Line lengths in milliseconds at full size, by model.
pub(super) fn base_lengths(model: SpaceModel) -> [f64; LINES] {
    match model {
        SpaceModel::Room => [
            11.3, 12.4, 13.7, 15.1, 16.6, 18.3, 20.1, 22.2, 24.4, 26.9, 29.7, 32.7, 36.0, 39.6,
            43.6, 47.9,
        ],
        SpaceModel::Hall => [
            29.7, 32.5, 35.6, 39.0, 42.6, 46.7, 51.1, 55.9, 61.2, 67.0, 73.3, 80.2, 87.8, 96.1,
            105.2, 113.4,
        ],
        SpaceModel::Plate => [
            7.1, 8.2, 9.5, 10.9, 12.6, 14.6, 16.8, 19.4, 22.5, 25.9, 30.0, 34.6, 40.0, 46.2, 53.5,
            61.9,
        ],
        SpaceModel::Chamber => [
            17.3, 19.1, 21.2, 23.4, 26.0, 28.8, 31.9, 35.3, 39.1, 43.3, 48.0, 53.2, 58.9, 65.2,
            72.3, 79.1,
        ],
    }
}

/// Early reflection times in milliseconds at full size, by model.
pub(super) fn reflection_times(model: SpaceModel) -> [f64; ER_TAPS] {
    match model {
        SpaceModel::Room => [
            2.0, 3.3, 4.8, 6.5, 8.1, 9.9, 11.7, 13.4, 15.2, 17.3, 19.5, 21.8, 24.2, 26.7, 29.3,
            32.1,
        ],
        SpaceModel::Hall => [
            6.1, 9.3, 12.7, 16.4, 20.3, 24.6, 29.1, 33.8, 38.7, 43.9, 49.4, 55.1, 61.2, 67.5, 74.1,
            81.3,
        ],
        SpaceModel::Plate => [
            0.7, 1.3, 2.0, 2.7, 3.5, 4.4, 5.2, 6.1, 7.1, 8.1, 9.2, 10.4, 11.6, 12.9, 14.3, 15.8,
        ],
        SpaceModel::Chamber => [
            3.8, 5.7, 7.9, 10.1, 12.6, 15.2, 17.9, 20.8, 23.9, 27.1, 30.5, 34.1, 37.8, 41.7, 45.8,
            50.2,
        ],
    }
}

/// The share of full size: from 0.28 at 5% to 1 at 100%, faster near the top.
pub(super) fn size_factor(percent: f64) -> f64 {
    0.28 + 0.72 * ((percent - 5.0) / 95.0).clamp(0.0, 1.0).powf(1.3)
}
