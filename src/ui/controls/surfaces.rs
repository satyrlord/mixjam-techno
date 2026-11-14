//! Chassis surfaces: the tool panel and the rail panel in the current theme,
//! the rail separator, and the inset, section, and well frames.
use super::*;

/// A tool chassis in the current theme: a flat software panel with its
/// palette's accent along the top on Studio, molded plastic on Aero, and the
/// Folded Metal artwork on Vintage.
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

/// Material slices use one image pixel per physical pixel. Edge tiles are
/// repeated and clipped; all boundaries fade into the solid chassis. Native
/// rounded geometry owns the smooth outer silhouette and uninterrupted bevel.
fn metal_panel(ui: &egui::Ui, rect: Rect, palette: Palette) {
    let p = ui.painter();
    p.rect_filled(rect, 8.0, roles().ink);
    p.rect_filled(rect.shrink(1.5), 7.0, palette.body());
    let art = artwork(ui.ctx(), palette);
    paint_grain(ui, &art.grain, rect);
    let ppp = ui.ctx().pixels_per_point();
    let corner = art.material[0].size_vec2() / ppp;
    let edge_x = art.material[4].size_vec2() / ppp;
    let edge_y = art.material[6].size_vec2() / ppp;
    for (index, y) in [(4, rect.top()), (5, rect.bottom() - edge_x.y)] {
        let mut x = rect.left() + corner.x;
        let clip = Rect::from_min_max(
            Pos2::new(x, rect.top()),
            Pos2::new((rect.right() - corner.x).max(x), rect.bottom()),
        );
        while x < rect.right() - corner.x {
            fixed_image(ui, &art.material[index], Pos2::new(x, y), clip);
            x += edge_x.x;
        }
    }
    for (index, x) in [(6, rect.left()), (7, rect.right() - edge_y.x)] {
        let mut y = rect.top() + corner.y;
        let clip = Rect::from_min_max(
            Pos2::new(rect.left(), y),
            Pos2::new(rect.right(), (rect.bottom() - corner.y).max(y)),
        );
        while y < rect.bottom() - corner.y {
            fixed_image(ui, &art.material[index], Pos2::new(x, y), clip);
            y += edge_y.y;
        }
    }
    // Alpha fades make overlap continuous, even in a short collapsed bar.
    // Half-window clipping would expose a hard line through the material.
    for (index, right, bottom) in [
        (0, false, false),
        (1, true, false),
        (2, false, true),
        (3, true, true),
    ] {
        let min = Pos2::new(
            if right {
                rect.right() - corner.x
            } else {
                rect.left()
            },
            if bottom {
                rect.bottom() - corner.y
            } else {
                rect.top()
            },
        );
        fixed_image(ui, &art.material[index], min, rect.shrink(4.0));
    }
    // These continuous antialiased paths never depend on source crop shape,
    // raster tile boundaries or the amount of available window space.
    p.rect_stroke(
        rect.shrink(0.75),
        8.0,
        Stroke::new(1.5_f32, roles().ink),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        rect.shrink(2.0),
        6.5,
        Stroke::new(1.0_f32, palette.body().lerp_to_gamma(roles().legend, 0.24)),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        rect.shrink(3.0),
        5.5,
        Stroke::new(1.0_f32, palette.body().gamma_multiply(0.72)),
        egui::StrokeKind::Inside,
    );
}

/// A narrow chassis uses the same grain and side folds as tool windows, without
/// overlapping their wide corner patches across the control column.
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
            return;
        }
        Material::Plastic => {
            theme::plastic(p, rect, 10.0, palette.body());
            return;
        }
        Material::Metal => {}
    }
    p.rect_filled(rect, 5.0, palette.body());
    let art = artwork(ui.ctx(), palette);
    paint_grain(ui, &art.grain, rect);
    let ppp = ui.ctx().pixels_per_point();
    for (index, right) in [(6, false), (7, true)] {
        let size = art.material[index].size_vec2() / ppp;
        let x = if right {
            rect.right() - size.x
        } else {
            rect.left()
        };
        let mut y = rect.top();
        while y < rect.bottom() {
            fixed_image(ui, &art.material[index], Pos2::new(x, y), rect.shrink(3.0));
            y += size.y;
        }
    }
    p.rect_stroke(
        rect,
        5.0,
        Stroke::new(1.0_f32, roles().ink),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        rect.shrink(1.0),
        4.0,
        Stroke::new(1.0_f32, palette.body().lerp_to_gamma(roles().legend, 0.20)),
        egui::StrokeKind::Inside,
    );
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
