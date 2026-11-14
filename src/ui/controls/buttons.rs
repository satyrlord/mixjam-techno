//! Buttons: the shared button faces by role, the header icon buttons, the
//! toggle, and the keyboard focus ring they share.
use super::*;

pub(crate) fn icon_button_surface(ui: &egui::Ui, response: &Response, active: bool) {
    paint_button(ui, response, "", ButtonRole::Secondary);
    if active {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            5.0,
            Stroke::new(1.0_f32, print_color(ui)),
            egui::StrokeKind::Inside,
        );
    }
}

/// The palette's accent is reserved for focus, selection and active indicators.
pub(crate) fn print_color(ui: &egui::Ui) -> Color32 {
    ui.visuals().selection.bg_fill
}

fn focus(ui: &egui::Ui, response: &Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            5.0,
            Stroke::new(2.0_f32, focus_color(ui)),
            egui::StrokeKind::Outside,
        );
    }
}

pub(crate) fn focus_color(ui: &egui::Ui) -> Color32 {
    Palette::of(ui).focus()
}

fn paint_button(ui: &egui::Ui, response: &Response, label: &str, role: ButtonRole) {
    let rect = response.rect;
    let down = response.is_pointer_button_down_on();
    let base = match role {
        ButtonRole::Secondary | ButtonRole::Tertiary => roles().face,
        ButtonRole::Primary => roles().primary,
        ButtonRole::Danger => roles().danger,
    };
    let base = if !response.enabled() {
        roles().disabled_face
    } else if down {
        base.gamma_multiply(0.8)
    } else if response.hovered() {
        base.lerp_to_gamma(Color32::WHITE, 0.09)
    } else {
        base
    };
    let painter = ui.painter();
    let quiet = matches!(role, ButtonRole::Tertiary);
    let raised = !quiet || (response.enabled() && (response.hovered() || down));
    let material = roles().material;
    // Every theme's key face, and so its label, sits in the same place.
    let face = rect
        .shrink(2.0)
        .translate(Vec2::new(0.0, if down { 1.0 } else { -1.0 }));
    if raised {
        match material {
            Material::Flat => {
                painter.rect_filled(face, 3.0, base);
                painter.rect_stroke(
                    face,
                    3.0,
                    Stroke::new(
                        1.0_f32,
                        if matches!(role, ButtonRole::Secondary | ButtonRole::Tertiary) {
                            roles().widget_edge
                        } else {
                            base.lerp_to_gamma(roles().ink, 0.35)
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
            }
            Material::Plastic => {
                painter.rect_filled(
                    rect.translate(Vec2::new(0.0, 1.0)),
                    9.0,
                    Color32::from_black_alpha(60),
                );
                theme::plastic(painter, face, 8.0, base);
            }
            Material::Metal => {
                painter.rect_filled(rect, 5.0, roles().ink);
                painter.rect_filled(face, 3.0, base);
            }
        }
    }
    if material == Material::Metal && matches!(role, ButtonRole::Primary) && response.enabled() {
        let art = artwork(ui.ctx(), Palette::of(ui));
        let size = art.primary.size_vec2() / ui.ctx().pixels_per_point();
        let mut y = face.top();
        while y < face.bottom() {
            let mut x = face.left();
            while x < face.right() {
                fixed_image(ui, &art.primary, Pos2::new(x, y), face.shrink(2.0));
                x += size.x;
            }
            y += size.y;
        }
        if down {
            painter.rect_filled(face.shrink(2.0), 2.0, Color32::from_black_alpha(25));
        }
    }
    if raised && material == Material::Metal {
        painter.line_segment(
            [
                face.left_top() + Vec2::new(3.0, 1.0),
                face.right_top() + Vec2::new(-3.0, 1.0),
            ],
            Stroke::new(1.0_f32, Color32::from_white_alpha(55)),
        );
    }
    let color = match role {
        ButtonRole::Primary => roles().ink,
        _ => roles().legend,
    };
    let color = if response.enabled() {
        color
    } else {
        roles().disabled_text
    };
    let mut text = egui::RichText::new(label)
        .font(FontId::proportional(13.0))
        .color(color);
    if quiet && !raised {
        text = text.underline();
    }
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (face.width() - 12.0).max(0.0),
        egui::TextStyle::Button,
    );
    painter
        .with_clip_rect(face.intersect(ui.clip_rect()))
        .galley(face.center() - galley.size() / 2.0, galley, color);
    focus(ui, response);
}

pub(crate) fn button(ui: &mut egui::Ui, label: &str, role: ButtonRole) -> Response {
    let width = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(13.0), roles().ink)
        .size()
        .x
        + 26.0;
    let (_, response) = ui.allocate_exact_size(Vec2::new(width.max(72.0), 30.0), Sense::click());
    paint_button(ui, &response, label, role);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response
}

pub(crate) fn button_at(
    ui: &mut egui::Ui,
    rect: Rect,
    key: &str,
    label: &str,
    role: ButtonRole,
    enabled: bool,
) -> Response {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("hardware-button", key))
            .max_rect(rect),
    );
    if !enabled {
        child.disable();
    }
    let response = child.interact(rect, child.id().with(key), Sense::click());
    paint_button(&child, &response, label, role);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response
}

