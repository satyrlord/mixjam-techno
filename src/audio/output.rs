//! Device output with a selected buffer target. The device can use another
//! callback interval; buffer capacity is not a callback deadline.
use super::{
    device_mixer::{DeviceMixer, MixerInput, Playback, device_mixer},
    flight,
    source::PlaybackSource,
};
use crate::options::PlaybackBufferTarget;
use cpal::{
    BufferSize, SampleFormat, SampleRate, StreamConfig, SupportedBufferSize, SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::Instant,
};

/// The configuration accepted when opening the output stream. A fixed
/// request does not establish the device's callback interval or measured latency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputConfiguration {
    pub channels: u16,
    pub sample_rate: u32,
    /// `None` means that the system default buffer was requested.
    pub requested_buffer_frames: Option<u32>,
    /// CPAL does not expose the negotiated output capacity.
    pub negotiated_buffer_frames: Option<u32>,
    /// The frames requested by the most recent device callback, not capacity.
    pub callback_frames: Option<u32>,
    /// The measured interval between the most recent two callbacks.
    pub callback_period_micros: Option<u64>,
}

#[derive(Default)]
struct CallbackMetrics {
    frames: AtomicU32,
    period_ns: AtomicU64,
}

impl CallbackMetrics {
    fn record(&self, frames: usize, period_ns: Option<u64>) {
        self.frames
            .store(frames.min(u32::MAX as usize) as u32, Ordering::Relaxed);
        if let Some(period_ns) = period_ns {
            self.period_ns.store(period_ns, Ordering::Relaxed);
        }
    }

    fn configuration(&self, mut configuration: OutputConfiguration) -> OutputConfiguration {
        let frames = self.frames.load(Ordering::Relaxed);
        configuration.callback_frames = (frames != 0).then_some(frames);
        let period_ns = self.period_ns.load(Ordering::Relaxed);
        configuration.callback_period_micros = (period_ns != 0).then_some(period_ns / 1000);
        configuration
    }
}

pub(crate) struct Output {
    // Keep the stream alive while the mixer takes playback.
    _stream: cpal::Stream,
    input: MixerInput,
    configuration: OutputConfiguration,
    metrics: Arc<CallbackMetrics>,
    device: cpal::Device,
    format: SupportedStreamConfig,
    target: PlaybackBufferTarget,
    fallback_reason: Option<String>,
    name: String,
    /// The last half minute of what the callback wrote, and its counters.
    flight: Arc<flight::Flight>,
    /// The thread that watches `flight` for faults, while this output lives.
    watch: Option<flight::Watch>,
}

/// What a capture records of an output.
#[derive(Clone)]
struct Described {
    name: String,
    sample_format: String,
    configuration: OutputConfiguration,
    metrics: Arc<CallbackMetrics>,
    target: PlaybackBufferTarget,
    fallback_reason: Option<String>,
    flight: Arc<flight::Flight>,
}

impl Described {
    fn json(&self) -> Value {
        let configuration = self.metrics.configuration(self.configuration);
        json!({
            "name": self.name,
            "sample_rate": configuration.sample_rate,
            "channels": configuration.channels,
            "sample_format": self.sample_format,
            "buffer_target": target_name(self.target),
            "requested_buffer_frames": configuration.requested_buffer_frames,
            "last_callback_frames": configuration.callback_frames,
            "last_callback_period_micros": configuration.callback_period_micros,
            "status": status_text(configuration, self.target, self.fallback_reason.as_deref()),
            "fallback_reason": self.fallback_reason,
            "mixer_sources": self.flight.mixer_sources(),
        })
    }
}

fn target_name(target: PlaybackBufferTarget) -> &'static str {
    match target {
        PlaybackBufferTarget::Automatic => "Automatic",
        PlaybackBufferTarget::Ms20 => "20 ms",
        PlaybackBufferTarget::Ms40 => "40 ms",
    }
}

/// The requested buffer, as the status and the log name it.
fn request_text(configuration: OutputConfiguration) -> String {
    configuration.requested_buffer_frames.map_or_else(
        || "system default".to_owned(),
        |frames| format!("{frames} frames"),
    )
}

