use super::*;

#[test]
fn a_track_whose_eq_is_off_renders_exactly_as_before() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(2, 1, 0, 256).unwrap();
    let mut project = editor.project().clone();
    project.tracks[1].automation.volume = vec![
        crate::automation::Point {
            tick: 0,
            value: -12.0,
        },
        crate::automation::Point {
            tick: 256,
            value: 0.0,
        },
    ];
    let samples = HashMap::from([(1, tone(220.0)), (2, tone(5000.0))]);
    let before = render_with_samples(&project, &samples).unwrap();
    // Settings that the EQ keeps while it is off change nothing, on a
    // plain track and on an automated one.
    for track in &mut project.tracks[..2] {
        track.eq = crate::eq::ChannelEq {
            low_cut: 400.0,
            high_db: -18.0,
            mid_db: 9.0,
            ..Default::default()
        };
    }
    assert!(render_with_samples(&project, &samples).unwrap().samples == before.samples);
    // Switched on with every band flat, it passes the sound unchanged.
    for track in &mut project.tracks[..2] {
        track.eq = crate::eq::ChannelEq {
            on: true,
            ..Default::default()
        };
    }
    let flat = render_with_samples(&project, &samples).unwrap();
    assert!(
        flat.samples
            .iter()
            .zip(&before.samples)
            .all(|(a, b)| (a - b).abs() < 1e-6)
    );
}

#[test]
fn the_channel_eq_shapes_its_track_before_the_automation() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(2, 1, 0, 256).unwrap();
    let mut project = editor.project().clone();
    let samples = HashMap::from([(1, tone(12_000.0)), (2, tone(200.0))]);
    let solo = |project: &Project, track: usize| {
        let mut alone = project.clone();
        alone.tracks[track].solo = true;
        render_with_samples(&alone, &samples).unwrap().samples
    };
    // The tones last one second of the one-bar clips.
    let frames = 11_025..44_100;
    let open = [0, 1].map(|track| level_db(&solo(&project, track), frames.clone()));
    // A high shelf at 8 kHz lowers 12 kHz by about its gain; 200 Hz stays.
    for track in &mut project.tracks[..2] {
        track.eq = crate::eq::ChannelEq {
            on: true,
            high_db: -12.0,
            ..Default::default()
        };
    }
    let top = level_db(&solo(&project, 0), frames.clone()) - open[0];
    assert!((top + 12.0).abs() < 1.5, "12 kHz: {top:.2} dB");
    let low = level_db(&solo(&project, 1), frames.clone()) - open[1];
    assert!(low.abs() < 0.5, "200 Hz: {low:.2} dB");
    // A low cut at 200 Hz removes 50 Hz, on a track with automation too.
    let samples = HashMap::from([(1, tone(50.0)), (2, tone(50.0))]);
    let mut project = editor.project().clone();
    project.tracks[1].automation.filter = vec![crate::automation::Point {
        tick: 0,
        value: 0.0,
    }];
    let open = level_db(
        &render_with_samples(&project, &samples).unwrap().samples,
        frames.clone(),
    );
    for track in &mut project.tracks[..2] {
        track.eq = crate::eq::ChannelEq {
            on: true,
            low_cut: 200.0,
            ..Default::default()
        };
    }
    let cut = level_db(
        &render_with_samples(&project, &samples).unwrap().samples,
        frames,
    );
    assert!(cut - open < -20.0, "50 Hz: {:.2} dB", cut - open);
}

#[test]
fn a_drive_does_not_read_sound_outside_the_playback_loop() {
    let mut editor = crate::engine::Editor::new(Project::default());
    // The selected first bar is silent. Sound starts in the next bar.
    editor.place(1, 0, 256, 256).unwrap();
    let mut project = editor.project().clone();
    // The master's key reads the same track in its own chain.
    project.master.bus_compressor.on = true;
    project.master.bus_compressor.key = 1;
    let end = (256.0 * project.frames_per_tick(Mix::SAMPLE_RATE)).round() as u64;
    let samples = HashMap::from([(1, tone(5000.0))]);
    for on in [false, true] {
        project.tracks[0].drive = crate::drive::Drive {
            on,
            amount: 24.0,
            mix: 100.0,
            focus: 20.0,
            ..Default::default()
        };
        for frames in [16, 256] {
            let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
            let start = end - frames as u64;
            let mut playback = PlaybackSource::new(mix, start, Some([start, end]), None);
            // Two cycles check the restart as well. The 16-frame loop
            // ends before even the drive's priming read has finished.
            let output: Vec<f32> = playback.by_ref().take(frames * 4).collect();
            assert_eq!(output.len(), frames * 4);
            let peak = output.iter().fold(0.0_f32, |peak, x| peak.max(x.abs()));
            assert_eq!(
                peak, 0.0,
                "drive on={on}, {frames} frames: sound outside loop leaked at {peak}"
            );
            let keys = playback.audio.keys(&playback.state, frames);
            assert!(!keys.audio.is_empty());
            assert!(keys.audio.iter().all(|sample| *sample == 0.0));
        }
    }
}

