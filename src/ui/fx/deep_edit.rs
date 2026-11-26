//! Layer 2 floating FX deep edit window (`816x544 pt` compact / `816x624 pt`
//! expanded) with unified bays, collapsible ducker, live scopes, and meters.
use super::super::controls::instrument::{
    KeyStrip, ScopeState, delay_scope_frame_state, key_strip, meter_bar_no_tail_dot,
    reverb_scope_frame_state, scope_frame_state,
};
use super::super::master::deep_cell;
use super::super::*;
use super::faces::{Slot, dial_slot};
use super::unit::{
    Side, SyncedTime, bus_switch, choice, effect_keys, module_and_preset_combos, synced_time_dial,
};
use super::{DEEP_EDIT_TITLE, fx_motif};
use mixjam_techno::application::FxBusMeterReading;
use mixjam_techno::application::fx::{
    Aetherform, BUSES, DelayCharacter, DelayParam, DistortionParam, DistortionShape, Echoform,
    FxBus, FxBuses, Module, ReverbCharacter, ReverbParam, SHIMMER_INTERVALS, SpaceModel, TimeMode,
};

const WINDOW_W: f32 = 816.0;
const WORKSPACE_W: f32 = 780.0;
const LEFT_BAY_W: f32 = 500.0;
const BAY_GAP: f32 = 8.0;
const TABS_H: f32 = 28.0;
const HEADER_H: f32 = 28.0;
const TOP_ROW_H: f32 = 240.0;
const MID_ROW_H: f32 = 108.0;
const DUCKER_FOLDED_H: f32 = 28.0;
const DUCKER_OPEN_H: f32 = 108.0;
const PREFIX: &str = "Deep edit";

#[derive(Clone, Copy)]
struct DialGrid {
    cols: usize,
    cell_w: f32,
    start_y: f32,
    well_offset: f32,
    label_offset: f32,
    row_h: f32,
}

impl DialGrid {
    const fn row(cols: usize, cell_w: f32, start_y: f32) -> Self {
        Self {
            cols,
            cell_w,
            start_y,
            well_offset: 38.0,
            label_offset: 58.0,
            row_h: 92.0,
        }
    }

    const fn mid_row(cols: usize, cell_w: f32) -> Self {
        Self {
            cols,
            cell_w,
            start_y: 46.0,
            well_offset: 32.0,
            label_offset: 50.0,
            row_h: 88.0,
        }
    }

    const fn two_row(cols: usize, cell_w: f32, start_y: f32, row_h: f32) -> Self {
        Self {
            cols,
            cell_w,
            start_y,
            well_offset: 30.0,
            label_offset: 47.0,
            row_h,
        }
    }
}

fn draw_dial_grid(ui: &mut egui::Ui, rect: Rect, grid: DialGrid, bus: &mut FxBus, slots: &[Slot]) {
    for (i, slot) in slots.iter().enumerate() {
        let positions = grid_positions(rect, grid, i, slots.len());
        dial_slot(ui, positions, PREFIX, bus, slot, None);
    }
}

/// The knob, value well, and label centres of dial `index` of `count`.
fn grid_positions(rect: Rect, grid: DialGrid, index: usize, count: usize) -> [Pos2; 3] {
    let (offset, row) = grid_cell(index, count, grid.cols);
    let x = rect.center().x + offset * grid.cell_w;
    let y = rect.top() + grid.start_y + row as f32 * grid.row_h;
    [
        Pos2::new(x, y),
        Pos2::new(x, y + grid.well_offset),
        Pos2::new(x, y + grid.label_offset),
    ]
}

/// The cells of a grid of `count` dials, for the bay's motif art to follow.
fn grid_cells(rect: Rect, grid: DialGrid, count: usize) -> Vec<Rect> {
    (0..count)
        .map(|index| {
            let [knob, _, label] = grid_positions(rect, grid, index, count);
            deep_cell(knob, label.y, grid.cell_w)
        })
        .collect()
}

/// The column offset of dial `index` from the bay's center, in cells, and its
/// row. Each row centers on its own dial count, so a short last row stays
/// centered under the full rows.
fn grid_cell(index: usize, count: usize, cols: usize) -> (f32, usize) {
    let cols = cols.max(1);
    let row = index / cols;
    let in_row = (count - row * cols).min(cols);
    let col = index % cols;
    (col as f32 + 0.5 - in_row as f32 / 2.0, row)
}

fn top_and_mid_bays(area: Rect) -> (Rect, Rect, Rect) {
    let right_w = (area.width() - LEFT_BAY_W - BAY_GAP).max(220.0);
    let top_left = Rect::from_min_size(area.min, Vec2::new(LEFT_BAY_W, TOP_ROW_H));
    let scope = Rect::from_min_size(
        area.min + Vec2::new(LEFT_BAY_W + BAY_GAP, 0.0),
        Vec2::new(right_w, TOP_ROW_H),
    );
    let mid = Rect::from_min_size(
        area.min + Vec2::new(0.0, TOP_ROW_H + BAY_GAP),
        Vec2::new(area.width(), MID_ROW_H),
    );
    (top_left, scope, mid)
}

fn ducker_rect(area: Rect, expanded: bool) -> Rect {
    let h = if expanded {
        DUCKER_OPEN_H
    } else {
        DUCKER_FOLDED_H
    };
    Rect::from_min_size(
        area.min + Vec2::new(0.0, TOP_ROW_H + BAY_GAP + MID_ROW_H + BAY_GAP),
        Vec2::new(area.width(), h),
    )
}

#[derive(Clone, Copy)]
struct BaySkin {
    motif: super::super::master::MotifSpec,
    accent: Color32,
    active: bool,
    bypassed: bool,
    seed: u64,
}

impl BaySkin {
    fn scope_state(self) -> ScopeState<'static> {
        let status = if self.bypassed {
            Some("BYPASSED")
        } else if !self.active {
            Some("INACTIVE")
        } else {
            None
        };
        ScopeState {
            active: self.active,
            status,
            accent: self.accent,
        }
    }
}

