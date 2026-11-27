//! 3-theme code-painted instrument kit shared by the FX rack and Deep Edit.
//! Branches strictly on `Material` (`Flat`, `Plastic`, `Metal`) via `theme::roles()`.
use super::{HoverHints, Material, roles, theme, toggle_at};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Stroke, Vec2,
};
use mixjam_techno::application::FxBusMeterReading;
use mixjam_techno::application::fx::ReverbProfile;

#[path = "instrument_scopes.rs"]
mod scopes;
pub(in super::super) use scopes::*;

#[cfg(test)]
#[path = "instrument_tests.rs"]
mod tests;

pub(in super::super) fn paint_metal_top_and_led(
    painter: &egui::Painter,
    rect: Rect,
    active: bool,
    show_led: bool,
) {
    let r = roles();
    painter.line_segment(
        [
            Pos2::new(rect.left() + 6.0, rect.top() + 2.0),
            Pos2::new(rect.right() - 6.0, rect.top() + 2.0),
        ],
        Stroke::new(1.0_f32, Color32::from_white_alpha(54)),
    );
    if !show_led {
        return;
    }
    let led = Pos2::new(rect.left() + 16.5, rect.top() + 11.5);
    painter.circle_filled(led, 3.0, r.ink);
    if active {
        painter.circle_filled(led, 2.1, r.lamp_on);
        painter.circle_filled(
            led - Vec2::splat(0.7),
            0.7,
            Color32::from_rgb(255, 244, 210),
        );
    } else {
        painter.circle_filled(led, 2.1, r.lamp_off);
    }
}

fn paint_metal_bay_accents(painter: &egui::Painter, rect: Rect, active: bool, show_led: bool) {
    let r = roles();
    painter.rect_stroke(
        rect.shrink(1.5),
        5.0,
        Stroke::new(1.0_f32, Color32::from_rgb(108, 122, 110)),
        egui::StrokeKind::Inside,
    );
    for corner in [
        rect.left_top() + Vec2::new(6.0, 6.0),
        rect.right_top() + Vec2::new(-6.0, 6.0),
    ] {
        painter.circle_filled(corner, 2.0, r.well_edge);
        painter.circle_stroke(corner, 2.0, Stroke::new(0.7_f32, r.ink));
        painter.line_segment(
            [corner - Vec2::new(1.1, 0.8), corner + Vec2::new(1.1, 0.8)],
            Stroke::new(0.8_f32, r.ink),
        );
    }
    if rect.height() >= 84.0 {
        for corner in [
            rect.left_bottom() + Vec2::new(6.0, -6.0),
            rect.right_bottom() + Vec2::new(-6.0, -6.0),
        ] {
            painter.circle_filled(corner, 2.0, r.well_edge);
            painter.line_segment(
                [corner - Vec2::new(1.1, 0.8), corner + Vec2::new(1.1, 0.8)],
                Stroke::new(0.8_f32, r.ink),
            );
        }
    }
    paint_metal_top_and_led(painter, rect, active, show_led);
}

pub(in super::super) fn paint_bay_header(
    painter: &egui::Painter,
    rect: Rect,
    title: &str,
    title_x: f32,
    text_color: Color32,
    rule_color: Color32,
    is_metal: bool,
) {
    painter.text(
        Pos2::new(title_x, rect.top() + 11.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(10.5),
        text_color,
    );
    let rule_y = rect.top() + 19.0;
    painter.line_segment(
        [
            Pos2::new(rect.left() + 10.0, rule_y),
            Pos2::new(rect.right() - 10.0, rule_y),
        ],
        Stroke::new(1.1_f32, rule_color),
    );
    if is_metal {
        // Modular silk-screen bracket end-ticks.
        for x in [rect.left() + 10.0, rect.right() - 10.0] {
            painter.line_segment(
                [Pos2::new(x, rule_y), Pos2::new(x, rule_y + 3.5)],
                Stroke::new(1.1_f32, rule_color),
            );
        }
    }
}

struct BaySpec<'a> {
    title: &'a str,
    accent: Color32,
    ink_on_light: bool,
    active: bool,
    show_led: bool,
}

fn paint_bay(ui: &egui::Ui, rect: Rect, spec: BaySpec<'_>) {
    let r = roles();
    let painter = ui.painter();
    let is_metal = r.material == Material::Metal;
    paint_bay_shell(painter, rect, spec.active, spec.show_led);
    let base_text = if spec.ink_on_light { r.ink } else { r.legend };
    let text_color = if spec.active {
        base_text
    } else {
        base_text.gamma_multiply(0.76)
    };
    let rule_color = if is_metal {
        let base = Color32::from_rgb(236, 136, 48);
        if spec.active {
            base
        } else {
            base.gamma_multiply(0.38)
        }
    } else if spec.active {
        spec.accent.gamma_multiply(0.55)
    } else {
        spec.accent.gamma_multiply(0.28)
    };
    let title_x = if is_metal && spec.show_led {
        rect.left() + 21.5
    } else {
        rect.left() + 14.0
    };
    paint_bay_header(
        painter, rect, spec.title, title_x, text_color, rule_color, is_metal,
    );
}

/// Draw a static recessed chassis bay with material-aware framing and title legend.
pub(in super::super) fn bay(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    accent: Color32,
    ink_on_light: bool,
) {
    paint_bay(
        ui,
        rect,
        BaySpec {
            title,
            accent,
            ink_on_light,
            active: true,
            show_led: false,
        },
    );
}

