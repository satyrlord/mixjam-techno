//! The pedal and rack-unit enclosures: unified 3-theme hardware shells
//! (Studio flat enamel, Aero molded plastic, Vintage 3D brushed metal) wrapping
//! bespoke per-unit artwork from `master/motifs.rs`.
use super::*;

const MASTER_MOTIFS: [(PedalKind, MotifSpec); 13] = [
    (
        PedalKind::SoftClip,
        MotifSpec {
            base: [246, 162, 74],
            secondary: [232, 138, 62],
            trim: [46, 22, 8],
            light_face: true,
            style: MotifStyle::DiagonalStripe,
            knob: theme::KnobStyle::Anodized([196, 98, 28]),
        },
    ),
    (
        PedalKind::Tube,
        MotifSpec {
            base: [106, 22, 28],
            secondary: [74, 16, 22],
            trim: [246, 198, 120],
            light_face: false,
            style: MotifStyle::TubeVents,
            knob: theme::KnobStyle::DarkPhenolic([248, 196, 114]),
        },
    ),
    (
        PedalKind::TrimEq,
        MotifSpec {
            base: [198, 202, 204],
            secondary: [176, 182, 186],
            trim: [34, 38, 42],
            light_face: true,
            style: MotifStyle::AluminumCurve,
            knob: theme::KnobStyle::BrushedChrome([24, 28, 32]),
        },
    ),
    (
        PedalKind::BusCompressor,
        MotifSpec {
            base: [30, 35, 40],
            secondary: [52, 59, 67],
            trim: [198, 210, 222],
            light_face: false,
            style: MotifStyle::RackMeterDeck,
            knob: theme::KnobStyle::BrushedChrome([20, 24, 28]),
        },
    ),
    (
        PedalKind::LiftEq,
        MotifSpec {
            base: [202, 230, 248],
            secondary: [124, 186, 238],
            trim: [24, 54, 96],
            light_face: true,
            style: MotifStyle::AtmosphericGradient,
            knob: theme::KnobStyle::BrushedChrome([22, 52, 98]),
        },
    ),
    (
        PedalKind::Maximizer,
        MotifSpec {
            base: [132, 218, 140],
            secondary: [168, 228, 118],
            trim: [14, 38, 22],
            light_face: true,
            style: MotifStyle::RadialSunburst,
            knob: theme::KnobStyle::Anodized([44, 156, 78]),
        },
    ),
    (
        PedalKind::Tape,
        MotifSpec {
            base: [54, 59, 57],
            secondary: [48, 68, 42],
            trim: [220, 226, 208],
            light_face: false,
            style: MotifStyle::TapeDeckInset,
            knob: theme::KnobStyle::DarkPhenolic([244, 238, 220]),
        },
    ),
    (
        PedalKind::Multiband,
        MotifSpec {
            base: [22, 30, 42],
            secondary: [22, 46, 76],
            trim: [132, 224, 242],
            light_face: false,
            style: MotifStyle::TriBandWells,
            knob: theme::KnobStyle::Anodized([64, 162, 214]),
        },
    ),
    (
        PedalKind::Imaging,
        MotifSpec {
            base: [20, 22, 26],
            secondary: [188, 194, 198],
            trim: [236, 240, 244],
            light_face: false,
            style: MotifStyle::PhaseBezel,
            knob: theme::KnobStyle::BrushedChrome([20, 22, 26]),
        },
    ),
    (
        PedalKind::Limiter,
        MotifSpec {
            base: [58, 18, 36],
            secondary: [84, 24, 50],
            trim: [248, 152, 176],
            light_face: false,
            style: MotifStyle::SteppedFrame,
            knob: theme::KnobStyle::DarkPhenolic([252, 164, 182]),
        },
    ),
    (
        PedalKind::BoosterEq,
        MotifSpec {
            base: [232, 186, 82],
            secondary: [216, 168, 66],
            trim: [42, 30, 10],
            light_face: true,
            style: MotifStyle::GraphicGrid,
            knob: theme::KnobStyle::Default,
        },
    ),
    (
        PedalKind::BoosterCompressor,
        MotifSpec {
            base: [108, 206, 186],
            secondary: [86, 188, 168],
            trim: [12, 38, 34],
            light_face: true,
            style: MotifStyle::SteppedFrame,
            knob: theme::KnobStyle::Anodized([48, 152, 134]),
        },
    ),
    (
        PedalKind::BoosterWide,
        MotifSpec {
            base: [26, 28, 30],
            secondary: [102, 58, 34],
            trim: [234, 236, 238],
            light_face: false,
            style: MotifStyle::WoodCheeks,
            knob: theme::KnobStyle::DarkPhenolic([230, 188, 255]),
        },
    ),
];