fn bus_bypassed(bus: &FxBus) -> bool {
    match &bus.module {
        Module::EchoformDelay(d) => d.bypass,
        Module::AetherformReverb(r) => r.bypass,
        Module::EmberformDrive(d) => d.bypass,
        Module::Empty => false,
    }
}

fn clear_legacy_bypass(module: &mut Module) {
    match module {
        Module::EchoformDelay(d) => d.bypass = false,
        Module::AetherformReverb(r) => r.bypass = false,
        Module::EmberformDrive(d) => d.bypass = false,
        Module::Empty => {}
    }
}

fn bus_skin_active(bus: &FxBus) -> bool {
    bus.powered && !bus.module.is_empty() && !bus_bypassed(bus)
}

fn bus_short_module(module: &Module) -> &'static str {
    match module {
        Module::Empty => "Empty",
        Module::EchoformDelay(_) => "Delay",
        Module::AetherformReverb(_) => "Reverb",
        Module::EmberformDrive(_) => "Drive",
    }
}

fn draw_badge(
    p: &egui::Painter,
    right_x: f32,
    cy: f32,
    tag: &str,
    bg: Color32,
    fg: Color32,
) -> f32 {
    let w = if tag.len() == 1 { 13.0 } else { 22.0 };
    let badge_rect = Rect::from_center_size(Pos2::new(right_x - w * 0.5, cy), Vec2::new(w, 13.0));
    p.rect_filled(badge_rect, 2.5, bg);
    p.text(
        badge_rect.center(),
        Align2::CENTER_CENTER,
        tag,
        FontId::proportional(8.0),
        fg,
    );
    badge_rect.left()
}

fn paint_tab_status(
    p: &egui::Painter,
    rect: Rect,
    bus: &FxBus,
    trim: Color32,
    active: bool,
    solo: bool,
) {
    let cy = rect.center().y + 1.0;
    if !bus.module.is_empty() {
        let dot = Pos2::new(rect.left() + 9.0, cy);
        if active {
            p.circle_filled(dot, 2.4, trim);
        } else {
            p.circle_stroke(
                dot,
                2.2,
                Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.55)),
            );
        }
    }
    let mut right_edge = rect.right() - 4.0;
    if bus_bypassed(bus) {
        right_edge = draw_badge(
            p,
            right_edge,
            cy,
            "BYP",
            roles().ink,
            roles().legend.gamma_multiply(0.76),
        ) - 2.0;
    }
    if solo && bus.is_active() {
        draw_badge(p, right_edge, cy, "S", roles().lamp_warn, roles().ink);
    }
}

fn tab_state_summary(bus: &FxBus, active: bool, solo: bool) -> String {
    let state = if bus.module.is_empty() {
        "empty"
    } else if active {
        "active"
    } else if bus_bypassed(bus) {
        "bypassed"
    } else {
        "off"
    };
    if solo && bus.is_active() {
        format!("{state}, solo")
    } else {
        state.to_owned()
    }
}

fn bus_tab_chip(
    ui: &mut egui::Ui,
    bus_index: usize,
    bus: &FxBus,
    selected: bool,
    solo: bool,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(116.0, TABS_H), Sense::hover());
    let name = format!("{PREFIX} bus {}", bus_index + 1);
    let resp = ui.interact(
        rect,
        ui.id().with(("fx-deep-tab", bus_index)),
        Sense::click(),
    );
    let (motif, _) = fx_motif(&bus.module);
    let active = bus_skin_active(bus);
    let ([face, _], trim) = motif.deep_zones(active);
    let p = ui.painter();
    p.rect_filled(rect, 4.0, face);
    let stripe = if active {
        trim
    } else {
        trim.gamma_multiply(0.28)
    };
    p.rect_filled(
        Rect::from_min_size(
            rect.min + Vec2::new(2.0, 1.5),
            Vec2::new(rect.width() - 4.0, 2.5),
        ),
        1.0,
        stripe,
    );
    if resp.hovered() {
        p.rect_filled(rect, 4.0, Color32::from_white_alpha(20));
    }
    let (border_w, border_c) = if selected {
        (1.8_f32, roles().legend)
    } else {
        (1.0_f32, roles().ink.gamma_multiply(0.65))
    };
    p.rect_stroke(
        rect,
        4.0,
        Stroke::new(border_w, border_c),
        egui::StrokeKind::Inside,
    );
    if resp.has_focus() {
        p.rect_stroke(
            rect.expand(1.0),
            4.5,
            Stroke::new(1.5_f32, controls::focus_color(ui)),
            egui::StrokeKind::Outside,
        );
    }
    paint_tab_status(p, rect, bus, trim, active, solo);
    let (text_x, align) = if bus.module.is_empty() {
        (rect.center().x, Align2::CENTER_CENTER)
    } else {
        (rect.left() + 15.0, Align2::LEFT_CENTER)
    };
    p.text(
        Pos2::new(text_x, rect.center().y + 1.0),
        align,
        format!("FX {} · {}", bus_index + 1, bus_short_module(&bus.module)),
        FontId::proportional(10.5),
        roles().legend,
    );
    let summary = tab_state_summary(bus, active, solo);
    named(&resp, egui::WidgetType::Button, &name);
    resp.ctx.accesskit_node_builder(resp.id, |node| {
        node.set_selected(selected);
        node.set_value(format!("{}, {summary}", bus_short_module(&bus.module)));
    });
    resp.hover_hint(format!(
        "Switch FX deep edit to FX {} ({}, {summary}). Alt+{} jumps here.",
        bus_index + 1,
        bus_short_module(&bus.module),
        bus_index + 1
    ))
}