/// Unknown ranges need an opening attempt. A known unsupported target must
/// not be presented as an accepted target of another size.
fn requested_buffer(
    rate: u32,
    supported: &SupportedBufferSize,
    target: PlaybackBufferTarget,
) -> Result<BufferSize, String> {
    let millis = match target {
        PlaybackBufferTarget::Automatic => return Ok(BufferSize::Default),
        PlaybackBufferTarget::Ms20 => 20_u32,
        PlaybackBufferTarget::Ms40 => 40_u32,
    };
    let frames =
        ((u64::from(rate) * u64::from(millis) + 500) / 1000).clamp(1, u64::from(u32::MAX)) as u32;
    if let SupportedBufferSize::Range { min, max } = *supported
        && (min > max || frames < min || frames > max || max == 0)
    {
        return Err(format!(
            "The {millis} ms buffer target requests {frames} frames. \
                 The audio device reports a supported range of {min} to {max} frames. \
                 Select Automatic or another target."
        ));
    }
    Ok(BufferSize::Fixed(frames))
}

impl Output {
    /// Startup can fall back to the system default, with a visible reason.
    pub(crate) fn open_with_target(target: PlaybackBufferTarget) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("There is no default audio output device.")?;
        match Self::open_device(&device, target) {
            Ok(output) => Ok(output),
            Err(original) => {
                flight::log(
                    "fallback",
                    format!(
                        "The default output {} did not open: {original}",
                        device.name().unwrap_or_default()
                    ),
                );
                host.output_devices()
                    .ok()
                    .and_then(|mut devices| {
                        devices.find_map(|device| Self::open_device(&device, target).ok())
                    })
                    .ok_or(original)
            }
        }
    }

    fn open_device(device: &cpal::Device, target: PlaybackBufferTarget) -> Result<Self, String> {
        let default = device.default_output_config().map_err(|e| e.to_string())?;
        let original = match Self::open_supported(device, default, target) {
            Ok(output) => return Ok(output),
            Err(error) => error,
        };
        flight::log(
            "fallback",
            format!("The default format did not open: {original} Trying the other formats."),
        );
        let Ok(supported) = device.supported_output_configs() else {
            return Err(original);
        };
        let mut ranges: Vec<_> = supported.collect();
        ranges.sort_by(|a, b| b.cmp_default_heuristics(a));
        for range in ranges {
            let (min, max) = (range.min_sample_rate(), range.max_sample_rate());
            let mut rates = vec![max];
            let mix = SampleRate(super::Mix::SAMPLE_RATE);
            if min < mix && mix < max {
                rates.push(mix);
            }
            if min != max {
                rates.push(min);
            }
            for rate in rates {
                if let Ok(output) =
                    Self::open_supported(device, range.with_sample_rate(rate), target)
                {
                    return Ok(output);
                }
            }
        }
        Err(original)
    }

    fn open_supported(
        device: &cpal::Device,
        format: SupportedStreamConfig,
        target: PlaybackBufferTarget,
    ) -> Result<Self, String> {
        let attempt = requested_buffer(format.sample_rate().0, format.buffer_size(), target)
            .and_then(|requested| Self::open_format(device, &format, requested));
        match attempt {
            Ok(mut output) => {
                output.target = target;
                Ok(output.watched("open"))
            }
            Err(error) if target != PlaybackBufferTarget::Automatic => {
                let mut output = Self::open_format(device, &format, BufferSize::Default)?;
                output.target = target;
                output.fallback_reason = Some(format!(
                    "The requested buffer is unavailable: {error} Using the system default buffer."
                ));
                Ok(output.watched("open"))
            }
            Err(error) => Err(error),
        }
    }

    fn described(&self) -> Described {
        Described {
            name: self.name.clone(),
            sample_format: self.format.sample_format().to_string(),
            configuration: self.configuration,
            metrics: Arc::clone(&self.metrics),
            target: self.target,
            fallback_reason: self.fallback_reason.clone(),
            flight: Arc::clone(&self.flight),
        }
    }

    /// Log the opened output and start watching what it plays for faults.
    fn watched(mut self, kind: &'static str) -> Self {
        let described = self.described();
        let buffer = request_text(self.configuration);
        let fallback = self
            .fallback_reason
            .as_deref()
            .map_or_else(String::new, |reason| format!(" {reason}"));
        flight::log(
            kind,
            format!(
                "{}: {} Hz, {} channels, {}, buffer {buffer}.{fallback}",
                described.name,
                self.configuration.sample_rate,
                self.configuration.channels,
                described.sample_format,
            ),
        );
        self.watch = flight::watch(Arc::clone(&self.flight), move || described.json());
        self
    }

    /// Save a capture of the last half minute that the device played to
    /// `data_root/diagnostics`, with `session` describing the song.
    pub(crate) fn capture(
        &self,
        reason: &str,
        data_root: &Path,
        session: Option<Value>,
    ) -> Result<flight::Saved, String> {
        flight::save(
            &self.flight,
            self.described().json(),
            reason,
            data_root,
            session.map(|session| (std::time::Duration::ZERO, session)),
        )
    }

    /// Prepare a silent replacement on the current device and sample format.
    /// The caller keeps this output until the preference transaction succeeds.
    pub(crate) fn replacement(&self, target: PlaybackBufferTarget) -> Result<Self, String> {
        let buffer = requested_buffer(
            self.format.sample_rate().0,
            self.format.buffer_size(),
            target,
        )?;
        let mut output = Self::open_format(&self.device, &self.format, buffer)
            .map_err(|error| format!("Cannot apply the playback buffer target: {error}"))?;
        output.target = target;
        Ok(output.watched("replacement"))
    }

    pub(crate) fn buffer_target_error(&self, target: PlaybackBufferTarget) -> Option<String> {
        requested_buffer(
            self.format.sample_rate().0,
            self.format.buffer_size(),
            target,
        )
        .err()
    }

    fn open_format(
        device: &cpal::Device,
        format: &SupportedStreamConfig,
        buffer_size: BufferSize,
    ) -> Result<Self, String> {
        Self::open_observed(
            device,
            format,
            buffer_size,
            |_, _| {},
            |error| eprintln!("Audio output failed: {error}"),
        )
    }

    /// Tests can add an observer. Production measurement uses atomics only;
    /// the callback does not allocate, lock, read files, or log measurements.
    fn open_observed<F, E>(
        device: &cpal::Device,
        format: &SupportedStreamConfig,
        buffer_size: BufferSize,
        mut observe: F,
        error: E,
    ) -> Result<Self, String>
    where
        F: FnMut(usize, &cpal::OutputCallbackInfo) + Send + 'static,
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        let mut config = format.config();
        config.buffer_size = buffer_size;
        let flight = flight::Flight::new(config.channels, config.sample_rate.0);
        let (mixer, input) = device_mixer(
            config.channels,
            config.sample_rate.0,
            Arc::clone(&flight.sources),
        );
        let writer = flight.writer();
        // A stream error ends the stream: the audio library's thread stops.
        let errors = Arc::clone(&flight);
        let mut error = error;
        let error = move |failure: cpal::StreamError| {
            errors.record_stream_error();
            flight::log("stream error", failure.to_string());
            error(failure);
        };
        let metrics = Arc::new(CallbackMetrics::default());
        let observed = Arc::clone(&metrics);
        let mut previous = None;
        let observe = move |frames, info: &cpal::OutputCallbackInfo| {
            let now = Instant::now();
            let period = previous.map(|before| {
                now.duration_since(before)
                    .as_nanos()
                    .min(u128::from(u64::MAX)) as u64
            });
            previous = Some(now);
            observed.record(frames, period);
            observe(frames, info);
        };
        macro_rules! stream {
            ($sample:ty) => {
                build::<$sample, _, _>(device, &config, mixer, writer, observe, error)
            };
        }
        let stream = match format.sample_format() {
            SampleFormat::F32 => stream!(f32),
            SampleFormat::F64 => stream!(f64),
            SampleFormat::I8 => stream!(i8),
            SampleFormat::I16 => stream!(i16),
            SampleFormat::I32 => stream!(i32),
            SampleFormat::I64 => stream!(i64),
            SampleFormat::U8 => stream!(u8),
            SampleFormat::U16 => stream!(u16),
            SampleFormat::U32 => stream!(u32),
            SampleFormat::U64 => stream!(u64),
            _ => return Err("The audio output sample format is not supported.".into()),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            _stream: stream,
            input,
            configuration: OutputConfiguration {
                channels: config.channels,
                sample_rate: config.sample_rate.0,
                requested_buffer_frames: match buffer_size {
                    BufferSize::Fixed(frames) => Some(frames),
                    BufferSize::Default => None,
                },
                negotiated_buffer_frames: None,
                callback_frames: None,
                callback_period_micros: None,
            },
            metrics,
            device: device.clone(),
            format: format.clone(),
            target: PlaybackBufferTarget::Automatic,
            fallback_reason: None,
            name: device
                .name()
                .unwrap_or_else(|_| "unnamed device".to_owned()),
            flight,
            watch: None,
        })
    }

    /// Play `source` at `volume` from the device's next buffer. Stop the
    /// playback it replaces first.
    pub(super) fn start(
        &mut self,
        source: PlaybackSource,
        volume: f32,
    ) -> Result<Playback, String> {
        self.input.start(source, volume)
    }

    pub(crate) fn configuration(&self) -> OutputConfiguration {
        self.metrics.configuration(self.configuration)
    }

    pub(crate) fn fallback_reason(&self) -> Option<&str> {
        self.fallback_reason.as_deref()
    }

    pub(crate) fn status(&self) -> String {
        status_text(self.configuration(), self.target, self.fallback_reason())
    }
}

