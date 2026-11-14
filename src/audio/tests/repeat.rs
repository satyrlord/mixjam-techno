use super::*;

#[test]
fn a_repeat_lane_that_is_off_passes_the_sound_through_unchanged() {
    let samples = unique_source();
    let live = whole(&repeat_song(&[]), &samples);
    // A lane that never turns on, and one with points that hold off.
    for points in [&[(0, 0.0)][..], &[(256, 0.0), (512, 0.0)]] {
        assert_eq!(whole(&repeat_song(points), &samples), live, "{points:?}");
    }
    // Around a run, the sound is the live sound to the bit: before it
    // turns on, and once the ramp after it turns off is over.
    let project = repeat_song(&[(256, 16.0), (512, 0.0)]);
    let out = whole(&project, &samples);
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: f64| (tick * frames_per_tick).round() as usize;
    let back = frame(512.0) + crate::automation::REPEAT_RAMP_FRAMES as usize;
    assert_eq!(out[..frame(256.0) * 2], live[..frame(256.0) * 2]);
    assert_eq!(out[back * 2..], live[back * 2..]);
    assert_ne!(
        out[frame(300.0) * 2..back * 2],
        live[frame(300.0) * 2..back * 2]
    );
    // The same track without a chain differs only by rounding.
    let mut plain = repeat_song(&[]);
    plain.tracks[0].automation.volume.clear();
    let plain = whole(&plain, &samples);
    assert!(plain.iter().zip(&live).all(|(a, b)| (a - b).abs() < 1e-6));
}

#[test]
fn a_held_repeat_is_silent_after_the_mix_ends() {
    let samples = unique_source();
    let project = repeat_song(&[(256, 16.0)]);
    let mix = mix_with_samples(&project, &samples).unwrap();
    for first in [mix.frames(), mix.frames() + 100, u64::MAX] {
        let mut out = [1.0; 512];
        mix.read(first, &mut out);
        assert!(out.iter().all(|value| *value == 0.0), "from {first}");
    }
    // A read across the end keeps every available frame and clears
    // the rest, even when the caller reuses a nonzero buffer.
    let first = mix.frames() - 128;
    let mut expected = [0.0; 256];
    mix.read(first, &mut expected);
    assert!(expected.iter().any(|value| value.abs() > 0.01));
    let mut out = [1.0; 512];
    mix.read(first, &mut out);
    assert_eq!(out[..256], expected);
    assert!(out[256..].iter().all(|value| *value == 0.0));
}

#[test]
fn a_repeat_lane_replays_the_slice_it_captured_each_sixteenth() {
    let samples = unique_source();
    let live = whole(&repeat_song(&[]), &samples);
    // On from bar 2 to bar 3: sixteen 1/16 notes of 16 ticks.
    let project = repeat_song(&[(256, 16.0), (512, 0.0)]);
    let out = whole(&project, &samples);
    let retriggers: Vec<u32> = (256..=512).step_by(16).collect();
    assert_slices(&out, &live, &project, 256, &retriggers);
    // Every repeat is the same slice: the one that starts at bar 2.
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    let first = &out[frame(256) * 2..frame(272) * 2];
    let ramp = crate::automation::REPEAT_RAMP_FRAMES as usize * 2;
    for tick in (272..512).step_by(16) {
        let slice = &out[frame(tick) * 2..frame(tick + 16) * 2];
        let length = first.len().min(slice.len());
        for (got, want) in slice[ramp..length].iter().zip(&first[ramp..length]) {
            assert!((got - want).abs() < 1e-6, "tick {tick}");
        }
    }
    // The slice is not what played live there.
    assert!(
        (out[(frame(400) + 1000) * 2] - live[(frame(400) + 1000) * 2]).abs() > 0.05,
        "the repeat must differ from the live sound"
    );
}

