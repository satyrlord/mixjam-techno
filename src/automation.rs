//! Track automation: breakpoint lanes that move a track's volume, its FX
//! sends, and its filter over the song.
//!
//! A lane is a list of points in tick order. Before its first point a lane
//! holds the first value, after its last point the last value, and between
//! two points it moves in a straight line. A lane without points does nothing.
use serde::{Deserialize, Serialize};

/// One automation point: a lane's value at a tick.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Point {
    pub tick: u32,
    pub value: f32,
}

/// The lanes that a track can automate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lane {
    /// Decibels added to the fader, from -60 (silence) to +6.
    Volume,
    /// The amount sent to one FX bus, from 0 to 1. While this lane has
    /// points, it replaces the track's send knob for the bus.
    Send(usize),
    /// A low-pass filter below 0 and a high-pass filter above 0, from -1 to
    /// 1. At 0 the filter is open.
    Filter,
}

/// The lowest volume value, which is silence.
pub const VOLUME_FLOOR_DB: f32 = -60.0;
pub const VOLUME_CEILING_DB: f32 = 6.0;

impl Lane {
    pub const ALL: [Self; 6] = [
        Self::Volume,
        Self::Send(0),
        Self::Send(1),
        Self::Send(2),
        Self::Send(3),
        Self::Filter,
    ];

    pub fn label(self) -> String {
        match self {
            Self::Volume => "Volume".into(),
            Self::Send(bus) => format!("FX {} send", bus + 1),
            Self::Filter => "Filter".into(),
        }
    }

    /// The lane's name in agent requests: volume, send1 to send4, or filter.
    pub fn key(self) -> String {
        match self {
            Self::Volume => "volume".into(),
            Self::Send(bus) => format!("send{}", bus + 1),
            Self::Filter => "filter".into(),
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|lane| lane.key() == key)
    }

    /// The lowest and highest value.
    pub fn range(self) -> (f32, f32) {
        match self {
            Self::Volume => (VOLUME_FLOOR_DB, VOLUME_CEILING_DB),
            Self::Send(_) => (0.0, 1.0),
            Self::Filter => (-1.0, 1.0),
        }
    }

    /// A value as the lane shows it.
    pub fn format(self, value: f32) -> String {
        match self {
            Self::Volume if value <= VOLUME_FLOOR_DB => "-inf dB".into(),
            Self::Volume => format!("{value:+.1} dB"),
            Self::Send(_) => format!("{:.0}%", value * 100.0),
            Self::Filter if value == 0.0 => "Open".into(),
            Self::Filter if value < 0.0 => format!("Low-pass {:.0} Hz", filter_cutoff(value)),
            Self::Filter => format!("High-pass {:.0} Hz", filter_cutoff(value)),
        }
    }
}

/// A track's automation lanes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Automation {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volume: Vec<Point>,
    #[serde(default, skip_serializing_if = "no_sends")]
    pub sends: [Vec<Point>; crate::fx::BUSES],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filter: Vec<Point>,
}

fn no_sends(sends: &[Vec<Point>; crate::fx::BUSES]) -> bool {
    sends.iter().all(Vec::is_empty)
}

impl Automation {
    pub fn is_empty(&self) -> bool {
        self.volume.is_empty() && no_sends(&self.sends) && self.filter.is_empty()
    }

    pub fn lane(&self, lane: Lane) -> &[Point] {
        match lane {
            Lane::Volume => &self.volume,
            Lane::Send(bus) => &self.sends[bus],
            Lane::Filter => &self.filter,
        }
    }

    pub fn lane_mut(&mut self, lane: Lane) -> &mut Vec<Point> {
        match lane {
            Lane::Volume => &mut self.volume,
            Lane::Send(bus) => &mut self.sends[bus],
            Lane::Filter => &mut self.filter,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        for lane in Lane::ALL {
            validate_points(lane, self.lane(lane))?;
        }
        Ok(())
    }
}

/// Points must be in strictly increasing tick order, within the timeline,
/// and within the lane's range.
pub fn validate_points(lane: Lane, points: &[Point]) -> Result<(), String> {
    let (low, high) = lane.range();
    if points
        .iter()
        .any(|point| !(low..=high).contains(&point.value) || point.tick > crate::engine::MAX_TICKS)
    {
        return Err(format!(
            "{} automation values must be from {low} to {high}, within the timeline.",
            lane.label()
        ));
    }
    if points.windows(2).any(|pair| pair[0].tick >= pair[1].tick) {
        return Err(format!(
            "{} automation points must be in increasing tick order, one per tick.",
            lane.label()
        ));
    }
    Ok(())
}

/// The value of a lane at a tick, which may fall between ticks.
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

/// A track's state-variable filter. Each render reader owns its history.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Filter {
    state: [f32; 4],
    setting: Option<(f32, f32)>,
    coefficients: [f32; 3],
}

/// The filter's resonance: a slight peak at the cutoff.
const FILTER_DAMPING: f32 = 1.0;

impl Filter {
    /// Filter stereo frames in place at one setting. Zero passes them through.
    pub(crate) fn process(&mut self, frames: &mut [f32], value: f32, sample_rate: f32) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn points(pairs: &[(u32, f32)]) -> Vec<Point> {
        pairs
            .iter()
            .map(|&(tick, value)| Point { tick, value })
            .collect()
    }

    #[test]
    fn a_lane_holds_its_ends_and_moves_in_lines_between_points() {
        let lane = points(&[(256, -12.0), (512, 0.0), (1024, 0.0)]);
        assert_eq!(value_at(&[], 10.0), None);
        assert_eq!(value_at(&lane, 0.0), Some(-12.0));
        assert_eq!(value_at(&lane, 384.0), Some(-6.0));
        assert_eq!(value_at(&lane, 4096.0), Some(0.0));
        let envelope = Envelope::new(&lane, 2.0).unwrap();
        assert_eq!(envelope.at(0), -12.0);
        assert_eq!(envelope.at(768), -6.0);
        assert_eq!(envelope.at(1024), 0.0);
        assert_eq!(envelope.at(100_000), 0.0);
    }

    #[test]
    fn lanes_reject_points_out_of_order_or_range() {
        let mut automation = Automation {
            volume: points(&[(0, -60.0), (256, 6.0)]),
            filter: points(&[(0, -1.0), (8, 1.0)]),
            ..Default::default()
        };
        automation.sends[2] = points(&[(0, 0.0), (64, 1.0)]);
        automation.validate().unwrap();
        for (lane, bad) in [
            (Lane::Volume, points(&[(0, 6.5)])),
            (Lane::Send(1), points(&[(0, -0.1)])),
            (Lane::Filter, points(&[(10, 0.0), (10, 0.5)])),
            (Lane::Filter, points(&[(10, 0.0), (5, 0.5)])),
            (Lane::Volume, points(&[(crate::engine::MAX_TICKS + 1, 0.0)])),
            (Lane::Volume, points(&[(0, f32::NAN)])),
        ] {
            let mut changed = automation.clone();
            *changed.lane_mut(lane) = bad;
            assert!(changed.validate().is_err(), "{lane:?}");
        }
        assert_eq!(Lane::from_key("send3"), Some(Lane::Send(2)));
        assert_eq!(Lane::from_key("pan"), None);
    }

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
}
