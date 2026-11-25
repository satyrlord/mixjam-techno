//! Sample Editor body. Session owns sample processing and saving.
use super::rect_in;
use super::*;
use mixjam_techno::application::MAX_PITCH_CENTS;

/// The editor's pitch range in semitones: the engine's pitch range.
const PITCH_SEMITONES: std::ops::RangeInclusive<f64> =
    -(MAX_PITCH_CENTS as f64) / 100.0..=MAX_PITCH_CENTS as f64 / 100.0;
const WINDOW_SIZE: Vec2 = Vec2::new(640.0, 368.0);
const PAD_X: f32 = 12.0;
const INNER_W: f32 = 616.0;
const TRIM_ACCENT: Color32 = Color32::from_rgb(108, 198, 224);
const PITCH_ACCENT: Color32 = Color32::from_rgb(236, 166, 76);

pub(super) struct Source<'a> {
    pub name: &'a str,
    pub frames: u64,
    pub sample_rate: u32,
    pub available: bool,
    pub missing: bool,
}

#[derive(Debug, PartialEq)]
pub(super) enum Action {
    None,
    Listen,
    Save,
    Details,
    Help,
}

pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    start: &mut u64,
    end: &mut u64,
    pitch: &mut f64,
) -> Action {
    let mut action = Action::None;
    let response = dialog::tool(
        ctx,
        "Sample editor",
        open,
        WINDOW_SIZE,
        controls::Palette::Granite,
        true,
        |ui| {
            action = draw_body(ui, &source, start, end, pitch);
        },
    );
    if response.help { Action::Help } else { action }
}

fn draw_body(
    ui: &mut egui::Ui,
    source: &Source<'_>,
    start: &mut u64,
    end: &mut u64,
    pitch: &mut f64,
) -> Action {
    let (bounds, mut panel) = super::tool_panel(ui, WINDOW_SIZE);
    draw_source_strip(&mut panel, bounds, source);
    draw_trim_bay(&mut panel, bounds, source, start, end);
    draw_pitch_bay(&mut panel, bounds, source, pitch);
    let reason = readiness_reason(source, *start, *end, *pitch);
    let action = draw_actions(&mut panel, bounds, reason.as_deref());
    if draw_footer(
        &mut panel,
        bounds,
        source,
        *start,
        *end,
        *pitch,
        reason.as_deref(),
    ) {
        return Action::Details;
    }
    action
}

fn draw_source_strip(ui: &mut egui::Ui, bounds: Rect, source: &Source<'_>) {
    let strip = rect_in(bounds, PAD_X, 6.0, INNER_W, 32.0);
    controls::well(ui, strip);
    let has_meta = source.available && source.frames > 0 && source.sample_rate > 0;
    let name_w = if has_meta { 456.0 } else { 596.0 };
    let name = ui.put(
        rect_in(bounds, PAD_X + 10.0, 9.0, name_w, 26.0),
        egui::Label::new(
            egui::RichText::new(source.name)
                .size(13.0)
                .color(roles().legend),
        )
        .truncate(),
    );
    name_control(name.clone(), "Source sound");
    ui.ctx()
        .accesskit_node_builder(name.id, |node| node.set_value(source.name));
    name.hover_hint(source.name);
    if has_meta {
        let secs = source.frames as f64 / f64::from(source.sample_rate);
        let meta = format!("{secs:.2} s · {} Hz", source.sample_rate);
        ui.painter().text(
            Pos2::new(strip.right() - 10.0, strip.center().y),
            Align2::RIGHT_CENTER,
            meta,
            FontId::proportional(11.0),
            roles().legend.gamma_multiply(0.75),
        );
    }
}

fn draw_trim_bay(
    ui: &mut egui::Ui,
    bounds: Rect,
    source: &Source<'_>,
    start: &mut u64,
    end: &mut u64,
) {
    let bay = rect_in(bounds, PAD_X, 48.0, INNER_W, 138.0);
    controls::instrument::bay(ui, bay, "Trim Range", TRIM_ACCENT, false);
    paint_trim_bar(
        ui,
        rect_in(bounds, 26.0, 76.0, 588.0, 36.0),
        source,
        *start,
        *end,
    );
    label(ui, bounds.min + Vec2::new(26.0, 146.0), "Start frame", 13.0);
    label(ui, bounds.min + Vec2::new(332.0, 146.0), "End frame", 13.0);
    ui.add_enabled_ui(source.available && source.frames > 0, |ui| {
        frame_value(
            ui,
            rect_in(bounds, 116.0, 122.0, 186.0, 48.0),
            "Start frame",
            start,
            0..=source.frames.saturating_sub(1),
        );
        frame_value(
            ui,
            rect_in(bounds, 422.0, 122.0, 186.0, 48.0),
            "End frame",
            end,
            1..=source.frames.max(1),
        );
    });
}

