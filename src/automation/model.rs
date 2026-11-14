//! The automation model: a lane's points, the lanes a track can automate,
//! a track's lanes together, and the ranges, labels, and checks that go
//! with them.
use super::filter_cutoff;
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
    /// The pan, from -1 (hard left) through 0 (center) to 1 (hard right),
    /// under the mixer's balance law. While this lane has points, it
    /// replaces the track's pan knob.
    Pan,
    /// The repeat division: 0 is off, and 4, 8, 16, 32, or 64 replays a
    /// slice of the track's own sound as 1/4 to 1/64 notes at the song
    /// tempo. Unlike the other lanes it steps: it is off before its first
    /// point and holds each point's value until the next point. Where it
    /// turns on at tick T, it captures the track's sound from T, and each
    /// division replays the slice from T that is one division long, so a
    /// faster division later in the same run keeps T and shortens the slice.
    /// It shapes the track's sound before the EQ, so the repeats go through
    /// the rest of the track's chain. See `Repeat`.
    Repeat,
    /// How much of each repeat sounds, from 5 to 100 percent of its slice.
    /// Like Repeat it steps: it is at 100% before its first point and holds
    /// each point's value until the next point. Each repeat takes the value
    /// held where it starts, plays for that share of its slice, and then
    /// falls to silence over `REPEAT_RAMP_FRAMES`. At 100% a repeat plays its
    /// whole slice. It does nothing while Repeat is off.
    RepeatGate,
    /// The playback speed in cents, from `PITCH_FLOOR_CENTS` (four octaves
    /// down, 1/16 speed) to `PITCH_CEILING_CENTS` (two octaves up, 4x
    /// speed). It is varispeed, as on a tape machine: pitch and speed move
    /// together, so a fall to the floor slows the sound to a stop. It rests
    /// at 0 before its first point, moves in straight lines in cents between
    /// points, and adds to each clip's own pitch. It acts where the clips
    /// read their sources, before the Repeat lane. See `Varispeed`.
    Pitch,
}

/// The lowest volume value, which is silence.
pub const VOLUME_FLOOR_DB: f32 = -60.0;
pub const VOLUME_CEILING_DB: f32 = 6.0;

/// The Repeat lane's values from the bottom of its row to the top: off, then
/// 1/4, 1/8, 1/16, 1/32, and 1/64 notes.
pub const REPEAT_STEPS: [f32; 6] = [0.0, 4.0, 8.0, 16.0, 32.0, 64.0];

/// The Repeat gate lane's range in percent of each repeat's slice, and the
/// step that the arrangement snaps its points to.
pub const REPEAT_GATE_FLOOR: f32 = 5.0;
pub const REPEAT_GATE_FULL: f32 = 100.0;
pub const REPEAT_GATE_STEP: f32 = 5.0;

/// The highest value of an FX bus's return lane: twice the full return,
/// about +6 dB. A throw then rises above a bus whose knob is at 100%, and the
/// bus's safety limiter still holds the return under -1 dBFS.
pub const RETURN_CEILING: f32 = 2.0;

/// The Pitch lane's range in cents: four octaves down to two octaves up.
/// Four octaves down plays at 1/16 speed, where a sound has fallen far
/// enough to read as a tape that has stopped.
pub const PITCH_FLOOR_CENTS: f32 = -4800.0;
pub const PITCH_CEILING_CENTS: f32 = 2400.0;

impl Lane {
    pub const ALL: [Self; 10] = [
        Self::Volume,
        Self::Send(0),
        Self::Send(1),
        Self::Send(2),
        Self::Send(3),
        Self::Filter,
        Self::Pan,
        Self::Repeat,
        Self::RepeatGate,
        Self::Pitch,
    ];

    pub fn label(self) -> String {
        match self {
            Self::Volume => "Volume".into(),
            Self::Send(bus) => format!("FX {} send", bus + 1),
            Self::Filter => "Filter".into(),
            Self::Pan => "Pan".into(),
            Self::Repeat => "Repeat".into(),
            Self::RepeatGate => "Repeat gate".into(),
            Self::Pitch => "Pitch".into(),
        }
    }

    /// The lane's name in agent requests: volume, send1 to send4, filter,
    /// pan, repeat, repeatgate, or pitch.
    pub fn key(self) -> String {
        match self {
            Self::Volume => "volume".into(),
            Self::Send(bus) => format!("send{}", bus + 1),
            Self::Filter => "filter".into(),
            Self::Pan => "pan".into(),
            Self::Repeat => "repeat".into(),
            Self::RepeatGate => "repeatgate".into(),
            Self::Pitch => "pitch".into(),
        }
    }

    /// Whether the lane steps, holding each point's value until the next
    /// point, instead of moving in a straight line between points.
    pub fn steps(self) -> bool {
        matches!(self, Self::Repeat | Self::RepeatGate)
    }

    /// Whether the lane rests at `before_first` before its first point
    /// instead of holding the first point's value: the stepped lanes and
    /// the Pitch lane.
    pub fn rests_before_first(self) -> bool {
        self.steps() || self == Self::Pitch
    }

