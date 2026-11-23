//! The Mix view's parts outside the track rows: the column captions in the
//! ruler strip, and the Master and returns rows below the lanes.
use super::*;
use theme::type_size::{CLIP, METADATA, SMALL};

/// The Master and returns rows below the lanes, and the gap above them.
pub(in super::super) const BLOCK_ROW: f32 = 32.0;
pub(in super::super) const BLOCK_HEIGHT: f32 = BLOCK_ROW * 2.0 + 6.0;

/// What the Master row changed, and where its Export tracks button goes.
pub(super) struct MasterOutcome {
    pub changed: bool,
    pub bypass: bool,
    pub export: Rect,
}

/// The Master row: the pedalboard state, as on the Mixer's Master strip,
/// Bypass under the switches, and the master level's fader, exact dB, and
/// Unity, aligned with the tracks' columns. The caller puts Export tracks in
/// the returned rectangle.
pub(super) fn master_row(
    ui: &mut egui::Ui,
    rect: Rect,
    offset: f32,
    gain: &mut f32,
    (bypass, pedals): (&mut bool, usize),
) -> MasterOutcome {
    row_surface(ui, rect, false);
    let columns = Columns::of(rect, offset);
    let mut ui = row_ui(ui, &columns, "mix_master");
    let ui = &mut ui;
    let height = BLOCK_ROW - 8.0;
    let name = columns.rect(TRACK, BLOCK_ROW);
    left_label(
        ui,
        Rect::from_min_max(name.min, Pos2::new(name.right(), name.center().y + 2.0)),
        "Master",
        CLIP,
    )
    .hover_hint("Master level affects the complete mix.");
    clipped_text(
        ui,
        name,
        Pos2::new(name.left(), name.center().y + 7.0),
        &master_summary(*bypass, pedals),
        METADATA,
        roles().strip_text,
    );
    let bypass = bypass_button(
        ui,
        columns.rect(
            Column::new(SWITCHES.left + 1.0, SWITCHES.right - 1.0),
            height,
        ),
        1.0,
        bypass,
    );
    let changed = level(ui, &columns, height, "Master", gain, false);
    MasterOutcome {
        changed,
        bypass,
        export: columns.rect(PAN.through(CENTER), height),
    }
}

/// The returns row: per FX bus its effect, its number and return level,
/// the return knob, Input, Limiter, and Edit. While the FX returns board
/// shows (`on_board`), the row keeps only the names, states, and Edit, so no
/// control is named twice. Returns whether a bus changed and which bus's
/// Edit was pressed.
pub(super) fn returns_row(
    ui: &mut egui::Ui,
    rect: Rect,
    offset: f32,
    buses: &mut FxBuses,
    automated: [bool; BUSES],
    on_board: bool,
) -> (bool, Option<usize>) {
    row_surface(ui, rect, true);
    let columns = Columns::of(rect, offset);
    let mut ui = row_ui(ui, &columns, "mix_returns");
    let ui = &mut ui;
    let height = BLOCK_ROW - 10.0;
    let mut changed = false;
    let mut open = None;
    for (index, bus) in buses.buses.iter_mut().enumerate() {
        let parts = return_parts(&columns, index, height);
        let name = format!("FX {}", index + 1);
        if index > 0 {
            // A rule in the gap keeps each group's Edit apart from the next
            // group's name.
            ui.painter().vline(
                parts.group.left() - RETURN_GAP / 2.0,
                (rect.top() + 6.0)..=(rect.bottom() - 6.0),
                Stroke::new(1.0_f32, roles().strip_slot),
            );
        }
        let before = *bus;
        // The effect leads, since the bus number repeats the captions' FX
        // order; its number and level follow in the smaller line.
        clipped_text(
            ui,
            parts.label,
            Pos2::new(parts.label.left(), rect.center().y - 7.0),
            bus.module.label(),
            SMALL,
            roles().legend,
        );
        clipped_text(
            ui,
            parts.label,
            Pos2::new(parts.label.left(), rect.center().y + 7.0),
            &format!("{name} · {}", return_state(bus, automated[index])),
            METADATA,
            roles().legend,
        );
        if on_board {
            let note = parts.knob.union(parts.limiter);
            clipped_text(
                ui,
                note,
                Pos2::new(note.left(), rect.center().y),
                "On the board below",
                METADATA,
                roles().muted,
            );
        } else {
            // An Empty bus returns silence: its knob and switches stay named
            // and reachable but dim, so Edit reads as the one useful action.
            let opacity = ui.opacity();
            if matches!(bus.module, Module::Empty) {
                ui.multiply_opacity(SILENT_OPACITY);
            }
            send_knob(
                ui,
                parts.knob,
                Look::Quiet,
                &format!("{name} return level"),
                &mut bus.return_level,
                automated[index],
            );
            bus_switches(ui, [parts.input, parts.limiter], Look::Quiet, &name, bus);
            ui.set_opacity(opacity);
        }
        if edit_button(ui, parts.edit, 1.0, &name) {
            open = Some(index);
        }
        changed |= *bus != before;
    }
    (changed, open)
}

