//! The dynamics pedals: the bus compressor, the multiband, and the
//! limiter. Each follows a level and brings its gain down; the multiband
//! and the limiter look ahead.
use super::{REFRESH, Refresh};
use crate::master as settings;
use crate::master::LimiterStyle;
use crate::master::dsp::{
    Biquad, Coefficients, DETECTOR, Delay, KNEE_DB, MovingMin, Ramp, Smoothed, TruePeak, cascade,
    db_to_gain, flush, gain_to_db, pole, reduction,
};
use std::f64::consts::FRAC_1_SQRT_2;

// Dual-stage lookahead gain ------------------------------------------------------------

#[derive(Clone, Copy)]
struct ReleaseTimes {
    fast_release: f64,
    slow_attack: f64,
    slow_release: f64,
}

impl ReleaseTimes {
    fn from_ms(fast_ms: f64, slow_attack_ms: f64, slow_ms: f64) -> Self {
        Self {
            fast_release: pole(fast_ms / 1000.0),
            slow_attack: pole(slow_attack_ms / 1000.0),
            slow_release: pole(slow_ms / 1000.0),
        }
    }
}

/// Stereo-linked lookahead gain with dual-stage (fast transient + slow program)
/// release feeding a `LOOKAHEAD`-frame moving-average ramp.
#[derive(Clone, Copy)]
struct DualStageGain<const LOOKAHEAD: usize, const SPAN: usize> {
    needs: MovingMin<SPAN>,
    span: usize,
    fast_held: f64,
    slow_held: f64,
    times: ReleaseTimes,
    ramp: Ramp<LOOKAHEAD>,
}

impl<const LOOKAHEAD: usize, const SPAN: usize> DualStageGain<LOOKAHEAD, SPAN> {
    fn new(span: usize, times: ReleaseTimes) -> Self {
        Self {
            needs: MovingMin::EMPTY,
            span,
            fast_held: 1.0,
            slow_held: 1.0,
            times,
            ramp: Ramp::UNITY,
        }
    }

    fn set_times(&mut self, times: ReleaseTimes) {
        self.times = times;
    }

    fn clear(&mut self) {
        *self = Self::new(self.span, self.times);
    }

    #[inline]
    fn push(&mut self, need: f64) -> f64 {
        let lowest = self.needs.push(need, self.span);
        self.fast_held = if lowest < self.fast_held {
            lowest
        } else {
            flush(lowest + (self.fast_held - lowest) * self.times.fast_release)
        };
        let slow_pole = if lowest < self.slow_held {
            self.times.slow_attack
        } else {
            self.times.slow_release
        };
        self.slow_held = flush(lowest + (self.slow_held - lowest) * slow_pole);
        self.ramp.push(self.fast_held.min(self.slow_held))
    }
}

// Bus compressor ----------------------------------------------------------------------

const SC_HPF_BYPASS_HZ: f64 = 20.5;
const SLOW_CREST_POLE_S: f64 = 0.25;

