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
    send_knob_in(ui, rect, scale, name, value, automated, false)
}

/// The Mix view's send knob: at 0, not automated, and not hovered or
/// focused, it rests as a thin ring with its pointer, so only the sends that
/// are in use show a head. Name, keys, and hint match `send_knob`.
pub(crate) fn quiet_send_knob(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    value: &mut f32,
    automated: bool,
) -> bool {
    send_knob_in(ui, rect, 1.0, name, value, automated, true)
}

fn send_knob_in(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    value: &mut f32,
    automated: bool,
    quiet: bool,
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
    let idle = quiet
        && *value == 0.0
        && !automated
        && !response.hovered()
        && !response.has_focus()
        && !response.dragged();
    if idle {
        let ink = roles().strip_text.gamma_multiply(0.45);
        ui.painter()
            .circle_stroke(center, radius - 1.0, Stroke::new(1.0 * scale, ink));
        let pointer = Vec2::angled((-225.0_f32).to_radians());
        ui.painter().line_segment(
            [
                center + pointer * (radius * 0.35),
                center + pointer * (radius - 3.0 * scale),
            ],
            Stroke::new(1.0 * scale, ink),
        );
    } else {
        send_knob_face(ui, &response, center, radius, scale, *value, automated);
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

/// A send knob's head: face, ring, amount arc, pointer, and the automation
/// ring.
fn send_knob_face(
    ui: &egui::Ui,
    response: &Response,
    center: Pos2,
    radius: f32,
    scale: f32,
    value: f32,
    automated: bool,
) {
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
    if value > 0.0 {
        let arc: Vec<Pos2> = (0..=24)
            .map(|step| {
                let angle = (-225.0 + 270.0 * value * step as f32 / 24.0).to_radians();
                center + Vec2::angled(angle) * (radius - 2.0 * scale)
            })
            .collect();
        ui.painter().add(egui::Shape::line(
            arc,
            Stroke::new(2.0 * scale, roles().strip_lit),
        ));
    }
    let pointer = Vec2::angled((-225.0 + 270.0 * value).to_radians());
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
    bus_switches_in(ui, rects, scale, name, bus, false);
}

/// The Mix view's Input and Limiter: on, their usual state, is a quiet
/// label with a small lamp; off, the exception, is a warm label. Names,
/// states, and hints match `bus_switches`.
pub(crate) fn quiet_bus_switches(ui: &mut egui::Ui, rects: [Rect; 2], name: &str, bus: &mut FxBus) {
    bus_switches_in(ui, rects, 1.0, name, bus, true);
}

fn bus_switches_in(
    ui: &mut egui::Ui,
    rects: [Rect; 2],
    scale: f32,
    name: &str,
    bus: &mut FxBus,
    quiet: bool,
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
        let response = if quiet {
            let response = face(ui, rect, &key, scale, None);
            let lamp = Pos2::new(rect.left() + 6.0 * scale, rect.center().y);
            ui.painter().circle_filled(
                lamp,
                2.0 * scale,
                if *on {
                    roles().lamp_on
                } else {
                    roles().lamp_off
                },
            );
            text(
                ui,
                rect.center() + Vec2::new(4.0, 0.0) * scale,
                label,
                10.0 * scale,
                if *on {
                    roles().muted
                } else {
                    roles().lamp_warn
                },
            );
            response
        } else {
            button(ui, rect, label, &key, scale, *on)
        };
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

/// The Mix view's Unity: a reset that shows only on hover or focus. The
/// name and hint are `unity_button`'s.
pub(crate) fn quiet_unity_button(ui: &mut egui::Ui, rect: Rect, name: &str) -> bool {
    reset_button(ui, rect, "Unity", &format!("Reset {name} gain to unity"))
        .hover_hint("Set gain to 0.0 dB (unity). Polarity stays unchanged.")
        .clicked()
}

/// The Mix view's Center, quiet like `quiet_unity_button`.
pub(crate) fn quiet_center_button(ui: &mut egui::Ui, rect: Rect, name: &str) -> bool {
    reset_button(ui, rect, "Center", &format!("Center {name} pan"))
        .hover_hint("Reset pan to center. Output routing stays unchanged.")
        .clicked()
}

/// A reset with no face and no label at rest. The dB value and the pan state
/// already say when a value is away from its default, so the label and a
/// face show only on hover or focus. The reset keeps its Tab stop and name.
fn reset_button(ui: &mut egui::Ui, rect: Rect, label: &str, name: &str) -> Response {
    let response = ui.interact(rect, ui.id().with(name), Sense::click());
    let active = response.hovered() || response.has_focus();
    if active {
        ui.painter().rect_filled(rect, 2.0, roles().strip_face);
    }
    if active {
        text(ui, rect.center(), label, 10.0, roles().legend);
    }
    focus_ring(ui, &response, rect, 1.0);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), name)
    });
    response
}

