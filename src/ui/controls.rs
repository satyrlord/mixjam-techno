//! Shared tool controls. Tools own values and actions; this module owns their
//! appearance in the current theme, pointer and keyboard interaction, and
//! accessibility. Interaction never depends on the theme.
use super::HoverHints;
use super::theme::{self, Material, roles};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Vec2};
use std::ops::RangeInclusive;
#[path = "controls/buttons.rs"]
mod buttons;
#[path = "controls/clip_fields.rs"]
mod clip_fields;
#[path = "controls/instrument.rs"]
pub(super) mod instrument;
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
pub(super) use clip_fields::{
    FadeEdge, FadeHandleSpec, NumberField, curve_field, curve_menu, fade_handle, gain_field,
    inline_gain_field, inline_number_field, number_field, snapped_fade_ticks,
};
pub(super) use knobs::{RotarySpec, rotary, wheel};
pub(super) use readouts::{disclosure, number, progress, radio, status_text, value_style};
pub(super) use sliders::{slider, stepped_slider};
pub(super) use surfaces::{
    inset, metal_rack_cabinet, panel, rail_panel, rail_separator, section, well,
};

/// Record the focused dial or fader. These controls use only arrows, Home,
/// and End, so the workspace keeps its transport key while one has focus.
pub(super) fn keep_transport_key(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("arrow-key-focus"), id));
}

pub(super) fn arrow_key_focus(ctx: &egui::Context) -> Option<egui::Id> {
    ctx.data(|data| data.get_temp(egui::Id::new("arrow-key-focus")))
}

/// Lock horizontal and vertical arrow keys to the focused slider or rotary control.
pub(super) fn lock_arrows(ui: &egui::Ui, id: egui::Id) {
    keep_transport_key(ui.ctx(), id);
    ui.memory_mut(|memory| {
        memory.set_focus_lock_filter(
            id,
            egui::EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                ..Default::default()
            },
        )
    });
}

const POINTER_BUTTONS: [egui::PointerButton; 5] = [
    egui::PointerButton::Primary,
    egui::PointerButton::Secondary,
    egui::PointerButton::Middle,
    egui::PointerButton::Extra1,
    egui::PointerButton::Extra2,
];

#[derive(Clone, Copy)]
struct DragPress {
    button: egui::PointerButton,
    origin: Pos2,
    frame: u64,
    started_at: Option<u64>,
    released: bool,
}

/// The original press on this widget. Egui's drag flags refer to the
/// buttons down in the current frame, and any release ends egui's drag.
fn drag_press(ui: &egui::Ui, response: &Response) -> Option<DragPress> {
    let id = response.id.with("drag-press");
    let (down, pressed, released, origin) = ui.input(|input| {
        let pointer = &input.pointer;
        (response.enabled()
            && !input.key_pressed(egui::Key::Escape)
            && (pointer.any_down() || pointer.any_released()))
        .then(|| {
            (
                POINTER_BUTTONS.map(|button| pointer.button_down(button)),
                POINTER_BUTTONS.map(|button| pointer.button_pressed(button)),
                POINTER_BUTTONS.map(|button| pointer.button_released(button)),
                pointer.press_origin(),
            )
        })
    })?;
    let frame = ui.ctx().cumulative_frame_nr();
    let pass = ui.ctx().cumulative_pass_nr();
    ui.data_mut(|data| {
        let previous = data.get_temp::<DragPress>(id);
        let mut press = previous
            .filter(|press| {
                let button = press.button as usize;
                frame.saturating_sub(press.frame) <= 1
                    && (!pressed[button] || press.frame == frame)
                    && (down[button] || released[button])
            })
            .or_else(|| {
                let mut buttons = POINTER_BUTTONS
                    .into_iter()
                    .filter(|&button| down[button as usize]);
                let button = buttons.next()?;
                if buttons.next().is_none()
                    && pressed[button as usize]
                    && response.is_pointer_button_down_on()
                {
                    Some(DragPress {
                        button,
                        origin: origin?,
                        frame,
                        started_at: None,
                        released: false,
                    })
                } else {
                    None
                }
            })?;
        press.frame = frame;
        press.released = released[press.button as usize] && !down[press.button as usize];
        if press.started_at.is_none() && response.drag_started() && !press.released {
            press.started_at = Some(pass);
        }
        data.insert_temp(id, press);
        Some(press)
    })
}

