use super::*;
use std::f64::consts::TAU;

const RATE: f64 = 44_100.0;

/// A stereo tone of `hz` with a peak of `peak`, `frames` long.
fn tone(hz: f64, peak: f64, frames: usize) -> Vec<f32> {
    (0..frames)
        .flat_map(|frame| {
            let x = (peak * (TAU * hz * frame as f64 / RATE).sin()) as f32;
            [x, x]
        })
        .collect()
}

/// The input with the drive's delay taken off: frame `n` of the result
/// is the output for input frame `n`.
fn drive(drive: &Drive, input: &[f32]) -> Vec<f32> {
    let run = Run::new(drive);
    let mut state = State::new(&run);
    let mut output = input.to_vec();
    output.extend(std::iter::repeat_n(0.0, LATENCY * 2));
    run.process(&mut output, &mut state);
    output.drain(..LATENCY * 2);
    output
}

/// The amplitude of `hz` in the left channel of whole seconds of frames,
/// by correlation.
fn amplitude(samples: &[f32], hz: f64) -> f64 {
    let frames = samples.len() / 2;
    let (mut re, mut im) = (0.0, 0.0);
    for frame in 0..frames {
        let phase = TAU * hz * frame as f64 / RATE;
        let x = f64::from(samples[frame * 2]);
        re += x * phase.cos();
        im += x * phase.sin();
    }
    2.0 * (re * re + im * im).sqrt() / frames as f64
}

fn db(value: f64) -> f64 {
    20.0 * value.log10()
}

/// The mean power of the left channel, in dB.
fn power_db(samples: &[f32]) -> f64 {
    let left: Vec<f64> = samples.iter().step_by(2).map(|x| f64::from(*x)).collect();
    10.0 * (left.iter().map(|x| x * x).sum::<f64>() / left.len() as f64).log10()
}

/// The last two seconds of three, past the detectors' settling.
fn settled(samples: &[f32]) -> &[f32] {
    &samples[44_100 * 2..]
}

fn on() -> Drive {
    Drive {
        on: true,
        ..Drive::default()
    }
}

#[test]
fn cached_curve_means_match_uncached_means_across_small_steps() {
    for character in [0.0, 0.25, 0.5, 1.0] {
        let mut mean = Mean::default();
        let mut previous = 0.0;
        // Repeated and close samples take the midpoint branch. The
        // wider step after them must calculate the correct endpoint,
        // including a return to silence and a crossing through zero.
        for next in [
            0.0,
            0.0,
            1e-7,
            0.5,
            0.500_000_1,
            -0.5,
            -0.500_000_1,
            10.0,
            -10.0,
            0.0,
            0.0,
        ] {
            let distance: f64 = next - previous;
            let expected = if distance.abs() < 1e-5 {
                curve(0.5 * (previous + next), character)
            } else {
                (integral(next, character) - integral(previous, character)) / distance
            };
            assert_eq!(mean.next(next, character).to_bits(), expected.to_bits());
            previous = next;
        }
    }
}

#[test]
fn initial_silence_fast_path_matches_full_processing_bit_for_bit() {
    for prefix in [0, 1, 68, 69, 70, 140, 1024] {
        for negative_zero in [false, true] {
            let mut input = vec![0.0_f32; prefix * 2];
            if negative_zero && !input.is_empty() {
                input[0] = -0.0;
            }
            // Start in one channel, then both. Leave a silent tail
            // after the sound so it must keep processing its histories.
            input.extend([0.8, 0.0]);
            input.extend(tone(220.0, 0.5, 512));
            input.extend(std::iter::repeat_n(0.0, 1024 * 2));
            for mix in [0.0, 50.0, 100.0] {
                let run = Run::new(&Drive {
                    amount: 24.0,
                    character: 100.0,
                    mix,
                    ..on()
                });
                let mut state = State::new(&run);
                let mut full = State {
                    started: true,
                    ..State::new(&run)
                };
                let mut actual = input.clone();
                let mut expected = input.clone();
                // A prefix can end inside a block or at its boundary.
                for (actual, expected) in actual.chunks_mut(34).zip(expected.chunks_mut(34)) {
                    run.process(actual, &mut state);
                    run.process(expected, &mut full);
                }
                assert!(state.started);
                let mismatch = actual
                    .iter()
                    .zip(&expected)
                    .position(|(a, b)| a.to_bits() != b.to_bits());
                assert!(
                    mismatch.is_none(),
                    "prefix {prefix}, negative zero {negative_zero}, mix {mix}: {mismatch:?}"
                );
                assert!(actual[(prefix + 513) * 2..].iter().any(|x| *x != 0.0));
            }
        }
    }
    let run = Run::new(&on());
    let mut state = State::new(&run);
    run.process(&mut [0.0; 1024], &mut state);
    assert!(!state.started);
    run.process(&mut [-0.0, 0.0], &mut state);
    assert!(state.started);
}

