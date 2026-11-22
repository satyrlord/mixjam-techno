//! The live control: the link between the window and the playback thread.
use super::{MasterChain, MeterReading, meters};
use crate::audio::Latest;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering},
};

/// Live meter & gain-reduction snapshot of one FX return bus.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FxBusMeterReading {
    pub peak_l: f32,
    pub peak_r: f32,
    pub rms_l: f32,
    pub rms_r: f32,
    pub ducker_gr_db: f32,
    pub limiter_gr_db: f32,
    pub tail_active: bool,
}

struct FxBusMeterAtomics {
    peak_l: AtomicU32,
    peak_r: AtomicU32,
    rms_l: AtomicU32,
    rms_r: AtomicU32,
    ducker_gr_db: AtomicU32,
    limiter_gr_db: AtomicU32,
    tail_active: AtomicBool,
}
impl FxBusMeterAtomics {
    fn new() -> Self {
        Self {
            peak_l: AtomicU32::new(0.0_f32.to_bits()),
            peak_r: AtomicU32::new(0.0_f32.to_bits()),
            rms_l: AtomicU32::new(0.0_f32.to_bits()),
            rms_r: AtomicU32::new(0.0_f32.to_bits()),
            ducker_gr_db: AtomicU32::new(0.0_f32.to_bits()),
            limiter_gr_db: AtomicU32::new(0.0_f32.to_bits()),
            tail_active: AtomicBool::new(false),
        }
    }
    fn store(&self, r: FxBusMeterReading) {
        self.peak_l.store(r.peak_l.to_bits(), Ordering::Relaxed);
        self.peak_r.store(r.peak_r.to_bits(), Ordering::Relaxed);
        self.rms_l.store(r.rms_l.to_bits(), Ordering::Relaxed);
        self.rms_r.store(r.rms_r.to_bits(), Ordering::Relaxed);
        self.ducker_gr_db
            .store(r.ducker_gr_db.to_bits(), Ordering::Relaxed);
        self.limiter_gr_db
            .store(r.limiter_gr_db.to_bits(), Ordering::Relaxed);
        self.tail_active.store(r.tail_active, Ordering::Relaxed);
    }
    fn load(&self) -> FxBusMeterReading {
        FxBusMeterReading {
            peak_l: f32::from_bits(self.peak_l.load(Ordering::Relaxed)),
            peak_r: f32::from_bits(self.peak_r.load(Ordering::Relaxed)),
            rms_l: f32::from_bits(self.rms_l.load(Ordering::Relaxed)),
            rms_r: f32::from_bits(self.rms_r.load(Ordering::Relaxed)),
            ducker_gr_db: f32::from_bits(self.ducker_gr_db.load(Ordering::Relaxed)),
            limiter_gr_db: f32::from_bits(self.limiter_gr_db.load(Ordering::Relaxed)),
            tail_active: self.tail_active.load(Ordering::Relaxed),
        }
    }
}

