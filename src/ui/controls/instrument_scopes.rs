//! Master Deep Edit oscilloscope views: GR history, 3-band GR, log-frequency EQ response,
//! and rotated M/S stereo vectorscope with phase correlation meter.
use super::ScopeState;
use super::{
    Align2, Color32, FontId, Material, Pos2, Rect, Stroke, Vec2, draw_glow_trace,
    draw_horiz_fractions, draw_scope_chrome, draw_scope_status, egui, hover_access_label,
    reduction_text, roles,
};

fn draw_reduction_grid(
    painter: &egui::Painter,
    plot: Rect,
    max_db: f32,
    grid: Color32,
    legend: Color32,
) {
    draw_horiz_fractions(
        painter,
        plot,
        &[3.0 / max_db, 6.0 / max_db, 9.0 / max_db],
        grid,
    );
    for db in [3_u8, 6, 9] {
        let y = plot.top() + (f32::from(db) / max_db) * plot.height();
        painter.text(
            Pos2::new(plot.left() + 2.0, y),
            Align2::LEFT_BOTTOM,
            format!("-{db}"),
            FontId::proportional(8.0),
            legend.gamma_multiply(0.55),
        );
    }
}

/// Top-down scrolling gain-reduction waveform scope (`0` dB at top to `-max_db`).
pub(in super::super::super) fn gr_history_scope(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    history: &[f32],
    max_db: f32,
    current_db: f32,
    accent: Color32,
) {
    gr_history_scope_state(
        ui,
        rect,
        title,
        history,
        max_db,
        current_db,
        ScopeState::active(accent),
    );
}

fn draw_gr_labels(
    painter: &egui::Painter,
    rect: Rect,
    trace: Color32,
    current_db: f32,
    state: ScopeState<'_>,
) -> (String, String) {
    let tag = reduction_text(current_db);
    let header_tag = match state.status {
        Some(status) if !state.active => format!("{tag} · {status}"),
        _ => tag.clone(),
    };
    let color = if state.active && current_db >= 0.05 {
        trace
    } else {
        roles()
            .legend
            .gamma_multiply(if state.active { 1.0 } else { 0.72 })
    };
    painter.text(
        Pos2::new(rect.right() - 8.0, rect.top() + 9.0),
        Align2::RIGHT_CENTER,
        &header_tag,
        FontId::proportional(9.5),
        color,
    );
    (tag, header_tag)
}

/// Top-down scrolling gain-reduction waveform scope with explicit active/status state.
pub(in super::super::super) fn gr_history_scope_state(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    history: &[f32],
    max_db: f32,
    current_db: f32,
    state: ScopeState<'_>,
) {
    let r = roles();
    let (plot, trace, grid, _) = draw_scope_chrome(ui, rect, title, state.accent);
    let painter = ui.painter();
    let (tag, header_tag) = draw_gr_labels(painter, rect, trace, current_db, state);
    let line_color = if state.active {
        trace
    } else {
        trace.gamma_multiply(0.45)
    };
    draw_reduction_grid(painter, plot, max_db, grid, r.legend);
    if history.len() >= 2 {
        let denom = (history.len() - 1) as f32;
        let poly: Vec<Pos2> = history
            .iter()
            .enumerate()
            .map(|(i, &db)| {
                let frac = (db / max_db).clamp(0.0, 1.0);
                Pos2::new(
                    plot.left() + (i as f32 / denom) * plot.width(),
                    plot.top() + frac * plot.height(),
                )
            })
            .collect();
        draw_glow_trace(
            painter,
            poly,
            line_color,
            state.active && r.material != Material::Flat,
        );
    }
    let summary = if state.active {
        format!("Current reduction {tag}")
    } else {
        format!("Current reduction {header_tag}")
    };
    hover_access_label(ui, rect, title, summary);
}

