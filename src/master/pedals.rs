//! The mastering pedals, designed for this application. Each works on
//! normalized stereo floats with its own state, and each continuous setting
//! glides to a new value. Pedals with a curve run it four times oversampled
//! and so delay the signal by `Oversampler::LATENCY`; the limiter delays it
//! by its lookahead.
use super::dsp::{
    Biquad, Coefficients, DcBlocker, Delay, Oversampler, Smoothed, TruePeak, db_to_gain, flush,
    gain_to_db, pole,
};
use crate::master as settings;
use std::f64::consts::FRAC_1_SQRT_2;

/// How often a pedal whose filters glide recomputes their coefficients.
const REFRESH: u32 = 16;

/// Counts down the frames until a gliding pedal next redesigns its filters.
#[derive(Clone, Copy)]
struct Refresh(u32);
impl Refresh {
    /// True when the filters need new coefficients: every `REFRESH` frames
    /// while a setting glides, and once more when it arrives.
    fn due(&mut self, gliding: bool, arrived: bool) -> bool {
        if !gliding {
            return false;
        }
        self.0 = self.0.saturating_sub(1);
        if self.0 == 0 || arrived {
            self.0 = REFRESH;
            return true;
        }
        false
    }
}

/// Gain reduction in dB of a soft-knee compressor for a level in dB.
fn reduction(level: f64, threshold: f64, ratio: f64, knee: f64) -> f64 {
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

/// Unity below `knee`. Above it the curve approaches `ceiling` as tanh
/// does, with the same slope and curvature where it starts to bend. A knee
/// at the ceiling clips there.
fn soft_clip(sample: f32, knee: f32, ceiling: f32) -> f32 {
    let magnitude = sample.abs();
    if magnitude <= knee {
        return sample;
    }
    let room = ceiling - knee;
    let shaped = if room > 1e-6 {
        knee + room * ((magnitude - knee) / room).tanh()
    } else {
        ceiling
    };
    shaped.copysign(sample)
}

// Soft clip -------------------------------------------------------------------------

/// Rounds peaks off below a ceiling. The amount sets how far below the
/// ceiling the curve starts to bend.
#[derive(Clone, Copy)]
pub(super) struct SoftClip {
    oversamplers: [Oversampler; 2],
    knee: Smoothed,
    ceiling: Smoothed,
}
impl SoftClip {
    pub(super) const LATENCY: usize = Oversampler::LATENCY;

    pub(super) fn new() -> Self {
        Self {
            oversamplers: [Oversampler::ZERO; 2],
            knee: Smoothed::new(1.0),
            ceiling: Smoothed::new(1.0),
        }
    }
    pub(super) fn set(&mut self, settings: &settings::SoftClip) {
        let ceiling = f64::from(settings.ceiling);
        self.ceiling.set(db_to_gain(ceiling));
        self.knee
            .set(db_to_gain(ceiling - f64::from(settings.amount)));
    }
    pub(super) fn clear(&mut self) {
        self.oversamplers = [Oversampler::ZERO; 2];
        self.knee.settle();
        self.ceiling.settle();
    }
    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let (knee, ceiling) = (self.knee.next() as f32, self.ceiling.next() as f32);
        let curve = |sample| soft_clip(sample, knee, ceiling);
        (
            self.oversamplers[0].process(left, curve),
            self.oversamplers[1].process(right, curve),
        )
    }
}

// Tube ----------------------------------------------------------------------------------

/// Bias of the tube curve: how much sooner positive swings flatten.
const TUBE_BIAS: f32 = 0.3;

/// `tanh(g x) / (g (1 + b tanh(g x)))`. Positive swings flatten sooner than
/// negative ones, which gives mostly even harmonics, and dividing by the
/// drive keeps quiet passages at their level. At zero drive it is the
/// identity.
fn tube(sample: f32, drive: f32) -> f32 {
    if drive < 1e-4 {
        return sample;
    }
    let shaped = (drive * sample).tanh();
    shaped / (drive * (1.0 + TUBE_BIAS * shaped))
}

/// Asymmetric saturation, blended with the dry signal. The asymmetry
/// leaves an offset that a 5 Hz high-pass removes.
#[derive(Clone, Copy)]
pub(super) struct Tube {
    oversamplers: [Oversampler; 2],
    dry: [Delay<{ Oversampler::LATENCY }>; 2],
    blockers: [DcBlocker; 2],
    /// The drive control, 0 to 10; the curve's gain is `4 (drive / 10)²`.
    drive: Smoothed,
    /// The wet share, 0 to 1.
    mix: Smoothed,
    blocker_pole: f64,
}
impl Tube {
    pub(super) const LATENCY: usize = Oversampler::LATENCY;

    pub(super) fn new() -> Self {
        Self {
            oversamplers: [Oversampler::ZERO; 2],
            dry: [Delay::ZERO; 2],
            blockers: [DcBlocker::ZERO; 2],
            drive: Smoothed::new(0.0),
            mix: Smoothed::new(1.0),
            blocker_pole: pole(1.0 / (2.0 * std::f64::consts::PI * 5.0)),
        }
    }
    pub(super) fn set(&mut self, settings: &settings::Tube) {
        self.drive.set(f64::from(settings.drive));
        self.mix.set(f64::from(settings.mix) / 100.0);
    }
    pub(super) fn clear(&mut self) {
        self.oversamplers = [Oversampler::ZERO; 2];
        self.dry = [Delay::ZERO; 2];
        self.blockers = [DcBlocker::ZERO; 2];
        self.drive.settle();
        self.mix.settle();
    }
    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let drive = self.drive.next() / 10.0;
        let gain = (4.0 * drive * drive) as f32;
        let mix = self.mix.next() as f32;
        let mut channel = |index: usize, sample: f32| {
            let wet = self.oversamplers[index].process(sample, |x| tube(x, gain));
            let wet = self.blockers[index].process(f64::from(wet), self.blocker_pole) as f32;
            let dry = self.dry[index].push(sample);
            dry + (wet - dry) * mix
        };
        (channel(0, left), channel(1, right))
    }
}

// Trim EQ -------------------------------------------------------------------------------

const MUD: (f64, f64) = (250.0, 2.5);
const HARSH: (f64, f64) = (3500.0, 3.0);

