//! The input unit at the left of the board, with its trim and VU meter, and
//! the output unit at the right, with the loudness meter.
use super::*;

/// The input unit: the socket from the mixer, the trim, and the VU meter
/// with a peak lamp for each side. The meter reads the trimmed signal.
pub(super) fn input_unit(
    ui: &mut egui::Ui,
    rect: Rect,
    chain: &mut MasterChain,
    metering: &Metering,
    lamps: &mut Lamps,
) {
    controls::inset(ui, rect);
    heading(ui, rect, "Input", "From the mixer", roles().legend);
    let now = ui.input(|input| input.time);
    let reading = &metering.reading;
    let live = metering.playing && reading.run > 0;
    for (side, peak) in reading.input_peaks.iter().enumerate() {
        if live && *peak {
            lamps.peaks[side] = now;
        }
    }
    let vu = if live {
        reading.input_vu
    } else {
        f32::NEG_INFINITY
    };
    let meter = Rect::from_min_size(
        rect.min + Vec2::new(12.0, 44.0),
        Vec2::new(rect.width() - 24.0, 74.0),
    );
    let needle = display_needle(
        ui.ctx(),
        ui.id().with("vu-needle"),
        vu_position(vu),
        metering.reduced,
    );
    vu_meter(ui, meter, needle);
    let lit = [0, 1].map(|side| live && now - lamps.peaks[side] < 0.4);
    for (side, label) in ["L", "R"].into_iter().enumerate() {
        let center = Pos2::new(
            meter.center().x - 22.0 + 44.0 * side as f32,
            meter.bottom() + 16.0,
        );
        led(ui, center, 4.5, lit[side], roles().lamp_peak);
        ui.painter().text(
            center + Vec2::new(if side == 0 { -12.0 } else { 12.0 }, 0.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0),
            roles().legend,
        );
    }
    ui.painter().text(
        Pos2::new(meter.center().x, meter.bottom() + 16.0),
        Align2::CENTER_CENTER,
        "PEAK",
        FontId::proportional(9.5),
        roles().legend.gamma_multiply(0.75),
    );
    let level = ui.interact(
        meter.union(Rect::from_center_size(
            Pos2::new(meter.center().x, meter.bottom() + 16.0),
            Vec2::new(meter.width(), 12.0),
        )),
        ui.id().with("input-level"),
        Sense::hover(),
    );
    let text = if live && vu.is_finite() {
        format!(
            "Input level {} VU{}",
            show_level(vu),
            match lit {
                [true, true] => ", peaks on both sides",
                [true, false] => ", peaks on the left",
                [false, true] => ", peaks on the right",
                _ => "",
            }
        )
    } else {
        "Input level: no mix playing".to_owned()
    };
    named(&level, egui::WidgetType::Label, &text);
    level.hover_hint("0 VU is -18 dBFS. Peak lamps light at -1 dBFS.");

    let x = rect.center().x - 8.0;
    let before = decimal(chain.trim);
    let mut value = before;
    let (label, name, hint, format) = face(Knob::Trim);
    dial(
        ui,
        [
            Pos2::new(x, rect.top() + 172.0),
            Pos2::new(x, rect.top() + 210.0),
            Pos2::new(x, rect.top() + 232.0),
        ],
        &mut value,
        Dial {
            name: &name,
            label: &label,
            hint,
            range: Knob::Trim.range(),
            format,
            choices: &[],
        },
        true,
    );
    if value != before {
        Knob::Trim.set(chain, value as f32);
    }

    // The socket at the right edge sends the trimmed mix into the row.
    let socket = Pos2::new(rect.right() - 12.0, rect.top() - PEDAL_TOP + SIGNAL_Y);
    cable(ui, socket.x, rect.right() + 1.0, socket.y, [false, false]);
    jack(ui, socket);
}

