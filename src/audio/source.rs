//! The playback source that the output device reads in blocks, and the
//! priority of the thread that reads it.
use super::{Frames, Mix, RenderState, hold_to_full_scale};
use crate::{
    fx::{BUSES, ReturnsProcessor},
    master::{MasterControl, MasterProcessor},
};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

/// Reads audio in blocks for the output device, looping when asked to.
pub(super) struct PlaybackSource {
    pub(super) state: RenderState,
    pub(super) audio: Arc<dyn Frames>,
    channels: usize,
    /// The next frame to read into the block.
    pub(super) frame: u64,
    /// The frame after the last one played before stopping or looping.
    end: u64,
    /// The frame where a loop resumes.
    loop_start: Option<u64>,
    pub(super) block: Vec<f32>,
    /// The frames of a whole block: `BLOCK_FRAMES` but in tests.
    block_frames: usize,
    /// The next sample in `block`, and the count of valid samples in it.
    pub(super) cursor: usize,
    pub(super) filled: usize,
    /// The master pedalboard of a stereo mix. Its state runs on through loops.
    master: Option<MasterProcessor>,
    /// The FX buses of a song mix, with the block of their feeds. Their
    /// tails ring on after the song, as in export.
    returns: Option<ReturnsProcessor>,
    sends: Vec<f32>,
    /// Frames of the buses' tail played after the end, and of them those
    /// left of the current `Mix::TAIL_BLOCK`.
    ringing: u64,
    ringing_block: u64,
    /// Frames of the pedals' delay still to drop at the start, so the song
    /// starts on time, and still to play out after the end.
    pub(super) lead: usize,
    tail: usize,
    /// Frames of the block that the key tracks cover: none after the song.
    keyed: usize,
    /// Whether the thread that reads this source has been asked to run at
    /// an audio thread's priority.
    raised: bool,
    /// Whether the pedals have measured their last frame. They stay with the
    /// source, so the audio thread does not free them.
    finished: bool,
}
impl PlaybackSource {
    /// About 5.8 ms. The device's callback, every 10 ms or so, renders the
    /// blocks it needs inside its own period, so a block's work must be a
    /// small part of the period: a callback then renders one or two blocks.
    /// The level glides, the meters, and the tails keep their own timing
    /// (`GLIDE_FRAMES`, `MasterProcessor`, `Mix::TAIL_BLOCK`).
    pub(super) const BLOCK_FRAMES: usize = 256;

    #[cfg(test)]
    pub(super) fn new(
        audio: Arc<dyn Frames>,
        start: u64,
        loop_region: Option<[u64; 2]>,
        master: Option<Arc<MasterControl>>,
    ) -> Self {
        Self::with_block(audio, start, loop_region, master, Self::BLOCK_FRAMES)
    }

    /// A source that renders blocks of `block_frames` frames.
    #[cfg(test)]
    pub(super) fn with_block(
        audio: Arc<dyn Frames>,
        start: u64,
        loop_region: Option<[u64; 2]>,
        master: Option<Arc<MasterControl>>,
        block_frames: usize,
    ) -> Self {
        Self::with_quality_block(
            audio,
            start,
            loop_region,
            master,
            block_frames,
            crate::playback_quality::PlaybackQuality::FULL,
        )
    }

