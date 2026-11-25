//! Bespoke per-unit hardware motifs rendered underneath unified enclosure
//! finish passes (surface sheen, grain, edge rims, and corner screws) across
//! Studio, Aero, and Vintage.
use super::layout::{FOOTER_TOP, HEADER};
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MotifStyle {
    GraphicGrid,
    SteppedFrame,
    WoodCheeks,
    DiagonalStripe,
    TubeVents,
    AluminumCurve,
    RackMeterDeck,
    RadialSunburst,
    AtmosphericGradient,
    TapeDeckInset,
    PhaseBezel,
    TriBandWells,
    EmptyBay,
    CosmicNebula,
    DualSubBays,
    DriveRail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MotifSpec {
    pub base: [u8; 3],
    pub secondary: [u8; 3],
    pub trim: [u8; 3],
    pub light_face: bool,
    pub style: MotifStyle,
    pub knob: theme::KnobStyle,
}

fn to_rgb([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub(super) fn dim_zone([r, g, b]: [u8; 3], on: bool, light_face: bool) -> Color32 {
    let base = Color32::from_rgb(r, g, b);
    if on {
        return base;
    }
    let anchor = if light_face {
        Color32::from_rgb(174, 178, 182)
    } else {
        Color32::from_rgb(32, 36, 40)
    };
    base.lerp_to_gamma(anchor, 0.56)
}

impl MotifSpec {
    /// Text-bearing background colors and trim ink for automated contrast verification.
    pub(crate) fn zones(self, on: bool) -> ([Color32; 2], Color32) {
        let a = dim_zone(self.base, on, self.light_face);
        let b = match self.style {
            MotifStyle::WoodCheeks | MotifStyle::PhaseBezel | MotifStyle::EmptyBay => a,
            _ => dim_zone(self.secondary, on, self.light_face),
        };
        ([a, b], to_rgb(self.trim))
    }

    #[cfg(test)]
    pub(crate) fn extra_band_zones(self, on: bool) -> Vec<Color32> {
        match self.style {
            MotifStyle::TriBandWells => vec![
                dim_zone([24, 52, 62], on, self.light_face),
                dim_zone([38, 34, 78], on, self.light_face),
            ],
            MotifStyle::DualSubBays => vec![dim_zone([18, 44, 92], on, self.light_face)],
            _ => Vec::new(),
        }
    }
}

struct MotifContext {
    rect: Rect,
    inner: Rect,
    zone_a: Color32,
    zone_b: Color32,
    secondary: Color32,
    trim: Color32,
    on: bool,
    light_face: bool,
    seed: u64,
}

type MotifPainter = fn(&egui::Painter, &MotifContext);

const STYLE_PAINTERS: [(MotifStyle, MotifPainter); 16] = [
    (MotifStyle::GraphicGrid, paint_graphic_grid),
    (MotifStyle::SteppedFrame, paint_stepped_frame),
    (MotifStyle::WoodCheeks, paint_wood_cheeks),
    (MotifStyle::DiagonalStripe, paint_diagonal_stripe),
    (MotifStyle::TubeVents, paint_tube_vents),
    (MotifStyle::AluminumCurve, paint_aluminum_curve),
    (MotifStyle::RackMeterDeck, paint_rack_meter_deck),
    (MotifStyle::RadialSunburst, paint_radial_sunburst),
    (MotifStyle::AtmosphericGradient, paint_atmospheric_gradient),
    (MotifStyle::TapeDeckInset, paint_tape_deck_inset),
    (MotifStyle::PhaseBezel, paint_phase_bezel),
    (MotifStyle::TriBandWells, paint_tri_band_wells),
    (MotifStyle::EmptyBay, paint_empty_bay),
    (MotifStyle::CosmicNebula, paint_cosmic_nebula),
    (MotifStyle::DualSubBays, paint_dual_sub_bays),
    (MotifStyle::DriveRail, paint_drive_rail),
];

/// Draw bespoke hardware motif art strictly inside the enclosure's front face.
pub(super) fn paint_motif_art(ui: &egui::Ui, rect: Rect, spec: MotifSpec, on: bool, seed: u64) {
    let ([zone_a, zone_b], trim) = spec.zones(on);
    let trim = if on { trim } else { trim.gamma_multiply(0.6) };
    let inset = if roles().material == theme::Material::Metal {
        4.5
    } else {
        2.0
    };
    let ctx = MotifContext {
        rect,
        inner: rect.shrink(inset),
        zone_a,
        zone_b,
        secondary: dim_zone(spec.secondary, on, spec.light_face),
        trim,
        on,
        light_face: spec.light_face,
        seed,
    };
    let (_, paint) = STYLE_PAINTERS
        .iter()
        .find(|&&(style, _)| style == spec.style)
        .expect("every MotifStyle has a painter");
    paint(ui.painter(), &ctx);
}

fn master_deck(ctx: &MotifContext, margin_x: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(ctx.inner.left() + margin_x, ctx.rect.top() + HEADER + 4.0),
        Pos2::new(
            ctx.inner.right() - margin_x,
            ctx.rect.top() + FOOTER_TOP - 4.0,
        ),
    )
}

fn paint_graphic_grid(p: &egui::Painter, ctx: &MotifContext) {
    let band = master_deck(ctx, 5.0);
    p.rect_filled(
        band,
        4.0,
        Color32::from_rgba_unmultiplied(ctx.zone_b.r(), ctx.zone_b.g(), ctx.zone_b.b(), 85),
    );
    let step = (band.width() / 8.0).max(12.0);
    for col in 1..8 {
        let x = band.left() + col as f32 * step;
        p.line_segment(
            [
                Pos2::new(x, band.top() + 4.0),
                Pos2::new(x, band.bottom() - 4.0),
            ],
            Stroke::new(0.8_f32, ctx.trim.gamma_multiply(0.18)),
        );
    }
    p.rect_stroke(
        band,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
}

fn paint_stepped_frame(p: &egui::Painter, ctx: &MotifContext) {
    let frame = master_deck(ctx, 4.0);
    p.rect_filled(
        frame.shrink(3.0),
        5.0,
        Color32::from_rgba_unmultiplied(ctx.zone_b.r(), ctx.zone_b.g(), ctx.zone_b.b(), 110),
    );
    p.rect_stroke(
        frame,
        4.0,
        Stroke::new(1.2_f32, ctx.trim.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        frame.shrink(3.0),
        3.0,
        Stroke::new(0.8_f32, ctx.trim.gamma_multiply(0.32)),
        egui::StrokeKind::Inside,
    );
}

fn paint_wood_cheeks(p: &egui::Painter, ctx: &MotifContext) {
    let rect = ctx.rect;
    let cheek_w = 6.5;
    for is_right in [false, true] {
        let x0 = if is_right {
            rect.right() - cheek_w - 1.5
        } else {
            rect.left() + 1.5
        };
        let cheek = Rect::from_min_max(
            Pos2::new(x0, rect.top() + 6.0),
            Pos2::new(x0 + cheek_w, rect.bottom() - 6.0),
        );
        p.rect_filled(cheek, 2.5, ctx.secondary);
        for grain in [0.3_f32, 0.65] {
            let gx = cheek.left() + cheek.width() * grain;
            p.line_segment(
                [
                    Pos2::new(gx, cheek.top() + 3.0),
                    Pos2::new(gx, cheek.bottom() - 3.0),
                ],
                Stroke::new(0.8_f32, Color32::from_black_alpha(75)),
            );
        }
        p.rect_stroke(
            cheek,
            2.5,
            Stroke::new(0.8_f32, Color32::from_black_alpha(110)),
            egui::StrokeKind::Inside,
        );
    }
    let blocks = [
        Color32::from_rgb(232, 166, 98),
        Color32::from_rgb(112, 206, 196),
        Color32::from_rgb(208, 152, 236),
    ];
    for (idx, col) in blocks.into_iter().enumerate() {
        let bx = rect.center().x + (idx as f32 - 1.5) * 14.0 + 1.0;
        p.rect_filled(
            Rect::from_min_size(
                Pos2::new(bx, rect.top() + HEADER + 6.0),
                Vec2::new(11.0, 4.0),
            ),
            1.0,
            col,
        );
    }
    p.rect_stroke(
        Rect::from_min_max(
            Pos2::new(rect.left() + cheek_w + 5.0, rect.top() + HEADER + 4.0),
            Pos2::new(rect.right() - cheek_w - 5.0, rect.top() + FOOTER_TOP - 4.0),
        ),
        3.0,
        Stroke::new(0.9_f32, ctx.trim.gamma_multiply(0.35)),
        egui::StrokeKind::Inside,
    );
}

fn paint_diagonal_stripe(p: &egui::Painter, ctx: &MotifContext) {
    let deck = master_deck(ctx, 4.0);
    let bottom_deck = Rect::from_min_max(
        Pos2::new(deck.left() + 3.0, deck.center().y + 4.0),
        deck.max - Vec2::splat(3.0),
    );
    p.rect_filled(
        bottom_deck,
        4.0,
        Color32::from_rgba_unmultiplied(ctx.zone_b.r(), ctx.zone_b.g(), ctx.zone_b.b(), 145),
    );
    p.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(deck.left() + 2.0, deck.bottom() - 26.0),
            Pos2::new(deck.right() - 2.0, deck.top() + 12.0),
            Pos2::new(deck.right() - 2.0, deck.top() + 26.0),
            Pos2::new(deck.left() + 2.0, deck.bottom() - 12.0),
        ],
        Color32::from_rgba_unmultiplied(ctx.trim.r(), ctx.trim.g(), ctx.trim.b(), 36),
        Stroke::NONE,
    ));
    p.rect_stroke(
        deck,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
}

fn paint_tube_vents(p: &egui::Painter, ctx: &MotifContext) {
    let rect = ctx.rect;
    let body = master_deck(ctx, 4.0);
    p.rect_filled(body, 5.0, ctx.zone_b);
    p.rect_stroke(
        body,
        5.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.55)),
        egui::StrokeKind::Inside,
    );
    let glow = if ctx.on {
        Color32::from_rgb(246, 174, 82)
    } else {
        Color32::from_rgb(56, 36, 28)
    };
    for slot in -2..=2 {
        let vx = rect.center().x + slot as f32 * 15.0;
        let slit = Rect::from_center_size(
            Pos2::new(vx, rect.top() + HEADER + 11.0),
            Vec2::new(10.0, 3.0),
        );
        p.rect_filled(
            slit,
            1.5,
            glow.gamma_multiply(if ctx.on { 0.85 } else { 0.5 }),
        );
        p.rect_stroke(
            slit,
            1.5,
            Stroke::new(0.7_f32, roles().ink),
            egui::StrokeKind::Inside,
        );
    }
}

