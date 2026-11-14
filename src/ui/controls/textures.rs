//! The bundled Folded Metal artwork: its decoded textures per palette, and
//! the helpers that paint a texture or its grain.
use super::*;

macro_rules! control {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../assets/folded-metal/controls/100/",
            $name,
            ".png"
        ))
        .as_slice()
    };
}

macro_rules! material {
    ($palette:literal, $name:literal) => {
        include_bytes!(concat!(
            "../../../assets/folded-metal/",
            $palette,
            "/100/",
            $name,
            ".png"
        ))
        .as_slice()
    };
}

macro_rules! parts {
    ($palette:literal) => {
        [
            material!($palette, "corner-tl"),
            material!($palette, "corner-tr"),
            material!($palette, "corner-bl"),
            material!($palette, "corner-br"),
            material!($palette, "edge-top"),
            material!($palette, "edge-bottom"),
            material!($palette, "edge-left"),
            material!($palette, "edge-right"),
        ]
    };
}

pub(super) struct Artwork {
    pub(super) material: Vec<egui::TextureHandle>,
    pub(super) grain: egui::TextureHandle,
    pub(super) knob: egui::TextureHandle,
    pub(super) primary: egui::TextureHandle,
}

pub(super) fn artwork(ctx: &egui::Context, palette: Palette) -> Arc<Artwork> {
    let id = egui::Id::new(("folded-metal-artwork", palette));
    if let Some(art) = ctx.data(|data| data.get_temp::<Arc<Artwork>>(id)) {
        return art;
    }
    let load = |name: &str, bytes: &[u8]| {
        let image = image::load_from_memory(bytes)
            .expect("Bundled Folded Metal PNG must decode")
            .into_rgba8();
        ctx.load_texture(
            format!("folded-metal-{palette:?}-{name}"),
            egui::ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                image.as_raw(),
            ),
            egui::TextureOptions::NEAREST,
        )
    };
    let parts = match palette {
        Palette::Utility => parts!("utility"),
        Palette::Marine => parts!("marine"),
        Palette::Granite => parts!("granite"),
        Palette::Red => parts!("red"),
        Palette::Olive => parts!("olive"),
        Palette::Indigo => parts!("indigo"),
    };
    let grain = match palette {
        Palette::Utility => material!("utility", "paint-grain"),
        Palette::Marine => material!("marine", "paint-grain"),
        Palette::Granite => material!("granite", "paint-grain"),
        Palette::Red => material!("red", "paint-grain"),
        Palette::Olive => material!("olive", "paint-grain"),
        Palette::Indigo => material!("indigo", "paint-grain"),
    };
    let art = Arc::new(Artwork {
        material: parts
            .iter()
            .enumerate()
            .map(|(part, bytes)| load(&format!("part-{part}"), bytes))
            .collect(),
        grain: load("grain", grain),
        knob: load("knob", control!("knob")),
        primary: load("primary", control!("primary")),
    });
    ctx.data_mut(|data| data.insert_temp(id, art.clone()));
    art
}

pub(super) fn fixed_image(ui: &egui::Ui, texture: &egui::TextureHandle, min: Pos2, clip: Rect) {
    let ppp = ui.ctx().pixels_per_point();
    let min = (min.to_vec2() * ppp).round().to_pos2() / ppp;
    ui.painter()
        .with_clip_rect(clip.intersect(ui.clip_rect()))
        .image(
            texture.id(),
            Rect::from_min_size(min, texture.size_vec2() / ppp),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
}

/// Repeat a static native-resolution grain field. Only vertex opacity changes
/// near the perimeter; UV coordinates always preserve one physical texel.
pub(super) fn paint_grain(ui: &egui::Ui, texture: &egui::TextureHandle, rect: Rect) {
    let ppp = ui.ctx().pixels_per_point();
    let rect = Rect::from_min_max(
        (rect.min.to_vec2() * ppp).round().to_pos2() / ppp,
        (rect.max.to_vec2() * ppp).round().to_pos2() / ppp,
    );
    let size = texture.size_vec2() / ppp;
    let origin = rect.min;
    let interior = rect.shrink(4.0);
    let cuts = |minimum: f32, maximum: f32, low: f32, high: f32| {
        let mut points = vec![minimum.max(low + 4.0), maximum.min(high - 4.0)];
        for distance in [4.0, 8.0, 12.0, 16.0] {
            for point in [low + distance, high - distance] {
                if point > points[0] && point < points[1] {
                    points.push(point);
                }
            }
        }
        points.sort_by(f32::total_cmp);
        points.dedup();
        points
    };
    let fade = |position: Pos2| {
        let enter = |distance: f32| {
            let amount = ((distance - 4.0) / 12.0).clamp(0.0, 1.0);
            amount * amount * (3.0 - 2.0 * amount)
        };
        let alpha = enter(position.x - rect.left())
            * enter(rect.right() - position.x)
            * enter(position.y - rect.top())
            * enter(rect.bottom() - position.y);
        Color32::from_white_alpha((alpha * 255.0).round() as u8)
    };
    let mut y = origin.y;
    while y < interior.bottom() {
        let mut x = origin.x;
        while x < interior.right() {
            let mut mesh = egui::Mesh::with_texture(texture.id());
            let xs = cuts(x, x + size.x, rect.left(), rect.right());
            let ys = cuts(y, y + size.y, rect.top(), rect.bottom());
            for pair_y in ys.windows(2) {
                for pair_x in xs.windows(2) {
                    let start = mesh.vertices.len() as u32;
                    for position in [
                        Pos2::new(pair_x[0], pair_y[0]),
                        Pos2::new(pair_x[1], pair_y[0]),
                        Pos2::new(pair_x[1], pair_y[1]),
                        Pos2::new(pair_x[0], pair_y[1]),
                    ] {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos: position,
                            uv: Pos2::new((position.x - x) / size.x, (position.y - y) / size.y),
                            color: fade(position),
                        });
                    }
                    mesh.indices.extend_from_slice(&[
                        start,
                        start + 1,
                        start + 2,
                        start,
                        start + 2,
                        start + 3,
                    ]);
                }
            }
            ui.painter()
                .with_clip_rect(interior.intersect(ui.clip_rect()))
                .add(mesh);
            x += size.x;
        }
        y += size.y;
    }
}
