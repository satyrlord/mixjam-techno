use super::*;

#[test]
fn explicit_missing_source_is_silent_even_when_cached_audio_has_the_same_id() {
    let mut project = song();
    project.missing_samples.push(
        crate::engine::MissingSample::native(
            crate::products::Product::T2,
            5,
            "Missing sound".into(),
            140.0,
            TICKS_PER_BAR,
            false,
            "The referenced sound is unavailable.".into(),
        )
        .unwrap(),
    );
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 75600],
        },
    )]);
    let rendered = render_with_samples(&project, &samples).unwrap();
    assert_eq!(rendered.frames(), 151200);
    assert!(rendered.samples.iter().all(|sample| *sample == 0.0));
    project.missing_samples.clear();
    assert!(render_with_samples(&project, &HashMap::new()).is_err());
}

#[test]
fn a_new_preparation_reuses_the_sources_and_their_pitched_copies() {
    // Sound 5 plays at two pitches and never as it is; sound 6 as it is.
    let mut project = song();
    project.clips[0].pitch_cents = 300;
    for (id, sample_id, pitch_cents) in [(2, 5, -300), (3, 6, 0)] {
        project.clips.push(Clip {
            id,
            sample_id,
            pitch_cents,
            ..project.clips[0].clone()
        });
    }
    let reads = std::sync::Mutex::new(Vec::new());
    let read = |id| {
        reads.lock().unwrap().push(id);
        Ok(Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: (0..20_000)
                .map(|frame| (frame as f32 * 0.05).sin() * 0.5)
                .collect(),
        })
    };
    let mut decoded = HashMap::new();
    load_sources(&project, &mut decoded, read).unwrap();
    let mut first = reads.lock().unwrap().clone();
    first.sort_unstable();
    assert_eq!(
        first,
        [5, 6],
        "Each source is read once, at any number of pitches."
    );
    let mut keys: Vec<_> = decoded.keys().copied().collect();
    keys.sort_unstable();
    assert_eq!(keys, [(5, -300), (5, 300), (6, 0)]);
    let pitched = Arc::clone(&decoded[&(5, 300)]);

    // A gain edit reads and pitches nothing again.
    project.clips[0].gain = 0.25;
    reads.lock().unwrap().clear();
    load_sources(&project, &mut decoded, read).unwrap();
    assert!(reads.lock().unwrap().is_empty());
    assert!(Arc::ptr_eq(&pitched, &decoded[&(5, 300)]));

    // A source that is not normalized PCM is refused when it is read.
    project.clips[2].sample_id = 7;
    let refused = load_sources(&project, &mut decoded, |_| {
        Ok(Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![f32::NAN; 100],
        })
    });
    assert!(refused.is_err());
    assert!(!decoded.contains_key(&(7, 0)));
    assert!(Arc::ptr_eq(&pitched, &decoded[&(5, 300)]));
}

#[test]
fn work_shared_among_threads_comes_back_in_order() {
    let items: Vec<u64> = (0..200).collect();
    assert_eq!(
        in_parallel(&items, |item| item * item),
        items.iter().map(|item| item * item).collect::<Vec<_>>()
    );
    assert!(in_parallel(&[] as &[u64], |item| *item).is_empty());
}

#[test]
fn loop_source_keeps_channels_aligned_and_preserves_intro() {
    let pcm = Arc::new(Pcm {
        sample_rate: 44100,
        channels: 2,
        samples: vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7],
    });
    let source = PlaybackSource::new(pcm, 0, Some([1, 3]), None);
    assert_eq!(
        source.take(14).collect::<Vec<_>>(),
        vec![
            0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.2, 0.3, 0.4, 0.5, 0.2, 0.3, 0.4, 0.5
        ]
    );
}

#[test]
fn captured_source_tempo_is_not_applied_twice() {
    let mut p = song();
    p.bpm = 180.0;
    p.clips[0].offset_frames = 0;
    p.clips[0].sample_id = 1_000_000;
    p.imported_samples.push(crate::engine::ImportedSample {
        id: 1_000_000,
        name: "Recorded take".into(),
        group_name: "Wave".into(),
        path: "take.wav".into(),
        source_bpm: 180.0,
        source_sample_rate: Some(44100),
        origin: None,
    });
    let pcm = Pcm {
        sample_rate: 44100,
        channels: 1,
        samples: vec![0.1, 0.2, 0.3],
    };
    let out = render_with_samples(&p, &HashMap::from([(1_000_000, pcm)])).unwrap();
    let start = p.frames_per_tick(44100) * 256.0;
    let index = start.round() as usize * 2;
    assert_eq!(
        &out.samples[index..index + 6],
        &[0.05, 0.05, 0.1, 0.1, 0.15, 0.15]
    );
}

