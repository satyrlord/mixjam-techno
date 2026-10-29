//! The lane ducker: it lowers its track while another track, its key,
//! sounds, such as a bass that makes room for the kick.
//!
//! The key is the key track's own signal after its EQ, automation, fader,
//! and pan, before its own ducker, so duckers never chain. A muted key, or
//! one that Solo leaves out, does not duck. The ducker follows the key's
//! peak and lowers its track by as much as the key rises over the
//! threshold, never by more than the depth; the attack and release smooth
//! the change. It works after the track's level and before its sends.
//!
//! The same key tracks feed the master pedals' and the FX buses' keys
//! (`Keys`): a dynamics pedal or an FX ducker whose key names a track
//! listens to that track instead of its own input.
use crate::master::{KnobRange, range, ratio_range};
use serde::{Deserialize, Serialize};

/// One track's ducker settings. The ducker keeps them while it is off.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ducker {
    pub on: bool,
    /// The key track, from 1 to 64.
    pub key: u8,
    /// The key level in dBFS above which the track ducks.
    pub threshold: f32,
    /// The most the track ducks, in dB.
    pub depth: f32,
    /// Milliseconds.
    pub attack: f32,
    /// Milliseconds.
    pub release: f32,
}

impl Default for Ducker {
    fn default() -> Self {
        let mut ducker = Self {
            on: false,
            key: 1,
            threshold: 0.0,
            depth: 0.0,
            attack: 0.0,
            release: 0.0,
        };
        for param in DuckerParam::ALL {
            param.set(&mut ducker, param.range().default);
        }
        ducker
    }
}

/// A continuous control of the ducker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DuckerParam {
    Threshold,
    Depth,
    Attack,
    Release,
}

impl DuckerParam {
    pub const ALL: [Self; 4] = [Self::Threshold, Self::Depth, Self::Attack, Self::Release];

    pub const fn range(self) -> KnobRange {
        match self {
            Self::Threshold => range(-60.0, 0.0, -30.0, 0.5),
            Self::Depth => range(0.0, 24.0, 6.0, 0.5),
            Self::Attack => ratio_range(0.1, 50.0, 2.0, 0.1),
            Self::Release => ratio_range(10.0, 1000.0, 120.0, 1.0),
        }
    }

    /// The control's name after its track's, such as "ducker depth".
    pub fn name(self) -> &'static str {
        match self {
            Self::Threshold => "ducker threshold",
            Self::Depth => "ducker depth",
            Self::Attack => "ducker attack",
            Self::Release => "ducker release",
        }
    }

    pub fn get(self, ducker: &Ducker) -> f32 {
        match self {
            Self::Threshold => ducker.threshold,
            Self::Depth => ducker.depth,
            Self::Attack => ducker.attack,
            Self::Release => ducker.release,
        }
    }

    /// Set the control, held to its range.
    pub fn set(self, ducker: &mut Ducker, value: f32) {
        let range = self.range();
        let value = if value.is_nan() {
            range.default
        } else {
            value.clamp(range.low, range.high)
        };
        match self {
            Self::Threshold => ducker.threshold = value,
            Self::Depth => ducker.depth = value,
            Self::Attack => ducker.attack = value,
            Self::Release => ducker.release = value,
        }
    }
}

impl Ducker {
    /// `track` is the ducker's own track, from 0. A ducker that is on needs
    /// another track as its key; one that is off keeps any key.
    pub fn validate(&self, track: usize) -> Result<(), String> {
        if DuckerParam::ALL.into_iter().any(|param| {
            let (value, range) = (param.get(self), param.range());
            !(value.is_finite() && (range.low..=range.high).contains(&value))
        }) {
            return Err("A ducker setting is out of range.".into());
        }
        if !(1..=crate::engine::TRACK_COUNT).contains(&usize::from(self.key)) {
            return Err("The ducker's key must be a track from 1 to 64.".into());
        }
        if self.on && usize::from(self.key) == track + 1 {
            return Err("The ducker's key must be another track.".into());
        }
        Ok(())
    }

    /// The settings and the switch as a new track has them.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The key track's index, from 0.
    pub fn key_track(&self) -> usize {
        usize::from(self.key).saturating_sub(1)
    }
}

/// How fast the key's peak falls: 10 ms.
const PEAK_FALL_SECONDS: f32 = 0.010;

/// A smoothing pole for a time constant in seconds at the mix rate.
fn pole(seconds: f32, rate: f32) -> f32 {
    (-1.0 / (seconds * rate).max(1e-6)).exp()
}

/// A ducker's settings, ready for the audio thread.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Run {
    threshold_db: f32,
    depth_db: f32,
    attack: f32,
    release: f32,
    fall: f32,
}

impl Run {
    pub(crate) fn new(ducker: &Ducker, rate: f32) -> Self {
        Self {
            threshold_db: ducker.threshold,
            depth_db: ducker.depth,
            attack: pole(ducker.attack / 1000.0, rate),
            release: pole(ducker.release / 1000.0, rate),
            fall: pole(PEAK_FALL_SECONDS, rate),
        }
    }

