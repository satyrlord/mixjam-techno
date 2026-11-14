//! The small controls of a mixer row: the send knob, the switch and text
//! buttons, the pan slider, and the buttons for EQ, polarity, and bypass.
use super::*;

/// A small knob for a send amount from 0 to 1: drag, the wheel or arrow keys
/// (Shift for fine steps), Home and End, or a double-click back to 0. An `automated`
/// knob is ringed: an automation lane sets its amount while the song plays.
pub(crate) fn send_knob(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    value: &mut f32,
    automated: bool,
) -> bool {
    let response = ui.interact(rect, ui.id().with(name), Sense::click_and_drag());
    let before = *value;
    let fine = ui.input(|input| input.modifiers.shift);
    if response.dragged() {
        let delta = ui.input(|input| input.pointer.delta());
        *value += (delta.x - delta.y) / 120.0 * if fine { 0.1 } else { 1.0 };
    }
    if response.double_clicked() {
        *value = 0.0;
    }
    *value += controls::wheel(ui, &response) * if fine { 0.01 } else { 0.05 };
    if response.has_focus() {
        // Space still plays and stops while the knob has focus.
        controls::keep_transport_key(ui.ctx(), response.id);
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
        let step = if fine { 0.01 } else { 0.05 };
        ui.input_mut(|input| {
            let modifiers = input.modifiers;
            if input.consume_key(modifiers, egui::Key::ArrowUp)
                || input.consume_key(modifiers, egui::Key::ArrowRight)
            {
                *value += step;
            }
            if input.consume_key(modifiers, egui::Key::ArrowDown)
                || input.consume_key(modifiers, egui::Key::ArrowLeft)
            {
                *value -= step;
            }
            if input.consume_key(modifiers, egui::Key::Home) {
                *value = 0.0;
            }
            if input.consume_key(modifiers, egui::Key::End) {
                *value = 1.0;
            }
        });
    }
    ui.input(|input| {
        use egui::accesskit::{Action, ActionData};
        for request in input.accesskit_action_requests(response.id, Action::SetValue) {
            if let Some(ActionData::NumericValue(draft)) = request.data
                && draft.is_finite()
            {
                *value = draft as f32;
            }
        }
        let turns = input.num_accesskit_action_requests(response.id, Action::Increment) as f32
            - input.num_accesskit_action_requests(response.id, Action::Decrement) as f32;
        *value += turns * 0.05;
    });
    // Keep saved precision and small fine-drag movements. Rounding belongs
    // only to the displayed percentage.
    *value = (*value).clamp(0.0, 1.0);
    let center = rect.center();
    let radius = rect.width() / 2.0;
    ui.painter()
        .circle_filled(center, radius, roles().strip_face);
    ui.painter().circle_stroke(
        center,
        radius,
        Stroke::new(
            1.0 * scale,
            if response.has_focus() {
                roles().strip_lit
            } else {
                roles().strip_slot
            },
        ),
    );
    if *value > 0.0 {
        let arc: Vec<Pos2> = (0..=24)
            .map(|step| {
                let angle = (-225.0 + 270.0 * *value * step as f32 / 24.0).to_radians();
                center + Vec2::angled(angle) * (radius - 2.0 * scale)
            })
            .collect();
        ui.painter().add(egui::Shape::line(
            arc,
            Stroke::new(2.0 * scale, roles().strip_lit),
        ));
    }
    let pointer = Vec2::angled((-225.0 + 270.0 * *value).to_radians());
    ui.painter().line_segment(
        [center, center + pointer * (radius - 3.0 * scale)],
        Stroke::new(1.5 * scale, roles().legend),
    );
    if automated {
        ui.painter().circle_stroke(
            center,
            radius + 2.0 * scale,
            Stroke::new(1.0 * scale, roles().legend),
        );
    }
    focus_ring(ui, &response, rect, scale);
    response.widget_info(|| egui::WidgetInfo::slider(response.enabled(), f64::from(*value), name));
    ui.ctx().accesskit_node_builder(response.id, |builder| {
        use egui::accesskit::Action;
        builder.set_min_numeric_value(0.0);
        builder.set_max_numeric_value(1.0);
        builder.set_numeric_value_step(0.01);
        builder.add_action(Action::SetValue);
        builder.add_action(Action::Increment);
        builder.add_action(Action::Decrement);
    });
    response.hover_hint(format!(
        "{name}: {:.0}%. Drag, or use the wheel or arrow keys; Shift for fine steps; \
         double-click for 0.{}",
        *value * 100.0,
        if automated {
            "\nAutomated: its automation lane sets it while the song plays."
        } else {
            ""
        }
    ));
    before != *value
}

