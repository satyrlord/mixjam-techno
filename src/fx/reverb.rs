//! The Aetherform Reverb's signal processing.
//!
//! The input passes the pre-delay. From there, model-specific early
//! reflections run beside a chain of input all-passes that blur the sound.
//! The blurred sound passes the low and high cut once and feeds an
//! eight-line feedback delay network with a Householder matrix. Each line
//! has its damping, the character's colouring, and an all-pass inside the
//! loop. A pitch shifter can feed the tail back an interval higher: the
//! shimmer. The early and late sounds then blend at equal power, and the
//! output is widened, ducked, and trimmed.
//!
//! A line's gain is `10^(-3 t / decay)` for its length `t` in seconds, so
//! every line falls by 60 dB in the decay time whatever the size. The cuts
//! stay out of the loop: inside it they would cut again on every
//! circulation, and a hall would die in a fraction of its decay. The
//! damping is designed per line from its length, so the tail falls by
//! 60 dB at the high cut in a fixed share of the decay and in the decay
//! below it. Size, model, and pre-delay changes crossfade between old and
//! new read positions, never glide, so the pitch never bends.
use super::parts::{AllPass, Ducker, Glide, Lfo, Line, OnePole, Tone, bound, drive, widen};
use super::{Aetherform, ReverbCharacter, SpaceModel};
use crate::master::dsp::{RATE, db_to_gain, flush};
use std::f64::consts::{FRAC_PI_2, PI};

const LINES: usize = 8;
/// A retime crossfades over 50 ms.
const FADE: usize = 2205;
/// The pitch shifter's window: 60 ms.
const WINDOW: f64 = 2646.0;
/// The vintage character's slow drift of every line's read position, in hertz.
const WANDER_HZ: f64 = 0.07;
/// The lengths of the all-passes inside the loop, one per line.
const INNER_MS: [f64; LINES] = [2.31, 3.07, 3.71, 4.39, 2.73, 3.41, 4.13, 4.87];
/// The share of the decay in which the tail falls by 60 dB at the high cut.
const HIGH_CUT_DECAY: f64 = 0.6;
/// The vintage character's darker share.
const VINTAGE_HIGH_CUT_DECAY: f64 = 0.4;

