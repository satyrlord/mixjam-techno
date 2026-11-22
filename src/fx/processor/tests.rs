use super::*;
use crate::fx::{Aetherform, Echoform, Emberform, TimeMode};
use crate::master::dsp::TruePeak;

fn plain_delay() -> Echoform {
    Echoform {
        division_l: crate::fx::Division::Quarter,
        division_r: crate::fx::Division::Quarter,
        ping_pong: false,
        width: 100.0,
        low_cut: 20.0,
        high_cut: 20_000.0,
        mod_depth: 0.0,
        duck_amount: 0.0,
        feedback: 80.0,
        output_db: 0.0,
        character: crate::fx::DelayCharacter::Digital,
        ..Echoform::default()
    }
}

fn buses(module: Module) -> FxBuses {
    let mut buses = FxBuses::default();
    buses.buses[0].module = module;
    buses
}

fn feed(processor: &mut ReturnsProcessor, frames: usize, input: impl Fn(usize) -> f32) -> Vec<f32> {
    let mut out = Vec::new();
    for start in (0..frames).step_by(1024) {
        let count = 1024.min(frames - start);
        let mut block = vec![0.0; count * 2];
        let mut sends = vec![0.0; count * 2 * BUSES];
        for i in 0..count {
            sends[i * 2 * BUSES] = input(start + i);
            sends[i * 2 * BUSES + 1] = input(start + i);
        }
        processor.process(&mut block, Some(&sends));
        out.extend(block);
    }
    out
}

#[test]
fn a_module_change_uses_its_new_key_within_the_same_block() {
    let render = |block_size: usize, split: bool| {
        let control = MasterControl::new(&crate::master::MasterChain::default());
        control.set_returns(&buses(Module::AetherformReverb(Aetherform::default())));
        let mut processor = ReturnsProcessor::live(control.clone(), 140.0);
        feed(&mut processor, 2048, |_| 0.1);
        control.set_returns(&buses(Module::EchoformDelay(Echoform {
            mode: TimeMode::Free,
            time_ms_l: 5.0,
            time_ms_r: 5.0,
            feedback: 0.0,
            duck_amount: 100.0,
            duck_key: 1,
            ..plain_delay()
        })));
        let frames = 4096;
        let mut output = Vec::new();
        for start in (0..frames).step_by(block_size) {
            let count = block_size.min(frames - start);
            let mut sends = vec![0.0; count * 2 * BUSES];
            for (frame, feed) in sends.chunks_mut(2 * BUSES).enumerate() {
                let sample = (0.17 * (start + frame) as f32).sin() * 0.1;
                feed[..2].copy_from_slice(&[sample, sample]);
            }
            let audio = vec![1.0; count * 2];
            let keys = crate::ducker::Keys {
                tracks: &[0],
                audio: &audio,
            };
            if split {
                let mut returns = vec![0.0; count * 2 * BUSES];
                processor.split(&mut returns, Some(&sends), keys, None);
                for frame in returns.chunks(2 * BUSES) {
                    output.extend_from_slice(&frame[..2]);
                }
            } else {
                let mut block = vec![0.0; count * 2];
                processor.process_keyed(&mut block, Some(&sends), keys);
                output.extend(block);
            }
        }
        output
    };
    let expected = render(1, false);
    assert!(expected.iter().any(|sample| sample.abs() > 0.001));
    for split in [false, true] {
        for block_size in [256, 1024] {
            assert_eq!(render(block_size, split), expected);
        }
    }
}

#[test]
fn a_ducker_whose_key_names_a_track_follows_that_track() {
    let delay = Echoform {
        duck_amount: 100.0,
        duck_key: 3,
        ..plain_delay()
    };
    let frames = 88_200;
    let mut sends = vec![0.0_f32; frames * 2 * BUSES];
    for (frame, feed) in sends.chunks_mut(2 * BUSES).enumerate() {
        let value = 0.003 * (frame as f32 * 0.05).sin();
        feed[..2].copy_from_slice(&[value, value]);
    }
    let run = |delay: Echoform, key: f32| {
        let mut processor = ReturnsProcessor::new(&buses(Module::EchoformDelay(delay)), 120.0);
        let audio = vec![key; frames * 2];
        let keys = crate::ducker::Keys {
            tracks: &[2],
            audio: &audio,
        };
        let mut block = vec![0.0; frames * 2];
        processor.process_keyed(&mut block, Some(&sends), keys);
        block[frames..]
            .iter()
            .map(|v| f64::from(*v).powi(2))
            .sum::<f64>()
    };
    let quiet = run(delay, 0.0);
    assert!(quiet > 0.0);
    assert!(run(delay, 0.5) < quiet * 0.01);
    let own = Echoform {
        duck_key: 0,
        ..delay
    };
    assert_eq!(run(own, 0.5), run(own, 0.0));
}

