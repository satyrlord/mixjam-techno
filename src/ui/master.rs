//! Mastering window body: the master pedalboard, laid out as the signal flows.
//! The mixer's output enters at the left, passes through each pedal in order,
//! and leaves at the right. Shared hardware controls and shell own
//! presentation; the caller turns a changed chain into an edit.
use super::controls::{CREAM, INK};
use super::*;

pub(super) enum Action {
    None,
    Change(MasterChain),
    Help,
}

const MARGIN: f32 = 8.0;
const JACK_WIDTH: f32 = 64.0;
const CABLE: f32 = 26.0;
const PEDAL_TOP: f32 = 8.0;
const PEDAL_HEIGHT: f32 = 280.0;
const HEADER: f32 = 36.0;
const FOOTER_TOP: f32 = 236.0;
/// The height of the cables and jacks, through the middle of the pedal bodies.
const SIGNAL_Y: f32 = PEDAL_TOP + 136.0;
/// One knob with its value well and label.
const CELL: f32 = 60.0;
const PEDAL_PADDING: f32 = 14.0;
const FOOTER_HEIGHT: f32 = 36.0;
const BOARD_HEIGHT: f32 = PEDAL_TOP + PEDAL_HEIGHT + 12.0 + FOOTER_HEIGHT;

fn name(kind: PedalKind) -> &'static str {
    match kind {
        PedalKind::EjayEq => "EQ",
        PedalKind::EjayCompressor => "Compressor",
        PedalKind::EjayWide => "Stereo wide",
    }
}

/// Knob columns and rows of a pedal.
fn grid(kind: PedalKind) -> (usize, usize) {
    match kind {
        PedalKind::EjayEq => (5, 2),
        PedalKind::EjayCompressor => (2, 2),
        PedalKind::EjayWide => (1, 1),
    }
}

fn pedal_width(kind: PedalKind) -> f32 {
    let (columns, _) = grid(kind);
    (columns as f32 * CELL + 2.0 * PEDAL_PADDING).max(124.0)
}

fn board_width(chain: &MasterChain) -> f32 {
    2.0 * MARGIN
        + 2.0 * JACK_WIDTH
        + (chain.order.len() + 1) as f32 * CABLE
        + chain
            .order
            .iter()
            .map(|kind| pedal_width(*kind))
            .sum::<f32>()
}

/// Bands 0 and 9 are shelves with a 12 dB range; the others reach 18 dB.
fn band_range_db(band: usize) -> f64 {
    if band == 0 || band == 9 { 12.0 } else { 18.0 }
}

fn band_db(band: usize, value: f64) -> f64 {
    let range = band_range_db(band);
    value / f64::from(EQ_BAND_MAX) * 2.0 * range - range
}

fn band_label(hz: f64) -> String {
    if hz >= 1000.0 {
        format!("{} kHz", hz / 1000.0)
    } else {
        format!("{hz} Hz")
    }
}

/// The pedal being dragged to a new place, by its position on the board.
#[derive(Clone, Copy)]
struct Drag {
    from: usize,
}

