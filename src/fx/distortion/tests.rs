use super::*;
use crate::fx::oversample::RenderQuality;
use crate::fx::settings::{DistortionShape, Emberform};

fn settings(shape: DistortionShape) -> Emberform {
    Emberform {
        shape,
        duck_amount_percent: 0.0,
        ..Emberform::default()
    }
}

fn render(
    settings: &Emberform,
    quality: RenderQuality,
    frames: usize,
    input: impl Fn(usize) -> f64,
) -> Vec<f64> {
    let mut core = Core::new();
    core.set(settings);
    core.settle();
    (0..frames)
        .map(|frame| {
            let x = input(frame);
            core.process_with_quality(x, x, quality).0
        })
        .collect()
}

fn rms(samples: &[f64]) -> f64 {
    (samples.iter().map(|x| x * x).sum::<f64>() / samples.len() as f64).sqrt()
}

/// The amplitude of `samples` at `hz`, from a single-bin DFT.
fn bin(samples: &[f64], hz: f64) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    for (n, x) in samples.iter().enumerate() {
        let phase = 2.0 * PI * hz * n as f64 / RATE;
        re += x * phase.cos();
        im += x * phase.sin();
    }
    2.0 * (re * re + im * im).sqrt() / samples.len() as f64
}

#[test]
fn every_shape_holds_the_level_and_carries_no_dc() {
    let tone = |frame: usize| 0.25 * (2.0 * PI * 220.0 * frame as f64 / RATE).sin();
    let input: Vec<f64> = (0..RATE as usize).map(tone).collect();
    let reference = rms(&input[22_050..]);
    for quality in [RenderQuality::Live, RenderQuality::Offline] {
        for shape in DistortionShape::ALL {
            let out = render(
                &Emberform {
                    drive_db: 24.0,
                    asymmetry_percent: 60.0,
                    low_cut_hz: 20.0,
                    high_cut_hz: 20_000.0,
                    output_db: 0.0,
                    level_match: true,
                    ..settings(shape)
                },
                quality,
                input.len(),
                tone,
            );
            let tail = &out[22_050..];
            let level = 20.0 * (rms(tail) / reference).log10();
            assert!(
                level.abs() < 3.0,
                "{quality:?} {shape:?} is {level:.1} dB off"
            );
            let mean = tail.iter().sum::<f64>() / tail.len() as f64;
            assert!(
                mean.abs() < 1e-3,
                "{quality:?} {shape:?} carries {mean} of DC"
            );
            assert!(tail.iter().all(|x| x.is_finite() && x.abs() < 2.0));
        }
    }
}

#[test]
fn the_drive_adds_harmonics_and_tracks_live_and_offline_latency() {
    let hz = 441.0;
    let tone = |frame: usize| 0.2 * (2.0 * PI * hz * frame as f64 / RATE).sin();
    let gentle = render(
        &Emberform {
            drive_db: 0.0,
            ..settings(DistortionShape::Tape)
        },
        RenderQuality::Live,
        44_100,
        tone,
    );
    let hot = render(
        &Emberform {
            drive_db: 30.0,
            ..settings(DistortionShape::Tape)
        },
        RenderQuality::Live,
        44_100,
        tone,
    );
    let third = |out: &[f64]| bin(&out[22_050..], 3.0 * hz) / bin(&out[22_050..], hz);
    assert!(third(&hot) > 10.0 * third(&gentle));

    let peak_for = |quality: RenderQuality| {
        let mut core = Core::new();
        core.set(&Emberform {
            drive_db: 0.0,
            low_cut_hz: 20.0,
            high_cut_hz: 20_000.0,
            ..settings(DistortionShape::HardClip)
        });
        core.settle();
        let out: Vec<f64> = (0..160)
            .map(|frame| {
                core.process_with_quality(if frame == 10 { 0.3 } else { 0.0 }, 0.0, quality)
                    .0
            })
            .collect();
        (0..160)
            .max_by(|&a, &b| out[a].abs().total_cmp(&out[b].abs()))
            .unwrap()
    };

    let live_peak = peak_for(RenderQuality::Live);
    assert!((10..=16).contains(&live_peak), "live peak={live_peak}");

    let offline_peak = peak_for(RenderQuality::Offline);
    assert!(
        (10 + Core::OFFLINE_LATENCY..=12 + Core::OFFLINE_LATENCY).contains(&offline_peak),
        "offline peak={offline_peak}"
    );
}

