use super::*;

#[test]
fn playback_meters_include_the_final_output_frame() {
    let mut samples = vec![0.0; 1_024 * 2];
    samples[1_023 * 2] = 1.0;
    let audio: Arc<dyn Frames> = Arc::new(Pcm {
        sample_rate: 44_100,
        channels: 2,
        samples: samples.clone(),
    });
    let control = MasterControl::new(&MasterChain::default());
    let played: Vec<_> = PlaybackSource::new(audio, 0, None, Some(control.clone())).collect();
    assert_eq!(played, samples);
    let reading = control.meters();
    assert!(reading.true_peak_max >= 0.0, "{reading:?}");
    assert!(reading.overs > 0, "{reading:?}");
    assert_eq!(reading.integrated, f32::NEG_INFINITY);
}

#[test]
fn live_pedal_switches_keep_the_timeline_and_final_frame() {
    for initially_on in [false, true] {
        let mut samples = vec![0.0; 10_000 * 2];
        for frame in [0, 7_000, 9_999] {
            samples[frame * 2] = 0.25;
            samples[frame * 2 + 1] = -0.25;
        }
        let audio: Arc<dyn Frames> = Arc::new(Pcm {
            sample_rate: 44_100,
            channels: 2,
            samples: samples.clone(),
        });
        let mut chain = MasterChain::default();
        chain.limiter.on = initially_on;
        chain.limiter.gain = 0.0;
        let control = MasterControl::new(&chain);
        let mut source = PlaybackSource::new(audio, 0, None, Some(control.clone()));
        let mut played: Vec<_> = source.by_ref().take(2_048 * 2).collect();
        chain.limiter.on = !initially_on;
        control.set(&chain);
        played.extend(source);
        assert_eq!(played.len(), samples.len());
        for (index, (played, expected)) in played.iter().zip(&samples).enumerate() {
            assert_eq!(
                played, expected,
                "Sample {index}; limiter initially on: {initially_on}"
            );
        }
    }
}

#[test]
fn neutral_live_switches_crossfade_aligned_audio() {
    let samples: Vec<_> = (0..10_000)
        .flat_map(|frame| {
            let sample = (frame as f32 * 0.13).sin() * 0.25;
            [sample, -sample]
        })
        .collect();
    for initially_on in [false, true] {
        let audio: Arc<dyn Frames> = Arc::new(Pcm {
            sample_rate: 44_100,
            channels: 2,
            samples: samples.clone(),
        });
        let mut chain = MasterChain::default();
        chain.limiter.on = initially_on;
        chain.limiter.gain = 0.0;
        let control = MasterControl::new(&chain);
        let mut source = PlaybackSource::new(audio, 0, None, Some(control.clone()));
        let mut played: Vec<_> = source.by_ref().take(2_048 * 2).collect();
        chain.limiter.on = !initially_on;
        control.set(&chain);
        played.extend(source);
        assert_eq!(played.len(), samples.len());
        for (index, (played, expected)) in played.iter().zip(&samples).enumerate() {
            assert_eq!(
                played, expected,
                "Sample {index}; limiter initially on: {initially_on}"
            );
        }
    }
}

#[test]
fn playback_and_export_run_the_same_master_pedalboard() {
    let source = Pcm {
        sample_rate: 44_100,
        channels: 2,
        samples: (0..20_000)
            .map(|i| ((i as f32) * 0.037).sin() * if i % 2 == 0 { 0.6 } else { 0.3 })
            .collect(),
    };
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    let samples = HashMap::from([(1, source)]);
    let dry = render_with_samples(&project, &samples).unwrap();
    project.master.booster_compressor.on = true;
    project.master.booster_wide.on = true;
    project.master.booster_eq.on = true;
    project.master.booster_eq.bands[0] = 90;
    let exported = render_with_samples(&project, &samples).unwrap();
    assert_ne!(exported.samples, dry.samples);

    let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
    let played: Vec<f32> = PlaybackSource::new(
        mix.clone(),
        0,
        None,
        Some(MasterControl::new(&project.master)),
    )
    .collect();
    assert_eq!(played, exported.samples);
    // Without a control, as for a tool preview, the mix plays dry.
    let preview: Vec<f32> = PlaybackSource::new(mix, 0, None, None).collect();
    assert_eq!(preview, dry.samples);

    // Pedals that delay the sound still line up with the song: a limiter
    // with nothing to limit gives back the dry mix exactly.
    let mut delayed = editor.project().clone();
    delayed.master.limiter.on = true;
    delayed.master.limiter.gain = 0.0;
    assert_eq!(
        render_with_samples(&delayed, &samples).unwrap().samples,
        dry.samples
    );
    delayed.master.tube.on = true;
    delayed.master.limiter.gain = 6.0;
    let exported = render_with_samples(&delayed, &samples).unwrap();
    assert_eq!(exported.samples.len(), dry.samples.len());
    let mix = mix_with_samples(&delayed, &samples).unwrap();
    let path = std::env::temp_dir().join(format!("mixjam-master-{}.wav", std::process::id()));
    mix.write_wav(&path, &delayed.master, &delayed.fx).unwrap();
    let written = read_wav(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(written.samples.len(), exported.samples.len());
    assert!(
        written
            .samples
            .iter()
            .zip(&exported.samples)
            .all(|(a, b)| float_to_i16(*a) == float_to_i16(*b))
    );
    let played: Vec<f32> = PlaybackSource::new(
        Arc::new(mix),
        0,
        None,
        Some(MasterControl::new(&delayed.master)),
    )
    .collect();
    assert_eq!(played, exported.samples);
}

#[test]
fn bypass_skips_the_trim_and_pedals_for_export_and_playback() {
    let source = Pcm {
        sample_rate: 44_100,
        channels: 2,
        samples: (0..20_000)
            .map(|i| ((i as f32) * 0.037).sin() * if i % 2 == 0 { 0.6 } else { 0.3 })
            .collect(),
    };
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    let samples = HashMap::from([(1, source)]);
    let dry = render_with_samples(&project, &samples).unwrap();
    project.master.trim = -9.0;
    project.master.tube.on = true;
    project.master.limiter.on = true;
    project.master.booster_compressor.on = true;
    let mastered = render_with_samples(&project, &samples).unwrap();
    assert_ne!(mastered.samples, dry.samples);
    project.master.bypass = true;
    assert!(!project.master.is_active());
    assert_eq!(project.master.latency(), 0);
    let bypassed = render_with_samples(&project, &samples).unwrap();
    assert_eq!(bypassed.samples, dry.samples);
    let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
    let played: Vec<f32> = PlaybackSource::new(
        mix.clone(),
        0,
        None,
        Some(MasterControl::new(&project.master)),
    )
    .collect();
    assert_eq!(played, dry.samples);

    // Bypassing during playback crossfades to the dry mix and keeps the
    // song on the timeline.
    project.master.bypass = false;
    let control = MasterControl::new(&project.master);
    let mut playback = PlaybackSource::new(mix, 0, None, Some(Arc::clone(&control)));
    let mut live: Vec<f32> = playback.by_ref().take(8192).collect();
    control.set(&MasterChain {
        bypass: true,
        ..project.master
    });
    live.extend(playback);
    assert_eq!(live.len(), dry.samples.len());
    // The switch lands at frame 4096. The new board fills its delay and
    // the 30 ms crossfade runs, while the trim glides to unity; by frame
    // 8192 the dry mix plays exactly.
    assert_ne!(live[..2 * 4096], dry.samples[..2 * 4096]);
    assert_eq!(live[2 * 8192..], dry.samples[2 * 8192..]);
}
