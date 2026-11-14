//! The playback flight recorder. It keeps the last half minute of what the
//! output callback wrote to the device, and of what the device mixer gave
//! the callback, in rings that are allocated when the stream opens. A
//! watcher thread looks at the newest half second four times a second for
//! the sound of a fault: sustained loud broadband noise, a non-finite
//! sample, or a stream error. A capture writes the rings as WAV files and
//! the state of the device and the song as JSON to `diagnostics/<local
//! time>/` in the data folder, when asked or once a few seconds after a
//! fault.
//!
//! The callback side only stores into preallocated atomics and adds to
//! counters: it does not allocate, lock, or wait.
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Seconds of output that a capture holds.
pub(crate) const CAPTURE_SECONDS: usize = 30;
/// Seconds of ring beyond the capture: a callback may overwrite them while
/// a capture copies the ring. A callback writes far less than this.
const MARGIN_SECONDS: usize = 1;
/// How often the watcher looks at the output, and how much it looks at.
const WATCH_PERIOD: Duration = Duration::from_millis(250);
const WINDOW_SECONDS: f64 = 0.5;
/// Consecutive noisy windows that make a fault: four windows of half a
/// second, a quarter second apart, cover 1.25 seconds of noise.
pub(crate) const NOISE_WINDOWS: u32 = 4;
/// A channel is noisy when its level is above this RMS level...
pub(crate) const NOISE_RMS_DBFS: f32 = -15.0;
/// ...and its spectrum between `FLAT_LOW_HZ` and `FLAT_HIGH_HZ` is this flat:
/// the geometric over the arithmetic mean of its averaged power spectrum.
/// White noise measures about 0.98 in a half-second window. Mastered songs
/// measured at most 0.29 and solo hat, cymbal, and noise lanes at most 0.60;
/// a band-pass noise sweep measures about 0.3, as its low bands are empty.
pub(crate) const NOISE_FLATNESS: f32 = 0.75;
const FLAT_LOW_HZ: f64 = 40.0;
const FLAT_HIGH_HZ: f64 = 16_000.0;
/// How long after a fault an automatic capture is saved, so that the ring
/// holds both the onset and the noise.
const AUTO_DELAY: Duration = Duration::from_secs(4);
/// At most one automatic capture in this time.
const AUTO_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// The most events that the log keeps.
const MAX_EVENTS: usize = 256;

/// Two rings of the same length: the device's samples as the callback wrote
/// them, and the mixer's samples before the callback replaced a non-finite
/// one and converted them to the device's sample type.
pub(crate) struct Ring {
    device: Box<[AtomicU32]>,
    source: Box<[AtomicU32]>,
    /// Samples published so far. The writer adds a callback's samples once
    /// it has stored them.
    written: AtomicU64,
    /// Samples at the oldest end of the ring that a capture does not read.
    margin: usize,
}

fn atomics(len: usize) -> Box<[AtomicU32]> {
    std::iter::repeat_with(|| AtomicU32::new(0))
        .take(len)
        .collect()
}

impl Ring {
    fn new(frames: usize, margin_frames: usize, channels: usize) -> Self {
        let len = (frames + margin_frames).max(1) * channels.max(1);
        Self {
            device: atomics(len),
            source: atomics(len),
            written: AtomicU64::new(0),
            margin: margin_frames * channels.max(1),
        }
    }

    fn len(&self) -> usize {
        self.device.len()
    }