#[test]
fn offline_oversampling_suppresses_high_frequency_aliasing() {
    let f0 = 4048.0;
    let tone = |frame: usize| 0.45 * (2.0 * PI * f0 * frame as f64 / RATE).sin();
    let probe_freqs: Vec<f64> = [
        428.0, 1272.0, 1940.0, 2776.0, 3620.0, 5320.0, 6164.0, 7252.0, 9368.0, 11308.0, 13416.0,
        15356.0, 17464.0, 19404.0,
    ]
    .into_iter()
    .collect();

    for shape in DistortionShape::ALL {
        if shape == DistortionShape::Bitcrush {
            continue;
        }
        let out = render(
            &Emberform {
                drive_db: 24.0,
                asymmetry_percent: 35.0,
                low_cut_hz: 20.0,
                high_cut_hz: 20_000.0,
                output_db: 0.0,
                ..settings(shape)
            },
            RenderQuality::Offline,
            88_200,
            tone,
        );
        let tail = &out[44_100..];
        let ref_amp = bin(tail, f0).max(bin(tail, 2.0 * f0)).max(1e-9);
        let mut worst_spur_db = -200.0_f64;
        let mut worst_hz = 0.0_f64;
        let mut total_alias_sq = 0.0_f64;
        for &hz in &probe_freqs {
            let amp = bin(tail, hz);
            let db = 20.0 * (amp / ref_amp).max(1e-12).log10();
            if db > worst_spur_db {
                worst_spur_db = db;
                worst_hz = hz;
            }
            total_alias_sq += amp * amp;
        }
        let total_alias_db = 10.0 * (total_alias_sq / (ref_amp * ref_amp)).max(1e-12).log10();
        assert!(
            worst_spur_db < -85.0,
            "{shape:?} worst alias spur {worst_spur_db:.1} dBc at {worst_hz} Hz >= -85 dBc"
        );
        assert!(
            total_alias_db < -85.0,
            "{shape:?} total alias {total_alias_db:.1} dB"
        );
    }
}

#[test]
fn the_crusher_steps_and_holds_and_rejects_imd_spurs() {
    let ramp = |frame: usize| (frame as f64 / 2000.0).min(0.8);
    let mut core = Core::new();
    core.set(&Emberform {
        drive_db: 0.0,
        bits: 3,
        crush_rate_hz: 4410.0,
        low_cut_hz: 20.0,
        high_cut_hz: 20_000.0,
        ..settings(DistortionShape::Bitcrush)
    });
    core.settle();
    let mut changes = 0;
    let mut last = None;
    for frame in 0..1000 {
        core.process(ramp(frame), ramp(frame));
        let held = core.sides[0].held;
        if last != Some(held) {
            changes += 1;
            last = Some(held);
        }
        assert_eq!((held * 4.0).fract(), 0.0, "{held} is not on a step");
    }
    assert!(changes <= 105, "{changes}");

    // Dual-tone 100 Hz + 7000 Hz at 8-bit, 10 kHz crush with pre-AA + post reconstruction.
    let dual = |frame: usize| {
        let t = frame as f64 / RATE;
        0.25 * (2.0 * PI * 100.0 * t).sin() + 0.25 * (2.0 * PI * 7000.0 * t).sin()
    };
    let out = render(
        &Emberform {
            drive_db: 0.0,
            bits: 8,
            crush_rate_hz: 10_000.0,
            low_cut_hz: 20.0,
            high_cut_hz: 9_000.0,
            output_db: 0.0,
            level_match: false,
            ..settings(DistortionShape::Bitcrush)
        },
        RenderQuality::Offline,
        55_125,
        dual,
    );
    let tail = &out[11_025..];
    let fund = bin(tail, 100.0).max(1e-9);
    // 7000 Hz aliased around 10000 Hz lands at 3000 Hz (and +-100 Hz IMD at 2900/3100 Hz).
    for spur_hz in [2900.0, 3000.0, 3100.0] {
        let spur_db = 20.0 * (bin(tail, spur_hz) / fund).max(1e-12).log10();
        assert!(
            spur_db < -50.0,
            "Bitcrush IMD/alias spur at {spur_hz} Hz is {spur_db:.1} dBc"
        );
    }
}

#[test]
fn level_match_toggle_and_silence_gate_behave_cleanly() {
    let tone = |frame: usize| 0.2 * (2.0 * PI * 110.0 * frame as f64 / RATE).sin();
    let matched = render(
        &Emberform {
            drive_db: 24.0,
            level_match: true,
            ..settings(DistortionShape::Tube)
        },
        RenderQuality::Offline,
        44_100,
        tone,
    );
    let unmatched = render(
        &Emberform {
            drive_db: 24.0,
            level_match: false,
            ..settings(DistortionShape::Tube)
        },
        RenderQuality::Offline,
        44_100,
        tone,
    );
    let boost_db = 20.0 * (rms(&unmatched[22_050..]) / rms(&matched[22_050..])).log10();
    assert!(
        boost_db > 4.0,
        "Expected un-matched drive boost > 4 dB, got {boost_db:.2} dB"
    );

    // Silence gate at -58 dBFS: freeze matched_gain when input drops to -70 dBFS.
    let mut core = Core::new();
    core.set(&Emberform {
        drive_db: 24.0,
        level_match: true,
        ..settings(DistortionShape::Tube)
    });
    core.settle();
    for frame in 0..22_050 {
        let x = 0.25 * (2.0 * PI * 110.0 * frame as f64 / RATE).sin();
        core.process_with_quality(x, x, RenderQuality::Offline);
    }
    for _ in 0..4410 {
        core.process_with_quality(3e-4, 3e-4, RenderQuality::Offline);
    }
    let held_gain = core.matched_gain;
    for frame in 0..4410 {
        let x = 3e-4 * (2.0 * PI * 110.0 * frame as f64 / RATE).sin();
        core.process_with_quality(x, x, RenderQuality::Offline);
    }
    assert!(
        (core.matched_gain - held_gain).abs() < 1e-6,
        "matched_gain drifted during -70 dBFS quiet tail: {held_gain} -> {}",
        core.matched_gain
    );
}

