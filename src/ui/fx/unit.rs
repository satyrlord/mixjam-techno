//! Layer 1 compact FX bus card and shared selector/switch helpers for Layer 2.
use super::super::mixer::{Look, bus_solo, bus_switches};
use super::faces::{Slot, dial_slot, macro_slots};
use super::master::{COMPACT_WELL, paint_motif_enclosure};
use super::*;
use super::{DIALS_Y, FOOTER_H, FOOTER_Y, MAX_CELL, PADDING, SELECT_Y};
use mixjam_techno::application::FxBusMeterReading;
use mixjam_techno::application::fx::{
    Aetherform, DelayParam, Division, Echoform, Emberform, FxBus, Module, TimeMode,
};

pub(super) enum CardAction {
    None,
    ClearTail,
    ToggleSolo,
    OpenDeepEdit,
}

pub(super) fn at(rect: Rect, x: f32, y: f32) -> Pos2 {
    rect.min + Vec2::new(x, y)
}

pub(super) fn bus_switch(
    ui: &mut egui::Ui,
    pos: Pos2,
    name: &str,
    label: &str,
    value: &mut bool,
    hint: &str,
    ink_on_light: bool,
) -> Response {
    controls::instrument::bat_toggle(
        ui,
        Rect::from_min_size(pos, Vec2::new(30.0, 22.0)),
        name,
        label,
        value,
        hint,
        ink_on_light,
    )
}

pub(super) fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    value: &mut T,
    options: &[(T, &str)],
    hint: &str,
) {
    choice_with_label(ui, rect, name, value, options, hint, |label| label);
}

struct ComboMeta<'a> {
    name: &'a str,
    current: &'a str,
    hint: &'a str,
}

fn show_styled_combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    rect: Rect,
    meta: ComboMeta<'_>,
    value: &mut T,
    options: &[(T, &str)],
    selected: egui::RichText,
) {
    let response = egui::ComboBox::from_id_salt(meta.name)
        .selected_text(selected)
        .width(rect.width())
        .height(320.0)
        .truncate()
        .show_ui(ui, |ui| {
            for (option, label) in options {
                if ui.selectable_label(option == value, *label).clicked() {
                    *value = *option;
                    ui.close();
                }
            }
        })
        .response;
    form::choice_named(&response, meta.name, meta.current);
    response.hover_hint(meta.hint).disabled_hint(meta.hint);
}

fn choice_with_label<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    value: &mut T,
    options: &[(T, &str)],
    hint: &str,
    display_label: fn(&str) -> &str,
) {
    let current = options
        .iter()
        .find(|(option, _)| option == value)
        .map_or("", |(_, label)| *label);
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        let selected = choice_style(ui, rect, display_label(current));
        show_styled_combo(
            ui,
            rect,
            ComboMeta {
                name,
                current,
                hint,
            },
            value,
            options,
            selected,
        );
    });
}

fn fit_division_font(ui: &egui::Ui, label: &str, max_width: f32) -> f32 {
    for size in [13.0_f32, 12.0, 11.5, 11.0, 10.0] {
        let w = ui.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(label.into(), FontId::proportional(size), Color32::WHITE)
                .size()
                .x
        });
        if w <= max_width {
            return size;
        }
    }
    10.0
}

fn division_choice_style(ui: &mut egui::Ui, rect: Rect, label: &str) -> egui::RichText {
    let pad_x = if rect.width() >= 56.0 {
        6.0
    } else if rect.width() >= 48.0 {
        4.0
    } else {
        2.0
    };
    let narrow = rect.width() < 52.0;
    let icon_w = if narrow { 7.0 } else { 9.0 };
    let icon_spacing = if narrow { 1.0 } else { 3.0 };
    let spacing = ui.spacing_mut();
    spacing.interact_size.y = rect.height();
    spacing.button_padding = Vec2::new(pad_x, 1.0);
    spacing.icon_width = icon_w;
    spacing.icon_spacing = icon_spacing;
    let avail = (rect.width() - 2.0 * pad_x - icon_w - icon_spacing - 2.0).max(16.0);
    let size = fit_division_font(ui, label, avail);
    if !ui.is_enabled() {
        ui.style_mut().visuals.override_text_color = Some(roles().legend.gamma_multiply(0.66));
    }
    egui::RichText::new(label).size(size)
}