/// Top-down 3-band (`Low <200 Hz`, `Mid 200-3k`, `High >3 kHz`) gain-reduction meter scope
/// with explicit active/status state.
pub(in super::super::super) fn multiband_gr_scope_state(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    reductions: [f32; 3],
    max_db: f32,
    state: ScopeState<'_>,
) {
    let r = roles();
    let (full_plot, mid_color, grid, low_color) = draw_scope_chrome(ui, rect, title, state.accent);
    let plot = Rect::from_min_max(full_plot.min, full_plot.max - Vec2::new(0.0, 14.0));
    let painter = ui.painter();
    draw_scope_status(painter, rect, state);
    draw_reduction_grid(painter, plot, max_db, grid, r.legend);
    let dim = if state.active { 1.0 } else { 0.45 };
    let band_defs = [
        ("LOW <200", low_color.gamma_multiply(dim), reductions[0]),
        ("MID 200-3k", mid_color.gamma_multiply(dim), reductions[1]),
        (
            "HIGH >3k",
            Color32::from_rgb(186, 152, 246).gamma_multiply(dim),
            reductions[2],
        ),
    ];
    let col_w = plot.width() / 3.0;
    for (idx, (label, color, db)) in band_defs.iter().enumerate() {
        let bar_w = (col_w * 0.48).clamp(24.0, 56.0);
        let cx = plot.left() + (idx as f32 + 0.5) * col_w;
        let track = Rect::from_min_max(
            Pos2::new(cx - bar_w * 0.5, plot.top() + 2.0),
            Pos2::new(cx + bar_w * 0.5, plot.bottom() - 2.0),
        );
        // An empty track is a faint outline, so 0 dB reads as no reduction,
        // not as a full grey bar.
        painter.rect_filled(track, 2.0, r.well_edge.gamma_multiply(0.10));
        painter.rect_stroke(
            track,
            2.0,
            Stroke::new(1.0_f32, r.well_edge.gamma_multiply(0.45)),
            egui::StrokeKind::Inside,
        );
        let frac = (*db / max_db).clamp(0.0, 1.0);
        if frac > 0.005 {
            let fill = Rect::from_min_max(
                track.min,
                Pos2::new(track.right(), track.top() + track.height() * frac),
            );
            painter.rect_filled(fill, 2.0, *color);
        }
        painter.text(
            Pos2::new(cx, rect.bottom() - 9.0),
            Align2::CENTER_CENTER,
            format!("{label}  {}", reduction_text(*db)),
            FontId::proportional(9.0),
            r.legend
                .gamma_multiply(if state.active { 1.0 } else { 0.72 }),
        );
    }
    hover_access_label(
        ui,
        rect,
        title,
        format!(
            "Low {}, Mid {}, High {}",
            reduction_text(reductions[0]),
            reduction_text(reductions[1]),
            reduction_text(reductions[2])
        ),
    );
}

/// Logarithmically spaced 20 Hz–20 kHz EQ magnitude response scope (`±max_db`).
pub(in super::super::super) fn eq_response_scope(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    curve_db: &[f32],
    max_db: f32,
    active: bool,
    accent: Color32,
) {
    eq_response_scope_state(
        ui,
        rect,
        title,
        curve_db,
        max_db,
        ScopeState {
            active,
            status: (!active).then_some("OFF"),
            accent,
        },
    );
}