fn paint_aluminum_curve(p: &egui::Painter, ctx: &MotifContext) {
    let rect = ctx.rect;
    let deck = master_deck(ctx, 4.0);
    p.rect_filled(deck, 4.0, ctx.zone_b);
    p.rect_stroke(
        deck,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.5)),
        egui::StrokeKind::Inside,
    );
    let cy = rect.top() + HEADER + 15.0;
    let curve: Vec<Pos2> = (0..=16)
        .map(|i| {
            let t = i as f32 / 16.0;
            let bump = (t * std::f32::consts::PI).sin() * 3.5;
            Pos2::new(rect.left() + 18.0 + t * (rect.width() - 36.0), cy - bump)
        })
        .collect();
    p.add(egui::Shape::line(
        curve,
        Stroke::new(1.2_f32, ctx.trim.gamma_multiply(0.55)),
    ));
}

fn paint_rack_meter_deck(p: &egui::Painter, ctx: &MotifContext) {
    let (rect, inner) = (ctx.rect, ctx.inner);
    let split_y = rect.top() + HEADER + 26.0;
    let deck = Rect::from_min_max(
        Pos2::new(inner.left() + 2.0, split_y),
        Pos2::new(inner.right() - 2.0, rect.top() + FOOTER_TOP - 4.0),
    );
    p.rect_filled(deck, 4.0, ctx.zone_b);
    p.line_segment(
        [
            Pos2::new(inner.left() + 3.0, split_y),
            Pos2::new(inner.right() - 3.0, split_y),
        ],
        Stroke::new(1.2_f32, ctx.trim.gamma_multiply(0.7)),
    );
    let meter_bezel = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + HEADER + 15.0),
        Vec2::new((rect.width() - 36.0).max(40.0), 15.0),
    );
    let amber = if ctx.on {
        Color32::from_rgb(76, 56, 22)
    } else {
        Color32::from_rgb(28, 30, 32)
    };
    p.rect_filled(meter_bezel, 3.0, amber);
    p.rect_stroke(
        meter_bezel,
        3.0,
        Stroke::new(1.0_f32, roles().ink),
        egui::StrokeKind::Inside,
    );
}

