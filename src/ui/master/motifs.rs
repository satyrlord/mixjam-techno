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

pub(crate) const DEEP_EDIT_DIM: f32 = 0.28;

fn to_rgb([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn dim_zone_by([r, g, b]: [u8; 3], amount: f32, light_face: bool) -> Color32 {
    let base = Color32::from_rgb(r, g, b);
    if amount <= 0.0 {
        return base;
    }
    let anchor = if light_face {
        Color32::from_rgb(174, 178, 182)
    } else {
        Color32::from_rgb(32, 36, 40)
    };
    base.lerp_to_gamma(anchor, amount)
}

impl MotifSpec {
    fn zones_by(self, amount: f32) -> ([Color32; 2], Color32) {
        let a = dim_zone_by(self.base, amount, self.light_face);
        let b = match self.style {
            MotifStyle::WoodCheeks | MotifStyle::PhaseBezel | MotifStyle::EmptyBay => a,
            _ => dim_zone_by(self.secondary, amount, self.light_face),
        };
        ([a, b], to_rgb(self.trim))
    }

    /// Text-bearing background colors and trim ink for automated contrast verification.
    pub(crate) fn zones(self, on: bool) -> ([Color32; 2], Color32) {
        self.zones_by(if on { 0.0 } else { 0.56 })
    }

    /// Expanded Deep Edit background zones (gentler 28% dim when off or bypassed).
    pub(crate) fn deep_zones(self, active: bool) -> ([Color32; 2], Color32) {
        let ([a, b], raw_trim) = self.zones_by(if active { 0.0 } else { DEEP_EDIT_DIM });
        let trim = if active {
            raw_trim
        } else {
            raw_trim.gamma_multiply(0.76)
        };
        ([a, b], trim)
    }

    /// High-contrast silkscreen ink matching the faceplate brightness.
    pub(crate) fn face_ink(self) -> Color32 {
        if self.light_face {
            roles().ink
        } else {
            roles().legend
        }
    }

    /// Luminous unit accent color safe to draw on dark oscilloscope glass.
    pub(crate) fn scope_accent(self) -> Color32 {
        if self.light_face {
            to_rgb(self.base)
        } else {
            to_rgb(self.trim)
        }
    }

    #[cfg(test)]
    fn extra_band_zones_by(self, amount: f32) -> Vec<Color32> {
        match self.style {
            MotifStyle::TriBandWells => vec![
                dim_zone_by([24, 52, 62], amount, self.light_face),
                dim_zone_by([38, 34, 78], amount, self.light_face),
            ],
            MotifStyle::DualSubBays => vec![dim_zone_by([18, 44, 92], amount, self.light_face)],
            _ => Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn extra_band_zones(self, on: bool) -> Vec<Color32> {
        self.extra_band_zones_by(if on { 0.0 } else { 0.56 })
    }

    #[cfg(test)]
    pub(crate) fn deep_extra_band_zones(self, active: bool) -> Vec<Color32> {
        self.extra_band_zones_by(if active { 0.0 } else { DEEP_EDIT_DIM })
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
    dim_amount: f32,
    light_face: bool,
    seed: u64,
    /// The Deep Edit dial cells the art follows, in grid order; empty on a
    /// pedal face, whose controls sit where its art expects them.
    cells: Vec<Rect>,
}

/// How far a dial cell's knob center sits below the cell's top.
pub(crate) const DEEP_CELL_KNOB: f32 = 22.0;

/// The rectangle a Deep Edit dial occupies: its column, from the top of the
/// knob at `knob` to the bottom of its label at `label_y`.
pub(crate) fn deep_cell(knob: Pos2, label_y: f32, width: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(knob.x - width / 2.0, knob.y - DEEP_CELL_KNOB),
        Pos2::new(knob.x + width / 2.0, label_y + 7.0),
    )
}

fn knob_center(cell: Rect) -> Pos2 {
    Pos2::new(cell.center().x, cell.top() + DEEP_CELL_KNOB)
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

const DEEP_STYLE_PAINTERS: [(MotifStyle, MotifPainter); 16] = [
    (MotifStyle::GraphicGrid, paint_deep_graphic_grid),
    (MotifStyle::SteppedFrame, paint_deep_stepped_frame),
    (MotifStyle::WoodCheeks, paint_deep_wood_cheeks),
    (MotifStyle::DiagonalStripe, paint_deep_diagonal_stripe),
    (MotifStyle::TubeVents, paint_deep_tube_vents),
    (MotifStyle::AluminumCurve, paint_deep_aluminum_curve),
    (MotifStyle::RackMeterDeck, paint_deep_rack_meter_deck),
    (MotifStyle::RadialSunburst, paint_deep_radial_sunburst),
    (
        MotifStyle::AtmosphericGradient,
        paint_deep_atmospheric_gradient,
    ),
    (MotifStyle::TapeDeckInset, paint_deep_tape_deck_inset),
    (MotifStyle::PhaseBezel, paint_deep_phase_bezel),
    (MotifStyle::TriBandWells, paint_deep_tri_band_wells),
    (MotifStyle::EmptyBay, paint_deep_empty_bay),
    (MotifStyle::CosmicNebula, paint_deep_cosmic_nebula),
    (MotifStyle::DualSubBays, paint_deep_dual_sub_bays),
    (MotifStyle::DriveRail, paint_deep_drive_rail),
];

fn run_motif_painter(
    ui: &egui::Ui,
    table: &[(MotifStyle, MotifPainter); 16],
    style: MotifStyle,
    ctx: &MotifContext,
) {
    let (_, paint) = table
        .iter()
        .find(|&&(entry_style, _)| entry_style == style)
        .expect("every MotifStyle has a painter");
    paint(ui.painter(), ctx);
}

fn build_context(
    rect: Rect,
    spec: MotifSpec,
    on: bool,
    deep: bool,
    seed: u64,
    cells: &[Rect],
) -> MotifContext {
    let dim_amount = if on {
        0.0
    } else if deep {
        DEEP_EDIT_DIM
    } else {
        0.56
    };
    let ([zone_a, zone_b], trim) = if deep {
        spec.deep_zones(on)
    } else {
        let (zones, raw_trim) = spec.zones(on);
        (
            zones,
            if on {
                raw_trim
            } else {
                raw_trim.gamma_multiply(0.6)
            },
        )
    };
    let inset = if roles().material == theme::Material::Metal {
        4.5
    } else {
        2.0
    };
    MotifContext {
        rect,
        inner: rect.shrink(inset),
        zone_a,
        zone_b,
        secondary: dim_zone_by(spec.secondary, dim_amount, spec.light_face),
        trim,
        on,
        dim_amount,
        light_face: spec.light_face,
        seed,
        cells: cells.to_vec(),
    }
}

/// Draw bespoke hardware motif art strictly inside the enclosure's front face.
pub(super) fn paint_motif_art(ui: &egui::Ui, rect: Rect, spec: MotifSpec, on: bool, seed: u64) {
    let ctx = build_context(rect, spec, on, false, seed, &[]);
    run_motif_painter(ui, &STYLE_PAINTERS, spec.style, &ctx);
}

/// Draw a calmer expanded hardware motif inside a Deep Edit parameter bay.
/// The art that marks out controls (band wells, a halo, a split, a trace)
/// follows `cells`, the bay's dial cells.
pub(super) fn paint_deep_motif_art(
    ui: &egui::Ui,
    rect: Rect,
    spec: MotifSpec,
    active: bool,
    seed: u64,
    cells: &[Rect],
) {
    let ctx = build_context(rect, spec, active, true, seed, cells);
    run_motif_painter(ui, &DEEP_STYLE_PAINTERS, spec.style, &ctx);
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

fn deep_deck(ctx: &MotifContext) -> Rect {
    Rect::from_min_max(
        Pos2::new(ctx.rect.left() + 10.0, ctx.rect.top() + 22.0),
        Pos2::new(ctx.rect.right() - 10.0, ctx.rect.bottom() - 10.0),
    )
}

/// The x of each boundary between two dial columns, left to right.
fn column_boundaries(cells: &[Rect]) -> Vec<f32> {
    let mut xs: Vec<f32> = cells
        .iter()
        .flat_map(|cell| [cell.left(), cell.right()])
        .collect();
    xs.sort_by(f32::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    // The outer edges of the grid are no boundaries.
    if xs.len() >= 2 {
        xs.remove(0);
        xs.pop();
    }
    xs
}

fn draw_graphic_grid(
    p: &egui::Painter,
    ctx: &MotifContext,
    band: Rect,
    fill_alpha: u8,
    line_alpha: f32,
    border_alpha: f32,
) {
    p.rect_filled(
        band,
        4.0,
        Color32::from_rgba_unmultiplied(ctx.zone_b.r(), ctx.zone_b.g(), ctx.zone_b.b(), fill_alpha),
    );
    // On a Deep Edit bay the faders' grid lines run between the dial columns.
    let step = (band.width() / 8.0).max(12.0);
    let xs: Vec<f32> = if ctx.cells.is_empty() {
        (1..8).map(|col| band.left() + col as f32 * step).collect()
    } else {
        column_boundaries(&ctx.cells)
    };
    for x in xs {
        p.line_segment(
            [
                Pos2::new(x, band.top() + 4.0),
                Pos2::new(x, band.bottom() - 4.0),
            ],
            Stroke::new(0.8_f32, ctx.trim.gamma_multiply(line_alpha)),
        );
    }
    p.rect_stroke(
        band,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(border_alpha)),
        egui::StrokeKind::Inside,
    );
}

fn paint_graphic_grid(p: &egui::Painter, ctx: &MotifContext) {
    draw_graphic_grid(p, ctx, master_deck(ctx, 5.0), 85, 0.18, 0.45);
}

fn paint_deep_graphic_grid(p: &egui::Painter, ctx: &MotifContext) {
    draw_graphic_grid(p, ctx, deep_deck(ctx), 75, 0.14, 0.35);
}

fn draw_stepped_frame(
    p: &egui::Painter,
    ctx: &MotifContext,
    frame: Rect,
    shrink: f32,
    fill_alpha: u8,
    outer_alpha: f32,
    inner_alpha: f32,
) {
    p.rect_filled(
        frame.shrink(shrink),
        5.0,
        Color32::from_rgba_unmultiplied(ctx.zone_b.r(), ctx.zone_b.g(), ctx.zone_b.b(), fill_alpha),
    );
    p.rect_stroke(
        frame,
        4.0,
        Stroke::new(1.2_f32, ctx.trim.gamma_multiply(outer_alpha)),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        frame.shrink(shrink),
        3.0,
        Stroke::new(0.8_f32, ctx.trim.gamma_multiply(inner_alpha)),
        egui::StrokeKind::Inside,
    );
}

fn paint_stepped_frame(p: &egui::Painter, ctx: &MotifContext) {
    draw_stepped_frame(p, ctx, master_deck(ctx, 4.0), 3.0, 110, 0.6, 0.32);
}

fn paint_deep_stepped_frame(p: &egui::Painter, ctx: &MotifContext) {
    draw_stepped_frame(p, ctx, deep_deck(ctx), 3.5, 95, 0.48, 0.24);
}

fn draw_wood_cheek_pair(
    p: &egui::Painter,
    ctx: &MotifContext,
    left_x: f32,
    right_x: f32,
    top: f32,
    bottom: f32,
    cheek_w: f32,
) {
    for x0 in [left_x, right_x] {
        let cheek = Rect::from_min_max(Pos2::new(x0, top), Pos2::new(x0 + cheek_w, bottom));
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
}

fn draw_wood_color_blocks(p: &egui::Painter, start_x: f32, top_y: f32, step: f32) {
    let blocks = [
        Color32::from_rgb(232, 166, 98),
        Color32::from_rgb(112, 206, 196),
        Color32::from_rgb(208, 152, 236),
    ];
    for (idx, col) in blocks.into_iter().enumerate() {
        p.rect_filled(
            Rect::from_min_size(
                Pos2::new(start_x + idx as f32 * step, top_y),
                Vec2::new(11.0, 4.0),
            ),
            1.0,
            col,
        );
    }
}

fn paint_wood_cheeks(p: &egui::Painter, ctx: &MotifContext) {
    let rect = ctx.rect;
    let cheek_w = 6.5;
    draw_wood_cheek_pair(
        p,
        ctx,
        rect.left() + 1.5,
        rect.right() - cheek_w - 1.5,
        rect.top() + 6.0,
        rect.bottom() - 6.0,
        cheek_w,
    );
    draw_wood_color_blocks(
        p,
        rect.center().x - 1.5 * 14.0 + 1.0,
        rect.top() + HEADER + 6.0,
        14.0,
    );
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

fn paint_deep_wood_cheeks(p: &egui::Painter, ctx: &MotifContext) {
    let deck = deep_deck(ctx);
    let cheek_w = 6.5;
    draw_wood_cheek_pair(
        p,
        ctx,
        deck.left(),
        deck.right() - cheek_w,
        deck.top(),
        deck.bottom(),
        cheek_w,
    );
    // Right-aligned in header (`right - 56 .. right - 16`, clear of top-right screw).
    draw_wood_color_blocks(p, ctx.rect.right() - 56.0, ctx.rect.top() + 9.0, 14.5);
    p.rect_stroke(
        Rect::from_min_max(
            Pos2::new(deck.left() + cheek_w + 3.0, deck.top() + 1.0),
            Pos2::new(deck.right() - cheek_w - 3.0, deck.bottom() - 1.0),
        ),
        3.0,
        Stroke::new(0.9_f32, ctx.trim.gamma_multiply(0.25)),
        egui::StrokeKind::Inside,
    );
}

fn draw_diagonal_stripe(
    p: &egui::Painter,
    ctx: &MotifContext,
    deck: Rect,
    fill_alpha: u8,
    stripe_alpha: u8,
    border_alpha: f32,
) {
    let bottom_deck = Rect::from_min_max(
        Pos2::new(deck.left() + 3.0, deck.center().y + 4.0),
        deck.max - Vec2::splat(3.0),
    );
    p.rect_filled(
        bottom_deck,
        4.0,
        Color32::from_rgba_unmultiplied(ctx.zone_b.r(), ctx.zone_b.g(), ctx.zone_b.b(), fill_alpha),
    );
    p.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(deck.left() + 2.0, deck.bottom() - 26.0),
            Pos2::new(deck.right() - 2.0, deck.top() + 12.0),
            Pos2::new(deck.right() - 2.0, deck.top() + 26.0),
            Pos2::new(deck.left() + 2.0, deck.bottom() - 12.0),
        ],
        Color32::from_rgba_unmultiplied(ctx.trim.r(), ctx.trim.g(), ctx.trim.b(), stripe_alpha),
        Stroke::NONE,
    ));
    p.rect_stroke(
        deck,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(border_alpha)),
        egui::StrokeKind::Inside,
    );
}

fn paint_diagonal_stripe(p: &egui::Painter, ctx: &MotifContext) {
    draw_diagonal_stripe(p, ctx, master_deck(ctx, 4.0), 145, 36, 0.45);
}

fn paint_deep_diagonal_stripe(p: &egui::Painter, ctx: &MotifContext) {
    draw_diagonal_stripe(p, ctx, deep_deck(ctx), 110, 24, 0.38);
}

fn draw_tube_vents(
    p: &egui::Painter,
    ctx: &MotifContext,
    body: Rect,
    slit_center: Pos2,
    slit_step: f32,
    slit_size: Vec2,
    border_alpha: f32,
) {
    p.rect_filled(body, 5.0, ctx.zone_b);
    p.rect_stroke(
        body,
        5.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(border_alpha)),
        egui::StrokeKind::Inside,
    );
    let glow = if ctx.on {
        Color32::from_rgb(246, 174, 82)
    } else {
        Color32::from_rgb(56, 36, 28)
    };
    for slot in -2..=2 {
        let vx = slit_center.x + slot as f32 * slit_step;
        let slit = Rect::from_center_size(Pos2::new(vx, slit_center.y), slit_size);
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

fn paint_tube_vents(p: &egui::Painter, ctx: &MotifContext) {
    draw_tube_vents(
        p,
        ctx,
        master_deck(ctx, 4.0),
        Pos2::new(ctx.rect.center().x, ctx.rect.top() + HEADER + 11.0),
        15.0,
        Vec2::new(10.0, 3.0),
        0.55,
    );
}

fn paint_deep_tube_vents(p: &egui::Painter, ctx: &MotifContext) {
    // 5 vents right-aligned across `right - 88 .. right - 16` in header.
    draw_tube_vents(
        p,
        ctx,
        deep_deck(ctx),
        Pos2::new(ctx.rect.right() - 52.0, ctx.rect.top() + 11.0),
        14.0,
        Vec2::new(9.0, 3.0),
        0.45,
    );
}

fn draw_aluminum_curve(
    p: &egui::Painter,
    ctx: &MotifContext,
    deck: Rect,
    cy: f32,
    bump_h: f32,
    curve_alpha: f32,
    border_alpha: f32,
) {
    p.rect_filled(deck, 4.0, ctx.zone_b);
    p.rect_stroke(
        deck,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(border_alpha)),
        egui::StrokeKind::Inside,
    );
    let span = (deck.width() - 28.0).max(24.0);
    let curve: Vec<Pos2> = (0..=16)
        .map(|i| {
            let t = i as f32 / 16.0;
            let bump = (t * std::f32::consts::PI).sin() * bump_h;
            Pos2::new(deck.left() + 14.0 + t * span, cy - bump)
        })
        .collect();
    p.add(egui::Shape::line(
        curve,
        Stroke::new(1.2_f32, ctx.trim.gamma_multiply(curve_alpha)),
    ));
}

fn paint_aluminum_curve(p: &egui::Painter, ctx: &MotifContext) {
    draw_aluminum_curve(
        p,
        ctx,
        master_deck(ctx, 4.0),
        ctx.rect.top() + HEADER + 15.0,
        3.5,
        0.55,
        0.5,
    );
}

fn paint_deep_aluminum_curve(p: &egui::Painter, ctx: &MotifContext) {
    let deck = deep_deck(ctx);
    // The curve runs in the free band under the dials, never through them.
    // Without such a band the deck keeps its brushed plate and no curve.
    let lowest = ctx.cells.iter().map(|cell| cell.bottom()).reduce(f32::max);
    let (cy, curve_alpha) = match lowest {
        None => (deck.top() + 18.0, 0.24),
        Some(bottom) if deck.bottom() - bottom >= 20.0 => {
            ((bottom + deck.bottom()) / 2.0 + 3.0, 0.24)
        }
        Some(_) => (deck.top(), 0.0),
    };
    draw_aluminum_curve(p, ctx, deck, cy, 6.0, curve_alpha, 0.40);
}

fn amber_meter_bezel(p: &egui::Painter, meter_bezel: Rect, on: bool) {
    let amber = if on {
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
    amber_meter_bezel(p, meter_bezel, ctx.on);
}

fn paint_deep_rack_meter_deck(p: &egui::Painter, ctx: &MotifContext) {
    let deck = deep_deck(ctx);
    p.rect_filled(deck, 4.0, ctx.zone_b);
    p.rect_stroke(
        deck,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.42)),
        egui::StrokeKind::Inside,
    );
}

fn draw_radial_sunburst(
    p: &egui::Painter,
    ctx: &MotifContext,
    deck: Rect,
    center: Pos2,
    radius: f32,
    style: (u8, f32, f32),
) {
    let (halo_alpha, ray_alpha, border_alpha) = style;
    let halo =
        Color32::from_rgba_unmultiplied(ctx.zone_b.r(), ctx.zone_b.g(), ctx.zone_b.b(), halo_alpha);
    p.circle_filled(center, radius, halo);
    for ray in 0..16 {
        let dir = Vec2::angled((ray as f32 * 22.5).to_radians());
        p.line_segment(
            [
                center + dir * (radius * 0.54),
                center + dir * (radius + 2.0),
            ],
            Stroke::new(1.0_f32, ctx.trim.gamma_multiply(ray_alpha)),
        );
    }
    p.rect_stroke(
        deck,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(border_alpha)),
        egui::StrokeKind::Inside,
    );
}

fn paint_radial_sunburst(p: &egui::Painter, ctx: &MotifContext) {
    draw_radial_sunburst(
        p,
        ctx,
        master_deck(ctx, 4.0),
        Pos2::new(ctx.rect.center().x, ctx.rect.top() + 94.0),
        44.0,
        (if ctx.on { 150 } else { 90 }, 0.24, 0.45),
    );
}

fn paint_deep_radial_sunburst(p: &egui::Painter, ctx: &MotifContext) {
    let deck = deep_deck(ctx);
    // The burst rings the dials' knob row, clear of the value and the label.
    let (center, radius) = if ctx.cells.is_empty() {
        (deck.center(), (deck.height() * 0.42).clamp(36.0, 68.0))
    } else {
        let sum = ctx
            .cells
            .iter()
            .fold(Vec2::ZERO, |sum, cell| sum + knob_center(*cell).to_vec2());
        (
            (sum / ctx.cells.len() as f32).to_pos2(),
            DEEP_CELL_KNOB + 20.0,
        )
    };
    draw_radial_sunburst(
        p,
        ctx,
        deck,
        center,
        radius,
        (if ctx.on { 110 } else { 70 }, 0.22, 0.38),
    );
}

fn draw_atmospheric_gradient(p: &egui::Painter, ctx: &MotifContext, deck: Rect, border_alpha: f32) {
    theme::gradient(p, deck, 4.0, ctx.zone_a, ctx.zone_b);
    p.rect_stroke(
        deck,
        4.0,
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(border_alpha)),
        egui::StrokeKind::Inside,
    );
}

fn paint_atmospheric_gradient(p: &egui::Painter, ctx: &MotifContext) {
    draw_atmospheric_gradient(p, ctx, master_deck(ctx, 4.0), 0.55);
}

fn paint_deep_atmospheric_gradient(p: &egui::Painter, ctx: &MotifContext) {
    draw_atmospheric_gradient(p, ctx, deep_deck(ctx), 0.42);
}

fn draw_tape_deck_inset(
    p: &egui::Painter,
    ctx: &MotifContext,
    deck: Rect,
    badge: Option<Rect>,
    led_step: f32,
    border_alpha: f32,
) {
    p.rect_filled(deck, 5.0, ctx.zone_b);
    p.rect_stroke(
        deck,
        5.0,
        Stroke::new(1.1_f32, ctx.trim.gamma_multiply(border_alpha)),
        egui::StrokeKind::Inside,
    );
    let Some(badge) = badge else {
        return;
    };
    p.rect_filled(badge, badge.height() * 0.5, Color32::from_rgb(36, 40, 38));
    p.rect_stroke(
        badge,
        badge.height() * 0.5,
        Stroke::new(0.9_f32, ctx.trim.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    let led_color = if ctx.on {
        Color32::from_rgb(236, 72, 58)
    } else {
        Color32::from_rgb(76, 36, 34)
    };
    for offset in [-led_step, 0.0, led_step] {
        p.circle_filled(badge.center() + Vec2::new(offset, 0.0), 2.1, led_color);
    }
}

fn paint_tape_deck_inset(p: &egui::Painter, ctx: &MotifContext) {
    draw_tape_deck_inset(
        p,
        ctx,
        master_deck(ctx, 5.0),
        Some(Rect::from_center_size(
            Pos2::new(ctx.rect.center().x, ctx.rect.top() + HEADER + 13.0),
            Vec2::new(42.0, 11.0),
        )),
        10.0,
        0.65,
    );
}

fn paint_deep_tape_deck_inset(p: &egui::Painter, ctx: &MotifContext) {
    draw_tape_deck_inset(p, ctx, deep_deck(ctx), None, 0.0, 0.48);
}

/// Where the phase wave runs and whether the plate has its own inner frame.
struct PhaseWave {
    /// The wave's centre line, relative to the plate centre, or absolute.
    y: WaveY,
    amp: f32,
    alpha: u8,
    inner_frame: bool,
}

enum WaveY {
    FromCenter(f32),
    At(f32),
}

fn draw_phase_bezel(
    p: &egui::Painter,
    ctx: &MotifContext,
    bezel: Rect,
    inset: f32,
    wave: PhaseWave,
) {
    let PhaseWave {
        y,
        amp: wave_amp,
        alpha,
        inner_frame,
    } = wave;
    p.rect_filled(bezel, 5.0, ctx.secondary);
    let plate = bezel.shrink(inset);
    p.rect_filled(plate, 4.0, ctx.zone_a);
    if inner_frame {
        p.rect_stroke(
            plate.shrink(2.0),
            3.0,
            Stroke::new(1.0_f32, ctx.trim.gamma_multiply(0.65)),
            egui::StrokeKind::Inside,
        );
    }
    let cy = match y {
        WaveY::FromCenter(offset) => plate.center().y + offset,
        WaveY::At(cy) => cy,
    };
    let wave: Vec<Pos2> = (0..=18)
        .map(|i| {
            let t = i as f32 / 18.0;
            Pos2::new(
                plate.left() + 8.0 + t * (plate.width() - 16.0),
                cy + (t * std::f32::consts::TAU).sin() * wave_amp,
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

fn paint_phase_bezel(p: &egui::Painter, ctx: &MotifContext) {
    draw_phase_bezel(
        p,
        ctx,
        master_deck(ctx, 4.0),
        3.0,
        PhaseWave {
            y: WaveY::FromCenter(-14.0),
            amp: 8.0,
            alpha: if ctx.on { 65 } else { 30 },
            inner_frame: true,
        },
    );
}

fn paint_deep_phase_bezel(p: &egui::Painter, ctx: &MotifContext) {
    // On a Deep Edit bay the bay frame is the only frame, and the wave runs
    // along the knob row, behind the dials and clear of their labels.
    let y = match ctx.cells.first() {
        Some(cell) => WaveY::At(knob_center(*cell).y),
        None => WaveY::FromCenter(0.0),
    };
    draw_phase_bezel(
        p,
        ctx,
        deep_deck(ctx),
        2.5,
        PhaseWave {
            y,
            amp: 8.0,
            alpha: if ctx.on { 38 } else { 20 },
            inner_frame: false,
        },
    );
}

fn draw_tri_band_wells(p: &egui::Painter, ctx: &MotifContext, wells_at: [Rect; 3]) {
    let wells: [([u8; 3], [u8; 3]); 3] = [
        ([24, 52, 62], [236, 174, 82]),
        (
            [ctx.zone_b.r(), ctx.zone_b.g(), ctx.zone_b.b()],
            [102, 212, 238],
        ),
        ([38, 34, 78], [186, 152, 246]),
    ];
    for (idx, ((fill_rgb, cap_rgb), well)) in wells.into_iter().zip(wells_at).enumerate() {
        let fill = if idx == 1 {
            ctx.zone_b
        } else {
            dim_zone_by(fill_rgb, ctx.dim_amount, ctx.light_face)
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

fn paint_tri_band_wells(p: &egui::Painter, ctx: &MotifContext) {
    let well_w = 60.0;
    let start_x = ctx.rect.center().x - (3.0 * well_w + 8.0) / 2.0;
    let y0 = ctx.rect.top() + HEADER + 28.0;
    let y1 = ctx.rect.top() + FOOTER_TOP - 5.0;
    draw_tri_band_wells(p, ctx, three_wells(start_x, y0, y1, well_w));
}

fn three_wells(start_x: f32, y0: f32, y1: f32, well_w: f32) -> [Rect; 3] {
    std::array::from_fn(|idx| {
        let x0 = start_x + idx as f32 * (well_w + 4.0);
        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x0 + well_w, y1))
    })
}

fn paint_deep_tri_band_wells(p: &egui::Painter, ctx: &MotifContext) {
    let deck = deep_deck(ctx);
    // One well behind each band dial; a fourth dial (the key) stands on the
    // bare deck, so the wells read as the three bands they are.
    let wells = match ctx.cells.as_slice() {
        [low, mid, high, ..] => [*low, *mid, *high].map(|cell| {
            Rect::from_min_max(
                Pos2::new(cell.left() + 3.0, (cell.top() - 4.0).max(deck.top())),
                Pos2::new(cell.right() - 3.0, (cell.bottom() + 3.0).min(deck.bottom())),
            )
        }),
        _ => {
            let well_w = ((deck.width() - 8.0) / 3.0).max(40.0);
            three_wells(deck.left(), deck.top(), deck.bottom(), well_w)
        }
    };
    draw_tri_band_wells(p, ctx, wells);
}

fn draw_empty_bay_accents(p: &egui::Painter, bay: Rect, radius: f32) {
    let r = roles();
    match r.material {
        theme::Material::Flat => {
            p.rect_stroke(
                bay.shrink(4.0),
                (radius - 1.5).max(1.0),
                Stroke::new(1.0_f32, r.well_edge.gamma_multiply(0.28)),
                egui::StrokeKind::Inside,
            );
            for x in [bay.left() + 8.0, bay.right() - 8.0] {
                p.line_segment(
                    [
                        Pos2::new(x, bay.top() + 10.0),
                        Pos2::new(x, bay.bottom() - 10.0),
                    ],
                    Stroke::new(1.0_f32, r.well_edge.gamma_multiply(0.32)),
                );
            }
        }
        theme::Material::Plastic => {
            p.line_segment(
                [
                    Pos2::new(bay.left() + 8.0, bay.top() + 1.5),
                    Pos2::new(bay.right() - 8.0, bay.top() + 1.5),
                ],
                Stroke::new(1.0_f32, Color32::from_white_alpha(26)),
            );
        }
        theme::Material::Metal => {
            let mut y = bay.top() + 6.0;
            while y < bay.bottom() - 4.0 {
                p.line_segment(
                    [
                        Pos2::new(bay.left() + 6.0, y),
                        Pos2::new(bay.right() - 6.0, y),
                    ],
                    Stroke::new(1.0_f32, Color32::from_white_alpha(10)),
                );
                y += 4.0;
            }
            for x in [bay.left() + 8.0, bay.right() - 8.0] {
                for dy in [-18.0_f32, 18.0] {
                    let cy = (bay.center().y + dy).clamp(bay.top() + 8.0, bay.bottom() - 8.0);
                    p.circle_filled(Pos2::new(x, cy), 1.6, r.ink);
                    p.circle_stroke(
                        Pos2::new(x, cy),
                        1.6,
                        Stroke::new(0.7_f32, r.well_edge.gamma_multiply(0.5)),
                    );
                }
            }
        }
    }
}

fn draw_empty_bay(p: &egui::Painter, _ctx: &MotifContext, bay: Rect) {
    let r = roles();
    let (fill, radius, edge_alpha) = match r.material {
        theme::Material::Flat => (Color32::from_rgb(20, 23, 29), 5.0_f32, 0.55_f32),
        theme::Material::Plastic => (Color32::from_rgb(16, 26, 46), 8.0, 0.72),
        theme::Material::Metal => (Color32::from_rgb(21, 24, 23), 3.0, 0.65),
    };
    p.rect_filled(bay, radius, fill);
    p.rect_stroke(
        bay,
        radius,
        Stroke::new(1.0_f32, r.well_edge.gamma_multiply(edge_alpha)),
        egui::StrokeKind::Inside,
    );
    draw_empty_bay_accents(p, bay, radius);
}

fn paint_empty_bay(p: &egui::Painter, ctx: &MotifContext) {
    let bay = Rect::from_min_max(
        Pos2::new(ctx.inner.left() + 6.0, ctx.rect.top() + 72.0),
        Pos2::new(ctx.inner.right() - 6.0, ctx.rect.bottom() - 12.0),
    );
    draw_empty_bay(p, ctx, bay);
}

fn paint_deep_empty_bay(p: &egui::Painter, ctx: &MotifContext) {
    draw_empty_bay(p, ctx, deep_deck(ctx));
}

fn fx_deck(ctx: &MotifContext, margin_x: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(ctx.inner.left() + margin_x, ctx.rect.top() + 72.0),
        Pos2::new(ctx.inner.right() - margin_x, ctx.rect.bottom() - 48.0),
    )
}

fn draw_cosmic_nebula(
    p: &egui::Painter,
    ctx: &MotifContext,
    deck: Rect,
    alphas: [u8; 3],
    stars: usize,
    border_alpha: f32,
) {
    let [orb_a_alpha, orb_b_alpha, star_alpha] = alphas;
    let orb_a = deck.center() + Vec2::new(-deck.width() * 0.18, 6.0);
    let orb_b = deck.center() + Vec2::new(deck.width() * 0.20, -4.0);
    p.circle_filled(
        orb_a,
        deck.height() * 0.36,
        Color32::from_rgba_unmultiplied(
            ctx.zone_b.r(),
            ctx.zone_b.g(),
            ctx.zone_b.b(),
            orb_a_alpha,
        ),
    );
    p.circle_filled(
        orb_b,
        deck.height() * 0.28,
        Color32::from_rgba_unmultiplied(
            ctx.zone_a.r().saturating_add(10),
            ctx.zone_b.g().saturating_add(18),
            ctx.zone_b.b(),
            orb_b_alpha,
        ),
    );
    let mut rng = ctx.seed.wrapping_mul(0x9e37_79b9).wrapping_add(17);
    for _ in 0..stars {
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
        Stroke::new(1.0_f32, ctx.trim.gamma_multiply(border_alpha)),
        egui::StrokeKind::Inside,
    );
}

fn paint_cosmic_nebula(p: &egui::Painter, ctx: &MotifContext) {
    let alphas = if ctx.on {
        [165, 120, 155]
    } else {
        [90, 65, 70]
    };
    draw_cosmic_nebula(p, ctx, fx_deck(ctx, 4.0), alphas, 14, 0.45);
}

fn paint_deep_cosmic_nebula(p: &egui::Painter, ctx: &MotifContext) {
    let alphas = if ctx.on { [110, 85, 110] } else { [70, 55, 55] };
    draw_cosmic_nebula(p, ctx, deep_deck(ctx), alphas, 16, 0.38);
}

fn draw_dual_sub_bays(
    p: &egui::Painter,
    ctx: &MotifContext,
    outer: Rect,
    border_alpha: f32,
    show_bridge: bool,
) {
    // On a Deep Edit bay the split falls on the dial column boundary nearest
    // the middle, so no dial straddles the two sub-bays.
    let split = column_boundaries(&ctx.cells)
        .into_iter()
        .min_by(|a, b| {
            (a - outer.center().x)
                .abs()
                .total_cmp(&(b - outer.center().x).abs())
        })
        .unwrap_or(outer.center().x);
    let left_bay = Rect::from_min_max(
        outer.min,
        Pos2::new((split - 3.0).max(outer.left() + 32.0), outer.bottom()),
    );
    let right_bay = Rect::from_min_max(
        Pos2::new((split + 3.0).min(outer.right() - 32.0), outer.top()),
        outer.max,
    );
    let right_col = dim_zone_by([18, 44, 92], ctx.dim_amount, ctx.light_face);
    for (bay, col) in [(left_bay, ctx.zone_b), (right_bay, right_col)] {
        p.rect_filled(bay, 5.0, col);
        p.rect_stroke(
            bay,
            5.0,
            Stroke::new(1.0_f32, ctx.trim.gamma_multiply(border_alpha)),
            egui::StrokeKind::Inside,
        );
    }
    if show_bridge {
        let bridge =
            Rect::from_center_size(Pos2::new(split, outer.center().y), Vec2::new(16.0, 6.0));
        p.rect_filled(bridge, 3.0, ctx.trim.gamma_multiply(0.45));
    }
}

fn paint_dual_sub_bays(p: &egui::Painter, ctx: &MotifContext) {
    draw_dual_sub_bays(p, ctx, fx_deck(ctx, 4.0), 0.55, true);
}

fn paint_deep_dual_sub_bays(p: &egui::Painter, ctx: &MotifContext) {
    let deck = deep_deck(ctx);
    let top = ctx
        .cells
        .iter()
        .map(|cell| cell.top())
        .reduce(f32::min)
        .map_or(deck.top() + 34.0, |c| (c - 22.0).max(deck.top() + 34.0));
    let outer = Rect::from_min_max(Pos2::new(deck.left(), top), deck.max);
    draw_dual_sub_bays(p, ctx, outer, 0.42, false);
}

fn draw_drive_rail(
    p: &egui::Painter,
    ctx: &MotifContext,
    frame: Rect,
    pip_cx: f32,
    pip_y: f32,
    border_alpha: f32,
) {
    p.rect_filled(frame, 5.0, ctx.zone_b);
    p.rect_stroke(
        frame,
        5.0,
        Stroke::new(1.1_f32, ctx.trim.gamma_multiply(border_alpha)),
        egui::StrokeKind::Inside,
    );
    for (i, rgb_pip) in [[236, 94, 52], [244, 148, 56], [248, 194, 78], [244, 98, 92]]
        .into_iter()
        .enumerate()
    {
        let pip = Rect::from_center_size(
            Pos2::new(pip_cx + (i as f32 - 1.5) * 14.0, pip_y),
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

fn paint_drive_rail(p: &egui::Painter, ctx: &MotifContext) {
    let frame = fx_deck(ctx, 5.0);
    draw_drive_rail(p, ctx, frame, ctx.rect.center().x, frame.top() + 5.0, 0.6);
}

fn paint_deep_drive_rail(p: &egui::Painter, ctx: &MotifContext) {
    // 4 pips right-aligned across `right - 68 .. right - 16` in header.
    draw_drive_rail(
        p,
        ctx,
        deep_deck(ctx),
        ctx.rect.right() - 42.0,
        ctx.rect.top() + 11.0,
        0.48,
    );
}
