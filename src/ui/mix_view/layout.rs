//! The Mix view's column table: where each column of a row sits, how the
//! sounds' column takes spare width, and when the columns scroll. Rows,
//! captions, the Master and returns rows, and the tests all read it, so a
//! column moves in one place.
use super::*;

/// A column of a Mix view row: its left and right edges, in points from the
/// row's left edge, before the columns scroll.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in super::super) struct Column {
    pub left: f32,
    pub right: f32,
}

impl Column {
    pub(in super::super) const fn new(left: f32, right: f32) -> Self {
        Self { left, right }
    }

    /// The column `width` points wide that starts `gap` points after this
    /// one.
    const fn then(self, gap: f32, width: f32) -> Self {
        Self::new(self.right + gap, self.right + gap + width)
    }

    /// From this column's left edge to `last`'s right edge.
    pub(in super::super) const fn through(self, last: Self) -> Self {
        Self::new(self.left, last.right)
    }

    pub(in super::super) const fn width(self) -> f32 {
        self.right - self.left
    }

    /// The cell `index` of `count` equal cells.
    pub(in super::super) const fn cell(self, index: usize, count: usize) -> Self {
        let width = self.width() / count as f32;
        let left = self.left + width * index as f32;
        Self::new(left, left + width)
    }

    /// This column when the sounds' column has `room` more points: a column
    /// that ends where the track column ends widens, and a column after it
    /// moves right.
    const fn placed(self, room: f32) -> Self {
        if self.left >= TRACK.right {
            Self::new(self.left + room, self.right + room)
        } else if self.right >= TRACK.right {
            Self::new(self.left, self.right + room)
        } else {
            self
        }
    }
}

/// The track number stays in place while the other columns scroll.
pub(in super::super) const PINNED: f32 = 34.0;
/// The width of a switch button, in a cell of `SWITCH_CELL`.
pub(super) const SWITCH: f32 = 22.0;
const SWITCH_CELL: f32 = 24.0;
/// A send's knob, at the left of its cell.
pub(super) const KNOB: f32 = 18.0;
const SEND_CELL: f32 = 42.0;

/// The track column: the category mark, then the sounds.
pub(in super::super) const TRACK: Column = Column::new(40.0, 150.0);
pub(super) const MARK: Column = Column::new(TRACK.left, TRACK.left + 8.0);
pub(in super::super) const SOUNDS: Column = Column::new(52.0, TRACK.right);
/// M, S, L, and R, one cell each.
pub(in super::super) const SWITCHES: Column = TRACK.then(2.0, SWITCH_CELL * 4.0);
pub(in super::super) const POLARITY: Column = SWITCHES.then(0.0, SWITCH_CELL);
pub(in super::super) const FADER: Column = POLARITY.then(8.0, 136.0);
pub(in super::super) const VALUE: Column = FADER.then(4.0, 56.0);
pub(in super::super) const UNITY: Column = VALUE.then(2.0, 36.0);
pub(in super::super) const PAN: Column = UNITY.then(10.0, 70.0);
pub(in super::super) const CENTER: Column = PAN.then(2.0, 40.0);
pub(in super::super) const STATE: Column = CENTER.then(6.0, 84.0);
/// One cell per FX bus: its knob, then its amount while it sends.
pub(in super::super) const SENDS: Column = STATE.then(6.0, SEND_CELL * BUSES as f32);
pub(in super::super) const EQ: Column = SENDS.then(6.0, 32.0);
/// The returns row shares this span among its buses.
pub(in super::super) const RETURNS: Column = TRACK.through(EQ);

/// The width of a row's columns; a narrower header column scrolls them.
pub(in super::super) const CONTENT_WIDTH: f32 = EQ.right + 6.0;
/// The lanes keep at least this width beside the Mix view's column.
const LANES_MIN: f32 = 160.0;
/// When the lanes have room, the column grows past its content by up to this
/// much, and the sounds' column takes it.
const NAME_ROOM: f32 = 40.0;

/// The send cell of `bus`.
pub(in super::super) const fn send_cell(bus: usize) -> Column {
    SENDS.cell(bus, BUSES)
}

/// The send knob of `bus`, and where its amount shows.
pub(super) const fn send_parts(bus: usize) -> (Column, Column) {
    let cell = send_cell(bus);
    (
        Column::new(cell.left, cell.left + KNOB),
        Column::new(cell.left + KNOB + 3.0, cell.right),
    )
}