fn paint_radial_sunburst(p: &egui::Painter, ctx: &MotifContext) {
    let rect = ctx.rect;
    let deck = master_deck(ctx, 4.0);
    let center = Pos2::new(rect.center().x, rect.top() + 94.0);
    let halo = Color32::from_rgba_unmultiplied(
        ctx.zone_b.r(),
        ctx.zone_b.g(),
        ctx.zone_b.b(),
        if ctx.on { 150 } else { 90 },
    );
    p.circle_filled(center, 44.0, halo);
    for ray in 0..16 {
        let dir = Vec2::angled((ray as f32 * 22.5).to_radians());
        p.line_segment(
            [center + dir * 24.0, center + dir * 46.0],
            Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.24)),
        );
    }
    p.rect_stroke(
        deck,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
}

fn paint_atmospheric_gradient(p: &egui::Painter, ctx: &MotifContext) {
    let deck = master_deck(ctx, 4.0);
    theme::gradient(p, deck, 4.0, ctx.zone_a, ctx.zone_b);
    p.rect_stroke(
        deck,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.55)),
        egui::StrokeKind::Inside,
    );
}

fn paint_tape_deck_inset(p: &egui::Painter, ctx: &MotifContext) {
    let rect = ctx.rect;
    let deck = master_deck(ctx, 5.0);
    p.rect_filled(deck, 5.0, ctx.zone_b);
    p.rect_stroke(
        deck,
        5.0,
        Stroke::new(1.1_f32, ctx.trim.gamma_multiply(0.65)),
        egui::StrokeKind::Inside,
    );
    let badge = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + HEADER + 13.0),
        Vec2::new(42.0, 11.0),
    );
    p.rect_filled(badge, 5.5, Color32::from_rgb(36, 40, 38));
    p.rect_stroke(
        badge,
        5.5,
        Stroke::new(0.9_f32, ctx.trim.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    let led_color = if ctx.on {
        Color32::from_rgb(236, 72, 58)
    } else {
        Color32::from_rgb(76, 36, 34)
    };
    for offset in [-10.0_f32, 0.0, 10.0] {
        p.circle_filled(badge.center() + Vec2::new(offset, 0.0), 2.1, led_color);
    }
}

fn paint_phase_bezel(p: &egui::Painter, ctx: &MotifContext) {
    let bezel = master_deck(ctx, 4.0);
    p.rect_filled(bezel, 5.0, ctx.secondary);
    let plate = bezel.shrink(3.0);
    p.rect_filled(plate, 4.0, ctx.zone_a);
    p.rect_stroke(
        plate.shrink(2.0),
        3.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.65)),
        egui::StrokeKind::Inside,
    );
    let cy = plate.center().y - 14.0;
    let alpha = if ctx.on { 65 } else { 30 };
    let wave: Vec<Pos2> = (0..=18)
        .map(|i| {
            let t = i as f32 / 18.0;
            Pos2::new(
                plate.left() + 8.0 + t * (plate.width() - 16.0),
                cy + (t * std::f32::consts::TAU).sin() * 8.0,
            )
        })
        .collect();
    p.add(egui::Shape::line(
        wave,
        Stroke::new(
            1.2_f32,
            Color32::from_rgba_unmultiplied(156, 210, 250, alpha),
        ),
    ));
}

