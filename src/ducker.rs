//! The lane ducker: it lowers its track while another track, its key,
//! sounds, such as a bass that makes room for the kick.
//!
//! The key is the key track's own signal after its EQ, compressor, width,
//! automation, fader, and pan, before its own ducker, so duckers never
//! chain, and without the master gain, so the master gain does not change
//! how far a track ducks. A muted key, or
//! one that Solo leaves out, does not duck. The ducker follows the key's
//! peak and lowers its track by as much as the key rises over the
//! threshold, never by more than the depth; the attack and release smooth
//! the change. It works after the track's level and before its sends.
//!
//! The same key tracks feed the master pedals' and the FX buses' keys
//! (`Keys`): a dynamics pedal or an FX ducker whose key names a track
//! listens to that track instead of its own input.
use crate::master::dsp::{fast_db_to_gain, fast_log10, flush, pole_at};
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
        let value = self.range().hold(value);
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
        if !DuckerParam::ALL
            .into_iter()
            .all(|param| param.range().contains(param.get(self)))
        {
            return Err("A ducker setting is out of range.".into());
        }
        if !(1..=crate::engine::TRACK_COUNT).contains(&usize::from(self.key)) {
            return Err("The ducker's key must be a track from 1 to 64.".into());
        }
        if self.keys_itself(track) {
            return Err("The ducker's key must be another track.".into());
        }
        Ok(())
    }

    /// Whether the ducker is on and keyed by its own track, from 0.
    fn keys_itself(&self, track: usize) -> bool {
        self.on && usize::from(self.key) == track + 1
    }

    /// Set the key from a dial's value for the ducker of `track`, from 0.
    /// The value is rounded and held to tracks 1 to 64; a value that is not
    /// a number keeps the key. A track never keys its own ducker. A new key
    /// on the own track steps past it in the direction of the change, or
    /// keeps the old key at the end of the range. A ducker that is on and
    /// still keyed by its own track takes track 2 on track 1, else track 1.
    /// After the switch turns on, call it with the current key.
    pub fn set_key(&mut self, track: usize, key: f64) {
        let tracks = 1.0..=crate::engine::TRACK_COUNT as f64;
        let (own, before) = (track as f64 + 1.0, f64::from(self.key));
        let mut key = if key.is_nan() {
            before
        } else {
            key.round().clamp(*tracks.start(), *tracks.end())
        };
        if key != before && key == own {
            key = if key > before { own + 1.0 } else { own - 1.0 };
            if !tracks.contains(&key) {
                key = before;
            }
        }
        self.key = key as u8;
        if self.keys_itself(track) {
            self.key = if track == 0 { 2 } else { 1 };
        }
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
const PEAK_FALL_SECONDS: f64 = 0.010;
/// Key envelope pre-smooth pole (`0.4 ms`) suppressing hard-transient click splash.
const KEY_SMOOTH_SECONDS: f64 = 0.0004;
/// Soft knee width around `threshold`, in dB.
const KNEE_DB: f64 = 3.0;
const HALF_KNEE_DB: f64 = 0.5 * KNEE_DB;

/// A ducker's settings, ready for the audio thread.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Run {
    threshold_db: f64,
    depth_db: f64,
    attack2: f64,
    release2: f64,
    fall: f64,
    key_smooth: f64,
}

impl Run {
    pub(crate) fn new(ducker: &Ducker, rate: f32) -> Self {
        let rate = f64::from(rate);
        let attack = pole_at(f64::from(ducker.attack) / 1000.0, rate);
        let release = pole_at(f64::from(ducker.release) / 1000.0, rate);
        Self {
            threshold_db: f64::from(ducker.threshold),
            depth_db: f64::from(ducker.depth),
            attack2: attack * attack,
            release2: release * release,
            fall: pole_at(PEAK_FALL_SECONDS, rate),
            key_smooth: pole_at(KEY_SMOOTH_SECONDS, rate),
        }
    }

    #[inline]
    fn step_unflushed(&self, key: f32, state: &mut State) -> f32 {
        let raw = f64::from(key);
        state.smoothed_key = raw + (state.smoothed_key - raw) * self.key_smooth;
        state.peak = state.smoothed_key.max(state.peak * self.fall);
        if (state.phase & 1) == 0 {
            let level = 20.0 * fast_log10(state.peak.max(1e-12));
            let excess = level - self.threshold_db;
            let knee_excess = if excess <= -HALF_KNEE_DB {
                0.0
            } else if excess >= HALF_KNEE_DB {
                excess
            } else {
                let x = excess + HALF_KNEE_DB;
                (x * x) / (2.0 * KNEE_DB)
            };
            let target = knee_excess.clamp(0.0, self.depth_db);
            let speed = if target > state.reduction {
                self.attack2
            } else {
                self.release2
            };
            state.reduction = target + (state.reduction - target) * speed;
            state.cached_gain = if state.reduction < 1e-4 {
                state.reduction = 0.0;
                1.0
            } else {
                fast_db_to_gain(-state.reduction) as f32
            };
        }
        state.phase = state.phase.wrapping_add(1);
        state.cached_gain
    }

    /// The gain for one frame, given the key's level at that frame (the
    /// larger channel's magnitude). The state carries from frame to frame.
    #[inline]
    pub(crate) fn gain(&self, key: f32, state: &mut State) -> f32 {
        let g = self.step_unflushed(key, state);
        state.smoothed_key = flush(state.smoothed_key);
        state.peak = flush(state.peak);
        g
    }