/// Fits a choice's combo to `rect`: it keeps the height it is given, so it
/// lines up with its row. A narrow choice takes smaller lettering and a
/// smaller arrow; a note length in a dial's value row, the narrowest, takes
/// the smallest. Returns the lettering of the chosen `label`.
fn choice_style(ui: &mut egui::Ui, rect: Rect, label: &str) -> egui::RichText {
    ui.spacing_mut().interact_size.y = rect.height();
    let selected = egui::RichText::new(label);
    if rect.width() >= 120.0 {
        return selected;
    }
    let tight = rect.width() < 60.0;
    let spacing = ui.spacing_mut();
    spacing.button_padding.x = if tight { 2.0 } else { 3.0 };
    spacing.icon_width = if tight { 7.0 } else { 9.0 };
    spacing.icon_spacing = if tight { 1.0 } else { 2.0 };
    selected.size(if tight { 10.0 } else { 11.0 })
}

pub(super) fn module_and_preset_combos(
    ui: &mut egui::Ui,
    effect_rect: Rect,
    preset_rect: Rect,
    prefix: &str,
    bus: &mut FxBus,
    compact: bool,
) {
    let mut kind = module_kind(&bus.module);
    let before = kind;
    choice_with_label(
        ui,
        effect_rect,
        &format!("{prefix} effect"),
        &mut kind,
        &[
            (0, "Empty"),
            (1, "Echoform Delay"),
            (2, "Aetherform Reverb"),
            (3, "Emberform Drive"),
        ],
        "The effect on this bus. A new effect starts from its first preset; the old one's \
         tail is cut.",
        if compact {
            compact_effect_label
        } else {
            |label| label
        },
    );
    if kind != before {
        load_effect(bus, new_module(kind));
    }
    preset_combo(ui, preset_rect, prefix, bus);
}

/// The kind of `module` as the effect menu numbers it.
fn module_kind(module: &Module) -> u8 {
    match module {
        Module::Empty => 0,
        Module::EchoformDelay(_) => 1,
        Module::AetherformReverb(_) => 2,
        Module::EmberformDrive(_) => 3,
    }
}

/// The preset menu of the bus's effect, with "Custom" first while no preset
/// matches. An empty bus has no preset menu.
fn preset_combo(ui: &mut egui::Ui, preset_rect: Rect, prefix: &str, bus: &mut FxBus) {
    let presets = bus.module.presets();
    if presets.is_empty() {
        return;
    }
    let current = bus.matching_preset().unwrap_or("Custom");
    let mut chosen = current;
    let mut with_custom: Vec<(&str, &str)> =
        presets.iter().map(|(name, _)| (*name, *name)).collect();
    if current == "Custom" {
        with_custom.insert(0, ("Custom", "Custom"));
    }
    choice(
        ui,
        preset_rect,
        &format!("{prefix} preset"),
        &mut chosen,
        &with_custom,
        "Load a factory preset. It sets the effect controls and return level, and keeps the duck key.",
    );
    if chosen != current {
        bus.load_preset(chosen);
    }
}

/// A new effect of `kind`: 1 a delay, 2 a reverb, 3 a drive, else none.
fn new_module(kind: u8) -> Module {
    match kind {
        1 => Module::EchoformDelay(Echoform::default()),
        2 => Module::AetherformReverb(Aetherform::default()),
        3 => Module::EmberformDrive(Emberform::default()),
        _ => Module::Empty,
    }
}