/// Cleanup: a 12 dB per octave Butterworth high-pass, and narrow cuts at
/// 250 Hz and 3.5 kHz.
#[derive(Clone, Copy)]
pub(super) struct TrimEq {
    filters: [Biquad; 3],
    high_pass: Smoothed,
    mud: Smoothed,
    harsh: Smoothed,
    refresh: Refresh,
}
impl TrimEq {
    pub(super) fn new() -> Self {
        Self {
            filters: [Biquad::FLAT; 3],
            high_pass: Smoothed::new(20.0),
            mud: Smoothed::new(0.0),
            harsh: Smoothed::new(0.0),
            refresh: Refresh(REFRESH),
        }
    }
    pub(super) fn set(&mut self, settings: &settings::TrimEq) {
        self.high_pass.set(f64::from(settings.high_pass));
        self.mud.set(f64::from(settings.mud));
        self.harsh.set(f64::from(settings.harsh));
    }
    fn design(&mut self) {
        self.filters[0].set(Coefficients::high_pass(
            self.high_pass.value(),
            FRAC_1_SQRT_2,
        ));
        self.filters[1].set(Coefficients::peaking(MUD.0, MUD.1, self.mud.value()));
        self.filters[2].set(Coefficients::peaking(HARSH.0, HARSH.1, self.harsh.value()));
    }
    pub(super) fn clear(&mut self) {
        self.filters.iter_mut().for_each(Biquad::clear);
        for setting in [&mut self.high_pass, &mut self.mud, &mut self.harsh] {
            setting.settle();
        }
        self.design();
    }
    fn settled(&self) -> bool {
        self.high_pass.settled() && self.mud.settled() && self.harsh.settled()
    }
    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let gliding = !self.settled();
        if gliding {
            for setting in [&mut self.high_pass, &mut self.mud, &mut self.harsh] {
                setting.next();
            }
        }
        if self.refresh.due(gliding, self.settled()) {
            self.design();
        }
        let mut samples = [f64::from(left), f64::from(right)];
        for filter in &mut self.filters {
            for (channel, sample) in samples.iter_mut().enumerate() {
                *sample = filter.process(channel, *sample);
            }
        }
        (samples[0] as f32, samples[1] as f32)
    }
}

// Bus compressor ----------------------------------------------------------------------

/// The time constant of the compressors' power detectors.
const DETECTOR: f64 = 0.003;
const KNEE_DB: f64 = 6.0;

/// A feed-forward compressor on the stereo mix. One detector follows the
/// mean power of both channels, so both get the same gain; a soft knee
/// shapes the reduction, and attack and release smooth it. No makeup gain.
#[derive(Clone, Copy)]
pub(super) struct BusCompressor {
    threshold: Smoothed,
    ratio: Smoothed,
    attack: f64,
    release: f64,
    detector: f64,
    power: f64,
    /// Current gain reduction in dB.
    reduction: f64,
    /// The most reduction since `take_reduction`, in dB.
    most: f64,
}
impl BusCompressor {
    pub(super) fn new() -> Self {
        Self {
            threshold: Smoothed::new(0.0),
            ratio: Smoothed::new(1.0),
            attack: 0.0,
            release: 0.0,
            detector: pole(DETECTOR),
            power: 0.0,
            reduction: 0.0,
            most: 0.0,
        }
    }
    pub(super) fn set(&mut self, settings: &settings::BusCompressor) {
        self.threshold.set(f64::from(settings.threshold));
        self.ratio.set(f64::from(settings.ratio));
        self.attack = pole(f64::from(settings.attack) / 1000.0);
        self.release = pole(f64::from(settings.release) / 1000.0);
    }
    pub(super) fn clear(&mut self) {
        (self.power, self.reduction, self.most) = (0.0, 0.0, 0.0);
        self.threshold.settle();
        self.ratio.settle();
    }
    pub(super) fn take_reduction(&mut self) -> f32 {
        std::mem::take(&mut self.most) as f32
    }
    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let (threshold, ratio) = (self.threshold.next(), self.ratio.next());
        let (l, r) = (f64::from(left), f64::from(right));
        let power = 0.5 * (l * l + r * r);
        self.power = flush(power + (self.power - power) * self.detector);
        let level = 10.0 * self.power.max(1e-12).log10();
        let target = reduction(level, threshold, ratio, KNEE_DB);
        let speed = if target > self.reduction {
            self.attack
        } else {
            self.release
        };
        self.reduction = flush(target + (self.reduction - target) * speed);
        if self.reduction == 0.0 {
            return (left, right);
        }
        self.most = self.most.max(self.reduction);
        let gain = db_to_gain(-self.reduction);
        ((l * gain) as f32, (r * gain) as f32)
    }
}

// Maximizer ---------------------------------------------------------------------------------

/// The maximizer's output ceiling, in dBFS.
const MAXIMIZER_CEILING_DB: f64 = -1.0;

/// Raises the level by a quarter of a decibel per percent of boost into a
/// soft clipper with a fixed ceiling, so the sound gets louder while its
/// peaks stay put. The curve bends from 1 dB plus the boost below the
/// ceiling.
#[derive(Clone, Copy)]
pub(super) struct Maximizer {
    oversamplers: [Oversampler; 2],
    drive: Smoothed,
    knee: Smoothed,
}
impl Maximizer {
    pub(super) const LATENCY: usize = Oversampler::LATENCY;

    pub(super) fn new() -> Self {
        Self {
            oversamplers: [Oversampler::ZERO; 2],
            drive: Smoothed::new(1.0),
            knee: Smoothed::new(1.0),
        }
    }
    pub(super) fn set(&mut self, settings: &settings::Maximizer) {
        let drive = 0.25 * f64::from(settings.boost);
        self.drive.set(db_to_gain(drive));
        self.knee
            .set(db_to_gain(MAXIMIZER_CEILING_DB - 1.0 - drive));
    }
    pub(super) fn clear(&mut self) {
        self.oversamplers = [Oversampler::ZERO; 2];
        self.drive.settle();
        self.knee.settle();
    }
    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let (drive, knee) = (self.drive.next() as f32, self.knee.next() as f32);
        let ceiling = db_to_gain(MAXIMIZER_CEILING_DB) as f32;
        let curve = |sample| soft_clip(sample, knee, ceiling);
        (
            self.oversamplers[0].process(left * drive, curve),
            self.oversamplers[1].process(right * drive, curve),
        )
    }
}

// Lift EQ -------------------------------------------------------------------------------------