/// The output unit: the socket from the last pedal, the loudness readings,
/// a loudness bar with the -14 LUFS delivery band, a true-peak bar with its
/// -1 dBTP line, and the OVER lamp, which latches until it is clicked.
pub(super) fn output_unit(ui: &mut egui::Ui, rect: Rect, metering: &Metering, lamps: &mut Lamps) {
    controls::inset(ui, rect);
    heading(ui, rect, "Output", "To playback and export", roles().legend);
    let reading = &metering.reading;
    let live = metering.playing && reading.run > 0;
    if reading.run != lamps.run {
        (lamps.run, lamps.overs) = (reading.run, 0);
    }
    if reading.overs > lamps.overs {
        (lamps.overs, lamps.over) = (reading.overs, true);
    }

    // The OVER lamp sits where a pedal's lamp would.
    let over = Rect::from_center_size(
        rect.right_top() + Vec2::new(-34.0, 19.0),
        Vec2::new(56.0, 26.0),
    );
    let response = ui.interact(over, ui.id().with("over-lamp"), Sense::click());
    if response.clicked() {
        lamps.over = false;
    }
    led(
        ui,
        over.left_center() + Vec2::new(10.0, 0.0),
        5.5,
        lamps.over,
        roles().lamp_peak,
    );
    ui.painter().text(
        over.left_center() + Vec2::new(22.0, 0.0),
        Align2::LEFT_CENTER,
        "OVER",
        FontId::proportional(11.0),
        roles().legend,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            over.expand(1.0),
            4.0,
            Stroke::new(2.0_f32, controls::focus_color(ui)),
            egui::StrokeKind::Outside,
        );
    }
    named(&response, egui::WidgetType::Button, "Clear OVER");
    let state = if lamps.over {
        "OVER: detected."
    } else {
        "OVER: clear."
    };
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_description(state));
    response.hover_hint(format!(
        "{state} Lights when the output's true peak passes {OVER_DBTP:.0} dBTP, and stays lit \
         until you click it."
    ));

    let rows: [(&str, f32, &str); 4] = [
        (
            "Momentary",
            if live {
                reading.momentary
            } else {
                f32::NEG_INFINITY
            },
            "LUFS",
        ),
        (
            "Short-term",
            if live {
                reading.short_term
            } else {
                f32::NEG_INFINITY
            },
            "LUFS",
        ),
        ("Integrated", reading.integrated, "LUFS"),
        ("True peak", reading.true_peak_max, "dBTP"),
    ];
    for (index, (label, value, _)) in rows.iter().enumerate() {
        let y = rect.top() + 56.0 + index as f32 * 28.0;
        ui.painter().text(
            Pos2::new(rect.left() + 30.0, y),
            Align2::LEFT_CENTER,
            *label,
            FontId::proportional(11.5),
            roles().legend,
        );
        let well = Rect::from_center_size(Pos2::new(rect.right() - 42.0, y), WELL);
        controls::well(ui, well);
        let hot = index == 3 && *value > OVER_DBTP;
        ui.painter().text(
            well.center(),
            Align2::CENTER_CENTER,
            show_level(*value),
            FontId::proportional(13.0),
            if hot {
                roles().lamp_peak
            } else {
                roles().legend
            },
        );
    }
    let summary = rows
        .iter()
        .map(|(label, value, unit)| {
            format!("{} {} {unit}", label.to_lowercase(), show_level(*value))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let readings = ui.interact(
        Rect::from_min_max(
            Pos2::new(rect.left() + 26.0, rect.top() + 44.0),
            Pos2::new(rect.right() - 8.0, rect.top() + 152.0),
        ),
        ui.id().with("output-loudness"),
        Sense::hover(),
    );
    named(
        &readings,
        egui::WidgetType::Label,
        &format!("Output loudness: {summary}"),
    );
    readings.hover_hint(
        "Momentary and short-term loudness follow the last 400 ms and 3 s. Integrated loudness \
         and the true peak cover the playback so far.",
    );

    // Bars: loudness from -36 to 0 LUFS with the -14 LUFS band, true peak
    // from -24 to 0 dBTP with the -1 dBTP line.
    let left = rect.left() + 30.0;
    let right = rect.right() - 14.0;
    let loudness = Rect::from_min_max(
        Pos2::new(left, rect.top() + 178.0),
        Pos2::new(right, rect.top() + 192.0),
    );
    let at = |bar: Rect, low: f32, value: f32| {
        bar.left() + bar.width() * ((value - low) / -low).clamp(0.0, 1.0)
    };
    bar(ui, loudness);
    if live && reading.momentary.is_finite() {
        let fill = Rect::from_min_max(
            loudness.min + Vec2::new(2.0, 3.0),
            Pos2::new(
                at(loudness, -36.0, reading.momentary),
                loudness.bottom() - 3.0,
            ),
        );
        ui.painter()
            .rect_filled(fill, 1.0, roles().legend.gamma_multiply(0.85));
    }
    // The delivery band stays visible over the fill.
    let band = Rect::from_x_y_ranges(
        at(loudness, -36.0, -15.0)..=at(loudness, -36.0, -13.0),
        loudness.y_range(),
    );
    ui.painter()
        .rect_filled(band, 1.0, roles().lamp_good.gamma_multiply(0.55));
    ui.painter().rect_stroke(
        band,
        1.0,
        Stroke::new(1.0_f32, roles().lamp_good),
        egui::StrokeKind::Inside,
    );
    if live && reading.short_term.is_finite() {
        let x = at(loudness, -36.0, reading.short_term);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(x - 4.0, loudness.bottom() + 6.0),
                Pos2::new(x + 4.0, loudness.bottom() + 6.0),
                Pos2::new(x, loudness.bottom()),
            ],
            controls::print_color(ui),
            Stroke::NONE,
        ));
    }
    scale(
        ui,
        loudness,
        &[
            (-36.0, "-36"),
            (-24.0, "-24"),
            (-14.0, "-14"),
            (0.0, "0 LUFS"),
        ],
        -36.0,
    );

    let peak = Rect::from_min_max(
        Pos2::new(left, rect.top() + 222.0),
        Pos2::new(right, rect.top() + 232.0),
    );
    bar(ui, peak);
    if live && reading.true_peak.is_finite() {
        let fill = Rect::from_min_max(
            peak.min + Vec2::new(2.0, 2.5),
            Pos2::new(at(peak, -24.0, reading.true_peak), peak.bottom() - 2.5),
        );
        ui.painter()
            .rect_filled(fill, 1.0, roles().legend.gamma_multiply(0.85));
    }
    if reading.true_peak_max.is_finite() {
        let x = at(peak, -24.0, reading.true_peak_max);
        ui.painter().line_segment(
            [
                Pos2::new(x, peak.top() + 1.0),
                Pos2::new(x, peak.bottom() - 1.0),
            ],
            Stroke::new(2.0_f32, roles().legend),
        );
    }
    let line = at(peak, -24.0, OVER_DBTP);
    ui.painter().line_segment(
        [
            Pos2::new(line, peak.top() - 3.0),
            Pos2::new(line, peak.bottom() + 3.0),
        ],
        Stroke::new(2.0_f32, roles().lamp_peak),
    );
    scale(
        ui,
        peak,
        &[(-24.0, "-24"), (-12.0, "-12"), (-1.0, "-1 dBTP")],
        -24.0,
    );

    // The socket at the left edge takes the last pedal's output.
    let socket = Pos2::new(rect.left() + 12.0, rect.top() - PEDAL_TOP + SIGNAL_Y);
    cable(ui, rect.left() - 1.0, socket.x, socket.y, [false, false]);
    jack(ui, socket);
}