fn paint_trim_bar(ui: &egui::Ui, rect: Rect, source: &Source<'_>, start: u64, end: u64) {
    let (_, trace, grid, _) = controls::instrument::scope_well(ui, rect, TRIM_ACCENT);
    let plot = rect.shrink2(Vec2::new(6.0, 5.0));
    let painter = ui.painter();
    painter.line_segment(
        [plot.left_center(), plot.right_center()],
        Stroke::new(1.0_f32, grid),
    );
    controls::instrument::draw_quarter_grid(painter, plot, grid);
    paint_trim_selection(painter, plot, source, start, end, trace);
}

fn paint_trim_selection(
    painter: &egui::Painter,
    plot: Rect,
    source: &Source<'_>,
    start: u64,
    end: u64,
    trace: Color32,
) {
    let legend = roles().legend;
    if !source.available || source.missing || source.frames == 0 {
        painter.text(
            plot.center(),
            Align2::CENTER_CENTER,
            "No active audio",
            FontId::proportional(11.0),
            legend.gamma_multiply(0.65),
        );
        return;
    }
    if start >= end || end > source.frames {
        painter.text(
            plot.center(),
            Align2::CENTER_CENTER,
            "Invalid trim range",
            FontId::proportional(11.0),
            roles().coral,
        );
        return;
    }
    let f0 = (start as f32 / source.frames as f32).clamp(0.0, 1.0);
    let f1 = (end as f32 / source.frames as f32).clamp(0.0, 1.0);
    let x0 = plot.left() + f0 * plot.width();
    let x1 = (plot.left() + f1 * plot.width()).max(x0 + 2.0);
    let active = Rect::from_min_max(Pos2::new(x0, plot.top()), Pos2::new(x1, plot.bottom()));
    painter.rect_filled(
        active,
        2.0,
        Color32::from_rgba_unmultiplied(trace.r(), trace.g(), trace.b(), 44),
    );
    for edge in [
        [active.left_top(), active.right_top()],
        [active.left_bottom(), active.right_bottom()],
    ] {
        painter.line_segment(edge, Stroke::new(1.2_f32, trace));
    }
    for x in [x0, x1] {
        painter.line_segment(
            [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
            Stroke::new(2.0_f32, trace),
        );
    }
    painter.text(
        Pos2::new(plot.right() - 4.0, plot.center().y),
        Align2::RIGHT_CENTER,
        format!("{:.0}% · {} fr", (f1 - f0) * 100.0, end - start),
        FontId::proportional(11.0),
        legend,
    );
}

fn draw_pitch_bay(ui: &mut egui::Ui, bounds: Rect, source: &Source<'_>, pitch: &mut f64) {
    let bay = rect_in(bounds, PAD_X, 196.0, INNER_W, 78.0);
    controls::instrument::bay(ui, bay, "Pitch & Speed", PITCH_ACCENT, false);
    label(
        ui,
        bounds.min + Vec2::new(26.0, 242.0),
        "Pitch (semitones)",
        13.0,
    );
    let rail_rect = rect_in(bounds, 148.0, 224.0, 250.0, 36.0);
    ui.add_enabled_ui(source.available && source.frames > 0, |ui| {
        pitch_rail(ui, rail_rect, pitch);
        pitch_value(ui, rect_in(bounds, 412.0, 218.0, 114.0, 48.0), pitch);
    });
    let mid_x = rail_rect.center().x;
    ui.painter().line_segment(
        [
            Pos2::new(mid_x, rail_rect.bottom() - 4.0),
            Pos2::new(mid_x, rail_rect.bottom() + 1.0),
        ],
        Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.45)),
    );
    for (x, align, text) in [
        (
            rail_rect.left() + 4.0,
            Align2::LEFT_CENTER,
            format!("{:+}", PITCH_SEMITONES.start()),
        ),
        (mid_x, Align2::CENTER_CENTER, "0".to_owned()),
        (
            rail_rect.right() - 4.0,
            Align2::RIGHT_CENTER,
            format!("{:+}", PITCH_SEMITONES.end()),
        ),
    ] {
        ui.painter().text(
            Pos2::new(x, bounds.top() + 266.0),
            align,
            text,
            FontId::proportional(9.5),
            roles().legend.gamma_multiply(0.65),
        );
    }
    let ratio_well = rect_in(bounds, 536.0, 227.0, 78.0, 30.0);
    controls::well(ui, ratio_well);
    let speed = 2.0_f64.powf(*pitch / 12.0);
    let ratio_color = if pitch.abs() > 0.05 {
        PITCH_ACCENT
    } else {
        roles().legend
    };
    ui.painter().text(
        ratio_well.center(),
        Align2::CENTER_CENTER,
        format!("{speed:.2}x"),
        FontId::proportional(12.5),
        ratio_color,
    );
}

