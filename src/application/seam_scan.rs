//! Plays a song through live playback from many starts shortly before the
//! clip seams of chosen bars, so that playback's blocks fall on each seam at
//! many phases, at both sample qualities. Compares the dry mix of each start
//! with one contiguous read, and scans the mastered output for broadband
//! noise.
//!
//! ```text
//! MIXJAM_SEAM_SONG=song.mixjam MIXJAM_SEAM_BARS=65,69,73 cargo test --release --lib seam_scan -- --ignored --nocapture
//! ```
use super::*;
use crate::playback_quality::{PlaybackQuality, SampleQuality};

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.into())
}

/// A small fixed-seed generator, so a run can be repeated.
struct Random(u64);

impl Random {
    fn below(&mut self, range: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % range.max(1)
    }
}

/// Plays a song through live playback from the bars in `MIXJAM_SEAM_FROM`
/// to the bar `MIXJAM_SEAM_TO`, and compares each with the export, frame by
/// frame, from where the export's buses and pedals have the same input.
#[test]
#[ignore = "plays a real song"]
fn long_play_scan() {
    let path =
        PathBuf::from(std::env::var("MIXJAM_SEAM_SONG").expect("MIXJAM_SEAM_SONG names a song"));
    let from: Vec<u32> = env("MIXJAM_SEAM_FROM", "1")
        .split(',')
        .map(|bar| bar.trim().parse().unwrap())
        .collect();
    let to: u32 = env("MIXJAM_SEAM_TO", "81").parse().unwrap();
    let (project, library, _) =
        crate::project_io::open_song(&path, &Library::empty()).expect("opens");
    let fpt = project.frames_per_tick(audio::Mix::SAMPLE_RATE);
    let bar_frame =
        |bar: u32| (f64::from((bar - 1) * crate::engine::TICKS_PER_BAR) * fpt).round() as u64;
    let mix: Arc<audio::Mix> = Arc::new(
        audio::prepare_project_with_quality(&project, &library, PlaybackQuality::FULL)
            .expect("prepares"),
    );
    let export = mix
        .to_pcm(&project.master, &project.fx)
        .expect("exports")
        .samples;
    let mut detector = audio::flight::Detector::new(audio::Mix::SAMPLE_RATE, 2);
    let window = detector.window_samples();
    for bar in from {
        let start = bar_frame(bar);
        let end = bar_frame(to);
        let control = MasterControl::new(&project.master);
        control.set_returns(&project.fx);
        control.set_levels(&project);
        let mut out = Vec::new();
        let started = std::time::Instant::now();
        audio::play_offline(
            mix.clone(),
            start,
            None,
            Some(control),
            PlaybackQuality::FULL,
            end - start,
            |block| out.extend_from_slice(block),
        );
        let elapsed = started.elapsed().as_secs_f64();
        // The difference from the export in each bar.
        let mut report = Vec::new();
        for check in bar..to {
            let (a, b) = (bar_frame(check), bar_frame(check + 1));
            let largest = (a..b)
                .flat_map(|frame| {
                    let at = ((frame - start) * 2) as usize;
                    let expected = (frame * 2) as usize;
                    [0, 1].map(|channel| {
                        (out[at + channel] - export.get(expected + channel).copied().unwrap_or(0.0))
                            .abs()
                    })
                })
                .fold(0.0_f32, f32::max);
            report.push(format!("{check}:{largest:.0e}"));
        }
        let mut noisy = Vec::new();
        let mut flattest = 0.0_f32;
        let mut at = 0;
        while at + window <= out.len() {
            for reading in detector.read(&out[at..at + window]) {
                flattest = flattest.max(reading.flatness);
                if reading.noisy() {
                    noisy.push(start + at as u64 / 2);
                }
            }
            at += window / 2;
        }
        let s = super::sequence_scan::scan(&out);
        println!(
            "from bar {bar} to {to} in {elapsed:.1} s: peak {:.3}, non-finite {}, flattest {flattest:.2}, \
             noisy windows {} (first at {:?}), loud noise {}\n  difference from the export by bar: {}",
            s.peak,
            s.non_finite,
            noisy.len(),
            noisy.first(),
            super::sequence_scan::loud_noise(&out),
            report.join(" ")
        );
    }
}

