//! The pedal enclosure: enamel colors, and the painted metal finish with its
//! brushed grain, sheen, scratches, and bevel.
use super::*;

/// Family accent stripe color (Amber/Copper saturation, Steel dynamics, Teal tone/stereo, Brass booster).
pub(super) fn family_accent(kind: PedalKind) -> Color32 {
    match kind {
        PedalKind::SoftClip | PedalKind::Tube | PedalKind::Maximizer | PedalKind::Tape => {
            Color32::from_rgb(232, 152, 62)
        }
        PedalKind::BusCompressor | PedalKind::Multiband | PedalKind::Limiter => {
            Color32::from_rgb(92, 172, 224)
        }
        PedalKind::TrimEq | PedalKind::LiftEq | PedalKind::Imaging => {
            Color32::from_rgb(98, 198, 172)
        }
        PedalKind::BoosterEq | PedalKind::BoosterCompressor | PedalKind::BoosterWide => {
            Color32::from_rgb(206, 178, 92)
        }
    }
}

/// True when the mastering faceplate uses a dark background and light legend ink.
pub(super) fn master_face_dark_mode() -> bool {
    matches!(
        roles().material,
        theme::Material::Flat | theme::Material::Metal
    )
}

/// High-contrast silk-screen ink for mastering cards and chips.
pub(super) fn master_face_ink() -> Color32 {
    if master_face_dark_mode() {
        roles().legend
    } else {
        roles().ink
    }
}

fn vintage_anodized_rgb(kind: PedalKind, on: bool) -> Color32 {
    if !on {
        return Color32::from_rgb(29, 30, 29);
    }
    let [r, g, b] = match kind {
        PedalKind::SoftClip | PedalKind::Tube | PedalKind::Maximizer | PedalKind::Tape => {
            [44, 37, 33]
        }
        PedalKind::BusCompressor | PedalKind::Multiband | PedalKind::Limiter => [33, 40, 46],
        PedalKind::TrimEq | PedalKind::LiftEq | PedalKind::Imaging => [33, 43, 39],
        _ => [42, 39, 32],
    };
    Color32::from_rgb(r, g, b)
}

/// 3-theme faceplate color for Mastering cards and chips:
/// - `Metal` (`Vintage`): dark 1950s–70s anodized charcoal/espresso aluminum (no pastels!)
/// - `Flat` (`Studio`): dark graphite faceplate
/// - `Plastic` (`Aero`): light molded translucent enamel
pub(super) fn master_face_color(kind: PedalKind, on: bool) -> Color32 {
    match roles().material {
        theme::Material::Metal => vintage_anodized_rgb(kind, on),
        theme::Material::Flat => {
            let [r, g, b] = if on { [40, 44, 50] } else { [28, 31, 35] };
            Color32::from_rgb(r, g, b)
        }
        theme::Material::Plastic => {
            enamel(kind, on).lerp_to_gamma(Color32::from_rgb(232, 240, 250), 0.35)
        }
    }
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
            chip.expand(2.0),
            5.0,
            Stroke::new(2.0_f32, controls::focus_color(ui)),
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
    let ink = master_face_ink();
    let accent = family_accent(kind);
    let stripe = if on || selected {
        accent
    } else {
        accent.gamma_multiply(0.30)
    };
    ui.painter().rect_filled(chip, 4.0, face);
    ui.painter().rect_filled(
        Rect::from_min_size(
            chip.min + Vec2::new(2.0, 1.0),
            Vec2::new(chip.width() - 4.0, 2.0),
        ),
        1.0,
        stripe,
    );
    if hovered {
        ui.painter()
            .rect_filled(chip, 4.0, Color32::from_white_alpha(24));
    }
    let border_w = if selected { 1.8_f32 } else { 1.0_f32 };
    let border_c = if selected {
        accent
    } else {
        ink.gamma_multiply(0.35)
    };
    ui.painter().rect_stroke(
        chip,
        4.0,
        Stroke::new(border_w, border_c),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        chip.center() + Vec2::new(0.0, 1.0),
        Align2::CENTER_CENTER,
        short_name(kind),
        FontId::proportional(10.5),
        ink,
    );
}

