//! The Aetherform Reverb's signal processing.
//!
//! The input passes the pre-delay. From there, model-specific early
//! reflections run beside a chain of input all-passes that blur the sound.
//! The blurred sound feeds an eight-line feedback delay network with a
//! Householder matrix. Each line has its tone filters, the character's
//! colouring, and an all-pass inside the loop, so damping builds with every
//! circulation. A pitch shifter can feed the tail back an interval higher:
//! the shimmer. The early and late sounds then blend at equal power, and
//! the output is widened, ducked, and trimmed.
//!
//! A line's gain is `10^(-3 t / decay)` for its length `t` in seconds, so
//! every line falls by 60 dB in the decay time whatever the size. Size,
//! model, and pre-delay changes crossfade between old and new read
//! positions, never glide, so the pitch never bends.
use super::parts::{AllPass, Ducker, Glide, Lfo, Line, OnePole, Tone, bound, drive, widen};
use super::{Aetherform, ReverbCharacter, SpaceModel};
use crate::master::dsp::{RATE, db_to_gain};
use std::f64::consts::{FRAC_PI_2, PI};

const LINES: usize = 8;
/// A retime crossfades over 50 ms.
const FADE: usize = 2205;
/// The pitch shifter's window: 60 ms.
const WINDOW: f64 = 2646.0;
/// The lengths of the all-passes inside the loop, one per line.
const INNER_MS: [f64; LINES] = [2.31, 3.07, 3.71, 4.39, 2.73, 3.41, 4.13, 4.87];

/// Line lengths in milliseconds at full size, by model.
fn base_lengths(model: SpaceModel) -> [f64; LINES] {
    match model {
        SpaceModel::Room => [19.1, 23.3, 27.7, 31.1, 36.7, 41.3, 47.9, 53.3],
        SpaceModel::Hall => [47.3, 55.9, 66.1, 76.7, 89.3, 103.1, 118.7, 134.9],
        SpaceModel::Plate => [9.7, 12.1, 14.3, 16.9, 19.3, 22.7, 25.1, 28.3],
        SpaceModel::Chamber => [29.9, 35.3, 41.9, 47.3, 55.1, 62.9, 71.3, 79.7],
    }
}

/// Early reflection times in milliseconds at full size, by model.
fn reflection_times(model: SpaceModel) -> [f64; 14] {
    match model {
        SpaceModel::Room => [
            2.1, 3.7, 5.3, 7.9, 9.4, 11.8, 13.1, 15.6, 17.9, 20.3, 22.8, 25.4, 28.1, 31.0,
        ],
        SpaceModel::Hall => [
            6.3, 9.8, 13.1, 17.6, 21.9, 26.4, 31.2, 36.7, 42.1, 48.3, 54.9, 61.7, 69.2, 77.5,
        ],
        SpaceModel::Plate => [
            0.7, 1.3, 2.1, 2.8, 3.6, 4.5, 5.3, 6.4, 7.2, 8.3, 9.5, 10.6, 11.9, 13.2,
        ],
        SpaceModel::Chamber => [
            3.9, 6.1, 8.7, 11.2, 13.8, 16.9, 19.7, 22.6, 25.9, 29.3, 32.8, 36.4, 40.3, 44.6,
        ],
    }
}

/// The share of full size: from 0.28 at 5% to 1 at 100%, faster near the top.
fn size_factor(percent: f64) -> f64 {
    0.28 + 0.72 * ((percent - 5.0) / 95.0).clamp(0.0, 1.0).powf(1.3)
}

fn is_prime(n: usize) -> bool {
    n >= 2
        && (2..)
            .take_while(|d| d * d <= n)
            .all(|d| !n.is_multiple_of(d))
}

/// The prime number of frames nearest above a length in seconds.
fn prime_frames(seconds: f64) -> usize {
    let mut n = (seconds * RATE).round() as usize;
    while !is_prime(n) {
        n += 1;
    }
    n
}

