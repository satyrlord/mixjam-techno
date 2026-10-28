//! Effects module of the bottom section: the four FX buses that the mixer's
//! sends feed, side by side as units on a board. Each unit holds its effect,
//! the effect's preset and settings, the bus's input switch and return level,
//! its safety limiter, and Clear Tail. The caller turns changed buses into an
//! edit and a Clear Tail request into a session call.
use super::controls::INK;
use super::master::{Dial, Format, dial, enamel_of, enclosure_of, heading, lamp};
use super::*;
use mixjam_techno::application::KnobRange;
use mixjam_techno::application::fx::{
    Aetherform, BUSES, DelayCharacter, DelayParam, Division, Echoform, FxBus, FxBuses, Module,
    Range, ReverbCharacter, ReverbParam, SHIMMER_INTERVALS, SpaceModel, TimeMode,
};

pub(super) enum Action {
    None,
    Change(Box<FxBuses>),
    ClearTail(usize),
}

const MARGIN: f32 = 8.0;
const GAP: f32 = 14.0;
const UNIT_TOP: f32 = 10.0;
const UNIT_HEIGHT: f32 = 364.0;
const HEADER: f32 = 36.0;
/// One dial with its value well and label, and the pitch of dial rows.
const CELL: f32 = 62.0;
const ROW: f32 = 96.0;
const PADDING: f32 = 14.0;
/// The effect and preset selectors, then the effect's switches, then two
/// rows of dials, then the bus's switches.
const SELECT_Y: f32 = HEADER + 8.0;
const MODE_Y: f32 = SELECT_Y + 34.0;
const DIALS_Y: f32 = MODE_Y + 36.0;
const FOOTER_Y: f32 = DIALS_Y + 2.0 * ROW;
/// Room under the units for their shadows and the row's scroll bar.
const ROW_HEIGHT: f32 = UNIT_TOP + UNIT_HEIGHT + 20.0;
const BOARD_INSET: f32 = 2.0;
/// The lower panel's height that shows the whole board.
pub(super) const MODULE_HEIGHT: f32 = ROW_HEIGHT + 2.0 * BOARD_INSET + 64.0;

const DELAY_ENAMEL: [u8; 3] = [112, 192, 214];
const REVERB_ENAMEL: [u8; 3] = [184, 168, 234];
const EMPTY_ENAMEL: [u8; 3] = [196, 194, 186];

fn enamel(module: &Module) -> [u8; 3] {
    match module {
        Module::Empty => EMPTY_ENAMEL,
        Module::EchoformDelay(_) => DELAY_ENAMEL,
        Module::AetherformReverb(_) => REVERB_ENAMEL,
    }
}

/// The dials a unit shows: the return level first, then the effect's own.
enum Slot {
    Return,
    Delay(DelayParam),
    Reverb(ReverbParam),
}

fn slots(module: &Module) -> Vec<Slot> {
    match module {
        Module::Empty => Vec::new(),
        Module::EchoformDelay(_) => std::iter::once(Slot::Return)
            .chain(DelayParam::ALL.into_iter().map(Slot::Delay))
            .collect(),
        Module::AetherformReverb(_) => std::iter::once(Slot::Return)
            .chain(
                [
                    ReverbParam::PreDelay,
                    ReverbParam::Decay,
                    ReverbParam::Size,
                    ReverbParam::LateBalance,
                    ReverbParam::Diffusion,
                    ReverbParam::Density,
                    ReverbParam::LowCut,
                    ReverbParam::HighCut,
                    ReverbParam::Width,
                    ReverbParam::Drive,
                    ReverbParam::ModRate,
                    ReverbParam::ModDepth,
                    ReverbParam::ShimmerAmount,
                    ReverbParam::DuckAmount,
                    ReverbParam::DuckRelease,
                    ReverbParam::Output,
                ]
                .into_iter()
                .map(Slot::Reverb),
            )
            .collect(),
    }
}

/// Dials in a row: two rows hold them all.
fn columns(module: &Module) -> usize {
    slots(module).len().div_ceil(2)
}

fn unit_width(module: &Module) -> f32 {
    (columns(module) as f32 * CELL + 2.0 * PADDING).max(320.0)
}

fn knob(range: Range) -> KnobRange {
    KnobRange {
        low: range.low,
        high: range.high,
        default: range.default,
        step: range.step,
        logarithmic: range.log,
    }
}

const fn in_unit(unit: &'static str, decimals: usize) -> Format {
    Format::Unit {
        unit,
        decimals,
        signed: false,
    }
}