#[test]
fn a_faster_division_in_the_same_run_keeps_the_slice_start() {
    let samples = unique_source();
    let live = whole(&repeat_song(&[]), &samples);
    // A roll: 1/8, then 1/16, then 1/32, then off, from bar 2.
    let project = repeat_song(&[(256, 8.0), (320, 16.0), (352, 32.0), (384, 0.0)]);
    let out = whole(&project, &samples);
    let retriggers = [256, 288, 320, 336, 352, 360, 368, 376, 384];
    assert_slices(&out, &live, &project, 256, &retriggers);
    // A later run captures again, at its own tick.
    let project = repeat_song(&[(256, 8.0), (320, 0.0), (640, 32.0), (704, 0.0)]);
    let out = whole(&project, &samples);
    assert_slices(&out, &live, &project, 256, &[256, 288, 320]);
    let retriggers: Vec<u32> = (640..=704).step_by(8).collect();
    assert_slices(&out, &live, &project, 640, &retriggers);
    // A 1/4 and a 1/64 division hold their lengths too.
    let project = repeat_song(&[(256, 4.0), (384, 64.0), (416, 0.0)]);
    let out = whole(&project, &samples);
    let mut retriggers = vec![256, 320];
    retriggers.extend((384..=416).step_by(4));
    assert_slices(&out, &live, &project, 256, &retriggers);
}

#[test]
fn repeats_cross_fade_without_steps_over_the_ramp_bound() {
    // A sustained tone, which a hard cut would break at each retrigger.
    let amplitude = 0.5;
    let hz = 220.0;
    let wave = |i: usize| amplitude * (std::f32::consts::TAU * hz * i as f32 / 44_100.0).sin();
    let samples = HashMap::from([(
        1,
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: (0..REPEAT_SOURCE_FRAMES).map(wave).collect(),
        },
    )]);
    let project = repeat_song(&[
        (256, 8.0),
        (320, 16.0),
        (352, 32.0),
        (368, 64.0),
        (376, 0.0),
        (512, 16.0),
        (520, 32.0),
        (540, 0.0),
    ]);
    let out = whole(&project, &samples);
    let live = whole(&repeat_song(&[]), &samples);
    let left = |samples: &[f32]| -> Vec<f32> {
        samples
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| pair[0])
            .collect()
    };
    let (out, live) = (left(&out), left(&live));
    let largest = |samples: &[f32]| {
        samples
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0, f32::max)
    };
    // The tone's own largest step as the track plays it, and the most
    // that an equal-gain crossfade between two copies of it can add: the
    // gap between them, at most twice the amplitude, times the ramp's
    // steepest step, pi / (2R).
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    let span = frame(250)..frame(560);
    let tone = largest(&live[span.clone()]);
    assert!(tone < 1.1 * amplitude * std::f32::consts::TAU * hz / 44_100.0);
    let ramp = crate::automation::REPEAT_RAMP_FRAMES as f32;
    let bound = tone + 2.0 * amplitude * std::f32::consts::PI / (2.0 * ramp);
    let step = largest(&out[span]);
    assert!(step <= bound * 1.001, "a step of {step} over {bound}");
    // Without the ramps the retriggers would jump: a slice ends at
    // another phase of the tone than the next one starts at.
    let capture = frame(256);
    let jump = [256, 288, 320, 336, 352, 360, 368, 372, 376]
        .windows(2)
        .map(|pair| {
            let length = frame(pair[1]) - frame(pair[0]);
            (live[capture] - live[capture + length - 1]).abs()
        })
        .fold(0.0, f32::max);
    assert!(jump > 4.0 * bound, "{jump}");
}

#[test]
fn playback_started_inside_a_repeat_run_plays_what_export_plays() {
    let samples = unique_source();
    let project = repeat_song(&[(256, 8.0), (320, 16.0), (352, 32.0), (384, 0.0)]);
    let export = whole(&project, &samples);
    let mix = mix_with_samples(&project, &samples).unwrap();
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as u64;
    let end = frame(512) as usize;
    // Mid-slice, on a retrigger, inside a crossfade, and in the ramp
    // back to the live sound: the run's start is behind each.
    for start in [
        frame(300),
        frame(336),
        frame(336) + 50,
        frame(364) + 7,
        frame(384) + 60,
    ] {
        let start = start as usize;
        let mut out = vec![0.0; (end - start) * 2];
        let mut state = RenderState::default();
        let mut at = start;
        for size in [1000, 1, 127, 4096, 333, 129].into_iter().cycle() {
            if at >= end {
                break;
            }
            let size = size.min(end - at);
            mix.read_block(
                at as u64,
                &mut out[(at - start) * 2..(at - start + size) * 2],
                None,
                &mut state,
            );
            at += size;
        }
        assert_eq!(out, export[start * 2..end * 2], "from frame {start}");
    }
    // The track's stem is the same audio.
    let mut stem = vec![0.0; export.len()];
    mix.mix(0, &mut stem, None, &mut RenderState::default(), Some(0));
    assert_eq!(stem, export);
}

