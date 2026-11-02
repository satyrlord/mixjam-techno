//! A real-time check of live playback. It drives `PlaybackSource` with a
//! live `MasterControl` through rodio's own sink and mixer, which convert it
//! to the device's rate, in callbacks of a WASAPI shared-mode period, without
//! an audio device. It reports the work done in each callback.
//!
//! Run it on a release build, at high priority on a quiet machine:
//!
//! ```text
//! MIXJAM_BENCH_SONG=path/to/song.mixjam \
//!     cargo test --release --lib realtime_bench -- --ignored --nocapture
//! ```
//!
//! `MIXJAM_BENCH_START` and `MIXJAM_BENCH_SECONDS` choose the stretch of the
//! song (120 and 60 seconds by default), and `MIXJAM_BENCH_REPLACES` the
//! number of replaced mixes (5 by default). Both measured stretches must
//! fit within the song; each replacement measures 2.2 seconds.
//! `MIXJAM_BENCH_DRIVE=off` and `MIXJAM_BENCH_MASTER=off` select reduced
//! playback independently. They never change export construction.
//! `MIXJAM_BENCH_CONTINUOUS=1` plays from the song's start through the
//! unmeasured stretch before `MIXJAM_BENCH_START`, then measures the selected
//! stretch at both rates. Replacement measurements stay in the default mode.
use super::*;
use crate::playback_quality::{Oversampling, PlaybackQuality};
use rodio::{Sink, dynamic_mixer};
use std::time::Instant;

/// The callback period that the deadline counts: 10 ms.
const PERIOD_MS: f64 = 10.0;

