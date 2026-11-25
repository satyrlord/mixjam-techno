//! Chassis surfaces: the tool panel and the rail panel in the current theme,
//! the rail separator, and the inset, section, and well frames.
use super::*;

/// A tool chassis in the current theme: a flat software panel with its
/// palette's accent along the top on Studio, molded plastic on Aero, and an
/// olive-sage modular hardware panel in code on Vintage.
pub(crate) fn panel(ui: &egui::Ui, rect: Rect) {
    let palette = Palette::of(ui);
    let p = ui.painter();
    match roles().material {
        Material::Flat => {
            p.rect_filled(rect, 6.0, palette.body());
            p.rect_stroke(
                rect,
                6.0,
                Stroke::new(1.0_f32, roles().ink),
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                rect.shrink(1.0),
                5.0,
                Stroke::new(1.0_f32, palette.body().lerp_to_gamma(roles().legend, 0.1)),
                egui::StrokeKind::Inside,
            );
            p.line_segment(
                [
                    rect.left_top() + Vec2::new(8.0, 2.0),
                    rect.right_top() + Vec2::new(-8.0, 2.0),
                ],
                Stroke::new(2.0_f32, palette.accent()),
            );
        }
        Material::Plastic => theme::plastic(p, rect, 14.0, palette.body()),
        Material::Metal => metal_panel(ui, rect, palette),
    }
}

fn corner_screw(painter: &egui::Painter, center: Pos2) {
    painter.circle_filled(center, 2.3, roles().well_edge);
    painter.circle_stroke(center, 2.3, Stroke::new(0.8_f32, roles().ink));
    painter.line_segment(
        [center - Vec2::new(1.4, 1.0), center + Vec2::new(1.4, 1.0)],
        Stroke::new(0.9_f32, roles().ink),
    );
}

fn paint_metal_brushed_lines(painter: &egui::Painter, rect: Rect, legend: Color32) {
    let inner = rect.shrink(3.0);
    if inner.width() < 24.0 || inner.height() < 16.0 {
        return;
    }
    for share in [0.16_f32, 0.34, 0.56, 0.78] {
        let y = (inner.top() + inner.height() * share).round();
        painter.line_segment(
            [Pos2::new(inner.left(), y), Pos2::new(inner.right(), y)],
            Stroke::new(
                0.7_f32,
                Color32::from_rgba_unmultiplied(legend.r(), legend.g(), legend.b(), 10),
            ),
        );
    }
}

/// Procedural anodized modular plate with rack seam, machined bevel, and corner screws.
fn metal_panel(ui: &egui::Ui, rect: Rect, palette: Palette) {
    let p = ui.painter();
    let body = palette.body();
    p.rect_filled(rect, 7.0, roles().ink);
    theme::gradient(
        p,
        rect.shrink(2.0),
        5.5,
        body.lerp_to_gamma(Color32::WHITE, 0.06),
        body.lerp_to_gamma(Color32::BLACK, 0.08),
    );
    paint_metal_brushed_lines(p, rect, roles().legend);
    p.rect_stroke(
        rect.shrink(1.0),
        6.5,
        Stroke::new(1.5_f32, roles().ink),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        rect.shrink(2.0),
        5.5,
        Stroke::new(1.0_f32, body.lerp_to_gamma(roles().legend, 0.24)),
        egui::StrokeKind::Inside,
    );
    p.line_segment(
        [
            rect.left_top() + Vec2::new(10.0, 2.5),
            rect.right_top() + Vec2::new(-10.0, 2.5),
        ],
        Stroke::new(1.0_f32, Color32::from_white_alpha(58)),
    );
    if rect.height() >= 64.0 && rect.width() >= 96.0 {
        for offset in [
            Vec2::new(7.5, 7.5),
            Vec2::new(rect.width() - 7.5, 7.5),
            Vec2::new(7.5, rect.height() - 7.5),
            Vec2::new(rect.width() - 7.5, rect.height() - 7.5),
        ] {
            corner_screw(p, rect.min + offset);
        }
    }
}

/// Paint a stark jet-black modular rack cabinet chassis for dark hardware containers on Vintage.
pub(crate) fn metal_rack_cabinet(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 7.0, roles().ink);
    theme::gradient(
        painter,
        rect.shrink(1.5),
        5.5,
        Color32::from_rgb(21, 25, 22),
        Color32::from_rgb(13, 15, 14),
    );
    painter.line_segment(
        [
            rect.left_top() + Vec2::new(8.0, 2.0),
            rect.right_top() + Vec2::new(-8.0, 2.0),
        ],
        Stroke::new(1.0_f32, Color32::from_white_alpha(28)),
    );
}

