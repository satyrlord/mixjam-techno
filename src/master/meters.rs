//! Live measurement of the master during playback. The input meter reads
//! the mix after the trim on a VU scale. The output meter reads what leaves
//! the last pedal as loudness after ITU-R BS.1770-4 and EBU R 128:
//! K-weighted momentary (400 ms), short-term (3 s), and gated integrated
//! loudness, with true peak from four-times interpolation.
//!
//! The audio thread publishes a reading after each block through atomics;
//! the window reads the latest one without waiting.
use super::dsp::{Biquad, Coefficients, RATE, TruePeak, db_to_gain, pole};
use std::{
    f64::consts::PI,
    sync::atomic::{AtomicU32, Ordering},
};

/// What the master meters read. Levels are in decibels; a level with
/// nothing measured is negative infinity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeterReading {
    /// Counts the playbacks measured so far; zero before the first.
    pub run: u32,
    /// The input after the trim on a VU scale, where 0 VU is -18 dBFS.
    pub input_vu: f32,
    /// Left and right input samples at or above -1 dBFS in the last block.
    pub input_peaks: [bool; 2],
    /// Gain reduction of the bus compressor and of the limiter in the last
    /// block, in dB.
    pub compressor_reduction: f32,
    pub limiter_reduction: f32,
    /// Output loudness in LUFS over the last 400 ms and 3 s, and gated over
    /// the whole playback.
    pub momentary: f32,
    pub short_term: f32,
    pub integrated: f32,
    /// Output true peak in dBTP: in the last block, and the highest of the
    /// playback.
    pub true_peak: f32,
    pub true_peak_max: f32,
    /// Blocks of the playback whose true peak passed -1 dBTP.
    pub overs: u32,
}
impl Default for MeterReading {
    fn default() -> Self {
        Self {
            run: 0,
            input_vu: f32::NEG_INFINITY,
            input_peaks: [false; 2],
            compressor_reduction: 0.0,
            limiter_reduction: 0.0,
            momentary: f32::NEG_INFINITY,
            short_term: f32::NEG_INFINITY,
            integrated: f32::NEG_INFINITY,
            true_peak: f32::NEG_INFINITY,
            true_peak_max: f32::NEG_INFINITY,
            overs: 0,
        }
    }
}

/// The delivery limit that the OVER lamp watches, in dBTP.
pub const OVER_DBTP: f32 = -1.0;

