use super::*;

#[test]
fn master_construction_prepares_tables_before_the_first_callback() {
    const CHILD: &str = "MIXJAM_DSP_TABLE_INIT_TEST";
    const CHECKED: &str = "DSP tables are ready before processing.";
    if std::env::var_os(CHILD).is_some() {
        use std::sync::atomic::Ordering;
        assert_eq!(TABLE_INITIALIZATIONS.load(Ordering::Relaxed), 0);
        let chain = crate::master::MasterChain::default();
        let _processor = crate::master::MasterProcessor::new(&chain);
        assert_eq!(TABLE_INITIALIZATIONS.load(Ordering::Relaxed), 3);
        println!("{CHECKED}");
        return;
    }
    // A fresh test process proves constructor ownership without depending
    // on which other DSP tests initialized the process-wide tables first.
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "master::dsp::tests::master_construction_prepares_tables_before_the_first_callback",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        result.status.success() && String::from_utf8_lossy(&result.stdout).contains(CHECKED),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn sine(hz: f64, amplitude: f64, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| (amplitude * (2.0 * PI * hz * i as f64 / RATE).sin()) as f32)
        .collect()
}

fn rms(samples: &[f32]) -> f64 {
    (samples.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
}

#[test]
fn paired_oversampling_matches_independent_histories_bit_for_bit() {
    let mut input = sine(55.0, 0.8, 4000);
    input.extend(sine(17_000.0, 0.9, 4000));
    let mut seed = 0x1234_5678_u32;
    input.extend((0..4000).map(|_| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (f64::from(seed) / f64::from(u32::MAX) * 2.0 - 1.0) as f32
    }));
    input.extend(std::iter::repeat_n(0.0, 4000));
    let (mut driven, mut reference) = (Oversampler::ZERO, Oversampler::ZERO);
    let (mut paired, mut paired_reference) = (Oversampler::ZERO, Oversampler::ZERO);
    let (mut last, mut paired_last) = ([0.0_f32; 2], [0.0_f32; 2]);
    // Stateful curves exercise the order of the four inputs. The
    // histories carry through arbitrary block boundaries and silence.
    let shape = |sample: f32, previous: &mut f32, nonlinear: bool| {
        let mean = 0.5 * (sample + *previous);
        *previous = sample;
        if nonlinear { (mean * 8.0).tanh() } else { mean }
    };
    let mut start = 0;
    for size in [1, 3, 128, 441, 69, 2049].into_iter().cycle() {
        let end = (start + size).min(input.len());
        for (offset, sample) in input[start..end].iter().copied().enumerate() {
            let expected = [
                driven.process(sample, |x| shape(x, &mut last[0], true)),
                reference.process(sample, |x| shape(x, &mut last[1], false)),
            ];
            let actual = paired.process_pair(&mut paired_reference, sample, |x| {
                [
                    shape(x, &mut paired_last[0], true),
                    shape(x, &mut paired_last[1], false),
                ]
            });
            assert_eq!(
                actual.map(f32::to_bits),
                expected.map(f32::to_bits),
                "frame {}",
                start + offset
            );
        }
        start = end;
        if start == input.len() {
            break;
        }
    }
}

#[test]
fn oversampling_delays_audible_sound_by_whole_frames_without_coloring_it() {
    for hz in [40.0, 1000.0, 10_000.0, 19_000.0] {
        let input = sine(hz, 0.8, 8000);
        let mut oversampler = Oversampler::ZERO;
        let output: Vec<f32> = input
            .iter()
            .map(|v| oversampler.process(*v, |x| x))
            .collect();
        let tail = &output[2000..];
        let error: Vec<f32> = tail
            .iter()
            .zip(&input[2000 - Oversampler::LATENCY..])
            .map(|(out, inp)| out - inp)
            .collect();
        let level = gain_to_db(rms(&error) / rms(tail));
        assert!(level < -80.0, "{hz} Hz differs by {level:.1} dB");
    }
}