    /// The newest published samples, oldest first: at most `max`, whole
    /// frames of `channels`. With `source`, the mixer's samples too.
    /// Returns the published sample position at the end of the copy.
    fn copy(
        &self,
        max: usize,
        channels: usize,
        device: &mut Vec<f32>,
        mut source: Option<&mut Vec<f32>>,
    ) -> u64 {
        let channels = channels.max(1) as u64;
        let len = self.len() as u64;
        let end = self.written.load(Ordering::Acquire);
        let count = (max as u64).min(len - self.margin as u64).min(end);
        let count = count - count % channels;
        let start = end - count;
        device.clear();
        device.extend((start..end).map(|index| {
            f32::from_bits(self.device[(index % len) as usize].load(Ordering::Relaxed))
        }));
        if let Some(source) = source.as_deref_mut() {
            source.clear();
            source.extend((start..end).map(|index| {
                f32::from_bits(self.source[(index % len) as usize].load(Ordering::Relaxed))
            }));
        }
        // A callback that ran during the copy can have stored up to a
        // margin of samples past what it published, over the oldest ones.
        let after = self.written.load(Ordering::Acquire);
        let intact = (after + self.margin as u64).saturating_sub(len);
        if intact > start {
            let stale = ((intact - start).div_ceil(channels) * channels).min(count) as usize;
            device.drain(..stale);
            if let Some(source) = source {
                source.drain(..stale);
            }
        }
        end
    }
}

/// The ring, the callback's counters, and the count of sources on the
/// device's mixer, shared by the callback, the watcher, and the output.
pub(crate) struct Flight {
    pub(crate) channels: u16,
    pub(crate) sample_rate: u32,
    ring: Ring,
    writer_taken: AtomicBool,
    non_finite: AtomicU64,
    callbacks: AtomicU64,
    max_fill_ns: AtomicU64,
    late_fills: AtomicU64,
    stream_errors: AtomicU64,
    /// Sources added to the device's mixer that it has not dropped yet.
    pub(crate) sources: Arc<AtomicUsize>,
    /// The watcher's latest reading of each channel.
    readings: Mutex<Vec<Reading>>,
}

impl Flight {
    pub(crate) fn new(channels: u16, sample_rate: u32) -> Arc<Self> {
        Self::with_seconds(channels, sample_rate, CAPTURE_SECONDS, MARGIN_SECONDS)
    }

    fn with_seconds(
        channels: u16,
        sample_rate: u32,
        seconds: usize,
        margin_seconds: usize,
    ) -> Arc<Self> {
        let rate = sample_rate as usize;
        Arc::new(Self {
            channels,
            sample_rate,
            ring: Ring::new(seconds * rate, margin_seconds * rate, usize::from(channels)),
            writer_taken: AtomicBool::new(false),
            non_finite: AtomicU64::new(0),
            callbacks: AtomicU64::new(0),
            max_fill_ns: AtomicU64::new(0),
            late_fills: AtomicU64::new(0),
            stream_errors: AtomicU64::new(0),
            sources: Arc::new(AtomicUsize::new(0)),
            readings: Mutex::new(Vec::new()),
        })
    }

    /// The callback's side of the ring. There is one writer.
    pub(crate) fn writer(self: &Arc<Self>) -> Writer {
        assert!(
            !self.writer_taken.swap(true, Ordering::Relaxed),
            "The flight recorder has one writer."
        );
        Writer {
            flight: Arc::clone(self),
            slot: (self.ring.written.load(Ordering::Relaxed) % self.ring.len() as u64) as usize,
            count: 0,
        }
    }