/// A feed-forward compressor on the stereo mix with a sidechain high-pass
/// filter, crest-responsive release, parallel dry/wet mix, and smoothed
/// makeup gain.
#[derive(Clone, Copy)]
pub(in crate::master) struct BusCompressor {
    threshold: Smoothed,
    ratio: Smoothed,
    sc_hpf_hz: Smoothed,
    sc_hpf: [Biquad; 2],
    sc_refresh: Refresh,
    mix: Smoothed,
    makeup: Smoothed,
    attack: f64,
    release: f64,
    fast_release: f64,
    detector: f64,
    slow_pole: f64,
    power: f64,
    slow_over: f64,
    /// Current gain reduction in dB.
    reduction: f64,
    /// The most reduction since `take_reduction`, in dB.
    most: f64,
}
impl BusCompressor {
    pub(in crate::master) fn new() -> Self {
        Self {
            threshold: Smoothed::new(0.0),
            ratio: Smoothed::new(1.0),
            sc_hpf_hz: Smoothed::new(20.0),
            sc_hpf: [Biquad::FLAT; 2],
            sc_refresh: Refresh(REFRESH),
            mix: Smoothed::new(1.0),
            makeup: Smoothed::new(1.0),
            attack: 0.0,
            release: 0.0,
            fast_release: 0.0,
            detector: pole(DETECTOR),
            slow_pole: pole(SLOW_CREST_POLE_S),
            power: 0.0,
            slow_over: 0.0,
            reduction: 0.0,
            most: 0.0,
        }
    }
    pub(in crate::master) fn set(&mut self, settings: &settings::BusCompressor) {
        let release_s = f64::from(settings.release) / 1000.0;
        self.threshold.set(f64::from(settings.threshold));
        self.ratio.set(f64::from(settings.ratio));
        self.sc_hpf_hz.set(f64::from(settings.sc_hpf_hz));
        self.mix.set(f64::from(settings.mix) / 100.0);
        self.makeup.set(db_to_gain(f64::from(settings.makeup_db)));
        self.attack = pole(f64::from(settings.attack) / 1000.0);
        self.release = pole(release_s);
        self.fast_release = pole((release_s * 0.4).max(0.02));
    }
    pub(in crate::master) fn clear(&mut self) {
        (self.power, self.slow_over, self.reduction, self.most) = (0.0, 0.0, 0.0, 0.0);
        self.sc_hpf.iter_mut().for_each(Biquad::clear);
        for setting in [
            &mut self.threshold,
            &mut self.ratio,
            &mut self.sc_hpf_hz,
            &mut self.mix,
            &mut self.makeup,
        ] {
            setting.settle();
        }
        self.design_sc_hpf(self.sc_hpf_hz.value());
    }
    fn design_sc_hpf(&mut self, hz: f64) {
        let coeffs = if hz > SC_HPF_BYPASS_HZ {
            Coefficients::high_pass(hz, FRAC_1_SQRT_2)
        } else {
            Coefficients::FLAT
        };
        for stage in &mut self.sc_hpf {
            stage.set(coeffs);
        }
    }
    pub(in crate::master) fn take_reduction(&mut self) -> f32 {
        std::mem::take(&mut self.most) as f32
    }
    #[cfg(test)]
    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.process_keyed(left, right, None)
    }
    /// With a key, the detector follows the key instead of the input.
    pub(in crate::master) fn process_keyed(
        &mut self,
        left: f32,
        right: f32,
        key: Option<(f32, f32)>,
    ) -> (f32, f32) {
        let (threshold, ratio) = (self.threshold.next(), self.ratio.next());
        let gliding_sc = !self.sc_hpf_hz.settled();
        let sc_hz = self.sc_hpf_hz.next();
        if self.sc_refresh.due(gliding_sc, self.sc_hpf_hz.settled()) {
            self.design_sc_hpf(sc_hz);
        }
        let mix = self.mix.next();
        let makeup = self.makeup.next();
        let (l, r) = (f64::from(left), f64::from(right));
        let (mut kl, mut kr) = key.map_or((l, r), |(kl, kr)| (f64::from(kl), f64::from(kr)));
        if sc_hz > SC_HPF_BYPASS_HZ {
            kl = cascade(&mut self.sc_hpf, 0, kl);
            kr = cascade(&mut self.sc_hpf, 1, kr);
        }
        let power = 0.5 * (kl * kl + kr * kr);
        self.power = flush(power + (self.power - power) * self.detector);
        let level = 10.0 * self.power.max(1e-12).log10();
        let target = reduction(level, threshold, ratio, KNEE_DB);
        self.slow_over = flush(target + (self.slow_over - target) * self.slow_pole);
        let speed = if target > self.reduction {
            self.attack
        } else {
            let crest =
                ((self.reduction - self.slow_over) / self.reduction.max(1e-6)).clamp(0.0, 1.0);
            self.release + crest * (self.fast_release - self.release)
        };
        self.reduction = flush(target + (self.reduction - target) * speed);
        if self.reduction == 0.0 && makeup == 1.0 {
            return (left, right);
        }
        self.most = self.most.max(self.reduction);
        let wet_gain = db_to_gain(-self.reduction);
        let total = (1.0 + (wet_gain - 1.0) * mix) * makeup;
        ((l * total) as f32, (r * total) as f32)
    }
}

