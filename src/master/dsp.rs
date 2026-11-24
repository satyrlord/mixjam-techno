//! Parts that the mastering pedals and meters share: smoothing, biquad
//! filters, four-times oversampling, short delays, true-peak
//! interpolation, and the compressor curve that the lane compressor uses
//! too. They work at the mix rate on fixed-size state, so
//! processing never allocates.
use std::{f64::consts::PI, sync::LazyLock};

#[cfg(test)]
static TABLE_INITIALIZATIONS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Prepare every shared table before a processor reaches the audio thread.
/// Even a bypassed board can later enable a pedal or start its meters.
pub(crate) fn prepare_tables() {
    LazyLock::force(&OUTER);
    LazyLock::force(&INNER);
    LazyLock::force(&PHASES);
    crate::fx::oversample::prepare_tables();
    crate::drive::prepare_tables();
}

/// The mix rate, in frames per second.
pub(crate) const RATE: f64 = crate::audio::Mix::SAMPLE_RATE as f64;

pub(crate) fn db_to_gain(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

pub(crate) fn gain_to_db(gain: f64) -> f64 {
    20.0 * gain.log10()
}

/// Fast audio-rate base-2 logarithm (`x > 0`, normal), `< 2e-6` error (worst
/// near a mantissa of 2).
#[inline]
pub(crate) fn fast_log2(x: f64) -> f64 {
    let bits = x.to_bits();
    let exp = ((bits >> 52) as i64 - 1023) as f64;
    let m = f64::from_bits((bits & 0x000F_FFFF_FFFF_FFFF) | 0x3FF0_0000_0000_0000);
    let t = (m - 1.0) / (m + 1.0);
    let t2 = t * t;
    let ln_m =
        2.0 * t * (1.0 + t2 * (1.0 / 3.0 + t2 * (1.0 / 5.0 + t2 * (1.0 / 7.0 + t2 * (1.0 / 9.0)))));
    exp + ln_m * std::f64::consts::LOG2_E
}

/// Fast audio-rate base-10 logarithm (`x > 0`), `< 5e-7` error.
#[inline]
pub(crate) fn fast_log10(x: f64) -> f64 {
    fast_log2(x) * (1.0 / std::f64::consts::LOG2_10)
}

/// Fast audio-rate base-2 exponential, `< 2e-7` relative error.
#[inline]
pub(crate) fn fast_exp2(x: f64) -> f64 {
    let k = x.round();
    let y = (x - k) * std::f64::consts::LN_2;
    let pow2_f = 1.0
        + y * (1.0
            + y * (0.5
                + y * (1.0 / 6.0 + y * (1.0 / 24.0 + y * (1.0 / 120.0 + y * (1.0 / 720.0))))));
    let pow2_k = f64::from_bits((((k as i64) + 1023).clamp(1, 2046) as u64) << 52);
    pow2_k * pow2_f
}

/// Fast audio-rate dB-to-amplitude conversion (`< 0.00001 dB` error).
#[inline]
pub(crate) fn fast_db_to_gain(db: f64) -> f64 {
    fast_exp2(db * (std::f64::consts::LOG2_10 / 20.0))
}

#[inline]
fn tan_small(u: f64) -> f64 {
    let u2 = u * u;
    u * (945.0 - u2 * (105.0 - u2)) / (945.0 - u2 * (420.0 - 15.0 * u2))
}

/// Fast audio-rate `tan(x)` for `x in (0, 0.48 * PI]`, `< 1e-7` error.
#[inline]
pub(crate) fn fast_tan(x: f64) -> f64 {
    if x <= std::f64::consts::FRAC_PI_4 {
        tan_small(x)
    } else {
        1.0 / tan_small(std::f64::consts::FRAC_PI_2 - x)
    }
}

/// Zero for a value far below hearing, so that recursive states never turn
/// denormal and slow the audio thread down.
#[inline]
pub(crate) fn flush(value: f64) -> f64 {
    if value.abs() < 1e-30 { 0.0 } else { value }
}

/// `flush` for single-precision states.
#[inline]
pub(crate) fn flush32(value: f32) -> f32 {
    if value.abs() < 1e-30 { 0.0 } else { value }
}

/// The smallest of the last `span` values pushed, `span` at most `N`: a
/// queue of the values that can still become the smallest. The limiters
/// keep the lowest gain need of their hold with it.
#[derive(Clone, Copy)]
pub(crate) struct MovingMin<const N: usize> {
    pub(crate) values: [f64; N],
    pub(crate) times: [u32; N],
    pub(crate) len: usize,
    pub(crate) head: usize,
    pub(crate) now: u32,
}
impl<const N: usize> MovingMin<N> {
    pub(crate) const EMPTY: Self = Self {
        values: [0.0; N],
        times: [0; N],
        head: 0,
        len: 0,
        now: 0,
    };

    pub(crate) fn push(&mut self, value: f64, span: usize) -> f64 {
        // Expire before insertion: a full rising queue otherwise overwrites
        // its oldest value before its timestamp can be checked.
        while self.len > 0 && self.now.wrapping_sub(self.times[self.head]) as usize >= span {
            self.head = (self.head + 1) % N;
            self.len -= 1;
        }
        while self.len > 0 && self.values[(self.head + self.len - 1) % N] >= value {
            self.len -= 1;
        }
        let at = (self.head + self.len) % N;
        (self.values[at], self.times[at]) = (value, self.now);
        self.len += 1;
        self.now = self.now.wrapping_add(1);
        self.values[self.head]
    }
}

/// The type a lookahead gain keeps its held gain in.
pub(crate) trait Held: Copy + PartialOrd + Into<f64> {
    const ONE: Self;
    /// A need that the moving minimum gives back. It holds the needs as
    /// pushed, so the conversion is exact.
    fn from_need(need: f64) -> Self;
    /// The held gain one frame of release closer to `lowest`.
    fn release(lowest: Self, held: Self, pole: Self) -> Self;
}
impl Held for f64 {
    const ONE: Self = 1.0;
    fn from_need(need: f64) -> Self {
        need
    }
    #[inline]
    fn release(lowest: Self, held: Self, pole: Self) -> Self {
        flush(lowest + (held - lowest) * pole)
    }
}
/// The master limiter's gain, which has always released without a flush.
impl Held for f32 {
    const ONE: Self = 1.0;
    fn from_need(need: f64) -> Self {
        need as f32
    }
    #[inline]
    fn release(lowest: Self, held: Self, pole: Self) -> Self {
        lowest + (held - lowest) * pole
    }
}

/// The gain of a stereo-linked lookahead limiter. Each frame brings the
/// gain it needs; the gain takes the lowest need of the last `span` frames,
/// recovers after it with a one-pole release, and ramps over the last
/// `LOOKAHEAD` frames, so it is already down when a peak arrives. The
/// limiter delays its sound by `LOOKAHEAD` frames itself.
#[derive(Clone, Copy)]
pub(crate) struct LookaheadGain<T, const LOOKAHEAD: usize, const SPAN: usize> {
    needs: MovingMin<SPAN>,
    span: usize,
    held: T,
    release: T,
    ramp: Ramp<LOOKAHEAD>,
}

/// The mean of the last `N` held gains, at most 1. A lookahead limiter
/// ramps its gain with it, so the gain is already down when a peak arrives.
#[derive(Clone, Copy)]
pub(crate) struct Ramp<const N: usize> {
    held: [f64; N],
    at: usize,
    sum: f64,
}
impl<const N: usize> Ramp<N> {
    pub(crate) const UNITY: Self = Self {
        held: [1.0; N],
        at: 0,
        sum: N as f64,
    };

    #[inline]
    pub(crate) fn push(&mut self, held: f64) -> f64 {
        self.sum += held - self.held[self.at];
        self.held[self.at] = held;
        self.at += 1;
        if self.at == N {
            // Recount now and then, so rounding cannot build up.
            self.at = 0;
            self.sum = self.held.iter().sum();
        }
        (self.sum / N as f64).min(1.0)
    }
}
impl<T: Held, const LOOKAHEAD: usize, const SPAN: usize> LookaheadGain<T, LOOKAHEAD, SPAN> {
    /// `span` is at most `SPAN`, and `release` is the release's pole.
    pub(crate) fn new(span: usize, release: T) -> Self {
        Self {
            needs: MovingMin::EMPTY,
            span,
            held: T::ONE,
            release,
            ramp: Ramp::UNITY,
        }
    }
    /// Forget every need, and keep the span and the release.
    pub(crate) fn clear(&mut self) {
        *self = Self::new(self.span, self.release);
    }
    /// Take a frame's need and return the gain, at most 1, for the frame
    /// `LOOKAHEAD` frames ago.
    #[inline]
    pub(crate) fn push(&mut self, need: T) -> f64 {
        let lowest = T::from_need(self.needs.push(need.into(), self.span));
        self.held = if lowest < self.held {
            lowest
        } else {
            T::release(lowest, self.held, self.release)
        };
        self.ramp.push(self.held.into())
    }
}

/// The feedback coefficient of a one-pole filter with this time constant at `rate`.
pub(crate) fn pole_at(seconds: f64, rate: f64) -> f64 {
    (-1.0 / (seconds * rate).max(1e-6)).exp()
}

/// The feedback coefficient of a one-pole filter with this time constant at [`RATE`].
pub(crate) fn pole(seconds: f64) -> f64 {
    pole_at(seconds, RATE)
}

/// The time constant of the compressors' power detectors.
pub(crate) const DETECTOR: f64 = 0.003;
/// The width of the compressors' soft knee, in dB.
pub(crate) const KNEE_DB: f64 = 6.0;

/// Gain reduction in dB of a soft-knee compressor for a level in dB.
pub(crate) fn reduction(level: f64, threshold: f64, ratio: f64, knee: f64) -> f64 {
    let over = level - threshold;
    let slope = 1.0 - 1.0 / ratio;
    if 2.0 * over <= -knee {
        0.0
    } else if 2.0 * over < knee {
        slope * (over + knee / 2.0).powi(2) / (2.0 * knee)
    } else {
        slope * over
    }
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    loop {
        term *= (x / (2.0 * k)).powi(2);
        sum += term;
        if term < sum * 1e-17 {
            return sum;
        }
        k += 1.0;
    }
}

/// The Kaiser window at `position`, which runs from `-half` to `half`.
fn kaiser(position: f64, half: f64, beta: f64) -> f64 {
    let ratio = position / half;
    bessel_i0(beta * (1.0 - ratio * ratio).max(0.0).sqrt()) / bessel_i0(beta)
}

#[inline]
fn dot<const N: usize>(a: &[f32; N], b: &[f32; N]) -> f32 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

/// Four independent sums let the compiler vectorize the live saturation FIR.
#[inline]
fn dot_lanes<const N: usize>(a: &[f32; N], b: &[f32; N]) -> f32 {
    let mut sums = [0.0; 4];
    let (a, a_tail) = a.as_chunks::<4>();
    let (b, b_tail) = b.as_chunks::<4>();
    for (a, b) in a.iter().zip(b) {
        sums[0] += a[0] * b[0];
        sums[1] += a[1] * b[1];
        sums[2] += a[2] * b[2];
        sums[3] += a[3] * b[3];
    }
    let tail: f32 = a_tail.iter().zip(b_tail).map(|(a, b)| a * b).sum();
    (sums[0] + sums[1]) + (sums[2] + sums[3]) + tail
}

/// The last `N` samples of a filter's input, newest first. Each sample is
/// written twice, `N` apart, so the last `N` are always one slice and no
/// sample moves.
#[derive(Clone, Copy)]
struct History<const N: usize> {
    samples: [[f32; N]; 2],
    /// Where the newest sample is.
    at: usize,
}
impl<const N: usize> History<N> {
    const ZERO: Self = Self {
        samples: [[0.0; N]; 2],
        at: 0,
    };

    /// Take a sample and return the last `N`: entry `i` is the sample `i`
    /// pushes ago.
    #[inline]
    fn push(&mut self, sample: f32) -> &[f32; N] {
        self.at = if self.at == 0 { N - 1 } else { self.at - 1 };
        let at = self.at;
        let samples = self.samples.as_flattened_mut();
        samples[at] = sample;
        samples[at + N] = sample;
        samples[at..at + N]
            .first_chunk()
            .expect("the history holds two copies")
    }
}

// Smoothing -------------------------------------------------------------------

/// A setting that glides to its target along a one-pole curve, so a knob
/// turn reaches the sound without zipper noise. The master pedals' settings
/// glide with a 5 ms time constant, so a turn arrives within about 20 ms;
/// the FX settings each have their own.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Smoothed {
    value: f64,
    target: f64,
    /// The share of the gap that each sample closes.
    step: f64,
    /// The value snaps to the target when the gap is this small, relative
    /// to the target.
    snap: f64,
}
impl Smoothed {
    /// `1 - e^(-1 / (5 ms × 44.1 kHz))`.
    const STEP: f64 = 0.004_524_879;

