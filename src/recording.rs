//! Modern input capture with the original manual's two-bar count-in.
use crate::audio::Pcm;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    cell::UnsafeCell,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub fn count_in(bpm: f64) -> Result<Pcm, String> {
    if !bpm.is_finite() || !crate::engine::TEMPO_RANGE.contains(&bpm) {
        return Err("Effective tempo must be from 7.5 to 1200 BPM.".into());
    }
    let sample_rate = crate::audio::Mix::SAMPLE_RATE;
    let beat_frames = sample_rate as f64 * 60.0 / bpm;
    let mut samples = vec![0.0; (beat_frames * 8.0).round() as usize];
    for beat in 0..8 {
        let start = (beat as f64 * beat_frames).round() as usize;
        let frequency = if beat % 4 == 0 { 2200.0 } else { 1100.0 };
        for frame in 0..441 {
            if let Some(out) = samples.get_mut(start + frame) {
                let time = frame as f64 / sample_rate as f64;
                *out = (0.3
                    * (-time * 400.0).exp()
                    * (std::f64::consts::TAU * frequency * time).sin())
                    as f32;
            }
        }
    }
    Ok(Pcm {
        sample_rate,
        channels: 1,
        samples,
    })
}

/// The mono samples of a take, in chunks. A take has no duration limit, and
/// a chunk is never copied as the take grows; the drain worker thread handles
/// every allocation off the real-time audio input callback.
#[derive(Default)]
struct Take {
    chunks: Vec<Vec<f32>>,
    len: usize,
}
impl Take {
    /// Samples per chunk: 16 MiB, about 95 seconds at 44.1 kHz.
    const CHUNK: usize = 1 << 22;

