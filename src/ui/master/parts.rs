//! Small hardware parts drawn on the board: lamps, patch sockets and cables,
//! the move arrows, and the plate that heads each unit.
use super::*;

/// A unit's or pedal's name with the smaller line under it.
pub(crate) fn heading(ui: &egui::Ui, rect: Rect, title: &str, line: &str, color: Color32) {
    let at = |y: f32| Pos2::new(rect.left() + PEDAL_PADDING, rect.top() + y);
    // A long name shrinks until it clears the lamp.
    let room = rect.width() - PEDAL_PADDING - 34.0;
    let size = [14.0, 13.0, 12.0, 11.0]
        .into_iter()
        .find(|size| {
            ui.painter()
                .layout_no_wrap(title.into(), FontId::proportional(*size), color)
                .size()
                .x
                <= room
        })
        .unwrap_or(11.0);
    ui.painter().text(
        at(15.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(size),
        color,
    );
    ui.painter().text(
        at(30.0),
        Align2::LEFT_CENTER,
        line,
        FontId::proportional(10.0),
        color,
    );
}

/// A socket for a patch cable.
pub(super) fn jack(ui: &egui::Ui, center: Pos2) {
    ui.painter().circle_filled(center, 9.0, roles().ink);
    ui.painter().circle_stroke(
        center,
        9.0,
        Stroke::new(2.0_f32, roles().legend.gamma_multiply(0.6)),
    );
    ui.painter().circle_filled(center, 3.5, Color32::BLACK);
}

/// A patch cable from one point to the next, with an arrow for the
/// direction and a metal plug at each end that has one.
pub(super) fn cable(ui: &egui::Ui, from: f32, to: f32, y: f32, plugs: [bool; 2]) {
    let painter = ui.painter();
    painter.line_segment(
        [Pos2::new(from, y + 1.0), Pos2::new(to, y + 1.0)],
        Stroke::new(6.0_f32, Color32::from_black_alpha(120)),
    );
    painter.line_segment(
        [Pos2::new(from, y), Pos2::new(to, y)],
        Stroke::new(4.0_f32, roles().ink),
    );
    painter.line_segment(
        [Pos2::new(from, y - 1.0), Pos2::new(to, y - 1.0)],
        Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.25)),
    );
    for (left, right) in [(from, from + 8.0), (to - 8.0, to)]
        .into_iter()
        .zip(plugs)
        .filter_map(|(ends, plug)| plug.then_some(ends))
    {
        let plug = Rect::from_min_max(Pos2::new(left, y - 4.5), Pos2::new(right, y + 4.5));
        painter.rect_filled(plug, 1.5, roles().cap);
        painter.rect_stroke(
            plug,
            1.5,
            Stroke::new(1.0_f32, roles().ink),
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
    if to - from > 20.0 {
        let tip = Pos2::new((from + to) / 2.0 + 4.0, y - 10.0);
        painter.add(egui::Shape::convex_polygon(
            vec![
                tip + Vec2::new(-7.0, -4.0),
                tip + Vec2::new(1.0, 0.0),
                tip + Vec2::new(-7.0, 4.0),
            ],
            roles().legend.gamma_multiply(0.7),
            Stroke::NONE,
        ));
    }
}

/// A small round lamp.
pub(super) fn led(ui: &egui::Ui, center: Pos2, radius: f32, lit: bool, color: Color32) {
    let painter = ui.painter();
    if lit {
        painter.circle_filled(center, radius + 4.0, color.gamma_multiply(0.18));
    }
    painter.circle_filled(center, radius + 1.5, roles().ink);
    painter.circle_filled(center, radius, if lit { color } else { roles().lamp_off });
    if lit {
        painter.circle_filled(
            center + Vec2::new(-1.2, -1.2),
            radius * 0.3,
            Color32::from_white_alpha(200),
        );
    }
}

/// The pedal's lamp. Lit, it glows in the application's active lime.
pub(crate) fn lamp(ui: &egui::Ui, center: Pos2, on: bool) {
    let painter = ui.painter();
    if on {
        for (radius, alpha) in [(12.0, 28), (9.0, 54)] {
            painter.circle_filled(
                center,
                radius,
                roles().lamp_on.gamma_multiply(alpha as f32 / 255.0),
            );
        }
    }
    painter.circle_filled(center, 6.5, roles().ink);
    painter.circle_filled(
        center,
        4.5,
        if on {
            roles().lamp_on
        } else {
            roles().lamp_off
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
pub(super) fn arrow(ui: &egui::Ui, center: Pos2, direction: f32, enabled: bool) {
    let color = if enabled {
        roles().legend
    } else {
        roles().legend.gamma_multiply(0.35)
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
