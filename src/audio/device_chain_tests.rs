//! Playback through the device's mixer, as the device callback reads it,
//! without a device.
use super::{
    Mix, Pcm,
    device_mixer::{DeviceMixer, MixerInput, Playback},
    flight, prepare_project_with_quality,
    source::{PlaybackSource, play_offline},
};
use crate::{library::Library, master::MasterControl};
use std::sync::Arc;

fn playback(samples: Vec<f32>) -> PlaybackSource {
    PlaybackSource::with_quality_block(
        Arc::new(Pcm {
            sample_rate: Mix::SAMPLE_RATE,
            channels: 2,
            samples,
        }),
        0,
        None,
        None,
        PlaybackSource::BLOCK_FRAMES,
        crate::playback_quality::PlaybackQuality::FULL,
    )
}

fn mixer(device_rate: u32) -> (DeviceMixer, MixerInput) {
    super::device_mixer::device_mixer(2, device_rate, Arc::default())
}

/// Play `source` in place of `old`, which plays out its block and ends.
fn replace(input: &mut MixerInput, source: PlaybackSource, old: Option<Playback>) -> Playback {
    if let Some(old) = old {
        old.end();
    }
    input.start(source, 1.0).unwrap()
}

/// One device callback of `frames` stereo frames.
fn pull(out: &mut DeviceMixer, frames: usize) -> Vec<[f32; 2]> {
    let mut buffer = vec![[0.0_f32; 2]; frames];
    out.fill(buffer.as_flattened_mut(), &mut ());
    buffer
}

/// Left at +0.5 and right at -0.5 throughout.
fn apart(frames: usize) -> Vec<f32> {
    (0..frames).flat_map(|_| [0.5_f32, -0.5]).collect()
}

#[test]
fn playback_keeps_left_and_right_apart_from_its_first_frame() {
    for device_rate in [44_100, 48_000] {
        let (mut out, mut input) = mixer(device_rate);
        let mut current = Some(replace(&mut input, playback(apart(44_100)), None));
        // The playback starts at the device's next buffer, at any rate.
        let first = pull(&mut out, 400);
        assert!(
            first
                .iter()
                .all(|[left, right]| *left > 0.4 && *right < -0.4)
        );
        for (round, lead) in [0, 2, 74, 300, 1000].into_iter().enumerate() {
            pull(&mut out, lead);
            // Each replacement of the playback, after a different number of
            // frames, starts with its own left and right.
            current = Some(replace(&mut input, playback(apart(44_100)), current.take()));
            // The old playback plays out its block beside the new one and
            // ends; left and right never swap.
            for (frame, [left, right]) in pull(&mut out, 2000).into_iter().enumerate() {
                assert!(
                    left > 0.4 && right < -0.4,
                    "{device_rate} Hz, replacement {round}, frame {frame}: {left} {right}"
                );
            }
        }
    }
}

#[test]
fn playback_resamples_for_the_device_without_seams() {
    let frames = 44_100;
    let sine: Vec<f32> = (0..frames)
        .flat_map(|i| {
            let x = 0.5 * (std::f32::consts::TAU * 1000.0 * i as f32 / 44_100.0).sin();
            [x, x]
        })
        .collect();
    for device_rate in [44_100_u32, 48_000] {
        let (mut out, mut input) = mixer(device_rate);
        let _current = replace(&mut input, playback(sine.clone()), None);
        let left: Vec<f32> = pull(&mut out, device_rate as usize / 2)
            .into_iter()
            .map(|[left, _]| left)
            .skip_while(|left| *left == 0.0)
            .collect();
        // The sine's own curvature, sample to sample. A resampler that
        // restarted at each 256-frame block of the playback would leave a
        // seam of up to four times this.
        let own = 0.5 * (2.0 * (std::f32::consts::PI * 1000.0 / device_rate as f32).sin()).powi(2);
        let largest = left[10..]
            .windows(3)
            .map(|w| (w[2] - 2.0 * w[1] + w[0]).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            largest < 1.5 * own,
            "{device_rate} Hz: {largest} against {own}"
        );
    }
}

