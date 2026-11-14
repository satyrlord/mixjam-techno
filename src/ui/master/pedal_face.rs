//! One pedal: its header, gain-reduction lamps, dials, move buttons, and switch,
//! and bringing a control into view when it takes keyboard focus.
use super::*;

/// One pedal: header with name and lamp, gain-reduction lamps on dynamics
/// pedals, the dials, then the move buttons and the switch. Returns the
/// position a move button sends the pedal to.
pub(super) fn pedal(
    ui: &mut egui::Ui,
    chain: &mut MasterChain,
    kind: PedalKind,
    rect: Rect,
    index: usize,
    metering: &Metering,
) -> Option<usize> {
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y);
    let on = chain.is_on(kind);
    enclosure(ui, rect, kind, on);
    heading(ui, rect, title(kind), family(kind), roles().ink);
    lamp(ui, at(rect.width() - 18.0, 19.0), on);
    for y in [HEADER + 2.0, FOOTER_TOP - 2.0] {
        ui.painter().line_segment(
            [at(10.0, y), at(rect.width() - 10.0, y)],
            Stroke::new(1.0_f32, roles().ink.gamma_multiply(0.35)),
        );
    }

    let (columns, rows) = grid(kind);
    let lamps = reduction_lamps(kind);
    let stack = lamps.map_or(0.0, |_| LAMP_ROW) + rows as f32 * ROW - 8.0;
    let top = HEADER + ((FOOTER_TOP - HEADER) - stack) / 2.0;
    let dials = dial_count(kind);
    let cell = |slot: usize| {
        let (column, row) = (slot % columns, slot / columns);
        // A short last row is centered under the one above.
        let in_row = if row + 1 == rows {
            dials - row * columns
        } else {
            columns
        };
        let left = (rect.width() - in_row as f32 * CELL) / 2.0;
        let x = left + (column as f32 + 0.5) * CELL;
        let y = top + lamps.map_or(0.0, |_| LAMP_ROW) + 22.0 + row as f32 * ROW;
        [at(x, y), at(x, y + 38.0), at(x, y + 60.0)]
    };
    if let Some(thresholds) = lamps {
        let reduction = match kind {
            PedalKind::BusCompressor => metering.reading.compressor_reduction,
            _ => metering.reading.limiter_reduction,
        };
        let reduction = if on && metering.playing {
            reduction
        } else {
            0.0
        };
        reduction_meter(
            ui,
            at(rect.width() / 2.0, top + 10.0),
            name(kind),
            reduction,
            &thresholds,
        );
    }
    // The dials sit on the enamel itself, which they read as a light surface.
    ui.scope(|ui| {
        ui.visuals_mut().dark_mode = false;
        for (slot, knob) in kind.knobs().iter().enumerate() {
            let (label, name, hint, format) = face(*knob);
            let before = decimal(knob.get(chain));
            let mut value = before;
            dial(
                ui,
                cell(slot),
                &mut value,
                Dial {
                    name: &name,
                    label: &label,
                    hint,
                    range: knob.range(),
                    format,
                    choices: &[],
                },
                on,
            );
            if value != before {
                knob.set(chain, value as f32);
            }
        }
        if kind == PedalKind::Tape {
            let mut speed = f64::from(chain.tape.speed);
            dial(
                ui,
                cell(1),
                &mut speed,
                Dial {
                    name: "Tape speed",
                    label: "Speed",
                    hint: "15 or 30 inches per second: moves the low-end bump and the treble \
                           roll-off.",
                    range: KnobRange {
                        low: 15.0,
                        high: 30.0,
                        default: 30.0,
                        step: 15.0,
                        logarithmic: false,
                    },
                    format: Format::Speed,
                    choices: &[15.0, 30.0],
                },
                on,
            );
            chain.tape.speed = if speed < 22.5 { 15 } else { 30 };
        }
    });

    let mut moved = None;
    // The limiter has no move buttons, and the pedal before it cannot pass it.
    let buttons = if kind.movable() {
        vec![
            (PEDAL_PADDING - 4.0, -1.0, "left", index > 0),
            (
                rect.width() - PEDAL_PADDING - 26.0,
                1.0,
                "right",
                chain
                    .order
                    .get(index + 1)
                    .is_some_and(|next| next.movable()),
            ),
        ]
    } else {
        Vec::new()
    };
    // A move button follows its pedal into view on the next frame. Where the
    // move turns it off, at the row's ends, focus passes to the other arrow.
    let follow_id = egui::Id::new("master-move-follow");
    let follow = ui
        .ctx()
        .data(|data| data.get_temp::<(String, bool)>(follow_id));
    for (offset, direction, step, enabled) in buttons {
        let button = Rect::from_min_size(at(offset, FOOTER_TOP + 6.0), Vec2::new(30.0, 28.0));
        let label = format!("Move {} {step}", name(kind));
        let response = controls::button_at(
            ui,
            button,
            &label,
            "",
            controls::ButtonRole::Secondary,
            enabled,
        );
        arrow(ui, button.center(), direction, enabled);
        let response = reveal(name_control(response, &label)).hover_hint(format!(
            "Move {} one place {step} in the signal path.",
            name(kind)
        ));
        if let Some((target, focus)) = &follow
            && *target == label
        {
            ui.ctx()
                .data_mut(|data| data.remove::<(String, bool)>(follow_id));
            if *focus {
                response.request_focus();
            }
            response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
        }
        if response.clicked() {
            let to = if step == "left" { index - 1 } else { index + 1 };
            moved = Some(to);
            // After a step right, the pedal that follows it is the one that
            // followed its new place before the move.
            let at_end = if step == "left" {
                to == 0
            } else {
                !chain.order.get(to + 1).is_some_and(|next| next.movable())
            };
            let other = if step == "left" { "right" } else { "left" };
            let target = if at_end {
                format!("Move {} {other}", name(kind))
            } else {
                label.clone()
            };
            let focus = response.has_focus();
            ui.ctx()
                .data_mut(|data| data.insert_temp(follow_id, (target, focus)));
        }
    }
    let mut switch = on;
    let response = controls::toggle_at(
        ui,
        Rect::from_center_size(
            at(rect.width() / 2.0 + 8.0, FOOTER_TOP + 20.0),
            Vec2::new(44.0, 24.0),
        ),
        name(kind),
        "",
        &mut switch,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            response.enabled(),
            switch,
            name(kind),
        )
    });
    reveal(response).hover_hint(format!(
        "{}: {}. A pedal that is off keeps its settings and passes the signal on.",
        name(kind),
        if switch { "on" } else { "off" }
    ));
    chain.set_on(kind, switch);
    moved
}

