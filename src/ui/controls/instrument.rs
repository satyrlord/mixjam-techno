//! 3-theme code-painted instrument kit shared by the FX rack and Deep Edit.
//! Branches strictly on `Material` (`Flat`, `Plastic`, `Metal`) via `theme::roles()`.
use super::{HoverHints, Material, roles, toggle_at};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Stroke, Vec2,
};
use mixjam_techno::application::FxBusMeterReading;

/// Draw a recessed chassis bay with material-aware framing and title legend.
pub(in super::super) fn bay(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    accent: Color32,
    ink_on_light: bool,
) {
    let r = roles();
    let painter = ui.painter();
    let radius = match r.material {
        Material::Flat => CornerRadius::same(4),
        Material::Plastic => CornerRadius::same(8),
        Material::Metal => CornerRadius::same(5),
    };
    let fill = match r.material {
        Material::Flat => Color32::from_rgba_unmultiplied(r.ink.r(), r.ink.g(), r.ink.b(), 18),
        Material::Plastic => {
            Color32::from_rgba_unmultiplied(r.face.r(), r.face.g(), r.face.b(), 30)
        }
        Material::Metal => Color32::from_rgba_unmultiplied(r.ink.r(), r.ink.g(), r.ink.b(), 22),
    };
    painter.rect_filled(rect, radius, fill);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, r.bay_edge.gamma_multiply(0.75)),
        egui::StrokeKind::Inside,
    );
    if r.material == Material::Metal {
        // Corner chassis screws / bevel highlight on vintage metal bays.
        let top_highlight = [
            Pos2::new(rect.left() + 5.0, rect.top() + 1.0),
            Pos2::new(rect.right() - 5.0, rect.top() + 1.0),
        ];
        painter.line_segment(
            top_highlight,
            Stroke::new(1.0_f32, Color32::from_white_alpha(48)),
        );
        for corner in [
            rect.left_top() + Vec2::new(7.0, 7.0),
            rect.right_top() + Vec2::new(-7.0, 7.0),
        ] {
            painter.circle_filled(corner, 2.0, r.well_edge);
        }
    }
    let text_color = if ink_on_light { r.ink } else { r.legend };
    painter.text(
        Pos2::new(rect.left() + 12.0, rect.top() + 10.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(10.5),
        text_color,
    );
    let rule_y = rect.top() + 18.0;
    painter.line_segment(
        [
            Pos2::new(rect.left() + 10.0, rule_y),
            Pos2::new(rect.right() - 10.0, rule_y),
        ],
        Stroke::new(1.0_f32, accent.gamma_multiply(0.55)),
    );
}

/// Faceted jewel pilot lamp (Metal), translucent orb (Plastic), or crisp indicator (Flat).
pub(in super::super) fn jewel_lamp(
    ui: &egui::Ui,
    center: Pos2,
    radius: f32,
    lit: bool,
    color: Color32,
) {
    let r = roles();
    let painter = ui.painter();
    if lit {
        painter.circle_filled(center, radius + 4.5, color.gamma_multiply(0.22));
    }
    painter.circle_filled(center, radius + 1.6, r.ink);
    let body = if lit { color } else { r.lamp_off };
    painter.circle_filled(center, radius, body);
    if lit {
        match r.material {
            Material::Metal => {
                // Multi-facet diamond specular cut for vintage test-gear jewel lamps.
                let f = radius * 0.52;
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        center + Vec2::new(0.0, -f),
                        center + Vec2::new(f, 0.0),
                        center + Vec2::new(0.0, f),
                        center + Vec2::new(-f, 0.0),
                    ],
                    Color32::from_white_alpha(120),
                    Stroke::NONE,
                ));
            }
            Material::Plastic => {
                painter.circle_filled(
                    center + Vec2::new(-radius * 0.3, -radius * 0.3),
                    radius * 0.35,
                    Color32::from_white_alpha(200),
                );
            }
            Material::Flat => {}
        }
    }
}