/// Draw a switchable recessed chassis bay with active/dimmed status LED.
pub(in super::super) fn bay_with_state(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    accent: Color32,
    ink_on_light: bool,
    active: bool,
) {
    paint_bay(
        ui,
        rect,
        BaySpec {
            title,
            accent,
            ink_on_light,
            active,
            show_led: true,
        },
    );
}

fn paint_bay_shell(painter: &egui::Painter, rect: Rect, active: bool, show_led: bool) {
    let r = roles();
    if r.material == Material::Metal {
        painter.rect_filled(rect, 6.0, r.ink);
        let (top, bottom) = if active {
            (Color32::from_rgb(86, 98, 88), Color32::from_rgb(68, 79, 70))
        } else {
            (Color32::from_rgb(64, 72, 66), Color32::from_rgb(52, 60, 54))
        };
        theme::gradient(painter, rect.shrink(1.5), 5.0, top, bottom);
        paint_metal_bay_accents(painter, rect, active, show_led);
        return;
    }
    let (radius, fill) = match r.material {
        Material::Flat => (
            CornerRadius::same(4),
            Color32::from_rgba_unmultiplied(r.ink.r(), r.ink.g(), r.ink.b(), 18),
        ),
        _ => (
            CornerRadius::same(8),
            Color32::from_rgba_unmultiplied(r.face.r(), r.face.g(), r.face.b(), 30),
        ),
    };
    painter.rect_filled(rect, radius, fill);
    let alpha = if active { 0.75 } else { 0.45 };
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, r.bay_edge.gamma_multiply(alpha)),
        egui::StrokeKind::Inside,
    );
}

fn metal_slide_colors(value: bool, enabled: bool) -> (Color32, Color32) {
    let r = roles();
    let tab_fill = match (value, enabled) {
        (true, true) => Color32::from_rgb(218, 224, 218),
        (true, false) => Color32::from_rgb(162, 168, 162),
        (false, true) => Color32::from_rgb(136, 142, 136),
        (false, false) => Color32::from_rgb(96, 102, 96),
    };
    let lamp_color = match (value, enabled) {
        (true, true) => r.lamp_on,
        (true, false) => r.lamp_on.gamma_multiply(0.45),
        (false, _) => r.lamp_off,
    };
    (tab_fill, lamp_color)
}

fn paint_metal_slide_switch(
    painter: &egui::Painter,
    switch_rect: Rect,
    value: bool,
    enabled: bool,
) {
    let r = roles();
    // Modular recessed slide-switch bezel with machined actuator tab.
    let slot = Rect::from_center_size(switch_rect.center(), Vec2::new(26.0, 14.0));
    painter.rect_filled(slot, 2.0, r.ink);
    painter.rect_stroke(
        slot,
        2.0,
        Stroke::new(
            1.0_f32,
            r.well_edge
                .gamma_multiply(if enabled { 0.75 } else { 0.35 }),
        ),
        egui::StrokeKind::Inside,
    );
    let tab_x = if value {
        slot.right() - 7.5
    } else {
        slot.left() + 7.5
    };
    let tab = Rect::from_center_size(Pos2::new(tab_x, slot.center().y), Vec2::new(12.0, 11.0));
    let (tab_fill, lamp_color) = metal_slide_colors(value, enabled);
    painter.rect_filled(tab, 1.5, tab_fill);
    painter.line_segment(
        [
            Pos2::new(tab.center().x, tab.top() + 2.0),
            Pos2::new(tab.center().x, tab.bottom() - 2.0),
        ],
        Stroke::new(1.6_f32, r.ink),
    );
    let lamp_x = if value {
        slot.left() + 5.0
    } else {
        slot.right() - 5.0
    };
    painter.circle_filled(Pos2::new(lamp_x, slot.center().y), 2.3, lamp_color);
}

fn bat_label_width(ui: &egui::Ui, label: &str) -> f32 {
    if label.is_empty() {
        return 0.0;
    }
    ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(label.into(), FontId::proportional(11.0), Color32::WHITE)
            .size()
            .x
    }) + 8.0
}

/// Toggle switch styled cleanly for chassis hardware across all three themes.
pub(in super::super) fn bat_toggle(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    label: &str,
    value: &mut bool,
    hint: &str,
    ink_on_light: bool,
) -> Response {
    let r = roles();
    let enabled = ui.is_enabled();
    let switch_rect = Rect::from_min_size(rect.min, Vec2::new(28.0, 20.0));
    let label_w = bat_label_width(ui, label);
    let hit_rect = Rect::from_min_size(
        rect.min,
        Vec2::new(
            rect.width().max(switch_rect.width() + label_w),
            switch_rect.height(),
        ),
    );
    let response = toggle_at(ui, hit_rect, name, "", value);
    super::reveal_focus(ui, &response);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, response.enabled(), *value, name)
    });
    if r.material == Material::Metal {
        paint_metal_slide_switch(ui.painter(), switch_rect, *value, enabled);
    }
    if !label.is_empty() {
        let base = if ink_on_light { r.ink } else { r.legend };
        let color = if enabled {
            base
        } else {
            base.gamma_multiply(if ink_on_light { 0.58 } else { 0.65 })
        };
        ui.painter().text(
            Pos2::new(switch_rect.right() + 4.0, switch_rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(11.0),
            color,
        );
    }
    let hint_text = format!("{name}: {}. {hint}", if *value { "on" } else { "off" });
    response.hover_hint(&hint_text).disabled_hint(hint_text)
}

/// Where a strip of keys sits and what it is called.
pub(in super::super) struct KeyStrip<'a> {
    pub rect: Rect,
    /// A column of keys, not a row.
    pub vertical: bool,
    /// Each key is named "{name} {label}".
    pub name: &'a str,
    /// The chosen key's light.
    pub accent: Color32,
}

