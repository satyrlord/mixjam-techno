//! Horizontal sliders: the continuous slider and the stepped slider over
//! fixed choices.
use super::*;

pub(crate) fn slider(
    ui: &mut egui::Ui,
    value: &mut f64,
    range: RangeInclusive<f64>,
    name: &str,
) -> Response {
    ui.scope(|ui| {
        let (low, high) = (*range.start(), *range.end());
        // egui uses inactive.bg_fill for both the rail and idle handle. Keep
        // its native rail and interaction, then paint the shared silver cap.
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_fill = roles().ink;
        for visual in [
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
        ] {
            visual.bg_stroke = Stroke::new(1.0_f32, roles().ink);
            visual.fg_stroke = Stroke::new(1.0_f32, roles().ink);
        }
        widgets.hovered.bg_fill = roles().cap;
        widgets.active.bg_fill = roles().cap;
        let response = ui.add(
            egui::Slider::new(value, range)
                .show_value(false)
                .step_by(0.1)
                .clamping(egui::SliderClamping::Edits),
        );
        let radius = response.rect.height() / 2.5;
        let fraction = ((*value - low) / (high - low)).clamp(0.0, 1.0) as f32;
        let center = Pos2::new(
            egui::lerp(
                (response.rect.left() + radius)..=(response.rect.right() - radius),
                fraction,
            ),
            response.rect.center().y,
        );
        ui.painter().circle_filled(center, radius, roles().cap);
        ui.painter()
            .circle_stroke(center, radius, Stroke::new(1.5_f32, roles().ink));
        if response.has_focus() {
            ui.painter()
                .circle_stroke(center, radius + 2.0, Stroke::new(2.0_f32, focus_color(ui)));
        }
        response.widget_info(|| egui::WidgetInfo::slider(response.enabled(), *value, name));
        response
    })
    .inner
}

