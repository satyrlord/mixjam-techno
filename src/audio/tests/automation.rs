use super::*;

#[test]
fn automation_reaches_breakpoints_inside_a_render_chunk() {
    let mut project = song();
    project.clips[0].start_tick = 0;
    project.clips[0].offset_frames = 0;
    project.tracks[0].automation.sends[0] = [(0, 0.0), (1, 1.0), (2, 0.0)]
        .map(|(tick, value)| crate::automation::Point { tick, value })
        .into();
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![1.0; 1000],
        },
    )]);
    let mix = mix_with_samples(&project, &samples).unwrap();
    let mut out = vec![0.0; 1024 * 2];
    let mut sends = vec![0.0; 1024 * 2 * BUSES];
    mix.read_block(0, &mut out, Some(&mut sends), &mut RenderState::default());
    assert_eq!(sends[295 * 2 * BUSES], 0.5);
}

#[test]
fn automation_readers_are_independent_of_block_sizes_and_reset_after_seek() {
    let mut project = song();
    project.clips[0].start_tick = 0;
    project.clips[0].offset_frames = 0;
    project.tracks[0].automation.filter = [(0, -0.9), (8, -0.2), (16, 0.4)]
        .map(|(tick, value)| crate::automation::Point { tick, value })
        .into();
    project.tracks[0].automation.volume = [(0, -20.0), (1, 0.0), (5, -6.0)]
        .map(|(tick, value)| crate::automation::Point { tick, value })
        .into();
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: (0..6000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect(),
        },
    )]);
    let mix = mix_with_samples(&project, &samples).unwrap();
    let mut expected = vec![0.0; 12_000];
    mix.read_block(0, &mut expected, None, &mut RenderState::default());
    let mut state = RenderState::default();
    let mut actual = vec![0.0; expected.len()];
    let mut start = 0;
    for size in [1, 131, 79, 2049, 3, 997].into_iter().cycle() {
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
    assert!(
        actual == expected,
        "block sizes changed the automation samples"
    );
    let mut seeked = [0.0; 512];
    let mut fresh = [0.0; 512];
    mix.read_block(1000, &mut seeked, None, &mut state);
    mix.read_block(1000, &mut fresh, None, &mut RenderState::default());
    assert_eq!(seeked, fresh);
}

#[test]
fn a_filter_lane_crossing_zero_renders_without_a_click() {
    let mix_of = |lane: [(u32, f32); 2]| {
        let mut project = song();
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        project.tracks[0].automation.filter = lane
            .map(|(tick, value)| crate::automation::Point { tick, value })
            .into();
        let samples = HashMap::from([(
            5,
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: (0..20_000)
                    .map(|i| {
                        let t = i as f32 / 44_100.0;
                        0.4 * ((std::f32::consts::TAU * 100.0 * t).sin()
                            + (std::f32::consts::TAU * 3000.0 * t).sin())
                    })
                    .collect(),
            },
        )]);
        let mix = mix_with_samples(&project, &samples).unwrap();
        let mut out = vec![0.0; 14_000 * 2];
        mix.read_block(0, &mut out, None, &mut RenderState::default());
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        // Include the move and the filter's 10 ms crossfade.
        let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
        let first = (f64::from(lane[0].0) * frames_per_tick).round() as usize;
        let last = (f64::from(lane[1].0) * frames_per_tick).round() as usize;
        let margin = Mix::SAMPLE_RATE as usize / 100;
        left[first - margin..last + margin]
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0, f32::max)
    };
    let crossing = mix_of([(8, -0.3), (9, 0.3)]);
    let low = mix_of([(8, -0.3), (9, -0.001)]);
    let high = mix_of([(8, 0.001), (9, 0.3)]);
    assert!(
        crossing <= low.max(high) + 0.03,
        "crossing step {crossing} against {low} and {high}"
    );
}

