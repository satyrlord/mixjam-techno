use super::*;

#[test]
fn the_filter_cuts_below_or_above_its_cutoff_and_opens_at_zero() {
    let rate = 44_100.0;
    let level = |value: f32, hz: f32| {
        let mut filter = Filter::default();
        let mut frames: Vec<f32> = (0..8820)
            .flat_map(|frame| {
                let x = (std::f32::consts::TAU * hz * frame as f32 / rate).sin();
                [x, x]
            })
            .collect();
        filter.process(&mut frames, value, rate);
        let tail = &frames[8820..];
        (tail.iter().map(|x| x * x).sum::<f32>() / tail.len() as f32 * 2.0).sqrt()
    };
    assert!((level(0.0, 1000.0) - 1.0).abs() < 1e-3);
    // A low-pass near 200 Hz passes the bass and removes the treble.
    assert!(level(-0.75, 60.0) > 0.9);
    assert!(level(-0.75, 5000.0) < 0.01);
    // A high-pass near 450 Hz does the opposite, 12 dB an octave.
    assert!(level(0.5, 60.0) < 0.03);
    assert!(level(0.5, 8000.0) > 0.9);
    assert!((filter_cutoff(-1.0) - 40.0).abs() < 0.1);
    assert!((filter_cutoff(1.0) - 10_000.0).abs() < 1.0);
}

/// The filter as it was before the crossing fix: one state serves both
/// modes, so its cutoff jumps when the lane crosses zero.
#[derive(Default)]
struct SingleState {
    state: [f32; 4],
    setting: Option<(f32, f32)>,
    coefficients: [f32; 3],
}

impl SingleState {
    fn process(&mut self, frames: &mut [f32], value: f32, sample_rate: f32) {
        let k = FILTER_DAMPING;
        if self.setting != Some((value, sample_rate)) {
            let cutoff = filter_cutoff(value).min(sample_rate * 0.45);
            let g = (std::f32::consts::PI * cutoff / sample_rate).tan();
            let a1 = 1.0 / (1.0 + g * (g + k));
            self.coefficients = [a1, g * a1, g * g * a1];
            self.setting = Some((value, sample_rate));
        }
        let [a1, a2, a3] = self.coefficients;
        let state = &mut self.state;
        for frame in frames.as_chunks_mut::<2>().0 {
            for (channel, sample) in frame.iter_mut().enumerate() {
                let (ic1, ic2) = (state[channel * 2], state[channel * 2 + 1]);
                let v0 = *sample;
                let v3 = v0 - ic2;
                let v1 = a1 * ic1 + a2 * v3;
                let v2 = ic2 + a2 * ic1 + a3 * v3;
                state[channel * 2] = 2.0 * v1 - ic1;
                state[channel * 2 + 1] = 2.0 * v2 - ic2;
                if value < 0.0 {
                    *sample = v2;
                } else if value > 0.0 {
                    *sample = v0 - k * v1 - v2;
                }
            }
        }
    }
}

const RATE: f32 = 44_100.0;

/// A steady 100 Hz plus 3 kHz signal, left channel, one value per frame.
fn run_lane(
    frames: usize,
    value: impl Fn(usize) -> f32,
    mut process: impl FnMut(&mut [f32], f32),
) -> Vec<f32> {
    let mut left = Vec::with_capacity(frames);
    for frame in 0..frames {
        let t = frame as f32 / RATE;
        let x = 0.4 * (std::f32::consts::TAU * 100.0 * t).sin()
            + 0.4 * (std::f32::consts::TAU * 3000.0 * t).sin();
        let mut pair = [x, x];
        process(&mut pair, value(frame));
        left.push(pair[0]);
    }
    left
}

fn largest_step(samples: &[f32]) -> f32 {
    samples
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0, f32::max)
}