    /// The gain for one frame, given the key's level at that frame (the
    /// larger channel's magnitude). The state carries from frame to frame.
    #[inline]
    pub(crate) fn gain(&self, key: f32, state: &mut State) -> f32 {
        state.peak = key.max(state.peak * self.fall);
        let level = 20.0 * state.peak.max(1e-9).log10();
        let target = (level - self.threshold_db).clamp(0.0, self.depth_db);
        let speed = if target > state.reduction {
            self.attack
        } else {
            self.release
        };
        state.reduction = target + (state.reduction - target) * speed;
        if state.reduction < 1e-4 {
            state.reduction = 0.0;
            return 1.0;
        }
        10.0_f32.powf(-state.reduction / 20.0)
    }
}

/// The ducker's history: the key's falling peak and the current reduction
/// in dB.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct State {
    peak: f32,
    reduction: f32,
}

/// One block of the key tracks that the master pedals and the FX buses
/// listen to: each key track as it sounds after its fader, stereo, in the
/// slots of `tracks`, frame after frame. A key track that is not among
/// them is silent.
#[derive(Clone, Copy, Debug)]
pub struct Keys<'a> {
    /// The track, from 0, in each slot.
    pub tracks: &'a [usize],
    /// `2 * tracks.len()` samples a frame: each slot's left and right.
    pub audio: &'a [f32],
}

impl Keys<'_> {
    /// No key track sounds.
    pub const NONE: Keys<'static> = Keys {
        tracks: &[],
        audio: &[],
    };

    /// The slot of a key setting: 0 is the pedal's own input, 1 to 64 a
    /// track. `None` for the own input, and for a track outside the keys,
    /// `Some(None)`, which is silent.
    pub(crate) fn slot(&self, key: u8) -> Option<Option<usize>> {
        let track = usize::from(key).checked_sub(1)?;
        Some(self.tracks.iter().position(|&slot| slot == track))
    }

    /// The left and right samples of a slot at a frame of the block;
    /// silence for a missing slot or a frame past the audio.
    #[inline]
    pub(crate) fn frame(&self, slot: Option<usize>, frame: usize) -> (f32, f32) {
        let stride = 2 * self.tracks.len();
        match slot {
            Some(slot) if stride * (frame + 1) <= self.audio.len() => {
                let at = frame * stride + 2 * slot;
                (self.audio[at], self.audio[at + 1])
            }
            _ => (0.0, 0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_over_the_threshold_ducks_by_its_excess_up_to_the_depth() {
        let ducker = Ducker {
            on: true,
            key: 2,
            threshold: -30.0,
            depth: 6.0,
            attack: 0.1,
            release: 50.0,
        };
        let run = Run::new(&ducker, 44_100.0);
        let settle = |key: f32| {
            let mut state = State::default();
            (0..4410).fold(1.0, |_, _| run.gain(key, &mut state))
        };
        // 10 dB over the threshold: the depth caps the reduction at 6 dB.
        assert!((20.0 * settle(10f32.powf(-20.0 / 20.0)).log10() + 6.0).abs() < 0.01);
        // 3 dB over: 3 dB down.
        assert!((20.0 * settle(10f32.powf(-27.0 / 20.0)).log10() + 3.0).abs() < 0.01);
        // Under the threshold: no change.
        assert_eq!(settle(10f32.powf(-40.0 / 20.0)), 1.0);
    }

    #[test]
    fn the_track_recovers_at_the_release_after_the_key_stops() {
        let ducker = Ducker {
            on: true,
            key: 2,
            threshold: -40.0,
            depth: 12.0,
            attack: 0.1,
            release: 100.0,
        };
        let run = Run::new(&ducker, 44_100.0);
        let mut state = State::default();
        for _ in 0..4410 {
            run.gain(0.5, &mut state);
        }
        let ducked = run.gain(0.0, &mut state);
        assert!(ducked < 0.3);
        // The key's peak falls first; a quarter of a second later most of
        // the depth has come back.
        let later = (0..11_025).fold(0.0, |_, _| run.gain(0.0, &mut state));
        assert!(later > 0.8 && later < 1.0, "{later}");
        let much_later = (0..44_100).fold(0.0, |_, _| run.gain(0.0, &mut state));
        assert_eq!(much_later, 1.0);
    }

    #[test]
    fn settings_validate_their_ranges_and_their_key() {
        let mut ducker = Ducker {
            on: true,
            key: 3,
            ..Ducker::default()
        };
        assert!(ducker.validate(0).is_ok());
        assert!(ducker.validate(2).is_err(), "a track cannot key itself");
        ducker.on = false;
        assert!(ducker.validate(2).is_ok(), "while off it keeps the key");
        ducker.on = true;
        ducker.key = 0;
        assert!(ducker.validate(0).is_err());
        ducker.key = 65;
        assert!(ducker.validate(0).is_err());
        ducker.key = 3;
        ducker.depth = 30.0;
        assert!(ducker.validate(0).is_err());
        DuckerParam::Depth.set(&mut ducker, 30.0);
        assert_eq!(ducker.depth, 24.0);
        assert!(Ducker::default().is_default());
    }
}