/// The latest reading, shared between the audio thread and the window.
/// Fields update one by one; a reading may mix two neighbouring blocks.
pub(super) struct Shared {
    run: AtomicU32,
    input_vu: AtomicU32,
    input_peaks: AtomicU32,
    compressor_reduction: AtomicU32,
    limiter_reduction: AtomicU32,
    momentary: AtomicU32,
    short_term: AtomicU32,
    integrated: AtomicU32,
    true_peak: AtomicU32,
    true_peak_max: AtomicU32,
    overs: AtomicU32,
}
impl Shared {
    pub(super) fn new() -> Self {
        let level = || AtomicU32::new(f32::NEG_INFINITY.to_bits());
        Self {
            run: AtomicU32::new(0),
            input_vu: level(),
            input_peaks: AtomicU32::new(0),
            compressor_reduction: AtomicU32::new(0),
            limiter_reduction: AtomicU32::new(0),
            momentary: level(),
            short_term: level(),
            integrated: level(),
            true_peak: level(),
            true_peak_max: level(),
            overs: AtomicU32::new(0),
        }
    }
    /// Begin a new playback's readings.
    pub(super) fn start(&self) {
        self.store(&MeterReading {
            run: self.run.load(Ordering::Relaxed).wrapping_add(1).max(1),
            ..MeterReading::default()
        });
    }
    fn store(&self, reading: &MeterReading) {
        let float =
            |atomic: &AtomicU32, value: f32| atomic.store(value.to_bits(), Ordering::Relaxed);
        float(&self.input_vu, reading.input_vu);
        self.input_peaks.store(
            u32::from(reading.input_peaks[0]) | u32::from(reading.input_peaks[1]) << 1,
            Ordering::Relaxed,
        );
        float(&self.compressor_reduction, reading.compressor_reduction);
        float(&self.limiter_reduction, reading.limiter_reduction);
        float(&self.momentary, reading.momentary);
        float(&self.short_term, reading.short_term);
        float(&self.integrated, reading.integrated);
        float(&self.true_peak, reading.true_peak);
        float(&self.true_peak_max, reading.true_peak_max);
        self.overs.store(reading.overs, Ordering::Relaxed);
        self.run.store(reading.run, Ordering::Release);
    }
    pub(super) fn read(&self) -> MeterReading {
        let run = self.run.load(Ordering::Acquire);
        let float = |atomic: &AtomicU32| f32::from_bits(atomic.load(Ordering::Relaxed));
        let peaks = self.input_peaks.load(Ordering::Relaxed);
        MeterReading {
            run,
            input_vu: float(&self.input_vu),
            input_peaks: [peaks & 1 != 0, peaks & 2 != 0],
            compressor_reduction: float(&self.compressor_reduction),
            limiter_reduction: float(&self.limiter_reduction),
            momentary: float(&self.momentary),
            short_term: float(&self.short_term),
            integrated: float(&self.integrated),
            true_peak: float(&self.true_peak),
            true_peak_max: float(&self.true_peak_max),
            overs: self.overs.load(Ordering::Relaxed),
        }
    }
}

// Loudness -------------------------------------------------------------------------

