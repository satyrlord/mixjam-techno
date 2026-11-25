//! Effect Studio body. Shared hardware controls and shell own presentation.
use super::rect_in;
use super::*;

const WINDOW_SIZE: Vec2 = Vec2::new(640.0, 432.0);
const PAD_X: f32 = 12.0;
const INNER_W: f32 = 616.0;
const FILTER_ACCENT: Color32 = Color32::from_rgb(86, 204, 188);
const ECHO_ACCENT: Color32 = Color32::from_rgb(96, 194, 242);
const ROBOT_ACCENT: Color32 = Color32::from_rgb(186, 152, 246);
const DISTORT_ACCENT: Color32 = Color32::from_rgb(246, 148, 76);

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

fn dial_positions(bounds: Rect, x: f32, knob_y: f32) -> [Pos2; 3] {
    [
        bounds.min + Vec2::new(x, knob_y),
        bounds.min + Vec2::new(x, knob_y + 38.0),
        bounds.min + Vec2::new(x, knob_y + 63.0),
    ]
}

pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    feedback: Feedback<'_>,
    state: &mut EffectsState,
) -> Action {
    let mut action = Action::None;
    let response = dialog::tool(
        ctx,
        "Effect Studio",
        open,
        WINDOW_SIZE,
        controls::Palette::Red,
        true,
        |ui| {
            action = draw_body(ui, &source, &feedback, state);
        },
    );
    if response.help { Action::Help } else { action }
}

fn draw_body(
    ui: &mut egui::Ui,
    source: &Source<'_>,
    feedback: &Feedback<'_>,
    state: &mut EffectsState,
) -> Action {
    let (bounds, mut panel) = super::tool_panel(ui, WINDOW_SIZE);
    draw_source_strip(&mut panel, bounds, source);
    draw_effect_bays(&mut panel, bounds, state);
    let action = draw_actions(&mut panel, bounds, source);
    if draw_footer(&mut panel, bounds, source, feedback) {
        return Action::Details;
    }
    action
}

fn draw_effect_bays(ui: &mut egui::Ui, bounds: Rect, state: &mut EffectsState) {
    draw_filter_bay(ui, bounds, state);
    draw_echo_bay(ui, bounds, state);
    draw_robotize_bay(ui, bounds, state);
    draw_distortion_bay(ui, bounds, state);
}

fn draw_source_strip(ui: &mut egui::Ui, bounds: Rect, source: &Source<'_>) {
    controls::well(ui, rect_in(bounds, PAD_X, 6.0, INNER_W, 28.0));
    ui.scope(|ui| {
        let text = egui::RichText::new(source.name)
            .font(FontId::proportional(13.0))
            .color(roles().legend);
        let label = ui.put(
            rect_in(bounds, 22.0, 9.0, 596.0, 22.0),
            egui::Label::new(text).truncate(),
        );
        named(&label, egui::WidgetType::Label, "Source sound");
        ui.ctx().accesskit_node_builder(label.id, |builder| {
            builder.set_label("Source sound");
            builder.set_value(source.name);
        });
        label.hover_hint(source.name);
    });
}

fn bank_toggle(ui: &mut egui::Ui, bay: Rect, name: &str, value: &mut bool) -> bool {
    let rect = Rect::from_min_size(
        Pos2::new(bay.right() - 44.0, bay.top() + 1.0),
        Vec2::new(28.0, 20.0),
    );
    controls::instrument::bat_toggle(
        ui,
        rect,
        name,
        "",
        value,
        "Turning it off discards its settings.",
        false,
    )
    .changed()
}

fn draw_filter_bay(ui: &mut egui::Ui, bounds: Rect, state: &mut EffectsState) {
    let bay = rect_in(bounds, PAD_X, 44.0, 344.0, 138.0);
    let mut enabled = state.filter.is_some();
    controls::instrument::bay_with_state(ui, bay, "Filter", FILTER_ACCENT, false, enabled);
    if bank_toggle(ui, bay, "Filter", &mut enabled) {
        state.filter = enabled.then(Default::default);
    }
    let mut filter = state.filter.clone().unwrap_or_default();
    theme::with_knob_style(theme::KnobStyle::Anodized([44, 156, 138]), || {
        for (x, value, name, low, high, reset, step, decimals, logarithmic) in [
            (
                61.0,
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
                143.0,
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
                225.0,
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
                307.0,
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
                ui,
                dial_positions(bounds, x, 98.0),
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
                enabled,
            );
        }
    });
    if enabled {
        state.filter = Some(filter);
    }
}

fn draw_echo_bay(ui: &mut egui::Ui, bounds: Rect, state: &mut EffectsState) {
    let bay = rect_in(bounds, 366.0, 44.0, 262.0, 138.0);
    let mut enabled = state.echo.is_some();
    controls::instrument::bay_with_state(ui, bay, "Echo", ECHO_ACCENT, false, enabled);
    if bank_toggle(ui, bay, "Echo", &mut enabled) {
        state.echo = enabled.then(Default::default);
    }
    let mut echo = state.echo.clone().unwrap_or_default();
    theme::with_knob_style(theme::KnobStyle::Anodized([56, 162, 214]), || {
        for (x, value, name, low, high, reset, step, decimals) in [
            (
                415.0,
                &mut echo.delay_beats,
                "Delay (beats)",
                mixjam_techno::application::MIN_ECHO_BEATS,
                2.0,
                0.5,
                0.01,
                3,
            ),
            (
                497.0,
                &mut echo.feedback,
                "Feedback",
                0.0,
                0.95,
                0.4,
                0.01,
                2,
            ),
            (579.0, &mut echo.wet, "Echo volume", 0.0, 1.0, 0.35, 0.01, 2),
        ] {
            control(
                ui,
                dial_positions(bounds, x, 98.0),
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
                enabled,
            );
        }
    });
    if enabled {
        state.echo = Some(echo);
    }
}

