use super::*;

fn delay(settings: &Echoform, bpm: f64) -> Core {
    let mut core = Core::new();
    core.set(settings, bpm, 0.0);
    core.settle();
    core
}

fn arrivals(core: &mut Core, frames: usize, side: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut above = false;
    for i in 0..frames {
        let x = if i == 0 { 1.0 } else { 0.0 };
        let (l, r) = core.process(x, x);
        let value = [l, r][side].abs();
        if value > 0.05 && !above {
            out.push(i);
        }
        above = value > 0.05;
    }
    out
}

fn plain() -> Echoform {
    Echoform {
        character: DelayCharacter::Digital,
        ping_pong: false,
        width: 100.0,
        low_cut: 20.0,
        high_cut: 20_000.0,
        mod_depth: 0.0,
        duck_amount: 0.0,
        output_db: 0.0,
        feedback: 50.0,
        ..Echoform::default()
    }
}

#[test]
fn synced_repeats_land_on_their_note_lengths_at_each_tempo() {
    for bpm in [120.0, 125.0, 140.0, 180.0] {
        let settings = Echoform {
            division_l: super::super::Division::Quarter,
            division_r: super::super::Division::EighthDotted,
            ..plain()
        };
        let mut core = delay(&settings, bpm);
        let left = arrivals(&mut core, 200_000, 0);
        assert!(
            left.len() >= 3,
            "{bpm}: fewer than three left repeats: {left:?}"
        );
        let quarter = 60.0 / bpm * RATE;
        for (k, &frame) in left.iter().take(3).enumerate() {
            let expected = quarter * (k + 1) as f64;
            assert!(
                (frame as f64 - expected).abs() <= 2.0,
                "{bpm}: {frame} vs {expected}"
            );
        }
        let mut core = delay(&settings, bpm);
        let right = arrivals(&mut core, 100_000, 1);
        assert!((right[0] as f64 - 0.75 * quarter).abs() <= 2.0);
    }
}

#[test]
fn free_times_and_the_compensation_set_each_side() {
    let settings = Echoform {
        mode: super::super::TimeMode::Free,
        time_ms_l: 100.0,
        time_ms_r: 250.0,
        ..plain()
    };
    let mut core = Core::new();
    core.set(&settings, 120.0, 220.0);
    core.settle();
    assert_eq!(arrivals(&mut core, 20_000, 0)[0], 4410 - 220);
    let mut core = Core::new();
    core.set(&settings, 120.0, 220.0);
    core.settle();
    assert_eq!(arrivals(&mut core, 20_000, 1)[0], 11_025 - 220);
}

#[test]
fn ping_pong_alternates_sides_and_stereo_keeps_them() {
    let settings = Echoform {
        mode: super::super::TimeMode::Free,
        time_ms_l: 100.0,
        time_ms_r: 100.0,
        ping_pong: true,
        ..plain()
    };
    let mut core = delay(&settings, 120.0);
    let mut energy = [[0.0; 2]; 4];
    for i in 0..4 * 4410 + 2000 {
        let (l, r) = core.process(if i == 0 { 1.0 } else { 0.0 }, 0.0);
        let slot = i / 4410;
        if (1..=4).contains(&slot) && i % 4410 < 50 {
            energy[slot - 1][0] += l * l;
            energy[slot - 1][1] += r * r;
        }
    }
    assert!(energy[0][0] > 100.0 * energy[0][1]);
    assert!(energy[1][1] > 100.0 * energy[1][0]);
    assert!(energy[2][0] > 100.0 * energy[2][1]);
}