/// A set of early reflection taps for both sides: delays in frames and gains.
#[derive(Clone, Copy, Default)]
struct Taps {
    delays: [[usize; 14]; 2],
    gains: [[f64; 14]; 2],
    count: usize,
}
impl Taps {
    fn new(model: SpaceModel, size: f64, density: f64) -> Self {
        let scale = 0.4 + 0.6 * size_factor(size);
        let count = 4 + (10.0 * density).round() as usize;
        let mut taps = Self {
            count,
            ..Self::default()
        };
        for (side, (delays, gains)) in taps.delays.iter_mut().zip(&mut taps.gains).enumerate() {
            for (k, ms) in reflection_times(model).into_iter().enumerate().take(count) {
                let ms = if side == 0 { ms } else { ms * 1.07 + 1.3 };
                delays[k] = ((ms * scale / 1000.0 * RATE).round() as usize).max(1);
                let sign = if (k + side) % 2 == 0 { 1.0 } else { -1.0 };
                gains[k] = 0.85f64.powi(k as i32) * sign;
            }
            // The reflections together carry about the input's energy.
            let norm = 1.0 / gains.iter().map(|gain| gain * gain).sum::<f64>().sqrt();
            gains.iter_mut().for_each(|gain| *gain *= norm);
        }
        taps
    }
    #[inline]
    fn read(&self, lines: &[Line; 2]) -> [f64; 2] {
        std::array::from_fn(|side| {
            (0..self.count)
                .map(|k| f64::from(lines[side].at(self.delays[side][k])) * self.gains[side][k])
                .sum()
        })
    }
}

/// A granular pitch shifter for one channel: two read heads sweep across
/// a window at the pitch ratio, faded in and out by sine windows that keep
/// the power even.
struct Shifter {
    line: Line,
    phase: f64,
    band: [OnePole; 2],
}
impl Shifter {
    fn new() -> Self {
        Self {
            line: Line::new(WINDOW as usize + 8),
            phase: 0.0,
            band: [OnePole::default(); 2],
        }
    }
    fn clear(&mut self) {
        self.line.clear();
        self.band.iter_mut().for_each(OnePole::clear);
        self.phase = 0.0;
    }
    /// Band-limit the input so the shifted copy stays under the Nyquist
    /// frequency and the tone's high cut.
    fn tune(&mut self, ratio: f64, high_cut: f64) {
        let corner = (0.45 * RATE / ratio).min(high_cut);
        self.band.iter_mut().for_each(|pole| pole.tune(corner));
    }
    #[inline]
    fn process(&mut self, input: f64, ratio: f64) -> f64 {
        let limited = self.band[0].low(input);
        let limited = self.band[1].low(limited);
        self.line.push(limited as f32);
        self.phase = (self.phase + (ratio - 1.0) / WINDOW).rem_euclid(1.0);
        let other = (self.phase + 0.5).fract();
        let a = self.line.read(2.0 + (1.0 - self.phase) * WINDOW);
        let b = self.line.read(2.0 + (1.0 - other) * WINDOW);
        a * (PI * self.phase).sin() + b * (PI * other).sin()
    }
}

/// A delay with a crossfade from its old length to a new one.
#[derive(Clone, Copy)]
struct Retime {
    length: f64,
    previous: f64,
}

