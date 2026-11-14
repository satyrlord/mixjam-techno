use super::*;

#[test]
fn clip_fades_shape_the_clip_and_the_fade_out_ends_with_its_sound() {
    use crate::engine::{Fade, FadeCurve, Fades};
    // One bar of sound in a two-bar clip: 75,600 frames at 140 BPM.
    let mut project = song();
    project.clips[0].start_tick = 0;
    project.clips[0].offset_frames = 0;
    project.clips[0].length_ticks = 2 * TICKS_PER_BAR;
    project.clips[0].fades = Fades {
        fade_in: Fade {
            ticks: 64,
            curve: FadeCurve::Linear,
        },
        fade_out: Fade {
            ticks: 64,
            curve: FadeCurve::Exponential,
        },
    };
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 75_600],
        },
    )]);
    let at = |project: &Project, frame: u64| {
        let mut out = [0.0; 2];
        Frames::read(
            &mix_with_samples(project, &samples).unwrap(),
            frame,
            &mut out,
        );
        out[0]
    };
    // A quarter note is 18,900 frames.
    assert!(at(&project, 0) < 1e-4);
    assert!((at(&project, 9_450) - 0.125).abs() < 1e-4);
    assert_eq!(at(&project, 30_000), 0.25);
    let middle = 0.25 * (1000.0_f32.sqrt() - 1.0) / 999.0;
    assert!((at(&project, 75_600 - 9_450) - middle).abs() < 1e-4);
    assert!(at(&project, 75_599) < 1e-4);
    assert_eq!(at(&project, 75_600), 0.0);
    // A clip shorter than its sound fades out to the clip's end.
    project.clips[0].length_ticks = 3 * TICKS_PER_BAR / 4;
    project.clips[0].fades.fade_out.curve = FadeCurve::Linear;
    assert!((at(&project, 56_700 - 9_450) - 0.125).abs() < 1e-4);
    assert!(at(&project, 56_699) < 1e-4);
    assert_eq!(at(&project, 28_350), 0.25);
}

#[test]
fn a_pitched_clip_plays_higher_in_the_same_time() {
    // One bar of a 441 Hz tone at 140 BPM, played an octave up.
    let mut project = song();
    project.clips[0].start_tick = 0;
    project.clips[0].offset_frames = 0;
    project.clips[0].pitch_cents = 1200;
    let tone = Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: (0..75_600)
            .map(|frame| (std::f32::consts::TAU * 441.0 * frame as f32 / 44_100.0).sin() * 0.5)
            .collect(),
    };
    let rendered = render_with_samples(&project, &HashMap::from([(5, tone)])).unwrap();
    let left: Vec<f32> = rendered.samples.iter().step_by(2).copied().collect();
    let crossings = |range: std::ops::Range<usize>| {
        left[range.clone()]
            .windows(2)
            .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
            .count() as f64
            * 44_100.0
            / range.len() as f64
    };
    let measured = crossings(10_000..60_000);
    assert!((measured - 882.0).abs() < 4.0, "{measured}");
    // The sound still fills its bar and stops where it did.
    assert!(left[70_000..75_000].iter().any(|sample| sample.abs() > 0.2));
    assert!(left[75_600..].iter().all(|sample| *sample == 0.0));
}

#[test]
fn fade_curves_rise_from_silence_to_full() {
    use crate::engine::{FadeCurve, Fades};
    for curve in FadeCurve::ALL {
        assert_eq!(curve.gain(0.0), 0.0);
        assert!((curve.gain(1.0) - 1.0).abs() < 1e-6);
        let steps: Vec<_> = (0..=100).map(|t| curve.gain(t as f32 / 100.0)).collect();
        assert!(steps.windows(2).all(|pair| pair[1] > pair[0]), "{curve:?}");
    }
    assert!((FadeCurve::EqualPower.gain(0.5).powi(2) - 0.5).abs() < 1e-6);
    let mut fades = Fades::default();
    fades.fade_in.ticks = 300;
    fades.fade_out.ticks = 100;
    let fitted = fades.fit(256);
    assert_eq!((fitted.fade_in.ticks, fitted.fade_out.ticks), (256, 0));
    let fitted = fades.fit(350);
    assert_eq!((fitted.fade_in.ticks, fitted.fade_out.ticks), (300, 50));
}

