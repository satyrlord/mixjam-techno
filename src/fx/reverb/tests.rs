//! Tests of the reverb: its lines, retiming, and tail.
use super::*;

fn neutral() -> Aetherform {
    Aetherform {
        character: ReverbCharacter::Natural,
        pre_delay_ms: 0.0,
        mod_depth_percent: 0.0,
        duck_amount_percent: 0.0,
        output_db: 0.0,
        low_cut_hz: 20.0,
        high_cut_hz: 20_000.0,
        ..Aetherform::default()
    }
}

fn reverb(settings: &Aetherform) -> Core {
    let mut core = Core::new();
    core.set(settings, 0.0);
    core.settle();
    core
}

/// Energy in 50 ms windows after an impulse, in dB.
fn decay_curve(core: &mut Core, seconds: f64) -> Vec<f64> {
    let window = 2205;
    let frames = (seconds * RATE) as usize;
    let mut curve = Vec::new();
    let mut sum = 0.0;
    for i in 0..frames {
        let x = if i == 0 { 1.0 } else { 0.0 };
        let (l, r) = core.process(x, x);
        assert!(l.is_finite() && r.is_finite());
        sum += l * l + r * r;
        if (i + 1) % window == 0 {
            curve.push(10.0 * (sum + 1e-30).log10());
            sum = 0.0;
        }
    }
    curve
}

/// The time to fall 60 dB, from the slope between -5 and -25 dB below
/// the curve's peak.
fn rt60(curve: &[f64]) -> f64 {
    let peak = curve.iter().cloned().fold(f64::MIN, f64::max);
    let at = |drop: f64| curve.iter().rposition(|v| *v >= peak - drop).unwrap() as f64;
    let (a, b) = (at(5.0), at(25.0));
    (b - a) * 0.05 * 60.0 / 20.0
}

#[test]
fn the_tail_falls_60_db_in_the_decay_time() {
    for (model, decay) in [
        (SpaceModel::Room, 0.8),
        (SpaceModel::Chamber, 2.8),
        (SpaceModel::Hall, 6.0),
        (SpaceModel::Plate, 1.9),
    ] {
        let settings = Aetherform {
            space_model: model,
            decay_seconds: decay,
            late_balance_percent: 100.0,
            ..neutral()
        };
        let mut core = reverb(&settings);
        let decay = f64::from(decay);
        let measured = rt60(&decay_curve(&mut core, decay * 1.6 + 0.5));
        assert!(
            (measured / decay - 1.0).abs() < 0.25,
            "{model:?}: {measured:.2} s for {decay} s"
        );
    }
}

/// Energy in 50 ms windows during and after a 2 s sine of `hz`, in dB.
fn burst_curve(core: &mut Core, hz: f64, seconds: f64) -> Vec<f64> {
    let window = 2205;
    let burst = (2.0 * RATE) as usize;
    let mut curve = Vec::new();
    let mut sum = 0.0;
    for i in 0..(seconds * RATE) as usize {
        let x = if i < burst {
            0.1 * (2.0 * PI * hz * i as f64 / RATE).sin()
        } else {
            0.0
        };
        let (l, r) = core.process(x, x);
        sum += l * l + r * r;
        if (i + 1) % window == 0 {
            curve.push(10.0 * (sum + 1e-30).log10());
            sum = 0.0;
        }
    }
    curve
}

#[test]
fn the_cuts_keep_the_decay_inside_the_band() {
    // A low and high cut inside the loop once cut on every circulation:
    // a 7.5 s hall cut at 260 Hz died in 0.5 s at 110 Hz.
    for character in [ReverbCharacter::Natural, ReverbCharacter::Vintage] {
        for hz in [110.0, 440.0, 1760.0] {
            let settings = Aetherform {
                space_model: SpaceModel::Hall,
                decay_seconds: 6.0,
                late_balance_percent: 100.0,
                low_cut_hz: 260.0,
                high_cut_hz: 4800.0,
                character,
                ..neutral()
            };
            let measured = rt60(&burst_curve(&mut reverb(&settings), hz, 12.5));
            assert!(
                (measured / 6.0 - 1.0).abs() < 0.25,
                "{character:?} at {hz} Hz: {measured:.2} s for 6 s"
            );
        }
    }
}

