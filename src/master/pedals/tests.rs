use super::dynamics::CROSSOVERS;
use super::{
    BusCompressor, Imaging, LiftEq, Limiter, Maximizer, Multiband, SoftClip, Tape, TrimEq, Tube,
};
use crate::master as settings;
use crate::master::dsp::{
    Biquad, Coefficients, Oversampler, RATE, TruePeak, db_to_gain, gain_to_db, program,
};
use std::f64::consts::{FRAC_1_SQRT_2, PI};

fn run(frames: &[(f32, f32)], mut pedal: impl FnMut(f32, f32) -> (f32, f32)) -> Vec<(f32, f32)> {
    frames.iter().map(|(l, r)| pedal(*l, *r)).collect()
}

#[test]
fn multiband_key_filters_do_not_resume_old_audio() {
    let mut pedal = Multiband::new();
    pedal.set(&settings::Multiband {
        on: true,
        low: 100.0,
        mid: 100.0,
        high: 100.0,
        key: 2,
    });
    pedal.clear();
    for _ in 0..4000 {
        pedal.process_keyed(0.001, 0.001, Some((2.0, 2.0)));
    }
    for _ in 0..50_000 {
        pedal.process(0.001, 0.001);
    }
    let mut own = pedal;
    let mut difference = 0.0_f32;
    for _ in 0..2000 {
        let output = pedal.process_keyed(0.001, 0.001, Some((0.0, 0.0)));
        let expected = own.process(0.001, 0.001);
        difference = difference.max((output.0 - expected.0).abs());
    }
    assert!(
        difference < 1e-7,
        "The old key changed the output by {difference}"
    );
}

#[test]
fn limiter_key_detector_does_not_resume_old_audio() {
    let mut pedal = Limiter::new();
    pedal.set(&settings::Limiter {
        on: true,
        gain: 0.0,
        ceiling: -1.0,
        key: 2,
        ..settings::Limiter::default()
    });
    pedal.clear();
    for _ in 0..4000 {
        pedal.process_keyed(0.001, 0.001, Some((2.0, 2.0)));
    }
    for _ in 0..50_000 {
        pedal.process(0.001, 0.001);
    }
    let mut own = pedal;
    let mut difference = 0.0_f32;
    for _ in 0..2000 {
        let output = pedal.process_keyed(0.001, 0.001, Some((0.0, 0.0)));
        let expected = own.process(0.001, 0.001);
        difference = difference.max((output.0 - expected.0).abs());
    }
    assert!(
        difference < 1e-7,
        "The old key changed the output by {difference}"
    );
}

fn rms_db(frames: &[(f32, f32)]) -> f64 {
    let power = frames
        .iter()
        .map(|(l, r)| (f64::from(*l).powi(2) + f64::from(*r).powi(2)) / 2.0)
        .sum::<f64>()
        / frames.len() as f64;
    10.0 * power.log10()
}

/// The level of the difference between an output and the input it
/// should equal `delay` frames earlier, relative to the input.
fn residue_db(output: &[(f32, f32)], input: &[(f32, f32)], delay: usize) -> f64 {
    let skip = 4096;
    let difference: Vec<(f32, f32)> = output[skip..]
        .iter()
        .zip(&input[skip - delay..])
        .map(|((a, b), (c, d))| (a - c, b - d))
        .collect();
    rms_db(&difference) - rms_db(&input[skip - delay..input.len() - delay])
}

/// The input through the oversampling alone, as a pedal with a straight
/// curve gives it.
fn oversampled(input: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut oversamplers = [Oversampler::ZERO; 2];
    run(input, |l, r| {
        (
            oversamplers[0].process(l, |x| x),
            oversamplers[1].process(r, |x| x),
        )
    })
}

