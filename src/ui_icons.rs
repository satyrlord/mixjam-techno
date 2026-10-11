//! Bundled action artwork. Texture handles live for one native egui context.
use eframe::egui::{self, TextureHandle, TextureId};

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

    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Open => include_bytes!("../assets/rail/open.png"),
            Self::Save => include_bytes!("../assets/rail/save.png"),
            Self::SaveAs => include_bytes!("../assets/rail/save-as.png"),
            Self::Export => include_bytes!("../assets/rail/export.png"),
            Self::Import => include_bytes!("../assets/rail/import.png"),
            Self::Libraries => include_bytes!("../assets/rail/libraries.png"),
            Self::New => include_bytes!("../assets/rail/new.png"),
            Self::Mixer => include_bytes!("../assets/rail/mixer.png"),
            Self::Erase => include_bytes!("../assets/rail/erase.png"),
            Self::Delete => include_bytes!("../assets/rail/delete.png"),
            Self::Effects => include_bytes!("../assets/rail/effects.png"),
            Self::StepSynth => include_bytes!("../assets/rail/step-synth.png"),
            Self::Edit => include_bytes!("../assets/rail/edit.png"),
            Self::Stretch => include_bytes!("../assets/rail/stretch.png"),
            Self::Record => include_bytes!("../assets/rail/record.png"),
            Self::Mix => include_bytes!("../assets/rail/mix.png"),
            Self::Undo => include_bytes!("../assets/rail/undo.png"),
            Self::Redo => include_bytes!("../assets/rail/redo.png"),
        }
    }
}

pub(crate) struct RailIcons {
    textures: Vec<TextureHandle>,
}

impl RailIcons {
    pub(crate) fn load(ctx: &egui::Context) -> Result<Self, String> {
        let textures = RailIcon::ALL
            .into_iter()
            .map(|icon| {
                let mut image = image::load_from_memory(icon.bytes())
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
                    egui::TextureOptions::LINEAR,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self { textures })
    }

    pub(crate) fn texture_id(&self, icon: RailIcon) -> TextureId {
        self.textures[icon as usize].id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_rail_icon_has_visible_artwork_and_a_transparent_background() {
        for icon in RailIcon::ALL {
            let image = image::load_from_memory(icon.bytes())
                .unwrap_or_else(|error| panic!("{}: {error}", icon.label()))
                .into_rgba8();
            assert_eq!(image.dimensions(), (96, 96), "{}", icon.label());
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
        }
    }
}
