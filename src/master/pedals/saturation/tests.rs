use super::*;
use crate::master::dsp::{RATE, TruePeak, db_to_gain, gain_to_db};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

const TEST_FREQ_HZ: f64 = 376.0 * RATE / 4096.0;

fn sine_4048(frames: usize, dbfs: f64) -> Vec<(f32, f32)> {
    let amp = db_to_gain(dbfs);
    (0..frames)
        .map(|i| {
            let v = (amp * (2.0 * PI * TEST_FREQ_HZ * i as f64 / RATE).sin()) as f32;
            (v, v)
        })
        .collect()
}

fn measure_nonharm_and_worst_spur(samples: &[f32]) -> (f64, f64, f64) {
    let n = samples.len();
    let mut power_sig = 0.0_f64;
    let mut power_alias = 0.0_f64;
    let mut peak_fund = 0.0_f64;
    let mut worst_spur = 0.0_f64;
    let mut worst_hz = 0.0_f64;
    let df = RATE / n as f64;
    let bins = n / 2;
    let mut mag = vec![0.0_f64; bins];
    for (k, slot) in mag.iter_mut().enumerate().skip(4) {
        let w0 = 2.0 * PI * k as f64 / n as f64;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &s) in samples.iter().enumerate() {
            let t = 2.0 * PI * i as f64 / n as f64;
            let win =
                0.35875 - 0.48829 * t.cos() + 0.14128 * (2.0 * t).cos() - 0.01168 * (3.0 * t).cos();
            let ph = w0 * i as f64;
            re += f64::from(s) * win * ph.cos();
            im -= f64::from(s) * win * ph.sin();
        }
        *slot = (re * re + im * im).sqrt();
    }
    for (k, &m) in mag.iter().enumerate() {
        let hz = k as f64 * df;
        if hz < 50.0 {
            continue;
        }
        let near_harm = (1..=5).any(|h| (hz - f64::from(h) * TEST_FREQ_HZ).abs() <= 45.0);
        if (hz - TEST_FREQ_HZ).abs() <= 45.0 {
            peak_fund = peak_fund.max(m);
            power_sig += m * m;
        } else if !near_harm {
            power_alias += m * m;
            if m > worst_spur {
                worst_spur = m;
                worst_hz = hz;
            }
        }
    }
    let total_alias_db = 10.0 * ((power_alias + 1e-30) / (power_sig + 1e-30)).log10();
    let worst_spur_dbc = 20.0 * ((worst_spur + 1e-18) / (peak_fund + 1e-18)).log10();
    (total_alias_db, worst_spur_dbc, worst_hz)
}

#[test]
fn reduced_saturation_keeps_integer_impulse_delay_and_parallel_dry_paths() {
    for quality in [Oversampling::FourX, Oversampling::Off] {
        let mut tube = Tube::with_quality(quality);
        tube.set(&settings::Tube {
            mix: 0.0,
            drive: 10.0,
            ..Default::default()
        });
        tube.clear();
        for frame in 0..200 {
            let input = if frame == 0 {
                (0.75, -0.25)
            } else {
                (0.0, 0.0)
            };
            let expected = if frame == LIVE_SAT_LATENCY {
                (0.75, -0.25)
            } else {
                (0.0, 0.0)
            };
            assert_eq!(
                tube.process(input.0, input.1),
                expected,
                "{quality:?} {frame}"
            );
        }
    }
    let mut off_sat = MasterSaturation::new(RenderQuality::Live, Oversampling::Off);
    let curve = SatCurve::SoftClip {
        knee: 1.0,
        ceiling: 1.0,
    };
    for frame in 0..200 {
        let output = off_sat.process(if frame == 0 { 0.4 } else { 0.0 }, curve);
        assert_eq!(output, if frame == LIVE_SAT_LATENCY { 0.4 } else { 0.0 });
    }
}

#[test]
fn offline_saturation_pedals_suppress_4048hz_aliasing_below_minus_90_db() {
    let input = sine_4048(16_384, -3.0);
    type SatConfigFn = fn(&mut SoftClip, &mut Tube, &mut Maximizer, &mut Tape) -> u8;
    let run_case = |configure: SatConfigFn| {
        let mut soft = SoftClip::with_render_quality(RenderQuality::Offline, Oversampling::FourX);
        let mut tube = Tube::with_render_quality(RenderQuality::Offline, Oversampling::FourX);
        let mut max = Maximizer::with_render_quality(RenderQuality::Offline, Oversampling::FourX);
        let mut tape = Tape::with_render_quality(RenderQuality::Offline, Oversampling::FourX);
        let which = configure(&mut soft, &mut tube, &mut max, &mut tape);
        soft.clear();
        tube.clear();
        max.clear();
        tape.clear();
        let out: Vec<f32> = input
            .iter()
            .map(|&(l, r)| match which {
                0 => soft.process(l, r).0,
                1 => tube.process(l, r).0,
                2 => max.process(l, r).0,
                _ => tape.process(l, r).0,
            })
            .collect();
        measure_nonharm_and_worst_spur(&out[8192..8192 + 4096])
    };
    let cases: [(&str, SatConfigFn); 4] = [
        ("SoftClip", |s, _, _, _| {
            s.set(&settings::SoftClip {
                on: true,
                amount: 4.0,
                ceiling: -1.0,
            });
            0
        }),
        ("Tube", |_, t, _, _| {
            t.set(&settings::Tube {
                on: true,
                drive: 6.0,
                mix: 100.0,
            });
            1
        }),
        ("Maximizer", |_, _, m, _| {
            m.set(&settings::Maximizer {
                on: true,
                boost: 25.0,
            });
            2
        }),
        ("Tape", |_, _, _, p| {
            p.set(&settings::Tape {
                on: true,
                drive: 6.0,
                speed: 30,
            });
            3
        }),
    ];
    for (name, cfg) in cases {
        let (alias_db, spur_dbc, worst_hz) = run_case(cfg);
        assert!(
            alias_db < -90.0,
            "{name} alias={alias_db:.2} dB (worst_spur={spur_dbc:.2} dBc at {worst_hz:.1} Hz)"
        );
        assert!(
            spur_dbc < -85.0,
            "{name} worst spur={spur_dbc:.2} dBc at {worst_hz:.1} Hz"
        );
    }
}

