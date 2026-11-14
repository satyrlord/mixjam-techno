use super::*;

/// The frames of the reverse tests' source: a rising ramp in the left
/// channel and a falling one in the right, so a frame read from the
/// wrong place or the wrong way shows.
const RAMP_FRAMES: usize = 200_000;

fn ramp_source() -> HashMap<u32, Pcm> {
    let up = |i: usize| 0.9 * i as f32 / RAMP_FRAMES as f32;
    HashMap::from([(
        1,
        Pcm {
            sample_rate: 44_100,
            channels: 2,
            samples: (0..RAMP_FRAMES).flat_map(|i| [up(i), -up(i)]).collect(),
        },
    )])
}

/// One clip on track 1 from bar 2 for `ticks`, trimmed by `offset`
/// frames, forward or reversed.
fn reversed_song(offset: u32, ticks: u32, reverse: bool) -> Project {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 256, ticks).unwrap();
    let mut project = editor.project().clone();
    project.master_gain = 1.0;
    project.clips[0].offset_frames = offset;
    project.clips[0].reverse = reverse;
    project
}

#[test]
fn a_reversed_clip_plays_its_slice_backwards_within_the_clip() {
    let samples = ramp_source();
    for (offset, ticks) in [(0, 256), (12_345, 128), (40_000, 300)] {
        let forward = whole(&reversed_song(offset, ticks, false), &samples);
        let project = reversed_song(offset, ticks, true);
        let reversed = whole(&project, &samples);
        let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
        let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
        let (start, end) = (frame(256), frame(256 + ticks));
        // Silent outside the clip, and each frame the forward clip's
        // mirror frame.
        assert!(reversed[..start * 2].iter().all(|value| *value == 0.0));
        assert!(reversed[end * 2..].iter().all(|value| *value == 0.0));
        for at in start..end {
            let mirror = start + end - 1 - at;
            assert_eq!(reversed[at * 2], forward[mirror * 2], "{offset}: {at}");
            assert_eq!(
                reversed[at * 2 + 1],
                forward[mirror * 2 + 1],
                "{offset}: {at}"
            );
        }
        // The left ramp falls through the clip and ends on the frame at
        // the trim: the slice from `offset` over the clip's length.
        assert!(reversed[start * 2] > reversed[(end - 1) * 2]);
        let first = 0.9 * offset as f32 / RAMP_FRAMES as f32;
        assert!((reversed[(end - 1) * 2] - first).abs() < 1e-6, "{offset}");
        assert!(
            reversed[start * 2..end * 2]
                .chunks(2)
                .zip(reversed[start * 2 + 2..end * 2].chunks(2))
                .all(|(a, b)| b[0] <= a[0] && b[1] >= a[1])
        );
    }
}

#[test]
fn a_reversed_clip_longer_than_its_sound_ends_with_the_clip() {
    let samples = ramp_source();
    // 200,000 frames from frame 150,000 leave 50,000, about 0.57 bars at
    // 140 BPM, in a clip of 2 bars.
    let forward = whole(&reversed_song(150_000, 512, false), &samples);
    let project = reversed_song(150_000, 512, true);
    let reversed = whole(&project, &samples);
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    let (start, end) = (frame(256), frame(768));
    let sound = forward[start * 2..end * 2]
        .chunks(2)
        .take_while(|pair| pair[0] != 0.0)
        .count();
    assert!(sound > 1000 && sound < end - start - 1000, "{sound}");
    assert!(
        reversed[start * 2..(end - sound) * 2]
            .iter()
            .all(|v| *v == 0.0)
    );
    assert!(reversed[(end - 1) * 2] > 0.6);
}