/// Plays a real song through the whole device chain, through the device's
/// mixer, in callbacks of irregular sizes as a device asks,
/// at 44.1 and 48 kHz. Compares what the device would get with the same
/// chain read in callbacks of one size, and at 44.1 kHz with the playback
/// source alone, and scans it for broadband noise.
///
/// ```text
/// MIXJAM_SEAM_SONG=song.mixjam MIXJAM_SEAM_FROM=61,65 MIXJAM_SEAM_TO=77 cargo test --release --lib device_chain_song_scan -- --ignored --nocapture
/// ```
#[test]
#[ignore = "plays a real song"]
fn device_chain_song_scan() {
    let env = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.into());
    let path = std::path::PathBuf::from(env("MIXJAM_SEAM_SONG", ""));
    let from: Vec<u32> = env("MIXJAM_SEAM_FROM", "61")
        .split(',')
        .map(|bar| bar.trim().parse().unwrap())
        .collect();
    let to: u32 = env("MIXJAM_SEAM_TO", "77").parse().unwrap();
    let (project, library, _) =
        crate::project_io::open_song(&path, &Library::empty()).expect("opens");
    let fpt = project.frames_per_tick(Mix::SAMPLE_RATE);
    let bar_frame =
        |bar: u32| (f64::from((bar - 1) * crate::engine::TICKS_PER_BAR) * fpt).round() as u64;
    let quality = crate::playback_quality::PlaybackQuality::FULL;
    let mix: Arc<Mix> =
        Arc::new(prepare_project_with_quality(&project, &library, quality).expect("prepares"));
    let control = || {
        let control = MasterControl::new(&project.master);
        control.set_returns(&project.fx);
        control.set_levels(&project);
        control
    };
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut random = move |range: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % range
    };
    for bar in from {
        // A start at a tick, as the cursor gives.
        let tick = ((bar - 1) * crate::engine::TICKS_PER_BAR) as u64 - random(256);
        let start = (tick as f64 * fpt).round() as u64;
        let frames = bar_frame(to) - start;
        let mut alone = Vec::new();
        play_offline(
            mix.clone(),
            start,
            None,
            Some(control()),
            quality,
            frames,
            |block| alone.extend_from_slice(block),
        );
        for device_rate in [44_100_u32, 48_000] {
            let device_frames =
                (frames * u64::from(device_rate) / u64::from(Mix::SAMPLE_RATE)) as usize;
            let mut chains = Vec::new();
            for irregular in [false, true] {
                let (mut out, mut input) = mixer(device_rate);
                let source = PlaybackSource::with_quality_block(
                    mix.clone(),
                    start,
                    None,
                    Some(control()),
                    PlaybackSource::BLOCK_FRAMES,
                    quality,
                );
                let _current = replace(&mut input, source, None);
                let mut device = Vec::with_capacity(device_frames * 2);
                while device.len() < device_frames * 2 {
                    let callback = if irregular {
                        [441, 480, 512, 1024][random(4) as usize].max(1 + random(2000) as usize)
                            * usize::from(random(3) == 0)
                            + [441, 480, 512, 1024][random(4) as usize]
                    } else {
                        480
                    };
                    device.extend(pull(&mut out, callback).into_iter().flatten());
                }
                device.truncate(device_frames * 2);
                chains.push(device);
            }
            let (steady, irregular) = (&chains[0], &chains[1]);
            let differing = steady
                .iter()
                .zip(irregular)
                .position(|(a, b)| a.to_bits() != b.to_bits());
            // At the source's rate the chain plays the source as it is.
            let against_source = (device_rate == Mix::SAMPLE_RATE).then(|| {
                let lead = steady.iter().position(|x| *x != 0.0).unwrap_or(0)
                    - alone.iter().position(|x| *x != 0.0).unwrap_or(0);
                steady[lead..]
                    .iter()
                    .zip(&alone)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0_f32, f32::max)
            });
            let mut detector = flight::Detector::new(device_rate, 2);
            let window = detector.window_samples();
            let (mut flattest, mut noisy) = (0.0_f32, 0);
            let mut at = 0;
            while at + window <= irregular.len() {
                for reading in detector.read(&irregular[at..at + window]) {
                    flattest = flattest.max(reading.flatness);
                    noisy += usize::from(reading.noisy());
                }
                at += window / 2;
            }
            let peak = irregular.iter().fold(0.0_f32, |peak, x| peak.max(x.abs()));
            println!(
                "from tick {tick} (bar {bar}) to bar {to} at {device_rate} Hz: \
                 irregular callbacks differ from steady ones at {differing:?}; \
                 largest difference from the source alone {against_source:?}; \
                 peak {peak:.3}, flattest window {flattest:.2}, noisy windows {noisy}"
            );
        }
    }
}