    /// A setting with a 5 ms glide.
    pub(crate) fn new(value: f64) -> Self {
        Self {
            value,
            target: value,
            step: Self::STEP,
            snap: 1e-6,
        }
    }
    /// A setting with its own time constant, which glides closer to the
    /// target before it snaps.
    pub(crate) fn with_time(value: f64, seconds: f64) -> Self {
        Self {
            value,
            target: value,
            step: 1.0 - pole(seconds),
            snap: 1e-9,
        }
    }
    pub(crate) fn set(&mut self, target: f64) {
        self.target = target;
    }
    pub(crate) fn set_time(&mut self, seconds: f64) {
        self.step = 1.0 - pole(seconds);
    }
    /// Move straight to the target.
    pub(crate) fn settle(&mut self) {
        self.value = self.target;
    }
    /// Set the value, keeping the target: the glide goes on from there.
    pub(crate) fn hold(&mut self, value: f64) {
        self.value = value;
    }
    pub(crate) fn settled(&self) -> bool {
        self.value == self.target
    }
    pub(crate) fn value(&self) -> f64 {
        self.value
    }
    pub(crate) fn target(&self) -> f64 {
        self.target
    }
    #[inline]
    pub(crate) fn next(&mut self) -> f64 {
        if self.value != self.target {
            let gap = self.target - self.value;
            self.value = if gap.abs() <= self.snap * (1.0 + self.target.abs()) {
                self.target
            } else {
                self.value + gap * self.step
            };
        }
        self.value
    }
}

