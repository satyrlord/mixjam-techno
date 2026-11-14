//! The device's mixer: what the output callback plays. The window prepares
//! each playback as a voice and hands it to the callback through a wait-free
//! handoff; its stop switch, pause, volume, and position are atomics that
//! the voice shares with the window's handle. Ended voices go back through
//! a second handoff to the retired playback thread, which frees them. The
//! callback renders the voices into a buffer allocated with the mixer,
//! converting each to the device's rate and channels, so it never locks,
//! waits, allocates, or frees.
use super::{
    flight,
    handoff::{Consumer, Producer, handoff},
    output::fill_into,
    source::PlaybackSource,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

/// Voices that the mixer holds at once: the playing one, and replaced ones
/// that play out their block or wait to be freed.
const VOICES: usize = 8;
/// New playback on its way to the callback.
const INCOMING: usize = 16;
/// Ended playback on its way to be freed.
const RETIRED: usize = 64;
/// Frames that the mixer renders at a time; a larger request renders several.
const CHUNK_FRAMES: usize = 1024;
/// How often the retired playback thread looks for playback to free.
const SWEEP: Duration = Duration::from_millis(20);

/// What a voice and the window's handle share.
struct Shared {
    stop: AtomicBool,
    paused: AtomicBool,
    /// The gain, as f32 bits.
    volume: AtomicU32,
    /// Frames that the voice has read from its source.
    played: AtomicU64,
    ended: AtomicBool,
}

/// The window's handle on a playback. Dropping it stops the playback.
pub(super) struct Playback {
    shared: Arc<Shared>,
}

impl Playback {
    /// Set the gain, which the callback takes at its next buffer.
    pub(super) fn set_volume(&self, volume: f32) {
        self.shared
            .volume
            .store(volume.to_bits(), Ordering::Relaxed);
    }

    /// Hold the playback where it is, in silence.
    pub(super) fn pause(&self) {
        self.shared.paused.store(true, Ordering::Relaxed);
    }

    pub(super) fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::Relaxed)
    }

    /// Whether the source has sound left and the playback is not paused.
    pub(super) fn is_playing(&self) -> bool {
        !self.shared.ended.load(Ordering::Acquire) && !self.is_paused()
    }

    /// Frames of the source played so far.
    pub(super) fn played_frames(&self) -> u64 {
        self.shared.played.load(Ordering::Relaxed)
    }

    /// Stop the playback: it plays out the block it is in and ends.
    pub(super) fn end(self) {}
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
    }
}

/// One playback inside the mixer: its source, read at the device's rate.
pub(super) struct Voice {
    source: PlaybackSource,
    shared: Arc<Shared>,
    /// The source's frames per device frame, as a reduced ratio `from / to`.
    from: u64,
    to: u64,
    /// The source frames on either side of the next device frame, and where
    /// the next device frame and the left source frame fall in the ratio's
    /// cycle. `right` is `None` after the source's last frame.
    left: [f32; 2],
    right: Option<[f32; 2]>,
    output_step: u64,
    input_step: u64,
    started: bool,
    /// Whether the source's last frame has played.
    drained: bool,
    played: u64,
    ended: bool,
}

impl Voice {
    fn new(source: PlaybackSource, shared: Arc<Shared>, device_rate: u32) -> Self {
        let rate = u64::from(source.sample_rate().max(1));
        let device = u64::from(device_rate.max(1));
        let common = gcd(rate, device);
        Self {
            source,
            shared,
            from: rate / common,
            to: device / common,
            left: [0.0; 2],
            right: None,
            output_step: 0,
            input_step: 0,
            started: false,
            drained: false,
            played: 0,
            ended: false,
        }
    }

    /// The next frame of the source, both channels (a mono source's twice).
    fn read(&mut self) -> Option<[f32; 2]> {
        let frame = self
            .source
            .next_frame(self.shared.stop.load(Ordering::Acquire));
        self.played += u64::from(frame.is_some());
        frame
    }

    /// Move the left source frame on by one.
    fn step(&mut self) {
        self.input_step += 1;
        if let Some(right) = self.right {
            self.left = right;
        }
        self.right = self.read();
    }

    /// The next device frame, read between source frames along a straight
    /// line, or `None` when the voice ends.
    fn next_frame(&mut self) -> Option<[f32; 2]> {
        if self.from == self.to {
            return self.read();
        }
        if !self.started {
            self.started = true;
            self.left = self.read()?;
            self.right = self.read();
        }
        if self.output_step == self.to {
            self.output_step = 0;
            self.step();
            while self.input_step != self.from {
                self.step();
            }
            self.input_step = 0;
        } else {
            let wanted = (self.from * self.output_step / self.to) % self.from;
            while self.input_step != wanted {
                self.step();
            }
        }
        // After the source's last frame, that frame plays once more.
        let Some(right) = self.right else {
            if self.drained {
                return None;
            }
            self.drained = true;
            return Some(self.left);
        };
        let share = ((self.from * self.output_step) % self.to) as f32 / self.to as f32;
        self.output_step += 1;
        Some(
            [0, 1]
                .map(|channel| self.left[channel] + (right[channel] - self.left[channel]) * share),
        )
    }

