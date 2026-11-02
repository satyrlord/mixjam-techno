//! Track automation: breakpoint lanes that move a track's volume, its FX
//! sends, its filter, and its pan over the song, and the Repeat lane that
//! retriggers its sound.
//!
//! A lane is a list of points in tick order. Before its first point a lane
//! holds the first value, after its last point the last value, and between
//! two points it moves in a straight line. The Repeat lane is the exception:
//! it steps. It is off before its first point and holds each point's value
//! until the next point. A lane without points does nothing.
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
}

/// The lowest volume value, which is silence.
pub const VOLUME_FLOOR_DB: f32 = -60.0;
pub const VOLUME_CEILING_DB: f32 = 6.0;

/// The Repeat lane's values from the bottom of its row to the top: off, then
/// 1/4, 1/8, 1/16, 1/32, and 1/64 notes.
pub const REPEAT_STEPS: [f32; 6] = [0.0, 4.0, 8.0, 16.0, 32.0, 64.0];

impl Lane {
    pub const ALL: [Self; 8] = [
        Self::Volume,
        Self::Send(0),
        Self::Send(1),
        Self::Send(2),
        Self::Send(3),
        Self::Filter,
        Self::Pan,
        Self::Repeat,
    ];

    pub fn label(self) -> String {
        match self {
            Self::Volume => "Volume".into(),
            Self::Send(bus) => format!("FX {} send", bus + 1),
            Self::Filter => "Filter".into(),
            Self::Pan => "Pan".into(),
            Self::Repeat => "Repeat".into(),
        }
    }

    /// The lane's name in agent requests: volume, send1 to send4, filter,
    /// pan, or repeat.
    pub fn key(self) -> String {
        match self {
            Self::Volume => "volume".into(),
            Self::Send(bus) => format!("send{}", bus + 1),
            Self::Filter => "filter".into(),
            Self::Pan => "pan".into(),
            Self::Repeat => "repeat".into(),
        }
    }

