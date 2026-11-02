//! Device output with a requested 40 ms buffer. The device can use another
//! callback interval; buffer capacity is not a callback deadline.
use cpal::{
    BufferSize, SampleFormat, SampleRate, StreamConfig, SupportedBufferSize, SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use rodio::{Sink, dynamic_mixer};
use std::sync::Arc;

const BUFFER_MILLIS: u32 = 40;

/// The configuration accepted when opening the output stream. A fixed
/// request does not establish the device's callback interval or measured latency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputConfiguration {
    pub channels: u16,
    pub sample_rate: u32,
    /// `None` means the fixed request failed and the system default was used.
    pub requested_buffer_frames: Option<u32>,
}

pub(super) struct Output {
    // Keep the stream alive while the mixer accepts sources.
    _stream: cpal::Stream,
    mixer: Arc<dynamic_mixer::DynamicMixerController<f32>>,
    configuration: OutputConfiguration,
}

/// Request 40 ms, held to an explicitly reported supported range. Unknown
/// ranges are tried as requested, then retried with the system default.
fn requested_buffer(rate: u32, supported: &SupportedBufferSize) -> BufferSize {
    let frames = ((u64::from(rate) * u64::from(BUFFER_MILLIS) + 500) / 1000)
        .clamp(1, u64::from(u32::MAX)) as u32;
    let frames = match *supported {
        SupportedBufferSize::Range { min, max } if min <= max && max > 0 => {
            frames.clamp(min.max(1), max)
        }
        _ => frames,
    };
    BufferSize::Fixed(frames)
}

impl Output {
    pub(super) fn open() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("There is no default audio output device.")?;
        match Self::open_device(&device) {
            Ok(output) => Ok(output),
            Err(original) => host
                .output_devices()
                .ok()
                .and_then(|mut devices| devices.find_map(|device| Self::open_device(&device).ok()))
                .ok_or(original),
        }
    }

    fn open_device(device: &cpal::Device) -> Result<Self, String> {
        let default = device.default_output_config().map_err(|e| e.to_string())?;
        let original = match Self::open_supported(device, default) {
            Ok(output) => return Ok(output),
            Err(error) => error,
        };
        let Ok(supported) = device.supported_output_configs() else {
            return Err(original);
        };
        let mut ranges: Vec<_> = supported.collect();
        ranges.sort_by(|a, b| b.cmp_default_heuristics(a));
        for range in ranges {
            let (min, max) = (range.min_sample_rate(), range.max_sample_rate());
            let mut rates = vec![max];
            if min < SampleRate(44_100) && SampleRate(44_100) < max {
                rates.push(SampleRate(44_100));
            }
            if min != max {
                rates.push(min);
            }
            for rate in rates {
                if let Ok(output) = Self::open_supported(device, range.with_sample_rate(rate)) {
                    return Ok(output);
                }
            }
        }
        Err(original)
    }

    fn open_supported(
        device: &cpal::Device,
        format: SupportedStreamConfig,
    ) -> Result<Self, String> {
        let requested = requested_buffer(format.sample_rate().0, format.buffer_size());
        Self::open_format(device, &format, requested).or_else(|_| {
            let output = Self::open_format(device, &format, BufferSize::Default)?;
            eprintln!("The audio device rejected the requested buffer. Using its default buffer.");
            Ok(output)
        })
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

    /// The observer is empty in production. Tests can measure callbacks
    /// without adding allocations or logging to the audio thread.
    fn open_observed<F, E>(
        device: &cpal::Device,
        format: &SupportedStreamConfig,
        buffer_size: BufferSize,
        observe: F,
        error: E,
    ) -> Result<Self, String>
    where
        F: FnMut(usize, &cpal::OutputCallbackInfo) + Send + 'static,
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        let mut config = format.config();
        config.buffer_size = buffer_size;
        let (mixer, source) = dynamic_mixer::mixer::<f32>(config.channels, config.sample_rate.0);
        macro_rules! stream {
            ($sample:ty) => {
                build::<$sample, _, _>(device, &config, source, observe, error)
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
            mixer,
            configuration: OutputConfiguration {
                channels: config.channels,
                sample_rate: config.sample_rate.0,
                requested_buffer_frames: match buffer_size {
                    BufferSize::Fixed(frames) => Some(frames),
                    BufferSize::Default => None,
                },
            },
        })
    }

    pub(super) fn sink(&self) -> Sink {
        let (sink, source) = Sink::new_idle();
        self.mixer.add(source);
        sink
    }

    pub(super) fn configuration(&self) -> OutputConfiguration {
        self.configuration
    }
}