// Biquad filters ----------------------------------------------------------------

/// Normalized biquad coefficients, designed with the formulas of Robert
/// Bristow-Johnson's audio EQ cookbook.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Coefficients {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}
impl Coefficients {
    pub(crate) const FLAT: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    fn new([b0, b1, b2]: [f64; 3], [a0, a1, a2]: [f64; 3]) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
    /// The sine and cosine of a frequency's angle at the mix rate.
    fn angle(hz: f64) -> (f64, f64) {
        (2.0 * PI * hz / RATE).sin_cos()
    }
    pub(crate) fn low_pass(hz: f64, q: f64) -> Self {
        Self::low_pass_at(hz, q, RATE)
    }
    /// A low-pass at any sample rate, for a sound that is not at the mix rate.
    pub(crate) fn low_pass_at(hz: f64, q: f64, rate: f64) -> Self {
        let (sin, cos) = (2.0 * PI * hz / rate).sin_cos();
        let alpha = sin / (2.0 * q);
        let side = (1.0 - cos) / 2.0;
        Self::new(
            [side, 1.0 - cos, side],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }
    pub(crate) fn high_pass(hz: f64, q: f64) -> Self {
        let (sin, cos) = Self::angle(hz);
        let alpha = sin / (2.0 * q);
        let side = (1.0 + cos) / 2.0;
        Self::new(
            [side, -(1.0 + cos), side],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }
    pub(crate) fn all_pass(hz: f64, q: f64) -> Self {
        let (sin, cos) = Self::angle(hz);
        let alpha = sin / (2.0 * q);
        Self::new(
            [1.0 - alpha, -2.0 * cos, 1.0 + alpha],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }
    pub(crate) fn peaking(hz: f64, q: f64, db: f64) -> Self {
        let (sin, cos) = Self::angle(hz);
        let alpha = sin / (2.0 * q);
        let a = 10f64.powf(db / 40.0);
        Self::new(
            [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a],
        )
    }
    /// A shelf's `alpha` term and `A` for a slope and gain. The slope is held
    /// to at most 1: above it, a high gain makes the square root negative.
    fn shelf(sin: f64, slope: f64, db: f64) -> (f64, f64) {
        let slope = slope.clamp(f64::EPSILON, 1.0);
        let a = 10f64.powf(db / 40.0);
        let alpha = sin / 2.0 * ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).sqrt();
        (a, 2.0 * a.sqrt() * alpha)
    }
    pub(crate) fn low_shelf(hz: f64, slope: f64, db: f64) -> Self {
        let (sin, cos) = Self::angle(hz);
        let (a, root) = Self::shelf(sin, slope, db);
        Self::new(
            [
                a * ((a + 1.0) - (a - 1.0) * cos + root),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                a * ((a + 1.0) - (a - 1.0) * cos - root),
            ],
            [
                (a + 1.0) + (a - 1.0) * cos + root,
                -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                (a + 1.0) + (a - 1.0) * cos - root,
            ],
        )
    }
    pub(crate) fn high_shelf(hz: f64, slope: f64, db: f64) -> Self {
        let (sin, cos) = Self::angle(hz);
        let (a, root) = Self::shelf(sin, slope, db);
        Self::new(
            [
                a * ((a + 1.0) + (a - 1.0) * cos + root),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                a * ((a + 1.0) + (a - 1.0) * cos - root),
            ],
            [
                (a + 1.0) - (a - 1.0) * cos + root,
                2.0 * ((a - 1.0) - (a + 1.0) * cos),
                (a + 1.0) - (a - 1.0) * cos - root,
            ],
        )
    }
    /// Poles mapped from continuous-time `s^2 + 2*zeta*w0*s + w0^2` via `z = exp(s*T)`.
    fn matched_poles(w0: f64, zeta: f64) -> (f64, f64) {
        if zeta <= 1.0 {
            let r = (-zeta * w0).exp();
            let a1 = -2.0 * r * (w0 * (1.0 - zeta * zeta).max(0.0).sqrt()).cos();
            (a1, r * r)
        } else {
            let s = (zeta * zeta - 1.0).sqrt();
            let r1 = (-w0 * (zeta + s)).exp();
            let r2 = (-w0 * (zeta - s)).exp();
            (-(r1 + r2), r1 * r2)
        }
    }
    /// Solve symmetric real biquad numerator `[b0, b1, b2]` matching DC, Nyquist,
    /// and crossover frequency `f0` squared magnitudes against analog prototype.
    fn solve_matched_numerator(
        a1: f64,
        a2: f64,
        f0: f64,
        g0_sq: f64,
        g_nyq_sq: f64,
        g_f0_sq: f64,
    ) -> [f64; 3] {
        let a0_sq = (1.0 + a1 + a2).powi(2);
        let a1_sq = (1.0 - a1 + a2).powi(2);
        let phi0 = (PI * f0 / RATE).sin().powi(2);
        let a_f0 = a0_sq * (1.0 - phi0) + a1_sq * phi0 - 16.0 * a2 * phi0 * (1.0 - phi0);
        let b0_sq = g0_sq * a0_sq;
        let b1_sq = g_nyq_sq * a1_sq;
        let b_f0 = g_f0_sq * a_f0;
        let four_b0b2 = -(b_f0 - b0_sq * (1.0 - phi0) - b1_sq * phi0) / (4.0 * phi0 * (1.0 - phi0));
        let sb0 = b0_sq.max(0.0).sqrt();
        let sb1 = b1_sq.max(0.0).sqrt();
        let w = 0.5 * (sb0 + sb1);
        let b1 = 0.5 * (sb0 - sb1);
        let disc = (w * w - four_b0b2).max(0.0).sqrt();
        [0.5 * (w + disc), b1, 0.5 * (w - disc)]
    }
    /// Continuous-time analog RBJ high-shelf squared magnitude at frequency `hz`.
    pub(crate) fn analog_high_shelf_mag2(hz: f64, fc: f64, slope: f64, db: f64) -> f64 {
        let slope = slope.clamp(f64::EPSILON, 1.0);
        let a = 10f64.powf(db / 40.0);
        let inv_q = ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).sqrt();
        let k = a.sqrt() * inv_q;
        let u = hz / fc;
        let uu = u * u;
        (a * a) * ((1.0 - a * uu).powi(2) + (k * u).powi(2)) / ((a - uu).powi(2) + (k * u).powi(2))
    }
    /// Nyquist-decramped high-shelf filter matching the continuous-time analog prototype.
    pub(crate) fn high_shelf_matched(hz: f64, slope: f64, db: f64) -> Self {
        if db.abs() < 1e-9 {
            return Self::FLAT;
        }
        if db < 0.0 {
            let boost = Self::high_shelf_matched(hz, slope, -db);
            return Self::new([1.0, boost.a1, boost.a2], [boost.b0, boost.b1, boost.b2]);
        }
        let slope = slope.clamp(f64::EPSILON, 1.0);
        let a = 10f64.powf(db / 40.0);
        let inv_q = ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).sqrt();
        let w_p = 2.0 * PI * hz * a.sqrt() / RATE;
        let (a1, a2) = Self::matched_poles(w_p, 0.5 * inv_q);
        let g_nyq_sq = Self::analog_high_shelf_mag2(0.5 * RATE, hz, slope, db);
        let b = Self::solve_matched_numerator(a1, a2, hz, 1.0, g_nyq_sq, a * a);
        Self::new(b, [1.0, a1, a2])
    }
    /// Nyquist-decramped peaking EQ filter matching the continuous-time analog prototype.
    pub(crate) fn peaking_matched(hz: f64, q: f64, db: f64) -> Self {
        if db.abs() < 1e-9 {
            return Self::FLAT;
        }
        let a = 10f64.powf(db / 40.0);
        let w0 = 2.0 * PI * hz / RATE;
        let (a1, a2) = Self::matched_poles(w0, 1.0 / (2.0 * q * a));
        let u = 0.5 * RATE / hz;
        let uu = 1.0 - u * u;
        let g_nyq_sq = (uu * uu + (u * a / q).powi(2)) / (uu * uu + (u / (q * a)).powi(2));
        let b = Self::solve_matched_numerator(a1, a2, hz, 1.0, g_nyq_sq, a.powi(4));
        Self::new(b, [1.0, a1, a2])
    }
    /// Coefficients given directly, for filters designed elsewhere.
    pub(crate) fn raw(b: [f64; 3], a: [f64; 3]) -> Self {
        Self::new(b, a)
    }
    /// `[b0, b1, b2, a1, a2]`, for a filter that runs its own loop.
    pub(crate) fn values(&self) -> [f64; 5] {
        [self.b0, self.b1, self.b2, self.a1, self.a2]
    }
    /// The magnitude response at a frequency.
    pub(crate) fn magnitude(&self, hz: f64) -> f64 {
        let w = 2.0 * PI * hz / RATE;
        let part = |c0: f64, c1: f64, c2: f64| {
            let re = c0 + c1 * w.cos() + c2 * (2.0 * w).cos();
            let im = -(c1 * w.sin() + c2 * (2.0 * w).sin());
            re.hypot(im)
        };
        part(self.b0, self.b1, self.b2) / part(1.0, self.a1, self.a2)
    }
}