#[test]
fn a_reversed_clips_fades_and_gain_stay_in_its_own_time() {
    use crate::engine::{Fade, FadeCurve, Fades};
    let samples = ramp_source();
    let raw = whole(&reversed_song(5_000, 256, true), &samples);
    let mut project = reversed_song(5_000, 256, true);
    project.clips[0].gain = 0.5;
    project.clips[0].fades = Fades {
        fade_in: Fade {
            ticks: 64,
            curve: FadeCurve::Linear,
        },
        fade_out: Fade {
            ticks: 32,
            curve: FadeCurve::Linear,
        },
    };
    let faded = whole(&project, &samples);
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    let (start, end) = (frame(256), frame(512));
    let (fade_in, fade_out) = (
        (64.0 * frames_per_tick).round() as usize,
        (32.0 * frames_per_tick).round() as usize,
    );
    // The fade in rises from the clip's start and the fade out falls to
    // its end, as on a forward clip: they are not mirrored.
    for since in 0..fade_in {
        let gain = 0.5 * (since as f32 + 0.5) / fade_in as f32;
        let at = (start + since) * 2;
        assert!((faded[at] - raw[at] * gain).abs() < 1e-5, "in {since}");
    }
    for left in 1..=fade_out {
        let gain = 0.5 * (left as f32 - 0.5) / fade_out as f32;
        let at = (end - left) * 2;
        assert!((faded[at] - raw[at] * gain).abs() < 1e-5, "out {left}");
    }
    let middle = (start + end) / 2 * 2;
    assert!((faded[middle] - raw[middle] * 0.5).abs() < 1e-6);
}

#[test]
fn a_reversed_clip_pitches_in_its_own_time() {
    let samples = ramp_source();
    let mut forward = reversed_song(0, 256, false);
    forward.clips[0].pitch_cents = 1200;
    let mut reversed = forward.clone();
    reversed.clips[0].reverse = true;
    let (forward_out, reversed_out) = (whole(&forward, &samples), whole(&reversed, &samples));
    assert_ne!(forward_out, whole(&reversed_song(0, 256, false), &samples));
    let frames_per_tick = forward.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    let (start, end) = (frame(256), frame(512));
    for at in start..end {
        let mirror = start + end - 1 - at;
        assert_eq!(reversed_out[at * 2], forward_out[mirror * 2]);
    }
}

#[test]
fn playback_started_inside_a_reversed_clip_plays_what_export_plays() {
    let samples = ramp_source();
    let mut project = reversed_song(7_000, 384, true);
    // A chain on the track too: a Repeat roll over the reversed sound.
    project.tracks[0].automation.repeat = [(448, 16.0), (512, 0.0)]
        .map(|(tick, value)| crate::automation::Point { tick, value })
        .to_vec();
    let export = whole(&project, &samples);
    let mix = mix_with_samples(&project, &samples).unwrap();
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as u64;
    let end = frame(640) as usize;
    for start in [frame(300), frame(400) + 17, frame(460) + 3] {
        let start = start as usize;
        let mut out = vec![0.0; (end - start) * 2];
        let mut state = RenderState::default();
        let mut at = start;
        for size in [777, 1, 4096, 129].into_iter().cycle() {
            if at >= end {
                break;
            }
            let size = size.min(end - at);
            mix.read_block(
                at as u64,
                &mut out[(at - start) * 2..(at - start + size) * 2],
                None,
                &mut state,
            );
            at += size;
        }
        assert_eq!(out, export[start * 2..end * 2], "from frame {start}");
    }
    let mut stem = vec![0.0; export.len()];
    mix.mix(0, &mut stem, None, &mut RenderState::default(), Some(0));
    assert_eq!(stem, export);
}

/// One clip on track 1 from bar 2 for `ticks`, with the Pitch lane
/// `pitch`.
fn pitched_song(ticks: u32, pitch: &[(u32, f32)]) -> Project {
    let mut project = reversed_song(0, ticks, false);
    project.tracks[0].automation.pitch = pitch
        .iter()
        .map(|&(tick, value)| crate::automation::Point { tick, value })
        .collect();
    project
}

/// The value of `plain`'s left channel at a fractional frame.
fn left_at(plain: &[f32], frame: f64) -> f32 {
    let index = frame.floor() as usize;
    let fraction = (frame - index as f64) as f32;
    plain[index * 2] + (plain[index * 2 + 2] - plain[index * 2]) * fraction
}

