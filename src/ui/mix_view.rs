//! The arrangement's Mix view: each lane header widens into a mixer row, and
//! a block below the lanes holds the Master row and the four FX returns. The
//! rows use the Mixer strips' controls, names, and edits, so the two views
//! differ only in layout.
use super::mixer::{
    bus_switches, bypass_button, center_button, edit_button, eq_button, gain_value, pan_slider,
    polarity_button, return_state, row_surface, send_knob, switch_button, track_state,
    unity_button,
};
use super::*;
use mixjam_techno::application::Track;
use mixjam_techno::application::fx::{BUSES, FxBuses};

/// The Mix view: whether the score shows it, and its column scroll.
#[derive(Default)]
pub(super) struct MixViewState {
    /// The score shows the Mix view's mixer rows instead of the lane headers.
    /// It never shows with the Mixer tab.
    pub(super) shown: bool,
    /// How far the Mix view's columns are scrolled to the left.
    pub(super) scroll: f32,
    /// The Mix view control last scrolled into view for keyboard focus.
    pub(super) revealed: Option<egui::Id>,
}

/// The width of a row's columns; a narrower header column scrolls them.
pub(super) const CONTENT_WIDTH: f32 = 876.0;
/// The track number stays in place while the other columns scroll.
pub(super) const PINNED: f32 = 34.0;
/// The lanes keep at least this width beside the Mix view's column.
const LANES_MIN: f32 = 160.0;
/// Each column, from the row's left edge.
const NAME: (f32, f32) = (38.0, 148.0);
const SWITCH_X: [f32; 4] = [164.0, 188.0, 212.0, 236.0];
const POLARITY_X: f32 = 260.0;
const FADER: (f32, f32) = (276.0, 400.0);
const VALUE: (f32, f32) = (404.0, 460.0);
const UNITY: (f32, f32) = (464.0, 504.0);
const PAN: (f32, f32) = (512.0, 572.0);
const CENTER: (f32, f32) = (576.0, 620.0);
const STATE: (f32, f32) = (624.0, 712.0);
const SEND_X: [f32; BUSES] = [728.0, 752.0, 776.0, 800.0];
const EQ: (f32, f32) = (816.0, 848.0);
/// The Master and returns rows below the lanes, and the gap above them.
pub(super) const BLOCK_ROW: f32 = 32.0;
pub(super) const BLOCK_HEIGHT: f32 = BLOCK_ROW * 2.0 + 6.0;
/// One FX return's group in the returns row.
const RETURN_GROUP: f32 = 210.0;
/// The column captions above the rows, at their columns' left edges.
const CAPTIONS: [(&str, f32); 6] = [
    ("Track", NAME.0),
    ("Outputs", 152.0),
    ("Level", FADER.0),
    ("Pan", PAN.0),
    ("Sends", 718.0),
    ("EQ", EQ.0),
];

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
    ui.painter()
        .with_clip_rect(area.intersect(ui.clip_rect()))
        .text(
            pos,
            Align2::LEFT_CENTER,
            value,
            FontId::proportional(size),
            color,
        );
}

