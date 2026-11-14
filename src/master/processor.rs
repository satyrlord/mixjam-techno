//! The processor: runs the board over blocks of stereo audio, takes new
//! settings, crossfades between boards, and feeds the meters.
use super::board::{Board, KeyFrame, LIVE_LATENCY};
use super::dsp::{self, Smoothed};
use super::{MasterChain, MasterControl, PedalKind, meters};
use std::sync::Arc;

/// The crossfade after a pedal is switched or moved during playback: 30 ms.
pub(super) const FADE: usize = 1323;
/// The meters publish a reading once this many frames, about 23 ms, however
/// short playback's blocks are: more than 30 times a second. A reading holds
/// the true peak of its frames and counts one over if it passes the limit,
/// so its length sets what the window shows and counts.
const PUBLISH_FRAMES: usize = 1024;

/// Runs the board over stereo audio. Its state carries from one block to
/// the next.
pub struct MasterProcessor {
    control: Option<(Arc<MasterControl>, u64)>,
    trim: Smoothed,
    pub(super) board: Box<Board>,
    /// The board before the last switch or move, while it fades out.
    previous: Box<Board>,
    /// Frames left of the crossfade from `previous` to `board`.
    fade: usize,
    /// Let a rewired board fill its delays before it joins the crossfade.
    warmup: usize,
    meters: Option<Box<meters::Meters>>,
    /// Do not measure the padding that playback removes before its first frame.
    meter_lead: usize,
    /// Frames measured since the meters last published.
    unpublished: usize,
}
impl MasterProcessor {
    /// Playback and export start with the Booster compressor's smoothed
    /// controls at their targets, every mastering pedal at its settings,
    /// and every other state cleared. The original starts the smoothed
    /// controls at a fixed value when the library loads and never resets
    /// them; here each playback starts as a settled original would.
    pub fn new(chain: &MasterChain) -> Self {
        Self::with_quality(chain, crate::playback_quality::Oversampling::FourX)
    }

    /// Construct playback saturation at an explicit quality. Exports use `new`.
    pub fn with_quality(
        chain: &MasterChain,
        quality: crate::playback_quality::Oversampling,
    ) -> Self {
        dsp::prepare_tables();
        let mut board = Box::new(Board::with_quality(quality));
        board.apply(chain);
        board.settle();
        Self {
            control: None,
            trim: Smoothed::new(chain.trim_gain()),
            previous: board.clone(),
            board,
            fade: 0,
            warmup: 0,
            meters: None,
            meter_lead: 0,
            unpublished: 0,
        }
    }
    /// A processor that takes its settings from `control` as they change
    /// and measures for its meters.
    pub fn live(control: Arc<MasterControl>) -> Self {
        Self::live_with_quality(control, crate::playback_quality::Oversampling::FourX)
    }