pub(crate) fn header_button(
    ui: &mut egui::Ui,
    rect: Rect,
    key: &str,
    name: &str,
    icon: HeaderIcon,
) -> Response {
    let child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("hardware-header", key))
            .max_rect(rect),
    );
    let response = child.interact(rect, child.id().with(key), Sense::click());
    let center = rect.center();
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(
            rect.shrink(2.0),
            3.0,
            Color32::from_black_alpha(if response.is_pointer_button_down_on() {
                35
            } else {
                16
            }),
        );
    }
    let pen = Stroke::new(1.8_f32, roles().legend);
    match icon {
        HeaderIcon::Close => {
            ui.painter().rect_filled(rect.shrink(2.0), 4.0, roles().ink);
            ui.painter().rect_stroke(
                rect.shrink(3.0),
                3.0,
                Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.35)),
                egui::StrokeKind::Inside,
            );
            ui.painter().line_segment(
                [center + Vec2::new(-6.0, -6.0), center + Vec2::new(6.0, 6.0)],
                pen,
            );
            ui.painter().line_segment(
                [center + Vec2::new(6.0, -6.0), center + Vec2::new(-6.0, 6.0)],
                pen,
            );
        }
        HeaderIcon::Collapse | HeaderIcon::Expand => {
            let vertices = if matches!(icon, HeaderIcon::Collapse) {
                vec![
                    center + Vec2::new(-5.5, -3.5),
                    center + Vec2::new(5.5, -3.5),
                    center + Vec2::new(0.0, 5.0),
                ]
            } else {
                vec![
                    center + Vec2::new(-3.5, -5.5),
                    center + Vec2::new(-3.5, 5.5),
                    center + Vec2::new(5.0, 0.0),
                ]
            };
            ui.painter().add(egui::Shape::convex_polygon(
                vertices,
                roles().legend,
                Stroke::NONE,
            ));
        }
        HeaderIcon::Help => {
            ui.painter().circle_filled(center, 11.0, roles().ink);
            ui.painter().circle_stroke(
                center,
                10.0,
                Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.4)),
            );
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                "?",
                FontId::proportional(15.0),
                roles().legend,
            );
        }
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), name)
    });
    focus(ui, &response);
    response.hover_hint(name)
}

pub(crate) fn toggle_at(
    ui: &mut egui::Ui,
    rect: Rect,
    key: &str,
    label: &str,
    value: &mut bool,
) -> Response {
    let mut response = ui.interact(rect, ui.id().with(key), Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let center = Pos2::new(rect.left() + 14.0, rect.center().y);
    ui.painter().rect_filled(
        Rect::from_center_size(center, Vec2::new(26.0, 14.0)),
        7.0,
        roles().ink,
    );
    ui.painter().circle_filled(
        center + Vec2::new(if *value { 6.0 } else { -6.0 }, -0.5),
        5.5,
        if *value { print_color(ui) } else { roles().cap },
    );
    ui.painter().text(
        Pos2::new(rect.left() + 34.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        roles().legend,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            response.enabled(),
            *value,
            label,
        )
    });
    focus(ui, &response);
    response
}
