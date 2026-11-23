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
    unit_face(ui, rect, "Input");
    input_meter(ui, rect, metering, lamps);
    let x = rect.center().x - 6.0;
    trim_dial(
        ui,
        [109.0, 141.0, 161.0].map(|y| Pos2::new(x, rect.top() + y)),
        chain,
        "Trim",
    );

    // The socket at the right edge sends the trimmed mix into the row.
    let socket = Pos2::new(rect.right() - 10.0, rect.top() - PEDAL_TOP + SIGNAL_Y);
    cable(ui, socket.x, rect.right() + 1.0, socket.y, [false, false]);
    jack(ui, socket);
}

fn input_meter(ui: &egui::Ui, rect: Rect, metering: &Metering, lamps: &mut Lamps) {
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
        rect.min + Vec2::new(10.0, 24.0),
        Vec2::new(rect.width() - 20.0, 44.0),
    );
    let needle = display_needle(
        ui.ctx(),
        ui.id().with("vu-needle"),
        vu_position(vu),
        metering.reduced,
    );
    vu_meter(ui, meter, needle);
    let lit = [0, 1].map(|side| live && now - lamps.peaks[side] < 0.4);
    let peak_y = meter.bottom() + 9.0;
    input_peaks(ui, Pos2::new(meter.center().x, peak_y), lit);
    let level = ui.interact(
        meter.union(Rect::from_center_size(
            Pos2::new(meter.center().x, peak_y),
            Vec2::new(meter.width(), 12.0),
        )),
        ui.id().with("input-level"),
        Sense::hover(),
    );
    named(
        &level,
        egui::WidgetType::Label,
        &input_level_text(live, vu, lit),
    );
    level.hover_hint("0 VU is -18 dBFS. Peak lamps light at -1 dBFS.");
}

fn input_level_text(live: bool, vu: f32, lit: [bool; 2]) -> String {
    if live && vu.is_finite() {
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
    }
}

fn input_peaks(ui: &egui::Ui, center: Pos2, lit: [bool; 2]) {
    for (side, label) in ["L", "R"].into_iter().enumerate() {
        let center = Pos2::new(center.x - 20.0 + 40.0 * side as f32, center.y);
        led(ui, center, 3.8, lit[side], roles().lamp_peak);
        ui.painter().text(
            center + Vec2::new(if side == 0 { -11.0 } else { 11.0 }, 0.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(10.0),
            roles().legend,
        );
    }
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        "PEAK",
        FontId::proportional(9.0),
        roles().legend.gamma_multiply(0.75),
    );
}

