//! The return stage: runs the four FX buses on the send feeds and adds their
//! returns to the dry mix for live preview (`RenderQuality::Live`) and offline
//! export/stems (`RenderQuality::Offline`).
//!
//! Offline renders keep exact integer dry-path latency compensation (`latency_samples()`)
//! across all active buses and pass all returns through 4x true-peak safety limiting,
//! followed by the post-limiter ducker aligned to the return output time.
use super::parts::{Ducker, Line, SafetyLimiter, ceiling_with_gr};
use super::{BUSES, FxBus, FxBuses, Module, RenderQuality, delay, distortion, reverb};
use crate::master::dsp::Smoothed;
use crate::master::{FxBusMeterReading, MasterControl};
use std::sync::Arc;

/// The safety limiters' lookahead in Live mode: 5 ms.
pub(crate) const LOOKAHEAD: usize = RATE / 200;
/// Maximum offline latency across any active FX bus (Drive 72 + TruePeak limiter 252).
pub(crate) const MAX_OFFLINE_LATENCY: usize =
    distortion::Core::OFFLINE_LATENCY + SafetyLimiter::OFFLINE_LATENCY;
/// The longest tail that playback and export let ring after the song: one minute.
pub(crate) const TAIL_LIMIT: u64 = 60 * RATE as u64;
/// A cut (Clear Tail or a change of effect) fades the return out over 12 ms.
const CUT: usize = RATE * 12 / 1000;
/// A bus sleeps after two seconds without input or internal activity.
const SLEEP_AFTER: usize = 2 * RATE;
const RATE: usize = crate::audio::Mix::SAMPLE_RATE as usize;
const SILENCE: f64 = 1e-6;
/// The frames that a block run goes through bus by bus.
const CHUNK: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Empty,
    Delay,
    Reverb,
    Drive,
}

fn kind(module: &Module) -> Kind {
    match module {
        Module::Empty => Kind::Empty,
        Module::EchoformDelay(_) => Kind::Delay,
        Module::AetherformReverb(_) => Kind::Reverb,
        Module::EmberformDrive(_) => Kind::Drive,
    }
}

impl Kind {
    /// Effect core latency before SafetyLimiter (`Offline` FIR oversampling).
    fn effect_latency(self, quality: RenderQuality) -> usize {
        match (self, quality) {
            (Self::Drive, RenderQuality::Offline) => distortion::Core::OFFLINE_LATENCY,
            (Self::Delay, RenderQuality::Offline) => delay::Core::OFFLINE_LATENCY,
            _ => 0,
        }
    }

    /// Total frames by which this bus's raw wet path follows its input at `quality`.
    fn latency(self, quality: RenderQuality) -> usize {
        match (self, quality) {
            (Self::Empty, _) => 0,
            (Self::Drive, RenderQuality::Live) => 0,
            (Self::Drive, RenderQuality::Offline) => MAX_OFFLINE_LATENCY,
            (Self::Delay | Self::Reverb, RenderQuality::Live) => LOOKAHEAD,
            (Self::Delay, RenderQuality::Offline) => {
                delay::Core::OFFLINE_LATENCY + SafetyLimiter::OFFLINE_LATENCY
            }
            (Self::Reverb, RenderQuality::Offline) => SafetyLimiter::OFFLINE_LATENCY,
        }
    }
}

#[inline]
fn delay_sample(line: &mut Line, sample: f64, delay: usize) -> f64 {
    if delay == 0 {
        return sample;
    }
    line.push(sample as f32);
    f64::from(line.at(delay + 1))
}

struct Bus {
    delay: delay::Core,
    reverb: reverb::Core,
    drive: distortion::Core,
    ducker: Ducker,
    kind: Kind,
    settings: FxBus,
    duck_amount: Smoothed,
    duck_threshold_db: Smoothed,
    bypass: bool,
    /// The input switch, gliding so a switch does not click.
    input: Smoothed,
    level: Smoothed,
    limiter: SafetyLimiter,
    pad: [Line; 2],
    key_delay: [Line; 2],
    quality: RenderQuality,
    pad_frames: usize,
    awake: bool,
    quiet: usize,
    slept: u64,
    cut: usize,
    pending: Option<FxBus>,
}