// Multiband ------------------------------------------------------------------------------------

/// The crossovers between the low, mid, and high bands.
pub(super) const CROSSOVERS: [f64; 2] = [200.0, 3000.0];
/// How far each limiter looks ahead: 2 ms, so the gain is already down
/// when a peak arrives.
const BAND_LOOKAHEAD: usize = 88;
/// How long each band keeps the gain a peak needs, in frames. The low band
/// holds 10 ms, so a bass wave is not reshaped within its cycle; the mids
/// and highs hold 3.3 and 2.2 ms. `band_release_times` gives the recovery.
const BAND_HOLDS: [usize; 3] = [441, 146, 96];
/// The output stage holds 3.2 ms.
const OUTPUT_HOLD: usize = 141;
/// The longest span over which a limiter takes the lowest need.
const SPAN: usize = BAND_LOOKAHEAD + 1 + BAND_HOLDS[0];
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

fn band_release_times(band: usize) -> ReleaseTimes {
    match band {
        0 => ReleaseTimes::from_ms(20.0, 180.0, 140.0),
        1 => ReleaseTimes::from_ms(12.0, 180.0, 90.0),
        _ => ReleaseTimes::from_ms(8.0, 160.0, 60.0),
    }
}

fn output_release_times() -> ReleaseTimes {
    ReleaseTimes::from_ms(10.0, 180.0, 80.0)
}