#[test]
fn a_pitch_lane_reads_the_source_at_its_varispeed_clock() {
    let samples = ramp_source();
    // Normal speed to bar 2.5, then a fall to two octaves down at bar 3,
    // held to the clip's end at bar 3.5.
    let lane = [(384, 0.0), (512, -2400.0)];
    let project = pitched_song(384, &lane);
    let plain = whole(&pitched_song(384, &[]), &samples);
    let out = whole(&project, &samples);
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    let (start, fall, end) = (frame(256), frame(384), frame(640));
    // To the first point the sound is the plain clip. A clip under a
    // Pitch lane always reads through the band-limited kernel, which
    // copies a steady ramp to within its DC accuracy, where the plain
    // clip at its own tempo copies the frames.
    let worst = (0..fall * 2)
        .map(|at| (out[at] - plain[at]).abs())
        .fold(0.0_f32, f32::max);
    assert!(worst < 1e-5, "{worst}");
    let points = project.tracks[0].automation.pitch.clone();
    let clock = crate::automation::Varispeed::new(&points, frames_per_tick).unwrap();
    // After it, each frame reads where the clock has come to: the plain
    // clip at the clock's distance from the clip's start.
    for at in (fall..end).step_by(7) {
        let along = clock.at(at as u64) - clock.at(start as u64);
        let want = left_at(&plain, start as f64 + along);
        assert!((out[at * 2] - want).abs() < 1e-5, "frame {at}");
    }
    // Two octaves down plays at a quarter of the speed.
    let late = frame(600);
    let slope = |samples: &[f32], at: usize| samples[(at + 100) * 2] - samples[at * 2];
    let ratio = slope(&out, late) / slope(&plain, late);
    assert!((ratio - 0.25).abs() < 1e-3, "{ratio}");
    // The read never jumps: no frame moves further than at normal
    // speed, beyond the rounding of the samples. The straight-line read
    // copies the ramp exactly, so it shows the clock alone; the
    // band-limited kernel's gain ripples with the read's phase by more
    // than this ramp's step (see the kernel's own click test).
    let low = crate::playback_quality::PlaybackQuality {
        samples: crate::playback_quality::SampleQuality::Low,
        ..crate::playback_quality::PlaybackQuality::FULL
    };
    let largest = |samples: &[f32]| {
        (start + 1..end)
            .map(|at| (samples[at * 2] - samples[(at - 1) * 2]).abs())
            .fold(0.0_f32, f32::max)
    };
    let (moved, normal) = (
        largest(&whole_quality(&project, &samples, low)),
        largest(&plain),
    );
    assert!(moved <= normal * 1.01, "{moved} against {normal}");
}

#[test]
fn a_fast_pitch_lane_ends_the_source_with_the_clip_fade_and_reads_no_further() {
    let samples = ramp_source();
    // An octave up from the clip's start runs the 200,000-frame source
    // out at twice the speed, before the four-bar clip ends.
    let mut project = pitched_song(1024, &[(256, 1200.0)]);
    project.clips[0].fades.fade_out.ticks = 64;
    let out = whole(&project, &samples);
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    let (start, end) = (frame(256), frame(1280));
    let source_end = start + RAMP_FRAMES.div_ceil(2);
    assert!(source_end < end);
    // The fade out ends where the source does, and silence follows.
    assert!(out[source_end * 2..].iter().all(|value| *value == 0.0));
    let last = out[(source_end - 1) * 2];
    assert!(last > 0.0 && last < 0.001, "{last}");
    let fade = frame(64);
    let before = out[(source_end - fade) * 2];
    assert!(before > 0.7, "{before}");
    // A tape-stop: slowed four octaves, the clip ends before its source
    // and its own fade out ends the sound at the clip's end.
    let mut stopped = pitched_song(1024, &[(256, 0.0), (384, -4800.0)]);
    stopped.clips[0].fades.fade_out.ticks = 64;
    let out = whole(&stopped, &samples);
    assert!(out[(end - 1) * 2] > 0.0);
    assert!(out[end * 2..].iter().all(|value| *value == 0.0));
}

/// A bright stereo source: a 13.3 kHz tone over a low one on the left,
/// and a 9 kHz tone on the right.
fn bright_source(frames: usize, channels: u16) -> Pcm {
    let left = |i: usize| 0.4 * (i as f32 * 1.9).sin() + 0.3 * (i as f32 * 0.05).sin();
    let right = |i: usize| 0.5 * (i as f32 * 1.3).cos();
    Pcm {
        sample_rate: 44_100,
        channels,
        samples: (0..frames)
            .flat_map(|i| {
                if channels == 1 {
                    vec![left(i)]
                } else {
                    vec![left(i), right(i)]
                }
            })
            .collect(),
    }
}