/// A linear move from `from` to `to` over 132 frames (3 ms) from frame `at`.
fn ramp(from: f32, to: f32, at: usize) -> impl Fn(usize) -> f32 {
    move |frame| from + (to - from) * frame.saturating_sub(at).min(132) as f32 / 132.0
}

type Process = Box<dyn FnMut(&mut [f32], f32)>;

/// The largest step in the 20 ms around a crossing at frame 11025, for
/// the lane `crossing` and for the larger of the same moves that stay on
/// one side of zero.
fn crossing_and_reference(
    crossing: impl Fn(usize) -> f32,
    make: impl Fn() -> Process,
) -> (f32, f32) {
    let largest = |lane: &dyn Fn(usize) -> f32| {
        let mut process = make();
        let out = run_lane(22_050, lane, &mut process);
        largest_step(&out[11_025 - 882..11_025 + 1_100])
    };
    let near = largest(&crossing);
    let reference = largest(&ramp(-0.3, -0.001, 11_025)).max(largest(&ramp(0.001, 0.3, 11_025)));
    (near, reference)
}

fn lanes() -> [Box<dyn Fn(usize) -> f32>; 2] {
    [
        // A ramp across zero in a few milliseconds.
        Box::new(ramp(-0.3, 0.3, 11_025)),
        // Down to exactly zero, a hold there, and up the other side.
        Box::new(|frame| match frame {
            ..11_157 => ramp(-0.3, 0.0, 11_025)(frame),
            11_157..11_257 => 0.0,
            _ => ramp(0.0, 0.3, 11_257)(frame),
        }),
    ]
}

#[test]
fn the_filter_crosses_between_low_pass_and_high_pass_without_a_click() {
    for (index, lane) in lanes().into_iter().enumerate() {
        let (near, reference) = crossing_and_reference(&lane, || {
            let mut filter = Filter::default();
            Box::new(move |frames, value| filter.process(frames, value, RATE))
        });
        assert!(
            near <= reference + 0.03,
            "lane {index}: crossing step {near} against {reference}"
        );
        // The single-state filter this replaced clicks across a direct
        // crossing. A hold at exactly zero passed the signal dry.
        if index != 0 {
            continue;
        }
        let (old, old_reference) = crossing_and_reference(&lane, || {
            let mut filter = SingleState::default();
            Box::new(move |frames, value| filter.process(frames, value, RATE))
        });
        assert!(
            old > old_reference + 0.1,
            "lane {index}: the old crossing step {old} against {old_reference}"
        );
    }
}

#[test]
fn a_lane_that_stays_on_one_side_sounds_as_it_did() {
    let ramps: [Box<dyn Fn(usize) -> f32>; 4] = [
        Box::new(|_| -0.3),
        Box::new(|frame| -0.9 + 0.8 * (frame.min(2_000) as f32 / 2_000.0)),
        Box::new(|_| 0.5),
        Box::new(|frame| 0.9 - 0.8 * (frame.min(2_000) as f32 / 2_000.0)),
    ];
    for (index, lane) in ramps.into_iter().enumerate() {
        let mut filter = Filter::default();
        let new = run_lane(6_000, &lane, |frames, value| {
            filter.process(frames, value, RATE);
        });
        let mut filter = SingleState::default();
        let old = run_lane(6_000, &lane, |frames, value| {
            filter.process(frames, value, RATE);
        });
        assert!(new == old, "lane {index} changed");
    }
}

#[test]
fn a_filter_fade_in_progress_continues_when_the_side_changes_again() {
    // Low-pass to high-pass, then back to low-pass while that fade runs.
    let lane = |frame: usize| match frame {
        ..2_100 => ramp(-0.3, 0.3, 2_000)(frame),
        _ => ramp(0.3, -0.3, 2_200)(frame),
    };
    let mut filter = Filter::default();
    let out = run_lane(5_000, lane, |frames, value| {
        filter.process(frames, value, RATE);
    });
    let step = largest_step(&out[1_800..4_000]);
    assert!(step < 0.26, "{step}");
}
