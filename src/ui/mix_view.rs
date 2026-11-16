//! The arrangement's Mix view: each lane header widens into a mixer row, and
//! a block below the lanes holds the Master row and the four FX returns. The
//! rows use the Mixer strips' names and edits, so the two views differ in
//! layout and in how quietly a row at its defaults reads.
use super::mixer::{
    bypass_button, edit_button, eq_button, gain_value, pan_slider, quiet_bus_switches,
    quiet_center_button, quiet_polarity_button, quiet_send_knob, quiet_switch, quiet_unity_button,
    return_state, row_surface, send_knob, track_state,
};
use super::*;
use mixjam_techno::application::Track;
use mixjam_techno::application::fx::{BUSES, FxBuses};

/// The Mix view: whether the score shows it, and its column scroll.
#[derive(Default)]
pub(super) struct MixViewState {
    /// The score shows the Mix view's mixer rows instead of the lane headers.
    /// It never shows with the Mixer board.
    pub(super) shown: bool,
    /// How far the Mix view's columns are scrolled to the left.
    pub(super) scroll: f32,
    /// The Mix view control last scrolled into view for keyboard focus.
    pub(super) revealed: Option<egui::Id>,
    /// The Mix view folded the lower panel when it opened; leaving it opens
    /// the panel again unless the lower panel was opened or folded since.
    pub(super) restore_boards: bool,
}

/// The width of a row's columns; a narrower header column scrolls them.
pub(super) const CONTENT_WIDTH: f32 = EQ.1 + 6.0;
/// The track number stays in place while the other columns scroll.
pub(super) const PINNED: f32 = 34.0;
/// The lanes keep at least this width beside the Mix view's column.
const LANES_MIN: f32 = 160.0;
/// The type sizes of the design's clip, small, and metadata roles.
const CLIP_TEXT: f32 = 12.0;
const SMALL_TEXT: f32 = 11.0;
const META_TEXT: f32 = 10.0;
/// A row that cannot be heard keeps its switches clear and dims the rest.
const SILENT_OPACITY: f32 = 0.45;
/// Each column, from the row's left edge. The track column holds the
/// category mark and the sounds.
const TRACK: (f32, f32) = (40.0, 150.0);
const MARK_X: f32 = 44.0;
const NAME: (f32, f32) = (52.0, TRACK.1);
const SWITCH: f32 = 22.0;
const SWITCH_X: [f32; 4] = [164.0, 188.0, 212.0, 236.0];
const POLARITY_X: f32 = 260.0;
const FADER: (f32, f32) = (280.0, 416.0);
const VALUE: (f32, f32) = (420.0, 476.0);
const UNITY: (f32, f32) = (478.0, 514.0);
const PAN: (f32, f32) = (524.0, 594.0);
const CENTER: (f32, f32) = (596.0, 636.0);
const STATE: (f32, f32) = (642.0, 726.0);
/// Each send is a cell: its knob, then its amount while it sends.
const SENDS_LEFT: f32 = 732.0;
const SEND_CELL: f32 = 42.0;
const KNOB: f32 = 18.0;
const EQ: (f32, f32) = (
    SENDS_LEFT + SEND_CELL * BUSES as f32 + 6.0,
    SENDS_LEFT + SEND_CELL * BUSES as f32 + 38.0,
);
/// The Master and returns rows below the lanes, and the gap above them.
pub(super) const BLOCK_ROW: f32 = 32.0;
pub(super) const BLOCK_HEIGHT: f32 = BLOCK_ROW * 2.0 + 6.0;
/// One FX return's group in the returns row; the four span the columns.
const RETURN_GROUP: f32 = (EQ.1 - TRACK.0) / BUSES as f32;

/// The send cell of `bus`, from the row's left edge.
const fn send_cell(bus: usize) -> (f32, f32) {
    let left = SENDS_LEFT + SEND_CELL * bus as f32;
    (left, left + SEND_CELL)
}

/// The middle of the switch columns, over L and R.
const fn outputs_x() -> f32 {
    (SWITCH_X[2] + SWITCH_X[3]) / 2.0
}

