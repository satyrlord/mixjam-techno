//! Readouts and simple widgets: status text, progress, numbers, and the
//! disclosure header and radio button.
use super::*;

/// A tool footer's status text in `rect`, centered while it fits and scrolled
/// from its top when it does not. A new message scrolls back to the top. Screen
/// readers hear each change politely, and an error with an "Error:" prefix.
pub(crate) fn status_text(
    ui: &mut egui::Ui,
    rect: Rect,
    id_salt: &str,
    text: &str,
    size: f32,
    error: bool,
    hover: &str,
) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    let previous_id = ui.id().with("previous-status");
    let changed = ui
        .ctx()
        .data(|data| data.get_temp::<String>(previous_id).as_deref() != Some(text));
    ui.ctx()
        .data_mut(|data| data.insert_temp(previous_id, text.to_owned()));
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .min_scrolled_height(0.0)
        .max_height(rect.height())
        .auto_shrink([false, false]);
    if changed {
        scroll = scroll.vertical_scroll_offset(0.0);
    }
    scroll.show(&mut child, |ui| {
        let color = if error { roles().coral } else { roles().legend };
        let mut job = egui::text::LayoutJob::simple(
            text.into(),
            FontId::proportional(size),
            color,
            ui.available_width(),
        );
        job.halign = egui::Align::Center;
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        let (area, label) = ui.allocate_exact_size(
            Vec2::new(
                ui.available_width(),
                galley.mesh_bounds.height().max(rect.height()),
            ),
            Sense::hover(),
        );
        let origin = if galley.mesh_bounds.height() <= rect.height() {
            area.center() - galley.mesh_bounds.center().to_vec2()
        } else {
            Pos2::new(
                area.center().x - galley.mesh_bounds.center().x,
                area.top() - galley.mesh_bounds.top(),
            )
        };
        ui.painter().galley(origin, galley, color);
        let spoken = if error {
            format!("Error: {text}")
        } else {
            text.into()
        };
        label.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, label.enabled(), &spoken)
        });
        ui.ctx().accesskit_node_builder(label.id, |node| {
            node.set_live(egui::accesskit::Live::Polite)
        });
        label.hover_hint(hover);
    });
}

/// Keep status text above the fill so contrast does not change with progress.
/// No numeric value is reported until the total amount of work is known.
pub(crate) fn progress(ui: &mut egui::Ui, label: &str, fraction: Option<f32>, width: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::hover());
    let text =
        ui.painter()
            .layout_no_wrap(label.into(), FontId::proportional(13.0), roles().legend);
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .galley(
            Pos2::new(rect.center().x - text.size().x / 2.0, rect.top()),
            text,
            roles().legend,
        );
    let rail = Rect::from_min_max(
        rect.left_bottom() - Vec2::new(0.0, 10.0),
        rect.right_bottom(),
    );
    well(ui, rail);
    let inner = rail.shrink(2.0);
    let fill = if let Some(fraction) = fraction {
        Rect::from_min_size(
            inner.min,
            Vec2::new(inner.width() * fraction.clamp(0.0, 1.0), inner.height()),
        )
    } else {
        let phase = ui.input(|input| (input.time * 1.8).sin() as f32 * 0.5 + 0.5);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
        Rect::from_min_size(
            inner.min + Vec2::new(inner.width() * 0.75 * phase, 0.0),
            Vec2::new(inner.width() * 0.25, inner.height()),
        )
    };
    if fill.width() > 0.0 {
        ui.painter().rect_filled(fill, 1.0, print_color(ui));
    }
    response.widget_info(|| {
        let mut info =
            egui::WidgetInfo::labeled(egui::WidgetType::ProgressIndicator, ui.is_enabled(), label);
        info.value = fraction.map(|value| f64::from(value.clamp(0.0, 1.0)) * 100.0);
        info
    });
}

pub(crate) fn value_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.override_font_id = Some(FontId::proportional(13.0));
    style.drag_value_text_style = egui::TextStyle::Body;
    style.spacing.button_padding = Vec2::new(2.0, 0.0);
    // Compact wells have an 18-point inner rectangle. A larger native minimum
    // grows the invisible value button below that rectangle and shifts its text.
    style.spacing.interact_size.y = 18.0;
    style.wrap_mode = Some(egui::TextWrapMode::Extend);
    style.visuals.override_text_color = Some(roles().legend);
    style.visuals.extreme_bg_color = roles().ink;
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.bg_fill = Color32::TRANSPARENT;
        widget.weak_bg_fill = Color32::TRANSPARENT;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke.color = roles().legend;
    }
    // egui paints a focused or dragged value with these visuals. The edge
    // shows keyboard focus on a field that is not yet being typed in.
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, roles().legend);
}

/// Compact editable value for ordinary dialog forms.
pub(crate) fn number(
    ui: &mut egui::Ui,
    value: &mut f64,
    range: RangeInclusive<f64>,
    name: &str,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(104.0, 30.0), Sense::hover());
    well(ui, rect);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(name)
            .max_rect(rect.shrink(3.0)),
    );
    value_style(&mut child);
    let response = child.add_sized(
        rect.shrink(3.0).size(),
        egui::DragValue::new(value).range(range),
    );
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(name));
    response
}

pub(crate) fn disclosure(label: impl Into<String>) -> egui::CollapsingHeader {
    egui::CollapsingHeader::new(egui::RichText::new(label.into()).color(roles().legend))
}

pub(crate) fn radio(ui: &mut egui::Ui, selected: bool, label: &str) -> Response {
    ui.add(egui::RadioButton::new(
        selected,
        egui::RichText::new(label).color(roles().legend),
    ))
}