impl Bus {
    fn new(quality: RenderQuality) -> Self {
        Self {
            delay: delay::Core::new(),
            reverb: reverb::Core::new(),
            drive: distortion::Core::new(),
            ducker: Ducker::new(),
            kind: Kind::Empty,
            settings: FxBus::default(),
            duck_amount: Smoothed::new(0.0),
            duck_threshold_db: Smoothed::new(-27.0),
            bypass: false,
            input: Smoothed::with_time(1.0, 0.003),
            level: Smoothed::with_time(0.0, 0.005),
            limiter: SafetyLimiter::new(),
            pad: [
                Line::new(MAX_OFFLINE_LATENCY),
                Line::new(MAX_OFFLINE_LATENCY),
            ],
            key_delay: [
                Line::new(MAX_OFFLINE_LATENCY),
                Line::new(MAX_OFFLINE_LATENCY),
            ],
            quality,
            pad_frames: 0,
            awake: false,
            quiet: 0,
            slept: 0,
            cut: 0,
            pending: None,
        }
    }

    fn total_wet_latency(&self) -> usize {
        if self.kind == Kind::Empty {
            0
        } else {
            self.kind.latency(self.quality) + self.pad_frames
        }
    }

    /// Take new settings. A different effect waits for a cut of the old
    /// one's tail while it still sounds.
    fn apply(&mut self, settings: &FxBus, bpm: f64, settle: bool) {
        if kind(&settings.module) != self.kind && !settle && self.awake {
            self.pending = Some(*settings);
            if self.cut == 0 {
                self.cut = CUT;
            }
            return;
        }
        self.pending = None;
        if kind(&settings.module) != self.kind {
            self.kind = kind(&settings.module);
            self.silence();
        }
        let compensation = match self.quality {
            RenderQuality::Live => LOOKAHEAD as f64,
            RenderQuality::Offline => 0.0,
        };
        let fresh = settle || !self.awake;
        match &settings.module {
            Module::Empty => {
                self.duck_amount.set(0.0);
                self.duck_threshold_db.set(-27.0);
                self.bypass = true;
            }
            Module::EchoformDelay(delay) => {
                self.delay.set(delay, bpm, compensation);
                if fresh {
                    self.delay.settle();
                }
                self.duck_amount.set(f64::from(delay.duck_amount) / 100.0);
                self.duck_threshold_db
                    .set(f64::from(delay.duck_threshold_db));
                self.ducker.set_release(f64::from(delay.duck_release));
                self.bypass = delay.bypass;
            }
            Module::AetherformReverb(reverb) => {
                self.reverb.set(reverb, compensation);
                if fresh {
                    self.reverb.settle();
                }
                self.duck_amount
                    .set(f64::from(reverb.duck_amount_percent) / 100.0);
                self.duck_threshold_db
                    .set(f64::from(reverb.duck_threshold_db));
                self.ducker.set_release(f64::from(reverb.duck_release_ms));
                self.bypass = reverb.bypass;
            }
            Module::EmberformDrive(drive) => {
                self.drive.set(drive);
                if fresh {
                    self.drive.settle();
                }
                self.duck_amount
                    .set(f64::from(drive.duck_amount_percent) / 100.0);
                self.duck_threshold_db
                    .set(f64::from(drive.duck_threshold_db));
                self.ducker.set_release(f64::from(drive.duck_release_ms));
                self.bypass = drive.bypass;
            }
        }
        self.input.set(if settings.powered { 1.0 } else { 0.0 });
        self.level.set(f64::from(settings.return_level));
        if fresh {
            self.duck_amount.settle();
            self.duck_threshold_db.settle();
        }
        if settle {
            self.input.settle();
            self.level.settle();
        }
        self.settings = *settings;
    }

