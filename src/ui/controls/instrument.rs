//! 3-theme code-painted instrument kit shared by the FX rack and Deep Edit.
//! Branches strictly on `Material` (`Flat`, `Plastic`, `Metal`) via `theme::roles()`.
use super::{HoverHints, Material, roles, toggle_at};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Stroke, Vec2,
};
use mixjam_techno::application::FxBusMeterReading;
use mixjam_techno::application::fx::ReverbProfile;

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
/// The ducker's lane shows only on a bus that `ducks`.
pub(in super::super) fn meter_bar(
    ui: &egui::Ui,
    rect: Rect,
    name: &str,
    reading: FxBusMeterReading,
    ducks: bool,
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

    let inner = rect.shrink(2.5);
    meter_lanes(painter, inner, reading, well_bg, ducks);
    meter_end(painter, inner, reading);
}

/// How far a peak of amplitude `amp` fills its lane, from -48 to 0 dBFS.
fn peak_fraction(amp: f32) -> f32 {
    if amp <= 1e-5 {
        return 0.0;
    }
    ((20.0 * amp.log10() + 48.0) / 48.0).clamp(0.0, 1.0)
}

/// How far a gain reduction of `db` fills its lane: all of it at 24 dB.
fn reduction_fraction(db: f32) -> f32 {
    (db / 24.0).clamp(0.0, 1.0)
}

/// The meter's lanes, each behind its letter: the L and R peaks over -48 to
/// 0 dBFS, then, on a bus that `ducks`, D, the ducker's gain reduction.
fn meter_lanes(
    painter: &egui::Painter,
    inner: Rect,
    reading: FxBusMeterReading,
    well_bg: Color32,
    ducks: bool,
) {
    let r = roles();
    let lanes = Rect::from_min_max(Pos2::new(inner.left() + 10.0, inner.top()), inner.max);
    let count = if ducks { 3 } else { 2 };
    let lane_h = ((lanes.height() - 2.0 * (count - 1) as f32) / count as f32).max(2.5);
    // A peak lane turns to the peak colour over 92% of its length.
    let peak = |amp: f32| {
        let fill = peak_fraction(amp);
        (
            fill,
            if fill > 0.92 {
                r.lamp_peak
            } else {
                r.lamp_good
            },
        )
    };
    let ((l, l_color), (r_fill, r_color)) = (peak(reading.peak_l), peak(reading.peak_r));
    let levels = [
        ("L", l, l_color),
        ("R", r_fill, r_color),
        ("D", reduction_fraction(reading.ducker_gr_db), r.lamp_warn),
    ];
    let track_at = |row: usize| {
        let y0 = lanes.top() + row as f32 * (lane_h + 2.0);
        Rect::from_min_max(
            Pos2::new(lanes.left(), y0),
            Pos2::new(lanes.right() - 8.0, y0 + lane_h),
        )
    };
    for (row, (letter, fill_frac, color)) in levels.into_iter().take(count).enumerate() {
        let track = track_at(row);
        // A silent lane still shows its track, so the meter reads as one.
        painter.rect_filled(track, 1.0, r.well_edge.gamma_multiply(0.35));
        painter.text(
            Pos2::new(inner.left() + 4.5, track.center().y),
            Align2::CENTER_CENTER,
            letter,
            FontId::proportional(8.5),
            r.legend.gamma_multiply(0.85),
        );
        if fill_frac > 0.0 {
            let fill = Rect::from_min_max(
                track.min,
                Pos2::new(track.left() + track.width() * fill_frac, track.max.y),
            );
            painter.rect_filled(fill, 1.0, color);
        }
    }
    // Breaks at -24 and -12 dB split the peak lanes into segments.
    let (top, bottom) = (track_at(0), track_at(1));
    for db in [-24.0, -12.0] {
        let x = top.left() + top.width() * (db + 48.0) / 48.0;
        painter.line_segment(
            [Pos2::new(x, top.top()), Pos2::new(x, bottom.bottom())],
            Stroke::new(1.0_f32, well_bg),
        );
    }
}