    /// Count one callback that took `elapsed` to write `frames`. A fill
    /// longer than frames/rate exceeds this modeled budget; it does not
    /// establish the callback cadence or a physical underrun.
    #[inline]
    pub(crate) fn record_fill(&self, frames: usize, elapsed: Duration) {
        let nanos = elapsed.as_nanos().min(u128::from(u64::MAX)) as u64;
        self.callbacks.fetch_add(1, Ordering::Relaxed);
        self.max_fill_ns.fetch_max(nanos, Ordering::Relaxed);
        let budget = frames as u64 * 1_000_000_000 / u64::from(self.sample_rate.max(1));
        if nanos > budget {
            self.late_fills.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn record_stream_error(&self) {
        self.stream_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn mixer_sources(&self) -> usize {
        self.sources.load(Ordering::Relaxed)
    }

    fn counters(&self) -> Value {
        json!({
            "callbacks": self.callbacks.load(Ordering::Relaxed),
            "max_callback_fill_micros": self.max_fill_ns.load(Ordering::Relaxed) / 1000,
            "late_callbacks": self.late_fills.load(Ordering::Relaxed),
            "non_finite_samples_replaced": self.non_finite.load(Ordering::Relaxed),
            "stream_errors": self.stream_errors.load(Ordering::Relaxed),
            "mixer_sources": self.mixer_sources(),
        })
    }
}

/// What the output callback records of each sample it writes.
pub(crate) trait Tap {
    /// `source` is the mixer's sample, or `None` when it had none;
    /// `device` is the written sample read back as a float.
    fn sample(&mut self, source: Option<f32>, device: f32);
    /// The callback has written its buffer; `non_finite` samples of it
    /// were written as silence.
    fn finish(&mut self, non_finite: u64);
}

impl Tap for () {
    #[inline]
    fn sample(&mut self, _: Option<f32>, _: f32) {}
    #[inline]
    fn finish(&mut self, _: u64) {}
}

/// The callback's side of the ring.
pub(crate) struct Writer {
    flight: Arc<Flight>,
    slot: usize,
    count: u64,
}

impl Writer {
    /// Counts one device callback of `frames` frames that took `elapsed`.
    pub(crate) fn record_fill(&self, frames: usize, elapsed: Duration) {
        self.flight.record_fill(frames, elapsed);
    }
}

impl Tap for Writer {
    #[inline]
    fn sample(&mut self, source: Option<f32>, device: f32) {
        let ring = &self.flight.ring;
        ring.source[self.slot].store(source.unwrap_or(0.0).to_bits(), Ordering::Relaxed);
        ring.device[self.slot].store(device.to_bits(), Ordering::Relaxed);
        self.slot += 1;
        if self.slot == ring.device.len() {
            self.slot = 0;
        }
        self.count += 1;
    }

    #[inline]
    fn finish(&mut self, non_finite: u64) {
        self.flight
            .ring
            .written
            .fetch_add(self.count, Ordering::Release);
        self.count = 0;
        if non_finite > 0 {
            self.flight
                .non_finite
                .fetch_add(non_finite, Ordering::Relaxed);
        }
    }
}

/// One channel's level and spectral flatness over a window.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub(crate) struct Reading {
    pub(crate) rms_dbfs: f32,
    pub(crate) flatness: f32,
}

impl Reading {
    pub(crate) fn noisy(&self) -> bool {
        self.rms_dbfs > NOISE_RMS_DBFS && self.flatness > NOISE_FLATNESS
    }
}

/// Measures the level and the spectral flatness of each channel of a
/// window, from an averaged power spectrum of half-overlapping Hann
/// segments of about 40 ms.
pub(crate) struct Detector {
    sample_rate: u32,
    channels: usize,
    window: Vec<f32>,
    real: Vec<f32>,
    imaginary: Vec<f32>,
    /// The transform's factors: cos and sin of -2 pi k / size, k < size / 2.
    twiddles: Vec<(f32, f32)>,
    power: Vec<f64>,
}

impl Detector {
    pub(crate) fn new(sample_rate: u32, channels: u16) -> Self {
        let size = (sample_rate as usize / 24)
            .next_power_of_two()
            .clamp(256, 16_384);
        let window = (0..size)
            .map(|n| {
                let phase = std::f64::consts::TAU * n as f64 / size as f64;
                (0.5 - 0.5 * phase.cos()) as f32
            })
            .collect();
        Self {
            sample_rate,
            channels: usize::from(channels.max(1)),
            window,
            real: vec![0.0; size],
            imaginary: vec![0.0; size],
            twiddles: (0..size / 2)
                .map(|k| {
                    let (sin, cos) = (-std::f64::consts::TAU * k as f64 / size as f64).sin_cos();
                    (cos as f32, sin as f32)
                })
                .collect(),
            power: vec![0.0; size / 2 + 1],
        }
    }

    /// The samples of one window at this rate and channel count.
    pub(crate) fn window_samples(&self) -> usize {
        (f64::from(self.sample_rate) * WINDOW_SECONDS) as usize * self.channels
    }

    /// A reading of each channel of `interleaved`.
    pub(crate) fn read(&mut self, interleaved: &[f32]) -> Vec<Reading> {
        (0..self.channels)
            .map(|channel| self.read_channel(interleaved, channel))
            .collect()
    }

    fn read_channel(&mut self, interleaved: &[f32], channel: usize) -> Reading {
        let frames = interleaved.len() / self.channels;
        let sample = |frame: usize| {
            let value = interleaved[frame * self.channels + channel];
            if value.is_finite() { value } else { 0.0 }
        };
        let energy: f64 = (0..frames)
            .map(|frame| f64::from(sample(frame)).powi(2))
            .sum();
        let rms = (energy / frames.max(1) as f64).sqrt();
        let rms_dbfs = if rms > 0.0 {
            (20.0 * rms.log10()) as f32
        } else {
            f32::NEG_INFINITY
        };
        let size = self.window.len();
        let mut segments = 0;
        self.power.fill(0.0);
        let mut start = 0;
        while start + size <= frames {
            for n in 0..size {
                self.real[n] = sample(start + n) * self.window[n];
                self.imaginary[n] = 0.0;
            }
            fft(&mut self.real, &mut self.imaginary, &self.twiddles);
            for (bin, power) in self.power.iter_mut().enumerate() {
                *power +=
                    f64::from(self.real[bin]).powi(2) + f64::from(self.imaginary[bin]).powi(2);
            }
            segments += 1;
            start += size / 2;
        }
        let hertz_per_bin = f64::from(self.sample_rate) / size as f64;
        let low = ((FLAT_LOW_HZ / hertz_per_bin).ceil() as usize).max(1);
        let high = ((FLAT_HIGH_HZ.min(0.45 * f64::from(self.sample_rate)) / hertz_per_bin)
            as usize)
            .min(size / 2);
        let flatness = if segments == 0 || high <= low {
            0.0
        } else {
            let bins = &self.power[low..=high];
            let mean = bins.iter().sum::<f64>() / bins.len() as f64;
            if mean <= 1e-30 {
                0.0
            } else {
                let log_mean = bins
                    .iter()
                    .map(|power| (power + mean * 1e-12).ln())
                    .sum::<f64>()
                    / bins.len() as f64;
                (log_mean.exp() / mean) as f32
            }
        };
        Reading { rms_dbfs, flatness }
    }
}

/// An in-place radix-2 transform of a power-of-two length, with the
/// factors of that length.
fn fft(real: &mut [f32], imaginary: &mut [f32], twiddles: &[(f32, f32)]) {
    let size = real.len();
    let mut j = 0;
    for i in 1..size {
        let mut bit = size >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            real.swap(i, j);
            imaginary.swap(i, j);
        }
    }
    let mut length = 2;
    while length <= size {
        let half = length / 2;
        let stride = size / length;
        for start in (0..size).step_by(length) {
            for k in 0..half {
                let (cos, sin) = twiddles[k * stride];
                let (a, b) = (start + k, start + k + half);
                let t_real = real[b] * cos - imaginary[b] * sin;
                let t_imaginary = real[b] * sin + imaginary[b] * cos;
                real[b] = real[a] - t_real;
                imaginary[b] = imaginary[a] - t_imaginary;
                real[a] += t_real;
                imaginary[a] += t_imaginary;
            }
        }
        length <<= 1;
    }
}

/// One entry of the audio event log.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct Event {
    pub(crate) unix_ms: u64,
    pub(crate) local_time: String,
    pub(crate) kind: &'static str,
    pub(crate) text: String,
}