fn paint_tri_band_wells(p: &egui::Painter, ctx: &MotifContext) {
    let rect = ctx.rect;
    let wells: [([u8; 3], [u8; 3]); 3] = [
        ([24, 52, 62], [236, 174, 82]),
        (
            [ctx.zone_b.r(), ctx.zone_b.g(), ctx.zone_b.b()],
            [102, 212, 238],
        ),
        ([38, 34, 78], [186, 152, 246]),
    ];
    let well_w = 60.0;
    let start_x = rect.center().x - (3.0 * well_w + 8.0) / 2.0;
    let y0 = rect.top() + HEADER + 28.0;
    let y1 = rect.top() + FOOTER_TOP - 5.0;
    for (idx, (fill_rgb, cap_rgb)) in wells.into_iter().enumerate() {
        let x0 = start_x + idx as f32 * (well_w + 4.0);
        let well = Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x0 + well_w, y1));
        let fill = if idx == 1 {
            ctx.zone_b
        } else {
            dim_zone(fill_rgb, ctx.on, ctx.light_face)
        };
        let cap = to_rgb(cap_rgb);
        p.rect_filled(well, 4.0, fill);
        p.rect_stroke(
            well,
            4.0,
            Stroke::new(
                1.0_f32,
                cap.gamma_multiply(if ctx.on { 0.55 } else { 0.25 }),
            ),
            egui::StrokeKind::Inside,
        );
        p.line_segment(
            [
                Pos2::new(well.left() + 4.0, well.top() + 2.0),
                Pos2::new(well.right() - 4.0, well.top() + 2.0),
            ],
            Stroke::new(
                2.0_f32,
                if ctx.on {
                    cap
                } else {
                    cap.gamma_multiply(0.35)
                },
            ),
        );
    }
}

