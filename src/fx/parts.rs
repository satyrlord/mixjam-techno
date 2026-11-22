//! Parts that the delay and the reverb share. Delay lines live on the heap
//! and are sized when an effect is built, so processing never allocates.
use super::RenderQuality;
use crate::master::dsp::{LookaheadGain, RATE, TruePeak, db_to_gain, flush, gain_to_db, pole};
use std::f64::consts::{PI, TAU};

/// A delay line of one channel, read at fractional positions with cubic
/// Lagrange interpolation.
pub(super) struct Line {
    buffer: Box<[f32]>,
    mask: usize,
    /// Where the next sample goes.
    write: usize,
}
impl Line {
    /// A line that reaches at least `frames` back, with 16-tap sinc margin.
    pub(super) fn new(frames: usize) -> Self {
        let size = (frames + 16).next_power_of_two();
        Self {
            buffer: vec![0.0; size].into_boxed_slice(),
            mask: size - 1,
            write: 0,
        }
    }
    pub(super) fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
    /// The longest delay that `read` reaches.
    pub(super) fn reach(&self) -> f64 {
        (self.buffer.len() - 3) as f64
    }
    #[inline]
    pub(super) fn push(&mut self, sample: f32) {
        self.buffer[self.write] = sample;
        self.write = (self.write + 1) & self.mask;
    }
    /// The sample pushed `delay` pushes ago; 1 is the last one.
    #[inline]
    pub(super) fn at(&self, delay: usize) -> f32 {
        self.buffer[self.write.wrapping_sub(delay) & self.mask]
    }
    /// The signal `delay` frames back, between pushed samples. The delay is
    /// held between 2 frames and the line's reach.
    #[inline]
    pub(super) fn read(&self, delay: f64) -> f64 {
        let delay = delay.clamp(2.0, self.reach());
        let whole = delay.floor();
        let f = delay - whole;
        let n = whole as usize;
        let y = |k: isize| f64::from(self.at((n as isize + k) as usize));
        let (a, b, c, d) = (y(-1), y(0), y(1), y(2));
        let c0 = -f * (f - 1.0) * (f - 2.0) / 6.0;
        let c1 = (f + 1.0) * (f - 1.0) * (f - 2.0) / 2.0;
        let c2 = -(f + 1.0) * f * (f - 2.0) / 2.0;
        let c3 = (f + 1.0) * f * (f - 1.0) / 6.0;
        c0 * a + c1 * b + c2 * c + c3 * d
    }
    /// 16-tap Blackman-windowed sinc interpolation for transparent offline high-frequency reads.
    #[inline]
    pub(super) fn read_sinc(&self, delay: f64) -> f64 {
        if delay < 8.0 || delay > (self.buffer.len() - 9) as f64 {
            return self.read(delay);
        }
        let whole = delay.floor();
        let f = delay - whole;
        let n = whole as usize;
        if f < 1e-9 {
            return f64::from(self.at(n));
        }
        let mut sum = 0.0;
        let mut norm = 0.0;
        for k in -7_isize..=8 {
            let x = f - k as f64;
            let sinc = if x.abs() < 1e-9 {
                1.0
            } else {
                (PI * x).sin() / (PI * x)
            };
            let u = x / 8.0;
            let w = 0.42 + 0.5 * (PI * u).cos() + 0.08 * (2.0 * PI * u).cos();
            let tap = w * sinc;
            sum += tap * f64::from(self.at((n as isize + k) as usize));
            norm += tap;
        }
        sum / norm
    }
}

