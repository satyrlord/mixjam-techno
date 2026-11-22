//! Layer 1 compact FX bus card and shared selector/switch helpers for Layer 2.
use super::faces::{Slot, dial_slot, macro_slots};
use super::master::{enamel_of, enclosure_of};
use super::*;
use super::{CARD_CELL, DIALS_Y, FOOTER_Y, PADDING, SELECT_Y, UNIT_HEIGHT, enamel};
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
        let mut selected = egui::RichText::new(display_label(current));
        if rect.width() < 120.0 {
            let spacing = ui.spacing_mut();
            spacing.button_padding.x = 3.0;
            spacing.icon_width = 9.0;
            spacing.icon_spacing = 2.0;
            selected = selected.size(11.0);
        }
        let response = egui::ComboBox::from_id_salt(name)
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
        form::choice_named(&response, name, current);
        response.hover_hint(hint);
    });
}

pub(super) fn module_and_preset_combos(
    ui: &mut egui::Ui,
    effect_rect: Rect,
    preset_rect: Rect,
    prefix: &str,
    bus: &mut FxBus,
    compact: bool,
) {
    let mut kind = match bus.module {
        Module::Empty => 0,
        Module::EchoformDelay(_) => 1,
        Module::AetherformReverb(_) => 2,
        Module::EmberformDrive(_) => 3,
    };
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
        bus.module = match kind {
            1 => Module::EchoformDelay(Echoform::default()),
            2 => Module::AetherformReverb(Aetherform::default()),
            3 => Module::EmberformDrive(Emberform::default()),
            _ => Module::Empty,
        };
        if let Some((_, mix)) = bus.module.presets().first() {
            bus.return_level = mix / 100.0;
        }
    }
    let presets = bus.module.presets();
    if !presets.is_empty() {
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
    choice(
        ui,
        rect,
        name,
        division,
        &Division::ALL.map(|division| (division, division.label())),
        "Delay time as a note length at the song tempo.",
    );
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

fn card_header(
    ui: &mut egui::Ui,
    rect: Rect,
    bus_name: &str,
    bus: &mut FxBus,
    solo: bool,
    sounding: bool,
    tail_active: bool,
) -> CardAction {
    let mut action = CardAction::None;
    bus_switch(
        ui,
        at(rect, 6.0, 7.0),
        &format!("{bus_name} power"),
        "",
        &mut bus.powered,
        "Off stops new sound into the effect; its tail rings out.",
        true,
    );
    let lamp_color = if sounding && tail_active {
        roles().lamp_on
    } else if sounding {
        roles().lamp_good
    } else {
        roles().lamp_off
    };
    controls::instrument::jewel_lamp(ui, at(rect, 39.0, 17.0), 4.2, sounding, lamp_color);

    ui.painter().text(
        at(rect, 47.0, 17.0),
        Align2::LEFT_CENTER,
        bus_name,
        FontId::proportional(11.5),
        roles().ink,
    );
    let right = rect.width() - PADDING;
    let mut solo_val = solo;
    if ui
        .add_enabled_ui(bus.is_active(), |ui| {
            bus_switch(
                ui,
                at(rect, right - 118.0, 7.0),
                &format!("{bus_name} solo"),
                "S",
                &mut solo_val,
                "Monitor only soloed FX returns during playback.",
                true,
            )
        })
        .inner
        .changed()
    {
        action = CardAction::ToggleSolo;
    }
    bus_switch(
        ui,
        at(rect, right - 82.0, 7.0),
        &format!("{bus_name} limiter"),
        "L",
        &mut bus.limiter,
        "Limits return peaks to -1 dBFS during playback and -1 dBTP in exports.",
        true,
    );
    let edit_rect = Rect::from_min_size(at(rect, right - 40.0, 6.0), Vec2::new(40.0, 22.0));
    let edit_name = format!("Deep edit {bus_name}");
    let edit_resp = controls::button_at(
        ui,
        edit_rect,
        &edit_name,
        "Edit",
        controls::ButtonRole::Secondary,
        true,
    );
    if name_control(edit_resp, &edit_name)
        .hover_hint("Open all parameters, scopes, and telemetry in FX Deep Edit.")
        .clicked()
    {
        action = CardAction::OpenDeepEdit;
    }
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
    enclosure_of(
        ui,
        rect,
        enamel_of(enamel(&bus.module), sounding),
        sounding,
        index as u64 + 21,
    );
    let mut action = CardAction::None;
    ui.scope(|ui| {
        ui.visuals_mut().dark_mode = false;
        action = card_header(ui, rect, &bus_name, bus, solo, sounding, meter.tail_active);

        let inner_w = rect.width() - 2.0 * PADDING;
        let effect_w = ((inner_w - 6.0) * 0.56).max(80.0);
        let preset_w = (inner_w - 6.0 - effect_w).max(80.0);
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

        let slots = macro_slots(&bus.module);
        let left = (rect.width() - slots.len() as f32 * CARD_CELL) / 2.0;
        for (i, slot) in slots.iter().enumerate() {
            let x = left + (i as f32 + 0.5) * CARD_CELL;
            let y = DIALS_Y + 20.0;
            let positions = [at(rect, x, y), at(rect, x, y + 36.0), at(rect, x, y + 58.0)];
            if let (Slot::Delay(DelayParam::TimeL), Module::EchoformDelay(delay)) =
                (slot, &mut bus.module)
                && delay.mode == TimeMode::Sync
            {
                synced_time_macro(ui, positions, &bus_name, delay);
            } else {
                dial_slot(ui, positions, &bus_name, bus, slot, true);
            }
        }

        let footer_y = FOOTER_Y.min(UNIT_HEIGHT - 44.0);
        let clear_w = 76.0;
        let meter_w = if bus.module.is_empty() {
            inner_w
        } else {
            (inner_w - clear_w - 6.0).max(80.0)
        };
        controls::instrument::meter_bar(
            ui,
            Rect::from_min_size(at(rect, PADDING, footer_y), Vec2::new(meter_w, 24.0)),
            &format!("{bus_name} return meters"),
            meter,
        );
        if !bus.module.is_empty() {
            let clear_rect = Rect::from_min_size(
                at(rect, rect.width() - PADDING - clear_w, footer_y),
                Vec2::new(clear_w, 24.0),
            );
            let clear_label = format!("Clear {bus_name} tail");
            let clear_resp = controls::button_at(
                ui,
                clear_rect,
                &clear_label,
                "Clear tail",
                controls::ButtonRole::Secondary,
                true,
            );
            if name_control(clear_resp, &clear_label)
                .hover_hint("Cut the effect's ringing tail now. Not an edit.")
                .clicked()
            {
                action = CardAction::ClearTail;
            }
        }
    });
    action
}

fn synced_time_macro(ui: &mut egui::Ui, positions: [Pos2; 3], bus: &str, delay: &mut Echoform) {
    let [center, _, label] = positions;
    left_division(
        ui,
        Rect::from_center_size(center, Vec2::new(CARD_CELL - 2.0, 24.0)),
        bus,
        delay,
    );
    ui.painter().text(
        label,
        Align2::CENTER_CENTER,
        if delay.link_times { "Time" } else { "Time L" },
        FontId::proportional(10.0),
        roles().ink,
    );
}
