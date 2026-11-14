use super::*;
use crate::{
    audio::{
        Frames, Mix, Pcm,
        flight::{self, tests::heap_use},
        prepare::mix_with_samples_quality,
    },
    engine::Project,
    master::MasterControl,
    playback_quality::PlaybackQuality,
};
use std::collections::HashMap;

fn pcm(channels: u16, samples: Vec<f32>) -> Arc<dyn Frames> {
    Arc::new(Pcm {
        sample_rate: Mix::SAMPLE_RATE,
        channels,
        samples,
    })
}

fn source(audio: Arc<dyn Frames>) -> PlaybackSource {
    PlaybackSource::new(audio, 0, None, None)
}

fn mixer(channels: u16, rate: u32) -> (DeviceMixer, MixerInput) {
    device_mixer(channels, rate, Arc::default())
}

#[test]
fn a_playback_plays_through_the_mixer_and_silence_follows_it() {
    let (mut mixer, mut input) = mixer(2, Mix::SAMPLE_RATE);
    let mut idle = [1.0_f32; 4];
    mixer.fill(&mut idle, &mut ());
    assert_eq!(idle, [0.0; 4]);
    let playback = input
        .start(source(pcm(2, vec![0.25, -0.5, 0.75, -0.25])), 1.0)
        .unwrap();
    assert!(playback.is_playing());
    let mut out = [1.0_f32; 8];
    mixer.fill(&mut out, &mut ());
    assert_eq!(out, [0.25, -0.5, 0.75, -0.25, 0.0, 0.0, 0.0, 0.0]);
    assert!(!playback.is_playing());
    assert_eq!(playback.played_frames(), 2);
    let mut tail = [1_i16; 64];
    mixer.fill(&mut tail, &mut ());
    assert_eq!(tail, [0; 64]);
}

#[test]
fn sources_take_the_devices_channels() {
    // Mono plays on both channels of a stereo device.
    let (mut mono_source, mut input) = mixer(2, Mix::SAMPLE_RATE);
    let _playback = input.start(source(pcm(1, vec![0.5, -0.25])), 1.0);
    let mut out = [0.0_f32; 4];
    mono_source.fill(&mut out, &mut ());
    assert_eq!(out, [0.5, 0.5, -0.25, -0.25]);
    // A mono device plays the left channel.
    let (mut mono_device, mut input) = mixer(1, Mix::SAMPLE_RATE);
    let _playback = input.start(source(pcm(2, vec![0.5, -0.25, 0.75, -0.5])), 1.0);
    let mut out = [0.0_f32; 2];
    mono_device.fill(&mut out, &mut ());
    assert_eq!(out, [0.5, 0.75]);
    // Channels past the second stay silent.
    let (mut surround, mut input) = mixer(4, Mix::SAMPLE_RATE);
    let _playback = input.start(source(pcm(2, vec![0.5, -0.25])), 1.0);
    let mut out = [1.0_f32; 4];
    surround.fill(&mut out, &mut ());
    assert_eq!(out, [0.5, -0.25, 0.0, 0.0]);
}

#[test]
fn volume_and_pause_take_effect_at_the_next_buffer() {
    let (mut mixer, mut input) = mixer(2, Mix::SAMPLE_RATE);
    let playback = input.start(source(pcm(2, vec![0.5; 2000])), 0.5).unwrap();
    let mut out = [0.0_f32; 20];
    mixer.fill(&mut out, &mut ());
    assert_eq!(out, [0.25; 20]);
    playback.set_volume(1.0);
    mixer.fill(&mut out, &mut ());
    assert_eq!(out, [0.5; 20]);
    playback.pause();
    assert!(!playback.is_playing());
    mixer.fill(&mut out, &mut ());
    assert_eq!(out, [0.0; 20]);
    assert_eq!(playback.played_frames(), 20);
    // A stopped paused playback ends at once.
    playback.end();
    mixer.fill(&mut out, &mut ());
    assert!(mixer.voices.iter().all(Option::is_none));
}

#[test]
fn another_rate_reads_between_source_frames() {
    // Four source frames at 44.1 kHz, rising by 0.1 a frame, read at 88.2 kHz.
    let (mut mixer, mut input) = mixer(1, 2 * Mix::SAMPLE_RATE);
    let _playback = input.start(source(pcm(1, vec![0.0, 0.1, 0.2, 0.3])), 1.0);
    let mut out = [1.0_f32; 10];
    mixer.fill(&mut out, &mut ());
    let expected = [0.0, 0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.0, 0.0, 0.0];
    for (got, want) in out.iter().zip(expected) {
        assert!((got - want).abs() < 1e-6, "{out:?}");
    }
}

#[test]
fn the_mixer_holds_no_more_voices_than_it_has_places() {
    let (mut mixer, mut input) = mixer(2, Mix::SAMPLE_RATE);
    // Paused playback never ends, so each keeps its place.
    let playing: Vec<_> = (0..VOICES + 2)
        .map(|_| {
            let playback = input.start(source(pcm(2, vec![0.0; 2000])), 1.0).unwrap();
            playback.pause();
            playback
        })
        .collect();
    let mut out = [0.0_f32; 2];
    mixer.fill(&mut out, &mut ());
    assert_eq!(mixer.held.load(Ordering::Relaxed), VOICES);
    // Ending them frees the places for the two that waited.
    drop(playing);
    mixer.fill(&mut out, &mut ());
    mixer.fill(&mut out, &mut ());
    assert_eq!(mixer.held.load(Ordering::Relaxed), 0);
}

