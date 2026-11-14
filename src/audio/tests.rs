//! Tests of the audio module: the prepared mix, the block mixer, playback,
//! and export. The helpers that several of the themed files use are here.
use super::{
    frames::Ramp,
    prepare::{in_parallel, load_sources, mix_with_samples_quality},
    source::{PRIORITY_REQUESTS, PlaybackSource, SYSTEM_PRIORITY},
    track::Voice,
    *,
};
use crate::{
    engine::{Clip, FadeCurve, Project, TICKS_PER_BAR},
    fx::{BUSES, FxBuses},
    master::{MasterChain, MasterControl},
};
use std::{
    collections::HashMap,
    ops::Range,
    path::Path,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

mod automation;
mod clip_reading;
mod dynamics;
mod master_board;
mod mixer;
mod pcm;
mod playback;
mod repeat;
mod returns;
mod sources;
mod stems;
mod track_effects;

pub(super) fn song() -> Project {
    Project {
        master_gain: 1.0,
        clips: vec![Clip {
            id: 1,
            sample_id: 5,
            track: 0,
            start_tick: TICKS_PER_BAR,
            length_ticks: TICKS_PER_BAR,
            offset_frames: 1,
            gain: 0.5,
            fades: Default::default(),
            pitch_cents: 0,
            reverse: false,
        }],
        ..Project::default()
    }
}

/// A steady stereo source at 0.5 in both channels, `frames` long, so a
/// track's output is its left and right gains times 0.5.
pub(super) fn steady(frames: usize) -> Pcm {
    Pcm {
        sample_rate: 44_100,
        channels: 2,
        samples: vec![0.5; frames * 2],
    }
}

/// The frames of one source for the Repeat tests: a four-bar clip on
/// track 1 at 140 BPM.
pub(super) const REPEAT_SOURCE_FRAMES: usize = 350_000;

/// A song with one clip on track 1 over four bars, whose track holds a
/// 0 dB volume lane, so that it renders in a chain with or without a
/// Repeat lane, and the Repeat lane `repeat`.
pub(super) fn repeat_song(repeat: &[(u32, f32)]) -> Project {
    use crate::automation::Point;
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 1024).unwrap();
    let mut project = editor.project().clone();
    project.master_gain = 1.0;
    project.tracks[0].automation.volume = vec![Point {
        tick: 0,
        value: 0.0,
    }];
    project.tracks[0].automation.repeat = repeat
        .iter()
        .map(|&(tick, value)| Point { tick, value })
        .collect();
    project
}

/// A source that never repeats itself: a tone in each channel and a
/// slow rise, so a frame read from the wrong place shows.
pub(super) fn unique_source() -> HashMap<u32, Pcm> {
    let rise = |i: usize| 0.2 * i as f32 / REPEAT_SOURCE_FRAMES as f32;
    let wave = |hz: f32, i: usize| 0.4 * (std::f32::consts::TAU * hz * i as f32 / 44_100.0).sin();
    HashMap::from([(
        1,
        Pcm {
            sample_rate: 44_100,
            channels: 2,
            samples: (0..REPEAT_SOURCE_FRAMES)
                .flat_map(|i| [wave(220.0, i) + rise(i), wave(330.0, i) - rise(i)])
                .collect(),
        },
    )])
}

/// The whole song as export reads it, without the board.
pub(super) fn whole(project: &Project, samples: &HashMap<u32, Pcm>) -> Vec<f32> {
    let mix = mix_with_samples(project, samples).unwrap();
    let mut out = vec![0.0; mix.frames() as usize * 2];
    mix.read_block(0, &mut out, None, &mut RenderState::default());
    out
}

/// Check that `out`, from each retrigger tick in `retriggers` plus the
/// ramp to the next one, plays the live sound from `capture` onward.
pub(super) fn assert_slices(
    out: &[f32],
    live: &[f32],
    project: &Project,
    capture: u32,
    retriggers: &[u32],
) {
    let frames_per_tick = project.frames_per_tick(Mix::SAMPLE_RATE);
    let frame = |tick: u32| (f64::from(tick) * frames_per_tick).round() as usize;
    let ramp = crate::automation::REPEAT_RAMP_FRAMES as usize;
    let at = frame(capture);
    for pair in retriggers.windows(2) {
        let (start, next) = (frame(pair[0]), frame(pair[1]));
        for since in ramp..next - start {
            for channel in 0..2 {
                let (got, want) = (
                    out[(start + since) * 2 + channel],
                    live[(at + since) * 2 + channel],
                );
                assert!(
                    (got - want).abs() < 1e-6,
                    "tick {}, {since} frames in: {got} against {want}",
                    pair[0]
                );
            }
        }
    }
}

/// A mono tone of `hz` at half scale, one second long.
pub(super) fn tone(hz: f32) -> Pcm {
    Pcm {
        sample_rate: 44_100,
        channels: 1,
        samples: (0..44_100)
            .map(|i| 0.5 * (std::f32::consts::TAU * hz * i as f32 / 44_100.0).sin())
            .collect(),
    }
}

/// The RMS level of the left channel between two frames, in dB.
pub(super) fn level_db(samples: &[f32], frames: std::ops::Range<usize>) -> f64 {
    let sum: f64 = frames
        .clone()
        .map(|frame| f64::from(samples[frame * 2]).powi(2))
        .sum();
    10.0 * (sum / frames.len() as f64).log10()
}

/// One second of a stereo source: `left` and `right` of the frame.
pub(super) fn stereo(left: impl Fn(usize) -> f32, right: impl Fn(usize) -> f32) -> Pcm {
    Pcm {
        sample_rate: 44_100,
        channels: 2,
        samples: (0..44_100).flat_map(|i| [left(i), right(i)]).collect(),
    }
}