/// The link between the window and playback: settings that playback reads
/// while it runs, and the meters it measures. An edit stores new settings;
/// the audio thread takes them at its next block without locking or
/// waiting. The FX buses (see `fx`) take their settings and Clear Tail
/// requests here too,
/// and so do the track levels: each fader, pan, and polarity. Playback
/// keeps its lane compressors' histories here, so a playback started again
/// where the last one was, after a pause or for a mix prepared again,
/// carries on from them.
pub struct MasterControl {
    chain: Latest<MasterChain>,
    pub(super) meters: meters::Shared,
    returns: Latest<crate::fx::FxBuses>,
    fx_meters: [FxBusMeterAtomics; crate::fx::BUSES],
    fx_solo_mask: AtomicU8,
    tail_clears: [AtomicU64; crate::fx::BUSES],
    /// Each track's signed gain (including master gain) and pan knob packed
    /// into one atomic value. A prepared mix decides whether its pan lane
    /// replaces that knob. Both values are NaN until set.
    gain_and_pan: [AtomicU64; crate::engine::TRACK_COUNT],
    /// Serializes publishers and remembers which lane compressors are on.
    level_writer: Mutex<u64>,
    /// Odd while levels and master gain are being published. Audio makes
    /// one bounded snapshot attempt and keeps its last snapshot on overlap.
    level_sequence: AtomicU64,
    /// The master gain inside those levels, as f32 bits. NaN until set.
    /// It is not part of any level pair: a lane ducker's key reads it to
    /// leave the master gain out of its level.
    master_gain: AtomicU32,
    /// Each track's compressor history as playback's last block left it
    /// (`compressor::State::pack`), the frame that block ended at, and its
    /// source epoch, so
    /// that a new playback start just behind that frame carries on from
    /// it. NaN until a playback stores one. Only the audio thread uses them.
    compressors: [[AtomicU64; 3]; crate::engine::TRACK_COUNT],
    compressor_epoch: AtomicU64,
    compressor_reset: AtomicU64,
    compressor_track_reset: [AtomicU64; crate::engine::TRACK_COUNT],
}
impl MasterControl {
    pub fn new(chain: &MasterChain) -> Arc<Self> {
        Arc::new(Self {
            chain: Latest::new(*chain),
            meters: meters::Shared::new(),
            returns: Latest::new(crate::fx::FxBuses::default()),
            fx_meters: std::array::from_fn(|_| FxBusMeterAtomics::new()),
            fx_solo_mask: AtomicU8::new(0),
            tail_clears: std::array::from_fn(|_| AtomicU64::new(0)),
            gain_and_pan: std::array::from_fn(|_| {
                let unset = u64::from(f32::NAN.to_bits());
                AtomicU64::new(unset | (unset << 32))
            }),
            master_gain: AtomicU32::new(f32::NAN.to_bits()),
            level_writer: Mutex::new(0),
            level_sequence: AtomicU64::new(0),
            compressors: std::array::from_fn(|_| {
                let unset = u64::from(f32::NAN.to_bits());
                [
                    AtomicU64::new(unset | (unset << 32)),
                    AtomicU64::new(0),
                    AtomicU64::new(0),
                ]
            }),
            compressor_epoch: AtomicU64::new(0),
            compressor_reset: AtomicU64::new(0),
            compressor_track_reset: std::array::from_fn(|_| AtomicU64::new(0)),
        })
    }
    /// Store the project's track levels. Playback glides to them over about
    /// 23 ms, so a fader moves without a click and without a new mix.
    pub fn set_levels(&self, project: &crate::engine::Project) {
        let mut switched = self.level_writer.lock().unwrap_or_else(|e| e.into_inner());
        let enabled = project
            .tracks
            .iter()
            .enumerate()
            .fold(0_u64, |mask, (index, track)| {
                mask | (u64::from(track.compressor.on) << index)
            });
        if *switched != enabled {
            let epoch = self.compressor_epoch.fetch_add(1, Ordering::AcqRel) + 1;
            for (track, reset) in self.compressor_track_reset.iter().enumerate() {
                if (*switched ^ enabled) & (1_u64 << track) != 0 {
                    reset.store(epoch, Ordering::Release);
                }
            }
            *switched = enabled;
        }
        self.level_sequence.fetch_add(1, Ordering::SeqCst);
        for (stored, track) in self.gain_and_pan.iter().zip(&project.tracks) {
            let gain = u64::from((track.effective_gain() * project.master_gain).to_bits());
            let pan = u64::from(track.pan.to_bits());
            stored.store(gain | (pan << 32), Ordering::SeqCst);
        }
        self.master_gain
            .store(project.master_gain.to_bits(), Ordering::SeqCst);
        self.level_sequence.fetch_add(1, Ordering::SeqCst);
    }
    /// All track levels and their master gain from the same publication.
    /// Apply each pan knob only when the prepared mix has no pan lane.
    /// The edited project's lane may still be waiting for its new mix.
    /// Returns None if unset or a writer overlaps this single attempt.
    pub(crate) fn levels(
        &self,
        automated_pan: &[bool; crate::engine::TRACK_COUNT],
    ) -> Option<([[f32; 2]; crate::engine::TRACK_COUNT], f32)> {
        let before = self.level_sequence.load(Ordering::SeqCst);
        if before & 1 != 0 {
            return None;
        }
        let levels = std::array::from_fn(|track| {
            let packed = self.gain_and_pan[track].load(Ordering::SeqCst);
            let gain = f32::from_bits(packed as u32);
            let pan = if automated_pan[track] {
                0.0
            } else {
                f32::from_bits((packed >> 32) as u32)
            };
            crate::automation::pan_gains(pan).map(|side| gain * side)
        });
        let gain = f32::from_bits(self.master_gain.load(Ordering::SeqCst));
        (before == self.level_sequence.load(Ordering::SeqCst) && gain.is_finite())
            .then_some((levels, gain))
    }
    pub(crate) fn compressor_epoch(&self) -> u64 {
        self.compressor_epoch.load(Ordering::Acquire)
    }
    /// Explicit transport discontinuities discard every track's old
    /// reduction. Switch changes use per-track floors in `set_levels`.
    /// Old playback sources cannot publish into the new epoch.
    pub(crate) fn reset_compressors(&self) {
        let epoch = self.compressor_epoch.fetch_add(1, Ordering::AcqRel) + 1;
        self.compressor_reset.store(epoch, Ordering::Release);
    }
    fn compressor_valid(&self, track: usize, epoch: u64) -> bool {
        epoch >= self.compressor_reset.load(Ordering::Acquire)
            && epoch >= self.compressor_track_reset[track].load(Ordering::Acquire)
    }
    /// Keep a track's compressor history at the end of a playback block,
    /// and the frame after that block. Called by the audio thread.
    pub(crate) fn keep_compressor(
        &self,
        track: usize,
        state: crate::compressor::State,
        at: u64,
        epoch: u64,
    ) {
        if !self.compressor_valid(track, epoch) {
            return;
        }
        let [kept, frame, saved_epoch] = &self.compressors[track];
        frame.store(at, Ordering::Relaxed);
        saved_epoch.store(epoch, Ordering::Relaxed);
        kept.store(state.pack(), Ordering::Release);
    }
    /// The compressor history that playback last kept for a track, if any,
    /// with the frame after the block that left it.
    pub(crate) fn kept_compressor(&self, track: usize) -> Option<(crate::compressor::State, u64)> {
        let [kept, frame, saved_epoch] = &self.compressors[track];
        crate::compressor::State::unpack(kept.load(Ordering::Acquire))
            .filter(|_| self.compressor_valid(track, saved_epoch.load(Ordering::Relaxed)))
            .map(|state| (state, frame.load(Ordering::Relaxed)))
    }
    pub fn set_returns(&self, buses: &crate::fx::FxBuses) {
        let active = buses
            .buses
            .iter()
            .enumerate()
            .fold(0, |mask, (bus, settings)| {
                mask | (u8::from(settings.is_active()) << bus)
            });
        self.fx_solo_mask.fetch_and(active, Ordering::Release);
        self.returns.set(*buses);
    }
    pub fn returns(&self) -> crate::fx::FxBuses {
        self.returns.get()
    }
    /// Cut the tail of one bus: it fades out over 12 ms and its effect
    /// starts again from silence. This is not a setting and not an edit.
    pub fn clear_tail(&self, bus: usize) {
        if let Some(count) = self.tail_clears.get(bus) {
            count.fetch_add(1, Ordering::Release);
        }
    }
    pub(crate) fn tail_clears(&self, bus: usize) -> u64 {
        self.tail_clears[bus].load(Ordering::Acquire)
    }
    /// New bus settings since `seen`. A read that a write overlaps leaves
    /// `seen` unchanged, so the next block tries again.
    pub(crate) fn poll_returns(&self, seen: &mut u64) -> Option<crate::fx::FxBuses> {
        self.returns.newer(seen)
    }
    /// Publish live return & gain-reduction meters for one FX bus (audio thread).
    pub(crate) fn publish_fx_meter(&self, bus: usize, reading: FxBusMeterReading) {
        if let Some(slot) = self.fx_meters.get(bus) {
            slot.store(reading);
        }
    }
    /// Latest live meter readings across all four FX buses.
    pub fn fx_meters(&self) -> [FxBusMeterReading; crate::fx::BUSES] {
        std::array::from_fn(|bus| self.fx_meters[bus].load())
    }
    /// Set monitoring solo for one FX bus (runtime only, never saved).
    pub fn set_fx_solo(&self, bus: usize, solo: bool) {
        if bus >= crate::fx::BUSES {
            return;
        }
        let bit = 1_u8 << bus;
        if solo && self.returns().buses[bus].is_active() {
            self.fx_solo_mask.fetch_or(bit, Ordering::Release);
        } else {
            self.fx_solo_mask.fetch_and(!bit, Ordering::Release);
        }
    }
    pub fn fx_solo(&self, bus: usize) -> bool {
        bus < crate::fx::BUSES && (self.fx_solo_mask.load(Ordering::Acquire) & (1_u8 << bus)) != 0
    }
    pub fn fx_solo_mask(&self) -> u8 {
        self.fx_solo_mask.load(Ordering::Acquire)
    }
    pub(crate) fn clear_fx_solos(&self) {
        self.fx_solo_mask.store(0, Ordering::Release);
    }
    pub fn set(&self, chain: &MasterChain) {
        self.chain.set(*chain);
    }
    pub fn get(&self) -> MasterChain {
        self.chain.get()
    }
    /// The latest reading of the playback that runs through this control.
    pub fn meters(&self) -> MeterReading {
        self.meters.read()
    }
    /// New settings since `seen`. A read that a write overlaps leaves
    /// `seen` unchanged, so the next block tries again.
    pub(super) fn poll(&self, seen: &mut u64) -> Option<MasterChain> {
        self.chain.newer(seen)
    }
}

#[cfg(test)]
mod tests;