#[test]
fn feedback_decays_below_unity_and_stays_bounded_above_it() {
    let peak_after = |feedback: f32, character| {
        let settings = Echoform {
            mode: super::super::TimeMode::Free,
            time_ms_l: 50.0,
            time_ms_r: 50.0,
            feedback,
            character,
            ..plain()
        };
        let mut core = delay(&settings, 120.0);
        let mut late: f64 = 0.0;
        for i in 0..20 * 44_100 {
            let x = if i < 2205 {
                0.8 * (i as f64 * 0.07).sin()
            } else {
                0.0
            };
            let (l, r) = core.process(x, x);
            assert!(l.is_finite() && r.is_finite());
            if i > 18 * 44_100 {
                late = late.max(l.abs()).max(r.abs());
            }
        }
        late
    };
    assert!(peak_after(60.0, DelayCharacter::Digital) < 1e-6);
    for character in [
        DelayCharacter::Digital,
        DelayCharacter::Analog,
        DelayCharacter::Tape,
    ] {
        let held = peak_after(110.0, character);
        assert!(held < 1.6, "{character:?}: {held}");
    }
}

#[test]
fn the_loop_filters_darken_each_repeat() {
    let settings = Echoform {
        mode: super::super::TimeMode::Free,
        time_ms_l: 100.0,
        time_ms_r: 100.0,
        feedback: 90.0,
        high_cut: 2000.0,
        ..plain()
    };
    let mut core = delay(&settings, 120.0);
    let mut amplitude = [(0.0, 0.0); 6];
    for i in 0..7 * 4410 {
        let x = if i < 441 {
            (2.0 * PI * 6000.0 * i as f64 / RATE).sin()
        } else {
            0.0
        };
        let (l, _) = core.process(x, x);
        let slot = i / 4410;
        if (1..=6).contains(&slot) {
            let angle = 2.0 * PI * 6000.0 * i as f64 / RATE;
            amplitude[slot - 1].0 += l * angle.cos();
            amplitude[slot - 1].1 += l * angle.sin();
        }
    }
    let levels: Vec<f64> = amplitude.iter().map(|(re, im)| re.hypot(*im)).collect();
    for pair in levels.windows(2) {
        assert!(pair[1] < pair[0] * 0.9 * 0.5, "{levels:?}");
    }
}

#[test]
fn smear_spreads_feedback_repeats_without_moving_the_first_echo() {
    let time = (0.1 * RATE) as usize;
    let render = |smear_percent| {
        let mut core = delay(
            &Echoform {
                mode: super::super::TimeMode::Free,
                time_ms_l: 100.0,
                time_ms_r: 100.0,
                smear_percent,
                ..plain()
            },
            120.0,
        );
        (0..3 * time)
            .map(|frame| core.process(f64::from(u8::from(frame == 0)), 0.0).0)
            .collect::<Vec<_>>()
    };
    let clean = render(0.0);
    let smeared = render(60.0);
    assert_eq!(&clean[..2 * time], &smeared[..2 * time]);
    let spread = |samples: &[f64]| {
        samples[2 * time + 100..2 * time + 1764]
            .iter()
            .map(|sample| sample * sample)
            .sum::<f64>()
    };
    assert!(spread(&smeared) > 5.0 * spread(&clean));
}

#[test]
fn resonance_boosts_the_cutoff_more_than_the_passband() {
    let response = |filter_resonance, hz: f64| {
        let mut core = delay(
            &Echoform {
                mode: super::super::TimeMode::Free,
                time_ms_l: 20.0,
                time_ms_r: 20.0,
                feedback: 0.0,
                high_cut: 2000.0,
                filter_resonance,
                ..plain()
            },
            120.0,
        );
        (0..8820)
            .map(|frame| {
                let input = 0.1 * (2.0 * PI * hz * frame as f64 / RATE).sin();
                core.process(input, input).0.powi(2)
            })
            .skip(4410)
            .sum::<f64>()
    };
    let boost = |hz| response(80.0, hz) / response(0.0, hz);
    assert!(boost(2000.0) > 4.0 * boost(500.0));
}