#[test]
fn neutral_settings_pass_the_mix_through() {
    let input = program(20_000);
    let quiet: Vec<(f32, f32)> = input.iter().map(|(l, r)| (l * 0.5, r * 0.5)).collect();
    let through = oversampled(&quiet);

    let mut tube = Tube::new();
    tube.set(&settings::Tube {
        on: true,
        drive: 6.0,
        mix: 0.0,
    });
    tube.clear();
    let dry = run(&input, |l, r| tube.process(l, r));
    assert_eq!(&dry[Tube::LATENCY..], &input[..input.len() - Tube::LATENCY]);

    // At unity ceiling, Soft Clip leaves this quiet signal unchanged.
    let mut soft_clip = SoftClip::new();
    soft_clip.set(&settings::SoftClip {
        on: true,
        amount: 0.0,
        ceiling: 0.0,
    });
    soft_clip.clear();
    assert_eq!(run(&quiet, |l, r| soft_clip.process(l, r)), through);
    let mut maximizer = Maximizer::new();
    maximizer.set(&settings::Maximizer {
        on: true,
        boost: 0.0,
    });
    maximizer.clear();
    // The Maximizer leaves the same program at unity gain below the knee.
    let maximized = run(&quiet, |l, r| maximizer.process(l, r));
    assert!((rms_db(&maximized) - rms_db(&through)).abs() < 0.05);

    let mut tape = Tape::new();
    tape.set(&settings::Tape {
        on: true,
        drive: 0.0,
        speed: 15,
    });
    tape.clear();
    assert!(residue_db(&run(&quiet, |l, r| tape.process(l, r)), &through, 0) < -120.0);

    let mut lift = LiftEq::new();
    lift.set(&settings::LiftEq {
        on: true,
        low: 0.0,
        air: 0.0,
    });
    lift.clear();
    assert_eq!(run(&input, |l, r| lift.process(l, r)), input);

    let mut compressor = BusCompressor::new();
    compressor.set(&settings::BusCompressor {
        on: true,
        threshold: 0.0,
        ratio: 1.5,
        attack: 10.0,
        release: 300.0,
        key: 0,
        ..settings::BusCompressor::default()
    });
    compressor.clear();
    assert_eq!(run(&quiet, |l, r| compressor.process(l, r)), quiet);
}

#[test]
fn multiband_at_zero_is_the_crossover_all_pass() {
    let input = program(12_000);
    let mut multiband = Multiband::new();
    multiband.set(&settings::Multiband {
        on: true,
        low: 0.0,
        mid: 0.0,
        high: 0.0,
        key: 0,
    });
    multiband.clear();
    let q = FRAC_1_SQRT_2;
    let [mut first, mut second] = CROSSOVERS.map(|hz| Biquad::new(Coefficients::all_pass(hz, q)));
    let expected: Vec<(f32, f32)> = run(&input, |l, r| {
        let mut pass =
            |channel, x| second.process(channel, first.process(channel, f64::from(x))) as f32;
        (pass(0, l), pass(1, r))
    });
    let output = run(&input, |l, r| multiband.process(l, r));
    for ((a, b), (x, y)) in output[Multiband::LATENCY..].iter().zip(&expected) {
        assert!((a - x).abs() < 1e-6 && (b - y).abs() < 1e-6);
    }
}

#[test]
fn multiband_lifts_the_body_and_keeps_the_peaks() {
    let input = program(4 * 44_100);
    let peak = |frames: &[(f32, f32)]| {
        frames
            .iter()
            .fold(0.0f32, |most, (l, r)| most.max(l.abs()).max(r.abs()))
    };
    let mut multiband = Multiband::new();
    multiband.set(&settings::Multiband {
        on: true,
        low: 100.0,
        mid: 100.0,
        high: 100.0,
        key: 0,
    });
    multiband.clear();
    let output = run(&input, |l, r| multiband.process(l, r));
    let output = &output[Multiband::LATENCY..];
    assert!(peak(output) <= peak(&input) * 1.0001);
    assert!(rms_db(&output[44_100..]) > rms_db(&input[44_100..]) + 2.0);
}

