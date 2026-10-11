//! Transparent PNG artwork. Texture handles live for one native egui context.
use eframe::egui::{self, TextureHandle, TextureId};

// Pick a prepared density variant and paint one texel per physical pixel.
const ICON_PIXELS: [usize; 3] = [20, 30, 40];

macro_rules! rail_pngs {
    ($name:literal) => {
        [
            include_bytes!(concat!("../assets/rail/20/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../assets/rail/30/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../assets/rail/40/", $name, ".png")).as_slice(),
        ]
    };
}

const BRAND_PNGS: [&[u8]; 3] = [
    include_bytes!("../assets/app/40/mixjam-techno.png"),
    include_bytes!("../assets/app/60/mixjam-techno.png"),
    include_bytes!("../assets/app/80/mixjam-techno.png"),
];

fn density_index(pixels_per_point: f32) -> usize {
    let desired = 20.0 * pixels_per_point;
    ICON_PIXELS
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            ((**a as f32 - desired).abs()).total_cmp(&(**b as f32 - desired).abs())
        })
        .map_or(0, |(index, _)| index)
}

fn pixel_rect(center: egui::Pos2, pixels: usize, pixels_per_point: f32) -> egui::Rect {
    let left = ((center.x * pixels_per_point - pixels as f32 / 2.0).round()) / pixels_per_point;
    let top = ((center.y * pixels_per_point - pixels as f32 / 2.0).round()) / pixels_per_point;
    egui::Rect::from_min_size(
        egui::pos2(left, top),
        egui::Vec2::splat(pixels as f32 / pixels_per_point),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(crate) enum RailIcon {
    Open,
    Save,
    SaveAs,
    Export,
    Import,
    Libraries,
    New,
    Mixer,
    Erase,
    Delete,
    Effects,
    StepSynth,
    Edit,
    Stretch,
    Record,
    Mix,
    Undo,
    Redo,
}

impl RailIcon {
    pub(crate) const ALL: [Self; 18] = [
        Self::Open,
        Self::Save,
        Self::SaveAs,
        Self::Export,
        Self::Import,
        Self::Libraries,
        Self::New,
        Self::Mixer,
        Self::Erase,
        Self::Delete,
        Self::Effects,
        Self::StepSynth,
        Self::Edit,
        Self::Stretch,
        Self::Record,
        Self::Mix,
        Self::Undo,
        Self::Redo,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Save => "Save",
            Self::SaveAs => "Save as",
            Self::Export => "Export",
            Self::Import => "Import",
            Self::Libraries => "Libraries",
            Self::New => "New",
            Self::Mixer => "Mixer",
            Self::Erase => "Erase",
            Self::Delete => "Delete",
            Self::Effects => "Effects",
            Self::StepSynth => "Step synth",
            Self::Edit => "Edit",
            Self::Stretch => "Stretch",
            Self::Record => "Recording destination",
            Self::Mix => "Mix tools",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
        }
    }

    pub(crate) fn is_toggle(self) -> bool {
        matches!(self, Self::Mixer | Self::Erase)
    }

    fn bytes(self, density: usize) -> &'static [u8] {
        match self {
            Self::Open => rail_pngs!("open")[density],
            Self::Save => rail_pngs!("save")[density],
            Self::SaveAs => rail_pngs!("save-as")[density],
            Self::Export => rail_pngs!("export")[density],
            Self::Import => rail_pngs!("import")[density],
            Self::Libraries => rail_pngs!("libraries")[density],
            Self::New => rail_pngs!("new")[density],
            Self::Mixer => rail_pngs!("mixer")[density],
            Self::Erase => rail_pngs!("erase")[density],
            Self::Delete => rail_pngs!("delete")[density],
            Self::Effects => rail_pngs!("effects")[density],
            Self::StepSynth => rail_pngs!("step-synth")[density],
            Self::Edit => rail_pngs!("edit")[density],
            Self::Stretch => rail_pngs!("stretch")[density],
            Self::Record => rail_pngs!("record")[density],
            Self::Mix => rail_pngs!("mix")[density],
            Self::Undo => rail_pngs!("undo")[density],
            Self::Redo => rail_pngs!("redo")[density],
        }
    }
}

pub(crate) struct RailIcons {
    textures: Vec<TextureHandle>,
    brand: TextureHandle,
    density: usize,
}