const LOW_SHELF_HZ: f64 = 90.0;
const AIR_SHELF_HZ: f64 = 12_000.0;
const LIFT_SLOPE: f64 = 0.6;

/// Wide, gentle shelves that add weight at 90 Hz and air at 12 kHz.
#[derive(Clone, Copy)]
pub(super) struct LiftEq {
    filters: [Biquad; 2],
    low: Smoothed,
    air: Smoothed,
    refresh: Refresh,
}
impl LiftEq {
    pub(super) fn new() -> Self {
        Self {
            filters: [Biquad::FLAT; 2],
            low: Smoothed::new(0.0),
            air: Smoothed::new(0.0),
            refresh: Refresh(REFRESH),
        }
    }
    pub(super) fn set(&mut self, settings: &settings::LiftEq) {
        self.low.set(f64::from(settings.low));
        self.air.set(f64::from(settings.air));
    }
    fn design(&mut self) {
        self.filters[0].set(Coefficients::low_shelf(
            LOW_SHELF_HZ,
            LIFT_SLOPE,
            self.low.value(),
        ));
        self.filters[1].set(Coefficients::high_shelf(
            AIR_SHELF_HZ,
            LIFT_SLOPE,
            self.air.value(),
        ));
    }
    pub(super) fn clear(&mut self) {
        self.filters.iter_mut().for_each(Biquad::clear);
        self.low.settle();
        self.air.settle();
        self.design();
    }
    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let gliding = !(self.low.settled() && self.air.settled());
        if gliding {
            self.low.next();
            self.air.next();
        }
        if self
            .refresh
            .due(gliding, self.low.settled() && self.air.settled())
        {
            self.design();
        }
        let mut samples = [f64::from(left), f64::from(right)];
        for filter in &mut self.filters {
            for (channel, sample) in samples.iter_mut().enumerate() {
                *sample = filter.process(channel, *sample);
            }
        }
        (samples[0] as f32, samples[1] as f32)
    }
}

// Tape ----------------------------------------------------------------------------------------

/// Emphasis before the tape curve and its inverse after it, so treble
/// saturates sooner than bass, as on tape.
const EMPHASIS: (f64, f64, f64) = (3000.0, 0.7, 6.0);
/// Head bump centres at 15 and 30 inches per second.
const BUMP_HZ: [f64; 2] = [55.0, 35.0];
/// Treble roll-off corners at 15 and 30 inches per second.
const ROLL_OFF_HZ: [f64; 2] = [10_000.0, 16_000.0];

/// `tanh(g x) / g`: symmetric, so it adds odd harmonics, at unity gain for
/// quiet passages.
fn tape(sample: f32, drive: f32) -> f32 {
    if drive < 1e-4 {
        return sample;
    }
    (drive * sample).tanh() / drive
}

/// Tape saturation with pre- and de-emphasis. The drive also sets how much
/// low-end head bump and treble roll-off the tape adds; speed moves both.
/// At zero drive the tape adds nothing.
#[derive(Clone, Copy)]
pub(super) struct Tape {
    oversamplers: [Oversampler; 2],
    /// Emphasis, de-emphasis, head bump, and roll-off.
    filters: [Biquad; 4],
    drive: Smoothed,
    /// From 0 at 15 inches per second to 1 at 30.
    speed: Smoothed,
    refresh: Refresh,
}
impl Tape {
    pub(super) const LATENCY: usize = Oversampler::LATENCY;

    pub(super) fn new() -> Self {
        let (hz, slope, db) = EMPHASIS;
        Self {
            oversamplers: [Oversampler::ZERO; 2],
            filters: [
                Biquad::new(Coefficients::high_shelf(hz, slope, db)),
                Biquad::new(Coefficients::high_shelf(hz, slope, -db)),
                Biquad::FLAT,
                Biquad::FLAT,
            ],
            drive: Smoothed::new(0.0),
            speed: Smoothed::new(1.0),
            refresh: Refresh(REFRESH),
        }
    }
    pub(super) fn set(&mut self, settings: &settings::Tape) {
        self.drive.set(f64::from(settings.drive));
        self.speed.set(if settings.speed == 15 { 0.0 } else { 1.0 });
    }
    fn design(&mut self) {
        let color = (self.drive.value() / 10.0).max(0.0).sqrt();
        let speed = self.speed.value();
        let between = |[slow, fast]: [f64; 2]| slow + (fast - slow) * speed;
        self.filters[2].set(Coefficients::peaking(between(BUMP_HZ), 1.0, 2.0 * color));
        self.filters[3].set(Coefficients::high_shelf(
            between(ROLL_OFF_HZ),
            0.7,
            -3.0 * color,
        ));
    }
    pub(super) fn clear(&mut self) {
        self.oversamplers = [Oversampler::ZERO; 2];
        self.filters.iter_mut().for_each(Biquad::clear);
        self.drive.settle();
        self.speed.settle();
        self.design();
    }
    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let gliding = !(self.drive.settled() && self.speed.settled());
        if gliding {
            self.drive.next();
            self.speed.next();
        }
        if self
            .refresh
            .due(gliding, self.drive.settled() && self.speed.settled())
        {
            self.design();
        }
        let drive = self.drive.value() / 10.0;
        let gain = (5.0 * drive * drive) as f32;
        let mut channel = |index: usize, sample: f32| {
            let emphasized = self.filters[0].process(index, f64::from(sample)) as f32;
            let saturated = self.oversamplers[index].process(emphasized, |x| tape(x, gain));
            let mut sample = f64::from(saturated);
            for filter in &mut self.filters[1..] {
                sample = filter.process(index, sample);
            }
            sample as f32
        };
        (channel(0, left), channel(1, right))
    }
}

// Stereo imaging ------------------------------------------------------------------------------