fn strip_arrow_delta(response: &egui::Response, vertical: bool) -> isize {
    if !response.has_focus() {
        return 0;
    }
    let (prev, next) = if vertical {
        (egui::Key::ArrowUp, egui::Key::ArrowDown)
    } else {
        (egui::Key::ArrowLeft, egui::Key::ArrowRight)
    };
    response.ctx.input_mut(|input| {
        if input.consume_key(egui::Modifiers::NONE, prev) {
            -1
        } else if input.consume_key(egui::Modifiers::NONE, next) {
            1
        } else {
            0
        }
    })
}

struct StripKeyDraw {
    key: Rect,
    well_bg: Color32,
    radius: f32,
    index: usize,
    prev_selected: bool,
}

fn draw_strip_key<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    strip: &KeyStrip<'_>,
    draw: StripKeyDraw,
    item: &(T, &str, &str),
    value: &mut T,
) -> (egui::Response, isize) {
    let enabled = ui.is_enabled();
    let (option, label, hint) = *item;
    let selected = *value == option;
    let response = ui.interact(
        draw.key,
        strip_key_id(ui, strip.name, draw.index),
        egui::Sense::click(),
    );
    super::reveal_focus(ui, &response);
    name_key(
        &response,
        &format!("{} {label}", strip.name),
        label,
        (enabled, selected),
    );
    let face = KeyFace {
        rect: draw.key.shrink(1.5),
        radius: draw.radius - 1.0,
        well: draw.well_bg,
        accent: strip.accent,
    };
    let ink = paint_key_face(
        painter,
        face,
        selected,
        enabled && response.hovered(),
        enabled,
    );
    if draw.index > 0 && !selected && !draw.prev_selected {
        paint_key_seam(painter, draw.key, strip.vertical, roles().well_edge);
    }
    painter.text(
        draw.key.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        ink,
    );
    paint_key_focus(ui, painter, &response, draw.key, draw.radius);
    let step = if enabled {
        strip_arrow_delta(&response, strip.vertical)
    } else {
        0
    };
    let response = response.hover_hint(hint).disabled_hint(hint);
    (response, step)
}

fn apply_strip_arrow<T: Copy + PartialEq>(
    ui: &egui::Ui,
    strip_name: &str,
    arrow_target: Option<usize>,
    value: &mut T,
    options: &[(T, &str, &str)],
) -> bool {
    let Some(target) = arrow_target else {
        return false;
    };
    let changed = *value != options[target].0;
    if changed {
        *value = options[target].0;
    }
    let target_id = strip_key_id(ui, strip_name, target);
    // The strip took the arrow, so egui must not move focus again.
    ui.ctx().memory_mut(|m| {
        m.request_focus(target_id);
        m.move_focus(egui::FocusDirection::None);
    });
    changed
}

/// The id of a strip's key, the same for its interaction and its arrow focus.
fn strip_key_id(ui: &egui::Ui, strip_name: &str, index: usize) -> egui::Id {
    ui.id().with((strip_name, index))
}

/// One choice of a short list as a strip of keys, each option's
/// `(value, label, hint)`. The chosen key lights in the strip's accent; on a
/// disabled strip it keeps a dim accent face, so the choice still reads.
/// Returns whether the choice changed.
pub(in super::super) fn key_strip<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    strip: KeyStrip<'_>,
    value: &mut T,
    options: &[(T, &str, &str)],
) -> bool {
    let r = roles();
    let font = FontId::proportional(11.0);
    let labels: Vec<&str> = options.iter().map(|(_, label, _)| *label).collect();
    let keys = strip_keys(ui, &strip, &labels, &font);
    let (radius, well_bg) = strip_well(r.material, r.strip_slot);
    let painter = ui.painter().clone();
    painter.rect_filled(strip.rect, radius, well_bg);
    let mut changed = false;
    let mut arrow_target: Option<usize> = None;
    let count = options.len();
    for (index, (item, key)) in options.iter().zip(&keys).enumerate() {
        let prev_selected = index > 0 && *value == options[index - 1].0;
        let (response, step) = draw_strip_key(
            ui,
            &painter,
            &strip,
            StripKeyDraw {
                key: *key,
                well_bg,
                radius,
                index,
                prev_selected,
            },
            item,
            value,
        );
        if response.clicked() && *value != item.0 {
            *value = item.0;
            changed = true;
        }
        if step != 0 && count > 0 {
            arrow_target = Some((index as isize + step).rem_euclid(count as isize) as usize);
        }
    }
    changed |= apply_strip_arrow(ui, strip.name, arrow_target, value, options);
    painter.rect_stroke(
        strip.rect,
        radius,
        Stroke::new(1.0_f32, r.well_edge.gamma_multiply(0.7)),
        egui::StrokeKind::Inside,
    );
    changed
}

/// The rectangle of each key of a strip: equal heights in a vertical strip,
/// widths in proportion to the labels in a horizontal one.
fn strip_keys(ui: &egui::Ui, strip: &KeyStrip<'_>, labels: &[&str], font: &FontId) -> Vec<Rect> {
    let rect = strip.rect;
    let widths: Vec<f32> = labels
        .iter()
        .map(|label| {
            ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap((*label).to_owned(), font.clone(), Color32::WHITE)
                    .size()
                    .x
            }) + 10.0
        })
        .collect();
    let total: f32 = widths.iter().sum();
    let mut start = if strip.vertical {
        rect.top()
    } else {
        rect.left()
    };
    let mut keys = Vec::with_capacity(labels.len());
    for width in &widths {
        let key = if strip.vertical {
            let h = rect.height() / labels.len() as f32;
            Rect::from_min_size(Pos2::new(rect.left(), start), Vec2::new(rect.width(), h))
        } else {
            let w = rect.width() * width / total;
            Rect::from_min_size(Pos2::new(start, rect.top()), Vec2::new(w, rect.height()))
        };
        start += if strip.vertical {
            key.height()
        } else {
            key.width()
        };
        keys.push(key);
    }
    keys
}

