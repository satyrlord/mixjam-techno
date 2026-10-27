//! Parts that the mastering pedals and meters share: smoothing, biquad
//! filters, four-times oversampling, short delays, and true-peak
//! interpolation. They work at the mix rate on fixed-size state, so
//! processing never allocates.
use std::{f64::consts::PI, sync::LazyLock};

/// The mix rate, in frames per second.
pub(crate) const RATE: f64 = 44_100.0;

pub(crate) fn db_to_gain(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

pub(crate) fn gain_to_db(gain: f64) -> f64 {
    20.0 * gain.log10()
}

/// Zero for a value far below hearing, so that recursive states never turn
/// denormal and slow the audio thread down.
#[inline]
pub(crate) fn flush(value: f64) -> f64 {
    if value.abs() < 1e-30 { 0.0 } else { value }
}

/// The feedback coefficient of a one-pole filter with this time constant.
pub(crate) fn pole(seconds: f64) -> f64 {
    (-1.0 / (seconds * RATE)).exp()
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

// Smoothing -------------------------------------------------------------------

/// A setting that glides to its target with a 5 ms time constant, so a knob
/// turn reaches the sound within about 20 ms without zipper noise.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Smoothed {
    value: f64,
    target: f64,
}
impl Smoothed {
    /// `1 - e^(-1 / (5 ms × 44.1 kHz))`.
    const STEP: f64 = 0.004_524_879;

    pub(crate) fn new(value: f64) -> Self {
        Self {
            value,
            target: value,
        }
    }
    pub(crate) fn set(&mut self, target: f64) {
        self.target = target;
    }
    /// Move straight to the target.
    pub(crate) fn settle(&mut self) {
        self.value = self.target;
    }
    pub(crate) fn settled(&self) -> bool {
        self.value == self.target
    }
    pub(crate) fn value(&self) -> f64 {
        self.value
    }
    #[inline]
    pub(crate) fn next(&mut self) -> f64 {
        if self.value != self.target {
            let gap = self.target - self.value;
            self.value = if gap.abs() <= 1e-6 * (1.0 + self.target.abs()) {
                self.target
            } else {
                self.value + gap * Self::STEP
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
        let (sin, cos) = Self::angle(hz);
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
    /// A shelf's `alpha` term and `A` for a slope and gain.
    fn shelf(sin: f64, slope: f64, db: f64) -> (f64, f64) {
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
    /// Coefficients given directly, for filters designed elsewhere.
    pub(crate) fn raw(b: [f64; 3], a: [f64; 3]) -> Self {
        Self::new(b, a)
    }
    #[cfg(test)]
    pub(crate) fn values(&self) -> [f64; 5] {
        [self.b0, self.b1, self.b2, self.a1, self.a2]
    }
    /// The magnitude response at a frequency.
    #[cfg(test)]
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
    #[inline]
    pub(crate) fn process(&mut self, channel: usize, input: f64) -> f64 {
        let c = &self.coefficients;
        let [s1, s2] = &mut self.state[channel];
        let output = c.b0 * input + *s1;
        *s1 = flush(c.b1 * input - c.a1 * output + *s2);
        *s2 = flush(c.b2 * input - c.a2 * output);
        output
    }
}

// Oversampling -------------------------------------------------------------------

/// The taps at odd offsets from the centre of a Kaiser-windowed halfband
/// low-pass with `2 TAPS - 1` taps. Its other taps are zero except the
/// centre, which is one half. The taps sum to one half, so DC passes exactly.
fn halfband<const TAPS: usize>(beta: f64) -> [f32; TAPS] {
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
static OUTER: LazyLock<[f32; 64]> = LazyLock::new(|| halfband(9.0));
/// The inner stage, between twice and four times the mix rate: 23 taps.
static INNER: LazyLock<[f32; 12]> = LazyLock::new(|| halfband(9.0));

/// One channel of one halfband stage, up and down. `TAPS - 1` is odd, so
/// every tap that the polyphase sums use is nonzero.
#[derive(Clone, Copy)]
struct Halfband<const TAPS: usize> {
    input: [f32; TAPS],
    even: [f32; TAPS],
    odd: [f32; TAPS],
}
impl<const TAPS: usize> Halfband<TAPS> {
    const ZERO: Self = Self {
        input: [0.0; TAPS],
        even: [0.0; TAPS],
        odd: [0.0; TAPS],
    };

    /// Two output samples for one input sample.
    #[inline]
    fn up(&mut self, taps: &[f32; TAPS], sample: f32) -> [f32; 2] {
        self.input.copy_within(..TAPS - 1, 1);
        self.input[0] = sample;
        [2.0 * dot(taps, &self.input), self.input[TAPS / 2 - 1]]
    }
    /// One output sample for two input samples.
    #[inline]
    fn down(&mut self, taps: &[f32; TAPS], [even, odd]: [f32; 2]) -> f32 {
        self.even.copy_within(..TAPS - 1, 1);
        self.even[0] = even;
        self.odd.copy_within(..TAPS - 1, 1);
        self.odd[0] = odd;
        dot(taps, &self.even) + 0.5 * self.odd[TAPS / 2]
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
    pub(crate) const ZERO: Self = Self {
        outer: Halfband::ZERO,
        inner: Halfband::ZERO,
        held: 0.0,
    };

    #[inline]
    pub(crate) fn process(&mut self, sample: f32, mut curve: impl FnMut(f32) -> f32) -> f32 {
        let (outer, inner) = (&*OUTER, &*INNER);
        let [a, b] = self.outer.up(outer, sample);
        let [p0, p1] = self.inner.up(inner, a);
        let [p2, p3] = self.inner.up(inner, b);
        let c = self.inner.down(inner, [curve(p0), curve(p1)]);
        let d = self.inner.down(inner, [curve(p2), curve(p3)]);
        let pair = [self.held, c];
        self.held = d;
        self.outer.down(outer, pair)
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
    history: [f32; 32],
}
impl TruePeak {
    /// The interval that `push` measures starts this many frames before
    /// the sample it takes.
    pub(crate) const DELAY: usize = 16;
    pub(crate) const ZERO: Self = Self { history: [0.0; 32] };

    /// Take the next sample and return the peak of the interval from the
    /// sample `DELAY` frames ago to the next one, both samples included.
    #[inline]
    pub(crate) fn push(&mut self, sample: f32) -> f32 {
        self.history.copy_within(..31, 1);
        self.history[0] = sample;
        PHASES
            .iter()
            .map(|taps| dot(taps, &self.history).abs())
            .fold(self.history[15].abs().max(self.history[16].abs()), f32::max)
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
mod tests {
    use super::*;

    fn sine(hz: f64, amplitude: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| (amplitude * (2.0 * PI * hz * i as f64 / RATE).sin()) as f32)
            .collect()
    }

    fn rms(samples: &[f32]) -> f64 {
        (samples.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
    }

    #[test]
    fn oversampling_delays_audible_sound_by_whole_frames_without_coloring_it() {
        for hz in [40.0, 1000.0, 10_000.0, 19_000.0] {
            let input = sine(hz, 0.8, 8000);
            let mut oversampler = Oversampler::ZERO;
            let output: Vec<f32> = input
                .iter()
                .map(|v| oversampler.process(*v, |x| x))
                .collect();
            let tail = &output[2000..];
            let error: Vec<f32> = tail
                .iter()
                .zip(&input[2000 - Oversampler::LATENCY..])
                .map(|(out, inp)| out - inp)
                .collect();
            let level = gain_to_db(rms(&error) / rms(tail));
            assert!(level < -80.0, "{hz} Hz differs by {level:.1} dB");
        }
    }

    #[test]
    fn oversampling_keeps_curve_harmonics_from_folding_into_the_band() {
        // A hard clip of 15 kHz makes 45 kHz and 75 kHz, which would fold
        // to 0.9 kHz and 13.2 kHz at the mix rate. The analysis spans whole
        // cycles of every tone involved, so none leaks into another's bin.
        let input = sine(15_000.0, 1.0, 4096 + 11_760);
        let mut oversampler = Oversampler::ZERO;
        let output: Vec<f64> = input
            .iter()
            .map(|v| f64::from(oversampler.process(*v, |x| x.clamp(-0.5, 0.5))))
            .collect();
        let tail = &output[4096..];
        let amplitude = |hz: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, v) in tail.iter().enumerate() {
                let angle = 2.0 * PI * hz * i as f64 / RATE;
                re += v * angle.cos();
                im += v * angle.sin();
            }
            2.0 * re.hypot(im) / tail.len() as f64
        };
        let fundamental = amplitude(15_000.0);
        for alias in [900.0, 13_200.0] {
            let level = gain_to_db(amplitude(alias) / fundamental);
            assert!(level < -70.0, "Alias at {alias} Hz is {level:.1} dB");
        }
    }

    #[test]
    fn filters_follow_the_cookbook_shapes() {
        let close = |a: f64, b: f64| (a - b).abs() < 0.01;
        let peak = Coefficients::peaking(250.0, 2.5, -3.0);
        assert!(close(gain_to_db(peak.magnitude(250.0)), -3.0));
        assert!(close(gain_to_db(peak.magnitude(5000.0)), 0.0));
        let high = Coefficients::high_pass(20.0, std::f64::consts::FRAC_1_SQRT_2);
        assert!(close(gain_to_db(high.magnitude(20.0)), -3.01));
        let shelf = Coefficients::high_shelf(12_000.0, 0.6, 2.0);
        assert!(close(gain_to_db(shelf.magnitude(20.0)), 0.0));
        assert!(gain_to_db(shelf.magnitude(20_000.0)) > 1.5);
        // A shelf and its opposite cancel.
        let back = Coefficients::high_shelf(12_000.0, 0.6, -2.0);
        for hz in [100.0, 5000.0, 15_000.0] {
            assert!((shelf.magnitude(hz) * back.magnitude(hz) - 1.0).abs() < 1e-9);
        }
        // Linkwitz-Riley low and high halves sum to an all-pass.
        let q = std::f64::consts::FRAC_1_SQRT_2;
        let (mut low, mut high) = (
            [Biquad::new(Coefficients::low_pass(500.0, q)); 2],
            [Biquad::new(Coefficients::high_pass(500.0, q)); 2],
        );
        let mut all = Biquad::new(Coefficients::all_pass(500.0, q));
        for i in 0..4000 {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let (a, b) = (low[0].process(0, x), high[0].process(0, x));
            let sum = low[1].process(0, a) + high[1].process(0, b);
            assert!((sum - all.process(0, x)).abs() < 1e-12);
        }
    }

    #[test]
    fn true_peaks_see_between_the_samples() {
        // A quarter-rate sine sampled at 45 degrees peaks between samples, 3 dB
        // above them.
        let input: Vec<f32> = (0..400)
            .map(|i| (0.5 * (PI / 2.0 * i as f64 + PI / 4.0).sin()) as f32)
            .collect();
        let sample_peak = input.iter().fold(0.0_f32, |a, v| a.max(v.abs()));
        let mut meter = TruePeak::ZERO;
        // Skip the tone's abrupt start, which rings like any step.
        let true_peak = input
            .iter()
            .map(|v| meter.push(*v))
            .skip(2 * TruePeak::DELAY)
            .fold(0.0_f32, f32::max);
        let lift = gain_to_db(f64::from(true_peak / sample_peak));
        assert!((lift - 3.01).abs() < 0.02, "{lift}");
    }
}