#[test]
fn live_and_offline_match_musically() {
    let bass = |frame: usize| {
        let t = frame as f64 / RATE;
        0.25 * (2.0 * PI * 65.0 * t).sin() + 0.12 * (2.0 * PI * 195.0 * t).sin()
    };
    for shape in DistortionShape::ALL {
        let s = Emberform {
            drive_db: 18.0,
            asymmetry_percent: 25.0,
            level_match: true,
            ..settings(shape)
        };
        let live = render(&s, RenderQuality::Live, 44_100, bass);
        let offline = render(&s, RenderQuality::Offline, 44_100, bass);
        let delta_db = 20.0 * (rms(&live[22_050..]) / rms(&offline[22_050..])).log10();
        assert!(
            delta_db.abs() <= 0.5,
            "{shape:?} Live vs Offline RMS delta {delta_db:.2} dB > 0.5 dB"
        );
    }
}

#[test]
fn bypass_and_a_shape_change_stay_quiet() {
    let tone = |frame: usize| 0.3 * (2.0 * PI * 110.0 * frame as f64 / RATE).sin();
    let mut core = Core::new();
    core.set(&settings(DistortionShape::Tube));
    core.settle();
    let mut largest_step: f64 = 0.0;
    let mut previous = 0.0;
    for frame in 0..20_000 {
        if frame == 10_000 {
            core.set(&settings(DistortionShape::Foldback));
        }
        let (l, _) = core.process(tone(frame), tone(frame));
        if frame > 5000 {
            largest_step = largest_step.max((l - previous).abs());
        }
        previous = l;
    }
    assert!(largest_step < 0.2, "{largest_step}");
    let out = render(
        &Emberform {
            bypass: true,
            ..settings(DistortionShape::Fuzz)
        },
        RenderQuality::Offline,
        2000,
        tone,
    );
    assert!(out.iter().all(|x| *x == 0.0));
}

#[test]
fn changing_to_bitcrush_fades_the_existing_live_curve() {
    let from = Emberform {
        drive_db: 6.0,
        low_cut_hz: 20.0,
        high_cut_hz: 20_000.0,
        level_match: false,
        bits: 2,
        ..settings(DistortionShape::HardClip)
    };
    let to = Emberform {
        shape: DistortionShape::Bitcrush,
        ..from
    };
    let tone = |frame: usize| 0.18 * (2.0 * PI * 441.0 * frame as f64 / RATE).cos();
    let reference = render(&from, RenderQuality::Live, 1600, tone);
    let target = render(&to, RenderQuality::Live, 1600, tone);
    let mut core = Core::new();
    core.set(&from);
    core.settle();
    let switched: Vec<_> = (0..1600)
        .map(|frame| {
            if frame == 1000 {
                core.set(&to);
            }
            let input = tone(frame);
            core.process_with_quality(input, input, RenderQuality::Live)
                .0
        })
        .collect();
    let largest_delta = |a: &[f64], b: &[f64]| {
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max)
    };
    assert!(largest_delta(&reference[1000..1008], &target[1000..1008]) > 0.05);
    assert!(largest_delta(&reference[1000..1008], &switched[1000..1008]) < 0.01);
    assert!(largest_delta(&reference[1500..1600], &switched[1500..1600]) > 0.05);
}

#[test]
fn a_resting_core_wakes_without_a_click() {
    let tone = |frame: usize| 0.5 * (2.0 * PI * 220.0 * frame as f64 / RATE).sin();
    for shape in DistortionShape::ALL {
        let mut core = Core::new();
        core.set(&Emberform {
            asymmetry_percent: 100.0,
            bits: 2,
            ..settings(shape)
        });
        core.settle();
        for wake in ["new", "cleared"] {
            let peak = (0..2205)
                .map(|_| {
                    let (l, r) = core.process(0.0, 0.0);
                    l.abs().max(r.abs())
                })
                .fold(0.0, f64::max);
            assert!(peak < 1e-9, "{shape:?}, {wake}: {peak}");
            for frame in 0..4410 {
                core.process(tone(frame), tone(frame));
            }
            core.clear();
        }
    }
}
