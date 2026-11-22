//! The Aetherform Reverb's signal processing.
//!
//! The input passes the pre-delay. From there, model-specific early
//! reflections run beside a chain of input all-passes that blur the sound.
//! The blurred sound passes the low and high cut once and feeds a
//! sixteen-line feedback delay network. A fast Walsh-Hadamard transform with
//! a signed permutation mixes the lines. Each line has its damping, the
//! character's colouring, and two all-passes inside the loop. A phase
//! vocoder can feed the tail back an interval higher: the shimmer. The early
//! and late sounds then blend at equal power, and the output is widened and
//! trimmed.
//!
//! A line's gain is `10^(-3 t / decay)` for its length `t` in seconds, so
//! every line falls by 60 dB in the decay time whatever the size. The cuts
//! stay out of the loop: inside it they would cut again on every
//! circulation, and a hall would die in a fraction of its decay. The
//! damping is designed per line from its length, so the tail falls by
//! 60 dB at the high cut in a fixed share of the decay and in the decay
//! below it. Bass and treble shelves adjust the decay of their bands
//! inside the loop. Size, model, and pre-delay changes crossfade between old and
//! new read positions, never glide, so the pitch never bends. Offline
//! renders read the delays with a windowed sinc.
use super::parts::{
    AllPass, FixedAllPass, FixedAllPassBank, Lfo, Line, OnePole, RingBank, ShelfBank, bound, drive,
    lfo_step, widen,
};
use super::{Aetherform, RenderQuality, ReverbCharacter, SpaceModel};
use crate::master::dsp::{RATE, Smoothed, db_to_gain, flush, gain_to_db};
use std::f64::consts::{FRAC_PI_2, PI};

mod profile;
mod shifter;
use profile::{base_lengths, high_cut_share, reflection_times, size_factor};
use shifter::Shifter;

pub use profile::ReverbProfile;
pub(crate) use profile::sample_reverb_profile;

const LINES: usize = 16;
/// The frames of each delay line: the hall's longest line, its deepest
/// modulation, and room to spare.
const FDN_CAP: usize = 8192;
/// The number of early reflection taps of each side.
const ER_TAPS: usize = 16;
const RIGHT_ER_SIGNS: [f64; ER_TAPS] = [
    1.0, 1.0, -1.0, 1.0, -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0,
];
/// A retime crossfades over 50 ms.
const FADE: usize = 2205;
/// The vintage character's slow drift of every line's read position, in hertz.
const WANDER_HZ: f64 = 0.07;
/// The lengths of the first all-passes inside the loop, one per line.
const INNER1_MS: [f64; LINES] = [
    7.11, 8.87, 10.69, 12.71, 7.93, 9.71, 11.33, 13.67, 7.51, 9.31, 11.09, 13.11, 8.33, 10.13,
    12.13, 14.29,
];
/// The lengths of the second all-passes inside the loop, one per line.
const INNER2_MS: [f64; LINES] = [
    15.71, 18.11, 20.89, 23.93, 16.69, 19.31, 22.31, 25.13, 16.31, 18.71, 21.53, 24.41, 17.33,
    20.11, 23.11, 26.33,
];
/// The share of the decay in which the tail falls by 60 dB at the high cut.
const HIGH_CUT_DECAY: f64 = 0.6;
/// The vintage character's darker share.
const VINTAGE_HIGH_CUT_DECAY: f64 = 0.4;
const BASS_DECAY_CORNER: f64 = 280.0;
const TREBLE_DECAY_CORNER: f64 = 4500.0;
/// The mix of the lines: a signed permutation of the Hadamard transform.
const PERM: [usize; LINES] = [3, 10, 5, 14, 1, 12, 7, 0, 11, 2, 15, 6, 9, 4, 13, 8];
const PSIGN: [f64; LINES] = [
    1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0,
];
/// The signs with which each side's input and output reach its lines.
const SIGN_IN: [f64; LINES / 2] = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
const SIGN_OUT: [[f64; LINES / 2]; 2] = [
    [1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0],
];
/// The energy-keeping share of the input and output of each line.
const LINE_SHARE: f64 = 0.353_553_390_593_273_8;
const RING_C1: f64 = 0.923_879_532_511_286_7;
const RING_S1: f64 = 0.382_683_432_365_089_8;
const RING_D1: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// The cosines and sines of the sixteen equal steps of a turn.
const RING_COS: [f64; LINES] = [
    1.0, RING_C1, RING_D1, RING_S1, 0.0, -RING_S1, -RING_D1, -RING_C1, -1.0, -RING_C1, -RING_D1,
    -RING_S1, 0.0, RING_S1, RING_D1, RING_C1,
];
const RING_SIN: [f64; LINES] = [
    0.0, RING_S1, RING_D1, RING_C1, 1.0, RING_C1, RING_D1, RING_S1, 0.0, -RING_S1, -RING_D1,
    -RING_C1, -1.0, -RING_C1, -RING_D1, -RING_S1,
];

