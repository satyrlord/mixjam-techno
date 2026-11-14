use super::*;

#[test]
fn wav_export_rejects_riff_size_overflow_before_opening_output() {
    // The data fits u32, but adding the 36-byte RIFF payload header does not.
    let mix = Mix {
        quality: crate::playback_quality::PlaybackQuality::FULL,
        frames: (u64::from(u32::MAX) - 36) / 4 + 1,
        voices: Vec::new(),
        chains: Vec::new(),
        keys: Vec::new(),
        key_tracks: Vec::new(),
        side: false,
        levels: [[1.0; 2]; crate::engine::TRACK_COUNT],
        automated_pan: [false; crate::engine::TRACK_COUNT],
        master_gain: 1.0,
        bpm: 140.0,
        fed: [false; BUSES],
        sends: false,
        drives: 0,
        returns: None,
    };
    // An invalid destination proves that admission runs before file creation,
    // without writing a multi-gigabyte fixture on a broken implementation.
    let error = mix
        .write_wav(Path::new(""), &MasterChain::default(), &FxBuses::default())
        .unwrap_err();
    assert_eq!(error, "This song is too long for a WAV file.");
}

#[test]
fn the_wav_sample_limit_is_what_the_riff_size_allows() {
    // The data chunk and the RIFF size are 32-bit, and the RIFF payload also
    // holds 36 bytes of headers.
    let data = crate::audio::MAX_WAV_SAMPLES as u64 * 2;
    assert!(data + 36 <= u64::from(u32::MAX));
    assert!(data + 2 + 36 > u64::from(u32::MAX));
}

#[test]
fn a_wav_past_the_old_sample_limit_reads_whole() {
    // 64,000,001 samples: one more than the fixed limit that the reader had.
    // The file is sparse, so the test writes only its header.
    let samples = 64_000_001u32;
    let mut wav = b"RIFF".to_vec();
    wav.extend((36 + samples).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend([1, 0, 1, 0]);
    wav.extend(44_100u32.to_le_bytes());
    wav.extend(44_100u32.to_le_bytes());
    wav.extend([1, 0, 8, 0]);
    wav.extend(b"data");
    wav.extend(samples.to_le_bytes());
    let path = std::env::temp_dir().join(format!("mixjam-long-wav-{}.wav", std::process::id()));
    {
        let mut file = std::fs::File::create(&path).unwrap();
        std::io::Write::write_all(&mut file, &wav).unwrap();
        file.set_len(wav.len() as u64 + u64::from(samples)).unwrap();
    }
    let mut count = 0u64;
    let (spec, frames) = crate::audio::visit_wav_samples(&path, |_, _| count += 1).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!((spec.channels, spec.bits_per_sample), (1, 8));
    assert_eq!((count, u64::from(frames)), (64_000_001, 64_000_001));
}

#[test]
fn a_stray_end_is_hidden_and_a_real_tail_is_not() {
    // Tails of original product samples, 16-bit, after 10 ms of the given
    // lead-in. The frame count never changes.
    let tail = |lead: &dyn Fn(usize) -> i16, tail: &[i16]| {
        let mut pcm = Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: (0..600)
                .map(lead)
                .chain(tail.iter().copied())
                .map(|x| f32::from(x) / 32768.0)
                .collect(),
        };
        let before = pcm.samples.clone();
        hide_stray_end(&mut pcm);
        assert_eq!(pcm.samples.len(), before.len());
        let end = |samples: &[f32]| -> Vec<i16> {
            samples[samples.len() - tail.len()..]
                .iter()
                .map(|x| (x * 32768.0).round() as i16)
                .collect()
        };
        (end(&before), end(&pcm.samples), before == pcm.samples)
    };
    let silent = |_: usize| 0;
    let loud = |i: usize| (((i * 7) % 13) as i16 - 6) * 3_000;
    let (_, fixed, _) = tail(&silent, &[0, 0, 0, 0, 0, 0, 0, 9282]);
    assert_eq!(fixed, [0, 0, 0, 0, 0, 0, 0, 0]);
    let (_, fixed, _) = tail(&silent, &[0, 0, 0, 0, 0, 0, 0, 399]);
    assert_eq!(fixed, [0; 8]);
    let (_, fixed, _) = tail(&|_| -4, &[-4, -4, -4, -3, -4, -4, -4, 1729]);
    assert_eq!(fixed, [-4, -4, -4, -3, -4, -4, -4, -4]);
    let (_, fixed, _) = tail(&silent, &[0, 0, 0, 0, 0, 0, -3853, 4756]);
    assert_eq!(fixed, [0; 8]);
    // A quiet sound that dies away to silence and ends on one frame
    // between two and four times its peak.
    let quiet = |i: usize| (((i * 7) % 13) as i16 - 6) * 500;
    let (_, fixed, _) = tail(&quiet, &[0, 0, 0, 0, 0, 0, 0, 9282]);
    assert_eq!(fixed, [0; 8]);
    let (_, fixed, _) = tail(&quiet, &[0, 0, 0, 2, 0, 0, 0, 9282]);
    assert_eq!(fixed, [0, 0, 0, 2, 0, 0, 0, 0]);
    // The same sound that is still sounding at its end is a real tail.
    let (_, kept, unchanged) = tail(&quiet, &[2000, -2500, 1800, -2200, 2400, -1900, 2100, 7000]);
    assert!(unchanged, "became {kept:?}");
    // A sound still sounding at its end, a smooth tail, then a single
    // frame that jumps (hold-on-005-r).
    let smooth = |i: usize| ((i % 200) as i16 - 100).abs() * 14 + 1066;
    let (_, fixed, _) = tail(&smooth, &[1644, 1648, 1494, 1208, 922, 692, 562, 9478]);
    assert_eq!(fixed, [1644, 1648, 1494, 1208, 922, 692, 562, 562]);
    // The loud real tails also stay with a lead that continues from their start,
    // so the lone-jump rule is what must keep them.
    for real in [
        [-5338, 4359, 3207, -10247, 15196, -20289, 23170, -21643],
        [816, 2006, 10203, 18743, 23341, 23767, 16673, 3815],
    ] {
        let start = real[0];
        let (_, kept, unchanged) = tail(&|_| start, &real);
        assert!(unchanged, "{real:?} became {kept:?}");
    }
    for real in [
        [-5338, 4359, 3207, -10247, 15196, -20289, 23170, -21643],
        [816, 2006, 10203, 18743, 23341, 23767, 16673, 3815],
        [-518, -490, 162, 865, 3077, 4644, 8113, 9916],
    ] {
        let (_, kept, unchanged) = tail(&loud, &real);
        assert!(unchanged, "{real:?} became {kept:?}");
    }
}