/// Mid and side: the mid passes untouched. The side loses everything below
/// the mono frequency through a 24 dB per octave Linkwitz-Riley high-pass,
/// and the width scales the rest. The two channels always sum to twice the
/// mid, so mono playback never changes.
#[derive(Clone, Copy)]
pub(super) struct Imaging {
    side: [Biquad; 2],
    width: Smoothed,
    mono_below: Smoothed,
    refresh: Refresh,
}
impl Imaging {
    pub(super) fn new() -> Self {
        Self {
            side: [Biquad::FLAT; 2],
            width: Smoothed::new(1.0),
            mono_below: Smoothed::new(120.0),
            refresh: Refresh(REFRESH),
        }
    }
    pub(super) fn set(&mut self, settings: &settings::Imaging) {
        self.width.set(f64::from(settings.width) / 100.0);
        self.mono_below.set(f64::from(settings.mono_below));
    }
    fn design(&mut self) {
        let coefficients = Coefficients::high_pass(self.mono_below.value(), FRAC_1_SQRT_2);
        self.side.iter_mut().for_each(|f| f.set(coefficients));
    }
    pub(super) fn clear(&mut self) {
        self.side.iter_mut().for_each(Biquad::clear);
        self.width.settle();
        self.mono_below.settle();
        self.design();
    }
    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let gliding = !self.mono_below.settled();
        if gliding {
            self.mono_below.next();
        }
        if self.refresh.due(gliding, self.mono_below.settled()) {
            self.design();
        }
        let width = self.width.next();
        let mid = 0.5 * (left + right);
        let side = self.side[0].process(0, f64::from(0.5 * (left - right)));
        let side = (self.side[1].process(0, side) * width) as f32;
        (mid + side, mid - side)
    }
}

// Multiband ------------------------------------------------------------------------------------

/// The crossovers between the low, mid, and high bands.
const CROSSOVERS: [f64; 2] = [200.0, 3000.0];
/// How far each limiter looks ahead: 2 ms, so the gain is already down
/// when a peak arrives.
const BAND_LOOKAHEAD: usize = 88;
/// How long each band keeps the gain a peak needs, in frames, and how fast
/// it recovers after, in seconds. The low band holds 10 ms, so a bass wave
/// is not reshaped within its cycle; the mids and highs hold 3.3 and 2.2 ms.
const BAND_TIMES: [(usize, f64); 3] = [(441, 0.045), (146, 0.03), (96, 0.03)];
/// The output stage holds 3.2 ms and recovers over 60 ms.
const OUTPUT_TIMES: (usize, f64) = (141, 0.06);
/// The longest span over which a limiter takes the lowest need.
const SPAN: usize = BAND_LOOKAHEAD + 1 + BAND_TIMES[0].0;
/// How long a recent peak lasts, in seconds. Ceilings sit at recent peaks,
/// so they follow each section of a song and a breakdown is limited as
/// hard as a drop.
const PEAK_FALL: f64 = 1.5;
/// At the largest amount, a band is driven 12 dB into a ceiling 6 dB above
/// its recent peak.
const DRIVE_DB: f64 = 12.0;
const ROOM_DB: f64 = 6.0;
/// The share of the limited band in the output; the rest is the band as it
/// came in.
const WET: f64 = 0.6;

/// The smallest of the last `span` values pushed, `span` at most `N`: a
/// queue of the values that can still become the smallest.
#[derive(Clone, Copy)]
struct MovingMin<const N: usize> {
    values: [f64; N],
    times: [u32; N],
    head: usize,
    len: usize,
    now: u32,
}
impl<const N: usize> MovingMin<N> {
    const EMPTY: Self = Self {
        values: [0.0; N],
        times: [0; N],
        head: 0,
        len: 0,
        now: 0,
    };

    fn push(&mut self, value: f64, span: usize) -> f64 {
        while self.len > 0 && self.values[(self.head + self.len - 1) % N] >= value {
            self.len -= 1;
        }
        let at = (self.head + self.len) % N;
        (self.values[at], self.times[at]) = (value, self.now);
        self.len += 1;
        if self.now.wrapping_sub(self.times[self.head]) as usize >= span {
            self.head = (self.head + 1) % N;
            self.len -= 1;
        }
        self.now = self.now.wrapping_add(1);
        self.values[self.head]
    }
}

/// A stereo-linked lookahead limiter on sample peaks. Each frame needs the
/// gain that brings it under the threshold; the gain takes the lowest need
/// within the hold, recovers after it, and ramps over the lookahead.
#[derive(Clone, Copy)]
struct PeakLimiter {
    delays: [Delay<BAND_LOOKAHEAD>; 2],
    needs: MovingMin<SPAN>,
    span: usize,
    held: f64,
    ramp: [f64; BAND_LOOKAHEAD],
    ramp_at: usize,
    ramp_sum: f64,
    release: f64,
}
impl PeakLimiter {
    fn new((hold, release): (usize, f64)) -> Self {
        Self {
            delays: [Delay::ZERO; 2],
            needs: MovingMin::EMPTY,
            span: BAND_LOOKAHEAD + 1 + hold,
            held: 1.0,
            ramp: [1.0; BAND_LOOKAHEAD],
            ramp_at: 0,
            ramp_sum: BAND_LOOKAHEAD as f64,
            release: pole(release),
        }
    }
    fn clear(&mut self) {
        let (span, release) = (self.span, self.release);
        *self = Self::new((0, 1.0));
        (self.span, self.release) = (span, release);
    }
    /// The frame `BAND_LOOKAHEAD` ago and the gain it gets.
    fn process(&mut self, left: f64, right: f64, threshold: f64) -> ([f64; 2], f64) {
        let level = left.abs().max(right.abs());
        let need = if level > threshold {
            threshold / level
        } else {
            1.0
        };
        let lowest = self.needs.push(need, self.span);
        self.held = if lowest < self.held {
            lowest
        } else {
            flush(lowest + (self.held - lowest) * self.release)
        };
        self.ramp_sum += self.held - self.ramp[self.ramp_at];
        self.ramp[self.ramp_at] = self.held;
        self.ramp_at += 1;
        if self.ramp_at == BAND_LOOKAHEAD {
            // Recount now and then, so rounding cannot build up.
            self.ramp_at = 0;
            self.ramp_sum = self.ramp.iter().sum();
        }
        let delayed = [
            f64::from(self.delays[0].push(left as f32)),
            f64::from(self.delays[1].push(right as f32)),
        ];
        (delayed, (self.ramp_sum / BAND_LOOKAHEAD as f64).min(1.0))
    }
}

