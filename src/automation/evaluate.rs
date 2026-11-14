//! Reading lanes: a lane's value at a tick, a value as a gain or a cutoff,
//! and the lanes resolved to output frames for rendering.
use super::{Point, VOLUME_FLOOR_DB};

/// The value of a lane at a tick, which may fall between ticks. This is the
/// straight-line rule; the Repeat lane steps instead (`Repeat`).
pub fn value_at(points: &[Point], tick: f64) -> Option<f32> {
    let after = points.partition_point(|point| f64::from(point.tick) <= tick);
    match (
        after.checked_sub(1).map(|index| points[index]),
        points.get(after).copied(),
    ) {
        (None, None) => None,
        (Some(point), None) | (None, Some(point)) => Some(point.value),
        (Some(a), Some(b)) => {
            let t = (tick - f64::from(a.tick)) / f64::from(b.tick - a.tick);
            Some(a.value + (b.value - a.value) * t as f32)
        }
    }
}

/// A volume value as an amplitude. The floor is silence.
pub fn volume_gain(db: f32) -> f32 {
    if db <= VOLUME_FLOOR_DB {
        0.0
    } else {
        crate::engine::db_to_amplitude(db)
    }
}

/// The filter's cutoff in Hz: from 20 kHz down to 40 Hz as a low-pass, or
/// from 20 Hz up to 10 kHz as a high-pass.
pub fn filter_cutoff(value: f32) -> f32 {
    if value < 0.0 {
        20_000.0 * 0.002_f32.powf(-value)
    } else {
        20.0 * 500.0_f32.powf(value)
    }
}

/// A lane resolved to output frames for rendering.
#[derive(Debug)]
pub(crate) struct Envelope {
    frames: Vec<u64>,
    values: Vec<f32>,
}

impl Envelope {
    pub(crate) fn new(points: &[Point], frames_per_tick: f64) -> Option<Self> {
        (!points.is_empty()).then(|| Self {
            frames: points
                .iter()
                .map(|point| (f64::from(point.tick) * frames_per_tick).round() as u64)
                .collect(),
            values: points.iter().map(|point| point.value).collect(),
        })
    }

    pub(crate) fn at(&self, frame: u64) -> f32 {
        let after = self.frames.partition_point(|at| *at <= frame);
        if after == 0 {
            return self.values[0];
        }
        if after == self.frames.len() {
            return self.values[after - 1];
        }
        let (a, b) = (self.frames[after - 1], self.frames[after]);
        let t = (frame - a) as f32 / (b - a) as f32;
        self.values[after - 1] + (self.values[after] - self.values[after - 1]) * t
    }

    /// Whether the lane ever rises above zero.
    pub(crate) fn any_positive(&self) -> bool {
        self.values.iter().any(|value| *value > 0.0)
    }
}

/// The Pitch lane resolved to output frames: a varispeed clock.
///
/// At each output frame the lane plays at the speed `2^(cents / 1200)`,
/// with the cents moving in a straight line between points, at 0 before the
/// first point and at the last value after the last point. `at` gives the
/// clock's time at a frame: the integral of the speed from frame 0, in
/// frames at normal speed. A clip reads its source at the clock's distance
/// from the clip's start instead of the frame's, so pitch and speed move
/// together and the read never jumps. The integral has a closed form on
/// each segment, so the clock is a pure function of the frame: playback
/// from any frame and an export read the same positions.
#[derive(Debug)]
pub(crate) struct Varispeed {
    /// Each point's frame, its cents, and the clock's time there.
    frames: Vec<f64>,
    cents: Vec<f64>,
    times: Vec<f64>,
}

impl Varispeed {
    /// `None` for a lane without points.
    pub(crate) fn new(points: &[Point], frames_per_tick: f64) -> Option<Self> {
        if points.is_empty() {
            return None;
        }
        let frames: Vec<f64> = points
            .iter()
            .map(|point| (f64::from(point.tick) * frames_per_tick).round())
            .collect();
        let cents: Vec<f64> = points.iter().map(|point| f64::from(point.value)).collect();
        // The clock runs at normal speed up to the first point.
        let mut times = vec![frames[0]];
        for index in 1..frames.len() {
            let length = frames[index] - frames[index - 1];
            let time =
                times[index - 1] + Self::segment(cents[index - 1], cents[index], length, length);
            times.push(time);
        }
        Some(Self {
            frames,
            cents,
            times,
        })
    }

    /// The speed at `cents`.
    fn speed(cents: f64) -> f64 {
        (cents / 1200.0).exp2()
    }

    /// The clock's advance `along` frames into a segment `length` frames
    /// long whose cents move from `from` to `to`.
    fn segment(from: f64, to: f64, length: f64, along: f64) -> f64 {
        let start = Self::speed(from);
        // The speed is exp(rate * frames) times its start.
        let rate = if length > 0.0 {
            (to - from) / length * std::f64::consts::LN_2 / 1200.0
        } else {
            0.0
        };
        if rate.abs() < 1e-15 {
            start * along
        } else {
            start * (rate * along).exp_m1() / rate
        }
    }

    /// The clock's time at an output frame.
    pub(crate) fn at(&self, frame: u64) -> f64 {
        let frame = frame as f64;
        let after = self.frames.partition_point(|at| *at <= frame);
        if after == 0 {
            return frame;
        }
        let index = after - 1;
        let along = frame - self.frames[index];
        let time = self.times[index];
        match self.frames.get(after) {
            Some(next) => {
                time + Self::segment(
                    self.cents[index],
                    self.cents[after],
                    next - self.frames[index],
                    along,
                )
            }
            None => time + Self::speed(self.cents[index]) * along,
        }
    }

    /// The first frame from `from` to `limit` at which the clock has run
    /// `distance` frames past its time at `from`, or `limit`.
    pub(crate) fn reach(&self, from: u64, distance: f64, limit: u64) -> u64 {
        let base = self.at(from);
        let (mut low, mut high) = (from, limit);
        while low < high {
            let middle = low + (high - low) / 2;
            if self.at(middle) - base >= distance {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    }
}
