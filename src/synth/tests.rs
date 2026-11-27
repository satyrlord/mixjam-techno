use super::*;
use crate::effects::{Filter, FilterMode};
use std::f64::consts::TAU;

fn flat_source() -> Pcm {
    Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.5; 4000],
    }
}

fn rest_pattern() -> StepSynthState {
    StepSynthState {
        bpm: 120.0,
        steps: vec![Step::default(); STEPS],
        ..Default::default()
    }
}

fn tone_mag(samples: &[f32], ch: usize, channels: usize, freq_hz: f64, sr: f64) -> f64 {
    let mono: Vec<f32> = samples.iter().skip(ch).step_by(channels).copied().collect();
    let n = mono.len().max(1);
    let (mut re, mut im, mut w_sum) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (i, &x) in mono.iter().enumerate() {
        let w = 0.5 * (1.0 - (TAU * i as f64 / n as f64).cos());
        let ph = TAU * freq_hz * i as f64 / sr;
        re += f64::from(x) * w * ph.cos();
        im += f64::from(x) * w * ph.sin();
        w_sum += w;
    }
    2.0 * (re * re + im * im).sqrt() / w_sum.max(1.0)
}

#[test]
fn a_note_sounds_for_the_gate_of_its_step_and_fades_out() {
    let mut state = rest_pattern();
    state.steps[0] = Step::note(0);
    state.gate = 50;
    let output = render(&flat_source(), &state).unwrap();
    assert_eq!(output.frames(), 2000);
    assert!(output.samples[..61].iter().all(|x| *x == 0.5));
    assert!(output.samples[61] < 0.5 && output.samples[62] > 0.0);
    assert!(output.samples[63..].iter().all(|x| *x == 0.0));
}

#[test]
fn source_exhaustion_fades_at_the_actual_end_including_slides() {
    for (pitch, slide) in [(0, false), (12, false), (0, true)] {
        let source = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.5; if slide { 200 } else { 100 }],
        };
        let mut state = rest_pattern();
        state.steps[0] = Step {
            slide,
            ..Step::note(pitch)
        };
        if slide {
            state.steps[1] = Step::note(12);
        }
        let output = render(&source, &state).unwrap();
        let end = output.samples.iter().rposition(|x| *x != 0.0).unwrap() + 1;
        assert!(end < if slide { 250 } else { 125 });
        assert!((output.samples[end - 3] - 0.5).abs() < 1e-4);
        assert!((output.samples[end - 2] - 1.0 / 3.0).abs() < 1e-4);
        assert!((output.samples[end - 1] - 1.0 / 6.0).abs() < 1e-4);
        assert!(output.samples[end..].iter().all(|x| *x == 0.0));

        state.loop_source = true;
        let output = render(&source, &state).unwrap();
        let scheduled_end = if slide { 250 } else { 125 };
        assert!((output.samples[scheduled_end - 3] - 0.5).abs() < 1e-4);
        assert!(output.samples[scheduled_end - 1] > 0.0);
        assert_eq!(output.samples[scheduled_end], 0.0);
    }
}

#[test]
fn ties_hold_a_note_and_a_new_note_restarts_the_source() {
    let source = Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: (0..1000).map(|i| i as f32 / 1000.0).collect(),
    };
    let mut state = rest_pattern();
    state.steps[0] = Step::note(0);
    state.steps[1] = Step::tie();
    state.steps[2] = Step::note(12);
    for step in &mut state.steps[3..8] {
        *step = Step::tie();
    }
    let output = render(&source, &state).unwrap();
    // Unity pitch reads direct frames through ties and restarts at step 2.
    assert_eq!(output.samples[200], 0.2);
    assert!(
        output.samples[250] < 0.001,
        "sinc restart: {}",
        output.samples[250]
    );
    assert!(output.samples[251] > 0.0 && output.samples[251] < 0.01);
    assert!(output.samples[747] > 0.90);
    assert!(output.samples[749] > 0.0 && output.samples[749] < 0.36);
    assert_eq!(output.samples[750], 0.0);
}