/// Toggle switch styled cleanly for chassis hardware across all three themes.
pub(in super::super) fn bat_toggle(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    label: &str,
    value: &mut bool,
    hint: &str,
    ink_on_light: bool,
) -> Response {
    let r = roles();
    let switch_rect = Rect::from_min_size(rect.min, Vec2::new(28.0, 20.0));
    let response = toggle_at(ui, switch_rect, name, "", value);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, response.enabled(), *value, name)
    });
    if r.material == Material::Metal {
        // Chrome bat-handle pin indicator centered inside the metal toggle throat.
        let tip_x = if *value {
            switch_rect.right() - 7.0
        } else {
            switch_rect.left() + 7.0
        };
        let pin = Pos2::new(tip_x, switch_rect.center().y);
        ui.painter().circle_filled(pin, 4.0, r.cap);
        ui.painter()
            .circle_stroke(pin, 4.0, Stroke::new(1.0_f32, r.ink));
    }
    if !label.is_empty() {
        let color = if ink_on_light { r.ink } else { r.legend };
        ui.painter().text(
            Pos2::new(switch_rect.right() + 4.0, switch_rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(11.0),
            color,
        );
    }
    response.hover_hint(format!(
        "{name}: {}. {hint}",
        if *value { "on" } else { "off" }
    ))
}

/// Compact stereo peak and gain-reduction meters with a numeric readout.
pub(in super::super) fn meter_bar(
    ui: &egui::Ui,
    rect: Rect,
    name: &str,
    reading: FxBusMeterReading,
) {
    let response = ui.interact(rect, ui.id().with(name), egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, name));
    response.ctx.accesskit_node_builder(response.id, |node| {
        node.set_label(name);
        node.set_value(meter_readout(reading));
    });
    if response.hovered() {
        response.hover_hint(meter_readout(reading));
    }

    let r = roles();
    let painter = ui.painter();
    let radius = match r.material {
        Material::Flat => CornerRadius::same(3),
        Material::Plastic => CornerRadius::same(5),
        Material::Metal => CornerRadius::same(2),
    };
    let well_bg = if r.material == Material::Metal {
        Color32::from_rgb(16, 20, 17)
    } else {
        r.strip_slot
    };
    painter.rect_filled(rect, radius, well_bg);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, r.well_edge.gamma_multiply(0.7)),
        egui::StrokeKind::Inside,
    );

    let inner = rect.shrink(3.0);
    let lane_h = ((inner.height() - 4.0) / 3.0).max(2.5);
    let norm = |amp: f32| {
        if amp <= 1e-5 {
            0.0
        } else {
            let db = 20.0 * amp.log10();
            ((db + 48.0) / 48.0).clamp(0.0, 1.0)
        }
    };
    let gr_norm = |db: f32| (db / 24.0).clamp(0.0, 1.0);

    let levels = [
        (norm(reading.peak_l), r.lamp_good),
        (norm(reading.peak_r), r.lamp_good),
        (gr_norm(reading.ducker_gr_db), r.lamp_warn),
    ];
    for (row, (fill_frac, base_color)) in levels.into_iter().enumerate() {
        let y0 = inner.top() + row as f32 * (lane_h + 2.0);
        let track = Rect::from_min_max(
            Pos2::new(inner.left(), y0),
            Pos2::new(inner.right() - 8.0, y0 + lane_h),
        );
        if fill_frac > 0.0 {
            let color = if row < 2 && fill_frac > 0.92 {
                r.lamp_peak
            } else {
                base_color
            };
            let fill = Rect::from_min_max(
                track.min,
                Pos2::new(track.left() + track.width() * fill_frac, track.max.y),
            );
            painter.rect_filled(fill, 1.0, color);
        }
    }
    if reading.limiter_gr_db > 0.1 {
        let lim_frac = gr_norm(reading.limiter_gr_db);
        let lim_rect = Rect::from_min_max(
            Pos2::new(
                inner.right() - 6.0,
                inner.bottom() - inner.height() * lim_frac,
            ),
            Pos2::new(inner.right(), inner.bottom()),
        );
        painter.rect_filled(lim_rect, 1.0, r.lamp_peak);
    } else {
        let dot_color = if reading.tail_active {
            r.lamp_on
        } else {
            r.lamp_off
        };
        painter.circle_filled(
            Pos2::new(inner.right() - 3.0, inner.center().y),
            2.5,
            dot_color,
        );
    }
}

fn meter_readout(reading: FxBusMeterReading) -> String {
    let dbfs = |amplitude: f32| {
        if amplitude > 0.0 {
            format!("{:.1} dBFS", 20.0 * amplitude.log10())
        } else {
            "silent".to_owned()
        }
    };
    format!(
        "Left peak {}, RMS {}. Right peak {}, RMS {}. Ducker reduction {:.1} dB. \
         Limiter reduction {:.1} dB. Tail {}.",
        dbfs(reading.peak_l),
        dbfs(reading.rms_l),
        dbfs(reading.peak_r),
        dbfs(reading.rms_r),
        reading.ducker_gr_db,
        reading.limiter_gr_db,
        if reading.tail_active {
            "active"
        } else {
            "idle"
        },
    )
}