/// The corner radius and the well colour of a strip in `material`.
fn strip_well(material: Material, slot: Color32) -> (f32, Color32) {
    match material {
        Material::Flat => (3.0, slot),
        Material::Plastic => (5.0, slot),
        Material::Metal => (2.0, Color32::from_rgb(16, 20, 17)),
    }
}

/// Give a strip key its accessible name and state.
/// `state` is whether the strip is enabled and whether the key is chosen.
fn name_key(response: &egui::Response, full: &str, label: &str, state: (bool, bool)) {
    let (enabled, selected) = state;
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label)
    });
    response.ctx.accesskit_node_builder(response.id, |node| {
        node.set_label(full);
        node.set_selected(selected);
    });
}

/// The face of one strip key.
#[derive(Clone, Copy)]
struct KeyFace {
    rect: Rect,
    radius: f32,
    well: Color32,
    accent: Color32,
}

/// Light the chosen key, or the hovered one faintly. Returns the legend ink.
fn paint_key_face(
    painter: &egui::Painter,
    face: KeyFace,
    selected: bool,
    hovered: bool,
    enabled: bool,
) -> Color32 {
    let r = roles();
    if selected {
        let fill = if enabled {
            face.accent
        } else {
            face.well.lerp_to_gamma(face.accent, 0.42)
        };
        painter.rect_filled(face.rect, face.radius, fill);
        return if super::contrast(fill, r.ink) >= super::contrast(fill, r.legend) {
            r.ink
        } else {
            r.legend
        };
    }
    if hovered {
        painter.rect_filled(face.rect, face.radius, r.legend.gamma_multiply(0.10));
    }
    if enabled {
        r.legend.gamma_multiply(0.86)
    } else {
        r.disabled_text
    }
}

/// The focus ring of a strip key that holds the keyboard focus.
fn paint_key_focus(
    ui: &egui::Ui,
    painter: &egui::Painter,
    response: &egui::Response,
    key: Rect,
    radius: f32,
) {
    if response.has_focus() {
        painter.rect_stroke(
            key.expand(1.0),
            radius,
            Stroke::new(2.0_f32, super::focus_color(ui)),
            egui::StrokeKind::Outside,
        );
    }
}

/// The seam between two unlit keys.
fn paint_key_seam(painter: &egui::Painter, key: Rect, vertical: bool, edge: Color32) {
    let seam = if vertical {
        [
            Pos2::new(key.left() + 4.0, key.top()),
            Pos2::new(key.right() - 4.0, key.top()),
        ]
    } else {
        [
            Pos2::new(key.left(), key.top() + 4.0),
            Pos2::new(key.left(), key.bottom() - 4.0),
        ]
    };
    painter.line_segment(seam, Stroke::new(1.0_f32, edge.gamma_multiply(0.6)));
}

/// Active and bypass/off status passed to state-aware oscilloscope views.
#[derive(Clone, Copy)]
pub(in super::super) struct ScopeState<'a> {
    pub active: bool,
    pub status: Option<&'a str>,
    pub accent: Color32,
}

impl<'a> ScopeState<'a> {
    pub(in super::super) const fn active(accent: Color32) -> Self {
        Self {
            active: true,
            status: None,
            accent,
        }
    }
}

/// Compact stereo peak and gain-reduction meters with a numeric readout.
/// The ducker's lane shows only on a bus that `ducks`.
pub(in super::super) fn meter_bar(
    ui: &egui::Ui,
    rect: Rect,
    name: &str,
    reading: FxBusMeterReading,
    ducks: bool,
) {
    paint_meter_bar(ui, rect, name, reading, ducks, true);
}

/// Compact meter bar without the trailing tail dot (used alongside a dedicated Tail pill).
pub(in super::super) fn meter_bar_no_tail_dot(
    ui: &egui::Ui,
    rect: Rect,
    name: &str,
    reading: FxBusMeterReading,
    ducks: bool,
) {
    paint_meter_bar(ui, rect, name, reading, ducks, false);
}

fn paint_meter_bar(
    ui: &egui::Ui,
    rect: Rect,
    name: &str,
    reading: FxBusMeterReading,
    ducks: bool,
    show_tail_dot: bool,
) {
    let response = ui.interact(rect, ui.id().with(name), egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, name));
    response.ctx.accesskit_node_builder(response.id, |node| {
        node.set_label(name);
        node.set_value(meter_readout(reading));
    });
    if response.hovered() {
        response.hover_hint(meter_readout(reading));
    }

    let r = roles();
    let painter = ui.painter();
    let radius = match r.material {
        Material::Flat => CornerRadius::same(3),
        Material::Plastic => CornerRadius::same(5),
        Material::Metal => CornerRadius::same(2),
    };
    let well_bg = if r.material == Material::Metal {
        Color32::from_rgb(16, 20, 17)
    } else {
        r.strip_slot
    };
    painter.rect_filled(rect, radius, well_bg);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, r.well_edge.gamma_multiply(0.7)),
        egui::StrokeKind::Inside,
    );

    let inner = rect.shrink(2.5);
    meter_lanes(painter, inner, reading, well_bg, ducks);
    meter_end(painter, inner, reading, show_tail_dot);
}

