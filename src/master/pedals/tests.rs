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

    // Below their curves, the soft clip and maximizer are the
    // oversampling alone.
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
    assert_eq!(run(&quiet, |l, r| maximizer.process(l, r)), through);

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
