//! The Vintage Mixer's materials, painted in code: the ivory strips, the
//! graphite floor and returns, and the dark ridged fader cap. Studio and Aero
//! paint their strips with the theme's surfaces.
use super::*;

/// The two strip materials of Vintage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stock {
    Ivory,
    Graphite,
}

/// How far a material's fine grain strays from its mean, in 8-bit levels,
/// and how far each channel strays on its own: a faint warm and cool speckle.
const GRAIN_DEPTH: f32 = 0.95;
const TINT: f32 = 0.3;

impl Stock {
    const ALL: [Self; 2] = [Self::Ivory, Self::Graphite];

    /// The material's mean color in 8-bit levels, and the seed of its noise.
    fn recipe(self) -> ([f32; 3], u64) {
        match self {
            // Warm beige enamel, a little darker than the strip role, as the
            // prepared tiles were, so the strip text keeps its contrast.
            Self::Ivory => ([195.8, 187.1, 168.7], 0x69_766f_7279),
            // Near-black graphite, a step lighter than the floor role, so
            // the strip edges and the slots read on it.
            Self::Graphite => ([46.3, 48.0, 48.6], 0x6772_6170_6869_7465),
        }
    }
}

/// The size of a material tile in texels. One texel covers one physical
/// pixel, so the 108-pixel strips show no repeat in their grain.
pub(super) const TILE: [usize; 2] = [256, 256];

/// One tile of a material. It repeats without a seam: the grain is softened
/// across the tile's edges, and the speckle is white noise.
pub(super) fn tile_image(stock: Stock) -> egui::ColorImage {
    let (mean, seed) = stock.recipe();
    let grain = grain::soft_field(TILE, seed);
    let tints: [Vec<f32>; 3] = std::array::from_fn(|channel| {
        let seed = seed ^ ((channel as u64 + 1) << 48);
        grain::field(TILE, seed, |random, _| random.normal())
    });
    let pixels = grain
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let [r, g, b] = std::array::from_fn(|channel| {
                let level = mean[channel] + GRAIN_DEPTH * value + TINT * tints[channel][index];
                level.round().clamp(0.0, 255.0) as u8
            });
            Color32::from_rgb(r, g, b)
        })
        .collect();
    egui::ColorImage::new(TILE, pixels)
}

/// The material tiles, made once for each context.
fn textures(ctx: &egui::Context) -> Arc<[egui::TextureHandle; 2]> {
    let id = egui::Id::new("mixer-material");
    if let Some(textures) = ctx.data(|data| data.get_temp::<Arc<[egui::TextureHandle; 2]>>(id)) {
        return textures;
    }
    let textures = Arc::new(Stock::ALL.map(|stock| {
        ctx.load_texture(
            format!("mixer-{stock:?}"),
            tile_image(stock),
            egui::TextureOptions::NEAREST_REPEAT,
        )
    }));
    ctx.data_mut(|data| data.insert_temp(id, textures.clone()));
    textures
}

/// Whether the theme paints the Mixer in its materials.
pub(super) fn vintage() -> bool {
    roles().material == theme::Material::Metal
}

/// Covers `rect` with a material. The grain starts at the physical pixel
/// nearest the rect's top left, so it moves with a strip as the bank scrolls,
/// and one texel falls on each physical pixel.
pub(super) fn tile(painter: &egui::Painter, rect: Rect, stock: Stock) {
    if !rect.is_finite() || !painter.clip_rect().intersect(rect).is_positive() {
        return;
    }
    let ppp = painter.ctx().pixels_per_point();
    let origin = (rect.min.to_vec2() * ppp).round() / ppp;
    let texels = Vec2::new(TILE[0] as f32, TILE[1] as f32);
    let uv = |pos: Pos2| ((pos.to_vec2() - origin) * ppp / texels).to_pos2();
    let mut mesh = egui::Mesh::with_texture(textures(painter.ctx())[stock as usize].id());
    mesh.add_rect_with_uv(
        rect,
        Rect::from_min_max(uv(rect.min), uv(rect.max)),
        Color32::WHITE,
    );
    painter.add(mesh);
}

/// Paints the Mixer's floor: graphite in Vintage, the floor color elsewhere.
pub(super) fn floor(painter: &egui::Painter, rect: Rect) {
    if vintage() {
        tile(painter, rect, Stock::Graphite);
    } else {
        painter.rect_filled(rect, 0.0, roles().mixer_floor);
    }
}

/// Paints a strip's body: ivory, or graphite for a return, in Vintage, and
/// the theme's strip surface elsewhere.
pub(super) fn strip_body(painter: &egui::Painter, rect: Rect, is_return: bool) {
    if vintage() {
        tile(
            painter,
            rect,
            if is_return {
                Stock::Graphite
            } else {
                Stock::Ivory
            },
        );
    } else {
        theme::surface(
            painter,
            rect,
            roles().corner(3.0),
            if is_return {
                roles().return_strip
            } else {
                roles().strip
            },
        );
    }
}

/// Paints an Inspector clip card in the strips' material: the ivory, or for a
/// return the graphite, in Vintage, and the theme's strip colors elsewhere.
pub(crate) fn row_surface(ui: &egui::Ui, rect: Rect, is_return: bool) {
    let painter = ui.painter();
    strip_body(painter, rect, is_return);
    painter.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0_f32, roles().strip_edge),
        egui::StrokeKind::Inside,
    );
}