/// A biquad in transposed direct form 2, in double precision, with state
/// for two channels. Its coefficients may change between samples.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Biquad {
    coefficients: Coefficients,
    state: [[f64; 2]; 2],
}
impl Biquad {
    pub(crate) const FLAT: Self = Self::new(Coefficients::FLAT);

    pub(crate) const fn new(coefficients: Coefficients) -> Self {
        Self {
            coefficients,
            state: [[0.0; 2]; 2],
        }
    }
    pub(crate) fn set(&mut self, coefficients: Coefficients) {
        self.coefficients = coefficients;
    }
    pub(crate) fn clear(&mut self) {
        self.state = [[0.0; 2]; 2];
    }
    pub(crate) fn history(&self) -> [[f64; 2]; 2] {
        self.state
    }
    pub(crate) fn set_history(&mut self, history: [[f64; 2]; 2]) {
        self.state = history;
    }
    #[inline]
    pub(crate) fn process(&mut self, channel: usize, input: f64) -> f64 {
        let c = &self.coefficients;
        let [s1, s2] = &mut self.state[channel];
        let output = c.b0 * input + *s1;
        *s1 = flush(c.b1 * input - c.a1 * output + *s2);
        *s2 = flush(c.b2 * input - c.a2 * output);
        output
    }

    #[inline]
    pub(crate) fn process_unflushed(&mut self, channel: usize, input: f64) -> f64 {
        let c = &self.coefficients;
        let [s1, s2] = &mut self.state[channel];
        let output = c.b0 * input + *s1;
        *s1 = c.b1 * input - c.a1 * output + *s2;
        *s2 = c.b2 * input - c.a2 * output;
        output
    }

