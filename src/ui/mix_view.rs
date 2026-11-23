//! The arrangement's Mix view: each lane header widens into a mixer row, and
//! a block below the lanes holds the Master row and the four FX returns. The
//! rows use the Mixer strips' widgets in their quiet look, with the same
//! names and edits, so the two views differ only in layout and in how
//! quietly a row at its defaults reads.
//!
//! - `layout`: the column table and the geometry that reads it. Pure, with
//!   no Ui and no session.
//! - `rows`: a track's row, the drop row's cue, and the edge fade. A row
//!   takes plain values (`RowFacts`, a `TrackMix`) and reports an `Outcome`.
//! - `block`: the captions and the Master and returns rows, and
//!   `MixJamApp::mix_block`, which places them and sends their edits.
//! - `state`: `MixViewState`, and opening and closing the view.
#[path = "mix_view/block.rs"]
pub(super) mod block;
#[path = "mix_view/layout.rs"]
pub(super) mod layout;
#[path = "mix_view/rows.rs"]
pub(super) mod rows;
#[path = "mix_view/state.rs"]
pub(super) mod state;

use super::mixer::{
    Look, bus_switches, bypass_button, center_button, edit_button, eq_button, gain_value_in,
    master_summary, pan_slider, polarity_button, return_state, row_surface, row_surface_each,
    send_knob, switch_button, track_state, unity_button,
};
use super::*;
use layout::*;
use mixjam_techno::application::fx::{BUSES, FxBuses, Module};
use mixjam_techno::application::{Silence, Track};
use rows::*;

pub(super) use block::{BLOCK_HEIGHT, BLOCK_ROW};
pub(super) use layout::{column_width, max_offset};
pub(super) use rows::{RowFacts, edge_fade, new_track_cue, track_row};
pub(super) use state::MixViewState;