fn tab_arrow_step(resp: &Response, index: usize) -> Option<usize> {
    if !resp.has_focus() {
        return None;
    }
    resp.ctx.input_mut(|input| {
        if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
            Some((index + BUSES - 1) % BUSES)
        } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
            Some((index + 1) % BUSES)
        } else {
            None
        }
    })
}

fn tail_status_pill(ui: &mut egui::Ui, bus: &FxBus, reading: FxBusMeterReading) {
    let has_tail = bus.module.has_tail();
    let tail_rect = ui
        .allocate_exact_size(Vec2::new(92.0, TABS_H), Sense::hover())
        .0;
    controls::well(ui, tail_rect);
    let (dot_color, text, text_color, hint) = if !has_tail {
        (
            roles().lamp_off,
            "No tail",
            roles().legend.gamma_multiply(0.6),
            "This effect has no ringing feedback tail.",
        )
    } else if reading.tail_active {
        (
            roles().lamp_on,
            "Tail: Active",
            roles().legend,
            "Delay or reverb tail is currently ringing out.",
        )
    } else {
        (
            roles().lamp_off,
            "Tail: Idle",
            roles().legend.gamma_multiply(0.82),
            "No ringing delay or reverb tail is active.",
        )
    };
    ui.painter().circle_filled(
        Pos2::new(tail_rect.left() + 10.0, tail_rect.center().y),
        2.6,
        dot_color,
    );
    ui.painter().text(
        Pos2::new(tail_rect.left() + 17.0, tail_rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(10.5),
        text_color,
    );
    let resp = ui.interact(tail_rect, ui.id().with("fx-deep-tail-pill"), Sense::hover());
    named(&resp, egui::WidgetType::Label, "Deep edit tail status");
    resp.ctx.accesskit_node_builder(resp.id, |node| {
        node.set_label("Deep edit tail status");
        node.set_value(text);
    });
    resp.hover_hint(hint);
}

fn bus_tabs(
    ui: &mut egui::Ui,
    bus_index: &mut usize,
    buses: &[FxBus; BUSES],
    meters: &[FxBusMeterReading; BUSES],
    ducks: [bool; BUSES],
    solos: [bool; BUSES],
) {
    let mut focus_target: Option<usize> = None;
    let mut tab_ids = [egui::Id::NULL; BUSES];
    ui.horizontal(|ui| {
        for (i, bus) in buses.iter().enumerate() {
            let resp = bus_tab_chip(ui, i, bus, i == *bus_index, solos[i]);
            tab_ids[i] = resp.id;
            if let Some(next) = tab_arrow_step(&resp, i) {
                *bus_index = next;
                focus_target = Some(next);
            }
            if resp.clicked() {
                *bus_index = i;
            }
        }
        ui.separator();
        let m = meters[*bus_index];
        let meter_rect = ui
            .allocate_exact_size(Vec2::new(148.0, TABS_H), Sense::hover())
            .0;
        meter_bar_no_tail_dot(
            ui,
            meter_rect,
            "Deep edit return meters",
            m,
            ducks[*bus_index],
        );
        tail_status_pill(ui, &buses[*bus_index], m);
    });
    if let Some(target) = focus_target {
        // The tab row took the arrow, so egui must not move focus again.
        ui.ctx().memory_mut(|m| {
            m.request_focus(tab_ids[target]);
            m.move_focus(egui::FocusDirection::None);
        });
    }
}

fn fx_undo_cue_id() -> egui::Id {
    egui::Id::new("fx-deep-undo-cue")
}

fn fx_workspace_height(empty: bool, ducker_open: bool) -> f32 {
    let ducker_h = if !empty && ducker_open {
        DUCKER_OPEN_H
    } else {
        DUCKER_FOLDED_H
    };
    TOP_ROW_H + BAY_GAP + MID_ROW_H + BAY_GAP + ducker_h
}

fn bus_reset_candidate(bus: &FxBus) -> Option<FxBus> {
    let (first_preset, _) = bus.module.presets().first()?;
    let mut candidate = *bus;
    if candidate.load_preset(first_preset) && candidate != *bus {
        Some(candidate)
    } else {
        None
    }
}

fn bus_header_switches(ui: &mut egui::Ui, top_rect: Rect, bus: &mut FxBus, mut solo: bool) -> bool {
    let empty = bus.module.is_empty();
    let active = bus.is_active();
    let mut idle = (false, false);
    let (powered, limited) = if empty {
        (&mut idle.0, &mut idle.1)
    } else {
        (&mut bus.powered, &mut bus.limiter)
    };
    let was_powered = *powered;
    ui.add_enabled_ui(!empty, |ui| {
        let resp = bus_switch(
            ui,
            top_rect.min + Vec2::new(326.0, 4.0),
            &format!("{PREFIX} power"),
            "Active",
            powered,
            "Switch this FX bus on or off.",
            false,
        );
        resp.disabled_hint("Load an effect before switching this bus on.");
    });
    if !empty && *powered && !was_powered {
        clear_legacy_bypass(&mut bus.module);
    }
    let solo_toggled = ui
        .add_enabled_ui(active, |ui| {
            let resp = bus_switch(
                ui,
                top_rect.min + Vec2::new(400.0, 4.0),
                &format!("{PREFIX} solo"),
                "Solo",
                &mut solo,
                "Monitor only soloed FX returns during playback.",
                false,
            );
            resp.disabled_hint("Load and activate this bus before soloing.")
        })
        .inner
        .changed();
    ui.add_enabled_ui(!empty, |ui| {
        let resp = bus_switch(
            ui,
            top_rect.min + Vec2::new(466.0, 4.0),
            &format!("{PREFIX} limiter"),
            "Limiter",
            limited,
            super::super::mixer::LIMITER_HINT,
            false,
        );
        resp.disabled_hint("Load an effect before enabling the bus limiter.");
    });
    solo_toggled
}

fn header_actions(ui: &mut egui::Ui, top_rect: Rect, bus: &mut FxBus) -> (bool, bool) {
    let reset_target = bus_reset_candidate(bus);
    let reset_rect = Rect::from_min_size(
        Pos2::new(top_rect.right() - 140.0, top_rect.top() + 2.0),
        Vec2::new(50.0, 24.0),
    );
    let reset_name = format!("{PREFIX} reset effect");
    let reset_resp = name_control(
        controls::button_at(
            ui,
            reset_rect,
            &reset_name,
            "Reset",
            controls::ButtonRole::Secondary,
            reset_target.is_some(),
        ),
        &reset_name,
    );
    let mut reset_clicked = false;
    if reset_resp
        .hover_hint("Restore this effect's parameters to default (keeps Active, Limiter, and Key).")
        .disabled_hint(if bus.module.is_empty() {
            "Load an effect first."
        } else {
            "Effect parameters are already at default."
        })
        .clicked()
        && let Some(candidate) = reset_target
    {
        *bus = candidate;
        controls::set_timed_cue(ui.ctx(), fx_undo_cue_id(), "Ctrl+Z to undo");
        reset_clicked = true;
    }

    let has_tail = bus.module.has_tail();
    let clear_rect = Rect::from_min_size(
        Pos2::new(top_rect.right() - 84.0, top_rect.top() + 2.0),
        Vec2::new(84.0, 24.0),
    );
    let clear_name = format!("{PREFIX} clear tail");
    let resp = controls::button_at(
        ui,
        clear_rect,
        &clear_name,
        "Clear tail",
        controls::ButtonRole::Secondary,
        has_tail,
    );
    let clear_tail = name_control(resp, &clear_name)
        .hover_hint("Cut the effect's ringing tail now. Not an edit.")
        .disabled_hint("Only Delay and Reverb produce a ringing tail.")
        .clicked();
    (clear_tail, reset_clicked)
}

fn header_bar(ui: &mut egui::Ui, bus: &mut FxBus, solo: bool) -> (bool, bool, bool) {
    let top_rect = ui
        .allocate_exact_size(Vec2::new(WORKSPACE_W, HEADER_H), Sense::hover())
        .0;
    let before_kind = bus_short_module(&bus.module);
    let before_preset = bus.matching_preset();
    module_and_preset_combos(
        ui,
        Rect::from_min_size(top_rect.min + Vec2::new(0.0, 2.0), Vec2::new(172.0, 24.0)),
        Rect::from_min_size(top_rect.min + Vec2::new(180.0, 2.0), Vec2::new(136.0, 24.0)),
        PREFIX,
        bus,
        false,
    );
    let discrete_combo =
        bus_short_module(&bus.module) != before_kind || bus.matching_preset() != before_preset;
    if discrete_combo {
        controls::set_timed_cue(ui.ctx(), fx_undo_cue_id(), "Ctrl+Z to undo");
    }
    let solo_toggled = bus_header_switches(ui, top_rect, bus, solo);
    if let Some(text) = controls::timed_cue_text(ui.ctx(), fx_undo_cue_id()) {
        ui.painter().text(
            Pos2::new(top_rect.left() + 544.0, top_rect.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(10.5),
            roles().legend.gamma_multiply(0.8),
        );
    }
    let (clear_tail, reset_clicked) = header_actions(ui, top_rect, bus);
    (clear_tail, solo_toggled, discrete_combo || reset_clicked)
}

fn draw_bay_dials(
    ui: &mut egui::Ui,
    rect: Rect,
    title: &str,
    skin: BaySkin,
    grid: DialGrid,
    bus: &mut FxBus,
    slots: &[Slot],
) {
    super::super::master::paint_deep_edit_bay(
        ui,
        rect,
        title,
        skin.motif,
        skin.active,
        skin.seed,
        None,
    );
    draw_dial_grid(ui, rect, grid, bus, slots);
}

fn paint_inactive_bay_tag(ui: &egui::Ui, rect: Rect, skin: BaySkin) {
    let tag = if skin.bypassed {
        Some("BYPASSED")
    } else if !skin.active {
        Some("INACTIVE")
    } else {
        None
    };
    if let Some(tag) = tag {
        ui.painter().text(
            Pos2::new(rect.right() - 14.0, rect.top() + 11.0),
            Align2::RIGHT_CENTER,
            tag,
            FontId::proportional(9.5),
            skin.motif.face_ink().gamma_multiply(0.85),
        );
    }
}

fn ducker_expanded_id() -> egui::Id {
    egui::Id::new("fx-deep-ducker-expanded")
}

fn ducker_is_expanded(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(ducker_expanded_id()))
        .unwrap_or_else(|| ctx.viewport_rect().height() >= 760.0)
}

fn format_duck_key(key: u8) -> String {
    if key == 0 {
        "Own".to_owned()
    } else {
        format!("Trk {key}")
    }
}

fn ducker_summary(bus: &FxBus) -> Option<(String, &'static [Slot])> {
    match &bus.module {
        Module::EmberformDrive(d) => Some((
            format!(
                "{} · {:.0} dB · {:.0}% · {:.0} ms",
                format_duck_key(d.duck_key),
                d.duck_threshold_db,
                d.duck_amount_percent,
                d.duck_release_ms
            ),
            &[
                Slot::Drive(DistortionParam::DuckKey),
                Slot::Drive(DistortionParam::DuckThreshold),
                Slot::Drive(DistortionParam::DuckAmount),
                Slot::Drive(DistortionParam::DuckRelease),
            ],
        )),
        Module::EchoformDelay(d) => Some((
            format!(
                "{} · {:.0} dB · {:.0}% · {:.0} ms",
                format_duck_key(d.duck_key),
                d.duck_threshold_db,
                d.duck_amount,
                d.duck_release
            ),
            &[
                Slot::Delay(DelayParam::DuckKey),
                Slot::Delay(DelayParam::DuckThreshold),
                Slot::Delay(DelayParam::DuckAmount),
                Slot::Delay(DelayParam::DuckRelease),
            ],
        )),
        Module::AetherformReverb(r) => Some((
            format!(
                "{} · {:.0} dB · {:.0}% · {:.0} ms",
                format_duck_key(r.duck_key),
                r.duck_threshold_db,
                r.duck_amount_percent,
                r.duck_release_ms
            ),
            &[
                Slot::Reverb(ReverbParam::DuckKey),
                Slot::Reverb(ReverbParam::DuckThreshold),
                Slot::Reverb(ReverbParam::DuckAmount),
                Slot::Reverb(ReverbParam::DuckRelease),
            ],
        )),
        Module::Empty => None,
    }
}

fn ducker_bay(ui: &mut egui::Ui, area: Rect, bus: &mut FxBus, skin: BaySkin, expanded: bool) {
    let Some((summary, slots)) = ducker_summary(bus) else {
        return;
    };
    let rect = ducker_rect(area, expanded);
    let title = if expanded {
        "▾ Post-Limiter Ducker"
    } else {
        "▸ Post-Limiter Ducker"
    };
    super::super::master::paint_deep_edit_bay(
        ui,
        rect,
        title,
        skin.motif,
        skin.active,
        skin.seed,
        None,
    );
    let header_bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 22.0));
    ui.painter().text(
        Pos2::new(rect.right() - 14.0, rect.top() + 11.0),
        Align2::RIGHT_CENTER,
        if expanded { "Fold" } else { &summary },
        FontId::proportional(10.0),
        skin.motif.face_ink().gamma_multiply(0.82),
    );
    let toggle_name = format!("{PREFIX} toggle ducker");
    let resp = ui.interact(header_bar, ui.id().with(&toggle_name), Sense::click());
    named(&resp, egui::WidgetType::Button, &toggle_name);
    if resp.has_focus() {
        ui.painter().rect_stroke(
            header_bar.shrink(1.0),
            3.0,
            Stroke::new(1.5_f32, controls::focus_color(ui)),
            egui::StrokeKind::Inside,
        );
    }
    if resp
        .hover_hint(if expanded {
            "Fold Post-Limiter Ducker controls."
        } else {
            "Expand Post-Limiter Ducker controls."
        })
        .clicked()
    {
        ui.ctx()
            .data_mut(|data| data.insert_temp(ducker_expanded_id(), !expanded));
    }
    if expanded {
        draw_dial_grid(ui, rect, DialGrid::mid_row(4, 176.0), bus, slots);
    }
}

