//! One unit of the FX returns board: its selectors, the effect's switches,
//! its dials, and the bus's own switches.
use super::faces::{Face, Slot, columns, delay_face, drive_face, knob, reverb_face, slots};
use super::master::{Dial, Format, dial, enamel_of, enclosure_of, heading, lamp};
use super::*;
use mixjam_techno::application::KnobRange;
use mixjam_techno::application::fx::{
    Aetherform, DelayCharacter, DelayParam, DistortionParam, DistortionShape, Division, Echoform,
    Emberform, FxBus, Module, Range, ReverbCharacter, ReverbParam, SHIMMER_INTERVALS, SpaceModel,
    TimeMode,
};

/// A switch on the enamel with its label printed beside it. Returns the
/// switch's response.
fn switch(
    ui: &mut egui::Ui,
    at: Pos2,
    name: &str,
    label: &str,
    value: &mut bool,
    hint: &str,
) -> Response {
    let rect = Rect::from_min_size(at, Vec2::new(30.0, 24.0));
    let response = controls::toggle_at(ui, rect, name, "", value);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, response.enabled(), *value, name)
    });
    ui.painter().text(
        Pos2::new(rect.right() + 2.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.5),
        roles().ink,
    );
    response.hover_hint(format!(
        "{name}: {}. {hint}",
        if *value { "on" } else { "off" }
    ))
}

/// A list of choices at `rect`, named for assistive technology.
fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    value: &mut T,
    options: &[(T, &str)],
    hint: &str,
) {
    let current = options
        .iter()
        .find(|(option, _)| option == value)
        .map_or("", |(_, label)| *label);
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        let response = egui::ComboBox::from_id_salt(name)
            .selected_text(current)
            .width(rect.width())
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
        name_control(response, name).hover_hint(hint);
    });
}

/// One dial of a unit, from its lettering `face` and the setting's `range`
/// and `current` value. The new value when the person changed it.
fn face_dial(
    ui: &mut egui::Ui,
    positions: [Pos2; 3],
    bus_name: &str,
    face: Face,
    range: Range,
    current: f32,
    enabled: bool,
) -> Option<f32> {
    let mut value = f64::from(current);
    let before = value;
    dial(
        ui,
        positions,
        &mut value,
        Dial {
            name: &format!("{bus_name} {}", face.name),
            label: face.label,
            hint: face.hint,
            range: knob(range),
            format: face.format,
            choices: &[],
        },
        enabled,
    );
    (value != before).then_some(value as f32)
}

/// A point of the unit at `x` and `y` from its top left corner.
fn at(rect: Rect, x: f32, y: f32) -> Pos2 {
    rect.min + Vec2::new(x, y)
}

/// The selectors of the effect and of its preset.
fn selectors(ui: &mut egui::Ui, rect: Rect, bus_name: &str, bus: &mut FxBus) {
    let mut kind = match bus.module {
        Module::Empty => 0,
        Module::EchoformDelay(_) => 1,
        Module::AetherformReverb(_) => 2,
        Module::EmberformDrive(_) => 3,
    };
    let before = kind;
    choice(
        ui,
        Rect::from_min_size(at(rect, PADDING, SELECT_Y), Vec2::new(176.0, 26.0)),
        &format!("{bus_name} effect"),
        &mut kind,
        &[
            (0, "Empty"),
            (1, "Echoform Delay"),
            (2, "Aetherform Reverb"),
            (3, "Emberform Drive"),
        ],
        "The effect on this bus. A new effect starts from its first preset; the old one's \
         tail is cut.",
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
            Rect::from_min_size(at(rect, PADDING + 186.0, SELECT_Y), Vec2::new(170.0, 26.0)),
            &format!("{bus_name} preset"),
            &mut chosen,
            &with_custom,
            "Load a factory preset: it sets every control but the duck key, \
             and the return level.",
        );
        if chosen != current {
            bus.load_preset(chosen);
        }
    }
}