#[test]
fn a_half_time_loop_plays_at_its_own_pitch() {
    let mut p = song();
    p.bpm = 180.0;
    p.clips[0].offset_frames = 0;
    p.clips[0].sample_id = 1_000_000;
    p.imported_samples.push(crate::engine::ImportedSample {
        id: 1_000_000,
        name: "Half-time loop".into(),
        group_name: "Wave".into(),
        path: "loop.wav".into(),
        source_bpm: 90.0,
        source_sample_rate: Some(44100),
        origin: None,
    });
    let pcm = Pcm {
        sample_rate: 44100,
        channels: 1,
        samples: vec![0.1, 0.2, 0.3],
    };
    let out = render_with_samples(&p, &HashMap::from([(1_000_000, pcm)])).unwrap();
    let index = (p.frames_per_tick(44100) * 256.0).round() as usize * 2;
    assert_eq!(
        &out.samples[index..index + 6],
        &[0.05, 0.05, 0.1, 0.1, 0.15, 0.15]
    );
}

#[test]
fn exact_source_tempos_reject_unrepresentable_band_limited_reads_before_playback() {
    use crate::playback_quality::{PlaybackQuality, SampleQuality};
    let mut project = song();
    project.clips[0].sample_id = 1_000_000;
    project.clips[0].start_tick = 0;
    project.clips[0].offset_frames = 0;
    project
        .imported_samples
        .push(crate::engine::ImportedSample {
            id: 1_000_000,
            name: "Exact source".into(),
            group_name: "MixJam".into(),
            path: "source.wav".into(),
            source_bpm: project.bpm / 1_000_000_000.0,
            source_sample_rate: Some(44_100),
            origin: None,
        });
    project
        .source_notices
        .push("MixJam v7 source controls: {}".into());
    project.validate().unwrap();
    let samples = HashMap::from([(
        1_000_000,
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.25; 64],
        },
    )]);
    assert!(mix_with_samples(&project, &samples).is_err());
    let low = PlaybackQuality {
        samples: SampleQuality::Low,
        ..PlaybackQuality::FULL
    };
    assert!(mix_with_samples_quality(&project, &samples, low).is_ok());

    // This base speed fits the kernel's fixed-point spacing. Only a
    // positive Pitch value can move it beyond that numeric limit.
    project.imported_samples[0].source_bpm = project.bpm / 5_000_000.0;
    assert!(mix_with_samples(&project, &samples).is_ok());
    for cents in [0.0, -4800.0, 2400.0] {
        project.tracks[0].automation.pitch = vec![crate::automation::Point {
            tick: 0,
            value: cents,
        }];
        assert_eq!(mix_with_samples(&project, &samples).is_err(), cents > 0.0);
    }
}

#[test]
fn an_extremely_slow_exact_source_keeps_the_clip_end_and_fade() {
    use crate::playback_quality::{PlaybackQuality, SampleQuality};
    let mut project = song();
    project.clips[0].sample_id = 1_000_000;
    project.clips[0].offset_frames = 0;
    project.clips[0].fades.fade_out.ticks = 128;
    project
        .imported_samples
        .push(crate::engine::ImportedSample {
            id: 1_000_000,
            name: "Slow exact source".into(),
            group_name: "MixJam".into(),
            path: "source.wav".into(),
            source_bpm: 1e100,
            source_sample_rate: Some(44_100),
            origin: None,
        });
    project
        .source_notices
        .push("MixJam v7 source controls: {}".into());
    project.validate().unwrap();
    let samples = HashMap::from([(
        1_000_000,
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.25; 64],
        },
    )]);
    for samples_quality in [SampleQuality::High, SampleQuality::Low] {
        let mix = mix_with_samples_quality(
            &project,
            &samples,
            PlaybackQuality {
                samples: samples_quality,
                ..PlaybackQuality::FULL
            },
        )
        .unwrap();
        let voice = &mix.voices[0];
        assert!(voice.start > 0);
        assert_eq!(voice.sound_end, voice.end);
        assert_eq!(voice.fade(voice.start), 1.0);
        let last_gain = voice.fade(voice.end - 1);
        assert!(last_gain > 0.0 && last_gain < 1.0);
    }
}

#[test]
fn factual_source_tempos_do_not_change_the_discrete_project_clock() {
    for bpm in [0.001, 2000.0] {
        let mut project = song();
        project.clips[0].sample_id = 1_000_000;
        project.clips[0].start_tick = 0;
        project.clips[0].offset_frames = 0;
        project
            .imported_samples
            .push(crate::engine::ImportedSample {
                id: 1_000_000,
                name: "Source".into(),
                group_name: "Wave".into(),
                path: "source.wav".into(),
                source_bpm: bpm,
                source_sample_rate: Some(44100),
                origin: None,
            });
        let samples = HashMap::from([(
            1_000_000,
            Pcm {
                sample_rate: 44100,
                channels: 1,
                samples: vec![0.5; 32],
            },
        )]);
        let pcm = render_with_samples(&project, &samples).unwrap();
        pcm.validate().unwrap();
        assert_eq!(pcm.frames(), 75600);
        assert!(pcm.samples[0] > 0.0);
        project.imported_samples[0].source_bpm = f64::MIN_POSITIVE;
        assert!(render_with_samples(&project, &samples).is_err());
    }
}