    pub(crate) fn flush_state(&mut self) {
        for ch in &mut self.state {
            if ch[0].abs() < 1e-46 {
                ch[0] = 0.0;
            }
            if ch[1].abs() < 1e-46 {
                ch[1] = 0.0;
            }
        }
    }
}

/// A `Biquad` that holds each state of both channels side by side, so that
/// one frame runs as one pair of lanes. Each lane does the same arithmetic as
/// `Biquad::process_unflushed`, so it gives the same bits.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StereoBiquad {
    coefficients: Coefficients,
    s1: [f64; 2],
    s2: [f64; 2],
}
impl StereoBiquad {
    pub(crate) const FLAT: Self = Self::new(Coefficients::FLAT);

    pub(crate) const fn new(coefficients: Coefficients) -> Self {
        Self {
            coefficients,
            s1: [0.0; 2],
            s2: [0.0; 2],
        }
    }

    pub(crate) fn set(&mut self, coefficients: Coefficients) {
        self.coefficients = coefficients;
    }

    #[inline]
    pub(crate) fn process_unflushed(&mut self, input: [f64; 2]) -> [f64; 2] {
        let c = &self.coefficients;
        let output = [c.b0 * input[0] + self.s1[0], c.b0 * input[1] + self.s1[1]];
        self.s1 = [
            c.b1 * input[0] - c.a1 * output[0] + self.s2[0],
            c.b1 * input[1] - c.a1 * output[1] + self.s2[1],
        ];
        self.s2 = [
            c.b2 * input[0] - c.a2 * output[0],
            c.b2 * input[1] - c.a2 * output[1],
        ];
        output
    }