/// The effect's own switches and choices.
fn effect_row(ui: &mut egui::Ui, rect: Rect, bus_name: &str, bus: &mut FxBus) {
    match &mut bus.module {
        Module::Empty => {
            ui.painter().text(
                at(rect, rect.width() / 2.0, DIALS_Y + ROW),
                Align2::CENTER_CENTER,
                "Choose an effect for this bus.",
                FontId::proportional(13.0),
                roles().ink,
            );
        }
        Module::EchoformDelay(delay) => {
            let mut sync = delay.mode == TimeMode::Sync;
            switch(
                ui,
                at(rect, PADDING, MODE_Y),
                &format!("{bus_name} sync"),
                "Sync",
                &mut sync,
                "On, the times are note lengths at the song's tempo; off, the Time dials set \
                 them in milliseconds.",
            );
            delay.mode = if sync { TimeMode::Sync } else { TimeMode::Free };
            let divisions: Vec<(Division, &str)> = Division::ALL
                .iter()
                .map(|division| (*division, division.label()))
                .collect();
            ui.add_enabled_ui(sync, |ui| {
                choice(
                    ui,
                    Rect::from_min_size(at(rect, PADDING + 76.0, MODE_Y), Vec2::new(66.0, 24.0)),
                    &format!("{bus_name} left division"),
                    &mut delay.division_l,
                    &divisions,
                    "The left delay as a note length: dotted lasts half as long again, \
                     triplet two thirds.",
                );
                choice(
                    ui,
                    Rect::from_min_size(at(rect, PADDING + 150.0, MODE_Y), Vec2::new(66.0, 24.0)),
                    &format!("{bus_name} right division"),
                    &mut delay.division_r,
                    &divisions,
                    "The right delay as a note length.",
                );
            });
            switch(
                ui,
                at(rect, PADDING + 228.0, MODE_Y),
                &format!("{bus_name} ping-pong"),
                "Ping-pong",
                &mut delay.ping_pong,
                "Repeats cross from side to side.",
            );
            choice(
                ui,
                Rect::from_min_size(at(rect, PADDING + 324.0, MODE_Y), Vec2::new(92.0, 24.0)),
                &format!("{bus_name} character"),
                &mut delay.character,
                &[
                    (DelayCharacter::Digital, "Digital"),
                    (DelayCharacter::Analog, "Analog"),
                    (DelayCharacter::Tape, "Tape"),
                ],
                "Digital repeats stay clean; Analog warms and softens them; Tape saturates, \
                 darkens, and wavers.",
            );
        }
        Module::AetherformReverb(reverb) => {
            choice(
                ui,
                Rect::from_min_size(at(rect, PADDING, MODE_Y), Vec2::new(112.0, 24.0)),
                &format!("{bus_name} space"),
                &mut reverb.space_model,
                &SpaceModel::ALL.map(|model| (model, model.label())),
                "The shape of the space: its reflections and delay lengths.",
            );
            choice(
                ui,
                Rect::from_min_size(at(rect, PADDING + 120.0, MODE_Y), Vec2::new(112.0, 24.0)),
                &format!("{bus_name} character"),
                &mut reverb.character,
                &[
                    (ReverbCharacter::Natural, "Natural"),
                    (ReverbCharacter::Vintage, "Vintage"),
                    (ReverbCharacter::Bloom, "Bloom"),
                ],
                "Natural is neutral; Vintage saturates and darkens the tail; Bloom swells in \
                 slowly and opens up.",
            );
            switch(
                ui,
                at(rect, PADDING + 242.0, MODE_Y),
                &format!("{bus_name} early reflections"),
                "Early",
                &mut reverb.early_reflections_enabled,
                "The first discrete reflections of the space.",
            );
            switch(
                ui,
                at(rect, PADDING + 322.0, MODE_Y),
                &format!("{bus_name} shimmer"),
                "Shimmer",
                &mut reverb.shimmer_enabled,
                "Feeds the tail back pitched up by the interval.",
            );
            let intervals: Vec<(u8, String)> = SHIMMER_INTERVALS
                .iter()
                .map(|semitones| (*semitones, format!("+{semitones}")))
                .collect();
            let labels: Vec<(u8, &str)> = intervals
                .iter()
                .map(|(value, label)| (*value, label.as_str()))
                .collect();
            ui.add_enabled_ui(reverb.shimmer_enabled, |ui| {
                choice(
                    ui,
                    Rect::from_min_size(at(rect, PADDING + 418.0, MODE_Y), Vec2::new(76.0, 24.0)),
                    &format!("{bus_name} shimmer interval"),
                    &mut reverb.shimmer_interval_semitones,
                    &labels,
                    "The shimmer's pitch step in semitones: a fifth, an octave, an octave and \
                     a fifth, or two octaves.",
                );
            });
        }
        Module::EmberformDrive(drive) => {
            choice(
                ui,
                Rect::from_min_size(at(rect, PADDING, MODE_Y), Vec2::new(132.0, 24.0)),
                &format!("{bus_name} type"),
                &mut drive.shape,
                &DistortionShape::ALL.map(|shape| (shape, shape.label())),
                "The curve: Tube and Tape are warm, Overdrive crunches, Fuzz buzzes, Hard \
                 clip cuts, Foldback rings, Rectifier adds the octave, Bitcrush steps.",
            );
        }
    }
}

