//! The rotary knob: its range and stepping, the mouse wheel, and the control
//! with its pointer, keyboard, and accessibility behavior.
use super::*;

#[derive(Clone, Copy)]
pub(crate) struct RotarySpec<'a> {
    pub name: &'a str,
    pub low: f64,
    pub high: f64,
    pub reset: f64,
    pub step: f64,
    pub decimals: usize,
    pub logarithmic: bool,
    pub choices: &'a [f64],
}

impl RotarySpec<'_> {
    fn fraction(self, value: f64) -> f64 {
        let value = value.clamp(self.low, self.high);
        if self.logarithmic {
            (value / self.low).ln() / (self.high / self.low).ln()
        } else {
            (value - self.low) / (self.high - self.low)
        }
    }

    fn value(self, fraction: f64) -> f64 {
        let fraction = fraction.clamp(0.0, 1.0);
        if self.logarithmic {
            self.low * (self.high / self.low).powf(fraction)
        } else {
            self.low + fraction * (self.high - self.low)
        }
    }

    pub(super) fn bounded(self, value: f64) -> f64 {
        let value = value.clamp(self.low, self.high);
        self.choices
            .iter()
            .copied()
            .min_by(|a, b| (a - value).abs().total_cmp(&(b - value).abs()))
            .unwrap_or(value)
    }

    pub(super) fn stepped(self, value: f64, direction: f64, fine: bool) -> f64 {
        if direction == 0.0 {
            return value;
        }
        if !self.choices.is_empty() {
            return if direction > 0.0 {
                self.choices
                    .iter()
                    .copied()
                    .find(|choice| *choice > value)
                    .unwrap_or(self.high)
            } else {
                self.choices
                    .iter()
                    .copied()
                    .rev()
                    .find(|choice| *choice < value)
                    .unwrap_or(self.low)
            };
        }
        let amount = direction * if fine { 0.1 } else { 1.0 };
        if !self.logarithmic {
            return (value + amount * self.step).clamp(self.low, self.high);
        }
        let next = self.value(self.fraction(value) + amount * 0.01);
        if fine {
            return next;
        }
        let precision = 10.0_f64.powi(self.decimals as i32);
        let next = (next * precision).round() / precision;
        if (next - value).abs() < self.step {
            (value + direction * self.step).clamp(self.low, self.high)
        } else {
            next.clamp(self.low, self.high)
        }
    }
}

/// The wheel's direction over a hovered control: 1 up, -1 down, 0 for none.
/// The control takes the wheel, so a scroll area behind it stays put. Shift
/// turns the wheel sideways, so both axes count.
pub(crate) fn wheel(ui: &egui::Ui, response: &Response) -> f32 {
    if !response.hovered() {
        return 0.0;
    }
    ui.input_mut(|input| {
        let delta = input.raw_scroll_delta.y + input.raw_scroll_delta.x;
        input.raw_scroll_delta = Vec2::ZERO;
        input.smooth_scroll_delta = Vec2::ZERO;
        if delta.abs() > 0.0 {
            delta.signum()
        } else {
            0.0
        }
    })
}

fn rotary_apply_drag(
    ui: &egui::Ui,
    draft_id: egui::Id,
    value: &mut f64,
    spec: &RotarySpec<'_>,
    before: f64,
) {
    let draft = ui
        .ctx()
        .data(|data| data.get_temp::<f64>(draft_id))
        .unwrap_or_else(|| spec.fraction(before));
    let (delta, shift) = ui.input(|i| (i.pointer.delta(), i.modifiers.shift));
    let scale = if shift { 0.1 } else { 1.0 };
    let draft = (draft + f64::from((delta.x - delta.y) / 150.0) * scale).clamp(0.0, 1.0);
    ui.ctx().data_mut(|data| data.insert_temp(draft_id, draft));
    let precision = 10.0_f64.powi(spec.decimals as i32);
    *value = spec.bounded((spec.value(draft) * precision).round() / precision);
}

fn rotary_drag(
    ui: &egui::Ui,
    response: &Response,
    value: &mut f64,
    spec: &RotarySpec<'_>,
    before: f64,
) {
    let start = super::primary_drag_start(ui, response);
    if response.clicked() || start.is_some() {
        response.request_focus();
    }
    let draft_id = response.id.with("draft");
    if start.is_some() {
        ui.ctx()
            .data_mut(|data| data.insert_temp(draft_id, spec.fraction(before)));
    }
    if super::primary_dragged(ui, response) {
        rotary_apply_drag(ui, draft_id, value, spec, before);
    }
}

fn rotary_keys(ui: &egui::Ui, direction: &mut f64, value: &mut f64, spec: &RotarySpec<'_>) {
    ui.input_mut(|input| {
        let m = input.modifiers;
        let inc =
            input.consume_key(m, egui::Key::ArrowUp) || input.consume_key(m, egui::Key::ArrowRight);
        let dec = input.consume_key(m, egui::Key::ArrowDown)
            || input.consume_key(m, egui::Key::ArrowLeft);
        if inc {
            *direction = 1.0;
        } else if dec {
            *direction = -1.0;
        }
        if input.consume_key(m, egui::Key::Home) {
            *value = spec.low;
        }
        if input.consume_key(m, egui::Key::End) {
            *value = spec.high;
        }
    });
}

