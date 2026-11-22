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
fn bass_and_treble_controls_change_their_own_tail_bands() {
    let base = Aetherform {
        space_model: SpaceModel::Hall,
        decay_seconds: 2.0,
        late_balance_percent: 100.0,
        ..neutral()
    };
    let measure = |settings, hz| rt60(&burst_curve(&mut reverb(settings), hz, 6.0));
    let bass = Aetherform {
        bass_mult: 0.4,
        ..base
    };
    let treble = Aetherform {
        treble_mult: 0.3,
        ..base
    };
    assert!(measure(&bass, 125.0) < 0.75 * measure(&base, 125.0));
    assert!(measure(&treble, 8000.0) < 0.75 * measure(&base, 8000.0));
    let mid = measure(&base, 1000.0);
    assert!((measure(&bass, 1000.0) - mid).abs() <= 0.3);
    assert!((measure(&treble, 1000.0) - mid).abs() <= 0.3);
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
            now >= 2.0 && now <= core.lines.reach(),
            "queued {queued}: new head asks for {now} frames"
        );
        assert!(before >= 2.0 && before <= core.lines.reach());
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

// Measurements -------------------------------------------------------------

/// The magnitudes of the first half of the transform of `signal`, whose
/// length is a power of two.
fn fft_magnitudes(signal: &[f64]) -> Vec<f64> {
    let n = signal.len();
    assert!(n.is_power_of_two());
    let (mut re, mut im) = (signal.to_vec(), vec![0.0; n]);
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let step = -2.0 * PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (step * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                (re[b], im[b]) = (re[a] - tr, im[a] - ti);
                (re[a], im[a]) = (re[a] + tr, im[a] + ti);
            }
        }
        len *= 2;
    }
    (0..=n / 2).map(|k| re[k].hypot(im[k])).collect()
}

fn blackman_harris(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let x = 2.0 * PI * i as f64 / (n - 1) as f64;
            0.35875 - 0.48829 * x.cos() + 0.14128 * (2.0 * x).cos() - 0.01168 * (3.0 * x).cos()
        })
        .collect()
}

/// The stereo response to an impulse at frame 0.
fn impulse_response(settings: &Aetherform, frames: usize) -> Vec<(f64, f64)> {
    let mut core = reverb(settings);
    (0..frames)
        .map(|i| {
            let x = if i == 0 { 0.5 } else { 0.0 };
            core.process(x, x)
        })
        .collect()
}

/// How flat the spectrum of the late tail is, from 0 to 1: the geometric
/// over the arithmetic mean of the magnitudes from 200 Hz to 6 kHz.
fn late_flatness(response: &[(f64, f64)]) -> f64 {
    let start = (0.3 * RATE) as usize;
    let window = blackman_harris(32_768);
    let mono: Vec<f64> = response[start..start + 32_768]
        .iter()
        .zip(&window)
        .map(|((l, r), w)| 0.5 * (l + r) * w)
        .collect();
    let spectrum = fft_magnitudes(&mono);
    let bin = |hz: f64| (hz / RATE * 32_768.0) as usize;
    let band: Vec<f64> = spectrum[bin(200.0)..bin(6000.0)]
        .iter()
        .map(|m| m + 1e-15)
        .collect();
    let log_mean = band.iter().map(|m| m.ln()).sum::<f64>() / band.len() as f64;
    log_mean.exp() / (band.iter().sum::<f64>() / band.len() as f64)
}

/// The largest correlation of the two sides within one millisecond.
fn iacc(segment: &[(f64, f64)]) -> f64 {
    let energy =
        |side: fn(&(f64, f64)) -> f64| segment.iter().map(|f| side(f).powi(2)).sum::<f64>();
    let denominator = (energy(|f| f.0) * energy(|f| f.1)).sqrt();
    let n = segment.len() as isize;
    (-44_isize..=44)
        .map(|lag| {
            (0..n)
                .filter(|i| (0..n).contains(&(i + lag)))
                .map(|i| segment[i as usize].0 * segment[(i + lag) as usize].1)
                .sum::<f64>()
                .abs()
        })
        .fold(0.0, f64::max)
        / denominator
}

