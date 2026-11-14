use super::*;

#[test]
fn playback_glides_to_live_track_levels_and_export_keeps_the_prepared_ones() {
    for chained in [false, true] {
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        // A track with automation mixes through its chain.
        if chained {
            project.tracks[0].automation.volume = vec![crate::automation::Point {
                tick: 0,
                value: 0.0,
            }];
        }
        let source = Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.8; 75_600],
        };
        let mix = mix_with_samples(&project, &HashMap::from([(5, source)])).unwrap();
        let live = MasterControl::new(&project.master);
        live.set_levels(&project);
        let mut state = RenderState {
            live: Some(Arc::clone(&live)),
            ..RenderState::default()
        };
        let mut block = vec![0.0; 2048];
        mix.read_block(0, &mut block, None, &mut state);
        assert!(
            (block[2046] - 0.4).abs() < 1e-3,
            "chained {chained}: {}",
            block[2046]
        );
        project.tracks[0].gain = 0.25;
        live.set_levels(&project);
        mix.read_block(1024, &mut block, None, &mut state);
        let left: Vec<f32> = block.iter().step_by(2).copied().collect();
        let jump = left
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0, f32::max);
        assert!(
            jump < 1e-3,
            "chained {chained}: the level must glide, not step: {jump}"
        );
        assert!(
            (left[0] - 0.4).abs() < 2e-3,
            "chained {chained}: {}",
            left[0]
        );
        assert!(
            (left[1023] - 0.1).abs() < 1e-3,
            "chained {chained}: {}",
            left[1023]
        );
        mix.read_block(2048, &mut block, None, &mut state);
        assert!((block[0] - 0.1).abs() < 1e-3);
        // Export reads the levels the mix was prepared with.
        assert!(!mix.has_levels(&project));
        mix.read_block(2048, &mut block, None, &mut RenderState::default());
        assert!((block[0] - 0.4).abs() < 1e-3);
    }
}

#[test]
fn export_during_playback_does_not_change_filter_history() {
    let mut project = song();
    project.clips[0].start_tick = 0;
    project.clips[0].offset_frames = 0;
    project.tracks[0].automation.filter = vec![crate::automation::Point {
        tick: 0,
        value: -0.8,
    }];
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 75_600],
        },
    )]);
    let mix = Arc::new(mix_with_samples(&project, &samples).unwrap());
    let baseline: Vec<_> = PlaybackSource::new(mix.clone(), 0, None, None).collect();
    let mut playback = PlaybackSource::new(mix.clone(), 0, None, None);
    let mut actual: Vec<_> = playback.by_ref().take(2048).collect();
    mix.to_pcm(&project.master, &project.fx).unwrap();
    actual.extend(playback);
    assert!(
        actual == baseline,
        "export changed the playing filter history"
    );
}

#[test]
fn a_mix_reads_any_block_like_the_whole_render_and_has_no_length_limit() {
    let source = Pcm {
        sample_rate: 44_100,
        channels: 2,
        samples: (0..44_100 * 2)
            .map(|index| ((index % 199) as f32 / 199.0) - 0.5)
            .collect(),
    };
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(1, 3, 100, 300).unwrap();
    editor.place(1, 63, 700, 256).unwrap();
    let samples = HashMap::from([(1, source.clone())]);
    let mix = mix_with_samples(editor.project(), &samples).unwrap();
    let whole = mix
        .to_pcm(&editor.project().master, &editor.project().fx)
        .unwrap();
    // Blocks of any size and position give the same samples, and silence past the end.
    for block_frames in [1, 7, 2048, 65_536] {
        let mut pieces = Vec::new();
        let mut frame = 0;
        while frame < mix.frames() {
            let mut block = vec![1.0; block_frames * 2];
            Frames::read(&mix, frame, &mut block);
            pieces.extend(block);
            frame += block_frames as u64;
        }
        assert_eq!(pieces[..whole.samples.len()], whole.samples[..]);
        assert!(pieces[whole.samples.len()..].iter().all(|x| *x == 0.0));
    }
    // The last bar of the timeline: about 2 hours at 140 BPM.
    let mut long = crate::engine::Editor::new(Project::default());
    let last_bar = crate::engine::MAX_TICKS - 256;
    long.place(1, 0, last_bar, 256).unwrap();
    let mix = mix_with_samples(long.project(), &samples).unwrap();
    assert!(mix.frames() > 300_000_000);
    // The whole timeline at this tempo fits a WAV file, so it plays and
    // exports. `to_pcm` has no length limit of its own: it reserves room for
    // the song, and only a length that no machine holds is an error.
    assert!(mix.frames() < Mix::MAX_WAV_FRAMES);
    assert!(Mix::reserve_pcm(33_000_000).unwrap().capacity() >= 66_000_000);
    assert!(Mix::reserve_pcm(u64::MAX / 4).is_err());
    assert!(Mix::reserve_pcm(u64::MAX).is_err());
    let first = (f64::from(last_bar) * long.project().frames_per_tick(44_100)).round() as u64;
    let mut block = vec![0.0; 2048 * 2];
    Frames::read(&mix, first, &mut block);
    let gain = long.project().master_gain;
    assert_eq!(
        block[..8],
        source.samples[..8]
            .iter()
            .map(|x| x * gain)
            .collect::<Vec<_>>()[..]
    );
    let mut before = vec![1.0; 64];
    Frames::read(&mix, first - 32, &mut before);
    assert!(before.iter().all(|x| *x == 0.0));
}