fn paint_empty_bay(p: &egui::Painter, ctx: &MotifContext) {
    let inner = ctx.inner;
    let bay = Rect::from_min_max(
        Pos2::new(inner.left() + 2.0, ctx.rect.top() + 34.0),
        inner.max - Vec2::splat(2.0),
    );
    let cavity = bay.shrink(8.0);
    let top_wall = vec![
        bay.left_top(),
        bay.right_top(),
        cavity.right_top(),
        cavity.left_top(),
    ];
    let left_wall = vec![
        bay.left_top(),
        cavity.left_top(),
        cavity.left_bottom(),
        bay.left_bottom(),
    ];
    let right_wall = vec![
        bay.right_top(),
        bay.right_bottom(),
        cavity.right_bottom(),
        cavity.right_top(),
    ];
    let bottom_wall = vec![
        bay.left_bottom(),
        cavity.left_bottom(),
        cavity.right_bottom(),
        bay.right_bottom(),
    ];
    for (poly, shade) in [
        (top_wall, -0.35),
        (left_wall, -0.15),
        (right_wall, 0.15),
        (bottom_wall, 0.28),
    ] {
        let color = if shade >= 0.0 {
            ctx.secondary.lerp_to_gamma(Color32::WHITE, shade)
        } else {
            ctx.secondary.lerp_to_gamma(Color32::BLACK, -shade)
        };
        p.add(egui::Shape::convex_polygon(poly, color, Stroke::NONE));
    }
    p.rect_filled(cavity, 3.0, Color32::from_rgb(15, 17, 19));
    for frac in [0.28_f32, 0.5, 0.72] {
        let ty = cavity.top() + cavity.height() * frac;
        p.line_segment(
            [
                Pos2::new(cavity.left() + 14.0, ty),
                Pos2::new(cavity.right() - 14.0, ty),
            ],
            Stroke::new(0.8_f32, Color32::from_rgba_unmultiplied(72, 82, 86, 45)),
        );
    }
    p.rect_stroke(
        cavity,
        3.0,
        Stroke::new(1.0_f32, roles().ink),
        egui::StrokeKind::Inside,
    );
    for side_x in [cavity.left() + 9.0, cavity.right() - 9.0] {
        let rail = Rect::from_min_max(
            Pos2::new(side_x - 3.0, cavity.top() + 8.0),
            Pos2::new(side_x + 3.0, cavity.bottom() - 8.0),
        );
        p.rect_filled(rail, 1.5, Color32::from_rgb(46, 50, 54));
        for step in 0..5 {
            let py = rail.top() + 6.0 + step as f32 * ((rail.height() - 12.0) / 4.0);
            p.circle_filled(Pos2::new(side_x, py), 1.3, ctx.trim);
        }
    }
}