fn space_settings(space_model: SpaceModel) -> Aetherform {
    Aetherform {
        space_model,
        decay_seconds: 2.5,
        size_percent: 75.0,
        mod_depth_percent: 20.0,
        mod_rate_hz: 0.35,
        diffusion_percent: 80.0,
        density_percent: 85.0,
        late_balance_percent: 50.0,
        width_percent: 100.0,
        ..neutral()
    }
}

#[test]
fn offline_spaces_achieve_dense_smooth_late_tails_and_low_iacc() {
    for model in SpaceModel::ALL {
        let response = impulse_response(&space_settings(model), (1.6 * RATE) as usize);
        let peak = response
            .iter()
            .map(|(l, r)| l.abs().max(r.abs()))
            .fold(0.0, f64::max);
        let onset = response
            .iter()
            .position(|(l, r)| l.abs().max(r.abs()) >= 0.01 * peak)
            .unwrap();
        let flatness = late_flatness(&response);
        let early = iacc(&response[onset..onset + (0.1 * RATE) as usize]);
        let late = iacc(&response[(0.3 * RATE) as usize..(1.5 * RATE) as usize]);
        assert!(flatness >= 0.80, "{model:?}: flatness {flatness:.3}");
        assert!(early <= 0.15, "{model:?}: early IACC {early:.3}");
        assert!(late <= 0.1, "{model:?}: late IACC {late:.3}");
    }
}

#[test]
fn offline_predelay_and_12khz_sinc_reads_stay_accurate() {
    // The strongest early reflection moves by the pre-delay, to a frame.
    let peak_at = |pre_delay_ms: f32| {
        let settings = Aetherform {
            pre_delay_ms,
            late_balance_percent: 0.0,
            ..space_settings(SpaceModel::Plate)
        };
        let response = impulse_response(&settings, 600);
        response
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.0.abs().total_cmp(&b.1.0.abs()))
            .unwrap()
            .0 as f64
    };
    let gap = peak_at(2.0) - peak_at(0.0);
    assert!((gap - 0.002 * RATE).abs() <= 1.0, "{gap} frames");
    // The tail's energy at 12 kHz stays level with 1 kHz.
    let settings = Aetherform {
        late_balance_percent: 100.0,
        decay_seconds: 1.0,
        ..space_settings(SpaceModel::Hall)
    };
    let response = impulse_response(&settings, 65_536);
    let mono: Vec<f64> = response.iter().map(|(l, r)| 0.5 * (l + r)).collect();
    let spectrum = fft_magnitudes(&mono);
    let band = |low: f64, high: f64| {
        let bin = |hz: f64| (hz / RATE * 65_536.0) as usize;
        let power: f64 = spectrum[bin(low)..bin(high)].iter().map(|m| m * m).sum();
        10.0 * (power / (bin(high) - bin(low)) as f64).log10()
    };
    let droop = band(11_000.0, 13_000.0) - band(900.0, 1100.0);
    assert!(droop.abs() < 0.5, "{droop:.2} dB at 12 kHz");
}

/// A frame source of one sine, with `amplitude`.
fn shift_sine(hz: f64, amplitude: f64, ratio: f64, quality: RenderQuality) -> Vec<f64> {
    let mut shifter = Shifter::new();
    shifter.tune(ratio, 20_000.0);
    (0..(3.0 * RATE) as usize)
        .map(|i| {
            let x = amplitude * (2.0 * PI * hz * i as f64 / RATE).sin();
            shifter.process_pair([x, 0.5 * x], ratio, quality)[0]
        })
        .collect()
}

