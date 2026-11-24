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
fn antiderivative2_derivative_matches_first_integral_across_characters() {
    let h = 1e-6;
    for character in [0.0, 0.25, 0.5, 1.0] {
        assert_eq!(antiderivative2(0.0, character), 0.0);
        for u in [
            -20.0, -5.0, -0.75, -0.5001, -0.4999, -0.1, -1e-4, 1e-4, 0.1, 0.4999, 0.5001, 0.75,
            5.0, 20.0,
        ] {
            let numeric =
                (antiderivative2(u + h, character) - antiderivative2(u - h, character)) / (2.0 * h);
            let exact = integral(u, character);
            let tol = 1e-8 * (1.0 + exact.abs());
            assert!(
                (numeric - exact).abs() < tol,
                "u={u} c={character}: numeric={numeric} exact={exact}"
            );
        }
        // ADAA2 degenerate (< ADAA2_EPS) uses triangular tent weights [0.25, 0.5, 0.25];
        // well-separated samples use exact ADAA2 quotient weights [1/3, 1/3, 1/3].
        let lut = &*F2_TANH_LUT;
        let mut adaa = Adaa2::default();
        let mut x1 = 0.0;
        let mut x2 = 0.0;
        for x0 in [0.0, 0.0, 1e-8, 1e-8, 0.5e-8, -1e-8, 0.0, 0.0] {
            let s = adaa.step::<false>(x0, character, lut);
            let tent = curve(0.25 * x0 + 0.5 * x1 + 0.25 * x2, character);
            assert!(
                (s - tent).abs() < 1e-14,
                "x={x0} c={character}: s={s} tent={tent}"
            );
            x2 = x1;
            x1 = x0;
        }
        let mut adaa = Adaa2::default();
        let mut line = LinearAdaa2::default();
        for x0 in [0.0, 1e-2, -2e-2, 3e-2, -1e-2, 0.0] {
            let s = adaa.step::<true>(x0, character, lut);
            let l = line.step(x0);
            assert!((s - l).abs() < 2e-5, "x={x0} c={character}: s={s} l={l}");
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
    for (render_quality, quality) in [
        (RenderQuality::Offline, DriveOversampling::TwoX),
        (RenderQuality::Live, DriveOversampling::TwoX),
        (RenderQuality::Live, DriveOversampling::Off),
    ] {
        for mix in [0.0, 100.0] {
            let settings = Drive {
                amount: 24.0,
                character: 100.0,
                mix,
                ..on()
            };
            let run = Run::with_quality(&settings, quality, render_quality);
            let lead = run.latency();
            let expected_lead = match render_quality {
                RenderQuality::Offline => LATENCY,
                RenderQuality::Live => 0,
            };
            assert_eq!(lead, expected_lead);
            let mut state = State::new(&run);
            let mut whole = input.clone();
            whole.extend(std::iter::repeat_n(0.0, lead * 2));
            run.process(&mut whole, &mut state);
            if lead > 0 && mix == 0.0 {
                assert!(whole[..lead * 2].iter().all(|x| *x == 0.0));
            }
            let mut blocked = input.clone();
            blocked.extend(std::iter::repeat_n(0.0, lead * 2));
            let mut state = State::new(&run);
            for block in blocked.chunks_mut(34) {
                run.process(block, &mut state);
            }
            assert_eq!(whole, blocked, "{render_quality:?} {quality:?} mix {mix}");
            if mix == 0.0 {
                assert_eq!(&whole[lead * 2..], input);
            } else {
                assert_ne!(
                    &whole[lead * 2..],
                    input,
                    "{render_quality:?} {quality:?} must saturate"
                );
                assert!(whole.iter().all(|x| x.is_finite()));
            }
            if render_quality == RenderQuality::Offline {
                let mut offline_default = input.clone();
                offline_default.extend(std::iter::repeat_n(0.0, LATENCY * 2));
                let default_run = Run::new(&settings);
                default_run.process(&mut offline_default, &mut State::new(&default_run));
                assert_eq!(whole, offline_default);
            }
        }
    }
}

#[test]
fn base_rate_curve_and_reference_share_one_sample_adaa2_delay() {
    let input = tone(5_000.0, 1e-2, 44_100);
    // Exact ADAA2 of f(x) = x is (x[n] + x[n-1] + x[n-2]) / 3, matching the
    // curve's 1-sample group delay above ADAA2_EPS so copy - clean cancels.
    let lut = &*F2_TANH_LUT;
    let mut adaa = Adaa2::default();
    let mut line = LinearAdaa2::default();
    let mut ref_output = Vec::new();
    for frame in 0..44_100 {
        let s0 = f64::from(input[frame * 2]);
        let s1 = if frame >= 1 {
            f64::from(input[(frame - 1) * 2])
        } else {
            0.0
        };
        let s2 = if frame >= 2 {
            f64::from(input[(frame - 2) * 2])
        } else {
            0.0
        };
        let expected = (s0 + s1 + s2) / 3.0;
        let shaped = adaa.step::<true>(s0, 0.0, lut);
        let reference = line.step(s0);
        assert!((shaped - expected).abs() < 1e-6);
        assert!((reference - expected).abs() < 1e-15);
        ref_output.extend([reference as f32, reference as f32]);
    }
    let magnitude = amplitude(&ref_output, 5_000.0) / amplitude(&input, 5_000.0);
    let expected_mag = (1.0 + 2.0 * (std::f64::consts::TAU * 5_000.0 / RATE).cos()) / 3.0;
    assert!((magnitude - expected_mag).abs() < 1e-4);
}

#[test]
fn playback_quality_and_offline_8x_aliasing_targets() {
    // 4048 Hz at -1 dBFS with +24 dB Drive, character 50% (odd + even harmonics).
    // Offline 8x FIR + ADAA2 must hold total non-harmonic aliasing < -90 dB
    // and worst spur < -85 dBc; Live 2x IIR + ADAA2 must beat 1x Off.
    let f0 = 4048.0;
    let peak = db_to_gain(-1.0);
    let settled_frames = 44_100;
    let total_frames = 2 * 44_100;
    let input = tone(f0, peak, total_frames);
    let settings = Drive {
        amount: 24.0,
        character: 50.0,
        mix: 100.0,
        focus: 20.0,
        ..on()
    };
    let measure = |render_quality: RenderQuality, drive_os: DriveOversampling| {
        let run = Run::with_quality(&settings, drive_os, render_quality);
        let lead = run.latency();
        let mut state = State::new(&run);
        let mut output = input.clone();
        output.extend(std::iter::repeat_n(0.0, lead * 2));
        run.process(&mut output, &mut state);
        let seg = &output[(44_100 + lead) * 2..(44_100 + lead + settled_frames) * 2];
        // Harmonics k*f0 aliased into [0, Fs/2], excluding harmonic 1 (f0) and DC/Nyquist.
        let fund = amplitude(seg, f0).max(1e-12);
        let mut worst_spur_db = f64::NEG_INFINITY;
        let mut alias_power = 0.0;
        let fund_power = fund * fund;
        for k in 2..=60_u32 {
            let fk = f64::from(k) * f0;
            // Folded frequency in [0, RATE/2]
            let rem = fk % RATE;
            let folded = if rem <= 0.5 * RATE { rem } else { RATE - rem };
            // Skip exact true harmonics below Nyquist (< 20 kHz for k <= 4)
            if fk < 0.5 * RATE {
                continue;
            }
            // Skip near-DC / near-fundamental coincidences
            if folded < 20.0 || (folded - f0).abs() < 2.0 {
                continue;
            }
            let amp = amplitude(seg, folded);
            let rel_db = db(amp / fund);
            worst_spur_db = worst_spur_db.max(rel_db);
            alias_power += amp * amp;
        }
        let total_alias_db = 10.0 * (alias_power / fund_power).max(1e-20).log10();
        (total_alias_db, worst_spur_db)
    };
    let (off8_tot, off8_spur) = measure(RenderQuality::Offline, DriveOversampling::TwoX);
    let (live2_tot, live2_spur) = measure(RenderQuality::Live, DriveOversampling::TwoX);
    let (live1_tot, live1_spur) = measure(RenderQuality::Live, DriveOversampling::Off);
    assert!(
        off8_tot < -90.0 && off8_spur < -85.0,
        "Offline 8x ADAA2 aliasing too high: total {off8_tot:.2} dB, spur {off8_spur:.2} dBc"
    );
    assert!(
        live2_spur < live1_spur - 15.0 && live2_tot < live1_tot - 15.0,
        "Live 2x IIR ({live2_tot:.1}/{live2_spur:.1}) must beat 1x Off ({live1_tot:.1}/{live1_spur:.1})"
    );
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

#[test]
fn transfer_preview_keeps_the_straight_half_and_full_output_trim() {
    let settings = Drive {
        amount: 12.0,
        character: 100.0,
        mix: 100.0,
        output: 12.0,
        ..on()
    };
    let trace = sample_drive_curve(&settings, 3);
    let gain = db_to_gain(12.0);
    assert!((f64::from(trace[0]) + gain).abs() < 1e-6);
    assert_eq!(trace[1], 0.0);
    assert!((f64::from(trace[2]) - gain.tanh()).abs() < 1e-6);
}

#[test]
fn every_quality_keeps_the_playback_alias_threshold() {
    // The playback threshold that the earlier 4x default met: a 5 kHz tone
    // at 24 dB Drive folds its fifth harmonic from 25 to 19.1 kHz. The 2x
    // playback default and export must stay below -65 dBc, and Off must
    // stay at least 10 dB above 2x. Do not weaken these thresholds.
    let input = tone(5000.0, 0.9, 2 * 44_100);
    let settings = Drive {
        amount: 24.0,
        character: 0.0,
        mix: 100.0,
        focus: 20.0,
        ..on()
    };
    let alias = |render_quality, quality| {
        let run = Run::with_quality(&settings, quality, render_quality);
        let lead = run.latency();
        let mut output = input.clone();
        output.extend(std::iter::repeat_n(0.0, lead * 2));
        run.process(&mut output, &mut State::new(&run));
        let output = &output[(44_100 + lead) * 2..];
        let alias = db(amplitude(output, 19_100.0) / amplitude(output, 5000.0));
        println!("Drive {render_quality:?} {quality:?}: 19.1 kHz alias {alias:.2} dBc");
        alias
    };
    let export = alias(RenderQuality::Offline, DriveOversampling::TwoX);
    let two = alias(RenderQuality::Live, DriveOversampling::TwoX);
    let off = alias(RenderQuality::Live, DriveOversampling::Off);
    assert!(export < -65.0, "export alias {export:.2} dBc");
    assert!(two < -65.0, "2x alias {two:.2} dBc");
    assert!(off > two + 10.0, "Off {off:.2} dBc against 2x {two:.2} dBc");
}

#[test]
fn every_quality_keeps_long_silent_tails_independent_of_block_size() {
    // The filter histories decay through the denormal guard during a long
    // silence. Where a block ends must not change a single output bit.
    let mut input = tone(3_000.0, 0.9, 2_048);
    input.extend(std::iter::repeat_n(0.0, 2 * 60_000));
    for (render_quality, quality) in [
        (RenderQuality::Offline, DriveOversampling::TwoX),
        (RenderQuality::Live, DriveOversampling::TwoX),
        (RenderQuality::Live, DriveOversampling::Off),
    ] {
        let settings = Drive {
            amount: 24.0,
            character: 50.0,
            mix: 100.0,
            focus: 200.0,
            ..on()
        };
        let run = Run::with_quality(&settings, quality, render_quality);
        let mut whole = input.clone();
        run.process(&mut whole, &mut State::new(&run));
        for frames in [7, 34, 128, 256] {
            let mut blocked = input.clone();
            let mut state = State::new(&run);
            for block in blocked.chunks_mut(frames * 2) {
                run.process(block, &mut state);
            }
            let first = whole
                .iter()
                .zip(&blocked)
                .position(|(a, b)| a.to_bits() != b.to_bits());
            assert_eq!(
                first, None,
                "{render_quality:?} {quality:?} in {frames}-frame blocks"
            );
        }
    }
}

#[test]
fn gated_power_matching_leaves_quiet_tails_unpumped() {
    let mut input = tone(55.0, db_to_gain(-6.0), 44_100);
    input.extend(tone(220.0, db_to_gain(-65.0), 2 * 44_100));
    let output = drive(
        &Drive {
            amount: 24.0,
            character: 100.0,
            mix: 100.0,
            focus: 20.0,
            output: 0.0,
            ..on()
        },
        &input,
    );
    let before_tail = power_db(&input[2 * 44_100 * 2..]);
    let after_tail = power_db(&output[2 * 44_100 * 2..]);
    assert!(
        (after_tail - before_tail).abs() < 0.2,
        "quiet tail moved by {:.2} dB",
        after_tail - before_tail
    );
}

#[test]
fn offline_impulse_has_zero_sample_lag_after_lead_compensation() {
    let mut input = vec![0.0_f32; 512 * 2];
    input[200 * 2] = 0.8;
    input[200 * 2 + 1] = 0.8;
    let out = drive(
        &Drive {
            amount: 12.0,
            character: 25.0,
            mix: 50.0,
            focus: 100.0,
            ..on()
        },
        &input,
    );
    let (peak_idx, _) = out
        .iter()
        .step_by(2)
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .unwrap();
    assert_eq!(peak_idx, 200, "impulse peak shifted to {peak_idx}");
}