    /// Duck a stereo block with constant track `level` in one tight loop.
    pub(crate) fn process_stereo_gain(
        &self,
        keys: &[f32],
        frames: &mut [f32],
        level: [f32; 2],
        state: &mut State,
    ) {
        let [l0, l1] = level;
        for (&key, frame) in keys.iter().zip(frames.as_chunks_mut::<2>().0) {
            let g = self.step_unflushed(key, state);
            frame[0] = (frame[0] * g) * l0;
            frame[1] = (frame[1] * g) * l1;
        }
        state.smoothed_key = flush(state.smoothed_key);
        state.peak = flush(state.peak);
    }
}

/// The ducker's history: the pre-smoothed key envelope, falling peak, and
/// active gain reduction in dB.
#[derive(Clone, Copy, Debug)]
pub(crate) struct State {
    smoothed_key: f64,
    peak: f64,
    reduction: f64,
    phase: u8,
    cached_gain: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            smoothed_key: 0.0,
            peak: 0.0,
            reduction: 0.0,
            phase: 0,
            cached_gain: 1.0,
        }
    }
}

impl State {
    pub(crate) fn reduction_db(&self) -> f32 {
        self.reduction as f32
    }
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

    #[test]
    fn a_ducker_never_takes_its_own_track_as_its_key() {
        let keyed = |on: bool, key: u8, track: usize, value: f64| {
            let mut ducker = Ducker {
                on,
                key,
                ..Ducker::default()
            };
            ducker.set_key(track, value);
            ducker.key
        };
        // Values round and stay on tracks 1 to 64; not a number keeps the key.
        assert_eq!(keyed(true, 3, 9, 7.4), 7);
        assert_eq!(keyed(true, 3, 9, -5.0), 1);
        assert_eq!(keyed(true, 3, 9, 200.0), 64);
        assert_eq!(keyed(true, 3, 9, f64::NAN), 3);
        // A new key on the own track steps past it in the direction of the change.
        assert_eq!(keyed(true, 3, 4, 5.0), 6);
        assert_eq!(keyed(true, 8, 4, 5.0), 4);
        assert_eq!(keyed(false, 8, 4, 5.0), 4);
        // At the end of the range it keeps the old key.
        assert_eq!(keyed(true, 3, 0, 1.0), 3);
        assert_eq!(keyed(true, 60, 63, 64.0), 60);
        // A ducker switched on with its own track as the key takes another one.
        assert_eq!(keyed(true, 1, 0, 1.0), 2);
        assert_eq!(keyed(true, 5, 4, 5.0), 1);
        assert_eq!(keyed(false, 5, 4, 5.0), 5);
        for track in 0..crate::engine::TRACK_COUNT {
            for key in 1..=crate::engine::TRACK_COUNT as u8 {
                for value in [0.0, f64::from(key), 33.0, 65.0] {
                    let mut ducker = Ducker {
                        on: true,
                        key,
                        ..Ducker::default()
                    };
                    ducker.set_key(track, value);
                    assert!(ducker.validate(track).is_ok(), "{track} {key} {value}");
                }
            }
        }
    }

    #[test]
    fn soft_knee_blends_smoothly_around_threshold() {
        let ducker = Ducker {
            on: true,
            key: 2,
            threshold: -20.0,
            depth: 12.0,
            attack: 0.1,
            release: 50.0,
        };
        let run = Run::new(&ducker, 44_100.0);
        let mut previous_db = -1.0_f64;
        for offset in [-2.0, -1.5, -0.75, 0.0, 0.75, 1.5, 2.0] {
            let key = 10_f32.powf((-20.0 + offset) / 20.0);
            let mut state = State::default();
            let gain = (0..4410).fold(1.0_f32, |_, _| run.gain(key, &mut state));
            let gr_db = -20.0 * f64::from(gain).log10();
            assert!(gr_db >= previous_db, "offset {offset}: {gr_db}");
            previous_db = gr_db;
            if offset == 0.0 {
                // At threshold inside 3 dB soft knee: KNEE_DB / 8 = 0.375 dB.
                assert!(
                    (gr_db - 0.375).abs() < 0.02,
                    "knee at threshold: {gr_db:.3}"
                );
            }
        }
    }

    #[test]
    fn fast_attack_hard_kick_suppresses_high_frequency_click_splash() {
        let ducker = Ducker {
            on: true,
            key: 2,
            threshold: -30.0,
            depth: 18.0,
            attack: 0.1,
            release: 100.0,
        };
        let run = Run::new(&ducker, 44_100.0);
        let mut state = State::default();
        let n = 4096;
        let mut ducked = vec![0.0_f64; n];
        for (i, sample) in ducked.iter_mut().enumerate() {
            let key = if (1024..1400).contains(&i) { 0.8 } else { 0.0 };
            let bass = 0.5 * (std::f64::consts::TAU * 80.0 * i as f64 / 44_100.0).sin();
            *sample = bass * f64::from(run.gain(key, &mut state));
        }
        // High-pass energy > 4 kHz via 4th-order difference proxy around click.
        let mut splash = 0.0_f64;
        for w in ducked[1000..1100].windows(2) {
            let d = w[1] - w[0];
            splash += d * d;
        }
        let rms_diff = (splash / 99.0).sqrt();
        assert!(
            rms_diff < 0.03,
            "hard kick click splash derivative RMS too high: {rms_diff:.4}"
        );
    }
}