/// The dials, in two rows.
fn dial_grid(ui: &mut egui::Ui, rect: Rect, bus_name: &str, bus: &mut FxBus) {
    let all = slots(&bus.module);
    let per_row = columns(&bus.module).max(1);
    for (slot_index, slot) in all.iter().enumerate() {
        let (column, row) = (slot_index % per_row, slot_index / per_row);
        let in_row = if row == 0 {
            per_row.min(all.len())
        } else {
            all.len() - per_row
        };
        let left = (rect.width() - in_row as f32 * CELL) / 2.0;
        let x = left + (column as f32 + 0.5) * CELL;
        let y = DIALS_Y + 22.0 + row as f32 * ROW;
        let positions = [at(rect, x, y), at(rect, x, y + 38.0), at(rect, x, y + 60.0)];
        match slot {
            Slot::Return => {
                let mut value = f64::from((bus.return_level * 100.0).round());
                let before = value;
                dial(
                    ui,
                    positions,
                    &mut value,
                    Dial {
                        name: &format!("{bus_name} return level"),
                        label: "Return",
                        hint: "The level of the effect's sound in the mix.",
                        range: KnobRange {
                            low: 0.0,
                            high: 100.0,
                            default: 100.0,
                            step: 1.0,
                            logarithmic: false,
                        },
                        format: Format::whole("%"),
                        choices: &[],
                    },
                    true,
                );
                if value != before {
                    bus.return_level = (value / 100.0) as f32;
                }
            }
            Slot::Delay(param) => {
                let Module::EchoformDelay(delay) = &mut bus.module else {
                    continue;
                };
                let enabled = !matches!(param, DelayParam::TimeL | DelayParam::TimeR)
                    || delay.mode == TimeMode::Free;
                let face = delay_face(*param);
                let current = param.get(delay);
                if let Some(value) = face_dial(
                    ui,
                    positions,
                    bus_name,
                    face,
                    param.range(),
                    current,
                    enabled,
                ) {
                    param.set(delay, value);
                }
            }
            Slot::Reverb(param) => {
                let Module::AetherformReverb(reverb) = &mut bus.module else {
                    continue;
                };
                let enabled = *param != ReverbParam::ShimmerAmount || reverb.shimmer_enabled;
                let face = reverb_face(*param);
                let current = param.get(reverb);
                if let Some(value) = face_dial(
                    ui,
                    positions,
                    bus_name,
                    face,
                    param.range(),
                    current,
                    enabled,
                ) {
                    param.set(reverb, value);
                }
            }
            Slot::Drive(param) => {
                let Module::EmberformDrive(drive) = &mut bus.module else {
                    continue;
                };
                let enabled = !matches!(param, DistortionParam::Bits | DistortionParam::CrushRate)
                    || drive.shape == DistortionShape::Bitcrush;
                let face = drive_face(*param);
                let current = param.get(drive);
                if let Some(value) = face_dial(
                    ui,
                    positions,
                    bus_name,
                    face,
                    param.range(),
                    current,
                    enabled,
                ) {
                    param.set(drive, value);
                }
            }
        }
    }
}

/// The bus's own switches, and Clear Tail. True when Clear Tail was pressed.
fn footer(ui: &mut egui::Ui, rect: Rect, bus_name: &str, bus: &mut FxBus) -> bool {
    let mut cleared = false;
    let footer = FOOTER_Y + 14.0;
    switch(
        ui,
        at(rect, PADDING, footer),
        &format!("{bus_name} power"),
        "Input",
        &mut bus.powered,
        "Off stops new sound into the effect; its tail rings out.",
    );
    switch(
        ui,
        at(rect, PADDING + 80.0, footer),
        &format!("{bus_name} limiter"),
        "Limiter",
        &mut bus.limiter,
        "Holds the return under -1 dBFS.",
    );
    let bypass = match &mut bus.module {
        Module::EchoformDelay(delay) => Some(&mut delay.bypass),
        Module::AetherformReverb(reverb) => Some(&mut reverb.bypass),
        Module::EmberformDrive(drive) => Some(&mut drive.bypass),
        Module::Empty => None,
    };
    if let Some(bypass) = bypass {
        switch(
            ui,
            at(rect, PADDING + 172.0, footer),
            &format!("{bus_name} bypass"),
            "Bypass",
            bypass,
            "Silences the effect while it keeps running, so its tail returns when Bypass goes \
             off.",
        );
        let button = Rect::from_min_size(
            at(rect, rect.width() - PADDING - 96.0, footer),
            Vec2::new(96.0, 26.0),
        );
        let label = format!("Clear {bus_name} tail");
        let response = controls::button_at(
            ui,
            button,
            &label,
            "Clear tail",
            controls::ButtonRole::Secondary,
            true,
        );
        if name_control(response, &label)
            .hover_hint(
                "Cut the effect's ringing tail now. This is not an edit and cannot be undone.",
            )
            .clicked()
        {
            cleared = true;
        }
    }
    cleared
}

/// One bus's unit. True when Clear Tail was pressed.
pub(super) fn unit(ui: &mut egui::Ui, rect: Rect, index: usize, bus: &mut FxBus) -> bool {
    let bus_name = format!("FX {}", index + 1);
    let sounding = bus.powered && !bus.module.is_empty();
    enclosure_of(
        ui,
        rect,
        enamel_of(enamel(&bus.module), sounding),
        sounding,
        index as u64 + 21,
    );
    heading(ui, rect, &bus_name, bus.module.label(), roles().ink);
    lamp(ui, at(rect, rect.width() - 18.0, 19.0), sounding);
    let mut cleared = false;
    ui.scope(|ui| {
        ui.visuals_mut().dark_mode = false;
        selectors(ui, rect, &bus_name, bus);
        effect_row(ui, rect, &bus_name, bus);
        dial_grid(ui, rect, &bus_name, bus);
        cleared = footer(ui, rect, &bus_name, bus);
    });
    cleared
}