#[test]
fn automation_moves_the_volume_filter_and_sends_of_its_track() {
    use crate::automation::Point;
    let point = |tick, value| Point { tick, value };
    let mut project = song();
    project.clips[0].start_tick = 0;
    project.clips[0].offset_frames = 0;
    let tone = |hz: f32| Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: (0..75_600)
            .map(|frame| (std::f32::consts::TAU * hz * frame as f32 / 44_100.0).sin())
            .collect(),
    };
    let level = |project: &Project, hz: f32, from: u64, sends: bool| {
        let mix = mix_with_samples(project, &HashMap::from([(5, tone(hz))])).unwrap();
        let mut out = vec![0.0; 2048];
        let mut feeds = vec![0.0; 1024 * 2 * BUSES];
        let sending = mix.read_block(
            from,
            &mut out,
            Some(&mut feeds),
            &mut RenderState::default(),
        );
        let rms = |values: Vec<f32>| {
            (values.iter().map(|x| x * x).sum::<f32>() / values.len() as f32).sqrt()
        };
        if sends {
            assert!(sending);
            rms(feeds
                .iter()
                .skip(2 * 2)
                .step_by(2 * BUSES)
                .copied()
                .collect())
        } else {
            rms(out.iter().step_by(2).copied().collect())
        }
    };
    let plain = level(&project, 440.0, 30_000, false);
    let later = level(&project, 440.0, 40_000, false);
    // Volume rises from silence to unity over the first half bar.
    project.tracks[0].automation.volume = vec![point(0, -60.0), point(128, 0.0)];
    assert!(level(&project, 440.0, 0, false) < plain * 0.01);
    let halfway = level(&project, 440.0, 18_900 - 512, false);
    assert!((halfway / plain - 0.0316).abs() < 0.01, "{halfway}");
    assert!((level(&project, 440.0, 40_000, false) - later).abs() < 1e-6);
    // The low-pass removes a high tone and keeps a low one.
    project.tracks[0].automation.volume.clear();
    project.tracks[0].automation.filter = vec![point(0, -0.75)];
    assert!(level(&project, 5000.0, 30_000, false) < plain * 0.02);
    assert!(level(&project, 60.0, 30_000, false) > plain * 0.8);
    // A send lane replaces the knob of its bus only.
    project.tracks[0].automation.filter.clear();
    project.tracks[0].sends[2] = 0.0;
    project.tracks[0].automation.sends[2] = vec![point(0, 0.5)];
    assert!((level(&project, 440.0, 30_000, true) - plain * 0.5).abs() < 1e-4);
}

#[test]
fn a_pan_lane_pans_its_track_under_the_balance_law_and_replaces_the_knob() {
    use crate::automation::Point;
    let point = |tick, value| Point { tick, value };
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 1024).unwrap();
    let mut project = editor.project().clone();
    project.master_gain = 1.0;
    project.tracks[0].sends[0] = 1.0;
    let samples = HashMap::from([(1, steady(700_000))]);
    // Hard left through bar 1, to the center by tick 320, held to tick
    // 512, then hard right from tick 576.
    project.tracks[0].automation.pan = vec![
        point(0, -1.0),
        point(256, -1.0),
        point(320, 0.0),
        point(512, 0.0),
        point(576, 1.0),
    ];
    // The left and right output, and the FX 1 feed, at a frame.
    let at = |project: &Project, frame: u64| {
        let mix = mix_with_samples(project, &samples).unwrap();
        let mut out = vec![0.0; 2 * 64];
        let mut feeds = vec![0.0; 64 * 2 * BUSES];
        mix.read_block(
            frame,
            &mut out,
            Some(&mut feeds),
            &mut RenderState::default(),
        );
        ([out[0], out[1]], [feeds[0], feeds[1]])
    };
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: f64| (tick * frames_per_tick) as u64;
    let close = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4;
    for knob in [0.0, 0.7, -1.0] {
        project.tracks[0].pan = knob;
        for (tick, expected) in [
            (100.0, [0.5, 0.0]),
            (288.0, [0.5, 0.25]),
            (400.0, [0.5, 0.5]),
            (700.0, [0.0, 0.5]),
        ] {
            let (out, feed) = at(&project, frame(tick));
            assert!(close(out, expected), "knob {knob}, tick {tick}: {out:?}");
            // The send follows the pan, as it follows the knob.
            assert!(close(feed, expected), "knob {knob}, tick {tick}: {feed:?}");
        }
    }
    // The knob's level stays out of the track while the lane has points,
    // so moving the knob changes nothing.
    let levels = track_levels(&project);
    project.tracks[0].pan = 0.7;
    assert_eq!(track_levels(&project), levels);
    assert_eq!(levels[0], [1.0, 1.0]);
    // Without points the knob pans the track again.
    project.tracks[0].automation.pan.clear();
    let (out, _) = at(&project, frame(400.0));
    assert!(close(out, [0.15, 0.5]), "{out:?}");
    assert_eq!(track_levels(&project)[0], [0.3, 1.0]);
}