/// Family accent stripe color (Amber/Copper saturation, Steel dynamics, Teal tone/stereo, Brass booster).
pub(super) fn family_accent(kind: PedalKind) -> Color32 {
    match kind {
        PedalKind::SoftClip | PedalKind::Tube | PedalKind::Maximizer | PedalKind::Tape => {
            Color32::from_rgb(236, 154, 62)
        }
        PedalKind::BusCompressor | PedalKind::Multiband | PedalKind::Limiter => {
            Color32::from_rgb(92, 184, 232)
        }
        PedalKind::TrimEq | PedalKind::LiftEq | PedalKind::Imaging => {
            Color32::from_rgb(102, 208, 178)
        }
        PedalKind::BoosterEq | PedalKind::BoosterCompressor | PedalKind::BoosterWide => {
            Color32::from_rgb(218, 184, 86)
        }
    }
}

/// Procedural stompbox motif for each Mastering pedal across all three themes.
pub(super) fn master_motif(kind: PedalKind) -> MotifSpec {
    MASTER_MOTIFS
        .iter()
        .find(|&&(entry_kind, _)| entry_kind == kind)
        .expect("every PedalKind has a master motif")
        .1
}

/// True when the mastering faceplate uses a dark background and light legend ink.
pub(super) fn master_face_dark_mode(kind: PedalKind) -> bool {
    !master_motif(kind).light_face
}

/// High-contrast silk-screen ink for mastering cards and chips.
pub(super) fn master_face_ink(kind: PedalKind) -> Color32 {
    if master_face_dark_mode(kind) {
        roles().legend
    } else {
        roles().ink
    }
}

/// Primary base color used on the compact signal-order chip strip.
pub(super) fn master_face_color(kind: PedalKind, on: bool) -> Color32 {
    let ([primary, _], _) = master_motif(kind).zones(on);
    primary
}

/// A signal-order chip with the same focus and appearance in both views.
pub(super) fn signal_chip(
    ui: &mut egui::Ui,
    chip: Rect,
    kind: PedalKind,
    on: bool,
    selected: bool,
    label: &str,
) -> Response {
    let response = ui.interact(chip, ui.id().with(("signal-chip", kind)), Sense::click());
    paint_signal_chip(ui, chip, kind, on, selected, response.hovered());
    if response.has_focus() {
        ui.painter().rect_stroke(
            chip.expand(1.0),
            4.5,
            Stroke::new(1.5_f32, controls::focus_color(ui)),
            egui::StrokeKind::Outside,
        );
    }
    if response.gained_focus() {
        response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
    }
    named(&response, egui::WidgetType::Button, label);
    response
}