/// Draw the 3-theme Mastering card chassis (`Vintage` dark anodized metal with corner
/// screws, `Studio` dark graphite, `Aero` light molded plastic) plus top family stripe.
pub(super) fn master_enclosure(ui: &egui::Ui, rect: Rect, kind: PedalKind, on: bool) {
    let face = master_face_color(kind, on);
    let accent = family_accent(kind);
    let painter = ui.painter();
    match roles().material {
        theme::Material::Flat => {
            painter.rect_filled(rect, 4.0, face);
            painter.rect_stroke(
                rect,
                4.0,
                Stroke::new(
                    1.0_f32,
                    if on {
                        accent.gamma_multiply(0.55)
                    } else {
                        roles().bay_edge
                    },
                ),
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
            theme::plastic(painter, rect, 10.0, face);
        }
        theme::Material::Metal => {
            enclosure_of(ui, rect, face, on, kind as u64 + 1);
            // Engraved bezel & 4 corner slotted chassis screws for 1950s–70s hardware feel.
            painter.rect_stroke(
                rect.shrink(4.0),
                5.0,
                Stroke::new(1.0_f32, Color32::from_white_alpha(26)),
                egui::StrokeKind::Inside,
            );
            for offset in [
                Vec2::new(6.0, 6.0),
                Vec2::new(rect.width() - 6.0, 6.0),
                Vec2::new(6.0, rect.height() - 6.0),
                Vec2::new(rect.width() - 6.0, rect.height() - 6.0),
            ] {
                let c = rect.min + offset;
                painter.circle_filled(c, 2.2, roles().well_edge);
                painter.line_segment(
                    [c - Vec2::new(1.3, 1.0), c + Vec2::new(1.3, 1.0)],
                    Stroke::new(0.8_f32, Color32::from_black_alpha(160)),
                );
            }
        }
    }
    let stripe = Rect::from_min_size(
        rect.min + Vec2::new(8.0, 1.5),
        Vec2::new((rect.width() - 16.0).max(8.0), 2.5),
    );
    painter.rect_filled(
        stripe,
        1.0,
        if on {
            accent
        } else {
            accent.gamma_multiply(0.30)
        },
    );
}

/// The painted enclosure of each pedal. Like stompboxes, each kind has its
/// own enamel; it always appears with the pedal's printed name. The families
/// share hues: golds for equalisers, blues and greens for dynamics, warm
/// oranges for saturation, and violets for stereo. A pedal that is off
/// loses most of its color and some light, so the pedals in use stand out.
/// Ink lettering keeps at least 5:1 contrast in both states.
pub(super) fn enamel(kind: PedalKind, on: bool) -> Color32 {
    enamel_of(
        match kind {
            PedalKind::BoosterEq => [222, 172, 64],
            PedalKind::BoosterCompressor => [84, 184, 166],
            PedalKind::BoosterWide => [200, 150, 220],
            PedalKind::SoftClip => [240, 150, 120],
            PedalKind::Tube => [240, 166, 80],
            PedalKind::TrimEq => [196, 204, 92],
            PedalKind::BusCompressor => [120, 178, 228],
            PedalKind::Maximizer => [130, 206, 150],
            PedalKind::LiftEq => [236, 208, 108],
            PedalKind::Tape => [206, 180, 140],
            PedalKind::Imaging => [166, 160, 232],
            PedalKind::Multiband => [100, 196, 204],
            PedalKind::Limiter => [238, 140, 160],
        },
        on,
    )
}

/// An enamel colour, lit when its unit is on, greyed and darkened when off.
pub(crate) fn enamel_of([red, green, blue]: [u8; 3], on: bool) -> Color32 {
    let color = enamel_finish(red, green, blue, on);
    match roles().material {
        // Studio's software modules keep each hue but settle it toward a
        // matte light gray.
        theme::Material::Flat => color.lerp_to_gamma(Color32::from_rgb(218, 221, 224), 0.35),
        // Unlit plastic stays a little lighter than unlit enamel, so ink
        // keeps its contrast down to the molded foot.
        theme::Material::Plastic if !on => color.lerp_to_gamma(Color32::WHITE, 0.12),
        _ => color,
    }
}

fn enamel_finish(red: u8, green: u8, blue: u8, on: bool) -> Color32 {
    if on {
        return Color32::from_rgb(red, green, blue);
    }
    let gray = 0.2126 * f32::from(red) + 0.7152 * f32::from(green) + 0.0722 * f32::from(blue);
    let toward = |channel: u8| {
        ((f32::from(channel) + (gray - f32::from(channel)) * 0.7) * 0.83).round() as u8
    };
    Color32::from_rgb(toward(red), toward(green), toward(blue))
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

pub(crate) fn enclosure_of(ui: &egui::Ui, rect: Rect, enamel: Color32, on: bool, seed: u64) {
    let painter = ui.painter();
    match roles().material {
        // A software module: a flat face and a hairline edge.
        theme::Material::Flat => {
            painter.rect_filled(rect, 4.0, enamel);
            painter.rect_stroke(
                rect,
                4.0,
                Stroke::new(1.5_f32, roles().ink),
                egui::StrokeKind::Inside,
            );
            return;
        }
        // A molded plastic case on a soft shadow.
        theme::Material::Plastic => {
            painter.add(
                egui::epaint::Shadow {
                    offset: [0, 4],
                    blur: 10,
                    spread: 0,
                    color: Color32::from_black_alpha(110),
                }
                .as_shape(rect, egui::CornerRadius::same(14)),
            );
            theme::plastic(painter, rect, 14.0, enamel);
            return;
        }
        theme::Material::Metal => {}
    }
    let radius = egui::CornerRadius::same(CORNER as u8);
    let body = Rect::from_min_max(rect.min, rect.max + Vec2::splat(DEPTH));
    // A wide soft shadow, and a close dark one where the body meets the board.
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
    // The outline of the face swept down and right: the walls facing away
    // from the light show where the face does not cover them.
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
        lit(enamel, -0.62),
        Stroke::NONE,
    ));
    let face = bevel(rect, enamel);
    if on {
        let finish = finish(ui.ctx());
        // The grain keeps one texel to a screen pixel, from a different
        // part of the tile on each pedal.
        let pixels = ui.ctx().pixels_per_point();
        let start = Vec2::new((seed * 97 % 512) as f32, (seed * 61 % 256) as f32);
        let grain = overlay(&face, finish.grain.id(), |point| {
            let texel = (point - rect.min) * pixels + start;
            Pos2::new(texel.x / GRAIN[0] as f32, texel.y / GRAIN[1] as f32)
        });
        let sheen = overlay(&face, finish.sheen.id(), |point| {
            let lean = SHEEN_LEAN * (rect.bottom() - point.y);
            let across =
                (point.x - rect.left() + lean) / (rect.width() + SHEEN_LEAN * rect.height());
            Pos2::new(across, 0.5)
        });
        painter.add(face);
        painter.add(grain);
        painter.add(sheen);
        // Scratches keep clear of the printed name.
        let below = rect.shrink(BEVEL + 3.0);
        let below =
            Rect::from_x_y_ranges(below.x_range(), rect.top() + HEADER + 4.0..=below.bottom());
        scratches(painter, below, seed);
    } else {
        painter.add(face);
    }
    painter.add(egui::Shape::closed_line(
        outline,
        Stroke::new(1.5_f32, roles().ink),
    ));
}

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
