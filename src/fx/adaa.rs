//! Second-order antiderivative anti-aliasing (ADAA2) and transfer curve
//! evaluation for the analog Emberform Drive shapes.
use super::{DistortionShape, Emberform};
use crate::master::dsp::db_to_gain;
use std::f64::consts::{FRAC_PI_2, PI};

const ADAA_EPS: f64 = 1e-4;
const DYNAMIC_BIAS_SCALE: f64 = 0.45;

#[inline]
fn alg_curve_antiderivs(x: f64, k: f64) -> (f64, f64, f64) {
    let u = k * x;
    let r = (1.0 + u * u).sqrt();
    let f = u / (k * r);
    let inv_k2 = 1.0 / (k * k);
    let f1 = (r - 1.0) * inv_k2;
    let f2 = 0.5 * (u * r + u.asinh() - 2.0 * u) * (inv_k2 / k);
    (f, f1, f2)
}

#[inline]
fn atan_curve_antiderivs(x: f64, gain_in: f64, gain_out: f64) -> (f64, f64, f64) {
    let u = gain_in * x;
    let at = u.atan();
    let l1p = (u * u).ln_1p();
    let f = gain_out * at;
    let inv_a = 1.0 / gain_in;
    let f1 = gain_out * inv_a * (u * at - 0.5 * l1p);
    let f2 = gain_out * (inv_a * inv_a) * (0.5 * (u * u - 1.0) * at - 0.5 * u * l1p + 0.5 * u);
    (f, f1, f2)
}

#[inline]
pub(super) fn eval_shape(shape: DistortionShape, x: f64) -> (f64, f64, f64) {
    match shape {
        DistortionShape::Tube => {
            if x >= 0.0 {
                alg_curve_antiderivs(x, 1.0)
            } else {
                alg_curve_antiderivs(x, 0.65)
            }
        }
        DistortionShape::Tape => atan_curve_antiderivs(x, FRAC_PI_2, 1.0 / FRAC_PI_2),
        DistortionShape::Overdrive => alg_curve_antiderivs(x, 1.0),
        DistortionShape::Fuzz => atan_curve_antiderivs(x, PI, 2.0 / PI),
        DistortionShape::HardClip => {
            let a = x.abs();
            let s = x.signum();
            if a <= 1.0 {
                (x, 0.5 * x * x, x * x * x / 6.0)
            } else {
                (s, a - 0.5, s * (0.5 * a * a - 0.5 * a + (1.0 / 6.0)))
            }
        }
        DistortionShape::Foldback => {
            let s = x.sin();
            (s, 1.0 - x.cos(), x - s)
        }
        DistortionShape::Rectifier => {
            const K: f64 = 0.25;
            let r = (x * x + K * K).sqrt();
            let s = (x / K).asinh();
            let f = r - K;
            let f1 = 0.5 * (x * r + K * K * s) - K * x;
            let f2 =
                (r * r * r) / 6.0 + 0.5 * K * K * (x * s - r) - 0.5 * K * x * x + (K * K * K) / 3.0;
            (f, f1, f2)
        }
        DistortionShape::Bitcrush => {
            let c = x.clamp(-1.0, 1.0);
            (c, 0.0, 0.0)
        }
    }
}

#[inline]
pub(super) fn curve_only(shape: DistortionShape, x: f64) -> f64 {
    eval_shape(shape, x).0
}

#[inline]
pub(super) fn pre_gain(shape: DistortionShape) -> f64 {
    match shape {
        DistortionShape::Fuzz => 4.0,
        _ => 1.0,
    }
}

#[inline]
pub(super) fn biased_input(sample: f64, gain: f64, asymmetry: f64, envelope: f64) -> f64 {
    sample * gain + DYNAMIC_BIAS_SCALE * asymmetry * (envelope * gain).min(1.2)
}

pub(super) fn crusher_levels(bits: u8) -> f64 {
    f64::from(2u32.pow(u32::from(bits.clamp(1, 16)) - 1))
}

#[inline]
pub(super) fn quantize(sample: f64, levels: f64, dither: f64) -> f64 {
    ((sample.clamp(-1.0, 1.0) + dither) * levels).round() / levels
}

/// One channel's ADAA2 integration memory.
#[derive(Clone, Copy, Default)]
pub(super) struct Adaa2State {
    x1: f64,
    x2: f64,
    f2_1: f64,
    f2_2: f64,
}

impl Adaa2State {
    pub(super) fn rest_at(&mut self, shape: DistortionShape, x: f64) {
        self.x1 = x;
        self.x2 = x;
        self.retarget(shape);
    }

    pub(super) fn retarget(&mut self, shape: DistortionShape) {
        self.f2_1 = eval_shape(shape, self.x1).2;
        self.f2_2 = eval_shape(shape, self.x2).2;
    }

    #[inline]
    fn d1(shape: DistortionShape, xa: f64, xb: f64, f2a: f64, f2b: f64) -> f64 {
        let dx = xa - xb;
        if dx.abs() > ADAA_EPS {
            (f2a - f2b) / dx
        } else {
            eval_shape(shape, 0.5 * (xa + xb)).1
        }
    }