/// The column captions: their text, where they anchor, and how.
fn caption_list() -> [(String, f32, Align2); 9] {
    let fx = |bus: usize| {
        (
            format!("FX {}", bus + 1),
            send_cell(bus).0,
            Align2::LEFT_CENTER,
        )
    };
    [
        ("Track".into(), TRACK.0, Align2::LEFT_CENTER),
        ("Out".into(), outputs_x(), Align2::CENTER_CENTER),
        ("Level".into(), FADER.0, Align2::LEFT_CENTER),
        ("Pan".into(), PAN.0, Align2::LEFT_CENTER),
        fx(0),
        fx(1),
        fx(2),
        fx(3),
        ("EQ".into(), (EQ.0 + EQ.1) / 2.0, Align2::CENTER_CENTER),
    ]
}

/// The header column's width in the Mix view for a score this wide.
pub(super) fn column_width(available: f32) -> f32 {
    (available - LANES_MIN).clamp(lanes::LANE_HEADER_WIDTH, CONTENT_WIDTH)
}

/// The largest horizontal offset of the columns in a header this wide.
pub(super) fn max_offset(column: f32) -> f32 {
    (CONTENT_WIDTH - column).max(0.0)
}

/// What a row changed: its track or level, and whether its EQ was pressed.
#[derive(Default)]
pub(super) struct Outcome {
    pub changed: bool,
    pub open_eq: bool,
}

/// The part of a track that a Mix view row edits. It is small and `Copy`, so
/// a row is drawn without cloning the track and its automation each frame.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct RowMix {
    gain: f32,
    pan: f32,
    polarity_inverted: bool,
    muted: bool,
    solo: bool,
    left_enabled: bool,
    right_enabled: bool,
    sends: [f32; BUSES],
}

impl RowMix {
    pub(super) fn of(track: &Track) -> Self {
        Self {
            gain: track.gain,
            pan: track.pan,
            polarity_inverted: track.polarity_inverted,
            muted: track.muted,
            solo: track.solo,
            left_enabled: track.left_enabled,
            right_enabled: track.right_enabled,
            sends: track.sends,
        }
    }

    /// `track` with this row's edits.
    pub(super) fn applied(self, track: &Track) -> Track {
        Track {
            gain: self.gain,
            pan: self.pan,
            polarity_inverted: self.polarity_inverted,
            muted: self.muted,
            solo: self.solo,
            left_enabled: self.left_enabled,
            right_enabled: self.right_enabled,
            sends: self.sends,
            ..track.clone()
        }
    }
}

/// What a row shows but does not edit.
pub(super) struct RowFacts {
    pub any_solo: bool,
    pub pan_automated: bool,
    pub sends_automated: [bool; BUSES],
    pub processing: bool,
    /// The category colour of the track's first sound.
    pub category: Option<Color32>,
}

impl RowFacts {
    pub(super) fn of(track: &Track, any_solo: bool, category: Option<Color32>) -> Self {
        Self {
            any_solo,
            pan_automated: !track.automation.pan.is_empty(),
            sends_automated: std::array::from_fn(|bus| !track.automation.sends[bus].is_empty()),
            processing: mixer::processing_on(track),
            category,
        }
    }
}

/// A child Ui over `rect` whose columns scroll by `offset` and clip at the
/// pinned number.
fn row_ui(ui: &mut egui::Ui, rect: Rect, salt: impl std::hash::Hash) -> egui::Ui {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(salt));
    let body = Rect::from_min_max(Pos2::new(rect.left() + PINNED, rect.top()), rect.max);
    child.set_clip_rect(body.intersect(ui.clip_rect()));
    child
}

fn span(rect: Rect, offset: f32, (left, right): (f32, f32), height: f32) -> Rect {
    Rect::from_x_y_ranges(
        rect.left() + left - offset..=rect.left() + right - offset,
        rect.center().y - height / 2.0..=rect.center().y + height / 2.0,
    )
}

fn centred(rect: Rect, offset: f32, x: f32, size: Vec2) -> Rect {
    Rect::from_center_size(Pos2::new(rect.left() + x - offset, rect.center().y), size)
}