#[test]
fn the_high_cut_shortens_the_decay_above_it() {
    let settings = Aetherform {
        space_model: SpaceModel::Hall,
        decay_seconds: 6.0,
        late_balance_percent: 100.0,
        high_cut_hz: 4800.0,
        ..neutral()
    };
    let low = rt60(&burst_curve(&mut reverb(&settings), 1000.0, 12.5));
    let high = rt60(&burst_curve(&mut reverb(&settings), 4800.0, 12.5));
    assert!(
        high < 0.8 * low && high > 0.4 * low,
        "{high:.2} s at the high cut, {low:.2} s at 1 kHz"
    );
}

#[test]
fn size_does_not_change_the_decay_and_models_differ() {
    let measure = |size: f32, model| {
        let settings = Aetherform {
            space_model: model,
            size_percent: size,
            decay_seconds: 2.0,
            late_balance_percent: 100.0,
            ..neutral()
        };
        rt60(&decay_curve(&mut reverb(&settings), 3.5))
    };
    let (small, large) = (
        measure(10.0, SpaceModel::Hall),
        measure(100.0, SpaceModel::Hall),
    );
    assert!((small / large - 1.0).abs() < 0.25, "{small} vs {large}");
    // The first late echo comes later in a hall than in a room.
    let first = |model| {
        let settings = Aetherform {
            space_model: model,
            late_balance_percent: 100.0,
            diffusion_percent: 0.0,
            ..neutral()
        };
        let mut core = reverb(&settings);
        (0..20_000)
            .position(|i| core.process(if i == 0 { 1.0 } else { 0.0 }, 0.0).0.abs() > 1e-4)
            .unwrap()
    };
    assert!(first(SpaceModel::Hall) > first(SpaceModel::Room));
}

#[test]
fn pre_delay_holds_back_the_sound() {
    let onset = |pre_delay_ms: f32| {
        let settings = Aetherform {
            pre_delay_ms,
            late_balance_percent: 0.0,
            ..neutral()
        };
        let mut core = reverb(&settings);
        (0..30_000)
            .position(|i| core.process(if i == 0 { 1.0 } else { 0.0 }, 0.0).0.abs() > 1e-4)
            .unwrap()
    };
    let (none, long) = (onset(0.0), onset(120.0));
    let gap = long - none;
    assert!((gap as f64 - 0.12 * RATE).abs() < 3.0, "{gap}");
}

#[test]
fn extremes_stay_finite_and_bounded() {
    let settings = Aetherform {
        decay_seconds: 30.0,
        shimmer_enabled: true,
        shimmer_amount_percent: 100.0,
        shimmer_interval_semitones: 24,
        drive_percent: 100.0,
        width_percent: 200.0,
        mod_depth_percent: 100.0,
        character: ReverbCharacter::Bloom,
        ..Aetherform::default()
    };
    let mut core = reverb(&settings);
    let mut peak: f64 = 0.0;
    for i in 0..10 * 44_100 {
        let x = if i < 44_100 {
            0.9 * (i as f64 * 0.03).sin()
        } else {
            0.0
        };
        let (l, r) = core.process(x, -x);
        assert!(l.is_finite() && r.is_finite());
        peak = peak.max(l.abs()).max(r.abs());
    }
    assert!(peak < 8.0, "{peak}");
}