/// How far a peak of amplitude `amp` fills its lane, from -48 to 0 dBFS.
fn peak_fraction(amp: f32) -> f32 {
    if amp <= 1e-5 {
        return 0.0;
    }
    ((20.0 * amp.log10() + 48.0) / 48.0).clamp(0.0, 1.0)
}

/// How far a gain reduction of `db` fills its lane: all of it at 24 dB.
fn reduction_fraction(db: f32) -> f32 {
    (db / 24.0).clamp(0.0, 1.0)
}

/// The meter's lanes, each behind its letter: the L and R peaks over -48 to
/// 0 dBFS, then, on a bus that `ducks`, D, the ducker's gain reduction.
fn meter_lanes(
    painter: &egui::Painter,
    inner: Rect,
    reading: FxBusMeterReading,
    well_bg: Color32,
    ducks: bool,
) {
    let r = roles();
    let lanes = Rect::from_min_max(Pos2::new(inner.left() + 10.0, inner.top()), inner.max);
    let count = if ducks { 3 } else { 2 };
    let lane_h = ((lanes.height() - 2.0 * (count - 1) as f32) / count as f32).max(2.5);
    // A peak lane turns to the peak colour over 92% of its length.
    let peak = |amp: f32| {
        let fill = peak_fraction(amp);
        (
            fill,
            if fill > 0.92 {
                r.lamp_peak
            } else {
                r.lamp_good
            },
        )
    };
    let ((l, l_color), (r_fill, r_color)) = (peak(reading.peak_l), peak(reading.peak_r));
    let levels = [
        ("L", l, l_color),
        ("R", r_fill, r_color),
        ("D", reduction_fraction(reading.ducker_gr_db), r.lamp_warn),
    ];
    let track_at = |row: usize| {
        let y0 = lanes.top() + row as f32 * (lane_h + 2.0);
        Rect::from_min_max(
            Pos2::new(lanes.left(), y0),
            Pos2::new(lanes.right() - 8.0, y0 + lane_h),
        )
    };
    for (row, (letter, fill_frac, color)) in levels.into_iter().take(count).enumerate() {
        let track = track_at(row);
        // A silent lane still shows its track, so the meter reads as one.
        painter.rect_filled(track, 1.0, r.well_edge.gamma_multiply(0.35));
        painter.text(
            Pos2::new(inner.left() + 4.5, track.center().y),
            Align2::CENTER_CENTER,
            letter,
            FontId::proportional(8.5),
            r.legend.gamma_multiply(0.85),
        );
        if fill_frac > 0.0 {
            let fill = Rect::from_min_max(
                track.min,
                Pos2::new(track.left() + track.width() * fill_frac, track.max.y),
            );
            painter.rect_filled(fill, 1.0, color);
        }
    }
    // Breaks at -24 and -12 dB split the peak lanes into segments.
    let (top, bottom) = (track_at(0), track_at(1));
    for db in [-24.0, -12.0] {
        let x = top.left() + top.width() * (db + 48.0) / 48.0;
        painter.line_segment(
            [Pos2::new(x, top.top()), Pos2::new(x, bottom.bottom())],
            Stroke::new(1.0_f32, well_bg),
        );
    }
}

/// The meter's right end: the limiter's gain reduction while it acts, else
/// optionally a dot lit while the tail rings.
fn meter_end(
    painter: &egui::Painter,
    inner: Rect,
    reading: FxBusMeterReading,
    show_tail_dot: bool,
) {
    let r = roles();
    if reading.limiter_gr_db > 0.1 {
        let lim_frac = reduction_fraction(reading.limiter_gr_db);
        let lim_rect = Rect::from_min_max(
            Pos2::new(
                inner.right() - 6.0,
                inner.bottom() - inner.height() * lim_frac,
            ),
            Pos2::new(inner.right(), inner.bottom()),
        );
        painter.rect_filled(lim_rect, 1.0, r.lamp_peak);
    } else if show_tail_dot {
        let dot_color = if reading.tail_active {
            r.lamp_on
        } else {
            r.lamp_off
        };
        painter.circle_filled(
            Pos2::new(inner.right() - 3.0, inner.center().y),
            2.5,
            dot_color,
        );
    }
}

fn meter_readout(reading: FxBusMeterReading) -> String {
    let dbfs = |amplitude: f32| {
        if amplitude > 0.0 {
            format!("{:.1} dBFS", 20.0 * amplitude.log10())
        } else {
            "silent".to_owned()
        }
    };
    format!(
        "Left peak {}, RMS {}. Right peak {}, RMS {}. Ducker reduction {:.1} dB. \
         Limiter reduction {:.1} dB. Tail {}.",
        dbfs(reading.peak_l),
        dbfs(reading.rms_l),
        dbfs(reading.peak_r),
        dbfs(reading.rms_r),
        reading.ducker_gr_db,
        reading.limiter_gr_db,
        if reading.tail_active {
            "active"
        } else {
            "idle"
        },
    )
}