    /// Whether the lane steps, holding each point's value until the next
    /// point, instead of moving in a straight line between points.
    pub fn steps(self) -> bool {
        self == Self::Repeat
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
        }
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
    }

    pub fn lane(&self, lane: Lane) -> &[Point] {
        match lane {
            Lane::Volume => &self.volume,
            Lane::Send(bus) => &self.sends[bus],
            Lane::Filter => &self.filter,
            Lane::Pan => &self.pan,
            Lane::Repeat => &self.repeat,
        }
    }

    pub fn lane_mut(&mut self, lane: Lane) -> &mut Vec<Point> {
        match lane {
            Lane::Volume => &mut self.volume,
            Lane::Send(bus) => &mut self.sends[bus],
            Lane::Filter => &mut self.filter,
            Lane::Pan => &mut self.pan,
            Lane::Repeat => &mut self.repeat,
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

/// The crossfade, in frames at the mix rate, at each retrigger of the Repeat
/// lane and where it turns on or off: 132 frames, 3 ms at 44.1 kHz. A ramp
/// of 2 ms or more keeps a cut in a sustained sound from clicking, and 3 ms
/// stays short beside the shortest slice, a 1/64 note at 300 BPM (551
/// frames), so the repeats keep their attack. It also stays within one tick
/// at 300 BPM (137.8 frames), and points are at least a tick apart, so a
/// crossfade always ends before the next point changes the lane.
pub(crate) const REPEAT_RAMP_FRAMES: u64 = 132;

/// The Repeat lane resolved to output frames.
///
/// It is a pure function of the output frame: each frame reads the track's
/// own sound (its clips, before the EQ) at another frame. A run starts where
/// the lane turns on, at tick T, and lasts until it turns off. Within a run,
/// each stretch at one division retriggers on its own first tick and then
/// every division; a retrigger plays the track's sound from T onward until
/// the next one, so the slice is one division long. The incoming slice
/// rises over `REPEAT_RAMP_FRAMES` while the outgoing one plays on and falls,
/// an equal-gain raised-cosine crossfade. At T the slice is the live sound
/// itself, so turning on needs no fade; turning off crossfades from the last
/// slice back to the live sound. Playback that starts inside a run therefore
/// plays the same repeats as an export: nothing depends on what played
/// before.
#[derive(Debug)]
pub(crate) struct Repeat {
    frames_per_tick: f64,
    runs: Vec<Run>,
}

/// A stretch where the Repeat lane is on.
#[derive(Debug)]
struct Run {
    /// The tick where the lane turned on, T, where it captures.
    capture: u32,
    /// Each stretch at one division: its first tick and the division's
    /// length in ticks. The first starts at `capture`.
    divisions: Vec<(u32, u32)>,
    /// The tick where the lane turns off; `None` holds it to the end.
    end: Option<u32>,
}

impl Run {
    /// The last retrigger tick before `tick`, which is after the run's start.
    fn last_retrigger_before(&self, tick: u32) -> u32 {
        let &(start, division) = self
            .divisions
            .iter()
            .rev()
            .find(|(start, _)| *start < tick)
            .expect("a run's first division starts before its later ticks");
        start + (tick - start - 1) / division * division
    }
}

/// Where one output frame of a Repeat lane reads its track's sound: the
/// frame to read and its weight, and, during a crossfade, the outgoing
/// frame and its weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tap {
    pub(crate) main: (u64, f32),
    pub(crate) tail: Option<(u64, f32)>,
}

impl Repeat {
    /// `None` for a lane that never turns on.
    pub(crate) fn new(points: &[Point], frames_per_tick: f64) -> Option<Self> {
        let mut runs: Vec<Run> = Vec::new();
        let mut held = 0;
        for point in points {
            // Values are checked: whole divisions of a bar.
            let value = point.value as u32;
            if value == held {
                continue;
            }
            // Off has no division; a division is a whole number of ticks.
            match crate::engine::TICKS_PER_BAR.checked_div(value) {
                None => {
                    if let Some(run) = runs.last_mut() {
                        run.end = Some(point.tick);
                    }
                }
                Some(division) => {
                    let division = division.max(1);
                    if held == 0 {
                        runs.push(Run {
                            capture: point.tick,
                            divisions: vec![(point.tick, division)],
                            end: None,
                        });
                    } else if let Some(run) = runs.last_mut() {
                        run.divisions.push((point.tick, division));
                    }
                }
            }
            held = value;
        }
        (!runs.is_empty()).then_some(Self {
            frames_per_tick,
            runs,
        })
    }

    fn frame(&self, tick: u32) -> u64 {
        (f64::from(tick) * self.frames_per_tick).round() as u64
    }

    /// The rise of the incoming sound `since` frames into a crossfade.
    fn rise(since: u64) -> f32 {
        let along = (since as f32 + 0.5) / REPEAT_RAMP_FRAMES as f32;
        0.5 - 0.5 * (std::f32::consts::PI * along).cos()
    }

    /// Where output `frame` reads the track's sound.
    pub(crate) fn tap(&self, frame: u64) -> Tap {
        let live = Tap {
            main: (frame, 1.0),
            tail: None,
        };
        let after = self
            .runs
            .partition_point(|run| self.frame(run.capture) <= frame);
        let Some(run) = after.checked_sub(1).map(|index| &self.runs[index]) else {
            return live;
        };
        let capture = self.frame(run.capture);
        if let Some(end) = run.end
            && frame >= self.frame(end)
        {
            // Off again: the live sound rises over the last slice's tail.
            let since = frame - self.frame(end);
            if since >= REPEAT_RAMP_FRAMES {
                return live;
            }
            let last = self.frame(run.last_retrigger_before(end));
            let rise = Self::rise(since);
            return Tap {
                main: (frame, rise),
                tail: Some((capture + (frame - last), 1.0 - rise)),
            };
        }
        let at = run
            .divisions
            .partition_point(|(start, _)| self.frame(*start) <= frame)
            - 1;
        let (start, division) = run.divisions[at];
        // The retrigger at or before the frame: a first guess from the
        // tempo, then corrected for the rounding of ticks to frames.
        let along = (frame as f64 / self.frames_per_tick - f64::from(start)).max(0.0);
        let mut count = (along / f64::from(division)) as u32;
        while self.frame(start + (count + 1) * division) <= frame {
            count += 1;
        }
        while count > 0 && self.frame(start + count * division) > frame {
            count -= 1;
        }
        let retrigger = start + count * division;
        let since = frame - self.frame(retrigger);
        let main = capture + since;
        if retrigger == run.capture || since >= REPEAT_RAMP_FRAMES {
            return Tap {
                main: (main, 1.0),
                tail: None,
            };
        }
        let previous = if count > 0 {
            retrigger - division
        } else {
            run.last_retrigger_before(start)
        };
        let rise = Self::rise(since);
        Tap {
            main: (main, rise),
            tail: Some((capture + (frame - self.frame(previous)), 1.0 - rise)),
        }
    }
}

/// A track's state-variable filter. Each render reader owns its history.
///
/// A low-pass and a high-pass section both run every frame, so each already
/// follows the signal when the lane crosses to its side. The output moves
/// between the low-pass, the dry input, and the high-pass over a short fade,
/// so crossing zero never clicks.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Filter {
    setting: Option<(f32, f32)>,
    low: Section,
    high: Section,
    blend: Blend,
}