/// Plays a song through live playback from bar 1 to `MIXJAM_SEAM_TO` while
/// the wheel turns track pans and faders as a scroll over the track headers
/// does: a step every `MIXJAM_WHEEL_FRAMES` frames (default 2205, 50 ms) on
/// tracks in turn, and scans the output for broadband noise.
#[test]
#[ignore = "plays a real song"]
fn wheel_scan() {
    let path =
        PathBuf::from(std::env::var("MIXJAM_SEAM_SONG").expect("MIXJAM_SEAM_SONG names a song"));
    let to: u32 = env("MIXJAM_SEAM_TO", "81").parse().unwrap();
    let every: u64 = env("MIXJAM_WHEEL_FRAMES", "2205").parse().unwrap();
    let (project, library, _) =
        crate::project_io::open_song(&path, &Library::empty()).expect("opens");
    let fpt = project.frames_per_tick(audio::Mix::SAMPLE_RATE);
    let bar_frame =
        |bar: u32| (f64::from((bar - 1) * crate::engine::TICKS_PER_BAR) * fpt).round() as u64;
    let mix: Arc<audio::Mix> = Arc::new(
        audio::prepare_project_with_quality(&project, &library, PlaybackQuality::FULL)
            .expect("prepares"),
    );
    let used: Vec<usize> = (0..project.tracks.len())
        .filter(|index| project.clips.iter().any(|clip| clip.track == *index))
        .collect();
    let mut detector = audio::flight::Detector::new(audio::Mix::SAMPLE_RATE, 2);
    let window = detector.window_samples();
    for (label, faders) in [("pan", false), ("pan and fader", true)] {
        let control = MasterControl::new(&project.master);
        control.set_returns(&project.fx);
        control.set_levels(&project);
        let mut edited = project.clone();
        let mut random = Random(0x9E37_79B9_7F4A_7C15);
        let mut out = Vec::new();
        let mut played = 0_u64;
        let mut next = every;
        let mut steps = 0_u64;
        let end = bar_frame(to);
        let live = Arc::clone(&control);
        audio::play_offline(
            mix.clone(),
            0,
            None,
            Some(control),
            PlaybackQuality::FULL,
            end,
            |block| {
                out.extend_from_slice(block);
                played += block.len() as u64 / 2;
                while played >= next {
                    let track = used[(steps as usize / 8) % used.len()];
                    let turn = if random.below(2) == 0 { 0.05 } else { -0.05 };
                    let state = &mut edited.tracks[track];
                    state.pan =
                        ((state.pan * 100.0 + turn * 100.0).round() / 100.0).clamp(-1.0, 1.0);
                    if faders {
                        state.gain =
                            (state.gain * if turn > 0.0 { 1.06 } else { 0.94 }).clamp(0.0, 2.0);
                    }
                    live.set_levels(&edited);
                    steps += 1;
                    next += every;
                }
            },
        );
        let mut noisy = Vec::new();
        let mut flattest = 0.0_f32;
        let mut at = 0;
        while at + window <= out.len() {
            for reading in detector.read(&out[at..at + window]) {
                flattest = flattest.max(reading.flatness);
                if reading.noisy() {
                    noisy.push(at as u64 / 2);
                }
            }
            at += window / 2;
        }
        let s = super::sequence_scan::scan(&out);
        println!(
            "{label}, {steps} wheel steps, bar 1 to {to}: peak {:.3}, non-finite {}, flattest {flattest:.2}, \
             noisy windows {} (first at frame {:?}), loud noise {}",
            s.peak,
            s.non_finite,
            noisy.len(),
            noisy.first(),
            super::sequence_scan::loud_noise(&out),
        );
    }
}