    /// The block-end guard of `Biquad::flush_state`.
    pub(crate) fn flush_state(&mut self) {
        for state in self.s1.iter_mut().chain(&mut self.s2) {
            if state.abs() < 1e-46 {
                *state = 0.0;
            }
        }
    }
}

/// One channel's sample through biquads in a row.
#[inline]
pub(crate) fn cascade(filters: &mut [Biquad], channel: usize, sample: f64) -> f64 {
    filters
        .iter_mut()
        .fold(sample, |sample, filter| filter.process(channel, sample))
}

// Saturation helpers ------------------------------------------------------------------

/// A power far below hearing, so that silence divides by nothing.
pub(crate) const POWER_FLOOR: f64 = 1e-12;

/// `ln cosh u`, the antiderivative of `tanh`, without overflow.
#[inline]
pub(crate) fn log_cosh(u: f64) -> f64 {
    let size = u.abs();
    size + (-2.0 * size).exp().ln_1p() - std::f64::consts::LN_2
}

/// A power detector: two one-pole stages in a row, which smooth the ripple
/// of a low note far better than one stage with the same response time.
#[derive(Clone, Copy, Default)]
pub(crate) struct PowerDetector([f64; 2]);

impl PowerDetector {
    #[inline]
    pub(crate) fn follow(&mut self, power: f64, pole: f64) -> f64 {
        let [first, second] = &mut self.0;
        *first = flush(power + (*first - power) * pole);
        *second = flush(*first + (*second - *first) * pole);
        *second
    }
}

// Oversampling -------------------------------------------------------------------

/// The taps at odd offsets from the centre of a Kaiser-windowed halfband
/// low-pass with `2 TAPS - 1` taps. Its other taps are zero except the
/// centre, which is one half. The taps sum to one half, so DC passes exactly.
pub(crate) fn halfband<const TAPS: usize>(beta: f64) -> [f32; TAPS] {
    let middle = (TAPS - 1) as f64;
    let raw: [f64; TAPS] = std::array::from_fn(|j| {
        let offset = 2.0 * j as f64 - middle;
        0.5 * sinc(offset / 2.0) * kaiser(offset, middle, beta)
    });
    let sum: f64 = raw.iter().sum();
    raw.map(|tap| (tap * 0.5 / sum) as f32)
}

/// The outer stage, between the mix rate and twice it: 127 taps, which
/// pass up to 20 kHz and stop from 24 kHz by about 90 dB.
pub(crate) static OUTER: LazyLock<[f32; 64]> = LazyLock::new(|| {
    #[cfg(test)]
    TABLE_INITIALIZATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    halfband(9.0)
});
/// The inner stage, between twice and four times the mix rate: 23 taps.
pub(crate) static INNER: LazyLock<[f32; 12]> = LazyLock::new(|| {
    #[cfg(test)]
    TABLE_INITIALIZATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    halfband(9.0)
});

/// One channel of one halfband stage, up and down. `TAPS - 1` is odd, so
/// every tap that the polyphase sums use is nonzero.
#[derive(Clone, Copy)]
pub(crate) struct Halfband<const TAPS: usize> {
    input: History<TAPS>,
    even: History<TAPS>,
    odd: History<TAPS>,
}
impl<const TAPS: usize> Halfband<TAPS> {
    pub(crate) const ZERO: Self = Self {
        input: History::ZERO,
        even: History::ZERO,
        odd: History::ZERO,
    };

    /// Two output samples for one input sample.
    #[inline]
    pub(crate) fn up(&mut self, taps: &[f32; TAPS], sample: f32) -> [f32; 2] {
        self.up_filtered::<false, TAPS>(taps, sample)
    }

    #[inline]
    fn up_filtered<const GROUPED: bool, const USED: usize>(
        &mut self,
        taps: &[f32; USED],
        sample: f32,
    ) -> [f32; 2] {
        let dot = if GROUPED {
            dot_lanes::<USED>
        } else {
            dot::<USED>
        };
        let input = self
            .input
            .push(sample)
            .first_chunk()
            .expect("FIR taps fit the history");
        [2.0 * dot(taps, input), input[USED / 2 - 1]]
    }
    /// One output sample for two input samples.
    #[inline]
    pub(crate) fn down(&mut self, taps: &[f32; TAPS], [even, odd]: [f32; 2]) -> f32 {
        self.down_filtered::<false, TAPS>(taps, [even, odd])
    }