fn drive_workspace(
    ui: &mut egui::Ui,
    area: Rect,
    bus: &mut FxBus,
    skin: BaySkin,
    ducker_open: bool,
) {
    let (drive_bay, scope_bay, tone_bay) = top_and_mid_bays(area);
    let grid = DialGrid::row(3, 136.0, 114.0);

    super::super::master::paint_deep_edit_bay(
        ui,
        drive_bay,
        "Drive & Curve",
        skin.motif,
        skin.active,
        skin.seed,
        Some(&grid_cells(drive_bay, grid, 3)),
    );
    paint_inactive_bay_tag(ui, drive_bay, skin);
    if let Module::EmberformDrive(drive) = &mut bus.module {
        let mut points = [0.0_f32; 65];
        Session::fx_drive_curve(drive, &mut points);
        scope_frame_state(ui, scope_bay, "Transfer Curve", &points, skin.scope_state());
        choice(
            ui,
            Rect::from_min_size(
                drive_bay.min + Vec2::new(14.0, 28.0),
                Vec2::new(136.0, 24.0),
            ),
            &format!("{PREFIX} drive type"),
            &mut drive.shape,
            &DistortionShape::ALL.map(|s| (s, s.label())),
            "Select wave-shaper or bitcrusher model.",
        );
        bus_switch(
            ui,
            drive_bay.min + Vec2::new(164.0, 29.0),
            &format!("{PREFIX} level match"),
            "Level match",
            &mut drive.level_match,
            "Equalize output loudness to match driven input.",
            false,
        );
    }
    draw_dial_grid(
        ui,
        drive_bay,
        grid,
        bus,
        &[
            Slot::Drive(DistortionParam::Drive),
            Slot::Drive(DistortionParam::Asymmetry),
            Slot::Drive(DistortionParam::Width),
        ],
    );

    draw_bay_dials(
        ui,
        tone_bay,
        "Tone, Bitcrush & Output",
        skin,
        DialGrid::mid_row(6, 122.0),
        bus,
        &[
            Slot::Drive(DistortionParam::LowCut),
            Slot::Drive(DistortionParam::HighCut),
            Slot::Drive(DistortionParam::Bits),
            Slot::Drive(DistortionParam::CrushRate),
            Slot::Drive(DistortionParam::Output),
            Slot::Return,
        ],
    );

    ducker_bay(ui, area, bus, skin, ducker_open);
}