#[test]
fn post_limiter_ducker_tracks_dry_transients_within_two_ms() {
    for module in [
        Module::EmberformDrive(Emberform {
            drive_db: 0.0,
            duck_amount_percent: 100.0,
            duck_threshold_db: -24.0,
            duck_release_ms: 80.0,
            duck_key: 1,
            low_cut_hz: 20.0,
            high_cut_hz: 20_000.0,
            ..Emberform::default()
        }),
        Module::EchoformDelay(Echoform {
            mode: TimeMode::Free,
            time_ms_l: 1.0,
            time_ms_r: 1.0,
            feedback: 0.0,
            duck_amount: 100.0,
            duck_threshold_db: -24.0,
            duck_release: 80.0,
            duck_key: 1,
            ..plain_delay()
        }),
    ] {
        let frames = 2048;
        let step_at = 600usize;
        let mut sends = vec![0.0_f32; frames * 2 * BUSES];
        for (i, ch) in sends.chunks_mut(2 * BUSES).enumerate() {
            let tone = 0.25 * (i as f32 * 0.12).sin();
            ch[0] = tone;
            ch[1] = tone;
        }
        let mut key_audio = vec![0.0_f32; frames * 2];
        for i in step_at..frames {
            key_audio[i * 2] = 0.9;
            key_audio[i * 2 + 1] = 0.9;
        }
        let keys = crate::ducker::Keys {
            tracks: &[0],
            audio: &key_audio,
        };
        for quality in [RenderQuality::Live, RenderQuality::Offline] {
            let mut processor = ReturnsProcessor::with_quality(&buses(module), 120.0, quality);
            let lat = processor.latency_samples();
            let mut block = vec![0.0_f32; frames * 2];
            processor.process_keyed(&mut block, Some(&sends), keys);

            let aligned_step = step_at + lat;
            let baseline = block[(aligned_step - 200) * 2..aligned_step * 2]
                .iter()
                .fold(0.0_f32, |m, v| m.max(v.abs()));
            let after_2ms = aligned_step + 88; // 2.0 ms at 44.1 kHz
            let ducked = block[after_2ms * 2..(after_2ms + 64) * 2]
                .iter()
                .fold(0.0_f32, |m, v| m.max(v.abs()));
            assert!(
                baseline > 0.05 && ducked < baseline * 0.25,
                "{quality:?} baseline={baseline} ducked={ducked}"
            );
        }
    }
}

#[test]
fn live_edits_glide_the_ducker_even_with_an_already_loud_key() {
    let control = MasterControl::new(&crate::master::MasterChain::default());
    let drive = Emberform {
        duck_amount_percent: 0.0,
        duck_threshold_db: -24.0,
        duck_key: 1,
        ..Emberform::default()
    };
    let settings = |drive| buses(Module::EmberformDrive(drive));
    control.set_returns(&settings(drive));
    let mut processor = ReturnsProcessor::live(control.clone(), 120.0);
    let meter_after = |processor: &mut ReturnsProcessor, frames: usize| {
        let mut block = vec![0.0; frames * 2];
        let sends = vec![0.25; frames * 2 * BUSES];
        let audio = vec![0.9; frames * 2];
        let keys = crate::ducker::Keys {
            tracks: &[0],
            audio: &audio,
        };
        processor.process_keyed(&mut block, Some(&sends), keys);
        control.fx_meters()[0].ducker_gr_db
    };
    assert_eq!(meter_after(&mut processor, 2048), 0.0);
    let full = Emberform {
        duck_amount_percent: 100.0,
        ..drive
    };
    control.set_returns(&settings(full));
    assert!(meter_after(&mut processor, 1) < 0.5);
    assert!(meter_after(&mut processor, 2048) > 23.0);
    control.set_returns(&settings(Emberform {
        duck_threshold_db: 0.0,
        ..full
    }));
    assert!(meter_after(&mut processor, 1) > 23.0);
    meter_after(&mut processor, 2048);
    assert!((9.0..13.0).contains(&meter_after(&mut processor, 1)));
}