#[test]
fn a_drive_that_is_off_or_at_no_mix_renders_exactly_as_before() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(2, 1, 0, 256).unwrap();
    let mut project = editor.project().clone();
    project.tracks[1].eq.on = true;
    project.tracks[1].eq.low_db = 6.0;
    let samples = HashMap::from([(1, tone(55.0)), (2, tone(5000.0))]);
    let render = |project: &Project| render_with_samples(project, &samples).unwrap().samples;
    let before = render(&project);
    // Settings that the drive keeps while it is off change nothing.
    for track in &mut project.tracks[..2] {
        track.drive = crate::drive::Drive {
            amount: 24.0,
            mix: 100.0,
            ..Default::default()
        };
    }
    assert!(render(&project) == before);
    // On at no mix, it gives the sound back bit for bit and on time: the
    // chain reads ahead by the drive's delay, on a plain track and after
    // an EQ.
    for track in &mut project.tracks[..2] {
        track.drive.on = true;
        track.drive.mix = 0.0;
    }
    assert!(render(&project) == before);
}

#[test]
fn the_drive_adds_harmonics_before_the_fader_and_keeps_the_level() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    let samples = HashMap::from([(1, tone(55.0))]);
    let render = |project: &Project| render_with_samples(project, &samples).unwrap().samples;
    // The tone lasts one second of the one-bar clip; 0.5 s settles.
    let frames = 22_050..44_100;
    let left = |samples: &[f32]| -> Vec<f32> {
        samples[frames.start * 2..frames.end * 2]
            .iter()
            .step_by(2)
            .copied()
            .collect()
    };
    let amplitude = |samples: &[f32], hz: f64| {
        let (mut re, mut im) = (0.0, 0.0);
        for (frame, x) in left(samples).iter().enumerate() {
            let phase = std::f64::consts::TAU * hz * (frames.start + frame) as f64 / 44_100.0;
            re += f64::from(*x) * phase.cos();
            im += f64::from(*x) * phase.sin();
        }
        2.0 * (re * re + im * im).sqrt() / frames.len() as f64
    };
    let clean = render(&project);
    let open = level_db(&clean, frames.clone());
    project.tracks[0].drive = crate::drive::Drive {
        on: true,
        amount: 24.0,
        character: 0.0,
        mix: 100.0,
        focus: 110.0,
        output: 0.0,
    };
    let driven = render(&project);
    // The level holds, and the fundamental with it, while the 3rd
    // harmonic comes up from nothing.
    let level = level_db(&driven, frames.clone()) - open;
    assert!(level.abs() < 0.2, "{level:.2} dB");
    let fundamental = 20.0 * (amplitude(&driven, 55.0) / amplitude(&clean, 55.0)).log10();
    assert!(fundamental.abs() < 0.75, "{fundamental:.2} dB");
    let third = 20.0 * (amplitude(&driven, 165.0) / amplitude(&driven, 55.0)).log10();
    let before = 20.0 * (amplitude(&clean, 165.0) / amplitude(&clean, 55.0)).log10();
    assert!(
        before < -90.0 && third > -14.0,
        "{before:.1} to {third:.1} dB"
    );
    // The fader follows the drive, so it does not change how hard the
    // drive works: 12 dB down on the fader is 12 dB down.
    project.tracks[0].gain = crate::engine::db_to_amplitude(-12.0);
    let lower = level_db(&render(&project), frames.clone()) - open;
    assert!((lower + 12.0 - level).abs() < 0.01, "{lower:.3} dB");
    // The output trim moves it by hand.
    project.tracks[0].gain = 1.0;
    project.tracks[0].drive.output = -3.0;
    let trimmed = level_db(&render(&project), frames) - open;
    assert!((trimmed + 3.0 - level).abs() < 0.01, "{trimmed:.3} dB");
}