#[test]
fn settings_validate_their_ranges() {
    let mut drive = Drive::default();
    assert!(!drive.on && drive.is_default());
    assert_eq!(
        (
            drive.amount,
            drive.character,
            drive.mix,
            drive.focus,
            drive.output
        ),
        (12.0, 25.0, 50.0, 100.0, 0.0)
    );
    drive.validate().unwrap();
    for param in DriveParam::ALL {
        let range = param.range();
        for bad in [range.low - 1.0, range.high + 1.0, f32::NAN, f32::INFINITY] {
            let mut changed = drive;
            match param {
                DriveParam::Amount => changed.amount = bad,
                DriveParam::Character => changed.character = bad,
                DriveParam::Mix => changed.mix = bad,
                DriveParam::Focus => changed.focus = bad,
                DriveParam::Output => changed.output = bad,
            }
            assert!(changed.validate().is_err(), "{param:?} {bad}");
        }
        let mut held = drive;
        param.set(&mut held, range.high * 10.0);
        assert_eq!(param.get(&held), range.high);
    }
    DriveParam::Focus.set(&mut drive, 5.0);
    assert_eq!(drive.focus, 20.0);
}

#[test]
fn a_zero_mix_passes_the_track_bit_for_bit() {
    let input = tone(55.0, 0.5, 44_100);
    let output = drive(
        &Drive {
            mix: 0.0,
            amount: 24.0,
            character: 100.0,
            ..on()
        },
        &input,
    );
    assert!(output == input);
}

#[test]
fn playback_qualities_keep_delay_stereo_identity_and_block_history() {
    let mut input = tone(55.0, 0.6, 4096);
    // Independent channels, an initial impulse, and a source gap.
    input[..2].copy_from_slice(&[0.8, -0.4]);
    for frame in input.as_chunks_mut::<2>().0 {
        frame[1] *= -0.5;
    }
    input[1024..2048].fill(0.0);
    for quality in [Oversampling::FourX, Oversampling::Off] {
        for mix in [0.0, 100.0] {
            let settings = Drive {
                amount: 24.0,
                character: 100.0,
                mix,
                ..on()
            };
            let run = Run::with_quality(&settings, quality);
            let mut state = State::new(&run);
            let mut whole = input.clone();
            whole.extend([0.0; LATENCY * 2]);
            run.process(&mut whole, &mut state);
            if quality == Oversampling::Off || mix == 0.0 {
                assert!(whole[..LATENCY * 2].iter().all(|x| *x == 0.0));
            }
            let mut blocked = input.clone();
            blocked.extend([0.0; LATENCY * 2]);
            let mut state = State::new(&run);
            for block in blocked.chunks_mut(34) {
                run.process(block, &mut state);
            }
            assert_eq!(whole, blocked, "{quality:?} mix {mix}");
            if mix == 0.0 {
                assert_eq!(&whole[LATENCY * 2..], input);
            } else {
                assert_ne!(&whole[LATENCY * 2..], input, "Off must still saturate");
                assert!(whole.iter().all(|x| x.is_finite()));
            }
            if quality == Oversampling::FourX {
                let mut legacy = input.clone();
                legacy.extend([0.0; LATENCY * 2]);
                let legacy_run = Run::new(&settings);
                legacy_run.process(&mut legacy, &mut State::new(&legacy_run));
                assert_eq!(whole, legacy);
            }
        }
    }
}