#[test]
fn the_shimmer_adds_energy_an_octave_up() {
    // A 400 Hz tone; with the shimmer on, 800 Hz grows in the tail.
    let level_at = |settings: &Aetherform, hz: f64| {
        let mut core = reverb(settings);
        let (mut re, mut im) = (0.0, 0.0);
        for i in 0..4 * 44_100 {
            let x = if i < 44_100 {
                0.3 * (2.0 * PI * 400.0 * i as f64 / RATE).sin()
            } else {
                0.0
            };
            let (l, _) = core.process(x, x);
            if i >= 44_100 + 4410 {
                let angle = 2.0 * PI * hz * i as f64 / RATE;
                re += l * angle.cos();
                im += l * angle.sin();
            }
        }
        re.hypot(im)
    };
    let plain = Aetherform {
        late_balance_percent: 100.0,
        decay_seconds: 4.0,
        ..neutral()
    };
    let shimmering = Aetherform {
        shimmer_enabled: true,
        shimmer_amount_percent: 80.0,
        ..plain
    };
    let without = level_at(&plain, 800.0) / level_at(&plain, 400.0);
    let with = level_at(&shimmering, 800.0) / level_at(&shimmering, 400.0);
    assert!(with > 4.0 * without, "{without} vs {with}");
}

#[test]
fn a_shimmering_hall_dies_away() {
    // A long bloom hall with the shimmer on, as songs set it. With the
    // cuts out of the loop, the shimmer's octave-up return must still
    // pass them, or each pass climbs an octave and the tail runs away.
    for amount in [38.0, 100.0] {
        let settings = Aetherform {
            space_model: SpaceModel::Hall,
            decay_seconds: 9.5,
            character: ReverbCharacter::Bloom,
            low_cut_hz: 340.0,
            high_cut_hz: 7200.0,
            shimmer_enabled: true,
            shimmer_amount_percent: amount,
            ..neutral()
        };
        let mut core = reverb(&settings);
        let mut seed = 1u32;
        let (mut during, mut late) = (0.0, 0.0);
        for i in 0..(30.0 * RATE) as usize {
            let x = if i < (2.0 * RATE) as usize {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                0.3 * (f64::from(seed >> 8) / f64::from(1u32 << 24) - 0.5)
            } else {
                0.0
            };
            let (l, r) = core.process(x, x);
            let energy = l * l + r * r;
            if i < (2.0 * RATE) as usize {
                during += energy;
            } else if i >= (28.0 * RATE) as usize {
                late += energy;
            }
        }
        // 2 s against 2 s, 26 s after the input stopped: far under.
        let drop = 10.0 * (late / during + 1e-30).log10();
        assert!(drop < -40.0, "shimmer {amount} %: {drop:.1} dB");
    }
}

#[test]
fn width_zero_is_mono_and_bypass_keeps_the_tail() {
    let settings = Aetherform {
        width_percent: 0.0,
        ..neutral()
    };
    let mut core = reverb(&settings);
    for i in 0..20_000 {
        let (l, r) = core.process(if i == 0 { 1.0 } else { 0.0 }, 0.0);
        assert!((l - r).abs() < 1e-12);
    }
    let mut core = reverb(&neutral());
    core.process(1.0, 1.0);
    core.set(
        &Aetherform {
            bypass: true,
            ..neutral()
        },
        0.0,
    );
    core.settle();
    for _ in 0..10_000 {
        assert_eq!(core.process(0.0, 0.0), (0.0, 0.0));
    }
    core.set(&neutral(), 0.0);
    core.settle();
    let tail = (0..2000)
        .map(|_| core.process(0.0, 0.0).0.abs())
        .fold(0.0, f64::max);
    assert!(tail > 1e-5, "{tail}");
}

#[test]
fn a_size_change_crossfades_without_a_jump() {
    let mut core = reverb(&neutral());
    let mut last = 0.0;
    let mut worst: f64 = 0.0;
    for i in 0..40_000 {
        if i == 20_000 {
            core.set(
                &Aetherform {
                    size_percent: 20.0,
                    ..neutral()
                },
                0.0,
            );
        }
        let x = 0.3 * (2.0 * PI * 220.0 * i as f64 / RATE).sin();
        let (l, _) = core.process(x, x);
        if i > 19_000 {
            worst = worst.max((l - last).abs());
        }
        last = l;
    }
    // A 220 Hz tone near unit level moves at most about 0.03 per frame.
    assert!(worst < 0.1, "{worst}");
}