pub(super) fn show(ctx: &egui::Context, open: &mut bool, chain: &MasterChain) -> Action {
    let mut draft = chain.clone();
    let size = Vec2::new(board_width(chain), BOARD_HEIGHT);
    let response = dialog::tool(
        ctx,
        "Mastering",
        open,
        size,
        controls::Palette::Indigo,
        true,
        |ui| {
            let (bounds, _) = ui.allocate_exact_size(size, Sense::hover());
            let mut panel = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
            let drag_id = panel.id().with("pedal-drag");
            let mut drag = panel
                .ctx()
                .data(|data| data.get_temp::<Drag>(drag_id).map(|d| d.from));

            // Lay the board out from left to right.
            let mut x = bounds.left() + MARGIN;
            let input = Rect::from_min_size(
                Pos2::new(x, bounds.top() + SIGNAL_Y - 60.0),
                Vec2::new(JACK_WIDTH, 120.0),
            );
            x += JACK_WIDTH + CABLE;
            let pedals: Vec<(PedalKind, Rect)> = chain
                .order
                .iter()
                .map(|kind| {
                    let rect = Rect::from_min_size(
                        Pos2::new(x, bounds.top() + PEDAL_TOP),
                        Vec2::new(pedal_width(*kind), PEDAL_HEIGHT),
                    );
                    x += rect.width() + CABLE;
                    (*kind, rect)
                })
                .collect();
            let output =
                Rect::from_min_size(Pos2::new(x, input.top()), Vec2::new(JACK_WIDTH, 120.0));

            jack(&panel, input, "From", "mixer");
            jack(&panel, output, "To", "output");
            let mut ends = vec![input.right()];
            for (_, rect) in &pedals {
                ends.push(rect.left());
                ends.push(rect.right());
            }
            ends.push(output.left());
            for [from, to] in ends.as_chunks::<2>().0 {
                cable(&panel, *from, *to, bounds.top() + SIGNAL_Y);
            }

            let count = pedals.len();
            for (index, (kind, rect)) in pedals.iter().enumerate() {
                let moved = pedal(&mut panel, &mut draft, *kind, *rect, index, count);
                if let Some(to) = moved {
                    draft.move_pedal(index, to);
                }
                // The header is the handle that drags a pedal along the board.
                let handle = Rect::from_min_size(rect.min, Vec2::new(rect.width() - 30.0, HEADER));
                let grip = panel.interact(
                    handle,
                    panel.id().with(("pedal-grip", *kind)),
                    Sense::drag(),
                );
                let grip = name_control(grip, &format!("Drag {}", name(*kind)))
                    .on_hover_cursor(egui::CursorIcon::Grab)
                    .on_hover_text(
                        "Drag along the board to change where this pedal sits in the signal path.",
                    );
                if grip.drag_started() {
                    drag = Some(index);
                }
                if grip.dragged() {
                    panel.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
            }

            // While a pedal is dragged, mark where it would land.
            if let Some(from) = drag {
                let accent = panel.visuals().selection.bg_fill;
                let pointer = panel.ctx().pointer_latest_pos();
                let released = panel.input(|input| input.pointer.any_released());
                let target = pointer.map(|pointer| {
                    pedals
                        .iter()
                        .enumerate()
                        .filter(|(index, (_, rect))| *index != from && rect.center().x < pointer.x)
                        .count()
                });
                if let Some(to) = target {
                    let (_, dragged) = pedals[from];
                    panel.painter().rect_stroke(
                        dragged,
                        6.0,
                        Stroke::new(2.0_f32, accent),
                        egui::StrokeKind::Outside,
                    );
                    let others: Vec<Rect> = pedals
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| *index != from)
                        .map(|(_, (_, rect))| *rect)
                        .collect();
                    let marker_x = match (
                        to.checked_sub(1).and_then(|i| others.get(i)),
                        others.get(to),
                    ) {
                        (Some(before), _) => before.right() + CABLE / 2.0,
                        (None, Some(after)) => after.left() - CABLE / 2.0,
                        (None, None) => dragged.center().x,
                    };
                    panel.painter().line_segment(
                        [
                            Pos2::new(marker_x, bounds.top() + PEDAL_TOP),
                            Pos2::new(marker_x, bounds.top() + PEDAL_TOP + PEDAL_HEIGHT),
                        ],
                        Stroke::new(3.0_f32, accent),
                    );
                    if released {
                        draft.move_pedal(from, to);
                    }
                }
                if released || pointer.is_none() {
                    drag = None;
                }
            }
            panel.ctx().data_mut(|data| match drag {
                Some(from) => data.insert_temp(drag_id, Drag { from }),
                None => data.remove::<Drag>(drag_id),
            });

            let path: Vec<String> = draft
                .order
                .iter()
                .map(|kind| {
                    if draft.is_on(*kind) {
                        name(*kind).to_owned()
                    } else {
                        format!("{} (off)", name(*kind))
                    }
                })
                .collect();
            let status = format!(
                "Signal: mixer → {} → output.{}",
                path.join(" → "),
                if draft.is_active() {
                    ""
                } else {
                    " Every pedal is off: the mix is unprocessed."
                }
            );
            footer(
                &mut panel,
                Rect::from_min_size(
                    Pos2::new(bounds.left() + MARGIN, bounds.bottom() - FOOTER_HEIGHT),
                    Vec2::new(bounds.width() - 2.0 * MARGIN, FOOTER_HEIGHT),
                ),
                &status,
            );
        },
    );
    if response.help {
        Action::Help
    } else if draft != *chain {
        Action::Change(draft)
    } else {
        Action::None
    }
}

/// A socket at one end of the board.
fn jack(ui: &egui::Ui, rect: Rect, first: &str, second: &str) {
    controls::inset(ui, rect);
    for (text, y) in [(first, -30.0), (second, -14.0)] {
        ui.painter().text(
            rect.center() + Vec2::new(0.0, y),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(12.0),
            CREAM,
        );
    }
    let socket = rect.center() + Vec2::new(0.0, 20.0);
    ui.painter().circle_filled(socket, 11.0, INK);
    ui.painter().circle_stroke(
        socket,
        11.0,
        Stroke::new(2.0_f32, CREAM.gamma_multiply(0.6)),
    );
    ui.painter().circle_filled(socket, 4.0, Color32::BLACK);
}