#[test]
fn a_slide_glides_into_the_next_note_without_restarting() {
    let source = Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: (0..1000).map(|i| i as f32 / 1000.0).collect(),
    };
    let mut state = rest_pattern();
    state.steps[0] = Step {
        slide: true,
        ..Step::note(0)
    };
    state.steps[1] = Step::note(12);
    state.slide_ms = 100.0;
    let notes = state.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].end, 1.0);
    assert!(notes[1].slide_in);
    let output = render(&source, &state).unwrap();
    assert!(output.samples[125] > 0.12);
    let step = output.samples[176] - output.samples[175];
    assert!(step > 0.0013 && step < 0.002, "{step}");
}

#[test]
fn unfinished_slides_retarget_the_current_pitch_and_level() {
    let source = Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: (0..10000).map(|i| i as f32 / 10000.0).collect(),
    };
    let mut state = rest_pattern();
    state.slide_ms = 1000.0;
    for (index, note) in [0, 12, 24].into_iter().enumerate() {
        state.steps[index] = Step {
            slide: index < 2,
            ..Step::note(note)
        };
    }
    let output = render(&source, &state).unwrap();
    let increment = output.samples[251] - output.samples[250];
    let expected = 2f32.powf(1.5 / 12.0) / 10000.0;
    assert!(
        (increment - expected).abs() < 1e-5,
        "{increment} vs {expected}"
    );

    state.steps[0].velocity = 20;
    state.steps[1].velocity = 100;
    state.steps[2].velocity = 40;
    let output = render(&flat_source(), &state).unwrap();
    assert!((output.samples[250] - 0.15).abs() < 1e-4);
    assert!((output.samples[250] - output.samples[249]).abs() < 0.001);
}

#[test]
fn accent_and_velocity_set_the_level() {
    let mut state = rest_pattern();
    state.steps[0] = Step {
        velocity: 50,
        ..Step::note(0)
    };
    state.steps[1] = Step {
        accent: true,
        ..Step::note(0)
    };
    state.accent_db = 6.0;
    let output = render(&flat_source(), &state).unwrap();
    assert_eq!(output.samples[10], 0.25);
    assert!((output.samples[135] - 0.5 * 10f32.powf(0.3)).abs() < 1e-6);
}

#[test]
fn a_loud_render_is_turned_down_whole_instead_of_clipping() {
    let mut source = flat_source();
    source.samples.fill(1.0);
    let mut state = rest_pattern();
    state.steps[0] = Step::note(0);
    state.steps[1] = Step {
        accent: true,
        ..Step::note(0)
    };
    state.accent_db = 6.0;
    let (output, turned_down) = render_measured(&source, &state).unwrap();
    let accent = 10f64.powf(0.3);
    assert!(
        (turned_down - 20.0 * accent.log10()).abs() < 1e-4,
        "{turned_down}"
    );
    assert!((output.samples[135] - 1.0).abs() < 1e-6);
    assert!((output.samples[10] - (1.0 / accent) as f32).abs() < 1e-5);
    let full_scale = output.samples.iter().filter(|x| x.abs() >= 1.0).count();
    assert!(
        full_scale < 125,
        "Only the accented step reaches full scale."
    );

    state.steps[1].accent = false;
    let (output, turned_down) = render_measured(&source, &state).unwrap();
    assert_eq!(turned_down, 0.0);
    assert_eq!(output.samples[10], 1.0);
}

#[test]
fn full_accent_keeps_headroom_until_the_effects_output_gain() {
    let mut source = flat_source();
    source.samples.fill(0.8);
    let mut state = rest_pattern();
    state.steps[0] = Step {
        accent: true,
        ..Step::note(0)
    };
    state.accent_db = 12.0;
    state.effects.volume = 0.2;
    let output = render(&source, &state).unwrap();
    let expected = 0.8 * 10f32.powf(12.0 / 20.0) * 0.2;
    assert!((output.samples[10] - expected).abs() < 1e-6);
    output.validate().unwrap();

    state.effects.volume = 1.0;
    let output = render(&source, &state).unwrap();
    assert_eq!(output.samples[10], 1.0);
    output.validate().unwrap();
    source.samples[0] = 1.0;
    assert!(render(&source, &state).is_ok());
    source.samples[0] = 1.01;
    assert!(render(&source, &state).is_err());
}