/// Put `module` on `bus`, with the return level of its first preset.
fn load_effect(bus: &mut FxBus, module: Module) {
    bus.module = module;
    if let Some((_, mix)) = bus.module.presets().first() {
        bus.return_level = mix / 100.0;
    }
}

struct TileStyle<'a> {
    prefix: &'a str,
    title: &'a str,
    full: &'a str,
    subtitle: &'a str,
    wide: bool,
}

fn effect_tile_subtitle(kind: u8, compact: bool) -> &'static str {
    match (kind, compact) {
        (1, false) => "Tempo-synced echo",
        (1, true) => "Echo",
        (2, false) => "Lush room & shimmer",
        (2, true) => "Space",
        (_, false) => "Analog drive & crush",
        (_, true) => "Drive",
    }
}

fn paint_tile_body(ui: &egui::Ui, rect: Rect, style: &TileStyle<'_>, lamp: Color32) {
    let r = roles();
    let p = ui.painter();
    if style.wide {
        let cy = rect.center().y - 8.0;
        let title_w = ui.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(style.title.into(), FontId::proportional(12.5), r.legend)
                .size()
                .x
        });
        let start_x = rect.center().x - (13.0 + title_w) / 2.0;
        let lamp_at = Pos2::new(start_x + 4.0, cy);
        p.circle_filled(lamp_at, 4.5, Color32::from_black_alpha(140));
        p.circle_filled(lamp_at, 3.5, lamp);
        p.text(
            Pos2::new(start_x + 13.0, cy),
            Align2::LEFT_CENTER,
            style.title,
            FontId::proportional(12.5),
            r.legend,
        );
        p.text(
            Pos2::new(rect.center().x, cy + 19.0),
            Align2::CENTER_CENTER,
            style.subtitle,
            FontId::proportional(10.0),
            r.legend.gamma_multiply(0.72),
        );
    } else {
        let lamp_at = Pos2::new(rect.left() + 14.0, rect.center().y);
        p.circle_filled(lamp_at, 4.0, Color32::from_black_alpha(140));
        p.circle_filled(lamp_at, 3.0, lamp);
        p.text(
            Pos2::new(rect.left() + 24.0, rect.center().y),
            Align2::LEFT_CENTER,
            style.title,
            FontId::proportional(11.5),
            r.legend,
        );
        p.text(
            Pos2::new(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            style.subtitle,
            FontId::proportional(9.5),
            r.legend.gamma_multiply(0.62),
        );
    }
}