/// A stereo-linked lookahead limiter on sample peaks with dual-stage release.
#[derive(Clone, Copy)]
struct PeakLimiter {
    delays: [Delay<BAND_LOOKAHEAD>; 2],
    gain: DualStageGain<BAND_LOOKAHEAD, SPAN>,
}
impl PeakLimiter {
    fn new(hold: usize, times: ReleaseTimes) -> Self {
        Self {
            delays: [Delay::ZERO; 2],
            gain: DualStageGain::new(BAND_LOOKAHEAD + 1 + hold, times),
        }
    }
    fn clear(&mut self) {
        self.delays = [Delay::ZERO; 2];
        self.gain.clear();
    }
    /// The frame `BAND_LOOKAHEAD` ago and the gain it gets. The gain holds
    /// the louder of the frame and `key`, a level, under the threshold.
    fn process(&mut self, left: f64, right: f64, threshold: f64, key: f64) -> ([f64; 2], f64) {
        let level = left.abs().max(right.abs()).max(key);
        let need = if level > threshold {
            threshold / level
        } else {
            1.0
        };
        let gain = self.gain.push(need);
        let delayed = [
            f64::from(self.delays[0].push(left as f32)),
            f64::from(self.delays[1].push(right as f32)),
        ];
        (delayed, gain)
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
    fn new(hold: usize, times: ReleaseTimes) -> Self {
        Self {
            limiter: PeakLimiter::new(hold, times),
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
    /// Returns `[left, right]` and the compression gain reduction in dB.
    fn process(&mut self, left: f64, right: f64, fall: f64, key: f64) -> ([f64; 2], f32) {
        self.peak = flush(left.abs().max(right.abs()).max(self.peak * fall));
        let threshold = (self.peak * self.room / self.drive).max(1e-5);
        let ([left, right], gain) = self.limiter.process(left, right, threshold, key);
        let total = 1.0 + self.wet * (gain * self.drive - 1.0);
        let gr_db = if self.amount > 0.0 && gain < 1.0 {
            (-gain_to_db(gain) as f32).max(0.0)
        } else {
            0.0
        };
        ([left * total, right * total], gr_db)
    }
}

/// Three bands split by fourth-order Linkwitz-Riley crossovers, each driven
/// into its own stereo-linked lookahead limiter, then an output limiter
/// that keeps the sum under the mix's recent peak. The low band also passes
/// the high crossover's all-pass, so with every amount at zero the bands sum
/// to a flat all-pass.
#[derive(Clone, Copy)]
pub(in crate::master) struct Multiband {
    /// Low-pass and high-pass at the low crossover, then low-pass and
    /// high-pass at the high crossover, two sections each.
    splits: [[Biquad; 2]; 4],
    all_pass: Biquad,
    /// The same crossovers for the key.
    key_splits: [[Biquad; 2]; 4],
    key_all_pass: Biquad,
    bands: [Band; 3],
    amounts: [Smoothed; 3],
    output: PeakLimiter,
    /// The mix's recent peak, and its value as it was when the frames now
    /// leaving the bands came in.
    peak: f64,
    planned: Delay<BAND_LOOKAHEAD>,
    fall: f64,
    /// Peak gain reduction per band since the last `take_reductions`, in dB.
    reductions: [f32; 3],
}
impl Multiband {
    pub(in crate::master) const LATENCY: usize = 2 * BAND_LOOKAHEAD;

    pub(in crate::master) fn new() -> Self {
        let q = FRAC_1_SQRT_2;
        let [low, high] = CROSSOVERS;
        let splits = [
            [Biquad::new(Coefficients::low_pass(low, q)); 2],
            [Biquad::new(Coefficients::high_pass(low, q)); 2],
            [Biquad::new(Coefficients::low_pass(high, q)); 2],
            [Biquad::new(Coefficients::high_pass(high, q)); 2],
        ];
        let all_pass = Biquad::new(Coefficients::all_pass(high, q));
        let bands = std::array::from_fn(|i| Band::new(BAND_HOLDS[i], band_release_times(i)));
        Self {
            splits,
            all_pass,
            key_splits: splits,
            key_all_pass: all_pass,
            bands,
            amounts: [Smoothed::new(0.0); 3],
            output: PeakLimiter::new(OUTPUT_HOLD, output_release_times()),
            peak: 0.0,
            planned: Delay::ZERO,
            fall: pole(PEAK_FALL),
            reductions: [0.0; 3],
        }
    }
    pub(in crate::master) fn set(&mut self, settings: &settings::Multiband) {
        for (amount, value) in
            self.amounts
                .iter_mut()
                .zip([settings.low, settings.mid, settings.high])
        {
            amount.set(f64::from(value) / 100.0);
        }
    }
    pub(in crate::master) fn clear(&mut self) {
        for split in self.splits.iter_mut().chain(&mut self.key_splits).flatten() {
            split.clear();
        }
        self.all_pass.clear();
        self.key_all_pass.clear();
        self.bands.iter_mut().for_each(Band::clear);
        self.amounts.iter_mut().for_each(Smoothed::settle);
        self.output.clear();
        self.peak = 0.0;
        self.planned = Delay::ZERO;
        self.reductions = [0.0; 3];
    }
    pub(in crate::master) fn take_reductions(&mut self) -> [f32; 3] {
        std::mem::take(&mut self.reductions)
    }
    /// The low, mid, and high bands of a stereo frame, each `[left, right]`.
    fn split(
        splits: &mut [[Biquad; 2]; 4],
        all_pass: &mut Biquad,
        left: f32,
        right: f32,
    ) -> [[f64; 2]; 3] {
        let mut bands = [[0.0; 2]; 3];
        for (channel, sample) in [f64::from(left), f64::from(right)].into_iter().enumerate() {
            let [low_pass, high_pass, mid_pass, top_pass] = &mut *splits;
            let low = cascade(low_pass, channel, sample);
            let rest = cascade(high_pass, channel, sample);
            bands[0][channel] = all_pass.process(channel, low);
            bands[1][channel] = cascade(mid_pass, channel, rest);
            bands[2][channel] = cascade(top_pass, channel, rest);
        }
        bands
    }
    #[cfg(test)]
    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.process_keyed(left, right, None)
    }
    /// With a key, each band's limiter also holds the key's same band
    /// under the band's ceiling, so the band gives way while the key sounds.
    pub(in crate::master) fn process_keyed(
        &mut self,
        left: f32,
        right: f32,
        key: Option<(f32, f32)>,
    ) -> (f32, f32) {
        let bands = Self::split(&mut self.splits, &mut self.all_pass, left, right);
        // Drain the key's crossovers while following the pedal's own input.
        // Switching back to a track must not resume an old filter tail.
        let (kl, kr) = key.unwrap_or((0.0, 0.0));
        let key_bands = Self::split(&mut self.key_splits, &mut self.key_all_pass, kl, kr);
        let keys = if key.is_some() {
            key_bands.map(|[l, r]| l.abs().max(r.abs()))
        } else {
            [0.0; 3]
        };
        let mut sum = [0.0; 2];
        let mut on = false;
        for (i, ((band, amount), [l, r])) in self
            .bands
            .iter_mut()
            .zip(&mut self.amounts)
            .zip(bands)
            .enumerate()
        {
            band.set_amount(amount.next());
            on |= band.amount > 0.0;
            let ([bl, br], gr_db) = band.process(l, r, self.fall, keys[i]);
            self.reductions[i] = self.reductions[i].max(gr_db);
            sum[0] += bl;
            sum[1] += br;
        }
        let level = f64::from(left.abs().max(right.abs()));
        self.peak = flush(level.max(self.peak * self.fall));
        let planned = f64::from(self.planned.push(self.peak as f32));
        // With every amount at zero the bands sum to the all-pass, whose
        // peaks may differ from the input's, so the output stage stands by.
        let threshold = if on { planned } else { f64::INFINITY };
        let ([l, r], gain) = self.output.process(sum[0], sum[1], threshold, 0.0);
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
const LIMITER_MARGIN_DB: f64 = 0.08;
/// The detector's delay, the lookahead, and the spread after a peak.
const LIMITER_LATENCY: usize = TruePeak::DELAY + LOOKAHEAD + SPREAD - 1;

fn limiter_profile(style: LimiterStyle) -> (ReleaseTimes, f64) {
    match style {
        LimiterStyle::Transparent => (ReleaseTimes::from_ms(12.0, 180.0, 140.0), 0.0),
        LimiterStyle::Punchy => (ReleaseTimes::from_ms(6.0, 220.0, 90.0), 0.6),
        LimiterStyle::Modern => (ReleaseTimes::from_ms(5.0, 260.0, 120.0), 1.2),
    }
}

/// The stereo-linked soft peak shave of the Punchy and Modern styles, ahead
/// of the lookahead gain. It keeps the channel balance and leaves audio
/// below its knee bit-exact. A shave of any depth already holds loud peaks
/// near the ceiling, so a style change fades the shave in or out and glides
/// only between depths above zero.
#[derive(Clone, Copy)]
struct PreShave {
    /// The knee below the ceiling, in dB.
    depth: Smoothed,
    /// 1 while a style shaves, 0 while it does not.
    mix: Smoothed,
}
impl PreShave {
    fn new(depth_db: f64) -> Self {
        Self {
            depth: Smoothed::new(depth_db),
            mix: Smoothed::new(if depth_db > 0.0 { 1.0 } else { 0.0 }),
        }
    }
    fn set(&mut self, depth_db: f64) {
        if depth_db > 0.0 {
            self.depth.set(depth_db);
            self.mix.set(1.0);
        } else {
            self.mix.set(0.0);
        }
    }
    fn settle(&mut self) {
        self.depth.settle();
        self.mix.settle();
    }
    /// The gain that shaves a frame, at most 1.
    #[inline]
    fn gain(&mut self, left: f32, right: f32, ceiling: f32) -> f64 {
        let (depth, mix) = (self.depth.next(), self.mix.next());
        if mix == 0.0 || depth <= 0.0 {
            return 1.0;
        }
        let limit = f64::from(ceiling);
        let knee = limit * db_to_gain(-depth);
        let mag = f64::from(left.abs().max(right.abs()));
        if mag <= knee || limit <= knee {
            return 1.0;
        }
        let room = (limit - knee).max(1e-9);
        let shaved = (knee + room * ((mag - knee) / room).tanh()) / mag;
        if mix >= 1.0 {
            shaved
        } else {
            1.0 + mix * (shaved - 1.0)
        }
    }
}

/// A lookahead brickwall limiter on true peak with three selectable release
/// characters (`Transparent`, `Punchy`, `Modern`). The detector interpolates
/// four times as the output meter does and applies dual-stage release.
#[derive(Clone, Copy)]
pub(in crate::master) struct Limiter {
    gain: Smoothed,
    ceiling: Smoothed,
    style: LimiterStyle,
    shave: PreShave,
    /// The pre-shave gain of each sample, until the sample leaves.
    shave_gains: Delay<LIMITER_LATENCY>,
    /// The ceiling as it was when the samples now leaving came in.
    planned: Delay<LIMITER_LATENCY>,
    detectors: [TruePeak; 2],
    /// The key's true peak, which the limiter also holds under the ceiling.
    key_detectors: [TruePeak; 2],
    delays: [Delay<LIMITER_LATENCY>; 2],
    envelope: DualStageGain<LOOKAHEAD, WINDOW>,
    /// The most reduction since `take_reduction`, in dB.
    most: f32,
}
impl Limiter {
    pub(in crate::master) const LATENCY: usize = LIMITER_LATENCY;

    pub(in crate::master) fn new() -> Self {
        let style = LimiterStyle::default();
        let (times, shave_db) = limiter_profile(style);
        Self {
            gain: Smoothed::new(1.0),
            ceiling: Smoothed::new(1.0),
            style,
            shave: PreShave::new(shave_db),
            shave_gains: Delay::filled(1.0),
            planned: Delay::filled(1.0),
            detectors: [TruePeak::ZERO; 2],
            key_detectors: [TruePeak::ZERO; 2],
            delays: [Delay::ZERO; 2],
            envelope: DualStageGain::new(WINDOW, times),
            most: 0.0,
        }
    }
    pub(in crate::master) fn set(&mut self, settings: &settings::Limiter) {
        self.gain.set(db_to_gain(f64::from(settings.gain)));
        self.ceiling
            .set(db_to_gain(f64::from(settings.ceiling) - LIMITER_MARGIN_DB));
        self.style = settings.style;
        let (times, shave_db) = limiter_profile(settings.style);
        self.envelope.set_times(times);
        self.shave.set(shave_db);
    }
    pub(in crate::master) fn clear(&mut self) {
        let (gain, ceiling, style, shave) = (self.gain, self.ceiling, self.style, self.shave);
        *self = Self::new();
        (self.gain, self.ceiling, self.style, self.shave) = (gain, ceiling, style, shave);
        self.envelope.set_times(limiter_profile(style).0);
        self.gain.settle();
        self.ceiling.settle();
        self.shave.settle();
        self.planned = Delay::filled(self.ceiling.value() as f32);
    }
    pub(in crate::master) fn take_reduction(&mut self) -> f32 {
        std::mem::take(&mut self.most)
    }
    #[cfg(test)]
    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.process_keyed(left, right, None)
    }
    /// With a key, the gain holds the louder of the sound and the key,
    /// both after the limiter's gain, under the ceiling: the sound gives
    /// way while the key peaks, and never passes the ceiling itself.
    pub(in crate::master) fn process_keyed(
        &mut self,
        left: f32,
        right: f32,
        key: Option<(f32, f32)>,
    ) -> (f32, f32) {
        let gain = self.gain.next() as f32;
        let ceiling = self.ceiling.next() as f32;
        let (left, right) = (left * gain, right * gain);
        let shave_gain = self.shave.gain(left, right, ceiling);
        let (left, right) = if shave_gain < 1.0 {
            let shave = shave_gain as f32;
            (left * shave, right * shave)
        } else {
            (left, right)
        };
        let mut peak = self.detectors[0]
            .push(left)
            .max(self.detectors[1].push(right));
        // Keep the detector's time moving even without an external key.
        let (key_left, key_right) = key.unwrap_or((0.0, 0.0));
        let key_peak = self.key_detectors[0]
            .push(key_left * gain)
            .max(self.key_detectors[1].push(key_right * gain));
        if key.is_some() {
            peak = peak.max(key_peak);
        }
        let need = if peak > ceiling { ceiling / peak } else { 1.0 };
        let envelope = self.envelope.push(f64::from(need)) as f32;
        // The leaving sample was shaved when it came in.
        let shaved = self.shave_gains.push(shave_gain as f32);
        let total = envelope * shaved;
        if total < 1.0 {
            self.most = self.most.max(-gain_to_db(f64::from(total)) as f32);
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
mod tests;