fn status_text(
    configuration: OutputConfiguration,
    target: PlaybackBufferTarget,
    fallback_reason: Option<&str>,
) -> String {
    let target = target_name(target);
    let request = request_text(configuration);
    let callback = configuration.callback_frames.map_or_else(
        || "Callback frames unavailable".to_owned(),
        |frames| format!("Last callback: {frames} frames"),
    );
    let period = configuration.callback_period_micros.map_or_else(
        || "measured callback interval unavailable".to_owned(),
        |micros| {
            format!(
                "last measured callback interval: {:.2} ms",
                micros as f64 / 1000.0
            )
        },
    );
    let mut status = format!(
        "Target: {target}; request: {request} at {} Hz. \
         Negotiated buffer capacity unavailable. {callback}; {period}.",
        configuration.sample_rate
    );
    if let Some(reason) = fallback_reason {
        status.push(' ');
        status.push_str(reason);
    }
    status
}

fn build<T, F, E>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut mixer: DeviceMixer,
    mut writer: flight::Writer,
    mut observe: F,
    error: E,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
    f32: cpal::FromSample<T>,
    F: FnMut(usize, &cpal::OutputCallbackInfo) + Send + 'static,
    E: FnMut(cpal::StreamError) + Send + 'static,
{
    let channels = usize::from(config.channels);
    device.build_output_stream(
        config,
        move |data: &mut [T], info| {
            let started = Instant::now();
            let frames = data.len() / channels;
            observe(frames, info);
            mixer.fill(data, &mut writer);
            writer.record_fill(frames, started.elapsed());
        },
        error,
        None,
    )
}