#[test]
fn all_synced_repeats_stay_on_the_grid() {
    let mut processor = ReturnsProcessor::new(&buses(Module::EchoformDelay(plain_delay())), 120.0);
    let lat = processor.latency_samples();
    let out = feed(
        &mut processor,
        70_000 + lat,
        |i| if i == 0 { 0.5 } else { 0.0 },
    );
    for repeat in 1..=3 {
        let expected = repeat * 22_050 + lat;
        let at = (expected - 600..expected + 20)
            .max_by(|&a, &b| out[a * 2].abs().total_cmp(&out[b * 2].abs()))
            .unwrap();
        assert!(
            at.abs_diff(expected) <= 2,
            "repeat {repeat}: {at}, expected {expected}"
        );
    }
}

#[test]
fn a_long_note_delay_does_not_sleep_before_its_first_echo() {
    let settings = Echoform {
        division_l: crate::fx::Division::WholeDotted,
        division_r: crate::fx::Division::WholeDotted,
        ..plain_delay()
    };
    let mut processor = ReturnsProcessor::new(&buses(Module::EchoformDelay(settings)), 120.0);
    let lat = processor.latency_samples();
    let out = feed(&mut processor, 140_000, |i| if i == 0 { 0.5 } else { 0.0 });
    let expected = 132_300 + lat;
    assert!(
        out[(expected - 10) * 2..(expected + 10) * 2]
            .iter()
            .any(|x| x.abs() > 0.05)
    );
}

#[test]
fn bypass_preserves_the_hidden_tail() {
    let control = MasterControl::new(&crate::master::MasterChain::default());
    let settings = Echoform {
        feedback: 99.0,
        ..plain_delay()
    };
    control.set_returns(&buses(Module::EchoformDelay(settings)));
    let mut processor = ReturnsProcessor::live(control.clone(), 120.0);
    feed(&mut processor, 1000, |i| if i < 100 { 0.5 } else { 0.0 });
    control.set_returns(&buses(Module::EchoformDelay(Echoform {
        bypass: true,
        ..settings
    })));
    feed(&mut processor, 3 * 44_100, |_| 0.0);
    control.set_returns(&buses(Module::EchoformDelay(settings)));
    let out = feed(&mut processor, 44_100, |_| 0.0);
    assert!(out.iter().any(|x| x.abs() > 0.001));
}

#[test]
fn a_return_scales_with_its_input_across_a_sleep() {
    let reverb = Aetherform {
        character: crate::fx::ReverbCharacter::Natural,
        decay_seconds: 1.0,
        mod_rate_hz: 0.7,
        mod_depth_percent: 60.0,
        duck_amount_percent: 0.0,
        shimmer_enabled: true,
        ..Aetherform::default()
    };
    let delay = Echoform {
        mode: TimeMode::Free,
        time_ms_l: 120.0,
        time_ms_r: 170.0,
        feedback: 40.0,
        mod_depth: 6.0,
        mod_rate: 0.9,
        ..plain_delay()
    };
    let burst = 44_100;
    let frames = 14 * 44_100;
    let render = |module: Module, gain: f32| {
        let mut processor = ReturnsProcessor::new(&buses(module), 120.0);
        let mut out = Vec::new();
        for start in (0..frames).step_by(1024) {
            let count = 1024.min(frames - start);
            let mut block = vec![0.0; count * 2];
            let sounding = |i: usize| i < burst || i >= frames - burst;
            if (start..start + count).any(sounding) {
                let mut sends = vec![0.0; count * 2 * BUSES];
                for i in (0..count).filter(|i| sounding(start + i)) {
                    let x = gain * 0.01 * (0.031 * (start + i) as f32).sin();
                    sends[i * 2 * BUSES..i * 2 * BUSES + 2].copy_from_slice(&[x, x]);
                }
                processor.process(&mut block, Some(&sends));
            } else {
                processor.process(&mut block, None);
            }
            out.extend(block);
        }
        out
    };
    for module in [
        Module::EchoformDelay(delay),
        Module::AetherformReverb(reverb),
    ] {
        let quiet = render(module, 1.0);
        let loud = render(module, 8.0);
        let second = 2 * (frames - burst)..;
        let signal: f64 = loud[second.clone()]
            .iter()
            .map(|x| f64::from(*x).powi(2))
            .sum();
        let error: f64 = loud[second.clone()]
            .iter()
            .zip(&quiet[second])
            .map(|(l, q)| (f64::from(*l) - 8.0 * f64::from(*q)).powi(2))
            .sum();
        assert!(signal > 0.0);
        assert!(
            error < signal * 1e-8,
            "{} second burst differs by {:.1} dB",
            module.label(),
            10.0 * (error / signal).log10()
        );
    }
}

