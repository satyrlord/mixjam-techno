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
pub(super) const fn width_range(default: f32) -> Range {
    range(0.0, 200.0, default, 1.0)
}
pub(super) const fn low_cut_range(default: f32) -> Range {
    log_range(20.0, 2000.0, default, 1.0)
}
pub(super) const fn high_cut_range(default: f32) -> Range {
    log_range(1000.0, 20_000.0, default, 10.0)
}
/// The ranges of the controls that every effect ends with, in this order:
/// ducker threshold, amount, release, and key, then output.
pub(super) struct CommonRanges {
    duck_threshold: Range,
    duck_amount: Range,
    duck_release: Range,
    output: Range,
}
impl CommonRanges {
    pub(super) const fn new(
        duck_threshold_db: f32,
        duck_amount_percent: f32,
        duck_release_ms: f32,
        output_db: f32,
    ) -> Self {
        Self {
            duck_threshold: range(-48.0, 0.0, duck_threshold_db, 0.5),
            duck_amount: range(0.0, 100.0, duck_amount_percent, 1.0),
            duck_release: log_range(50.0, 2500.0, duck_release_ms, 1.0),
            output: range(-24.0, 12.0, output_db, 0.1),
        }
    }
    /// An effect's range table, indexed by control. `own` pairs the index of
    /// each of the effect's own controls with its range. The common controls
    /// take the last five indexes. Each control must get one range. In a
    /// `const` table, a control with no range or two ranges stops the build.
    pub(super) const fn table<const OWN: usize, const TOTAL: usize>(
        self,
        own: [(usize, Range); OWN],
    ) -> [Range; TOTAL] {
        assert!(OWN + 5 == TOTAL, "each control needs one range");
        let common = [
            self.duck_threshold,
            self.duck_amount,
            self.duck_release,
            KEY_RANGE,
            self.output,
        ];
        let mut out = [KEY_RANGE; TOTAL];
        let mut placed = [false; TOTAL];
        let mut i = 0;
        while i < TOTAL {
            let (at, range) = if i < OWN {
                own[i]
            } else {
                (i, common[i - OWN])
            };
            assert!(!placed[at], "a control has two ranges");
            placed[at] = true;
            out[at] = range;
            i += 1;
        }
        out
    }
}
/// Whether a ducker's key is the bus's own input. Such a key is not saved.
pub(super) fn is_own(key: &u8) -> bool {
    *key == 0
}

#[cfg(test)]
mod tests {
    use super::super::{DelayParam, DistortionParam, ReverbParam};
    use super::*;

    const COMMON: CommonRanges = CommonRanges::new(-20.0, 50.0, 300.0, 0.0);

    #[test]
    fn a_range_table_places_each_range_at_its_named_control() {
        let first = range(1.0, 2.0, 1.0, 1.0);
        let second = log_range(10.0, 20.0, 10.0, 1.0);
        let table: [Range; 7] = COMMON.table([(1, first), (0, second)]);
        assert_eq!(
            table,
            [
                second,
                first,
                COMMON.duck_threshold,
                COMMON.duck_amount,
                COMMON.duck_release,
                KEY_RANGE,
                COMMON.output,
            ]
        );
    }

    #[test]
    #[should_panic(expected = "a control has two ranges")]
    fn a_range_table_refuses_a_control_with_two_ranges() {
        let own = range(1.0, 2.0, 1.0, 1.0);
        let _: [Range; 7] = COMMON.table([(1, own), (1, own)]);
    }

    #[test]
    #[should_panic(expected = "a control has two ranges")]
    fn a_range_table_refuses_an_own_control_in_a_common_slot() {
        let own = range(1.0, 2.0, 1.0, 1.0);
        let _: [Range; 7] = COMMON.table([(0, own), (6, own)]);
    }

    #[test]
    #[should_panic(expected = "each control needs one range")]
    fn a_range_table_refuses_a_control_without_a_range() {
        let own = range(1.0, 2.0, 1.0, 1.0);
        let _: [Range; 8] = COMMON.table([(1, own), (0, own)]);
    }

    /// The common ranges sit at the last five indexes, so each effect's
    /// controls must end with the common controls in their order.
    #[test]
    fn each_effect_ends_with_the_common_controls() {
        let common = |all: &[usize], tail: [usize; 5]| {
            let start = all.len() - 5;
            assert_eq!(tail, [start, start + 1, start + 2, start + 3, start + 4]);
            assert!(all.iter().enumerate().all(|(i, &at)| i == at));
        };
        use DelayParam as D;
        common(
            &D::ALL.map(|p| p as usize),
            [
                D::DuckThreshold,
                D::DuckAmount,
                D::DuckRelease,
                D::DuckKey,
                D::Output,
            ]
            .map(|p| p as usize),
        );
        use DistortionParam as E;
        common(
            &E::ALL.map(|p| p as usize),
            [
                E::DuckThreshold,
                E::DuckAmount,
                E::DuckRelease,
                E::DuckKey,
                E::Output,
            ]
            .map(|p| p as usize),
        );
        use ReverbParam as R;
        common(
            &R::ALL.map(|p| p as usize),
            [
                R::DuckThreshold,
                R::DuckAmount,
                R::DuckRelease,
                R::DuckKey,
                R::Output,
            ]
            .map(|p| p as usize),
        );
    }
}