/// The K-weighting of BS.1770 at a sample rate: a high shelf for the
/// head's effect, then the RLB high-pass. The standard gives coefficients
/// for 48 kHz; these come from the analog prototypes behind them.
fn k_weighting(rate: f64) -> [Coefficients; 2] {
    let shelf = {
        let (hz, db, q) = (
            1_681.974_450_955_533,
            3.999_843_853_973_347,
            0.707_175_236_955_419_6,
        );
        let k = (PI * hz / rate).tan();
        let high = 10f64.powf(db / 20.0);
        let band = high.powf(0.499_666_774_154_541_6);
        Coefficients::raw(
            [
                high + band * k / q + k * k,
                2.0 * (k * k - high),
                high - band * k / q + k * k,
            ],
            [
                1.0 + k / q + k * k,
                2.0 * (k * k - 1.0),
                1.0 - k / q + k * k,
            ],
        )
    };
    let high_pass = {
        let (hz, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
        let k = (PI * hz / rate).tan();
        let a0 = 1.0 + k / q + k * k;
        // The standard's numerator is 1, -2, 1 against this denominator.
        Coefficients::raw(
            [a0, -2.0 * a0, a0],
            [a0, 2.0 * (k * k - 1.0), 1.0 - k / q + k * k],
        )
    };
    [shelf, high_pass]
}

/// 100 ms: the step of the gating blocks.
const STEP_FRAMES: usize = 4410;
/// Gating blocks are binned by loudness from -70 LUFS in 0.01 LU steps.
/// +800 LUFS covers finite f32 samples, including K-weighting headroom.
/// The trimmed mix can exceed full scale; it is not limited to +10 LUFS.
const FLOOR_LUFS: f64 = -70.0;
const BIN_LU: f64 = 0.01;
const BINS: usize = 87_000;

fn lufs(power: f64) -> f64 {
    -0.691 + 10.0 * power.log10()
}

/// The loudness of one playback. Each 100 ms step closes a 400 ms gating
/// block; blocks above the absolute gate go into a histogram, which keeps
/// the memory fixed however long the song is. The relative gate then sits
/// 10 LU below their mean power.
pub(super) struct Loudness {
    filters: [Biquad; 2],
    sum: f64,
    frames: usize,
    /// The mean power of each of the last 30 steps.
    steps: [f64; 30],
    closed: usize,
    /// Count and power total of the gating blocks in each bin.
    histogram: Box<[(u32, f64)]>,
    /// Normal playback only scans the populated part of the histogram.
    highest_bin: usize,
    pub(super) momentary: f64,
    pub(super) short_term: f64,
    pub(super) integrated: f64,
}
impl Loudness {
    pub(super) fn new() -> Self {
        Self::at(RATE)
    }
    fn at(rate: f64) -> Self {
        Self {
            filters: k_weighting(rate).map(Biquad::new),
            sum: 0.0,
            frames: 0,
            steps: [0.0; 30],
            closed: 0,
            histogram: vec![(0, 0.0); BINS].into_boxed_slice(),
            highest_bin: 0,
            momentary: f64::NEG_INFINITY,
            short_term: f64::NEG_INFINITY,
            integrated: f64::NEG_INFINITY,
        }
    }
    #[inline]
    pub(super) fn push(&mut self, left: f32, right: f32) {
        let mut power = 0.0;
        for (channel, sample) in [left, right].into_iter().enumerate() {
            let shelved = self.filters[0].process(channel, f64::from(sample));
            let weighted = self.filters[1].process(channel, shelved);
            power += weighted * weighted;
        }
        self.sum += power;
        self.frames += 1;
        if self.frames == STEP_FRAMES {
            self.close();
        }
    }
    fn close(&mut self) {
        self.steps[self.closed % 30] = self.sum / STEP_FRAMES as f64;
        self.closed += 1;
        (self.sum, self.frames) = (0.0, 0);
        // Steps before the playback began count as silence.
        let mean = |count: usize| {
            (0..count.min(self.closed))
                .map(|back| self.steps[(self.closed - 1 - back) % 30])
                .sum::<f64>()
                / count as f64
        };
        let block = mean(4);
        self.momentary = lufs(block);
        self.short_term = lufs(mean(30));
        if self.closed >= 4 && self.momentary > FLOOR_LUFS {
            let bin = (((self.momentary - FLOOR_LUFS) / BIN_LU) as usize).min(BINS - 1);
            self.histogram[bin].0 += 1;
            self.histogram[bin].1 += block;
            self.highest_bin = self.highest_bin.max(bin);
            self.integrated = self.gated();
        }
    }
    fn gated(&self) -> f64 {
        let histogram = &self.histogram[..=self.highest_bin];
        let (count, power) = histogram
            .iter()
            .fold((0u64, 0.0), |(n, p), (c, s)| (n + u64::from(*c), p + s));
        if count == 0 {
            return f64::NEG_INFINITY;
        }
        let gate = lufs(power / count as f64) - 10.0;
        let first = ((gate - FLOOR_LUFS) / BIN_LU).ceil().max(0.0) as usize;
        let (count, power) = histogram[first.min(histogram.len())..]
            .iter()
            .fold((0u64, 0.0), |(n, p), (c, s)| (n + u64::from(*c), p + s));
        if count == 0 {
            f64::NEG_INFINITY
        } else {
            lufs(power / count as f64)
        }
    }
}

// The meters ------------------------------------------------------------------------

/// `20 log10(pi / (2 sqrt 2))`: a sine's RMS over its rectified mean.
const SINE_FORM_DB: f64 = 0.912_097_5;
/// The level of 0 VU, in dBFS.
const REFERENCE_DBFS: f64 = -18.0;

/// The measuring state of one playback. It lives on the audio thread.
pub(super) struct Meters {
    /// Two critically damped poles per channel on the rectified input,
    /// which reach 99% of a step in 300 ms, as a VU needle does.
    needle: [[f64; 2]; 2],
    needle_step: f64,
    input_peaks: [bool; 2],
    loudness: Loudness,
    true_peaks: [TruePeak; 2],
    block_peak: f32,
    peak: f32,
    overs: u32,
    run: u32,
}
impl Meters {
    pub(super) fn new(shared: &Shared) -> Self {
        shared.start();
        Self {
            needle: [[0.0; 2]; 2],
            needle_step: 1.0 - pole(0.045_2),
            input_peaks: [false; 2],
            loudness: Loudness::new(),
            true_peaks: [TruePeak::ZERO; 2],
            block_peak: 0.0,
            peak: 0.0,
            overs: 0,
            run: shared.read().run,
        }
    }
    #[inline]
    pub(super) fn input(&mut self, left: f32, right: f32) {
        let hot = db_to_gain(-1.0) as f32;
        for (channel, sample) in [left, right].into_iter().enumerate() {
            let [first, second] = &mut self.needle[channel];
            *first += (f64::from(sample.abs()) - *first) * self.needle_step;
            *second += (*first - *second) * self.needle_step;
            self.input_peaks[channel] |= sample.abs() >= hot;
        }
    }
    #[inline]
    pub(super) fn output(&mut self, left: f32, right: f32) {
        self.loudness.push(left, right);
        let peak = self.true_peaks[0]
            .push(left)
            .max(self.true_peaks[1].push(right));
        self.block_peak = self.block_peak.max(peak);
    }
    /// The interpolator still holds the last output intervals. Complete
    /// only that detector; silence here is not part of the played program.
    pub(super) fn finish(&mut self, shared: &Shared) {
        let mut tail = 0.0_f32;
        for _ in 0..TruePeak::DELAY {
            tail = tail
                .max(self.true_peaks[0].push(0.0))
                .max(self.true_peaks[1].push(0.0));
        }
        self.peak = self.peak.max(tail);
        let tail_db = 20.0 * tail.log10();
        if tail_db > OVER_DBTP {
            self.overs += 1;
        }
        let mut reading = shared.read();
        reading.true_peak = reading.true_peak.max(tail_db);
        reading.true_peak_max = 20.0 * self.peak.log10();
        reading.overs = self.overs;
        shared.store(&reading);
    }
    /// Publish the block's reading and start the next block.
    pub(super) fn publish(&mut self, shared: &Shared, compressor: f32, limiter: f32) {
        let decibels = |gain: f64| 20.0 * gain.log10();
        let needle = self.needle[0][1].max(self.needle[1][1]);
        let block_peak = decibels(f64::from(self.block_peak)) as f32;
        self.peak = self.peak.max(self.block_peak);
        if block_peak > OVER_DBTP {
            self.overs += 1;
        }
        shared.store(&MeterReading {
            run: self.run,
            input_vu: (decibels(needle) + SINE_FORM_DB - REFERENCE_DBFS) as f32,
            input_peaks: std::mem::take(&mut self.input_peaks),
            compressor_reduction: compressor,
            limiter_reduction: limiter,
            momentary: self.loudness.momentary as f32,
            short_term: self.loudness.short_term as f32,
            integrated: self.loudness.integrated as f32,
            true_peak: block_peak,
            true_peak_max: decibels(f64::from(self.peak)) as f32,
            overs: self.overs,
        });
        self.block_peak = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f64, dbfs: f64, seconds: f64, rate: f64) -> impl Iterator<Item = f32> {
        let amplitude = db_to_gain(dbfs);
        (0..(seconds * rate) as usize)
            .map(move |i| (amplitude * (2.0 * PI * hz * i as f64 / rate).sin()) as f32)
    }

    #[test]
    fn k_weighting_matches_the_standard_at_48_khz() {
        // b0, b1, b2, a1, a2 as BS.1770-4 lists them.
        let expected = [
            [
                1.535_124_859_586_97,
                -2.691_696_189_406_38,
                1.198_392_810_852_85,
                -1.690_659_293_182_41,
                0.732_480_774_215_85,
            ],
            [1.0, -2.0, 1.0, -1.990_047_454_833_98, 0.990_072_250_366_21],
        ];
        for (stage, expected) in k_weighting(48_000.0).iter().zip(expected) {
            for (got, want) in stage.values().iter().zip(expected) {
                assert!((got - want).abs() < 1e-8, "{got} against {want}");
            }
        }
    }

    #[test]
    fn a_sine_at_minus_23_dbfs_reads_minus_23_lufs() {
        // EBU Tech 3341 case 1: 1 kHz in both channels at -23 dBFS reads
        // -23 LUFS momentary, short-term, and integrated, within 0.1 LU.
        for rate in [RATE, 48_000.0] {
            let mut loudness = Loudness::at(rate);
            for sample in tone(1000.0, -23.0, 20.0, rate) {
                loudness.push(sample, sample);
            }
            // The step length is fixed in frames, so at 48 kHz steps are
            // shorter; the levels still hold for a steady tone.
            for value in [loudness.momentary, loudness.short_term, loudness.integrated] {
                assert!((value + 23.0).abs() < 0.1, "{value} at {rate}");
            }
        }
    }

    #[test]
    fn integrated_loudness_retains_audio_above_full_scale() {
        // +24 dB is reachable with the input trim. The larger value
        // checks that the histogram also covers finite f32 audio.
        for level in [24.0, 700.0] {
            let mut loudness = Loudness::new();
            for sample in tone(1000.0, level, 4.0, RATE) {
                loudness.push(sample, sample);
            }
            assert!(
                (loudness.integrated - level).abs() < 0.1,
                "{}",
                loudness.integrated
            );
        }
    }

    #[test]
    fn integrated_loudness_gates_out_quiet_passages() {
        // EBU Tech 3341 case 3 in shape: -36, -23, and -36 dBFS sections
        // integrate to -23 LUFS, because the relative gate drops the quiet
        // ones. A silent section below the absolute gate also drops out.
        let mut loudness = Loudness::new();
        for (dbfs, seconds) in [(-36.0, 10.0), (-23.0, 60.0), (-36.0, 10.0)] {
            for sample in tone(1000.0, dbfs, seconds, RATE) {
                loudness.push(sample, sample);
            }
        }
        for _ in 0..(RATE * 10.0) as usize {
            loudness.push(0.0, 0.0);
        }
        assert!(
            (loudness.integrated + 23.0).abs() < 0.1,
            "{}",
            loudness.integrated
        );
    }

    #[test]
    fn readings_reach_the_window_and_restart_with_each_playback() {
        let shared = Shared::new();
        assert_eq!(shared.read(), MeterReading::default());
        let mut meters = Meters::new(&shared);
        // A -18 dBFS RMS sine reads 0 VU; a sine whose peaks reach 0 dBFS
        // has its true peak there and lights the OVER count.
        for sample in tone(1000.0, -18.0 + 3.010_3, 1.0, RATE) {
            meters.input(sample, sample);
            meters.output(sample, sample);
        }
        meters.publish(&shared, 1.5, 0.0);
        let reading = shared.read();
        assert_eq!(reading.run, 1);
        assert!(reading.input_vu.abs() < 0.1, "{}", reading.input_vu);
        assert_eq!(reading.input_peaks, [false; 2]);
        assert_eq!(reading.compressor_reduction, 1.5);
        assert_eq!(reading.overs, 0);
        for sample in tone(1000.0, 0.0, 0.1, RATE) {
            meters.input(sample, -sample);
            meters.output(sample, -sample);
        }
        meters.publish(&shared, 0.0, 0.0);
        let reading = shared.read();
        assert_eq!(reading.input_peaks, [true; 2]);
        assert!(reading.true_peak.abs() < 0.05 && reading.true_peak_max.abs() < 0.05);
        assert_eq!(reading.overs, 1);

        let next = Meters::new(&shared);
        assert_eq!(next.run, 2);
        assert_eq!(
            shared.read(),
            MeterReading {
                run: 2,
                ..MeterReading::default()
            }
        );
    }
}