fn draw_robotize_bay(ui: &mut egui::Ui, bounds: Rect, state: &mut EffectsState) {
    let bay = rect_in(bounds, PAD_X, 192.0, 246.0, 138.0);
    let mut enabled = state.robotize.is_some();
    controls::instrument::bay_with_state(ui, bay, "Robotize", ROBOT_ACCENT, false, enabled);
    if bank_toggle(ui, bay, "Robotize", &mut enabled) {
        state.robotize = enabled.then(Default::default);
    }
    let mut robot = state.robotize.clone().unwrap_or_default();
    theme::with_knob_style(theme::KnobStyle::BrushedChrome([42, 34, 86]), || {
        for (x, value, name, low, high, reset, step, decimals, logarithmic) in [
            (
                80.0,
                &mut robot.frequency_hz,
                "Color",
                1.0,
                20000.0,
                80.0,
                1.0,
                0,
                true,
            ),
            (
                190.0,
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
                ui,
                dial_positions(bounds, x, 246.0),
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
                enabled,
            );
        }
    });
    if enabled {
        state.robotize = Some(robot);
    }
}

fn draw_distortion_bay(ui: &mut egui::Ui, bounds: Rect, state: &mut EffectsState) {
    let bay = rect_in(bounds, 268.0, 192.0, 360.0, 138.0);
    controls::instrument::bay_with_state(
        ui,
        bay,
        "Distortion / output",
        DISTORT_ACCENT,
        false,
        true,
    );
    theme::with_knob_style(theme::KnobStyle::Anodized([208, 102, 42]), || {
        for (x, value, name, high, reset, step, decimals) in [
            (
                336.0,
                &mut state.dirty,
                "Dirty distortion",
                1.0,
                0.0,
                0.001,
                3,
            ),
            (
                448.0,
                &mut state.noise,
                "Noise distortion",
                1.0,
                0.0,
                0.001,
                3,
            ),
            (560.0, &mut state.volume, "Output volume", 2.0, 1.0, 0.01, 2),
        ] {
            control(
                ui,
                dial_positions(bounds, x, 246.0),
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
    });
}

fn draw_actions(ui: &mut egui::Ui, bounds: Rect, source: &Source<'_>) -> Action {
    let available = source.available && !source.missing;
    let mut action = Action::None;
    for (x, width, text, next, enabled) in [
        (PAD_X, 199.0, "Listen", Action::Listen, available),
        (221.0, 198.0, "Save sound", Action::Save, available),
        (429.0, 199.0, "Reset", Action::Reset, true),
    ] {
        if action_button(ui, rect_in(bounds, x, 340.0, width, 36.0), text, enabled).clicked() {
            action = next;
        }
    }
    action
}

fn draw_footer(
    ui: &mut egui::Ui,
    bounds: Rect,
    source: &Source<'_>,
    feedback: &Feedback<'_>,
) -> bool {
    let status = if source.missing {
        "Missing audio. Open Import details to resolve the sound."
    } else if !source.available {
        "Select a sound in the palette to begin."
    } else {
        feedback
            .message
            .unwrap_or("Listen before saving. Save sound adds a new WAV to Effect.")
    };
    let status_rect = rect_in(bounds, PAD_X, 386.0, INNER_W, 38.0);
    status_footer(
        ui,
        status_rect,
        status,
        feedback.error || source.missing,
        source.missing,
    );
    source.missing
        && controls::button_at(
            ui,
            rect_in(bounds, 492.0, 390.0, 128.0, 30.0),
            "Import details",
            "Import details",
            controls::ButtonRole::Secondary,
            true,
        )
        .clicked()
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
    value: &mut f32,
    spec: Control<'_>,
    enabled: bool,
) {
    let [center, well, label] = positions;
    // The logarithmic dials are the frequency dials, Cutoff and Color: their
    // values show Hz, in a wider well.
    let (unit, width) = if spec.logarithmic {
        (" Hz", 72.0)
    } else {
        ("", 60.0)
    };
    ui.painter().text(
        label,
        Align2::CENTER_CENTER,
        spec.name,
        FontId::proportional(12.0),
        roles().legend,
    );
    controls::well(ui, Rect::from_center_size(well, Vec2::new(width, 20.0)));
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
                Rect::from_center_size(well, Vec2::new(width - 2.0, 24.0)),
                egui::DragValue::new(value)
                    .range(spec.low..=spec.high)
                    .clamp_existing_to_range(false)
                    .speed(spec.step)
                    .fixed_decimals(spec.decimals)
                    .suffix(unit),
            );
            name_control(response, &format!("{} value", spec.name))
                .hover_hint(format!("{}: drag, or click to enter a value.", spec.name));
        });
    });
}
fn action_button(ui: &mut egui::Ui, rect: Rect, text: &str, enabled: bool) -> Response {
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
    .disabled_hint("Select an available sound to process it.")
    .hover_hint(if text == "Save sound" {
        "Save a new WAV and add it to Effect."
    } else {
        text
    })
}
fn status_footer(ui: &mut egui::Ui, rect: Rect, text: &str, error: bool, missing: bool) {
    controls::status_well(ui, rect, error);
    let response = ui.interact(rect, ui.id().with("status"), Sense::hover());
    named(&response, egui::WidgetType::Panel, "Effect Studio status");
    let text_rect = Rect::from_min_max(
        rect.min + Vec2::new(10.0, 6.0),
        rect.max - Vec2::new(if missing { 148.0 } else { 10.0 }, 6.0),
    );
    controls::status_text(ui, text_rect, "effects-status", text, 12.5, error, text);
}
