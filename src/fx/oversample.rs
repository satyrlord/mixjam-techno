//! Zero-allocation FX oversamplers for Live (polyphase IIR) and Offline
//! (linear-phase integer-latency half-band FIR) processing.
use crate::master::dsp::{Halfband, INNER, OUTER, halfband};
use std::sync::LazyLock;

/// Selects playback or export processing for the effects and master board.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderQuality {
    /// Playback with fixed master alignment and a smaller processing budget.
    #[default]
    Live,
    /// High-ratio linear-phase oversampling with integer delay compensation.
    Offline,
}

/// Inner high-rate stage for 8x/16x cascades: 15 taps (8 odd offsets).
static OUTER_MID: LazyLock<[f32; 8]> = LazyLock::new(|| halfband(9.0));

/// Force initialization of FX/master oversampler tap tables before audio starts.
pub(crate) fn prepare_tables() {
    LazyLock::force(&OUTER_MID);
}

#[inline]
fn flush(v: f64) -> f64 {
    if v.abs() < 1e-30 { 0.0 } else { v }
}

#[derive(Clone, Copy)]
struct PolyphasePath<const N: usize> {
    x_prev: [f64; N],
    y_prev: [f64; N],
}
impl<const N: usize> PolyphasePath<N> {
    const ZERO: Self = Self {
        x_prev: [0.0; N],
        y_prev: [0.0; N],
    };

    #[inline]
    fn step(&mut self, coefs: &[f64; N], mut x: f64) -> f64 {
        for (&a, (xp, yp)) in coefs
            .iter()
            .zip(self.x_prev.iter_mut().zip(&mut self.y_prev))
        {
            let y = flush(a * (x - *yp) + *xp);
            *xp = x;
            *yp = y;
            x = y;
        }
        x
    }
}

const HB_A: [f64; 4] = [
    0.041_990_362_926_260_6,
    0.311_556_375_012_294,
    0.639_598_615_604_573_3,
    0.927_214_201_577_095_4,
];
const HB_B: [f64; 3] = [
    0.155_621_116_573_42,
    0.479_402_088_963_036_9,
    0.786_647_025_233_635_4,
];

/// Fast zero-PDC 2x polyphase IIR oversampler for Live playback.
#[derive(Clone, Copy)]
pub(crate) struct HalfBandIir2x {
    up_a: PolyphasePath<4>,
    up_b: PolyphasePath<3>,
    dn_a: PolyphasePath<4>,
    dn_b: PolyphasePath<3>,
}
impl HalfBandIir2x {
    pub(crate) const ZERO: Self = Self {
        up_a: PolyphasePath::ZERO,
        up_b: PolyphasePath::ZERO,
        dn_a: PolyphasePath::ZERO,
        dn_b: PolyphasePath::ZERO,
    };

    pub(crate) fn clear(&mut self) {
        *self = Self::ZERO;
    }

    #[inline]
    pub(crate) fn process(&mut self, sample: f64, mut f: impl FnMut(f64) -> f64) -> f64 {
        let p0 = f(self.up_a.step(&HB_A, sample));
        let p1 = f(self.up_b.step(&HB_B, sample));
        0.5 * (self.dn_a.step(&HB_A, p1) + self.dn_b.step(&HB_B, p0))
    }
}

/// Linear-phase integer-latency 8x/16x FIR oversampler reusing `master::dsp::Halfband`.
#[derive(Clone, Copy)]
pub(crate) struct FirOversampler {
    s1: Halfband<64>,
    s2: Halfband<12>,
    s4: Halfband<8>,
    s8: Halfband<8>,
    held2: f32,
    held4: f32,
    held8: f32,
    use_16x: bool,
}
impl FirOversampler {
    pub(crate) const LATENCY_8X: usize = 71;
    pub(crate) const LATENCY_16X: usize = 72;

    pub(crate) const fn new(use_16x: bool) -> Self {
        Self {
            s1: Halfband::ZERO,
            s2: Halfband::ZERO,
            s4: Halfband::ZERO,
            s8: Halfband::ZERO,
            held2: 0.0,
            held4: 0.0,
            held8: 0.0,
            use_16x,
        }
    }

    pub(crate) fn clear(&mut self) {
        let use_16x = self.use_16x;
        *self = Self::new(use_16x);
    }

    #[inline]
    pub(crate) fn process(&mut self, sample: f64, mut f: impl FnMut(f64) -> f64) -> f64 {
        let [a, b] = self.s1.up(&OUTER, sample as f32);
        let c = self.process_s2(a, &mut f);
        let d = self.process_s2(b, &mut f);
        let pair = [self.held2, c];
        self.held2 = d;
        f64::from(self.s1.down(&OUTER, pair))
    }