    /// Add this voice's next frames to `mixed`, interleaved in `channels`.
    fn add_to(&mut self, mixed: &mut [f32], channels: usize) {
        if self.ended {
            return;
        }
        let stopping = self.shared.stop.load(Ordering::Acquire);
        if self.shared.paused.load(Ordering::Relaxed) {
            // A paused voice has nothing to play out.
            if stopping {
                self.end();
            }
            return;
        }
        let volume = f32::from_bits(self.shared.volume.load(Ordering::Relaxed));
        for frame in mixed.chunks_exact_mut(channels) {
            let Some([left, right]) = self.next_frame() else {
                self.end();
                break;
            };
            // The first channel takes the left, the second the right, and
            // any others stay silent; a mono device takes the left.
            frame[0] += left * volume;
            if let Some(second) = frame.get_mut(1) {
                *second += right * volume;
            }
        }
        self.shared.played.store(self.played, Ordering::Relaxed);
    }

    fn end(&mut self) {
        self.ended = true;
        self.shared.played.store(self.played, Ordering::Relaxed);
        self.shared.ended.store(true, Ordering::Release);
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a.max(1) } else { gcd(b, a % b) }
}

/// The callback's side: the voices it plays and their buffer.
pub(super) struct DeviceMixer {
    channels: usize,
    voices: [Option<Box<Voice>>; VOICES],
    incoming: Consumer<Box<Voice>>,
    retired: Producer<Box<Voice>>,
    mixed: Box<[f32]>,
    /// The voices that the mixer holds, for captures.
    held: Arc<AtomicUsize>,
}

/// The window's side: it hands new playback to the callback.
pub(super) struct MixerInput {
    rate: u32,
    incoming: Producer<Box<Voice>>,
}

/// A mixer for a device of `channels` channels at `rate`, and its input.
/// `held` counts the voices that the mixer holds. This starts the thread
/// that frees retired playback; it ends after the mixer.
pub(super) fn device_mixer(
    channels: u16,
    rate: u32,
    held: Arc<AtomicUsize>,
) -> (DeviceMixer, MixerInput) {
    let channels = usize::from(channels.max(1));
    let (input, incoming) = handoff(INCOMING);
    let (retired, freeing) = handoff(RETIRED);
    // If the thread cannot start, its side of the handoff is gone, and the
    // mixer frees ended voices in place.
    let _ = std::thread::Builder::new()
        .name("retired playback".into())
        .spawn(move || free_retired(freeing));
    (
        DeviceMixer {
            channels,
            voices: std::array::from_fn(|_| None),
            incoming,
            retired,
            mixed: vec![0.0; CHUNK_FRAMES * channels].into_boxed_slice(),
            held,
        },
        MixerInput {
            rate,
            incoming: input,
        },
    )
}

/// Free retired playback as it arrives, until the mixer has gone. A
/// replaced mix can hold hundreds of megabytes of samples.
fn free_retired(mut retired: Consumer<Box<Voice>>) {
    loop {
        let abandoned = retired.abandoned();
        while let Some(voice) = retired.pop() {
            drop(voice);
        }
        if abandoned {
            return;
        }
        std::thread::sleep(SWEEP);
    }
}

impl MixerInput {
    /// Hand `source` to the callback, at `volume`. The callback starts it at
    /// its next buffer. Stop any playback it replaces first.
    pub(super) fn start(
        &mut self,
        source: PlaybackSource,
        volume: f32,
    ) -> Result<Playback, String> {
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            volume: AtomicU32::new(volume.to_bits()),
            played: AtomicU64::new(0),
            ended: AtomicBool::new(false),
        });
        let voice = Box::new(Voice::new(source, Arc::clone(&shared), self.rate));
        self.incoming
            .push(voice)
            .map_err(|_| "The audio output does not take new playback.".to_owned())?;
        Ok(Playback { shared })
    }
}

impl DeviceMixer {
    /// Write the device buffer: the sum of the voices, or silence when there
    /// are none. `tap` records each sample, as `fill_into` describes.
    pub(super) fn fill<T>(&mut self, output: &mut [T], tap: &mut impl flight::Tap)
    where
        T: cpal::Sample + cpal::FromSample<f32>,
        f32: cpal::FromSample<T>,
    {
        self.admit();
        if self.voices.iter().flatten().all(|voice| voice.ended) {
            fill_into(output, &mut std::iter::empty(), tap);
        } else {
            for chunk in output.chunks_mut(self.mixed.len()) {
                let mixed = &mut self.mixed[..chunk.len()];
                mixed.fill(0.0);
                for voice in self.voices.iter_mut().flatten() {
                    voice.add_to(mixed, self.channels);
                }
                fill_into(chunk, &mut mixed.iter().copied(), tap);
            }
        }
        self.retire();
    }

    /// Retire ended voices and take new ones into free places. A new voice
    /// waits in the handoff while every place is taken.
    fn admit(&mut self) {
        self.retire();
        for place in &mut self.voices {
            if place.is_none() {
                match self.incoming.pop() {
                    Some(voice) => *place = Some(voice),
                    None => break,
                }
            }
        }
        self.count();
    }

    /// Hand ended voices to the retired playback thread. A full handoff
    /// keeps them for the next buffer; without that thread they are freed
    /// here, as nothing else can free them.
    fn retire(&mut self) {
        let abandoned = self.retired.abandoned();
        for place in &mut self.voices {
            let Some(voice) = place.take_if(|voice| voice.ended) else {
                continue;
            };
            if abandoned {
                drop(voice);
            } else if let Err(voice) = self.retired.push(voice) {
                *place = Some(voice);
            }
        }
        self.count();
    }

    fn count(&self) {
        let held = self.voices.iter().flatten().count();
        self.held.store(held, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests;