/// Zero-delay-feedback TPT state-variable filter with soft peak saturation at high resonance.
#[derive(Clone, Copy, Default)]
pub(super) struct Svf {
    g: f64,
    k: f64,
    ic1: f64,
    ic2: f64,
}
impl Svf {
    pub(super) fn tune(&mut self, hz: f64, q: f64) {
        self.g = (PI * hz.clamp(10.0, 0.49 * RATE) / RATE).tan();
        self.k = 1.0 / q.clamp(0.5, 6.0);
    }
    pub(super) fn clear(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }
    #[inline]
    fn step(&mut self, x: f64) -> (f64, f64, f64) {
        let a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        let a2 = self.g * a1;
        let a3 = self.g * a2;
        let v3 = x - self.ic2;
        let raw_v1 = a1 * self.ic1 + a2 * v3;
        let v1 = if self.k < 1.2 {
            2.0 * (0.5 * raw_v1).tanh()
        } else {
            raw_v1
        };
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = flush(2.0 * v1 - self.ic1);
        self.ic2 = flush(2.0 * v2 - self.ic2);
        let high = x - self.k * v1 - v2;
        (v2, v1, high)
    }
    #[inline]
    pub(super) fn low(&mut self, x: f64) -> f64 {
        self.step(x).0
    }
    #[inline]
    pub(super) fn high(&mut self, x: f64) -> f64 {
        self.step(x).2
    }
}

/// A one-pole filter in topology-preserving form: stable at any corner and
/// under fast changes.
#[derive(Clone, Copy, Default)]
pub(super) struct OnePole {
    gain: f64,
    state: f64,
}
impl OnePole {
    pub(super) fn tune(&mut self, hz: f64) {
        let warped = (PI * hz.clamp(1.0, 0.49 * RATE) / RATE).tan();
        self.gain = warped / (1.0 + warped);
    }
    pub(super) fn clear(&mut self) {
        self.state = 0.0;
    }
    #[inline]
    pub(super) fn low(&mut self, input: f64) -> f64 {
        let v = (input - self.state) * self.gain;
        let output = v + self.state;
        self.state = flush(output + v);
        output
    }
    #[inline]
    pub(super) fn high(&mut self, input: f64) -> f64 {
        input - self.low(input)
    }
}

/// A high-pass and a low-pass of two poles each: 12 dB per octave on both
/// sides of the band.
#[derive(Clone, Copy, Default)]
pub(super) struct Tone {
    high: [OnePole; 2],
    low: [OnePole; 2],
}
impl Tone {
    pub(super) fn tune(&mut self, low_cut: f64, high_cut: f64) {
        self.high.iter_mut().for_each(|pole| pole.tune(low_cut));
        self.low.iter_mut().for_each(|pole| pole.tune(high_cut));
    }
    pub(super) fn clear(&mut self) {
        self.high
            .iter_mut()
            .chain(&mut self.low)
            .for_each(OnePole::clear);
    }
    #[inline]
    pub(super) fn process(&mut self, input: f64) -> f64 {
        let x = self.high[0].high(input);
        let x = self.high[1].high(x);
        let x = self.low[0].low(x);
        self.low[1].low(x)
    }
}

/// A sine oscillator with a phase from 0 to 1.
#[derive(Clone, Copy, Default)]
pub(super) struct Lfo {
    phase: f64,
}
impl Lfo {
    /// The sine at `offset` turns from the current phase.
    #[inline]
    pub(super) fn value(&self, offset: f64) -> f64 {
        (TAU * (self.phase + offset)).sin()
    }
    #[inline]
    pub(super) fn advance(&mut self, hz: f64) {
        self.phase = (self.phase + hz / RATE).fract();
    }
    /// Advance by `frames` frames at once.
    pub(super) fn skip(&mut self, hz: f64, frames: u64) {
        self.phase = (self.phase + hz * frames as f64 / RATE).fract();
    }
}