/// The frequency of the strongest partial near `hz`, by parabolic
/// interpolation of the magnitudes.
fn partial_near(signal: &[f64], hz: f64) -> f64 {
    let window = blackman_harris(65_536);
    let windowed: Vec<f64> = signal[22_050..22_050 + 65_536]
        .iter()
        .zip(&window)
        .map(|(x, w)| x * w)
        .collect();
    let spectrum = fft_magnitudes(&windowed);
    let bin = |hz: f64| (hz / RATE * 65_536.0) as usize;
    let best = (bin(hz * 0.94)..bin(hz * 1.06))
        .max_by(|a, b| spectrum[*a].total_cmp(&spectrum[*b]))
        .unwrap();
    let (a, b, c) = (spectrum[best - 1], spectrum[best], spectrum[best + 1]);
    (best as f64 + 0.5 * (a - c) / (a - 2.0 * b + c)) * RATE / 65_536.0
}

#[test]
fn phase_vocoder_shimmer_holds_pitch_and_low_am_ripple() {
    for quality in [RenderQuality::Live, RenderQuality::Offline] {
        for semitones in [12.0_f64, 7.0] {
            let ratio = 2f64.powf(semitones / 12.0);
            let shifted = shift_sine(440.0, 0.3, ratio, quality);
            let found = partial_near(&shifted, 440.0 * ratio);
            let cents = 1200.0 * (found / (440.0 * ratio)).log2();
            assert!(
                cents.abs() < 2.0,
                "{quality:?} +{semitones}: {cents:.2} cents"
            );
            let levels: Vec<f64> = shifted[22_050..110_250]
                .as_chunks::<512>()
                .0
                .iter()
                .map(|chunk| (chunk.iter().map(|x| x * x).sum::<f64>() / 512.0).sqrt())
                .collect();
            let high = levels.iter().copied().fold(0.0, f64::max);
            let low = levels.iter().copied().fold(f64::INFINITY, f64::min);
            let ripple = 20.0 * (high / low).log10();
            assert!(ripple < 0.5, "{quality:?} +{semitones}: {ripple:.2} dB");
        }
        // A chord keeps every partial in tune.
        let chord: Vec<f64> = {
            let mut shifter = Shifter::new();
            shifter.tune(2.0, 20_000.0);
            (0..(3.0 * RATE) as usize)
                .map(|i| {
                    let t = i as f64 / RATE;
                    let x: f64 = [261.6256, 311.127, 391.9954]
                        .iter()
                        .map(|hz| 0.1 * (2.0 * PI * hz * t).sin())
                        .sum();
                    shifter.process_pair([x, x], 2.0, quality)[0]
                })
                .collect()
        };
        for hz in [261.6256, 311.127, 391.9954] {
            let cents = 1200.0 * (partial_near(&chord, 2.0 * hz) / (2.0 * hz)).log2();
            assert!(
                cents.abs() < 2.0,
                "{quality:?} chord {hz}: {cents:.2} cents"
            );
        }
        // Eight times the input gives exactly eight times the output.
        let (quiet, loud) = (
            shift_sine(440.0, 0.02, 2.0, quality),
            shift_sine(440.0, 0.16, 2.0, quality),
        );
        assert!(
            quiet
                .iter()
                .zip(&loud)
                .all(|(q, l)| (8.0 * q - l).abs() < 1e-12)
        );
    }
}

#[test]
fn bloom_character_smears_smoothly_without_slapback_spikes() {
    let settings = Aetherform {
        character: ReverbCharacter::Bloom,
        late_balance_percent: 100.0,
        ..space_settings(SpaceModel::Hall)
    };
    let response = impulse_response(&settings, (0.2 * RATE) as usize);
    let window = (0.005 * RATE) as usize;
    let energies: Vec<f64> = response[(0.03 * RATE) as usize..(0.15 * RATE) as usize]
        .chunks(window)
        .map(|chunk| chunk.iter().map(|(l, r)| l * l + r * r).sum::<f64>())
        .collect();
    let mean = energies.iter().sum::<f64>() / energies.len() as f64;
    let peak = energies.iter().copied().fold(0.0, f64::max);
    assert!(peak < 6.0 * mean, "peak {peak:e} against mean {mean:e}");
}