#[test]
fn width_and_bypass_shape_only_the_output() {
    let settings = Echoform {
        mode: super::super::TimeMode::Free,
        time_ms_l: 30.0,
        time_ms_r: 60.0,
        width: 0.0,
        ..plain()
    };
    let mut core = delay(&settings, 120.0);
    for i in 0..10_000 {
        let (l, r) = core.process(if i == 0 { 1.0 } else { 0.0 }, 0.0);
        assert!((l - r).abs() < 1e-12);
    }
    let mut core = delay(&plain(), 120.0);
    core.process(1.0, 1.0);
    let mut bypassed = plain();
    bypassed.bypass = true;
    core.set(&bypassed, 120.0, 0.0);
    core.settle();
    for _ in 0..30_000 {
        assert_eq!(core.process(0.0, 0.0), (0.0, 0.0));
    }
    core.set(&plain(), 120.0, 0.0);
    core.settle();
    let mut heard: f64 = 0.0;
    for _ in 0..60_000 {
        heard = heard.max(core.process(0.0, 0.0).0.abs());
    }
    assert!(heard > 0.01, "the tail rang on behind the bypass");
}

#[test]
fn the_soften_corner_glides_with_a_character_change() {
    let tape = Echoform {
        character: DelayCharacter::Tape,
        high_cut: 8000.0,
        ..plain()
    };
    let mut core = delay(&tape, 120.0);
    core.process(0.0, 0.0);
    core.set(
        &Echoform {
            character: DelayCharacter::Analog,
            ..tape
        },
        120.0,
        0.0,
    );
    core.process(0.0, 0.0);
    let corner = 8000.0 * (1.0 - 0.25 * core.analog.value() - 0.45 * core.tape.value());
    let mut expected = OnePole::default();
    expected.tune(corner);
    for side in 0..2 {
        let mut soften = core.soften[side];
        soften.clear();
        assert_eq!(soften.low(1.0), expected.low(1.0), "side {side}");
        expected.clear();
    }
}

#[test]
fn tape_delay_without_modulation_keeps_a_tone_below_its_input_peak() {
    let settings = Echoform {
        mode: super::super::TimeMode::Free,
        time_ms_l: 200.0,
        time_ms_r: 200.0,
        feedback: 0.0,
        mod_depth: 0.0,
        character: DelayCharacter::Tape,
        ..plain()
    };
    let mut core = delay(&settings, 120.0);
    let mut levels = Vec::new();
    for i in 0..60_000 {
        let x = (2.0 * PI * 440.0 * i as f64 / RATE).sin() * 0.5;
        let (l, _) = core.process(x, x);
        if i > 20_000 {
            levels.push(l);
        }
    }
    let peak = levels.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
    assert!(peak > 0.0 && peak < 0.5 + 1e-3, "{peak}");
}

#[test]
fn engaging_live_drive_does_not_feed_a_click_into_the_delay() {
    let mut settings = Echoform {
        mode: super::super::TimeMode::Free,
        time_ms_l: 1.0,
        time_ms_r: 1.0,
        feedback: 0.0,
        ..plain()
    };
    let mut changing = delay(&settings, 120.0);
    let mut unchanged = delay(&settings, 120.0);
    for _ in 0..4096 {
        changing.process(0.3, 0.3);
        unchanged.process(0.3, 0.3);
    }
    settings.drive = 100.0;
    changing.set(&settings, 120.0, 0.0);
    let mut peak = 0.0_f64;
    for _ in 0..64 {
        let changed_sample = changing.process(0.3, 0.3).0;
        let baseline = unchanged.process(0.3, 0.3).0;
        peak = peak.max((changed_sample - baseline).abs());
    }
    assert!(peak < 0.02, "peak transient after engaging Drive: {peak}");
}

#[test]
fn disengaging_live_analog_drive_does_not_feed_a_click_into_the_delay() {
    let mut settings = Echoform {
        mode: super::super::TimeMode::Free,
        time_ms_l: 1.0,
        time_ms_r: 1.0,
        character: DelayCharacter::Analog,
        drive: 100.0,
        feedback: 0.0,
        ..plain()
    };
    let mut core = delay(&settings, 120.0);
    for _ in 0..8192 {
        core.process(0.3, 0.3);
    }
    settings.drive = 0.0;
    core.set(&settings, 120.0, 0.0);
    let mut peak = 0.0_f64;
    let mut previous = core.process(0.3, 0.3).0;
    for _ in 0..8192 {
        let sample = core.process(0.3, 0.3).0;
        peak = peak.max((sample - previous).abs());
        previous = sample;
    }
    assert!(
        peak < 0.02,
        "peak step after disengaging Analog Drive: {peak}"
    );
}