#[test]
fn playback_quality_keeps_key_alignment_seeks_loops_and_export_boundary() {
    use crate::playback_quality::{Oversampling, PlaybackQuality};
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(1, 1, 0, 256).unwrap();
    let mut project = editor.project().clone();
    project.tracks[1].gain = 0.0;
    project.master.bus_compressor.on = true;
    // Key 1 is track index 0. Its active Drive runs independently in
    // the audible chain and the master's separate key chain below.
    project.master.bus_compressor.key = 1;
    let samples = HashMap::from([(
        1,
        stereo(
            |i| {
                if i == 0 {
                    0.8
                } else {
                    (i as f32 * 0.1).sin() * 0.4
                }
            },
            |i| {
                if i == 3 {
                    -0.6
                } else {
                    (i as f32 * 0.13).sin() * -0.3
                }
            },
        ),
    )]);
    let full_export = mix_with_samples(&project, &samples)
        .unwrap()
        .to_pcm(&project.master, &project.fx)
        .unwrap();
    for drive in [Oversampling::FourX, Oversampling::Off] {
        for master in [Oversampling::FourX, Oversampling::Off] {
            let quality = PlaybackQuality {
                drive,
                master,
                samples: crate::playback_quality::SampleQuality::High,
            };
            for mix_value in [0.0, 100.0] {
                project.tracks[0].drive = crate::drive::Drive {
                    on: true,
                    mix: mix_value,
                    amount: 24.0,
                    ..Default::default()
                };
                let prepared = mix_with_samples_quality(&project, &samples, quality).unwrap();
                assert_eq!(prepared.quality(), quality);
                let mut expected = vec![0.0; 4096];
                let mut state = RenderState::default();
                prepared.prepare_state(&mut state);
                let allocation = state.drives.as_ptr();
                prepared.read_block(0, &mut expected, None, &mut state);
                assert_eq!(allocation, state.drives.as_ptr());
                let keys = prepared.keys(&state, expected.len() / 2);
                assert_eq!(
                    &keys.audio[..expected.len()],
                    expected,
                    "audible/key Drive alignment {quality:?} Mix {mix_value}"
                );
                let mut blocked = vec![0.0; expected.len()];
                let mut state = RenderState::default();
                for (index, chunk) in blocked.chunks_mut(34).enumerate() {
                    prepared.read_block((index * 17) as u64, chunk, None, &mut state);
                }
                assert_eq!(blocked, expected);
                let mut seek = vec![0.0; 512];
                let mut fresh = vec![0.0; 512];
                prepared.read_block(8192, &mut seek, None, &mut state);
                prepared.read_block(8192, &mut fresh, None, &mut RenderState::default());
                assert_eq!(seek, fresh);
                if quality != PlaybackQuality::FULL {
                    assert!(prepared.to_pcm(&project.master, &project.fx).is_err());
                    assert!(
                        prepared
                            .write_wav(Path::new(""), &project.master, &project.fx)
                            .is_err()
                    );
                    assert!(
                        prepared
                            .write_stems(Path::new(""), "test", &project.fx)
                            .is_err()
                    );
                }
                let audio: Arc<dyn Frames> = Arc::new(prepared);
                for span in [1, 16, 68, 70] {
                    let mut playback = PlaybackSource::with_quality_block(
                        audio.clone(),
                        0,
                        Some([0, span]),
                        Some(MasterControl::new(&project.master)),
                        PlaybackSource::BLOCK_FRAMES,
                        quality,
                    );
                    let actual: Vec<_> = playback.by_ref().take(span as usize * 4).collect();
                    assert_eq!(actual.len(), span as usize * 4);
                    assert!(actual.iter().all(|x| x.is_finite()));
                }
                // Zero Mix keeps both independent channels at their exact
                // positions, including the compensated initial impulses.
                if mix_value == 0.0 {
                    let mut clean_project = project.clone();
                    clean_project.tracks[0].drive.on = false;
                    let clean = mix_with_samples(&clean_project, &samples).unwrap();
                    let mut dry = vec![0.0; expected.len()];
                    clean.read_block(0, &mut dry, None, &mut RenderState::default());
                    assert_eq!(dry, expected);
                }
            }
        }
    }
    project.tracks[0].drive.on = false;
    assert_eq!(
        full_export.samples,
        mix_with_samples(&project, &samples)
            .unwrap()
            .to_pcm(&project.master, &project.fx)
            .unwrap()
            .samples
    );
}