fn build<T, F, E>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut source: dynamic_mixer::DynamicMixer<f32>,
    mut observe: F,
    error: E,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
    F: FnMut(usize, &cpal::OutputCallbackInfo) + Send + 'static,
    E: FnMut(cpal::StreamError) + Send + 'static,
{
    let channels = usize::from(config.channels);
    device.build_output_stream(
        config,
        move |data: &mut [T], info| {
            observe(data.len() / channels, info);
            fill(data, &mut source);
        },
        error,
        None,
    )
}

fn fill<T: cpal::Sample + cpal::FromSample<f32>>(
    output: &mut [T],
    source: &mut impl Iterator<Item = f32>,
) {
    for sample in output {
        *sample = source.next().map_or(T::EQUILIBRIUM, T::from_sample);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    #[test]
    fn requests_forty_milliseconds_within_known_device_limits() {
        for (rate, frames) in [(44_100, 1764), (48_000, 1920), (96_000, 3840)] {
            assert_eq!(
                requested_buffer(rate, &SupportedBufferSize::Unknown),
                BufferSize::Fixed(frames)
            );
        }
        let limited = SupportedBufferSize::Range { min: 128, max: 512 };
        assert_eq!(requested_buffer(48_000, &limited), BufferSize::Fixed(512));
        let minimum = SupportedBufferSize::Range {
            min: 2048,
            max: 4096,
        };
        assert_eq!(requested_buffer(48_000, &minimum), BufferSize::Fixed(2048));
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

    #[test]
    fn idle_sink_queue_plays_through_the_device_mixer() {
        // Rodio's empty queue starts with mono 48 kHz metadata. Keep the
        // fixture at that format so this checks the bridge, not rodio's
        // separate queue metadata transition and resampling behavior.
        let (controller, mut mixer) = dynamic_mixer::mixer::<f32>(1, 48_000);
        let (sink, queue) = Sink::new_idle();
        controller.add(queue);
        sink.append(rodio::buffer::SamplesBuffer::new(
            1,
            48_000,
            vec![0.25, -0.5, 0.75, -0.25],
        ));
        let mut output = [0.0_f32; 4];
        fill(&mut output, &mut mixer);
        assert_eq!(output, [0.25, -0.5, 0.75, -0.25]);
        sink.stop();
        let mut tail = [1.0_f32; 64];
        fill(&mut tail, &mut mixer);
        assert_eq!(tail, [0.0; 64]);
    }

    #[test]
    fn stereo_mixer_keeps_the_source_channels_in_order() {
        let (controller, mut mixer) = dynamic_mixer::mixer::<f32>(2, 44_100);
        controller.add(rodio::buffer::SamplesBuffer::new(
            2,
            44_100,
            vec![0.25, -0.5, 0.75, -0.25],
        ));
        let mut output = [1.0_f32; 8];
        fill(&mut output, &mut mixer);
        assert_eq!(output, [0.25, -0.5, 0.75, -0.25, 0.0, 0.0, 0.0, 0.0]);
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
    #[ignore = "opens the default physical audio device for six seconds of silence"]
    fn native_output_buffer_probe() {
        let device = cpal::default_host()
            .default_output_device()
            .expect("output device");
        let format = device
            .default_output_config()
            .expect("default output format");
        println!("Device: {}", device.name().unwrap_or_default());
        for buffer in [
            BufferSize::Default,
            requested_buffer(format.sample_rate().0, format.buffer_size()),
        ] {
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
            )
            .expect("output opens");
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
                "{buffer:?}, {} Hz: {count} callbacks; first {} frames; frames {}..{}, mean {:.1}; \
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