/// A meter's dark track.
fn bar(ui: &egui::Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 2.0, roles().ink);
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.25)),
        egui::StrokeKind::Inside,
    );
}

/// Marks and figures under a meter bar whose scale runs from `low` to 0.
fn scale(ui: &egui::Ui, bar: Rect, marks: &[(f32, &str)], low: f32) {
    for (value, label) in marks {
        let x = bar.left() + bar.width() * ((value - low) / -low);
        ui.painter().line_segment(
            [
                Pos2::new(x, bar.bottom() + 1.0),
                Pos2::new(x, bar.bottom() + 4.0),
            ],
            Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.6)),
        );
        let align = if *value == low {
            Align2::LEFT_TOP
        } else if x > bar.right() - 30.0 {
            Align2::RIGHT_TOP
        } else {
            Align2::CENTER_TOP
        };
        let x = if *value == low {
            bar.left()
        } else if x > bar.right() - 30.0 {
            bar.right()
        } else {
            x
        };
        ui.painter().text(
            Pos2::new(x, bar.bottom() + 6.0),
            align,
            *label,
            FontId::proportional(9.5),
            roles().legend.gamma_multiply(0.75),
        );
    }
}

/// A level as the meters print it.
fn show_level(value: f32) -> String {
    if value.is_finite() {
        format!("{:.1}", value + 0.0)
    } else {
        "-".to_owned()
    }
}