/// One band driven into a ceiling above its own recent peak and blended
/// with the band as it came in. The drive lifts the quieter body of the
/// band, and its peaks rise at most by the room.
#[derive(Clone, Copy)]
struct Band {
    limiter: PeakLimiter,
    peak: f64,
    amount: f64,
    drive: f64,
    room: f64,
    wet: f64,
}
impl Band {
    fn new(times: (usize, f64)) -> Self {
        Self {
            limiter: PeakLimiter::new(times),
            peak: 0.0,
            amount: 0.0,
            drive: 1.0,
            room: 1.0,
            wet: 0.0,
        }
    }
    fn clear(&mut self) {
        self.limiter.clear();
        self.peak = 0.0;
    }
    /// Below a tenth, the amount fades the blend in rather than the drive,
    /// so none at all leaves the band as it came in.
    fn set_amount(&mut self, amount: f64) {
        if amount != self.amount {
            self.amount = amount;
            self.drive = db_to_gain(DRIVE_DB * amount);
            self.room = db_to_gain(ROOM_DB * amount);
            self.wet = WET * (10.0 * amount).min(1.0);
        }
    }
    fn process(&mut self, left: f64, right: f64, fall: f64) -> [f64; 2] {
        self.peak = flush(left.abs().max(right.abs()).max(self.peak * fall));
        let threshold = self.peak * self.room / self.drive;
        let ([left, right], gain) = self.limiter.process(left, right, threshold);
        let total = 1.0 + self.wet * (gain * self.drive - 1.0);
        [left * total, right * total]
    }
}

/// Three bands split by fourth-order Linkwitz-Riley crossovers, each driven
/// into its own stereo-linked lookahead limiter, then an output limiter
/// that keeps the sum under the mix's recent peak, so nothing comes out
/// louder than recent peaks went in. The low band also passes the high
/// crossover's all-pass, so with every amount at zero the bands sum to a
/// flat all-pass.
#[derive(Clone, Copy)]
pub(super) struct Multiband {
    /// Low-pass and high-pass at the low crossover, then low-pass and
    /// high-pass at the high crossover, two sections each.
    splits: [[Biquad; 2]; 4],
    all_pass: Biquad,
    bands: [Band; 3],
    amounts: [Smoothed; 3],
    output: PeakLimiter,
    /// The mix's recent peak, and its value as it was when the frames now
    /// leaving the bands came in.
    peak: f64,
    planned: Delay<BAND_LOOKAHEAD>,
    fall: f64,
}
impl Multiband {
    pub(super) const LATENCY: usize = 2 * BAND_LOOKAHEAD;

    pub(super) fn new() -> Self {
        let q = FRAC_1_SQRT_2;
        let [low, high] = CROSSOVERS;
        Self {
            splits: [
                [Biquad::new(Coefficients::low_pass(low, q)); 2],
                [Biquad::new(Coefficients::high_pass(low, q)); 2],
                [Biquad::new(Coefficients::low_pass(high, q)); 2],
                [Biquad::new(Coefficients::high_pass(high, q)); 2],
            ],
            all_pass: Biquad::new(Coefficients::all_pass(high, q)),
            bands: BAND_TIMES.map(Band::new),
            amounts: [Smoothed::new(0.0); 3],
            output: PeakLimiter::new(OUTPUT_TIMES),
            peak: 0.0,
            planned: Delay::ZERO,
            fall: pole(PEAK_FALL),
        }
    }
    pub(super) fn set(&mut self, settings: &settings::Multiband) {
        for (amount, value) in
            self.amounts
                .iter_mut()
                .zip([settings.low, settings.mid, settings.high])
        {
            amount.set(f64::from(value) / 100.0);
        }
    }
    pub(super) fn clear(&mut self) {
        for split in self.splits.iter_mut().flatten() {
            split.clear();
        }
        self.all_pass.clear();
        self.bands.iter_mut().for_each(Band::clear);
        self.amounts.iter_mut().for_each(Smoothed::settle);
        self.output.clear();
        self.peak = 0.0;
        self.planned = Delay::ZERO;
    }
    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let mut bands = [[0.0; 2]; 3];
        for (channel, sample) in [f64::from(left), f64::from(right)].into_iter().enumerate() {
            let [low_pass, high_pass, mid_pass, top_pass] = &mut self.splits;
            let section = |[first, second]: &mut [Biquad; 2], x| {
                let x = first.process(channel, x);
                second.process(channel, x)
            };
            let low = section(low_pass, sample);
            let rest = section(high_pass, sample);
            bands[0][channel] = self.all_pass.process(channel, low);
            bands[1][channel] = section(mid_pass, rest);
            bands[2][channel] = section(top_pass, rest);
        }
        let mut sum = [0.0; 2];
        let mut on = false;
        for ((band, amount), [l, r]) in self.bands.iter_mut().zip(&mut self.amounts).zip(bands) {
            band.set_amount(amount.next());
            on |= band.amount > 0.0;
            let [l, r] = band.process(l, r, self.fall);
            sum[0] += l;
            sum[1] += r;
        }
        let level = f64::from(left.abs().max(right.abs()));
        self.peak = flush(level.max(self.peak * self.fall));
        let planned = f64::from(self.planned.push(self.peak as f32));
        // With every amount at zero the bands sum to the all-pass, whose
        // peaks may differ from the input's, so the output stage stands by.
        let threshold = if on { planned } else { f64::INFINITY };
        let ([l, r], gain) = self.output.process(sum[0], sum[1], threshold);
        ((l * gain) as f32, (r * gain) as f32)
    }
}

// Limiter ---------------------------------------------------------------------------------------

/// The lookahead: 2.5 ms.
const LOOKAHEAD: usize = 110;
/// Frames on each side of a peak that keep the gain the peak needs, so the
/// interpolated points around it are limited too.
const SPREAD: usize = TruePeak::DELAY;
/// The span over which the gain takes the lowest requirement.
const WINDOW: usize = LOOKAHEAD + 1 + 2 * SPREAD;
/// Headroom below the ceiling for rounding in the gain path, in dB.
const LIMITER_MARGIN_DB: f64 = 0.02;
/// The detector's delay, the lookahead, and the spread after a peak.
const LIMITER_LATENCY: usize = TruePeak::DELAY + LOOKAHEAD + SPREAD - 1;