#[test]
fn multiband_lifts_a_breakdown_as_much_as_a_drop() {
    // The same program 8 dB apart: each section's ceilings follow its
    // own peaks, so both come out lifted alike.
    let quiet = db_to_gain(-8.0) as f32;
    let mut input = program(7 * 44_100);
    for (l, r) in &mut input[3 * 44_100..] {
        (*l, *r) = (*l * quiet, *r * quiet);
    }
    let mut multiband = Multiband::new();
    multiband.set(&settings::Multiband {
        on: true,
        low: 60.0,
        mid: 60.0,
        high: 60.0,
        key: 0,
    });
    multiband.clear();
    let output = run(&input, |l, r| multiband.process(l, r));
    let output = &output[Multiband::LATENCY..];
    let lift =
        |range: std::ops::Range<usize>| rms_db(&output[range.clone()]) - rms_db(&input[range]);
    let (loud, soft) = (
        lift(44_100..3 * 44_100),
        lift(5 * 44_100..6 * 44_100 + 22_050),
    );
    assert!(loud > 1.0, "the drop is lifted {loud:.2} dB");
    assert!(
        (loud - soft).abs() < 0.3,
        "drop {loud:.2} dB, breakdown {soft:.2} dB"
    );
}

#[test]
fn stereo_imaging_never_changes_mono() {
    let input = program(8000);
    let mono: Vec<(f32, f32)> = input.iter().map(|(l, _)| (*l, *l)).collect();
    for (width, mono_below) in [(60.0, 300.0), (140.0, 60.0), (105.0, 120.0)] {
        let mut imaging = Imaging::new();
        imaging.set(&settings::Imaging {
            on: true,
            width,
            mono_below,
        });
        imaging.clear();
        assert_eq!(run(&mono, |l, r| imaging.process(l, r)), mono);
        let mut imaging = Imaging::new();
        imaging.set(&settings::Imaging {
            on: true,
            width,
            mono_below,
        });
        imaging.clear();
        for ((l, r), (a, b)) in input.iter().zip(run(&input, |l, r| imaging.process(l, r))) {
            assert!(
                ((l + r) - (a + b)).abs() < 1e-6,
                "The mono sum must stay the same."
            );
        }
    }
}

#[test]
fn tube_leans_even_and_tape_leans_odd() {
    let frames = 44_100;
    let input: Vec<(f32, f32)> = (0..frames)
        .map(|i| {
            let v = (0.5 * (2.0 * PI * 1000.0 * i as f64 / RATE).sin()) as f32;
            (v, v)
        })
        .collect();
    let harmonic = |output: &[(f32, f32)], n: f64| {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, (v, _)) in output[4410..].iter().enumerate() {
            let angle = 2.0 * PI * 1000.0 * n * i as f64 / RATE;
            re += f64::from(*v) * angle.cos();
            im += f64::from(*v) * angle.sin();
        }
        re.hypot(im)
    };
    let mut tube = Tube::new();
    tube.set(&settings::Tube {
        on: true,
        drive: 6.0,
        mix: 100.0,
    });
    tube.clear();
    let warm = run(&input, |l, r| tube.process(l, r));
    assert!(harmonic(&warm, 2.0) > 2.0 * harmonic(&warm, 3.0));
    let mut tape = Tape::new();
    tape.set(&settings::Tape {
        on: true,
        drive: 8.0,
        speed: 30,
    });
    tape.clear();
    let saturated = run(&input, |l, r| tape.process(l, r));
    assert!(harmonic(&saturated, 3.0) > 4.0 * harmonic(&saturated, 2.0));

    // Neither moves quiet passages from their level.
    let quiet: Vec<(f32, f32)> = input.iter().map(|(l, r)| (l * 0.1, r * 0.1)).collect();
    let tube_level = rms_db(&run(&quiet, |l, r| tube.process(l, r))[4410..]);
    let tape_level = rms_db(&run(&quiet, |l, r| tape.process(l, r))[4410..]);
    for level in [tube_level, tape_level] {
        assert!((level - rms_db(&quiet[4410..])).abs() < 0.3, "{level}");
    }
}

