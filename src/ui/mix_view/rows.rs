//! A track's row in the Mix view, the drop row's cue, the edge fade, and the
//! text helpers that the Master and returns rows share.
use super::*;
use theme::type_size::{CLIP, METADATA, SMALL};

/// A child Ui over the row whose columns clip at the pinned number.
pub(super) fn row_ui(ui: &mut egui::Ui, columns: &Columns, salt: impl std::hash::Hash) -> egui::Ui {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(columns.row).id_salt(salt));
    child.set_clip_rect(columns.body().intersect(ui.clip_rect()));
    child
}

/// A row that cannot be heard keeps its switches clear and dims the rest.
pub(in super::super) const SILENT_OPACITY: f32 = 0.45;
/// How far the right edge of a row fades while more columns scroll beyond it.
const EDGE_FADE: f32 = 24.0;

/// What a row changed: its track or level, and whether its EQ was pressed.
#[derive(Default)]
pub(in super::super) struct Outcome {
    pub changed: bool,
    pub open_eq: bool,
    /// The row from its number through Center, where the EQ window opens
    /// beside it.
    pub eq_anchor: Option<Rect>,
}

/// What a row shows but does not edit.
#[derive(Clone, Copy, Debug, Default)]
pub(in super::super) struct RowFacts {
    pub any_solo: bool,
    pub pan_automated: bool,
    pub sends_automated: [bool; BUSES],
    pub processing: bool,
    /// The category colour of the track's first sound.
    pub category: Option<Color32>,
}

impl RowFacts {
    pub(in super::super) fn of(track: &Track, any_solo: bool, category: Option<Color32>) -> Self {
        Self {
            any_solo,
            pan_automated: !track.automation.pan.is_empty(),
            sends_automated: std::array::from_fn(|bus| !track.automation.sends[bus].is_empty()),
            processing: track.processing_on(),
            category,
        }
    }
}

/// The type size of a row's name: Clip, or Small in a short row.
fn name_size(row: Rect) -> f32 {
    if row.height() < 30.0 { SMALL } else { CLIP }
}

