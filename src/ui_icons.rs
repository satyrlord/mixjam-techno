//! Transparent PNG artwork. Texture handles live for one native egui context.
use eframe::egui::{self, TextureHandle, TextureId};

// The artwork has one prepared size. Paint one texel per physical pixel.
const ICON_PIXELS: usize = 20;

macro_rules! rail_png {
    ($name:literal) => {
        include_bytes!(concat!("../assets/rail/20/", $name, ".png")).as_slice()
    };
}

const BRAND_PNG: &[u8] = include_bytes!("../assets/app/40/mixjam-techno.png");

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
    Options,
    New,
    Mixer,
    Master,
    Erase,
    Delete,
    Effects,
    StepSynth,
    Edit,
    Stretch,
    Record,
    Undo,
    Redo,
    Help,
    AlbumReview,
    FxReturns,
}

impl RailIcon {
    pub(crate) const ALL: [Self; 21] = [
        Self::Open,
        Self::Save,
        Self::SaveAs,
        Self::Export,
        Self::Import,
        Self::Options,
        Self::New,
        Self::Mixer,
        Self::Master,
        Self::Erase,
        Self::Delete,
        Self::Effects,
        Self::StepSynth,
        Self::Edit,
        Self::Stretch,
        Self::Record,
        Self::Undo,
        Self::Redo,
        Self::Help,
        Self::AlbumReview,
        Self::FxReturns,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Save => "Save",
            Self::SaveAs => "Save as",
            Self::Export => "Export",
            Self::Import => "Import",
            Self::Options => "Options",
            Self::New => "New",
            Self::Mixer => "Mixer",
            Self::Master => "Mastering",
            Self::Erase => "Erase",
            Self::Delete => "Delete",
            Self::Effects => "Effects",
            Self::StepSynth => "Step synth",
            Self::Edit => "Edit",
            Self::Stretch => "Stretch",
            Self::Record => "Recording destination",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Help => "Help",
            Self::AlbumReview => "Album Review",
            Self::FxReturns => "FX returns",
        }
    }

    /// The name printed under the icon in the tool rail. Every name fits the
    /// rail at one size; a long one is shortened here, never shrunk. The
    /// accessible name and the tooltip keep the full `label`.
    pub(crate) fn rail_name(self) -> &'static str {
        match self {
            Self::AlbumReview => "Review",
            other => other.label(),
        }
    }

    /// The tooltip: what the action does, with its window name or shortcut.
    /// `label` stays the short accessible name.
    pub(crate) fn hint(self) -> &'static str {
        match self {
            Self::Open => "Open a song (Ctrl+O)",
            Self::Save => "Save the song (Ctrl+S)",
            Self::SaveAs => "Save as a new song file (Ctrl+Shift+S)",
            Self::Export => "Export the song as a stereo WAV",
            Self::Import => "Import a WAV sound",
            Self::Options => "Options: open or focus preferences (Ctrl+,)",
            Self::New => "New song (Ctrl+N)",
            Self::Mixer => "Mixer: switch the score between its Arrange and Mix views",
            Self::Master => "Mastering: show the master pedalboard, or fold the lower panel",
            Self::Erase => "Erase: click clips to delete them (Esc ends)",
            Self::Delete => "Delete the selected clips (Delete)",
            Self::Effects => "Effects: show or close Effect Studio",
            Self::StepSynth => "Step synth: show Step synth, or fold the lower panel",
            Self::Edit => "Edit: show or close Sample Editor",
            Self::Stretch => "Stretch: show or close Time Stretcher",
            Self::Undo => "Undo (Ctrl+Z)",
            Self::Redo => "Redo (Ctrl+Shift+Z or Ctrl+Y)",
            Self::Help => "Help (F1)",
            Self::AlbumReview => "Album Review: show or close the album analyses",
            Self::FxReturns => "FX returns: show the four FX buses, or fold the lower panel",
            other => other.label(),
        }
    }

    pub(crate) fn is_toggle(self) -> bool {
        matches!(
            self,
            Self::Options
                | Self::Mixer
                | Self::Master
                | Self::Erase
                | Self::Effects
                | Self::StepSynth
                | Self::Edit
                | Self::Stretch
                | Self::AlbumReview
                | Self::FxReturns
        )
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Open => rail_png!("open"),
            Self::Save => rail_png!("save"),
            Self::SaveAs => rail_png!("save-as"),
            Self::Export => rail_png!("export"),
            Self::Import => rail_png!("import"),
            Self::Options => rail_png!("options"),
            Self::New => rail_png!("new"),
            Self::Mixer => rail_png!("mixer"),
            Self::Master => rail_png!("master"),
            Self::Erase => rail_png!("erase"),
            Self::Delete => rail_png!("delete"),
            Self::Effects => rail_png!("effects"),
            Self::StepSynth => rail_png!("step-synth"),
            Self::Edit => rail_png!("edit"),
            Self::Stretch => rail_png!("stretch"),
            Self::Record => rail_png!("record"),
            Self::Undo => rail_png!("undo"),
            Self::Redo => rail_png!("redo"),
            Self::Help => rail_png!("help"),
            Self::AlbumReview => rail_png!("album-review"),
            Self::FxReturns => rail_png!("fx-returns"),
        }
    }
}