    fn silence(&mut self) {
        self.delay.clear();
        self.reverb.clear();
        self.drive.clear();
        self.ducker.clear();
        self.limiter.clear();
        self.pad.iter_mut().for_each(Line::clear);
        self.key_delay.iter_mut().for_each(Line::clear);
        self.awake = false;
        self.quiet = 0;
        self.slept = 0;
    }

    /// Sleep, end any cut, and take the effect that waited for the cut.
    fn sleep(&mut self, bpm: f64) {
        self.cut = 0;
        self.silence();
        if let Some(settings) = self.pending.take() {
            self.apply(&settings, bpm, false);
        }
    }

    fn clear_tail(&mut self) {
        if self.awake && self.cut == 0 {
            self.cut = CUT;
        }
    }

    fn duck_key(&self) -> u8 {
        self.settings.module.duck_key()
    }

    #[inline]
    fn ensure_awake(&mut self, fed: bool) -> bool {
        if self.awake {
            return true;
        }
        if !fed || self.kind == Kind::Empty {
            self.slept += 1;
            return false;
        }
        match self.kind {
            Kind::Delay => self.delay.skip(self.slept),
            Kind::Reverb => self.reverb.skip(self.slept, self.quality),
            Kind::Drive | Kind::Empty => {}
        }
        self.awake = true;
        self.quiet = 0;
        self.slept = 0;
        true
    }

    #[inline]
    fn process_effect(&mut self, left: f64, right: f64) -> (f64, f64) {
        match self.kind {
            Kind::Empty => (0.0, 0.0),
            Kind::Delay => self.delay.process_with_quality(left, right, self.quality),
            Kind::Reverb => self.reverb.process_with_quality(left, right, self.quality),
            Kind::Drive => self.drive.process_with_quality(left, right, self.quality),
        }
    }

    #[inline]
    fn limit_and_align(&mut self, wet_l: f64, wet_r: f64, lane: Option<f64>) -> (f64, f64, f64) {
        let level = match lane {
            Some(value) => {
                self.level.hold(value);
                value
            }
            None => self.level.next(),
        };
        let (lim_l, lim_r, lim_gr) = match (self.kind, self.quality) {
            (Kind::Drive, RenderQuality::Live) => {
                ceiling_with_gr(wet_l * level, wet_r * level, self.settings.limiter)
            }
            _ => self.limiter.process_quality(
                wet_l * level,
                wet_r * level,
                self.settings.limiter,
                self.quality,
            ),
        };
        let aligned_l = delay_sample(&mut self.pad[0], lim_l, self.pad_frames);
        let aligned_r = delay_sample(&mut self.pad[1], lim_r, self.pad_frames);
        (aligned_l, aligned_r, lim_gr)
    }

    #[inline]
    fn apply_cut(&mut self, out: &mut (f64, f64), bpm: f64) -> bool {
        if self.cut == 0 {
            return true;
        }
        self.cut -= 1;
        if self.cut == 0 {
            self.sleep(bpm);
            return false;
        }
        let scale = self.cut as f64 / CUT as f64;
        out.0 *= scale;
        out.1 *= scale;
        true
    }

    #[inline]
    fn update_sleep(&mut self, fed: bool, bpm: f64) {
        let (activity, wait) = match self.kind {
            Kind::Delay => (
                self.delay.activity(),
                SLEEP_AFTER
                    + self.delay.silent_gap()
                    + MAX_OFFLINE_LATENCY.saturating_sub(self.kind.effect_latency(self.quality)),
            ),
            Kind::Reverb => (
                self.reverb.activity(),
                SLEEP_AFTER + self.total_wet_latency(),
            ),
            Kind::Drive => (
                self.drive.activity(),
                SLEEP_AFTER + self.total_wet_latency(),
            ),
            Kind::Empty => (0.0, SLEEP_AFTER),
        };
        if fed || activity > SILENCE {
            self.quiet = 0;
        } else {
            self.quiet += 1;
            if self.quiet > wait {
                self.sleep(bpm);
            }
        }
    }