#[test]
fn the_repeats_go_through_the_pan_lane_and_the_sends() {
    let samples = unique_source();
    let mut project = repeat_song(&[(256, 8.0), (320, 16.0), (352, 32.0), (384, 0.0)]);
    let still = whole(&project, &samples);
    // The pan sweeps from hard left to hard right over the roll.
    let pan =
        [(256, -1.0), (384, 1.0)].map(|(tick, value)| crate::automation::Point { tick, value });
    project.tracks[0].automation.pan = pan.to_vec();
    project.tracks[0].sends[0] = 1.0;
    let mix = mix_with_samples(&project, &samples).unwrap();
    let frames = mix.frames() as usize;
    let mut out = vec![0.0; frames * 2];
    let mut feeds = vec![0.0; frames * 2 * BUSES];
    mix.read_block(0, &mut out, Some(&mut feeds), &mut RenderState::default());
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    for at in frame(256)..frame(384) {
        let value = crate::automation::value_at(&pan, at as f64 / frames_per_tick).unwrap();
        let sides = crate::automation::pan_gains(value);
        for channel in 0..2 {
            let want = still[at * 2 + channel] * sides[channel];
            assert!((out[at * 2 + channel] - want).abs() < 1e-3, "frame {at}");
            // The send takes the repeats after the pan.
            assert_eq!(feeds[at * 2 * BUSES + channel], out[at * 2 + channel]);
        }
    }
    // The first repeats sound on the left, the last on the right.
    let energy = |range: std::ops::Range<usize>, channel: usize| -> f32 {
        range.map(|at| out[at * 2 + channel].powi(2)).sum()
    };
    assert!(energy(frame(256)..frame(264), 1) < 0.02 * energy(frame(256)..frame(264), 0));
    assert!(energy(frame(376)..frame(384), 0) < 0.02 * energy(frame(376)..frame(384), 1));
}

/// The Repeat song with the Repeat gate lane `gate` on track 1.
fn gated_song(repeat: &[(u32, f32)], gate: &[(u32, f32)]) -> Project {
    let mut project = repeat_song(repeat);
    project.tracks[0].automation.repeat_gate = gate
        .iter()
        .map(|&(tick, value)| crate::automation::Point { tick, value })
        .collect();
    project
}

#[test]
fn a_full_repeat_gate_plays_exactly_as_no_gate() {
    let samples = unique_source();
    let roll = [(256, 8.0), (320, 16.0), (352, 32.0), (384, 0.0)];
    let plain = whole(&repeat_song(&roll), &samples);
    for gate in [&[(0, 100.0)][..], &[(256, 100.0), (352, 100.0)]] {
        assert_eq!(whole(&gated_song(&roll, gate), &samples), plain, "{gate:?}");
    }
    // Without Repeat a gate changes nothing either.
    assert_eq!(
        whole(&gated_song(&[], &[(0, 25.0)]), &samples),
        whole(&repeat_song(&[]), &samples)
    );
}

