//! Mastering board of the lower panel: the master pedalboard, laid out
//! as the signal flows. The mixer's output enters the input unit at the left, with its
//! trim and VU meter. It passes each pedal in order along a row that
//! scrolls, and reaches the output unit at the right, with the loudness
//! meter. A strip below the board names every pedal in signal order and
//! brings the one chosen into view. Shared hardware controls and shell own
//! presentation; the caller turns a changed chain into an edit.
use super::*;
use mixjam_techno::application::{
    Knob, KnobRange, MIX_SAMPLE_RATE, MeterReading, OVER_DBTP, presets,
};
#[path = "master/board.rs"]
mod board;
#[path = "master/deep_edit.rs"]
mod deep_edit;
#[path = "master/dials.rs"]
mod dials;
#[path = "master/enclosure.rs"]
mod enclosure;
#[path = "master/layout.rs"]
mod layout;
#[path = "master/parts.rs"]
mod parts;
#[path = "master/pedal_face.rs"]
mod pedal_face;
#[path = "master/units.rs"]
mod units;
use board::*;
pub(super) use board::{bypass_switch, preset_selector, show, status};
pub(super) use deep_edit::{DEEP_EDIT_TITLE, show_deep_edit};
use dials::*;
pub(super) use dials::{COMPACT_WELL, Dial, Format, compact_dial, decimal, dial};
use enclosure::*;
pub(super) use enclosure::{enamel_of, enclosure_of};
pub(super) use layout::MODULE_HEIGHT;
use layout::*;
pub(super) use parts::lamp;
use parts::*;
use pedal_face::*;
use units::*;

pub(super) enum Action {
    None,
    Change(MasterChain),
    OpenDeepEdit(PedalKind),
}

/// What the meters show: the latest reading, and whether a mix plays.
pub(super) struct Metering {
    pub reading: MeterReading,
    pub playing: bool,
    pub reduced: bool,
}
impl Metering {
    pub(super) fn repaint_delay(&self) -> Duration {
        Duration::from_millis(if self.reduced { 67 } else { 33 })
    }
}
pub(super) fn display_needle(ctx: &egui::Context, id: egui::Id, target: f32, reduced: bool) -> f32 {
    if reduced {
        target
    } else {
        ctx.animate_value_with_time(id, target, 0.06)
    }
}

#[cfg(test)]
#[path = "master/tests.rs"]
mod tests;