    #[inline]
    fn down_filtered<const GROUPED: bool, const USED: usize>(
        &mut self,
        taps: &[f32; USED],
        [even, odd]: [f32; 2],
    ) -> f32 {
        let dot = if GROUPED {
            dot_lanes::<USED>
        } else {
            dot::<USED>
        };
        let even = self
            .even
            .push(even)
            .first_chunk()
            .expect("FIR taps fit the history");
        let odd = self.odd.push(odd);
        dot(taps, even) + 0.5 * odd[USED / 2]
    }
}

/// Runs a curve at four times the mix rate for one channel: two halfband
/// stages up, the curve on each of the four samples, and the same stages
/// down. The output follows the input by a whole number of frames.
#[derive(Clone, Copy)]
pub(crate) struct Oversampler {
    outer: Halfband<64>,
    inner: Halfband<12>,
    /// One sample at twice the rate. It makes the inner stage's delay of
    /// five and a half frames a whole six.
    held: f32,
}
impl Oversampler {
    /// 63 frames for the outer stage and 6 for the inner one.
    pub(crate) const LATENCY: usize = 69;

    #[cfg(test)]
    #[inline]
    pub(crate) fn process(&mut self, sample: f32, curve: impl FnMut(f32) -> f32) -> f32 {
        self.process_with_outer::<false, 64>(sample, &OUTER, curve)
    }

    /// Run two curves on the same upsampled input, with separate output
    /// filters. Both histories must start at ZERO and remain paired: the
    /// reference's input filters are unused, so do not call `process` on it.
    /// Each output keeps the arithmetic order of an independent processor.
    #[cfg(test)]
    #[inline]
    pub(crate) fn process_pair(
        &mut self,
        reference: &mut Self,
        sample: f32,
        mut curve: impl FnMut(f32) -> [f32; 2],
    ) -> [f32; 2] {
        let (outer, inner) = (&*OUTER, &*INNER);
        let [a, b] = self.outer.up(outer, sample);
        let [p0, p1] = self.inner.up(inner, a);
        let [p2, p3] = self.inner.up(inner, b);
        let shaped = [p0, p1, p2, p3].map(&mut curve);
        let mut output = [0.0; 2];
        for (index, state) in [self, reference].into_iter().enumerate() {
            let c = state
                .inner
                .down(inner, [shaped[0][index], shaped[1][index]]);
            let d = state
                .inner
                .down(inner, [shaped[2][index], shaped[3][index]]);
            let pair = [state.held, c];
            state.held = d;
            output[index] = state.outer.down(outer, pair);
        }
        output
    }
}

impl Oversampler {
    pub(crate) const ZERO: Self = Self {
        outer: Halfband::ZERO,
        inner: Halfband::ZERO,
        held: 0.0,
    };

    #[inline]
    pub(crate) fn process_live_saturation<const TAPS: usize>(
        &mut self,
        sample: f32,
        outer: &[f32; TAPS],
        nonlinear: bool,
        curve: impl FnMut(f32) -> f32,
    ) -> f32 {
        if nonlinear {
            self.process_with_outer::<true, TAPS>(sample, outer, curve)
        } else {
            self.process_with_outer::<false, TAPS>(sample, outer, curve)
        }
    }

    #[inline]
    fn process_with_outer<const GROUPED: bool, const TAPS: usize>(
        &mut self,
        sample: f32,
        outer: &[f32; TAPS],
        mut curve: impl FnMut(f32) -> f32,
    ) -> f32 {
        let inner = &*INNER;
        let [a, b] = self.outer.up_filtered::<GROUPED, TAPS>(outer, sample);
        let [p0, p1] = self.inner.up_filtered::<GROUPED, 12>(inner, a);
        let [p2, p3] = self.inner.up_filtered::<GROUPED, 12>(inner, b);
        let c = self
            .inner
            .down_filtered::<GROUPED, 12>(inner, [curve(p0), curve(p1)]);
        let d = self
            .inner
            .down_filtered::<GROUPED, 12>(inner, [curve(p2), curve(p3)]);
        let pair = [self.held, c];
        self.held = d;
        self.outer.down_filtered::<GROUPED, TAPS>(outer, pair)
    }
}

// Delays and peaks ---------------------------------------------------------------------

/// A delay of exactly `N` frames for one channel.
#[derive(Clone, Copy)]
pub(crate) struct Delay<const N: usize> {
    buffer: [f32; N],
    position: usize,
}
impl<const N: usize> Delay<N> {
    pub(crate) const ZERO: Self = Self {
        buffer: [0.0; N],
        position: 0,
    };

    /// A delay that has held `value` for as long as it reaches.
    pub(crate) const fn filled(value: f32) -> Self {
        Self {
            buffer: [value; N],
            position: 0,
        }
    }

    #[inline]
    pub(crate) fn push(&mut self, sample: f32) -> f32 {
        self.push_delayed(sample, N)
    }