#[test]
fn mixer_attenuation_and_polarity_are_independent_of_pan_and_routing() {
    let mut project = song();
    project.clips[0].start_tick = 0;
    project.clips[0].offset_frames = 0;
    project.tracks[0].pan = 0.25;
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44100,
            channels: 2,
            samples: vec![0.125, -0.25, 0.25, -0.125],
        },
    )]);
    for amplitude in [
        0.0,
        crate::engine::db_to_amplitude(-6.0),
        1.0,
        crate::engine::db_to_amplitude(crate::engine::MAX_MIXER_GAIN_DB),
    ] {
        project.tracks[0].gain = amplitude;
        project.tracks[0].polarity_inverted = false;
        let mut normal = [0.0; 4];
        Frames::read(
            &mix_with_samples(&project, &samples).unwrap(),
            0,
            &mut normal,
        );
        assert!((normal[0] - 0.125 * 0.5 * amplitude * 0.75).abs() < 1e-7);
        assert!((normal[1] - -0.25 * 0.5 * amplitude).abs() < 1e-7);
        project.tracks[0].polarity_inverted = true;
        let mut reversed = [0.0; 4];
        Frames::read(
            &mix_with_samples(&project, &samples).unwrap(),
            0,
            &mut reversed,
        );
        for (normal, reversed) in normal.into_iter().zip(reversed) {
            assert_eq!(reversed, -normal);
        }
    }
    project.tracks[0].gain = 1.0;
    project.tracks[0].right_enabled = false;
    let mut routed = [0.0; 4];
    Frames::read(
        &mix_with_samples(&project, &samples).unwrap(),
        0,
        &mut routed,
    );
    assert!(routed[0] < 0.0);
    assert_eq!([routed[1], routed[3]], [0.0, 0.0]);
}

#[test]
fn solo_plays_only_soloed_tracks_and_mute_wins_over_solo() {
    let mut project = song();
    project.clips[0].start_tick = 0;
    project.clips[0].offset_frames = 0;
    // Tracks 1 to 3 each play the same sound.
    for track in 1..3 {
        let mut clip = project.clips[0].clone();
        clip.id = 10 + track as u64;
        clip.track = track;
        project.clips.push(clip);
    }
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44100,
            channels: 1,
            samples: vec![0.25; 4],
        },
    )]);
    let one = render_with_samples(&project, &samples).unwrap().samples[0] / 3.0;
    for (muted, solo, playing) in [
        ([false; 3], [false; 3], 3.0),
        ([true, false, false], [false; 3], 2.0),
        ([false; 3], [false, true, false], 1.0),
        ([false; 3], [true, true, false], 2.0),
        ([true, false, false], [true, true, false], 1.0),
        ([true, false, false], [true, false, false], 0.0),
    ] {
        for track in 0..3 {
            project.tracks[track].muted = muted[track];
            project.tracks[track].solo = solo[track];
        }
        let rendered = render_with_samples(&project, &samples).unwrap();
        assert!(
            (rendered.samples[0] - one * playing).abs() < 1e-7,
            "{muted:?} {solo:?}"
        );
    }
}

#[test]
fn last_track_preserves_scheduled_offset_gain_and_channel_routing() {
    let mut p = song();
    p.clips[0].track = 63;
    p.tracks[63].right_enabled = false;
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44100,
            channels: 1,
            samples: vec![0.0, 0.5, 1.0, -1.0],
        },
    )]);
    let out = render_with_samples(&p, &samples).unwrap();
    assert_eq!(out.frames(), 151200);
    assert_eq!(out.samples[151198], 0.0);
    assert_eq!(
        &out.samples[151200..151206],
        &[0.25, 0.0, 0.5, 0.0, -0.5, 0.0]
    );
}

#[test]
fn tempo_and_pitch_use_one_clock() {
    let mut p = song();
    p.bpm = 280.0;
    p.pitch_ratio = 2.0;
    let p = crate::engine::normalize_import(p).unwrap();
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44100,
            channels: 1,
            samples: vec![1.0; 75600],
        },
    )]);
    assert_eq!(p.bpm, 180.0);
    assert_eq!(p.pitch_ratio, 1.0);
    assert_eq!(render_with_samples(&p, &samples).unwrap().frames(), 117600);
}