/// A song that runs every part of playback: a lane with its EQ,
/// compressor, and a ducker keyed by another lane, a narrowed lane with
/// filter automation, a lane with volume automation, sends to a delay
/// whose tail dies and to a reverb, and master pedals, one of them keyed.
fn busy_song() -> (Project, HashMap<u32, Pcm>) {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(2, 1, 64, 192).unwrap();
    editor.place(3, 2, 128, 128).unwrap();
    let mut project = editor.project().clone();
    let points = |points: &[(u32, f32)]| -> Vec<crate::automation::Point> {
        points
            .iter()
            .map(|&(tick, value)| crate::automation::Point { tick, value })
            .collect()
    };
    project.master_gain = 0.6;
    let lane = &mut project.tracks[0];
    lane.eq = crate::eq::ChannelEq {
        on: true,
        high_db: -6.0,
        ..Default::default()
    };
    lane.compressor = crate::compressor::Compressor {
        on: true,
        threshold: -24.0,
        ratio: 4.0,
        ..Default::default()
    };
    lane.ducker = crate::ducker::Ducker {
        on: true,
        key: 2,
        threshold: -24.0,
        depth: 12.0,
        attack: 1.0,
        release: 50.0,
    };
    lane.sends[0] = 0.4;
    project.tracks[1].width = 50.0;
    project.tracks[1].automation.filter = points(&[(0, -0.5), (256, 0.3)]);
    project.tracks[1].sends[1] = 0.5;
    project.tracks[2].automation.volume = points(&[(128, -12.0), (256, 0.0)]);
    project.fx.buses[0].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
        mode: crate::fx::TimeMode::Free,
        time_ms_l: 120.0,
        time_ms_r: 180.0,
        feedback: 40.0,
        ..crate::fx::Echoform::default()
    });
    project.fx.buses[1].module = crate::fx::Module::AetherformReverb(crate::fx::Aetherform {
        decay_seconds: 0.5,
        ..crate::fx::Aetherform::default()
    });
    project.master.bus_compressor = crate::master::BusCompressor {
        on: true,
        threshold: -20.0,
        ratio: 4.0,
        attack: 5.0,
        release: 100.0,
        key: 3,
    };
    project.master.multiband.on = true;
    project.master.limiter.on = true;
    let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0)), (3, tone(880.0))]);
    (project, samples)
}

/// Play `project` from its start in blocks of `block` frames. With
/// `edit`, the live controls take the edited project once the source
/// has rendered `at` frames, a multiple of `block`.
fn play_in_blocks(
    mix: &Arc<Mix>,
    project: &Project,
    block: usize,
    edit: Option<(usize, &Project)>,
) -> Vec<f32> {
    let control = MasterControl::new(&project.master);
    control.set_returns(&project.fx);
    control.set_levels(project);
    let audio: Arc<dyn Frames> = mix.clone();
    let mut source = PlaybackSource::with_block(audio, 0, None, Some(control.clone()), block);
    let mut played = Vec::new();
    if let Some((at, edited)) = edit {
        let lead = source.lead;
        played.extend(source.by_ref().take((at - lead) * 2));
        assert_eq!((source.frame, source.cursor), (at as u64, source.filled));
        control.set(&edited.master);
        control.set_returns(&edited.fx);
        control.set_levels(edited);
    }
    played.extend(source);
    played
}

#[test]
fn playback_does_not_depend_on_its_block_size() {
    let (project, samples) = busy_song();
    let mix = Arc::new(mix_with_samples(&project, &samples).unwrap());
    let exported = mix.to_pcm(&project.master, &project.fx).unwrap().samples;
    // The buses ring on for several tail blocks after the song.
    let tail = exported.len() / 2 - mix.frames() as usize;
    assert!(tail > 8 * Mix::TAIL_BLOCK as usize, "{tail}");
    for block in [PlaybackSource::BLOCK_FRAMES, 100] {
        let played = play_in_blocks(&mix, &project, block, None);
        assert_eq!(played.len(), exported.len(), "{block}-frame blocks");
        assert!(
            played == exported,
            "{block}-frame blocks differ from export"
        );
    }
    // Live edits take effect at the same frame whatever the block size,
    // a moved fader glides over the same frames, and a switched pedal
    // crossfades over them.
    let mut edited = project.clone();
    edited.tracks[0].gain = 0.3;
    edited.tracks[1].pan = -0.5;
    edited.master_gain = 0.8;
    edited.master.tape.on = true;
    edited.fx.buses[0].return_level = 0.5;
    let at = 25_600;
    let whole = play_in_blocks(&mix, &project, 1024, Some((at, &edited)));
    assert!(whole != exported);
    let short = play_in_blocks(&mix, &project, 100, Some((at, &edited)));
    assert!(short == whole, "100-frame blocks differ after an edit");
}