fn readiness_reason(source: &Source<'_>, start: u64, end: u64, pitch: f64) -> Option<String> {
    if source.missing {
        Some(
            "This sound is missing and plays silence. Open Import details to \
             inspect or resolve it."
                .into(),
        )
    } else if !source.available || source.frames == 0 {
        Some("Select a nonempty sound or clip before editing.".into())
    } else if start >= end || end > source.frames {
        Some("End frame must be greater than Start frame and within the source.".into())
    } else if !pitch.is_finite() || !PITCH_SEMITONES.contains(&pitch) {
        Some(format!(
            "Pitch must be from {} to {} semitones.",
            PITCH_SEMITONES.start(),
            PITCH_SEMITONES.end()
        ))
    } else {
        None
    }
}

fn draw_actions(ui: &mut egui::Ui, bounds: Rect, reason: Option<&str>) -> Action {
    let ready = reason.is_none();
    let mut action = Action::None;
    let listen = action_button(
        ui,
        rect_in(bounds, PAD_X, 284.0, 303.0, 36.0),
        "Listen edit",
        false,
        ready,
    );
    if listen
        .hover_hint("Preview the selected frames at this pitch. The source stays unchanged.")
        .disabled_hint(reason.unwrap_or_default())
        .clicked()
    {
        action = Action::Listen;
    }
    let save = action_button(
        ui,
        rect_in(bounds, 325.0, 284.0, 303.0, 36.0),
        "Save edited sound",
        true,
        ready,
    );
    if save
        .hover_hint("Save the edit as a new WAV in Wave. The original source stays unchanged.")
        .disabled_hint(reason.unwrap_or_default())
        .clicked()
    {
        action = Action::Save;
    }
    action
}

fn draw_footer(
    ui: &mut egui::Ui,
    bounds: Rect,
    source: &Source<'_>,
    start: u64,
    end: u64,
    pitch: f64,
    reason: Option<&str>,
) -> bool {
    let summary = reason.map(str::to_owned).unwrap_or_else(|| {
        let selected = end - start;
        if source.sample_rate > 0 {
            format!(
                "{selected} source frames selected · {:.3} s after pitch",
                (selected as f64 / 2.0_f64.powf(pitch / 12.0))
                    .round()
                    .max(1.0)
                    / f64::from(source.sample_rate)
            )
        } else {
            format!("{selected} source frames selected")
        }
    });
    let footer_rect = rect_in(bounds, PAD_X, 328.0, INNER_W, 36.0);
    controls::status_well(ui, footer_rect, reason.is_some());
    let width = if source.missing { 464.0 } else { 596.0 };
    let color = if reason.is_some() {
        roles().coral
    } else {
        roles().legend
    };
    let status = ui.put(
        rect_in(bounds, 22.0, 328.0, width, 36.0),
        egui::Label::new(egui::RichText::new(&summary).size(12.5).color(color)).wrap(),
    );
    name_control(status.clone(), "Sample editor status");
    ui.ctx().accesskit_node_builder(status.id, |node| {
        node.set_value(summary.as_str());
        node.set_live(egui::accesskit::Live::Polite);
    });
    status.hover_hint(&summary);
    source.missing
        && action_button(
            ui,
            rect_in(bounds, 494.0, 332.0, 128.0, 28.0),
            "Import details",
            false,
            true,
        )
        .clicked()
}

fn label(ui: &egui::Ui, at: Pos2, value: &str, size: f32) {
    ui.painter().text(
        at,
        Align2::LEFT_CENTER,
        value,
        FontId::proportional(size),
        roles().legend,
    );
}

fn action_button(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    primary: bool,
    enabled: bool,
) -> Response {
    controls::button_at(
        ui,
        rect,
        name,
        name,
        if primary {
            controls::ButtonRole::Primary
        } else {
            controls::ButtonRole::Secondary
        },
        enabled,
    )
}

fn stepper(ui: &mut egui::Ui, rect: Rect, name: &str, up: bool) -> Response {
    let response = controls::button_at(
        ui,
        rect,
        name,
        if up { "+" } else { "−" },
        controls::ButtonRole::Secondary,
        true,
    );
    name_control(response, name)
}