/// Lowers the wet sound while the dry or key input is loud. A stereo-linked
/// peak follower with a 1 ms attack; the gain falls by up to 24 dB along a
/// 30 dB soft knee centered on `threshold_db`.
#[derive(Clone, Copy)]
pub(super) struct Ducker {
    envelope: f64,
    attack: f64,
    release: f64,
}
impl Ducker {
    pub(super) fn new() -> Self {
        Self {
            envelope: 0.0,
            attack: 1.0 - pole(0.001),
            release: 0.0,
        }
    }
    pub(super) fn clear(&mut self) {
        self.envelope = 0.0;
    }
    pub(super) fn set_release(&mut self, ms: f64) {
        self.release = 1.0 - pole(ms / 1000.0);
    }
    /// The wet gain and reduction in dB at `amount` (0..1) and `threshold_db` (-48..0).
    #[inline]
    pub(super) fn gain_at(
        &mut self,
        left: f64,
        right: f64,
        amount: f64,
        threshold_db: f64,
    ) -> (f64, f64) {
        let level = left.abs().max(right.abs());
        let rate = if level > self.envelope {
            self.attack
        } else {
            self.release
        };
        self.envelope = flush(self.envelope + (level - self.envelope) * rate);
        if amount <= 0.0 || self.envelope < 1e-5 {
            return (1.0, 0.0);
        }
        let db = gain_to_db(self.envelope);
        let x = ((db - (threshold_db - 15.0)) / 30.0).clamp(0.0, 1.0);
        let knee = x * x * (3.0 - 2.0 * x);
        let gr_db = 24.0 * amount * knee;
        (db_to_gain(-gr_db), gr_db)
    }
    #[cfg(test)]
    #[inline]
    pub(super) fn gain(&mut self, left: f64, right: f64, amount: f64) -> f64 {
        self.gain_at(left, right, amount, -27.0).0
    }
}

/// Saturation before an effect: `tanh(x g) / g * (1 + 0.5 drive)` with
/// `g = 1 + 8 drive`, blended with the clean input by the drive,
/// so 0 is an exact bypass.
#[inline]
pub(super) fn drive(input: f64, amount: f64) -> f64 {
    if amount <= 0.0 {
        return input;
    }
    let g = 1.0 + 8.0 * amount;
    let shaped = (input * g).tanh() / g * (1.0 + 0.5 * amount);
    input + (shaped - input) * amount
}

/// Keeps a feedback loop finite: unchanged up to 0.9, then bending toward 1.5.
#[inline]
pub(super) fn bound(sample: f64) -> f64 {
    let size = sample.abs();
    if size <= 0.9 {
        sample
    } else {
        sample.signum() * (0.9 + 0.6 * ((size - 0.9) / 0.6).tanh())
    }
}

/// Mid/side width: 0 is mono, 1 unchanged, 2 doubles the side.
#[inline]
pub(super) fn widen(left: f64, right: f64, width: f64) -> (f64, f64) {
    let mid = 0.5 * (left + right);
    let side = 0.5 * (left - right) * width;
    (mid + side, mid - side)
}

/// The frames whose gain needs the safety limiter compares: its lookahead
/// and the frame now arriving.
const NEEDS: usize = super::LOOKAHEAD + 1;
const TP_SPREAD: usize = TruePeak::DELAY;
const OFFLINE_NEEDS: usize = super::LOOKAHEAD + 1 + 2 * TP_SPREAD;

/// The safety limiter's ceiling, in dBFS.
pub fn return_limiter_ceiling_dbfs() -> f64 {
    gain_to_db(SafetyLimiter::CEILING)
}

/// A return's safety limiter: stereo-linked, -1 dBFS live / -1.0 dBTP offline
/// ceiling, lookahead detection, and a 100 ms release.
pub(super) struct SafetyLimiter {
    delays: [Line; 2],
    gain: LookaheadGain<f64, { super::LOOKAHEAD }, NEEDS>,
    offline_delays: [Line; 2],
    offline_gain: LookaheadGain<f64, { super::LOOKAHEAD }, OFFLINE_NEEDS>,
    detectors: [TruePeak; 2],
}
impl SafetyLimiter {
    const CEILING: f64 = 0.891_250_938_133_745_5; // -1 dBFS
    const TRUE_PEAK_CEILING: f64 = 0.889_201_055_065_978_5; // -1.02 dBFS margin for <= -1.0 dBTP
    pub(super) const OFFLINE_LATENCY: usize = super::LOOKAHEAD + 2 * TP_SPREAD;