/// A song with drives, EQs, compressors, a send to a delay bus, and master
/// pedals: the work of a real playback.
fn song() -> (Project, Arc<dyn Frames>) {
    let mut editor = crate::engine::Editor::new(Project::default());
    for track in 0..6 {
        for bar in 0..8 {
            editor.place(1, track, bar * 256, 256).unwrap();
        }
    }
    let mut project = editor.project().clone();
    for (index, track) in project.tracks[..6].iter_mut().enumerate() {
        track.gain = 0.15;
        track.drive.on = index < 2;
        track.drive.amount = 24.0;
        track.eq.on = index % 3 == 0;
        track.compressor.on = index % 2 == 0;
        track.sends[0] = 0.5;
    }
    project.fx.buses[0].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform::default());
    project.master.limiter.on = true;
    project.master.set_on(crate::master::PedalKind::Tube, true);
    let sound = Pcm {
        sample_rate: Mix::SAMPLE_RATE,
        channels: 2,
        samples: (0..Mix::SAMPLE_RATE * 2)
            .flat_map(|frame| {
                let x = (frame as f32 * 0.031).sin() * 0.4 + (frame as f32 * 0.7).sin() * 0.1;
                [x, -x]
            })
            .collect(),
    };
    let mix = mix_with_samples_quality(
        &project,
        &HashMap::from([(1, sound)]),
        PlaybackQuality::FULL,
    )
    .unwrap();
    (project, Arc::new(mix))
}

/// The device's callback as the output runs it: the mixer into a float or
/// a 16-bit buffer, through the flight recorder.
struct Device {
    mixer: DeviceMixer,
    writer: flight::Writer,
    floats: Vec<f32>,
    shorts: Vec<i16>,
    callbacks: usize,
}

impl Device {
    fn callbacks(&mut self, count: usize) {
        for _ in 0..count {
            if self.callbacks % 3 == 2 {
                self.mixer.fill(&mut self.shorts, &mut self.writer);
            } else {
                self.mixer.fill(&mut self.floats, &mut self.writer);
            }
            self.writer.record_fill(480, Duration::from_micros(100));
            self.callbacks += 1;
        }
    }

    /// `callbacks`, asserting that they neither allocate nor free.
    fn quiet(&mut self, what: &str, count: usize) {
        let used = heap_use(|| self.callbacks(count));
        assert_eq!(
            (used.allocations, used.frees),
            (0, 0),
            "{what}: allocations and frees in {count} callbacks"
        );
    }
}

#[test]
fn the_callback_neither_allocates_nor_frees_after_warm_up() {
    let (mut project, mix) = song();
    let control = MasterControl::new(&project.master);
    control.set_returns(&project.fx);
    control.set_levels(&project);
    let play = |from: u64, region: Option<[u64; 2]>| {
        PlaybackSource::new(Arc::clone(&mix), from, region, Some(Arc::clone(&control)))
    };
    let flight = flight::Flight::new(2, 48_000);
    let (mixer, mut input) = device_mixer(2, 48_000, Arc::clone(&flight.sources));
    let mut device = Device {
        mixer,
        writer: flight.writer(),
        floats: vec![0.0; 2 * 480],
        shorts: vec![0; 2 * 441],
        callbacks: 0,
    };
    let first = input.start(play(0, None), 1.0).unwrap();
    device.callbacks(50);
    device.quiet("steady playback", 200);

    // Live edits: the board, the buses, the levels, and the volume.
    let mut chain = project.master;
    chain.trim = -3.0;
    chain.set_on(crate::master::PedalKind::Tape, true);
    control.set(&chain);
    let mut buses = project.fx;
    buses.buses[0].return_level = 0.3;
    control.set_returns(&buses);
    control.clear_tail(0);
    project.tracks[0].gain = 0.05;
    control.set_levels(&project);
    first.set_volume(0.5);
    device.quiet("live edits", 100);

    // A replacement: the old voice plays out its block and is retired.
    first.end();
    let second = input.start(play(3 * 44_100, None), 1.0).unwrap();
    device.quiet("a replacement", 100);
    assert_eq!(flight.mixer_sources(), 1);

    // A loop of a fifth of a second, many times round.
    second.end();
    let looping = input
        .start(play(44_100, Some([44_100, 44_100 + 8_820])), 1.0)
        .unwrap();
    device.quiet("a loop", 200);
    looping.pause();
    device.quiet("a pause", 20);

    // The end of the song, its tails, and the silence after it.
    looping.end();
    let ending = input.start(play(mix.frames() - 4_410, None), 1.0).unwrap();
    let mut callbacks = 0;
    while ending.is_playing() {
        device.quiet("the end of the song", 10);
        callbacks += 10;
        assert!(callbacks < 10_000, "the song does not end");
    }
    drop(ending);
    device.quiet("silence", 50);
    assert_eq!(flight.mixer_sources(), 0);
}

/// The callback's side, as written: no locks, waits, or channels. The
/// allocation check above cannot see a lock, so this reads the source of
/// the modules that the callback runs. The master control's settings,
/// which the callback reads, are lock-free too.
#[test]
fn the_callback_side_takes_no_locks() {
    let lock_words = [
        "Mutex",
        "RwLock",
        "Condvar",
        "Barrier",
        "mpsc",
        ".lock(",
        "try_lock",
        "sync_channel",
    ];
    for (name, text) in [
        ("device_mixer.rs", include_str!("../device_mixer.rs")),
        ("handoff.rs", include_str!("../handoff.rs")),
        ("source.rs", include_str!("../source.rs")),
        ("output.rs", include_str!("../output.rs")),
    ] {
        for word in lock_words {
            assert!(!text.contains(word), "{name} uses {word}");
        }
    }
    let control = include_str!("../../master/control.rs");
    assert!(!control.contains("try_lock"), "the master control locks");
}