fn delay_workspace(
    ui: &mut egui::Ui,
    area: Rect,
    bus: &mut FxBus,
    bpm: f64,
    skin: BaySkin,
    ducker_open: bool,
) {
    let (time_bay, scope_bay, tone_bay) = top_and_mid_bays(area);
    let grid = DialGrid::row(4, 112.0, 116.0);

    super::super::master::paint_deep_edit_bay(
        ui,
        time_bay,
        "Time & Feedback",
        skin.motif,
        skin.active,
        skin.seed,
        Some(&grid_cells(time_bay, grid, 4)),
    );
    paint_inactive_bay_tag(ui, time_bay, skin);
    // The timing, character, and ping-pong controls sit in one strip above the split sub-bays.
    if let Module::EchoformDelay(delay) = &mut bus.module {
        let mut sync = delay.mode == TimeMode::Sync;
        bus_switch(
            ui,
            time_bay.min + Vec2::new(12.0, 26.0),
            &format!("{PREFIX} sync"),
            "Sync",
            &mut sync,
            "Lock delay times to project tempo divisions.",
            false,
        );
        delay.mode = if sync { TimeMode::Sync } else { TimeMode::Free };
        let link_before = delay.link_times;
        bus_switch(
            ui,
            time_bay.min + Vec2::new(82.0, 26.0),
            &format!("{PREFIX} link times"),
            "Link L/R",
            &mut delay.link_times,
            "Keep left and right delay times matched.",
            false,
        );
        if delay.link_times && !link_before {
            delay.time_ms_r = delay.time_ms_l;
            delay.division_r = delay.division_l;
        }
        ui.painter().text(
            time_bay.min + Vec2::new(172.0, 36.0),
            Align2::LEFT_CENTER,
            "Mode",
            FontId::proportional(10.5),
            roles().legend.gamma_multiply(0.85),
        );
        let hint = "Feedback saturation and character.";
        key_strip(
            ui,
            KeyStrip {
                rect: Rect::from_min_size(
                    time_bay.min + Vec2::new(208.0, 26.0),
                    Vec2::new(160.0, 22.0),
                ),
                vertical: false,
                name: &format!("{PREFIX} delay character"),
                accent: skin.accent,
            },
            &mut delay.character,
            &[
                (DelayCharacter::Digital, "Digital", hint),
                (DelayCharacter::Analog, "Analog", hint),
                (DelayCharacter::Tape, "Tape", hint),
            ],
        );
        bus_switch(
            ui,
            time_bay.min + Vec2::new(380.0, 26.0),
            &format!("{PREFIX} ping-pong"),
            "Ping-pong",
            &mut delay.ping_pong,
            "Alternate echoes across stereo channels.",
            false,
        );
    }
    time_dials(ui, time_bay, grid, bus);

    draw_bay_dials(
        ui,
        tone_bay,
        "Tone, Smear, Mod & Output",
        skin,
        DialGrid::mid_row(9, 84.0),
        bus,
        &[
            Slot::Delay(DelayParam::LowCut),
            Slot::Delay(DelayParam::HighCut),
            Slot::Delay(DelayParam::FilterResonance),
            Slot::Delay(DelayParam::Smear),
            Slot::Delay(DelayParam::ModRate),
            Slot::Delay(DelayParam::ModDepth),
            Slot::Delay(DelayParam::Width),
            Slot::Delay(DelayParam::Output),
            Slot::Return,
        ],
    );

    ducker_bay(ui, area, bus, skin, ducker_open);

    if let Module::EchoformDelay(delay) = &bus.module {
        let mut taps = [(0.0_f32, 0.0_f32, 0.0_f32); 32];
        let count = Session::fx_delay_taps(delay, bpm, &mut taps);
        delay_scope_frame_state(
            ui,
            scope_bay,
            "Tap & Decay",
            &taps[..count],
            delay.ping_pong,
            skin.scope_state(),
        );
    }
}

