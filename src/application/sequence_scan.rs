//! Opens many songs one after another in one session, as the window does,
//! and plays each through live playback without a device. Each song's
//! playback is compared with the same song opened and played in a fresh
//! session, so any state that one song leaves for the next shows up as a
//! difference, and the output is scanned for non-finite or runaway samples.
//!
//! ```text
//! MIXJAM_SEQUENCE_SONGS=list.txt cargo test --release --lib sequence_scan -- --ignored --nocapture
//! ```
//!
//! The list names one song per line. `MIXJAM_SEQUENCE_SECONDS` sets how much
//! of each song plays (60 seconds by default).
use super::*;
use crate::engine::Project;

fn wait_for_render(session: &mut Session) -> Result<Arc<audio::Mix>, String> {
    session.request_render()?;
    loop {
        session.poll();
        if let Some(mix) = &session.render
            && !session.render_dirty
        {
            return Ok(Arc::clone(mix));
        }
        if session.failed_revision == Some(session.revision) {
            return Err("the render failed".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[derive(Default)]
pub(super) struct Scan {
    pub(super) peak: f32,
    pub(super) non_finite: usize,
    first_non_finite: Option<usize>,
    /// Loud windows whose sample-to-sample steps are as large as white
    /// noise's: the first such frame and the count of such windows.
    noisy: usize,
    first_noisy: Option<usize>,
    /// The most noisy windows in a row.
    longest_noisy: usize,
}

pub(super) fn scan(samples: &[f32]) -> Scan {
    let mut result = Scan::default();
    for (index, &value) in samples.iter().enumerate() {
        if value.is_finite() {
            result.peak = result.peak.max(value.abs());
        } else {
            result.non_finite += 1;
            result.first_non_finite.get_or_insert(index / 2);
        }
    }
    // White noise's steps have about 1.4 times its RMS; music's far less.
    const WINDOW: usize = 1024;
    let mut run = 0;
    for (window, frames) in samples.chunks(2 * WINDOW).enumerate() {
        let (mut power, mut steps) = (0.0_f64, 0.0_f64);
        for pair in frames.chunks(2).collect::<Vec<_>>().windows(2) {
            for (&a, &b) in pair[0].iter().zip(pair[1]) {
                let (a, b) = (f64::from(a), f64::from(b));
                power += b * b;
                steps += (b - a) * (b - a);
            }
        }
        let count = (frames.len().max(4) - 2) as f64;
        let rms = (power / count).sqrt();
        if rms > 0.1 && (steps / power).sqrt() > 1.2 {
            result.noisy += 1;
            result.first_noisy.get_or_insert(window * WINDOW);
            run += 1;
            result.longest_noisy = result.longest_noisy.max(run);
        } else {
            run = 0;
        }
    }
    result
}

#[test]
#[ignore = "plays a list of real songs"]
fn sequence_scan() {
    let list = std::env::var("MIXJAM_SEQUENCE_SONGS").expect("MIXJAM_SEQUENCE_SONGS names a list");
    let seconds: f64 = std::env::var("MIXJAM_SEQUENCE_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(60.0);
    let frames = (seconds * f64::from(audio::Mix::SAMPLE_RATE)) as u64;
    let paths: Vec<PathBuf> = std::fs::read_to_string(&list)
        .expect("the list reads")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect();
    // With MIXJAM_SEQUENCE_LIBRARY set, the session starts from the library
    // folders saved in the data folder, as the window does.
    let base = if std::env::var_os("MIXJAM_SEQUENCE_LIBRARY").is_some() {
        let config = crate::project_io::load_wave_config()
            .expect("the folder settings read")
            .expect("the data folder names library folders");
        let started = std::time::Instant::now();
        let library = crate::project_io::open_wav_folders_cached(
            &crate::project_io::data_root(),
            &config.roots,
            config.source_bpm,
            None,
        )
        .expect("the library folders load");
        println!(
            "library: {} sounds in {:.1} s",
            library.samples.len(),
            started.elapsed().as_secs_f64()
        );
        library
    } else {
        Library::empty()
    };
    let mut session = Session::headless(Arc::new(base.clone()), Project::default()).unwrap();
    let mut differing = 0;
    let from: usize = std::env::var("MIXJAM_SEQUENCE_FROM")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    for (index, path) in paths.iter().enumerate().skip(from) {
        let name = path.file_stem().unwrap().to_string_lossy();
        if let Err(error) = session.open(path) {
            println!("{index:3} {name}: does not open: {error}");
            continue;
        }
        let mix = match wait_for_render(&mut session) {
            Ok(mix) => mix,
            Err(error) => {
                println!("{index:3} {name}: {error}");
                continue;
            }
        };
        // With MIXJAM_SEQUENCE_ONLY=index, the other songs open and render
        // but do not play.
        if let Ok(only) = std::env::var("MIXJAM_SEQUENCE_ONLY")
            && only.split(',').all(|value| value.parse() != Ok(index))
        {
            continue;
        }
        let quality = session.options.playback_quality();
        // As `apply_playback_buffer` does before each playback.
        session.master.set(&session.editor.project().master);
        session.master.set_returns(&session.editor.project().fx);
        session.master.set_levels(session.editor.project());
        let mut sequential = Vec::new();
        audio::play_offline(
            mix.clone(),
            0,
            None,
            Some(session.master.clone()),
            quality,
            frames,
            |block| sequential.extend_from_slice(block),
        );

        // With MIXJAM_SEQUENCE_REFERENCE=export, the reference is the
        // export of the same prepared mix instead of a fresh session's playback.
        let fresh = if std::env::var("MIXJAM_SEQUENCE_REFERENCE").as_deref() == Ok("export") {
            let project = session.editor.project();
            let mut export = match mix.to_pcm(&project.master, &project.fx) {
                Ok(pcm) => pcm.samples,
                Err(error) => {
                    println!("{index:3} {name}: does not export: {error}");
                    continue;
                }
            };
            export.truncate(
                sequential
                    .len()
                    .max((2 * frames) as usize)
                    .min(export.len()),
            );
            export
        } else {
            let (project, library, _) =
                crate::project_io::open_song(path, &base).expect("opens again");
            let fresh_mix: Arc<audio::Mix> = Arc::new(
                audio::prepare_project_with_quality(&project, &library, quality).expect("prepares"),
            );
            let control = MasterControl::new(&project.master);
            control.set_returns(&project.fx);
            control.set_levels(&project);
            let mut fresh = Vec::new();
            audio::play_offline(
                fresh_mix,
                0,
                None,
                Some(control),
                quality,
                frames,
                |block| fresh.extend_from_slice(block),
            );
            fresh
        };

        let (a, b) = (scan(&sequential), scan(&fresh));
        let mut first_difference = None;
        let mut largest = 0.0_f32;
        for (frame, (x, y)) in sequential.iter().zip(&fresh).enumerate() {
            let difference = if x.is_finite() && y.is_finite() {
                (x - y).abs()
            } else if x.to_bits() == y.to_bits() {
                0.0
            } else {
                f32::INFINITY
            };
            if difference > 0.0 {
                first_difference.get_or_insert(frame / 2);
                largest = largest.max(difference);
            }
        }
        if first_difference.is_some() || sequential.len() != fresh.len() {
            differing += 1;
        }
        println!(
            "{index:3} {name}: peak {:.3} (fresh {:.3}), non-finite {} at {:?} (fresh {}), \
             noisy windows {} at {:?} longest {} (fresh {}), \
             largest difference {largest:.3e} first at frame {first_difference:?}, lengths {} {}",
            a.peak,
            b.peak,
            a.non_finite,
            a.first_non_finite,
            b.non_finite,
            a.noisy,
            a.first_noisy,
            a.longest_noisy,
            b.noisy,
            sequential.len(),
            fresh.len(),
        );
    }
    println!("{differing} of {} songs differ", paths.len());
}

/// Loud windows of `samples` whose steps are close to white noise's.
pub(super) fn loud_noise(samples: &[f32]) -> usize {
    samples
        .chunks(2048)
        .filter(|frames| {
            let (mut power, mut steps) = (0.0_f64, 0.0_f64);
            for index in 2..frames.len() {
                let (a, b) = (f64::from(frames[index - 2]), f64::from(frames[index]));
                power += b * b;
                steps += (b - a) * (b - a);
            }
            let rms = (power / frames.len().max(1) as f64).sqrt();
            rms > 0.25 && (steps / power.max(1e-30)).sqrt() > 1.0
        })
        .count()
}

/// Plays each song of the list the ways the window can besides from the
/// start: from points inside the song, round a loop for minutes, and with
/// the pedalboard changed during playback. Prints the peak, the non-finite
/// samples, and the loud noise windows of each.
///
/// ```text
/// MIXJAM_SEQUENCE_SONGS=list.txt cargo test --release --lib live_scan -- --ignored --nocapture
/// ```
#[test]
#[ignore = "plays a list of real songs"]
fn live_scan() {
    let list = std::env::var("MIXJAM_SEQUENCE_SONGS").expect("MIXJAM_SEQUENCE_SONGS names a list");
    let paths: Vec<PathBuf> = std::fs::read_to_string(&list)
        .expect("the list reads")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect();
    let rate = u64::from(audio::Mix::SAMPLE_RATE);
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let mut flagged = 0;
    for (index, path) in paths.iter().enumerate() {
        let name = path.file_stem().unwrap().to_string_lossy();
        if let Err(error) = session.open(path) {
            println!("{index:3} {name}: does not open: {error}");
            continue;
        }
        let mix = match wait_for_render(&mut session) {
            Ok(mix) => mix,
            Err(error) => {
                println!("{index:3} {name}: {error}");
                continue;
            }
        };
        let quality = session.options.playback_quality();
        let project = session.editor.project().clone();
        let total = audio::Frames::frames(&*mix);
        let mut report = Vec::new();
        let check = |label: String, samples: &[f32], report: &mut Vec<String>| {
            let s = scan(samples);
            let loud = loud_noise(samples);
            if s.non_finite > 0 || loud > 0 {
                report.push(format!(
                    "{label}: peak {:.3} non-finite {} at {:?} loud noise {loud}",
                    s.peak, s.non_finite, s.first_non_finite
                ));
            }
        };
        let play =
            |start: u64, region: Option<[u64; 2]>, frames: u64, changes: &[(u64, MasterChain)]| {
                session.master.set(&project.master);
                session.master.set_returns(&project.fx);
                session.master.set_levels(&project);
                session.master.reset_compressors();
                let control = session.master.clone();
                let mut out = Vec::new();
                let mut played = 0_u64;
                let mut next = 0;
                audio::play_offline(
                    mix.clone(),
                    start,
                    region,
                    Some(control.clone()),
                    quality,
                    frames,
                    |block| {
                        out.extend_from_slice(block);
                        played += block.len() as u64 / 2;
                        while next < changes.len() && played >= changes[next].0 {
                            control.set(&changes[next].1);
                            next += 1;
                        }
                    },
                );
                out
            };
        // From inside the song.
        for share in [0.17, 0.41, 0.63, 0.88] {
            let start = (total as f64 * share) as u64;
            let out = play(start, None, 20 * rate, &[]);
            check(format!("from {share}"), &out, &mut report);
        }
        // Round a 10-second loop for 4 minutes: the buses ring on through it.
        let a = total * 2 / 5;
        let b = (a + 10 * rate).min(total);
        if b > a {
            let out = play(a, Some([a, b]), 240 * rate, &[]);
            check("loop".into(), &out, &mut report);
            let pass = (2 * (b - a)) as usize;
            let rms = |part: &[f32]| {
                (part.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / part.len().max(1) as f64)
                    .sqrt()
            };
            let (first, last) = (
                rms(&out[..pass.min(out.len())]),
                rms(&out[out.len().saturating_sub(pass)..]),
            );
            if last > 2.0 * first + 0.01 {
                report.push(format!("loop grows: rms {first:.3} then {last:.3}"));
            }
        }
        // The pedalboard changed during playback, then changed back.
        let presets = &crate::master::presets::PRESETS;
        let other = presets[index % presets.len()].applied_to(&project.master);
        let out = play(
            0,
            None,
            30 * rate,
            &[(10 * rate, other), (20 * rate, project.master)],
        );
        check("board change".into(), &out, &mut report);
        if !report.is_empty() {
            flagged += 1;
        }
        println!(
            "{index:3} {name}: {}",
            if report.is_empty() {
                "clean".to_string()
            } else {
                report.join("; ")
            }
        );
    }
    println!("{flagged} of {} songs flagged", paths.len());
}