/// The Vintage fader cap's size in physical pixels.
pub(super) const CAP: [u8; 2] = [24, 32];
/// The top row of the cap's ivory level stripe, which is three rows tall.
const STRIPE: usize = 14;
/// The top rows of the machined ridges: three above the stripe, three below.
const RIDGES: [usize; 6] = [6, 9, 12, 20, 23, 26];
/// The rows of the face with a shade of their own, from the top: a bevel
/// along the top edge toward the light, the shadow over the stripe, the
/// stripe with its bright upper half, the lip under it, and the bottom edge
/// as it turns away into shadow.
const SHADED_ROWS: [(usize, [u8; 3]); 9] = [
    (1, [0x72; 3]),
    (2, [0x4d; 3]),
    (STRIPE - 1, [13, 13, 15]),
    (STRIPE, [140, 140, 125]),
    (STRIPE + 1, [255, 255, 228]),
    (STRIPE + 2, [235, 226, 200]),
    (STRIPE + 3, [66, 65, 62]),
    (29, [0x1c; 3]),
    (30, [0x12; 3]),
];

/// Paints a Mixer fader cap centred on `center`: Vintage's machined cap,
/// snapped to physical pixels, or the theme's cap, flat on Studio and
/// molded on Aero. Both stay inside Vintage's 24 by 32 pixels.
pub(super) fn gain_cap(painter: &egui::Painter, center: Pos2, scale: f32) {
    if vintage() {
        let size = Vec2::new(f32::from(CAP[0]), f32::from(CAP[1])) * scale;
        let min = ((center - size / 2.0).to_vec2() / scale).round() * scale;
        painter.add(fader_cap_mesh(min.to_pos2(), scale));
    } else {
        let height = if roles().material == theme::Material::Flat {
            16.0
        } else {
            24.0
        };
        theme::fader_cap(painter, center, Vec2::new(24.0, height) * scale);
    }
}

/// The cap as one mesh, from its top left `min`, one row of the face for
/// each physical pixel of `pixel` points: a dark machined block lit from the
/// upper left, ridged above and below a bright ivory stripe, in a dark rim
/// with softened corners.
pub(super) fn fader_cap_mesh(min: Pos2, pixel: f32) -> egui::Mesh {
    let [width, height] = CAP.map(usize::from);
    let at = |x: usize, y: usize, size: [usize; 2]| {
        Rect::from_min_size(
            min + Vec2::new(x as f32, y as f32) * pixel,
            Vec2::new(size[0] as f32, size[1] as f32) * pixel,
        )
    };
    let mut face = egui::Mesh::default();
    let rim = Color32::from_gray(0x20);
    face.add_colored_rect(at(0, 1, [width, height - 2]), rim);
    for y in [0, height - 1] {
        face.add_colored_rect(at(1, y, [width - 2, 1]), rim);
        for x in [0, width - 1] {
            face.add_colored_rect(at(x, y, [1, 1]), rim.gamma_multiply(0.3));
        }
    }
    for y in 1..height - 1 {
        let left = cap_row(y);
        // Light falls from the left: the face darkens toward its right edge,
        // the ivory far less than the dark metal.
        let fall = if (STRIPE..STRIPE + 3).contains(&y) {
            0.97
        } else {
            0.72
        };
        gradient_quad(
            &mut face,
            at(1, y, [width - 2, 1]),
            left,
            scaled(left, fall),
        );
    }
    // The rounded left edge catches the light; the right edge turns away.
    let edges = [
        (1, Color32::from_white_alpha(40)),
        (2, Color32::from_white_alpha(14)),
        (width - 2, Color32::from_black_alpha(90)),
    ];
    for (x, color) in edges {
        face.add_colored_rect(at(x, 1, [1, height - 2]), color);
    }
    face
}

/// The color of one row of the cap's face at its left edge, from the top.
fn cap_row(y: usize) -> Color32 {
    if let Some(&(_, [r, g, b])) = SHADED_ROWS.iter().find(|(row, _)| *row == y) {
        return Color32::from_rgb(r, g, b);
    }
    let body = 0x26 - (y / 4) as u8;
    let level = if RIDGES.contains(&y) {
        body + 0x24
    } else if RIDGES.contains(&(y - 1)) {
        // A shadow under each ridge, deeper above the stripe, where the
        // ridges face away from the light.
        (f32::from(body) * if y < STRIPE { 0.6 } else { 0.9 }).round() as u8
    } else {
        body
    };
    Color32::from_gray(level)
}

fn scaled(color: Color32, amount: f32) -> Color32 {
    let level = |channel: u8| (f32::from(channel) * amount).round() as u8;
    Color32::from_rgb(level(color.r()), level(color.g()), level(color.b()))
}

/// A rectangle that shades from `left` to `right` across its width.
fn gradient_quad(mesh: &mut egui::Mesh, rect: Rect, left: Color32, right: Color32) {
    let start = mesh.vertices.len() as u32;
    for (pos, color) in [
        (rect.left_top(), left),
        (rect.right_top(), right),
        (rect.right_bottom(), right),
        (rect.left_bottom(), left),
    ] {
        mesh.colored_vertex(pos, color);
    }
    mesh.add_triangle(start, start + 1, start + 2);
    mesh.add_triangle(start, start + 2, start + 3);
}

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;