fn paint_effect_tile(
    ui: &mut egui::Ui,
    rect: Rect,
    style: &TileStyle<'_>,
    module: &Module,
) -> bool {
    let name = format!("{} {}", style.prefix, style.full);
    let id = ui.id().with(&name);
    let response = ui.interact(rect, id, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    let (motif, lamp) = super::fx_motif(module);
    let ([face, _], trim) = motif.deep_zones(true);
    let r = roles();
    let p = ui.painter();
    let lift = if response.is_pointer_button_down_on() {
        0.22
    } else if response.hovered() {
        0.12
    } else {
        0.0
    };
    let bg = face
        .lerp_to_gamma(r.strip_slot, 0.30)
        .lerp_to_gamma(trim, lift);
    let radius = 6.0_f32;
    p.rect_filled(rect, radius, bg);
    let bar = if style.wide {
        Rect::from_min_size(
            rect.min + Vec2::new(4.0, 2.0),
            Vec2::new(rect.width() - 8.0, 3.0),
        )
    } else {
        Rect::from_min_size(
            rect.min + Vec2::new(2.0, 4.0),
            Vec2::new(3.0, rect.height() - 8.0),
        )
    };
    p.rect_filled(bar, 1.5, trim);
    let (stroke_w, stroke_c) = if response.hovered() {
        (1.5_f32, trim)
    } else {
        (1.0_f32, trim.gamma_multiply(0.45))
    };
    p.rect_stroke(
        rect,
        radius,
        Stroke::new(stroke_w, stroke_c),
        egui::StrokeKind::Inside,
    );
    if response.has_focus() {
        p.rect_stroke(
            rect.expand(1.5),
            radius + 1.0,
            Stroke::new(2.0_f32, controls::focus_color(ui)),
            egui::StrokeKind::Outside,
        );
    }
    paint_tile_body(ui, rect, style, lamp);
    name_control(response, &name)
        .hover_hint(format!(
            "Load {} on this bus, from its first preset.",
            style.full
        ))
        .clicked()
}

/// Empty-bus module launcher: 3 wide module tiles where room allows, or 3 stacked strips.
pub(super) fn effect_keys(
    ui: &mut egui::Ui,
    area: Rect,
    prefix: &str,
    bus: &mut FxBus,
    compact_context: bool,
) {
    let wide = area.width() >= 220.0;
    let caption_h = 16.0_f32;
    let (origin, step, tile_size, caption_gap) = if wide {
        let gap = 8.0_f32;
        let tile_w = ((area.width() - 20.0 - 2.0 * gap) / 3.0).clamp(68.0, 196.0);
        let tile_h = if area.height() > 160.0 { 92.0 } else { 78.0 };
        let top = area.center().y - (caption_h + 10.0 + tile_h) / 2.0;
        let left = area.center().x - (3.0 * tile_w + 2.0 * gap) / 2.0;
        (
            Pos2::new(left, top + caption_h + 10.0),
            Vec2::new(tile_w + gap, 0.0),
            Vec2::new(tile_w, tile_h),
            10.0_f32,
        )
    } else {
        let gap = 6.0_f32;
        let tile_w = (area.width() - 12.0).clamp(112.0, 210.0);
        let tile_h = 28.0_f32;
        let top = area.center().y - (caption_h + 8.0 + 3.0 * tile_h + 2.0 * gap) / 2.0;
        let left = area.center().x - tile_w / 2.0;
        (
            Pos2::new(left, top + caption_h + 8.0),
            Vec2::new(0.0, tile_h + gap),
            Vec2::new(tile_w, tile_h),
            8.0_f32,
        )
    };
    ui.painter().text(
        Pos2::new(area.center().x, origin.y - caption_gap - caption_h / 2.0),
        Align2::CENTER_CENTER,
        "Choose an effect",
        FontId::proportional(11.0),
        roles().legend.gamma_multiply(0.78),
    );
    let long_title = wide && (tile_size.x >= 146.0 || !compact_context);
    for (index, kind) in [1_u8, 2, 3].into_iter().enumerate() {
        let module = new_module(kind);
        let full = module.label();
        let style = TileStyle {
            prefix,
            title: if long_title {
                full
            } else {
                compact_effect_label(full)
            },
            full,
            subtitle: effect_tile_subtitle(kind, !wide || tile_size.x < 120.0),
            wide,
        };
        let rect = Rect::from_min_size(origin + step * index as f32, tile_size);
        if paint_effect_tile(ui, rect, &style, &module) {
            load_effect(bus, module);
        }
    }
}

fn compact_effect_label(label: &str) -> &str {
    match label {
        "Echoform Delay" => "Delay",
        "Aetherform Reverb" => "Reverb",
        "Emberform Drive" => "Drive",
        _ => label,
    }
}

pub(super) fn note_division(ui: &mut egui::Ui, rect: Rect, name: &str, division: &mut Division) {
    let options = Division::ALL.map(|option| (option, option.label()));
    let current = options
        .iter()
        .find(|(option, _)| option == division)
        .map_or("", |(_, label)| *label);
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        let selected = division_choice_style(ui, rect, current);
        show_styled_combo(
            ui,
            rect,
            ComboMeta {
                name,
                current,
                hint: "Delay time as a note length at the song tempo.",
            },
            division,
            &options,
            selected,
        );
    });
}

