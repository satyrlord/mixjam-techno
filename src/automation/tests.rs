use super::*;

fn points(pairs: &[(u32, f32)]) -> Vec<Point> {
    pairs
        .iter()
        .map(|&(tick, value)| Point { tick, value })
        .collect()
}

#[test]
fn a_lane_holds_its_ends_and_moves_in_lines_between_points() {
    let lane = points(&[(256, -12.0), (512, 0.0), (1024, 0.0)]);
    assert_eq!(value_at(&[], 10.0), None);
    assert_eq!(value_at(&lane, 0.0), Some(-12.0));
    assert_eq!(value_at(&lane, 384.0), Some(-6.0));
    assert_eq!(value_at(&lane, 4096.0), Some(0.0));
    let envelope = Envelope::new(&lane, 2.0).unwrap();
    assert_eq!(envelope.at(0), -12.0);
    assert_eq!(envelope.at(768), -6.0);
    assert_eq!(envelope.at(1024), 0.0);
    assert_eq!(envelope.at(100_000), 0.0);
}

#[test]
fn lanes_reject_points_out_of_order_or_range() {
    let mut automation = Automation {
        volume: points(&[(0, -60.0), (256, 6.0)]),
        filter: points(&[(0, -1.0), (8, 1.0)]),
        ..Default::default()
    };
    automation.sends[2] = points(&[(0, 0.0), (64, 1.0)]);
    automation.pan = points(&[(0, -1.0), (16, 1.0), (32, 0.0)]);
    automation.validate().unwrap();
    for (lane, bad) in [
        (Lane::Volume, points(&[(0, 6.5)])),
        (Lane::Send(1), points(&[(0, -0.1)])),
        (Lane::Filter, points(&[(10, 0.0), (10, 0.5)])),
        (Lane::Filter, points(&[(10, 0.0), (5, 0.5)])),
        (Lane::Volume, points(&[(crate::engine::MAX_TICKS + 1, 0.0)])),
        (Lane::Volume, points(&[(0, f32::NAN)])),
        (Lane::Pan, points(&[(0, -1.01)])),
        (Lane::Pan, points(&[(0, 1.01)])),
        (Lane::Pan, points(&[(4, 0.0), (4, 1.0)])),
        (Lane::Repeat, points(&[(0, 12.0)])),
        (Lane::Repeat, points(&[(0, 16.5)])),
        (Lane::Repeat, points(&[(0, 128.0)])),
        (Lane::Repeat, points(&[(0, 2.0)])),
        (Lane::Repeat, points(&[(0, -16.0)])),
        (Lane::Repeat, points(&[(8, 16.0), (8, 0.0)])),
        (Lane::RepeatGate, points(&[(0, 4.9)])),
        (Lane::RepeatGate, points(&[(0, 100.5)])),
        (Lane::RepeatGate, points(&[(0, f32::NAN)])),
        (Lane::RepeatGate, points(&[(8, 50.0), (8, 25.0)])),
        (Lane::Pitch, points(&[(0, -4800.5)])),
        (Lane::Pitch, points(&[(0, 2400.5)])),
        (Lane::Pitch, points(&[(0, f32::NAN)])),
        (Lane::Pitch, points(&[(8, 0.0), (8, -1200.0)])),
    ] {
        let mut changed = automation.clone();
        *changed.lane_mut(lane) = bad;
        assert!(changed.validate().is_err(), "{lane:?}");
    }
    assert_eq!(Lane::from_key("send3"), Some(Lane::Send(2)));
    assert_eq!(Lane::from_key("pan"), Some(Lane::Pan));
    assert_eq!(Lane::from_key("balance"), None);
    assert_eq!(Lane::Pan.key(), "pan");
    assert_eq!(Lane::Pan.label(), "Pan");
    assert_eq!(Lane::from_key("repeat"), Some(Lane::Repeat));
    assert_eq!(Lane::Repeat.label(), "Repeat");
    automation.repeat = points(&[(0, 0.0), (16, 4.0), (32, 8.0), (48, 16.0), (64, 32.0)]);
    automation.repeat.push(Point {
        tick: 80,
        value: 64.0,
    });
    automation.validate().unwrap();
    assert_eq!(Lane::from_key("repeatgate"), Some(Lane::RepeatGate));
    assert_eq!(Lane::RepeatGate.label(), "Repeat gate");
    automation.repeat_gate = points(&[(0, 5.0), (16, 37.5), (32, 100.0)]);
    automation.validate().unwrap();
    assert_eq!(Lane::from_key("pitch"), Some(Lane::Pitch));
    assert_eq!(Lane::Pitch.label(), "Pitch");
    automation.pitch = points(&[(0, -4800.0), (16, 2400.0), (32, 12.5)]);
    automation.validate().unwrap();
    assert!(!automation.is_empty());
}

