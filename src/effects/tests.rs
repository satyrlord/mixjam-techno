use super::*;
use crate::audio::Pcm;
use std::f64::consts::TAU;

fn sine_pcm(hz: f64, frames: usize, sample_rate: u32, channels: u16, amp: f32) -> Pcm {
    let mut samples = Vec::with_capacity(frames * usize::from(channels));
    for i in 0..frames {
        let v = amp * (TAU * hz * i as f64 / f64::from(sample_rate)).sin() as f32;
        for _ in 0..channels {
            samples.push(v);
        }
    }
    Pcm {
        sample_rate,
        channels,
        samples,
    }
}

fn impulse_pcm(frames: usize, sample_rate: u32, channels: u16, amp: f32) -> Pcm {
    let mut samples = vec![0.0_f32; frames * usize::from(channels)];
    samples[..usize::from(channels)].fill(amp);
    Pcm {
        sample_rate,
        channels,
        samples,
    }
}

fn rms(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    (sum_sq / samples.len() as f64).sqrt()
}

fn windowed_tone_mag(samples: &[f32], freq_hz: f64, sample_rate: f64) -> f64 {
    let n = samples.len().max(1);
    let mut re = 0.0_f64;
    let mut im = 0.0_f64;
    let mut w_sum = 0.0_f64;
    for (i, &x) in samples.iter().enumerate() {
        let w = 0.5 * (1.0 - (TAU * i as f64 / n as f64).cos());
        let ph = TAU * freq_hz * i as f64 / sample_rate;
        re += f64::from(x) * w * ph.cos();
        im += f64::from(x) * w * ph.sin();
        w_sum += w;
    }
    2.0 * (re * re + im * im).sqrt() / w_sum.max(1.0)
}

fn channel_slice(pcm: &Pcm, ch: usize) -> Vec<f32> {
    let n = usize::from(pcm.channels);
    pcm.samples.iter().skip(ch).step_by(n).copied().collect()
}

#[test]
fn default_state_is_bypassed_and_preserves_audio() {
    let state = EffectsState::default();
    assert!(state.is_bypassed());
    assert!(state.validate().is_ok());
    let input = sine_pcm(440.0, 1024, 44_100, 1, 0.5);
    let out = process(&input, &state, 128.0).expect("bypass process");
    assert_eq!(out.channels, 1);
    assert_eq!(out.samples, input.samples);
}

#[test]
fn zero_wet_keeps_full_scale_pcm_dry() {
    let mut state = EffectsState::default();
    state.filter.enabled = true;
    state.filter.wet = 0.0;
    state.modulation.enabled = true;
    state.modulation.wet = 0.0;
    state.echo.enabled = true;
    state.echo.wet = 0.0;
    state.robotize.enabled = true;
    state.robotize.wet = 0.0;
    state.reverb.enabled = true;
    state.reverb.wet = 0.0;
    let input = impulse_pcm(1024, 44_100, 1, 1.0);
    let out = process(&input, &state, 120.0).unwrap();
    assert_eq!(out.samples, input.samples);
    assert_eq!(out.channels, 1);
}

#[test]
fn legacy_high_cutoff_survives_pattern_load_and_save() {
    let mut legacy = serde_json::to_value(crate::synth::StepSynthState::default()).unwrap();
    legacy["effects"]["filter"] = serde_json::json!({
        "cutoff_hz": 180000.0, "resonance": 0.2, "drive": 0.0, "wet": 0.75
    });
    let pattern: crate::synth::StepSynthState = serde_json::from_value(legacy).unwrap();
    pattern.validate().unwrap();
    assert_eq!(pattern.effects.filter.cutoff_hz, 180_000.0);
    let saved = serde_json::to_value(&pattern).unwrap();
    let reopened: crate::synth::StepSynthState = serde_json::from_value(saved).unwrap();
    assert_eq!(reopened, pattern);
}