#[test]
fn offline_delay_drive_suppresses_aliasing_and_separates_characters() {
    let render = |character| -> (f64, f64, f64) {
        let settings = Echoform {
            mode: super::super::TimeMode::Free,
            time_ms_l: 20.0,
            time_ms_r: 20.0,
            feedback: 0.0,
            drive: 100.0,
            character,
            ..plain()
        };
        let mut core = delay(&settings, 120.0);
        let n = 16_384;
        let skip = 20_000;
        let mut buf = Vec::with_capacity(n);
        for i in 0..skip + n {
            let x = std::f64::consts::FRAC_1_SQRT_2 * (2.0 * PI * 4048.0 * i as f64 / RATE).sin();
            let (l, _) = core.process_with_quality(x, x, RenderQuality::Offline);
            if i >= skip {
                buf.push(l);
            }
        }
        // Blackman-Harris DFT measurement of fundamental, H2, and non-harmonic alias peak.
        let mut f1 = 0.0_f64;
        let mut h2 = 0.0_f64;
        let mut worst_alias = 0.0_f64;
        let mut worst_hz = 0.0_f64;
        for bin in 20..(n / 2) {
            let hz = bin as f64 * RATE / n as f64;
            let (mut re, mut im) = (0.0, 0.0);
            for (i, &v) in buf.iter().enumerate() {
                let p = 2.0 * PI * bin as f64 * i as f64 / n as f64;
                let w = 0.35875 - 0.48829 * (2.0 * PI * i as f64 / (n - 1) as f64).cos()
                    + 0.14128 * (4.0 * PI * i as f64 / (n - 1) as f64).cos()
                    - 0.01168 * (6.0 * PI * i as f64 / (n - 1) as f64).cos();
                re += v * w * p.cos();
                im += v * w * p.sin();
            }
            let mag = re.hypot(im);
            let is_harm = (1..=5).any(|k| (hz - f64::from(k) * 4048.0).abs() < 45.0);
            if (hz - 4048.0).abs() < 10.0 {
                f1 = f1.max(mag);
            } else if (hz - 8096.0).abs() < 10.0 {
                h2 = h2.max(mag);
            } else if !is_harm && mag > worst_alias {
                worst_alias = mag;
                worst_hz = hz;
            }
        }
        let spur_dbc = 20.0 * ((worst_alias + 1e-15) / (f1 + 1e-15)).log10();
        let h2_dbc = 20.0 * ((h2 + 1e-15) / (f1 + 1e-15)).log10();
        (spur_dbc, h2_dbc, worst_hz)
    };
    let (dig_spur, dig_h2, dig_hz) = render(DelayCharacter::Digital);
    let (ana_spur, ana_h2, ana_hz) = render(DelayCharacter::Analog);
    let (tape_spur, tape_h2, tape_hz) = render(DelayCharacter::Tape);
    assert!(
        dig_spur < -85.0,
        "Digital spur {dig_spur:.2} dBc at {dig_hz:.1} Hz"
    );
    assert!(
        ana_spur < -85.0,
        "Analog spur {ana_spur:.2} dBc at {ana_hz:.1} Hz"
    );
    assert!(
        tape_spur < -85.0,
        "Tape spur {tape_spur:.2} dBc at {tape_hz:.1} Hz"
    );
    assert!(
        tape_h2 > dig_h2 + 30.0 && ana_h2 > dig_h2 + 15.0,
        "Characters must diverge cleanly: dig={dig_h2:.1}, ana={ana_h2:.1}, tape={tape_h2:.1}"
    );
}