#[test]
fn live_pan_lane_changes_wait_for_the_prepared_mix_and_keep_fader_edits() {
    for automated in [false, true] {
        let mut editor = crate::engine::Editor::new(Project::default());
        editor.place(1, 0, 0, 256).unwrap();
        let mut project = editor.project().clone();
        project.master_gain = 1.0;
        project.tracks[0].pan = 1.0;
        project.tracks[0].sends[0] = 1.0;
        let lane = vec![crate::automation::Point {
            tick: 0,
            value: -1.0,
        }];
        project.tracks[0].automation.pan = if automated { lane.clone() } else { vec![] };
        let samples = HashMap::from([(1, steady(44_100))]);
        let mix = mix_with_samples(&project, &samples).unwrap();
        let live = MasterControl::new(&project.master);
        live.set_levels(&project);
        let mut state = RenderState {
            live: Some(Arc::clone(&live)),
            ..RenderState::default()
        };
        let mut out = vec![0.0; 2048];
        let mut sends = vec![0.0; 2048 * BUSES];
        mix.read_block(0, &mut out, Some(&mut sends), &mut state);
        let initial = out.clone();
        // A structural edit publishes live controls before its replacement
        // mix is ready. The old mix must keep its own pan ownership.
        project.tracks[0].automation.pan = if automated { vec![] } else { lane.clone() };
        live.set_levels(&project);
        mix.read_block(1024, &mut out, Some(&mut sends), &mut state);
        for (frame, (actual, expected)) in out
            .as_chunks::<2>()
            .0
            .iter()
            .zip(initial.as_chunks::<2>().0)
            .enumerate()
        {
            assert_eq!(
                actual, expected,
                "old mix changed pan at {frame}; automated={automated}"
            );
        }
        for (frame, send) in out.as_chunks::<2>().0.iter().zip(sends.as_chunks::<8>().0) {
            assert_eq!(*frame, [send[0], send[1]]);
        }
        // Fader and knob edits still reach the old mix while the new one
        // prepares. The knob applies only if this mix has no pan lane.
        project.tracks[0].gain = 0.5;
        project.tracks[0].pan = -0.5;
        live.set_levels(&project);
        mix.read_block(2048, &mut out, None, &mut state);
        let expected = if automated {
            [0.25, 0.0]
        } else {
            [0.25, 0.125]
        };
        assert_eq!(&out[2046..], &expected);
        // Undo the structural edit while its render is pending.
        project.tracks[0].automation.pan = if automated { lane.clone() } else { vec![] };
        live.set_levels(&project);
        mix.read_block(3072, &mut out, None, &mut state);
        assert_eq!(&out[..2], &expected);
        assert_eq!(&out[2046..], &expected);
        // Redo, then adopt the new mix: only this mix takes the new lane
        // ownership. Export uses the same resulting pan and live fader.
        project.tracks[0].automation.pan = if automated { vec![] } else { lane };
        live.set_levels(&project);
        let replacement = mix_with_samples(&project, &samples).unwrap();
        let mut replaced = RenderState {
            live: Some(live),
            ..RenderState::default()
        };
        replacement.read_block(4096, &mut out, None, &mut replaced);
        let expected = if automated {
            [0.25, 0.125]
        } else {
            [0.25, 0.0]
        };
        assert_eq!(&out[..2], &expected);
        let playback = out.clone();
        replacement.read_block(4096, &mut out, None, &mut RenderState::default());
        assert_eq!(out, playback);
    }
}