#[test]
fn a_small_plate_keeps_distinct_lines_within_its_modulation() {
    let core = reverb(&Aetherform {
        space_model: SpaceModel::Plate,
        size_percent: 0.0,
        mod_depth_percent: 100.0,
        character: ReverbCharacter::Bloom,
        ..neutral()
    });
    let lines = core.positions().lines;
    assert!(lines.windows(2).all(|pair| pair[0] < pair[1]), "{lines:?}");
    assert!(
        core.depth.value() + 2.0 < lines[0],
        "{}",
        core.depth.value()
    );
}

#[test]
fn a_larger_space_restores_its_modulation_after_a_retime() {
    let small = Aetherform {
        space_model: SpaceModel::Plate,
        size_percent: 0.0,
        mod_depth_percent: 100.0,
        character: ReverbCharacter::Bloom,
        ..neutral()
    };
    let large = Aetherform {
        space_model: SpaceModel::Hall,
        size_percent: 100.0,
        ..small
    };
    let expected = reverb(&large).depth.value();
    for queued in [false, true] {
        for settle in [false, true] {
            let mut core = reverb(&small);
            if queued {
                core.set(
                    &Aetherform {
                        space_model: SpaceModel::Room,
                        size_percent: 30.0,
                        ..small
                    },
                    0.0,
                );
                for _ in 0..FADE / 2 {
                    core.process(0.0, 0.0);
                }
            }
            core.set(&large, 0.0);
            if settle {
                core.settle();
            } else {
                for _ in 0..FADE * 6 {
                    core.process(0.0, 0.0);
                }
            }
            assert!(core.queued.is_none());
            assert_eq!(core.fade, 0);
            assert!(
                (core.depth.value() - expected).abs() < 1e-6,
                "queued {queued}, settle {settle}: {} instead of {expected}",
                core.depth.value()
            );
        }
    }
}

#[test]
fn a_queued_smaller_space_keeps_its_modulation_within_its_lines() {
    let large = Aetherform {
        space_model: SpaceModel::Hall,
        size_percent: 100.0,
        mod_depth_percent: 100.0,
        character: ReverbCharacter::Bloom,
        ..neutral()
    };
    let small = Aetherform {
        space_model: SpaceModel::Plate,
        size_percent: 0.0,
        ..large
    };
    let mut core = reverb(&large);
    core.set(
        &Aetherform {
            size_percent: 70.0,
            ..large
        },
        0.0,
    );
    for _ in 0..FADE / 2 {
        core.process(0.0, 0.0);
    }
    core.set(&small, 0.0);
    assert!(core.queued.is_some());
    for _ in 0..FADE * 6 {
        core.process(0.0, 0.0);
    }
    let expected = reverb(&small);
    assert!(core.queued.is_none());
    assert_eq!(core.fade, 0);
    assert!(core.positions() == expected.positions());
    assert!((core.depth.value() - expected.depth.value()).abs() < 1e-6);
    assert!(core.depth.value() + 2.0 < core.positions().lines[0]);
}

#[test]
fn a_shrinking_space_bounds_each_read_head_during_the_depth_glide() {
    let large = Aetherform {
        space_model: SpaceModel::Hall,
        size_percent: 100.0,
        mod_depth_percent: 100.0,
        character: ReverbCharacter::Bloom,
        ..neutral()
    };
    let small = Aetherform {
        space_model: SpaceModel::Plate,
        size_percent: 0.0,
        ..large
    };
    for queued in [false, true] {
        let mut core = reverb(&large);
        if queued {
            core.set(
                &Aetherform {
                    size_percent: 99.0,
                    ..large
                },
                0.0,
            );
            for _ in 0..FADE - 1 {
                core.process(0.0, 0.0);
            }
            assert_eq!(core.fade, 1);
        }
        core.set(&small, 0.0);
        if queued {
            assert!(core.queued.is_some());
            core.process(0.0, 0.0);
            assert!(core.queued.is_none());
        }
        assert_eq!(core.fade, FADE);
        // The shortest line's oscillator is at its negative peak. The
        // depth is still gliding down from the larger space at this frame.
        core.lfo = Lfo::default();
        core.lfo.skip(1.0, (0.75 * RATE) as u64);
        let phase = core.lfo.value(0.0);
        let depth = core.depth.next();
        let length = core.lengths[0];
        assert!(depth > length.length);
        let vintage = core.vintage.next();
        let wander = core.wander.value(0.0);
        let now = modulated_length(length.length, depth, phase, vintage, wander);
        let before = modulated_length(length.previous, depth, phase, vintage, wander);
        assert!(
            now >= 2.0 && now <= core.lines[0].reach(),
            "queued {queued}: new head asks for {now} frames"
        );
        assert!(before >= 2.0 && before <= core.lines[0].reach());
        assert!((before - (length.previous - depth)).abs() < 1e-9);
    }
}