fn paint_signal_chip(
    ui: &egui::Ui,
    chip: Rect,
    kind: PedalKind,
    on: bool,
    selected: bool,
    hovered: bool,
) {
    let face = master_face_color(kind, on);
    let ink = master_face_ink(kind);
    let accent = family_accent(kind);
    let stripe = if on {
        accent
    } else {
        accent.gamma_multiply(0.28)
    };
    ui.painter().rect_filled(chip, 4.0, face);
    ui.painter().rect_filled(
        Rect::from_min_size(
            chip.min + Vec2::new(2.0, 1.5),
            Vec2::new(chip.width() - 4.0, 2.5),
        ),
        1.0,
        stripe,
    );
    if hovered {
        ui.painter()
            .rect_filled(chip, 4.0, Color32::from_white_alpha(24));
    }
    if selected {
        ui.painter().rect_stroke(
            chip,
            4.0,
            Stroke::new(1.0_f32, roles().ink),
            egui::StrokeKind::Inside,
        );
        ui.painter().rect_stroke(
            chip,
            4.5,
            Stroke::new(2.0_f32, roles().legend),
            egui::StrokeKind::Outside,
        );
    } else {
        ui.painter().rect_stroke(
            chip,
            4.0,
            Stroke::new(1.0_f32, roles().ink.gamma_multiply(0.65)),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().text(
        chip.center() + Vec2::new(0.0, 1.0),
        Align2::CENTER_CENTER,
        short_name(kind),
        FontId::proportional(10.5),
        ink,
    );
}

fn draw_metal_screws(painter: &egui::Painter, rect: Rect, include_bottom: bool) {
    let mut corners = vec![Vec2::new(6.0, 6.0), Vec2::new(rect.width() - 6.0, 6.0)];
    if include_bottom {
        corners.push(Vec2::new(6.0, rect.height() - 6.0));
        corners.push(Vec2::new(rect.width() - 6.0, rect.height() - 6.0));
    }
    for offset in corners {
        let c = rect.min + offset;
        painter.circle_filled(c, 2.2, roles().well_edge);
        painter.circle_stroke(c, 2.2, Stroke::new(0.7_f32, roles().ink));
        painter.line_segment(
            [c - Vec2::new(1.3, 1.0), c + Vec2::new(1.3, 1.0)],
            Stroke::new(0.8_f32, roles().ink),
        );
    }
}

fn paint_metal_shell(ui: &egui::Ui, rect: Rect, base: Color32) -> (Vec<Pos2>, egui::Mesh) {
    let painter = ui.painter();
    let radius = egui::CornerRadius::same(CORNER as u8);
    let body = Rect::from_min_max(rect.min, rect.max + Vec2::splat(DEPTH));
    for (offset, blur, alpha) in [([5, 8], 16, 160), ([1, 2], 4, 150)] {
        painter.add(
            egui::epaint::Shadow {
                offset,
                blur,
                spread: 0,
                color: Color32::from_black_alpha(alpha),
            }
            .as_shape(body, radius),
        );
    }
    let along = Vec2::splat(DEPTH);
    let mut outline = Vec::new();
    for (point, facing) in contour(rect, 0.0) {
        let away = facing.x + facing.y;
        if away > 1e-3 {
            outline.push(point + along);
        } else if away < -1e-3 {
            outline.push(point);
        } else if facing.x > 0.0 {
            outline.extend([point, point + along]);
        } else {
            outline.extend([point + along, point]);
        }
    }
    painter.add(egui::Shape::convex_polygon(
        outline.clone(),
        lit(base, -0.62),
        Stroke::NONE,
    ));
    let face = bevel(rect, base);
    painter.add(face.clone());
    (outline, face)
}

fn paint_metal_finish(
    ui: &egui::Ui,
    rect: Rect,
    outline: Vec<Pos2>,
    face: &egui::Mesh,
    on: bool,
    seed: u64,
    scratches_on: bool,
) {
    let painter = ui.painter();
    if on {
        let finish = finish(ui.ctx());
        let pixels = ui.ctx().pixels_per_point();
        let start = Vec2::new((seed * 97 % 512) as f32, (seed * 61 % 256) as f32);
        let grain = overlay(face, finish.grain.id(), |pt| {
            let texel = (pt - rect.min) * pixels + start;
            Pos2::new(texel.x / GRAIN[0] as f32, texel.y / GRAIN[1] as f32)
        });
        let sheen = overlay(face, finish.sheen.id(), |pt| {
            let lean = SHEEN_LEAN * (rect.bottom() - pt.y);
            let across = (pt.x - rect.left() + lean) / (rect.width() + SHEEN_LEAN * rect.height());
            Pos2::new(across, 0.5)
        });
        painter.add(grain);
        painter.add(sheen);
        if scratches_on {
            let below = rect.shrink(BEVEL + 3.0);
            let below =
                Rect::from_x_y_ranges(below.x_range(), rect.top() + HEADER + 4.0..=below.bottom());
            scratches(painter, below, seed);
        }
    }
    painter.add(egui::Shape::closed_line(
        outline,
        Stroke::new(1.5_f32, roles().ink),
    ));
    draw_metal_screws(painter, rect, true);
}

/// Shared 3-theme stompbox/rack card renderer for Mastering pedals and FX units.
pub(crate) fn paint_motif_enclosure(
    ui: &egui::Ui,
    rect: Rect,
    spec: MotifSpec,
    on: bool,
    seed: u64,
) {
    let ([zone_a, _], _) = spec.zones(on);
    let painter = ui.painter();
    match roles().material {
        theme::Material::Flat => {
            painter.rect_filled(rect, 4.0, zone_a);
            super::motifs::paint_motif_art(ui, rect, spec, on, seed);
            painter.rect_stroke(
                rect,
                4.0,
                Stroke::new(1.4_f32, roles().ink),
                egui::StrokeKind::Inside,
            );
        }
        theme::Material::Plastic => {
            painter.add(
                egui::epaint::Shadow {
                    offset: [0, 3],
                    blur: 8,
                    spread: 0,
                    color: Color32::from_black_alpha(100),
                }
                .as_shape(rect, egui::CornerRadius::same(10)),
            );
            theme::plastic(painter, rect, 10.0, zone_a);
            super::motifs::paint_motif_art(ui, rect, spec, on, seed);
            painter.rect_stroke(
                rect,
                10.0,
                Stroke::new(1.0_f32, zone_a.lerp_to_gamma(Color32::BLACK, 0.45)),
                egui::StrokeKind::Inside,
            );
        }
        theme::Material::Metal => {
            let (outline, face) = paint_metal_shell(ui, rect, zone_a);
            super::motifs::paint_motif_art(ui, rect, spec, on, seed);
            paint_metal_finish(
                ui,
                rect,
                outline,
                &face,
                on,
                seed,
                spec.style != MotifStyle::EmptyBay,
            );
        }
    }
}

/// Draw the 3-theme Mastering card chassis.
pub(super) fn master_enclosure(ui: &egui::Ui, rect: Rect, kind: PedalKind, on: bool) {
    paint_motif_enclosure(ui, rect, master_motif(kind), on, kind as u64 + 1);
}

#[derive(Clone, Copy)]
struct DeepBayArt<'a> {
    spec: MotifSpec,
    active: bool,
    seed: u64,
    /// The dial cells the motif art follows, or `None` for a plain frame.
    cells: Option<&'a [Rect]>,
}

fn paint_deep_metal_bay(
    ui: &egui::Ui,
    rect: Rect,
    zone_a: Color32,
    trim: Color32,
    art: DeepBayArt<'_>,
) {
    let painter = ui.painter();
    let plate = rect.shrink(1.5);
    painter.rect_filled(rect, 6.0, roles().ink);
    theme::gradient(painter, plate, 5.0, lit(zone_a, 0.04), lit(zone_a, -0.05));
    if let Some(cells) = art.cells {
        super::motifs::paint_deep_motif_art(ui, rect, art.spec, art.active, art.seed, cells);
    }
    if art.active {
        let finish = finish(ui.ctx());
        let pixels = ui.ctx().pixels_per_point();
        let start = Pos2::new(
            ((art.seed * 97 % 512) as f32) / GRAIN[0] as f32,
            ((art.seed * 61 % 256) as f32) / GRAIN[1] as f32,
        );
        let span = Pos2::new(
            start.x + plate.width() * pixels / GRAIN[0] as f32,
            start.y + plate.height() * pixels / GRAIN[1] as f32,
        );
        let mut grain = egui::Mesh::with_texture(finish.grain.id());
        grain.add_rect_with_uv(plate, Rect::from_min_max(start, span), Color32::WHITE);
        painter.add(grain);
    }
    painter.rect_stroke(
        plate,
        5.0,
        Stroke::new(1.0_f32, trim.gamma_multiply(0.30)),
        egui::StrokeKind::Inside,
    );
    draw_metal_screws(painter, rect, rect.height() >= 84.0);
    controls::instrument::paint_metal_top_and_led(painter, rect, art.active, art.cells.is_some());
}

fn paint_deep_bay_shell(
    ui: &egui::Ui,
    rect: Rect,
    zone_a: Color32,
    trim: Color32,
    art: DeepBayArt<'_>,
) {
    let r = roles();
    match r.material {
        theme::Material::Flat => {
            let p = ui.painter();
            p.rect_filled(rect, 4.0, zone_a);
            if let Some(cells) = art.cells {
                super::motifs::paint_deep_motif_art(
                    ui, rect, art.spec, art.active, art.seed, cells,
                );
            }
            p.rect_stroke(
                rect,
                4.0,
                Stroke::new(1.2_f32, r.ink),
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                rect.shrink(1.2),
                3.0,
                Stroke::new(0.9_f32, trim.gamma_multiply(0.32)),
                egui::StrokeKind::Inside,
            );
        }
        theme::Material::Plastic => {
            theme::plastic(ui.painter(), rect, 8.0, zone_a);
            if let Some(cells) = art.cells {
                super::motifs::paint_deep_motif_art(
                    ui, rect, art.spec, art.active, art.seed, cells,
                );
            }
            ui.painter().rect_stroke(
                rect,
                8.0,
                Stroke::new(1.0_f32, zone_a.lerp_to_gamma(Color32::BLACK, 0.42)),
                egui::StrokeKind::Inside,
            );
        }
        theme::Material::Metal => paint_deep_metal_bay(ui, rect, zone_a, trim, art),
    }
}

/// Draw a unit-skinned parameter bay mounted flat inside a Deep Edit rack shell.
/// With `art`, the bay carries its unit's motif art drawn to those dial cells
/// (see `motifs::deep_cell`); without it, a plain framed plate.
pub(crate) fn paint_deep_edit_bay(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    spec: MotifSpec,
    active: bool,
    seed: u64,
    art: Option<&[Rect]>,
) {
    let ([zone_a, _], trim) = spec.deep_zones(active);
    let ink = spec.face_ink();
    let is_metal = roles().material == theme::Material::Metal;
    let show_led = art.is_some();
    let deep_art = DeepBayArt {
        spec,
        active,
        seed,
        cells: art,
    };
    paint_deep_bay_shell(ui, rect, zone_a, trim, deep_art);
    let title_x = rect.left() + if is_metal && show_led { 24.0 } else { 12.0 };
    // The motif deck starts right under the title, so a rule there would
    // stack a second line on the deck's own edge.
    let rule_color = if show_led {
        Color32::TRANSPARENT
    } else {
        trim.gamma_multiply(if active { 0.65 } else { 0.35 })
    };
    controls::instrument::paint_bay_header(
        ui.painter(),
        rect,
        title,
        title_x,
        ink,
        rule_color,
        is_metal,
    );
}

/// The pedal's corner radius, the width of its rounded bevel, and how far its
/// side walls show below and to the right of the face.
const CORNER: f32 = 9.0;
const BEVEL: f32 = 6.0;
const DEPTH: f32 = 4.0;
/// Light falls on the board from the upper left, at 135 degrees, as it does
/// on the knob caps. This points from a pedal toward it.
const TO_LIGHT: Vec2 = Vec2::new(
    -std::f32::consts::FRAC_1_SQRT_2,
    -std::f32::consts::FRAC_1_SQRT_2,
);

/// How far the metal finish darkens the enamel at most: the grain's deepest
/// streak and the sheen's darker reflection. Ink keeps its contrast there.
pub(super) const GRAIN_DIP: f32 = 0.05;
pub(super) const SHEEN_DIP: f32 = 0.08;
/// The grain's typical depth, and the size of its tile in screen pixels.
const GRAIN_DEPTH: f32 = 0.02;
pub(super) const GRAIN: [usize; 2] = [512, 256];
/// How far the sheen leans from upright, as a share of the pedal's height.
const SHEEN_LEAN: f32 = 0.2;
const ALUMINIUM: Color32 = Color32::from_rgb(236, 238, 240);

/// The metal finish shared by every pedal that is on.
struct Finish {
    grain: egui::TextureHandle,
    sheen: egui::TextureHandle,
}

fn finish(ctx: &egui::Context) -> Arc<Finish> {
    let id = egui::Id::new("pedal-metal-finish");
    if let Some(finish) = ctx.data(|data| data.get_temp::<Arc<Finish>>(id)) {
        return finish;
    }
    let finish = Arc::new(Finish {
        grain: ctx.load_texture(
            "pedal-grain",
            grain_image(),
            egui::TextureOptions::NEAREST_REPEAT,
        ),
        sheen: ctx.load_texture("pedal-sheen", sheen_image(), egui::TextureOptions::LINEAR),
    });
    ctx.data_mut(|data| data.insert_temp(id, finish.clone()));
    finish
}

/// A texel that lightens what lies under it by a positive amount, adding
/// light, or darkens it by a negative one.
fn light_texel(amount: f32) -> Color32 {
    let level = (amount.abs().min(1.0) * 255.0).round() as u8;
    if amount > 0.0 {
        Color32::from_rgba_premultiplied(level, level, level, 0)
    } else {
        Color32::from_rgba_premultiplied(0, 0, 0, level)
    }
}

/// Brushed aluminium: fine streaks along the rows, each row a little lighter
/// or darker than the next and changing slowly along its length. The tile
/// repeats without a seam.
pub(super) fn grain_image() -> egui::ColorImage {
    const KNOT: usize = 32;
    let [width, height] = GRAIN;
    let mut random = Random(0x6d61_7374_6572);
    let mut values = Vec::with_capacity(width * height);
    for _ in 0..height {
        let row = random.normal();
        let knots: Vec<f32> = (0..width / KNOT).map(|_| random.normal()).collect();
        for x in 0..width {
            let (knot, part) = (x / KNOT, (x % KNOT) as f32 / KNOT as f32);
            let blend = (1.0 - (part * std::f32::consts::PI).cos()) / 2.0;
            let next = knots[(knot + 1) % knots.len()];
            let along = knots[knot] + (next - knots[knot]) * blend;
            values.push(0.6 * row + 0.45 * along + 0.25 * random.normal());
        }
    }
    let deviation =
        (values.iter().map(|value| value * value).sum::<f32>() / values.len() as f32).sqrt();
    let pixels = values
        .iter()
        .map(|value| light_texel((value / deviation * GRAIN_DEPTH).clamp(-GRAIN_DIP, GRAIN_DIP)))
        .collect();
    egui::ColorImage::new(GRAIN, pixels)
}

/// The sheen across a pedal from its left edge to its right. Brushed metal
/// stretches a highlight across its grain: a bright streak toward the light,
/// a darker reflection, then a fainter streak.
pub(super) fn sheen(across: f32) -> f32 {
    let bump = |centre: f32, width: f32| (-((across - centre) / width).powi(2)).exp();
    0.09 * bump(0.24, 0.16) + 0.13 * bump(0.24, 0.05) + 0.06 * bump(0.74, 0.07)
        - SHEEN_DIP * bump(0.5, 0.14)
}

fn sheen_image() -> egui::ColorImage {
    const TEXELS: usize = 256;
    let pixels = (0..TEXELS)
        .map(|texel| light_texel(sheen((texel as f32 + 0.5) / TEXELS as f32)))
        .collect();
    egui::ColorImage::new([TEXELS, 1], pixels)
}

/// The face's mesh again, painted with a texture laid across it.
fn overlay(face: &egui::Mesh, texture: egui::TextureId, uv: impl Fn(Pos2) -> Pos2) -> egui::Mesh {
    let mut mesh = egui::Mesh::with_texture(texture);
    mesh.indices.clone_from(&face.indices);
    mesh.vertices = face
        .vertices
        .iter()
        .map(|vertex| egui::epaint::Vertex {
            pos: vertex.pos,
            uv: uv(vertex.pos),
            color: Color32::WHITE,
        })
        .collect();
    mesh
}

/// Hairline scratches through the anodising, where the bare aluminium shows.
/// Each pedal keeps its own, seeded by its kind, so they stay with it as it
/// moves. Most are short and run in the pedal's few usual directions, some
/// in pairs or threes; a few are long. In each groove the wall toward the
/// light falls dark and the far wall catches it, brightest on scratches
/// across the light.
fn scratches(painter: &egui::Painter, face: Rect, seed: u64) {
    let mut random = Random(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let usual = [random.unit(), random.unit()].map(|turn| turn * std::f32::consts::PI);
    let count = 8 + (face.area() / 4500.0) as usize;
    for _ in 0..count {
        let heading = if random.unit() < 0.3 {
            random.unit() * std::f32::consts::PI
        } else {
            usual[usize::from(random.unit() < 0.5)] + (random.unit() - 0.5) * 0.4
        };
        let length = if random.unit() < 0.2 {
            40.0 + 90.0 * random.unit()
        } else {
            5.0 + 25.0 * random.unit()
        };
        let depth = 0.35 + 0.65 * random.unit().powi(2);
        let bend = (random.unit() - 0.5) * 0.5;
        let start = face.min + Vec2::new(random.unit(), random.unit()) * face.size();
        let siblings = if random.unit() < 0.3 {
            1 + usize::from(random.unit() < 0.5)
        } else {
            0
        };
        let gap = 1.5 + 1.5 * random.unit();
        for sibling in 0..=siblings {
            let shift = Vec2::angled(heading + std::f32::consts::FRAC_PI_2) * gap * sibling as f32;
            let share = 1.0 - 0.25 * sibling as f32;
            scratch(
                painter,
                face,
                start + shift,
                heading,
                length * share,
                bend,
                depth * share,
            );
        }
    }
}

/// One scratch along a gentle curve, fading in and out at its ends. It stops
/// where it would leave the face.
fn scratch(
    painter: &egui::Painter,
    face: Rect,
    start: Pos2,
    heading: f32,
    length: f32,
    bend: f32,
    depth: f32,
) {
    const SEGMENTS: usize = 5;
    let mut points = vec![start];
    for segment in 0..SEGMENTS {
        let turn = heading + bend * (segment as f32 / SEGMENTS as f32 - 0.5);
        let next = points[segment] + Vec2::angled(turn) * (length / SEGMENTS as f32);
        if !face.contains(next) {
            break;
        }
        points.push(next);
    }
    if points.len() < 3 {
        return;
    }
    let glint = 0.55 + 0.45 * Vec2::angled(heading).rot90().dot(TO_LIGHT).abs();
    let last = (points.len() - 1) as f32;
    for (index, pair) in points.windows(2).enumerate() {
        let middle = (index as f32 + 0.5) / last;
        let fade = (middle * std::f32::consts::PI).sin().sqrt() * depth;
        let [from, to] = [pair[0], pair[1]];
        let shadow = TO_LIGHT * 0.6;
        painter.line_segment(
            [from + shadow, to + shadow],
            Stroke::new(0.8_f32, roles().ink.gamma_multiply(0.35 * fade)),
        );
        painter.line_segment(
            [from - TO_LIGHT * 0.2, to - TO_LIGHT * 0.2],
            Stroke::new(0.8_f32, ALUMINIUM.gamma_multiply(0.9 * fade * glint)),
        );
    }
}

/// A small seeded generator, so the grain and each pedal's scratches come out
/// the same on every frame and every run.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    /// Uniform in [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1_u64 << 24) as f32
    }

    /// Standard normal.
    fn normal(&mut self) -> f32 {
        let radius = (-2.0 * (1.0 - self.unit()).ln()).sqrt();
        radius * (std::f32::consts::TAU * self.unit()).cos()
    }
}

