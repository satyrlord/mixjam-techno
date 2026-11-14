//! The dynamics pedals: the bus compressor, the multiband, and the
//! limiter. Each follows a level and brings its gain down; the multiband
//! and the limiter look ahead.
use crate::master as settings;
use crate::master::dsp::{
    Biquad, Coefficients, DETECTOR, Delay, KNEE_DB, MovingMin, Smoothed, TruePeak, db_to_gain,
    flush, gain_to_db, pole, reduction,
};
use std::f64::consts::FRAC_1_SQRT_2;

// Bus compressor ----------------------------------------------------------------------

/// A feed-forward compressor on the stereo mix. One detector follows the
/// mean power of both channels, so both get the same gain; a soft knee
/// shapes the reduction, and attack and release smooth it. No makeup gain.
#[derive(Clone, Copy)]
pub(in crate::master) struct BusCompressor {
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
    pub(in crate::master) fn new() -> Self {
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
    pub(in crate::master) fn set(&mut self, settings: &settings::BusCompressor) {
        self.threshold.set(f64::from(settings.threshold));
        self.ratio.set(f64::from(settings.ratio));
        self.attack = pole(f64::from(settings.attack) / 1000.0);
        self.release = pole(f64::from(settings.release) / 1000.0);
    }
    pub(in crate::master) fn clear(&mut self) {
        (self.power, self.reduction, self.most) = (0.0, 0.0, 0.0);
        self.threshold.settle();
        self.ratio.settle();
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
        let (l, r) = (f64::from(left), f64::from(right));
        let (kl, kr) = key.map_or((l, r), |(kl, kr)| (f64::from(kl), f64::from(kr)));
        let power = 0.5 * (kl * kl + kr * kr);
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

// Multiband ------------------------------------------------------------------------------------

/// The crossovers between the low, mid, and high bands.
pub(super) const CROSSOVERS: [f64; 2] = [200.0, 3000.0];
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
    /// The frame `BAND_LOOKAHEAD` ago and the gain it gets. The gain holds
    /// the louder of the frame and `key`, a level, under the threshold.
    fn process(&mut self, left: f64, right: f64, threshold: f64, key: f64) -> ([f64; 2], f64) {
        let level = left.abs().max(right.abs()).max(key);
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
    /// `key` is the key's level in the same band, or 0.
    fn process(&mut self, left: f64, right: f64, fall: f64, key: f64) -> [f64; 2] {
        self.peak = flush(left.abs().max(right.abs()).max(self.peak * fall));
        let threshold = self.peak * self.room / self.drive;
        let ([left, right], gain) = self.limiter.process(left, right, threshold, key);
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
        Self {
            splits,
            all_pass,
            key_splits: splits,
            key_all_pass: all_pass,
            bands: BAND_TIMES.map(Band::new),
            amounts: [Smoothed::new(0.0); 3],
            output: PeakLimiter::new(OUTPUT_TIMES),
            peak: 0.0,
            planned: Delay::ZERO,
            fall: pole(PEAK_FALL),
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
            let section = |[first, second]: &mut [Biquad; 2], x| {
                let x = first.process(channel, x);
                second.process(channel, x)
            };
            let low = section(low_pass, sample);
            let rest = section(high_pass, sample);
            bands[0][channel] = all_pass.process(channel, low);
            bands[1][channel] = section(mid_pass, rest);
            bands[2][channel] = section(top_pass, rest);
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
        for (((band, amount), [l, r]), key) in self
            .bands
            .iter_mut()
            .zip(&mut self.amounts)
            .zip(bands)
            .zip(keys)
        {
            band.set_amount(amount.next());
            on |= band.amount > 0.0;
            let [l, r] = band.process(l, r, self.fall, key);
            sum[0] += l;
            sum[1] += r;
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
const LIMITER_MARGIN_DB: f64 = 0.02;
/// The detector's delay, the lookahead, and the spread after a peak.
const LIMITER_LATENCY: usize = TruePeak::DELAY + LOOKAHEAD + SPREAD - 1;

/// A lookahead brickwall limiter on true peak. The detector interpolates
/// four times, as the output meter does. Each interval between samples
/// sets the gain it needs; the gain takes the lowest need within reach,
/// recovers over 80 ms, and ramps over the lookahead, so it is already low
/// when a peak arrives and stays flat across it.
#[derive(Clone, Copy)]
pub(in crate::master) struct Limiter {
    gain: Smoothed,
    ceiling: Smoothed,
    /// The ceiling as it was when the samples now leaving came in.
    planned: Delay<LIMITER_LATENCY>,
    detectors: [TruePeak; 2],
    /// The key's true peak, which the limiter also holds under the ceiling.
    key_detectors: [TruePeak; 2],
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
    pub(in crate::master) const LATENCY: usize = LIMITER_LATENCY;

    pub(in crate::master) fn new() -> Self {
        Self {
            gain: Smoothed::new(1.0),
            ceiling: Smoothed::new(1.0),
            planned: Delay::filled(1.0),
            detectors: [TruePeak::ZERO; 2],
            key_detectors: [TruePeak::ZERO; 2],
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
    pub(in crate::master) fn set(&mut self, settings: &settings::Limiter) {
        self.gain.set(db_to_gain(f64::from(settings.gain)));
        self.ceiling
            .set(db_to_gain(f64::from(settings.ceiling) - LIMITER_MARGIN_DB));
    }
    pub(in crate::master) fn clear(&mut self) {
        let (gain, ceiling) = (self.gain, self.ceiling);
        *self = Self::new();
        (self.gain, self.ceiling) = (gain, ceiling);
        self.gain.settle();
        self.ceiling.settle();
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
mod tests;