    pub(super) fn with_quality_block(
        audio: Arc<dyn Frames>,
        start: u64,
        loop_region: Option<[u64; 2]>,
        master: Option<Arc<MasterControl>>,
        block_frames: usize,
        quality: crate::playback_quality::PlaybackQuality,
    ) -> Self {
        let channels = audio.channels() as usize;
        let master = master.filter(|_| channels == 2);
        let live = master.clone();
        let returns = master
            .as_ref()
            .zip(audio.tempo())
            .map(|(control, bpm)| ReturnsProcessor::live(Arc::clone(control), bpm));
        let master =
            master.map(|control| MasterProcessor::live_with_quality(control, quality.master));
        let latency = master.as_ref().map_or(0, MasterProcessor::latency);
        let sends = if returns.is_some() {
            vec![0.0; block_frames * 2 * BUSES]
        } else {
            Vec::new()
        };
        let mut state = RenderState {
            compressor_epoch: live.as_ref().map(|live| live.compressor_epoch()),
            live,
            loop_region,
            // Allocate key scratch before the device reads this source.
            // A short first block must also leave room for a full block.
            key_audio: if audio.tempo().is_some() {
                vec![0.0; block_frames * 2 * crate::engine::MAX_KEYS]
            } else {
                Vec::new()
            },
            ..RenderState::default()
        };
        // The drives' histories too.
        audio.prepare_state(&mut state);
        Self {
            state,
            returns,
            sends,
            ringing: 0,
            ringing_block: 0,
            channels,
            frame: start,
            end: loop_region.map_or(audio.frames(), |[_, b]| b),
            loop_start: loop_region.map(|[a, _]| a),
            block: vec![0.0; block_frames * channels],
            block_frames,
            cursor: 0,
            filled: 0,
            master,
            lead: latency,
            tail: latency,
            keyed: 0,
            raised: false,
            finished: false,
            audio,
        }
    }
    /// Read and process the next block that has sound to play. False when
    /// nothing is left.
    pub(super) fn fill(&mut self) -> bool {
        loop {
            if self.frame >= self.end {
                if let Some(start) = self.loop_start {
                    self.frame = start;
                } else if let Some(returns) = self.returns.as_mut().filter(|returns| {
                    // As in export, whether the buses still ring is asked
                    // once a tail block, so playback ends at its frame.
                    self.ringing_block > 0
                        || returns.ringing() && self.ringing < crate::fx::TAIL_LIMIT
                }) {
                    if self.ringing_block == 0 {
                        self.ringing_block =
                            Mix::TAIL_BLOCK.min(crate::fx::TAIL_LIMIT - self.ringing);
                    }
                    let count = self.block_frames.min(self.ringing_block as usize);
                    // The song's frames go on through the tail, as in export.
                    let lanes = self
                        .audio
                        .return_lanes()
                        .map(|lanes| (lanes, self.end + self.ringing, None));
                    self.ringing += count as u64;
                    self.ringing_block -= count as u64;
                    self.filled = count * self.channels;
                    self.keyed = 0;
                    self.block[..self.filled].fill(0.0);
                    returns.process_at(
                        &mut self.block[..self.filled],
                        None,
                        crate::ducker::Keys::NONE,
                        lanes,
                    );
                    if self.process() {
                        return true;
                    }
                    continue;
                } else if self.tail > 0 {
                    let count = self.tail.min(self.block_frames);
                    self.tail -= count;
                    self.filled = count * self.channels;
                    self.keyed = 0;
                    self.block[..self.filled].fill(0.0);
                    if self.process() {
                        return true;
                    }
                    continue;
                } else {
                    if let Some(master) = self.master.as_mut().filter(|_| !self.finished) {
                        master.finish();
                    }
                    self.finished = true;
                    return false;
                }
            }
            let count = ((self.end - self.frame) as usize).min(self.block_frames);
            if count == 0 {
                return false;
            }
            self.filled = count * self.channels;
            self.keyed = count;
            match &mut self.returns {
                Some(returns) => {
                    let sends = &mut self.sends[..count * 2 * BUSES];
                    let sending = self.audio.read_block(
                        self.frame,
                        &mut self.block[..self.filled],
                        Some(sends),
                        &mut self.state,
                    );
                    returns.process_at(
                        &mut self.block[..self.filled],
                        sending.then_some(&*sends),
                        self.audio.keys(&self.state, count),
                        self.audio.return_lanes().map(|lanes| {
                            (
                                lanes,
                                self.frame,
                                self.loop_start.map(|start| [start, self.end]),
                            )
                        }),
                    );
                }
                None => {
                    self.audio.read_block(
                        self.frame,
                        &mut self.block[..self.filled],
                        None,
                        &mut self.state,
                    );
                }
            }
            self.frame += count as u64;
            if self.process() {
                return true;
            }
        }
    }
    /// Run the block through the pedals and drop what remains of their
    /// delay. True when the block still has samples to play.
    fn process(&mut self) -> bool {
        match &mut self.master {
            Some(master) => master.process_keyed(
                &mut self.block[..self.filled],
                self.audio.keys(&self.state, self.keyed),
            ),
            None => hold_to_full_scale(&mut self.block[..self.filled]),
        }
        let skip = self.lead.min(self.filled / self.channels);
        self.lead -= skip;
        self.cursor = skip * self.channels;
        self.cursor < self.filled
    }
    pub(super) fn sample_rate(&self) -> u32 {
        self.audio.sample_rate()
    }
    /// Fill the next block once this one has played. False at the end. The
    /// first fill on a thread asks for an audio thread's priority.
    fn refill(&mut self) -> bool {
        if !self.raised {
            self.raised = true;
            raise_priority_once();
        }
        self.fill()
    }
    /// The next frame, with a mono source's sample on both sides, or `None`
    /// at the end. With `stop`, the source ends once its block has played,
    /// so a replaced playback renders no other block.
    pub(super) fn next_frame(&mut self, stop: bool) -> Option<[f32; 2]> {
        if self.cursor == self.filled && (stop || !self.refill()) {
            return None;
        }
        let left = self.block[self.cursor];
        let right = self.block[self.cursor + self.channels - 1];
        self.cursor += self.channels;
        Some([left, right])
    }
}
impl Iterator for PlaybackSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.cursor == self.filled && !self.refill() {
            return None;
        }
        let value = self.block[self.cursor];
        self.cursor += 1;
        Some(value)
    }
}