/// The filter's resonance: a slight peak at the cutoff.
const FILTER_DAMPING: f32 = 1.0;

/// The fade between the low-pass, dry, and high-pass outputs.
const FILTER_FADE_SECONDS: f32 = 0.010;

/// The low-pass cutoff while the lane is not below zero.
const FILTER_LOW_OPEN_HZ: f32 = 20_000.0;

/// The high-pass cutoff while the lane is not above zero.
const FILTER_HIGH_REST_HZ: f32 = 20.0;

/// One state-variable filter with its coefficients and per-channel history.
#[derive(Clone, Copy, Debug, Default)]
struct Section {
    state: [f32; 4],
    tuning: Option<(f32, f32)>,
    coefficients: [f32; 3],
}

impl Section {
    fn tune(&mut self, cutoff: f32, sample_rate: f32) {
        if self.tuning != Some((cutoff, sample_rate)) {
            let k = FILTER_DAMPING;
            let g = (std::f32::consts::PI * cutoff.min(sample_rate * 0.45) / sample_rate).tan();
            let a1 = 1.0 / (1.0 + g * (g + k));
            self.coefficients = [a1, g * a1, g * g * a1];
            self.tuning = Some((cutoff, sample_rate));
        }
    }

    /// One sample of one channel: the band-pass and low-pass outputs.
    fn run(&mut self, channel: usize, v0: f32) -> (f32, f32) {
        let [a1, a2, a3] = self.coefficients;
        let (ic1, ic2) = (self.state[channel * 2], self.state[channel * 2 + 1]);
        let v3 = v0 - ic2;
        let v1 = a1 * ic1 + a2 * v3;
        let v2 = ic2 + a2 * ic1 + a3 * v3;
        self.state[channel * 2] = 2.0 * v1 - ic1;
        self.state[channel * 2 + 1] = 2.0 * v2 - ic2;
        (v1, v2)
    }
}

/// Which output the filter sounds: 0 low-pass, 1 dry, 2 high-pass.
#[derive(Clone, Copy, Debug, Default)]
struct Blend {
    target: Option<usize>,
    from: [f32; 3],
    elapsed: u32,
}

impl Blend {
    fn weights(&self, fade_frames: u32) -> [f32; 3] {
        let Some(target) = self.target else {
            return [0.0; 3];
        };
        let t = (self.elapsed as f32 / fade_frames as f32).min(1.0);
        let mut weights = self.from.map(|from| from * (1.0 - t));
        weights[target] += t;
        weights
    }