fn numeric_action_value(req: &egui::accesskit::ActionRequest) -> Option<f64> {
    match req.data {
        Some(egui::accesskit::ActionData::NumericValue(v)) if v.is_finite() => Some(v),
        _ => None,
    }
}

fn rotary_accesskit_input(ui: &egui::Ui, id: egui::Id, value: &mut f64, spec: &RotarySpec<'_>) {
    let (target, turns) = ui.input(|input| {
        use egui::accesskit::Action;
        let target = input
            .accesskit_action_requests(id, Action::SetValue)
            .filter_map(numeric_action_value)
            .last();
        let turns = input.num_accesskit_action_requests(id, Action::Increment) as i32
            - input.num_accesskit_action_requests(id, Action::Decrement) as i32;
        (target, turns)
    });
    if let Some(draft) = target {
        *value = spec.bounded(draft);
    }
    for _ in 0..turns.abs() {
        *value = spec.stepped(*value, f64::from(turns.signum()), false);
    }
}

fn rotary_ring(ui: &egui::Ui, response: &Response, center: Pos2, radius: f32, on_light: bool) {
    if !response.enabled() {
        ui.painter()
            .circle_filled(center, radius + 1.0, Color32::from_black_alpha(100));
    } else if response.has_focus() || response.hovered() {
        let width = if response.has_focus() {
            2.0_f32
        } else {
            1.0_f32
        };
        // In a dark bay every accent is legible; on paint, ink is.
        let ring = if on_light {
            roles().ink
        } else {
            print_color(ui)
        };
        ui.painter()
            .circle_stroke(center, radius + 1.0, Stroke::new(width, ring));
    }
}

fn rotary_paint(ui: &egui::Ui, response: &Response, rect: Rect, fraction: f32) {
    let center = rect.center();
    // Every theme's knob has the 18-point face of the bitmap knob.
    let radius = 18.0;
    // Dials sit in dark bays. On a light painted surface, which a caller marks
    // by turning `dark_mode` off, the scale and focus ring use ink instead.
    let on_light = !ui.visuals().dark_mode;
    // Studio's value arc replaces the printed scale.
    if roles().material != Material::Flat {
        let tick_color = if on_light {
            roles().ink
        } else {
            roles().legend
        };
        for index in 0..=18 {
            let vector = Vec2::angled((-225.0 + index as f32 * 15.0).to_radians());
            let tick_len = if index % 3 == 0 { 6.0 } else { 4.0 };
            ui.painter().line_segment(
                [
                    center + vector * (radius + 2.0),
                    center + vector * (radius + tick_len),
                ],
                Stroke::new(1.0_f32, tick_color),
            );
        }
    }
    theme::dial(
        ui.painter(),
        center,
        radius,
        fraction,
        print_color(ui),
        on_light,
    );
    rotary_ring(ui, response, center, radius, on_light);
}

fn rotary_accesskit_node(ui: &egui::Ui, response: &Response, value: f64, spec: &RotarySpec<'_>) {
    ui.ctx().accesskit_node_builder(response.id, |builder| {
        use egui::accesskit::Action;
        builder.set_min_numeric_value(spec.low);
        builder.set_max_numeric_value(spec.high);
        builder.set_numeric_value_step(spec.step);
        if response.enabled() {
            builder.add_action(Action::SetValue);
            if value < spec.high {
                builder.add_action(Action::Increment);
            }
            if value > spec.low {
                builder.add_action(Action::Decrement);
            }
        }
    });
}

fn rotary_step_direction(
    ui: &egui::Ui,
    has_focus: bool,
    direction: f64,
    value: &mut f64,
    spec: &RotarySpec<'_>,
) {
    if direction != 0.0 {
        if has_focus {
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        *value = spec.stepped(*value, direction, ui.input(|i| i.modifiers.shift));
    }
}

fn rotary_interact(
    ui: &egui::Ui,
    response: &Response,
    value: &mut f64,
    spec: &RotarySpec<'_>,
    before: f64,
) {
    let focused = response.has_focus();
    if focused {
        super::lock_arrows(ui, response.id);
    }
    rotary_drag(ui, response, value, spec, before);
    let mut direction = f64::from(wheel(ui, response));
    if focused {
        rotary_keys(ui, &mut direction, value, spec);
    }
    rotary_step_direction(ui, focused, direction, value, spec);
    rotary_accesskit_input(ui, response.id, value, spec);
    if response.double_clicked() {
        *value = spec.bounded(spec.reset);
    }
}

pub(crate) fn rotary(
    ui: &mut egui::Ui,
    rect: Rect,
    value: &mut f64,
    spec: RotarySpec<'_>,
) -> Response {
    let mut response = ui.interact(
        rect,
        ui.id().with(("rotary", spec.name)),
        Sense::click_and_drag(),
    );
    let before = *value;
    if response.enabled() {
        rotary_interact(ui, &response, value, &spec, before);
    }
    if before != *value {
        response.mark_changed();
    }
    rotary_paint(ui, &response, rect, spec.fraction(*value) as f32);
    response.widget_info(|| egui::WidgetInfo::slider(response.enabled(), *value, spec.name));
    rotary_accesskit_node(ui, &response, *value, &spec);
    response.hover_hint(format!(
        "{}: drag or use arrow keys. Hold Shift for fine movement. Double-click to reset.",
        spec.name
    ))
}