    #[cfg(test)]
    fn new(samples: &[f32]) -> Self {
        let mut take = Self::default();
        samples.iter().for_each(|&sample| take.push(sample));
        take
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.len
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn push(&mut self, sample: f32) {
        if self
            .chunks
            .last()
            .is_none_or(|chunk| chunk.len() == Self::CHUNK)
        {
            self.chunks.push(Vec::with_capacity(Self::CHUNK));
        }
        if let Some(chunk) = self.chunks.last_mut() {
            chunk.push(sample);
            self.len += 1;
        }
    }

    #[cfg(test)]
    fn clear(&mut self) {
        *self = Self::default();
    }

    /// The take as one buffer. Each chunk is freed once it is copied, so the
    /// copy needs room for the take and one chunk more.
    fn into_samples(self) -> Result<Vec<f32>, String> {
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(self.len)
            .map_err(|_| "There is not enough memory to keep the recording.".to_string())?;
        for chunk in self.chunks {
            samples.extend_from_slice(&chunk);
        }
        Ok(samples)
    }
}

/// Preallocated lock-free SPSC ring (~2 MiB = ~11.88 s at 44.1 kHz).
const RING_CAPACITY: usize = 1 << 19;
const DRAIN_POLL_INTERVAL: Duration = Duration::from_millis(5);
const CAPTURE_OVERFLOW: &str = "The recording buffer filled.";

struct SampleRing {
    slots: Box<[UnsafeCell<f32>]>,
    mask: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

// SAFETY: only the single `CaptureProducer` writes free slots `[tail..head + cap)`
// before advancing `tail` with `Release`; only the single drain consumer reads
// published slots `[head..tail)` before advancing `head` with `Release`.
unsafe impl Sync for SampleRing {}

impl SampleRing {
    fn new(capacity: usize) -> Self {
        let cap = capacity.max(1).next_power_of_two();
        Self {
            slots: (0..cap).map(|_| UnsafeCell::new(0.0)).collect(),
            mask: cap - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn drain_into(&self, take: &mut Take) {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        for index in head..tail {
            // SAFETY: producer published `index` before advancing `tail`.
            let sample = unsafe { *self.slots[index & self.mask].get() };
            take.push(sample);
        }
        if head != tail {
            self.head.store(tail, Ordering::Release);
        }
    }
}

struct Shared {
    ring: SampleRing,
    count_in_remaining: AtomicU64,
    captured_frames: AtomicU64,
    overflow_samples: AtomicU64,
    limit: usize,
    stop: AtomicBool,
    has_error: AtomicBool,
    error: Mutex<Option<String>>,
}

impl Shared {
    fn new(ring_capacity: usize, skip: u64, limit: usize) -> Arc<Self> {
        Arc::new(Self {
            ring: SampleRing::new(ring_capacity),
            count_in_remaining: AtomicU64::new(skip),
            captured_frames: AtomicU64::new(0),
            overflow_samples: AtomicU64::new(0),
            limit,
            stop: AtomicBool::new(false),
            has_error: AtomicBool::new(false),
            error: Mutex::new(None),
        })
    }

    fn set_error(&self, message: String) {
        if let Ok(mut guard) = self.error.lock() {
            *guard = Some(message);
            self.has_error.store(true, Ordering::Release);
        }
    }

    fn error_message(&self) -> Option<String> {
        if self.overflow_samples.load(Ordering::Acquire) != 0 {
            return Some(CAPTURE_OVERFLOW.into());
        }
        self.has_error
            .load(Ordering::Acquire)
            .then(|| self.error.lock().ok().and_then(|guard| guard.clone()))
            .flatten()
    }
}

struct CaptureProducer {
    channels: usize,
    skip: u64,
    captured: usize,
    shared: Arc<Shared>,
}

impl CaptureProducer {
    fn new(channels: usize, skip: u64, shared: Arc<Shared>) -> Self {
        Self {
            channels,
            skip,
            captured: 0,
            shared,
        }
    }

    /// Mix interleaved input frames to mono and push them without locking or
    /// allocating. A trailing partial frame is dropped.
    fn push(&mut self, mut data: impl ExactSizeIterator<Item = f32>) {
        // A take must be contiguous. After the first lost frame, keep only
        // what was recorded before it until the session stops the stream.
        if self.shared.overflow_samples.load(Ordering::Relaxed) != 0 {
            return;
        }
        let frames = data.len() / self.channels;
        let tail = self.shared.ring.tail.load(Ordering::Relaxed);
        let head = self.shared.ring.head.load(Ordering::Acquire);
        let free = self
            .shared
            .ring
            .slots
            .len()
            .saturating_sub(tail.wrapping_sub(head));
        let mut pushed = 0usize;
        let mut dropped = 0u64;
        for index in 0..frames {
            let sum: f32 = data.by_ref().take(self.channels).sum();
            if self.skip > 0 {
                self.skip -= 1;
                continue;
            }
            if self.captured >= self.shared.limit {
                break;
            }
            if pushed == free {
                dropped = (frames - index) as u64;
                break;
            }
            let value = sum / self.channels as f32;
            let sample = if value.is_finite() {
                value.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            // SAFETY: `pushed < free`, so `(tail + pushed) & mask` is not owned by the consumer.
            unsafe {
                *self.shared.ring.slots[(tail.wrapping_add(pushed)) & self.shared.ring.mask]
                    .get() = sample;
            }
            pushed += 1;
            self.captured += 1;
        }
        self.shared
            .count_in_remaining
            .store(self.skip, Ordering::Release);
        if pushed > 0 {
            self.shared
                .ring
                .tail
                .store(tail.wrapping_add(pushed), Ordering::Release);
            self.shared
                .captured_frames
                .store(self.captured as u64, Ordering::Release);
        }
        if dropped > 0 {
            self.shared
                .overflow_samples
                .store(dropped, Ordering::Release);
        }
    }
}

fn spawn_drain_worker(shared: Arc<Shared>) -> std::io::Result<JoinHandle<Take>> {
    thread::Builder::new()
        .name("mixjam-recorder".into())
        .spawn(move || {
            let mut take = Take::default();
            while !shared.stop.load(Ordering::Acquire) {
                shared.ring.drain_into(&mut take);
                thread::park_timeout(DRAIN_POLL_INTERVAL);
            }
            shared.ring.drain_into(&mut take);
            take
        })
}

pub struct RecordingStatus {
    pub count_in_frames_remaining: u64,
    pub captured_frames: u64,
    /// Frames lost in the callback that stopped capture. Later callbacks are ignored.
    pub overflow_samples: u64,
    pub sample_rate: u32,
    pub limit_reached: bool,
    pub error: Option<String>,
}

pub struct Recorder {
    stream: Option<cpal::Stream>,
    shared: Arc<Shared>,
    worker: Option<JoinHandle<Take>>,
    fixture_take: Option<Take>,
    sample_rate: u32,
}

impl Drop for Recorder {
    fn drop(&mut self) {
        drop(self.stream.take());
        self.shared.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}

impl Recorder {
    #[cfg(test)]
    pub(crate) fn fixture() -> Self {
        let take = Take::new(&[0.25, -0.25]);
        let shared = Shared::new(1, 0, crate::audio::MAX_WAV_SAMPLES);
        shared
            .captured_frames
            .store(take.len() as u64, Ordering::Relaxed);
        Self {
            stream: None,
            shared,
            worker: None,
            fixture_take: Some(take),
            sample_rate: 44_100,
        }
    }

    #[cfg(test)]
    fn synthetic(
        channels: usize,
        skip: u64,
        limit: usize,
        ring_capacity: usize,
        with_worker: bool,
    ) -> (Self, CaptureProducer) {
        let shared = Shared::new(ring_capacity, skip, limit);
        let producer = CaptureProducer::new(channels, skip, Arc::clone(&shared));
        let worker = with_worker.then(|| spawn_drain_worker(Arc::clone(&shared)).unwrap());
        let fixture_take = (!with_worker).then(Take::default);
        (
            Self {
                stream: None,
                shared,
                worker,
                fixture_take,
                sample_rate: 44_100,
            },
            producer,
        )
    }

    #[cfg(test)]
    pub(crate) fn overflow_fixture() -> Self {
        let (recorder, mut producer) = Self::synthetic(1, 0, 16, 2, false);
        producer.push([0.25, -0.25, 0.5].into_iter());
        recorder
    }

    pub fn start(bpm: f64) -> Result<Self, String> {
        if !bpm.is_finite() || !crate::engine::TEMPO_RANGE.contains(&bpm) {
            return Err("Effective tempo must be from 7.5 to 1200 BPM.".into());
        }
        let device = cpal::default_host()
            .default_input_device()
            .ok_or("No audio input device is available.")?;
        let supported = device
            .default_input_config()
            .map_err(|e| format!("Cannot read the audio input settings: {e}"))?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        if config.channels == 0
            || config.channels > 32
            || config.sample_rate.0 == 0
            || config.sample_rate.0 > 384000
        {
            return Err("The audio input settings are not supported.".into());
        }
        let sample_rate = config.sample_rate.0;
        let skip = (sample_rate as f64 * 480.0 / bpm).round() as u64;
        let shared = Shared::new(RING_CAPACITY, skip, crate::audio::MAX_WAV_SAMPLES);
        let mut capture = CaptureProducer::new(config.channels as usize, skip, Arc::clone(&shared));
        let errors = Arc::clone(&shared);
        let on_error = move |error: cpal::StreamError| {
            errors.set_error(format!("Audio input failed: {error}"));
        };
        let stream = match format {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _| capture.push(data.iter().copied()),
                on_error,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config,
                move |data: &[i16], _| capture.push(data.iter().map(|x| *x as f32 / 32768.0)),
                on_error,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_input_stream(
                &config,
                move |data: &[u16], _| {
                    capture.push(data.iter().map(|x| (*x as f32 - 32768.0) / 32768.0))
                },
                on_error,
                None,
            ),
            _ => return Err("This audio input sample format is not supported.".into()),
        }
        .map_err(|e| format!("Cannot open the audio input: {e}"));
        Self::start_stream(stream, shared, sample_rate)
    }

    fn start_stream(
        stream: Result<cpal::Stream, String>,
        shared: Arc<Shared>,
        sample_rate: u32,
    ) -> Result<Self, String> {
        let stream = stream?;
        let worker = spawn_drain_worker(Arc::clone(&shared))
            .map_err(|error| format!("Cannot start the recording worker: {error}"))?;
        let recorder = Self {
            stream: Some(stream),
            shared,
            worker: Some(worker),
            fixture_take: None,
            sample_rate,
        };
        recorder
            .stream
            .as_ref()
            .expect("input stream")
            .play()
            .map_err(|e| format!("Cannot start the audio input: {e}"))?;
        Ok(recorder)
    }

    pub fn status(&self) -> RecordingStatus {
        let captured_frames = self.shared.captured_frames.load(Ordering::Acquire);
        RecordingStatus {
            count_in_frames_remaining: self.shared.count_in_remaining.load(Ordering::Acquire),
            captured_frames,
            overflow_samples: self.shared.overflow_samples.load(Ordering::Relaxed),
            sample_rate: self.sample_rate,
            limit_reached: captured_frames >= self.shared.limit as u64,
            error: self.shared.error_message(),
        }
    }

    /// Returns a take and any input error that stopped it.
    pub fn finish(mut self) -> Result<(Pcm, Option<String>), String> {
        drop(self.stream.take());
        self.shared.stop.store(true, Ordering::Release);
        let take = if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            worker
                .join()
                .map_err(|_| "The recording buffer is unavailable.".to_string())?
        } else {
            let mut take = self.fixture_take.take().unwrap_or_default();
            self.shared.ring.drain_into(&mut take);
            take
        };
        // A failed input keeps the audio that it captured before the failure.
        let error = self.shared.error_message();
        if take.is_empty() {
            return Err(error.unwrap_or_else(|| "No audio was recorded after the count-in.".into()));
        }
        Ok((
            Pcm {
                sample_rate: self.sample_rate,
                channels: 1,
                samples: take.into_samples()?,
            },
            error,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_input_open_error_leaves_no_drain_worker() {
        let shared = Shared::new(8, 0, 16);
        let result = Recorder::start_stream(
            Err("Cannot open the audio input.".into()),
            Arc::clone(&shared),
            44_100,
        );
        assert_eq!(
            result.err().as_deref(),
            Some("Cannot open the audio input.")
        );
        assert_eq!(Arc::strong_count(&shared), 1);
    }

    #[test]
    fn count_in_discards_exact_frames_and_capture_is_bounded() {
        let (recorder, mut producer) = Recorder::synthetic(2, 2, 2, 16, false);
        producer.push([1.0, 1.0, 0.5, 0.5, 0.2, 0.6, 0.0, 0.2, 1.0, 1.0].into_iter());
        let status = recorder.status();
        assert_eq!(status.count_in_frames_remaining, 0);
        assert_eq!(status.captured_frames, 2);
        assert!(status.limit_reached);
        assert_eq!(status.overflow_samples, 0);
        let (pcm, error) = recorder.finish().unwrap();
        assert_eq!(pcm.samples, vec![0.4, 0.1]);
        assert_eq!(error, None);
    }

    #[test]
    fn a_take_has_no_duration_limit_before_the_wav_limit() {
        // The WAV limit is the only one: more than 13 hours at 44.1 kHz.
        let (recorder, _) = Recorder::synthetic(1, 0, crate::audio::MAX_WAV_SAMPLES, 16, false);
        assert!(recorder.shared.limit / 44_100 / 3600 >= 13);
        assert_eq!(recorder.shared.limit, crate::audio::MAX_WAV_SAMPLES);
    }

    #[test]
    fn a_take_keeps_its_order_across_chunks() {
        let count = Take::CHUNK * 2 + 3;
        let (recorder, mut producer) =
            Recorder::synthetic(1, 0, crate::audio::MAX_WAV_SAMPLES, 1 << 14, true);
        let values: Vec<f32> = (0..count)
            .map(|index| (index % 1000) as f32 / 1000.0)
            .collect();
        for chunk in values.chunks(1024) {
            producer.push(chunk.iter().copied());
            while recorder.status().captured_frames > 0
                && recorder.shared.ring.tail.load(Ordering::Relaxed)
                    - recorder.shared.ring.head.load(Ordering::Acquire)
                    > (1 << 13)
            {
                thread::sleep(Duration::from_millis(1));
            }
        }
        let (pcm, error) = recorder.finish().unwrap();
        let samples = pcm.samples;
        assert_eq!(samples.len(), count);
        assert!(samples == values);
        assert_eq!(error, None);
    }

    #[test]
    fn a_failed_input_keeps_the_audio_captured_before_the_failure() {
        let recorder = Recorder::fixture();
        recorder.shared.set_error("Audio input failed.".into());
        let (pcm, error) = recorder.finish().unwrap();
        assert_eq!(pcm.samples, vec![0.25, -0.25]);
        assert_eq!(error.as_deref(), Some("Audio input failed."));
        let mut recorder = Recorder::fixture();
        recorder.fixture_take.as_mut().unwrap().clear();
        recorder.shared.set_error("Audio input failed.".into());
        assert_eq!(recorder.finish().unwrap_err(), "Audio input failed.");
    }

    #[test]
    fn synthetic_callbacks_update_lock_free_status_and_drain_across_worker() {
        let (recorder, mut producer) =
            Recorder::synthetic(2, 3, crate::audio::MAX_WAV_SAMPLES, 64, true);
        producer.push([0.9, -0.9, 0.1, 0.3].into_iter());
        let mid = recorder.status();
        assert_eq!(mid.count_in_frames_remaining, 1);
        assert_eq!(mid.captured_frames, 0);
        assert_eq!(mid.overflow_samples, 0);

        producer.push([0.5, 0.5, 0.25, 0.75, -0.5, -0.5].into_iter());
        let done = recorder.status();
        assert_eq!(done.count_in_frames_remaining, 0);
        assert_eq!(done.captured_frames, 2);
        let (pcm, error) = recorder.finish().unwrap();
        assert_eq!(pcm.samples, vec![0.5, -0.5]);
        assert_eq!(error, None);
    }

    #[test]
    fn ring_overflow_reports_exact_dropped_samples_and_keeps_prefix() {
        let (mut recorder, mut producer) =
            Recorder::synthetic(1, 0, crate::audio::MAX_WAV_SAMPLES, 8, false);
        producer.push((0..12).map(|i| (i as f32) / 16.0));
        recorder
            .shared
            .ring
            .drain_into(recorder.fixture_take.as_mut().unwrap());
        producer.push([1.0; 4].into_iter());
        let status = recorder.status();
        assert_eq!(status.captured_frames, 8);
        assert_eq!(status.overflow_samples, 4);
        assert!(!status.limit_reached);
        assert_eq!(status.error.as_deref(), Some(CAPTURE_OVERFLOW));
        let (pcm, error) = recorder.finish().unwrap();
        assert_eq!(error.as_deref(), Some(CAPTURE_OVERFLOW));
        assert_eq!(
            pcm.samples,
            (0..8).map(|i| (i as f32) / 16.0).collect::<Vec<_>>()
        );
    }

    #[test]
    fn metronome_has_eight_beats_and_two_bars() {
        let pcm = count_in(120.0).unwrap();
        assert_eq!(pcm.frames(), 176400);
        for beat in 0..8 {
            let start = beat * 22050;
            assert!(
                pcm.samples[start..start + 441]
                    .iter()
                    .any(|x| x.abs() > 0.01)
            );
            assert!(
                pcm.samples[start + 441..start + 22050]
                    .iter()
                    .all(|x| *x == 0.0)
            );
        }
    }
}