/// Recessed scope box and theme colors (`bg`, `trace`, `grid`, `secondary_trace`).
pub(in super::super) fn scope_well(
    ui: &egui::Ui,
    rect: Rect,
    accent: Color32,
) -> (Color32, Color32, Color32, Color32) {
    let r = roles();
    let painter = ui.painter();
    let (bg, trace, grid) = match r.material {
        Material::Metal => {
            let trace = accent.lerp_to_gamma(Color32::from_rgb(236, 248, 232), 0.14);
            (
                Color32::from_rgb(12, 18, 15).lerp_to_gamma(accent, 0.05),
                trace,
                Color32::from_rgba_unmultiplied(trace.r(), trace.g(), trace.b(), 44),
            )
        }
        Material::Plastic => {
            let trace = accent.lerp_to_gamma(Color32::from_rgb(230, 248, 255), 0.16);
            (
                Color32::from_rgb(8, 22, 48).lerp_to_gamma(accent, 0.07),
                trace,
                Color32::from_rgba_unmultiplied(trace.r(), trace.g(), trace.b(), 44),
            )
        }
        Material::Flat => (
            r.strip_slot,
            accent,
            Color32::from_rgba_unmultiplied(r.grid_line.r(), r.grid_line.g(), r.grid_line.b(), 64),
        ),
    };
    let radius = CornerRadius::same(if r.material == Material::Plastic {
        7
    } else {
        4
    });
    painter.rect_filled(rect, radius, bg);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, r.well_edge.lerp_to_gamma(accent, 0.25)),
        egui::StrokeKind::Inside,
    );
    painter.line_segment(
        [
            Pos2::new(rect.left() + 8.0, rect.top() + 1.5),
            Pos2::new(rect.right() - 8.0, rect.top() + 1.5),
        ],
        Stroke::new(1.5_f32, accent.gamma_multiply(0.55)),
    );
    let warm_primary = trace.r() > trace.b().saturating_add(24);
    let right_color = match (r.material, warm_primary) {
        (Material::Flat, true) => Color32::from_rgb(104, 196, 236),
        (Material::Flat, false) => Color32::from_rgb(232, 174, 84),
        (_, true) => Color32::from_rgb(112, 214, 246),
        (_, false) => Color32::from_rgb(242, 182, 88),
    };
    (bg, trace, grid, right_color)
}

pub(super) fn draw_scope_chrome(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    accent: Color32,
) -> (Rect, Color32, Color32, Color32) {
    let r = roles();
    let (_, trace, grid, right_color) = scope_well(ui, rect, accent);
    let plot = rect.shrink2(Vec2::new(8.0, 18.0));
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.top() + 9.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(10.0),
        r.legend,
    );
    (plot, trace, grid, right_color)
}

/// Recessed oscilloscope plotting `points` (`y(x)` over `[-1, 1]`).
pub(in super::super) fn scope_frame(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    points: &[f32],
    accent: Color32,
) {
    scope_frame_state(ui, rect, title, points, ScopeState::active(accent));
}

pub(super) fn draw_scope_status(painter: &egui::Painter, rect: Rect, state: ScopeState<'_>) {
    if let Some(status) = state.status.filter(|_| !state.active) {
        painter.text(
            Pos2::new(rect.right() - 8.0, rect.top() + 9.0),
            Align2::RIGHT_CENTER,
            status,
            FontId::proportional(9.0),
            roles().legend.gamma_multiply(0.76),
        );
    }
}

/// Transfer-curve oscilloscope with explicit active/off status and AccessKit label.
pub(in super::super) fn scope_frame_state(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    points: &[f32],
    state: ScopeState<'_>,
) {
    let r = roles();
    let painter = ui.painter();
    let (plot, trace, grid, _) = draw_scope_chrome(ui, rect, title, state.accent);
    draw_scope_status(painter, rect, state);
    let y = |val: f32| plot.center().y - val.clamp(-1.2, 1.2) * (plot.height() * 0.4);
    // Center axes & linear reference line y = x.
    painter.line_segment(
        [plot.left_center(), plot.right_center()],
        Stroke::new(1.0_f32, grid),
    );
    painter.line_segment(
        [plot.center_top(), plot.center_bottom()],
        Stroke::new(1.0_f32, grid),
    );
    painter.line_segment(
        [
            Pos2::new(plot.left(), y(-1.0)),
            Pos2::new(plot.right(), y(1.0)),
        ],
        Stroke::new(1.0_f32, grid),
    );

    if points.len() >= 2 {
        let denom = (points.len() - 1) as f32;
        let poly: Vec<Pos2> = points
            .iter()
            .enumerate()
            .map(|(i, &v)| Pos2::new(plot.left() + (i as f32 / denom) * plot.width(), y(v)))
            .collect();
        let color = if state.active {
            trace
        } else {
            trace.gamma_multiply(0.45)
        };
        draw_glow_trace(
            painter,
            poly,
            color,
            state.active && r.material != Material::Flat,
        );
    }
    let summary = match state.status {
        Some(tag) if !state.active => format!("Non-linear transfer curve ({tag})"),
        _ => "Non-linear transfer curve (Active)".to_owned(),
    };
    hover_access_label(ui, rect, title, summary);
}

pub(super) fn draw_glow_trace(
    painter: &egui::Painter,
    poly: Vec<Pos2>,
    trace: Color32,
    glow: bool,
) {
    if glow {
        painter.add(egui::Shape::line(
            poly.clone(),
            Stroke::new(3.5_f32, trace.gamma_multiply(0.28)),
        ));
    }
    painter.add(egui::Shape::line(poly, Stroke::new(1.8_f32, trace)));
}

pub(super) fn draw_horiz_fractions(
    painter: &egui::Painter,
    plot: Rect,
    fractions: &[f32],
    grid: Color32,
) {
    for &frac in fractions {
        let y = plot.top() + frac * plot.height();
        painter.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(1.0_f32, grid.gamma_multiply(0.7)),
        );
    }
}

pub(super) fn hover_access_label(ui: &egui::Ui, rect: Rect, title: &str, value: String) {
    let response = ui.interact(rect, ui.id().with(title), egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, title));
    let hint = format!("{title}: {value}");
    response.ctx.accesskit_node_builder(response.id, |node| {
        node.set_label(title);
        node.set_value(value);
    });
    response.hover_hint(hint);
}