/// Recessed oscilloscope plotting `points` (`y(x)` over `[-1, 1]`).
pub(in super::super) fn scope_frame(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    points: &[f32],
    accent: Color32,
) {
    let r = roles();
    let painter = ui.painter();
    let (bg, trace, grid) = match r.material {
        Material::Metal => (
            Color32::from_rgb(12, 22, 16),
            Color32::from_rgb(120, 240, 152),
            Color32::from_rgba_unmultiplied(110, 190, 130, 46),
        ),
        Material::Plastic => (
            Color32::from_rgb(8, 28, 60),
            Color32::from_rgb(128, 234, 242),
            Color32::from_rgba_unmultiplied(140, 210, 250, 48),
        ),
        Material::Flat => (
            r.strip_slot,
            accent,
            Color32::from_rgba_unmultiplied(r.grid_line.r(), r.grid_line.g(), r.grid_line.b(), 64),
        ),
    };
    let radius = CornerRadius::same(if r.material == Material::Plastic {
        7
    } else {
        4
    });
    painter.rect_filled(rect, radius, bg);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, r.well_edge),
        egui::StrokeKind::Inside,
    );

    let plot = rect.shrink2(Vec2::new(8.0, 18.0));
    painter.text(
        Pos2::new(rect.left() + 8.0, rect.top() + 9.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(10.0),
        r.legend,
    );
    // Center axes & linear reference line y = x.
    painter.line_segment(
        [plot.left_center(), plot.right_center()],
        Stroke::new(1.0_f32, grid),
    );
    painter.line_segment(
        [plot.center_top(), plot.center_bottom()],
        Stroke::new(1.0_f32, grid),
    );
    painter.line_segment(
        [plot.left_bottom(), plot.right_top()],
        Stroke::new(1.0_f32, grid),
    );

    if points.len() >= 2 {
        let denom = (points.len() - 1) as f32;
        let to_screen = |i: usize, val: f32| {
            let x = plot.left() + (i as f32 / denom) * plot.width();
            let y = plot.center().y - (val.clamp(-1.2, 1.2) / 1.2) * (plot.height() * 0.48);
            Pos2::new(x, y)
        };
        let poly: Vec<Pos2> = points
            .iter()
            .enumerate()
            .map(|(i, &v)| to_screen(i, v))
            .collect();
        if r.material != Material::Flat {
            painter.add(egui::Shape::line(
                poly.clone(),
                Stroke::new(3.5_f32, trace.gamma_multiply(0.28)),
            ));
        }
        painter.add(egui::Shape::line(poly, Stroke::new(1.8_f32, trace)));
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{fx, master, theme};
    use super::super::contrast;
    use super::roles;
    use mixjam_techno::application::options::Theme;

    #[test]
    fn instrument_palette_meets_contrast_targets_across_all_three_themes() {
        for theme_kind in [Theme::Studio, Theme::Aero, Theme::Vintage] {
            theme::paint_as(theme_kind);
            let r = roles();
            let panel_text = contrast(r.panel, r.text);
            assert!(
                panel_text >= 5.0,
                "{theme_kind:?} panel/text contrast {panel_text:.2}:1 < 5.0:1"
            );
            let face_legend = contrast(r.face, r.legend);
            assert!(
                face_legend >= 5.0,
                "{theme_kind:?} face/legend contrast {face_legend:.2}:1 < 5.0:1"
            );
            let meter_ink = contrast(r.meter_face, r.ink);
            assert!(
                meter_ink >= 5.0,
                "{theme_kind:?} meter_face/ink contrast {meter_ink:.2}:1 < 5.0:1"
            );
            let primary_btn = contrast(r.primary, r.on_light);
            assert!(
                primary_btn >= 4.5,
                "{theme_kind:?} primary button contrast {primary_btn:.2}:1 < 4.5:1"
            );
            let danger_btn = contrast(r.danger, r.legend);
            assert!(
                danger_btn >= 4.5,
                "{theme_kind:?} danger button contrast {danger_btn:.2}:1 < 4.5:1"
            );
            for enamel in [
                fx::DELAY_ENAMEL,
                fx::REVERB_ENAMEL,
                fx::DRIVE_ENAMEL,
                fx::EMPTY_ENAMEL,
            ] {
                for on in [true, false] {
                    let ratio = contrast(master::enamel_of(enamel, on), r.ink);
                    assert!(
                        ratio >= 5.0,
                        "{theme_kind:?} enamel {enamel:?} (on={on}) contrast {ratio:.2}:1 < 5.0:1"
                    );
                }
            }
        }
        theme::paint_as(Theme::Studio);
    }
}