#[test]
fn swing_delays_every_second_sixteenth() {
    let mut state = rest_pattern();
    state.steps[1] = Step::note(0);
    state.swing = 75;
    let notes = state.notes();
    assert_eq!((notes[0].start, notes[0].end), (1.5, 2.0));
    let output = render(&flat_source(), &state).unwrap();
    assert_eq!(output.samples[186], 0.0);
    assert_eq!(output.samples[188], 0.5);
}

#[test]
fn an_odd_length_renders_until_it_lines_up_with_the_bar() {
    let mut state = rest_pattern();
    state.length = 12;
    state.steps[0] = Step::note(0);
    state.steps[20] = Step::note(5);
    assert_eq!(state.sequence_steps(), 48);
    let starts: Vec<f64> = state.notes().iter().map(|note| note.start).collect();
    assert_eq!(starts, vec![0.0, 12.0, 24.0, 36.0]);
    assert_eq!(render(&flat_source(), &state).unwrap().frames(), 6000);
    state.length = 63;
    assert_eq!(state.sequence_steps(), 1008);
    state.length = 64;
    assert_eq!(state.sequence_steps(), 64);
}

#[test]
fn old_patterns_preserve_note_timing_and_bar_contents() {
    let mut steps = vec![None; 64];
    steps[0] = Some(0);
    steps[4] = Some(7);
    steps[32] = Some(-3);
    steps[48] = Some(12);
    let old = serde_json::json!({
        "bpm": 140.0,
        "active_bars": [true, false, true, false],
        "steps": steps,
        "cut_sixteenth": false,
        "loop_source": true,
        "effects": EffectsState::default(),
    });
    let state: StepSynthState = serde_json::from_value(old.clone()).unwrap();
    state.validate().unwrap();
    assert_eq!(state.length, 32);
    assert!(state.loop_source);
    assert_eq!(state.steps[0], Step::note(0));
    assert_eq!(state.steps[1], Step::tie());
    assert_eq!(state.steps[4], Step::note(7));
    assert_eq!(state.steps[16], Step::note(-3));
    assert_eq!(state.steps[31], Step::tie());
    assert_eq!(state.steps[32], Step::default());
    assert_eq!(state.steps[48], Step::note(12));
    assert_eq!(state.steps[63], Step::tie());
    let notes = state.notes();
    assert_eq!(
        notes
            .iter()
            .map(|note| (note.start, note.end, note.note))
            .collect::<Vec<_>>(),
        vec![(0.0, 4.0, 0), (4.0, 16.0, 7), (16.0, 32.0, -3)]
    );

    let mut cut = old;
    cut["cut_sixteenth"] = true.into();
    let state: StepSynthState = serde_json::from_value(cut).unwrap();
    assert_eq!(state.steps[1], Step::default());
    assert_eq!(state.notes()[0].end, 1.0);
}

#[test]
fn old_patterns_without_an_active_bar_stay_invalid() {
    let old = serde_json::json!({
        "bpm": 140.0,
        "active_bars": vec![false; 4],
        "steps": vec![Some(0); STEPS],
        "cut_sixteenth": false,
        "loop_source": false,
        "effects": EffectsState::default(),
    });
    let error = serde_json::from_value::<StepSynthState>(old).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Switch on at least one Active bar.")
    );
}

