//! Effect Studio body. Shared hardware controls and shell own presentation.
use super::controls::CREAM;
use super::*;
const ERROR: Color32 = Color32::from_rgb(255, 167, 143);
pub(super) struct Source<'a> {
    pub name: &'a str,
    pub available: bool,
    pub missing: bool,
}

pub(super) struct Feedback<'a> {
    pub message: Option<&'a str>,
    pub error: bool,
}

pub(super) enum Action {
    None,
    Listen,
    Save,
    Reset,
    Details,
    Help,
}

pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    feedback: Feedback<'_>,
    state: &mut EffectsState,
) -> Result<Action, String> {
    let mut action = Action::None;
    let response = dialog::tool(
        ctx,
        "Effect Studio",
        open,
        Vec2::new(640.0, 400.0),
        controls::Palette::Red,
        true,
        |ui| {
            let scale = 1.0;
            let (bounds, _) = ui.allocate_exact_size(Vec2::new(640.0, 400.0), Sense::hover());
            let point = |x: f32, y: f32| bounds.min + Vec2::new(x, y - 48.0);
            let rect = |x, y, w, h| Rect::from_min_size(point(x, y), Vec2::new(w, h));
            let mut panel = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
            // Recessed modules sit within the colored folded chassis.
            controls::inset(&panel, rect(16.0, 48.0, 608.0, 24.0));
            for (x, y, w, h) in [
                (16.0, 76.0, 344.0, 126.0),
                (372.0, 76.0, 252.0, 126.0),
                (16.0, 208.0, 260.0, 128.0),
                (280.0, 208.0, 344.0, 128.0),
            ] {
                controls::inset(&panel, rect(x, y, w, h));
            }
            for (x, y, w, label) in [
                (22.0, 78.0, 330.0, "Filter"),
                (382.0, 78.0, 222.0, "Echo"),
                (22.0, 210.0, 244.0, "Robotize"),
                (288.0, 210.0, 316.0, "Distortion / output"),
            ] {
                controls::section(&panel, rect(x, y, w, 22.0), label);
            }
            panel.scope(|ui| {
                let text = egui::RichText::new(source.name)
                    .font(FontId::proportional(13.0 * scale))
                    .color(CREAM);
                let label = ui.put(
                    rect(27.0, 48.0, 586.0, 22.0),
                    egui::Label::new(text).truncate(),
                );
                named(&label, egui::WidgetType::Label, "Source sound");
                ui.ctx().accesskit_node_builder(label.id, |builder| {
                    builder.set_label("Source sound");
                    builder.set_value(source.name);
                });
                label.on_hover_text(source.name);
            });

            let mut filter_enabled = state.filter.is_some();
            if toggle(
                &mut panel,
                point(338.0, 89.0),
                scale,
                "Filter",
                &mut filter_enabled,
            ) {
                state.filter = filter_enabled.then(Default::default);
            }
            let mut filter = state.filter.clone().unwrap_or_default();
            for (x, value, name, low, high, reset, step, decimals, logarithmic) in [
                (
                    65.5,
                    &mut filter.cutoff_hz,
                    "Cutoff",
                    20.0,
                    20000.0,
                    2000.0,
                    1.0,
                    0,
                    true,
                ),
                (
                    147.0,
                    &mut filter.resonance,
                    "Resonance",
                    0.0,
                    1.0,
                    0.0,
                    0.001,
                    3,
                    false,
                ),
                (
                    229.5,
                    &mut filter.drive,
                    "Overdrive",
                    0.0,
                    1.0,
                    0.0,
                    0.001,
                    3,
                    false,
                ),
                (
                    312.0,
                    &mut filter.wet,
                    "Filter volume",
                    0.0,
                    1.0,
                    1.0,
                    0.001,
                    3,
                    false,
                ),
            ] {
                control(
                    &mut panel,
                    [point(x, 131.0), point(x, 166.0), point(x, 189.0)],
                    scale,
                    value,
                    Control {
                        name,
                        low,
                        high,
                        reset,
                        step,
                        decimals,
                        logarithmic,
                    },
                    filter_enabled,
                );
            }
            if filter_enabled {
                state.filter = Some(filter);
            }

            let mut echo_enabled = state.echo.is_some();
            if toggle(
                &mut panel,
                point(600.0, 89.0),
                scale,
                "Echo",
                &mut echo_enabled,
            ) {
                state.echo = echo_enabled.then(Default::default);
            }
            let mut echo = state.echo.clone().unwrap_or_default();
            for (x, value, name, low, high, reset, step, decimals) in [
                (
                    423.0,
                    &mut echo.delay_beats,
                    "Delay (beats)",
                    0.001,
                    2.0,
                    0.5,
                    0.01,
                    3,
                ),
                (
                    500.0,
                    &mut echo.feedback,
                    "Feedback",
                    0.0,
                    0.95,
                    0.4,
                    0.01,
                    2,
                ),
                (577.5, &mut echo.wet, "Echo volume", 0.0, 1.0, 0.35, 0.01, 2),
            ] {
                control(
                    &mut panel,
                    [point(x, 131.0), point(x, 166.0), point(x, 189.0)],
                    scale,
                    value,
                    Control {
                        name,
                        low,
                        high,
                        reset,
                        step,
                        decimals,
                        logarithmic: false,
                    },
                    echo_enabled,
                );
            }
            if echo_enabled {
                state.echo = Some(echo);
            }

            let mut robot_enabled = state.robotize.is_some();
            if toggle(
                &mut panel,
                point(246.0, 221.0),
                scale,
                "Robotize",
                &mut robot_enabled,
            ) {
                state.robotize = robot_enabled.then(Default::default);
            }
            let mut robot = state.robotize.clone().unwrap_or_default();
            for (x, value, name, low, high, reset, step, decimals, logarithmic) in [
                (
                    93.0,
                    &mut robot.frequency_hz,
                    "Color",
                    1.0,
                    2000.0,
                    80.0,
                    1.0,
                    0,
                    true,
                ),
                (
                    196.0,
                    &mut robot.wet,
                    "Robotize volume",
                    0.0,
                    1.0,
                    0.5,
                    0.01,
                    2,
                    false,
                ),
            ] {
                control(
                    &mut panel,
                    [point(x, 263.0), point(x, 298.0), point(x, 321.0)],
                    scale,
                    value,
                    Control {
                        name,
                        low,
                        high,
                        reset,
                        step,
                        decimals,
                        logarithmic,
                    },
                    robot_enabled,
                );
            }
            if robot_enabled {
                state.robotize = Some(robot);
            }

            for (x, value, name, high, reset, step, decimals) in [
                (
                    340.5,
                    &mut state.dirty,
                    "Dirty distortion",
                    1.0,
                    0.0,
                    0.001,
                    3,
                ),
                (
                    449.0,
                    &mut state.noise,
                    "Noise distortion",
                    1.0,
                    0.0,
                    0.001,
                    3,
                ),
                (558.0, &mut state.volume, "Output volume", 2.0, 1.0, 0.01, 2),
            ] {
                control(
                    &mut panel,
                    [point(x, 263.0), point(x, 298.0), point(x, 321.0)],
                    scale,
                    value,
                    Control {
                        name,
                        low: 0.0,
                        high,
                        reset,
                        step,
                        decimals,
                        logarithmic: false,
                    },
                    true,
                );
            }
            let available = source.available && !source.missing;
            for (x, width, text, next, enabled) in [
                (48.0, 164.0, "Listen", Action::Listen, available),
                (232.0, 177.0, "Save sound", Action::Save, available),
                (430.0, 165.0, "Reset", Action::Reset, true),
            ] {
                if action_button(
                    &mut panel,
                    rect(x, 350.0, width, 36.0),
                    text,
                    enabled,
                    scale,
                )
                .clicked()
                {
                    action = next;
                }
            }
            let status = if source.missing {
                "Missing audio. Open Import details to resolve the sound."
            } else if !source.available {
                "Select a sound in the palette to begin."
            } else {
                feedback
                    .message
                    .unwrap_or("Listen before saving. Save sound adds a new WAV to Effect.")
            };
            let status_rect = rect(8.0, 404.0, 624.0, 36.0);
            status_footer(
                &mut panel,
                status_rect,
                status,
                feedback.error,
                source.missing,
                scale,
            );
            if source.missing
                && controls::button_at(
                    &mut panel,
                    rect(526.0, 407.0, 94.0, 30.0),
                    "Import details",
                    "Import details",
                    controls::ButtonRole::Secondary,
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
    Ok(action)
}
fn toggle(ui: &mut egui::Ui, center: Pos2, _scale: f32, name: &str, value: &mut bool) -> bool {
    let response = controls::toggle_at(
        ui,
        Rect::from_center_size(center, Vec2::new(36.0, 24.0)),
        name,
        "",
        value,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, response.enabled(), *value, name)
    });
    let changed = response.changed();
    response.on_hover_text(format!(
        "{name}: {}. Turning it off discards its settings.",
        if *value { "on" } else { "off" }
    ));
    changed
}
#[derive(Clone, Copy)]
struct Control<'a> {
    name: &'a str,
    low: f32,
    high: f32,
    reset: f32,
    step: f32,
    decimals: usize,
    logarithmic: bool,
}
fn control(
    ui: &mut egui::Ui,
    positions: [Pos2; 3],
    _scale: f32,
    value: &mut f32,
    spec: Control<'_>,
    enabled: bool,
) {
    let [center, well, label] = positions;
    ui.painter().text(
        label,
        Align2::CENTER_CENTER,
        spec.name,
        FontId::proportional(12.0),
        CREAM,
    );
    controls::well(ui, Rect::from_center_size(well, Vec2::new(60.0, 20.0)));
    ui.add_enabled_ui(enabled, |ui| {
        let mut draft = f64::from(*value);
        let before = draft;
        controls::rotary(
            ui,
            Rect::from_center_size(center, Vec2::splat(44.0)),
            &mut draft,
            controls::RotarySpec {
                name: spec.name,
                low: f64::from(spec.low),
                high: f64::from(spec.high),
                reset: f64::from(spec.reset),
                step: f64::from(spec.step),
                decimals: spec.decimals,
                logarithmic: spec.logarithmic,
                choices: &[],
            },
        );
        if draft != before {
            *value = draft as f32;
        }
        ui.scope(|ui| {
            controls::value_style(ui);
            let response = ui.put(
                Rect::from_center_size(well, Vec2::new(58.0, 18.0)),
                egui::DragValue::new(value)
                    .range(spec.low..=spec.high)
                    .speed(spec.step)
                    .fixed_decimals(spec.decimals),
            );
            name_control(response, &format!("{} value", spec.name))
                .on_hover_text(format!("{}: drag, or click to enter a value.", spec.name));
        });
    });
}
fn action_button(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    enabled: bool,
    _scale: f32,
) -> Response {
    controls::button_at(
        ui,
        rect,
        text,
        text,
        if text == "Save sound" {
            controls::ButtonRole::Primary
        } else {
            controls::ButtonRole::Secondary
        },
        enabled,
    )
    .on_disabled_hover_text("Select an available sound to process it.")
    .on_hover_text(if text == "Save sound" {
        "Save a new WAV and add it to Effect."
    } else {
        text
    })
}
fn named(response: &Response, kind: egui::WidgetType, text: &str) {
    response.widget_info(|| egui::WidgetInfo::labeled(kind, response.enabled(), text));
}
fn status_footer(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    error: bool,
    missing: bool,
    scale: f32,
) {
    controls::well(ui, rect);
    ui.painter().rect_stroke(
        rect,
        4.0 * scale,
        Stroke::new(1.0_f32, Color32::from_rgb(90, 65, 58)),
        egui::StrokeKind::Inside,
    );
    let response = ui.interact(rect, ui.id().with("status"), Sense::hover());
    named(&response, egui::WidgetType::Panel, "Effect Studio status");
    let text_rect = Rect::from_min_max(
        rect.min + Vec2::new(9.0, 6.0) * scale,
        rect.max - Vec2::new(if missing { 116.0 } else { 9.0 }, 6.0) * scale,
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(text_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(text_rect.intersect(ui.clip_rect()));
    let previous_id = ui.id().with("previous-status");
    let changed = ui
        .ctx()
        .data(|data| data.get_temp::<String>(previous_id).as_deref() != Some(text));
    ui.ctx()
        .data_mut(|data| data.insert_temp(previous_id, text.to_owned()));
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("effects-status")
        .max_height(text_rect.height())
        .auto_shrink([false, false]);
    if changed {
        scroll = scroll.vertical_scroll_offset(0.0);
    }
    scroll.show(&mut child, |ui| {
        let color = if error { ERROR } else { CREAM };
        let mut job = egui::text::LayoutJob::simple(
            text.into(),
            FontId::proportional(12.5 * scale),
            color,
            ui.available_width(),
        );
        job.halign = egui::Align::Center;
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        let (area, response) = ui.allocate_exact_size(
            Vec2::new(
                ui.available_width(),
                galley.mesh_bounds.height().max(text_rect.height()),
            ),
            Sense::hover(),
        );
        let origin = if galley.mesh_bounds.height() <= text_rect.height() {
            area.center() - galley.mesh_bounds.center().to_vec2()
        } else {
            Pos2::new(
                area.center().x - galley.mesh_bounds.center().x,
                area.top() - galley.mesh_bounds.top(),
            )
        };
        ui.painter().galley(origin, galley, color);
        named(
            &response,
            egui::WidgetType::Label,
            &if error {
                format!("Error: {text}")
            } else {
                text.into()
            },
        );
        ui.ctx().accesskit_node_builder(response.id, |builder| {
            builder.set_live(egui::accesskit::Live::Polite)
        });
        response.on_hover_text(text);
    });
}
