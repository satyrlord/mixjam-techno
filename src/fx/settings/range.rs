//! The range of an effect's continuous control.
/// A continuous control's range, default, and step. A logarithmic control
/// moves by ratios.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub low: f32,
    pub high: f32,
    pub default: f32,
    pub step: f32,
    pub log: bool,
}
pub(super) const fn range(low: f32, high: f32, default: f32, step: f32) -> Range {
    Range {
        low,
        high,
        default,
        step,
        log: false,
    }
}
pub(super) const fn log_range(low: f32, high: f32, default: f32, step: f32) -> Range {
    Range {
        low,
        high,
        default,
        step,
        log: true,
    }
}
impl Range {
    /// A value held to the range. A value that is not a number gives the default.
    pub(crate) fn hold(self, value: f32) -> f32 {
        if value.is_nan() {
            self.default
        } else {
            value.clamp(self.low, self.high)
        }
    }
    /// Whether a value is a finite number in the range.
    pub(crate) fn contains(self, value: f32) -> bool {
        value.is_finite() && (self.low..=self.high).contains(&value)
    }
}
/// A ducker's key: 0, the bus's own input, or a track from 1 to 64.
pub(super) const KEY_RANGE: Range = range(0.0, crate::engine::TRACK_COUNT as f32, 0.0, 1.0);
/// Whether a ducker's key is the bus's own input. Such a key is not saved.
pub(super) fn is_own(key: &u8) -> bool {
    *key == 0
}