/// A lookahead brickwall limiter on true peak. The detector interpolates
/// four times, as the output meter does. Each interval between samples
/// sets the gain it needs; the gain takes the lowest need within reach,
/// recovers over 80 ms, and ramps over the lookahead, so it is already low
/// when a peak arrives and stays flat across it.
#[derive(Clone, Copy)]
pub(super) struct Limiter {
    gain: Smoothed,
    ceiling: Smoothed,
    /// The ceiling as it was when the samples now leaving came in.
    planned: Delay<LIMITER_LATENCY>,
    detectors: [TruePeak; 2],
    delays: [Delay<LIMITER_LATENCY>; 2],
    needs: [f32; WINDOW],
    need_at: usize,
    held: f32,
    ramp: [f32; LOOKAHEAD],
    ramp_at: usize,
    ramp_sum: f64,
    release: f32,
    /// The most reduction since `take_reduction`, in dB.
    most: f32,
}
impl Limiter {
    pub(super) const LATENCY: usize = LIMITER_LATENCY;

    pub(super) fn new() -> Self {
        Self {
            gain: Smoothed::new(1.0),
            ceiling: Smoothed::new(1.0),
            planned: Delay::filled(1.0),
            detectors: [TruePeak::ZERO; 2],
            delays: [Delay::ZERO; 2],
            needs: [1.0; WINDOW],
            need_at: 0,
            held: 1.0,
            ramp: [1.0; LOOKAHEAD],
            ramp_at: 0,
            ramp_sum: LOOKAHEAD as f64,
            release: pole(0.08) as f32,
            most: 0.0,
        }
    }
    pub(super) fn set(&mut self, settings: &settings::Limiter) {
        self.gain.set(db_to_gain(f64::from(settings.gain)));
        self.ceiling
            .set(db_to_gain(f64::from(settings.ceiling) - LIMITER_MARGIN_DB));
    }
    pub(super) fn clear(&mut self) {
        let (gain, ceiling) = (self.gain, self.ceiling);
        *self = Self::new();
        (self.gain, self.ceiling) = (gain, ceiling);
        self.gain.settle();
        self.ceiling.settle();
        self.planned = Delay::filled(self.ceiling.value() as f32);
    }
    pub(super) fn take_reduction(&mut self) -> f32 {
        std::mem::take(&mut self.most)
    }
    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let gain = self.gain.next() as f32;
        let ceiling = self.ceiling.next() as f32;
        let (left, right) = (left * gain, right * gain);
        let peak = self.detectors[0]
            .push(left)
            .max(self.detectors[1].push(right));
        self.needs[self.need_at] = if peak > ceiling { ceiling / peak } else { 1.0 };
        self.need_at = (self.need_at + 1) % WINDOW;
        let lowest = self.needs.iter().copied().fold(1.0, f32::min);
        self.held = if lowest < self.held {
            lowest
        } else {
            lowest + (self.held - lowest) * self.release
        };
        self.ramp_sum += f64::from(self.held) - f64::from(self.ramp[self.ramp_at]);
        self.ramp[self.ramp_at] = self.held;
        self.ramp_at += 1;
        if self.ramp_at == LOOKAHEAD {
            // Recount now and then, so rounding cannot build up.
            self.ramp_at = 0;
            self.ramp_sum = self.ramp.iter().map(|v| f64::from(*v)).sum();
        }
        let envelope = ((self.ramp_sum / LOOKAHEAD as f64) as f32).min(1.0);
        if envelope < 1.0 {
            self.most = self.most.max(-gain_to_db(f64::from(envelope)) as f32);
        }
        // A leaving sample's gain was planned against the ceiling a little
        // after it came in. While the ceiling falls, that is above the
        // ceiling now, and clamping to the lower one would clip the sample;
        // while it rises, it is below. Settings change at most once a
        // playback block, so the ceiling turns at most once within the
        // delay, and the higher of the two ends covers the plan.
        let entered = self.planned.push(ceiling);
        let bound = ceiling.max(entered);
        let limit = |sample: f32| (sample * envelope).clamp(-bound, bound);
        (
            limit(self.delays[0].push(left)),
            limit(self.delays[1].push(right)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master::dsp::{RATE, program};
    use std::f64::consts::PI;

    fn run(
        frames: &[(f32, f32)],
        mut pedal: impl FnMut(f32, f32) -> (f32, f32),
    ) -> Vec<(f32, f32)> {
        frames.iter().map(|(l, r)| pedal(*l, *r)).collect()
    }

    fn rms_db(frames: &[(f32, f32)]) -> f64 {
        let power = frames
            .iter()
            .map(|(l, r)| (f64::from(*l).powi(2) + f64::from(*r).powi(2)) / 2.0)
            .sum::<f64>()
            / frames.len() as f64;
        10.0 * power.log10()
    }

    /// The level of the difference between an output and the input it
    /// should equal `delay` frames earlier, relative to the input.
    fn residue_db(output: &[(f32, f32)], input: &[(f32, f32)], delay: usize) -> f64 {
        let skip = 4096;
        let difference: Vec<(f32, f32)> = output[skip..]
            .iter()
            .zip(&input[skip - delay..])
            .map(|((a, b), (c, d))| (a - c, b - d))
            .collect();
        rms_db(&difference) - rms_db(&input[skip - delay..input.len() - delay])
    }

    /// The input through the oversampling alone, as a pedal with a straight
    /// curve gives it.
    fn oversampled(input: &[(f32, f32)]) -> Vec<(f32, f32)> {
        let mut oversamplers = [Oversampler::ZERO; 2];
        run(input, |l, r| {
            (
                oversamplers[0].process(l, |x| x),
                oversamplers[1].process(r, |x| x),
            )
        })
    }

    #[test]
    fn neutral_settings_pass_the_mix_through() {
        let input = program(20_000);
        let quiet: Vec<(f32, f32)> = input.iter().map(|(l, r)| (l * 0.5, r * 0.5)).collect();
        let through = oversampled(&quiet);

        let mut tube = Tube::new();
        tube.set(&settings::Tube {
            on: true,
            drive: 6.0,
            mix: 0.0,
        });
        tube.clear();
        let dry = run(&input, |l, r| tube.process(l, r));
        assert_eq!(&dry[Tube::LATENCY..], &input[..input.len() - Tube::LATENCY]);

        // Below their curves, the soft clip and maximizer are the
        // oversampling alone.
        let mut soft_clip = SoftClip::new();
        soft_clip.set(&settings::SoftClip {
            on: true,
            amount: 0.0,
            ceiling: 0.0,
        });
        soft_clip.clear();
        assert_eq!(run(&quiet, |l, r| soft_clip.process(l, r)), through);
        let mut maximizer = Maximizer::new();
        maximizer.set(&settings::Maximizer {
            on: true,
            boost: 0.0,
        });
        maximizer.clear();
        assert_eq!(run(&quiet, |l, r| maximizer.process(l, r)), through);

        let mut tape = Tape::new();
        tape.set(&settings::Tape {
            on: true,
            drive: 0.0,
            speed: 15,
        });
        tape.clear();
        assert!(residue_db(&run(&quiet, |l, r| tape.process(l, r)), &through, 0) < -120.0);

        let mut lift = LiftEq::new();
        lift.set(&settings::LiftEq {
            on: true,
            low: 0.0,
            air: 0.0,
        });
        lift.clear();
        assert_eq!(run(&input, |l, r| lift.process(l, r)), input);

        let mut compressor = BusCompressor::new();
        compressor.set(&settings::BusCompressor {
            on: true,
            threshold: 0.0,
            ratio: 1.5,
            attack: 10.0,
            release: 300.0,
        });
        compressor.clear();
        assert_eq!(run(&quiet, |l, r| compressor.process(l, r)), quiet);
    }

    #[test]
    fn multiband_at_zero_is_the_crossover_all_pass() {
        let input = program(12_000);
        let mut multiband = Multiband::new();
        multiband.set(&settings::Multiband {
            on: true,
            low: 0.0,
            mid: 0.0,
            high: 0.0,
        });
        multiband.clear();
        let q = FRAC_1_SQRT_2;
        let [mut first, mut second] =
            CROSSOVERS.map(|hz| Biquad::new(Coefficients::all_pass(hz, q)));
        let expected: Vec<(f32, f32)> = run(&input, |l, r| {
            let mut pass =
                |channel, x| second.process(channel, first.process(channel, f64::from(x))) as f32;
            (pass(0, l), pass(1, r))
        });
        let output = run(&input, |l, r| multiband.process(l, r));
        for ((a, b), (x, y)) in output[Multiband::LATENCY..].iter().zip(&expected) {
            assert!((a - x).abs() < 1e-6 && (b - y).abs() < 1e-6);
        }
    }

    #[test]
    fn multiband_lifts_the_body_and_keeps_the_peaks() {
        let input = program(4 * 44_100);
        let peak = |frames: &[(f32, f32)]| {
            frames
                .iter()
                .fold(0.0f32, |most, (l, r)| most.max(l.abs()).max(r.abs()))
        };
        let mut multiband = Multiband::new();
        multiband.set(&settings::Multiband {
            on: true,
            low: 100.0,
            mid: 100.0,
            high: 100.0,
        });
        multiband.clear();
        let output = run(&input, |l, r| multiband.process(l, r));
        let output = &output[Multiband::LATENCY..];
        assert!(peak(output) <= peak(&input) * 1.0001);
        assert!(rms_db(&output[44_100..]) > rms_db(&input[44_100..]) + 2.0);
    }

    #[test]
    fn multiband_lifts_a_breakdown_as_much_as_a_drop() {
        // The same program 8 dB apart: each section's ceilings follow its
        // own peaks, so both come out lifted alike.
        let quiet = db_to_gain(-8.0) as f32;
        let mut input = program(7 * 44_100);
        for (l, r) in &mut input[3 * 44_100..] {
            (*l, *r) = (*l * quiet, *r * quiet);
        }
        let mut multiband = Multiband::new();
        multiband.set(&settings::Multiband {
            on: true,
            low: 60.0,
            mid: 60.0,
            high: 60.0,
        });
        multiband.clear();
        let output = run(&input, |l, r| multiband.process(l, r));
        let output = &output[Multiband::LATENCY..];
        let lift =
            |range: std::ops::Range<usize>| rms_db(&output[range.clone()]) - rms_db(&input[range]);
        let (loud, soft) = (
            lift(44_100..3 * 44_100),
            lift(5 * 44_100..6 * 44_100 + 22_050),
        );
        assert!(loud > 1.0, "the drop is lifted {loud:.2} dB");
        assert!(
            (loud - soft).abs() < 0.3,
            "drop {loud:.2} dB, breakdown {soft:.2} dB"
        );
    }

    #[test]
    fn stereo_imaging_never_changes_mono() {
        let input = program(8000);
        let mono: Vec<(f32, f32)> = input.iter().map(|(l, _)| (*l, *l)).collect();
        for (width, mono_below) in [(60.0, 300.0), (140.0, 60.0), (105.0, 120.0)] {
            let mut imaging = Imaging::new();
            imaging.set(&settings::Imaging {
                on: true,
                width,
                mono_below,
            });
            imaging.clear();
            assert_eq!(run(&mono, |l, r| imaging.process(l, r)), mono);
            let mut imaging = Imaging::new();
            imaging.set(&settings::Imaging {
                on: true,
                width,
                mono_below,
            });
            imaging.clear();
            for ((l, r), (a, b)) in input.iter().zip(run(&input, |l, r| imaging.process(l, r))) {
                assert!(
                    ((l + r) - (a + b)).abs() < 1e-6,
                    "The mono sum must stay the same."
                );
            }
        }
    }

    #[test]
    fn tube_leans_even_and_tape_leans_odd() {
        let frames = 44_100;
        let input: Vec<(f32, f32)> = (0..frames)
            .map(|i| {
                let v = (0.5 * (2.0 * PI * 1000.0 * i as f64 / RATE).sin()) as f32;
                (v, v)
            })
            .collect();
        let harmonic = |output: &[(f32, f32)], n: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, (v, _)) in output[4410..].iter().enumerate() {
                let angle = 2.0 * PI * 1000.0 * n * i as f64 / RATE;
                re += f64::from(*v) * angle.cos();
                im += f64::from(*v) * angle.sin();
            }
            re.hypot(im)
        };
        let mut tube = Tube::new();
        tube.set(&settings::Tube {
            on: true,
            drive: 6.0,
            mix: 100.0,
        });
        tube.clear();
        let warm = run(&input, |l, r| tube.process(l, r));
        assert!(harmonic(&warm, 2.0) > 2.0 * harmonic(&warm, 3.0));
        let mut tape = Tape::new();
        tape.set(&settings::Tape {
            on: true,
            drive: 8.0,
            speed: 30,
        });
        tape.clear();
        let saturated = run(&input, |l, r| tape.process(l, r));
        assert!(harmonic(&saturated, 3.0) > 4.0 * harmonic(&saturated, 2.0));

        // Neither moves quiet passages from their level.
        let quiet: Vec<(f32, f32)> = input.iter().map(|(l, r)| (l * 0.1, r * 0.1)).collect();
        let tube_level = rms_db(&run(&quiet, |l, r| tube.process(l, r))[4410..]);
        let tape_level = rms_db(&run(&quiet, |l, r| tape.process(l, r))[4410..]);
        for level in [tube_level, tape_level] {
            assert!((level - rms_db(&quiet[4410..])).abs() < 0.3, "{level}");
        }
    }