/// A stepped slider over fixed choices, after the original products' speed
/// slider. Notches sit at even spacing whatever their values. With `labels`, each notch is
/// named below the rail; otherwise the value reads at the right end.
pub(crate) fn stepped_slider(
    ui: &mut egui::Ui,
    rect: Rect,
    value: &mut f64,
    choices: &[f64],
    name: &str,
    labels: bool,
    reset: Option<f64>,
) -> Response {
    let mut response = ui.interact(
        rect,
        ui.id().with(("stepped_slider", name)),
        Sense::click_and_drag(),
    );
    let before = *value;
    let nearest = |target: f64| {
        (0..choices.len())
            .min_by(|a, b| {
                (choices[*a] - target)
                    .abs()
                    .total_cmp(&(choices[*b] - target).abs())
            })
            .unwrap_or(0)
    };
    let last = choices.len().saturating_sub(1);
    let radius = 7.0;
    let readout = if labels { 0.0 } else { 24.0 };
    let left = rect.left() + radius + 1.0;
    let right = rect.right() - readout - radius - 1.0;
    let rail_y = if labels {
        rect.top() + radius + 3.0
    } else {
        rect.center().y
    };
    let x_of = |index: usize| egui::lerp(left..=right, index as f32 / last.max(1) as f32);
    if response.enabled() && !choices.is_empty() {
        let mut index = nearest(*value);
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
        if response.is_pointer_button_down_on() || response.clicked() {
            response.request_focus();
            if let Some(pointer) = response.interact_pointer_pos() {
                let fraction = ((pointer.x - left) / (right - left).max(1.0)).clamp(0.0, 1.0);
                index = (fraction * last as f32).round() as usize;
            }
        }
        let mut direction = wheel(ui, &response) as i32;
        if response.has_focus() {
            ui.input_mut(|input| {
                let modifiers = input.modifiers;
                for key in [egui::Key::ArrowUp, egui::Key::ArrowRight] {
                    if input.consume_key(modifiers, key) {
                        direction += 1;
                    }
                }
                for key in [egui::Key::ArrowDown, egui::Key::ArrowLeft] {
                    if input.consume_key(modifiers, key) {
                        direction -= 1;
                    }
                }
                if input.consume_key(modifiers, egui::Key::Home) {
                    index = 0;
                }
                if input.consume_key(modifiers, egui::Key::End) {
                    index = last;
                }
            });
        }
        ui.input(|input| {
            use egui::accesskit::{Action, ActionData};
            for request in input.accesskit_action_requests(response.id, Action::SetValue) {
                if let Some(ActionData::NumericValue(draft)) = request.data
                    && draft.is_finite()
                {
                    index = nearest(draft);
                }
            }
            direction += input.num_accesskit_action_requests(response.id, Action::Increment) as i32
                - input.num_accesskit_action_requests(response.id, Action::Decrement) as i32;
        });
        if direction != 0 && response.has_focus() {
            // Arrow keys step the slider; they must not also move focus.
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        index = (index as i32 + direction).clamp(0, last as i32) as usize;
        if index != nearest(before) {
            *value = choices[index];
        }
        if let Some(reset) = reset
            && response.double_clicked()
        {
            *value = choices[nearest(reset)];
        }
    }
    if before != *value {
        response.mark_changed();
    }

    // Sliders sit on dark panels; a caller marks a light surface by turning
    // `dark_mode` off, and the scale then uses ink, as dials do.
    let mark = if ui.visuals().dark_mode {
        roles().legend
    } else {
        roles().ink
    };
    let painter = ui.painter();
    // The rail is a dark slot with a well edge, so it reads on dark panels.
    painter.rect(
        Rect::from_min_max(
            Pos2::new(left - 3.0, rail_y - 2.5),
            Pos2::new(right + 3.0, rail_y + 2.5),
        ),
        2.5,
        roles().ink,
        Stroke::new(1.0_f32, roles().well_edge),
        egui::StrokeKind::Outside,
    );
    let current = nearest(*value);
    for (index, choice) in choices.iter().enumerate() {
        let x = x_of(index);
        painter.line_segment(
            [Pos2::new(x, rail_y - 6.0), Pos2::new(x, rail_y + 6.0)],
            Stroke::new(1.0_f32, mark.gamma_multiply(0.7)),
        );
        if labels {
            painter.text(
                Pos2::new(x, rail_y + radius + 4.0),
                egui::Align2::CENTER_TOP,
                format!("{choice:.0}"),
                FontId::proportional(11.0),
                if index == current {
                    mark
                } else {
                    mark.gamma_multiply(0.6)
                },
            );
        }
    }
    if !labels {
        painter.text(
            Pos2::new(rect.right(), rect.center().y),
            egui::Align2::RIGHT_CENTER,
            format!("{value:.0}"),
            FontId::proportional(13.0),
            mark,
        );
    }
    let cap = Pos2::new(x_of(current), rail_y);
    painter.circle_filled(cap, radius, roles().cap);
    painter.circle_stroke(cap, radius, Stroke::new(1.5_f32, roles().ink));
    if !response.enabled() {
        painter.rect_filled(rect, 2.0, Color32::from_black_alpha(100));
    } else if response.has_focus() {
        painter.circle_stroke(cap, radius + 2.0, Stroke::new(2.0_f32, focus_color(ui)));
    } else if response.hovered() {
        painter.circle_stroke(cap, radius + 1.0, Stroke::new(1.0_f32, mark));
    }
    response.widget_info(|| egui::WidgetInfo::slider(response.enabled(), *value, name));
    ui.ctx().accesskit_node_builder(response.id, |builder| {
        use egui::accesskit::Action;
        if let (Some(low), Some(high)) = (choices.first(), choices.last()) {
            builder.set_min_numeric_value(*low);
            builder.set_max_numeric_value(*high);
        }
        if response.enabled() {
            builder.add_action(Action::SetValue);
            if current < last {
                builder.add_action(Action::Increment);
            }
            if current > 0 {
                builder.add_action(Action::Decrement);
            }
        }
    });
    let choices = choices
        .iter()
        .map(|choice| format!("{choice:.0}"))
        .collect::<Vec<_>>()
        .join(", ");
    response.hover_hint(match reset {
        Some(reset) => format!(
            "{name}: {choices}. Drag, click a notch, or use the arrow keys. Double-click to reset \
             to {reset:.0}."
        ),
        None => format!("{name}: {choices}. Drag, click a notch, or use the arrow keys."),
    })
}