/// The needle's travel from -20 to +3 VU, spread by voltage as VU scales are.
pub(super) fn vu_position(vu: f32) -> f32 {
    if !vu.is_finite() {
        return 0.0;
    }
    let volts = |db: f32| 10f32.powf(db / 20.0);
    ((volts(vu.min(4.0)) - volts(-20.0)) / (volts(3.0) - volts(-20.0))).clamp(0.0, 1.03)
}

/// An ivory VU face with its scale, red zone, and needle.
fn vu_meter(ui: &egui::Ui, face: Rect, position: f32) {
    let painter = ui.painter().with_clip_rect(face.intersect(ui.clip_rect()));
    painter.rect_filled(face, 4.0, roles().meter_face);
    let pivot = Pos2::new(face.center().x, face.bottom() + 22.0);
    let radius = face.height() + 4.0;
    let point = |position: f32, reach: f32| {
        let angle = (-40.0 + 80.0 * position).to_radians();
        pivot + Vec2::new(angle.sin(), -angle.cos()) * reach
    };
    // The red zone above 0 VU.
    let zone: Vec<Pos2> = (0..=12)
        .map(|step| point(vu_position(3.0 * step as f32 / 12.0), radius - 6.0))
        .collect();
    painter.add(egui::Shape::line(
        zone,
        Stroke::new(4.0_f32, Color32::from_rgb(196, 52, 40)),
    ));
    for (value, label) in [
        (-20.0, Some("20")),
        // The scale is crowded at its low end, so -10 has a mark only.
        (-10.0, None),
        (-7.0, None),
        (-5.0, Some("5")),
        (-3.0, None),
        (-2.0, None),
        (-1.0, None),
        (0.0, Some("0")),
        (1.0, None),
        (2.0, None),
        (3.0, Some("+3")),
    ] {
        let position = vu_position(value);
        let long = label.is_some();
        painter.line_segment(
            [
                point(position, radius - if long { 10.0 } else { 7.0 }),
                point(position, radius - 2.0),
            ],
            Stroke::new(1.0_f32, roles().ink),
        );
        if let Some(label) = label {
            painter.text(
                point(position, radius - 17.0),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(9.5),
                roles().ink,
            );
        }
    }
    painter.text(
        Pos2::new(face.center().x, face.bottom() - 11.0),
        Align2::CENTER_CENTER,
        "VU",
        FontId::proportional(11.0),
        roles().ink,
    );
    painter.line_segment(
        [pivot, point(position, radius)],
        Stroke::new(1.5_f32, roles().ink),
    );
    ui.painter().rect_stroke(
        face,
        4.0,
        Stroke::new(1.5_f32, roles().ink),
        egui::StrokeKind::Outside,
    );
}