#[test]
fn offline_sub_5ms_tap_is_sample_accurate() {
    use super::super::{FxBuses, Module, ReturnsProcessor};
    for limiter in [false, true] {
        let mut buses = FxBuses::default();
        buses.buses[0].limiter = limiter;
        buses.buses[0].return_level = 1.0;
        buses.buses[0].module = Module::EchoformDelay(Echoform {
            mode: super::super::TimeMode::Free,
            time_ms_l: 2.5,
            time_ms_r: 2.5,
            feedback: 0.0,
            ..plain()
        });
        let mut proc = ReturnsProcessor::new(&buses, 120.0);
        let lat = proc.latency_samples();
        let frames = 2048;
        let mut block = vec![0.0_f32; frames * 2];
        let mut sends = vec![0.0_f32; frames * 2 * super::super::BUSES];
        block[100 * 2] = 0.5;
        block[100 * 2 + 1] = 0.5;
        sends[100 * 2 * super::super::BUSES] = 0.5;
        sends[100 * 2 * super::super::BUSES + 1] = 0.5;
        proc.process(&mut block, Some(&sends));
        let (dry_pk, _) = block
            .chunks(2)
            .enumerate()
            .filter(|(i, _)| *i < 100 + lat + 50)
            .max_by(|a, b| a.1[0].abs().total_cmp(&b.1[0].abs()))
            .unwrap();
        // Wet arrival sits past dry peak by ~110.25 samples.
        let search_start = dry_pk + 60;
        let (wet_pk, _) = block[search_start * 2..]
            .chunks(2)
            .enumerate()
            .max_by(|a, b| a.1[0].abs().total_cmp(&b.1[0].abs()))
            .unwrap();
        let actual = search_start + wet_pk - dry_pk;
        let err = (actual as f64 - 0.0025 * RATE).abs();
        assert!(
            err <= 1.0,
            "limiter={limiter}: actual={actual} err={err:.2}"
        );
    }
}

#[test]
fn tape_character_has_native_decorrelated_wow_flutter_at_zero_mod_depth() {
    let run = |character| {
        let mut core = delay(
            &Echoform {
                mode: super::super::TimeMode::Free,
                time_ms_l: 40.0,
                time_ms_r: 40.0,
                feedback: 0.0,
                mod_depth: 0.0,
                character,
                ..plain()
            },
            120.0,
        );
        let mut diff_sq = 0.0;
        let mut sum_sq = 0.0;
        for i in 0..44_100 {
            let x = 0.5 * (2.0 * PI * 4048.0 * i as f64 / RATE).sin();
            let (l, r) = core.process_with_quality(x, x, RenderQuality::Offline);
            if i >= 15_000 {
                diff_sq += (l - r).powi(2);
                sum_sq += (l + r).powi(2);
            }
        }
        20.0 * ((diff_sq + 1e-24) / (sum_sq + 1e-24)).sqrt().log10()
    };
    let tape_diff_db = run(DelayCharacter::Tape);
    let dig_diff_db = run(DelayCharacter::Digital);
    assert!(tape_diff_db > -25.0, "Tape L-R diff {tape_diff_db:.2} dB");
    assert!(dig_diff_db < -120.0, "Digital L-R diff {dig_diff_db:.2} dB");
}

#[test]
fn resonant_svf_stays_bounded_under_extreme_feedback_and_resonance() {
    let mut core = delay(
        &Echoform {
            mode: super::super::TimeMode::Free,
            time_ms_l: 30.0,
            time_ms_r: 30.0,
            feedback: 110.0,
            filter_resonance: 100.0,
            high_cut: 3500.0,
            ..plain()
        },
        120.0,
    );
    let mut late: f64 = 0.0;
    for i in 0..88_200 {
        let x = if i < 1000 {
            0.8 * (i as f64 * 0.13).sin()
        } else {
            0.0
        };
        let (l, r) = core.process(x, x);
        assert!(l.is_finite() && r.is_finite());
        if i > 66_000 {
            late = late.max(l.abs()).max(r.abs());
        }
    }
    assert!(late < 1.6, "resonant feedback unbounded: {late}");
}