    #[test]
    fn bus_compressor_follows_its_static_curve() {
        let mut compressor = BusCompressor::new();
        compressor.set(&settings::BusCompressor {
            on: true,
            threshold: -20.0,
            ratio: 4.0,
            attack: 1.0,
            release: 100.0,
        });
        compressor.clear();
        // A steady sine 10 dB over the threshold settles 7.5 dB lower.
        let amplitude = db_to_gain(-10.0) * 2f64.sqrt();
        let mut last = 0.0_f32;
        for i in 0..44_100 {
            let v = (amplitude * (2.0 * PI * 1000.0 * i as f64 / RATE).sin()) as f32;
            last = last.max(if i > 40_000 {
                compressor.process(v, v).0.abs()
            } else {
                compressor.process(v, v);
                0.0
            });
        }
        let reduction = gain_to_db(amplitude) - gain_to_db(f64::from(last));
        assert!((reduction - 7.5).abs() < 0.3, "{reduction}");
        assert!((compressor.take_reduction() - 7.5).abs() < 0.3);
        assert_eq!(compressor.take_reduction(), 0.0);
    }

    #[test]
    fn limiter_holds_true_peak_under_the_ceiling() {
        let mut signals: Vec<Vec<(f32, f32)>> = vec![program(30_000)];
        // Inter-sample peaks: quarter-rate sines at 45 degrees, single-sample
        // spikes, and full-scale square waves.
        signals.push(
            (0..8000)
                .map(|i| {
                    let v = (0.9 * (PI / 2.0 * i as f64 + PI / 4.0).sin()) as f32;
                    (v, -v)
                })
                .collect(),
        );
        signals.push(
            (0..8000)
                .map(|i| {
                    if i % 997 == 0 {
                        (1.0, -1.0)
                    } else {
                        (0.01, 0.0)
                    }
                })
                .collect(),
        );
        signals.push(
            (0..8000)
                .map(|i| {
                    let v = if (i / 7) % 2 == 0 { 1.0 } else { -1.0 };
                    (v, v)
                })
                .collect(),
        );
        for (index, signal) in signals.iter().enumerate() {
            for ceiling in [-1.0, -3.0, 0.0] {
                let mut limiter = Limiter::new();
                limiter.set(&settings::Limiter {
                    on: true,
                    gain: 12.0,
                    ceiling,
                });
                limiter.clear();
                let output = run(signal, |l, r| limiter.process(l, r));
                let mut meters = [TruePeak::ZERO; 2];
                let peak = output.iter().fold(0.0_f32, |p, (l, r)| {
                    p.max(meters[0].push(*l)).max(meters[1].push(*r))
                });
                let (peak, ceiling) = (gain_to_db(f64::from(peak)), f64::from(ceiling));
                assert!(
                    peak <= ceiling + 1e-3,
                    "Signal {index} at {ceiling} dBTP peaks at {peak:.3}"
                );
                assert!(
                    peak > ceiling - 1.0,
                    "Signal {index} must reach near the ceiling, not {peak:.3}"
                );
            }
        }
    }