#[test]
fn base_rate_curve_and_reference_have_the_same_half_frame_phase() {
    let input = tone(10_000.0, 1e-8, 44_100);
    // The reference is the straight line's interval mean at the base
    // rate, not the undelayed current sample. Integer compensation does
    // not remove its half-frame phase or its cos(pi*f/rate) roll-off.
    let mut held = 0.0_f32;
    let mut mean = Mean::default();
    let mut pair_output = Vec::new();
    for (frame, samples) in input.as_chunks::<2>().0.iter().enumerate() {
        let pair = mean.pair(samples[0], &mut held, 1.0, 1.0, 0.0);
        let previous = if frame == 0 {
            0.0
        } else {
            input[(frame - 1) * 2]
        };
        let expected = 0.5 * (previous + samples[0]);
        assert!((pair[0] - expected).abs() < 2e-15);
        assert_eq!(pair[1], expected);
        pair_output.extend([pair[1], pair[1]]);
    }
    let magnitude = amplitude(&pair_output, 10_000.0) / amplitude(&input, 10_000.0);
    let expected = (std::f64::consts::PI * 10_000.0 / RATE).cos();
    assert!((magnitude - expected).abs() < 1e-4);
}

#[test]
fn playback_quality_records_alias_and_frequency_response_tradeoffs() {
    let input = tone(5000.0, 0.9, 2 * 44_100);
    let settings = Drive {
        amount: 24.0,
        character: 0.0,
        mix: 100.0,
        focus: 20.0,
        ..on()
    };
    let mut aliases = [0.0; 2];
    for (index, quality) in [Oversampling::FourX, Oversampling::Off]
        .into_iter()
        .enumerate()
    {
        let run = Run::with_quality(&settings, quality);
        let mut state = State::new(&run);
        let mut output = input.clone();
        output.extend([0.0; LATENCY * 2]);
        run.process(&mut output, &mut state);
        let output = &output[(44_100 + LATENCY) * 2..];
        let fundamental = amplitude(output, 5000.0);
        // The fifth harmonic above Nyquist folds from 25 to 19.1 kHz.
        aliases[index] = db(amplitude(output, 19_100.0) / fundamental);
        println!(
            "Drive {quality:?}: 5 kHz tone, 24 dB Drive, 19.1 kHz alias {:.2} dBc",
            aliases[index]
        );
    }
    assert!(aliases[0] < -65.0);
    assert!(aliases[1] > aliases[0] + 10.0);
    for hz in [55.0, 1000.0, 5000.0, 10_000.0, 19_000.0] {
        let reference_db = db((std::f64::consts::PI * hz / RATE).cos());
        println!(
            "Drive Off interval-mean reference at {hz} Hz: {reference_db:.3} dB, half-frame phase lag"
        );
    }
}