#[test]
fn bus_compressor_follows_its_static_curve() {
    let mut compressor = BusCompressor::new();
    compressor.set(&settings::BusCompressor {
        on: true,
        threshold: -20.0,
        ratio: 4.0,
        attack: 1.0,
        release: 100.0,
        key: 0,
        ..settings::BusCompressor::default()
    });
    compressor.clear();
    // A steady sine 10 dB over the threshold settles 7.5 dB lower.
    let amplitude = db_to_gain(-10.0) * 2f64.sqrt();
    let mut last = 0.0_f32;
    for i in 0..44_100 {
        let v = (amplitude * (2.0 * PI * 1000.0 * i as f64 / RATE).sin()) as f32;
        last = last.max(if i > 40_000 {
            compressor.process(v, v).0.abs()
        } else {
            compressor.process(v, v);
            0.0
        });
    }
    let reduction = gain_to_db(amplitude) - gain_to_db(f64::from(last));
    assert!((reduction - 7.5).abs() < 0.3, "{reduction}");
    assert!((compressor.take_reduction() - 7.5).abs() < 0.3);
    assert_eq!(compressor.take_reduction(), 0.0);
}

#[test]
fn limiter_holds_true_peak_under_the_ceiling() {
    let mut signals: Vec<Vec<(f32, f32)>> = vec![program(30_000)];
    // Inter-sample peaks: quarter-rate sines at 45 degrees, single-sample
    // spikes, and full-scale square waves.
    signals.push(
        (0..8000)
            .map(|i| {
                let v = (0.9 * (PI / 2.0 * i as f64 + PI / 4.0).sin()) as f32;
                (v, -v)
            })
            .collect(),
    );
    signals.push(
        (0..8000)
            .map(|i| {
                if i % 997 == 0 {
                    (1.0, -1.0)
                } else {
                    (0.01, 0.0)
                }
            })
            .collect(),
    );
    signals.push(
        (0..8000)
            .map(|i| {
                let v = if (i / 7) % 2 == 0 { 1.0 } else { -1.0 };
                (v, v)
            })
            .collect(),
    );
    for (index, signal) in signals.iter().enumerate() {
        for ceiling in [-1.0, -3.0, 0.0] {
            let mut limiter = Limiter::new();
            limiter.set(&settings::Limiter {
                on: true,
                gain: 12.0,
                ceiling,
                key: 0,
                ..settings::Limiter::default()
            });
            limiter.clear();
            let output = run(signal, |l, r| limiter.process(l, r));
            let mut meters = [TruePeak::ZERO; 2];
            let peak = output.iter().fold(0.0_f32, |p, (l, r)| {
                p.max(meters[0].push(*l)).max(meters[1].push(*r))
            });
            let (peak, ceiling) = (gain_to_db(f64::from(peak)), f64::from(ceiling));
            assert!(
                peak <= ceiling + 1e-3,
                "Signal {index} at {ceiling} dBTP peaks at {peak:.3}"
            );
            assert!(
                peak > ceiling - 1.0,
                "Signal {index} must reach near the ceiling, not {peak:.3}"
            );
        }
    }
}

#[test]
fn limiter_keeps_quiet_sound_unchanged_after_its_delay() {
    let input: Vec<(f32, f32)> = program(10_000)
        .iter()
        .map(|(l, r)| (l * 0.1, r * 0.1))
        .collect();
    let mut limiter = Limiter::new();
    limiter.set(&settings::Limiter {
        on: true,
        gain: 0.0,
        ceiling: -1.0,
        key: 0,
        ..settings::Limiter::default()
    });
    limiter.clear();
    let output = run(&input, |l, r| limiter.process(l, r));
    assert_eq!(
        &output[Limiter::LATENCY..],
        &input[..input.len() - Limiter::LATENCY]
    );
    assert_eq!(limiter.take_reduction(), 0.0);
}

#[test]
fn lowering_the_limiter_ceiling_during_play_does_not_clip() {
    // The right channel is half the left. The gain reduction scales both
    // alike; a clipped sample on the left alone would break the ratio.
    let input: Vec<(f32, f32)> = (0..8820)
        .map(|i| {
            let t = i as f64 / RATE;
            let v =
                (0.6 * (2.0 * PI * 100.0 * t).sin() + 0.35 * (2.0 * PI * 3000.0 * t).sin()) as f32;
            (v, v * 0.5)
        })
        .collect();
    let mut limiter = Limiter::new();
    let mut settings = settings::Limiter {
        on: true,
        gain: 6.0,
        ceiling: 0.0,
        key: 0,
        ..settings::Limiter::default()
    };
    limiter.set(&settings);
    limiter.clear();
    let mut output = Vec::new();
    for (index, (l, r)) in input.iter().enumerate() {
        if index == 4410 {
            settings.ceiling = -3.0;
            limiter.set(&settings);
        }
        output.push(limiter.process(*l, *r));
    }
    for (frame, (l, r)) in output.iter().enumerate() {
        if l.abs() > 1e-3 {
            assert!(
                (r / l - 0.5).abs() < 1e-4,
                "Frame {frame} clips: {l} and {r}"
            );
        }
    }
}