/// Paints `value` left-aligned in `area`, cut at its right edge.
fn clipped_text(ui: &egui::Ui, area: Rect, pos: Pos2, value: &str, size: f32, color: Color32) {
    let painter = ui.painter().with_clip_rect(area.intersect(ui.clip_rect()));
    let mut job = egui::text::LayoutJob::simple_singleline(
        value.to_owned(),
        FontId::proportional(size),
        color,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width((area.right() - pos.x).max(0.0));
    let galley = painter.layout_job(job);
    let top = pos.y - galley.size().y * 0.5;
    painter.galley(Pos2::new(pos.x, top), galley, color);
}

/// A label at the left of `area`, elided at its right edge, in the strip's
/// ink.
fn left_label(ui: &mut egui::Ui, area: Rect, value: &str, size: f32) -> Response {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(area)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(value)
                        .size(size)
                        .color(roles().strip_text),
                )
                .truncate(),
            )
        },
    )
    .inner
}

/// The strip's ink for a value that only reports, such as a send amount.
fn quiet_ink() -> Color32 {
    roles().strip_text.gamma_multiply(0.7)
}

/// A track's row: its category and sounds, M, S, L, and R, polarity, fader,
/// exact dB, Unity, pan and Center, what makes it differ, four sends, and
/// EQ. The number and the record arm are the arrangement's, left of `rect`'s
/// scrolled columns. A row at its defaults stays quiet: Unity and Center
/// show only once their value moved, and the state only names an exception.
pub(super) fn track_row(
    ui: &mut egui::Ui,
    rect: Rect,
    offset: f32,
    index: usize,
    (summary, detail): (&str, &str),
    facts: &RowFacts,
    mix: &mut RowMix,
) -> Outcome {
    let mut ui = row_ui(ui, rect, ("mix_row", index));
    let ui = &mut ui;
    let name = format!("Track {}", index + 1);
    let height = (rect.height() - 4.0).min(24.0);
    let mut outcome = Outcome::default();
    for (switch, x) in TrackSwitch::ALL.into_iter().zip(SWITCH_X) {
        let on = match switch {
            TrackSwitch::Mute => &mut mix.muted,
            TrackSwitch::Solo => &mut mix.solo,
            TrackSwitch::Left => &mut mix.left_enabled,
            TrackSwitch::Right => &mut mix.right_enabled,
        };
        outcome.changed |= quiet_switch(
            ui,
            centred(rect, offset, x, Vec2::new(SWITCH, height)),
            &name,
            switch,
            on,
        );
    }
    let any_output = mix.left_enabled || mix.right_enabled;
    let solo_elsewhere = facts.any_solo && !mix.solo;
    if mix.muted || solo_elsewhere || !any_output {
        ui.multiply_opacity(SILENT_OPACITY);
    }
    if let Some(category) = facts.category {
        let mark = Pos2::new(rect.left() + MARK_X - offset, rect.center().y);
        ui.painter().circle_filled(mark, 3.5, category);
        ui.painter()
            .circle_stroke(mark, 3.5, Stroke::new(1.0_f32, roles().strip_slot));
    }
    left_label(
        ui,
        span(rect, offset, NAME, height),
        summary,
        if rect.height() < 30.0 {
            SMALL_TEXT
        } else {
            CLIP_TEXT
        },
    )
    .hover_hint(detail);
    outcome.changed |= quiet_polarity_button(
        ui,
        centred(rect, offset, POLARITY_X, Vec2::new(SWITCH, height)),
        &name,
        &mut mix.polarity_inverted,
    );
    outcome.changed |= level(ui, rect, offset, height, &name, &mut mix.gain);
    outcome.changed |= pan_slider(
        ui,
        span(rect, offset, PAN, 16.0),
        1.0,
        &name,
        &mut mix.pan,
        facts.pan_automated,
    );
    if quiet_center_button(ui, span(rect, offset, CENTER, height.min(18.0)), &name) {
        outcome.changed |= mix.pan != 0.0;
        mix.pan = 0.0;
    }
    // Only what differs from a plain track: a silencing cause, an automated
    // pan, or a pan off center.
    let exception = mix.muted || !any_output || solo_elsewhere || facts.pan_automated;
    if exception || mix.pan != 0.0 {
        let state = span(rect, offset, STATE, height);
        clipped_text(
            ui,
            state,
            state.left_center() + Vec2::new(4.0, 0.0),
            &track_state(
                mix.muted,
                any_output,
                solo_elsewhere,
                facts.pan_automated,
                mix.pan,
            ),
            SMALL_TEXT,
            roles().strip_text,
        );
    }
    let knob = height.min(KNOB);
    for bus in 0..BUSES {
        let (left, right) = send_cell(bus);
        outcome.changed |= quiet_send_knob(
            ui,
            centred(rect, offset, left + KNOB / 2.0, Vec2::splat(knob)),
            &format!("{name} send FX {}", bus + 1),
            &mut mix.sends[bus],
            facts.sends_automated[bus],
        );
        if mix.sends[bus] > 0.0 {
            let amount = span(rect, offset, (left + KNOB + 3.0, right), height);
            clipped_text(
                ui,
                amount,
                amount.left_center(),
                &format!("{:.0}", mix.sends[bus] * 100.0),
                META_TEXT,
                quiet_ink(),
            );
        }
    }
    outcome.open_eq = eq_button(
        ui,
        span(rect, offset, EQ, height),
        1.0,
        &name,
        facts.processing,
    );
    outcome
}