/// A patch cable from one socket to the next, with an arrow for the direction.
fn cable(ui: &egui::Ui, from: f32, to: f32, y: f32) {
    let painter = ui.painter();
    painter.line_segment(
        [Pos2::new(from, y + 1.0), Pos2::new(to, y + 1.0)],
        Stroke::new(6.0_f32, Color32::from_black_alpha(120)),
    );
    painter.line_segment(
        [Pos2::new(from, y), Pos2::new(to, y)],
        Stroke::new(4.0_f32, INK),
    );
    painter.line_segment(
        [Pos2::new(from, y - 1.0), Pos2::new(to, y - 1.0)],
        Stroke::new(1.0_f32, CREAM.gamma_multiply(0.25)),
    );
    // A metal plug seats each end in its socket.
    for (left, right) in [(from, from + 8.0), (to - 8.0, to)] {
        let plug = Rect::from_min_max(Pos2::new(left, y - 4.5), Pos2::new(right, y + 4.5));
        painter.rect_filled(plug, 1.5, Color32::from_rgb(198, 194, 180));
        painter.rect_stroke(
            plug,
            1.5,
            Stroke::new(1.0_f32, INK),
            egui::StrokeKind::Inside,
        );
        painter.line_segment(
            [
                plug.left_top() + Vec2::new(1.5, 2.0),
                plug.right_top() + Vec2::new(-1.5, 2.0),
            ],
            Stroke::new(1.0_f32, Color32::from_white_alpha(170)),
        );
    }
    let tip = Pos2::new((from + to) / 2.0 + 4.0, y - 10.0);
    painter.add(egui::Shape::convex_polygon(
        vec![
            tip + Vec2::new(-7.0, -4.0),
            tip + Vec2::new(1.0, 0.0),
            tip + Vec2::new(-7.0, 4.0),
        ],
        CREAM.gamma_multiply(0.7),
        Stroke::NONE,
    ));
}