/// The face as one mesh: rings from the edge in to the level face, then a
/// fan across it. Light on the bevel follows how steep it is at each ring and
/// which way that stretch of edge faces; the face brightens a little toward
/// the upper left.
fn bevel(rect: Rect, enamel: Color32) -> egui::Mesh {
    const RINGS: usize = 4;
    let span = rect.width() + rect.height();
    let shade = |point: Pos2, light: f32| {
        let sheen = 0.07 * (1.0 - (point.x - rect.left() + point.y - rect.top()) / span);
        lit(enamel, sheen + light)
    };
    let mut mesh = egui::Mesh::default();
    let mut around = 0;
    for ring in 0..=RINGS {
        let inward = ring as f32 / RINGS as f32;
        // A rounded edge: steep at the outline, level where it meets the face.
        let slope = (1.0 - inward).powf(1.1);
        let points = contour(rect, BEVEL * inward);
        around = points.len();
        for (point, facing) in points {
            let facing = facing.dot(TO_LIGHT);
            let light = slope * if facing > 0.0 { 0.68 } else { 0.6 } * facing;
            mesh.colored_vertex(point, shade(point, light));
        }
    }
    let ring = |ring: usize, index: usize| (ring * around + index % around) as u32;
    for outer in 0..RINGS {
        for index in 0..around {
            let [a, b] = [ring(outer, index), ring(outer, index + 1)];
            let [c, d] = [ring(outer + 1, index), ring(outer + 1, index + 1)];
            mesh.add_triangle(a, b, d);
            mesh.add_triangle(a, d, c);
        }
    }
    let centre = mesh.vertices.len() as u32;
    mesh.colored_vertex(rect.center(), shade(rect.center(), 0.0));
    for index in 0..around {
        mesh.add_triangle(centre, ring(RINGS, index), ring(RINGS, index + 1));
    }
    mesh
}

