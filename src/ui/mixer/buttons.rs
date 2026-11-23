//! The small controls of a mixer row: the send knob, the switch and text
//! buttons, the pan slider, and the buttons for EQ, polarity, and bypass.
use super::*;

/// How a mixer control is drawn: on a strip at its artwork's scale, with
/// faces and lit labels; or quiet in a Mix view row, where a control at its
/// usual state recedes. The name, keys, state, and hint are the same in
/// both, so a test or the agent channel finds the control either way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Look {
    Strip(f32),
    Quiet,
}

impl Look {
    pub(crate) fn scale(self) -> f32 {
        match self {
            Self::Strip(scale) => scale,
            Self::Quiet => 1.0,
        }
    }

    fn quiet(self) -> bool {
        self == Self::Quiet
    }
}

/// A small knob for a send amount from 0 to 1: drag, the wheel or arrow keys
/// (Shift for fine steps), Home and End, or a double-click back to 0. An
/// `automated` knob is ringed: an automation lane sets its amount while the
/// song plays. A quiet knob at 0, not automated, and not hovered or focused
/// rests as a thin ring with its pointer, so only the sends in use show a
/// head.
pub(crate) fn send_knob(
    ui: &mut egui::Ui,
    rect: Rect,
    look: Look,
    name: &str,
    value: &mut f32,
    automated: bool,
) -> bool {
    let scale = look.scale();
    let response = ui.interact(rect, ui.id().with(name), Sense::click_and_drag());
    let before = *value;
    let fine = ui.input(|input| input.modifiers.shift);
    if controls::primary_dragged(ui, &response) {
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
    let idle = look.quiet()
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
/// "{name} limiter". Quiet, on, their usual state, is a short label with a
/// lit lamp; off, the exception, gives no light: a hollow lamp and a struck
/// label.
pub(crate) fn bus_switches(
    ui: &mut egui::Ui,
    rects: [Rect; 2],
    look: Look,
    name: &str,
    bus: &mut FxBus,
) {
    let scale = look.scale();
    for (rect, (label, short), key, on, hint) in [
        (
            rects[0],
            ("Input", "In"),
            format!("{name} power"),
            &mut bus.powered,
            "Off stops new sound into the effect; its tail rings out.",
        ),
        (
            rects[1],
            ("Limiter", "Lim"),
            format!("{name} limiter"),
            &mut bus.limiter,
            LIMITER_HINT,
        ),
    ] {
        let response = if look.quiet() {
            lamp_face(ui, rect, &key, short, *on)
        } else {
            button(ui, rect, label, &key, scale, *on)
        };
        toggle(response, &key, on, hint);
    }
}

/// What a bus's limiter does, for every place that shows its switch.
pub(crate) const LIMITER_HINT: &str =
    "Limits return peaks to -1 dBFS during playback and -1 dBTP in exports.";

/// A quiet bus switch's face: a short label with a lamp, lit while `on`.
/// Off, the exception, gives no light: a hollow lamp and a struck label.
fn lamp_face(ui: &mut egui::Ui, rect: Rect, key: &str, short: &str, on: bool) -> Response {
    let response = face(ui, rect, key, 1.0, None);
    let lamp = Pos2::new(rect.left() + 6.0, rect.center().y);
    let label_at = rect.center() + Vec2::new(4.0, 0.0);
    if on {
        ui.painter().circle_filled(lamp, 2.0, roles().lamp_on);
        text(ui, label_at, short, 10.0, roles().muted);
    } else {
        ui.painter()
            .circle_stroke(lamp, 2.0, Stroke::new(1.0_f32, roles().muted));
        struck_text(ui, label_at, short, 10.0, roles().muted);
    }
    response
}

/// A switch's checked state, its on/off hint, and its toggle on a click.
fn toggle(response: Response, key: &str, on: &mut bool, hint: &str) -> bool {
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, response.enabled(), *on, key)
    });
    let clicked = response
        .hover_hint(format!("{key}: {}. {hint}", if *on { "on" } else { "off" }))
        .clicked();
    if clicked {
        *on = !*on;
    }
    clicked
}

