//! Playback through the device's mixer and a sink, as the device callback
//! reads it, without a device.
use super::*;
use rodio::dynamic_mixer::{DynamicMixer, DynamicMixerController, mixer};

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

fn sink(controller: &DynamicMixerController<f32>) -> Sink {
    output::sink_on(controller, 2, Mix::SAMPLE_RATE)
}

fn pull(out: &mut DynamicMixer<f32>, frames: usize) -> Vec<[f32; 2]> {
    (0..frames)
        .map(|_| [out.next().unwrap_or(0.0), out.next().unwrap_or(0.0)])
        .collect()
}

/// Left at +0.5 and right at -0.5 throughout.
fn apart(frames: usize) -> Vec<f32> {
    (0..frames).flat_map(|_| [0.5_f32, -0.5]).collect()
}

#[test]
fn playback_keeps_left_and_right_apart_from_its_first_frame() {
    for device_rate in [44_100, 48_000] {
        let (controller, mut out) = mixer::<f32>(2, device_rate);
        let mut current = Some(Current::start(
            sink(&controller),
            playback(apart(44_100)),
            None,
        ));
        // At another rate, the mixer's resampler reads ahead when the sink
        // is added, before the playback joins it, so a short silence leads.
        let first = pull(&mut out, 400);
        assert!(
            first
                .iter()
                .all(|[left, right]| *left >= 0.0 && *right <= 0.0)
        );
        assert!(
            first[300..]
                .iter()
                .all(|[left, right]| *left > 0.4 && *right < -0.4)
        );
        for (round, lead) in [0, 2, 74, 300, 1000].into_iter().enumerate() {
            pull(&mut out, lead);
            // Each replacement of the playback, after a different number of
            // frames, starts with its own left and right. Before, the mixer
            // read the first 512 samples of each playback as one channel.
            current = Some(Current::start(
                sink(&controller),
                playback(apart(44_100)),
                current.take(),
            ));
            // The old playback ends at its next block, and the new one can
            // start after the same short silence; left and right never swap.
            for (frame, [left, right]) in pull(&mut out, 2000).into_iter().enumerate() {
                let settled = frame >= 300;
                assert!(
                    left >= 0.0 && right <= 0.0 && (!settled || (left > 0.4 && right < -0.4)),
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
        let (controller, mut out) = mixer::<f32>(2, device_rate);
        let _current = Current::start(sink(&controller), playback(sine.clone()), None);
        let left: Vec<f32> = pull(&mut out, device_rate as usize / 2)
            .into_iter()
            .map(|[left, _]| left)
            .skip_while(|left| *left == 0.0)
            .collect();
        // The sine's own curvature, sample to sample. Before, the mixer
        // restarted its resampler every 256 frames of the playback, and each
        // restart left a seam of up to four times this.
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