#[test]
fn pattern_settings_omit_defaults_and_keep_disabled_edits() {
    let mut state = EffectsState::default();
    let default_json = serde_json::to_value(&state).unwrap();
    assert_eq!(
        default_json,
        serde_json::json!({"dirty": 0.0, "noise": 0.0, "volume": 1.0})
    );
    state.echo.enabled = true;
    state.filter.cutoff_hz = 900.0;
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json["filter"]["enabled"], false);
    assert!(json["echo"].get("ping_pong").is_none());
    assert!(json["echo"].get("tone").is_none());
    assert_eq!(serde_json::from_value::<EffectsState>(json).unwrap(), state);
}

#[test]
fn every_bay_accepts_the_lowest_supported_sample_rate() {
    let mut state = EffectsState::default();
    state.filter.enabled = true;
    state.modulation.enabled = true;
    state.echo.enabled = true;
    state.echo.tone = 0.5;
    state.robotize.enabled = true;
    state.reverb.enabled = true;
    state.dirty = 0.5;
    let input = impulse_pcm(16, 1, 1, 0.5);
    for mode in [ModulationMode::Chorus, ModulationMode::Phaser] {
        state.modulation.mode = mode;
        process(&input, &state, 120.0).unwrap();
    }
}

#[test]
fn short_loops_prime_the_complete_oversampling_filter() {
    let mut short = [0.2, 0.4, -0.1];
    let mut repeated = short.repeat(128);
    run_oversampled_16x(&mut short, true, |x| x * x);
    run_oversampled_16x(&mut repeated, true, |x| x * x);
    for (a, b) in short.iter().zip(&repeated[192..]) {
        assert!((a - b).abs() < 1e-7, "{a} != {b}");
    }
}

#[test]
fn stereo_expansion_respects_the_wav_size_limit_before_processing() {
    let max_stereo_frames = crate::audio::MAX_WAV_SAMPLES / 2;
    assert_eq!(
        render_frame_limit(max_stereo_frames, 2, 44_100.0),
        Ok(max_stereo_frames)
    );
    assert!(render_frame_limit(max_stereo_frames + 1, 2, 44_100.0).is_err());
    assert!(render_frame_limit(max_stereo_frames + 1, 1, 44_100.0).is_ok());
}

#[test]
fn non_destructive_bypass_keeps_dials_and_passes_dry() {
    let custom_filter = Filter {
        enabled: false,
        mode: FilterMode::Lp24,
        cutoff_hz: 640.0,
        resonance: 0.7,
        drive: 0.5,
        wet: 0.85,
    };
    let mut state = EffectsState {
        filter: custom_filter,
        modulation: Modulation {
            enabled: false,
            mode: ModulationMode::Phaser,
            rate_hz: 2.2,
            depth: 0.8,
            feedback: 0.5,
            wet: 0.6,
        },
        echo: Echo {
            enabled: false,
            delay_beats: 0.75,
            feedback: 0.6,
            tone: 0.4,
            ping_pong: true,
            wet: 0.5,
        },
        robotize: Robotize {
            enabled: false,
            mode: RobotizeMode::FreqShift,
            frequency_hz: 310.0,
            wet: 0.7,
        },
        reverb: Reverb {
            enabled: false,
            space: ReverbSpace::Hall,
            decay_s: 3.4,
            damping: 0.65,
            wet: 0.4,
        },
        ..EffectsState::default()
    };
    assert!(state.is_bypassed());
    let input = sine_pcm(523.25, 1024, 44_100, 1, 0.4);
    let out = process(&input, &state, 120.0).expect("disabled bays pass dry");
    assert_eq!(out.samples, input.samples);
    state.filter.enabled = true;
    assert_eq!(state.filter.cutoff_hz, 640.0);
    assert_eq!(state.filter.mode, FilterMode::Lp24);
}