    /// Returns `(left, right, ducker_gr_db, limiter_gr_db)`.
    #[inline]
    fn process(
        &mut self,
        left: f64,
        right: f64,
        key: Option<(f64, f64)>,
        lane: Option<f64>,
        bpm: f64,
    ) -> (f64, f64, f64, f64) {
        let gate = self.input.next();
        let (left, right) = (left * gate, right * gate);
        let fed = left != 0.0 || right != 0.0;
        // The live effect taps already compensate for limiter lookahead.
        // Offline, the wet padding and dry delay put both on the same clock.
        let key_latency = match self.quality {
            RenderQuality::Live => 0,
            RenderQuality::Offline => self.total_wet_latency(),
        };
        let (raw_key_l, raw_key_r) = key.unwrap_or((left, right));
        let key_l = delay_sample(&mut self.key_delay[0], raw_key_l, key_latency);
        let key_r = delay_sample(&mut self.key_delay[1], raw_key_r, key_latency);

        if !self.ensure_awake(fed) {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let (wet_l, wet_r) = self.process_effect(left, right);
        let (aligned_l, aligned_r, lim_gr) = self.limit_and_align(wet_l, wet_r, lane);
        let (duck_gain, raw_duck_gr) = self.ducker.gain_at(
            key_l,
            key_r,
            self.duck_amount.next(),
            self.duck_threshold_db.next(),
        );
        let duck_gr = if self.bypass { 0.0 } else { raw_duck_gr };
        let mut out = (aligned_l * duck_gain, aligned_r * duck_gain);
        if !self.apply_cut(&mut out, bpm) {
            return (0.0, 0.0, 0.0, 0.0);
        }
        self.update_sleep(fed, bpm);
        (out.0, out.1, duck_gr, lim_gr)
    }
}

/// The four buses' return lanes resolved to output frames.
pub struct ReturnLanes {
    lanes: [Option<crate::automation::Envelope>; BUSES],
}

impl ReturnLanes {
    /// `None` when no bus has a return lane.
    pub(crate) fn new(
        lanes: &[Vec<crate::automation::Point>; BUSES],
        frames_per_tick: f64,
    ) -> Option<Self> {
        let lanes = std::array::from_fn(|bus| {
            crate::automation::Envelope::new(&lanes[bus], frames_per_tick)
        });
        lanes.iter().any(Option::is_some).then_some(Self { lanes })
    }

    fn at(&self, bus: usize, frame: u64) -> Option<f64> {
        self.lanes[bus]
            .as_ref()
            .map(|lane| f64::from(lane.at(frame)))
    }
}

/// The return lanes, the block's first song frame, and an optional loop.
pub type Lanes<'a> = Option<(&'a ReturnLanes, u64, Option<[u64; 2]>)>;

fn lane_at(lanes: Lanes<'_>, bus: usize, frame: usize, offset: isize) -> Option<f64> {
    lanes.and_then(|(lanes, first, loop_region)| {
        let shifted = first as isize + frame as isize + offset;
        let mut at = shifted.max(0) as u64;
        if let Some([start, end]) = loop_region.filter(|[start, end]| start < end)
            && at >= end
        {
            at = start + (at - end) % (end - start);
        }
        lanes.at(bus, at)
    })
}

#[derive(Default, Clone, Copy)]
struct BusMeterAcc {
    peak_l: f64,
    peak_r: f64,
    sum_sq_l: f64,
    sum_sq_r: f64,
    duck_gr_db: f64,
    lim_gr_db: f64,
}

impl BusMeterAcc {
    #[inline]
    fn record(&mut self, l: f64, r: f64, duck_gr: f64, lim_gr: f64) {
        self.peak_l = self.peak_l.max(l.abs());
        self.peak_r = self.peak_r.max(r.abs());
        self.sum_sq_l += l * l;
        self.sum_sq_r += r * r;
        self.duck_gr_db = self.duck_gr_db.max(duck_gr);
        self.lim_gr_db = self.lim_gr_db.max(lim_gr);
    }