/// The printed label, the name after the bus's, the description, and the format.
fn delay_face(param: DelayParam) -> (&'static str, &'static str, &'static str, Format) {
    match param {
        DelayParam::TimeL => (
            "Time L",
            "left time",
            "The left delay in Free mode.",
            in_unit("ms", 0),
        ),
        DelayParam::TimeR => (
            "Time R",
            "right time",
            "The right delay in Free mode.",
            in_unit("ms", 0),
        ),
        DelayParam::Feedback => (
            "Feedback",
            "feedback",
            "How much of each repeat returns. Above 100% the loop's soft limit holds it.",
            Format::whole("%"),
        ),
        DelayParam::Width => (
            "Width",
            "width",
            "0% is mono, 100% unchanged, 200% doubles the sides.",
            Format::whole("%"),
        ),
        DelayParam::LowCut => (
            "Low cut",
            "low cut",
            "Each repeat loses more below this.",
            Format::whole("Hz"),
        ),
        DelayParam::HighCut => (
            "High cut",
            "high cut",
            "Each repeat loses more above this.",
            Format::whole("Hz"),
        ),
        DelayParam::ModRate => ("Rate", "modulation rate", "", in_unit("Hz", 2)),
        DelayParam::ModDepth => (
            "Depth",
            "modulation depth",
            "The peak change of the delay time.",
            Format::tenths("ms"),
        ),
        DelayParam::Drive => (
            "Drive",
            "drive",
            "Saturates the sound entering the delay.",
            Format::whole("%"),
        ),
        DelayParam::DuckAmount => (
            "Duck",
            "duck amount",
            "Lowers the repeats while the input plays.",
            Format::whole("%"),
        ),
        DelayParam::DuckRelease => (
            "Release",
            "duck release",
            "How fast the repeats return.",
            Format::whole("ms"),
        ),
        DelayParam::Output => ("Output", "output", "", Format::db(true)),
    }
}

fn reverb_face(param: ReverbParam) -> (&'static str, &'static str, &'static str, Format) {
    match param {
        ReverbParam::PreDelay => (
            "Pre-delay",
            "pre-delay",
            "The gap before the space answers.",
            Format::whole("ms"),
        ),
        ReverbParam::Decay => (
            "Decay",
            "decay",
            "The time the tail takes to fall by 60 dB.",
            in_unit("s", 2),
        ),
        ReverbParam::Size => (
            "Size",
            "size",
            "The size of the space. It does not change the decay.",
            Format::whole("%"),
        ),
        ReverbParam::LateBalance => (
            "Late",
            "late balance",
            "0% early reflections only, 100% the tail only.",
            Format::whole("%"),
        ),
        ReverbParam::Diffusion => (
            "Diffusion",
            "diffusion",
            "How fast reflections blur together.",
            Format::whole("%"),
        ),
        ReverbParam::Density => (
            "Density",
            "density",
            "How many reflections, and how full the tail.",
            Format::whole("%"),
        ),
        ReverbParam::LowCut => (
            "Low cut",
            "low cut",
            "The tail loses more below this as it decays.",
            Format::whole("Hz"),
        ),
        ReverbParam::HighCut => (
            "High cut",
            "high cut",
            "The tail loses more above this as it decays.",
            Format::whole("Hz"),
        ),
        ReverbParam::Width => (
            "Width",
            "width",
            "0% is mono, 100% unchanged, 200% doubles the sides.",
            Format::whole("%"),
        ),
        ReverbParam::Drive => (
            "Drive",
            "drive",
            "Saturates the sound entering the reverb.",
            Format::whole("%"),
        ),
        ReverbParam::ModRate => ("Rate", "modulation rate", "", in_unit("Hz", 2)),
        ReverbParam::ModDepth => ("Depth", "modulation depth", "", Format::whole("%")),
        ReverbParam::ShimmerAmount => (
            "Shimmer",
            "shimmer amount",
            "How much of the tail returns pitched up. It acts while Shimmer is on.",
            Format::whole("%"),
        ),
        ReverbParam::DuckAmount => (
            "Duck",
            "duck amount",
            "Lowers the tail while the input plays.",
            Format::whole("%"),
        ),
        ReverbParam::DuckRelease => (
            "Release",
            "duck release",
            "How fast the tail returns.",
            Format::whole("ms"),
        ),
        ReverbParam::Output => ("Output", "output", "", Format::db(true)),
    }
}