pub(super) fn trim_dial(
    ui: &mut egui::Ui,
    positions: [Pos2; 3],
    chain: &mut MasterChain,
    name: &str,
) {
    let before = decimal(chain.trim);
    let mut value = before;
    let (label, _, hint, format) = face(Knob::Trim);
    dial(
        ui,
        positions,
        &mut value,
        Dial {
            name,
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
}

/// The output unit: the socket from the last pedal, the loudness readings,
/// a loudness bar with the -14 LUFS delivery band, a true-peak bar with its
/// -1 dBTP line, and the OVER lamp, which latches until it is clicked.
pub(super) fn output_unit(ui: &mut egui::Ui, rect: Rect, metering: &Metering, lamps: &mut Lamps) {
    unit_face(ui, rect, "Output");
    let reading = &metering.reading;
    let live = metering.playing && reading.run > 0;
    let over = Rect::from_center_size(
        rect.right_top() + Vec2::new(-32.0, 14.0),
        Vec2::new(54.0, 22.0),
    );
    over_control(ui, over, "Clear OVER", reading, lamps);
    output_readouts(ui, rect, reading, live);

    let left = rect.left() + 26.0;
    let right = rect.right() - 10.0;
    let bar_bounds = Rect::from_min_max(
        Pos2::new(left, rect.top() + 114.0),
        Pos2::new(right, rect.top() + 124.0),
    );
    loudness_bar(ui, bar_bounds, reading, live);
    let peak_bounds = Rect::from_min_max(
        Pos2::new(left, rect.top() + 144.0),
        Pos2::new(right, rect.top() + 153.0),
    );
    peak_bar(ui, peak_bounds, reading, live);

    // The socket at the left edge takes the last pedal's output.
    let socket = Pos2::new(rect.left() + 10.0, rect.top() - PEDAL_TOP + SIGNAL_Y);
    cable(ui, rect.left() - 1.0, socket.x, socket.y, [false, false]);
    jack(ui, socket);
}

fn unit_face(ui: &egui::Ui, rect: Rect, title: &str) {
    controls::inset(ui, rect);
    ui.painter().text(
        Pos2::new(rect.left() + 10.0, rect.top() + 14.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(13.5),
        roles().legend,
    );
}

pub(super) fn over_control(
    ui: &egui::Ui,
    over: Rect,
    name: &str,
    reading: &MeterReading,
    lamps: &mut Lamps,
) {
    if reading.run != lamps.run {
        (lamps.run, lamps.overs) = (reading.run, 0);
    }
    if reading.overs > lamps.overs {
        (lamps.overs, lamps.over) = (reading.overs, true);
    }

    let response = ui.interact(over, ui.id().with("over-lamp"), Sense::click());
    if response.clicked() {
        lamps.over = false;
    }
    led(
        ui,
        over.left_center() + Vec2::new(9.0, 0.0),
        5.0,
        lamps.over,
        roles().lamp_peak,
    );
    ui.painter().text(
        over.left_center() + Vec2::new(20.0, 0.0),
        Align2::LEFT_CENTER,
        "OVER",
        FontId::proportional(10.5),
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
    named(&response, egui::WidgetType::Button, name);
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
}

fn output_readouts(ui: &egui::Ui, rect: Rect, reading: &MeterReading, live: bool) {
    let rows: [(&str, &str, f32, &str); 4] = [
        (
            "Mom",
            "Momentary",
            if live {
                reading.momentary
            } else {
                f32::NEG_INFINITY
            },
            "LUFS",
        ),
        (
            "Short",
            "Short-term",
            if live {
                reading.short_term
            } else {
                f32::NEG_INFINITY
            },
            "LUFS",
        ),
        ("Integ", "Integrated", reading.integrated, "LUFS"),
        ("Peak", "True peak", reading.true_peak_max, "dBTP"),
    ];
    let compact_well = Vec2::new(50.0, 17.0);
    for (index, (short_label, _, value, _)) in rows.iter().enumerate() {
        let y = rect.top() + 36.0 + index as f32 * 20.0;
        ui.painter().text(
            Pos2::new(rect.left() + 26.0, y),
            Align2::LEFT_CENTER,
            *short_label,
            FontId::proportional(11.0),
            roles().legend,
        );
        let well = Rect::from_center_size(Pos2::new(rect.right() - 33.0, y), compact_well);
        controls::well(ui, well);
        let hot = index == 3 && *value > OVER_DBTP;
        ui.painter().text(
            well.center(),
            Align2::CENTER_CENTER,
            show_level(*value),
            FontId::proportional(11.5),
            if hot {
                roles().lamp_peak
            } else {
                roles().legend
            },
        );
    }
    let summary = rows
        .iter()
        .map(|(_, label, value, unit)| {
            format!("{} {} {unit}", label.to_lowercase(), show_level(*value))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let readings = ui.interact(
        Rect::from_min_max(
            Pos2::new(rect.left() + 24.0, rect.top() + 26.0),
            Pos2::new(rect.right() - 6.0, rect.top() + 106.0),
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
}

fn at(bar: Rect, low: f32, value: f32) -> f32 {
    bar.left() + bar.width() * ((value - low) / -low).clamp(0.0, 1.0)
}

fn loudness_bar(ui: &egui::Ui, loudness: Rect, reading: &MeterReading, live: bool) {
    bar(ui, loudness);
    if live && reading.momentary.is_finite() {
        let fill = Rect::from_min_max(
            loudness.min + Vec2::new(2.0, 2.5),
            Pos2::new(
                at(loudness, -36.0, reading.momentary),
                loudness.bottom() - 2.5,
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
                Pos2::new(x - 3.5, loudness.bottom() + 5.0),
                Pos2::new(x + 3.5, loudness.bottom() + 5.0),
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
}

fn peak_bar(ui: &egui::Ui, peak: Rect, reading: &MeterReading, live: bool) {
    bar(ui, peak);
    if live && reading.true_peak.is_finite() {
        let fill = Rect::from_min_max(
            peak.min + Vec2::new(2.0, 2.0),
            Pos2::new(at(peak, -24.0, reading.true_peak), peak.bottom() - 2.0),
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
            Pos2::new(line, peak.top() - 2.5),
            Pos2::new(line, peak.bottom() + 2.5),
        ],
        Stroke::new(2.0_f32, roles().lamp_peak),
    );
    scale(
        ui,
        peak,
        &[(-24.0, "-24"), (-12.0, "-12"), (-1.0, "-1 dBTP")],
        -24.0,
    );
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
pub(super) fn show_level(value: f32) -> String {
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
    let pivot = Pos2::new(face.center().x, face.bottom() + 16.0);
    let radius = face.height() + 2.0;
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
                point(position, radius - 14.0),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(8.5),
                roles().ink,
            );
        }
    }
    painter.text(
        Pos2::new(face.center().x, face.bottom() - 6.5),
        Align2::CENTER_CENTER,
        "VU",
        FontId::proportional(9.0),
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