    pub(super) fn new() -> Self {
        let lookahead = super::LOOKAHEAD;
        Self {
            delays: [Line::new(lookahead), Line::new(lookahead)],
            gain: LookaheadGain::new(NEEDS, pole(0.1)),
            offline_delays: [
                Line::new(Self::OFFLINE_LATENCY),
                Line::new(Self::OFFLINE_LATENCY),
            ],
            offline_gain: LookaheadGain::new(OFFLINE_NEEDS, pole(0.1)),
            detectors: [TruePeak::ZERO; 2],
        }
    }
    pub(super) fn clear(&mut self) {
        self.delays.iter_mut().for_each(Line::clear);
        self.gain.clear();
        self.offline_delays.iter_mut().for_each(Line::clear);
        self.offline_gain.clear();
        self.detectors = [TruePeak::ZERO; 2];
    }
    /// Run the lookahead limiter for `quality` and return `(left, right, gr_db)`.
    #[inline]
    pub(super) fn process_quality(
        &mut self,
        left: f64,
        right: f64,
        on: bool,
        quality: RenderQuality,
    ) -> (f64, f64, f64) {
        match quality {
            RenderQuality::Live => {
                let lookahead = super::LOOKAHEAD;
                self.delays[0].push(left as f32);
                self.delays[1].push(right as f32);
                let delayed = (
                    f64::from(self.delays[0].at(lookahead + 1)),
                    f64::from(self.delays[1].at(lookahead + 1)),
                );
                let level = left.abs().max(right.abs());
                let need = if on && level > Self::CEILING {
                    Self::CEILING / level
                } else {
                    1.0
                };
                let gain = self.gain.push(need);
                let gr_db = if gain < 1.0 { -gain_to_db(gain) } else { 0.0 };
                (delayed.0 * gain, delayed.1 * gain, gr_db)
            }
            RenderQuality::Offline => {
                let latency = Self::OFFLINE_LATENCY;
                self.offline_delays[0].push(left as f32);
                self.offline_delays[1].push(right as f32);
                let delayed = (
                    f64::from(self.offline_delays[0].at(latency + 1)),
                    f64::from(self.offline_delays[1].at(latency + 1)),
                );
                let tp_l = f64::from(self.detectors[0].push(left as f32));
                let tp_r = f64::from(self.detectors[1].push(right as f32));
                let peak = tp_l.max(tp_r);
                let need = if on && peak > Self::TRUE_PEAK_CEILING {
                    Self::TRUE_PEAK_CEILING / peak
                } else {
                    1.0
                };
                let gain = self.offline_gain.push(need);
                let (out_l, out_r) = if on {
                    (
                        (delayed.0 * gain).clamp(-Self::CEILING, Self::CEILING),
                        (delayed.1 * gain).clamp(-Self::CEILING, Self::CEILING),
                    )
                } else {
                    delayed
                };
                let gr_db = if gain < 1.0 { -gain_to_db(gain) } else { 0.0 };
                (out_l, out_r, gr_db)
            }
        }
    }
    /// The frame `LOOKAHEAD` ago in live mode, limited when `on`.
    #[cfg(test)]
    #[inline]
    pub(super) fn process(&mut self, left: f64, right: f64, on: bool) -> (f64, f64) {
        let (l, r, _) = self.process_quality(left, right, on, RenderQuality::Live);
        (l, r)
    }
}

