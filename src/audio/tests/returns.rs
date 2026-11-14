use super::*;

#[test]
fn playback_and_export_stop_at_the_same_tail_limit() {
    let mut project = song();
    project.clips[0].start_tick = 0;
    project.clips[0].offset_frames = 0;
    project.tracks[0].sends[0] = 1.0;
    project.fx.buses[0].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
        mode: crate::fx::TimeMode::Free,
        time_ms_l: 10.0,
        time_ms_r: 10.0,
        feedback: 110.0,
        character: crate::fx::DelayCharacter::Digital,
        mod_depth: 0.0,
        duck_amount: 0.0,
        ..crate::fx::Echoform::default()
    });
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: (0..4410).map(|i| (i as f32 * 0.05).sin() * 0.5).collect(),
        },
    )]);
    let mix = Arc::new(mix_with_samples(&project, &samples).unwrap());
    let exported = mix.to_pcm(&project.master, &project.fx).unwrap();
    let control = MasterControl::new(&project.master);
    control.set_returns(&project.fx);
    let played: Vec<_> = PlaybackSource::new(mix, 0, None, Some(control)).collect();
    assert_eq!(played.len(), exported.samples.len());
    assert!(played == exported.samples, "playback differs from export");
}

#[test]
fn zero_level_returns_have_identical_playback_and_export_duration() {
    let mut project = song();
    project.clips[0].start_tick = 0;
    project.clips[0].offset_frames = 0;
    project.tracks[0].sends[0] = 1.0;
    project.fx.buses[0].return_level = 0.0;
    project.fx.buses[0].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
        feedback: 0.0,
        mod_depth: 0.0,
        ..crate::fx::Echoform::default()
    });
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 100],
        },
    )]);
    let mix = Arc::new(mix_with_samples(&project, &samples).unwrap());
    let exported = mix.to_pcm(&project.master, &project.fx).unwrap();
    let control = MasterControl::new(&project.master);
    control.set_returns(&project.fx);
    let played: Vec<_> = PlaybackSource::new(mix, 0, None, Some(control)).collect();
    assert_eq!(played.len(), exported.samples.len());
    assert!(played == exported.samples);
}

/// A one-bar song at 140 BPM: a 0.1 s tone on track 1 that sends to a
/// half-second echo on FX 1, whose Return knob is at `level` and whose
/// return lane is `lane`. The bus's safety limiter is off, so the return
/// is linear.
fn thrown_song(level: f32, lane: &[(u32, f32)]) -> (Project, HashMap<u32, Pcm>) {
    let source = Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: (0..4410).map(|i| ((i as f32) * 0.05).sin() * 0.5).collect(),
    };
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    project.fx.buses[0].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
        mode: crate::fx::TimeMode::Free,
        time_ms_l: 500.0,
        time_ms_r: 500.0,
        feedback: 40.0,
        duck_amount: 0.0,
        ..crate::fx::Echoform::default()
    });
    project.fx.buses[0].return_level = level;
    project.fx.buses[0].limiter = false;
    project.tracks[0].sends[0] = 0.8;
    project.return_automation[0] = lane
        .iter()
        .map(|&(tick, value)| crate::automation::Point { tick, value })
        .collect();
    (project, HashMap::from([(1, source)]))
}

#[test]
fn return_automation_wraps_with_the_playback_loop() {
    let (mut project, samples) = thrown_song(1.0, &[]);
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as u64;
    let region = [frame(80), frame(155)];
    let render = |project: &Project| {
        let control = MasterControl::new(&project.master);
        control.set_returns(&project.fx);
        let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(project, &samples).unwrap());
        PlaybackSource::new(mix, 0, Some(region), Some(control))
            .take((region[1] as usize + 200) * 2)
            .collect::<Vec<_>>()
    };
    let full = render(&project);
    project.return_automation[0] = [(80, 0.0), (81, 0.0), (154, 1.0), (155, 1.0)]
        .map(|(tick, value)| crate::automation::Point { tick, value })
        .to_vec();
    let automated = render(&project);
    let after_wrap = region[1] as usize * 2;
    let full = &full[after_wrap..after_wrap + 400];
    let automated = &automated[after_wrap..after_wrap + 400];
    assert!(full.iter().any(|sample| sample.abs() > 1e-4));
    assert!(automated.iter().all(|sample| sample.abs() < 1e-7));
}

#[test]
fn a_return_lane_replaces_the_return_knob() {
    let (project, samples) = thrown_song(0.5, &[]);
    let knob = render_with_samples(&project, &samples).unwrap();
    // A lane held at the knob's value sounds exactly like the knob, so
    // the knob's own value no longer counts.
    for level in [0.0, 1.0] {
        let (project, samples) = thrown_song(level, &[(0, 0.5)]);
        assert_eq!(
            render_with_samples(&project, &samples).unwrap().samples,
            knob.samples
        );
    }
    let (silent, samples) = thrown_song(0.0, &[]);
    let silent = render_with_samples(&silent, &samples).unwrap();
    assert_ne!(silent.samples, knob.samples);
    // Without points the knob counts again.
    let (cleared, samples) = thrown_song(0.5, &[]);
    assert_eq!(
        render_with_samples(&cleared, &samples).unwrap().samples,
        knob.samples
    );
}