#[test]
fn pitch_values_read_as_semitones_and_rest_at_zero() {
    assert_eq!(Lane::Pitch.format(0.0), "0 st");
    assert_eq!(Lane::Pitch.format(-4800.0), "-48 st");
    assert_eq!(Lane::Pitch.format(350.0), "+3.50 st");
    assert_eq!(Lane::Pitch.range(), (-4800.0, 2400.0));
    assert!(!Lane::Pitch.steps());
    assert!(Lane::Pitch.rests_before_first());
    assert_eq!(Lane::Pitch.before_first(), 0.0);
    assert!(!Lane::Pan.rests_before_first());
}

#[test]
fn the_varispeed_clock_integrates_the_speed_of_the_pitch_lane() {
    // At 100 frames a tick: normal speed to tick 10, an octave down at
    // once, a straight fall in cents to two octaves down at tick 20, and
    // held there.
    let clock = Varispeed::new(&points(&[(10, -1200.0), (20, -2400.0)]), 100.0).unwrap();
    assert_eq!(clock.at(0), 0.0);
    assert_eq!(clock.at(1000), 1000.0);
    // The integral of 2^(-1 - x / 1000) over 1000 frames.
    let fall = 1000.0 * (0.5 - 0.25) / std::f64::consts::LN_2;
    assert!((clock.at(2000) - (1000.0 + fall)).abs() < 1e-9);
    assert!((clock.at(1500) - clock.at(1499) - 2.0_f64.powf(-1.5)).abs() < 1e-3);
    assert!((clock.at(3000) - clock.at(2000) - 250.0).abs() < 1e-9);
    // Each frame advances the clock: it never stands still or runs back.
    for frame in 0..4000 {
        assert!(clock.at(frame + 1) > clock.at(frame));
    }
    // A flat lane at 0 is the frame itself.
    let flat = Varispeed::new(&points(&[(0, 0.0), (40, 0.0)]), 100.0).unwrap();
    assert_eq!(flat.at(1234), 1234.0);
    assert_eq!(flat.at(9999), 9999.0);
    // The frame where the clock has run a distance.
    let reached = clock.reach(2000, 100.0, 10_000);
    assert_eq!(reached, 2400);
    assert_eq!(clock.reach(2000, 1e9, 10_000), 10_000);
    assert!(Varispeed::new(&[], 100.0).is_none());
}

#[test]
fn return_lanes_hold_zero_to_twice_the_full_return() {
    validate_return_points(&points(&[(0, 0.0), (64, 2.0), (128, 0.35)])).unwrap();
    for bad in [
        points(&[(0, -0.01)]),
        points(&[(0, 2.01)]),
        points(&[(0, f32::INFINITY)]),
        points(&[(64, 1.0), (64, 0.5)]),
        points(&[(crate::engine::MAX_TICKS + 1, 1.0)]),
    ] {
        assert!(validate_return_points(&bad).is_err(), "{bad:?}");
    }
    assert_eq!(return_label(1.5), "150%");
    assert!(no_returns(&Default::default()));
}

#[test]
fn a_repeat_gate_reads_as_a_percentage_and_steps_from_full() {
    assert_eq!(Lane::RepeatGate.format(25.0), "25%");
    assert_eq!(Lane::RepeatGate.range(), (5.0, 100.0));
    assert!(Lane::RepeatGate.steps());
    assert_eq!(Lane::RepeatGate.before_first(), 100.0);
    assert_eq!(Lane::Repeat.before_first(), 0.0);
}

#[test]
fn a_gated_repeat_plays_its_share_of_the_slice_then_falls_silent() {
    let ramp = REPEAT_RAMP_FRAMES;
    // At 100 frames a tick, 1/16 (16 ticks) is 1600 frames. The gate is
    // 25% from tick 10, 50% from tick 42, and the lane rolls from 10.
    let lane = points(&[(10, 16.0), (58, 0.0)]);
    let gate = points(&[(10, 25.0), (42, 50.0)]);
    let repeat = Repeat::new(&lane, &gate, 100.0).unwrap();
    // The first slice at T is open for 400 frames, then falls.
    assert_eq!(repeat.tap(1000 + 399).main, (1399, 1.0));
    let falling = repeat.tap(1000 + 400 + ramp / 2).main.1;
    assert!(falling > 0.3 && falling < 0.7, "{falling}");
    assert_eq!(repeat.tap(1000 + 400 + ramp).main.1, 0.0);
    assert_eq!(repeat.tap(2599).main.1, 0.0);
    // The next repeat rises, open, and the silent one stays silent.
    let tap = repeat.tap(2600 + 3);
    assert!(tap.main.1 > 0.0);
    assert_eq!(tap.tail.unwrap().1, 0.0);
    assert_eq!(repeat.tap(2600 + ramp + 10).main, (1000 + ramp + 10, 1.0));
    assert_eq!(repeat.tap(2600 + 400 + ramp).main.1, 0.0);
    // From tick 42 each repeat holds 50%: 800 frames.
    assert_eq!(repeat.tap(4200 + 799).main.1, 1.0);
    assert_eq!(repeat.tap(4200 + 800 + ramp).main.1, 0.0);
    // Off at tick 58: the live sound returns over the silent tail.
    let tap = repeat.tap(5800 + 2);
    assert_eq!(tap.main.0, 5802);
    assert_eq!(tap.tail.unwrap().1, 0.0);
    // At 100% the gate changes nothing.
    let full = Repeat::new(&lane, &points(&[(0, 100.0)]), 100.0).unwrap();
    let plain = Repeat::new(&lane, &[], 100.0).unwrap();
    for frame in (900..6200).step_by(7) {
        assert_eq!(full.tap(frame), plain.tap(frame), "{frame}");
    }
    // Without Repeat the gate does nothing at all.
    assert!(Repeat::new(&[], &gate, 100.0).is_none());
}