#[test]
fn a_glide_has_no_steps() {
    // Moving the trim EQ's high-pass across its whole range while a
    // tone plays changes the output smoothly.
    let mut eq = TrimEq::new();
    eq.set(&settings::TrimEq {
        on: true,
        high_pass: 10.0,
        mud: 0.0,
        harsh: -3.0,
    });
    eq.clear();
    eq.set(&settings::TrimEq {
        on: true,
        high_pass: 40.0,
        mud: -3.0,
        harsh: 0.0,
    });
    let mut previous = 0.0_f32;
    for i in 0..8820 {
        let v = (0.5 * (2.0 * PI * 200.0 * i as f64 / RATE).sin()) as f32;
        let (out, _) = eq.process(v, v);
        if i > 0 {
            assert!((out - previous).abs() < 0.03, "Step at frame {i}");
        }
        previous = out;
    }
}

#[test]
fn bus_compressor_sc_hpf_reduces_kick_induced_pumping() {
    // 50 Hz loud kick bursts mixed with a steady quiet 8 kHz hi-hat carrier.
    let signal: Vec<(f32, f32)> = (0..44_100)
        .map(|i| {
            let phase = (i % 22_050) as f64 / RATE;
            let kick = if phase < 0.08 {
                0.7 * (2.0 * PI * 50.0 * i as f64 / RATE).sin() * (-phase * 25.0).exp()
            } else {
                0.0
            };
            let hat = 0.04 * (2.0 * PI * 8000.0 * i as f64 / RATE).sin();
            let v = (kick + hat) as f32;
            (v, v)
        })
        .collect();
    let measure_pump = |sc_hpf_hz: f32| {
        let mut comp = BusCompressor::new();
        comp.set(&settings::BusCompressor {
            on: true,
            threshold: -20.0,
            ratio: 4.0,
            attack: 0.5,
            release: 300.0,
            key: 0,
            sc_hpf_hz,
            mix: 100.0,
            makeup_db: 0.0,
        });
        comp.clear();
        let wet = run(&signal, |l, r| comp.process(l, r));
        let mut hp = Biquad::new(Coefficients::high_pass(3000.0, FRAC_1_SQRT_2));
        let filtered: Vec<f64> = wet[22_050..]
            .iter()
            .map(|(l, _)| hp.process(0, f64::from(*l)))
            .collect();
        let envelope =
            |slice: &[f64]| (slice.iter().map(|x| x * x).sum::<f64>() / slice.len() as f64).sqrt();
        // Window immediately after kick decay vs settled window later in the beat.
        let during = gain_to_db(envelope(&filtered[1800..3200]));
        let recovered = gain_to_db(envelope(&filtered[16_000..21_000]));
        (recovered - during).abs()
    };
    let flat_pump = measure_pump(20.0);
    let hpf_pump = measure_pump(100.0);
    assert!(
        flat_pump > 1.5 && hpf_pump < 0.8 && hpf_pump < flat_pump * 0.3,
        "flat_pump={flat_pump:.2} dB, hpf_pump={hpf_pump:.2} dB"
    );
}