pub(super) struct Core {
    activity: f64,
    pre: [Line; 2],
    pre_time: Retime,
    early: [Line; 2],
    taps: Taps,
    previous_taps: Taps,
    early_tone: [Tone; 2],
    diffusers: [[AllPass; 4]; 2],
    bloom: [[AllPass; 2]; 2],
    lines: [Line; LINES],
    lengths: [Retime; LINES],
    gains: [f64; LINES],
    tone: [Tone; LINES],
    /// The vintage character's extra damping.
    soften: [OnePole; LINES],
    inner: [AllPass; LINES],
    /// Frames left of the crossfade after a retime.
    fade: usize,
    lfo: Lfo,
    wander: Lfo,
    shifters: [Shifter; 2],
    /// The tail's last frame, which feeds the shimmer.
    shimmer: [f64; 2],
    ratio: f64,
    target_ratio: f64,
    shimmer_send: f64,
    shimmer_level: Glide,
    ducker: Ducker,
    mod_rate: f64,
    depth: Glide,
    diffusion: Glide,
    density: Glide,
    late_norm: Glide,
    early_gain: Glide,
    late_gain: Glide,
    width: Glide,
    output: Glide,
    drive: Glide,
    duck: Glide,
    vintage: Glide,
    bloomy: Glide,
    high_cut: f64,
}
impl Core {
    pub(super) fn new() -> Self {
        let line = |ms: f64| Line::new((ms / 1000.0 * RATE) as usize);
        let smooth = |value| Glide::new(value, 0.005);
        let diffusers = |times: [f64; 4]| times.map(|ms| AllPass::new(ms / 1000.0));
        Self {
            activity: 0.0,
            pre: [line(260.0), line(260.0)],
            pre_time: Retime {
                length: 2.0,
                previous: 2.0,
            },
            early: [line(90.0), line(90.0)],
            taps: Taps::default(),
            previous_taps: Taps::default(),
            early_tone: [Tone::default(); 2],
            diffusers: [
                diffusers([4.77, 3.59, 12.73, 9.30]),
                diffusers([4.91, 3.73, 12.27, 9.71]),
            ],
            bloom: [
                [AllPass::new(0.0613), AllPass::new(0.0897)],
                [AllPass::new(0.0671), AllPass::new(0.0973)],
            ],
            // The hall's longest line plus the deepest modulation.
            lines: std::array::from_fn(|_| line(150.0)),
            lengths: [Retime {
                length: 1000.0,
                previous: 1000.0,
            }; LINES],
            gains: [0.0; LINES],
            tone: [Tone::default(); LINES],
            soften: [OnePole::default(); LINES],
            inner: INNER_MS.map(|ms| AllPass::new(ms / 1000.0)),
            fade: 0,
            lfo: Lfo::default(),
            wander: Lfo::default(),
            shifters: [Shifter::new(), Shifter::new()],
            shimmer: [0.0; 2],
            ratio: 2.0,
            target_ratio: 2.0,
            shimmer_send: 0.0,
            shimmer_level: Glide::new(0.0, 0.04),
            ducker: Ducker::new(),
            mod_rate: 0.0,
            depth: smooth(0.0),
            diffusion: smooth(0.0),
            density: smooth(0.0),
            late_norm: smooth(0.0),
            early_gain: smooth(0.0),
            late_gain: smooth(0.0),
            width: smooth(1.0),
            output: smooth(0.0),
            drive: smooth(0.0),
            duck: smooth(0.0),
            vintage: Glide::new(0.0, 0.02),
            bloomy: Glide::new(0.0, 0.02),
            high_cut: 20_000.0,
        }
    }
    /// Take new settings; the delay of the bus's safety limiter comes off
    /// the pre-delay.
    pub(super) fn set(&mut self, s: &Aetherform, compensation: f64) {
        let size = f64::from(s.size_percent);
        let decay = f64::from(s.decay_seconds);
        let factor = size_factor(size);
        let mut retimed = false;
        let mut total = 0.0;
        for (i, ms) in base_lengths(s.space_model).into_iter().enumerate() {
            let seconds = (ms * factor).max(5.0) / 1000.0;
            let frames = prime_frames(seconds) as f64;
            if frames != self.lengths[i].length {
                self.lengths[i].previous = self.lengths[i].length;
                self.lengths[i].length = frames;
                retimed = true;
            }
            // A circulation also passes the line's inner all-pass.
            let circulation = frames / RATE + INNER_MS[i] / 1000.0;
            self.gains[i] = 10f64.powf(-3.0 * circulation / decay);
            total += self.gains[i] * self.gains[i];
        }
        // Each circulation keeps g² of the energy: scale the tail so a
        // steady input leaves it near its own level, whatever the decay.
        self.late_norm
            .set(1.6 * (1.0 - total / LINES as f64).sqrt());
        let pre = (f64::from(s.pre_delay_ms) / 1000.0 * RATE - compensation).max(2.0);
        if pre != self.pre_time.length {
            self.pre_time.previous = self.pre_time.length;
            self.pre_time.length = pre;
            retimed = true;
        }
        let density = f64::from(s.density_percent) / 100.0;
        let taps = Taps::new(s.space_model, size, density);
        if taps.delays != self.taps.delays || taps.count != self.taps.count {
            self.previous_taps = self.taps;
            retimed = true;
        }
        self.taps = taps;
        if retimed {
            self.fade = FADE;
        }
        let (low_cut, high_cut) = (f64::from(s.low_cut_hz), f64::from(s.high_cut_hz));
        self.high_cut = high_cut;
        self.tone
            .iter_mut()
            .for_each(|tone| tone.tune(low_cut, high_cut));
        self.early_tone
            .iter_mut()
            .for_each(|tone| tone.tune(low_cut, high_cut));
        self.soften
            .iter_mut()
            .for_each(|pole| pole.tune(0.55 * high_cut));
        let bloom = s.character == ReverbCharacter::Bloom;
        self.vintage
            .set(f64::from(u8::from(s.character == ReverbCharacter::Vintage)));
        self.bloomy.set(f64::from(u8::from(bloom)));
        self.mod_rate = f64::from(s.mod_rate_hz) * if bloom { 0.6 } else { 1.0 };
        let depth = (f64::from(s.mod_depth_percent) / 100.0).powi(2) * 0.004 * RATE;
        self.depth.set(depth * if bloom { 1.5 } else { 1.0 });
        self.diffusion
            .set(0.3 + 0.45 * f64::from(s.diffusion_percent) / 100.0);
        self.density.set(0.1 + 0.5 * density);
        let balance = f64::from(s.late_balance_percent) / 100.0 * FRAC_PI_2;
        let early_on = if s.early_reflections_enabled {
            1.0
        } else {
            0.0
        };
        self.early_gain.set(balance.cos() * early_on);
        self.late_gain.set(balance.sin());
        self.width.set(f64::from(s.width_percent) / 100.0);
        self.drive.set(f64::from(s.drive_percent) / 100.0);
        self.duck.set(f64::from(s.duck_amount_percent) / 100.0);
        self.ducker.set_release(f64::from(s.duck_release_ms));
        self.output.set(if s.bypass {
            0.0
        } else {
            db_to_gain(f64::from(s.output_db))
        });
        self.target_ratio = 2f64.powf(f64::from(s.shimmer_interval_semitones) / 12.0);
        self.shimmer_send = if s.shimmer_enabled {
            0.55 * (f64::from(s.shimmer_amount_percent) / 100.0).powf(1.5)
        } else {
            0.0
        };
        let ratio = self.target_ratio;
        self.shifters
            .iter_mut()
            .for_each(|shifter| shifter.tune(ratio, high_cut));
    }
    /// Move every setting straight to its target, with no crossfade.
    pub(super) fn settle(&mut self) {
        self.fade = 0;
        self.ratio = self.target_ratio;
        self.shimmer_level.set(self.shimmer_send);
        for glide in [
            &mut self.depth,
            &mut self.diffusion,
            &mut self.density,
            &mut self.late_norm,
            &mut self.early_gain,
            &mut self.late_gain,
            &mut self.width,
            &mut self.output,
            &mut self.drive,
            &mut self.duck,
            &mut self.vintage,
            &mut self.bloomy,
            &mut self.shimmer_level,
        ] {
            glide.settle();
        }
    }
    pub(super) fn clear(&mut self) {
        self.activity = 0.0;
        self.pre
            .iter_mut()
            .chain(&mut self.early)
            .for_each(Line::clear);
        self.lines.iter_mut().for_each(Line::clear);
        self.early_tone
            .iter_mut()
            .chain(&mut self.tone)
            .for_each(Tone::clear);
        self.soften.iter_mut().for_each(OnePole::clear);
        self.diffusers.iter_mut().flatten().for_each(AllPass::clear);
        self.bloom.iter_mut().flatten().for_each(AllPass::clear);
        self.inner.iter_mut().for_each(AllPass::clear);
        self.shifters.iter_mut().for_each(Shifter::clear);
        self.shimmer = [0.0; 2];
        self.ducker.clear();
    }
    pub(super) fn activity(&self) -> f64 {
        self.activity
    }
    /// The crossfade weight of the new positions during a retime.
    #[inline]
    fn blend(&self) -> f64 {
        let progress = 1.0 - self.fade as f64 / FADE as f64;
        0.5 - 0.5 * (PI * progress).cos()
    }
    #[inline]
    pub(super) fn process(&mut self, left: f64, right: f64) -> (f64, f64) {
        let duck = self.ducker.gain(left, right, self.duck.next());
        let amount = self.drive.next();
        let input = [drive(left, amount), drive(right, amount)];
        let fading = self.fade > 0;
        let weight = self.blend();
        // Pre-delay.
        let mut pre = [0.0; 2];
        for side in 0..2 {
            let now = self.pre[side].read(self.pre_time.length);
            pre[side] = if fading {
                let before = self.pre[side].read(self.pre_time.previous);
                before + (now - before) * weight
            } else {
                now
            };
            self.pre[side].push(input[side] as f32);
            self.early[side].push(pre[side] as f32);
        }
        // Early reflections, filtered once by the tone.
        let mut early = self.taps.read(&self.early);
        if fading {
            let before = self.previous_taps.read(&self.early);
            for (now, before) in early.iter_mut().zip(before) {
                *now = before + (*now - before) * weight;
            }
        }
        for (sample, tone) in early.iter_mut().zip(&mut self.early_tone) {
            *sample = tone.process(*sample);
        }
        // Input diffusion, the bloom's smear, and the shimmer's return.
        let diffusion = self.diffusion.next();
        let bloom = self.bloomy.next();
        let shimmer = self.shimmer_level.next();
        let mut feed = [0.0; 2];
        for side in 0..2 {
            let mut x = pre[side];
            for diffuser in &mut self.diffusers[side] {
                x = diffuser.process(x, diffusion);
            }
            if bloom > 0.0 {
                let [a, b] = &mut self.bloom[side];
                let smeared = b.process(a.process(x, 0.6), 0.6);
                x += (smeared - x) * bloom;
            }
            feed[side] = x + shimmer * self.shimmer[side];
        }
        // The network.
        let depth = self.depth.next();
        let vintage = self.vintage.next();
        let wobble = vintage * 0.5 * depth * self.wander.value(0.0);
        let density = self.density.next();
        let mut taps = [0.0; LINES];
        for (i, tap) in taps.iter_mut().enumerate() {
            let offset = depth * self.lfo.value(i as f64 / LINES as f64) + wobble;
            let length = self.lengths[i];
            let now = self.lines[i].read(length.length + offset);
            let mut x = if fading {
                let before = self.lines[i].read(length.previous + offset);
                before + (now - before) * weight
            } else {
                now
            };
            x = self.tone[i].process(x);
            if vintage > 0.0 {
                let warm = (1.4 * self.soften[i].low(x)).tanh() / 1.4;
                x += (warm - x) * vintage;
            }
            *tap = self.inner[i].process(x, density);
        }
        self.lfo.advance(self.mod_rate);
        self.wander.advance(0.07);
        self.activity = taps
            .iter()
            .chain(&pre)
            .chain(&early)
            .fold(0.0_f64, |peak, sample| peak.max(sample.abs()));
        let signs = [1.0, -1.0, 1.0, -1.0];
        let norm = self.late_norm.next();
        let mut late = [0.0; 2];
        for (i, tap) in taps.iter().enumerate() {
            late[i % 2] += tap * signs[i / 2] * 0.5 * norm;
        }
        let householder = taps.iter().sum::<f64>() * 2.0 / LINES as f64;
        for (i, tap) in taps.iter().enumerate() {
            let injected = feed[i % 2] * signs[i / 2] * 0.5;
            let sample = bound(self.gains[i] * (tap - householder) + injected);
            self.lines[i].push(sample as f32);
        }
        // The shimmer: an interval change waits until the old one has faded.
        if self.ratio != self.target_ratio {
            self.shimmer_level.set(0.0);
            if self.shimmer_level.value() < 1e-4 {
                self.ratio = self.target_ratio;
                self.shifters.iter_mut().for_each(Shifter::clear);
            }
        } else {
            self.shimmer_level.set(self.shimmer_send);
        }
        if shimmer > 0.0 || self.shimmer_send > 0.0 {
            for ((shimmer, shifter), late) in
                self.shimmer.iter_mut().zip(&mut self.shifters).zip(late)
            {
                *shimmer = shifter.process(late, self.ratio);
            }
        } else {
            self.shimmer = [0.0; 2];
        }
        if fading {
            self.fade -= 1;
        }
        let (early_gain, late_gain) = (self.early_gain.next(), self.late_gain.next());
        let (l, r) = widen(
            early[0] * early_gain + late[0] * late_gain,
            early[1] * early_gain + late[1] * late_gain,
            self.width.next(),
        );
        let level = self.output.next() * duck;
        (l * level, r * level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neutral() -> Aetherform {
        Aetherform {
            character: ReverbCharacter::Natural,
            pre_delay_ms: 0.0,
            mod_depth_percent: 0.0,
            duck_amount_percent: 0.0,
            output_db: 0.0,
            low_cut_hz: 20.0,
            high_cut_hz: 20_000.0,
            ..Aetherform::default()
        }
    }

    fn reverb(settings: &Aetherform) -> Core {
        let mut core = Core::new();
        core.set(settings, 0.0);
        core.settle();
        core
    }

    /// Energy in 50 ms windows after an impulse, in dB.
    fn decay_curve(core: &mut Core, seconds: f64) -> Vec<f64> {
        let window = 2205;
        let frames = (seconds * RATE) as usize;
        let mut curve = Vec::new();
        let mut sum = 0.0;
        for i in 0..frames {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let (l, r) = core.process(x, x);
            assert!(l.is_finite() && r.is_finite());
            sum += l * l + r * r;
            if (i + 1) % window == 0 {
                curve.push(10.0 * (sum + 1e-30).log10());
                sum = 0.0;
            }
        }
        curve
    }

    /// The time to fall 60 dB, from the slope between -5 and -25 dB below
    /// the curve's peak.
    fn rt60(curve: &[f64]) -> f64 {
        let peak = curve.iter().cloned().fold(f64::MIN, f64::max);
        let at = |drop: f64| curve.iter().rposition(|v| *v >= peak - drop).unwrap() as f64;
        let (a, b) = (at(5.0), at(25.0));
        (b - a) * 0.05 * 60.0 / 20.0
    }

    #[test]
    fn the_tail_falls_60_db_in_the_decay_time() {
        for (model, decay) in [
            (SpaceModel::Room, 0.8),
            (SpaceModel::Chamber, 2.8),
            (SpaceModel::Hall, 6.0),
            (SpaceModel::Plate, 1.9),
        ] {
            let settings = Aetherform {
                space_model: model,
                decay_seconds: decay,
                late_balance_percent: 100.0,
                ..neutral()
            };
            let mut core = reverb(&settings);
            let decay = f64::from(decay);
            let measured = rt60(&decay_curve(&mut core, decay * 1.6 + 0.5));
            assert!(
                (measured / decay - 1.0).abs() < 0.25,
                "{model:?}: {measured:.2} s for {decay} s"
            );
        }
    }

    #[test]
    fn size_does_not_change_the_decay_and_models_differ() {
        let measure = |size: f32, model| {
            let settings = Aetherform {
                space_model: model,
                size_percent: size,
                decay_seconds: 2.0,
                late_balance_percent: 100.0,
                ..neutral()
            };
            rt60(&decay_curve(&mut reverb(&settings), 3.5))
        };
        let (small, large) = (
            measure(10.0, SpaceModel::Hall),
            measure(100.0, SpaceModel::Hall),
        );
        assert!((small / large - 1.0).abs() < 0.25, "{small} vs {large}");
        // The first late echo comes later in a hall than in a room.
        let first = |model| {
            let settings = Aetherform {
                space_model: model,
                late_balance_percent: 100.0,
                diffusion_percent: 0.0,
                ..neutral()
            };
            let mut core = reverb(&settings);
            (0..20_000)
                .position(|i| core.process(if i == 0 { 1.0 } else { 0.0 }, 0.0).0.abs() > 1e-4)
                .unwrap()
        };
        assert!(first(SpaceModel::Hall) > first(SpaceModel::Room));
    }

    #[test]
    fn pre_delay_holds_back_the_sound() {
        let onset = |pre_delay_ms: f32| {
            let settings = Aetherform {
                pre_delay_ms,
                late_balance_percent: 0.0,
                ..neutral()
            };
            let mut core = reverb(&settings);
            (0..30_000)
                .position(|i| core.process(if i == 0 { 1.0 } else { 0.0 }, 0.0).0.abs() > 1e-4)
                .unwrap()
        };
        let (none, long) = (onset(0.0), onset(120.0));
        let gap = long - none;
        assert!((gap as f64 - 0.12 * RATE).abs() < 3.0, "{gap}");
    }

    #[test]
    fn extremes_stay_finite_and_bounded() {
        let settings = Aetherform {
            decay_seconds: 30.0,
            shimmer_enabled: true,
            shimmer_amount_percent: 100.0,
            shimmer_interval_semitones: 24,
            drive_percent: 100.0,
            width_percent: 200.0,
            mod_depth_percent: 100.0,
            character: ReverbCharacter::Bloom,
            ..Aetherform::default()
        };
        let mut core = reverb(&settings);
        let mut peak: f64 = 0.0;
        for i in 0..10 * 44_100 {
            let x = if i < 44_100 {
                0.9 * (i as f64 * 0.03).sin()
            } else {
                0.0
            };
            let (l, r) = core.process(x, -x);
            assert!(l.is_finite() && r.is_finite());
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak < 8.0, "{peak}");
    }

    #[test]
    fn the_shimmer_adds_energy_an_octave_up() {
        // A 400 Hz tone; with the shimmer on, 800 Hz grows in the tail.
        let level_at = |settings: &Aetherform, hz: f64| {
            let mut core = reverb(settings);
            let (mut re, mut im) = (0.0, 0.0);
            for i in 0..4 * 44_100 {
                let x = if i < 44_100 {
                    0.3 * (2.0 * PI * 400.0 * i as f64 / RATE).sin()
                } else {
                    0.0
                };
                let (l, _) = core.process(x, x);
                if i >= 44_100 + 4410 {
                    let angle = 2.0 * PI * hz * i as f64 / RATE;
                    re += l * angle.cos();
                    im += l * angle.sin();
                }
            }
            re.hypot(im)
        };
        let plain = Aetherform {
            late_balance_percent: 100.0,
            decay_seconds: 4.0,
            ..neutral()
        };
        let shimmering = Aetherform {
            shimmer_enabled: true,
            shimmer_amount_percent: 80.0,
            ..plain
        };
        let without = level_at(&plain, 800.0) / level_at(&plain, 400.0);
        let with = level_at(&shimmering, 800.0) / level_at(&shimmering, 400.0);
        assert!(with > 4.0 * without, "{without} vs {with}");
    }

    #[test]
    fn width_zero_is_mono_and_bypass_keeps_the_tail() {
        let settings = Aetherform {
            width_percent: 0.0,
            ..neutral()
        };
        let mut core = reverb(&settings);
        for i in 0..20_000 {
            let (l, r) = core.process(if i == 0 { 1.0 } else { 0.0 }, 0.0);
            assert!((l - r).abs() < 1e-12);
        }
        let mut core = reverb(&neutral());
        core.process(1.0, 1.0);
        core.set(
            &Aetherform {
                bypass: true,
                ..neutral()
            },
            0.0,
        );
        core.settle();
        for _ in 0..10_000 {
            assert_eq!(core.process(0.0, 0.0), (0.0, 0.0));
        }
        core.set(&neutral(), 0.0);
        core.settle();
        let tail = (0..2000)
            .map(|_| core.process(0.0, 0.0).0.abs())
            .fold(0.0, f64::max);
        assert!(tail > 1e-5, "{tail}");
    }

    #[test]
    fn a_size_change_crossfades_without_a_jump() {
        let mut core = reverb(&neutral());
        let mut last = 0.0;
        let mut worst: f64 = 0.0;
        for i in 0..40_000 {
            if i == 20_000 {
                core.set(
                    &Aetherform {
                        size_percent: 20.0,
                        ..neutral()
                    },
                    0.0,
                );
            }
            let x = 0.3 * (2.0 * PI * 220.0 * i as f64 / RATE).sin();
            let (l, _) = core.process(x, x);
            if i > 19_000 {
                worst = worst.max((l - last).abs());
            }
            last = l;
        }
        // A 220 Hz tone near unit level moves at most about 0.03 per frame.
        assert!(worst < 0.1, "{worst}");
    }
}
