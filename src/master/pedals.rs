//! The mastering pedals, designed for this application. Each works on
//! normalized stereo floats with its own state, and each continuous setting
//! glides to a new value. At full quality, pedals with a curve run it four
//! times oversampled and delay the signal by `Oversampler::LATENCY`. Reduced
//! playback runs their curves at the base rate with the same integer delay.
//! The limiter always keeps its lookahead and true-peak detector.
mod dynamics;
mod filters;
mod saturation;
#[cfg(test)]
mod tests;

pub(super) use dynamics::{BusCompressor, Limiter, Multiband};
pub(super) use filters::{Imaging, LiftEq, TrimEq};
pub(super) use saturation::{Maximizer, SoftClip, Tape, Tube};

/// How often a pedal whose filters glide recomputes their coefficients.
const REFRESH: u32 = 16;

/// Counts down the frames until a gliding pedal next redesigns its filters.
#[derive(Clone, Copy)]
struct Refresh(u32);
impl Refresh {
    /// True when the filters need new coefficients: every `REFRESH` frames
    /// while a setting glides, and once more when it arrives.
    fn due(&mut self, gliding: bool, arrived: bool) -> bool {
        if !gliding {
            return false;
        }
        self.0 = self.0.saturating_sub(1);
        if self.0 == 0 || arrived {
            self.0 = REFRESH;
            return true;
        }
        false
    }
}