/// One pedal: header with name and LED, knobs, then the move buttons and its
/// switch. Returns the position the pedal is moved to by a button.
fn pedal(
    ui: &mut egui::Ui,
    chain: &mut MasterChain,
    kind: PedalKind,
    rect: Rect,
    index: usize,
    count: usize,
) -> Option<usize> {
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y);
    let on = chain.is_on(kind);
    enclosure(ui, rect, enamel(kind, on));
    ui.painter().text(
        at(PEDAL_PADDING, 15.0),
        Align2::LEFT_CENTER,
        name(kind),
        FontId::proportional(14.0),
        INK,
    );
    ui.painter().text(
        at(PEDAL_PADDING, 30.0),
        Align2::LEFT_CENTER,
        "eJay Booster",
        FontId::proportional(10.0),
        INK,
    );
    lamp(ui, at(rect.width() - 18.0, 19.0), on);
    for y in [HEADER + 2.0, FOOTER_TOP - 2.0] {
        ui.painter().line_segment(
            [at(10.0, y), at(rect.width() - 10.0, y)],
            Stroke::new(1.0_f32, INK.gamma_multiply(0.35)),
        );
    }

    let (columns, rows) = grid(kind);
    let cell = |slot: usize| {
        let (column, row) = (slot % columns, slot / columns);
        // A short last row is centered under the one above.
        let in_row = if row + 1 == rows {
            knob_count(kind) - row * columns
        } else {
            columns
        };
        let left = (rect.width() - in_row as f32 * CELL) / 2.0;
        let x = left + (column as f32 + 0.5) * CELL;
        // A single row sits in the middle of the body.
        let y = HEADER + if rows == 1 { 76.0 } else { 28.0 } + row as f32 * 96.0;
        [at(x, y), at(x, y + 38.0), at(x, y + 60.0)]
    };
    // The knobs sit on the enamel itself, which the dials read as a light surface.
    ui.scope(|ui| {
        ui.visuals_mut().dark_mode = false;
        match kind {
            PedalKind::EjayEq => {
                for (band, hz) in EQ_FREQUENCIES.iter().enumerate() {
                    let mut value = f64::from(chain.ejay_eq.bands[band]);
                    knob(
                        ui,
                        cell(band),
                        &mut value,
                        Knob {
                            name: &format!("EQ {hz} Hz"),
                            label: &band_label(*hz),
                            high: f64::from(EQ_BAND_MAX),
                            reset: f64::from(EQ_BAND_MAX / 2),
                            format: Format::Decibels(band),
                        },
                        on,
                    );
                    chain.ejay_eq.bands[band] = value as u8;
                }
            }
            PedalKind::EjayCompressor => {
                let compressor = &mut chain.ejay_compressor;
                for (slot, (value, name, label)) in [
                    (&mut compressor.drive, "Compressor drive", "Drive"),
                    (&mut compressor.speed, "Compressor speed", "Speed"),
                    (&mut compressor.gain, "Compressor gain", "Gain"),
                ]
                .into_iter()
                .enumerate()
                {
                    *value = plain_knob(ui, cell(slot), *value, name, label, on);
                }
            }
            PedalKind::EjayWide => {
                chain.ejay_wide.spread = plain_knob(
                    ui,
                    cell(0),
                    chain.ejay_wide.spread,
                    "Stereo width",
                    "Width",
                    on,
                );
            }
        }
    });

    let mut moved = None;
    for (offset, direction, step, enabled) in [
        (PEDAL_PADDING - 4.0, -1.0, "left", index > 0),
        (
            rect.width() - PEDAL_PADDING - 26.0,
            1.0,
            "right",
            index + 1 < count,
        ),
    ] {
        let button = Rect::from_min_size(at(offset, FOOTER_TOP + 6.0), Vec2::new(30.0, 28.0));
        let response = controls::button_at(
            ui,
            button,
            &format!("Move {} {step}", name(kind)),
            "",
            controls::ButtonRole::Secondary,
            enabled,
        );
        arrow(ui, button.center(), direction, enabled);
        let response = name_control(response, &format!("Move {} {step}", name(kind)))
            .on_hover_text(format!(
                "Move {} one place {step} in the signal path.",
                name(kind)
            ));
        if response.clicked() {
            moved = Some(if step == "left" { index - 1 } else { index + 1 });
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
    response.on_hover_text(format!(
        "{}: {}. A pedal that is off keeps its settings and passes the signal on.",
        name(kind),
        if switch { "on" } else { "off" }
    ));
    chain.set_on(kind, switch);
    moved
}

/// The painted enclosure of each pedal. Like stompboxes, each kind has its
/// own enamel; it always appears with the pedal's printed name. A pedal that
/// is off loses most of its color and some light, so the pedals in use stand
/// out. Ink lettering keeps at least 5:1 contrast in both states.
fn enamel(kind: PedalKind, on: bool) -> Color32 {
    let [red, green, blue] = match kind {
        PedalKind::EjayEq => [222, 172, 64],
        PedalKind::EjayCompressor => [84, 184, 166],
        PedalKind::EjayWide => [200, 150, 220],
    };
    if on {
        return Color32::from_rgb(red, green, blue);
    }
    let gray = 0.2126 * f32::from(red) + 0.7152 * f32::from(green) + 0.0722 * f32::from(blue);
    let toward = |channel: u8| {
        ((f32::from(channel) + (gray - f32::from(channel)) * 0.7) * 0.83).round() as u8
    };
    Color32::from_rgb(toward(red), toward(green), toward(blue))
}

/// A soft drop shadow, the enamel body with a lit top edge, and an ink outline.
fn enclosure(ui: &egui::Ui, rect: Rect, enamel: Color32) {
    let radius = egui::CornerRadius::same(9);
    ui.painter().add(
        egui::epaint::Shadow {
            offset: [0, 5],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(110),
        }
        .as_shape(rect, radius),
    );
    ui.painter().rect_filled(rect, radius, enamel);
    // The upper face catches the light, as folded enamel does.
    ui.painter().rect_filled(
        Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEADER)),
        egui::CornerRadius {
            nw: 9,
            ne: 9,
            sw: 0,
            se: 0,
        },
        Color32::from_white_alpha(22),
    );
    ui.painter().line_segment(
        [
            rect.left_top() + Vec2::new(9.0, 1.5),
            rect.right_top() + Vec2::new(-9.0, 1.5),
        ],
        Stroke::new(1.0_f32, Color32::from_white_alpha(90)),
    );
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.5_f32, INK),
        egui::StrokeKind::Outside,
    );
}

/// The pedal's lamp. Lit, it glows in the application's active lime.
fn lamp(ui: &egui::Ui, center: Pos2, on: bool) {
    const LIME: Color32 = Color32::from_rgb(184, 227, 126);
    let painter = ui.painter();
    if on {
        for (radius, alpha) in [(12.0, 28), (9.0, 54)] {
            painter.circle_filled(center, radius, LIME.gamma_multiply(alpha as f32 / 255.0));
        }
    }
    painter.circle_filled(center, 6.5, INK);
    painter.circle_filled(
        center,
        4.5,
        if on {
            LIME
        } else {
            Color32::from_rgb(44, 48, 42)
        },
    );
    if on {
        painter.circle_filled(
            center + Vec2::new(-1.5, -1.5),
            1.5,
            Color32::from_white_alpha(200),
        );
    }
}