#[test]
fn bus_compressor_mix_and_makeup_blend_cleanly() {
    let input = program(22_050);
    let mut comp = BusCompressor::new();
    let mut cfg = settings::BusCompressor {
        on: true,
        threshold: -24.0,
        ratio: 4.0,
        attack: 1.0,
        release: 150.0,
        key: 0,
        sc_hpf_hz: 20.0,
        mix: 0.0,
        makeup_db: 0.0,
    };
    comp.set(&cfg);
    comp.clear();
    assert_eq!(run(&input, |l, r| comp.process(l, r)), input);

    cfg.mix = 100.0;
    comp.set(&cfg);
    comp.clear();
    let full_wet = run(&input, |l, r| comp.process(l, r));

    cfg.mix = 50.0;
    comp.set(&cfg);
    comp.clear();
    let half_mix = run(&input, |l, r| comp.process(l, r));
    for ((d, _), ((w, _), (h, _))) in input.iter().zip(full_wet.iter().zip(&half_mix)) {
        assert!((h - 0.5 * (d + w)).abs() < 1e-5);
    }

    cfg.mix = 100.0;
    cfg.makeup_db = 3.0;
    comp.set(&cfg);
    comp.clear();
    let boosted = run(&input, |l, r| comp.process(l, r));
    let delta_db = rms_db(&boosted[4410..]) - rms_db(&full_wet[4410..]);
    assert!((delta_db - 3.0).abs() < 0.05, "makeup delta {delta_db:.3}");
}

#[test]
fn multiband_reports_per_band_gain_reduction() {
    let tone = |hz: f64| -> Vec<(f32, f32)> {
        (0..22_050)
            .map(|i| {
                let s = (0.6 * (2.0 * PI * hz * i as f64 / RATE).sin()) as f32;
                (s, s)
            })
            .collect()
    };
    let mut zero = Multiband::new();
    zero.set(&settings::Multiband {
        on: true,
        low: 0.0,
        mid: 0.0,
        high: 0.0,
        key: 0,
    });
    zero.clear();
    let _ = run(&tone(80.0), |l, r| zero.process(l, r));
    assert_eq!(zero.take_reductions(), [0.0; 3]);

    let mut active = Multiband::new();
    active.set(&settings::Multiband {
        on: true,
        low: 80.0,
        mid: 80.0,
        high: 80.0,
        key: 0,
    });
    active.clear();
    // Skip crossover startup edge from t=0 sine slope before sampling steady-state band GR.
    let low_tone = tone(70.0);
    let _ = run(&low_tone[..2048], |l, r| active.process(l, r));
    let _ = active.take_reductions();
    let _ = run(&low_tone[2048..], |l, r| active.process(l, r));
    let low_gr = active.take_reductions();
    assert!(low_gr[0] > 1.0 && low_gr[0] > low_gr[2] * 3.0, "{low_gr:?}");
    assert_eq!(active.take_reductions(), [0.0; 3]);

    active.clear();
    let high_tone = tone(8000.0);
    let _ = run(&high_tone[..2048], |l, r| active.process(l, r));
    let _ = active.take_reductions();
    let _ = run(&high_tone[2048..], |l, r| active.process(l, r));
    let high_gr = active.take_reductions();
    assert!(
        high_gr[2] > 1.0 && high_gr[2] > high_gr[0] * 3.0,
        "{high_gr:?}"
    );
}

#[test]
fn limiter_all_styles_hold_true_peak_and_preserve_stereo_ratio() {
    let signal = program(30_000);
    for style in [
        settings::LimiterStyle::Transparent,
        settings::LimiterStyle::Punchy,
        settings::LimiterStyle::Modern,
    ] {
        let mut limiter = Limiter::new();
        limiter.set(&settings::Limiter {
            on: true,
            gain: 9.0,
            ceiling: -1.0,
            key: 0,
            style,
        });
        limiter.clear();
        let unbalanced: Vec<(f32, f32)> = signal.iter().map(|(l, _)| (*l, *l * 0.5)).collect();
        let output = run(&unbalanced, |l, r| limiter.process(l, r));
        let mut meters = [TruePeak::ZERO; 2];
        let mut peak = 0.0_f32;
        for (l, r) in &output {
            peak = peak.max(meters[0].push(*l)).max(meters[1].push(*r));
            if l.abs() > 1e-3 {
                assert!((r / l - 0.5).abs() < 1e-4, "{style:?} skewed stereo ratio");
            }
        }
        let peak_db = gain_to_db(f64::from(peak));
        assert!(peak_db <= -1.0 + 1e-3, "{style:?} peaked at {peak_db:.3}");
        assert!(limiter.take_reduction() > 0.5, "{style:?} reported 0 GR");
    }
}