/// The fader, exact dB entry, and Unity of `name`'s gain.
fn level(
    ui: &mut egui::Ui,
    rect: Rect,
    offset: f32,
    height: f32,
    name: &str,
    gain: &mut f32,
) -> bool {
    let mut changed = lanes::lane_fader(ui, span(rect, offset, FADER, height), name, gain);
    changed |= gain_value(ui, span(rect, offset, VALUE, height), 1.0, name, gain);
    if quiet_unity_button(ui, span(rect, offset, UNITY, height.min(18.0)), name) {
        changed |= *gain != 1.0;
        *gain = 1.0;
    }
    changed
}

/// What the Master row changed, and where its Export tracks button goes.
pub(super) struct MasterOutcome {
    pub changed: bool,
    pub bypass: bool,
    pub export: Rect,
}

/// The Master row: "Stereo output", Bypass under the switches, and the
/// master level's fader, exact dB, and Unity, aligned with the tracks'
/// columns. The caller puts Export tracks in the returned rectangle.
pub(super) fn master_row(
    ui: &mut egui::Ui,
    rect: Rect,
    offset: f32,
    gain: &mut f32,
    bypass: &mut bool,
) -> MasterOutcome {
    row_surface(ui, rect, false);
    let mut ui = row_ui(ui, rect, "mix_master");
    let ui = &mut ui;
    let height = BLOCK_ROW - 8.0;
    let name = span(rect, offset, TRACK, BLOCK_ROW);
    left_label(
        ui,
        Rect::from_min_max(name.min, Pos2::new(name.right(), name.center().y + 2.0)),
        "Master",
        CLIP_TEXT,
    )
    .hover_hint("Master level affects the complete mix.");
    clipped_text(
        ui,
        name,
        Pos2::new(name.left(), name.center().y + 7.0),
        "Stereo output",
        META_TEXT,
        roles().strip_text,
    );
    let bypass = bypass_button(
        ui,
        span(
            rect,
            offset,
            (
                SWITCH_X[0] - SWITCH / 2.0,
                SWITCH_X[SWITCH_X.len() - 1] + SWITCH / 2.0,
            ),
            height,
        ),
        1.0,
        bypass,
    );
    let changed = level(ui, rect, offset, height, "Master", gain);
    MasterOutcome {
        changed,
        bypass,
        export: span(rect, offset, (PAN.0, CENTER.1), height),
    }
}