fn setting(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Validated settings keep a benchmark from timing silence after the song
/// ends, or reporting an empty set of callbacks.
struct Workload {
    first: u64,
    seconds: f64,
    replaces: usize,
}

impl Workload {
    fn new(start: f64, seconds: f64, replaces: f64, frames: u64) -> Result<Self, String> {
        if !start.is_finite()
            || start < 0.0
            || !seconds.is_finite()
            || seconds < PERIOD_MS / 1000.0
            || !replaces.is_finite()
            || replaces < 1.0
            || replaces.fract() != 0.0
        {
            return Err(
                "Use a nonnegative start, at least 0.01 seconds, and a positive whole \
                 replacement count."
                    .into(),
            );
        }
        let end = start + seconds.max(replaces * 2.2);
        if end * f64::from(Mix::SAMPLE_RATE) > frames as f64 {
            return Err("Both benchmark stretches must fit within the song.".into());
        }
        Ok(Self {
            first: (start * f64::from(Mix::SAMPLE_RATE)) as u64,
            seconds,
            replaces: replaces as usize,
        })
    }
}

#[test]
fn benchmark_rejects_empty_or_out_of_song_workloads() {
    let frames = 10 * u64::from(Mix::SAMPLE_RATE);
    for (start, seconds, replaces) in [
        (120.0, 60.0, 5.0),
        (0.0, 11.0, 1.0),
        (0.0, 1.0, 5.0),
        (0.0, 0.0, 1.0),
        (0.0, 1.0, 0.0),
        (-1.0, 1.0, 1.0),
        (0.0, 1.0, 1.5),
        (f64::NAN, 1.0, 1.0),
        (0.0, f64::INFINITY, 1.0),
    ] {
        assert!(Workload::new(start, seconds, replaces, frames).is_err());
    }
    let workload = Workload::new(1.0, 9.0, 4.0, frames).unwrap();
    assert_eq!(workload.first, u64::from(Mix::SAMPLE_RATE));
    assert_eq!(workload.seconds, 9.0);
    assert_eq!(workload.replaces, 4);
}

/// The cycles that the calling thread has run, and their rate per second.
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

/// A device that pulls `frames` frames a callback at `rate` from rodio's
/// mixer, as cpal's WASAPI callback does.
struct Device {
    controller: Arc<dynamic_mixer::DynamicMixerController<f32>>,
    mixer: dynamic_mixer::DynamicMixer<f32>,
    buffer: Vec<f32>,
    cycles_per_ms: f64,
}

impl Device {
    fn new(rate: u32, frames: usize, cycles_per_ms: f64) -> Self {
        let (controller, mixer) = dynamic_mixer::mixer::<f32>(2, rate);
        Self {
            controller,
            mixer,
            buffer: vec![0.0; frames * 2],
            cycles_per_ms,
        }
    }

    /// Play `source` on this device in place of `old`, as `Player::play`
    /// does.
    fn play(&self, source: PlaybackSource, old: Option<Current>) -> Current {
        let (sink, queue) = Sink::new_idle();
        self.controller.add(queue);
        Current::start(sink, source, old)
    }

    fn callback(&mut self, times: &mut Times) {
        let (start, cycles) = (Instant::now(), thread_cycles());
        for sample in &mut self.buffer {
            *sample = self.mixer.next().unwrap_or(0.0);
        }
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
        for drive in [Oversampling::FourX, Oversampling::Off] {
            for master in [Oversampling::FourX, Oversampling::Off] {
                let quality = PlaybackQuality { drive, master };
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

#[test]
#[ignore = "needs a heavy song in MIXJAM_BENCH_SONG and a quiet machine"]
fn realtime_bench() {
    let song = std::env::var_os("MIXJAM_BENCH_SONG").expect("MIXJAM_BENCH_SONG names a song");
    let started = Instant::now();
    let (project, library) =
        crate::project_io::open(Path::new(&song), &Library::empty()).expect("the song opens");
    let oversampling = |name| match std::env::var(name).as_deref() {
        Ok("off") => Oversampling::Off,
        Ok("4x") | Err(_) => Oversampling::FourX,
        _ => panic!("Use off or 4x for {name}"),
    };
    let quality = PlaybackQuality {
        drive: oversampling("MIXJAM_BENCH_DRIVE"),
        master: oversampling("MIXJAM_BENCH_MASTER"),
    };
    let continuous = match std::env::var("MIXJAM_BENCH_CONTINUOUS").as_deref() {
        Ok("1") => true,
        Ok("0") | Err(_) => false,
        _ => panic!("Use 0 or 1 for MIXJAM_BENCH_CONTINUOUS"),
    };
    println!("playback quality: {quality:?}");
    println!("opened in {:.2} s", started.elapsed().as_secs_f64());
    let started = Instant::now();
    let mix: Arc<dyn Frames> =
        Arc::new(prepare_project_with_quality(&project, &library, quality).expect("prepared"));
    println!(
        "prepared in {:.2} s: {:.1} s of song, block {} frames",
        started.elapsed().as_secs_f64(),
        mix.frames() as f64 / f64::from(Mix::SAMPLE_RATE),
        PlaybackSource::BLOCK_FRAMES
    );
    let Workload {
        first,
        seconds,
        replaces,
    } = Workload::new(
        setting("MIXJAM_BENCH_START", 120.0),
        setting("MIXJAM_BENCH_SECONDS", 60.0),
        setting("MIXJAM_BENCH_REPLACES", 5.0),
        mix.frames(),
    )
    .expect("valid benchmark settings");
    let cycles_per_ms = cycles_per_ms();
    println!("thread cycles: {:.0} per ms", cycles_per_ms);
    // The device thread asks for an audio thread's priority, as playback
    // does on a real device, unless MIXJAM_BENCH_PRIORITY is 0.
    let priority = setting("MIXJAM_BENCH_PRIORITY", 1.0) != 0.0;
    SYSTEM_PRIORITY.with(|system| system.set(priority));
    println!("audio thread priority requested: {priority}");

    for (rate, frames) in [(48_000, 480), (44_100, 441)] {
        let mut device = Device::new(rate, frames, cycles_per_ms);
        let control = control(&project);
        let _playing = device.play(
            PlaybackSource::with_quality_block(
                Arc::clone(&mix),
                if continuous { 0 } else { first },
                None,
                Some(control),
                PlaybackSource::BLOCK_FRAMES,
                quality,
            ),
            None,
        );
        let warmup_callbacks = if continuous {
            (first as f64 / f64::from(Mix::SAMPLE_RATE) * f64::from(rate) / frames as f64) as usize
        } else {
            0
        };
        if warmup_callbacks > 0 {
            let warmup_started = Instant::now();
            let mut discarded = Times::default();
            for _ in 0..warmup_callbacks {
                device.callback(&mut discarded);
            }
            println!(
                "{rate} Hz uninterrupted warmup: {warmup_callbacks} callbacks, \
                 {:.2} s audio in {:.2} s wall",
                warmup_callbacks as f64 * frames as f64 / f64::from(rate),
                warmup_started.elapsed().as_secs_f64()
            );
        }
        let mut times = Times::default();
        let callbacks = (seconds * f64::from(rate) / frames as f64) as usize;
        for _ in 0..callbacks {
            device.callback(&mut times);
        }
        let label = if continuous {
            format!(
                "{rate} Hz, {frames}-frame callbacks, uninterrupted after {:.2} s \
                 warmup from song start",
                warmup_callbacks as f64 * frames as f64 / f64::from(rate)
            )
        } else {
            format!(
                "{rate} Hz, {frames}-frame callbacks, from {} s",
                first / 44_100
            )
        };
        times.report(&label, seconds);
    }
    if continuous {
        if let Some(failure) = priority_failure() {
            println!("{failure}");
        }
        return;
    }

    // A rebuilt mix replaces the playing one, whose sources nothing else
    // holds, as when an edit evicts them from the decoded cache.
    drop(mix);
    let mut device = Device::new(48_000, 480, cycles_per_ms);
    let control = control(&project);
    let mut playing = Some(Arc::new(
        prepare_project_with_quality(&project, &library, quality).unwrap(),
    ) as Arc<dyn Frames>);
    let mut position = first;
    let mut current = Some(device.play(
        PlaybackSource::with_quality_block(
            playing.clone().unwrap(),
            position,
            None,
            Some(Arc::clone(&control)),
            PlaybackSource::BLOCK_FRAMES,
            quality,
        ),
        None,
    ));
    let mut before = Times::default();
    let mut after = Times::default();
    for _ in 0..replaces {
        for _ in 0..200 {
            device.callback(&mut before);
        }
        position += 200 * 441;
        let next: Arc<dyn Frames> =
            Arc::new(prepare_project_with_quality(&project, &library, quality).unwrap());
        current = Some(device.play(
            PlaybackSource::with_quality_block(
                Arc::clone(&next),
                position,
                None,
                Some(Arc::clone(&control)),
                PlaybackSource::BLOCK_FRAMES,
                quality,
            ),
            current.take(),
        ));
        playing = Some(next);
        let mut replaced = Times::default();
        for _ in 0..20 {
            device.callback(&mut replaced);
        }
        position += 20 * 441;
        let worst = replaced.wall.iter().copied().fold(0.0, f64::max);
        println!("replace: worst of the next 20 callbacks {worst:.2} ms wall");
        after.wall.extend(replaced.wall);
        after.cpu.extend(replaced.cpu);
    }
    drop((playing, current));
    if let Some(failure) = priority_failure() {
        println!("{failure}");
    }
    before.report(
        "48000 Hz while a mix plays, before its replacement",
        replaces as f64 * 2.0,
    );
    after.report(
        "48000 Hz, 20 callbacks after each replacement",
        replaces as f64 * 0.2,
    );
}