#[test]
fn a_held_55_hz_tone_gains_harmonics_and_keeps_its_fundamental() {
    let input = tone(55.0, 0.5, 3 * 44_100);
    let clean = settled(&input);
    let fundamental = amplitude(clean, 55.0);
    // A pure tone has no harmonics to speak of.
    for harmonic in [110.0, 165.0] {
        assert!(db(amplitude(clean, harmonic) / fundamental) < -100.0);
    }
    // Parallel harmonics: the focus at 110 Hz, twice the note, keeps
    // the fundamental clean. Symmetric gives the 3rd, not the 2nd.
    let symmetric = Drive {
        amount: 24.0,
        character: 0.0,
        mix: 100.0,
        focus: 110.0,
        ..on()
    };
    let output = drive(&symmetric, &input);
    let output = settled(&output);
    let kept = db(amplitude(output, 55.0) / fundamental);
    assert!(kept.abs() < 0.75, "fundamental {kept:.2} dB");
    let second = db(amplitude(output, 110.0) / fundamental);
    let third = db(amplitude(output, 165.0) / fundamental);
    assert!(second < -60.0, "2nd {second:.1} dB");
    assert!((third + 10.7).abs() < 1.5, "3rd {third:.1} dB");
    // One-sided, the 2nd leads. It sits on the focus, 3 dB down from
    // the -11 dB that the curve makes.
    let even = Drive {
        character: 100.0,
        ..symmetric
    };
    let output = drive(&even, &input);
    let output = settled(&output);
    let kept = db(amplitude(output, 55.0) / fundamental);
    assert!(kept.abs() < 0.75, "fundamental {kept:.2} dB");
    let second = db(amplitude(output, 110.0) / fundamental);
    let third = db(amplitude(output, 165.0) / fundamental);
    assert!((second + 13.5).abs() < 2.0, "2nd {second:.1} dB");
    assert!(
        second > third + 6.0,
        "2nd {second:.1} dB, 3rd {third:.1} dB"
    );
    // Half the mix adds half the harmonics: 6 dB less.
    let half = drive(
        &Drive {
            mix: 50.0,
            ..symmetric
        },
        &input,
    );
    let third = db(amplitude(settled(&half), 165.0) / fundamental);
    assert!((third + 16.7).abs() < 1.5, "3rd {third:.1} dB");
}

#[test]
fn the_drive_keeps_the_track_power() {
    for (hz, peak) in [(49.0, 0.5), (87.0, 0.2), (440.0, 0.7)] {
        let input = tone(hz, peak, 3 * 44_100);
        let before = power_db(settled(&input));
        for (amount, character, mix, focus) in [
            (12.0, 25.0, 50.0, 100.0),
            (24.0, 0.0, 100.0, 20.0),
            (24.0, 100.0, 100.0, 150.0),
            (18.0, 60.0, 30.0, 400.0),
        ] {
            let settings = Drive {
                amount,
                character,
                mix,
                focus,
                ..on()
            };
            let after = power_db(settled(&drive(&settings, &input)));
            assert!(
                (after - before).abs() < 0.2,
                "{hz} Hz {settings:?}: {:.2} dB",
                after - before
            );
            // The output trim moves the level by hand.
            let trimmed = power_db(settled(&drive(
                &Drive {
                    output: -6.0,
                    ..settings
                },
                &input,
            )));
            assert!((trimmed - after + 6.0).abs() < 0.01, "{trimmed:.2}");
        }
    }
}

#[test]
fn the_result_carries_no_offset() {
    let input = tone(55.0, 0.8, 3 * 44_100);
    let output = drive(
        &Drive {
            amount: 24.0,
            character: 100.0,
            mix: 100.0,
            focus: 20.0,
            ..on()
        },
        &input,
    );
    let output = settled(&output);
    let left: Vec<f64> = output.iter().step_by(2).map(|x| f64::from(*x)).collect();
    let mean = left.iter().sum::<f64>() / left.len() as f64;
    assert!(mean.abs() < 1e-4, "offset {mean:e}");
    // Without the blocker the one-sided curve's offset would be about a
    // third of the peak.
    let peak = left.iter().fold(0.0_f64, |peak, x| peak.max(x.abs()));
    assert!(peak > 0.3, "{peak}");
}