/// Offline Drive dry and wet arrive at exact 0-sample relative lag, and offline
/// TruePeak safety limiting holds true peak at <= -1.0 dBTP across presets.
#[test]
fn a_drive_return_arrives_with_its_dry_in_offline_and_holds_true_peak_ceiling() {
    let drive = Emberform {
        shape: crate::fx::DistortionShape::HardClip,
        drive_db: 0.0,
        low_cut_hz: 20.0,
        high_cut_hz: 20_000.0,
        ..Emberform::default()
    };
    let mut processor = ReturnsProcessor::new(&buses(Module::EmberformDrive(drive)), 120.0);
    let lat = processor.latency_samples();
    assert_eq!(lat, MAX_OFFLINE_LATENCY);

    let frames = 44_100;
    let noise: Vec<f32> = (0..frames as u32)
        .scan(12_345u32, |seed, _| {
            *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            Some(0.2 * ((*seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0))
        })
        .collect();
    let mut dry_only = vec![0.0_f32; frames * 2];
    for i in 0..frames {
        dry_only[i * 2] = noise[i];
        dry_only[i * 2 + 1] = noise[i];
    }
    let mut empty = ReturnsProcessor::new(&FxBuses::default(), 120.0);
    empty.process(&mut dry_only, None);
    let _ = dry_only;

    // Cross-correlate Drive wet against un-delayed input to verify exact `lat` alignment,
    // AND compare mixed (dry + wet) vs shifted input to confirm 0-sample dry-vs-wet delta.
    let mut mixed = vec![0.0_f32; frames * 2];
    let mut sends = vec![0.0_f32; frames * 2 * BUSES];
    for i in 0..frames {
        mixed[i * 2] = noise[i];
        mixed[i * 2 + 1] = noise[i];
        sends[i * 2 * BUSES] = noise[i];
        sends[i * 2 * BUSES + 1] = noise[i];
    }
    processor.process(&mut mixed, Some(&sends));
    // Subtract delayed dry (`noise[i - lat]`) to isolate Drive return relative to dry!
    let wet_lag = (-8_isize..=8)
        .max_by(|&a, &b| {
            let score = |offset: isize| -> f64 {
                (20_000..30_000)
                    .map(|i| {
                        let wet = f64::from(mixed[i * 2] - noise[i - lat]);
                        let ref_idx = (i as isize - lat as isize - offset) as usize;
                        wet * f64::from(noise[ref_idx])
                    })
                    .sum()
            };
            score(a).total_cmp(&score(b))
        })
        .unwrap();
    assert_eq!(
        wet_lag, 0,
        "Drive return drifted by {wet_lag} samples from dry"
    );

    for (name, _) in Emberform::PRESETS {
        let mut settings = buses(Module::EmberformDrive(Emberform::preset(name).unwrap()));
        if let Module::EmberformDrive(ref mut d) = settings.buses[0].module {
            d.output_db = 12.0;
        }
        settings.buses[0].return_level = 1.0;
        let mut processor = ReturnsProcessor::new(&settings, 120.0);
        let out = feed(&mut processor, 22_050, |i| 0.95 * (i as f32 * 0.37).sin());
        let mut tp = TruePeak::ZERO;
        let mut max_tp = 0.0_f32;
        for frame in out.as_chunks::<2>().0 {
            max_tp = max_tp.max(tp.push(frame[0]));
        }
        let ceiling_1dbtp = 0.891_251_f32;
        assert!(max_tp <= ceiling_1dbtp, "{name}: true peak {max_tp}");
        assert!(max_tp > 0.05, "{name} is silent");
    }
}

#[test]
fn fx_solo_and_live_meters_track_buses_cleanly() {
    let control = MasterControl::new(&crate::master::MasterChain::default());
    let mut cfg = FxBuses::default();
    cfg.buses[0].module = Module::EmberformDrive(Emberform {
        drive_db: 12.0,
        duck_amount_percent: 50.0,
        duck_threshold_db: -24.0,
        output_db: 6.0,
        ..Emberform::default()
    });
    cfg.buses[1].module = Module::EchoformDelay(Echoform {
        mode: TimeMode::Free,
        time_ms_l: 5.0,
        time_ms_r: 5.0,
        ..plain_delay()
    });
    control.set_returns(&cfg);
    let mut processor = ReturnsProcessor::live(control.clone(), 120.0);
    assert_eq!(processor.latency_samples(), 0);

    let frames = 512;
    let mut block = vec![0.4_f32; frames * 2];
    let mut sends = vec![0.0_f32; frames * 2 * BUSES];
    for i in 0..frames {
        let x = 0.5 * (i as f32 * 0.1).sin();
        sends[i * 2 * BUSES..i * 2 * BUSES + 4].copy_from_slice(&[x, x, x, x]);
    }
    processor.process(&mut block, Some(&sends));
    let meters = control.fx_meters();
    assert!(meters[0].tail_active && meters[0].peak_l > 0.01);
    assert!(meters[1].tail_active && meters[1].peak_l > 0.001);
    assert!(!meters[2].tail_active);

    // Solo bus 0: mutes dry mix and bus 1, keeping only bus 0 return.
    control.set_fx_solo(0, true);
    let mut solo_block = vec![0.4_f32; frames * 2];
    let mut zero_sends = vec![0.0_f32; frames * 2 * BUSES];
    // Feed nothing to bus 0 now; compare silent solo against un-soloed dry=0.4.
    processor.process(&mut solo_block, Some(&zero_sends));
    assert!(
        solo_block.iter().all(|s| s.abs() < 0.35),
        "dry leaked through solo"
    );
    let _ = &mut zero_sends;
}

#[test]
fn an_empty_bus_returns_silence() {
    let mut processor = ReturnsProcessor::new(&FxBuses::default(), 120.0);
    let out = feed(&mut processor, 10_000, |_| 0.5);
    assert!(out.iter().all(|v| *v == 0.0));
    assert!(!processor.ringing());
}

#[test]
fn a_synced_echo_arrives_on_the_beat_despite_the_limiter() {
    let delay = Echoform {
        division_l: crate::fx::Division::Quarter,
        division_r: crate::fx::Division::Quarter,
        ping_pong: false,
        low_cut: 20.0,
        high_cut: 20_000.0,
        mod_depth: 0.0,
        duck_amount: 0.0,
        feedback: 0.0,
        character: crate::fx::DelayCharacter::Digital,
        ..Echoform::default()
    };
    let mut processor = ReturnsProcessor::new(&buses(Module::EchoformDelay(delay)), 125.0);
    let lat = processor.latency_samples();
    let out = feed(&mut processor, 30_000, |i| if i == 0 { 0.5 } else { 0.0 });
    let first = out.chunks(2).position(|f| f[0].abs() > 0.05).unwrap();
    assert!((first as i64 - (21_168 + lat as i64)).abs() <= 2, "{first}");
}

#[test]
fn the_return_level_scales_and_the_limiter_holds_the_ceiling() {
    let reverb = Aetherform {
        output_db: 12.0,
        duck_amount_percent: 0.0,
        ..Aetherform::default()
    };
    let mut settings = buses(Module::AetherformReverb(reverb));
    let loud = |i: usize| 0.9 * (i as f32 * 0.05).sin();
    let mut processor = ReturnsProcessor::new(&settings, 120.0);
    let out = feed(&mut processor, 44_100, loud);
    let peak = out.iter().fold(0.0_f32, |a, v| a.max(v.abs()));
    assert!(peak <= 0.892, "{peak}");
    settings.buses[0].return_level = 0.0;
    let mut processor = ReturnsProcessor::new(&settings, 120.0);
    let out = feed(&mut processor, 10_000, loud);
    assert!(out.iter().all(|v| *v == 0.0));
}

#[test]
fn power_off_stops_new_input_and_the_tail_rings_out_then_sleeps() {
    let delay = Echoform {
        mode: TimeMode::Free,
        time_ms_l: 100.0,
        time_ms_r: 100.0,
        feedback: 50.0,
        duck_amount: 0.0,
        ..Echoform::default()
    };
    let mut settings = buses(Module::EchoformDelay(delay));
    settings.buses[0].powered = false;
    let mut processor = ReturnsProcessor::new(&settings, 120.0);
    let out = feed(&mut processor, 20_000, |_| 0.5);
    assert!(out.iter().all(|v| *v == 0.0));
    settings.buses[0].powered = true;
    let mut processor = ReturnsProcessor::new(&settings, 120.0);
    feed(&mut processor, 2000, |i| if i < 100 { 0.5 } else { 0.0 });
    assert!(processor.ringing());
    feed(&mut processor, 4 * 44_100, |_| 0.0);
    assert!(!processor.ringing());
}

#[test]
fn a_bus_that_sleeps_during_a_cut_wakes_without_it() {
    let drive = FxBus {
        module: Module::EmberformDrive(Emberform::default()),
        ..FxBus::default()
    };
    let reverb = FxBus {
        module: Module::AetherformReverb(Aetherform::default()),
        ..FxBus::default()
    };
    for change in [false, true] {
        let mut bus = Bus::new(RenderQuality::Offline);
        bus.apply(&drive, 120.0, true);
        for _ in 0..64 {
            bus.process(0.5, 0.5, None, None, 120.0);
        }
        let max_quiet = SLEEP_AFTER + bus.total_wet_latency();
        while bus.awake && bus.quiet < max_quiet {
            bus.process(0.0, 0.0, None, None, 120.0);
        }
        assert!(bus.awake);
        if change {
            bus.apply(&reverb, 120.0, false);
        } else {
            bus.clear_tail();
        }
        assert_eq!(bus.cut, CUT);
        bus.process(0.0, 0.0, None, None, 120.0);
        assert!(!bus.awake);
        assert_eq!(bus.cut, 0);
        assert!(bus.pending.is_none());
        let expected = if change { Kind::Reverb } else { Kind::Drive };
        assert!(bus.kind == expected);
    }
}

#[test]
fn clear_tail_and_a_change_of_effect_cut_the_tail_quickly() {
    let control = MasterControl::new(&crate::master::MasterChain::default());
    let reverb = Aetherform {
        decay_seconds: 20.0,
        ..Aetherform::default()
    };
    control.set_returns(&buses(Module::AetherformReverb(reverb)));
    let mut processor = ReturnsProcessor::live(control.clone(), 120.0);
    feed(&mut processor, 8000, |i| if i < 4000 { 0.5 } else { 0.0 });
    control.clear_tail(0);
    let out = feed(&mut processor, 4096, |_| 0.0);
    let after = out[2 * 1500..].iter().fold(0.0_f32, |a, v| a.max(v.abs()));
    assert_eq!(after, 0.0);
    feed(&mut processor, 8000, |i| if i < 4000 { 0.5 } else { 0.0 });
    control.set_returns(&FxBuses::default());
    let out = feed(&mut processor, 4096, |_| 0.0);
    let after = out[2 * 1500..].iter().fold(0.0_f32, |a, v| a.max(v.abs()));
    assert_eq!(after, 0.0);
}

#[test]
fn clear_tail_and_effect_changes_fade_to_zero_without_a_jump() {
    for swap in [false, true] {
        let control = MasterControl::new(&crate::master::MasterChain::default());
        let reverb = Aetherform {
            decay_seconds: 20.0,
            ..Aetherform::default()
        };
        control.set_returns(&buses(Module::AetherformReverb(reverb)));
        let mut processor = ReturnsProcessor::live(control.clone(), 120.0);
        feed(&mut processor, 8000, |i| {
            if i < 4000 {
                (i as f32 * 0.05).sin() * 0.8
            } else {
                0.0
            }
        });
        if swap {
            control.set_returns(&FxBuses::default());
        } else {
            control.clear_tail(0);
        }
        let out = feed(&mut processor, 1024, |_| 0.0);
        let last_non_zero = out
            .as_chunks::<2>()
            .0
            .iter()
            .rposition(|frame| frame[0] != 0.0 || frame[1] != 0.0)
            .expect("the tail must fade out over the cut window");
        let end_peak = out[last_non_zero * 2]
            .abs()
            .max(out[last_non_zero * 2 + 1].abs());
        assert!(end_peak < 0.01, "swap={swap}: {end_peak}");
    }
}