impl RailIcons {
    pub(crate) fn load(ctx: &egui::Context) -> Result<Self, String> {
        let density = density_index(ctx.pixels_per_point());
        let textures = RailIcon::ALL
            .into_iter()
            .map(|icon| {
                let mut image = image::load_from_memory_with_format(
                    icon.bytes(density),
                    image::ImageFormat::Png,
                )
                .map_err(|error| format!("Cannot load the bundled {} icon: {error}", icon.label()))?
                .into_rgba8();
                // Use the preserved artwork as an alpha mask. The view owns its color.
                for pixel in image.pixels_mut() {
                    pixel.0[..3].fill(255);
                }
                let size = [image.width() as usize, image.height() as usize];
                Ok(ctx.load_texture(
                    format!("rail-{}", icon.label()),
                    egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
                    egui::TextureOptions::NEAREST,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let brand =
            image::load_from_memory_with_format(BRAND_PNGS[density], image::ImageFormat::Png)
                .map_err(|error| format!("Cannot load the bundled tribal mask: {error}"))?
                .into_rgba8();
        let brand = ctx.load_texture(
            "tribal-mask",
            egui::ColorImage::from_rgba_unmultiplied(
                [brand.width() as usize, brand.height() as usize],
                brand.as_raw(),
            ),
            egui::TextureOptions::NEAREST,
        );
        Ok(Self {
            textures,
            brand,
            density,
        })
    }

    pub(crate) fn texture_id(&self, icon: RailIcon) -> TextureId {
        self.textures[icon as usize].id()
    }

    pub(crate) fn matches_density(&self, ctx: &egui::Context) -> bool {
        self.density == density_index(ctx.pixels_per_point())
    }

    pub(crate) fn icon_rect(&self, ctx: &egui::Context, center: egui::Pos2) -> egui::Rect {
        pixel_rect(center, ICON_PIXELS[self.density], ctx.pixels_per_point())
    }

    pub(crate) fn brand_image(&self, ui: &mut egui::Ui) -> egui::Response {
        let pixels = ICON_PIXELS[self.density] * 2;
        let (rect, response) = ui.allocate_exact_size(
            egui::Vec2::splat(pixels as f32 / ui.ctx().pixels_per_point()),
            egui::Sense::hover(),
        );
        ui.painter().image(
            self.brand.id(),
            pixel_rect(rect.center(), pixels, ui.ctx().pixels_per_point()),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Image,
                ui.is_enabled(),
                "MixJam Techno tribal mask",
            )
        });
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_rail_icon_has_visible_artwork_and_a_transparent_background() {
        for (density, pixels) in ICON_PIXELS.into_iter().enumerate() {
            for icon in RailIcon::ALL {
                assert_eq!(
                    image::guess_format(icon.bytes(density)).unwrap(),
                    image::ImageFormat::Png
                );
                let image = image::load_from_memory_with_format(
                    icon.bytes(density),
                    image::ImageFormat::Png,
                )
                .unwrap()
                .into_rgba8();
                assert_eq!(image.dimensions(), (pixels as u32, pixels as u32));
                assert!(
                    image.pixels().any(|pixel| pixel[3] == 0),
                    "{} needs transparency",
                    icon.label()
                );
                assert!(
                    image.pixels().any(|pixel| pixel[3] > 0),
                    "{} has no artwork",
                    icon.label()
                );
                assert!(
                    image.pixels().any(|pixel| (1..255).contains(&pixel[3])),
                    "{} needs antialiased alpha edges",
                    icon.label()
                );
            }
            let brand =
                image::load_from_memory_with_format(BRAND_PNGS[density], image::ImageFormat::Png)
                    .unwrap()
                    .into_rgba8();
            assert_eq!(brand.dimensions(), (pixels as u32 * 2, pixels as u32 * 2));
            assert!(brand.pixels().any(|pixel| pixel[3] == 0));
        }
    }

    #[test]
    fn raster_rects_match_texels_and_snap_to_physical_pixels_at_each_density() {
        for scale in [
            0.75, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.0, 4.0, 4.5,
        ] {
            let density = density_index(scale);
            for pixels in [ICON_PIXELS[density], ICON_PIXELS[density] * 2] {
                let rect = pixel_rect(egui::pos2(18.3, 45.7), pixels, scale);
                assert!((rect.width() * scale - pixels as f32).abs() < 0.001);
                assert!((rect.height() * scale - pixels as f32).abs() < 0.001);
                for edge in [rect.left(), rect.top(), rect.right(), rect.bottom()] {
                    assert!((edge * scale - (edge * scale).round()).abs() < 0.001);
                }
            }
        }
    }
}
