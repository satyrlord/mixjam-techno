//! The board's measures, and where the steps in view sit across the grid.
use super::*;

/// The gap between the board's painted edge and its content.
pub(super) const BOARD_INSET: f32 = 2.0;
/// The lower panel's height that shows the whole board: the 512-point board,
/// and the panel's margins, top row, and the spacing under it.
pub(in super::super) const MODULE_HEIGHT: f32 = 512.0 + 2.0 * BOARD_INSET + 64.0;
/// The settings column's width, and the height of its content.
pub(super) const SETTINGS_WIDTH: f32 = 232.0;
pub(super) const SETTINGS_HEIGHT: f32 = 496.0;
/// Steps a pattern stores, and the steps in a bar.
pub(super) const STEPS: usize = pattern::STEPS;
pub(super) const BAR: usize = pattern::BAR_STEPS;
/// The narrowest step the grid draws. Wider boards show two or four bars.
pub(super) const MIN_STEP_WIDTH: f32 = 28.0;
/// The pitch row's pitch and its cell's height.
pub(super) const ROW: f32 = 16.0;
pub(super) const CELL_HEIGHT: f32 = 14.0;
/// The pitch rows: 25 semitones around the octave in view.
pub(super) const ROWS: i8 = 12;
/// The octaves the rows can shift by.
pub(super) const OCTAVES: std::ops::RangeInclusive<i8> = -3..=3;
/// The width of the label column at the left of the grid and lanes.
pub(super) const GUTTER: f32 = 50.0;
/// The Accent, Slide, and Velocity lanes together.
pub(super) const LANES_HEIGHT: f32 = 94.0;
/// Listen's width with its text, and the grid width below which it shows
/// its icon alone.
pub(super) const LISTEN_WIDTH: f32 = 86.0;
pub(super) const LISTEN_TEXT_WIDTH: f32 = 620.0;
/// The bar strip's width from which its keys are full size and Randomise
/// shows its word.
pub(super) const STRIP_WIDE_WIDTH: f32 = 820.0;

/// Where the grid column's steps sit: the first step in view, how many show,
/// and where each one starts.
#[derive(Clone, Copy)]
pub(super) struct Columns {
    pub(super) first: usize,
    pub(super) count: usize,
    pub(super) left: f32,
    pub(super) width: f32,
}

impl Columns {
    /// The steps in view for `grid_width` points from `left`: as many of
    /// four, two, or one bars as keep each step `MIN_STEP_WIDTH` wide, on
    /// the page that holds `view_bar`.
    pub(super) fn fit(left: f32, grid_width: f32, view_bar: usize) -> Self {
        let bars = [4, 2, 1]
            .into_iter()
            .find(|bars| grid_width / (*bars * BAR) as f32 >= MIN_STEP_WIDTH)
            .unwrap_or(1);
        Self {
            first: view_bar / bars * bars * BAR,
            count: bars * BAR,
            left,
            width: grid_width / (bars * BAR) as f32,
        }
    }

    /// The bars in view.
    pub(super) fn bars(self) -> usize {
        self.count / BAR
    }

    /// One past the last step in view.
    pub(super) fn end(self) -> usize {
        self.first + self.count
    }

    /// The last step in view.
    pub(super) fn last(self) -> usize {
        self.end() - 1
    }

    pub(super) fn x(self, step: usize) -> f32 {
        self.left + (step as f32 - self.first as f32) * self.width
    }

    pub(super) fn contains(self, step: usize) -> bool {
        (self.first..self.end()).contains(&step)
    }

    /// The step under `x`, clamped to the steps in view.
    pub(super) fn at(self, x: f32) -> usize {
        let offset = ((x - self.left) / self.width).floor().max(0.0) as usize;
        self.first + offset.min(self.count - 1)
    }

    /// The step boundary nearest `x`, from 0 to 64.
    pub(super) fn boundary(self, x: f32) -> usize {
        let offset = ((x - self.left) / self.width).round().max(0.0) as usize;
        (self.first + offset.min(self.count)).min(STEPS)
    }

    pub(super) fn span(self, from: usize, to: usize, top: f32, bottom: f32) -> Rect {
        Rect::from_min_max(Pos2::new(self.x(from), top), Pos2::new(self.x(to), bottom))
    }
}