    /// Aim at `source`. The first call sets it at once; later changes fade
    /// from the blend as it stands.
    fn aim(&mut self, source: usize, fade_frames: u32) {
        match self.target {
            None => {
                self.from = [0.0; 3];
                self.from[source] = 1.0;
                self.elapsed = fade_frames;
            }
            Some(target) if target != source => {
                self.from = self.weights(fade_frames);
                self.elapsed = 0;
            }
            Some(_) => {}
        }
        self.target = Some(source);
    }
}

impl Filter {
    /// Filter stereo frames in place at one setting. Zero passes them through.
    pub(crate) fn process(&mut self, frames: &mut [f32], value: f32, sample_rate: f32) {
        let k = FILTER_DAMPING;
        let fade_frames = ((FILTER_FADE_SECONDS * sample_rate).round() as u32).max(1);
        if self.setting != Some((value, sample_rate)) {
            self.low.tune(
                if value < 0.0 {
                    filter_cutoff(value)
                } else {
                    FILTER_LOW_OPEN_HZ
                },
                sample_rate,
            );
            self.high.tune(
                if value > 0.0 {
                    filter_cutoff(value)
                } else {
                    FILTER_HIGH_REST_HZ
                },
                sample_rate,
            );
            self.setting = Some((value, sample_rate));
        }
        let source = if value < 0.0 {
            0
        } else if value > 0.0 {
            2
        } else {
            1
        };
        self.blend.aim(source, fade_frames);
        for frame in frames.as_chunks_mut::<2>().0 {
            let fading = self.blend.elapsed < fade_frames;
            if fading {
                self.blend.elapsed += 1;
            }
            let weights = self.blend.weights(fade_frames);
            for (channel, sample) in frame.iter_mut().enumerate() {
                let v0 = *sample;
                let (_, low) = self.low.run(channel, v0);
                let (band, high) = self.high.run(channel, v0);
                let high = v0 - k * band - high;
                let outputs = [low, v0, high];
                *sample = if fading {
                    weights[0] * outputs[0] + weights[1] * outputs[1] + weights[2] * outputs[2]
                } else {
                    outputs[source]
                };
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
        automation.pan = points(&[(0, -1.0), (16, 1.0), (32, 0.0)]);
        automation.validate().unwrap();
        for (lane, bad) in [
            (Lane::Volume, points(&[(0, 6.5)])),
            (Lane::Send(1), points(&[(0, -0.1)])),
            (Lane::Filter, points(&[(10, 0.0), (10, 0.5)])),
            (Lane::Filter, points(&[(10, 0.0), (5, 0.5)])),
            (Lane::Volume, points(&[(crate::engine::MAX_TICKS + 1, 0.0)])),
            (Lane::Volume, points(&[(0, f32::NAN)])),
            (Lane::Pan, points(&[(0, -1.01)])),
            (Lane::Pan, points(&[(0, 1.01)])),
            (Lane::Pan, points(&[(4, 0.0), (4, 1.0)])),
            (Lane::Repeat, points(&[(0, 12.0)])),
            (Lane::Repeat, points(&[(0, 16.5)])),
            (Lane::Repeat, points(&[(0, 128.0)])),
            (Lane::Repeat, points(&[(0, 2.0)])),
            (Lane::Repeat, points(&[(0, -16.0)])),
            (Lane::Repeat, points(&[(8, 16.0), (8, 0.0)])),
        ] {
            let mut changed = automation.clone();
            *changed.lane_mut(lane) = bad;
            assert!(changed.validate().is_err(), "{lane:?}");
        }
        assert_eq!(Lane::from_key("send3"), Some(Lane::Send(2)));
        assert_eq!(Lane::from_key("pan"), Some(Lane::Pan));
        assert_eq!(Lane::from_key("balance"), None);
        assert_eq!(Lane::Pan.key(), "pan");
        assert_eq!(Lane::Pan.label(), "Pan");
        assert_eq!(Lane::from_key("repeat"), Some(Lane::Repeat));
        assert_eq!(Lane::Repeat.label(), "Repeat");
        automation.repeat = points(&[(0, 0.0), (16, 4.0), (32, 8.0), (48, 16.0), (64, 32.0)]);
        automation.repeat.push(Point {
            tick: 80,
            value: 64.0,
        });
        automation.validate().unwrap();
    }

    #[test]
    fn repeat_values_read_as_off_or_note_divisions_and_step() {
        let labels: Vec<String> = REPEAT_STEPS
            .iter()
            .map(|value| Lane::Repeat.format(*value))
            .collect();
        assert_eq!(labels, ["Off", "1/4", "1/8", "1/16", "1/32", "1/64"]);
        assert!(Lane::Repeat.steps());
        assert!(
            Lane::ALL
                .iter()
                .all(|lane| lane.steps() == (*lane == Lane::Repeat))
        );
    }

    #[test]
    fn a_repeat_lane_reads_its_slice_from_where_it_turned_on() {
        let ramp = REPEAT_RAMP_FRAMES;
        // At 100 frames a tick, 1/16 (16 ticks) is 1600 frames.
        let lane = points(&[(0, 0.0), (10, 16.0), (26, 16.0), (42, 32.0), (60, 0.0)]);
        let repeat = Repeat::new(&lane, 100.0).unwrap();
        let live = |frame| Tap {
            main: (frame, 1.0),
            tail: None,
        };
        // Off before the run, and the live sound itself at its start.
        assert_eq!(repeat.tap(999), live(999));
        assert_eq!(repeat.tap(1000), live(1000));
        assert_eq!(repeat.tap(2599), live(2599));
        // A point that holds the same division does not retrigger: the
        // 1/16 retriggers every 1600 frames from 1000.
        let tap = repeat.tap(2600 + ramp);
        assert_eq!(
            tap,
            Tap {
                main: (1000 + ramp, 1.0),
                tail: None
            }
        );
        assert_eq!(repeat.tap(4200 + ramp + 5).main, (1005 + ramp, 1.0));
        // In a crossfade the old slice plays on under the new one.
        let tap = repeat.tap(2600 + 10);
        assert_eq!(tap.main.0, 1010);
        assert_eq!(tap.tail.unwrap().0, 1000 + 1600 + 10);
        assert!((tap.main.1 + tap.tail.unwrap().1 - 1.0).abs() < 1e-6);
        // At tick 42 the division turns to 1/32, 800 frames, from 1000.
        assert_eq!(repeat.tap(4200 + 799).main.0, 1000 + 799);
        assert_eq!(repeat.tap(5000 + ramp).main, (1000 + ramp, 1.0));
        // Off at tick 60, frame 6000: the live sound rises over the last
        // slice, which started at 5800.
        let tap = repeat.tap(6000 + 3);
        assert_eq!(tap.main.0, 6003);
        assert_eq!(tap.tail.unwrap().0, 1000 + 203);
        assert_eq!(repeat.tap(6000 + ramp), live(6000 + ramp));
        // A lane that never turns on is no Repeat at all.
        assert!(Repeat::new(&points(&[(0, 0.0), (64, 0.0)]), 100.0).is_none());
        assert!(Repeat::new(&[], 100.0).is_none());
    }

    #[test]
    fn pan_values_read_as_left_center_or_right() {
        assert_eq!(Lane::Pan.format(0.0), "C");
        assert_eq!(Lane::Pan.format(-0.004), "C");
        assert_eq!(Lane::Pan.format(-0.4), "L 40");
        assert_eq!(Lane::Pan.format(1.0), "R 100");
        assert_eq!(Lane::Pan.format(0.26), "R 26");
        // The balance law: the far side falls, the near side stays.
        assert_eq!(pan_gains(0.0), [1.0, 1.0]);
        assert_eq!(pan_gains(-1.0), [1.0, 0.0]);
        assert_eq!(pan_gains(1.0), [0.0, 1.0]);
        assert_eq!(pan_gains(0.25), [0.75, 1.0]);
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
        let reference =
            largest(&ramp(-0.3, -0.001, 11_025)).max(largest(&ramp(0.001, 0.3, 11_025)));
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
}
