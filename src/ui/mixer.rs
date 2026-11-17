//! Compact mixer presentation. Gain magnitude and polarity remain separate project edits.
//! Each track strip also sends to the four FX buses, whose return strips follow the tracks,
//! and opens its channel EQ. The Master strip can bypass the master pedalboard.
use super::view::switch_hint;
use super::*;
use mixjam_techno::application::fx::{BUSES, FxBus, FxBuses};
use mixjam_techno::application::{
    MAX_MIXER_GAIN_DB, Silence, TrackMix, amplitude_to_db, db_to_amplitude,
};
#[path = "mixer/artwork.rs"]
mod artwork;
#[path = "mixer/buttons.rs"]
mod buttons;
#[path = "mixer/gain.rs"]
mod gain;
#[path = "mixer/strips.rs"]
mod strips;
use artwork::*;
pub(super) use artwork::{row_surface, row_surface_on};
use buttons::*;
pub(super) use buttons::{
    Look, bus_switches, bypass_button, center_button, edit_button, eq_button, pan_slider,
    polarity_button, send_knob, switch_button, unity_button,
};
use gain::*;
pub(super) use gain::{
    fraction_gain, gain_fraction, gain_keys, gain_semantics, gain_value, gain_value_in,
};
pub(super) use strips::{return_state, show, track_state};

/// What the mixer changed: the mix of each edited track with its index,
/// whether the master level moved, the FX buses, the master pedalboard's
/// bypass, the bus whose return strip asked to open it on the FX returns
/// board, and the track whose EQ button was pressed. A view edits a track's
/// mix, not a copy of the whole track.
#[derive(Default)]
pub(super) struct Changes {
    pub tracks: Vec<(usize, TrackMix)>,
    pub master: bool,
    pub fx: bool,
    pub bypass: bool,
    pub open: Option<usize>,
    pub eq: Option<usize>,
}