/// A voice of `source` from output frame 100 to 1100, from `offset`, at
/// `step`, with every gain at one.
fn plain_voice(source: Pcm, offset: u32, step: f64, reverse: bool, band_limited: bool) -> Voice {
    Voice {
        track: 0,
        source: Arc::new(source),
        start: 100,
        end: 1100,
        offset_frames: offset,
        step,
        channel_gains: [Some(1.0); 2],
        sends: [0.0; BUSES],
        fade_in: 0,
        fade_out: 0,
        curves: [FadeCurve::Linear; 2],
        sound_end: 1100,
        reverse,
        varispeed: None,
        band_limited,
    }
}

fn voice_out(voice: &Voice) -> Vec<f32> {
    let mut out = vec![0.0; 1200 * 2];
    voice.add(0, &mut out, None, &Ramp::UNITY);
    out
}

#[test]
fn an_off_tempo_voice_reads_band_limited_on_the_straight_line_reads_frames() {
    resample::prepare();
    for channels in [1, 2] {
        let source = bright_source(4_000, channels);
        let ch = usize::from(channels);
        let step = 147.0 / 140.0;
        let high = voice_out(&plain_voice(source.clone(), 1_000, step, false, true));
        let low = voice_out(&plain_voice(source.clone(), 1_000, step, false, false));
        // Silence before the clip and after it, on both reads.
        for out in [&high, &low] {
            assert!(out[..200].iter().all(|value| *value == 0.0));
            assert!(out[2200..].iter().all(|value| *value == 0.0));
        }
        let slice = &source.samples[1_000 * ch..];
        for frame in 100..1100 {
            let position = (frame - 100) as f64 * step;
            // High reads the clip's slice through the kernel at each
            // frame's own position, from the clip's first frame.
            let want = resample::read(slice, ch, position, resample::scale(step));
            // Low is the straight-line read as it always was.
            let index = 1_000 + position.floor() as usize;
            let fraction = (1_000.0 + position - index as f64) as f32;
            for channel in 0..2 {
                let source_channel = channel.min(ch - 1);
                assert_eq!(high[frame * 2 + channel], want[source_channel]);
                let a = source.samples[index * ch + source_channel];
                let b = source.samples[(index + 1) * ch + source_channel];
                assert_eq!(low[frame * 2 + channel], a + (b - a) * fraction);
            }
        }
        // The clip sounds from its first frame, which reads its offset's
        // frame, on either read.
        for channel in 0..2 {
            let first = source.samples[1_000 * ch + channel.min(ch - 1)];
            assert_eq!(low[200 + channel], first);
            assert_ne!(high[200 + channel], 0.0);
        }
    }
}

#[test]
fn a_band_limited_voice_ends_with_its_source_and_mirrors_when_reversed() {
    resample::prepare();
    // 900 frames from offset 100 run out at output frame 100 + 800 / 1.25.
    let source = bright_source(900, 2);
    let step = 1.25;
    let high = voice_out(&plain_voice(source.clone(), 100, step, false, true));
    let low = voice_out(&plain_voice(source.clone(), 100, step, false, false));
    let last = |out: &[f32]| {
        out.chunks(2)
            .rposition(|frame| frame != [0.0, 0.0])
            .unwrap()
    };
    assert_eq!(last(&high), 100 + 640 - 1);
    assert_eq!(last(&high), last(&low));
    // Near the end the read holds the last frame instead of reaching past it.
    assert!(
        high.iter()
            .all(|value| value.is_finite() && value.abs() < 1.0)
    );
    // Reversed, each frame plays what the forward frame at its mirror
    // plays, so the sound is a pure function of the source.
    let reversed = voice_out(&plain_voice(source, 100, step, true, true));
    for frame in 100..1100 {
        let mirror = 100 + 1100 - 1 - frame;
        for channel in 0..2 {
            assert_eq!(
                reversed[frame * 2 + channel],
                high[mirror * 2 + channel],
                "{frame}"
            );
        }
    }
}

fn whole_quality(
    project: &Project,
    samples: &HashMap<u32, Pcm>,
    quality: crate::playback_quality::PlaybackQuality,
) -> Vec<f32> {
    let mix = mix_with_samples_quality(project, samples, quality).unwrap();
    let mut out = vec![0.0; mix.frames() as usize * 2];
    mix.read_block(0, &mut out, None, &mut RenderState::default());
    out
}