/// The header column's width in the Mix view for a score this wide.
pub(in super::super) fn column_width(available: f32) -> f32 {
    (available - LANES_MIN).clamp(lanes::LANE_HEADER_WIDTH, CONTENT_WIDTH + NAME_ROOM)
}

/// The room a `column`-wide header gives the sounds' column past its own
/// width; the columns after it shift right by as much.
fn name_room(column: f32) -> f32 {
    (column - CONTENT_WIDTH).clamp(0.0, NAME_ROOM)
}

/// The largest horizontal offset of the columns in a header this wide.
pub(in super::super) fn max_offset(column: f32) -> f32 {
    (CONTENT_WIDTH - column).max(0.0)
}

/// Where a row's columns are on screen: the row, how far its columns
/// scrolled, and the sounds' extra room in a wide header.
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct Columns {
    pub(super) row: Rect,
    offset: f32,
    room: f32,
}

impl Columns {
    pub(in super::super) fn of(row: Rect, offset: f32) -> Self {
        Self {
            row,
            offset,
            room: name_room(row.width()),
        }
    }

    fn x(&self, x: f32) -> f32 {
        self.row.left() + x - self.offset
    }

    /// `column` on screen, `height` points high and centred in the row.
    pub(in super::super) fn rect(&self, column: Column, height: f32) -> Rect {
        let column = column.placed(self.room);
        let y = self.row.center().y;
        Rect::from_x_y_ranges(
            self.x(column.left)..=self.x(column.right),
            y - height / 2.0..=y + height / 2.0,
        )
    }

    /// A control of `size` in the middle of `column`.
    pub(in super::super) fn centred(&self, column: Column, size: Vec2) -> Rect {
        Rect::from_center_size(self.rect(column, 0.0).center(), size)
    }

    /// The part of the row that scrolls: right of the pinned number.
    pub(super) fn body(&self) -> Rect {
        Rect::from_min_max(
            Pos2::new(self.row.left() + PINNED, self.row.top()),
            self.row.max,
        )
    }
}

/// The parts of one bus's group in the returns row, on screen. The name and
/// state take the room that the controls leave, so a wider header shows more
/// of the effect's name.
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct ReturnParts {
    /// The whole group, without the gap after it.
    pub group: Rect,
    pub label: Rect,
    pub knob: Rect,
    pub input: Rect,
    pub limiter: Rect,
    pub edit: Rect,
}

/// The fixed widths in a returns group, from its knob to the gap after Edit.
const RETURN_KNOB: f32 = 20.0;
const RETURN_INPUT: f32 = 26.0;
const RETURN_LIMITER: f32 = 30.0;
const RETURN_EDIT: f32 = 32.0;
/// The gap after a group's Edit, where the rule before the next group goes.
pub(super) const RETURN_GAP: f32 = 6.0;

/// The group of `bus` in a returns row of `columns`, `height` points high.
pub(in super::super) fn return_parts(columns: &Columns, bus: usize, height: f32) -> ReturnParts {
    let group = columns.rect(RETURNS, height);
    let width = group.width() / BUSES as f32;
    let left = group.left() + width * bus as f32;
    let y = group.y_range();
    let mut right = left + width - RETURN_GAP;
    let mut take = |width: f32, gap: f32| {
        let part = Rect::from_x_y_ranges(right - width..=right, y);
        right -= width + gap;
        part
    };
    let edit = take(RETURN_EDIT, 4.0);
    let limiter = take(RETURN_LIMITER, 3.0);
    let input = take(RETURN_INPUT, 4.0);
    let knob = take(RETURN_KNOB, 4.0);
    ReturnParts {
        group: Rect::from_x_y_ranges(left..=left + width - RETURN_GAP, y),
        label: Rect::from_x_y_ranges(left..=right.max(left), y),
        knob: Rect::from_center_size(knob.center(), Vec2::splat(RETURN_KNOB)),
        input,
        limiter,
        edit,
    }
}

/// The column captions: their text, column, and whether they centre on it.
pub(super) fn caption_list() -> [(String, Column, Align2); 9] {
    let fx = |bus: usize| {
        (
            format!("FX {}", bus + 1),
            send_cell(bus),
            Align2::LEFT_CENTER,
        )
    };
    [
        ("Track".into(), TRACK, Align2::LEFT_CENTER),
        (
            "Out".into(),
            SWITCHES.cell(2, 4).through(SWITCHES.cell(3, 4)),
            Align2::CENTER_CENTER,
        ),
        ("Level".into(), FADER, Align2::LEFT_CENTER),
        ("Pan".into(), PAN, Align2::LEFT_CENTER),
        fx(0),
        fx(1),
        fx(2),
        fx(3),
        ("EQ".into(), EQ, Align2::CENTER_CENTER),
    ]
}