fn loud_limiter(style: settings::LimiterStyle) -> settings::Limiter {
    settings::Limiter {
        on: true,
        gain: 9.0,
        ceiling: -1.0,
        key: 0,
        style,
    }
}

#[test]
fn a_limiter_style_change_glides_without_a_step() {
    // A style change during playback reaches the pre-shave without a jump.
    let sine = |i: usize| (0.5 * (2.0 * PI * 100.0 * i as f64 / RATE).sin()) as f32;
    let mut limiter = Limiter::new();
    limiter.set(&loud_limiter(settings::LimiterStyle::Transparent));
    limiter.clear();
    let mut previous = 0.0_f32;
    let mut settled_step = 0.0_f32;
    for i in 0..20_000 {
        let (out, _) = limiter.process(sine(i), sine(i));
        if i >= 10_000 {
            settled_step = settled_step.max((out - previous).abs());
        }
        previous = out;
    }
    limiter.set(&loud_limiter(settings::LimiterStyle::Modern));
    let mut switch_step = 0.0_f32;
    for i in 20_000..26_000 {
        let (out, _) = limiter.process(sine(i), sine(i));
        switch_step = switch_step.max((out - previous).abs());
        previous = out;
    }
    assert!(
        switch_step < 1.5 * settled_step,
        "style switch stepped by {switch_step}, a settled frame moves {settled_step}"
    );
}

#[test]
fn limiter_reduction_reports_the_gain_each_sample_received() {
    // The pre-shave and the lookahead gain reach the same sample at
    // different times; the meter reports their product on that sample.
    // One short burst after silence: no earlier reduction lingers.
    let input: Vec<f32> = (0..30_000)
        .map(|i| {
            let burst = (20_000..20_300).contains(&i);
            f32::from(u8::from(burst)) * (0.5 * (2.0 * PI * 1000.0 * i as f64 / RATE).sin()) as f32
        })
        .collect();
    let mut limiter = Limiter::new();
    limiter.set(&loud_limiter(settings::LimiterStyle::Modern));
    limiter.clear();
    let gain = db_to_gain(9.0) as f32;
    let (mut most, mut reported) = (0.0_f64, 0.0_f32);
    for (i, v) in input.iter().enumerate() {
        let out = limiter.process(*v, *v).0;
        let reduction = limiter.take_reduction();
        let entered = input[i.saturating_sub(Limiter::LATENCY)] * gain;
        if i >= 20_000 && entered.abs() > 0.5 {
            most = most.max(-gain_to_db(f64::from(out / entered)));
            reported = reported.max(reduction);
        }
    }
    assert!(
        (f64::from(reported) - most).abs() < 0.1,
        "reported {reported:.2} dB, the samples received {most:.2} dB"
    );
}

/// The gain, in dB, of a settled pedal on a steady sine, measured over the
/// second half of one second.
fn sine_gain_db(hz: f64, mut pedal: impl FnMut(f32, f32) -> (f32, f32)) -> f64 {
    let frames = RATE as usize;
    let (mut input, mut output) = (0.0_f64, 0.0_f64);
    for i in 0..frames {
        let x = (0.25 * (2.0 * PI * hz * i as f64 / RATE).sin()) as f32;
        let (y, _) = pedal(x, x);
        if i >= frames / 2 {
            input += f64::from(x) * f64::from(x);
            output += f64::from(y) * f64::from(y);
        }
    }
    10.0 * (output / input).log10()
}

