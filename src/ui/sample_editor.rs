//! Sample Editor body. Session owns sample processing and saving.
use super::controls::CREAM;
use super::*;
const EDGE: Color32 = Color32::from_rgb(102, 110, 110);
pub(super) struct Source<'a> {
    pub name: &'a str,
    pub frames: u64,
    pub sample_rate: u32,
    pub available: bool,
    pub missing: bool,
}

#[derive(Debug, PartialEq)]
pub(super) enum Action {
    None,
    Listen,
    Save,
    Details,
    Help,
}

pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    start: &mut u64,
    end: &mut u64,
    pitch: &mut f64,
) -> Action {
    let mut action = Action::None;
    let response = dialog::tool(
        ctx,
        "Sample editor",
        open,
        Vec2::new(640.0, 320.0),
        controls::Palette::Granite,
        true,
        |ui| {
            let (bounds, _) = ui.allocate_exact_size(Vec2::new(640.0, 320.0), Sense::hover());
            let at = |x: f32, y: f32| bounds.min + Vec2::new(x, y - 48.0);
            let rect = |x, y, w, h| Rect::from_min_size(at(x, y), Vec2::new(w, h));
            let mut panel = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
            controls::inset(&panel, rect(24.0, 61.0, 592.0, 32.0));
            controls::inset(&panel, rect(16.0, 96.0, 608.0, 146.0));
            let name = panel.put(
                rect(32.0, 64.0, 576.0, 26.0),
                egui::Label::new(egui::RichText::new(source.name).size(13.0).color(CREAM))
                    .truncate(),
            );
            name_control(name.clone(), "Source sound");
            ctx.accesskit_node_builder(name.id, |node| node.set_value(source.name));
            name.on_hover_text(source.name);
            label(&panel, at(34.0, 126.0), "Start frame", 13.0);
            label(&panel, at(342.0, 126.0), "End frame", 13.0);
            panel.add_enabled_ui(source.available && source.frames > 0, |ui| {
                frame_value(
                    ui,
                    rect(128.0, 102.0, 168.0, 48.0),
                    "Start frame",
                    start,
                    0..=source.frames.saturating_sub(1),
                );
                frame_value(
                    ui,
                    rect(436.0, 102.0, 168.0, 48.0),
                    "End frame",
                    end,
                    1..=source.frames.max(1),
                );
                pitch_rail(ui, rect(34.0, 188.0, 242.0, 36.0), pitch);
                pitch_value(ui, rect(310.0, 182.0, 112.0, 48.0), pitch);
            });
            label(&panel, at(440.0, 206.0), "Pitch (semitones)", 13.0);
            let reason = if source.missing {
                Some(
                    "This sound is missing and plays silence. Open Import details to inspect or resolve it.",
                )
            } else if !source.available || source.frames == 0 {
                Some("Select a nonempty sound or clip before editing.")
            } else if *start >= *end || *end > source.frames {
                Some("End frame must be greater than Start frame and within the source.")
            } else if !pitch.is_finite() || !(-24.0..=24.0).contains(pitch) {
                Some("Pitch must be from -24 to 24 semitones.")
            } else {
                None
            };
            let ready = reason.is_none();
            let listen = action_button(
                &mut panel,
                rect(32.0, 266.0, 224.0, 36.0),
                "Listen edit",
                false,
                ready,
            );
            if listen
                .on_hover_text(
                    "Preview the selected frames at this pitch. The source stays unchanged.",
                )
                .on_disabled_hover_text(reason.unwrap_or("Select a nonempty trim range."))
                .clicked()
            {
                action = Action::Listen;
            }
            let save = action_button(
                &mut panel,
                rect(284.0, 266.0, 324.0, 36.0),
                "Save edited sound",
                true,
                ready,
            );
            if save
                .on_hover_text(
                    "Save the edit as a new WAV in Wave. The original source stays unchanged.",
                )
                .on_disabled_hover_text(reason.unwrap_or("Select a nonempty trim range."))
                .clicked()
            {
                action = Action::Save;
            }
            let summary = reason.map(str::to_owned).unwrap_or_else(|| {
                let selected = *end - *start;
                if source.sample_rate > 0 {
                    format!(
                        "{selected} source frames selected · {:.3} s after pitch",
                        (selected as f64 / 2.0_f64.powf(*pitch / 12.0))
                            .round()
                            .max(1.0)
                            / f64::from(source.sample_rate)
                    )
                } else {
                    format!("{selected} source frames selected")
                }
            });
            controls::well(&panel, rect(8.0, 326.0, 624.0, 36.0));
            panel.painter().rect_stroke(
                rect(8.0, 326.0, 624.0, 36.0),
                4.0,
                Stroke::new(1.0_f32, if reason.is_some() { CORAL } else { EDGE }),
                egui::StrokeKind::Inside,
            );
            let width = if source.missing { 460.0 } else { 592.0 };
            let status = panel.put(
                rect(24.0, 326.0, width, 36.0),
                egui::Label::new(
                    egui::RichText::new(&summary)
                        .size(12.5)
                        .color(if reason.is_some() { CORAL } else { CREAM }),
                )
                .wrap(),
            );
            name_control(status.clone(), "Sample editor status");
            ctx.accesskit_node_builder(status.id, |node| {
                node.set_value(summary.as_str());
                node.set_live(egui::accesskit::Live::Polite);
            });
            status.on_hover_text(&summary);
            if source.missing
                && action_button(
                    &mut panel,
                    rect(492.0, 330.0, 128.0, 28.0),
                    "Import details",
                    false,
                    true,
                )
                .clicked()
            {
                action = Action::Details;
            }
        },
    );
    if response.help {
        action = Action::Help;
    }
    action
}
fn label(ui: &egui::Ui, at: Pos2, value: &str, size: f32) {
    ui.painter().text(
        at,
        Align2::LEFT_CENTER,
        value,
        FontId::proportional(size),
        CREAM,
    );
}
fn action_button(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    primary: bool,
    enabled: bool,
) -> Response {
    controls::button_at(
        ui,
        rect,
        name,
        name,
        if primary {
            controls::ButtonRole::Primary
        } else {
            controls::ButtonRole::Secondary
        },
        enabled,
    )
}
fn stepper(ui: &mut egui::Ui, rect: Rect, name: &str, up: bool) -> Response {
    let response = controls::button_at(
        ui,
        rect,
        name,
        if up { "+" } else { "−" },
        controls::ButtonRole::Secondary,
        true,
    );
    name_control(response, name)
}
fn frame_value(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    value: &mut u64,
    range: std::ops::RangeInclusive<u64>,
) {
    controls::well(ui, rect);
    ui.scope_builder(egui::UiBuilder::new().id(name).max_rect(Rect::from_min_max(rect.min+Vec2::new(6.0,10.0),rect.max-Vec2::new(28.0,10.0))).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)),|ui|{
        controls::value_style(ui);let size=ui.available_size();ui.spacing_mut().interact_size=size;let response=ui.add(egui::DragValue::new(value).range(range.clone()).clamp_existing_to_range(false).speed(100).fixed_decimals(0).update_while_editing(false));if response.changed(){*value=(*value).clamp(*range.start(),*range.end());}
        name_control(response,name).on_hover_text(if name=="Start frame"{"First included source frame (zero-based). Click for exact entry; Enter applies. Use the adjacent buttons for one-frame steps (Shift: 100)."} else {"First excluded source frame. Set this to the source frame count to include the last frame. Click for exact entry; Enter applies. Use the adjacent buttons for one-frame steps (Shift: 100)."});
    });
    for (up, y) in [(true, 0.0), (false, 24.0)] {
        let label = format!("{} {name}", if up { "Increase" } else { "Decrease" });
        let button = stepper(
            ui,
            Rect::from_min_size(
                Pos2::new(rect.right() - 25.0, rect.top() + y),
                Vec2::new(24.0, 24.0),
            ),
            &label,
            up,
        );
        if button
            .on_hover_text("Change one frame. Hold Shift to change 100 frames.")
            .clicked()
        {
            let amount = if ui.input(|input| input.modifiers.shift) {
                100
            } else {
                1
            };
            *value = if up {
                value.saturating_add(amount)
            } else {
                value.saturating_sub(amount)
            };
            *value = (*value).clamp(*range.start(), *range.end());
        }
    }
}
fn pitch_value(ui: &mut egui::Ui, rect: Rect, pitch: &mut f64) {
    controls::well(ui, rect);
    ui.scope_builder(egui::UiBuilder::new().id("pitch-value").max_rect(Rect::from_min_max(rect.min+Vec2::new(6.0,10.0),rect.max-Vec2::new(28.0,10.0))).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)),|ui|{
        controls::value_style(ui);let size=ui.available_size();ui.spacing_mut().interact_size=size;let id=ui.next_auto_id();let context=ui.ctx().clone();let response=ui.add(egui::DragValue::new(pitch).range(-24.0..=24.0).clamp_existing_to_range(false).speed(0.1).custom_formatter(move|value,_|if context.memory(|memory|memory.has_focus(id)){format!("{value}")}else{format!("{value:.1}")}).update_while_editing(false));if response.changed(){*pitch=pitch.clamp(-24.0,24.0);}name_control(response,"Pitch value").on_hover_text("Enter semitones from -24 to +24. Pitch changes playback speed and output duration together. Zero keeps the source pitch.");
    });
    for (up, y) in [(true, 0.0), (false, 24.0)] {
        let name = if up {
            "Increase Pitch"
        } else {
            "Decrease Pitch"
        };
        let button = stepper(
            ui,
            Rect::from_min_size(
                Pos2::new(rect.right() - 25.0, rect.top() + y),
                Vec2::new(24.0, 24.0),
            ),
            name,
            up,
        );
        if button
            .on_hover_text("Change 0.1 semitone. Hold Shift for one semitone.")
            .clicked()
        {
            let amount = if ui.input(|input| input.modifiers.shift) {
                1.0
            } else {
                0.1
            };
            *pitch += if up { amount } else { -amount };
            *pitch = pitch.clamp(-24.0, 24.0);
        }
    }
}
fn pitch_rail(ui: &mut egui::Ui, rect: Rect, pitch: &mut f64) {
    ui.scope_builder(egui::UiBuilder::new().id("pitch-rail").max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)),|ui|{
        ui.spacing_mut().slider_width=rect.width();
        controls::slider(ui,pitch,-24.0..=24.0,"Pitch (semitones)")
            .on_hover_text("Pitch changes playback speed and sample length. +12 is one octave up and half the duration; -12 is one octave down and twice the duration. Use the numeric value for exact entry.");
    });
}
