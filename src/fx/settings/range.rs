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
/// Common ducker and output ranges shared across FX settings.
pub(super) const fn duck_threshold_range(default: f32) -> Range {
    range(-48.0, 0.0, default, 0.5)
}
pub(super) const fn duck_amount_range(default: f32) -> Range {
    range(0.0, 100.0, default, 1.0)
}
pub(super) const fn duck_release_range(default: f32) -> Range {
    log_range(50.0, 2500.0, default, 1.0)
}
pub(super) const fn output_range(default: f32) -> Range {
    range(-24.0, 12.0, default, 0.1)
}
/// Whether a ducker's key is the bus's own input. Such a key is not saved.
pub(super) fn is_own(key: &u8) -> bool {
    *key == 0
}