#[test]
fn serialized_state_roundtrips_and_invalid_values_are_rejected() {
    let mut state = StepSynthState::default();
    state.steps[3] = Step {
        accent: true,
        slide: true,
        velocity: 40,
        ..Step::note(-5)
    };
    state.steps[4] = Step::tie();
    let saved = serde_json::to_vec(&state).unwrap();
    assert_eq!(
        serde_json::from_slice::<StepSynthState>(&saved).unwrap(),
        state
    );
    let text = String::from_utf8(saved).unwrap();
    assert!(text.contains(r#"{"trig":"tie"}"#), "{text}");
    assert!(
        text.contains("{},{}"),
        "A rest saves as an empty step: {text}"
    );

    for change in [
        (|s: &mut StepSynthState| s.steps[0] = Step::note(49)) as fn(&mut StepSynthState),
        |s| s.steps[0].velocity = 0,
        |s| s.length = 0,
        |s| s.length = 65,
        |s| s.gate = 4,
        |s| s.swing = 76,
        |s| s.accent_db = 13.0,
        |s| s.slide_ms = 0.5,
        |s| s.root = 12,
        |s| s.start_ms = -1.0,
        |s| s.start_ms = 2001.0,
        |s| s.osc_sub = -0.1,
        |s| s.osc_sub = 1.1,
        |s| s.osc_octave = 3,
        |s| s.amp_decay_ms = 5.0,
        |s| s.amp_decay_ms = 2100.0,
        |s| s.filter_env_amount = 1.2,
        |s| s.filter_env_decay_ms = 10.0,
        |s| s.filter_env_decay_ms = 2500.0,
        |s| {
            s.steps.pop();
        },
    ] {
        let mut bad = state.clone();
        change(&mut bad);
        assert!(bad.validate().is_err());
    }
    let mut unknown = serde_json::to_value(&state).unwrap();
    unknown["extra"] = 1.into();
    assert!(serde_json::from_value::<StepSynthState>(unknown).is_err());

    state.scale = edit::Scale::Dorian;
    state.root = 5;
    state.start_ms = 12.0;
    state.source = Some(PathBuf::from("kick.wav"));
    state.source_mode = SourceMode::Synth;
    state.osc_waveform = OscWaveform::Square;
    state.osc_sub = 0.4;
    state.osc_octave = -1;
    state.amp_decay_ms = 250.0;
    state.filter_env_amount = 0.65;
    state.filter_env_decay_ms = 180.0;
    let saved = serde_json::to_value(&state).unwrap();
    assert_eq!(saved["scale"], "dorian");
    assert_eq!(saved["source_mode"], "synth");
    assert_eq!(saved["osc_waveform"], "square");
    assert_eq!(
        serde_json::from_value::<StepSynthState>(saved.clone()).unwrap(),
        state
    );
    let mut old = saved;
    for field in [
        "scale",
        "root",
        "start_ms",
        "source",
        "source_mode",
        "osc_waveform",
        "osc_sub",
        "osc_octave",
        "amp_decay_ms",
        "filter_env_amount",
        "filter_env_decay_ms",
    ] {
        old.as_object_mut().unwrap().remove(field);
    }
    let old = serde_json::from_value::<StepSynthState>(old).unwrap();
    assert_eq!(
        (
            old.scale,
            old.root,
            old.start_ms,
            old.source,
            old.source_mode,
            old.osc_waveform,
            old.osc_sub,
            old.osc_octave,
            old.amp_decay_ms,
            old.filter_env_amount,
            old.filter_env_decay_ms
        ),
        (
            edit::Scale::Minor,
            0,
            0.0,
            None,
            SourceMode::Sample,
            OscWaveform::Saw,
            0.0,
            0,
            0.0,
            0.0,
            DEFAULT_FILTER_ENV_DECAY_MS
        )
    );
    let text = serde_json::to_string(&StepSynthState::default()).unwrap();
    assert!(!text.contains("\"source_mode\""), "{text}");
}

#[test]
fn the_start_skips_the_source_head_and_loops_from_it() {
    let source = Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: (0..100).map(|i| i as f32 / 100.0).collect(),
    };
    let mut state = rest_pattern();
    state.steps[0] = Step::note(0);
    state.gate = 100;
    state.start_ms = 40.0;
    let output = render(&source, &state).unwrap();
    assert_eq!(output.samples[0], 0.4);
    assert_eq!(output.samples[60], 0.0);
    state.loop_source = true;
    let output = render(&source, &state).unwrap();
    assert_eq!(output.samples[59], 0.99);
    assert_eq!(output.samples[60], 0.4);
    state.start_ms = 100.0;
    assert!(
        render(&source, &state)
            .unwrap_err()
            .contains("past the end")
    );
}

#[test]
fn the_first_hit_is_found_after_leading_silence() {
    let mut samples = vec![0.0; 2 * 1000];
    samples[2 * 30] = 0.01;
    samples[2 * 50 + 1] = -0.8;
    let source = Pcm {
        sample_rate: 1000,
        channels: 2,
        samples,
    };
    assert_eq!(first_hit(&source), 49.0);
    let silent = Pcm {
        sample_rate: 1000,
        channels: 1,
        samples: vec![0.0; 10],
    };
    assert_eq!(first_hit(&silent), 0.0);
}

#[test]
fn stereo_source_preserves_independent_left_and_right_channels() {
    let sr = 44_100;
    let frames = sr * 3;
    let mut samples = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let t = i as f64 / sr as f64;
        samples.push(0.5 * (TAU * 440.0 * t).sin() as f32);
        samples.push(-0.5 * (TAU * 660.0 * t).sin() as f32);
    }
    let source = Pcm {
        sample_rate: sr as u32,
        channels: 2,
        samples,
    };
    let mut state = rest_pattern();
    state.steps[0] = Step::note(3);
    for s in &mut state.steps[1..16] {
        *s = Step::tie();
    }
    let out = render(&source, &state).unwrap();
    assert_eq!(out.channels, 2);
    let f_l = 440.0 * 2.0_f64.powf(0.25);
    let f_r = 660.0 * 2.0_f64.powf(0.25);
    let l_mag = tone_mag(&out.samples[4096..], 0, 2, f_l, sr as f64);
    let r_mag = tone_mag(&out.samples[4096..], 1, 2, f_r, sr as f64);
    let l_xtalk = tone_mag(&out.samples[4096..], 0, 2, f_r, sr as f64);
    let r_xtalk = tone_mag(&out.samples[4096..], 1, 2, f_l, sr as f64);
    assert!(l_mag > 0.35 && r_mag > 0.35, "L={l_mag} R={r_mag}");
    assert!(
        l_xtalk < 0.01 && r_xtalk < 0.01,
        "crosstalk L={l_xtalk} R={r_xtalk}"
    );
}