/// The sine of a phase, moved on by `slot` sixteenths of a turn.
#[inline]
fn shifted_sin((sin_a, cos_a): (f64, f64), slot: usize) -> f64 {
    let i = slot & (LINES - 1);
    sin_a * RING_COS[i] + cos_a * RING_SIN[i]
}

/// A rational `tanh`, within 3e-5 up to 2.1 and held at 1 beyond 3.5.
#[inline]
fn fast_tanh(u: f64) -> f64 {
    if u.abs() >= 3.5 {
        return u.signum();
    }
    let u2 = u * u;
    u * (945.0 + u2 * (105.0 + u2)) / (945.0 + u2 * (420.0 + 15.0 * u2))
}

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

/// A fast Walsh-Hadamard transform of the lines, unit-gain.
fn hadamard(x: &mut [f64; LINES]) {
    let mut h = 1;
    while h < LINES {
        for start in (0..LINES).step_by(h * 2) {
            for j in start..start + h {
                let (u, v) = (x[j], x[j + h]);
                x[j] = u + v;
                x[j + h] = u - v;
            }
        }
        h *= 2;
    }
    x.iter_mut().for_each(|v| *v *= 0.25);
}

/// A set of early reflection taps for both sides: delays in frames and gains.
#[derive(Clone, Copy, Default, PartialEq)]
struct Taps {
    delays: [[usize; ER_TAPS]; 2],
    gains: [[f64; ER_TAPS]; 2],
    count: usize,
}
impl Taps {
    fn new(model: SpaceModel, size: f64, density: f64) -> Self {
        let scale = 0.4 + 0.6 * size_factor(size);
        let count = 4 + (12.0 * density).round() as usize;
        let mut taps = Self {
            count,
            ..Self::default()
        };
        for (side, (delays, gains)) in taps.delays.iter_mut().zip(&mut taps.gains).enumerate() {
            for (k, ms) in reflection_times(model).into_iter().enumerate().take(count) {
                let ms = if side == 0 { ms } else { ms * 1.19 + 3.1 };
                delays[k] = ((ms * scale / 1000.0 * RATE).round() as usize).max(1);
                let sign = if side == 0 {
                    if k % 2 == 1 { -1.0 } else { 1.0 }
                } else {
                    RIGHT_ER_SIGNS[k]
                };
                gains[k] = 0.86f64.powi(k as i32) * sign;
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
            lines[side].read_taps(
                &self.delays[side][..self.count],
                &self.gains[side][..self.count],
            )
        })
    }
}