#[test]
fn a_pre_delay_retime_keeps_unchanged_current_lines_and_taps() {
    let small = Aetherform {
        space_model: SpaceModel::Plate,
        size_percent: 0.0,
        ..neutral()
    };
    let large = Aetherform {
        space_model: SpaceModel::Hall,
        size_percent: 100.0,
        ..small
    };
    let delayed = Aetherform {
        pre_delay_ms: 50.0,
        ..large
    };
    for queued in [false, true] {
        let mut core = reverb(&small);
        core.set(&large, 0.0);
        let current = core.positions();
        let before = if queued { FADE / 2 } else { FADE };
        for _ in 0..before {
            core.process(0.0, 0.0);
        }
        core.set(&delayed, 0.0);
        if queued {
            for _ in before..FADE {
                core.process(0.0, 0.0);
            }
        }
        assert_eq!(core.fade, FADE);
        assert!(core.queued.is_none());
        assert_eq!(core.lengths.map(|length| length.previous), current.lines);
        assert!(core.previous_taps == current.taps);
    }
}

#[test]
fn a_size_retime_keeps_unchanged_current_pre_delay() {
    let initial = Aetherform {
        space_model: SpaceModel::Hall,
        size_percent: 100.0,
        pre_delay_ms: 10.0,
        ..neutral()
    };
    let delayed = Aetherform {
        pre_delay_ms: 50.0,
        ..initial
    };
    let resized = Aetherform {
        size_percent: 80.0,
        ..delayed
    };
    for queued in [false, true] {
        let mut core = reverb(&initial);
        core.set(&delayed, 0.0);
        let current = core.positions();
        let before = if queued { FADE / 2 } else { FADE };
        for _ in 0..before {
            core.process(0.0, 0.0);
        }
        core.set(&resized, 0.0);
        if queued {
            for _ in before..FADE {
                core.process(0.0, 0.0);
            }
        }
        assert_eq!(core.fade, FADE);
        assert!(core.queued.is_none());
        assert_eq!(core.pre_time.previous, current.pre);
    }
}

#[test]
fn a_retime_during_a_crossfade_waits_for_it_and_never_jumps() {
    let mut core = reverb(&neutral());
    let mut last = 0.0;
    let mut worst: f64 = 0.0;
    for i in 0..40_000 {
        // A second size arrives halfway through the first crossfade.
        for (at, size) in [(20_000, 20.0), (20_000 + FADE / 2, 90.0)] {
            if i == at {
                core.set(
                    &Aetherform {
                        size_percent: size,
                        ..neutral()
                    },
                    0.0,
                );
            }
        }
        let x = 0.3 * (2.0 * PI * 220.0 * i as f64 / RATE).sin();
        let (l, _) = core.process(x, x);
        if i > 19_000 {
            worst = worst.max((l - last).abs());
        }
        last = l;
    }
    assert!(worst < 0.1, "{worst}");
    assert!(core.queued.is_none());
    assert_eq!(core.fade, 0);
    let expected = reverb(&Aetherform {
        size_percent: 90.0,
        ..neutral()
    });
    assert!(core.positions() == expected.positions());
}