#[test]
fn kaiser_sinc_resampling_rejects_foldback_and_images() {
    let sr = 44_100;
    let source = Pcm {
        sample_rate: sr as u32,
        channels: 2,
        samples: (0..sr)
            .flat_map(|i| {
                let v = 0.5 * (TAU * 8_000.0 * i as f64 / sr as f64).sin() as f32;
                [v, v]
            })
            .collect(),
    };
    let mut state = rest_pattern();
    state.steps[0] = Step::note(24);
    for s in &mut state.steps[1..16] {
        *s = Step::tie();
    }
    let up = render(&source, &state).unwrap();
    let alias_db = 20.0
        * (tone_mag(&up.samples[4096..], 0, 2, 12_100.0, sr as f64) / 0.5)
            .max(1e-8)
            .log10();
    assert!(alias_db < -50.0, "+24st 8kHz foldback {alias_db:.1} dB");

    state.steps[0] = Step::note(-12);
    let down = render(&source, &state).unwrap();
    let fund = tone_mag(&down.samples[4096..], 0, 2, 4_000.0, sr as f64);
    let img = tone_mag(&down.samples[4096..], 0, 2, 18_050.0, sr as f64);
    assert!((fund - 0.5).abs() < 0.01, "droop fund={fund}");
    assert!(20.0 * (img / fund).max(1e-8).log10() < -85.0);
}