/// A drawn arrow for the move buttons.
fn arrow(ui: &egui::Ui, center: Pos2, direction: f32, enabled: bool) {
    let color = if enabled {
        CREAM
    } else {
        CREAM.gamma_multiply(0.35)
    };
    let stroke = Stroke::new(1.8_f32, color);
    let tip = center + Vec2::new(5.0 * direction, 0.0);
    ui.painter()
        .line_segment([center - Vec2::new(5.0 * direction, 0.0), tip], stroke);
    for rise in [-4.0, 4.0] {
        ui.painter()
            .line_segment([tip, tip + Vec2::new(-4.0 * direction, rise)], stroke);
    }
}

fn knob_count(kind: PedalKind) -> usize {
    match kind {
        PedalKind::EjayEq => 10,
        PedalKind::EjayCompressor => 3,
        PedalKind::EjayWide => 1,
    }
}

fn plain_knob(
    ui: &mut egui::Ui,
    positions: [Pos2; 3],
    value: u16,
    name: &str,
    label: &str,
    enabled: bool,
) -> u16 {
    let mut draft = f64::from(value);
    knob(
        ui,
        positions,
        &mut draft,
        Knob {
            name,
            label,
            high: f64::from(KNOB_MAX),
            reset: f64::from(KNOB_MAX / 2),
            format: Format::Plain,
        },
        enabled,
    );
    draft as u16
}

#[derive(Clone, Copy)]
enum Format {
    Plain,
    /// The gain of an equaliser band, shown in decibels.
    Decibels(usize),
}

struct Knob<'a> {
    name: &'a str,
    label: &'a str,
    high: f64,
    reset: f64,
    format: Format,
}

/// A rotary over whole steps from 0 to `high`, its value well, and its label.
fn knob(ui: &mut egui::Ui, positions: [Pos2; 3], value: &mut f64, spec: Knob<'_>, enabled: bool) {
    let [center, well, label] = positions;
    ui.painter().text(
        label,
        Align2::CENTER_CENTER,
        spec.label,
        FontId::proportional(11.5),
        if ui.visuals().dark_mode { CREAM } else { INK },
    );
    controls::well(ui, Rect::from_center_size(well, Vec2::new(50.0, 20.0)));
    ui.add_enabled_ui(enabled, |ui| {
        let before = *value;
        controls::rotary(
            ui,
            Rect::from_center_size(center, Vec2::splat(44.0)),
            value,
            controls::RotarySpec {
                name: spec.name,
                low: 0.0,
                high: spec.high,
                reset: spec.reset,
                step: 1.0,
                decimals: 0,
                logarithmic: false,
                choices: &[],
            },
        );
        // Settings are whole steps. A fine step still moves one.
        *value = if *value > before {
            value.ceil()
        } else if *value < before {
            value.floor()
        } else {
            value.round()
        }
        .clamp(0.0, spec.high);
        ui.scope(|ui| {
            controls::value_style(ui);
            let mut entry = egui::DragValue::new(value).range(0.0..=spec.high).speed(1.0);
            let hint = match spec.format {
                Format::Plain => {
                    entry = entry.fixed_decimals(0);
                    format!(
                        "{}: 0 to {}. Drag, or click to enter a value.",
                        spec.name, spec.high
                    )
                }
                Format::Decibels(band) => {
                    let range = band_range_db(band);
                    let high = spec.high;
                    entry = entry
                        .custom_formatter(move |value, _| format!("{:+.1}", band_db(band, value)))
                        .custom_parser(move |text| {
                            let db: f64 = text.trim().trim_end_matches("dB").trim().parse().ok()?;
                            Some(((db + range) / (2.0 * range) * high).round())
                        });
                    format!(
                        "{}: -{range:.0} to +{range:.0} dB. Drag, or click to enter decibels. The middle position is flat.",
                        spec.name
                    )
                }
            };
            let response = ui.put(Rect::from_center_size(well, Vec2::new(48.0, 24.0)), entry);
            name_control(response, &format!("{} value", spec.name)).on_hover_text(hint);
        });
    });
}

fn footer(ui: &mut egui::Ui, rect: Rect, text: &str) {
    controls::well(ui, rect);
    let response = ui.interact(rect, ui.id().with("master-status"), Sense::hover());
    named(&response, egui::WidgetType::Label, text);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(12.5),
        CREAM,
    );
}