/// Time L, Time R, Feedback, and Drive in the upper delay bay.
fn time_dials(ui: &mut egui::Ui, time_bay: Rect, grid: DialGrid, bus: &mut FxBus) {
    let slots = [
        Slot::Delay(DelayParam::TimeL),
        Slot::Delay(DelayParam::TimeR),
        Slot::Delay(DelayParam::Feedback),
        Slot::Delay(DelayParam::Drive),
    ];
    for (index, slot) in slots.iter().enumerate() {
        let positions = grid_positions(time_bay, grid, index, slots.len());
        time_dial_entry(ui, positions, slot, bus);
    }
}

fn time_dial_entry(ui: &mut egui::Ui, positions: [Pos2; 3], slot: &Slot, bus: &mut FxBus) {
    match (time_side(slot), &mut bus.module) {
        (Some(side), Module::EchoformDelay(delay)) if delay.mode == TimeMode::Sync => {
            synced_time_slot(ui, positions, side, delay);
        }
        _ => dial_slot(ui, positions, PREFIX, bus, slot, None),
    }
}

/// The channel whose time `slot` sets, if it is a time dial.
fn time_side(slot: &Slot) -> Option<Side> {
    match slot {
        Slot::Delay(DelayParam::TimeL) => Some(Side::Left),
        Slot::Delay(DelayParam::TimeR) => Some(Side::Right),
        _ => None,
    }
}

/// One channel's time dial in Sync mode.
fn synced_time_slot(ui: &mut egui::Ui, positions: [Pos2; 3], side: Side, delay: &mut Echoform) {
    let at = SyncedTime {
        positions,
        well: 68.0,
        full: true,
        side,
        label: if side == Side::Left {
            "Time L"
        } else {
            "Time R"
        },
    };
    // The right time follows the left while the two are linked.
    let enabled = side == Side::Left || !delay.link_times;
    ui.add_enabled_ui(enabled, |ui| synced_time_dial(ui, at, PREFIX, delay));
}

