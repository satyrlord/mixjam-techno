//! Parts that the delay and the reverb share. Delay lines live on the heap
//! and are sized when an effect is built, so processing never allocates.
use crate::master::dsp::{RATE, db_to_gain, flush, gain_to_db, pole};
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
    /// A line that reaches at least `frames` back.
    pub(super) fn new(frames: usize) -> Self {
        let size = (frames + 4).next_power_of_two();
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
}

/// Lowers the wet sound while the dry input is loud. A stereo-linked peak
/// follower with a 7 ms attack; the gain falls by up to 24 dB along a soft
/// knee between -42 and -12 dBFS.
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
            attack: 1.0 - pole(0.007),
            release: 0.0,
        }
    }
    pub(super) fn clear(&mut self) {
        self.envelope = 0.0;
    }
    pub(super) fn set_release(&mut self, ms: f64) {
        self.release = 1.0 - pole(ms / 1000.0);
    }
    /// The wet gain for this input frame at `amount` from 0 to 1.
    #[inline]
    pub(super) fn gain(&mut self, left: f64, right: f64, amount: f64) -> f64 {
        let level = left.abs().max(right.abs());
        let rate = if level > self.envelope {
            self.attack
        } else {
            self.release
        };
        self.envelope = flush(self.envelope + (level - self.envelope) * rate);
        if amount <= 0.0 || self.envelope < 1e-5 {
            return 1.0;
        }
        let db = gain_to_db(self.envelope);
        let x = ((db + 42.0) / 30.0).clamp(0.0, 1.0);
        let knee = x * x * (3.0 - 2.0 * x);
        db_to_gain(-24.0 * amount * knee)
    }
}

/// Saturation before an effect: `tanh(x g) / g` with `g = 1 + 8 drive`,
/// blended with the clean input by the drive, so 0 is an exact bypass.
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

/// A setting that glides to its target with its own time constant.
#[derive(Clone, Copy)]
pub(super) struct Glide {
    value: f64,
    target: f64,
    step: f64,
}
impl Glide {
    pub(super) fn new(value: f64, seconds: f64) -> Self {
        Self {
            value,
            target: value,
            step: 1.0 - pole(seconds),
        }
    }
    pub(super) fn set(&mut self, target: f64) {
        self.target = target;
    }
    pub(super) fn set_time(&mut self, seconds: f64) {
        self.step = 1.0 - pole(seconds);
    }
    pub(super) fn settle(&mut self) {
        self.value = self.target;
    }
    pub(super) fn value(&self) -> f64 {
        self.value
    }
    pub(super) fn target(&self) -> f64 {
        self.target
    }
    #[inline]
    pub(super) fn next(&mut self) -> f64 {
        if self.value != self.target {
            let gap = self.target - self.value;
            self.value = if gap.abs() <= 1e-9 * (1.0 + self.target.abs()) {
                self.target
            } else {
                self.value + gap * self.step
            };
        }
        self.value
    }
}

/// The smallest of the last `span` values pushed: a queue of the values
/// that can still become the smallest.
struct MovingMin {
    values: Box<[f64]>,
    times: Box<[u32]>,
    head: usize,
    len: usize,
    now: u32,
}
impl MovingMin {
    fn new(span: usize) -> Self {
        Self {
            values: vec![0.0; span].into_boxed_slice(),
            times: vec![0; span].into_boxed_slice(),
            head: 0,
            len: 0,
            now: 0,
        }
    }
    fn push(&mut self, value: f64) -> f64 {
        let n = self.values.len();
        if self.len > 0 && self.now.wrapping_sub(self.times[self.head]) as usize >= n {
            self.head = (self.head + 1) % n;
            self.len -= 1;
        }
        while self.len > 0 && self.values[(self.head + self.len - 1) % n] >= value {
            self.len -= 1;
        }
        let at = (self.head + self.len) % n;
        (self.values[at], self.times[at]) = (value, self.now);
        self.len += 1;
        self.now = self.now.wrapping_add(1);
        self.values[self.head]
    }
}