#[test]
fn a_driven_track_reads_alike_in_any_blocks_and_after_a_seek() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    project.tracks[0].eq.on = true;
    project.tracks[0].eq.high_db = -6.0;
    project.tracks[0].drive = crate::drive::Drive {
        on: true,
        amount: 18.0,
        ..Default::default()
    };
    let samples = HashMap::from([(1, tone(220.0))]);
    let mix = mix_with_samples(&project, &samples).unwrap();
    let mut expected = vec![0.0; 12_000];
    mix.read_block(0, &mut expected, None, &mut RenderState::default());
    let mut state = RenderState::default();
    mix.prepare_state(&mut state);
    let mut actual = vec![0.0; expected.len()];
    let mut start = 0;
    for size in [1, 131, 79, 2049, 3, 997, 68, 69, 70].into_iter().cycle() {
        let end = (start + size).min(actual.len() / 2);
        mix.read_block(
            start as u64,
            &mut actual[start * 2..end * 2],
            None,
            &mut state,
        );
        start = end;
        if start == actual.len() / 2 {
            break;
        }
    }
    assert!(actual == expected, "block sizes changed the drive");
    // A seek starts the drive's history again at the new frame, as a
    // fresh read does, and leads it up to the frame: no silent gap.
    let mut seeked = [0.0; 512];
    let mut fresh = [0.0; 512];
    mix.read_block(20_000, &mut seeked, None, &mut state);
    mix.read_block(20_000, &mut fresh, None, &mut RenderState::default());
    assert_eq!(seeked, fresh);
    assert!(
        seeked[..crate::drive::LATENCY * 2]
            .iter()
            .any(|x| x.abs() > 0.1)
    );
}

#[test]
fn playback_drive_history_is_allocated_before_the_first_read() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(1, 1, 0, 256).unwrap();
    let mut project = editor.project().clone();
    // A drive on a plain track and on a ducker's key, which mixes in a
    // chain of its own too.
    project.tracks[0].drive.on = true;
    project.tracks[1].drive.on = true;
    project.tracks[0].ducker.on = true;
    project.tracks[0].ducker.key = 2;
    let samples = HashMap::from([(1, tone(110.0))]);
    let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
    let mut playback = PlaybackSource::new(mix, 0, None, Some(MasterControl::new(&project.master)));
    assert_eq!(playback.state.drives.len(), 3);
    let allocation = playback.state.drives.as_ptr();
    assert!(playback.fill());
    assert_eq!(playback.state.drives.len(), 3);
    assert_eq!(playback.state.drives.as_ptr(), allocation);
}

#[test]
fn the_width_folds_or_spreads_a_track_before_its_pan() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    let wave = |i: usize| 0.25 * (std::f32::consts::TAU * 330.0 * i as f32 / 44_100.0).sin();
    // An anti-phase source: all side, no mid.
    let samples = HashMap::from([(1, stereo(wave, |i| -wave(i)))]);
    let render = |project: &Project| render_with_samples(project, &samples).unwrap().samples;
    let before = render(&project);
    project.tracks[0].width = 100.0;
    assert!(render(&project) == before, "100% leaves the track as it is");
    let peak = |samples: &[f32]| samples.iter().fold(0.0_f32, |peak, x| peak.max(x.abs()));
    project.tracks[0].width = 0.0;
    assert!(peak(&render(&project)) < 1e-6, "0% folds the side away");
    project.tracks[0].width = 200.0;
    let wide = render(&project);
    assert!(
        wide.iter()
            .zip(&before)
            .all(|(wide, open)| (wide - 2.0 * open).abs() < 1e-6),
        "200% doubles the side"
    );
    // A track on one output has no side: the width leaves it alone.
    project.tracks[0].right_enabled = false;
    project.tracks[0].width = 100.0;
    let one = render(&project);
    project.tracks[0].width = 0.0;
    assert!(render(&project) == one);
    // The width works before the pan: a left-only source folded to mono
    // and panned hard left keeps half its level on the left, through
    // the master gain, and none on the right.
    let samples = HashMap::from([(1, stereo(wave, |_| 0.0))]);
    let mut project = editor.project().clone();
    project.tracks[0].width = 0.0;
    project.tracks[0].pan = -1.0;
    let panned = render_with_samples(&project, &samples).unwrap().samples;
    let (left, right): (Vec<f32>, Vec<f32>) = panned
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[left, right]| (left, right))
        .unzip();
    assert!(peak(&right) < 1e-6);
    let half = 0.125 * project.master_gain;
    assert!((peak(&left) - half).abs() < 1e-3, "{}", peak(&left));
}