/// Paints `value` left-aligned in `area`, cut at its right edge.
pub(super) fn clipped_text(
    ui: &egui::Ui,
    area: Rect,
    pos: Pos2,
    value: &str,
    size: f32,
    color: Color32,
) {
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
pub(super) fn left_label(ui: &mut egui::Ui, area: Rect, value: &str, size: f32) -> Response {
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

/// While columns remain right of a row in a `row`-wide column, fades the
/// last points of the row into its own surface, so a word or control at the
/// edge fades out instead of being cut. `surface` is the row's surface rect.
/// The surface is painted again in one-point slices of rising opacity, so
/// the fade matches every theme's material, Vintage's artwork included.
pub(in super::super) fn edge_fade(
    ui: &egui::Ui,
    row: Rect,
    surface: Rect,
    offset: f32,
    is_return: bool,
) {
    if offset >= max_offset(row.width()) - 0.5 {
        return;
    }
    let steps = EDGE_FADE as usize;
    let slices = (0..steps).filter_map(|step| {
        let left = surface.right() - EDGE_FADE + step as f32;
        let slice =
            Rect::from_x_y_ranges(left..=left + 1.0, surface.y_range()).intersect(ui.clip_rect());
        if slice.width() <= 0.0 {
            return None;
        }
        let mut painter = ui.painter().with_clip_rect(slice);
        painter.multiply_opacity((step + 1) as f32 / steps as f32);
        Some(painter)
    });
    row_surface_each(ui, slices, surface, is_return);
}

/// The header of the Mix view's drop row: a Muted "+" under the track
/// numbers and "New track" in the sounds' column, so the row reads as the
/// place for the next track, as its lane's drop hint says.
pub(in super::super) fn new_track_cue(ui: &egui::Ui, rect: Rect, offset: f32) {
    let size = name_size(rect);
    ui.painter().text(
        Pos2::new(rect.left() + lanes::NUMBER_X, rect.center().y - 1.0),
        Align2::CENTER_CENTER,
        "+",
        FontId::proportional(size + 2.0),
        roles().muted,
    );
    let columns = Columns::of(rect, offset);
    let sounds = columns.rect(SOUNDS, rect.height());
    clipped_text(
        ui,
        sounds.intersect(columns.body()),
        sounds.left_center(),
        "New track",
        size,
        roles().muted,
    );
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
pub(in super::super) fn track_row(
    ui: &mut egui::Ui,
    rect: Rect,
    offset: f32,
    index: usize,
    (summary, detail): (&str, &str),
    facts: &RowFacts,
    mix: &mut TrackMix,
) -> Outcome {
    let columns = Columns::of(rect, offset);
    let mut ui = row_ui(ui, &columns, ("mix_row", index));
    let ui = &mut ui;
    let name = format!("Track {}", index + 1);
    let height = (rect.height() - 4.0).min(24.0);
    let mut outcome = Outcome::default();
    for (cell, switch) in TrackSwitch::ALL.into_iter().enumerate() {
        let on = match switch {
            TrackSwitch::Mute => &mut mix.muted,
            TrackSwitch::Solo => &mut mix.solo,
            TrackSwitch::Left => &mut mix.left_enabled,
            TrackSwitch::Right => &mut mix.right_enabled,
        };
        outcome.changed |= switch_button(
            ui,
            columns.centred(SWITCHES.cell(cell, 4), Vec2::new(SWITCH, height)),
            Look::Quiet,
            &name,
            switch,
            on,
        );
    }
    let silence = mix.silence(facts.any_solo);
    let silent = silence.is_some();
    // A cause on the row itself keeps its words at full strength.
    let own_cause = silence.filter(|cause| cause.is_own());
    let full = ui.opacity();
    if silent {
        ui.multiply_opacity(SILENT_OPACITY);
    }
    if let Some(category) = facts.category {
        let mark = columns.rect(MARK, 0.0).center();
        ui.painter().circle_filled(mark, 3.5, category);
        ui.painter()
            .circle_stroke(mark, 3.5, Stroke::new(1.0_f32, roles().strip_slot));
    }
    left_label(ui, columns.rect(SOUNDS, height), summary, name_size(rect)).hover_hint(
        if silence == Some(Silence::SoloElsewhere) {
            format!("{detail}\nSolo elsewhere")
        } else {
            detail.to_owned()
        },
    );
    outcome.changed |= polarity_button(
        ui,
        columns.centred(POLARITY, Vec2::new(SWITCH, height)),
        Look::Quiet,
        &name,
        &mut mix.polarity_inverted,
    );
    outcome.changed |= level(ui, &columns, height, &name, &mut mix.gain, silent);
    outcome.changed |= pan_slider(
        ui,
        columns.rect(PAN, 16.0),
        Look::Quiet,
        &name,
        &mut mix.pan,
        facts.pan_automated,
    );
    if center_button(
        ui,
        columns.rect(CENTER, height.min(18.0)),
        Look::Quiet,
        &name,
    ) {
        outcome.changed |= mix.pan != 0.0;
        mix.pan = 0.0;
    }
    // Only what differs from a plain track: a cause on the row, an automated
    // pan, or a pan off center. Solo elsewhere is left to the dimming, the
    // lit Solo, and the sounds' hint, so one solo does not fill the column.
    if own_cause.is_some() || facts.pan_automated || mix.pan != 0.0 {
        let state = columns.rect(STATE, height);
        let dimmed = ui.opacity();
        if own_cause.is_some() {
            ui.set_opacity(full);
        }
        clipped_text(
            ui,
            state,
            state.left_center() + Vec2::new(4.0, 0.0),
            &track_state(own_cause, facts.pan_automated, mix.pan),
            SMALL,
            roles().strip_text,
        );
        ui.set_opacity(dimmed);
    }
    let knob = height.min(KNOB);
    for bus in 0..BUSES {
        let (knob_column, amount_column) = send_parts(bus);
        outcome.changed |= send_knob(
            ui,
            columns.centred(knob_column, Vec2::splat(knob)),
            Look::Quiet,
            &format!("{name} send FX {}", bus + 1),
            &mut mix.sends[bus],
            facts.sends_automated[bus],
        );
        if mix.sends[bus] > 0.0 {
            let amount = columns.rect(amount_column, height);
            clipped_text(
                ui,
                amount,
                amount.left_center(),
                &format!("{:.0}", mix.sends[bus] * 100.0),
                METADATA,
                quiet_ink(),
            );
        }
    }
    let center = columns.rect(CENTER, height);
    outcome.eq_anchor = Some(Rect::from_x_y_ranges(
        rect.left()..=center.right().clamp(rect.left() + PINNED, rect.right()),
        rect.y_range(),
    ));
    outcome.open_eq = eq_button(
        ui,
        columns.rect(EQ, height),
        Look::Quiet,
        &name,
        facts.processing,
        silent,
    );
    outcome
}

/// The fader, exact dB entry, and Unity of `name`'s gain. A `silent` row
/// draws the dB entry as an outline on its own surface.
pub(super) fn level(
    ui: &mut egui::Ui,
    columns: &Columns,
    height: f32,
    name: &str,
    gain: &mut f32,
    silent: bool,
) -> bool {
    let mut changed = lanes::strip_fader(ui, columns.rect(FADER, height), name, gain);
    changed |= gain_value_in(ui, columns.rect(VALUE, height), 1.0, name, gain, silent);
    if unity_button(ui, columns.rect(UNITY, height.min(18.0)), Look::Quiet, name) {
        changed |= *gain != 1.0;
        *gain = 1.0;
    }
    changed
}