fn frame_value(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    value: &mut u64,
    range: std::ops::RangeInclusive<u64>,
) {
    controls::well(ui, rect);
    ui.scope_builder(
        egui::UiBuilder::new()
            .id(name)
            .max_rect(Rect::from_min_max(
                rect.min + Vec2::new(6.0, 10.0),
                rect.max - Vec2::new(28.0, 10.0),
            ))
            .layout(egui::Layout::centered_and_justified(
                egui::Direction::LeftToRight,
            )),
        |ui| {
            controls::value_style(ui);
            let size = ui.available_size();
            ui.spacing_mut().interact_size = size;
            let response = ui.add(
                egui::DragValue::new(value)
                    .range(range.clone())
                    .clamp_existing_to_range(false)
                    .speed(100)
                    .fixed_decimals(0)
                    .update_while_editing(false),
            );
            if response.changed() {
                *value = (*value).clamp(*range.start(), *range.end());
            }
            name_control(response, name).hover_hint(if name == "Start frame" {
                "First included source frame (zero-based). Click for exact entry; Enter \
                 applies. Use the adjacent buttons for one-frame steps (Shift: 100)."
            } else {
                "First excluded source frame. Set this to the source frame count to include \
                 the last frame. Click for exact entry; Enter applies. Use the adjacent \
                 buttons for one-frame steps (Shift: 100)."
            });
        },
    );
    for (up, y) in [(true, 0.0), (false, 24.0)] {
        let label = format!("{} {name}", if up { "Increase" } else { "Decrease" });
        let button = stepper(
            ui,
            Rect::from_min_size(
                Pos2::new(rect.right() - 25.0, rect.top() + y),
                Vec2::new(24.0, 24.0),
            ),
            &label,
            up,
        );
        if button
            .hover_hint("Change one frame. Hold Shift to change 100 frames.")
            .clicked()
        {
            let amount = if ui.input(|input| input.modifiers.shift) {
                100
            } else {
                1
            };
            *value = if up {
                value.saturating_add(amount)
            } else {
                value.saturating_sub(amount)
            };
            *value = (*value).clamp(*range.start(), *range.end());
        }
    }
}

fn pitch_value(ui: &mut egui::Ui, rect: Rect, pitch: &mut f64) {
    controls::well(ui, rect);
    ui.scope_builder(
        egui::UiBuilder::new()
            .id("pitch-value")
            .max_rect(Rect::from_min_max(
                rect.min + Vec2::new(6.0, 10.0),
                rect.max - Vec2::new(28.0, 10.0),
            ))
            .layout(egui::Layout::centered_and_justified(
                egui::Direction::LeftToRight,
            )),
        |ui| {
            controls::value_style(ui);
            let size = ui.available_size();
            ui.spacing_mut().interact_size = size;
            let id = ui.next_auto_id();
            let context = ui.ctx().clone();
            let response = ui.add(
                egui::DragValue::new(pitch)
                    .range(PITCH_SEMITONES)
                    .clamp_existing_to_range(false)
                    .speed(0.1)
                    .custom_formatter(move |value, _| {
                        if context.memory(|memory| memory.has_focus(id)) {
                            format!("{value}")
                        } else {
                            format!("{value:.1}")
                        }
                    })
                    .update_while_editing(false),
            );
            if response.changed() {
                *pitch = pitch.clamp(*PITCH_SEMITONES.start(), *PITCH_SEMITONES.end());
            }
            name_control(response, "Pitch value").hover_hint(
                "Enter semitones from -24 to +24. Pitch changes playback speed and output \
                 duration together. Zero keeps the source pitch.",
            );
        },
    );
    for (up, y) in [(true, 0.0), (false, 24.0)] {
        let name = if up {
            "Increase Pitch"
        } else {
            "Decrease Pitch"
        };
        let button = stepper(
            ui,
            Rect::from_min_size(
                Pos2::new(rect.right() - 25.0, rect.top() + y),
                Vec2::new(24.0, 24.0),
            ),
            name,
            up,
        );
        if button
            .hover_hint("Change 0.1 semitone. Hold Shift for one semitone.")
            .clicked()
        {
            let amount = if ui.input(|input| input.modifiers.shift) {
                1.0
            } else {
                0.1
            };
            *pitch += if up { amount } else { -amount };
            *pitch = pitch.clamp(*PITCH_SEMITONES.start(), *PITCH_SEMITONES.end());
        }
    }
}

fn pitch_rail(ui: &mut egui::Ui, rect: Rect, pitch: &mut f64) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .id("pitch-rail")
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.spacing_mut().slider_width = rect.width();
            controls::slider(ui, pitch, PITCH_SEMITONES, "Pitch (semitones)").hover_hint(
                "Pitch changes playback speed and sample length. +12 is one octave up and half \
                 the duration; -12 is one octave down and twice the duration. Use the numeric \
                 value for exact entry.",
            );
        },
    );
}