pub(in super::super) fn draw_quarter_grid(painter: &egui::Painter, plot: Rect, grid: Color32) {
    for frac in [0.25_f32, 0.5, 0.75] {
        let gx = plot.left() + plot.width() * frac;
        painter.line_segment(
            [Pos2::new(gx, plot.top()), Pos2::new(gx, plot.bottom())],
            Stroke::new(1.0_f32, grid.gamma_multiply(0.65)),
        );
    }
}

fn paint_delay_legend(
    painter: &egui::Painter,
    rect: Rect,
    left_color: Color32,
    right_color: Color32,
    tag: &str,
    tag_color: Color32,
) {
    let y = rect.top() + 9.0;
    let tag_w = painter
        .layout_no_wrap(tag.to_owned(), FontId::proportional(9.0), tag_color)
        .size()
        .x;
    painter.text(
        Pos2::new(rect.right() - 8.0, y),
        Align2::RIGHT_CENTER,
        tag,
        FontId::proportional(9.0),
        tag_color,
    );
    painter.text(
        Pos2::new(rect.right() - 14.0 - tag_w, y),
        Align2::RIGHT_CENTER,
        "R",
        FontId::proportional(9.0),
        right_color,
    );
    painter.text(
        Pos2::new(rect.right() - 24.0 - tag_w, y),
        Align2::RIGHT_CENTER,
        "L",
        FontId::proportional(9.0),
        left_color,
    );
}

/// Delay tap scope with explicit active/inactive status and AccessKit readout.
pub(in super::super) fn delay_scope_frame_state(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    taps: &[(f32, f32, f32)],
    ping_pong: bool,
    state: ScopeState<'_>,
) {
    let r = roles();
    let painter = ui.painter();
    let (plot, raw_left, grid, raw_right) = draw_scope_chrome(ui, rect, title, state.accent);
    let dim = if state.active { 1.0 } else { 0.45 };
    let left_color = raw_left.gamma_multiply(dim);
    let right_color = raw_right.gamma_multiply(dim);
    let mode_tag = if ping_pong { "L/R PING" } else { "L/R DUAL" };
    let header_tag = match state.status {
        Some(status) if !state.active => format!("{mode_tag} · {status}"),
        _ => mode_tag.to_owned(),
    };
    paint_delay_legend(
        painter,
        rect,
        left_color,
        right_color,
        &header_tag,
        r.legend
            .gamma_multiply(if state.active { 0.85 } else { 0.72 }),
    );
    let mid_y = plot.center().y;
    let half_h = plot.height() * 0.46;
    painter.line_segment(
        [
            Pos2::new(plot.left(), mid_y),
            Pos2::new(plot.right(), mid_y),
        ],
        Stroke::new(1.0_f32, grid),
    );
    draw_quarter_grid(painter, plot, grid);

    for &(t, l_amp, r_amp) in taps {
        let x = plot.left() + t.clamp(0.0, 1.0) * (plot.width() - 4.0) + 2.0;
        let base = Pos2::new(x, mid_y);
        let glow = state.active && r.material != Material::Flat;
        draw_delay_stem(painter, base, l_amp, -1.0, half_h, left_color, glow);
        draw_delay_stem(painter, base, r_amp, 1.0, half_h, right_color, glow);
    }
    hover_access_label(
        ui,
        rect,
        title,
        format!("{header_tag}, {} plotted taps", taps.len()),
    );
}

fn draw_delay_stem(
    painter: &egui::Painter,
    base: Pos2,
    amp: f32,
    dir: f32,
    half_h: f32,
    color: Color32,
    glow: bool,
) {
    let norm = amp.abs().min(1.25) / 1.25;
    if norm <= 0.005 {
        return;
    }
    let tip = Pos2::new(base.x, base.y + dir * norm * half_h);
    if glow {
        painter.line_segment(
            [base, tip],
            Stroke::new(4.0_f32, color.gamma_multiply(0.26)),
        );
    }
    painter.line_segment([base, tip], Stroke::new(2.0_f32, color));
    painter.circle_filled(tip, 2.2, color);
}

/// The color of the high band's curve, apart from the middle and low bands.
pub(super) fn high_band_color(material: Material) -> Color32 {
    match material {
        Material::Metal => Color32::from_rgb(196, 226, 168),
        Material::Plastic => Color32::from_rgb(232, 240, 255),
        Material::Flat => Color32::from_rgb(214, 220, 232),
    }
}

/// Plot one decay curve over the scope's plot area, with a soft glow under
/// it when `glow` is set.
fn draw_decay_curve(
    ui: &egui::Ui,
    plot: Rect,
    curve: &[f32],
    color: Color32,
    width: f32,
    glow: bool,
) {
    let denom = (curve.len() - 1) as f32;
    let poly: Vec<Pos2> = curve
        .iter()
        .enumerate()
        .map(|(i, v)| {
            Pos2::new(
                plot.left() + i as f32 / denom * plot.width(),
                plot.bottom() - v.clamp(0.0, 1.0) * plot.height(),
            )
        })
        .collect();
    if glow {
        let stroke = Stroke::new(2.0 * width, color.gamma_multiply(0.28));
        ui.painter().add(egui::Shape::line(poly.clone(), stroke));
    }
    ui.painter()
        .add(egui::Shape::line(poly, Stroke::new(width, color)));
}

