//! The mixer's bitmap artwork, loaded per screen density, and the row surface
//! painted with it.
use super::*;

const UV: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));

macro_rules! pngs {
    ($name:literal) => {
        [
            include_bytes!(concat!("../../../assets/mixer/100/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../../assets/mixer/150/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../../assets/mixer/200/", $name, ".png")).as_slice(),
        ]
    };
}

pub(super) struct Artwork {
    pub(super) density: usize,
    textures: Vec<egui::TextureHandle>,
}

impl Artwork {
    pub(super) fn load(ctx: &egui::Context) -> Result<Arc<Self>, String> {
        let density = [1.0_f32, 1.5, 2.0]
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (**a - ctx.pixels_per_point())
                    .abs()
                    .total_cmp(&(**b - ctx.pixels_per_point()).abs())
            })
            .map_or(0, |(index, _)| index);
        let id = egui::Id::new(("mixer-artwork", density));
        if let Some(art) = ctx.data(|data| data.get_temp::<Arc<Self>>(id)) {
            return Ok(art);
        }
        let textures = [pngs!("ivory"), pngs!("graphite"), pngs!("fader")]
            .iter()
            .enumerate()
            .map(|(index, variants)| {
                let image = image::load_from_memory(variants[density])
                    .map_err(|error| format!("Cannot load mixer artwork: {error}"))?
                    .into_rgba8();
                Ok(ctx.load_texture(
                    format!("mixer-{density}-{index}"),
                    egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    ),
                    egui::TextureOptions::NEAREST,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let art = Arc::new(Self { density, textures });
        ctx.data_mut(|data| data.insert_temp(id, art.clone()));
        Ok(art)
    }

    pub(super) fn tile(&self, ui: &egui::Ui, index: usize, rect: Rect) {
        let ppp = ui.ctx().pixels_per_point();
        let size = self.textures[index].size_vec2() / ppp;
        let min = (rect.min.to_vec2() * ppp).round() / ppp;
        let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(rect));
        let mut y = min.y;
        while y < rect.bottom() {
            let mut x = min.x;
            while x < rect.right() {
                painter.image(
                    self.textures[index].id(),
                    Rect::from_min_size(Pos2::new(x, y), size),
                    UV,
                    Color32::WHITE,
                );
                x += size.x;
            }
            y += size.y;
        }
    }

    pub(super) fn fader(&self, ui: &egui::Ui, center: Pos2) {
        let ppp = ui.ctx().pixels_per_point();
        let size = self.textures[2].size_vec2() / ppp;
        let min = ((center - size / 2.0).to_vec2() * ppp).round() / ppp;
        ui.painter().image(
            self.textures[2].id(),
            Rect::from_min_size(min.to_pos2(), size),
            UV,
            Color32::WHITE,
        );
    }
}

/// Paints a Mix view row in the strips' material: the ivory, or for a return
/// the graphite, tiles in Vintage, and the theme's strip colors elsewhere.
pub(crate) fn row_surface(ui: &egui::Ui, rect: Rect, is_return: bool) {
    let art = (roles().material == theme::Material::Metal)
        .then(|| Artwork::load(ui.ctx()).ok())
        .flatten();
    if let Some(art) = art {
        art.tile(ui, usize::from(is_return), rect);
    } else {
        theme::surface(
            ui.painter(),
            rect,
            roles().corner(3.0),
            if is_return {
                roles().return_strip
            } else {
                roles().strip
            },
        );
    }
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0_f32, roles().strip_edge),
        egui::StrokeKind::Inside,
    );
}
