//! The master pedalboard: processing of the whole mix after the tracks are
//! summed, for playback and export alike.
//!
//! The mix enters through the input trim, passes the pedals that are on in
//! the board's order, and leaves for the output. The limiter is always the
//! last pedal, so nothing after it can raise the peaks it holds; the user can
//! put the others in any order. Two kinds of pedal share the board:
//! - The Booster pedals are the master effects of the D3, D4, and T3
//!   products (see `booster`). Recorded stereo oracle cases match sample for
//!   sample. Playback starts with settled compressor controls. The pedals work
//!   on 16-bit samples, as the original does.
//! - The mastering pedals are this application's own designs (see
//!   `pedals`). They work on floats. Those with a curve or a lookahead delay
//!   the sound; export removes the delay, and playback starts that much
//!   further into the mix.
//!
//! A board with every pedal off and the trim at 0 dB leaves the mix
//! untouched, and so does a bypassed board, whatever its settings: Bypass
//! skips the trim and every pedal, and the pedals keep their switches.
//! During playback, switching or moving a pedal, or the bypass, crossfades
//! from the old board to the new one, and meters read the input after the
//! trim and the output after the last pedal (see `meters`).
mod board;
mod booster;
mod chain;
mod control;
pub(crate) mod dsp;
mod knobs;
mod meters;
mod pedals;
pub mod presets;
mod processor;
mod settings;
#[cfg(test)]
mod tests;

pub use chain::MasterChain;
pub use control::MasterControl;
pub use knobs::{Knob, KnobRange, PedalKind};
pub(crate) use knobs::{range, ratio_range};
pub use meters::{MeterReading, OVER_DBTP};
pub use processor::MasterProcessor;
pub use settings::{
    BoosterCompressor, BoosterEq, BoosterWide, BusCompressor, EQ_BAND_MAX, EQ_FREQUENCIES, Imaging,
    KNOB_MAX, LiftEq, Limiter, Maximizer, Multiband, SoftClip, Tape, TrimEq, Tube,
};

#[cfg(test)]
use board::Board;
#[cfg(test)]
use processor::FADE;
