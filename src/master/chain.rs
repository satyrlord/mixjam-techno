//! The master chain: the input trim, every pedal's settings, and the order
//! of the pedals, with the checks that a saved chain must pass.
use super::dsp::db_to_gain;
use super::{
    BoosterCompressor, BoosterEq, BoosterWide, BusCompressor, Imaging, Knob, LiftEq, Limiter,
    Maximizer, Multiband, PedalKind, SoftClip, Tape, TrimEq, Tube,
};
use serde::{Deserialize, Serialize};

/// The input trim, every pedal with its switch, and the order the signal
/// passes through them. A pedal that is off keeps its settings and passes
/// the signal on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MasterChain {
    /// Skip the whole board, trim included, for playback and export. The
    /// pedals keep their settings and switches.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bypass: bool,
    /// The gain before the first pedal, in dB.
    pub trim: f32,
    /// Each pedal once, from the mixer's output to the master output,
    /// ending with the limiter.
    pub order: [PedalKind; 13],
    pub booster_eq: BoosterEq,
    pub booster_compressor: BoosterCompressor,
    pub booster_wide: BoosterWide,
    pub soft_clip: SoftClip,
    pub tube: Tube,
    pub trim_eq: TrimEq,
    pub bus_compressor: BusCompressor,
    pub maximizer: Maximizer,
    pub lift_eq: LiftEq,
    pub tape: Tape,
    pub imaging: Imaging,
    pub multiband: Multiband,
    pub limiter: Limiter,
}
impl Default for MasterChain {
    fn default() -> Self {
        Self {
            bypass: false,
            trim: 0.0,
            order: PedalKind::ALL,
            booster_eq: BoosterEq::default(),
            booster_compressor: BoosterCompressor::default(),
            booster_wide: BoosterWide::default(),
            soft_clip: SoftClip::default(),
            tube: Tube::default(),
            trim_eq: TrimEq::default(),
            bus_compressor: BusCompressor::default(),
            maximizer: Maximizer::default(),
            lift_eq: LiftEq::default(),
            tape: Tape::default(),
            imaging: Imaging::default(),
            multiband: Multiband::default(),
            limiter: Limiter::default(),
        }
    }
}
impl MasterChain {
    /// Whether the board changes the mix: it is not bypassed, and a pedal is
    /// on or the trim is not 0 dB.
    pub fn is_active(&self) -> bool {
        !self.bypass
            && (self.trim != 0.0 || PedalKind::ALL.into_iter().any(|kind| self.is_on(kind)))
    }
    /// Whether a pedal processes the mix: it is on and the board is not bypassed.
    pub fn runs(&self, kind: PedalKind) -> bool {
        !self.bypass && self.is_on(kind)
    }
    pub fn is_on(&self, kind: PedalKind) -> bool {
        match kind {
            PedalKind::BoosterEq => self.booster_eq.on,
            PedalKind::BoosterCompressor => self.booster_compressor.on,
            PedalKind::BoosterWide => self.booster_wide.on,
            PedalKind::SoftClip => self.soft_clip.on,
            PedalKind::Tube => self.tube.on,
            PedalKind::TrimEq => self.trim_eq.on,
            PedalKind::BusCompressor => self.bus_compressor.on,
            PedalKind::Maximizer => self.maximizer.on,
            PedalKind::LiftEq => self.lift_eq.on,
            PedalKind::Tape => self.tape.on,
            PedalKind::Imaging => self.imaging.on,
            PedalKind::Multiband => self.multiband.on,
            PedalKind::Limiter => self.limiter.on,
        }
    }
    pub fn set_on(&mut self, kind: PedalKind, on: bool) {
        match kind {
            PedalKind::BoosterEq => self.booster_eq.on = on,
            PedalKind::BoosterCompressor => self.booster_compressor.on = on,
            PedalKind::BoosterWide => self.booster_wide.on = on,
            PedalKind::SoftClip => self.soft_clip.on = on,
            PedalKind::Tube => self.tube.on = on,
            PedalKind::TrimEq => self.trim_eq.on = on,
            PedalKind::BusCompressor => self.bus_compressor.on = on,
            PedalKind::Maximizer => self.maximizer.on = on,
            PedalKind::LiftEq => self.lift_eq.on = on,
            PedalKind::Tape => self.tape.on = on,
            PedalKind::Imaging => self.imaging.on = on,
            PedalKind::Multiband => self.multiband.on = on,
            PedalKind::Limiter => self.limiter.on = on,
        }
    }
    /// A pedal's key: 0, its own input, for a pedal without one.
    pub fn key(&self, kind: PedalKind) -> u8 {
        match kind {
            PedalKind::BoosterCompressor => self.booster_compressor.key,
            PedalKind::BusCompressor => self.bus_compressor.key,
            PedalKind::Multiband => self.multiband.key,
            PedalKind::Limiter => self.limiter.key,
            _ => 0,
        }
    }
    /// The tracks, from 0, that the pedals that run listen to.
    pub fn key_tracks(&self) -> impl Iterator<Item = usize> + '_ {
        PedalKind::KEYED
            .into_iter()
            .filter(|kind| self.runs(*kind))
            .filter_map(|kind| usize::from(self.key(kind)).checked_sub(1))
    }
    /// The frames by which the pedals that run delay the sound.
    pub fn latency(&self) -> usize {
        self.order
            .iter()
            .filter(|kind| self.runs(**kind))
            .map(|kind| kind.latency())
            .sum()
    }
    /// Move the pedal at `from` to position `to`; the pedals between shift
    /// one place. Positions outside the board change nothing.
    /// The limiter stays last, so a move from or to its place changes nothing.
    pub fn move_pedal(&mut self, from: usize, to: usize) {
        let last = self.order.len() - 1;
        if from < last && to < last {
            let kind = self.order[from];
            if from < to {
                self.order.copy_within(from + 1..=to, from);
            } else {
                self.order.copy_within(to..from, to + 1);
            }
            self.order[to] = kind;
        }
    }
    /// The one knob that differs from `other`, when no switch, speed, or
    /// position differs too.
    pub fn changed_knob(&self, other: &Self) -> Option<Knob> {
        if self.bypass != other.bypass
            || self.order != other.order
            || self.tape.speed != other.tape.speed
            || PedalKind::ALL
                .into_iter()
                .any(|kind| self.is_on(kind) != other.is_on(kind))
        {
            return None;
        }
        let mut changed = Knob::all().filter(|knob| knob.get(self) != knob.get(other));
        match (changed.next(), changed.next()) {
            (Some(knob), None) => Some(knob),
            _ => None,
        }
    }
    /// The trim as a gain: unity while the board is bypassed.
    pub(super) fn trim_gain(&self) -> f64 {
        if self.bypass {
            1.0
        } else {
            db_to_gain(f64::from(self.trim))
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if PedalKind::ALL
            .iter()
            .any(|kind| self.order.iter().filter(|placed| *placed == kind).count() != 1)
        {
            return Err("The master pedal order must name each pedal once.".into());
        }
        if self.order.last() != Some(&PedalKind::Limiter) {
            return Err("The limiter must be the last master pedal.".into());
        }
        if Knob::all().any(|knob| {
            let (value, range) = (knob.get(self), knob.range());
            !(value.is_finite() && (range.low..=range.high).contains(&value))
        }) {
            return Err("A master pedal setting is out of range.".into());
        }
        if ![15, 30].contains(&self.tape.speed) {
            return Err("The tape speed must be 15 or 30 inches per second.".into());
        }
        Ok(())
    }
}