#[test]
fn a_quarter_gate_silences_each_repeat_after_a_quarter_of_its_slice() {
    let samples = unique_source();
    let live = whole(&repeat_song(&[]), &samples);
    // Sixteen 1/16 repeats from bar 2, each gated to 25%.
    let project = gated_song(&[(256, 16.0), (512, 0.0)], &[(256, 25.0)]);
    let out = whole(&project, &samples);
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    let ramp = crate::automation::REPEAT_RAMP_FRAMES as usize;
    let capture = frame(256);
    for tick in (256..512).step_by(16) {
        let (start, next) = (frame(tick), frame(tick + 16));
        let open = ((next - start) as f64 * 0.25).round() as usize;
        // Open: the captured slice itself, after the retrigger's rise.
        let rise = if tick == 256 { 0 } else { ramp };
        for since in rise..open {
            for channel in 0..2 {
                let (got, want) = (
                    out[(start + since) * 2 + channel],
                    live[(capture + since) * 2 + channel],
                );
                assert!((got - want).abs() < 1e-6, "tick {tick}, {since} in");
            }
        }
        // Within the ramp after a quarter, it falls; after it, silence.
        for at in start + open + ramp..next {
            assert_eq!(out[at * 2], 0.0, "tick {tick}, frame {at}");
            assert_eq!(out[at * 2 + 1], 0.0, "tick {tick}, frame {at}");
        }
        let falling = out[(start + open + ramp / 2) * 2].abs();
        let held = live[(capture + open + ramp / 2) * 2].abs();
        assert!(falling < held || held < 1e-3, "tick {tick}");
    }
    // The live sound returns where the lane turns off.
    let back = frame(512) + ramp;
    assert_eq!(out[back * 2..], live[back * 2..]);
}

#[test]
fn a_repeat_gate_holds_each_step_from_the_repeat_where_it_starts() {
    let samples = unique_source();
    // 25% from bar 2, 75% from beat 3 of it, and a point in the middle
    // of a repeat (tick 392), which waits for the next one.
    let project = gated_song(
        &[(256, 16.0), (512, 0.0)],
        &[(256, 25.0), (384, 75.0), (392, 50.0)],
    );
    let out = whole(&project, &samples);
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    let ramp = crate::automation::REPEAT_RAMP_FRAMES as usize;
    let energy = |tick: u32, from: f64, to: f64| -> f32 {
        let (start, next) = (frame(tick), frame(tick + 16));
        let length = (next - start) as f64;
        let first = start + (length * from) as usize;
        let last = start + (length * to) as usize;
        out[first * 2..last * 2]
            .iter()
            .map(|value| value * value)
            .sum()
    };
    // At 25%, the second half of each repeat is silent.
    for tick in [272, 336, 368] {
        assert!(energy(tick, 0.0, 0.2) > 0.1, "{tick}");
        assert_eq!(energy(tick, 0.5, 1.0), 0.0, "{tick}");
    }
    // At 75%, it sounds until three quarters; tick 392's 50% waits for
    // the repeat at 400.
    assert!(energy(384, 0.55, 0.7) > 0.1);
    let silent_from = |tick: u32, percent: f64| {
        let (start, next) = (frame(tick), frame(tick + 16));
        let open = ((next - start) as f64 * percent).round() as usize;
        out[(start + open + ramp) * 2..next * 2]
            .iter()
            .all(|value| *value == 0.0)
    };
    assert!(silent_from(384, 0.75));
    assert!(energy(400, 0.3, 0.45) > 0.1);
    assert!(silent_from(400, 0.5));
    assert!(silent_from(496, 0.5));
}

#[test]
fn playback_started_inside_a_gated_run_plays_what_export_plays() {
    let samples = unique_source();
    let project = gated_song(
        &[(256, 8.0), (320, 16.0), (352, 32.0), (384, 0.0)],
        &[(256, 40.0), (336, 10.0), (360, 70.0)],
    );
    let export = whole(&project, &samples);
    let mix = mix_with_samples(&project, &samples).unwrap();
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as u64;
    let end = frame(512) as usize;
    for start in [
        frame(300),
        frame(336) + 30,
        frame(337),
        frame(364) + 7,
        frame(384) + 60,
    ] {
        let start = start as usize;
        let mut out = vec![0.0; (end - start) * 2];
        let mut state = RenderState::default();
        let mut at = start;
        for size in [1000, 1, 127, 4096, 333, 129].into_iter().cycle() {
            if at >= end {
                break;
            }
            let size = size.min(end - at);
            mix.read_block(
                at as u64,
                &mut out[(at - start) * 2..(at - start + size) * 2],
                None,
                &mut state,
            );
            at += size;
        }
        assert_eq!(out, export[start * 2..end * 2], "from frame {start}");
    }
    let mut stem = vec![0.0; export.len()];
    mix.mix(0, &mut stem, None, &mut RenderState::default(), Some(0));
    assert_eq!(stem, export);
}