pub(super) fn left_division(ui: &mut egui::Ui, rect: Rect, prefix: &str, delay: &mut Echoform) {
    let before = delay.division_l;
    note_division(
        ui,
        rect,
        &format!("{prefix} left division"),
        &mut delay.division_l,
    );
    if delay.link_times && delay.division_l != before {
        delay.division_r = delay.division_l;
    }
}

/// The header row: the bus name and its In switch at the left; S, Lim, and
/// Edit at the right. In, S, and Lim are the mixer's quiet bus switches, so
/// a bus reads the same on its return strip and here.
fn card_header(
    ui: &mut egui::Ui,
    rect: Rect,
    bus_name: &str,
    bus: &mut FxBus,
    solo: bool,
) -> CardAction {
    let mut action = CardAction::None;
    let switch = |x: f32, width: f32| Rect::from_min_size(at(rect, x, 8.0), Vec2::new(width, 22.0));
    let name = ui.painter().text(
        at(rect, PADDING, 19.0),
        Align2::LEFT_CENTER,
        bus_name,
        FontId::proportional(11.5),
        roles().legend,
    );
    let right = rect.width() - PADDING;
    // The key face sits 2 points inside the button and keeps 6 points
    // either side of its 13-point label.
    let edit_w = ui
        .fonts_mut(|fonts| {
            fonts
                .layout_no_wrap("Edit".into(), FontId::proportional(13.0), Color32::WHITE)
                .size()
                .x
        })
        .ceil()
        + 16.0;
    let input = switch(name.right() - rect.left() + 6.0, 28.0);
    let limiter = switch(right - edit_w - 4.0 - 32.0, 32.0);
    // An empty bus has nothing to feed or limit: its In and Lim stay dark.
    if bus.module.is_empty() {
        let mut idle = *bus;
        idle.powered = false;
        idle.limiter = false;
        ui.add_enabled_ui(false, |ui| {
            bus_switches(ui, [input, limiter], Look::Quiet, bus_name, &mut idle);
        });
    } else {
        bus_switches(ui, [input, limiter], Look::Quiet, bus_name, bus);
    }
    let mut solo_val = solo;
    if ui
        .add_enabled_ui(bus.is_active(), |ui| {
            bus_solo(
                ui,
                switch(limiter.left() - rect.left() - 4.0 - 24.0, 24.0),
                bus_name,
                &mut solo_val,
            )
        })
        .inner
    {
        action = CardAction::ToggleSolo;
    }
    let edit_rect = switch(right - edit_w, edit_w);
    let edit_name = format!("Deep edit {bus_name}");
    let edit_resp = controls::button_at(
        ui,
        edit_rect,
        &edit_name,
        "Edit",
        controls::ButtonRole::Secondary,
        !bus.module.is_empty(),
    );
    if name_control(edit_resp, &edit_name)
        .hover_hint("Open all parameters, scopes, and telemetry in FX deep edit.")
        .clicked()
    {
        action = CardAction::OpenDeepEdit;
    }
    ui.painter().line_segment(
        [
            at(rect, 10.0, super::HEADER + 1.0),
            at(rect, rect.width() - 10.0, super::HEADER + 1.0),
        ],
        Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.26)),
    );
    action
}