#[test]
fn high_notes_driven_hard_do_not_alias() {
    // 3 kHz and 5 kHz at 1 dB under full scale, driven as hard as the
    // drive goes, with each character. Their harmonics are multiples of
    // the note; an alias would land between them. Plain four-times
    // oversampling leaves the 33rd harmonic of 5 kHz at -45 dB, folded
    // to 11.4 kHz.
    for hz in [3000.0, 5000.0] {
        let input = tone(hz, 0.9, 3 * 44_100);
        for character in [0.0, 50.0, 100.0] {
            let output = drive(
                &Drive {
                    amount: 24.0,
                    character,
                    mix: 100.0,
                    focus: 20.0,
                    ..on()
                },
                &input,
            );
            let output = settled(&output);
            let fundamental = amplitude(output, hz);
            // Every 100 Hz from 100 Hz to 20 kHz that is not a harmonic.
            let worst = (1..=200)
                .map(|step| f64::from(step) * 100.0)
                .filter(|frequency| (frequency / hz).fract().abs() > 1e-9)
                .map(|frequency| db(amplitude(output, frequency) / fundamental))
                .fold(f64::NEG_INFINITY, f64::max);
            assert!(worst < -65.0, "{hz} Hz {character}%: alias {worst:.1} dB");
            // The harmonics are there.
            let third = db(amplitude(output, 3.0 * hz) / fundamental);
            assert!(third > -35.0, "{hz} Hz {character}%: 3rd {third:.1} dB");
        }
    }
}

#[test]
fn the_gains_follow_level_jumps_without_pumping() {
    // A 50 Hz bass, whose cycle is 882 frames, that jumps 12 dB up and
    // then 24 dB down at the start of a cycle.
    let mut input = tone(50.0, 0.125, 44_100);
    input.extend(tone(50.0, 0.5, 44_100));
    input.extend(tone(50.0, 0.03, 44_100));
    let output = drive(
        &Drive {
            amount: 24.0,
            mix: 100.0,
            focus: 100.0,
            ..on()
        },
        &input,
    );
    // Cycle by cycle, the result keeps the clean level as it moves. The
    // gains glide: within 100 ms of a jump the level settles, and on the
    // way it moves by at most a few dB, without a click.
    let changes: Vec<f64> = (0..150)
        .map(|cycle| {
            let frames = cycle * 882 * 2..(cycle + 1) * 882 * 2;
            power_db(&output[frames.clone()]) - power_db(&input[frames])
        })
        .collect();
    for (cycle, change) in changes.iter().enumerate() {
        let settling = [50, 100]
            .iter()
            .any(|jump| (*jump..jump + 5).contains(&cycle));
        let bound = if settling { 3.0 } else { 0.5 };
        assert!(change.abs() < bound, "cycle {cycle}: {changes:.2?} dB");
    }
}

#[test]
fn nearby_settings_sound_nearby() {
    // Each control, swept in 48 steps across its range, changes the
    // harmonics and the level a little at a time, with no jump. The
    // controls sweep on their own threads: this is the suite's longest test.
    let input = tone(50.0, 0.5, 44_100);
    std::thread::scope(|scope| {
        for param in DriveParam::ALL {
            let input = &input;
            scope.spawn(move || sweep_control(param, input));
        }
    });
    fn sweep_control(param: DriveParam, input: &[f32]) {
        let range = param.range();
        let mut sweep = Vec::new();
        for index in 0..=48 {
            let mut settings = Drive {
                focus: 100.0,
                ..on()
            };
            let position = f64::from(index) / 48.0;
            let value = if range.logarithmic {
                f64::from(range.low) * f64::from(range.high / range.low).powf(position)
            } else {
                f64::from(range.low) + f64::from(range.high - range.low) * position
            };
            param.set(&mut settings, value as f32);
            let output = drive(&settings, input);
            // The last half second: 25 whole cycles.
            let output = &output[44_100..];
            let total = 10.0_f64.powf(power_db(output) / 10.0);
            let fundamental = amplitude(output, 50.0).powi(2) / 2.0;
            let harmonics = (total - fundamental).max(0.0).sqrt();
            sweep.push((value, power_db(output), harmonics));
        }
        let most = sweep.iter().fold(0.0_f64, |most, step| most.max(step.2));
        for pair in sweep.windows(2) {
            let [(_, level, harmonics), (value, next_level, next)] = [pair[0], pair[1]];
            assert!(
                (next - harmonics).abs() <= 0.15 * most,
                "{param:?} at {value:.1}: harmonics {harmonics:.4} to {next:.4}"
            );
            assert!(
                (next_level - level).abs() < 0.75,
                "{param:?} at {value:.1}: {level:.2} to {next_level:.2} dB"
            );
        }
    }
}