#[test]
fn a_fast_pan_sweep_moves_without_steps() {
    // Ratchets at 180 BPM: a 16th note is 16 ticks, 3675 frames, about
    // 83 ms. The lane swings from hard left to hard right and back on
    // each one, for four bars.
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 1024).unwrap();
    let mut project = editor.project().clone();
    project.bpm = 180.0;
    project.master_gain = 1.0;
    project.tracks[0].automation.pan = (0..=64)
        .map(|step| crate::automation::Point {
            tick: step * 16,
            value: if step % 2 == 0 { -1.0 } else { 1.0 },
        })
        .collect();
    let samples = HashMap::from([(1, steady(700_000))]);
    let mix = mix_with_samples(&project, &samples).unwrap();
    // Read as playback does, in blocks of uneven sizes that cross the
    // chain's 128-frame chunks at every phase.
    let frames = mix.frames() as usize;
    let mut out = vec![0.0; frames * 2];
    let mut state = RenderState::default();
    let mut at = 0;
    for size in [1000, 1, 127, 4096, 333, 129].into_iter().cycle() {
        if at >= frames {
            break;
        }
        let size = size.min(frames - at);
        mix.read_block(
            at as u64,
            &mut out[at * 2..(at + size) * 2],
            None,
            &mut state,
        );
        at += size;
    }
    let (left, right): (Vec<f32>, Vec<f32>) = out
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[left, right]| (left, right))
        .unzip();
    // The clip starts at frame 0 and ends with the song; judge the frames
    // between.
    let inner = 1..frames - 1;
    // A side's gain falls from 1 to 0 over half a sweep: 1837.5 frames.
    let slope = 0.5 / (3675.0 / 2.0);
    for (name, side) in [("left", &left), ("right", &right)] {
        let side = &side[inner.clone()];
        let step = side
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0, f32::max);
        // The clip reads 1.29 times fast, through the band-limited
        // kernel, whose gain on a steady sound ripples within 3e-5 of
        // one with the read's phase: 1.5e-5 on this 0.5.
        assert!(step <= slope * 1.01 + 2e-5, "{name} steps by {step}");
        // It swings the whole way.
        let high = side.iter().copied().fold(0.0, f32::max);
        let low = side.iter().copied().fold(1.0, f32::min);
        assert!(high > 0.499 && low < 0.001, "{name}: {low}..{high}");
    }
    // At each 16th the sound is at one side.
    let sixteenth = 3675;
    assert!(left[sixteenth] < 1e-4 && (right[sixteenth] - 0.5).abs() < 1e-4);
    assert!(right[2 * sixteenth] < 1e-4 && (left[2 * sixteenth] - 0.5).abs() < 1e-4);
}

#[test]
fn a_ducker_key_is_measured_after_its_pan_lane() {
    // The key holds sound in its left channel only, so its pan changes
    // its level: hard right silences it.
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(2, 1, 128, 128).unwrap();
    let mut project = editor.project().clone();
    let key = stereo(
        |i| 0.5 * (std::f32::consts::TAU * 60.0 * i as f32 / 44_100.0).sin(),
        |_| 0.0,
    );
    let samples = HashMap::from([(1, tone(220.0)), (2, key)]);
    let alone = |project: &Project| {
        let mix = mix_with_samples(project, &samples).unwrap();
        let mut out = vec![0.0; mix.frames() as usize * 2];
        mix.mix(0, &mut out, None, &mut RenderState::default(), Some(0));
        out
    };
    let open = alone(&project);
    project.tracks[0].ducker = crate::ducker::Ducker {
        on: true,
        key: 2,
        threshold: -40.0,
        depth: 12.0,
        attack: 0.1,
        release: 20.0,
    };
    let down = |project: &Project| {
        level_db(&alone(project), 39_000..44_000) - level_db(&open, 39_000..44_000)
    };
    assert!((down(&project) + 12.0).abs() < 0.2);
    // The lane pans the key hard right: its left channel, all its sound,
    // falls silent, and the track does not duck, whatever the knob says.
    project.tracks[1].pan = -1.0;
    project.tracks[1].automation.pan = vec![crate::automation::Point {
        tick: 0,
        value: 1.0,
    }];
    assert_eq!(alone(&project), open);
    // Hard left keeps it, though the knob is hard right.
    project.tracks[1].pan = 1.0;
    project.tracks[1].automation.pan[0].value = -1.0;
    assert!((down(&project) + 12.0).abs() < 0.2);
}