/// A bus's Input and Limiter switches, named "{name} power" and
/// "{name} limiter".
pub(crate) fn bus_switches(
    ui: &mut egui::Ui,
    rects: [Rect; 2],
    scale: f32,
    name: &str,
    bus: &mut FxBus,
) {
    for (rect, label, key, on, hint) in [
        (
            rects[0],
            "Input",
            format!("{name} power"),
            &mut bus.powered,
            "Off stops new sound into the effect; its tail rings out.",
        ),
        (
            rects[1],
            "Limiter",
            format!("{name} limiter"),
            &mut bus.limiter,
            "Holds the return under -1 dBFS.",
        ),
    ] {
        let response = button(ui, rect, label, &key, scale, *on);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, response.enabled(), *on, &key)
        });
        if response
            .hover_hint(format!("{key}: {}. {hint}", if *on { "on" } else { "off" }))
            .clicked()
        {
            *on = !*on;
        }
    }
}

/// "Edit {name}": shows the bus on the FX returns board. Returns whether it
/// was pressed.
pub(crate) fn edit_button(ui: &mut egui::Ui, rect: Rect, scale: f32, name: &str) -> bool {
    button(ui, rect, "Edit", &format!("Edit {name}"), scale, false)
        .hover_hint(format!(
            "Show {name} on the FX returns board, with all its controls."
        ))
        .clicked()
}

/// "Master bypass": skips the master pedalboard. Returns whether it changed.
pub(crate) fn bypass_button(ui: &mut egui::Ui, rect: Rect, scale: f32, bypass: &mut bool) -> bool {
    let response = button(ui, rect, "Bypass", "Master bypass", scale, *bypass);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            response.enabled(),
            *bypass,
            "Master bypass",
        )
    });
    let clicked = response
        .hover_hint(format!(
            "Master bypass: {}. Bypass skips the master pedalboard, trim included, for \
             playback and export, so you can master the exported mix elsewhere. The \
             pedals keep their settings. Mastering's Bypass all is the same switch.",
            if *bypass { "on" } else { "off" }
        ))
        .clicked();
    if clicked {
        *bypass = !*bypass;
    }
    clicked
}

/// "Reset {name} gain to unity". Returns whether it was pressed.
pub(crate) fn unity_button(ui: &mut egui::Ui, rect: Rect, scale: f32, name: &str) -> bool {
    button(
        ui,
        rect,
        "Unity",
        &format!("Reset {name} gain to unity"),
        scale,
        false,
    )
    .hover_hint("Set gain to 0.0 dB (unity). Polarity stays unchanged.")
    .clicked()
}

/// "Center {name} pan". Returns whether it was pressed.
pub(crate) fn center_button(ui: &mut egui::Ui, rect: Rect, scale: f32, name: &str) -> bool {
    button(
        ui,
        rect,
        "Center",
        &format!("Center {name} pan"),
        scale,
        false,
    )
    .hover_hint("Reset pan to center. Output routing stays unchanged.")
    .clicked()
}

/// A track's pan in `rect`, styled for the strips: "{name} pan". Returns
/// whether it changed.
pub(crate) fn pan_slider(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    pan: &mut f32,
    automated: bool,
) -> bool {
    let mut changed = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().slider_width = rect.width();
        ui.spacing_mut().interact_size.y = 16.0 * scale;
        // The rail is a dark slot on every theme's strip, and the idle
        // handle keeps a cap-colored edge on it.
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_fill = roles().strip_slot;
        widgets.inactive.fg_stroke = Stroke::new(1.5 * scale, roles().cap);
        for state in [&mut widgets.hovered, &mut widgets.active] {
            state.bg_fill = roles().cap;
            state.fg_stroke = Stroke::new(1.5 * scale, roles().strip_slot);
        }
        let response = pan_control(ui, name, pan, automated);
        changed = response.changed();
        focus_ring(ui, &response, rect, scale);
    });
    changed
}

