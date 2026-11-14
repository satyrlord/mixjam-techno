//! Shared tool controls. Tools own values and actions; this module owns their
//! appearance in the current theme, pointer and keyboard interaction, and
//! accessibility. Interaction never depends on the theme.
use super::HoverHints;
use super::theme::{self, Material, roles};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Vec2};
use std::{ops::RangeInclusive, sync::Arc};
#[macro_use]
#[path = "controls/textures.rs"]
mod textures;
#[path = "controls/buttons.rs"]
mod buttons;
#[path = "controls/knobs.rs"]
mod knobs;
#[path = "controls/readouts.rs"]
mod readouts;
#[path = "controls/sliders.rs"]
mod sliders;
#[path = "controls/surfaces.rs"]
mod surfaces;
pub(super) use buttons::{
    button, button_at, focus_color, header_button, icon_button_surface, print_color, rail_surface,
    toggle_at,
};
pub(super) use knobs::{RotarySpec, rotary, wheel};
pub(super) use readouts::{disclosure, number, progress, radio, status_text, value_style};
pub(super) use sliders::{slider, stepped_slider};
pub(super) use surfaces::{inset, panel, rail_panel, rail_separator, section, well};
use textures::*;

/// Record the focused dial or fader. These controls use only arrows, Home,
/// and End, so the workspace keeps its transport key while one has focus.
pub(super) fn keep_transport_key(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("arrow-key-focus"), id));
}

pub(super) fn arrow_key_focus(ctx: &egui::Context) -> Option<egui::Id> {
    ctx.data(|data| data.get_temp(egui::Id::new("arrow-key-focus")))
}

pub(super) fn contrast(a: Color32, b: Color32) -> f32 {
    let luminance = |color: Color32| {
        let linear = |value: u8| {
            let value = f32::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Palette {
    Utility,
    Marine,
    Granite,
    Red,
    Olive,
    Indigo,
}

impl Palette {
    /// The palette's chassis body in the current theme.
    pub(super) fn body(self) -> Color32 {
        roles().tools[self as usize].0
    }
    pub(super) fn accent(self) -> Color32 {
        roles().tools[self as usize].1
    }
    /// The keyboard focus ring. A ring crosses the chassis, so an accent
    /// below 3:1 against it gives way to cream.
    fn focus(self) -> Color32 {
        if contrast(self.accent(), self.body()) >= 3.0 {
            self.accent()
        } else {
            roles().legend
        }
    }
    fn of(ui: &egui::Ui) -> Self {
        [
            Self::Marine,
            Self::Granite,
            Self::Red,
            Self::Olive,
            Self::Indigo,
        ]
        .into_iter()
        .find(|palette| palette.body() == ui.visuals().panel_fill)
        .unwrap_or(Self::Utility)
    }
}

pub(super) fn apply_palette(ui: &mut egui::Ui, palette: Palette) {
    let visuals = ui.visuals_mut();
    visuals.panel_fill = palette.body();
    visuals.selection.bg_fill = palette.accent();
    visuals.selection.stroke = Stroke::new(1.0_f32, roles().ink);
    visuals.override_text_color = None;
}

#[derive(Clone, Copy)]
pub(super) enum ButtonRole {
    Secondary,
    Primary,
    Danger,
    /// A rare, reversible action: underlined text that gains a face on hover.
    Tertiary,
}

#[derive(Clone, Copy)]
pub(super) enum HeaderIcon {
    Collapse,
    Expand,
    Help,
    Close,
}

#[cfg(test)]
#[path = "controls/tests.rs"]
mod tests;