/// Device opens, replacements, fallbacks, stream errors, faults, and
/// captures of this process. It outlives each output.
static EVENTS: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());

/// Add an event to the log. Not for the data callback: it allocates.
pub(crate) fn log(kind: &'static str, text: impl Into<String>) {
    let event = Event {
        unix_ms: unix_ms(),
        local_time: local_time().1,
        kind,
        text: text.into(),
    };
    if let Ok(mut events) = EVENTS.lock() {
        if events.len() == MAX_EVENTS {
            events.pop_front();
        }
        events.push_back(event);
    }
}

pub(crate) fn events() -> Vec<Event> {
    EVENTS
        .lock()
        .map(|events| events.iter().cloned().collect())
        .unwrap_or_default()
}

/// The song's side of a capture: where the data folder is, and what the
/// session was doing.
#[derive(Clone, Debug)]
pub(crate) struct Context {
    pub(crate) data_root: PathBuf,
    pub(crate) state: Value,
}

/// The session's latest context, for an automatic capture.
static CONTEXT: Mutex<Option<(Instant, Context)>> = Mutex::new(None);

/// Publish the session's context for an automatic capture, at most once
/// in `every`. `make` runs only when the context is due.
pub(crate) fn publish_context(every: Duration, make: impl FnOnce() -> Context) {
    let Ok(mut published) = CONTEXT.lock() else {
        return;
    };
    if published
        .as_ref()
        .is_some_and(|(at, _)| at.elapsed() < every)
    {
        return;
    }
    *published = Some((Instant::now(), make()));
}