    fn finish(self, frames: usize, tail_active: bool) -> FxBusMeterReading {
        let n = frames.max(1) as f64;
        FxBusMeterReading {
            peak_l: self.peak_l as f32,
            peak_r: self.peak_r as f32,
            rms_l: (self.sum_sq_l / n).sqrt() as f32,
            rms_r: (self.sum_sq_r / n).sqrt() as f32,
            ducker_gr_db: self.duck_gr_db as f32,
            limiter_gr_db: self.lim_gr_db as f32,
            tail_active,
        }
    }
}

/// Runs the buses over blocks of the mix. Its state carries from one block
/// to the next.
pub struct ReturnsProcessor {
    control: Option<(Arc<MasterControl>, u64, [u64; BUSES])>,
    bpm: f64,
    quality: RenderQuality,
    dry_latency: usize,
    dry_delay: [Line; 2],
    buses: Box<[Bus; BUSES]>,
}

impl ReturnsProcessor {
    /// Offline processor at `RenderQuality::Offline` with integer dry PDC.
    pub fn new(settings: &FxBuses, bpm: f64) -> Self {
        Self::with_quality(settings, bpm, RenderQuality::Offline)
    }

    fn with_quality(settings: &FxBuses, bpm: f64, quality: RenderQuality) -> Self {
        super::prepare_tables();
        let mut buses: Box<[Bus; BUSES]> = Box::new(std::array::from_fn(|_| Bus::new(quality)));
        for (bus, settings) in buses.iter_mut().zip(&settings.buses) {
            bus.apply(settings, bpm, true);
        }
        let dry_latency = match quality {
            RenderQuality::Live => 0,
            RenderQuality::Offline => buses
                .iter()
                .map(|b| b.kind.latency(quality))
                .max()
                .unwrap_or(0),
        };
        for bus in buses.iter_mut() {
            bus.pad_frames = if bus.kind == Kind::Empty {
                0
            } else {
                dry_latency.saturating_sub(bus.kind.latency(quality))
            };
        }
        Self {
            control: None,
            bpm,
            quality,
            dry_latency,
            dry_delay: [
                Line::new(MAX_OFFLINE_LATENCY),
                Line::new(MAX_OFFLINE_LATENCY),
            ],
            buses,
        }
    }

    /// Live processor (`RenderQuality::Live`, 0 integer dry PDC) taking updates from `control`.
    pub fn live(control: Arc<MasterControl>, bpm: f64) -> Self {
        let mut seen = 0;
        let settings = control
            .poll_returns(&mut seen)
            .unwrap_or_else(|| control.returns());
        let mut processor = Self::with_quality(&settings, bpm, RenderQuality::Live);
        let clears = std::array::from_fn(|bus| control.tail_clears(bus));
        processor.control = Some((control, seen, clears));
        processor
    }

    /// Constant integer dry-path latency compensation in frames (`0` in Live).
    pub fn latency_samples(&self) -> usize {
        self.dry_latency
    }

    fn key_slots(&self, keys: &crate::ducker::Keys) -> [KeySlot; BUSES] {
        std::array::from_fn(|bus| KeySlot::new(keys, self.buses[bus].duck_key()))
    }

    /// Whether any bus still sounds.
    pub fn ringing(&self) -> bool {
        self.buses.iter().any(|bus| bus.awake)
    }

    fn poll(&mut self) {
        let Some((control, seen, clears)) = &mut self.control else {
            return;
        };
        if let Some(settings) = control.poll_returns(seen) {
            for (bus, settings) in self.buses.iter_mut().zip(&settings.buses) {
                bus.apply(settings, self.bpm, false);
            }
        }
        for (index, bus) in self.buses.iter_mut().enumerate() {
            let count = control.tail_clears(index);
            if count != clears[index] {
                clears[index] = count;
                bus.clear_tail();
            }
        }
    }