fn fx_deck(ctx: &MotifContext, margin_x: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(ctx.inner.left() + margin_x, ctx.rect.top() + 65.0),
        Pos2::new(ctx.inner.right() - margin_x, ctx.rect.bottom() - 42.0),
    )
}

fn paint_cosmic_nebula(p: &egui::Painter, ctx: &MotifContext) {
    let deck = fx_deck(ctx, 4.0);
    let orb_a = deck.center() + Vec2::new(-deck.width() * 0.18, 6.0);
    let orb_b = deck.center() + Vec2::new(deck.width() * 0.20, -4.0);
    p.circle_filled(
        orb_a,
        deck.height() * 0.36,
        Color32::from_rgba_unmultiplied(
            ctx.zone_b.r(),
            ctx.zone_b.g(),
            ctx.zone_b.b(),
            if ctx.on { 165 } else { 90 },
        ),
    );
    p.circle_filled(
        orb_b,
        deck.height() * 0.28,
        Color32::from_rgba_unmultiplied(
            ctx.zone_a.r().saturating_add(10),
            ctx.zone_b.g().saturating_add(18),
            ctx.zone_b.b(),
            if ctx.on { 120 } else { 65 },
        ),
    );
    let mut rng = ctx.seed.wrapping_mul(0x9e37_79b9).wrapping_add(17);
    let star_alpha = if ctx.on { 155 } else { 70 };
    for _ in 0..14 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let fx = ((rng >> 16) & 0xff) as f32 / 255.0;
        let fy = ((rng >> 24) & 0xff) as f32 / 255.0;
        let pt = deck.min
            + Vec2::new(
                8.0 + fx * (deck.width() - 16.0),
                8.0 + fy * (deck.height() - 16.0),
            );
        p.circle_filled(
            pt,
            1.0,
            Color32::from_rgba_unmultiplied(ctx.trim.r(), ctx.trim.g(), ctx.trim.b(), star_alpha),
        );
    }
    p.rect_stroke(
        deck,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
}

fn paint_dual_sub_bays(p: &egui::Painter, ctx: &MotifContext) {
    let outer = fx_deck(ctx, 4.0);
    let half_w = ((outer.width() - 6.0) / 2.0).max(32.0);
    let left_bay = Rect::from_min_max(outer.min, Pos2::new(outer.left() + half_w, outer.bottom()));
    let right_bay = Rect::from_min_max(Pos2::new(outer.right() - half_w, outer.top()), outer.max);
    let right_col = dim_zone([18, 44, 92], ctx.on, ctx.light_face);
    for (bay, col) in [(left_bay, ctx.zone_b), (right_bay, right_col)] {
        p.rect_filled(bay, 5.0, col);
        p.rect_stroke(
            bay,
            5.0,
            Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.55)),
            egui::StrokeKind::Inside,
        );
    }
    let bridge = Rect::from_center_size(
        Pos2::new(ctx.rect.center().x, outer.center().y),
        Vec2::new(16.0, 6.0),
    );
    p.rect_filled(bridge, 3.0, ctx.trim.gamma_multiply(0.45));
}

fn paint_drive_rail(p: &egui::Painter, ctx: &MotifContext) {
    let frame = fx_deck(ctx, 5.0);
    p.rect_filled(frame, 5.0, ctx.zone_b);
    p.rect_stroke(
        frame,
        5.0,
        Stroke::new(1.1_f32, ctx.trim.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    let pip_y = frame.top() + 3.0;
    let cx = ctx.rect.center().x;
    for (i, rgb_pip) in [[236, 94, 52], [244, 148, 56], [248, 194, 78], [244, 98, 92]]
        .into_iter()
        .enumerate()
    {
        let pip = Rect::from_center_size(
            Pos2::new(cx + (i as f32 - 1.5) * 14.0, pip_y),
            Vec2::new(10.0, 2.5),
        );
        let col = if ctx.on {
            to_rgb(rgb_pip)
        } else {
            to_rgb(rgb_pip).gamma_multiply(0.35)
        };
        p.rect_filled(pip, 1.0, col);
    }
}
