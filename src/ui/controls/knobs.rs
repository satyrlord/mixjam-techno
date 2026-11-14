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
        if response.has_focus() {
            keep_transport_key(ui.ctx(), response.id);
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
        }
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        let draft_id = response.id.with("draft");
        if response.drag_started() {
            ui.ctx()
                .data_mut(|data| data.insert_temp(draft_id, spec.fraction(before)));
        }
        if response.dragged() {
            let draft = ui
                .ctx()
                .data(|data| data.get_temp::<f64>(draft_id))
                .unwrap_or_else(|| spec.fraction(before));
            let delta = ui.input(|input| input.pointer.delta());
            let fine = ui.input(|input| input.modifiers.shift);
            let draft = (draft
                + f64::from((delta.x - delta.y) / 150.0) * if fine { 0.1 } else { 1.0 })
            .clamp(0.0, 1.0);
            ui.ctx().data_mut(|data| data.insert_temp(draft_id, draft));
            let precision = 10.0_f64.powi(spec.decimals as i32);
            *value = spec.bounded((spec.value(draft) * precision).round() / precision);
        }
        let mut direction = f64::from(wheel(ui, &response));
        if response.has_focus() {
            ui.input_mut(|input| {
                let modifiers = input.modifiers;
                if input.consume_key(modifiers, egui::Key::ArrowUp)
                    || input.consume_key(modifiers, egui::Key::ArrowRight)
                {
                    direction = 1.0;
                }
                if input.consume_key(modifiers, egui::Key::ArrowDown)
                    || input.consume_key(modifiers, egui::Key::ArrowLeft)
                {
                    direction = -1.0;
                }
                if input.consume_key(modifiers, egui::Key::Home) {
                    *value = spec.low;
                }
                if input.consume_key(modifiers, egui::Key::End) {
                    *value = spec.high;
                }
            });
        }
        if direction != 0.0 {
            if response.has_focus() {
                ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
            }
            *value = spec.stepped(*value, direction, ui.input(|input| input.modifiers.shift));
        }
        ui.input(|input| {
            use egui::accesskit::{Action, ActionData};
            for request in input.accesskit_action_requests(response.id, Action::SetValue) {
                if let Some(ActionData::NumericValue(draft)) = request.data
                    && draft.is_finite()
                {
                    *value = spec.bounded(draft);
                }
            }
            let turns = input.num_accesskit_action_requests(response.id, Action::Increment) as i32
                - input.num_accesskit_action_requests(response.id, Action::Decrement) as i32;
            for _ in 0..turns.abs() {
                *value = spec.stepped(*value, f64::from(turns.signum()), false);
            }
        });
        if response.double_clicked() {
            *value = spec.bounded(spec.reset);
        }
    }
    if before != *value {
        response.mark_changed();
    }
    let center = rect.center();
    // Every theme's knob has the 18-point face of the bitmap knob.
    let radius = 18.0;
    // Dials sit in dark bays. On a light painted surface, which a caller marks
    // by turning `dark_mode` off, the scale and focus ring use ink instead.
    let on_light = !ui.visuals().dark_mode;
    let fraction = spec.fraction(*value) as f32;
    // Studio's value arc replaces the printed scale.
    if roles().material != Material::Flat {
        for index in 0..=18 {
            let vector = Vec2::angled((-225.0 + index as f32 * 15.0).to_radians());
            ui.painter().line_segment(
                [
                    center + vector * (radius + 2.0),
                    center + vector * (radius + if index % 3 == 0 { 6.0 } else { 4.0 }),
                ],
                Stroke::new(
                    1.0_f32,
                    if on_light {
                        roles().ink
                    } else {
                        roles().legend
                    },
                ),
            );
        }
    }
    if roles().material == Material::Metal {
        let art = artwork(ui.ctx(), Palette::of(ui));
        let ppp = ui.ctx().pixels_per_point();
        let size = art.knob.size_vec2() / ppp;
        let min = ((center - size / 2.0).to_vec2() * ppp).round().to_pos2() / ppp;
        ui.painter().image(
            art.knob.id(),
            Rect::from_min_size(min, size),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        let vector = Vec2::angled((-225.0 + fraction * 270.0).to_radians());
        ui.painter().line_segment(
            [
                center + vector * radius * 0.2,
                center + vector * radius * 0.62,
            ],
            Stroke::new(2.0_f32, roles().ink),
        );
    } else {
        theme::dial(
            ui.painter(),
            center,
            radius,
            fraction,
            print_color(ui),
            on_light,
        );
    }
    if !response.enabled() {
        ui.painter()
            .circle_filled(center, radius + 1.0, Color32::from_black_alpha(100));
    } else if response.has_focus() || response.hovered() {
        ui.painter().circle_stroke(
            center,
            radius + 1.0,
            Stroke::new(
                if response.has_focus() {
                    2.0_f32
                } else {
                    1.0_f32
                },
                // In a dark bay every accent is legible; on paint, ink is.
                if on_light {
                    roles().ink
                } else {
                    print_color(ui)
                },
            ),
        );
    }
    response.widget_info(|| egui::WidgetInfo::slider(response.enabled(), *value, spec.name));
    ui.ctx().accesskit_node_builder(response.id, |builder| {
        use egui::accesskit::Action;
        builder.set_min_numeric_value(spec.low);
        builder.set_max_numeric_value(spec.high);
        builder.set_numeric_value_step(spec.step);
        if response.enabled() {
            builder.add_action(Action::SetValue);
            if *value < spec.high {
                builder.add_action(Action::Increment);
            }
            if *value > spec.low {
                builder.add_action(Action::Decrement);
            }
        }
    });
    response.hover_hint(format!(
        "{}: drag or use arrow keys. Hold Shift for fine movement. Double-click to reset.",
        spec.name
    ))
}