/// A narrow chassis uses the same procedural modular anodizing without overlapping screws.
pub(crate) fn rail_panel(ui: &egui::Ui, rect: Rect) {
    let palette = Palette::of(ui);
    let p = ui.painter();
    match roles().material {
        Material::Flat => {
            p.rect_filled(rect, 4.0, palette.body());
            p.rect_stroke(
                rect,
                4.0,
                Stroke::new(1.0_f32, roles().ink),
                egui::StrokeKind::Inside,
            );
        }
        Material::Plastic => theme::plastic(p, rect, 10.0, palette.body()),
        Material::Metal => {
            let body = palette.body();
            p.rect_filled(rect, 5.0, roles().ink);
            theme::gradient(
                p,
                rect.shrink(1.0),
                4.0,
                body.lerp_to_gamma(Color32::WHITE, 0.04),
                body.lerp_to_gamma(Color32::BLACK, 0.06),
            );
            p.rect_stroke(
                rect,
                5.0,
                Stroke::new(1.0_f32, roles().ink),
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                rect.shrink(1.0),
                4.0,
                Stroke::new(1.0_f32, body.lerp_to_gamma(roles().legend, 0.20)),
                egui::StrokeKind::Inside,
            );
        }
    }
}

pub(crate) fn rail_separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(48.0, 8.0), Sense::hover());
    let left = rect.left_center() + Vec2::new(3.0, 0.0);
    let right = rect.right_center() - Vec2::new(3.0, 0.0);
    ui.painter()
        .line_segment([left, right], Stroke::new(1.0_f32, roles().ink));
    ui.painter().line_segment(
        [left + Vec2::Y, right + Vec2::Y],
        Stroke::new(1.0_f32, Color32::from_white_alpha(45)),
    );
}

/// A recessed bay: a flat well on Studio, a luminous inset display on Aero,
/// and a pressed bay on Vintage.
pub(crate) fn inset(ui: &egui::Ui, rect: Rect) {
    let painter = ui.painter();
    match roles().material {
        Material::Flat => {
            painter.rect_filled(rect, 3.0, roles().face);
            painter.rect_stroke(
                rect,
                3.0,
                Stroke::new(1.0_f32, roles().bay_edge),
                egui::StrokeKind::Inside,
            );
            return;
        }
        Material::Plastic => {
            painter.rect_filled(rect, 10.0, roles().ink);
            theme::gradient(
                painter,
                rect.shrink(1.5),
                8.5,
                roles().face.lerp_to_gamma(Color32::BLACK, 0.25),
                roles().face.lerp_to_gamma(roles().bay_edge, 0.35),
            );
            painter.rect_stroke(
                rect,
                10.0,
                Stroke::new(1.0_f32, roles().bay_edge),
                egui::StrokeKind::Inside,
            );
            return;
        }
        Material::Metal => {}
    }
    ui.painter().rect_filled(
        rect.translate(Vec2::new(0.0, 1.0)),
        6.0,
        Color32::from_white_alpha(48),
    );
    ui.painter().rect_filled(rect, 6.0, roles().ink);
    ui.painter()
        .rect_filled(rect.shrink(2.0), 4.0, roles().face);
    ui.painter().rect_stroke(
        rect.shrink(2.0),
        4.0,
        Stroke::new(1.0_f32, roles().bay_edge),
        egui::StrokeKind::Inside,
    );
}

pub(crate) fn section(ui: &egui::Ui, rect: Rect, label: &str) {
    ui.painter().rect_filled(rect, 3.0, roles().face);
    let galley =
        ui.painter()
            .layout_no_wrap(label.into(), FontId::proportional(13.0), roles().legend);
    let width = galley.size().x;
    ui.painter().galley(
        rect.left_center() + Vec2::new(8.0, -galley.size().y / 2.0),
        galley,
        roles().legend,
    );
    let left = (rect.left() + width + 20.0).min(rect.right() - 8.0);
    if !label.is_empty() && left < rect.right() - 8.0 {
        ui.painter().line_segment(
            [
                Pos2::new(left, rect.center().y),
                rect.right_center() - Vec2::new(8.0, 0.0),
            ],
            Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.45)),
        );
    }
}

pub(crate) fn well(ui: &egui::Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 3.0, roles().ink);
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0_f32, roles().well_edge),
        egui::StrokeKind::Inside,
    );
}

pub(crate) fn status_well(ui: &egui::Ui, rect: Rect, error: bool) {
    well(ui, rect);
    let edge = if error {
        roles().coral
    } else {
        roles().well_edge.lerp_to_gamma(roles().legend, 0.2)
    };
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0_f32, edge),
        egui::StrokeKind::Inside,
    );
}