fn reverb_workspace(
    ui: &mut egui::Ui,
    area: Rect,
    bus: &mut FxBus,
    skin: BaySkin,
    ducker_open: bool,
) {
    let (space_bay, scope_bay, tone_bay) = top_and_mid_bays(area);
    let grid = DialGrid::two_row(5, 92.0, 74.0, 80.0);

    super::super::master::paint_deep_edit_bay(
        ui,
        space_bay,
        "Space, Damping & Shimmer",
        skin.motif,
        skin.active,
        skin.seed,
        Some(&grid_cells(space_bay, grid, 9)),
    );
    paint_inactive_bay_tag(ui, space_bay, skin);
    reverb_switches(ui, space_bay, grid, bus, skin.accent);
    draw_dial_grid(
        ui,
        space_bay,
        grid,
        bus,
        &[
            Slot::Reverb(ReverbParam::PreDelay),
            Slot::Reverb(ReverbParam::Decay),
            Slot::Reverb(ReverbParam::BassMult),
            Slot::Reverb(ReverbParam::TrebleMult),
            Slot::Reverb(ReverbParam::Size),
            Slot::Reverb(ReverbParam::LateBalance),
            Slot::Reverb(ReverbParam::Diffusion),
            Slot::Reverb(ReverbParam::Density),
            Slot::Reverb(ReverbParam::ShimmerAmount),
        ],
    );

    draw_bay_dials(
        ui,
        tone_bay,
        "Tone, Modulation & Output",
        skin,
        DialGrid::mid_row(8, 94.0),
        bus,
        &[
            Slot::Reverb(ReverbParam::LowCut),
            Slot::Reverb(ReverbParam::HighCut),
            Slot::Reverb(ReverbParam::ModRate),
            Slot::Reverb(ReverbParam::ModDepth),
            Slot::Reverb(ReverbParam::Drive),
            Slot::Reverb(ReverbParam::Width),
            Slot::Reverb(ReverbParam::Output),
            Slot::Return,
        ],
    );

    ducker_bay(ui, area, bus, skin, ducker_open);

    if let Module::AetherformReverb(reverb) = &bus.module {
        let tag = reverb_tag(reverb);
        let profile = Session::fx_reverb_profile(reverb);
        reverb_scope_frame_state(
            ui,
            scope_bay,
            "Decay & Space",
            &tag,
            &profile,
            skin.scope_state(),
        );
    }
}

/// The scope's tag: the space, the character, and the shimmer interval.
fn reverb_tag(reverb: &Aetherform) -> String {
    let character = match reverb.character {
        ReverbCharacter::Natural => "NATURAL",
        ReverbCharacter::Vintage => "VINTAGE",
        ReverbCharacter::Bloom => "BLOOM",
    };
    let mut tag = format!(
        "{} · {character}",
        reverb.space_model.label().to_uppercase()
    );
    if reverb.shimmer_enabled {
        tag.push_str(&format!(" +{}", reverb.shimmer_interval_semitones));
    }
    tag
}

/// The selectors and switches along the top of the space bay.
fn reverb_switches(
    ui: &mut egui::Ui,
    space_bay: Rect,
    grid: DialGrid,
    bus: &mut FxBus,
    accent: Color32,
) {
    let Module::AetherformReverb(reverb) = &mut bus.module else {
        return;
    };
    let at = |x: f32, w: f32| {
        Rect::from_min_size(space_bay.min + Vec2::new(x, 24.0), Vec2::new(w, 22.0))
    };
    ui.painter().text(
        space_bay.min + Vec2::new(12.0, 35.0),
        Align2::LEFT_CENTER,
        "Space",
        FontId::proportional(10.0),
        roles().legend.gamma_multiply(0.85),
    );
    let space = "Acoustic space model.";
    key_strip(
        ui,
        KeyStrip {
            rect: at(46.0, 146.0),
            vertical: false,
            name: &format!("{PREFIX} space"),
            accent,
        },
        &mut reverb.space_model,
        &SpaceModel::ALL.map(|m| (m, m.label(), space)),
    );
    ui.painter().text(
        space_bay.min + Vec2::new(200.0, 35.0),
        Align2::LEFT_CENTER,
        "Tone",
        FontId::proportional(10.0),
        roles().legend.gamma_multiply(0.85),
    );
    let tail = "Tail character.";
    key_strip(
        ui,
        KeyStrip {
            rect: at(228.0, 120.0),
            vertical: false,
            name: &format!("{PREFIX} reverb character"),
            accent,
        },
        &mut reverb.character,
        &[
            (ReverbCharacter::Natural, "Natural", tail),
            (ReverbCharacter::Vintage, "Vintage", tail),
            (ReverbCharacter::Bloom, "Bloom", tail),
        ],
    );
    bus_switch(
        ui,
        space_bay.min + Vec2::new(354.0, 25.0),
        &format!("{PREFIX} early reflections"),
        "Early",
        &mut reverb.early_reflections_enabled,
        "Initial room reflections.",
        false,
    );
    bus_switch(
        ui,
        space_bay.min + Vec2::new(416.0, 25.0),
        &format!("{PREFIX} shimmer"),
        "Shimmer",
        &mut reverb.shimmer_enabled,
        "Pitch-shifted tail feedback.",
        false,
    );
    // The interval keys stand in a cleanly padded column beside row 2's Shimmer dial.
    let [knob, _, label] = grid_positions(space_bay, grid, 8, 9);
    let column_left = knob.x + grid.cell_w / 2.0 + 8.0;
    let column_right = space_bay.right() - 12.0;
    ui.painter().text(
        Pos2::new((column_left + column_right) * 0.5, knob.y - 19.0),
        Align2::CENTER_CENTER,
        "Pitch",
        FontId::proportional(9.5),
        roles()
            .legend
            .gamma_multiply(if reverb.shimmer_enabled { 0.85 } else { 0.68 }),
    );
    let column = Rect::from_min_max(
        Pos2::new(column_left, knob.y - 10.0),
        Pos2::new(column_right, (label.y + 13.0).min(space_bay.bottom() - 8.0)),
    );
    let labels: Vec<String> = SHIMMER_INTERVALS.iter().map(|s| format!("+{s}")).collect();
    let options: Vec<(u8, &str, &str)> = SHIMMER_INTERVALS
        .iter()
        .zip(&labels)
        .map(|(s, l)| (*s, l.as_str(), "Shimmer pitch interval in semitones."))
        .collect();
    ui.add_enabled_ui(reverb.shimmer_enabled, |ui| {
        key_strip(
            ui,
            KeyStrip {
                rect: column,
                vertical: true,
                name: &format!("{PREFIX} shimmer interval"),
                accent,
            },
            &mut reverb.shimmer_interval_semitones,
            &options,
        );
    });
}