    /// Evaluate the 2nd-order antiderivative filter and update history.
    #[inline]
    pub(super) fn process(&mut self, shape: DistortionShape, x0: f64) -> f64 {
        let (_, _, f2_0) = eval_shape(shape, x0);
        let dx2 = x0 - self.x2;
        let out = if dx2.abs() > ADAA_EPS {
            let d_hi = Self::d1(shape, x0, self.x1, f2_0, self.f2_1);
            let d_lo = Self::d1(shape, self.x1, self.x2, self.f2_1, self.f2_2);
            2.0 * (d_hi - d_lo) / dx2
        } else {
            let x_mid = 0.5 * (x0 + self.x2);
            let dx1 = x_mid - self.x1;
            if dx1.abs() > ADAA_EPS {
                let (_, f1_mid, f2_mid) = eval_shape(shape, x_mid);
                let d_mid = Self::d1(shape, x_mid, self.x1, f2_mid, self.f2_1);
                2.0 * (f1_mid - d_mid) / dx1
            } else {
                curve_only(shape, 0.5 * (x_mid + self.x1))
            }
        };
        self.x2 = self.x1;
        self.f2_2 = self.f2_1;
        self.x1 = x0;
        self.f2_1 = f2_0;
        out
    }
}

/// Sample the normalized transfer curve `y(x)` over `x in [-1.0, 1.0]` into `points`
/// for the Deep Edit scope visualizer.
pub fn sample_transfer_curve(settings: &Emberform, points: &mut [f32]) {
    if points.is_empty() {
        return;
    }
    let n = points.len();
    let gain = db_to_gain(f64::from(settings.drive_db)) * pre_gain(settings.shape);
    let asym = f64::from(settings.asymmetry_percent) / 100.0;
    let levels = crusher_levels(settings.bits);
    let denom = (n.saturating_sub(1)).max(1) as f64;
    for (i, dst) in points.iter_mut().enumerate() {
        let x = -1.0 + 2.0 * (i as f64 / denom);
        // A static plot uses the settled envelope of each input point.
        let driven = biased_input(x, gain, asym, x.abs());
        let y = if settings.shape == DistortionShape::Bitcrush {
            quantize(driven, levels, 0.0)
        } else {
            curve_only(settings.shape, driven)
        };
        *dst = y.clamp(-1.5, 1.5) as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_scope_shows_the_driven_bias_and_crusher_steps() {
        for shape in [DistortionShape::HardClip, DistortionShape::Bitcrush] {
            let settings = Emberform {
                shape,
                drive_db: 6.0206,
                asymmetry_percent: 100.0,
                bits: 4,
                ..Emberform::default()
            };
            let mut points = [0.0; 9];
            sample_transfer_curve(&settings, &mut points);
            // At input +/-0.25 the gain doubles the envelope and the
            // sample, so the bias is +0.225 before shaping/rounding.
            let expected = match shape {
                DistortionShape::HardClip => [-0.275, 0.725],
                DistortionShape::Bitcrush => [-0.25, 0.75],
                _ => unreachable!(),
            };
            for (actual, expected) in [points[3], points[5]].into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1e-6, "{shape:?}: {points:?}");
            }
        }
    }

    #[test]
    fn first_and_second_antiderivatives_match_derivatives_across_all_shapes() {
        for shape in DistortionShape::ALL {
            if shape == DistortionShape::Bitcrush {
                continue;
            }
            let h = 1e-5;
            for step in -350..350 {
                let x = f64::from(step) * 0.0137;
                if shape == DistortionShape::HardClip && (x.abs() - 1.0).abs() < 0.01 {
                    continue;
                }
                if shape == DistortionShape::Rectifier && x.abs() < 0.01 {
                    continue;
                }
                let (f, f1, _) = eval_shape(shape, x);
                let (_, f1_p, f2_p) = eval_shape(shape, x + h);
                let (_, f1_m, f2_m) = eval_shape(shape, x - h);
                let slope_f1 = (f1_p - f1_m) / (2.0 * h);
                let slope_f2 = (f2_p - f2_m) / (2.0 * h);
                assert!(
                    (slope_f1 - f).abs() < 1e-4,
                    "{shape:?} F1' at {x}: {slope_f1} vs {f}"
                );
                assert!(
                    (slope_f2 - f1).abs() < 1e-4,
                    "{shape:?} F2' at {x}: {slope_f2} vs {f1}"
                );
            }
            let (_, f1_0, f2_0) = eval_shape(shape, 0.0);
            assert!(f1_0.abs() < 1e-12, "{shape:?} F1(0)={f1_0}");
            assert!(f2_0.abs() < 1e-12, "{shape:?} F2(0)={f2_0}");
        }
    }

    #[test]
    fn adaa2_near_equal_samples_fall_back_smoothly_without_nan() {
        for shape in DistortionShape::ALL {
            if shape == DistortionShape::Bitcrush {
                continue;
            }
            let mut st = Adaa2State::default();
            st.rest_at(shape, 0.5);
            let y1 = st.process(shape, 0.5 + 1e-8);
            let y2 = st.process(shape, 0.5);
            let expected = curve_only(shape, 0.5);
            assert!(
                y1.is_finite() && (y1 - expected).abs() < 1e-3,
                "{shape:?}: {y1}"
            );
            assert!(
                y2.is_finite() && (y2 - expected).abs() < 1e-3,
                "{shape:?}: {y2}"
            );
        }
    }
}