    #[inline]
    fn process_s2(&mut self, sample: f32, f: &mut impl FnMut(f64) -> f64) -> f32 {
        let [a, b] = self.s2.up(&INNER, sample);
        let c = self.process_s4(a, f);
        let d = self.process_s4(b, f);
        let pair = [self.held4, c];
        self.held4 = d;
        self.s2.down(&INNER, pair)
    }

    #[inline]
    fn process_s4(&mut self, sample: f32, f: &mut impl FnMut(f64) -> f64) -> f32 {
        let [a, b] = self.s4.up(&OUTER_MID, sample);
        let pair = if self.use_16x {
            let [p0, p1] = self.s8.up(&OUTER_MID, a);
            let [p2, p3] = self.s8.up(&OUTER_MID, b);
            let x0 = f(f64::from(p0)) as f32;
            let x1 = f(f64::from(p1)) as f32;
            let x2 = f(f64::from(p2)) as f32;
            let x3 = f(f64::from(p3)) as f32;
            let c = self.s8.down(&OUTER_MID, [x0, x1]);
            let d = self.s8.down(&OUTER_MID, [x2, x3]);
            let pair = [self.held8, c];
            self.held8 = d;
            pair
        } else {
            [f(f64::from(a)) as f32, f(f64::from(b)) as f32]
        };
        self.s4.down(&OUTER_MID, pair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn fir_oversamplers_have_exact_integer_latency_and_unity_dc() {
        for (use_16x, expected) in [
            (false, FirOversampler::LATENCY_8X),
            (true, FirOversampler::LATENCY_16X),
        ] {
            let mut os = FirOversampler::new(use_16x);
            let mut sum = 0.0;
            let mut peak = 0.0_f64;
            let mut peak_at = 0;
            for i in 0..200 {
                let x = if i == 0 { 1.0 } else { 0.0 };
                let y = os.process(x, |v| v);
                sum += y;
                if y.abs() > peak {
                    peak = y.abs();
                    peak_at = i;
                }
            }
            assert_eq!(peak_at, expected, "16x={use_16x}");
            assert!((sum - 1.0).abs() < 1e-4, "sum={sum}");
            assert!((peak - 1.0).abs() < 0.02, "peak={peak}");
        }
    }

    #[test]
    fn iir_and_fir_suppress_aliasing_from_hard_cubic_clip_above_90_db() {
        // Test 4048 Hz sine clipped strongly (odd+even polynomial) through 2x IIR and 8x FIR.
        let nfft = 16384;
        let f0 = 4048.0;
        let run = |mode_fir: Option<bool>| {
            let mut iir = HalfBandIir2x::ZERO;
            let mut fir = FirOversampler::new(mode_fir.unwrap_or(false));
            let mut out = vec![0.0_f64; nfft + 4096];
            for (n, dst) in out.iter_mut().enumerate() {
                let x = 0.707 * (2.0 * PI * f0 * n as f64 / 44_100.0).sin();
                let shape = |u: f64| {
                    let s = 8.0 * (u + 0.1 * u * u);
                    s / (1.0 + s * s).sqrt()
                };
                *dst = match mode_fir {
                    None => iir.process(x, shape),
                    Some(_) => fir.process(x, shape),
                };
            }
            // Measure fund (4048) vs folded spur at 7*4048 -> 15764 Hz
            let seg = &out[4096..];
            let tone = |hz: f64| {
                let (mut re, mut im) = (0.0, 0.0);
                for (i, &v) in seg.iter().enumerate() {
                    let w = 0.5 * (1.0 - (2.0 * PI * i as f64 / seg.len() as f64).cos());
                    let ph = 2.0 * PI * hz * i as f64 / 44_100.0;
                    re += v * w * ph.cos();
                    im += v * w * ph.sin();
                }
                (re * re + im * im).sqrt()
            };
            20.0 * (tone(15_764.0) / tone(f0).max(1e-12)).log10()
        };
        let iir_spur = run(None);
        let fir8_spur = run(Some(false));
        let fir16_spur = run(Some(true));
        assert!(fir8_spur < -70.0, "8x FIR spur {fir8_spur:.2} dBc");
        assert!(fir16_spur < -90.0, "16x FIR spur {fir16_spur:.2} dBc");
        assert!(iir_spur < -45.0, "2x IIR spur {iir_spur:.2} dBc");
    }

    #[test]
    fn polyphase_iir_is_flat_in_passband() {
        let mut iir = HalfBandIir2x::ZERO;
        let hz = 18_000.0;
        let mut peak_out = 0.0_f64;
        for i in 0..8192 {
            let x = (2.0 * PI * hz * i as f64 / 44_100.0).sin();
            let y = iir.process(x, |v| v);
            if i > 4096 {
                peak_out = peak_out.max(y.abs());
            }
        }
        assert!((peak_out - 1.0).abs() < 0.01, "18 kHz gain={peak_out}");
    }
}