#[test]
fn maximizer_holds_intersample_peaks_from_the_first_transient() {
    for (render, quality) in [
        (RenderQuality::Offline, Oversampling::FourX),
        (RenderQuality::Live, Oversampling::FourX),
        (RenderQuality::Live, Oversampling::Off),
    ] {
        for boost in [0.0, 0.1, 6.0, 25.0] {
            let mut maximizer = Maximizer::with_render_quality(render, quality);
            maximizer.set(&settings::Maximizer { on: true, boost });
            maximizer.clear();
            let mut tp = [TruePeak::ZERO; 2];
            let mut max_tp = 0.0_f32;
            for i in 0..4096 {
                let phase = FRAC_PI_4 * ((i / 128) % 8) as f64;
                let s = if i % 128 < 8 {
                    0.0
                } else {
                    (0.96 * (FRAC_PI_2 * i as f64 + phase).sin()) as f32
                };
                let (left, right) = maximizer.process(s, -s);
                max_tp = max_tp.max(tp[0].push(left)).max(tp[1].push(right));
            }
            let tp_db = gain_to_db(f64::from(max_tp));
            assert!(
                tp_db <= -1.0,
                "{render:?} {quality:?} boost {boost}: {tp_db:.2} dBTP"
            );
        }
    }
}

#[test]
fn maximizer_preserves_nominal_alignment_and_level_between_quality_modes() {
    let configurations = [
        (RenderQuality::Live, Oversampling::FourX, LIVE_SAT_LATENCY),
        (RenderQuality::Live, Oversampling::Off, LIVE_SAT_LATENCY),
        (
            RenderQuality::Offline,
            Oversampling::FourX,
            MAXIMIZER_OFFLINE_LATENCY,
        ),
    ];
    let mut levels = [0.0; 3];
    for (index, (render, quality, latency)) in configurations.into_iter().enumerate() {
        let mut maximizer = Maximizer::with_render_quality(render, quality);
        maximizer.set(&settings::Maximizer {
            on: true,
            boost: 25.0,
        });
        maximizer.clear();
        let (mut sum, mut moment) = (0.0, 0.0);
        for frame in 0..256 {
            let impulse = if frame == 16 { 0.03 } else { 0.0 };
            let weight = f64::from(maximizer.process(impulse, 0.0).0);
            sum += weight;
            moment += frame as f64 * weight;
        }
        let measured = moment / sum - 16.0;
        assert!(
            (measured - latency as f64).abs() <= 0.30,
            "{render:?} {quality:?}: {measured} vs {latency}"
        );

        maximizer.clear();
        let power = (0..8192)
            .map(|frame| {
                let t = frame as f64 / RATE;
                let sample = (0.30 * (2.0 * PI * 99.0 * t).sin()
                    + 0.18 * (2.0 * PI * 4048.0 * t).sin()) as f32;
                f64::from(maximizer.process(sample, sample).0).powi(2)
            })
            .skip(4096)
            .sum::<f64>();
        levels[index] = 10.0 * (power / 4096.0).log10();
    }
    assert!((levels[0] - levels[2]).abs() <= 0.5, "{levels:?}");
}

#[test]
fn base_rate_maximizer_preserves_below_knee_treble_as_boost_changes() {
    let measure = |boost| {
        let mut maximizer = Maximizer::with_quality(Oversampling::Off);
        maximizer.set(&settings::Maximizer { on: true, boost });
        maximizer.clear();
        let power = (0..1200)
            .map(|frame| {
                let sample = (0.1 * (2.0 * PI * frame as f64 / 3.0).sin()) as f32;
                f64::from(maximizer.process(sample, 0.0).0).powi(2)
            })
            .skip(300)
            .sum::<f64>();
        10.0 * (power / 900.0).log10()
    };
    let lift = measure(1.0) - measure(0.0);
    assert!(
        (lift - 0.25).abs() < 0.01,
        "expected 0.25 dB lift, got {lift:.2} dB"
    );
}
