//! Track automation: breakpoint lanes that move a track's volume, its FX
//! sends, its filter, and its pan over the song, the Repeat lane that
//! retriggers its sound and the Repeat gate lane that shortens each repeat,
//! the Pitch lane that changes its playback speed, and the return lanes of
//! the FX buses.
//!
//! A lane is a list of points in tick order. Before its first point a lane
//! holds the first value, after its last point the last value, and between
//! two points it moves in a straight line. The Repeat and Repeat gate lanes
//! are the exceptions: they step. Repeat is off, and the gate at 100%, before
//! the first point, and each holds each point's value until the next point.
//! The Pitch lane moves in straight lines but rests at 0 before its first
//! point. A lane without points does nothing.
mod evaluate;
mod filter;
mod model;
mod repeat;
#[cfg(test)]
mod tests;

pub(crate) use evaluate::{Envelope, Varispeed};
pub use evaluate::{filter_cutoff, value_at, volume_gain};
pub(crate) use filter::Filter;
pub use filter::FilterSlope;
pub use model::{
    Automation, COMPRESSION_FULL, COMPRESSION_STEP, Lane, PITCH_CEILING_CENTS, PITCH_FLOOR_CENTS,
    Point, REPEAT_GATE_FLOOR, REPEAT_GATE_FULL, REPEAT_GATE_STEP, REPEAT_STEPS, RETURN_CEILING,
    VOLUME_CEILING_DB, VOLUME_FLOOR_DB, moved_point, no_lanes, pan_gains, pan_label, pitch_label,
    return_label, set_point, snap_return, validate_points, validate_return_points,
};
#[cfg(test)]
pub(crate) use repeat::REPEAT_RAMP_FRAMES;
pub(crate) use repeat::{Repeat, Tap};