#[test]
fn a_stray_end_is_hidden_in_one_channel_only() {
    let mut samples = vec![0.0; 2 * 600];
    for frame in 0..600 {
        samples[frame * 2] = -4.0 / 32768.0;
    }
    let last = samples.len() - 2;
    samples[last] = 1729.0 / 32768.0;
    samples[last + 1] = 0.0;
    let mut pcm = Pcm {
        sample_rate: 44_100,
        channels: 2,
        samples,
    };
    let before = pcm.samples.clone();
    hide_stray_end(&mut pcm);
    assert_eq!(pcm.samples.len(), before.len());
    assert_eq!(pcm.samples[last], -4.0 / 32768.0);
    assert_eq!(pcm.samples[..last], before[..last]);
    assert_eq!(pcm.samples[last + 1], before[last + 1]);
}

#[test]
fn a_short_sound_keeps_its_end() {
    let mut pcm = Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.5],
    };
    let before = pcm.samples.clone();
    hide_stray_end(&mut pcm);
    assert_eq!(pcm.samples, before);
}

#[test]
fn dry_wav_admission_keeps_the_full_riff_capacity() {
    let mix = Mix {
        quality: crate::playback_quality::PlaybackQuality::FULL,
        frames: (u64::from(u32::MAX) - 36) / 4,
        voices: Vec::new(),
        chains: Vec::new(),
        keys: Vec::new(),
        key_tracks: Vec::new(),
        side: false,
        levels: [[1.0; 2]; crate::engine::TRACK_COUNT],
        automated_pan: [false; crate::engine::TRACK_COUNT],
        master_gain: 1.0,
        bpm: 140.0,
        fed: [false; BUSES],
        sends: false,
        drives: 0,
        returns: None,
    };
    let error = mix
        .write_wav(Path::new(""), &MasterChain::default(), &FxBuses::default())
        .unwrap_err();
    assert_ne!(error, "This song is too long for a WAV file.");
}

#[test]
fn clipping_and_float_pcm_boundaries() {
    assert_eq!(float_to_i16(-1.0), -32768);
    assert_eq!(float_to_i16(1.0), 32767);
    let mut p = song();
    p.clips[0].start_tick = 0;
    p.clips[0].offset_frames = 0;
    p.clips[0].gain = 2.0;
    let samples = HashMap::from([(
        5,
        Pcm {
            sample_rate: 44100,
            channels: 1,
            samples: vec![1.0],
        },
    )]);
    assert_eq!(
        &render_with_samples(&p, &samples).unwrap().samples[..2],
        &[1.0, 1.0]
    );
}

#[test]
fn extreme_float_wav_is_normalized_before_interpolation() {
    let path = std::env::temp_dir().join(format!("mixjam-techno-float-{}.wav", std::process::id()));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(&path, spec).unwrap();
    wav.write_sample(f32::MAX).unwrap();
    wav.write_sample(-f32::MAX).unwrap();
    wav.finalize().unwrap();
    let pcm = read_wav(&path).unwrap();
    assert_eq!(pcm.samples, vec![1.0, -1.0]);
    let mut p = song();
    p.clips[0].offset_frames = 0;
    p.bpm = 120.0;
    assert!(
        render_with_samples(&p, &HashMap::from([(5, pcm)]))
            .unwrap()
            .samples
            .iter()
            .all(|x| x.is_finite())
    );
    std::fs::remove_file(path).unwrap();
}