/// The returns row: per FX bus its name and effect, return level, state,
/// Input, Limiter, and Edit. While the FX returns board shows (`on_board`),
/// the row keeps only the names, states, and Edit, so no control is named
/// twice. Returns whether a bus changed and which bus's Edit was pressed.
pub(super) fn returns_row(
    ui: &mut egui::Ui,
    rect: Rect,
    offset: f32,
    buses: &mut FxBuses,
    automated: [bool; BUSES],
    on_board: bool,
) -> (bool, Option<usize>) {
    row_surface(ui, rect, true);
    let mut ui = row_ui(ui, rect, "mix_returns");
    let ui = &mut ui;
    let height = BLOCK_ROW - 10.0;
    let mut changed = false;
    let mut open = None;
    for (index, bus) in buses.buses.iter_mut().enumerate() {
        let left = TRACK.0 + index as f32 * RETURN_GROUP;
        let at = |from: f32, to: f32| span(rect, offset, (left + from, left + to), height);
        let name = format!("FX {}", index + 1);
        if index > 0 {
            // A rule in the 6-point gap keeps each group's Edit apart from
            // the next group's name.
            let x = span(rect, offset, (left - 3.0, left - 3.0), height).left();
            ui.painter().vline(
                x,
                (rect.top() + 6.0)..=(rect.bottom() - 6.0),
                Stroke::new(1.0_f32, roles().strip_slot),
            );
        }
        let before = *bus;
        let label = at(0.0, 82.0);
        clipped_text(
            ui,
            label,
            Pos2::new(label.left(), rect.center().y - 7.0),
            &format!("{name} {}", bus.module.label()),
            SMALL_TEXT,
            roles().legend,
        );
        clipped_text(
            ui,
            label,
            Pos2::new(label.left(), rect.center().y + 7.0),
            &return_state(bus, automated[index]),
            META_TEXT,
            roles().legend,
        );
        if on_board {
            let note = at(88.0, 191.0);
            clipped_text(
                ui,
                note,
                Pos2::new(note.left(), rect.center().y),
                "On the board below",
                META_TEXT,
                roles().muted,
            );
        } else {
            send_knob(
                ui,
                centred(rect, offset, left + 94.0, Vec2::splat(20.0)),
                1.0,
                &format!("{name} return level"),
                &mut bus.return_level,
                automated[index],
            );
            quiet_bus_switches(ui, [at(106.0, 144.0), at(147.0, 191.0)], &name, bus);
        }
        if edit_button(ui, at(194.0, RETURN_GROUP - 6.0), 1.0, &name) {
            open = Some(index);
        }
        changed |= *bus != before;
    }
    (changed, open)
}

/// The column captions in the ruler strip's header part, and a thin bar that
/// shows and drags the columns' offset when the column is narrower than its
/// rows. Returns the new offset.
pub(super) fn captions(ui: &mut egui::Ui, strip: Rect, offset: f32) -> f32 {
    let painter = ui.painter().with_clip_rect(
        Rect::from_min_max(Pos2::new(strip.left() + PINNED, strip.top()), strip.max)
            .intersect(ui.clip_rect()),
    );
    for (caption, x, align) in caption_list() {
        painter.text(
            Pos2::new(strip.left() + x - offset, strip.center().y - 2.0),
            align,
            caption.to_uppercase(),
            FontId::proportional(META_TEXT),
            roles().muted,
        );
    }
    let max = max_offset(strip.width());
    if max <= 0.0 {
        return 0.0;
    }
    let track = Rect::from_min_max(
        Pos2::new(strip.left() + PINNED, strip.bottom() - 5.0),
        Pos2::new(strip.right() - 2.0, strip.bottom() - 2.0),
    );
    let visible = strip.width() / CONTENT_WIDTH;
    let thumb_width = (track.width() * visible).max(16.0);
    let thumb_left = egui::lerp(track.left()..=track.right() - thumb_width, offset / max);
    let thumb = Rect::from_min_size(
        Pos2::new(thumb_left, track.top()),
        Vec2::new(thumb_width, track.height()),
    );
    let response = name_region(
        ui.interact(
            Rect::from_min_max(Pos2::new(track.left(), strip.top()), strip.max),
            ui.id().with("mix_columns"),
            Sense::drag(),
        ),
        "Mix view columns",
    )
    .hover_hint("Drag, or Shift and the wheel over the rows, to see the other columns.");
    let mut offset = offset;
    if response.dragged() {
        let travel = (track.width() - thumb_width).max(1.0);
        offset += response.drag_delta().x / travel * max;
    }
    painter.rect_filled(track, 1.5, roles().line);
    painter.rect_filled(
        thumb,
        1.5,
        if response.hovered() || response.dragged() {
            roles().text
        } else {
            roles().muted
        },
    );
    offset.clamp(0.0, max)
}