fn published_context() -> Option<(Duration, Context)> {
    CONTEXT
        .lock()
        .ok()?
        .as_ref()
        .map(|(at, context)| (at.elapsed(), context.clone()))
}

/// The folder of the latest automatic capture, or why it failed, until the
/// session reports it.
static AUTOMATIC: Mutex<Option<Result<PathBuf, String>>> = Mutex::new(None);

pub(crate) fn take_automatic() -> Option<Result<PathBuf, String>> {
    AUTOMATIC.lock().ok()?.take()
}

/// When the last automatic capture was scheduled, for the rate limit.
static LAST_AUTOMATIC: Mutex<Option<Instant>> = Mutex::new(None);

/// Whether an automatic capture may be scheduled `now`; if so, remember it.
fn allow_automatic(last: &mut Option<Instant>, now: Instant) -> bool {
    if last.is_some_and(|last| now.duration_since(last) < AUTO_INTERVAL) {
        return false;
    }
    *last = Some(now);
    true
}

/// A saved capture: its folder and its `state.json`.
#[derive(Clone, Debug)]
pub(crate) struct Saved {
    pub(crate) folder: PathBuf,
    pub(crate) state: Value,
}

/// Save a capture of `flight` under `data_root/diagnostics`. `device`
/// describes the output; `context` the session, if one is known.
pub(crate) fn save(
    flight: &Flight,
    device: Value,
    reason: &str,
    data_root: &Path,
    context: Option<(Duration, Value)>,
) -> Result<Saved, String> {
    let channels = usize::from(flight.channels.max(1));
    let rate = flight.sample_rate;
    let mut device_samples = Vec::new();
    let mut source_samples = Vec::new();
    flight.ring.copy(
        CAPTURE_SECONDS * rate as usize * channels,
        channels,
        &mut device_samples,
        Some(&mut source_samples),
    );
    let mut detector = Detector::new(rate, flight.channels);
    let tail = device_samples
        .len()
        .saturating_sub(detector.window_samples());
    let readings = detector.read(&device_samples[tail..]);
    let (stamp, local) = local_time();
    let folder = new_folder(&data_root.join("diagnostics"), &stamp)?;
    write_wav(
        &folder.join("device.wav"),
        &device_samples,
        flight.channels,
        rate,
    )?;
    write_wav(
        &folder.join("source.wav"),
        &source_samples,
        flight.channels,
        rate,
    )?;
    let frames = device_samples.len() / channels;
    let (context_age, context) = match context {
        Some((age, context)) => (Some(age.as_millis() as u64), context),
        None => (None, Value::Null),
    };
    let state = json!({
        "reason": reason,
        "captured_at": local,
        "captured_unix_ms": unix_ms(),
        "application_version": crate::APP_VERSION,
        "session": context,
        "session_age_ms": context_age,
        "device": device,
        "callback": flight.counters(),
        "last_half_second": readings,
        "watcher_readings": flight.readings.lock().map(|readings| readings.clone()).unwrap_or_default(),
        "noise_thresholds": {
            "rms_dbfs": NOISE_RMS_DBFS,
            "flatness": NOISE_FLATNESS,
            "band_hz": [FLAT_LOW_HZ, FLAT_HIGH_HZ],
            "windows": NOISE_WINDOWS,
            "window_seconds": WINDOW_SECONDS,
        },
        "events": events(),
        "files": {
            "device.wav": "What the callback wrote to the device, after non-finite samples became silence, read back as 32-bit float.",
            "source.wav": "What the device mixer gave the callback, before that; a non-finite sample stays as it was.",
            "frames": frames,
            "seconds": frames as f64 / f64::from(rate.max(1)),
        },
        "not_recorded": [
            "The negotiated device buffer capacity: the audio library does not report it.",
            "What the system mixer and the driver do after the callback.",
            "Physical underruns: late_callbacks compares fill time with frames/sample rate, \
             not the measured callback cadence.",
            "Per-track levels and the samples of each clip.",
        ],
    });
    let mut bytes = serde_json::to_vec_pretty(&state).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    std::fs::write(folder.join("state.json"), bytes)
        .map_err(|error| format!("Cannot write the capture state: {error}"))?;
    log("capture", format!("{reason}: saved {}", folder.display()));
    Ok(Saved { folder, state })
}