/// A drive return's safety ceiling, which adds no latency: stereo-linked,
/// unchanged up to -3.5 dBFS, then bending toward -1 dBFS and never past it.
/// Off, it passes the frame unchanged. Returns `(left, right, gr_db)`.
#[inline]
pub(super) fn ceiling_with_gr(left: f64, right: f64, on: bool) -> (f64, f64, f64) {
    const KNEE: f64 = 0.668_343_917_568_740_6; // -3.5 dBFS
    let peak = left.abs().max(right.abs());
    if !on || peak <= KNEE {
        return (left, right, 0.0);
    }
    // A hair under the ceiling, so rounding cannot pass it.
    let room = SafetyLimiter::CEILING * (1.0 - 1e-9) - KNEE;
    let gain = (KNEE + room * ((peak - KNEE) / room).tanh()) / peak;
    let gr_db = if gain < 1.0 { -gain_to_db(gain) } else { 0.0 };
    (left * gain, right * gain, gr_db)
}

#[cfg(test)]
#[inline]
pub(super) fn ceiling(left: f64, right: f64, on: bool) -> (f64, f64) {
    let (l, r, _) = ceiling_with_gr(left, right, on);
    (l, r)
}

/// A Schroeder all-pass of fixed length: it smears a sound in time
/// without colouring its spectrum.
pub(super) struct AllPass {
    line: Line,
    length: usize,
}
impl AllPass {
    pub(super) fn new(seconds: f64) -> Self {
        Self::with_modulation(seconds, 0)
    }
    pub(super) fn with_modulation(seconds: f64, extra_frames: usize) -> Self {
        let length = (seconds * RATE).round().max(1.0) as usize;
        Self {
            line: Line::new(length + extra_frames),
            length,
        }
    }
    pub(super) fn clear(&mut self) {
        self.line.clear();
    }
    #[inline]
    pub(super) fn process(&mut self, input: f64, gain: f64) -> f64 {
        let delayed = f64::from(self.line.at(self.length));
        self.filter(input, gain, delayed)
    }
    #[inline]
    pub(super) fn process_modulated(&mut self, input: f64, gain: f64, offset: f64) -> f64 {
        let delayed = self.line.read(self.length as f64 + offset);
        self.filter(input, gain, delayed)
    }
    #[inline]
    fn filter(&mut self, input: f64, gain: f64, delayed: f64) -> f64 {
        let v = input + gain * delayed;
        self.line.push(flush(v) as f32);
        delayed - gain * v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_the_limiter_reuses_its_buffers() {
        let mut limiter = SafetyLimiter::new();
        let pointers = limiter.delays.each_ref().map(|line| line.buffer.as_ptr());
        limiter.process(1.0, 1.0, true);
        limiter.clear();
        assert_eq!(
            pointers,
            limiter.delays.each_ref().map(|line| line.buffer.as_ptr())
        );
        for _ in 0..1000 {
            assert_eq!(limiter.process(0.0, 0.0, true), (0.0, 0.0));
        }
    }

    #[test]
    fn a_line_reads_whole_and_fractional_delays() {
        let mut line = Line::new(64);
        for i in 0..64 {
            line.push(i as f32);
        }
        // A ramp interpolates exactly: 63 was pushed last.
        assert_eq!(line.at(1), 63.0);
        assert!((line.read(10.0) - 54.0).abs() < 1e-9);
        assert!((line.read(10.25) - 53.75).abs() < 1e-9);
    }

    #[test]
    fn the_safety_limiter_holds_the_ceiling_and_keeps_its_delay() {
        let mut limiter = SafetyLimiter::new();
        let mut peak: f64 = 0.0;
        let mut first = None;
        for i in 0..20_000 {
            let x = if i >= 1000 {
                1.8 * (i as f64 * 0.05).sin()
            } else {
                0.0
            };
            let (l, r) = limiter.process(x, -x, true);
            peak = peak.max(l.abs()).max(r.abs());
            if first.is_none() && l.abs() > 1e-9 {
                first = Some(i);
            }
        }
        // The delay line holds 32-bit samples.
        assert!(peak <= SafetyLimiter::CEILING * (1.0 + 1e-6), "{peak}");
        // The first sound, at frame 1000, comes out LOOKAHEAD frames later.
        assert_eq!(first, Some(1000 + super::super::LOOKAHEAD));
        let mut off = SafetyLimiter::new();
        let mut loud: f64 = 0.0;
        for i in 0..2000 {
            loud = loud.max(
                off.process(1.5 * (i as f64 * 0.05).sin(), 0.0, false)
                    .0
                    .abs(),
            );
        }
        assert!(loud > 1.4);
    }

    #[test]
    fn the_ceiling_holds_minus_one_dbfs_without_delay() {
        assert_eq!(ceiling(0.5, -0.6, true), (0.5, -0.6));
        assert_eq!(ceiling(3.0, -2.0, false), (3.0, -2.0));
        for step in 0..400 {
            let x = 0.6 + f64::from(step) * 0.05;
            let (l, r) = ceiling(x, -0.5 * x, true);
            assert!(l <= SafetyLimiter::CEILING && l > 0.59, "{x}: {l}");
            assert!((r / l + 0.5).abs() < 1e-12, "the image stays put");
        }
    }

    #[test]
    fn drive_at_zero_is_exact_and_ducking_follows_the_input() {
        assert_eq!(drive(0.37, 0.0), 0.37);
        assert!(drive(0.9, 1.0) < 0.9);
        let mut ducker = Ducker::new();
        ducker.set_release(100.0);
        let mut gain = 1.0;
        for _ in 0..2000 {
            gain = ducker.gain(0.5, 0.5, 1.0);
        }
        assert!(gain < 10f64.powf(-20.0 / 20.0), "{gain}");
        for _ in 0..44_100 {
            gain = ducker.gain(0.0, 0.0, 1.0);
        }
        assert!(gain > 0.99);
    }

    #[test]
    fn read_sinc_preserves_high_frequencies_at_half_sample_delay() {
        let mut line = Line::new(64);
        let freq = 12_000.0;
        for i in 0..48 {
            line.push((TAU * freq * i as f64 / RATE).sin() as f32);
        }
        let exact = line.read_sinc(20.0);
        assert!((exact - f64::from(line.at(20))).abs() < 1e-6);
        // Measure steady-state amplitude across 236 samples at half-sample delay.
        let mut line = Line::new(512);
        let mut energy = 0.0;
        for i in 0..300 {
            line.push((TAU * freq * i as f64 / RATE).sin() as f32);
            if i >= 64 {
                energy += line.read_sinc(20.5).powi(2);
            }
        }
        let rms = (energy / 236.0).sqrt();
        let db_err = 20.0 * (rms / std::f64::consts::FRAC_1_SQRT_2).log10();
        assert!(db_err.abs() < 0.15, "12 kHz sinc droop {db_err:.3} dB");
    }

    #[test]
    fn svf_low_and_high_boost_at_resonance_and_stay_bounded() {
        let mut lp = Svf::default();
        lp.tune(2000.0, 4.0);
        let mut hp = Svf::default();
        hp.tune(2000.0, 4.0);
        let measure = |f: &mut Svf, low: bool, hz: f64| -> f64 {
            f.clear();
            let mut sum = 0.0;
            for i in 0..4410 {
                let x = 0.1 * (TAU * hz * i as f64 / RATE).sin();
                let y = if low { f.low(x) } else { f.high(x) };
                if i >= 2205 {
                    sum += y * y;
                }
            }
            (sum / 2205.0).sqrt()
        };
        assert!(measure(&mut lp, true, 2000.0) > 2.5 * measure(&mut lp, true, 200.0));
        assert!(measure(&mut hp, false, 2000.0) > 2.5 * measure(&mut hp, false, 12_000.0));
        assert!(measure(&mut lp, true, 12_000.0) < 0.1 * measure(&mut lp, true, 200.0));
        assert!(measure(&mut hp, false, 200.0) < 0.1 * measure(&mut hp, false, 12_000.0));

        lp.clear();
        for i in 0..4410 {
            let x = 10.0 * (TAU * 2000.0 * i as f64 / RATE).sin();
            let y = lp.low(x);
            assert!(y.is_finite() && y.abs() < 15.0, "{y}");
        }
    }
}