/// A track's row: its sounds, M, S, L, and R, polarity, fader, exact dB,
/// Unity, pan and Center, its state, four sends, and EQ. The number and the
/// record arm are the arrangement's, left of `rect`'s scrolled columns.
#[allow(clippy::too_many_arguments)]
pub(super) fn track_row(
    ui: &mut egui::Ui,
    rect: Rect,
    offset: f32,
    index: usize,
    (summary, detail): (&str, &str),
    any_solo: bool,
    track: &mut Track,
) -> Outcome {
    let mut ui = row_ui(ui, rect, ("mix_row", index));
    let ui = &mut ui;
    let name = format!("Track {}", index + 1);
    let height = (rect.height() - 4.0).min(24.0);
    let mut outcome = Outcome::default();
    let name_rect = span(rect, offset, NAME, height);
    ui.put(
        name_rect,
        egui::Label::new(
            egui::RichText::new(summary)
                .size(if rect.height() < 30.0 { 11.0 } else { 12.0 })
                .color(roles().strip_text),
        )
        .truncate(),
    )
    .hover_hint(detail);
    let switches = [
        TrackSwitch::Mute,
        TrackSwitch::Solo,
        TrackSwitch::Left,
        TrackSwitch::Right,
    ];
    for (switch, x) in switches.into_iter().zip(SWITCH_X) {
        let on = match switch {
            TrackSwitch::Mute => &mut track.muted,
            TrackSwitch::Solo => &mut track.solo,
            TrackSwitch::Left => &mut track.left_enabled,
            TrackSwitch::Right => &mut track.right_enabled,
        };
        outcome.changed |= switch_button(
            ui,
            centred(rect, offset, x, Vec2::new(22.0, height)),
            1.0,
            &name,
            switch,
            on,
        );
    }
    outcome.changed |= polarity_button(
        ui,
        centred(rect, offset, POLARITY_X, Vec2::new(22.0, height)),
        1.0,
        &name,
        &mut track.polarity_inverted,
    );
    outcome.changed |= level(ui, rect, offset, height, &name, &mut track.gain);
    let pan_automated = !track.automation.pan.is_empty();
    outcome.changed |= pan_slider(
        ui,
        span(rect, offset, PAN, 16.0),
        1.0,
        &format!("Track {}", index + 1),
        &mut track.pan,
        pan_automated,
    );
    if center_button(ui, span(rect, offset, CENTER, height.min(18.0)), 1.0, &name) {
        outcome.changed |= track.pan != 0.0;
        track.pan = 0.0;
    }
    let state = span(rect, offset, STATE, height);
    clipped_text(
        ui,
        state,
        state.left_center() + Vec2::new(4.0, 0.0),
        &track_state(
            track.muted,
            track.left_enabled || track.right_enabled,
            any_solo && !track.solo,
            pan_automated,
            track.pan,
        ),
        11.0,
        roles().strip_text,
    );
    let knob = height.min(18.0);
    for (bus, x) in SEND_X.into_iter().enumerate() {
        outcome.changed |= send_knob(
            ui,
            centred(rect, offset, x, Vec2::splat(knob)),
            1.0,
            &format!("{name} send FX {}", bus + 1),
            &mut track.sends[bus],
            !track.automation.sends[bus].is_empty(),
        );
    }
    outcome.open_eq = eq_button(
        ui,
        span(rect, offset, EQ, height),
        1.0,
        &name,
        mixer::processing_on(track),
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
    if unity_button(ui, span(rect, offset, UNITY, height.min(18.0)), 1.0, name) {
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

/// The Master row: "Stereo output", Bypass, and the master level's fader,
/// exact dB, and Unity, aligned with the tracks' columns. The caller puts
/// Export tracks in the returned rectangle.
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
    let name = span(rect, offset, NAME, BLOCK_ROW);
    ui.put(
        Rect::from_min_max(name.min, Pos2::new(name.right(), name.center().y)),
        egui::Label::new(
            egui::RichText::new("Master")
                .size(12.0)
                .color(roles().strip_text),
        )
        .truncate(),
    )
    .hover_hint("Master level affects the complete mix.");
    clipped_text(
        ui,
        name,
        Pos2::new(name.left(), name.center().y + 7.0),
        "Stereo output",
        10.0,
        roles().strip_text,
    );
    let bypass = bypass_button(
        ui,
        Rect::from_x_y_ranges(
            rect.left() + 152.0 - offset..=rect.left() + 248.0 - offset,
            rect.center().y - height / 2.0..=rect.center().y + height / 2.0,
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
        let left = PINNED + index as f32 * RETURN_GROUP;
        let at = |from: f32, to: f32| span(rect, offset, (left + from, left + to), height);
        let name = format!("FX {}", index + 1);
        let before = *bus;
        let label = at(0.0, 78.0);
        clipped_text(
            ui,
            label,
            Pos2::new(label.left(), rect.center().y - 7.0),
            &format!("{name}  {}", bus.module.label()),
            11.0,
            roles().legend,
        );
        clipped_text(
            ui,
            label,
            Pos2::new(label.left(), rect.center().y + 7.0),
            &return_state(bus, automated[index]),
            10.0,
            roles().legend,
        );
        if on_board {
            let note = at(84.0, 184.0);
            clipped_text(
                ui,
                note,
                Pos2::new(note.left(), rect.center().y),
                "On the board below",
                10.0,
                roles().muted,
            );
        } else {
            send_knob(
                ui,
                centred(rect, offset, left + 92.0, Vec2::splat(20.0)),
                1.0,
                &format!("{name} return level"),
                &mut bus.return_level,
                automated[index],
            );
            bus_switches(ui, [at(106.0, 142.0), at(146.0, 184.0)], 1.0, &name, bus);
        }
        if edit_button(ui, at(188.0, 208.0), 1.0, &name) {
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
    for (caption, x) in CAPTIONS {
        painter.text(
            Pos2::new(strip.left() + x - offset, strip.center().y - 2.0),
            Align2::LEFT_CENTER,
            caption.to_uppercase(),
            FontId::proportional(10.0),
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