/// The preset whose settings and return level a bus has, if any.
fn matching_preset(bus: &FxBus) -> Option<&'static str> {
    let level = (bus.return_level * 100.0).round();
    match &bus.module {
        Module::Empty => None,
        Module::EchoformDelay(delay) => Echoform::PRESETS
            .iter()
            .find(|(name, mix)| Echoform::preset(name).as_ref() == Some(delay) && *mix == level)
            .map(|(name, _)| *name),
        Module::AetherformReverb(reverb) => Aetherform::PRESETS
            .iter()
            .find(|(name, mix)| Aetherform::preset(name).as_ref() == Some(reverb) && *mix == level)
            .map(|(name, _)| *name),
    }
}

/// Show the board in the space left in `ui`. `reveal` brings a bus into view.
pub(super) fn show(ui: &mut egui::Ui, buses: &FxBuses, reveal: Option<usize>) -> Action {
    let mut draft = *buses;
    let mut clear = None;
    let area = ui.available_rect_before_wrap();
    ui.advance_cursor_after_rect(area);
    let mut board = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("effects-board")
            .max_rect(area),
    );
    dialog::hardware_style(&mut board, controls::Palette::Marine);
    board.set_clip_rect(area.intersect(ui.clip_rect()));
    controls::panel(&board, area);
    named(
        &board.interact(area, board.id().with("surface"), Sense::hover()),
        egui::WidgetType::Other,
        "Effects board",
    );
    let mut content = board.new_child(
        egui::UiBuilder::new()
            .id_salt("effects-content")
            .max_rect(area.shrink(BOARD_INSET)),
    );
    let lefts: Vec<f32> = buses
        .buses
        .iter()
        .scan(MARGIN, |left, bus| {
            let here = *left;
            *left += unit_width(&bus.module) + GAP;
            Some(here)
        })
        .collect();
    let natural = lefts[BUSES - 1] + unit_width(&buses.buses[BUSES - 1].module) + MARGIN;
    let mut scroll = egui::ScrollArea::both()
        .id_salt("effects-scroll")
        .auto_shrink([false, false]);
    if let Some(index) = reveal {
        scroll = scroll.horizontal_scroll_offset((lefts[index] - MARGIN).max(0.0));
    }
    scroll.show(&mut content, |ui| {
        let size = Vec2::new(ui.available_width().max(natural), ROW_HEIGHT);
        let (bounds, _) = ui.allocate_exact_size(size, Sense::hover());
        for (index, left) in lefts.iter().enumerate() {
            let rect = Rect::from_min_size(
                Pos2::new(bounds.left() + left, bounds.top() + UNIT_TOP),
                Vec2::new(unit_width(&buses.buses[index].module), UNIT_HEIGHT),
            );
            ui.push_id(("fx-unit", index), |ui| {
                if unit(ui, rect, index, &mut draft.buses[index]) {
                    clear = Some(index);
                }
            });
        }
    });
    if let Some(index) = clear {
        Action::ClearTail(index)
    } else if draft != *buses {
        Action::Change(Box::new(draft))
    } else {
        Action::None
    }
}

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
        INK,
    );
    response.on_hover_text(format!(
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
        name_control(response, name).on_hover_text(hint);
    });
}