#[test]
fn deserializes_legacy_null_and_active_bays() {
    let legacy_null = r#"{
        "filter": null,
        "echo": null,
        "robotize": null,
        "dirty": 0.2,
        "noise": 0.0,
        "volume": 1.0
    }"#;
    let parsed: EffectsState = serde_json::from_str(legacy_null).expect("legacy null json");
    assert!(!parsed.filter.enabled);
    assert!(!parsed.echo.enabled);
    assert!(!parsed.robotize.enabled);
    assert!(!parsed.modulation.enabled);
    assert!(!parsed.reverb.enabled);
    assert_eq!(parsed.dist_shape, DistortionShape::Warm);
    assert_eq!(parsed.tail_mode, TailMode::Trim);

    let legacy_active = r#"{
        "filter": {"cutoff_hz": 1200.0, "resonance": 0.4, "drive": 0.2, "wet": 0.8},
        "echo": {"delay_beats": 0.001, "feedback": 0.35, "wet": 0.4},
        "robotize": {"frequency_hz": 220.0, "wet": 0.5},
        "dirty": 0.0,
        "noise": 0.0,
        "volume": 1.0
    }"#;
    let parsed_active: EffectsState =
        serde_json::from_str(legacy_active).expect("legacy active json");
    assert!(parsed_active.filter.enabled);
    assert_eq!(parsed_active.filter.mode, FilterMode::Lp12);
    assert_eq!(parsed_active.filter.cutoff_hz, 1200.0);
    assert!(parsed_active.echo.enabled);
    assert_eq!(parsed_active.echo.delay_beats, MIN_ECHO_BEATS);
    assert!(!parsed_active.echo.ping_pong);
    assert_eq!(parsed_active.echo.tone, 0.0);
    assert!(parsed_active.robotize.enabled);
    assert_eq!(parsed_active.robotize.mode, RobotizeMode::RingMod);
}

#[test]
fn rejects_unknown_fields_and_invalid_ranges() {
    let mut unknown = serde_json::to_value(EffectsState::default()).unwrap();
    unknown["extra_field"] = serde_json::json!(1);
    assert!(serde_json::from_value::<EffectsState>(unknown).is_err());
    let mut tiny_echo = serde_json::to_value(EffectsState::default()).unwrap();
    tiny_echo["echo"] = serde_json::json!({"delay_beats": 0.0005, "feedback": 0.2, "wet": 0.5});
    let parsed: EffectsState = serde_json::from_value(tiny_echo).unwrap();
    assert!(parsed.validate().is_err());

    let mut bad = EffectsState::default();
    bad.filter.cutoff_hz = 10.0;
    assert!(bad.validate().is_err());
    bad = EffectsState::default();
    bad.filter.cutoff_hz = 180_001.0;
    assert!(bad.validate().is_err());
    bad = EffectsState::default();
    bad.echo.feedback = 0.99;
    assert!(bad.validate().is_err());
    bad = EffectsState::default();
    bad.reverb.decay_s = 12.0;
    assert!(bad.validate().is_err());
    bad = EffectsState::default();
    bad.modulation.rate_hz = 25.0;
    assert!(bad.validate().is_err());
    bad = EffectsState::default();
    bad.robotize.frequency_hz = 0.5;
    assert!(bad.validate().is_err());

    let input = sine_pcm(440.0, 256, 44_100, 1, 0.3);
    assert!(process(&input, &EffectsState::default(), 0.0).is_err());
}