#[test]
fn oversampling_keeps_curve_harmonics_from_folding_into_the_band() {
    // A hard clip of 15 kHz makes 45 kHz and 75 kHz, which would fold
    // to 0.9 kHz and 13.2 kHz at the mix rate. The analysis spans whole
    // cycles of every tone involved, so none leaks into another's bin.
    let input = sine(15_000.0, 1.0, 4096 + 11_760);
    let mut oversampler = Oversampler::ZERO;
    let output: Vec<f64> = input
        .iter()
        .map(|v| f64::from(oversampler.process(*v, |x| x.clamp(-0.5, 0.5))))
        .collect();
    let tail = &output[4096..];
    let amplitude = |hz: f64| {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in tail.iter().enumerate() {
            let angle = 2.0 * PI * hz * i as f64 / RATE;
            re += v * angle.cos();
            im += v * angle.sin();
        }
        2.0 * re.hypot(im) / tail.len() as f64
    };
    let fundamental = amplitude(15_000.0);
    for alias in [900.0, 13_200.0] {
        let level = gain_to_db(amplitude(alias) / fundamental);
        assert!(level < -70.0, "Alias at {alias} Hz is {level:.1} dB");
    }
}

#[test]
fn filters_follow_the_cookbook_shapes() {
    let close = |a: f64, b: f64| (a - b).abs() < 0.01;
    let peak = Coefficients::peaking(250.0, 2.5, -3.0);
    assert!(close(gain_to_db(peak.magnitude(250.0)), -3.0));
    assert!(close(gain_to_db(peak.magnitude(5000.0)), 0.0));
    let high = Coefficients::high_pass(20.0, std::f64::consts::FRAC_1_SQRT_2);
    assert!(close(gain_to_db(high.magnitude(20.0)), -3.01));
    let shelf = Coefficients::high_shelf(12_000.0, 0.6, 2.0);
    assert!(close(gain_to_db(shelf.magnitude(20.0)), 0.0));
    assert!(gain_to_db(shelf.magnitude(20_000.0)) > 1.5);
    // A shelf and its opposite cancel.
    let back = Coefficients::high_shelf(12_000.0, 0.6, -2.0);
    for hz in [100.0, 5000.0, 15_000.0] {
        assert!((shelf.magnitude(hz) * back.magnitude(hz) - 1.0).abs() < 1e-9);
    }
    // Linkwitz-Riley low and high halves sum to an all-pass.
    let q = std::f64::consts::FRAC_1_SQRT_2;
    let (mut low, mut high) = (
        [Biquad::new(Coefficients::low_pass(500.0, q)); 2],
        [Biquad::new(Coefficients::high_pass(500.0, q)); 2],
    );
    let mut all = Biquad::new(Coefficients::all_pass(500.0, q));
    for i in 0..4000 {
        let x = if i == 0 { 1.0 } else { 0.0 };
        let (a, b) = (low[0].process(0, x), high[0].process(0, x));
        let sum = low[1].process(0, a) + high[1].process(0, b);
        assert!((sum - all.process(0, x)).abs() < 1e-12);
    }
}

#[test]
fn a_steep_shelf_slope_at_a_high_gain_stays_finite() {
    // Above a slope of 1, 24 dB makes the cookbook's square root negative.
    for shelf in [Coefficients::low_shelf, Coefficients::high_shelf] {
        let steep = shelf(1000.0, 2.0, 24.0);
        assert!(steep.values().iter().all(|value| value.is_finite()));
        assert_eq!(steep, shelf(1000.0, 1.0, 24.0));
    }
}