/// The column captions in the ruler strip's header part, and a thin bar that
/// shows and drags the columns' offset when the column is narrower than its
/// rows. Returns the new offset.
pub(in super::super) fn captions(ui: &mut egui::Ui, strip: Rect, offset: f32) -> f32 {
    let columns = Columns::of(strip, offset);
    let painter = ui
        .painter()
        .with_clip_rect(columns.body().intersect(ui.clip_rect()));
    for (caption, column, align) in caption_list() {
        let column = columns.rect(column, 0.0);
        let x = if align == Align2::CENTER_CENTER {
            column.center().x
        } else {
            column.left()
        };
        // A caption the column's edge would cut is left out.
        let galley = painter.layout_no_wrap(
            caption.to_uppercase(),
            FontId::proportional(METADATA),
            roles().muted,
        );
        let at = align
            .anchor_size(Pos2::new(x, strip.center().y - 2.0), galley.size())
            .min;
        if at.x < painter.clip_rect().left()
            || at.x + galley.size().x > painter.clip_rect().right() - 2.0
        {
            continue;
        }
        painter.galley(at, galley, roles().muted);
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

impl MixJamApp {
    /// The Mix view's parts outside the lanes: the column captions in the
    /// ruler strip, the Master and returns rows below the lanes, the "No
    /// tracks" note, the columns' scrolling, and the rows' edits.
    pub(in super::super) fn mix_block(
        &mut self,
        ui: &mut egui::Ui,
        captions_strip: Rect,
        block: Rect,
        (offset, empty, wheel): (f32, bool, f32),
        mut changes: mixer::Changes,
    ) {
        let column = captions_strip.width();
        let mut new_offset = captions(ui, captions_strip, offset) - wheel;
        let mut master = self.session.project().master_gain;
        let mut buses = self.session.project().fx;
        let mut board = self.session.project().master;
        let automated: [bool; BUSES] =
            std::array::from_fn(|bus| !self.session.project().return_automation[bus].is_empty());
        let master_rect = Rect::from_min_size(block.min, Vec2::new(column, BLOCK_ROW));
        let returns_rect = master_rect.translate(Vec2::new(0.0, BLOCK_ROW + 2.0));
        let pedals = PedalKind::ALL
            .into_iter()
            .filter(|kind| board.is_on(*kind))
            .count();
        let row = master_row(
            ui,
            master_rect,
            offset,
            &mut master,
            (&mut board.bypass, pedals),
        );
        changes.master |= row.changed;
        changes.bypass = row.bypass;
        let mut export = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(row.export)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        export.set_clip_rect(
            Columns::of(master_rect, offset)
                .body()
                .intersect(ui.clip_rect()),
        );
        export.spacing_mut().interact_size.y = row.export.height();
        self.export_tracks_button(&mut export, CLIP);
        let on_board = self.board_shown(BottomModule::FxReturns);
        let (fx, open) = returns_row(ui, returns_rect, offset, &mut buses, automated, on_board);
        edge_fade(ui, master_rect, master_rect, offset, false);
        edge_fade(ui, returns_rect, returns_rect, offset, true);
        changes.fx = fx;
        changes.open = open;
        if empty {
            let mut note = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(
                captions_strip.left_bottom() + Vec2::new(PINNED + 4.0, 34.0),
                Vec2::new((column - PINNED - 8.0).max(80.0), 48.0),
            )));
            note.label(view::heading("No tracks", 15.0));
            note.label("Place a sound in the arrangement.");
        }
        // Shift and the wheel over the rows scroll the columns, after any
        // control under the pointer has taken the wheel.
        let rows = Rect::from_min_max(
            captions_strip.min,
            Pos2::new(captions_strip.right(), block.bottom()),
        );
        if ui.rect_contains_pointer(rows) {
            let delta = ui.input(|input| input.smooth_scroll_delta.x);
            if delta != 0.0 {
                new_offset -= delta;
                ui.input_mut(|input| input.smooth_scroll_delta.x = 0.0);
            }
        }
        // A control reached with the keyboard scrolls into view.
        let focused = ui.memory(|memory| memory.focused());
        if focused != self.mix.revealed {
            self.mix.revealed = focused;
            if let Some(response) = focused.and_then(|id| ui.ctx().read_response(id))
                && response.interact_rect != response.rect
                && rows.y_range().contains(response.rect.center().y)
            {
                new_offset += reveal(response.rect, captions_strip.left(), column, offset);
            }
        }
        let new_offset = new_offset.clamp(0.0, max_offset(column));
        if new_offset != self.mix.scroll {
            self.mix.scroll = new_offset;
            ui.ctx().request_repaint();
        }
        self.apply_mixer_changes(ui, changes, master, buses, board);
    }
}