/// A new folder `stamp` in `parent`, or `stamp-2` and so on if it exists.
fn new_folder(parent: &Path, stamp: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Cannot create {}: {error}", parent.display()))?;
    for attempt in 1..1000 {
        let name = if attempt == 1 {
            stamp.to_owned()
        } else {
            format!("{stamp}-{attempt}")
        };
        let folder = parent.join(name);
        match std::fs::create_dir(&folder) {
            Ok(()) => return Ok(folder),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Cannot create {}: {error}", folder.display())),
        }
    }
    Err(format!(
        "Cannot create a capture folder in {}.",
        parent.display()
    ))
}

fn write_wav(path: &Path, samples: &[f32], channels: u16, sample_rate: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: channels.max(1),
        sample_rate: sample_rate.max(1),
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let error = |error: hound::Error| format!("Cannot write {}: {error}", path.display());
    let mut writer = hound::WavWriter::create(path, spec).map_err(error)?;
    for &sample in samples {
        writer.write_sample(sample).map_err(error)?;
    }
    writer.finalize().map_err(error)
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// The local time as a folder name and as ISO 8601 text.
#[cfg(windows)]
fn local_time() -> (String, String) {
    // SAFETY: GetLocalTime only fills the returned structure.
    let time = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    (
        format!(
            "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
            time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
        ),
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}",
            time.wYear,
            time.wMonth,
            time.wDay,
            time.wHour,
            time.wMinute,
            time.wSecond,
            time.wMilliseconds
        ),
    )
}

#[cfg(not(windows))]
fn local_time() -> (String, String) {
    let millis = unix_ms();
    (
        format!("unix-{}", millis / 1000),
        format!("unix-ms {millis}"),
    )
}