#[test]
fn sample_delay_taps_reflects_ping_pong_link_and_feedback() {
    let mut out = [(0.0_f32, 0.0_f32, 0.0_f32); 16];
    let ping = Echoform {
        mode: super::super::TimeMode::Free,
        time_ms_l: 200.0,
        time_ms_r: 300.0,
        feedback: 50.0,
        ping_pong: true,
        width: 100.0,
        ..Echoform::default()
    };
    let n = sample_delay_taps(&ping, 120.0, &mut out);
    assert!(n >= 4);
    assert!(out[0].1 > 0.9 && out[0].2.abs() < 1e-6);
    assert!(out[1].1.abs() < 1e-6 && out[1].2 > 0.45);
    assert!(out[0].0 < out[1].0 && out[1].0 < out[2].0);

    let linked = Echoform {
        link_times: true,
        ping_pong: false,
        ..ping
    };
    let mut linked_out = [(0.0_f32, 0.0_f32, 0.0_f32); 16];
    let nl = sample_delay_taps(&linked, 120.0, &mut linked_out);
    assert!(nl >= 4);
    assert!((linked_out[0].1 - linked_out[0].2).abs() < 1e-6);
}

#[test]
fn stereo_scope_groups_coincident_echoes_and_uses_both_delay_times() {
    let settings = Echoform {
        mode: super::super::TimeMode::Free,
        time_ms_l: 300.0,
        time_ms_r: 100.0,
        ping_pong: false,
        width: 100.0,
        feedback: 50.0,
        ..plain()
    };
    let mut taps = [(0.0, 0.0, 0.0); 16];
    let count = sample_delay_taps(&settings, 120.0, &mut taps);
    assert_eq!(count, 9);
    assert_eq!(
        taps[..4],
        [
            (1.0 / 12.0, 0.0, 1.0),
            (2.0 / 12.0, 0.0, 0.5),
            (3.0 / 12.0, 1.0, 0.25),
            (4.0 / 12.0, 0.0, 0.125),
        ]
    );

    let mono = Echoform {
        width: 0.0,
        ..settings
    };
    sample_delay_taps(&mono, 120.0, &mut taps);
    assert_eq!(taps[0], (1.0 / 12.0, 0.5, 0.5));
    assert_eq!(taps[2], (3.0 / 12.0, 0.625, 0.625));
}

#[test]
fn delay_scope_shows_four_repeats_at_the_longest_note_length() {
    let mut taps = [(0.0, 0.0, 0.0); 16];
    for ping_pong in [false, true] {
        let settings = Echoform {
            division_l: super::super::Division::WholeDotted,
            division_r: super::super::Division::WholeDotted,
            ping_pong,
            ..plain()
        };
        let count = sample_delay_taps(&settings, 120.0, &mut taps);
        assert_eq!(count, 4, "ping-pong={ping_pong}");
        assert_eq!(taps[0].0, 0.25);
        assert_eq!(taps[3].0, 1.0);
    }
}

#[test]
fn digital_time_change_crossfades_smoothly_in_offline_and_live() {
    for quality in [RenderQuality::Live, RenderQuality::Offline] {
        let mut settings = Echoform {
            mode: super::super::TimeMode::Free,
            time_ms_l: 10.0,
            time_ms_r: 10.0,
            feedback: 0.0,
            character: DelayCharacter::Digital,
            ..plain()
        };
        let mut core = delay(&settings, 120.0);
        for _ in 0..1024 {
            core.process_with_quality(0.3, 0.3, quality);
        }
        settings.time_ms_l = 45.0;
        core.set(&settings, 120.0, 220.0);
        let mut max_step = 0.0_f64;
        let mut prev = 0.0_f64;
        for i in 0..2048 {
            let (l, _) = core.process_with_quality(0.3, 0.3, quality);
            if i > 0 {
                max_step = max_step.max((l - prev).abs());
            }
            prev = l;
        }
        assert!(max_step < 0.35, "{quality:?} jump step {max_step}");
    }
}