#[test]
fn filter_decramped_at_18khz_and_lp24_slope() {
    let lp12_18k = EffectsState {
        filter: Filter {
            enabled: true,
            mode: FilterMode::Lp12,
            cutoff_hz: 18_000.0,
            resonance: 0.0,
            drive: 0.0,
            wet: 1.0,
        },
        ..EffectsState::default()
    };
    let input = sine_pcm(18_000.0, 22_050, 44_100, 1, 0.5);
    let out = process(&input, &lp12_18k, 128.0).expect("18khz lp12");
    let mag_ratio = windowed_tone_mag(&out.samples[4096..], 18_000.0, 44_100.0)
        / windowed_tone_mag(&input.samples[4096..], 18_000.0, 44_100.0);
    let gain_db = 20.0 * mag_ratio.log10();
    assert!(
        (gain_db + 3.01).abs() <= 0.15,
        "18 kHz @ 44.1k Butterworth expected -3.01 dB ±0.15 dB, got {gain_db:.3} dB"
    );

    let lp24_500 = EffectsState {
        filter: Filter {
            enabled: true,
            mode: FilterMode::Lp24,
            cutoff_hz: 500.0,
            resonance: 0.0,
            drive: 0.0,
            wet: 1.0,
        },
        ..EffectsState::default()
    };
    let two_oct = sine_pcm(2_000.0, 22_050, 44_100, 1, 0.5);
    let out24 = process(&two_oct, &lp24_500, 128.0).expect("lp24 attenuation");
    let slope_db = 20.0
        * (windowed_tone_mag(&out24.samples[4096..], 2_000.0, 44_100.0)
            / windowed_tone_mag(&two_oct.samples[4096..], 2_000.0, 44_100.0))
        .max(1e-6)
        .log10();
    assert!(
        slope_db <= -44.0,
        "LP24 +2 octaves must attenuate >= 44 dB, got {slope_db:.2} dB"
    );
}