/// Paint one compact Layer 1 card.
pub(super) fn unit(
    ui: &mut egui::Ui,
    rect: Rect,
    index: usize,
    bus: &mut FxBus,
    meter: FxBusMeterReading,
    solo: bool,
) -> CardAction {
    let bus_name = format!("FX {}", index + 1);
    let sounding = bus.powered && !bus.module.is_empty();
    let (motif, _) = super::fx_motif(&bus.module);
    paint_motif_enclosure(ui, rect, motif, sounding, index as u64 + 21);
    let mut action = CardAction::None;
    ui.scope(|ui| {
        ui.visuals_mut().dark_mode = true;
        action = card_header(ui, rect, &bus_name, bus, solo);

        // The effect's short name needs less room than a preset's name.
        let inner_w = rect.width() - 2.0 * PADDING;
        let effect_w = ((inner_w - 6.0) * 0.34).round().clamp(62.0, 108.0);
        let preset_w = inner_w - 6.0 - effect_w;
        module_and_preset_combos(
            ui,
            Rect::from_min_size(at(rect, PADDING, SELECT_Y), Vec2::new(effect_w, 24.0)),
            Rect::from_min_size(
                at(rect, PADDING + effect_w + 6.0, SELECT_Y),
                Vec2::new(preset_w, 24.0),
            ),
            &bus_name,
            bus,
            true,
        );

        // An empty bus offers its effects in place of dials and meters.
        if bus.module.is_empty() {
            let slot_rect = Rect::from_min_size(
                at(rect, PADDING + effect_w + 6.0, SELECT_Y),
                Vec2::new(preset_w, 24.0),
            );
            let slot_text = if preset_w >= 112.0 {
                "Empty return slot"
            } else {
                "Empty"
            };
            ui.painter().text(
                Pos2::new(slot_rect.left() + 4.0, slot_rect.center().y),
                Align2::LEFT_CENTER,
                slot_text,
                FontId::proportional(11.0),
                roles().legend.gamma_multiply(0.58),
            );
            let area = Rect::from_min_max(
                at(rect, PADDING + 4.0, 76.0),
                at(rect, rect.width() - PADDING - 4.0, rect.height() - 16.0),
            );
            effect_keys(ui, area, &bus_name, bus, true);
            return;
        }
        paint_macro_dials(ui, rect, &bus_name, bus, motif.knob);

        let clear_w = 74.0;
        let meter_w = (inner_w - clear_w - 6.0).max(80.0);
        controls::instrument::meter_bar(
            ui,
            Rect::from_min_size(at(rect, PADDING, FOOTER_Y), Vec2::new(meter_w, FOOTER_H)),
            &format!("{bus_name} return meters"),
            meter,
            bus.module.ducks(),
        );
        let clear_rect = Rect::from_min_size(
            at(rect, rect.width() - PADDING - clear_w, FOOTER_Y),
            Vec2::new(clear_w, FOOTER_H),
        );
        let clear_label = format!("Clear {bus_name} tail");
        let clear_resp = controls::button_at(
            ui,
            clear_rect,
            &clear_label,
            "Clear tail",
            controls::ButtonRole::Secondary,
            bus.module.has_tail(),
        );
        if name_control(clear_resp, &clear_label)
            .hover_hint("Cut the effect's ringing tail now. Not an edit.")
            .disabled_hint("Only Delay and Reverb produce a ringing tail.")
            .clicked()
        {
            action = CardAction::ClearTail;
        }
    });
    action
}

fn paint_macro_dials(
    ui: &mut egui::Ui,
    rect: Rect,
    bus_name: &str,
    bus: &mut FxBus,
    knob: theme::KnobStyle,
) {
    let slots = macro_slots(&bus.module);
    let dial_span = rect.width() - 16.0;
    let cell = (dial_span / slots.len() as f32).min(MAX_CELL);
    let full = cell >= 62.0;
    let well = if full {
        (2.0 * cell - 8.0 - super::master::WELL.x).clamp(super::master::WELL.x, 64.0)
    } else {
        (cell - 2.0).min(COMPACT_WELL)
    };
    let compact = (!full).then_some(well);
    let left = (rect.width() - slots.len() as f32 * cell) / 2.0;
    theme::with_knob_style(knob, || {
        for (i, slot) in slots.iter().enumerate() {
            let x = left + (i as f32 + 0.5) * cell;
            let y = DIALS_Y + 26.0;
            let positions = [at(rect, x, y), at(rect, x, y + 40.0), at(rect, x, y + 61.0)];
            if let (Slot::Delay(DelayParam::TimeL), Module::EchoformDelay(delay)) =
                (slot, &mut bus.module)
                && delay.mode == TimeMode::Sync
            {
                let label = if delay.link_times { "Time" } else { "Time L" };
                synced_time_dial(
                    ui,
                    SyncedTime {
                        positions,
                        well,
                        full,
                        side: Side::Left,
                        label,
                    },
                    bus_name,
                    delay,
                );
            } else {
                dial_slot(ui, positions, bus_name, bus, slot, compact);
            }
        }
    });
}