/// The pole `b` of a line's damping, `y = (1 - b) x + b y'`, unity at 0 Hz.
/// Its gain at the high cut, whose cosine of the angular frequency is
/// `cosine`, adds the loss that makes the line fall 60 dB there in `share`
/// of the decay instead of the whole decay.
fn damping(circulation: f64, decay: f64, share: f64, cosine: f64) -> f64 {
    let squared = 10f64.powf(-6.0 * circulation / decay * (1.0 / share - 1.0));
    if squared >= 1.0 - 1e-12 {
        return 0.0;
    }
    let a = (1.0 - squared * cosine) / (1.0 - squared);
    a - (a * a - 1.0).sqrt()
}

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
#[derive(Clone, Copy, Default, PartialEq)]
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
    /// Silence the window. The read heads keep their phase, as the reverb's
    /// modulation does, so a bus that slept sounds as one that did not.
    fn clear(&mut self) {
        self.line.clear();
        self.band.iter_mut().for_each(OnePole::clear);
    }
    fn skip(&mut self, ratio: f64, frames: u64) {
        self.phase = (self.phase + frames as f64 * (ratio - 1.0) / WINDOW).rem_euclid(1.0);
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

/// The read position of one modulated delay head.
#[inline]
fn modulated_length(length: f64, depth: f64, phase: f64, vintage: f64, wander: f64) -> f64 {
    // A shorter head must be safe before the shared depth finishes gliding.
    // Each head keeps its own bound so the outgoing head remains continuous.
    let depth = depth.min(0.45 * length);
    let wobble = vintage * 0.5 * depth * wander;
    length + (depth * phase + wobble)
}

/// The delay positions that a set of settings asks for.
#[derive(Clone, Copy, PartialEq)]
struct Positions {
    lines: [f64; LINES],
    pre: f64,
    taps: Taps,
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
    /// The low and high cut, once on the sound that enters the network.
    input_tone: [Tone; 2],
    /// Each line's damping pole, and its last output.
    damping: [f64; LINES],
    damped: [f64; LINES],
    inner: [AllPass; LINES],
    /// Frames left of the crossfade after a retime.
    fade: usize,
    /// Positions that arrived during a crossfade. They start their own
    /// crossfade when it ends, so the output never jumps from a half blend.
    queued: Option<Positions>,
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
    /// The requested swing, before the active delay lengths bound it.
    requested_depth: f64,
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
            input_tone: [Tone::default(); 2],
            damping: [0.0; LINES],
            damped: [0.0; LINES],
            inner: INNER_MS.map(|ms| AllPass::new(ms / 1000.0)),
            fade: 0,
            queued: None,
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
            requested_depth: 0.0,
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
        let mut total = 0.0;
        let mut circulations = [0.0; LINES];
        let mut lines = [0.0; LINES];
        let bases = base_lengths(s.space_model);
        for (i, ms) in bases.into_iter().enumerate() {
            // The 5 ms floor scales with each line, so that small sizes keep
            // distinct lengths instead of sharing one comb.
            let seconds = (ms * factor).max(5.0 * ms / bases[0]) / 1000.0;
            let frames = prime_frames(seconds) as f64;
            lines[i] = frames;
            // A circulation also passes the line's inner all-pass.
            let circulation = frames / RATE + INNER_MS[i] / 1000.0;
            circulations[i] = circulation;
            self.gains[i] = 10f64.powf(-3.0 * circulation / decay);
            total += self.gains[i] * self.gains[i];
        }
        // Each circulation keeps g² of the energy: scale the tail so a
        // steady input leaves it near its own level, whatever the decay.
        self.late_norm
            .set(1.6 * (1.0 - total / LINES as f64).sqrt());
        let pre = (f64::from(s.pre_delay_ms) / 1000.0 * RATE - compensation).max(2.0);
        let density = f64::from(s.density_percent) / 100.0;
        let taps = Taps::new(s.space_model, size, density);
        if self.fade > 0 {
            let target = Positions { lines, pre, taps };
            self.queued = (target != self.positions()).then_some(target);
        } else {
            self.retime(Positions { lines, pre, taps });
        }
        let (low_cut, high_cut) = (f64::from(s.low_cut_hz), f64::from(s.high_cut_hz));
        self.high_cut = high_cut;
        self.input_tone
            .iter_mut()
            .chain(&mut self.early_tone)
            .for_each(|tone| tone.tune(low_cut, high_cut));
        let share = if s.character == ReverbCharacter::Vintage {
            VINTAGE_HIGH_CUT_DECAY
        } else {
            HIGH_CUT_DECAY
        };
        let cosine = (2.0 * PI * high_cut.min(0.49 * RATE) / RATE).cos();
        for (pole, circulation) in self.damping.iter_mut().zip(circulations) {
            *pole = damping(circulation, decay, share, cosine);
        }
        let bloom = s.character == ReverbCharacter::Bloom;
        self.vintage
            .set(f64::from(u8::from(s.character == ReverbCharacter::Vintage)));
        self.bloomy.set(f64::from(u8::from(bloom)));
        self.mod_rate = f64::from(s.mod_rate_hz) * if bloom { 0.6 } else { 1.0 };
        let depth = (f64::from(s.mod_depth_percent) / 100.0).powi(2) * 0.004 * RATE;
        self.requested_depth = depth * if bloom { 1.5 } else { 1.0 };
        self.update_depth();
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
    /// The positions the delays move to.
    fn positions(&self) -> Positions {
        Positions {
            lines: self.lengths.map(|length| length.length),
            pre: self.pre_time.length,
            taps: self.taps,
        }
    }
    /// Bound the swing to the positions that still play, and prepare for a
    /// queued shorter delay. An old position stops bounding it after its fade.
    fn update_depth(&mut self) {
        let mut shortest = self
            .lengths
            .iter()
            .map(|length| length.length)
            .fold(f64::INFINITY, f64::min);
        if self.fade > 0 {
            for length in &self.lengths {
                shortest = shortest.min(length.previous);
            }
        }
        if let Some(queued) = self.queued {
            for length in queued.lines {
                shortest = shortest.min(length);
            }
        }
        self.depth.set(self.requested_depth.min(0.45 * shortest));
    }
    /// Move the delays to `target`, with a crossfade when a delay changes.
    fn retime(&mut self, target: Positions) {
        if target == self.positions() {
            return;
        }
        // Every read head participates in the fade, including those whose
        // target did not move. Snapshot the whole current geometry together.
        for (length, frames) in self.lengths.iter_mut().zip(target.lines) {
            length.previous = length.length;
            length.length = frames;
        }
        self.pre_time.previous = self.pre_time.length;
        self.pre_time.length = target.pre;
        self.previous_taps = self.taps;
        self.taps = target.taps;
        self.fade = FADE;
    }
    /// Move every setting straight to its target, with no crossfade.
    pub(super) fn settle(&mut self) {
        if let Some(target) = self.queued.take() {
            self.retime(target);
        }
        self.fade = 0;
        self.update_depth();
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
            .chain(&mut self.input_tone)
            .for_each(Tone::clear);
        self.damped = [0.0; LINES];
        self.diffusers.iter_mut().flatten().for_each(AllPass::clear);
        self.bloom.iter_mut().flatten().for_each(AllPass::clear);
        self.inner.iter_mut().for_each(AllPass::clear);
        self.shifters.iter_mut().for_each(Shifter::clear);
        self.shimmer = [0.0; 2];
        self.ducker.clear();
    }
    /// Move the modulation and the shimmer's read heads on by `frames`
    /// frames that the bus slept through, so their phases do not depend on
    /// when the bus slept.
    pub(super) fn skip(&mut self, frames: u64) {
        self.lfo.skip(self.mod_rate, frames);
        self.wander.skip(WANDER_HZ, frames);
        if self.shimmer_level.value() > 0.0 || self.shimmer_send > 0.0 {
            let ratio = self.ratio;
            self.shifters
                .iter_mut()
                .for_each(|shifter| shifter.skip(ratio, frames));
        }
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
    #[cfg(test)]
    pub(super) fn process(&mut self, left: f64, right: f64) -> (f64, f64) {
        self.process_keyed(left, right, None)
    }
    /// With a key, the ducking follows it instead of the input.
    pub(super) fn process_keyed(
        &mut self,
        left: f64,
        right: f64,
        key: Option<(f64, f64)>,
    ) -> (f64, f64) {
        let (key_left, key_right) = key.unwrap_or((left, right));
        let duck = self.ducker.gain(key_left, key_right, self.duck.next());
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
            // The shimmer's return passes the cuts too: each pass climbs an
            // octave, and only the high cut keeps that climb from running away.
            feed[side] = self.input_tone[side].process(x + shimmer * self.shimmer[side]);
        }
        // The network.
        let depth = self.depth.next();
        let vintage = self.vintage.next();
        let wander = self.wander.value(0.0);
        let density = self.density.next();
        let mut taps = [0.0; LINES];
        for (i, tap) in taps.iter_mut().enumerate() {
            let phase = self.lfo.value(i as f64 / LINES as f64);
            let length = self.lengths[i];
            let now = self.lines[i].read(modulated_length(
                length.length,
                depth,
                phase,
                vintage,
                wander,
            ));
            let mut x = if fading {
                let before = self.lines[i].read(modulated_length(
                    length.previous,
                    depth,
                    phase,
                    vintage,
                    wander,
                ));
                before + (now - before) * weight
            } else {
                now
            };
            let pole = self.damping[i];
            x = flush((1.0 - pole) * x + pole * self.damped[i]);
            self.damped[i] = x;
            if vintage > 0.0 {
                let warm = (1.4 * x).tanh() / 1.4;
                x += (warm - x) * vintage;
            }
            *tap = self.inner[i].process(x, density);
        }
        self.lfo.advance(self.mod_rate);
        self.wander.advance(WANDER_HZ);
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
            if self.fade == 0 {
                if let Some(target) = self.queued.take() {
                    self.retime(target);
                }
                self.update_depth();
            }
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
mod tests;