pub(crate) struct RailIcons {
    textures: Vec<TextureHandle>,
    brand: TextureHandle,
}

impl RailIcons {
    pub(crate) fn load(ctx: &egui::Context) -> Result<Self, String> {
        let textures = RailIcon::ALL
            .into_iter()
            .map(|icon| {
                let mut image =
                    image::load_from_memory_with_format(icon.bytes(), image::ImageFormat::Png)
                        .map_err(|error| {
                            format!("Cannot load the bundled {} icon: {error}", icon.label())
                        })?
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
        let brand = image::load_from_memory_with_format(BRAND_PNG, image::ImageFormat::Png)
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
        Ok(Self { textures, brand })
    }

    pub(crate) fn texture_id(&self, icon: RailIcon) -> TextureId {
        self.textures[icon as usize].id()
    }

    pub(crate) fn icon_rect(&self, ctx: &egui::Context, center: egui::Pos2) -> egui::Rect {
        pixel_rect(center, ICON_PIXELS, ctx.pixels_per_point())
    }

    pub(crate) fn brand_image(&self, ui: &mut egui::Ui) -> egui::Response {
        let pixels = ICON_PIXELS * 2;
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
        let pixels = ICON_PIXELS as u32;
        for icon in RailIcon::ALL {
            assert_eq!(
                image::guess_format(icon.bytes()).unwrap(),
                image::ImageFormat::Png
            );
            let image = image::load_from_memory_with_format(icon.bytes(), image::ImageFormat::Png)
                .unwrap()
                .into_rgba8();
            assert_eq!(image.dimensions(), (pixels, pixels));
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
        let brand = image::load_from_memory_with_format(BRAND_PNG, image::ImageFormat::Png)
            .unwrap()
            .into_rgba8();
        assert_eq!(brand.dimensions(), (pixels * 2, pixels * 2));
        assert!(brand.pixels().any(|pixel| pixel[3] == 0));
    }

    #[test]
    fn raster_rects_match_texels_and_snap_to_physical_pixels() {
        for pixels in [ICON_PIXELS, ICON_PIXELS * 2] {
            let rect = pixel_rect(egui::pos2(18.3, 45.7), pixels, 1.0);
            assert!((rect.width() - pixels as f32).abs() < 0.001);
            assert!((rect.height() - pixels as f32).abs() < 0.001);
            for edge in [rect.left(), rect.top(), rect.right(), rect.bottom()] {
                assert!((edge - edge.round()).abs() < 0.001);
            }
        }
    }
}