/// Audio that counts its reads and says on which thread it is freed.
struct Probe {
    reads: Arc<std::sync::atomic::AtomicUsize>,
    freed: std::sync::mpsc::Sender<std::thread::ThreadId>,
}

impl Frames for Probe {
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        44_100
    }
    fn frames(&self) -> u64 {
        1_000_000
    }
    fn read(&self, _first_frame: u64, out: &mut [f32]) {
        self.reads.fetch_add(1, Ordering::Relaxed);
        out.fill(0.25);
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = self.freed.send(std::thread::current().id());
    }
}

/// A device mixer playing a probe, with the count of the voices it holds.
fn probe() -> (
    super::device_mixer::DeviceMixer,
    super::device_mixer::Playback,
    Arc<std::sync::atomic::AtomicUsize>,
    Arc<std::sync::atomic::AtomicUsize>,
    std::sync::mpsc::Receiver<std::thread::ThreadId>,
) {
    let reads = Arc::default();
    let (freed, freeing) = std::sync::mpsc::channel();
    let audio = Arc::new(Probe {
        reads: Arc::clone(&reads),
        freed,
    });
    let held = Arc::default();
    let (mixer, mut input) = super::device_mixer::device_mixer(2, 44_100, Arc::clone(&held));
    let playback = input
        .start(PlaybackSource::new(audio, 0, None, None), 1.0)
        .unwrap();
    (mixer, playback, reads, held, freeing)
}

#[test]
fn a_stopped_playback_plays_out_its_block_and_renders_no_other() {
    let (mut mixer, playback, reads, held, _freeing) = probe();
    let block = PlaybackSource::BLOCK_FRAMES;
    let mut out = vec![0.0_f32; (block + 10) * 2];
    mixer.fill(&mut out, &mut ());
    assert!(out.iter().all(|sample| *sample == 0.25));
    assert_eq!(reads.load(Ordering::Relaxed), 2);
    playback.end();
    let mut out = vec![1.0_f32; block * 4];
    mixer.fill(&mut out, &mut ());
    let sounding = out.iter().filter(|sample| **sample != 0.0).count();
    assert_eq!(sounding, (block - 10) * 2);
    assert_eq!(reads.load(Ordering::Relaxed), 2);
    assert_eq!(held.load(Ordering::Relaxed), 0);
}

#[test]
fn retired_playback_is_freed_on_another_thread() {
    let (mut mixer, playback, _reads, _held, freeing) = probe();
    let mut out = vec![0.0_f32; 64];
    mixer.fill(&mut out, &mut ());
    drop(playback);
    let mut out = vec![0.0_f32; PlaybackSource::BLOCK_FRAMES * 2];
    mixer.fill(&mut out, &mut ());
    let thread = freeing
        .recv_timeout(Duration::from_secs(10))
        .expect("the audio is freed");
    assert_ne!(thread, std::thread::current().id());
    drop(mixer);
}

#[test]
fn playback_asks_for_an_audio_threads_priority_once_per_thread() {
    std::thread::spawn(|| {
        SYSTEM_PRIORITY.with(|system| system.set(true));
        let requests = || PRIORITY_REQUESTS.with(std::cell::Cell::get);
        let audio: Arc<dyn Frames> = Arc::new(Pcm {
            sample_rate: 44_100,
            channels: 2,
            samples: vec![0.0; 4096],
        });
        let first = PlaybackSource::new(Arc::clone(&audio), 0, None, None);
        let second = PlaybackSource::new(audio, 0, None, None);
        assert_eq!(requests(), 0, "made before the device reads");
        #[cfg(windows)]
        // SAFETY: the pseudo handle names the calling thread.
        let priority = || unsafe {
            windows::Win32::System::Threading::GetThreadPriority(
                windows::Win32::System::Threading::GetCurrentThread(),
            )
        };
        #[cfg(windows)]
        let normal = priority();
        assert_eq!(first.count() + second.count(), 2 * 4096);
        assert_eq!(requests(), 1);
        assert_eq!(priority_failure(), None);
        #[cfg(windows)]
        assert!(priority() > normal, "{} after {normal}", priority());
    })
    .join()
    .unwrap();
}