#[test]
fn repeat_values_read_as_off_or_note_divisions_and_step() {
    let labels: Vec<String> = REPEAT_STEPS
        .iter()
        .map(|value| Lane::Repeat.format(*value))
        .collect();
    assert_eq!(labels, ["Off", "1/4", "1/8", "1/16", "1/32", "1/64"]);
    assert!(Lane::Repeat.steps());
    assert!(
        Lane::ALL
            .iter()
            .all(|lane| lane.steps() == matches!(lane, Lane::Repeat | Lane::RepeatGate))
    );
}

#[test]
fn a_repeat_lane_reads_its_slice_from_where_it_turned_on() {
    let ramp = REPEAT_RAMP_FRAMES;
    // At 100 frames a tick, 1/16 (16 ticks) is 1600 frames.
    let lane = points(&[(0, 0.0), (10, 16.0), (26, 16.0), (42, 32.0), (60, 0.0)]);
    let repeat = Repeat::new(&lane, &[], 100.0).unwrap();
    let live = |frame| Tap {
        main: (frame, 1.0),
        tail: None,
    };
    // Off before the run, and the live sound itself at its start.
    assert_eq!(repeat.tap(999), live(999));
    assert_eq!(repeat.tap(1000), live(1000));
    assert_eq!(repeat.tap(2599), live(2599));
    // A point that holds the same division does not retrigger: the
    // 1/16 retriggers every 1600 frames from 1000.
    let tap = repeat.tap(2600 + ramp);
    assert_eq!(
        tap,
        Tap {
            main: (1000 + ramp, 1.0),
            tail: None
        }
    );
    assert_eq!(repeat.tap(4200 + ramp + 5).main, (1005 + ramp, 1.0));
    // In a crossfade the old slice plays on under the new one.
    let tap = repeat.tap(2600 + 10);
    assert_eq!(tap.main.0, 1010);
    assert_eq!(tap.tail.unwrap().0, 1000 + 1600 + 10);
    assert!((tap.main.1 + tap.tail.unwrap().1 - 1.0).abs() < 1e-6);
    // At tick 42 the division turns to 1/32, 800 frames, from 1000.
    assert_eq!(repeat.tap(4200 + 799).main.0, 1000 + 799);
    assert_eq!(repeat.tap(5000 + ramp).main, (1000 + ramp, 1.0));
    // Off at tick 60, frame 6000: the live sound rises over the last
    // slice, which started at 5800.
    let tap = repeat.tap(6000 + 3);
    assert_eq!(tap.main.0, 6003);
    assert_eq!(tap.tail.unwrap().0, 1000 + 203);
    assert_eq!(repeat.tap(6000 + ramp), live(6000 + ramp));
    // A lane that never turns on is no Repeat at all.
    assert!(Repeat::new(&points(&[(0, 0.0), (64, 0.0)]), &[], 100.0).is_none());
    assert!(Repeat::new(&[], &[], 100.0).is_none());
}

#[test]
fn pan_values_read_as_left_center_or_right() {
    assert_eq!(Lane::Pan.format(0.0), "C");
    assert_eq!(Lane::Pan.format(-0.004), "C");
    assert_eq!(Lane::Pan.format(-0.4), "L 40");
    assert_eq!(Lane::Pan.format(1.0), "R 100");
    assert_eq!(Lane::Pan.format(0.26), "R 26");
    // The balance law: the far side falls, the near side stays.
    assert_eq!(pan_gains(0.0), [1.0, 1.0]);
    assert_eq!(pan_gains(-1.0), [1.0, 0.0]);
    assert_eq!(pan_gains(1.0), [0.0, 1.0]);
    assert_eq!(pan_gains(0.25), [0.75, 1.0]);
}