#[test]
fn sample_quality_low_reads_off_tempo_clips_in_straight_lines_and_cannot_export() {
    use crate::playback_quality::{PlaybackQuality, SampleQuality};
    let samples = HashMap::from([(1, bright_source(200_000, 2))]);
    let mut project = reversed_song(500, 256, false);
    let low = PlaybackQuality {
        samples: SampleQuality::Low,
        ..PlaybackQuality::FULL
    };
    // At the source's own tempo both reads copy the frames.
    assert_eq!(
        whole_quality(&project, &samples, low),
        whole(&project, &samples)
    );
    project.bpm = 160.0;
    let (high_out, low_out) = (
        whole(&project, &samples),
        whole_quality(&project, &samples, low),
    );
    assert_ne!(high_out, low_out);
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let start = (256.0 * frames_per_tick).round() as usize;
    // Both start on the clip's first frame.
    for out in [&high_out, &low_out] {
        assert!(out[..start * 2].iter().all(|value| *value == 0.0));
        assert!(out[start * 2] != 0.0);
    }
    // Only full quality exports.
    let prepared = mix_with_samples_quality(&project, &samples, low).unwrap();
    assert_eq!(prepared.quality(), low);
    assert!(prepared.to_pcm(&project.master, &project.fx).is_err());
    assert!(
        mix_with_samples(&project, &samples)
            .unwrap()
            .to_pcm(&project.master, &project.fx)
            .is_ok()
    );
}

/// The cost of one voice, by read. Run it in release on a quiet machine:
/// `cargo test --release --lib sample_read_cost -- --ignored --nocapture`.
#[test]
#[ignore = "release benchmark; use a quiet machine"]
fn sample_read_cost() {
    resample::prepare();
    let seconds = 20;
    let frames = 44_100 * seconds;
    let lane = Arc::new(
        crate::automation::Varispeed::new(
            &[
                crate::automation::Point {
                    tick: 0,
                    value: -300.0,
                },
                crate::automation::Point {
                    tick: 4096,
                    value: 300.0,
                },
            ],
            300.0,
        )
        .unwrap(),
    );
    for channels in [2, 1] {
        let source = bright_source(frames * 2, channels);
        for (name, step, varispeed) in [
            ("172 to 180 BPM", 180.0 / 172.0, false),
            ("48 kHz source", 48_000.0 / 44_100.0, false),
            ("165 BPM from 180", 165.0 / 180.0, false),
            ("Pitch lane, -3 to +3 semitones", 1.047, true),
        ] {
            let mut costs = [0.0; 2];
            for (cost, band_limited) in costs.iter_mut().zip([true, false]) {
                let mut voice = plain_voice(source.clone(), 0, step, false, band_limited);
                voice.end = frames as u64;
                voice.sound_end = frames as u64;
                voice.start = 0;
                voice.varispeed = varispeed.then(|| lane.clone());
                let mut out = vec![0.0; 512 * 2];
                let started = std::time::Instant::now();
                for block in 0..frames / 512 {
                    out.fill(0.0);
                    voice.add((block * 512) as u64, &mut out, None, &Ramp::UNITY);
                    std::hint::black_box(&out);
                }
                *cost = started.elapsed().as_secs_f64() * 1e9 / frames as f64;
            }
            println!(
                "{channels} ch, {name}: High {:.1} ns a frame ({:.2}% of a core at 44.1 kHz), \
                 Low {:.1} ns a frame ({:.2}%)",
                costs[0],
                costs[0] * 44_100.0 / 1e7,
                costs[1],
                costs[1] * 44_100.0 / 1e7,
            );
        }
    }
}

/// The largest step between neighbouring left frames in `range`.
fn largest_step(out: &[f32], range: Range<usize>) -> f32 {
    range
        .map(|at| (out[at * 2] - out[(at - 1) * 2]).abs())
        .fold(0.0_f32, f32::max)
}