/// One bus's unit. True when Clear Tail was pressed.
fn unit(ui: &mut egui::Ui, rect: Rect, index: usize, bus: &mut FxBus) -> bool {
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y);
    let bus_name = format!("FX {}", index + 1);
    let sounding = bus.powered && !bus.module.is_empty();
    enclosure_of(
        ui,
        rect,
        enamel_of(enamel(&bus.module), sounding),
        sounding,
        index as u64 + 21,
    );
    heading(ui, rect, &bus_name, bus.module.label(), INK);
    lamp(ui, at(rect.width() - 18.0, 19.0), sounding);
    let mut cleared = false;
    ui.scope(|ui| {
        ui.visuals_mut().dark_mode = false;
        // The effect, and its presets.
        let mut kind = match bus.module {
            Module::Empty => 0,
            Module::EchoformDelay(_) => 1,
            Module::AetherformReverb(_) => 2,
        };
        let before = kind;
        choice(
            ui,
            Rect::from_min_size(at(PADDING, SELECT_Y), Vec2::new(176.0, 26.0)),
            &format!("{bus_name} effect"),
            &mut kind,
            &[
                (0, "Empty"),
                (1, "Echoform Delay"),
                (2, "Aetherform Reverb"),
            ],
            "The effect on this bus. A new effect starts from its first preset; the old one's \
             tail is cut.",
        );
        if kind != before {
            (bus.module, bus.return_level) = match kind {
                1 => (
                    Module::EchoformDelay(Echoform::default()),
                    Echoform::PRESETS[0].1 / 100.0,
                ),
                2 => (
                    Module::AetherformReverb(Aetherform::default()),
                    Aetherform::PRESETS[0].1 / 100.0,
                ),
                _ => (Module::Empty, bus.return_level),
            };
        }
        let presets: Vec<&'static str> = match bus.module {
            Module::Empty => Vec::new(),
            Module::EchoformDelay(_) => Echoform::PRESETS.iter().map(|(name, _)| *name).collect(),
            Module::AetherformReverb(_) => {
                Aetherform::PRESETS.iter().map(|(name, _)| *name).collect()
            }
        };
        if !presets.is_empty() {
            let current = matching_preset(bus).unwrap_or("Custom");
            let mut chosen = current;
            let options: Vec<(&str, &str)> = presets.iter().map(|name| (*name, *name)).collect();
            let mut with_custom = options.clone();
            if current == "Custom" {
                with_custom.insert(0, ("Custom", "Custom"));
            }
            choice(
                ui,
                Rect::from_min_size(at(PADDING + 186.0, SELECT_Y), Vec2::new(170.0, 26.0)),
                &format!("{bus_name} preset"),
                &mut chosen,
                &with_custom,
                "Load a factory preset: it sets every control and the return level.",
            );
            if chosen != current {
                match bus.module {
                    Module::EchoformDelay(_) => {
                        if let Some(delay) = Echoform::preset(chosen) {
                            bus.module = Module::EchoformDelay(delay);
                        }
                        if let Some((_, mix)) =
                            Echoform::PRESETS.iter().find(|(name, _)| *name == chosen)
                        {
                            bus.return_level = mix / 100.0;
                        }
                    }
                    Module::AetherformReverb(_) => {
                        if let Some(reverb) = Aetherform::preset(chosen) {
                            bus.module = Module::AetherformReverb(reverb);
                        }
                        if let Some((_, mix)) =
                            Aetherform::PRESETS.iter().find(|(name, _)| *name == chosen)
                        {
                            bus.return_level = mix / 100.0;
                        }
                    }
                    Module::Empty => {}
                }
            }
        }
        // The effect's own switches and choices.
        match &mut bus.module {
            Module::Empty => {
                ui.painter().text(
                    at(rect.width() / 2.0, DIALS_Y + ROW),
                    Align2::CENTER_CENTER,
                    "Choose an effect for this bus.",
                    FontId::proportional(13.0),
                    INK,
                );
            }
            Module::EchoformDelay(delay) => {
                let mut sync = delay.mode == TimeMode::Sync;
                switch(
                    ui,
                    at(PADDING, MODE_Y),
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
                        Rect::from_min_size(at(PADDING + 76.0, MODE_Y), Vec2::new(66.0, 24.0)),
                        &format!("{bus_name} left division"),
                        &mut delay.division_l,
                        &divisions,
                        "The left delay as a note length: dotted lasts half as long again, \
                         triplet two thirds.",
                    );
                    choice(
                        ui,
                        Rect::from_min_size(at(PADDING + 150.0, MODE_Y), Vec2::new(66.0, 24.0)),
                        &format!("{bus_name} right division"),
                        &mut delay.division_r,
                        &divisions,
                        "The right delay as a note length.",
                    );
                });
                switch(
                    ui,
                    at(PADDING + 228.0, MODE_Y),
                    &format!("{bus_name} ping-pong"),
                    "Ping-pong",
                    &mut delay.ping_pong,
                    "Repeats cross from side to side.",
                );
                choice(
                    ui,
                    Rect::from_min_size(at(PADDING + 324.0, MODE_Y), Vec2::new(92.0, 24.0)),
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
                    Rect::from_min_size(at(PADDING, MODE_Y), Vec2::new(112.0, 24.0)),
                    &format!("{bus_name} space"),
                    &mut reverb.space_model,
                    &SpaceModel::ALL.map(|model| (model, model.label())),
                    "The shape of the space: its reflections and delay lengths.",
                );
                choice(
                    ui,
                    Rect::from_min_size(at(PADDING + 120.0, MODE_Y), Vec2::new(112.0, 24.0)),
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
                    at(PADDING + 242.0, MODE_Y),
                    &format!("{bus_name} early reflections"),
                    "Early",
                    &mut reverb.early_reflections_enabled,
                    "The first discrete reflections of the space.",
                );
                switch(
                    ui,
                    at(PADDING + 322.0, MODE_Y),
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
                        Rect::from_min_size(at(PADDING + 418.0, MODE_Y), Vec2::new(76.0, 24.0)),
                        &format!("{bus_name} shimmer interval"),
                        &mut reverb.shimmer_interval_semitones,
                        &labels,
                        "The shimmer's pitch step in semitones: a fifth, an octave, an octave and \
                         a fifth, or two octaves.",
                    );
                });
            }
        }
        // The dials, in two rows.
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
            let positions = [at(x, y), at(x, y + 38.0), at(x, y + 60.0)];
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
                    let (label, name, hint, format) = delay_face(*param);
                    let enabled = !matches!(param, DelayParam::TimeL | DelayParam::TimeR)
                        || delay.mode == TimeMode::Free;
                    let mut value = f64::from(param.get(delay));
                    let before = value;
                    dial(
                        ui,
                        positions,
                        &mut value,
                        Dial {
                            name: &format!("{bus_name} {name}"),
                            label,
                            hint,
                            range: knob(param.range()),
                            format,
                            choices: &[],
                        },
                        enabled,
                    );
                    if value != before {
                        param.set(delay, value as f32);
                    }
                }
                Slot::Reverb(param) => {
                    let Module::AetherformReverb(reverb) = &mut bus.module else {
                        continue;
                    };
                    let (label, name, hint, format) = reverb_face(*param);
                    let enabled = *param != ReverbParam::ShimmerAmount || reverb.shimmer_enabled;
                    let mut value = f64::from(param.get(reverb));
                    let before = value;
                    dial(
                        ui,
                        positions,
                        &mut value,
                        Dial {
                            name: &format!("{bus_name} {name}"),
                            label,
                            hint,
                            range: knob(param.range()),
                            format,
                            choices: &[],
                        },
                        enabled,
                    );
                    if value != before {
                        param.set(reverb, value as f32);
                    }
                }
            }
        }
        // The bus's own switches.
        let footer = FOOTER_Y + 14.0;
        switch(
            ui,
            at(PADDING, footer),
            &format!("{bus_name} power"),
            "Input",
            &mut bus.powered,
            "Off stops new sound into the effect; its tail rings out.",
        );
        switch(
            ui,
            at(PADDING + 80.0, footer),
            &format!("{bus_name} limiter"),
            "Limiter",
            &mut bus.limiter,
            "Holds the return under -1 dBFS.",
        );
        let bypass = match &mut bus.module {
            Module::EchoformDelay(delay) => Some(&mut delay.bypass),
            Module::AetherformReverb(reverb) => Some(&mut reverb.bypass),
            Module::Empty => None,
        };
        if let Some(bypass) = bypass {
            switch(
                ui,
                at(PADDING + 172.0, footer),
                &format!("{bus_name} bypass"),
                "Bypass",
                bypass,
                "Silences the effect while it keeps running, so its tail returns when Bypass goes \
                 off.",
            );
            let button = Rect::from_min_size(
                at(rect.width() - PADDING - 96.0, footer),
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
                .on_hover_text(
                    "Cut the effect's ringing tail now. This is not an edit and cannot be undone.",
                )
                .clicked()
            {
                cleared = true;
            }
        }
    });
    cleared
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_stays_legible_on_every_unit_lit_or_off() {
        for colour in [DELAY_ENAMEL, REVERB_ENAMEL, EMPTY_ENAMEL] {
            for on in [true, false] {
                let ratio = controls::contrast(enamel_of(colour, on), INK);
                assert!(ratio >= 5.0, "{colour:?} ({on}) gives {ratio:.2}:1");
            }
        }
    }

    #[test]
    fn every_unit_fits_its_dials_in_two_rows() {
        for module in [
            Module::Empty,
            Module::EchoformDelay(Echoform::default()),
            Module::AetherformReverb(Aetherform::default()),
        ] {
            let count = slots(&module).len();
            assert!(count <= 2 * columns(&module));
            assert!(columns(&module) as f32 * CELL + 2.0 * PADDING <= unit_width(&module));
        }
        assert_eq!(
            slots(&Module::AetherformReverb(Aetherform::default())).len(),
            17
        );
    }

    #[test]
    fn a_preset_matches_only_with_its_return_level() {
        let mut bus = FxBus {
            module: Module::AetherformReverb(Aetherform::preset("Dark Hall").unwrap()),
            return_level: 0.92,
            ..FxBus::default()
        };
        assert_eq!(matching_preset(&bus), Some("Dark Hall"));
        bus.return_level = 0.5;
        assert_eq!(matching_preset(&bus), None);
    }
}