/// A bus's quiet Solo, "{name} solo", drawn as a track's quiet Solo: its
/// letter, with its face Accent while on. Returns whether it changed.
pub(crate) fn bus_solo(ui: &mut egui::Ui, rect: Rect, name: &str, on: &mut bool) -> bool {
    let key = format!("{name} solo");
    let response = quiet_switch(ui, rect, &key, TrackSwitch::Solo, *on);
    toggle(
        response,
        &key,
        on,
        "While any FX return is soloed, only soloed returns sound during playback.",
    )
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

/// "Master bypass": skips the master pedalboard. Like the M, S, L, and R
/// switches, a lamp bar over its label shows that it latches; on bypass the
/// whole face fills amber. Returns whether it changed.
pub(crate) fn bypass_button(ui: &mut egui::Ui, rect: Rect, scale: f32, bypass: &mut bool) -> bool {
    let response = face(
        ui,
        rect,
        "Master bypass",
        scale,
        bypass.then(|| roles().strip_lit),
    );
    let (lamp, label) = if *bypass {
        (roles().ink, roles().ink)
    } else {
        (roles().muted, roles().legend)
    };
    ui.painter().rect_filled(
        Rect::from_center_size(
            rect.center_top() + Vec2::new(0.0, 5.0) * scale,
            Vec2::new(12.0, 2.0) * scale,
        ),
        1.0,
        lamp,
    );
    text(
        ui,
        rect.center() + Vec2::new(0.0, 3.0) * scale,
        "Bypass",
        10.0 * scale,
        label,
    );
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
pub(crate) fn unity_button(ui: &mut egui::Ui, rect: Rect, look: Look, name: &str) -> bool {
    reset(
        ui,
        rect,
        look,
        "Unity",
        &format!("Reset {name} gain to unity"),
    )
    .hover_hint("Set gain to 0.0 dB (unity). Polarity stays unchanged.")
    .clicked()
}

/// "Center {name} pan". Returns whether it was pressed.
pub(crate) fn center_button(ui: &mut egui::Ui, rect: Rect, look: Look, name: &str) -> bool {
    reset(ui, rect, look, "Center", &format!("Center {name} pan"))
        .hover_hint("Reset pan to center. Output routing stays unchanged.")
        .clicked()
}

/// A reset: a faced button on a strip, `reset_button` when quiet.
fn reset(ui: &mut egui::Ui, rect: Rect, look: Look, label: &str, name: &str) -> Response {
    match look {
        Look::Strip(scale) => button(ui, rect, label, name, scale, false),
        Look::Quiet => reset_button(ui, rect, label, name),
    }
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

/// A track's pan in `rect`: "{name} pan". On a strip, the rail is a dark
/// slot and the idle handle keeps a cap-coloured edge on it. Quiet, the slot
/// is the strip's rail well, like the Mix view's fader, and the handle keeps
/// a ring of strip ink at rest and in use, so it shows on Vintage's cream
/// well. Returns whether it changed.
pub(crate) fn pan_slider(
    ui: &mut egui::Ui,
    rect: Rect,
    look: Look,
    name: &str,
    pan: &mut f32,
    automated: bool,
) -> bool {
    let scale = look.scale();
    let roles = roles();
    let (slot, ring, active_ring) = match look {
        Look::Strip(_) => (roles.strip_slot, roles.cap, roles.strip_slot),
        Look::Quiet => (roles.strip_well, roles.strip_text, roles.strip_text),
    };
    let mut changed = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().slider_width = rect.width();
        ui.spacing_mut().interact_size.y = 16.0 * scale;
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_fill = slot;
        widgets.inactive.fg_stroke = Stroke::new(1.5 * scale, ring);
        for state in [&mut widgets.hovered, &mut widgets.active] {
            state.bg_fill = roles.cap;
            state.fg_stroke = Stroke::new(1.5 * scale, active_ring);
        }
        let response = pan_control(ui, name, pan, automated);
        changed = response.changed();
        // The send knob's ring, so an automated pan reads the same way.
        if automated {
            ui.painter().rect_stroke(
                rect.expand(2.0 * scale),
                3.0,
                Stroke::new(1.0 * scale, roles.legend),
                egui::StrokeKind::Outside,
            );
        }
        focus_ring(ui, &response, rect, scale);
    });
    changed
}

/// "{name} EQ", lit while `on`: opens the channel EQ window. With `outline`
/// and off, it is a Strip edge outline with strip ink on the row's own
/// surface, for a silent Mix view row. Returns whether it was pressed.
pub(crate) fn eq_button(
    ui: &mut egui::Ui,
    rect: Rect,
    look: Look,
    name: &str,
    on: bool,
    outline: bool,
) -> bool {
    let scale = look.scale();
    let eq_name = format!("{name} EQ");
    let response = if outline && !on {
        outline_button(ui, rect, "EQ", &eq_name, scale)
    } else {
        button(ui, rect, "EQ", &eq_name, scale, on)
    };
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

/// "{name} polarity invert": a slashed circle on a face that lights while
/// inverted. Quiet, the glyph at rest is thinner and Muted, like the quiet
/// M, S, L, and R. Returns whether it changed.
pub(crate) fn polarity_button(
    ui: &mut egui::Ui,
    rect: Rect,
    look: Look,
    name: &str,
    inverted: &mut bool,
) -> bool {
    let key = format!("{name} polarity invert");
    let (response, rest, width) = match look {
        Look::Strip(scale) => (
            button(ui, rect, "", &key, scale, *inverted),
            roles().legend,
            1.5,
        ),
        Look::Quiet => (
            face(ui, rect, &key, 1.0, inverted.then(|| roles().strip_lit)),
            roles().muted,
            1.0,
        ),
    };
    let color = if *inverted { roles().ink } else { rest };
    polarity_glyph(ui, rect, look.scale(), width, color);
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

/// A strip button without a face: a Strip edge outline, strip ink on hover
/// or the lit edge with focus, and its label in strip ink.
fn outline_button(ui: &mut egui::Ui, rect: Rect, label: &str, name: &str, scale: f32) -> Response {
    let response = ui.interact(rect, ui.id().with(name), Sense::click());
    let edge = if response.has_focus() {
        roles().strip_lit
    } else if response.hovered() {
        roles().strip_text
    } else {
        roles().strip_edge
    };
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0 * scale, edge),
        egui::StrokeKind::Inside,
    );
    focus_ring(ui, &response, rect, scale);
    text(ui, rect.center(), label, 10.0 * scale, roles().strip_text);
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

/// "{name} {switch}": Mute, Solo, or an output. On a strip, a lamp bar over
/// the letter lights while on. Quiet, each rests as a letter; Mute and Solo
/// fill their faces when on, and an output that is off strikes its letter,
/// so a row shows only what differs from a plain track. Returns whether it
/// changed.
pub(crate) fn switch_button(
    ui: &mut egui::Ui,
    rect: Rect,
    look: Look,
    name: &str,
    switch: TrackSwitch,
    enabled: &mut bool,
) -> bool {
    let label = format!("{name} {}", switch.name());
    let response = match look {
        Look::Strip(scale) => strip_switch(ui, rect, scale, &label, switch, *enabled),
        Look::Quiet => quiet_switch(ui, rect, &label, switch, *enabled),
    };
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

/// A strip's switch face: a lamp bar over the letter, both Accent while on.
fn strip_switch(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    label: &str,
    switch: TrackSwitch,
    enabled: bool,
) -> Response {
    let response = button(ui, rect, "", label, scale, false);
    let ink = if enabled {
        roles().accent
    } else {
        roles().muted
    };
    ui.painter().rect_filled(
        Rect::from_center_size(
            rect.center_top() + Vec2::new(0.0, 5.0) * scale,
            Vec2::new(12.0, 2.0) * scale,
        ),
        1.0,
        ink,
    );
    text(
        ui,
        rect.center() + Vec2::new(0.0, 3.0) * scale,
        switch.letter(),
        11.0 * scale,
        ink,
    );
    response
}

/// A quiet switch face: a letter, with Mute and Solo filled while on. An
/// output that is off gives no light: its letter is struck, in no colour.
fn quiet_switch(
    ui: &mut egui::Ui,
    rect: Rect,
    label: &str,
    switch: TrackSwitch,
    enabled: bool,
) -> Response {
    let (fill, letter) = match switch {
        TrackSwitch::Mute if enabled => (Some(roles().coral), roles().ink),
        TrackSwitch::Solo if enabled => (Some(roles().accent), roles().ink),
        _ => (None, roles().muted),
    };
    let response = face(ui, rect, label, 1.0, fill);
    if matches!(switch, TrackSwitch::Left | TrackSwitch::Right) && !enabled {
        struck_text(ui, rect.center(), switch.letter(), 11.0, letter);
    } else {
        text(ui, rect.center(), switch.letter(), 11.0, letter);
    }
    response
}

/// `text` with a line through it, for a switch that is off.
fn struck_text(ui: &egui::Ui, pos: Pos2, value: &str, size: f32, color: Color32) {
    let bounds = ui.painter().text(
        pos,
        Align2::CENTER_CENTER,
        value,
        FontId::proportional(size),
        color,
    );
    let y = bounds.center().y;
    ui.painter().hline(
        (bounds.left() - 2.0)..=(bounds.right() + 2.0),
        y,
        Stroke::new(1.0_f32, color),
    );
}