/// Bring a control into view at once when it gains keyboard focus, so Tab
/// can walk the whole row. The row takes the scroll for itself, so the
/// board returns to the top on the next frame, which shows the whole row.
pub(super) fn reveal(response: Response) -> Response {
    if response.gained_focus() {
        response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
        response
            .ctx
            .data_mut(|data| data.insert_temp(raise_id(), true));
    }
    response
}

/// The request to show the top of a scrolled board on its next frame.
pub(super) fn raise_id() -> egui::Id {
    egui::Id::new("mastering-raise")
}

/// A row of lamps for the gain reduction of a dynamics pedal; each lights
/// once the reduction reaches its step.
fn reduction_meter(ui: &egui::Ui, center: Pos2, pedal: &str, reduction: f32, steps: &[f32; 6]) {
    let first = center.x - 2.5 * 14.0 + 10.0;
    ui.painter().text(
        Pos2::new(first - 22.0, center.y),
        Align2::CENTER_CENTER,
        "GR",
        FontId::proportional(10.5),
        roles().ink,
    );
    for (index, step) in steps.iter().enumerate() {
        led(
            ui,
            Pos2::new(first + index as f32 * 14.0, center.y),
            4.0,
            reduction >= *step,
            roles().lamp_warn,
        );
    }
    let response = ui.interact(
        Rect::from_center_size(center + Vec2::new(-6.0, 0.0), Vec2::new(112.0, 14.0)),
        ui.id().with(("reduction", pedal)),
        Sense::hover(),
    );
    named(
        &response,
        egui::WidgetType::Label,
        &format!("{pedal} gain reduction {reduction:.1} dB"),
    );
    response.hover_hint(format!(
        "Gain reduction. The lamps light at {} dB.",
        steps.map(|step| step.to_string()).join(", ")
    ));
}