    #[test]
    fn limiter_keeps_quiet_sound_unchanged_after_its_delay() {
        let input: Vec<(f32, f32)> = program(10_000)
            .iter()
            .map(|(l, r)| (l * 0.1, r * 0.1))
            .collect();
        let mut limiter = Limiter::new();
        limiter.set(&settings::Limiter {
            on: true,
            gain: 0.0,
            ceiling: -1.0,
        });
        limiter.clear();
        let output = run(&input, |l, r| limiter.process(l, r));
        assert_eq!(
            &output[Limiter::LATENCY..],
            &input[..input.len() - Limiter::LATENCY]
        );
        assert_eq!(limiter.take_reduction(), 0.0);
    }

    #[test]
    fn lowering_the_limiter_ceiling_during_play_does_not_clip() {
        // The right channel is half the left. The gain reduction scales both
        // alike; a clipped sample on the left alone would break the ratio.
        let input: Vec<(f32, f32)> = (0..8820)
            .map(|i| {
                let t = i as f64 / RATE;
                let v = (0.6 * (2.0 * PI * 100.0 * t).sin() + 0.35 * (2.0 * PI * 3000.0 * t).sin())
                    as f32;
                (v, v * 0.5)
            })
            .collect();
        let mut limiter = Limiter::new();
        let mut settings = settings::Limiter {
            on: true,
            gain: 6.0,
            ceiling: 0.0,
        };
        limiter.set(&settings);
        limiter.clear();
        let mut output = Vec::new();
        for (index, (l, r)) in input.iter().enumerate() {
            if index == 4410 {
                settings.ceiling = -3.0;
                limiter.set(&settings);
            }
            output.push(limiter.process(*l, *r));
        }
        for (frame, (l, r)) in output.iter().enumerate() {
            if l.abs() > 1e-3 {
                assert!(
                    (r / l - 0.5).abs() < 1e-4,
                    "Frame {frame} clips: {l} and {r}"
                );
            }
        }
    }

    #[test]
    fn a_glide_has_no_steps() {
        // Moving the trim EQ's high-pass across its whole range while a
        // tone plays changes the output smoothly.
        let mut eq = TrimEq::new();
        eq.set(&settings::TrimEq {
            on: true,
            high_pass: 10.0,
            mud: 0.0,
            harsh: -3.0,
        });
        eq.clear();
        eq.set(&settings::TrimEq {
            on: true,
            high_pass: 40.0,
            mud: -3.0,
            harsh: 0.0,
        });
        let mut previous = 0.0_f32;
        for i in 0..8820 {
            let v = (0.5 * (2.0 * PI * 200.0 * i as f64 / RATE).sin()) as f32;
            let (out, _) = eq.process(v, v);
            if i > 0 {
                assert!((out - previous).abs() < 0.03, "Step at frame {i}");
            }
            previous = out;
        }
    }
}
