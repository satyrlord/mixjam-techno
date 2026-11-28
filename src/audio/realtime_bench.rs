//! A real-time check of live playback. It drives `PlaybackSource` with a
//! live `MasterControl` through the device's mixer, which converts it to the
//! device's rate, in callbacks of a shared-mode period, without an audio
//! device. It reports the work done in each callback.
//!
//! Run it on a release build, at high priority on a quiet machine. Each
//! benchmark builds its own synthetic arrangement, so it needs no song file:
//!
//! ```text
//! cargo test --release --lib realtime_options_bench -- --ignored --nocapture
//! cargo test --release --lib channel_strip_multitrack_bench -- --ignored --nocapture
//! ```
use super::{
    Frames, Mix, Pcm,
    device_mixer::{DeviceMixer, MixerInput, Playback, device_mixer},
    prepare::mix_with_samples_quality,
    priority_failure,
    source::{PlaybackSource, SYSTEM_PRIORITY},
};
use crate::{
    engine::Project,
    master::MasterControl,
    playback_quality::{DriveOversampling, Oversampling, PlaybackQuality, SampleQuality},
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

/// The callback period that the deadline counts: 10 ms.
const PERIOD_MS: f64 = 10.0;

/// The cycles that the calling thread has run.
#[cfg(windows)]
fn thread_cycles() -> u64 {
    let mut cycles = 0;
    // SAFETY: the pseudo handle names the calling thread.
    unsafe {
        let _ = windows::Win32::System::WindowsProgramming::QueryThreadCycleTime(
            windows::Win32::System::Threading::GetCurrentThread(),
            &mut cycles,
        );
    }
    cycles
}
#[cfg(not(windows))]
fn thread_cycles() -> u64 {
    0
}

/// Cycles per millisecond, from a busy wait of 200 ms.
fn cycles_per_ms() -> f64 {
    let (start, cycles) = (Instant::now(), thread_cycles());
    while start.elapsed() < Duration::from_millis(200) {
        std::hint::spin_loop();
    }
    (thread_cycles() - cycles) as f64 / start.elapsed().as_secs_f64() / 1000.0
}

/// The wall and thread CPU time of each callback, in milliseconds.
#[derive(Default)]
struct Times {
    wall: Vec<f64>,
    cpu: Vec<f64>,
}

impl Times {
    fn report(&self, label: &str, audio_seconds: f64) {
        let describe = |name: &str, values: &[f64]| {
            let mut sorted = values.to_vec();
            sorted.sort_by(f64::total_cmp);
            let at = |share: f64| sorted[((sorted.len() - 1) as f64 * share).round() as usize];
            let over = values.iter().filter(|value| **value > PERIOD_MS).count();
            format!(
                "{name}: mean {:.2} ms, p99 {:.2}, p99.9 {:.2}, max {:.2}; over {PERIOD_MS} ms: \
                 {over} of {} ({:.2}%)",
                values.iter().sum::<f64>() / values.len() as f64,
                at(0.99),
                at(0.999),
                sorted[sorted.len() - 1],
                values.len(),
                100.0 * over as f64 / values.len() as f64
            )
        };
        println!(
            "{label}\n  {}\n  {}\n  real-time factor (wall) {:.3}",
            describe("wall", &self.wall),
            describe("cpu ", &self.cpu),
            self.wall.iter().sum::<f64>() / 1000.0 / audio_seconds
        );
    }
}

/// A device that fills `frames` frames a callback at `rate` from the
/// device's mixer, as the output callback does.
struct Device {
    mixer: DeviceMixer,
    input: MixerInput,
    buffer: Vec<f32>,
    cycles_per_ms: f64,
}

impl Device {
    fn new(rate: u32, frames: usize, cycles_per_ms: f64) -> Self {
        let (mixer, input) = device_mixer(2, rate, Arc::default());
        Self {
            mixer,
            input,
            buffer: vec![0.0; frames * 2],
            cycles_per_ms,
        }
    }

    /// Play `source` on this device in place of `old`, as `Player::play`
    /// does.
    fn play(&mut self, source: PlaybackSource, old: Option<Playback>) -> Playback {
        if let Some(old) = old {
            old.end();
        }
        self.input.start(source, 1.0).unwrap()
    }

    fn callback(&mut self, times: &mut Times) {
        let (start, cycles) = (Instant::now(), thread_cycles());
        self.mixer.fill(&mut self.buffer, &mut ());
        times
            .cpu
            .push((thread_cycles() - cycles) as f64 / self.cycles_per_ms);
        times.wall.push(start.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(&self.buffer);
    }
}

fn control(project: &Project) -> Arc<MasterControl> {
    let control = MasterControl::new(&project.master);
    control.set_returns(&project.fx);
    control.set_levels(project);
    control
}

/// A fixed synthetic arrangement for repeatable options comparisons. Every
/// lane has sounded by the eighth second; each lane repeats for 32 bars.
fn options_fixture(
    tracks: usize,
    drives: usize,
    master_heavy: bool,
) -> (Project, HashMap<u32, Pcm>) {
    let mut editor = crate::engine::Editor::new(Project::default());
    for track in 0..tracks {
        for bar in track % 4..32 {
            editor.place(1, track, (bar * 256) as u32, 256).unwrap();
        }
    }
    let mut project = editor.project().clone();
    for (index, track) in project.tracks[..tracks].iter_mut().enumerate() {
        track.gain = 0.7 / tracks as f32;
        track.drive.on = index < drives;
        track.drive.amount = 24.0;
        track.drive.mix = 100.0;
        track.drive.focus = 20.0;
        track.eq.on = index % 3 == 0;
        track.compressor.on = index % 4 == 0;
    }
    project.master.limiter.on = true;
    if master_heavy {
        for pedal in [
            crate::master::PedalKind::SoftClip,
            crate::master::PedalKind::Tube,
            crate::master::PedalKind::Maximizer,
            crate::master::PedalKind::Tape,
        ] {
            project.master.set_on(pedal, true);
        }
        project.master.tube.drive = 10.0;
        project.master.maximizer.boost = 25.0;
        project.master.tape.drive = 10.0;
    }
    let source = Pcm {
        sample_rate: Mix::SAMPLE_RATE,
        channels: 2,
        samples: (0..Mix::SAMPLE_RATE * 4)
            .flat_map(|frame| {
                let seconds = f64::from(frame) / f64::from(Mix::SAMPLE_RATE);
                let sample = (0.5 * (std::f64::consts::TAU * 55.0 * seconds).sin()
                    + 0.25 * (std::f64::consts::TAU * 5000.0 * seconds).sin())
                    as f32;
                [sample, -0.8 * sample]
            })
            .collect(),
    };
    (project, HashMap::from([(1, source)]))
}

fn options_source(
    mix: Arc<dyn Frames>,
    project: &Project,
    quality: PlaybackQuality,
    first: u64,
) -> PlaybackSource {
    PlaybackSource::with_quality_block(
        mix,
        first,
        None,
        Some(control(project)),
        PlaybackSource::BLOCK_FRAMES,
        quality,
    )
}

/// Modeled device callbacks, without audible device output. Run in release
/// on a quiet machine. The 10 ms deadline is unchanged by buffer preferences.
#[test]
#[ignore = "release benchmark; use a quiet machine"]
fn realtime_options_bench() {
    let cycles = cycles_per_ms();
    SYSTEM_PRIORITY.with(|system| system.set(true));
    for (name, tracks, drives, master_heavy) in [
        ("sparse", 8, 2, false),
        ("dense-Drive", 22, 22, false),
        ("master-heavy", 12, 0, true),
        ("64-track-stress", 64, 64, true),
    ] {
        let (project, samples) = options_fixture(tracks, drives, master_heavy);
        for drive in [DriveOversampling::TwoX, DriveOversampling::Off] {
            for master in [Oversampling::FourX, Oversampling::Off] {
                let quality = PlaybackQuality {
                    drive,
                    master,
                    samples: SampleQuality::High,
                };
                let mix: Arc<dyn Frames> =
                    Arc::new(mix_with_samples_quality(&project, &samples, quality).unwrap());
                for (rate, frames) in [(48_000, 480), (44_100, 441)] {
                    let label = format!(
                        "{name}, tracks={tracks}, Drives={drives}, {quality:?}, {rate} Hz, {frames} frames"
                    );
                    let mut device = Device::new(rate, frames, cycles);
                    let mut current =
                        Some(device.play(options_source(mix.clone(), &project, quality, 0), None));
                    let mut times = Times::default();
                    for _ in 0..100 {
                        device.callback(&mut times);
                    }
                    times.report(&format!("{label}, start: 1 s"), 1.0);
                    current = Some(device.play(
                        options_source(
                            mix.clone(),
                            &project,
                            quality,
                            12 * u64::from(Mix::SAMPLE_RATE),
                        ),
                        current.take(),
                    ));
                    let mut times = Times::default();
                    for _ in 0..100 {
                        device.callback(&mut times);
                    }
                    times.report(&format!("{label}, seek to 12 s: 1 s"), 1.0);
                    // A new prepared object, with separate histories, replaces
                    // the previous source at its current musical position.
                    let replacement: Arc<dyn Frames> =
                        Arc::new(mix_with_samples_quality(&project, &samples, quality).unwrap());
                    current = Some(device.play(
                        options_source(
                            replacement,
                            &project,
                            quality,
                            13 * u64::from(Mix::SAMPLE_RATE),
                        ),
                        current.take(),
                    ));
                    let mut times = Times::default();
                    for _ in 0..100 {
                        device.callback(&mut times);
                    }
                    times.report(&format!("{label}, replacement at 13 s: 1 s"), 1.0);
                    drop(current);
                    let mut continuous = Device::new(rate, frames, cycles);
                    let _current =
                        continuous.play(options_source(mix.clone(), &project, quality, 0), None);
                    let mut warmup = Times::default();
                    for _ in 0..800 {
                        continuous.callback(&mut warmup);
                    }
                    let mut times = Times::default();
                    for _ in 0..100 {
                        continuous.callback(&mut times);
                    }
                    times.report(
                        &format!("{label}, uninterrupted from start after 8 s: 1 s"),
                        1.0,
                    );
                }
            }
        }
    }
    if let Some(failure) = priority_failure() {
        println!("{failure}");
    }
}

fn full_strip_bench_project(tracks: usize, with_drive: bool) -> (Project, HashMap<u32, Pcm>) {
    let mut editor = crate::engine::Editor::new(Project::default());
    for track in 0..tracks {
        for bar in 0..8 {
            editor.place(1, track, (bar * 256) as u32, 256).unwrap();
        }
    }
    let mut project = editor.project().clone();
    for (index, track) in project.tracks[..tracks].iter_mut().enumerate() {
        track.gain = 0.7 / tracks as f32;
        track.eq = crate::eq::ChannelEq {
            on: true,
            low_cut: 40.0,
            low_db: 2.0,
            mid_db: -2.0,
            peak2_db: 1.5,
            high_db: 1.0,
            high_cut: 16_000.0,
            cut_slope: crate::eq::CutSlope::Slope24,
            ..Default::default()
        };
        track.drive = crate::drive::Drive {
            on: with_drive,
            amount: 18.0,
            character: 40.0,
            mix: 80.0,
            focus: 90.0,
            output: 0.0,
        };
        track.compressor = crate::compressor::Compressor {
            on: true,
            threshold: -24.0,
            ratio: 4.0,
            attack: 2.0,
            release: 80.0,
            makeup: 0.0,
            sc_hpf_hz: 120.0,
        };
        track.width = 110.0;
        track.mono_below_hz = 120.0;
        track.filter_resonance = 25.0;
        track.filter_slope = crate::automation::FilterSlope::Slope24;
        track.automation.filter = vec![
            crate::automation::Point {
                tick: 0,
                value: 0.6,
            },
            crate::automation::Point {
                tick: 2048,
                value: -0.2,
            },
        ];
        if index > 0 {
            track.ducker = crate::ducker::Ducker {
                on: true,
                key: 1,
                threshold: -28.0,
                depth: 8.0,
                attack: 0.5,
                release: 80.0,
            };
        }
    }
    let source = Pcm {
        sample_rate: Mix::SAMPLE_RATE,
        channels: 2,
        samples: (0..Mix::SAMPLE_RATE * 4)
            .flat_map(|frame| {
                let s = f64::from(frame) / f64::from(Mix::SAMPLE_RATE);
                let x = (0.45 * (std::f64::consts::TAU * 65.0 * s).sin()
                    + 0.20 * (std::f64::consts::TAU * 1800.0 * s).sin())
                    as f32;
                [x, -0.7 * x]
            })
            .collect(),
    };
    (project, HashMap::from([(1, source)]))
}

/// Multi-track Channel Strip CPU benchmark (`--release`):
/// 16 full strips with 2x Drive (`<= 2.0 ms` p99) and 32 strips without Drive (`<= 1.5 ms` p99).
/// The p99 takes the lowest CPU time of each song position over `BENCH_PASSES` passes.
#[test]
#[ignore = "release benchmark; run in --release"]
fn channel_strip_multitrack_bench() {
    use crate::audio::RenderState;
    let cycles = cycles_per_ms();
    SYSTEM_PRIORITY.with(|system| system.set(true));
    let mut results = Vec::new();
    for (label, tracks, with_drive, limit_p99_ms) in [
        ("16 strips + 2x Drive", 16, true, 2.0),
        ("32 strips (no Drive)", 32, false, 1.5),
    ] {
        let (project, samples) = full_strip_bench_project(tracks, with_drive);
        let quality = PlaybackQuality::FULL;
        let mix = mix_with_samples_quality(&project, &samples, quality).unwrap();
        let mut state = RenderState::default();
        mix.prepare_state(&mut state);
        let drives_ptr = state.drives.as_ptr();
        let mut out = vec![0.0_f32; 256 * 2];
        let mut sends = vec![0.0_f32; 256 * 2 * crate::fx::BUSES];
        // Warmup
        for i in 0..100_u64 {
            mix.read_block(i * 256, &mut out, Some(&mut sends), &mut state);
        }
        // Each position of the song plays once per pass. The cost of a
        // position comes back on each pass, but a burst of background work
        // on the machine (about 25 ms, which slows each block by up to half)
        // lands on one pass only. Thus the check takes the lowest cost of
        // each position over the passes.
        let blocks = ((4 * 44_100) / 256) as u64;
        let reads = (BENCH_PASSES * blocks) as usize;
        let mut wall = Vec::with_capacity(reads);
        let mut cpu = Vec::with_capacity(reads);
        let mut steady = vec![f64::INFINITY; blocks as usize];
        for i in 0..BENCH_PASSES * blocks {
            let position = i % blocks;
            let (t0, c0) = (Instant::now(), thread_cycles());
            mix.read_block(position * 256, &mut out, Some(&mut sends), &mut state);
            let ms = (thread_cycles() - c0) as f64 / cycles;
            wall.push(t0.elapsed().as_secs_f64() * 1000.0);
            cpu.push(ms);
            let lowest = &mut steady[position as usize];
            *lowest = lowest.min(ms);
            std::hint::black_box(&out);
        }
        assert_eq!(state.drives.as_ptr(), drives_ptr, "drives reallocated");
        let p99_steady = percentile(&mut steady, 0.99);
        println!(
            "{label}: wall p50={:.3} ms p99={:.3} ms max={:.3} ms | cpu p50={:.3} ms p99={:.3} ms | steady p99={p99_steady:.3} ms",
            percentile(&mut wall, 0.5),
            percentile(&mut wall, 0.99),
            percentile(&mut wall, 1.0),
            percentile(&mut cpu, 0.5),
            percentile(&mut cpu, 0.99),
        );
        results.push((label, limit_p99_ms, p99_steady));
    }
    for (label, limit_p99_ms, p99_steady) in &results {
        assert!(
            p99_steady <= limit_p99_ms,
            "{label} steady p99 CPU {p99_steady:.3} ms > {limit_p99_ms:.2} ms"
        );
    }
}

/// The passes of `channel_strip_multitrack_bench` over the 4 s song.
const BENCH_PASSES: u64 = 3;

/// The value at fraction `q` of `values`, sorted in place.
fn percentile(values: &mut [f64], q: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * q).round() as usize]
}