#[test]
fn a_return_lane_glides_at_every_frame_without_steps() {
    // The first echo arrives at 0.5 s, tick 74.7 at 140 BPM, and lasts
    // 0.1 s. The lane rises from silence to full over ticks 70 to 90.
    let lane = [(70, 0.0), (90, 1.0)];
    let (project, samples) = thrown_song(1.0, &lane);
    let moving = render_with_samples(&project, &samples).unwrap().samples;
    let (dry, samples) = thrown_song(0.0, &[]);
    let dry = render_with_samples(&dry, &samples).unwrap().samples;
    let (full, samples) = thrown_song(1.0, &[]);
    let full = render_with_samples(&full, &samples).unwrap().samples;
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let points = lane.map(|(tick, value)| crate::automation::Point { tick, value });
    let (first, last) = (
        (70.0 * frames_per_tick) as usize,
        (90.0 * frames_per_tick) as usize,
    );
    let mut checked = 0;
    for at in first..last {
        // The return is the full one scaled by the lane at this very
        // frame, a straight line from frame to frame.
        let gain = crate::automation::value_at(&points, at as f64 / frames_per_tick).unwrap();
        let wet = full[at * 2] - dry[at * 2];
        let got = moving[at * 2] - dry[at * 2];
        assert!(
            (got - wet * gain).abs() < 2e-3 * (1.0 + wet.abs()),
            "frame {at}"
        );
        if wet.abs() > 0.05 {
            assert!((got / wet - gain).abs() < 0.02, "frame {at}: {}", got / wet);
            checked += 1;
        }
    }
    assert!(checked > 1000, "{checked}");
    // Within one block of 256 frames the gain still rises at each frame.
    let block = (80.0 * frames_per_tick) as usize;
    let ratios: Vec<f32> = (block..block + 256)
        .filter(|at| (full[at * 2] - dry[at * 2]).abs() > 0.1)
        .map(|at| (moving[at * 2] - dry[at * 2]) / (full[at * 2] - dry[at * 2]))
        .collect();
    assert!(ratios.len() > 20);
    assert!(ratios.first().unwrap() < ratios.last().unwrap());
}

#[test]
fn a_return_throw_rings_an_echo_tail_after_the_dry_sound_stops() {
    // The return is shut, then thrown to 200% from the end of the tone's
    // bar onward, after the tone itself has stopped.
    let throw = [(200, 0.0), (240, 2.0)];
    let (project, samples) = thrown_song(0.0, &throw);
    let thrown = render_with_samples(&project, &samples).unwrap().samples;
    let (shut, samples) = thrown_song(0.0, &[]);
    let shut = render_with_samples(&shut, &samples).unwrap().samples;
    let (open, samples) = thrown_song(1.0, &[]);
    let open = render_with_samples(&open, &samples).unwrap().samples;
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let after = (240.0 * frames_per_tick) as usize;
    let rms = |samples: &[f32]| -> f64 {
        let tail = &samples[after * 2..];
        (tail.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / tail.len() as f64).sqrt()
    };
    // The dry tone has long stopped: the shut return leaves silence.
    assert_eq!(rms(&shut), 0.0);
    // The throw rings on as an echo tail, twice the full return.
    assert!(rms(&thrown) > 1e-3, "{}", rms(&thrown));
    let ratio = rms(&thrown) / rms(&open);
    assert!((ratio - 2.0).abs() < 0.01, "{ratio}");
    assert!(thrown.len() > 2 * (after + 44_100), "the tail lasts");
}

#[test]
fn a_return_lane_plays_and_exports_alike_through_the_tail() {
    // The lane keeps moving past the song's end, into the tail.
    let (project, samples) = thrown_song(1.0, &[(0, 1.0), (256, 0.2), (700, 1.5)]);
    let exported = render_with_samples(&project, &samples).unwrap();
    let control = MasterControl::new(&project.master);
    control.set_returns(&project.fx);
    let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
    let played: Vec<f32> = PlaybackSource::new(mix.clone(), 0, None, Some(control)).collect();
    assert_eq!(played, exported.samples);
    let (still, samples) = thrown_song(1.0, &[(0, 1.0), (256, 0.2)]);
    assert_ne!(
        render_with_samples(&still, &samples).unwrap().samples,
        exported.samples
    );
}

#[test]
fn sends_feed_the_fx_buses_whose_tails_ring_past_the_song_in_export_and_playback() {
    let source = Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: (0..4410).map(|i| ((i as f32) * 0.05).sin() * 0.5).collect(),
    };
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    let samples = HashMap::from([(1, source)]);
    let dry = render_with_samples(&project, &samples).unwrap();
    // A bus with an effect but no sends changes nothing.
    project.fx.buses[0].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
        mode: crate::fx::TimeMode::Free,
        time_ms_l: 500.0,
        time_ms_r: 500.0,
        feedback: 40.0,
        duck_amount: 0.0,
        ..crate::fx::Echoform::default()
    });
    assert_eq!(
        render_with_samples(&project, &samples).unwrap().samples,
        dry.samples
    );
    project.tracks[0].sends[0] = 0.8;
    let wet = render_with_samples(&project, &samples).unwrap();
    // The song is one bar at 140 BPM; the echoes ring on after it.
    assert!(wet.samples.len() > dry.samples.len() + 44_100);
    let echo = &wet.samples[2 * 22_050..2 * 26_460];
    assert!(
        echo.iter().any(|v| v.abs() > 0.05),
        "the first echo is half a second in"
    );
    let end = &wet.samples[wet.samples.len() - 2048..];
    assert!(
        end.iter().all(|v| v.abs() < 1e-3),
        "the tail dies out before the end"
    );

    let control = MasterControl::new(&project.master);
    control.set_returns(&project.fx);
    let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
    let played: Vec<f32> = PlaybackSource::new(mix, 0, None, Some(control)).collect();
    assert_eq!(played, wet.samples);
}