/// The original press if it has just begun a drag. Call before testing the
/// response, so the press is recorded before movement reaches the threshold.
pub(super) fn drag_start(
    ui: &egui::Ui,
    response: &Response,
) -> Option<(egui::PointerButton, Pos2)> {
    if response.is_pointer_button_down_on() || response.drag_started() {
        let pass = ui.ctx().cumulative_pass_nr();
        drag_press(ui, response)
            .filter(|press| press.started_at == Some(pass))
            .map(|press| (press.button, press.origin))
    } else {
        None
    }
}

pub(super) fn primary_drag_start(ui: &egui::Ui, response: &Response) -> Option<Pos2> {
    drag_start(ui, response)
        .filter(|(button, _)| *button == egui::PointerButton::Primary)
        .map(|(_, origin)| origin)
}

fn primary_press(ui: &egui::Ui, response: &Response) -> Option<DragPress> {
    drag_press(ui, response).filter(|press| press.button == egui::PointerButton::Primary)
}

/// A primary click with no other pointer button held down. Keyboard and
/// assistive clicks also pass because no pointer button is down.
pub(super) fn primary_clicked(ui: &egui::Ui, response: &Response) -> bool {
    response.clicked()
        && !ui.input(|input| {
            POINTER_BUTTONS[1..]
                .iter()
                .any(|&button| input.pointer.button_down(button))
        })
}

/// The primary button is down on the control, and initiated the press on its
/// own. Only that button edits a control that follows the pointer while a
/// button is down on it.
pub(super) fn primary_down_on(ui: &egui::Ui, response: &Response) -> bool {
    primary_press(ui, response).is_some_and(|press| !press.released)
}

/// A drag belongs to the primary button only if that button pressed this
/// control on its own.
pub(super) fn primary_dragged(ui: &egui::Ui, response: &Response) -> bool {
    primary_press(ui, response).is_some_and(|press| press.started_at.is_some() && !press.released)
}

/// Like `primary_dragged`, including the release frame so a handle drag
/// finishes at its pointer position.
pub(super) fn primary_drag_or_stop(ui: &egui::Ui, response: &Response) -> bool {
    primary_press(ui, response).is_some_and(|press| press.started_at.is_some())
}

/// The pointer in the widget's layer while an owned gesture is active, also
/// on the frames where egui no longer reports the widget as dragged.
pub(super) fn drag_position(ui: &egui::Ui, response: &Response) -> Option<Pos2> {
    let pos = ui.input(|input| input.pointer.latest_pos())?;
    Some(
        ui.ctx()
            .layer_transform_from_global(response.layer_id)
            .map_or(pos, |transform| transform * pos),
    )
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
        let pass = ui.ctx().cumulative_pass_nr();
        if let Some(palette) = ui.data(|data| {
            ui.stack().iter().find_map(|ancestor| {
                data.get_temp::<(u64, Self)>(palette_id(ancestor.id))
                    .filter(|(stored_pass, _)| *stored_pass == pass)
                    .map(|(_, palette)| palette)
            })
        }) {
            return palette;
        }
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

fn palette_id(ui_id: egui::Id) -> egui::Id {
    ui_id.with("controls-palette")
}

pub(super) fn apply_palette(ui: &mut egui::Ui, palette: Palette) {
    let visuals = ui.visuals_mut();
    visuals.panel_fill = palette.body();
    visuals.selection.bg_fill = palette.accent();
    visuals.selection.stroke = Stroke::new(1.0_f32, roles().ink);
    visuals.override_text_color = None;
    let pass = ui.ctx().cumulative_pass_nr();
    let id = palette_id(ui.unique_id());
    ui.data_mut(|data| data.insert_temp(id, (pass, palette)));
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