/// The error of the last failed request for an audio thread's priority, or 0.
static PRIORITY_ERROR: AtomicU32 = AtomicU32::new(0);

#[cfg(test)]
thread_local! {
    /// Test threads count their priority requests and send them to the
    /// system only when a test asks, so tests do not run at an audio
    /// thread's priority.
    pub(super) static PRIORITY_REQUESTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static SYSTEM_PRIORITY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Ask the system to run the calling thread, the device's, as an audio
/// thread, once per thread. cpal 0.15.3 asks too, but passes a thread id
/// where its request needs a handle, so the system refuses it.
fn raise_priority_once() {
    thread_local! {
        static RAISED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if RAISED.with(|raised| raised.replace(true)) {
        return;
    }
    #[cfg(test)]
    {
        PRIORITY_REQUESTS.with(|count| count.set(count.get() + 1));
        if !SYSTEM_PRIORITY.with(std::cell::Cell::get) {
            return;
        }
    }
    if let Err(error) = raise_thread_priority() {
        PRIORITY_ERROR.store(error.max(1), Ordering::Relaxed);
    }
}

/// The multimedia class scheduler's "Pro Audio" task, or else the highest
/// priority of the process's class. The thread keeps it until it ends.
#[cfg(windows)]
fn raise_thread_priority() -> Result<(), u32> {
    use windows::Win32::System::Threading::{
        AvSetMmThreadCharacteristicsW, GetCurrentThread, SetThreadPriority,
        THREAD_PRIORITY_TIME_CRITICAL,
    };
    let mut task = 0;
    // SAFETY: the task name is a static wide string, and the pseudo handle
    // names the calling thread.
    unsafe {
        if AvSetMmThreadCharacteristicsW(windows::core::w!("Pro Audio"), &mut task).is_ok() {
            return Ok(());
        }
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL)
            .map_err(|error| error.code().0 as u32)
    }
}

#[cfg(not(windows))]
fn raise_thread_priority() -> Result<(), u32> {
    Ok(())
}

/// Why playback runs at a normal thread priority, if the system refused it
/// an audio thread's. Playback goes on either way.
pub fn priority_failure() -> Option<String> {
    let error = PRIORITY_ERROR.load(Ordering::Relaxed);
    (error != 0).then(|| {
        format!(
            "Playback could not raise its thread's priority (error 0x{error:08X}). \
             Audio may break up while the computer is busy."
        )
    })
}

/// Play `audio` as the device would, without a device: through a live
/// `PlaybackSource` from `start`, handing each processed block to `each`
/// until `frames` frames have played or the source ends.
#[cfg(test)]
pub(crate) fn play_offline(
    audio: Arc<dyn Frames>,
    start: u64,
    loop_region: Option<[u64; 2]>,
    master: Option<Arc<MasterControl>>,
    quality: crate::playback_quality::PlaybackQuality,
    frames: u64,
    mut each: impl FnMut(&[f32]),
) {
    let channels = audio.channels() as u64;
    let mut source = PlaybackSource::with_quality_block(
        audio,
        start,
        loop_region,
        master,
        PlaybackSource::BLOCK_FRAMES,
        quality,
    );
    let mut played = 0;
    while played < frames && source.fill() {
        let block = &source.block[source.cursor..source.filled];
        let take = (block.len() as u64 / channels).min(frames - played);
        each(&block[..(take * channels) as usize]);
        played += take;
        source.cursor = source.filled;
    }
}