/// "{name} EQ", lit while `on`: opens the channel EQ window. Returns whether
/// it was pressed.
pub(crate) fn eq_button(ui: &mut egui::Ui, rect: Rect, scale: f32, name: &str, on: bool) -> bool {
    let eq_name = format!("{name} EQ");
    let response = button(ui, rect, "EQ", &eq_name, scale, on);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, response.enabled(), on, &eq_name)
    });
    response
        .hover_hint(format!(
            "{name} channel processing: {}. Open the channel EQ window: low and high cuts, \
             shelves, and two peaks; the drive, which adds harmonics; the compressor and the \
             stereo width, before the track's automation and fader; and the ducker, which \
             lowers the track while another track sounds.",
            if on { "on" } else { "off" }
        ))
        .clicked()
}

/// "{name} polarity invert": a slashed circle, amber while inverted. Returns
/// whether it changed.
pub(crate) fn polarity_button(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    inverted: &mut bool,
) -> bool {
    let response = button(
        ui,
        rect,
        "",
        &format!("{name} polarity invert"),
        scale,
        *inverted,
    );
    let center = rect.center();
    let color = if *inverted {
        roles().ink
    } else {
        roles().legend
    };
    // The strip's 6-point circle, smaller in a short row.
    let radius = (rect.height() / 2.0 - 2.0).min(6.0 * scale);
    let unit = radius / 6.0;
    ui.painter()
        .circle_stroke(center, radius, Stroke::new(1.5 * scale, color));
    ui.painter().line_segment(
        [
            center + Vec2::new(-5.0, 7.0) * unit,
            center + Vec2::new(5.0, -7.0) * unit,
        ],
        Stroke::new(1.5 * scale, color),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            *inverted,
            format!("{name} polarity invert"),
        )
    });
    let clicked = response
        .hover_hint(format!(
            "{name} polarity: {}. Invert the sign of both output channels without changing \
             gain or pan. Negative dB values only reduce level.",
            if *inverted { "inverted" } else { "normal" }
        ))
        .clicked();
    if clicked {
        *inverted = !*inverted;
    }
    clicked
}

pub(super) fn text(ui: &egui::Ui, pos: Pos2, value: &str, size: f32, color: Color32) {
    ui.painter().text(
        pos,
        Align2::CENTER_CENTER,
        value,
        FontId::proportional(size),
        color,
    );
}

fn button(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    name: &str,
    scale: f32,
    selected: bool,
) -> Response {
    let response = ui.interact(rect, ui.id().with(name), Sense::click());
    let fill = if selected {
        roles().strip_lit
    } else if response.hovered() {
        roles().raised
    } else {
        roles().strip_face
    };
    ui.painter().rect_filled(rect, 2.0, fill);
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(
            1.0 * scale,
            if response.has_focus() {
                roles().strip_lit
            } else {
                roles().strip_slot
            },
        ),
        egui::StrokeKind::Inside,
    );
    focus_ring(ui, &response, rect, scale);
    text(
        ui,
        rect.center(),
        label,
        10.0 * scale,
        if selected {
            roles().ink
        } else {
            roles().legend
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), name)
    });
    response
}

/// Amber is illegible on Vintage's ivory, so each theme names the ring color
/// that reads on its strips; amber remains the inner edge on dark faces.
pub(super) fn focus_ring(ui: &egui::Ui, response: &Response, rect: Rect, scale: f32) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(1.0 * scale),
            3.0,
            Stroke::new(2.0 * scale, roles().strip_focus),
            egui::StrokeKind::Outside,
        );
    }
}

pub(crate) fn switch_button(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    switch: TrackSwitch,
    enabled: &mut bool,
) -> bool {
    let label = format!("{name} {}", switch.name());
    let response = button(ui, rect, "", &label, scale, false);
    ui.painter().rect_filled(
        Rect::from_center_size(
            rect.center_top() + Vec2::new(0.0, 5.0) * scale,
            Vec2::new(12.0, 2.0) * scale,
        ),
        1.0,
        if *enabled {
            roles().accent
        } else {
            roles().muted
        },
    );
    text(
        ui,
        rect.center() + Vec2::new(0.0, 3.0) * scale,
        switch.letter(),
        11.0 * scale,
        if *enabled {
            roles().accent
        } else {
            roles().muted
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            response.enabled(),
            *enabled,
            &label,
        )
    });
    let clicked = response
        .hover_hint(switch_hint(&label, switch, *enabled))
        .clicked();
    if clicked {
        *enabled = !*enabled;
    }
    clicked
}
