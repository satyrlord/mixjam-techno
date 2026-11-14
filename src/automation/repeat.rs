//! The Repeat lane resolved to output frames: where each frame reads the
//! track's own sound.
use super::{Point, REPEAT_GATE_FULL};

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
///
/// The Repeat gate shortens each repeat: a repeat that starts at a tick
/// where the gate holds G percent plays for G percent of its division and
/// then falls to silence over `REPEAT_RAMP_FRAMES`, with the falling half of
/// the same raised cosine. The gate weighs the repeat in the crossfades too,
/// so a gated repeat that is already silent does not come back in the next
/// one's rise. At 100% the gate leaves each weight as it is.
#[derive(Debug)]
pub(crate) struct Repeat {
    frames_per_tick: f64,
    runs: Vec<Run>,
    /// The Repeat gate lane's points, in percent.
    gate: Vec<Point>,
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
    /// The last retrigger tick before `tick`, which is after the run's
    /// start, and the length of its division.
    fn last_retrigger_before(&self, tick: u32) -> (u32, u32) {
        let &(start, division) = self
            .divisions
            .iter()
            .rev()
            .find(|(start, _)| *start < tick)
            .expect("a run's first division starts before its later ticks");
        (start + (tick - start - 1) / division * division, division)
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
    /// `None` for a lane that never turns on. `gate` is the Repeat gate
    /// lane.
    pub(crate) fn new(points: &[Point], gate: &[Point], frames_per_tick: f64) -> Option<Self> {
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
        (!runs.is_empty()).then(|| Self {
            frames_per_tick,
            runs,
            gate: gate.to_vec(),
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

    /// The gate's weight `since` frames into the repeat that starts at tick
    /// `retrigger` and is `division` ticks long: 1 while it is open, then a
    /// fall to silence over `REPEAT_RAMP_FRAMES`. Exactly 1 at 100%.
    fn gate(&self, retrigger: u32, division: u32, since: u64) -> f32 {
        let held = self.gate.partition_point(|point| point.tick <= retrigger);
        let percent = held
            .checked_sub(1)
            .map_or(REPEAT_GATE_FULL, |index| self.gate[index].value);
        if percent >= REPEAT_GATE_FULL {
            return 1.0;
        }
        let length = self.frame(retrigger + division) - self.frame(retrigger);
        let open = (length as f64 * f64::from(percent) / 100.0).round() as u64;
        match since.checked_sub(open) {
            None => 1.0,
            Some(closing) if closing < REPEAT_RAMP_FRAMES => 1.0 - Self::rise(closing),
            Some(_) => 0.0,
        }
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
            let (last, division) = run.last_retrigger_before(end);
            let played = frame - self.frame(last);
            let rise = Self::rise(since);
            return Tap {
                main: (frame, rise),
                tail: Some((
                    capture + played,
                    (1.0 - rise) * self.gate(last, division, played),
                )),
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
        let open = self.gate(retrigger, division, since);
        if retrigger == run.capture || since >= REPEAT_RAMP_FRAMES {
            return Tap {
                main: (main, open),
                tail: None,
            };
        }
        let (previous, length) = if count > 0 {
            (retrigger - division, division)
        } else {
            run.last_retrigger_before(start)
        };
        let played = frame - self.frame(previous);
        let rise = Self::rise(since);
        Tap {
            main: (main, rise * open),
            tail: Some((
                capture + played,
                (1.0 - rise) * self.gate(previous, length, played),
            )),
        }
    }
}