#[test]
fn synth_filter_headroom_preserves_normalized_waveform() {
    let state = EffectsState {
        filter: Filter {
            enabled: true,
            mode: FilterMode::Lp12,
            cutoff_hz: 1_200.0,
            resonance: 0.55,
            drive: 0.0,
            wet: 1.0,
        },
        ..EffectsState::default()
    };
    let quiet = sine_pcm(440.0, 4096, 44_100, 1, 0.125);
    let hot = sine_pcm(440.0, 4096, 44_100, 1, 4.0);
    let (out_q, gain_q) = process_synth(&quiet, &state, 128.0).expect("quiet synth filter");
    let (out_h, gain_h) = process_synth(&hot, &state, 128.0).expect("hot synth filter");
    assert!(gain_q <= 1.0 && gain_h < 0.35);
    let max_diff = out_q
        .samples
        .iter()
        .zip(&out_h.samples)
        .map(|(a, b)| (a * 32.0 * gain_h / gain_q - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(max_diff < 1e-4, "SVF linear scaling drift: {max_diff}");
}

fn max_alias_spur_dbc(samples: &[f32], fund_hz: f64, sr: f64) -> (f64, f64) {
    let fund = windowed_tone_mag(samples, fund_hz, sr).max(1e-9);
    // Aliased harmonics of 4048 Hz @ 44100 Hz (order 6..=10 folded below Nyquist)
    let alias_bins = [19_812.0, 15_764.0, 11_716.0, 7_668.0, 3_620.0];
    alias_bins
        .iter()
        .map(|&hz| {
            (
                hz,
                20.0 * (windowed_tone_mag(samples, hz, sr).max(1e-12) / fund).log10(),
            )
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
}

#[test]
fn oversampled_16x_filter_drive_and_distortion_keep_aliasing_below_90db() {
    let input = sine_pcm(4_048.0, 44_100, 44_100, 1, 0.5);
    let drive_state = EffectsState {
        filter: Filter {
            enabled: true,
            mode: FilterMode::Lp12,
            cutoff_hz: 16_000.0,
            resonance: 0.0,
            drive: 1.0,
            wet: 1.0,
        },
        ..EffectsState::default()
    };
    let drive_out = process(&input, &drive_state, 128.0).expect("filter drive");
    let (drive_bin, drive_spur) = max_alias_spur_dbc(&drive_out.samples[4096..], 4_048.0, 44_100.0);
    assert!(
        drive_spur < -90.0,
        "Filter drive alias spur at {drive_bin} Hz: {drive_spur:.1} dBc"
    );

    for shape in [
        DistortionShape::Warm,
        DistortionShape::Acid,
        DistortionShape::Fuzz,
    ] {
        let state = EffectsState {
            dist_shape: shape,
            dirty: 1.0,
            ..EffectsState::default()
        };
        let out = process(&input, &state, 128.0).expect("dirty distortion");
        let (bin, spur) = max_alias_spur_dbc(&out.samples[4096..], 4_048.0, 44_100.0);
        assert!(
            spur < -90.0,
            "{shape:?} alias spur at {bin} Hz: {spur:.1} dBc"
        );
    }

    let hot_vol = EffectsState {
        dist_shape: DistortionShape::Warm,
        dirty: 1.0,
        volume: 2.0,
        ..EffectsState::default()
    };
    let hot_out = process(&input, &hot_vol, 128.0).expect("warm volume 2.0");
    assert!(hot_out.samples.iter().all(|&x| x.abs() < 0.9999));
    let (v_bin, v_spur) = max_alias_spur_dbc(&hot_out.samples[4096..], 4_048.0, 44_100.0);
    assert!(
        v_spur < -70.0,
        "Warm volume 2.0 alias spur at {v_bin} Hz: {v_spur:.1} dBc"
    );
}

#[test]
fn volume_double_soft_ceiling_avoids_hard_flat_clipping() {
    let state = EffectsState {
        volume: 2.0,
        ..EffectsState::default()
    };
    let hot = sine_pcm(100.0, 2048, 44_100, 1, 0.8);
    process(&hot, &state, 128.0)
        .expect("hot volume")
        .validate()
        .expect("soft ceiling keeps normalized PCM");

    let mid = sine_pcm(100.0, 2048, 44_100, 1, 0.50);
    let knee = sine_pcm(100.0, 2048, 44_100, 1, 0.53);
    let out_mid = process(&mid, &state, 128.0).expect("mid volume");
    let out_knee = process(&knee, &state, 128.0).expect("knee volume");
    let peak_mid = out_mid.samples.iter().copied().fold(0.0_f32, f32::max);
    let peak_knee = out_knee.samples.iter().copied().fold(0.0_f32, f32::max);
    assert!(peak_knee < 1.0 && peak_knee > peak_mid);
    assert!(out_knee.samples.iter().all(|&x| x.abs() < 0.9999));
}

#[test]
fn robotize_ring_mod_and_freq_shift_suppress_aliasing_and_wrap_smoothly() {
    let input = sine_pcm(4_048.0, 44_100, 44_100, 1, 0.5);
    let ring = EffectsState {
        robotize: Robotize {
            enabled: true,
            mode: RobotizeMode::RingMod,
            frequency_hz: 1_000.0,
            wet: 1.0,
        },
        tail_mode: TailMode::WrapLoop,
        ..EffectsState::default()
    };
    let out = process(&input, &ring, 128.0).expect("ring mod");
    let up = windowed_tone_mag(&out.samples[2048..], 5_048.0, 44_100.0);
    let dn = windowed_tone_mag(&out.samples[2048..], 3_048.0, 44_100.0);
    let spur = windowed_tone_mag(&out.samples[2048..], 12_144.0, 44_100.0);
    assert!(up > 0.15 && dn > 0.15);
    assert!(20.0 * (spur.max(1e-12) / up).log10() < -90.0);

    let shift = EffectsState {
        robotize: Robotize {
            enabled: true,
            mode: RobotizeMode::FreqShift,
            frequency_hz: 250.0,
            wet: 1.0,
        },
        ..EffectsState::default()
    };
    let s_in = sine_pcm(1_000.0, 22_050, 44_100, 1, 0.5);
    let s_out = process(&s_in, &shift, 128.0).expect("freq shift");
    let upper = windowed_tone_mag(&s_out.samples[4096..], 1_250.0, 44_100.0);
    let lower = windowed_tone_mag(&s_out.samples[4096..], 750.0, 44_100.0);
    assert!(upper > 0.25 && upper > lower * 8.0);
}

#[test]
fn echo_supports_fractional_ping_pong_tone_and_tail_modes() {
    let impulse = impulse_pcm(22_050, 44_100, 1, 0.9);
    let ping = EffectsState {
        echo: Echo {
            enabled: true,
            delay_beats: MIN_ECHO_BEATS,
            feedback: 0.55,
            tone: 0.7,
            ping_pong: true,
            wet: 0.8,
        },
        tail_mode: TailMode::Trim,
        ..EffectsState::default()
    };
    let trimmed = process(&impulse, &ping, 120.0).expect("trim ping pong");
    assert_eq!(trimmed.frames(), 22_050);
    assert_eq!(trimmed.channels, 2);
    let left = channel_slice(&trimmed, 0);
    let right = channel_slice(&trimmed, 1);
    assert_ne!(left, right);
    // Expected first tap around 0.0625 * 0.5 s * 44100 = 1378 frames.
    assert!(left[1370..1390].iter().any(|x| x.abs() > 0.08));
    assert!(right[2750..2775].iter().any(|x| x.abs() > 0.015));

    let tone_in = sine_pcm(8_000.0, 22_050, 44_100, 1, 0.5);
    let mut echo_bright = ping;
    echo_bright.echo.tone = 0.0;
    echo_bright.echo.wet = 1.0;
    echo_bright.echo.ping_pong = false;
    let mut echo_dark = echo_bright.clone();
    echo_dark.echo.tone = 0.7;
    let out_bright = process(&tone_in, &echo_bright, 120.0).expect("bright echo");
    let out_dark = process(&tone_in, &echo_dark, 120.0).expect("damped echo");
    let mag_bright = windowed_tone_mag(&out_bright.samples[4096..], 8_000.0, 44_100.0);
    let mag_dark = windowed_tone_mag(&out_dark.samples[4096..], 8_000.0, 44_100.0);
    let tone_drop_db = 20.0 * (mag_dark.max(1e-9) / mag_bright.max(1e-9)).log10();
    assert!(
        tone_drop_db <= -5.0,
        "Echo tone=0.7 expected >= 5 dB damping at 8 kHz, got {tone_drop_db:.2} dB"
    );

    let ring = EffectsState {
        echo: Echo {
            enabled: true,
            delay_beats: 0.5,
            feedback: 0.92,
            tone: 0.2,
            ping_pong: false,
            wet: 0.6,
        },
        tail_mode: TailMode::RingOut,
        ..EffectsState::default()
    };
    let extended = process(&impulse, &ring, 120.0).expect("ring out cap");
    assert!(extended.frames() > 22_050);
    assert!(extended.frames() <= 22_050 + 6 * 44_100);
    assert!(extended.samples.last().copied().unwrap_or(1.0).abs() < 1e-4);

    let wrap = EffectsState {
        echo: Echo {
            enabled: true,
            delay_beats: 0.25,
            feedback: 0.5,
            tone: 0.0,
            ping_pong: false,
            wet: 0.5,
        },
        tail_mode: TailMode::WrapLoop,
        ..EffectsState::default()
    };
    let mut loop_in = vec![0.0_f32; 22_050];
    loop_in[18_000] = 0.8;
    let wrapped = process(
        &Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: loop_in,
        },
        &wrap,
        120.0,
    )
    .expect("wrap loop echo");
    assert_eq!(wrapped.frames(), 22_050);
    // Delayed by 0.25 * 0.5 s = 5512.5 frames -> wraps past 22050 into start around 1462.
    assert!(wrapped.samples[1455..1475].iter().any(|x| x.abs() > 0.15));
}

#[test]
fn ring_out_keeps_the_last_repeat_at_a_chunk_boundary() {
    let mut samples = vec![0.0; 477];
    samples[0] = 0.5;
    let source = Pcm {
        samples,
        channels: 1,
        sample_rate: 48000,
    };
    let state = EffectsState {
        echo: Echo {
            enabled: true,
            delay_beats: MIN_ECHO_BEATS,
            feedback: 0.0,
            wet: 1.0,
            ..Echo::default()
        },
        tail_mode: TailMode::RingOut,
        ..EffectsState::default()
    };
    let output = process(&source, &state, 120.0).unwrap();
    assert!((output.samples[1500] - 0.5).abs() < 1e-6);
    assert_eq!(output.samples.last(), Some(&0.0));
}

#[test]
fn modulation_and_reverb_produce_stereo_across_sample_rates() {
    let input = sine_pcm(330.0, 22_050, 48_000, 1, 0.5);
    for mode in [ModulationMode::Chorus, ModulationMode::Phaser] {
        let state = EffectsState {
            modulation: Modulation {
                enabled: true,
                mode,
                rate_hz: 0.8,
                depth: 0.65,
                feedback: 0.35,
                wet: 0.7,
            },
            tail_mode: TailMode::WrapLoop,
            ..EffectsState::default()
        };
        let out = process(&input, &state, 128.0).expect("modulation");
        assert_eq!(out.channels, 2);
        let l = channel_slice(&out, 0);
        let r = channel_slice(&out, 1);
        assert_ne!(l, r);
        assert!(rms(&l) > 0.05);
    }

    for rate in [44_100, 48_000, 96_000] {
        let mut burst = sine_pcm(220.0, rate as usize / 10, rate, 1, 0.6);
        burst.samples.resize(rate as usize / 2, 0.0);
        let hall = EffectsState {
            reverb: Reverb {
                enabled: true,
                space: ReverbSpace::Hall,
                decay_s: 2.2,
                damping: 0.4,
                wet: 0.5,
            },
            tail_mode: TailMode::RingOut,
            ..EffectsState::default()
        };
        let out = process(&burst, &hall, 128.0).expect("reverb");
        out.validate().expect("reverb valid pcm");
        assert_eq!(out.channels, 2);
        assert!(out.frames() > burst.frames());
        assert_ne!(channel_slice(&out, 0), channel_slice(&out, 1));
    }
}

#[test]
fn crush_curves_and_all_presets_validate_and_match() {
    let mut crush = EffectsState {
        dist_shape: DistortionShape::Crush,
        dirty: 0.6,
        ..EffectsState::default()
    };
    let input = sine_pcm(220.0, 4096, 44_100, 1, 0.6);
    let out = process(&input, &crush, 128.0).expect("crush");
    out.validate().expect("crush normalized");

    let f_curve = sample_filter_curve(&EffectsState::default().filter, 64);
    assert_eq!(f_curve.len(), 64);
    assert!(f_curve.iter().all(|v| v.abs() < 1e-4));

    let d_curve = sample_drive_curve(&EffectsState::default(), 65);
    assert_eq!(d_curve.len(), 65);
    assert!((d_curve[32] - 0.0).abs() < 1e-5);
    crush.dirty = 0.8;
    let crushed_curve = sample_drive_curve(&crush, 65);
    assert!(
        crushed_curve
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1.0)
    );

    let presets = effect_presets();
    assert_eq!(presets.len(), 8);
    for preset in presets {
        preset.state.validate().expect(preset.name);
        assert_eq!(matching_effect_preset(&preset.state), Some(preset.name));
        let rendered = process(&input, &preset.state, 128.0).expect(preset.name);
        rendered.validate().expect(preset.name);
    }
    let mut tweaked = presets[1].state.clone();
    tweaked.volume = 0.77;
    assert_eq!(matching_effect_preset(&tweaked), None);
}