/// How far the columns of a `column`-wide header whose left edge is at
/// `left`, scrolled by `offset`, must scroll so that `control` shows right of
/// the pinned number. Zero for a control in view, one wider than a column,
/// or one outside the columns, such as the pinned number or the record arm.
pub(super) fn reveal(control: Rect, left: f32, column: f32, offset: f32) -> f32 {
    let visible = left + PINNED..=left + column;
    let content = control.left() - left + offset;
    if control.width() >= 200.0
        || content < PINNED - 0.5
        || content + control.width() > CONTENT_WIDTH + 0.5
    {
        0.0
    } else if control.left() < *visible.start() {
        control.left() - visible.start() - 4.0
    } else if control.right() > *visible.end() {
        control.right() - visible.end() + 4.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_columns_follow_each_other_without_overlap() {
        let order = [
            TRACK, SWITCHES, POLARITY, FADER, VALUE, UNITY, PAN, CENTER, STATE, SENDS, EQ,
        ];
        for pair in order.windows(2) {
            assert!(
                pair[0].right <= pair[1].left,
                "{:?} overlaps {:?}",
                pair[0],
                pair[1]
            );
        }
        const { assert!(EQ.right < CONTENT_WIDTH) };
        assert_eq!(send_cell(BUSES - 1).right, SENDS.right);
    }

    #[test]
    fn a_wide_header_widens_the_sounds_and_moves_the_columns_after_them() {
        let narrow = Columns::of(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 30.0)), 0.0);
        let wide = Columns::of(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(CONTENT_WIDTH + 100.0, 30.0)),
            0.0,
        );
        let room = NAME_ROOM;
        assert_eq!(wide.rect(MARK, 8.0), narrow.rect(MARK, 8.0));
        assert_eq!(wide.rect(SOUNDS, 8.0).width(), SOUNDS.width() + room);
        assert_eq!(wide.rect(FADER, 8.0).left(), FADER.left + room);
        assert_eq!(
            wide.rect(RETURNS, 8.0).right(),
            narrow.rect(RETURNS, 8.0).right() + room
        );
    }

    #[test]
    fn a_returns_group_gives_its_name_the_room_its_controls_leave() {
        for width in [CONTENT_WIDTH, CONTENT_WIDTH + NAME_ROOM] {
            let columns = Columns::of(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 32.0)), 0.0);
            let mut previous_edit = f32::NEG_INFINITY;
            for bus in 0..BUSES {
                let parts = return_parts(&columns, bus, 22.0);
                assert!(parts.label.left() > previous_edit);
                assert!(parts.group.contains_rect(parts.label.union(parts.edit)));
                assert!(parts.label.width() >= 95.0, "{bus}: {:?}", parts.label);
                for (left, right) in [
                    (parts.label, parts.knob),
                    (parts.knob, parts.input),
                    (parts.input, parts.limiter),
                    (parts.limiter, parts.edit),
                ] {
                    assert!(left.right() <= right.left());
                }
                previous_edit = parts.edit.right();
            }
        }
    }

    #[test]
    fn a_control_reached_by_keyboard_scrolls_just_into_view() {
        let column = 400.0;
        let at = |content: f32, offset: f32| {
            Rect::from_min_size(Pos2::new(content - offset, 0.0), Vec2::new(20.0, 20.0))
        };
        // In view: no scroll.
        assert_eq!(reveal(at(200.0, 0.0), 0.0, column, 0.0), 0.0);
        // Past the right edge: scroll right until it shows, with a margin.
        assert_eq!(
            reveal(at(EQ.left, 0.0), 0.0, column, 0.0),
            EQ.left + 20.0 - column + 4.0
        );
        // Under the pinned number: scroll back left.
        assert_eq!(
            reveal(at(SWITCHES.left, 150.0), 0.0, column, 150.0),
            -(PINNED - (SWITCHES.left - 150.0)) - 4.0
        );
        // The pinned number and a wide field never scroll the columns.
        assert_eq!(reveal(at(4.0, 0.0), 0.0, column, 0.0), 0.0);
        let wide = Rect::from_min_size(Pos2::new(500.0, 0.0), Vec2::new(240.0, 20.0));
        assert_eq!(reveal(wide, 0.0, column, 0.0), 0.0);
    }
}
