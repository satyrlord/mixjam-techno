//! The board's smaller parts: dials, the Length well, the toggle, the status
//! well, the styles of its forms and scroll bars, and its lettering.
use super::*;

/// A dial in a cell: the knob at the left, its name over its value well.
pub(super) struct Dial<'a> {
    pub(super) name: &'a str,
    pub(super) low: f64,
    pub(super) high: f64,
    pub(super) reset: f64,
    pub(super) step: f64,
    pub(super) decimals: usize,
    pub(super) logarithmic: bool,
}

pub(super) fn dial(ui: &mut egui::Ui, cell: Rect, value: &mut f64, spec: Dial<'_>, hint: &str) {
    let center = Pos2::new(cell.left() + 22.0, cell.center().y);
    let well = Pos2::new(cell.right() - 27.0, cell.bottom() - 11.0);
    left_text(
        ui,
        Pos2::new(cell.left() + 49.0, cell.top() + 9.0),
        spec.name,
        12.0,
        roles().legend,
    );
    controls::rotary(
        ui,
        Rect::from_center_size(center, Vec2::splat(44.0)),
        value,
        controls::RotarySpec {
            name: spec.name,
            low: spec.low,
            high: spec.high,
            reset: spec.reset,
            step: spec.step,
            decimals: spec.decimals,
            logarithmic: spec.logarithmic,
            choices: &[],
        },
    )
    .hover_hint(hint);
    controls::well(ui, Rect::from_center_size(well, Vec2::new(54.0, 20.0)));
    ui.scope(|ui| {
        controls::value_style(ui);
        let response = ui.put(
            Rect::from_center_size(well, Vec2::new(52.0, 22.0)),
            egui::DragValue::new(value)
                .range(spec.low..=spec.high)
                .clamp_existing_to_range(false)
                .speed(spec.step)
                .fixed_decimals(spec.decimals),
        );
        name_control(response, &format!("{} value", spec.name))
            .hover_hint(format!("{}: drag, or click to enter a value.", spec.name));
    });
}

/// The pattern length as a value well: drag sideways or scroll to change it,
/// or use the arrow keys, Home, and End.
pub(super) fn length_control(ui: &mut egui::Ui, rect: Rect, length: &mut u8) {
    let response = ui.interact(rect, ui.id().with("length"), Sense::click_and_drag());
    let mut value = i64::from(*length);
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    let drag_id = response.id.with("drag");
    if response.drag_started() {
        ui.ctx()
            .data_mut(|data| data.insert_temp(drag_id, (value, 0.0_f32)));
    }
    if response.dragged()
        && let Some((start, travel)) = ui.ctx().data(|data| data.get_temp::<(i64, f32)>(drag_id))
    {
        let travel = travel + response.drag_delta().x - response.drag_delta().y;
        ui.ctx()
            .data_mut(|data| data.insert_temp(drag_id, (start, travel)));
        value = start + (travel / 6.0).round() as i64;
    }
    value += controls::wheel(ui, &response) as i64;
    if response.has_focus() {
        lock_arrows(ui, &response);
        ui.input_mut(|input| {
            for key in [egui::Key::ArrowUp, egui::Key::ArrowRight] {
                if input.consume_key(egui::Modifiers::NONE, key) {
                    value += 1;
                }
            }
            for key in [egui::Key::ArrowDown, egui::Key::ArrowLeft] {
                if input.consume_key(egui::Modifiers::NONE, key) {
                    value -= 1;
                }
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
                value = i64::from(*pattern::LENGTH_RANGE.start());
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
                value = i64::from(*pattern::LENGTH_RANGE.end());
            }
        });
    }
    ui.input(|input| {
        use egui::accesskit::{Action, ActionData};
        for request in input.accesskit_action_requests(response.id, Action::SetValue) {
            if let Some(ActionData::NumericValue(draft)) = request.data
                && draft.is_finite()
            {
                value = draft.round() as i64;
            }
        }
        value += input.num_accesskit_action_requests(response.id, Action::Increment) as i64
            - input.num_accesskit_action_requests(response.id, Action::Decrement) as i64;
    });
    *length = pattern::clamp_length(value);
    controls::well(ui, rect);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("{length}"),
        FontId::proportional(13.0),
        roles().legend,
    );
    focus(ui, &response, response.has_focus(), roles().legend);
    response.widget_info(|| egui::WidgetInfo::slider(true, f64::from(*length), "Length"));
    name_control(response, "Length").hover_hint(
        "The steps the pattern repeats over, from 1 to 64. Drag the end marker on the step \
         ruler, or drag or scroll here. Steps after the length keep their notes.",
    );
}