#[test]
fn a_pitch_lane_moves_the_kernel_without_a_click_and_removes_its_aliases() {
    use crate::playback_quality::{PlaybackQuality, SampleQuality};
    let tone = |radians: f32| {
        HashMap::from([(
            1,
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: (0..400_000)
                    .map(|i| 0.5 * (i as f32 * radians).sin())
                    .collect(),
            },
        )])
    };
    let frames_per_tick = reversed_song(0, 512, false).frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    // A 2 kHz tone jumps an octave at bar 2.5, at once, and the kernel's
    // cutoff halves. It plays at 4 kHz after, still in the pass band, so
    // no frame may step further than a 4 kHz sine of its level can.
    let two_khz = 2.0 * std::f32::consts::PI * 2_000.0 / 44_100.0;
    let jump = pitched_song(512, &[(384, 1200.0)]);
    let out = whole(&jump, &tone(two_khz));
    let at = frame(384);
    let level = out[(at - 2000) * 2..at * 2]
        .iter()
        .step_by(2)
        .fold(0.0_f32, |high, value| high.max(value.abs()));
    let bound = 2.0 * std::f32::consts::PI * 4_000.0 / 44_100.0 * level;
    let step = largest_step(&out, at - 500..at + 2_000);
    assert!(step <= bound * 1.02, "a step of {step} over {bound}");
    // An 8 kHz tone sped up two octaves would read at 32 kHz and fold to
    // 12.1 kHz. After the cutoff's 64-frame glide the band-limited read
    // has removed it; the straight-line read leaves it loud.
    let eight_khz = 2.0 * std::f32::consts::PI * 8_000.0 / 44_100.0;
    let fast = pitched_song(512, &[(384, 2400.0)]);
    let rms = |out: &[f32], range: Range<usize>| {
        let count = range.len() as f32;
        (range.map(|at| out[at * 2].powi(2)).sum::<f32>() / count).sqrt()
    };
    let high = whole(&fast, &tone(eight_khz));
    let low = whole_quality(
        &fast,
        &tone(eight_khz),
        PlaybackQuality {
            samples: SampleQuality::Low,
            ..PlaybackQuality::FULL
        },
    );
    let before = rms(&high, at - 2_000..at - 100);
    let after = at + 100..at + 4_000;
    assert!(
        rms(&high, after.clone()) < before * 1e-3,
        "{}",
        rms(&high, after)
    );
    assert!(rms(&low, after.clone()) > before * 0.3);
}

#[test]
fn playback_started_inside_a_pitch_fall_plays_what_export_plays() {
    let samples = unique_source();
    let mut project = repeat_song(&[(448, 16.0), (512, 0.0)]);
    // A tape-stop from bar 1.5 to bar 2.5, then back up, under a Repeat
    // roll.
    project.tracks[0].automation.pitch = [(128, 0.0), (384, -4800.0), (400, 0.0), (480, 700.0)]
        .map(|(tick, value)| crate::automation::Point { tick, value })
        .to_vec();
    let export = whole(&project, &samples);
    let mix = mix_with_samples(&project, &samples).unwrap();
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as u64;
    let end = frame(640) as usize;
    for start in [frame(100), frame(250) + 13, frame(390), frame(460) + 3] {
        let start = start as usize;
        let mut out = vec![0.0; (end - start) * 2];
        let mut state = RenderState::default();
        let mut at = start;
        for size in [1000, 1, 127, 4096, 333, 129].into_iter().cycle() {
            if at >= end {
                break;
            }
            let size = size.min(end - at);
            mix.read_block(
                at as u64,
                &mut out[(at - start) * 2..(at - start + size) * 2],
                None,
                &mut state,
            );
            at += size;
        }
        assert_eq!(out, export[start * 2..end * 2], "from frame {start}");
    }
    let mut stem = vec![0.0; export.len()];
    mix.mix(0, &mut stem, None, &mut RenderState::default(), Some(0));
    assert_eq!(stem, export);
}

#[test]
fn the_repeat_lane_replays_the_pitched_sound() {
    let samples = unique_source();
    let pitch = [(128, 0.0), (256, -1200.0)]
        .map(|(tick, value)| crate::automation::Point { tick, value })
        .to_vec();
    let mut live = repeat_song(&[]);
    live.tracks[0].automation.pitch = pitch.clone();
    let mut project = repeat_song(&[(320, 16.0), (384, 0.0)]);
    project.tracks[0].automation.pitch = pitch;
    let (live_out, out) = (whole(&live, &samples), whole(&project, &samples));
    // Each sixteenth from tick 320 replays the slowed sound from 320.
    assert_slices(&out, &live_out, &project, 320, &[320, 336, 352, 368, 384]);
}
