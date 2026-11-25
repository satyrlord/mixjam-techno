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

/// The header row: the bus name and its In switch at the left; S, Lim, and
/// Edit at the right. In, S, and Lim are the mixer's quiet bus switches, so
/// a bus reads the same on its return strip, in the Mix view, and here.
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
    bus_switches(ui, [input, limiter], Look::Quiet, bus_name, bus);
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
        true,
    );
    if name_control(edit_resp, &edit_name)
        .hover_hint("Open all parameters, scopes, and telemetry in FX Deep Edit.")
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

        paint_macro_dials(ui, rect, &bus_name, bus, motif.knob);

        let clear_w = 74.0;
        let meter_w = if bus.module.is_empty() {
            inner_w
        } else {
            (inner_w - clear_w - 6.0).max(80.0)
        };
        controls::instrument::meter_bar(
            ui,
            Rect::from_min_size(at(rect, PADDING, FOOTER_Y), Vec2::new(meter_w, FOOTER_H)),
            &format!("{bus_name} return meters"),
            meter,
            bus.module.ducks(),
        );
        if !bus.module.is_empty() {
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
        super::master::WELL.x
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
                synced_time_macro(ui, positions, well, full, bus_name, delay);
            } else {
                dial_slot(ui, positions, bus_name, bus, slot, compact);
            }
        }
    });
}

/// The left time in Sync mode: a dial that steps through the note lengths,
/// over the note length's list in the value row.
fn synced_time_macro(
    ui: &mut egui::Ui,
    positions: [Pos2; 3],
    well: f32,
    full: bool,
    bus: &str,
    delay: &mut Echoform,
) {
    let [center, value, label] = positions;
    let steps: [f64; Division::ALL.len()] = std::array::from_fn(|index| index as f64);
    let current = Division::ALL
        .iter()
        .position(|division| *division == delay.division_l)
        .unwrap_or(0);
    let mut step = current as f64;
    let name = format!("{bus} left division dial");
    let response = controls::rotary(
        ui,
        Rect::from_center_size(center, Vec2::splat(44.0)),
        &mut step,
        controls::RotarySpec {
            name: &name,
            low: 0.0,
            high: steps[steps.len() - 1],
            reset: current as f64,
            step: 1.0,
            decimals: 0,
            logarithmic: false,
            choices: &steps,
        },
    );
    name_control(response, &name).hover_hint("Turn to step the note length.");
    let chosen = Division::ALL[(step.round() as usize).min(steps.len() - 1)];
    if chosen != delay.division_l {
        delay.division_l = chosen;
        if delay.link_times {
            delay.division_r = chosen;
        }
    }
    left_division(
        ui,
        Rect::from_center_size(value, Vec2::new(well - 2.0, 24.0)),
        bus,
        delay,
    );
    ui.painter().text(
        label,
        Align2::CENTER_CENTER,
        if delay.link_times { "Time" } else { "Time L" },
        FontId::proportional(if full { 11.5 } else { 10.0 }),
        roles().legend,
    );
}