/// Points around the pedal's rounded outline, moved in by `inset`, each with
/// the outward direction it faces, clockwise from the lower right corner.
/// Each corner turns in even steps, so one point there faces diagonally.
fn contour(rect: Rect, inset: f32) -> Vec<(Pos2, Vec2)> {
    const STEPS: usize = 8;
    let centres = [
        rect.right_bottom() + Vec2::new(-CORNER, -CORNER),
        rect.left_bottom() + Vec2::new(CORNER, -CORNER),
        rect.left_top() + Vec2::new(CORNER, CORNER),
        rect.right_top() + Vec2::new(-CORNER, CORNER),
    ];
    let mut points = Vec::with_capacity(4 * (STEPS + 1));
    for (corner, centre) in centres.into_iter().enumerate() {
        for step in 0..=STEPS {
            let turn = corner as f32 + step as f32 / STEPS as f32;
            let facing = Vec2::angled(turn * std::f32::consts::FRAC_PI_2);
            points.push((centre + facing * (CORNER - inset), facing));
        }
    }
    points
}

/// The enamel as the light finds it: a positive amount lightens it toward
/// cream, a negative one darkens it.
pub(super) fn lit(enamel: Color32, amount: f32) -> Color32 {
    let toward = if amount > 0.0 {
        roles().legend
    } else {
        Color32::BLACK
    };
    let mix = |from: u8, to: u8| {
        (f32::from(from) + (f32::from(to) - f32::from(from)) * amount.abs().min(1.0)).round() as u8
    };
    Color32::from_rgb(
        mix(enamel.r(), toward.r()),
        mix(enamel.g(), toward.g()),
        mix(enamel.b(), toward.b()),
    )
}