    /// The value that a resting lane holds before its first point.
    pub fn before_first(self) -> f32 {
        if self == Self::RepeatGate {
            REPEAT_GATE_FULL
        } else {
            0.0
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
            Self::Filter | Self::Pan => (-1.0, 1.0),
            Self::Repeat => (0.0, 64.0),
            Self::RepeatGate => (REPEAT_GATE_FLOOR, REPEAT_GATE_FULL),
            Self::Pitch => (PITCH_FLOOR_CENTS, PITCH_CEILING_CENTS),
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
            Self::Pan => pan_label(value),
            Self::Repeat if value == 0.0 => "Off".into(),
            Self::Repeat => format!("1/{value:.0}"),
            Self::RepeatGate => format!("{value:.0}%"),
            Self::Pitch => pitch_label(value),
        }
    }
}

/// A Pitch lane value as the lane shows it, in semitones: "0 st", "-24 st",
/// or "+3.50 st".
pub fn pitch_label(cents: f32) -> String {
    let semitones = cents / 100.0;
    if semitones == 0.0 {
        "0 st".into()
    } else if semitones.fract() == 0.0 {
        format!("{semitones:+.0} st")
    } else {
        format!("{semitones:+.2} st")
    }
}

/// A pan value as the pan lane shows it: "L 40", "C", or "R 100", in percent
/// of the way to one side.
pub fn pan_label(value: f32) -> String {
    let percent = (value.abs() * 100.0).round();
    if percent == 0.0 {
        "C".into()
    } else if value < 0.0 {
        format!("L {percent:.0}")
    } else {
        format!("R {percent:.0}")
    }
}

/// The left and right gains of a pan value under the mixer's balance law:
/// the far side falls to silence and the near side stays at unity, so the
/// centre leaves both channels as they are.
pub fn pan_gains(pan: f32) -> [f32; 2] {
    [1.0 - pan.max(0.0), 1.0 + pan.min(0.0)]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pan: Vec<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repeat: Vec<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repeat_gate: Vec<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pitch: Vec<Point>,
}

fn no_sends(sends: &[Vec<Point>; crate::fx::BUSES]) -> bool {
    sends.iter().all(Vec::is_empty)
}

impl Automation {
    pub fn is_empty(&self) -> bool {
        self.volume.is_empty()
            && no_sends(&self.sends)
            && self.filter.is_empty()
            && self.pan.is_empty()
            && self.repeat.is_empty()
            && self.repeat_gate.is_empty()
            && self.pitch.is_empty()
    }

    pub fn lane(&self, lane: Lane) -> &[Point] {
        match lane {
            Lane::Volume => &self.volume,
            Lane::Send(bus) => &self.sends[bus],
            Lane::Filter => &self.filter,
            Lane::Pan => &self.pan,
            Lane::Repeat => &self.repeat,
            Lane::RepeatGate => &self.repeat_gate,
            Lane::Pitch => &self.pitch,
        }
    }

    pub fn lane_mut(&mut self, lane: Lane) -> &mut Vec<Point> {
        match lane {
            Lane::Volume => &mut self.volume,
            Lane::Send(bus) => &mut self.sends[bus],
            Lane::Filter => &mut self.filter,
            Lane::Pan => &mut self.pan,
            Lane::Repeat => &mut self.repeat,
            Lane::RepeatGate => &mut self.repeat_gate,
            Lane::Pitch => &mut self.pitch,
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
/// and within the lane's range. A Repeat point holds one of `REPEAT_STEPS`.
pub fn validate_points(lane: Lane, points: &[Point]) -> Result<(), String> {
    if lane == Lane::Repeat
        && points
            .iter()
            .any(|point| !REPEAT_STEPS.contains(&point.value))
    {
        return Err("Repeat automation values must be 0 (off), 4, 8, 16, 32, or 64.".into());
    }
    check_points(&lane.label(), lane.range(), points)
}

/// Points of `label`'s lane must be in strictly increasing tick order,
/// within the timeline, and from `low` to `high`.
fn check_points(label: &str, (low, high): (f32, f32), points: &[Point]) -> Result<(), String> {
    if points
        .iter()
        .any(|point| !(low..=high).contains(&point.value) || point.tick > crate::engine::MAX_TICKS)
    {
        return Err(format!(
            "{label} automation values must be from {low} to {high}, within the timeline."
        ));
    }
    if points.windows(2).any(|pair| pair[0].tick >= pair[1].tick) {
        return Err(format!(
            "{label} automation points must be in increasing tick order, one per tick."
        ));
    }
    Ok(())
}

/// An FX bus's return lane: points from 0 to `RETURN_CEILING` in strictly
/// increasing tick order within the timeline. While it has points, it
/// replaces the bus's return level, and it moves in straight lines.
pub fn validate_return_points(points: &[Point]) -> Result<(), String> {
    check_points("FX return", (0.0, RETURN_CEILING), points)
}

/// A return lane value as the arrangement shows it, in percent of the full
/// return: "150%".
pub fn return_label(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}

/// The return lanes of the four FX buses. A lane with points replaces its
/// bus's return level.
pub fn no_returns(lanes: &[Vec<Point>; crate::fx::BUSES]) -> bool {
    lanes.iter().all(Vec::is_empty)
}