fn draw_module_workspace(
    ui: &mut egui::Ui,
    area: Rect,
    bus_index: usize,
    bus: &mut FxBus,
    bpm: f64,
    ducker_open: bool,
) {
    let ui = &mut ui.new_child(egui::UiBuilder::new().id_salt("fx-bays").max_rect(area));
    let (motif, _) = fx_motif(&bus.module);
    let skin = BaySkin {
        motif,
        accent: motif.scope_accent(),
        active: bus_skin_active(bus),
        bypassed: bus_bypassed(bus),
        seed: bus_index as u64 + 21,
    };
    ui.scope(|ui| {
        ui.visuals_mut().selection.bg_fill = skin.accent;
        theme::with_knob_style(motif.knob, || match bus.module {
            Module::Empty => {
                super::super::master::paint_deep_edit_bay(
                    ui,
                    area,
                    "Empty bus",
                    skin.motif,
                    skin.active,
                    skin.seed,
                    Some(&[]),
                );
                effect_keys(ui, area.shrink2(Vec2::new(16.0, 26.0)), PREFIX, bus, false);
            }
            Module::EmberformDrive(_) => drive_workspace(ui, area, bus, skin, ducker_open),
            Module::EchoformDelay(_) => delay_workspace(ui, area, bus, bpm, skin, ducker_open),
            Module::AetherformReverb(_) => reverb_workspace(ui, area, bus, skin, ducker_open),
        });
    });
}

fn check_fx_tab_shortcut(ctx: &egui::Context, bus_index: &mut usize) {
    let alt = egui::Modifiers::ALT;
    let keys = [
        egui::Key::Num1,
        egui::Key::Num2,
        egui::Key::Num3,
        egui::Key::Num4,
    ];
    ctx.input_mut(|input| {
        for (idx, key) in keys.into_iter().enumerate() {
            if input.consume_key(alt, key) {
                *bus_index = idx;
                break;
            }
        }
    });
}

impl MixJamApp {
    fn commit_fx_draft(&mut self, draft: FxBuses, discrete: bool) {
        let cmd = EditCommand::SetEffects(draft);
        let result = if discrete {
            self.session.edit_discrete(cmd)
        } else {
            self.session.edit(cmd)
        };
        self.apply(result);
    }

    /// Floating Layer 2 FX deep edit workspace showing all parameters, live curves, and meters.
    pub(in super::super) fn fx_deep_edit_window(&mut self, ctx: &egui::Context) {
        let Some(mut bus_index) = self.fx_deep_edit else {
            return;
        };
        check_fx_tab_shortcut(ctx, &mut bus_index);
        let mut open = true;
        let buses = self.session.project().fx;
        let bpm = self.session.project().bpm;
        let mut draft = buses;
        let solos: [bool; BUSES] = std::array::from_fn(|index| self.session.fx_solo(index));
        let meters = self.boards.fx_meter_display.read();
        let generation = self.boards.master_view_generation;
        let mut clear_tail = false;
        let mut solo_toggled = false;
        let mut discrete = false;
        let ducker_open = ducker_is_expanded(ctx);
        let window_h = TABS_H
            + BAY_GAP
            + HEADER_H
            + BAY_GAP
            + fx_workspace_height(draft.buses[bus_index].module.is_empty(), ducker_open)
            + 12.0;

        let mut help = false;
        let mut bounds = ctx.viewport_rect().shrink(12.0);
        bounds.min.y = bounds.min.y.max(68.0);
        dialog::window(DEEP_EDIT_TITLE)
            .open(&mut open)
            .help(&mut help)
            .palette(controls::Palette::Marine)
            .default_width(WINDOW_W)
            .default_height(window_h)
            .fixed()
            .constrain_to(bounds)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = BAY_GAP;
                let ducks = draft.buses.map(|bus| bus.module.ducks());
                bus_tabs(ui, &mut bus_index, &draft.buses, &meters, ducks, solos);
                ui.push_id((generation, bus_index), |ui| {
                    let bus = &mut draft.buses[bus_index];
                    let (cleared, toggled, disc) = header_bar(ui, bus, solos[bus_index]);
                    clear_tail = cleared;
                    solo_toggled = toggled;
                    discrete = disc;
                    // The tabs and the effect selector above can change the
                    // bus on this frame, so its height follows them.
                    let workspace_h = fx_workspace_height(bus.module.is_empty(), ducker_open);
                    let area = ui
                        .allocate_exact_size(Vec2::new(WORKSPACE_W, workspace_h), Sense::hover())
                        .0;
                    draw_module_workspace(ui, area, bus_index, bus, bpm, ducker_open);
                });
            });

        self.fx_deep_edit = open.then_some(bus_index);
        if help {
            self.help.open_tool(help::Topic::FxReturns);
        }
        if clear_tail {
            self.session.clear_fx_tail(bus_index);
        }
        if solo_toggled {
            self.session.toggle_fx_solo(bus_index);
        }
        if draft != buses {
            self.commit_fx_draft(draft, discrete);
        }
    }
}

#[cfg(test)]
mod grid_tests {
    use super::grid_cell;

    #[test]
    fn a_short_last_row_centers_under_the_full_row() {
        let offsets: Vec<_> = (0..9).map(|i| grid_cell(i, 9, 5)).collect();
        assert_eq!(
            offsets,
            [
                (-2.0, 0),
                (-1.0, 0),
                (0.0, 0),
                (1.0, 0),
                (2.0, 0),
                (-1.5, 1),
                (-0.5, 1),
                (0.5, 1),
                (1.5, 1),
            ]
        );
    }

    #[test]
    fn full_rows_and_single_rows_center_on_the_bay() {
        let offsets: Vec<_> = (0..4).map(|i| grid_cell(i, 4, 2)).collect();
        assert_eq!(offsets, [(-0.5, 0), (0.5, 0), (-0.5, 1), (0.5, 1)]);
        let offsets: Vec<_> = (0..3).map(|i| grid_cell(i, 3, 4)).collect();
        assert_eq!(offsets, [(-1.0, 0), (0.0, 0), (1.0, 0)]);
    }
}