    pub fn live_with_quality(
        control: Arc<MasterControl>,
        quality: crate::playback_quality::Oversampling,
    ) -> Self {
        let mut seen = 0;
        let chain = control.poll(&mut seen).unwrap_or_else(|| control.get());
        let mut processor = Self::with_quality(&chain, quality);
        processor.board.live = true;
        processor.previous.live = true;
        processor.meters = Some(Box::new(meters::Meters::new(&control.meters)));
        processor.meter_lead = LIVE_LATENCY;
        processor.control = Some((control, seen));
        processor
    }
    /// The frames by which the output follows the input.
    pub fn latency(&self) -> usize {
        if self.board.live {
            LIVE_LATENCY
        } else {
            self.board.chain.latency()
        }
    }
    fn update(&mut self, chain: &MasterChain) {
        self.trim.set(chain.trim_gain());
        if self.board.rewired(chain) {
            *self.previous = *self.board;
            self.board.padding = [dsp::Delay::ZERO; 2];
            self.fade = FADE;
            self.warmup = LIVE_LATENCY;
        }
        self.board.apply(chain);
    }
    /// Process interleaved stereo samples in place: the trim, then the hold
    /// to full scale, then the pedals. The trim comes first, so a lower trim
    /// keeps a hot mix from clipping.
    pub fn process(&mut self, block: &mut [f32]) {
        self.process_keyed(block, crate::ducker::Keys::NONE);
    }
    /// `process` with the key tracks of the block, which the keyed pedals
    /// listen to after the same trim.
    pub fn process_keyed(&mut self, block: &mut [f32], keys: crate::ducker::Keys) {
        if self.fade == 0
            && let Some((control, seen)) = &mut self.control
            && let Some(chain) = control.poll(seen)
        {
            self.update(&chain);
        }
        let trimmed = !(self.trim.settled() && self.trim.value() == 1.0);
        if !trimmed && !self.board.active && self.fade == 0 && self.meters.is_none() {
            crate::audio::hold_to_full_scale(block);
            return;
        }
        let slots = self.board.key_slots(&keys);
        let previous_slots = self.previous.key_slots(&keys);
        for (frame, [left, right]) in block.as_chunks_mut::<2>().0.iter_mut().enumerate() {
            let (mut l, mut r) = (*left, *right);
            let mut gain = 1.0;
            if trimmed {
                gain = self.trim.next() as f32;
                (l, r) = (l * gain, r * gain);
            }
            if let Some(meters) = &mut self.meters {
                meters.input(l, r);
            }
            let key = |slots: [Option<Option<usize>>; 4]| -> KeyFrame {
                slots.map(|slot| {
                    slot.map(|slot| {
                        let (key_left, key_right) = keys.frame(slot, frame);
                        (key_left * gain, key_right * gain)
                    })
                })
            };
            (l, r) = (l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0));
            (l, r) = if self.fade > 0 {
                self.crossfade(l, r, key(slots), key(previous_slots))
            } else if self.board.active || self.board.live {
                self.board.process(l, r, key(slots))
            } else {
                (l, r)
            };
            if self.meter_lead > 0 {
                self.meter_lead -= 1;
            } else if let Some(meters) = &mut self.meters {
                meters.output(l, r);
            }
            (*left, *right) = (l, r);
            self.unpublished += 1;
            if self.unpublished == PUBLISH_FRAMES {
                self.publish();
            }
        }
    }
    /// Publish the meters' reading of the frames since the last one.
    fn publish(&mut self) {
        self.unpublished = 0;
        if let (Some(meters), Some((control, _))) = (&mut self.meters, &self.control) {
            let board = &mut self.board;
            let compressor = if board.chain.runs(PedalKind::BusCompressor) {
                board.bus_compressor.take_reduction()
            } else {
                0.0
            };
            let limiter = if board.chain.runs(PedalKind::Limiter) {
                board.limiter.take_reduction()
            } else {
                0.0
            };
            meters.publish(&control.meters, compressor, limiter);
        }
    }
    /// Complete delayed peak measurement after the last audible frame.
    /// This does not run the board or add samples to the loudness windows.
    pub(crate) fn finish(&mut self) {
        if self.unpublished > 0 {
            self.publish();
        }
        if let (Some(meters), Some((control, _))) = (&mut self.meters, &self.control) {
            meters.finish(&control.meters);
        }
    }
    /// Both boards on the same input, the old one fading out along a
    /// raised cosine. The two outputs are nearly the same sound, so their
    /// gains sum to one.
    fn crossfade(
        &mut self,
        left: f32,
        right: f32,
        keys: KeyFrame,
        previous_keys: KeyFrame,
    ) -> (f32, f32) {
        let (new_l, new_r) = self.board.process(left, right, keys);
        let (old_l, old_r) = self.previous.process(left, right, previous_keys);
        if self.warmup > 0 {
            self.warmup -= 1;
            return (old_l, old_r);
        }
        let progress = 1.0 - self.fade as f64 / FADE as f64;
        let weight = (0.5 - 0.5 * (std::f64::consts::PI * progress).cos()) as f32;
        self.fade -= 1;
        (
            old_l + (new_l - old_l) * weight,
            old_r + (new_r - old_r) * weight,
        )
    }
}