    fn publish_meters(&self, stats: &[BusMeterAcc; BUSES], frames: usize) {
        let Some((control, ..)) = &self.control else {
            return;
        };
        for (index, acc) in stats.iter().enumerate() {
            control.publish_fx_meter(index, acc.finish(frames, self.buses[index].awake));
        }
    }

    /// Add the returns to an interleaved stereo block of the dry mix.
    pub fn process(&mut self, block: &mut [f32], sends: Option<&[f32]>) {
        self.process_keyed(block, sends, crate::ducker::Keys::NONE);
    }

    /// `process` with the key tracks of the block, which the duckers whose
    /// key names a track follow.
    pub fn process_keyed(
        &mut self,
        block: &mut [f32],
        sends: Option<&[f32]>,
        keys: crate::ducker::Keys,
    ) {
        self.process_at(block, sends, keys, None);
    }

    /// `process_keyed` with the buses' return lanes from the block's frame.
    pub fn process_at(
        &mut self,
        block: &mut [f32],
        sends: Option<&[f32]>,
        keys: crate::ducker::Keys,
        lanes: Lanes,
    ) {
        self.poll();
        let solo_mask = self.control.as_ref().map_or(0, |(c, ..)| c.fx_solo_mask());
        let frames = block.len() / 2;
        let mut meters = [BusMeterAcc::default(); BUSES];
        if sends.is_some() || self.ringing() {
            let mut slots = self.key_slots(&keys);
            let mut sum = [[0.0_f64; 2]; CHUNK];
            for (number, chunk) in block.as_chunks_mut::<2>().0.chunks_mut(CHUNK).enumerate() {
                self.seed_chunk(chunk, &mut sum, solo_mask);
                self.accumulate_chunk(
                    number * CHUNK,
                    chunk.len(),
                    sends,
                    &keys,
                    &mut slots,
                    lanes,
                    &mut meters,
                    &mut sum,
                    solo_mask,
                );
                for (out, sum) in chunk.iter_mut().zip(&sum) {
                    *out = [sum[0] as f32, sum[1] as f32];
                }
            }
        } else {
            self.process_idle(block, frames, solo_mask);
        }
        self.publish_meters(&meters, frames);
    }

    fn seed_chunk(&mut self, chunk: &[[f32; 2]], sum: &mut [[f64; 2]; CHUNK], solo_mask: u8) {
        for (out, sum) in chunk.iter().zip(sum) {
            let dry = if self.dry_latency > 0 {
                [
                    delay_sample(&mut self.dry_delay[0], f64::from(out[0]), self.dry_latency),
                    delay_sample(&mut self.dry_delay[1], f64::from(out[1]), self.dry_latency),
                ]
            } else {
                [f64::from(out[0]), f64::from(out[1])]
            };
            *sum = if solo_mask == 0 { dry } else { [0.0, 0.0] };
        }
    }

    #[expect(clippy::too_many_arguments)]
    fn accumulate_chunk(
        &mut self,
        first: usize,
        count: usize,
        sends: Option<&[f32]>,
        keys: &crate::ducker::Keys,
        slots: &mut [KeySlot; BUSES],
        lanes: Lanes,
        meters: &mut [BusMeterAcc; BUSES],
        sum: &mut [[f64; 2]; CHUNK],
        solo_mask: u8,
    ) {
        for index in 0..BUSES {
            let audible = solo_mask == 0 || (solo_mask & (1 << index)) != 0;
            for (at, sum) in sum[..count].iter_mut().enumerate() {
                let (l, r, duck_gr, lim_gr) =
                    self.bus_frame(index, first + at, sends, keys, &mut slots[index], lanes);
                meters[index].record(l, r, duck_gr, lim_gr);
                if audible {
                    sum[0] += l;
                    sum[1] += r;
                }
            }
        }
    }