#[test]
fn amp_decay_and_accent_filter_envelope_honor_legato_slides() {
    let mut state = rest_pattern();
    state.bpm = 140.0;
    state.source_mode = SourceMode::Synth;
    state.amp_decay_ms = 120.0;
    state.steps[0] = Step::note(0);
    state.steps[1] = Step::tie();
    let decayed = render_standalone(&state).unwrap();
    assert_eq!(decayed.channels, 2);
    let early: f32 = decayed.samples[500..1500].iter().map(|x| x.abs()).sum();
    let late: f32 = decayed.samples[16_000..17_000]
        .iter()
        .map(|x| x.abs())
        .sum();
    assert!(late < early * 0.25, "early={early} late={late}");

    // Legato slide does not retrigger amp or filter envelope at step 1 boundary.
    let mut slide_pat = rest_pattern();
    slide_pat.bpm = 140.0;
    slide_pat.source_mode = SourceMode::Synth;
    slide_pat.amp_decay_ms = 200.0;
    slide_pat.steps[0] = Step {
        slide: true,
        ..Step::note(0)
    };
    slide_pat.steps[1] = Step::note(7);
    let slide_out = render_standalone(&slide_pat).unwrap();
    let step_frame = (44_100.0_f64 * 60.0 / 140.0 / 4.0).round() as usize;
    let boundary_jump =
        (slide_out.samples[step_frame * 2] - slide_out.samples[(step_frame - 1) * 2]).abs();
    assert!(boundary_jump < 0.08, "legato slide click {boundary_jump}");

    // Accent filter envelope opens cutoff above plain note even when accent_db = 0.
    let mut plain = rest_pattern();
    plain.bpm = 140.0;
    plain.source_mode = SourceMode::Synth;
    plain.osc_octave = 1;
    plain.filter_env_amount = 0.45;
    plain.filter_env_decay_ms = 220.0;
    plain.effects.filter = Filter {
        enabled: true,
        mode: FilterMode::Lp24,
        cutoff_hz: 120.0,
        resonance: 0.72,
        drive: 0.2,
        wet: 1.0,
    };
    plain.steps[0] = Step::note(0);
    let mut accented = plain.clone();
    accented.steps[0].accent = true;
    accented.accent_db = 0.0;
    let out_p = render_standalone(&plain).unwrap();
    let out_a = render_standalone(&accented).unwrap();
    let h5_p = tone_mag(&out_p.samples[..4096], 0, 2, 654.06, 44_100.0);
    let h5_a = tone_mag(&out_a.samples[..4096], 0, 2, 654.06, 44_100.0);
    assert!(
        h5_a > h5_p * 1.4,
        "accent filter upper harmonic: a={h5_a} vs p={h5_p}"
    );
}

#[test]
fn standalone_synth_oscillators_suppress_aliasing_below_88db() {
    let mut state = rest_pattern();
    state.bpm = 140.0;
    state.source_mode = SourceMode::Synth;
    state.osc_octave = 2;
    state.steps[0] = Step::note(24);
    for s in &mut state.steps[1..16] {
        *s = Step::tie();
    }
    for wave in [OscWaveform::Saw, OscWaveform::Square, OscWaveform::Triangle] {
        state.osc_waveform = wave;
        let out = render_standalone(&state).unwrap();
        assert_eq!(out.sample_rate, SYNTH_SAMPLE_RATE);
        let fund = tone_mag(&out.samples[4096..], 0, 2, 1046.502, 44_100.0);
        assert!(fund > 0.1, "{wave:?} fund={fund}");
        // Reflected upper harmonic of 1046.5 Hz near Nyquist: 42*1046.5=43953 -> 147 Hz, 41*1046.5=42906 -> 1194 Hz,
        // and 22*1046.5=23023 -> 21077 Hz.
        for alias_hz in [21_077.0, 20_030.0, 17_937.0] {
            let mag = tone_mag(&out.samples[4096..], 0, 2, alias_hz, 44_100.0);
            let db = 20.0 * (mag / fund).max(1e-9).log10();
            assert!(db < -88.0, "{wave:?} alias @ {alias_hz}: {db:.1} dBc");
        }
    }
}

#[test]
fn presets_apply_voice_and_effects_without_modifying_user_notes() {
    let mut state = StepSynthState::default();
    state.steps[2] = Step::note(7);
    state.length = 12;
    state.bpm = 135.0;
    state.source = Some(PathBuf::from("kick.wav"));
    let saved_steps = state.steps.clone();
    let presets = step_synth_presets();
    assert_eq!(presets.len(), 8);
    for preset in presets {
        preset.apply(&mut state);
        state.validate().unwrap();
        assert_eq!(matching_step_synth_preset(&state), Some(preset.name));
        assert_eq!(state.steps, saved_steps);
        assert_eq!(state.length, 12);
        assert_eq!(state.bpm, 135.0);
        assert_eq!(state.source, Some(PathBuf::from("kick.wav")));
    }
}