/// The Mix view's M, S, L, and R. Each rests as a quiet letter; Mute and
/// Solo fill their faces when on, and an output that is off turns its letter
/// warm, so a row shows only what differs from a plain track. Names, states,
/// and hints match `switch_button`.
pub(crate) fn quiet_switch(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    switch: TrackSwitch,
    enabled: &mut bool,
) -> bool {
    let label = format!("{name} {}", switch.name());
    let (fill, letter) = match switch {
        TrackSwitch::Mute if *enabled => (Some(roles().coral), roles().ink),
        TrackSwitch::Solo if *enabled => (Some(roles().accent), roles().ink),
        TrackSwitch::Left | TrackSwitch::Right if !*enabled => (None, roles().lamp_warn),
        _ => (None, roles().muted),
    };
    let response = face(ui, rect, &label, 1.0, fill);
    text(ui, rect.center(), switch.letter(), 11.0, letter);
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
    let roles = roles();
    pan_slider_in(
        ui,
        rect,
        scale,
        name,
        pan,
        automated,
        (roles.strip_slot, roles.cap, roles.strip_slot),
    )
}

/// The Mix view's pan: its slot is the strip's rail well, like its fader,
/// and its handle keeps a ring of strip ink at rest and in use, so it shows
/// on Vintage's cream well.
pub(crate) fn well_pan_slider(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    pan: &mut f32,
    automated: bool,
) -> bool {
    let roles = roles();
    pan_slider_in(
        ui,
        rect,
        1.0,
        name,
        pan,
        automated,
        (roles.strip_well, roles.strip_text, roles.strip_text),
    )
}

fn pan_slider_in(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    pan: &mut f32,
    automated: bool,
    (slot, ring, active_ring): (Color32, Color32, Color32),
) -> bool {
    let mut changed = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().slider_width = rect.width();
        ui.spacing_mut().interact_size.y = 16.0 * scale;
        // The rail is a dark slot on every theme's strip, and the idle
        // handle keeps a cap-colored edge on it.
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_fill = slot;
        widgets.inactive.fg_stroke = Stroke::new(1.5 * scale, ring);
        for state in [&mut widgets.hovered, &mut widgets.active] {
            state.bg_fill = roles().cap;
            state.fg_stroke = Stroke::new(1.5 * scale, active_ring);
        }
        let response = pan_control(ui, name, pan, automated);
        changed = response.changed();
        // The send knob's ring, so an automated pan reads the same way.
        if automated {
            ui.painter().rect_stroke(
                rect.expand(2.0 * scale),
                3.0,
                Stroke::new(1.0 * scale, roles().legend),
                egui::StrokeKind::Outside,
            );
        }
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
    let color = if *inverted {
        roles().ink
    } else {
        roles().legend
    };
    polarity_glyph(ui, rect, scale, 1.5, color);
    polarity_response(ui, response, name, inverted)
}

/// The Mix view's polarity: a Muted glyph on a dark face, like the quiet
/// M, S, L, and R, and a lit face only while inverted. Name, state, and hint
/// match `polarity_button`.
pub(crate) fn quiet_polarity_button(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    inverted: &mut bool,
) -> bool {
    let response = face(
        ui,
        rect,
        &format!("{name} polarity invert"),
        1.0,
        inverted.then(|| roles().strip_lit),
    );
    let color = if *inverted {
        roles().ink
    } else {
        roles().muted
    };
    polarity_glyph(ui, rect, 1.0, 1.0, color);
    polarity_response(ui, response, name, inverted)
}

/// The strip's 6-point slashed circle, smaller in a short row.
fn polarity_glyph(ui: &egui::Ui, rect: Rect, scale: f32, width: f32, color: Color32) {
    let center = rect.center();
    let radius = (rect.height() / 2.0 - 2.0).min(6.0 * scale);
    let unit = radius / 6.0;
    let stroke = Stroke::new(width * scale, color);
    ui.painter().circle_stroke(center, radius, stroke);
    ui.painter().line_segment(
        [
            center + Vec2::new(-5.0, 7.0) * unit,
            center + Vec2::new(5.0, -7.0) * unit,
        ],
        stroke,
    );
}

/// The polarity switch's name, state, hint, and toggle.
fn polarity_response(ui: &egui::Ui, response: Response, name: &str, inverted: &mut bool) -> bool {
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
    let response = face(ui, rect, name, scale, selected.then(|| roles().strip_lit));
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

/// A strip button's face, named `name`: `lit` fills it, else it rests dark
/// and lifts on hover.
fn face(ui: &mut egui::Ui, rect: Rect, name: &str, scale: f32, lit: Option<Color32>) -> Response {
    let response = ui.interact(rect, ui.id().with(name), Sense::click());
    let fill = lit.unwrap_or(if response.hovered() {
        roles().raised
    } else {
        roles().strip_face
    });
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