    /// Delay by up to the buffer's capacity, including zero frames.
    #[inline]
    pub(crate) fn push_delayed(&mut self, sample: f32, frames: usize) -> f32 {
        debug_assert!(frames <= N);
        let out = if frames == 0 {
            sample
        } else {
            self.buffer[(self.position + N - frames) % N]
        };
        self.buffer[self.position] = sample;
        self.position = if self.position + 1 == N {
            0
        } else {
            self.position + 1
        };
        out
    }
}

/// The three points between two samples, at quarters, as a 32-tap
/// Kaiser-windowed sinc gives them: within 0.06 dB up to 20 kHz. Each
/// row's taps sum to one.
static PHASES: LazyLock<[[f32; 32]; 3]> = LazyLock::new(|| {
    #[cfg(test)]
    TABLE_INITIALIZATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::array::from_fn(|phase| {
        let fraction = (phase + 1) as f64 / 4.0;
        // History entry `i` holds the sample `i` frames ago; the interval
        // runs from entry 16 to entry 15.
        let raw: [f64; 32] = std::array::from_fn(|i| {
            let distance = 16.0 - i as f64 - fraction;
            sinc(distance) * kaiser(distance, 17.0, 5.0)
        });
        let sum: f64 = raw.iter().sum();
        raw.map(|tap| (tap / sum) as f32)
    })
});

/// True-peak estimates for one channel by four-times interpolation, in the
/// manner of ITU-R BS.1770: the peak of each interval between samples.
#[derive(Clone, Copy)]
pub(crate) struct TruePeak {
    history: History<32>,
}
impl TruePeak {
    /// The interval that `push` measures starts this many frames before
    /// the sample it takes.
    pub(crate) const DELAY: usize = 16;
    pub(crate) const ZERO: Self = Self {
        history: History::ZERO,
    };

    /// Take the next sample and return the peak of the interval from the
    /// sample `DELAY` frames ago to the next one, both samples included.
    #[inline]
    pub(crate) fn push(&mut self, sample: f32) -> f32 {
        let history = self.history.push(sample);
        PHASES
            .iter()
            .map(|taps| dot(taps, history).abs())
            .fold(history[15].abs().max(history[16].abs()), f32::max)
    }
}

/// A first-order high-pass at 5 Hz that removes the offset an asymmetric
/// curve leaves behind.
#[derive(Clone, Copy)]
pub(crate) struct DcBlocker {
    input: f64,
    output: f64,
}
impl DcBlocker {
    pub(crate) const ZERO: Self = Self {
        input: 0.0,
        output: 0.0,
    };

    #[inline]
    pub(crate) fn process(&mut self, sample: f64, pole: f64) -> f64 {
        self.output = flush(sample - self.input + pole * self.output);
        self.input = sample;
        self.output
    }
}

/// Two `DcBlocker`s side by side, one per lane, with the same arithmetic.
#[derive(Clone, Copy)]
pub(crate) struct StereoDcBlocker {
    input: [f64; 2],
    output: [f64; 2],
}
impl StereoDcBlocker {
    pub(crate) const ZERO: Self = Self {
        input: [0.0; 2],
        output: [0.0; 2],
    };

    #[inline]
    pub(crate) fn process(&mut self, sample: [f64; 2], pole: f64) -> [f64; 2] {
        self.output = [
            flush(sample[0] - self.input[0] + pole * self.output[0]),
            flush(sample[1] - self.input[1] + pole * self.output[1]),
        ];
        self.input = sample;
        self.output
    }
}

/// A seeded stand-in for a mix at -18 dBFS RMS: kick-like thumps, a bass
/// line, bursts of noise like hi-hats, and a tone, a little different on
/// each side.
#[cfg(test)]
pub(crate) fn program(frames: usize) -> Vec<(f32, f32)> {
    let mut seed: u32 = 0x1234_5678;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        f64::from(seed) / f64::from(u32::MAX) * 2.0 - 1.0
    };
    let beat = (RATE * 60.0 / 128.0) as usize;
    let mut smooth = 0.0;
    let raw: Vec<(f64, f64)> = (0..frames)
        .map(|i| {
            let t = i as f64 / RATE;
            let since = (i % beat) as f64 / RATE;
            let kick = (2.0 * PI * (50.0 + 90.0 * (-since * 30.0).exp()) * since).sin()
                * (-since * 9.0).exp();
            let offbeat = (i / (beat / 2)) % 2 == 1;
            let bass = 0.5 * (2.0 * PI * 55.0 * t).sin() * if offbeat { 1.0 } else { 0.3 };
            let n = noise();
            smooth += (n - smooth) * 0.3;
            let hat = if i % (beat / 2) < beat / 8 {
                (n - smooth) * 0.6
            } else {
                0.0
            };
            let tone = 0.15 * (2.0 * PI * 440.0 * t).sin() * (PI * t).sin().abs();
            (
                kick + bass + hat + tone,
                0.9 * kick + bass + 0.8 * hat - tone,
            )
        })
        .collect();
    let power = raw.iter().map(|(l, r)| (l * l + r * r) / 2.0).sum::<f64>() / frames as f64;
    let scale = db_to_gain(-18.0) / power.sqrt();
    raw.into_iter()
        .map(|(l, r)| ((l * scale) as f32, (r * scale) as f32))
        .collect()
}

#[cfg(test)]
mod tests;