    fn process_idle(&mut self, block: &mut [f32], frames: usize, solo_mask: u8) {
        self.sleep_through(frames);
        if self.dry_latency > 0 {
            for out in block.as_chunks_mut::<2>().0.iter_mut() {
                out[0] = delay_sample(&mut self.dry_delay[0], f64::from(out[0]), self.dry_latency)
                    as f32;
                out[1] = delay_sample(&mut self.dry_delay[1], f64::from(out[1]), self.dry_latency)
                    as f32;
            }
        } else if solo_mask != 0 {
            block.fill(0.0);
        }
    }

    /// One frame of bus `index`: its send, key, and return lane.
    #[inline]
    fn bus_frame(
        &mut self,
        index: usize,
        frame: usize,
        sends: Option<&[f32]>,
        keys: &crate::ducker::Keys,
        slot: &mut KeySlot,
        lanes: Lanes,
    ) -> (f64, f64, f64, f64) {
        let (bpm, quality) = (self.bpm, self.quality);
        let bus = &mut self.buses[index];
        let (left, right) = sends.map_or((0.0, 0.0), |sends| {
            let at = frame * 2 * BUSES + index * 2;
            (f64::from(sends[at]), f64::from(sends[at + 1]))
        });
        let key = slot.frame(keys, bus.duck_key(), frame);
        let lane_offset = if quality == RenderQuality::Offline {
            -(bus.kind.effect_latency(quality) as isize)
        } else {
            bus.kind.latency(quality) as isize
        };
        let lane = lane_at(lanes, index, frame, lane_offset);
        bus.process(left, right, key, lane, bpm)
    }

    #[inline]
    fn frame(
        &mut self,
        frame: usize,
        sends: Option<&[f32]>,
        keys: &crate::ducker::Keys,
        slots: &mut [KeySlot; BUSES],
        lanes: Lanes,
    ) -> [(f64, f64, f64, f64); BUSES] {
        let mut returns = [(0.0, 0.0, 0.0, 0.0); BUSES];
        for (index, out) in returns.iter_mut().enumerate() {
            *out = self.bus_frame(index, frame, sends, keys, &mut slots[index], lanes);
        }
        returns
    }

    fn sleep_through(&mut self, frames: usize) {
        for bus in self.buses.iter_mut() {
            bus.slept += frames as u64;
        }
    }

    /// Each bus's return alone, for an export in parts.
    pub fn split(
        &mut self,
        returns: &mut [f32],
        sends: Option<&[f32]>,
        keys: crate::ducker::Keys,
        lanes: Lanes,
    ) {
        self.poll();
        returns.fill(0.0);
        let frames = returns.len() / (2 * BUSES);
        let mut meters = [BusMeterAcc::default(); BUSES];
        if sends.is_none() && !self.ringing() {
            self.sleep_through(frames);
            self.publish_meters(&meters, frames);
            return;
        }
        let mut slots = self.key_slots(&keys);
        for (frame, out) in returns
            .as_chunks_mut::<{ 2 * BUSES }>()
            .0
            .iter_mut()
            .enumerate()
        {
            let bus_frames = self.frame(frame, sends, &keys, &mut slots, lanes);
            for (index, (out_ch, (l, r, duck_gr, lim_gr))) in out
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .zip(bus_frames)
                .enumerate()
            {
                meters[index].record(l, r, duck_gr, lim_gr);
                *out_ch = [l as f32, r as f32];
            }
        }
        self.publish_meters(&meters, frames);
    }
}

struct KeySlot {
    key: u8,
    slot: Option<Option<usize>>,
}

impl KeySlot {
    fn new(keys: &crate::ducker::Keys, key: u8) -> Self {
        Self {
            key,
            slot: keys.slot(key),
        }
    }

    fn frame(&mut self, keys: &crate::ducker::Keys, key: u8, frame: usize) -> Option<(f64, f64)> {
        if key != self.key {
            *self = Self::new(keys, key);
        }
        self.slot.map(|slot| {
            let (left, right) = keys.frame(slot, frame);
            (f64::from(left), f64::from(right))
        })
    }
}

#[cfg(test)]
mod tests;