#[test]
fn true_peaks_see_between_the_samples() {
    // A quarter-rate sine sampled at 45 degrees peaks between samples, 3 dB
    // above them.
    let input: Vec<f32> = (0..400)
        .map(|i| (0.5 * (PI / 2.0 * i as f64 + PI / 4.0).sin()) as f32)
        .collect();
    let sample_peak = input.iter().fold(0.0_f32, |a, v| a.max(v.abs()));
    let mut meter = TruePeak::ZERO;
    // Skip the tone's abrupt start, which rings like any step.
    let true_peak = input
        .iter()
        .map(|v| meter.push(*v))
        .skip(2 * TruePeak::DELAY)
        .fold(0.0_f32, f32::max);
    let lift = gain_to_db(f64::from(true_peak / sample_peak));
    assert!((lift - 3.01).abs() < 0.02, "{lift}");
}

#[test]
fn a_mirrored_history_reads_as_a_shifted_one() {
    let mut history = History::<5>::ZERO;
    let mut shifted = [0.0_f32; 5];
    for step in 0..23 {
        let sample = step as f32 * 0.5 - 3.0;
        shifted.copy_within(..4, 1);
        shifted[0] = sample;
        assert_eq!(
            history.push(sample).map(f32::to_bits),
            shifted.map(f32::to_bits),
            "step {step}"
        );
    }
}

#[test]
fn moving_min_keeps_the_oldest_live_value_when_full() {
    let mut minimum = MovingMin::<4>::EMPTY;
    for index in 0usize..20 {
        assert_eq!(
            minimum.push(index as f64, 4),
            index.saturating_sub(3) as f64
        );
    }
}

#[test]
fn a_cascade_runs_each_channel_through_the_sections_in_order() {
    let designs = [
        Coefficients::high_pass(30.0, 0.7),
        Coefficients::peaking(300.0, 1.0, -3.0),
        Coefficients::high_shelf(8000.0, 0.6, 2.0),
    ];
    let (mut cascaded, mut by_hand) = (designs.map(Biquad::new), designs.map(Biquad::new));
    for (frame, (left, right)) in program(2000).into_iter().enumerate() {
        let mut expected = [f64::from(left), f64::from(right)];
        for filter in &mut by_hand {
            for (channel, sample) in expected.iter_mut().enumerate() {
                *sample = filter.process(channel, *sample);
            }
        }
        let actual = [
            cascade(&mut cascaded, 0, f64::from(left)),
            cascade(&mut cascaded, 1, f64::from(right)),
        ];
        assert_eq!(
            actual.map(f64::to_bits),
            expected.map(f64::to_bits),
            "frame {frame}"
        );
    }
}

#[test]
fn a_low_pass_at_any_rate_matches_the_cookbook_written_out() {
    // The Effect Studio and the sample editor wrote this design out by hand.
    for rate in [8000.0, 22_050.0, 44_100.0, 48_000.0, 96_000.0] {
        for hz in [20.0, 440.0, 5000.0, 19_000.0] {
            let hz = f64::min(hz, rate * 0.45);
            for q in [0.5, std::f64::consts::FRAC_1_SQRT_2, 3.0, 15.5] {
                let omega = 2.0 * PI * hz / rate;
                let cosine = omega.cos();
                let alpha = omega.sin() / (2.0 * q);
                let a0 = 1.0 + alpha;
                let b0 = (1.0 - cosine) / 2.0 / a0;
                let written = [
                    b0,
                    (1.0 - cosine) / a0,
                    b0,
                    -2.0 * cosine / a0,
                    (1.0 - alpha) / a0,
                ];
                assert_eq!(
                    Coefficients::low_pass_at(hz, q, rate)
                        .values()
                        .map(f64::to_bits),
                    written.map(f64::to_bits),
                    "{hz} Hz, Q {q}, {rate} Hz"
                );
            }
        }
    }
    // The sample editor divided by the square root of two, not by 2 Q.
    assert_eq!(
        (2.0 * std::f64::consts::FRAC_1_SQRT_2).to_bits(),
        std::f64::consts::SQRT_2.to_bits()
    );
    assert_eq!(
        Coefficients::low_pass(1000.0, 0.7),
        Coefficients::low_pass_at(1000.0, 0.7, RATE)
    );
}