#[test]
fn the_reverb_profile_tracks_space_early_reflections_and_band_decay() {
    let base = space_settings(SpaceModel::Hall);
    let plain = sample_reverb_profile(&base);
    assert_eq!(plain.rt60_bands[1], base.decay_seconds);
    assert!(plain.er_count > 4);
    assert!(
        plain
            .decay_curves
            .iter()
            .flatten()
            .all(|v| (0.0..=1.0).contains(v))
    );
    let shaped = sample_reverb_profile(&Aetherform {
        bass_mult: 1.5,
        treble_mult: 0.5,
        pre_delay_ms: 80.0,
        early_reflections_enabled: false,
        ..base
    });
    assert!(shaped.rt60_bands[0] > shaped.rt60_bands[1]);
    assert!(shaped.rt60_bands[2] < 0.6 * shaped.rt60_bands[1]);
    assert!(shaped.pre_delay_norm > plain.pre_delay_norm);
    assert_eq!(shaped.er_count, 0);
    let bloom = sample_reverb_profile(&Aetherform {
        character: ReverbCharacter::Bloom,
        ..base
    });
    assert!(bloom.decay_curves[1][2] < plain.decay_curves[1][2]);
}

#[test]
fn the_reverb_profile_keeps_a_long_pre_delay_on_its_time_axis() {
    let settings = Aetherform {
        decay_seconds: 0.2,
        pre_delay_ms: 250.0,
        size_percent: 100.0,
        ..space_settings(SpaceModel::Hall)
    };
    let profile = sample_reverb_profile(&settings);
    let seconds = |time: f32| time * profile.horizon_seconds;
    assert!((seconds(profile.pre_delay_norm) - 0.25).abs() < 1e-6);
    assert!((seconds(profile.er_taps[0].0) - 0.2561).abs() < 1e-6);
}

#[test]
fn live_feedback_heads_read_fractional_positions_with_cubic_interpolation() {
    let mut core = reverb(&neutral());
    core.lengths[0].length = 100.5;
    core.lines.push([1.0; LINES]);
    for _ in 1..100 {
        core.lines.push([0.0; LINES]);
    }
    let swing = Swing {
        depth: 0.0,
        vintage: 0.0,
        wander: 0.0,
        waves: [(0.0, 1.0); 3],
    };
    assert_eq!(core.read_heads(&swing, 1.0, RenderQuality::Live)[0], 0.5625);
}

#[test]
fn the_old_shimmer_interval_keeps_its_bandwidth_while_fading() {
    let settings = Aetherform {
        shimmer_enabled: true,
        shimmer_amount_percent: 100.0,
        shimmer_interval_semitones: 7,
        ..neutral()
    };
    let (mut reference, mut changed) = (reverb(&settings), reverb(&settings));
    let switch_at = 4096;
    for i in 0..switch_at + 4096 {
        if i == switch_at {
            changed.set(
                &Aetherform {
                    shimmer_interval_semitones: 24,
                    ..settings
                },
                0.0,
            );
        }
        let input = (2.0 * PI * 9000.0 * i as f64 / RATE).sin();
        for core in [&mut reference, &mut changed] {
            core.shimmer_level.next();
            core.update_shimmer([input; 2], RenderQuality::Live);
        }
        assert_eq!(changed.ratio, reference.ratio);
        assert_eq!(changed.shimmer, reference.shimmer, "frame {i}");
    }
}

#[test]
fn enabling_shimmer_again_does_not_replay_its_old_audio() {
    let settings = Aetherform {
        shimmer_enabled: true,
        shimmer_amount_percent: 100.0,
        ..neutral()
    };
    let mut core = reverb(&settings);
    let disabled = Aetherform {
        shimmer_enabled: false,
        ..settings
    };
    for i in 0..48_000 {
        if i == 4096 {
            core.set(&disabled, 0.0);
        }
        let tail = (2.0 * PI * 440.0 * i as f64 / RATE).sin();
        core.shimmer_level.next();
        core.update_shimmer([tail; 2], RenderQuality::Live);
    }
    assert_eq!(core.shimmer_level.value(), 0.0);
    core.set(&settings, 0.0);
    for i in 0..4096 {
        core.shimmer_level.next();
        core.update_shimmer([0.0; 2], RenderQuality::Live);
        assert_eq!(core.shimmer, [0.0; 2], "frame {i} after restart");
    }
}
