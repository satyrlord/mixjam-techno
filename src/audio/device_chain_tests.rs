//! Playback through the device's mixer, as the device callback reads it,
//! without a device.
use super::{
    Mix, Pcm,
    device_mixer::{DeviceMixer, MixerInput, Playback},
    source::PlaybackSource,
};
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