#[test]
#[ignore = "plays a real song"]
fn seam_scan() {
    let path =
        PathBuf::from(std::env::var("MIXJAM_SEAM_SONG").expect("MIXJAM_SEAM_SONG names a song"));
    let bars: Vec<u32> = env("MIXJAM_SEAM_BARS", "65,69,73")
        .split(',')
        .map(|bar| bar.trim().parse().unwrap())
        .collect();
    let starts: usize = env("MIXJAM_SEAM_STARTS", "48").parse().unwrap();
    let live_starts: usize = env("MIXJAM_SEAM_LIVE", "24").parse().unwrap();
    let qualities: Vec<PlaybackQuality> = env("MIXJAM_SEAM_QUALITY", "high,low")
        .split(',')
        .map(|name| match name.trim() {
            "low" => PlaybackQuality {
                samples: SampleQuality::Low,
                ..PlaybackQuality::FULL
            },
            _ => PlaybackQuality::FULL,
        })
        .collect();
    let (project, library, _) =
        crate::project_io::open_song(&path, &Library::empty()).expect("opens");
    let fpt = project.frames_per_tick(audio::Mix::SAMPLE_RATE);
    let bar_frame =
        |bar: u32| (f64::from((bar - 1) * crate::engine::TICKS_PER_BAR) * fpt).round() as u64;
    let bar_frames = bar_frame(2) - bar_frame(1);
    let mut random = Random(0x2545_f491_4f6c_dd1d);
    for quality in qualities {
        let started = std::time::Instant::now();
        let mix: Arc<audio::Mix> = Arc::new(
            audio::prepare_project_with_quality(&project, &library, quality).expect("prepares"),
        );
        println!(
            "{:?}: prepared in {:.1} s",
            quality.samples,
            started.elapsed().as_secs_f64()
        );
        for &bar in &bars {
            let seam = bar_frame(bar);
            let first = seam.saturating_sub(2 * bar_frames);
            let last = seam + 2 * bar_frames;
            // One contiguous dry read from two bars before the seam.
            let mut reference = vec![0.0_f32; ((last - first) * 2) as usize];
            let mut state = audio::RenderState::default();
            for (index, block) in reference.chunks_mut(8192).enumerate() {
                audio::Frames::read_block(
                    &*mix,
                    first + index as u64 * 4096,
                    block,
                    None,
                    &mut state,
                );
            }
            let started = std::time::Instant::now();
            let mut worst = (0.0_f32, 0_u64, 0_u64);
            let mut owned = vec![0.0_f32; 4096];
            for attempt in 0..starts {
                // Ticks before the seam, as the cursor gives, and free frames.
                let start = if attempt % 2 == 0 {
                    let tick = (seam as f64 / fpt) as u64 - 1 - random.below(256);
                    (tick as f64 * fpt).round() as u64
                } else {
                    seam - 1 - random.below(bar_frames)
                };
                let settle = (start + u64::from(audio::Mix::SAMPLE_RATE) / 2).max(first);
                let mut state = audio::RenderState::default();
                let mut frame = start;
                // Blocks of 256, as playback reads, or irregular ones.
                while frame < last {
                    let count = if attempt % 3 == 0 {
                        1 + random.below(1500)
                    } else {
                        256
                    }
                    .min(last - frame);
                    let block = &mut owned[..(count * 2) as usize];
                    audio::Frames::read_block(&*mix, frame, block, None, &mut state);
                    for (index, pair) in block.chunks(2).enumerate() {
                        let at = frame + index as u64;
                        if at < settle {
                            continue;
                        }
                        let expected = &reference[((at - first) * 2) as usize..][..2];
                        for (x, y) in pair.iter().zip(expected) {
                            let difference = if x.is_finite() {
                                (x - y).abs()
                            } else {
                                f32::INFINITY
                            };
                            if difference > worst.0 {
                                worst = (difference, start, at);
                            }
                        }
                    }
                    frame += count;
                }
            }
            println!(
                "{:?} bar {bar}: dry, {starts} starts in {:.1} s: largest difference {:.3e} \
                 (start {}, frame {} = seam {:+})",
                quality.samples,
                started.elapsed().as_secs_f64(),
                worst.0,
                worst.1,
                worst.2,
                worst.2 as i64 - seam as i64
            );
            // The mastered live output from fewer starts.
            let started = std::time::Instant::now();
            let mut detector = audio::flight::Detector::new(audio::Mix::SAMPLE_RATE, 2);
            let window = detector.window_samples();
            let mut flattest = (f32::NEG_INFINITY, 0.0_f32, 0_u64);
            let mut flagged = 0;
            for _ in 0..live_starts {
                let start = seam - 1 - random.below(bar_frames);
                let control = MasterControl::new(&project.master);
                control.set_returns(&project.fx);
                control.set_levels(&project);
                let mut out = Vec::new();
                audio::play_offline(
                    mix.clone(),
                    start,
                    None,
                    Some(control),
                    quality,
                    last - start,
                    |block| out.extend_from_slice(block),
                );
                let s = super::sequence_scan::scan(&out);
                let noise = super::sequence_scan::loud_noise(&out);
                let mut at = 0;
                while at + window <= out.len() {
                    for reading in detector.read(&out[at..at + window]) {
                        if reading.flatness > flattest.1 {
                            flattest = (reading.rms_dbfs, reading.flatness, start + at as u64 / 2);
                        }
                        if reading.noisy() {
                            flagged += 1;
                        }
                    }
                    at += window / 2;
                }
                if s.non_finite > 0 || noise > 0 || s.peak >= 1.0 {
                    println!(
                        "  live from {start}: peak {:.3} non-finite {} loud noise {noise}",
                        s.peak, s.non_finite
                    );
                }
            }
            println!(
                "{:?} bar {bar}: live, {live_starts} starts in {:.1} s: flattest window {:.2} \
                 at {:.1} dBFS (seam {:+} frames), noisy windows {flagged}",
                quality.samples,
                started.elapsed().as_secs_f64(),
                flattest.1,
                flattest.0,
                flattest.2 as i64 - seam as i64,
            );
        }
    }
}