/// A band of two high-pass and two low-pass poles. A pole pair at a cut
/// that does not touch the audible band stays out, so a wide-open tone is
/// flat to the top.
#[derive(Clone, Copy, Default)]
struct BandTone {
    high: [OnePole; 2],
    low: [OnePole; 2],
    use_high: bool,
    use_low: bool,
}
impl BandTone {
    fn tune(&mut self, low_cut: f64, high_cut: f64) {
        self.use_high = low_cut > 20.0;
        self.use_low = high_cut < 19_990.0;
        self.high.iter_mut().for_each(|pole| pole.tune(low_cut));
        self.low.iter_mut().for_each(|pole| pole.tune(high_cut));
    }
    fn clear(&mut self) {
        self.high
            .iter_mut()
            .chain(&mut self.low)
            .for_each(OnePole::clear);
    }
    #[inline]
    fn process(&mut self, input: f64) -> f64 {
        let mut x = input;
        if self.use_high {
            x = self.high[0].high(x);
            x = self.high[1].high(x);
        }
        if self.use_low {
            x = self.low[0].low(x);
            x = self.low[1].low(x);
        }
        x
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

/// Read `line` at `delay` frames: with a windowed sinc in an offline render.
#[inline]
fn read_line(line: &Line, delay: f64, quality: RenderQuality) -> f64 {
    match quality {
        RenderQuality::Live => line.read(delay),
        RenderQuality::Offline => line.read_sinc(delay),
    }
}

/// The pre-delay read. `current` is the frame that arrives now, so a delay
/// below two frames, even none, stays exact.
#[inline]
fn read_pre(line: &Line, current: f64, delay: f64, quality: RenderQuality) -> f64 {
    if delay >= 2.0 {
        return read_line(line, delay, quality);
    }
    let at = |k: usize| {
        if k == 0 {
            current
        } else {
            f64::from(line.at(k))
        }
    };
    let whole = delay.floor() as usize;
    let f = delay - whole as f64;
    at(whole) * (1.0 - f) + at(whole + 1) * f
}

/// The delay positions that a set of settings asks for.
#[derive(Clone, Copy, PartialEq)]
struct Positions {
    lines: [f64; LINES],
    pre: f64,
    taps: Taps,
}

/// The movement of the line read positions in one frame.
struct Swing {
    depth: f64,
    vintage: f64,
    wander: f64,
    waves: [(f64, f64); 3],
}

/// The decay of the bands of one setting.
struct Bands {
    decay: f64,
    share: f64,
    bass: f64,
    treble: f64,
}

pub(super) struct Core {
    activity: f64,
    pre: [Line; 2],
    pre_time: Retime,
    early: [Line; 2],
    taps: Taps,
    previous_taps: Taps,
    early_tone: [BandTone; 2],
    early_decor: [FixedAllPass; 2],
    diffusers: [[FixedAllPass; 4]; 2],
    bloom: [[AllPass; 4]; 2],
    lines: RingBank<LINES, FDN_CAP>,
    lengths: [Retime; LINES],
    gains: [f64; LINES],
    /// The low and high cut, once on the sound that enters the network.
    input_tone: [BandTone; 2],
    /// Each line's damping pole, and its last output.
    damping: [f64; LINES],
    damped: [f64; LINES],
    bass_shelves: ShelfBank<LINES>,
    treble_shelves: ShelfBank<LINES>,
    bass_shaped: bool,
    treble_shaped: bool,
    inner1: FixedAllPassBank<LINES>,
    inner2: FixedAllPassBank<LINES>,
    /// Frames left of the crossfade after a retime.
    fade: usize,
    /// Positions that arrived during a crossfade. They start their own
    /// crossfade when it ends, so the output never jumps from a half blend.
    queued: Option<Positions>,
    lfo: Lfo,
    lfo_b: Lfo,
    lfo_c: Lfo,
    wander: Lfo,
    shifter: Shifter,
    /// The tail's shifted frame, which feeds back as the shimmer.
    shimmer: [f64; 2],
    ratio: f64,
    target_ratio: f64,
    shimmer_send: f64,
    shimmer_level: Smoothed,
    /// The steps of the three modulation oscillators and the wander.
    lfo_steps: [u64; 4],
    /// The requested swing, before the active delay lengths bound it.
    requested_depth: f64,
    depth: Smoothed,
    diffusion: Smoothed,
    density: Smoothed,
    late_norm: Smoothed,
    early_gain: Smoothed,
    late_gain: Smoothed,
    width: Smoothed,
    output: Smoothed,
    drive: Smoothed,
    vintage: Smoothed,
    bloomy: Smoothed,
    high_cut: f64,
}

/// A delay line that holds `ms` milliseconds.
fn line_ms(ms: f64) -> Line {
    Line::new((ms / 1000.0 * RATE) as usize)
}

/// A setting that glides over 5 ms.
fn glide(value: f64) -> Smoothed {
    Smoothed::with_time(value, 0.005)
}

/// All-passes of the given lengths in milliseconds.
fn allpass_chain<const N: usize>(times: [f64; N]) -> [FixedAllPass; N] {
    times.map(|ms| FixedAllPass::new(ms / 1000.0))
}

/// All-passes of the given lengths whose read position can move.
fn bloom_chain(times: [f64; 4]) -> [AllPass; 4] {
    times.map(|ms| AllPass::with_modulation(ms / 1000.0, 12))
}

/// Build the shared tables before a processor reaches the audio thread.
pub(super) fn prepare_tables() {
    shifter::warm();
}

impl Core {
    pub(super) fn new() -> Self {
        let (line, smooth) = (line_ms, glide);
        let (chain, bloom) = (allpass_chain, bloom_chain);
        Self {
            activity: 0.0,
            pre: [line(260.0), line(260.0)],
            pre_time: Retime {
                length: 2.0,
                previous: 2.0,
            },
            early: [line(110.0), line(110.0)],
            taps: Taps::default(),
            previous_taps: Taps::default(),
            early_tone: [BandTone::default(); 2],
            early_decor: [FixedAllPass::new(0.0019), FixedAllPass::new(0.0043)],
            diffusers: [
                chain([4.77, 3.59, 12.73, 9.30]),
                chain([5.23, 4.19, 13.61, 10.13]),
            ],
            bloom: [bloom([4.3, 7.9, 12.7, 17.9]), bloom([4.9, 8.6, 13.4, 18.7])],
            // The hall's longest line plus the deepest modulation.
            lines: RingBank::new(),
            lengths: [Retime {
                length: 1000.0,
                previous: 1000.0,
            }; LINES],
            gains: [0.0; LINES],
            input_tone: [BandTone::default(); 2],
            damping: [0.0; LINES],
            damped: [0.0; LINES],
            bass_shelves: ShelfBank::new(),
            treble_shelves: ShelfBank::new(),
            bass_shaped: false,
            treble_shaped: false,
            inner1: FixedAllPassBank::new(INNER1_MS),
            inner2: FixedAllPassBank::new(INNER2_MS),
            fade: 0,
            queued: None,
            lfo: Lfo::default(),
            lfo_b: Lfo::default(),
            lfo_c: Lfo::default(),
            wander: Lfo::default(),
            shifter: Shifter::new(),
            shimmer: [0.0; 2],
            ratio: 2.0,
            target_ratio: 2.0,
            shimmer_send: 0.0,
            shimmer_level: Smoothed::with_time(0.0, 0.04),
            lfo_steps: [0, 0, 0, lfo_step(WANDER_HZ)],
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
            vintage: Smoothed::with_time(0.0, 0.02),
            bloomy: Smoothed::with_time(0.0, 0.02),
            high_cut: 20_000.0,
        }
    }
    /// Take new settings; the delay of the bus's safety limiter comes off
    /// the pre-delay.
    pub(super) fn set(&mut self, s: &Aetherform, compensation: f64) {
        let circulations = self.set_geometry(s, compensation);
        self.set_damping(s, &circulations);
        self.set_character(s);
        self.set_levels(s);
    }
    /// The delay lengths, their gains, and the early taps.
    fn set_geometry(&mut self, s: &Aetherform, compensation: f64) -> [f64; LINES] {
        let decay = f64::from(s.decay_seconds);
        let factor = size_factor(f64::from(s.size_percent));
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
            // A circulation also passes the line's inner all-passes.
            circulations[i] = frames / RATE + (INNER1_MS[i] + INNER2_MS[i]) / 1000.0;
            self.gains[i] = 10f64.powf(-3.0 * circulations[i] / decay);
            total += self.gains[i] * self.gains[i];
        }
        // Each circulation keeps g² of the energy: scale the tail so a
        // steady input leaves it near its own level, whatever the decay.
        self.late_norm
            .set(1.6 * (1.0 - total / LINES as f64).sqrt());
        let pre = (f64::from(s.pre_delay_ms) / 1000.0 * RATE - compensation).max(0.0);
        let density = f64::from(s.density_percent) / 100.0;
        let taps = Taps::new(s.space_model, f64::from(s.size_percent), density);
        if self.fade > 0 {
            let target = Positions { lines, pre, taps };
            self.queued = (target != self.positions()).then_some(target);
        } else {
            self.retime(Positions { lines, pre, taps });
        }
        circulations
    }
    /// The cuts, each line's damping, and the band shelves of the loop.
    fn set_damping(&mut self, s: &Aetherform, circulations: &[f64; LINES]) {
        let (low_cut, high_cut) = (f64::from(s.low_cut_hz), f64::from(s.high_cut_hz));
        self.high_cut = high_cut;
        self.input_tone
            .iter_mut()
            .chain(&mut self.early_tone)
            .for_each(|tone| tone.tune(low_cut, high_cut));
        let bands = Bands {
            decay: f64::from(s.decay_seconds),
            share: high_cut_share(s.character == ReverbCharacter::Vintage, high_cut),
            bass: f64::from(s.bass_mult),
            treble: f64::from(s.treble_mult),
        };
        let cosine = (2.0 * PI * high_cut.min(0.49 * RATE) / RATE).cos();
        self.bass_shaped = bands.bass != 1.0;
        self.treble_shaped = bands.treble != 1.0;
        for (i, &circulation) in circulations.iter().enumerate() {
            self.damping[i] = damping(circulation, bands.decay, bands.share, cosine);
            self.tune_shelves(i, circulation, &bands);
        }
    }
    /// Add the difference in loss per circulation for each band's decay
    /// time. The shelves affect the loop, not the input cuts, and never lift
    /// a line to unity gain.
    fn tune_shelves(&mut self, i: usize, circulation: f64, bands: &Bands) {
        let loss = 60.0 * circulation / bands.decay;
        let room = gain_to_db(0.9999 / self.gains[i]);
        if self.bass_shaped {
            let db = (loss * (1.0 - bands.bass.recip())).min(room);
            self.bass_shelves.tune_low(i, BASS_DECAY_CORNER, db);
        } else {
            self.bass_shelves.clear_line(i);
        }
        if self.treble_shaped {
            let db = (loss / bands.share * (1.0 - bands.treble.recip())).min(room);
            self.treble_shelves.tune_high(i, TREBLE_DECAY_CORNER, db);
        } else {
            self.treble_shelves.clear_line(i);
        }
    }
    /// The colour, the modulation, and the diffusion.
    fn set_character(&mut self, s: &Aetherform) {
        let bloom = s.character == ReverbCharacter::Bloom;
        self.vintage
            .set(f64::from(u8::from(s.character == ReverbCharacter::Vintage)));
        self.bloomy.set(f64::from(u8::from(bloom)));
        let rate = f64::from(s.mod_rate_hz) * if bloom { 0.6 } else { 1.0 };
        self.lfo_steps = [
            lfo_step(rate),
            lfo_step(1.29 * rate),
            lfo_step(0.73 * rate),
            lfo_step(WANDER_HZ),
        ];
        let depth = (f64::from(s.mod_depth_percent) / 100.0).powf(1.5) * 0.004 * RATE;
        self.requested_depth = depth * if bloom { 1.5 } else { 1.0 };
        self.update_depth();
        self.diffusion
            .set(0.3 + 0.45 * f64::from(s.diffusion_percent) / 100.0);
        self.density
            .set(0.1 + 0.5 * f64::from(s.density_percent) / 100.0);
    }
    /// The balance, the width, the output, and the shimmer.
    fn set_levels(&mut self, s: &Aetherform) {
        let balance = f64::from(s.late_balance_percent) / 100.0 * FRAC_PI_2;
        let early_on = f64::from(u8::from(s.early_reflections_enabled));
        self.early_gain.set(balance.cos() * early_on);
        self.late_gain.set(balance.sin());
        self.width.set(f64::from(s.width_percent) / 100.0);
        self.drive.set(f64::from(s.drive_percent) / 100.0);
        self.output.set(if s.bypass {
            0.0
        } else {
            db_to_gain(f64::from(s.output_db))
        });
        self.target_ratio = 2f64.powf(f64::from(s.shimmer_interval_semitones) / 12.0);
        let shimmer_send = if s.shimmer_enabled {
            0.55 * (f64::from(s.shimmer_amount_percent) / 100.0).powf(1.5)
        } else {
            0.0
        };
        if shimmer_send > 0.0 && self.shimmer_send == 0.0 && self.shimmer_level.value() == 0.0 {
            // A fully disabled shifter stopped reading the tail. Its old
            // audio must not return when shimmer is enabled again.
            self.shifter.clear();
        }
        self.shimmer_send = shimmer_send;
        self.shifter.tune(self.ratio, self.high_cut);
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
        self.shifter.tune(self.ratio, self.high_cut);
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
        self.lines.clear();
        self.early_tone
            .iter_mut()
            .chain(&mut self.input_tone)
            .for_each(BandTone::clear);
        self.damped = [0.0; LINES];
        self.bass_shelves.clear();
        self.treble_shelves.clear();
        self.early_decor.iter_mut().for_each(FixedAllPass::clear);
        self.diffusers
            .iter_mut()
            .flatten()
            .for_each(FixedAllPass::clear);
        self.bloom.iter_mut().flatten().for_each(AllPass::clear);
        self.inner1.clear();
        self.inner2.clear();
        self.shifter.clear();
        self.shimmer = [0.0; 2];
    }
    /// Move the modulation and the shimmer's hop grid on by `frames` frames
    /// that the bus slept through, so their phases do not depend on when
    /// the bus slept.
    pub(super) fn skip(&mut self, frames: u64, quality: RenderQuality) {
        let steps = self.lfo_steps;
        self.lfo.skip_step(steps[0], frames);
        self.lfo_b.skip_step(steps[1], frames);
        self.lfo_c.skip_step(steps[2], frames);
        self.wander.skip_step(steps[3], frames);
        if self.shimmer_level.value() > 0.0 || self.shimmer_send > 0.0 {
            self.shifter.skip(self.ratio, frames, quality);
        }
    }
    pub(super) fn activity(&self) -> f64 {
        self.activity
    }
    /// The crossfade weight of the new positions during a retime.
    #[inline]
    fn blend(&self) -> f64 {
        if self.fade == 0 {
            return 1.0;
        }
        let progress = 1.0 - self.fade as f64 / FADE as f64;
        0.5 - 0.5 * (PI * progress).cos()
    }
    /// The oscillator that moves line `i`: three that run at unrelated rates.
    #[inline]
    fn line_phase(waves: &[(f64, f64); 3], i: usize) -> f64 {
        0.5 * shifted_sin(waves[0], i)
            + 0.3 * shifted_sin(waves[1], i * 5)
            + 0.2 * shifted_sin(waves[2], i * 11)
    }
    /// The pre-delayed sound of both sides. It also fills the early lines.
    #[inline]
    fn read_pre_delay(&mut self, input: [f64; 2], weight: f64, quality: RenderQuality) -> [f64; 2] {
        let fading = self.fade > 0;
        let mut pre = [0.0; 2];
        for side in 0..2 {
            let line = &self.pre[side];
            let now = read_pre(line, input[side], self.pre_time.length, quality);
            pre[side] = if fading {
                let before = read_pre(line, input[side], self.pre_time.previous, quality);
                before + (now - before) * weight
            } else {
                now
            };
            self.pre[side].push(input[side] as f32);
            self.early[side].push(pre[side] as f32);
        }
        pre
    }
    /// The early reflections, decorrelated and filtered once by the tone.
    #[inline]
    fn read_early(&mut self, weight: f64) -> [f64; 2] {
        let mut early = self.taps.read(&self.early);
        if self.fade > 0 {
            let before = self.previous_taps.read(&self.early);
            for (now, before) in early.iter_mut().zip(before) {
                *now = before + (*now - before) * weight;
            }
        }
        let gains = [0.55, -0.55];
        for ((sample, tone), (decor, gain)) in early
            .iter_mut()
            .zip(&mut self.early_tone)
            .zip(self.early_decor.iter_mut().zip(gains))
        {
            *sample = tone.process(decor.process(*sample, gain));
        }
        early
    }
    /// Input diffusion, the bloom's smear, the shimmer's return, and the cuts.
    #[inline]
    fn diffuse(&mut self, pre: [f64; 2]) -> [f64; 2] {
        let diffusion = self.diffusion.next();
        let bloom = self.bloomy.next();
        let shimmer = self.shimmer_level.next();
        let mut feed = [0.0; 2];
        for side in 0..2 {
            // The shimmer's return passes the diffusers and the cuts too:
            // each pass climbs an octave, and only the high cut keeps that
            // climb from running away.
            let mut x = pre[side] + shimmer * self.shimmer[side];
            for diffuser in &mut self.diffusers[side] {
                x = diffuser.process(x, diffusion);
            }
            if bloom > 0.0 {
                x += (self.smear(side, x) - x) * bloom;
            }
            feed[side] = self.input_tone[side].process(x);
        }
        feed
    }
    /// The bloom's four short, slowly moving all-passes.
    #[inline]
    fn smear(&mut self, side: usize, input: f64) -> f64 {
        let mut x = input;
        let (w0, w1) = (self.lfo.sin_cos(), self.lfo_b.sin_cos());
        for (stage, pass) in self.bloom[side].iter_mut().enumerate() {
            let slot = stage * 4 + side * 2;
            let wave = shifted_sin(if stage % 2 == 0 { w0 } else { w1 }, slot);
            x = pass.process_modulated(x, 0.62, 4.0 * wave);
        }
        x
    }
    /// The raw outputs of the lines, each moved by the swing, with the old
    /// and new positions blended during a retime.
    #[inline]
    fn read_heads(&self, swing: &Swing, weight: f64, quality: RenderQuality) -> [f64; LINES] {
        if self.fade == 0 && quality == RenderQuality::Live {
            return std::array::from_fn(|i| {
                let phase = Self::line_phase(&swing.waves, i);
                let frames = self.lengths[i].length;
                let at = modulated_length(frames, swing.depth, phase, swing.vintage, swing.wander);
                self.lines.read_cubic(i, at)
            });
        }
        let fading = self.fade > 0;
        let mut taps = [0.0; LINES];
        for (i, tap) in taps.iter_mut().enumerate() {
            let phase = Self::line_phase(&swing.waves, i);
            let length = self.lengths[i];
            let head = |frames: f64| {
                let at = modulated_length(frames, swing.depth, phase, swing.vintage, swing.wander);
                match quality {
                    RenderQuality::Live => self.lines.read_cubic(i, at),
                    RenderQuality::Offline => self.lines.read_sinc(i, at),
                }
            };
            let now = head(length.length);
            *tap = if fading {
                let before = head(length.previous);
                before + (now - before) * weight
            } else {
                now
            };
        }
        taps
    }
    /// The outputs of the lines, damped and coloured, through both inner
    /// all-passes.
    #[inline]
    fn read_taps(&mut self, weight: f64, quality: RenderQuality) -> [f64; LINES] {
        let depth = self.depth.next();
        let vintage = self.vintage.next();
        let swing = Swing {
            depth,
            vintage,
            wander: if vintage > 0.0 && depth > 0.0 {
                self.wander.sin()
            } else {
                0.0
            },
            waves: if depth > 0.0 {
                [
                    self.lfo.sin_cos(),
                    self.lfo_b.sin_cos(),
                    self.lfo_c.sin_cos(),
                ]
            } else {
                [(0.0, 1.0); 3]
            },
        };
        let density = self.density.next();
        let mut taps = self.read_heads(&swing, weight, quality);
        for (tap, (&pole, damped)) in taps
            .iter_mut()
            .zip(self.damping.iter().zip(&mut self.damped))
        {
            *tap = flush((1.0 - pole) * *tap + pole * *damped);
            *damped = *tap;
        }
        if vintage > 0.0 {
            for x in &mut taps {
                *x += (fast_tanh(1.4 * *x) / 1.4 - *x) * vintage;
            }
        }
        self.inner1.process_all(&mut taps, density);
        self.inner2.process_all(&mut taps, 0.75 * density);
        taps
    }
    /// Mix the lines, feed them, and give the tail on each side.
    #[inline]
    fn step_network(&mut self, taps: [f64; LINES], feed: [f64; 2]) -> [f64; 2] {
        let steps = self.lfo_steps;
        self.lfo.advance_step(steps[0]);
        self.lfo_b.advance_step(steps[1]);
        self.lfo_c.advance_step(steps[2]);
        self.wander.advance_step(steps[3]);
        let norm = self.late_norm.next();
        let mut late = [0.0; 2];
        for (i, tap) in taps.iter().enumerate() {
            late[i % 2] += tap * SIGN_OUT[i % 2][i / 2] * LINE_SHARE * norm;
        }
        let mut mixed = taps;
        hadamard(&mut mixed);
        let mut feedback: [f64; LINES] =
            std::array::from_fn(|i| self.gains[i] * mixed[PERM[i]] * PSIGN[i]);
        if self.bass_shaped {
            self.bass_shelves.process_low(&mut feedback);
        }
        if self.treble_shaped {
            self.treble_shelves.process_high(&mut feedback);
        }
        let next: [f32; LINES] = std::array::from_fn(|i| {
            let injected = feed[i % 2] * SIGN_IN[i / 2] * LINE_SHARE;
            bound(feedback[i] + injected) as f32
        });
        self.lines.push(next);
        late
    }
    /// The shimmer: an interval change waits until the old one has faded.
    #[inline]
    fn update_shimmer(&mut self, late: [f64; 2], quality: RenderQuality) {
        let on = self.shimmer_level.value() > 0.0 || self.shimmer_send > 0.0;
        if self.ratio != self.target_ratio {
            self.shimmer_level.set(0.0);
            if self.shimmer_level.value() < 1e-4 {
                self.ratio = self.target_ratio;
                self.shifter.clear();
                self.shifter.tune(self.ratio, self.high_cut);
            }
        } else {
            self.shimmer_level.set(self.shimmer_send);
        }
        self.shimmer = if on {
            self.shifter.process_pair(late, self.ratio, quality)
        } else {
            [0.0; 2]
        };
    }
    #[inline]
    fn finish_fade(&mut self) {
        if self.fade == 0 {
            return;
        }
        self.fade -= 1;
        if self.fade == 0 {
            if let Some(target) = self.queued.take() {
                self.retime(target);
            }
            self.update_depth();
        }
    }
    #[inline]
    pub(super) fn process_with_quality(
        &mut self,
        left: f64,
        right: f64,
        quality: RenderQuality,
    ) -> (f64, f64) {
        let weight = self.blend();
        let amount = self.drive.next();
        let input = [drive(left, amount), drive(right, amount)];
        let pre = self.read_pre_delay(input, weight, quality);
        let early = self.read_early(weight);
        let feed = self.diffuse(pre);
        let taps = self.read_taps(weight, quality);
        self.activity = taps
            .iter()
            .chain(&pre)
            .chain(&early)
            .chain(&self.shimmer)
            .fold(0.0_f64, |peak, sample| peak.max(sample.abs()));
        let late = self.step_network(taps, feed);
        self.update_shimmer(late, quality);
        self.finish_fade();
        let (early_gain, late_gain) = (self.early_gain.next(), self.late_gain.next());
        let (l, r) = widen(
            early[0] * early_gain + late[0] * late_gain,
            early[1] * early_gain + late[1] * late_gain,
            self.width.next(),
        );
        let level = self.output.next();
        (l * level, r * level)
    }
    /// An offline frame, as the tests hear the reverb.
    #[cfg(test)]
    pub(super) fn process(&mut self, left: f64, right: f64) -> (f64, f64) {
        self.process_with_quality(left, right, RenderQuality::Offline)
    }
}

#[cfg(test)]
mod tests;