/// EQ response scope with explicit active/status state.
pub(in super::super::super) fn eq_response_scope_state(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    curve_db: &[f32],
    max_db: f32,
    state: ScopeState<'_>,
) {
    let r = roles();
    let (full_plot, trace, grid, _) = draw_scope_chrome(ui, rect, title, state.accent);
    let plot = Rect::from_min_max(full_plot.min, full_plot.max - Vec2::new(0.0, 13.0));
    let painter = ui.painter();
    draw_horiz_fractions(painter, plot, &[0.25, 0.75], grid);
    painter.line_segment(
        [plot.left_center(), plot.right_center()],
        Stroke::new(1.0_f32, grid.gamma_multiply(1.35)),
    );
    let log_span = 1000.0_f32.ln();
    for (hz, label) in [
        (50.0_f32, "50"),
        (200.0, "200"),
        (1000.0, "1k"),
        (4000.0, "4k"),
        (12_000.0, "12k"),
    ] {
        let x = plot.left() + ((hz / 20.0).ln() / log_span) * plot.width();
        painter.line_segment(
            [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
            Stroke::new(1.0_f32, grid.gamma_multiply(0.7)),
        );
        painter.text(
            Pos2::new(x, rect.bottom() - 8.5),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(8.5),
            r.legend,
        );
    }
    if curve_db.len() >= 2 {
        let denom = (curve_db.len() - 1) as f32;
        let poly: Vec<Pos2> = curve_db
            .iter()
            .enumerate()
            .map(|(i, &db)| {
                let frac = (db / max_db).clamp(-1.0, 1.0);
                Pos2::new(
                    plot.left() + (i as f32 / denom) * plot.width(),
                    plot.center().y - frac * (plot.height() * 0.5),
                )
            })
            .collect();
        let line_color = if state.active {
            trace
        } else {
            trace.gamma_multiply(0.45)
        };
        draw_glow_trace(
            painter,
            poly,
            line_color,
            state.active && r.material != Material::Flat,
        );
    }
    let summary = paint_eq_summary(painter, rect, curve_db, state);
    hover_access_label(ui, rect, title, summary);
}

fn paint_eq_summary(
    painter: &egui::Painter,
    rect: Rect,
    curve_db: &[f32],
    state: ScopeState,
) -> String {
    let (lo, hi) = curve_db
        .iter()
        .fold((0.0_f32, 0.0_f32), |(a, b), &v| (a.min(v), b.max(v)));
    let range_text = format!("{lo:+.1} to {hi:+.1} dB");
    let summary = match state.status {
        Some(tag) if !state.active => format!("{range_text} · {tag}"),
        _ => range_text,
    };
    let color = if state.active {
        roles().legend
    } else {
        roles().legend.gamma_multiply(0.76)
    };
    painter.text(
        Pos2::new(rect.right() - 8.0, rect.top() + 9.0),
        Align2::RIGHT_CENTER,
        &summary,
        FontId::proportional(9.0),
        color,
    );
    summary
}

fn draw_lissajous_diamond(
    painter: &egui::Painter,
    plot: Rect,
    points: &[(f32, f32)],
    trace: Color32,
    grid: Color32,
    legend: Color32,
    glow: bool,
) {
    let center = plot.center();
    let rad = (plot.width().min(plot.height()) * 0.48).max(16.0);
    let top = Pos2::new(center.x, center.y - rad);
    let right = Pos2::new(center.x + rad, center.y);
    let bottom = Pos2::new(center.x, center.y + rad);
    let left = Pos2::new(center.x - rad, center.y);
    painter.add(egui::Shape::closed_line(
        vec![top, right, bottom, left],
        Stroke::new(1.0_f32, grid),
    ));
    painter.line_segment(
        [top, bottom],
        Stroke::new(1.0_f32, grid.gamma_multiply(0.8)),
    );
    painter.line_segment(
        [left, right],
        Stroke::new(1.0_f32, grid.gamma_multiply(0.8)),
    );
    let label_color = legend.gamma_multiply(0.8);
    painter.text(
        Pos2::new(center.x - rad * 0.62, center.y - rad * 0.62),
        Align2::CENTER_CENTER,
        "L",
        FontId::proportional(8.5),
        label_color,
    );
    painter.text(
        Pos2::new(center.x + rad * 0.62, center.y - rad * 0.62),
        Align2::CENTER_CENTER,
        "R",
        FontId::proportional(8.5),
        label_color,
    );
    if points.iter().any(|&(l, r)| l.abs() + r.abs() > 1e-4) {
        let mut poly = Vec::with_capacity(points.len());
        for &(l, r) in points {
            let x = 0.5 * (r - l).clamp(-2.0, 2.0) * rad;
            let y = -0.5 * (l + r).clamp(-2.0, 2.0) * rad;
            let pt = Pos2::new(center.x + x, center.y + y);
            poly.push(pt);
        }
        draw_glow_trace(painter, poly.clone(), trace.gamma_multiply(0.55), glow);
        for pt in poly {
            painter.circle_filled(pt, 1.8, trace);
        }
    } else {
        painter.circle_filled(center, 2.2, trace.gamma_multiply(0.5));
    }
}

fn draw_correlation_bar(
    painter: &egui::Painter,
    row: Rect,
    correlation: f32,
    trace: Color32,
    warn: Color32,
    edge: Color32,
    legend: Color32,
) {
    let bar = Rect::from_min_max(
        Pos2::new(row.left(), row.center().y - 3.5),
        Pos2::new(
            (row.right() - 46.0).max(row.left() + 40.0),
            row.center().y + 3.5,
        ),
    );
    painter.rect_filled(bar, 2.0, edge.gamma_multiply(0.35));
    let mid_x = bar.center().x;
    let clamped = correlation.clamp(-1.0, 1.0);
    if clamped >= 0.0 {
        let fill = Rect::from_min_max(
            Pos2::new(mid_x, bar.top()),
            Pos2::new(mid_x + bar.width() * 0.5 * clamped, bar.bottom()),
        );
        painter.rect_filled(fill, 2.0, trace);
    } else {
        let fill = Rect::from_min_max(
            Pos2::new(mid_x + bar.width() * 0.5 * clamped, bar.top()),
            Pos2::new(mid_x, bar.bottom()),
        );
        painter.rect_filled(fill, 2.0, warn);
    }
    painter.line_segment(
        [
            Pos2::new(mid_x, bar.top() - 1.5),
            Pos2::new(mid_x, bar.bottom() + 1.5),
        ],
        Stroke::new(1.0_f32, legend.gamma_multiply(0.75)),
    );
    painter.text(
        Pos2::new(row.right(), row.center().y),
        Align2::RIGHT_CENTER,
        format!("{clamped:+.2}"),
        FontId::proportional(9.0),
        if clamped < 0.0 { warn } else { legend },
    );
}

/// Rotated 45° M/S stereo Lissajous diamond vectorscope and `-1..+1` phase correlation bar.
pub(in super::super::super) fn stereo_vectorscope(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    points: &[(f32, f32)],
    correlation: f32,
    mono_below_hz: f32,
    accent: Color32,
) {
    let r = roles();
    let (full_plot, trace, grid, _) = draw_scope_chrome(ui, rect, title, accent);
    let painter = ui.painter();
    painter.text(
        Pos2::new(rect.right() - 8.0, rect.top() + 9.0),
        Align2::RIGHT_CENTER,
        format!("Mono < {mono_below_hz:.0} Hz"),
        FontId::proportional(9.0),
        r.legend.gamma_multiply(0.85),
    );
    let meter_row = Rect::from_min_max(
        Pos2::new(full_plot.left(), full_plot.bottom() - 14.0),
        full_plot.max,
    );
    let plot = Rect::from_min_max(
        full_plot.min,
        Pos2::new(full_plot.right(), meter_row.top() - 4.0),
    );
    draw_lissajous_diamond(
        painter,
        plot,
        points,
        trace,
        grid,
        r.legend,
        r.material != Material::Flat,
    );
    draw_correlation_bar(
        painter,
        meter_row,
        correlation,
        trace,
        r.lamp_peak,
        r.well_edge,
        r.legend,
    );
    hover_access_label(
        ui,
        rect,
        title,
        format!("Correlation {correlation:+.2}, Mono below {mono_below_hz:.0} Hz"),
    );
}