/// Write the device buffer. A non-finite sample is written as silence: the
/// device and the system mixer can turn NaN or infinity into full-scale noise.
#[cfg(test)]
fn fill<T>(output: &mut [T], source: &mut impl Iterator<Item = f32>)
where
    T: cpal::Sample + cpal::FromSample<f32>,
    f32: cpal::FromSample<T>,
{
    fill_into(output, source, &mut ());
}

/// `fill`, handing `tap` each sample from the mixer and as written.
pub(super) fn fill_into<T>(
    output: &mut [T],
    source: &mut impl Iterator<Item = f32>,
    tap: &mut impl flight::Tap,
) where
    T: cpal::Sample + cpal::FromSample<f32>,
    f32: cpal::FromSample<T>,
{
    let mut non_finite = 0;
    for sample in output.iter_mut() {
        let value = source.next();
        *sample = match value {
            Some(value) if value.is_finite() => T::from_sample(value),
            Some(_) => {
                non_finite += 1;
                T::EQUILIBRIUM
            }
            None => T::EQUILIBRIUM,
        };
        tap.sample(value, <f32 as cpal::Sample>::from_sample(*sample));
    }
    tap.finish(non_finite);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    #[test]
    fn fill_writes_non_finite_samples_as_silence() {
        let mut source = [0.5, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.25].into_iter();
        let mut out = [1.0_f32; 6];
        fill(&mut out, &mut source);
        assert_eq!(out, [0.5, 0.0, 0.0, 0.0, -0.25, 0.0]);
        let mut source = [f32::NAN, 1.0].into_iter();
        let mut out = [7_i16; 2];
        fill(&mut out, &mut source);
        assert_eq!(out, [0, i16::MAX]);
    }

    #[test]
    fn requests_exact_targets_and_rejects_known_unsupported_ranges() {
        for (rate, frames20, frames40) in [
            (44_100, 882, 1764),
            (48_000, 960, 1920),
            (96_000, 1920, 3840),
        ] {
            assert_eq!(
                requested_buffer(
                    rate,
                    &SupportedBufferSize::Unknown,
                    PlaybackBufferTarget::Ms20
                ),
                Ok(BufferSize::Fixed(frames20))
            );
            assert_eq!(
                requested_buffer(
                    rate,
                    &SupportedBufferSize::Unknown,
                    PlaybackBufferTarget::Ms40
                ),
                Ok(BufferSize::Fixed(frames40))
            );
        }
        let limited = SupportedBufferSize::Range { min: 128, max: 512 };
        assert!(requested_buffer(48_000, &limited, PlaybackBufferTarget::Ms20).is_err());
        assert!(requested_buffer(48_000, &limited, PlaybackBufferTarget::Ms40).is_err());
        assert_eq!(
            requested_buffer(48_000, &limited, PlaybackBufferTarget::Automatic),
            Ok(BufferSize::Default)
        );
        let minimum = SupportedBufferSize::Range {
            min: 2048,
            max: 4096,
        };
        assert!(requested_buffer(48_000, &minimum, PlaybackBufferTarget::Ms40).is_err());
        let supported = SupportedBufferSize::Range {
            min: 960,
            max: 1920,
        };
        assert_eq!(
            requested_buffer(48_000, &supported, PlaybackBufferTarget::Ms20),
            Ok(BufferSize::Fixed(960))
        );
        assert_eq!(
            requested_buffer(48_000, &supported, PlaybackBufferTarget::Ms40),
            Ok(BufferSize::Fixed(1920))
        );
    }

    #[test]
    fn callback_observations_do_not_claim_negotiated_capacity() {
        let metrics = CallbackMetrics::default();
        let configuration = OutputConfiguration {
            channels: 2,
            sample_rate: 48_000,
            requested_buffer_frames: Some(1920),
            negotiated_buffer_frames: None,
            callback_frames: None,
            callback_period_micros: None,
        };
        assert_eq!(metrics.configuration(configuration), configuration);
        metrics.record(1056, None);
        let first = metrics.configuration(configuration);
        assert_eq!(first.callback_frames, Some(1056));
        assert_eq!(first.callback_period_micros, None);
        metrics.record(480, Some(10_100_000));
        let observed = metrics.configuration(configuration);
        assert_eq!(observed.requested_buffer_frames, Some(1920));
        assert_eq!(observed.negotiated_buffer_frames, None);
        assert_eq!(observed.callback_frames, Some(480));
        assert_eq!(observed.callback_period_micros, Some(10_100));
    }

    #[test]
    fn output_conversion_keeps_samples_and_fills_device_silence() {
        let mut floats = [1.0_f32; 5];
        fill(&mut floats, &mut [-0.5, 0.0, 0.5].into_iter());
        assert_eq!(floats, [-0.5, 0.0, 0.5, 0.0, 0.0]);
        let mut signed = [1_i16; 5];
        fill(&mut signed, &mut [-0.5, 0.0, 0.5].into_iter());
        assert_eq!(signed, [-16384, 0, 16384, 0, 0]);
        let mut unsigned = [1_u16; 5];
        fill(&mut unsigned, &mut [-0.5, 0.0, 0.5].into_iter());
        assert_eq!(unsigned, [16384, 32768, 49152, 32768, 32768]);
    }

    #[derive(Default)]
    struct Probe {
        count: AtomicUsize,
        first_frames: AtomicUsize,
        frames: AtomicU64,
        max_frames: AtomicUsize,
        min_frames: AtomicUsize,
        max_interval_ns: AtomicU64,
        interval_ns: AtomicU64,
        max_lead_ns: AtomicU64,
        min_lead_ns: AtomicU64,
        errors: AtomicUsize,
    }

    #[test]
    #[ignore = "opens the default physical audio device for nine seconds of silence"]
    fn native_output_buffer_probe() {
        let device = cpal::default_host()
            .default_output_device()
            .expect("output device");
        let format = device
            .default_output_config()
            .expect("default output format");
        println!("Device: {}", device.name().unwrap_or_default());
        for target in [
            PlaybackBufferTarget::Automatic,
            PlaybackBufferTarget::Ms20,
            PlaybackBufferTarget::Ms40,
        ] {
            let buffer =
                match requested_buffer(format.sample_rate().0, format.buffer_size(), target) {
                    Ok(buffer) => buffer,
                    Err(error) => {
                        println!("{target:?}: unsupported: {error}");
                        continue;
                    }
                };
            let probe = Arc::new(Probe {
                min_frames: AtomicUsize::new(usize::MAX),
                min_lead_ns: AtomicU64::new(u64::MAX),
                ..Probe::default()
            });
            let observed = Arc::clone(&probe);
            let failed = Arc::clone(&probe);
            let mut previous = None;
            let output = Output::open_observed(
                &device,
                &format,
                buffer,
                move |frames, info| {
                    let now = Instant::now();
                    if observed.count.fetch_add(1, Ordering::Relaxed) == 0 {
                        observed.first_frames.store(frames, Ordering::Relaxed);
                    }
                    observed.frames.fetch_add(frames as u64, Ordering::Relaxed);
                    observed.min_frames.fetch_min(frames, Ordering::Relaxed);
                    observed.max_frames.fetch_max(frames, Ordering::Relaxed);
                    if let Some(before) = previous {
                        let nanos = now.duration_since(before).as_nanos() as u64;
                        observed.interval_ns.fetch_add(nanos, Ordering::Relaxed);
                        observed.max_interval_ns.fetch_max(nanos, Ordering::Relaxed);
                    }
                    previous = Some(now);
                    let time = info.timestamp();
                    if let Some(lead) = time.playback.duration_since(&time.callback) {
                        let nanos = lead.as_nanos() as u64;
                        observed.min_lead_ns.fetch_min(nanos, Ordering::Relaxed);
                        observed.max_lead_ns.fetch_max(nanos, Ordering::Relaxed);
                    }
                },
                move |_| {
                    failed.errors.fetch_add(1, Ordering::Relaxed);
                },
            );
            let output = match output {
                Ok(output) => output,
                Err(error) if target != PlaybackBufferTarget::Automatic => {
                    println!("{target:?}: rejected: {error}");
                    continue;
                }
                Err(error) => panic!("Automatic output did not open: {error}"),
            };
            std::thread::sleep(Duration::from_secs(3));
            drop(output);
            let count = probe.count.load(Ordering::Relaxed);
            assert!(count > 1, "The device did not request audio.");
            assert_eq!(
                probe.errors.load(Ordering::Relaxed),
                0,
                "The device reported an error."
            );
            println!(
                "{target:?}: {buffer:?}, {} Hz: {count} callbacks; \
                 first {} frames; frames {}..{}, mean {:.1}; \
                 interval mean {:.3} ms, max {:.3} ms; timestamp lead {:.3}..{:.3} ms",
                format.sample_rate().0,
                probe.first_frames.load(Ordering::Relaxed),
                probe.min_frames.load(Ordering::Relaxed),
                probe.max_frames.load(Ordering::Relaxed),
                probe.frames.load(Ordering::Relaxed) as f64 / count as f64,
                probe.interval_ns.load(Ordering::Relaxed) as f64 / (count - 1) as f64 / 1e6,
                probe.max_interval_ns.load(Ordering::Relaxed) as f64 / 1e6,
                probe.min_lead_ns.load(Ordering::Relaxed) as f64 / 1e6,
                probe.max_lead_ns.load(Ordering::Relaxed) as f64 / 1e6,
            );
        }
    }
}