#[test]
fn eq_curve_sampler_matches_the_trim_and_lift_filters() {
    use super::sample_master_eq_curve;
    use crate::master::{MasterChain, PedalKind};

    let chain = MasterChain {
        trim_eq: settings::TrimEq {
            on: true,
            high_pass: 20.0,
            mud: -3.0,
            harsh: -3.0,
        },
        lift_eq: settings::LiftEq {
            on: true,
            low: 1.5,
            air: 2.0,
        },
        ..MasterChain::default()
    };
    let mut trim = TrimEq::new();
    trim.set(&chain.trim_eq);
    trim.clear();
    let mut lift = LiftEq::new();
    lift.set(&chain.lift_eq);
    lift.clear();
    let (mut trim_curve, mut lift_curve) = ([0.0_f32; 65], [0.0_f32; 65]);
    sample_master_eq_curve(&chain, PedalKind::TrimEq, &mut trim_curve);
    sample_master_eq_curve(&chain, PedalKind::LiftEq, &mut lift_curve);
    // Every fourth point, from 20 Hz to 20 kHz. Points 24 and 48 are near
    // the cuts at 250 Hz and 3.5 kHz, and point 60 is near the air shelf.
    for i in (0..=64).step_by(4) {
        let hz = 20.0 * 1000.0_f64.powf(i as f64 / 64.0);
        let trim_db = sine_gain_db(hz, |l, r| trim.process(l, r));
        let lift_db = sine_gain_db(hz, |l, r| lift.process(l, r));
        assert!(
            (trim_db - f64::from(trim_curve[i])).abs() < 0.05,
            "Trim at {hz:.0} Hz: audio {trim_db:.3} dB, curve {:.3} dB",
            trim_curve[i]
        );
        assert!(
            (lift_db - f64::from(lift_curve[i])).abs() < 0.05,
            "Lift at {hz:.0} Hz: audio {lift_db:.3} dB, curve {:.3} dB",
            lift_curve[i]
        );
    }
}

#[test]
fn mono_below_makes_low_stereo_audio_mono() {
    let mut imaging = Imaging::new();
    imaging.set(&settings::Imaging {
        on: true,
        width: 140.0,
        mono_below: 120.0,
    });
    imaging.clear();
    let sub: Vec<(f32, f32)> = (0..16_000)
        .map(|i| {
            let l = (0.5 * (2.0 * PI * 60.0 * i as f64 / RATE).sin()) as f32;
            let r = (0.5 * (2.0 * PI * 60.0 * i as f64 / RATE + 1.2).sin()) as f32;
            (l, r)
        })
        .collect();
    let out = run(&sub, |l, r| imaging.process(l, r));
    let (lr, ll, rr) = out[4096..]
        .iter()
        .fold((0.0_f64, 0.0_f64, 0.0_f64), |acc, &(l, r)| {
            let (l, r) = (f64::from(l), f64::from(r));
            (acc.0 + l * r, acc.1 + l * l, acc.2 + r * r)
        });
    let corr = lr / (ll * rr).sqrt();
    assert!(corr > 0.97, "60 Hz correlation {corr:.4}");
}

/// The CPU time of a 256-frame live block with all 13 pedals on. Run it on
/// a release build, on a quiet machine:
///
/// ```text
/// cargo test --release --lib full_13_pedal_live_block_cpu_p99 -- --ignored --nocapture
/// ```
///
/// A debug build only prints the times, because it is far slower.
#[test]
#[ignore = "release benchmark; use a quiet machine"]
fn full_13_pedal_live_block_cpu_p99() {
    use crate::master::{MasterChain, MasterControl, MasterProcessor, PedalKind};
    use crate::playback_quality::Oversampling;
    use std::time::Instant;

    let mut chain = MasterChain::default();
    for kind in PedalKind::ALL {
        chain.set_on(kind, true);
    }
    let control = MasterControl::new(&chain);
    let mut processor = MasterProcessor::live_with_quality(control, Oversampling::FourX);
    let seed: Vec<f32> = program(256).into_iter().flat_map(|(l, r)| [l, r]).collect();
    let mut block = vec![0.0_f32; 512];
    for _ in 0..24 {
        block.copy_from_slice(&seed);
        processor.process(&mut block);
    }
    let mut times_ms = Vec::with_capacity(400);
    for _ in 0..400 {
        block.copy_from_slice(&seed);
        let t0 = Instant::now();
        processor.process(&mut block);
        times_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    times_ms.sort_by(f64::total_cmp);
    let p50 = times_ms[200];
    let p95 = times_ms[380];
    let p99 = times_ms[396];
    println!("Live 13-pedal 256-frame block CPU: p50={p50:.3} ms p95={p95:.3} ms p99={p99:.3} ms");
    if !cfg!(debug_assertions) {
        assert!(p99 <= 1.0, "p99 {p99:.3} ms exceeded 1.0 ms budget");
    }
}