/// A gain reduction as a level change: `-2.1 dB`, or `0.0 dB` below a twentieth of a dB.
pub(in super::super) fn reduction_text(db: f32) -> String {
    if db >= 0.05 {
        format!("-{db:.1} dB")
    } else {
        "0.0 dB".to_owned()
    }
}

fn paint_reverb_band_legend(
    painter: &egui::Painter,
    right_x: f32,
    y: f32,
    bands: [f32; 3],
    colors: [Color32; 3],
) {
    let items = [
        (format!("T {:.1}s", bands[2]), colors[2]),
        (format!("M {:.1}s", bands[1]), colors[1]),
        (format!("B {:.1}s", bands[0]), colors[0]),
    ];
    let mut cursor = right_x;
    for (text, color) in items {
        let w = painter
            .layout_no_wrap(text.clone(), FontId::proportional(9.0), color)
            .size()
            .x;
        painter.text(
            Pos2::new(cursor, y),
            Align2::RIGHT_CENTER,
            text,
            FontId::proportional(9.0),
            color,
        );
        cursor -= w + 8.0;
    }
}

/// Reverb decay scope with explicit active/inactive dimming and color-coded footer legend.
pub(in super::super) fn reverb_scope_frame_state(
    ui: &egui::Ui,
    rect: Rect,
    title: &str,
    tag: &str,
    profile: &ReverbProfile,
    state: ScopeState<'_>,
) {
    let r = roles();
    let painter = ui.painter();
    let (mut plot, raw_mid, grid, raw_low) = draw_scope_chrome(ui, rect, title, state.accent);
    let dim = if state.active { 1.0 } else { 0.45 };
    let low_color = raw_low.gamma_multiply(dim);
    let mid_color = raw_mid.gamma_multiply(dim);
    let high_color = high_band_color(r.material).gamma_multiply(dim);
    let header_tag = match state.status {
        Some(status) if !state.active => format!("{tag} · {status}"),
        _ => tag.to_owned(),
    };
    painter.text(
        Pos2::new(rect.right() - 8.0, rect.top() + 9.0),
        Align2::RIGHT_CENTER,
        &header_tag,
        FontId::proportional(9.0),
        r.legend
            .gamma_multiply(if state.active { 0.85 } else { 0.72 }),
    );
    if profile.er_count > 0 {
        let early = Rect::from_min_max(plot.left_bottom() - Vec2::new(0.0, 44.0), plot.max);
        draw_early_reflections(ui, early, profile, low_color, grid);
        plot.max.y = early.top() - 6.0;
    }
    // -20 and -40 dB; the bottom edge is -60 dB.
    draw_horiz_fractions(painter, plot, &[1.0 / 3.0, 2.0 / 3.0], grid);
    let pre_x = plot.left() + profile.pre_delay_norm * plot.width();
    painter.line_segment(
        [
            Pos2::new(pre_x, plot.top()),
            Pos2::new(pre_x, plot.bottom()),
        ],
        Stroke::new(1.0_f32, grid),
    );
    plot.min.x = pre_x;
    let [low, mid, high] = &profile.decay_curves;
    let glow = state.active && r.material != Material::Flat;
    draw_decay_curve(ui, plot, low, low_color, 1.4, false);
    draw_decay_curve(ui, plot, high, high_color, 1.4, false);
    draw_decay_curve(ui, plot, mid, mid_color, 1.8, glow);
    let [b, m, t] = profile.rt60_bands;
    let horizon = profile.horizon_seconds;
    painter.text(
        Pos2::new(rect.left() + 8.0, rect.bottom() - 9.0),
        Align2::LEFT_CENTER,
        format!("0–{horizon:.2} s"),
        FontId::proportional(9.0),
        r.legend,
    );
    paint_reverb_band_legend(
        painter,
        rect.right() - 8.0,
        rect.bottom() - 9.0,
        profile.rt60_bands,
        [low_color, mid_color, high_color],
    );
    let pre_ms = profile.pre_delay_norm * horizon * 1000.0;
    hover_access_label(
        ui,
        rect,
        title,
        format!(
            "{title}. {header_tag}. Pre-delay {pre_ms:.0} ms. RT60 in seconds. \
             Bass {b:.2}. Middle {m:.2}. Treble {t:.2}. \
             Left-channel early reflections {}. Decay plot 0 to {horizon:.2} seconds.",
            profile.er_count
        ),
    );
}

/// Expand the reflection times on their own scale, starting after the pre-delay.
fn draw_early_reflections(
    ui: &egui::Ui,
    rect: Rect,
    profile: &ReverbProfile,
    trace: Color32,
    grid: Color32,
) {
    let painter = ui.painter();
    let taps = &profile.er_taps[..profile.er_count];
    let after_pre_ms = |t: f32| (t - profile.pre_delay_norm) * profile.horizon_seconds * 1000.0;
    let span = (after_pre_ms(taps[taps.len() - 1].0) / 10.0)
        .ceil()
        .max(1.0)
        * 10.0;
    painter.text(
        rect.left_top(),
        Align2::LEFT_TOP,
        format!("Early L · 0–{span:.0} ms after pre-delay"),
        FontId::proportional(9.0),
        roles().legend,
    );
    let axis = rect.center().y + 5.0;
    painter.line_segment(
        [Pos2::new(rect.left(), axis), Pos2::new(rect.right(), axis)],
        Stroke::new(1.0_f32, grid),
    );
    for &(time, strength) in taps {
        let x = rect.left() + after_pre_ms(time) / span * rect.width();
        let tip = Pos2::new(x, axis - 15.0 * strength);
        painter.line_segment([Pos2::new(x, axis), tip], Stroke::new(1.4_f32, trace));
        painter.circle_filled(tip, 1.6, trace);
    }
}