/// Keep the arrow keys in a focused control, so they do not move the focus.
pub(super) fn lock_arrows(ui: &egui::Ui, response: &Response) {
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

pub(super) fn toggle(
    ui: &mut egui::Ui,
    center: Pos2,
    label_at: Pos2,
    name: &str,
    value: &mut bool,
) {
    let hit = Rect::from_min_size(
        center - Vec2::new(14.0, 13.0),
        Vec2::new((label_at.x - center.x + 90.0).max(110.0), 26.0),
    );
    // The compact source row places its label separately from the switch,
    // so the full text fits before the settings column's scroll bar.
    let response = controls::toggle_at(ui, hit, name, "", value);
    left_text(ui, label_at, name, 13.0, roles().legend);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, response.enabled(), *value, name)
    });
    let status = format!("{name}: {}", if *value { "on" } else { "off" });
    let tooltip = match name {
        "Loop source" => format!(
            "{status}\n\n\
             On: Repeats the entire source audio from its beginning whenever it reaches the end, \
             for as long as the current note is allowed to play. It does not use embedded loop points.\n\n\
             Off: Plays the source once per note. If it ends before the next note, the remaining interval has no source audio.\n\n\
             Every new note restarts the source, unless the note before it slides into it. \
             Repetition lasts through the note's ties and its gate. \
             Loop source does not repeat the pattern or lengthen the sound."
        ),
        _ => status,
    };
    response.hover_hint(tooltip);
}

pub(super) fn status_footer(ui: &mut egui::Ui, rect: Rect, text: &str, error: bool, missing: bool) {
    controls::well(ui, rect);
    let response = ui.interact(rect, ui.id().with("status"), Sense::hover());
    named(&response, egui::WidgetType::Panel, "Step synth status");
    let text_rect = Rect::from_min_max(
        rect.min + Vec2::new(9.0, 5.0),
        rect.max - Vec2::new(if missing { 144.0 } else { 9.0 }, 5.0),
    );
    controls::status_text(ui, text_rect, "step-synth-status", text, 12.5, error, text);
}

pub(super) fn scroll_style(ui: &mut egui::Ui) {
    let scroll = &mut ui.style_mut().spacing.scroll;
    scroll.floating = false;
    scroll.bar_width = 6.0;
    scroll.bar_inner_margin = 4.0;
    scroll.bar_outer_margin = 0.0;
}

pub(super) fn form_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.override_font_id = Some(FontId::proportional(13.0));
    style.spacing.interact_size.y = 26.0;
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.slider_width = 180.0;
    style.visuals.override_text_color = Some(roles().legend);
    style.visuals.panel_fill = roles().face;
    style.visuals.extreme_bg_color = roles().face;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, roles().legend);
    for widgets in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widgets.bg_fill = roles().face;
        widgets.weak_bg_fill = roles().face;
        widgets.bg_stroke = Stroke::new(1.0_f32, roles().tool_edge);
        widgets.fg_stroke = Stroke::new(1.0_f32, roles().legend);
    }
}

/// Text centred on `center`, on whole pixels so that it does not jitter.
pub(super) fn lettering(ui: &egui::Ui, center: Pos2, text: &str, size: f32, color: Color32) {
    let font = FontId::proportional(size);
    let galley = ui.painter().layout_no_wrap(text.into(), font, color);
    let offset = Vec2::new(galley.mesh_bounds.center().x, galley.rect.center().y);
    let at =
        egui::emath::GuiRounding::round_to_pixels(center - offset, ui.ctx().pixels_per_point());
    ui.painter().galley(at, galley, color);
}

pub(super) fn left_text(ui: &egui::Ui, at: Pos2, text: &str, size: f32, color: Color32) {
    ui.painter().text(
        at,
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(size),
        color,
    );
}

/// The focus ring of a focused control, or the hover ring of an enabled one.
pub(super) fn focus(ui: &egui::Ui, response: &Response, focused: bool, color: Color32) {
    if focused || (response.hovered() && response.enabled()) {
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            2.0,
            Stroke::new(if focused { 2.0 } else { 1.0_f32 }, color),
            egui::StrokeKind::Inside,
        );
    }
}