/// Stops the watcher thread after it saves any pending capture.
pub(crate) struct Watch {
    stop: Arc<AtomicBool>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Watch `flight` for faults on a thread of its own. `device` describes the
/// output for an automatic capture.
pub(crate) fn watch(
    flight: Arc<Flight>,
    device: impl Fn() -> Value + Send + 'static,
) -> Option<Watch> {
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = Arc::clone(&stop);
    std::thread::Builder::new()
        .name("playback watcher".into())
        .spawn(move || {
            let mut watcher = Watcher::new(&flight, &LAST_AUTOMATIC);
            loop {
                std::thread::sleep(WATCH_PERIOD);
                let stopping = stopped.load(Ordering::Relaxed);
                if let Some(reason) = watcher.check(&flight, Instant::now(), stopping) {
                    let (age, context) = published_context().map_or_else(
                        || (None, crate::project_io::data_root()),
                        |(age, context)| (Some((age, context.state)), context.data_root),
                    );
                    let result =
                        save(&flight, device(), &reason, &context, age).map(|saved| saved.folder);
                    if let Err(error) = &result {
                        log("capture", format!("{reason}: {error}"));
                    }
                    if let Ok(mut automatic) = AUTOMATIC.lock() {
                        *automatic = Some(result);
                    }
                }
                if stopping {
                    break;
                }
            }
        })
        .ok()
        .map(|_| Watch { stop })
}

/// The watcher's state between looks.
struct Watcher {
    /// When the last automatic capture was scheduled, shared by the
    /// watchers of all outputs of the process.
    last: &'static Mutex<Option<Instant>>,
    detector: Detector,
    recent: Vec<f32>,
    /// Published sample position at the last look, and the duration of
    /// consecutive noisy audio in samples, including the initial window.
    written: u64,
    noisy_samples: u64,
    non_finite: u64,
    stream_errors: u64,
    /// A fault and when its capture is due.
    pending: Option<(Instant, String)>,
}

impl Watcher {
    fn new(flight: &Flight, last: &'static Mutex<Option<Instant>>) -> Self {
        Self {
            last,
            detector: Detector::new(flight.sample_rate, flight.channels),
            recent: Vec::new(),
            written: 0,
            noisy_samples: 0,
            non_finite: 0,
            stream_errors: 0,
            pending: None,
        }
    }

    /// Look at the newest window. Returns the reason for a capture that is
    /// due now. When stopping, return pending evidence without the delay.
    fn check(&mut self, flight: &Flight, now: Instant, stopping: bool) -> Option<String> {
        let samples = self.detector.window_samples();
        let end = flight.ring.copy(
            samples,
            usize::from(flight.channels),
            &mut self.recent,
            None,
        );
        let advance = end.saturating_sub(self.written);
        self.written = end;
        // A stalled stream supplies no new evidence. A gap larger than a
        // window also breaks the run: the intervening audio was not read.
        if advance == 0 || advance > samples as u64 {
            self.noisy_samples = 0;
        }
        let mut fault = None;
        // A window that the ring cannot fill yet is no reading.
        if advance > 0 && self.recent.len() == samples {
            let readings = self.detector.read(&self.recent);
            let noisy = readings.iter().copied().filter(Reading::noisy).fold(
                None::<Reading>,
                |loudest, reading| match loudest {
                    Some(loudest) if loudest.rms_dbfs >= reading.rms_dbfs => Some(loudest),
                    _ => Some(reading),
                },
            );
            let required = samples as u64 + samples as u64 / 2 * u64::from(NOISE_WINDOWS - 1);
            let previous = self.noisy_samples;
            self.noisy_samples = if noisy.is_none() {
                0
            } else if previous == 0 {
                samples as u64
            } else {
                previous.saturating_add(advance)
            };
            if let Some(reading) = noisy
                && previous < required
                && self.noisy_samples >= required
            {
                fault = Some(format!(
                    "Sustained broadband noise in the output: {:.1} dBFS RMS, spectral flatness {:.2}",
                    reading.rms_dbfs, reading.flatness
                ));
            }
            if let Ok(mut latest) = flight.readings.lock() {
                *latest = readings;
            }
        }
        let non_finite = flight.non_finite.load(Ordering::Relaxed);
        if non_finite > self.non_finite {
            fault = Some(format!(
                "{} non-finite samples from the mixer were written as silence",
                non_finite - self.non_finite
            ));
            self.non_finite = non_finite;
        }
        let errors = flight.stream_errors.load(Ordering::Relaxed);
        if errors > self.stream_errors {
            fault = Some("The audio stream reported an error".to_owned());
            self.stream_errors = errors;
        }
        if let Some(fault) = fault {
            log("fault", fault.clone());
            let allowed = self.pending.is_none()
                && self
                    .last
                    .lock()
                    .is_ok_and(|mut last| allow_automatic(&mut last, now));
            if allowed {
                self.pending = Some((now + AUTO_DELAY, format!("Automatic: {fault}")));
            }
        }
        match &self.pending {
            Some((due, _)) if stopping || now >= *due => {
                self.pending.take().map(|(_, reason)| reason)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
pub(in crate::audio) mod tests;