/// A delay channel.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    Left,
    Right,
}

impl Side {
    fn word(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// Where a synced time dial sits and what it shows.
pub(super) struct SyncedTime<'a> {
    pub(super) positions: [Pos2; 3],
    pub(super) well: f32,
    pub(super) full: bool,
    pub(super) side: Side,
    pub(super) label: &'a str,
}

fn division_reset_index(side: Side, fallback: usize) -> usize {
    let default_div = match side {
        Side::Left => Echoform::default().division_l,
        Side::Right => Echoform::default().division_r,
    };
    Division::ALL
        .iter()
        .position(|candidate| *candidate == default_div)
        .unwrap_or(fallback)
}

fn division_menu_rect(center: Pos2, well: f32, full: bool) -> Rect {
    let menu_w = if full { well } else { well - 2.0 };
    Rect::from_center_size(center, Vec2::new(menu_w, super::master::WELL.y))
}

/// One channel's time in Sync mode: a dial that steps through the note
/// lengths, over the note length's list in the value row. The left side
/// carries the right one along while the times are linked.
pub(super) fn synced_time_dial(
    ui: &mut egui::Ui,
    at: SyncedTime<'_>,
    bus: &str,
    delay: &mut Echoform,
) {
    let SyncedTime {
        positions,
        well,
        full,
        side,
        label: text,
    } = at;
    let [center, value, label] = positions;
    let division = match side {
        Side::Left => delay.division_l,
        Side::Right => delay.division_r,
    };
    let steps: [f64; Division::ALL.len()] = std::array::from_fn(|index| index as f64);
    let current = Division::ALL
        .iter()
        .position(|candidate| *candidate == division)
        .unwrap_or(0);
    let mut step = current as f64;
    let name = format!("{bus} {} division dial", side.word());
    let response = controls::rotary(
        ui,
        Rect::from_center_size(center, Vec2::splat(44.0)),
        &mut step,
        controls::RotarySpec {
            name: &name,
            low: 0.0,
            high: steps[steps.len() - 1],
            reset: division_reset_index(side, current) as f64,
            step: 1.0,
            decimals: 0,
            logarithmic: false,
            choices: &steps,
        },
    );
    let hint = "Turn to step the note length. Double-click to reset.";
    name_control(response, &name)
        .hover_hint(hint)
        .disabled_hint("Unlink L/R to adjust the right channel independently.");
    let chosen = Division::ALL[(step.round() as usize).min(steps.len() - 1)];
    let menu = division_menu_rect(value, well, full);
    set_division(side, chosen, delay);
    match side {
        Side::Left => left_division(ui, menu, bus, delay),
        Side::Right => note_division(
            ui,
            menu,
            &format!("{bus} right division"),
            &mut delay.division_r,
        ),
    }
    let ink = if ui.is_enabled() {
        roles().legend
    } else {
        roles().legend.gamma_multiply(0.66)
    };
    ui.painter().text(
        label,
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(if full { 11.5 } else { 10.0 }),
        ink,
    );
}

/// Set one channel's note length from its dial. The left side carries the
/// right one along while the times are linked.
fn set_division(side: Side, chosen: Division, delay: &mut Echoform) {
    match side {
        Side::Left if chosen != delay.division_l => {
            delay.division_l = chosen;
            if delay.link_times {
                delay.division_r = chosen;
            }
        }
        Side::Left => {}
        Side::Right => delay.division_r = chosen,
    }
}