/// The meter's right end: the limiter's gain reduction while it acts, else
/// a dot lit while the tail rings.
fn meter_end(painter: &egui::Painter, inner: Rect, reading: FxBusMeterReading) {
    let r = roles();
    if reading.limiter_gr_db > 0.1 {
        let lim_frac = reduction_fraction(reading.limiter_gr_db);
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

fn draw_scope_chrome(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    accent: Color32,
) -> (Rect, Color32, Color32, Color32) {
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
    let right_color = match r.material {
        Material::Metal => Color32::from_rgb(236, 178, 86),
        Material::Plastic => Color32::from_rgb(112, 184, 255),
        Material::Flat => accent.gamma_multiply(0.72),
    };
    (plot, trace, grid, right_color)
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
    let (plot, trace, grid, _) = draw_scope_chrome(ui, rect, title, accent);
    let y = |val: f32| plot.center().y - val.clamp(-1.2, 1.2) * (plot.height() * 0.4);
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
        [
            Pos2::new(plot.left(), y(-1.0)),
            Pos2::new(plot.right(), y(1.0)),
        ],
        Stroke::new(1.0_f32, grid),
    );

    if points.len() >= 2 {
        let denom = (points.len() - 1) as f32;
        let to_screen = |i: usize, val: f32| {
            let x = plot.left() + (i as f32 / denom) * plot.width();
            Pos2::new(x, y(val))
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

/// Plot the magnitudes of `(norm_t, L, R)` taps in separate left and right lanes.
pub(in super::super) fn delay_scope_frame(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    taps: &[(f32, f32, f32)],
    ping_pong: bool,
    accent: Color32,
) {
    let r = roles();
    let painter = ui.painter();
    let (plot, left_color, grid, right_color) = draw_scope_chrome(ui, rect, title, accent);
    let mode_tag = if ping_pong { "L/R PING" } else { "L/R DUAL" };
    painter.text(
        Pos2::new(rect.right() - 8.0, rect.top() + 9.0),
        Align2::RIGHT_CENTER,
        mode_tag,
        FontId::proportional(9.0),
        r.legend.gamma_multiply(0.85),
    );
    let mid_y = plot.center().y;
    let half_h = plot.height() * 0.46;
    painter.line_segment(
        [
            Pos2::new(plot.left(), mid_y),
            Pos2::new(plot.right(), mid_y),
        ],
        Stroke::new(1.0_f32, grid),
    );
    for frac in [0.25_f32, 0.5, 0.75] {
        let gx = plot.left() + plot.width() * frac;
        painter.line_segment(
            [Pos2::new(gx, plot.top()), Pos2::new(gx, plot.bottom())],
            Stroke::new(1.0_f32, grid.gamma_multiply(0.6)),
        );
    }

    for &(t, l_amp, r_amp) in taps {
        let x = plot.left() + t.clamp(0.0, 1.0) * (plot.width() - 4.0) + 2.0;
        let draw_stem = |amp: f32, dir: f32, color: Color32| {
            let norm = amp.abs().min(1.25) / 1.25;
            if norm <= 0.005 {
                return;
            }
            let tip = Pos2::new(x, mid_y + dir * norm * half_h);
            let base = Pos2::new(x, mid_y);
            if r.material != Material::Flat {
                painter.line_segment(
                    [base, tip],
                    Stroke::new(4.0_f32, color.gamma_multiply(0.26)),
                );
            }
            painter.line_segment([base, tip], Stroke::new(2.0_f32, color));
            painter.circle_filled(tip, 2.2, color);
        };
        draw_stem(l_amp, -1.0, left_color);
        draw_stem(r_amp, 1.0, right_color);
    }
}

/// The color of the high band's curve, apart from the middle and low bands.
fn high_band_color(material: Material) -> Color32 {
    match material {
        Material::Metal => Color32::from_rgb(196, 226, 168),
        Material::Plastic => Color32::from_rgb(232, 240, 255),
        Material::Flat => Color32::from_rgb(214, 220, 232),
    }
}

/// Plot one decay curve over the scope's plot area, with a soft glow under
/// it when `glow` is set.
fn draw_decay_curve(
    ui: &egui::Ui,
    plot: Rect,
    curve: &[f32],
    color: Color32,
    width: f32,
    glow: bool,
) {
    let denom = (curve.len() - 1) as f32;
    let poly: Vec<Pos2> = curve
        .iter()
        .enumerate()
        .map(|(i, v)| {
            Pos2::new(
                plot.left() + i as f32 / denom * plot.width(),
                plot.bottom() - v.clamp(0.0, 1.0) * plot.height(),
            )
        })
        .collect();
    if glow {
        let stroke = Stroke::new(2.0 * width, color.gamma_multiply(0.28));
        ui.painter().add(egui::Shape::line(poly.clone(), stroke));
    }
    ui.painter()
        .add(egui::Shape::line(poly, Stroke::new(width, color)));
}

/// Plot the early reflections, the pre-delay, and the decay of the low,
/// middle, and high bands of a reverb, with their times in the footer.
pub(in super::super) fn reverb_scope_frame(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    tag: &str,
    profile: &ReverbProfile,
    accent: Color32,
) {
    let r = roles();
    let painter = ui.painter();
    let (mut plot, mid_color, grid, low_color) = draw_scope_chrome(ui, rect, title, accent);
    painter.text(
        Pos2::new(rect.right() - 8.0, rect.top() + 9.0),
        Align2::RIGHT_CENTER,
        tag,
        FontId::proportional(9.0),
        r.legend.gamma_multiply(0.85),
    );
    if profile.er_count > 0 {
        let early = Rect::from_min_max(plot.left_bottom() - Vec2::new(0.0, 44.0), plot.max);
        draw_early_reflections(ui, early, profile, low_color, grid);
        plot.max.y = early.top() - 6.0;
    }
    // -20 and -40 dB; the bottom edge is -60 dB.
    for level in [1.0_f32 / 3.0, 2.0 / 3.0] {
        let y = plot.bottom() - level * plot.height();
        painter.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(1.0_f32, grid.gamma_multiply(0.7)),
        );
    }
    let pre_x = plot.left() + profile.pre_delay_norm * plot.width();
    painter.line_segment(
        [
            Pos2::new(pre_x, plot.top()),
            Pos2::new(pre_x, plot.bottom()),
        ],
        Stroke::new(1.0_f32, grid),
    );
    plot.min.x = pre_x;
    let [low, mid, high] = &profile.decay_curves;
    let glow = r.material != Material::Flat;
    draw_decay_curve(ui, plot, low, low_color, 1.4, false);
    draw_decay_curve(ui, plot, high, high_band_color(r.material), 1.4, false);
    draw_decay_curve(ui, plot, mid, mid_color, 1.8, glow);
    let [b, m, t] = profile.rt60_bands;
    let horizon = profile.horizon_seconds;
    painter.text(
        Pos2::new(rect.left() + 8.0, rect.bottom() - 9.0),
        Align2::LEFT_CENTER,
        format!("0–{horizon:.2} s"),
        FontId::proportional(9.0),
        r.legend,
    );
    painter.text(
        Pos2::new(rect.right() - 8.0, rect.bottom() - 9.0),
        Align2::RIGHT_CENTER,
        format!("B {b:.1}s  M {m:.1}s  T {t:.1}s"),
        FontId::proportional(9.0),
        r.legend.gamma_multiply(0.9),
    );
    let response = ui.interact(rect, ui.id().with(title), egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, title));
    response.ctx.accesskit_node_builder(response.id, |node| {
        let pre_ms = profile.pre_delay_norm * horizon * 1000.0;
        node.set_label(title);
        node.set_value(format!(
            "{title}. Pre-delay {pre_ms:.0} ms. RT60 in seconds. \
             Bass {b:.2}. Middle {m:.2}. Treble {t:.2}. \
             Left-channel early reflections {}. Decay plot 0 to {horizon:.2} seconds.",
            profile.er_count
        ));
    });
}

/// Expand the reflection times on their own scale, starting after the pre-delay.
fn draw_early_reflections(
    ui: &egui::Ui,
    rect: Rect,
    profile: &ReverbProfile,
    trace: Color32,
    grid: Color32,
) {
    let painter = ui.painter();
    let taps = &profile.er_taps[..profile.er_count];
    let after_pre_ms = |t: f32| (t - profile.pre_delay_norm) * profile.horizon_seconds * 1000.0;
    let span = (after_pre_ms(taps[taps.len() - 1].0) / 10.0)
        .ceil()
        .max(1.0)
        * 10.0;
    painter.text(
        rect.left_top(),
        Align2::LEFT_TOP,
        format!("Early L · 0–{span:.0} ms after pre-delay"),
        FontId::proportional(9.0),
        roles().legend,
    );
    let axis = rect.center().y + 5.0;
    painter.line_segment(
        [Pos2::new(rect.left(), axis), Pos2::new(rect.right(), axis)],
        Stroke::new(1.0_f32, grid),
    );
    for &(time, strength) in taps {
        let x = rect.left() + after_pre_ms(time) / span * rect.width();
        let tip = Pos2::new(x, axis - 15.0 * strength);
        painter.line_segment([Pos2::new(x, axis), tip], Stroke::new(1.4_f32, trace));
        painter.circle_filled(tip, 1.6, trace);
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{fx, master, theme};
    use super::super::contrast;
    use super::roles;
    use mixjam_techno::application::options::Theme;

    #[test]
    fn transfer_scope_reference_matches_an_identity_curve() {
        use super::{Color32, Pos2, Rect, Vec2, egui, scope_frame};

        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(240.0, 160.0));
                scope_frame(ui, rect, "Transfer curve", &[-1.0, 1.0], Color32::GRAY);
            });
        });
        let trace = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Path(path) if path.points.len() == 2 => Some(path.points.as_slice()),
            _ => None,
        });
        let trace = trace.expect("The transfer scope draws a trace.");
        assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::LineSegment { points, .. } if points.as_slice() == trace
        )));
    }

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

    #[test]
    fn instrument_controls_render_telemetry_scope_and_all_materials() {
        use super::{bay, delay_scope_frame, meter_bar, reverb_scope_frame, scope_frame};
        use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
        use mixjam_techno::application::FxBusMeterReading;
        use mixjam_techno::application::Session;
        use mixjam_techno::application::fx::Aetherform;

        let profile = Session::fx_reverb_profile(&Aetherform::default());
        let points: Vec<f32> = (0..65).map(|i| (i as f32 / 32.0) - 1.0).collect();
        let readings = [
            (
                "Hot meter",
                FxBusMeterReading {
                    peak_l: 0.25,
                    rms_l: 0.1,
                    peak_r: 0.98,
                    rms_r: 0.5,
                    ducker_gr_db: 6.0,
                    limiter_gr_db: 3.5,
                    tail_active: true,
                },
                &[
                    "dBFS",
                    "Ducker reduction 6.0 dB",
                    "Limiter reduction 3.5 dB",
                    "Tail active",
                ][..],
            ),
            (
                "Active tail meter",
                FxBusMeterReading {
                    peak_l: 0.1,
                    rms_l: 0.05,
                    peak_r: 0.1,
                    rms_r: 0.05,
                    ducker_gr_db: 0.0,
                    limiter_gr_db: 0.0,
                    tail_active: true,
                },
                &["Tail active"][..],
            ),
            (
                "Idle meter",
                FxBusMeterReading::default(),
                &["Left peak silent, RMS silent", "Tail idle"][..],
            ),
        ];

        for theme_kind in [Theme::Studio, Theme::Aero, Theme::Vintage] {
            theme::paint_as(theme_kind);
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let meter_rect = Rect::from_min_size(Pos2::new(20.0, 260.0), Vec2::new(180.0, 24.0));
            let delay_rect = Rect::from_min_size(Pos2::new(320.0, 330.0), Vec2::new(240.0, 140.0));
            let reverb_rect = Rect::from_min_size(Pos2::new(20.0, 350.0), Vec2::new(246.0, 120.0));
            for pass in 0..2 {
                let raw = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0))),
                    events: if pass == 1 {
                        vec![egui::Event::PointerMoved(meter_rect.center())]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                };
                let output = ctx.run(raw, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let accent = Color32::from_rgb(234, 164, 120);
                        bay(
                            ui,
                            Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(280.0, 110.0)),
                            "Drive & Curve",
                            accent,
                            true,
                        );
                        bay(
                            ui,
                            Rect::from_min_size(Pos2::new(20.0, 140.0), Vec2::new(280.0, 110.0)),
                            "Tone",
                            accent,
                            false,
                        );
                        for (idx, (name, reading, _)) in readings.iter().enumerate() {
                            let y = 260.0 + idx as f32 * 30.0;
                            meter_bar(
                                ui,
                                Rect::from_min_size(Pos2::new(20.0, y), Vec2::new(180.0, 24.0)),
                                name,
                                *reading,
                                idx == 0,
                            );
                        }

                        scope_frame(
                            ui,
                            Rect::from_min_size(Pos2::new(320.0, 50.0), Vec2::new(240.0, 160.0)),
                            "Transfer Curve",
                            &points,
                            accent,
                        );
                        scope_frame(
                            ui,
                            Rect::from_min_size(Pos2::new(320.0, 220.0), Vec2::new(240.0, 100.0)),
                            "Empty Scope",
                            &[],
                            accent,
                        );
                        delay_scope_frame(
                            ui,
                            delay_rect,
                            "Tap & Decay",
                            &[(0.15, 1.0, -0.25)],
                            true,
                            accent,
                        );
                        reverb_scope_frame(
                            ui,
                            reverb_rect,
                            "Decay & Space",
                            "HALL · BLOOM +12",
                            &profile,
                            accent,
                        );
                    });
                });
                if pass == 1 {
                    let update = output
                        .platform_output
                        .accesskit_update
                        .expect("accesskit update");
                    for (name, _, expected_parts) in readings {
                        let value = update
                            .nodes
                            .iter()
                            .find_map(|(_, node)| {
                                (node.label() == Some(name))
                                    .then(|| node.value().map(str::to_owned))
                                    .flatten()
                            })
                            .unwrap_or_else(|| panic!("{theme_kind:?}: missing {name}"));
                        for part in expected_parts {
                            assert!(
                                value.contains(part),
                                "{theme_kind:?} {name}: {value:?} missing {part:?}"
                            );
                        }
                    }
                    assert!(
                        output
                            .shapes
                            .iter()
                            .any(|s| matches!(s.shape, egui::Shape::Path(_))),
                        "{theme_kind:?}: scope_frame must emit polyline trace for 65 samples"
                    );
                    assert!(
                        output.shapes.iter().any(|shape| matches!(
                            &shape.shape,
                            egui::Shape::Circle(tip)
                                if delay_rect.contains(tip.center)
                                    && tip.center.y > delay_rect.center().y
                        )),
                        "{theme_kind:?}: the inverted right tap must appear below the center line"
                    );
                }
            }
        }
        theme::paint_as(Theme::Studio);
    }
}