/// A return's safety limiter: stereo-linked, -1 dBFS ceiling, `LOOKAHEAD`
/// frames of lookahead, and a 100 ms release. It always delays the return
/// by its lookahead, on or off, so switching it never moves the sound; the
/// effects shorten their own delays by as much.
pub(super) struct SafetyLimiter {
    delays: [Line; 2],
    needs: MovingMin,
    held: f64,
    ramp: Box<[f64]>,
    ramp_at: usize,
    ramp_sum: f64,
    release: f64,
}
impl SafetyLimiter {
    const CEILING: f64 = 0.891_250_938_133_745_5; // -1 dBFS

    pub(super) fn new() -> Self {
        let lookahead = super::LOOKAHEAD;
        Self {
            delays: [Line::new(lookahead), Line::new(lookahead)],
            needs: MovingMin::new(lookahead + 1),
            held: 1.0,
            ramp: vec![1.0; lookahead].into_boxed_slice(),
            ramp_at: 0,
            ramp_sum: lookahead as f64,
            release: pole(0.1),
        }
    }
    pub(super) fn clear(&mut self) {
        self.delays.iter_mut().for_each(Line::clear);
        self.needs.head = 0;
        self.needs.len = 0;
        self.needs.now = 0;
        self.held = 1.0;
        self.ramp.fill(1.0);
        self.ramp_at = 0;
        self.ramp_sum = self.ramp.len() as f64;
    }
    /// The frame `LOOKAHEAD` ago, limited when `on`.
    #[inline]
    pub(super) fn process(&mut self, left: f64, right: f64, on: bool) -> (f64, f64) {
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
        let lowest = self.needs.push(need);
        self.held = if lowest < self.held {
            lowest
        } else {
            flush(lowest + (self.held - lowest) * self.release)
        };
        self.ramp_sum += self.held - self.ramp[self.ramp_at];
        self.ramp[self.ramp_at] = self.held;
        self.ramp_at += 1;
        if self.ramp_at == lookahead {
            // Recount now and then, so rounding cannot build up.
            self.ramp_at = 0;
            self.ramp_sum = self.ramp.iter().sum();
        }
        let gain = (self.ramp_sum / lookahead as f64).min(1.0);
        (delayed.0 * gain, delayed.1 * gain)
    }
}

/// A Schroeder all-pass of fixed length: it smears a sound in time
/// without colouring its spectrum.
pub(super) struct AllPass {
    line: Line,
    length: usize,
}
impl AllPass {
    pub(super) fn new(seconds: f64) -> Self {
        let length = (seconds * RATE).round().max(1.0) as usize;
        Self {
            line: Line::new(length),
            length,
        }
    }
    pub(super) fn clear(&mut self) {
        self.line.clear();
    }
    #[inline]
    pub(super) fn process(&mut self, input: f64, gain: f64) -> f64 {
        let delayed = f64::from(self.line.at(self.length));
        let v = input + gain * delayed;
        self.line.push(flush(v) as f32);
        delayed - gain * v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_min_keeps_the_oldest_live_value_when_full() {
        let mut minimum = MovingMin::new(4);
        for index in 0usize..20 {
            assert_eq!(minimum.push(index as f64), index.saturating_sub(3) as f64);
        }
    }

    #[test]
    fn clearing_the_limiter_reuses_its_buffers() {
        let mut limiter = SafetyLimiter::new();
        let pointers = (
            limiter.delays[0].buffer.as_ptr(),
            limiter.needs.values.as_ptr(),
            limiter.ramp.as_ptr(),
        );
        limiter.process(1.0, 1.0, true);
        limiter.clear();
        assert_eq!(
            pointers,
            (
                limiter.delays[0].buffer.as_ptr(),
                limiter.needs.values.as_ptr(),
                limiter.ramp.as_ptr()
            )
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
}
