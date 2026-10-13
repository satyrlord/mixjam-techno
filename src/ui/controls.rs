//! Shared Folded Metal controls. Tools own values and actions; this module owns
//! their appearance, pointer and keyboard interaction, and accessibility.
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Vec2};
use std::{ops::RangeInclusive, sync::Arc};

pub(super) const INK: Color32 = Color32::from_rgb(18, 20, 18);
pub(super) const PANEL: Color32 = Color32::from_rgb(60, 65, 64);
pub(super) const CREAM: Color32 = Color32::from_rgb(249, 242, 220);
pub(super) const ACCENT: Color32 = Color32::from_rgb(231, 147, 49);
pub(super) const FACE: Color32 = Color32::from_rgb(35, 38, 36);
const PRIMARY: Color32 = Color32::from_rgb(212, 205, 182);
const DISABLED_FACE: Color32 = Color32::from_rgb(61, 64, 60);
const DISABLED_TEXT: Color32 = Color32::from_rgb(201, 203, 192);
const CAP: Color32 = Color32::from_rgb(190, 190, 185);
const DANGER: Color32 = Color32::from_rgb(131, 54, 39);
/// Hairlines around dark bays, value wells, and native widgets.
const BAY_EDGE: Color32 = Color32::from_rgb(70, 72, 64);
const WELL_EDGE: Color32 = Color32::from_rgb(131, 134, 126);

/// Record the focused dial or fader. These controls use only arrows, Home,
/// and End, so the workspace keeps its transport key while one has focus.
pub(super) fn keep_transport_key(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("arrow-key-focus"), id));
}

pub(super) fn arrow_key_focus(ctx: &egui::Context) -> Option<egui::Id> {
    ctx.data(|data| data.get_temp(egui::Id::new("arrow-key-focus")))
}

fn contrast(a: Color32, b: Color32) -> f32 {
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
}

impl Palette {
    fn body(self) -> Color32 {
        match self {
            Self::Utility => PANEL,
            Self::Marine => Color32::from_rgb(32, 83, 91),
            Self::Granite => Color32::from_rgb(52, 57, 60),
            Self::Red => Color32::from_rgb(154, 45, 39),
            Self::Olive => Color32::from_rgb(60, 68, 40),
        }
    }
    fn accent(self) -> Color32 {
        match self {
            Self::Utility => ACCENT,
            Self::Marine => Color32::from_rgb(85, 174, 183),
            Self::Granite => Color32::from_rgb(173, 186, 189),
            Self::Red => Color32::from_rgb(235, 116, 104),
            Self::Olive => Color32::from_rgb(157, 178, 94),
        }
    }
    /// The keyboard focus ring. A ring crosses the chassis, so an accent
    /// below 3:1 against it gives way to cream.
    fn focus(self) -> Color32 {
        if contrast(self.accent(), self.body()) >= 3.0 {
            self.accent()
        } else {
            CREAM
        }
    }
    fn of(ui: &egui::Ui) -> Self {
        [Self::Marine, Self::Granite, Self::Red, Self::Olive]
            .into_iter()
            .find(|palette| palette.body() == ui.visuals().panel_fill)
            .unwrap_or(Self::Utility)
    }
}

pub(super) fn apply_palette(ui: &mut egui::Ui, palette: Palette) {
    let visuals = ui.visuals_mut();
    visuals.panel_fill = palette.body();
    visuals.selection.bg_fill = palette.accent();
    visuals.selection.stroke = Stroke::new(1.0_f32, INK);
    visuals.override_text_color = None;
}

#[derive(Clone, Copy)]
pub(super) enum ButtonRole {
    Secondary,
    Primary,
    Danger,
}

#[derive(Clone, Copy)]
pub(super) enum HeaderIcon {
    Collapse,
    Expand,
    Help,
    Close,
}

macro_rules! variants {
    ($name:literal) => {
        [
            include_bytes!(concat!(
                "../../assets/folded-metal/controls/100/",
                $name,
                ".png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../../assets/folded-metal/controls/150/",
                $name,
                ".png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../../assets/folded-metal/controls/200/",
                $name,
                ".png"
            ))
            .as_slice(),
        ]
    };
}

macro_rules! material {
    ($palette:literal, $name:literal) => {
        [
            include_bytes!(concat!(
                "../../assets/folded-metal/",
                $palette,
                "/100/",
                $name,
                ".png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../../assets/folded-metal/",
                $palette,
                "/150/",
                $name,
                ".png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../../assets/folded-metal/",
                $palette,
                "/200/",
                $name,
                ".png"
            ))
            .as_slice(),
        ]
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

struct Artwork {
    material: Vec<egui::TextureHandle>,
    grain: egui::TextureHandle,
    knob: egui::TextureHandle,
    primary: egui::TextureHandle,
}

fn artwork(ctx: &egui::Context, palette: Palette) -> Arc<Artwork> {
    let density = [1.0_f32, 1.5, 2.0]
        .into_iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (a - ctx.pixels_per_point())
                .abs()
                .total_cmp(&(b - ctx.pixels_per_point()).abs())
        })
        .map_or(0, |(index, _)| index);
    let id = egui::Id::new(("folded-metal-artwork", palette, density));
    if let Some(art) = ctx.data(|data| data.get_temp::<Arc<Artwork>>(id)) {
        return art;
    }
    let load = |name: &str, bytes: &[u8]| {
        let image = image::load_from_memory(bytes)
            .expect("Bundled Folded Metal PNG must decode")
            .into_rgba8();
        ctx.load_texture(
            format!("folded-metal-{palette:?}-{name}-{density}"),
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
    };
    let grain = match palette {
        Palette::Utility => material!("utility", "paint-grain"),
        Palette::Marine => material!("marine", "paint-grain"),
        Palette::Granite => material!("granite", "paint-grain"),
        Palette::Red => material!("red", "paint-grain"),
        Palette::Olive => material!("olive", "paint-grain"),
    };
    let art = Arc::new(Artwork {
        material: parts
            .iter()
            .enumerate()
            .map(|(part, variants)| load(&format!("part-{part}"), variants[density]))
            .collect(),
        grain: load("grain", grain[density]),
        knob: load("knob", variants!("knob")[density]),
        primary: load("primary", variants!("primary")[density]),
    });
    ctx.data_mut(|data| data.insert_temp(id, art.clone()));
    art
}

fn fixed_image(ui: &egui::Ui, texture: &egui::TextureHandle, min: Pos2, clip: Rect) {
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
fn paint_grain(ui: &egui::Ui, texture: &egui::TextureHandle, rect: Rect) {
    let ppp = ui.ctx().pixels_per_point();
    let rect = Rect::from_min_max(
        (rect.min.to_vec2() * ppp).round().to_pos2() / ppp,
        (rect.max.to_vec2() * ppp).round().to_pos2() / ppp,
    );
    let size = texture.size_vec2() / ppp;
    let origin = (rect.min.to_vec2() * ppp).round().to_pos2() / ppp;
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

/// Material slices use one image pixel per physical pixel. Edge tiles are
/// repeated and clipped; all boundaries fade into the solid chassis. Native
/// rounded geometry owns the smooth outer silhouette and uninterrupted bevel.
pub(super) fn panel(ui: &egui::Ui, rect: Rect) {
    let palette = Palette::of(ui);
    let p = ui.painter();
    p.rect_filled(rect, 8.0, INK);
    p.rect_filled(rect.shrink(1.5), 7.0, palette.body());
    let art = artwork(ui.ctx(), palette);
    paint_grain(ui, &art.grain, rect);
    let ppp = ui.ctx().pixels_per_point();
    let corner = art.material[0].size_vec2() / ppp;
    let edge_x = art.material[4].size_vec2() / ppp;
    let edge_y = art.material[6].size_vec2() / ppp;
    for (index, y) in [(4, rect.top()), (5, rect.bottom() - edge_x.y)] {
        let mut x = rect.left() + corner.x;
        let clip = Rect::from_min_max(
            Pos2::new(x, rect.top()),
            Pos2::new((rect.right() - corner.x).max(x), rect.bottom()),
        );
        while x < rect.right() - corner.x {
            fixed_image(ui, &art.material[index], Pos2::new(x, y), clip);
            x += edge_x.x;
        }
    }
    for (index, x) in [(6, rect.left()), (7, rect.right() - edge_y.x)] {
        let mut y = rect.top() + corner.y;
        let clip = Rect::from_min_max(
            Pos2::new(rect.left(), y),
            Pos2::new(rect.right(), (rect.bottom() - corner.y).max(y)),
        );
        while y < rect.bottom() - corner.y {
            fixed_image(ui, &art.material[index], Pos2::new(x, y), clip);
            y += edge_y.y;
        }
    }
    // Alpha fades make overlap continuous, even in a short collapsed bar.
    // Half-window clipping would expose a hard line through the material.
    for (index, right, bottom) in [
        (0, false, false),
        (1, true, false),
        (2, false, true),
        (3, true, true),
    ] {
        let min = Pos2::new(
            if right {
                rect.right() - corner.x
            } else {
                rect.left()
            },
            if bottom {
                rect.bottom() - corner.y
            } else {
                rect.top()
            },
        );
        fixed_image(ui, &art.material[index], min, rect.shrink(4.0));
    }
    // These continuous antialiased paths never depend on source crop shape,
    // raster tile boundaries or the amount of available window space.
    p.rect_stroke(
        rect.shrink(0.75),
        8.0,
        Stroke::new(1.5_f32, INK),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        rect.shrink(2.0),
        6.5,
        Stroke::new(1.0_f32, palette.body().lerp_to_gamma(CREAM, 0.24)),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        rect.shrink(3.0),
        5.5,
        Stroke::new(1.0_f32, palette.body().gamma_multiply(0.72)),
        egui::StrokeKind::Inside,
    );
}

/// A narrow chassis uses the same grain and side folds as tool windows, without
/// overlapping their wide corner patches across the control column.
pub(super) fn rail_panel(ui: &egui::Ui, rect: Rect) {
    let palette = Palette::of(ui);
    let p = ui.painter();
    p.rect_filled(rect, 5.0, palette.body());
    let art = artwork(ui.ctx(), palette);
    paint_grain(ui, &art.grain, rect);
    let ppp = ui.ctx().pixels_per_point();
    for (index, right) in [(6, false), (7, true)] {
        let size = art.material[index].size_vec2() / ppp;
        let x = if right {
            rect.right() - size.x
        } else {
            rect.left()
        };
        let mut y = rect.top();
        while y < rect.bottom() {
            fixed_image(ui, &art.material[index], Pos2::new(x, y), rect.shrink(3.0));
            y += size.y;
        }
    }
    p.rect_stroke(
        rect,
        5.0,
        Stroke::new(1.0_f32, INK),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        rect.shrink(1.0),
        4.0,
        Stroke::new(1.0_f32, palette.body().lerp_to_gamma(CREAM, 0.20)),
        egui::StrokeKind::Inside,
    );
}

pub(super) fn rail_separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(48.0, 14.0), Sense::hover());
    let left = rect.left_center() + Vec2::new(3.0, 0.0);
    let right = rect.right_center() - Vec2::new(3.0, 0.0);
    ui.painter()
        .line_segment([left, right], Stroke::new(1.0_f32, INK));
    ui.painter().line_segment(
        [left + Vec2::Y, right + Vec2::Y],
        Stroke::new(1.0_f32, Color32::from_white_alpha(45)),
    );
}

pub(super) fn icon_button_surface(ui: &egui::Ui, response: &Response, active: bool) {
    paint_button(ui, response, "", ButtonRole::Secondary);
    if active {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            5.0,
            Stroke::new(1.0_f32, print_color(ui)),
            egui::StrokeKind::Inside,
        );
    }
}

/// The palette's accent is reserved for focus, selection and active indicators.
pub(super) fn print_color(ui: &egui::Ui) -> Color32 {
    ui.visuals().selection.bg_fill
}

pub(super) fn inset(ui: &egui::Ui, rect: Rect) {
    ui.painter().rect_filled(
        rect.translate(Vec2::new(0.0, 1.0)),
        6.0,
        Color32::from_white_alpha(48),
    );
    ui.painter().rect_filled(rect, 6.0, INK);
    ui.painter().rect_filled(rect.shrink(2.0), 4.0, FACE);
    ui.painter().rect_stroke(
        rect.shrink(2.0),
        4.0,
        Stroke::new(1.0_f32, BAY_EDGE),
        egui::StrokeKind::Inside,
    );
}

pub(super) fn section(ui: &egui::Ui, rect: Rect, label: &str) {
    ui.painter().rect_filled(rect, 3.0, FACE);
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(13.0), CREAM);
    let width = galley.size().x;
    ui.painter().galley(
        rect.left_center() + Vec2::new(8.0, -galley.size().y / 2.0),
        galley,
        CREAM,
    );
    let left = (rect.left() + width + 20.0).min(rect.right() - 8.0);
    if !label.is_empty() && left < rect.right() - 8.0 {
        ui.painter().line_segment(
            [
                Pos2::new(left, rect.center().y),
                rect.right_center() - Vec2::new(8.0, 0.0),
            ],
            Stroke::new(1.0_f32, CREAM.gamma_multiply(0.45)),
        );
    }
}

fn focus(ui: &egui::Ui, response: &Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            5.0,
            Stroke::new(2.0_f32, focus_color(ui)),
            egui::StrokeKind::Outside,
        );
    }
}

pub(super) fn focus_color(ui: &egui::Ui) -> Color32 {
    Palette::of(ui).focus()
}

fn paint_button(ui: &egui::Ui, response: &Response, label: &str, role: ButtonRole) {
    let rect = response.rect;
    let down = response.is_pointer_button_down_on();
    let base = match role {
        ButtonRole::Secondary => FACE,
        ButtonRole::Primary => PRIMARY,
        ButtonRole::Danger => DANGER,
    };
    let base = if !response.enabled() {
        DISABLED_FACE
    } else if down {
        base.gamma_multiply(0.8)
    } else if response.hovered() {
        base.lerp_to_gamma(Color32::WHITE, 0.09)
    } else {
        base
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 5.0, INK);
    let face = rect
        .shrink(2.0)
        .translate(Vec2::new(0.0, if down { 1.0 } else { -1.0 }));
    painter.rect_filled(face, 3.0, base);
    if matches!(role, ButtonRole::Primary) && response.enabled() {
        let art = artwork(ui.ctx(), Palette::of(ui));
        let size = art.primary.size_vec2() / ui.ctx().pixels_per_point();
        let mut y = face.top();
        while y < face.bottom() {
            let mut x = face.left();
            while x < face.right() {
                fixed_image(ui, &art.primary, Pos2::new(x, y), face.shrink(2.0));
                x += size.x;
            }
            y += size.y;
        }
        if down {
            painter.rect_filled(face.shrink(2.0), 2.0, Color32::from_black_alpha(25));
        }
    }
    painter.line_segment(
        [
            face.left_top() + Vec2::new(3.0, 1.0),
            face.right_top() + Vec2::new(-3.0, 1.0),
        ],
        Stroke::new(1.0_f32, Color32::from_white_alpha(55)),
    );
    let color = match role {
        ButtonRole::Primary => INK,
        _ => CREAM,
    };
    let color = if response.enabled() {
        color
    } else {
        DISABLED_TEXT
    };
    let galley = egui::WidgetText::from(
        egui::RichText::new(label)
            .font(FontId::proportional(13.0))
            .color(color),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (face.width() - 12.0).max(0.0),
        egui::TextStyle::Button,
    );
    painter
        .with_clip_rect(face.intersect(ui.clip_rect()))
        .galley(face.center() - galley.size() / 2.0, galley, color);
    focus(ui, response);
}

pub(super) fn button(ui: &mut egui::Ui, label: &str, role: ButtonRole) -> Response {
    let width = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(13.0), INK)
        .size()
        .x
        + 26.0;
    let (_, response) = ui.allocate_exact_size(Vec2::new(width.max(72.0), 30.0), Sense::click());
    paint_button(ui, &response, label, role);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response
}

pub(super) fn button_at(
    ui: &mut egui::Ui,
    rect: Rect,
    key: &str,
    label: &str,
    role: ButtonRole,
    enabled: bool,
) -> Response {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("hardware-button", key))
            .max_rect(rect),
    );
    if !enabled {
        child.disable();
    }
    let response = child.interact(rect, child.id().with(key), Sense::click());
    paint_button(&child, &response, label, role);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response
}

pub(super) fn header_button(
    ui: &mut egui::Ui,
    rect: Rect,
    key: &str,
    name: &str,
    icon: HeaderIcon,
) -> Response {
    let child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("hardware-header", key))
            .max_rect(rect),
    );
    let response = child.interact(rect, child.id().with(key), Sense::click());
    let center = rect.center();
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(
            rect.shrink(2.0),
            3.0,
            Color32::from_black_alpha(if response.is_pointer_button_down_on() {
                35
            } else {
                16
            }),
        );
    }
    let pen = Stroke::new(1.8_f32, CREAM);
    match icon {
        HeaderIcon::Close => {
            ui.painter().rect_filled(rect.shrink(2.0), 4.0, INK);
            ui.painter().rect_stroke(
                rect.shrink(3.0),
                3.0,
                Stroke::new(1.0_f32, CREAM.gamma_multiply(0.35)),
                egui::StrokeKind::Inside,
            );
            ui.painter().line_segment(
                [center + Vec2::new(-6.0, -6.0), center + Vec2::new(6.0, 6.0)],
                pen,
            );
            ui.painter().line_segment(
                [center + Vec2::new(6.0, -6.0), center + Vec2::new(-6.0, 6.0)],
                pen,
            );
        }
        HeaderIcon::Collapse | HeaderIcon::Expand => {
            let vertices = if matches!(icon, HeaderIcon::Collapse) {
                vec![
                    center + Vec2::new(-5.5, -3.5),
                    center + Vec2::new(5.5, -3.5),
                    center + Vec2::new(0.0, 5.0),
                ]
            } else {
                vec![
                    center + Vec2::new(-3.5, -5.5),
                    center + Vec2::new(-3.5, 5.5),
                    center + Vec2::new(5.0, 0.0),
                ]
            };
            ui.painter()
                .add(egui::Shape::convex_polygon(vertices, CREAM, Stroke::NONE));
        }
        HeaderIcon::Help => {
            ui.painter().circle_filled(center, 11.0, INK);
            ui.painter().circle_stroke(
                center,
                10.0,
                Stroke::new(1.0_f32, CREAM.gamma_multiply(0.4)),
            );
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                "?",
                FontId::proportional(15.0),
                CREAM,
            );
        }
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), name)
    });
    focus(ui, &response);
    response.on_hover_text(name)
}

pub(super) fn toggle_at(
    ui: &mut egui::Ui,
    rect: Rect,
    key: &str,
    label: &str,
    value: &mut bool,
) -> Response {
    let mut response = ui.interact(rect, ui.id().with(key), Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let center = Pos2::new(rect.left() + 14.0, rect.center().y);
    ui.painter().rect_filled(
        Rect::from_center_size(center, Vec2::new(26.0, 14.0)),
        7.0,
        INK,
    );
    ui.painter().circle_filled(
        center + Vec2::new(if *value { 6.0 } else { -6.0 }, -0.5),
        5.5,
        if *value { print_color(ui) } else { CAP },
    );
    ui.painter().text(
        Pos2::new(rect.left() + 34.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        CREAM,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            response.enabled(),
            *value,
            label,
        )
    });
    focus(ui, &response);
    response
}

pub(super) fn well(ui: &egui::Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 3.0, INK);
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0_f32, WELL_EDGE),
        egui::StrokeKind::Inside,
    );
}

/// Keep status text above the fill so contrast does not change with progress.
/// No numeric value is reported until the total amount of work is known.
pub(super) fn progress(ui: &mut egui::Ui, label: &str, fraction: Option<f32>, width: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::hover());
    let text = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(13.0), CREAM);
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .galley(
            Pos2::new(rect.center().x - text.size().x / 2.0, rect.top()),
            text,
            CREAM,
        );
    let rail = Rect::from_min_max(
        rect.left_bottom() - Vec2::new(0.0, 10.0),
        rect.right_bottom(),
    );
    well(ui, rail);
    let inner = rail.shrink(2.0);
    let fill = if let Some(fraction) = fraction {
        Rect::from_min_size(
            inner.min,
            Vec2::new(inner.width() * fraction.clamp(0.0, 1.0), inner.height()),
        )
    } else {
        let phase = ui.input(|input| (input.time * 1.8).sin() as f32 * 0.5 + 0.5);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
        Rect::from_min_size(
            inner.min + Vec2::new(inner.width() * 0.75 * phase, 0.0),
            Vec2::new(inner.width() * 0.25, inner.height()),
        )
    };
    if fill.width() > 0.0 {
        ui.painter().rect_filled(fill, 1.0, print_color(ui));
    }
    response.widget_info(|| {
        let mut info =
            egui::WidgetInfo::labeled(egui::WidgetType::ProgressIndicator, ui.is_enabled(), label);
        info.value = fraction.map(|value| f64::from(value.clamp(0.0, 1.0)) * 100.0);
        info
    });
}

pub(super) fn value_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.override_font_id = Some(FontId::proportional(13.0));
    style.drag_value_text_style = egui::TextStyle::Body;
    style.spacing.button_padding = Vec2::new(2.0, 0.0);
    // Compact wells have an 18-point inner rectangle. A larger native minimum
    // grows the invisible value button below that rectangle and shifts its text.
    style.spacing.interact_size.y = 18.0;
    style.wrap_mode = Some(egui::TextWrapMode::Extend);
    style.visuals.override_text_color = Some(CREAM);
    style.visuals.extreme_bg_color = INK;
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.bg_fill = Color32::TRANSPARENT;
        widget.weak_bg_fill = Color32::TRANSPARENT;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke.color = CREAM;
    }
    // egui paints a focused or dragged value with these visuals. The edge
    // shows keyboard focus on a field that is not yet being typed in.
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, CREAM);
}

/// Compact editable value for ordinary dialog forms.
pub(super) fn number(
    ui: &mut egui::Ui,
    value: &mut f64,
    range: RangeInclusive<f64>,
    name: &str,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(104.0, 30.0), Sense::hover());
    well(ui, rect);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(name)
            .max_rect(rect.shrink(3.0)),
    );
    value_style(&mut child);
    let response = child.add_sized(
        rect.shrink(3.0).size(),
        egui::DragValue::new(value).range(range),
    );
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(name));
    response
}

pub(super) fn disclosure(label: impl Into<String>) -> egui::CollapsingHeader {
    egui::CollapsingHeader::new(egui::RichText::new(label.into()).color(CREAM))
}

pub(super) fn radio(ui: &mut egui::Ui, selected: bool, label: &str) -> Response {
    ui.add(egui::RadioButton::new(
        selected,
        egui::RichText::new(label).color(CREAM),
    ))
}

pub(super) fn slider(
    ui: &mut egui::Ui,
    value: &mut f64,
    range: RangeInclusive<f64>,
    name: &str,
) -> Response {
    ui.scope(|ui| {
        let (low, high) = (*range.start(), *range.end());
        // egui uses inactive.bg_fill for both the rail and idle handle. Keep
        // its native rail and interaction, then paint the shared silver cap.
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.bg_fill = INK;
        for visual in [
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
        ] {
            visual.bg_stroke = Stroke::new(1.0_f32, INK);
            visual.fg_stroke = Stroke::new(1.0_f32, INK);
        }
        widgets.hovered.bg_fill = CAP;
        widgets.active.bg_fill = CAP;
        let response = ui.add(
            egui::Slider::new(value, range)
                .show_value(false)
                .step_by(0.1)
                .clamping(egui::SliderClamping::Edits),
        );
        let radius = response.rect.height() / 2.5;
        let fraction = ((*value - low) / (high - low)).clamp(0.0, 1.0) as f32;
        let center = Pos2::new(
            egui::lerp(
                (response.rect.left() + radius)..=(response.rect.right() - radius),
                fraction,
            ),
            response.rect.center().y,
        );
        ui.painter().circle_filled(center, radius, CAP);
        ui.painter()
            .circle_stroke(center, radius, Stroke::new(1.5_f32, INK));
        if response.has_focus() {
            ui.painter()
                .circle_stroke(center, radius + 2.0, Stroke::new(2.0_f32, focus_color(ui)));
        }
        response.widget_info(|| egui::WidgetInfo::slider(response.enabled(), *value, name));
        response
    })
    .inner
}

#[derive(Clone, Copy)]
pub(super) struct RotarySpec<'a> {
    pub name: &'a str,
    pub low: f64,
    pub high: f64,
    pub reset: f64,
    pub step: f64,
    pub decimals: usize,
    pub logarithmic: bool,
    pub choices: &'a [f64],
}

impl RotarySpec<'_> {
    fn fraction(self, value: f64) -> f64 {
        let value = value.clamp(self.low, self.high);
        if self.logarithmic {
            (value / self.low).ln() / (self.high / self.low).ln()
        } else {
            (value - self.low) / (self.high - self.low)
        }
    }

    fn value(self, fraction: f64) -> f64 {
        let fraction = fraction.clamp(0.0, 1.0);
        if self.logarithmic {
            self.low * (self.high / self.low).powf(fraction)
        } else {
            self.low + fraction * (self.high - self.low)
        }
    }

    fn bounded(self, value: f64) -> f64 {
        let value = value.clamp(self.low, self.high);
        self.choices
            .iter()
            .copied()
            .min_by(|a, b| (a - value).abs().total_cmp(&(b - value).abs()))
            .unwrap_or(value)
    }

    fn stepped(self, value: f64, direction: f64, fine: bool) -> f64 {
        if direction == 0.0 {
            return value;
        }
        if !self.choices.is_empty() {
            return if direction > 0.0 {
                self.choices
                    .iter()
                    .copied()
                    .find(|choice| *choice > value)
                    .unwrap_or(self.high)
            } else {
                self.choices
                    .iter()
                    .copied()
                    .rev()
                    .find(|choice| *choice < value)
                    .unwrap_or(self.low)
            };
        }
        let amount = direction * if fine { 0.1 } else { 1.0 };
        if !self.logarithmic {
            return (value + amount * self.step).clamp(self.low, self.high);
        }
        let next = self.value(self.fraction(value) + amount * 0.01);
        if fine {
            return next;
        }
        let precision = 10.0_f64.powi(self.decimals as i32);
        let next = (next * precision).round() / precision;
        if (next - value).abs() < self.step {
            (value + direction * self.step).clamp(self.low, self.high)
        } else {
            next.clamp(self.low, self.high)
        }
    }
}

pub(super) fn rotary(
    ui: &mut egui::Ui,
    rect: Rect,
    value: &mut f64,
    spec: RotarySpec<'_>,
) -> Response {
    let mut response = ui.interact(
        rect,
        ui.id().with(("rotary", spec.name)),
        Sense::click_and_drag(),
    );
    let before = *value;
    if response.enabled() {
        if response.has_focus() {
            keep_transport_key(ui.ctx(), response.id);
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
        }
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        let draft_id = response.id.with("draft");
        if response.drag_started() {
            ui.ctx()
                .data_mut(|data| data.insert_temp(draft_id, spec.fraction(before)));
        }
        if response.dragged() {
            let draft = ui
                .ctx()
                .data(|data| data.get_temp::<f64>(draft_id))
                .unwrap_or_else(|| spec.fraction(before));
            let delta = ui.input(|input| input.pointer.delta());
            let fine = ui.input(|input| input.modifiers.shift);
            let draft = (draft
                + f64::from((delta.x - delta.y) / 150.0) * if fine { 0.1 } else { 1.0 })
            .clamp(0.0, 1.0);
            ui.ctx().data_mut(|data| data.insert_temp(draft_id, draft));
            let precision = 10.0_f64.powi(spec.decimals as i32);
            *value = spec.bounded((spec.value(draft) * precision).round() / precision);
        }
        let mut direction = 0.0_f64;
        if response.hovered() {
            direction = ui.input_mut(|input| {
                let delta = input.raw_scroll_delta.y;
                input.raw_scroll_delta = Vec2::ZERO;
                input.smooth_scroll_delta = Vec2::ZERO;
                if delta.abs() > 0.0 {
                    f64::from(delta.signum())
                } else {
                    0.0
                }
            });
        }
        if response.has_focus() {
            ui.input_mut(|input| {
                let modifiers = input.modifiers;
                if input.consume_key(modifiers, egui::Key::ArrowUp)
                    || input.consume_key(modifiers, egui::Key::ArrowRight)
                {
                    direction = 1.0;
                }
                if input.consume_key(modifiers, egui::Key::ArrowDown)
                    || input.consume_key(modifiers, egui::Key::ArrowLeft)
                {
                    direction = -1.0;
                }
                if input.consume_key(modifiers, egui::Key::Home) {
                    *value = spec.low;
                }
                if input.consume_key(modifiers, egui::Key::End) {
                    *value = spec.high;
                }
            });
        }
        if direction != 0.0 {
            if response.has_focus() {
                ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
            }
            *value = spec.stepped(*value, direction, ui.input(|input| input.modifiers.shift));
        }
        ui.input(|input| {
            use egui::accesskit::{Action, ActionData};
            for request in input.accesskit_action_requests(response.id, Action::SetValue) {
                if let Some(ActionData::NumericValue(draft)) = request.data
                    && draft.is_finite()
                {
                    *value = spec.bounded(draft);
                }
            }
            let turns = input.num_accesskit_action_requests(response.id, Action::Increment) as i32
                - input.num_accesskit_action_requests(response.id, Action::Decrement) as i32;
            for _ in 0..turns.abs() {
                *value = spec.stepped(*value, f64::from(turns.signum()), false);
            }
        });
        if response.double_clicked() {
            *value = spec.bounded(spec.reset);
        }
    }
    if before != *value {
        response.mark_changed();
    }
    let center = rect.center();
    let art = artwork(ui.ctx(), Palette::of(ui));
    let ppp = ui.ctx().pixels_per_point();
    let size = art.knob.size_vec2() / ppp;
    let min = ((center - size / 2.0).to_vec2() * ppp).round().to_pos2() / ppp;
    let radius = size.x / 2.0;
    for index in 0..=18 {
        let vector = Vec2::angled((-225.0 + index as f32 * 15.0).to_radians());
        ui.painter().line_segment(
            [
                center + vector * (radius + 2.0),
                center + vector * (radius + if index % 3 == 0 { 6.0 } else { 4.0 }),
            ],
            Stroke::new(1.0_f32, CREAM),
        );
    }
    ui.painter().image(
        art.knob.id(),
        Rect::from_min_size(min, size),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    let vector = Vec2::angled((-225.0 + spec.fraction(*value) as f32 * 270.0).to_radians());
    ui.painter().line_segment(
        [
            center + vector * radius * 0.2,
            center + vector * radius * 0.62,
        ],
        Stroke::new(2.0_f32, INK),
    );
    if !response.enabled() {
        ui.painter()
            .circle_filled(center, radius + 1.0, Color32::from_black_alpha(100));
    } else if response.has_focus() || response.hovered() {
        ui.painter().circle_stroke(
            center,
            radius + 1.0,
            Stroke::new(
                if response.has_focus() {
                    2.0_f32
                } else {
                    1.0_f32
                },
                // The dial sits in a dark bay, where every accent is legible.
                print_color(ui),
            ),
        );
    }
    response.widget_info(|| egui::WidgetInfo::slider(response.enabled(), *value, spec.name));
    ui.ctx().accesskit_node_builder(response.id, |builder| {
        use egui::accesskit::Action;
        builder.set_min_numeric_value(spec.low);
        builder.set_max_numeric_value(spec.high);
        builder.set_numeric_value_step(spec.step);
        if response.enabled() {
            builder.add_action(Action::SetValue);
            if *value < spec.high {
                builder.add_action(Action::Increment);
            }
            if *value > spec.low {
                builder.add_action(Action::Decrement);
            }
        }
    });
    response.on_hover_text(format!(
        "{}: drag or use arrow keys. Hold Shift for fine movement. Double-click to reset.",
        spec.name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_status_keeps_text_above_visible_rail() {
        for fraction in [None, Some(0.0), Some(0.5), Some(1.0)] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    apply_palette(ui, Palette::Utility);
                    progress(ui, "Listing files…", fraction, 180.0);
                });
            });
            let text = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "Listing files…" => {
                        Some(text)
                    }
                    _ => None,
                })
                .unwrap();
            let rail = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.fill == INK && rect.rect.height() == 10.0 => {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .expect("An empty loader still needs its full recessed track");
            assert!(text.pos.y + text.galley.size().y <= rail.top());
            assert_eq!(text.galley.job.sections[0].format.color, CREAM);
            assert!(contrast(CREAM, Palette::Utility.body()) >= 4.5);
            let update = output.platform_output.accesskit_update.unwrap();
            let node = update
                .nodes
                .iter()
                .find(|(_, node)| node.role() == egui::accesskit::Role::ProgressIndicator)
                .unwrap();
            assert_eq!(
                node.1.numeric_value(),
                fraction.map(|value| f64::from(value) * 100.0)
            );
        }
    }

    #[test]
    fn numeric_text_fits_and_centers_in_its_own_container() {
        for density in [1.0, 1.5, 2.0] {
            for height in [18.0, 20.0, 24.0, 28.0] {
                let ctx = egui::Context::default();
                super::super::configure_style(&ctx);
                ctx.set_pixels_per_point(density);
                let rect = Rect::from_min_size(Pos2::new(40.0, 40.0), Vec2::new(100.0, height));
                let mut response_rect = Rect::NOTHING;
                let mut value = 2000.0_f64;
                let output = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        value_style(ui);
                        response_rect = ui
                            .put(rect, egui::DragValue::new(&mut value).fixed_decimals(0))
                            .rect;
                    });
                });
                assert!(
                    rect.contains_rect(response_rect),
                    "Value widget {response_rect:?} must fit {rect:?}"
                );
                let text = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == "2000" => {
                            Some(text.visual_bounding_rect())
                        }
                        _ => None,
                    })
                    .unwrap();
                let offset = (text.center() - rect.center()).abs() * density;
                assert!(
                    offset.x <= 1.0 && offset.y <= 1.0,
                    "Visible numeric text offset {offset:?} at {density}x in {height}-point container"
                );
            }
        }
    }

    #[test]
    fn folded_text_colors_stay_readable_on_their_actual_surfaces() {
        let luminance = |color: Color32| {
            let linear = |v: u8| {
                let v = f64::from(v) / 255.0;
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
        };
        for (role, text, background) in [
            ("utility labels", CREAM, Palette::Utility.body()),
            ("marine labels", CREAM, Palette::Marine.body()),
            ("granite labels", CREAM, Palette::Granite.body()),
            ("red labels", CREAM, Palette::Red.body()),
            ("olive labels", CREAM, Palette::Olive.body()),
            ("primary labels", INK, PRIMARY),
            ("utility selection", INK, Palette::Utility.accent()),
            ("marine selection", INK, Palette::Marine.accent()),
            ("granite selection", INK, Palette::Granite.accent()),
            ("red selection", INK, Palette::Red.accent()),
            ("olive selection", INK, Palette::Olive.accent()),
            ("value and button labels", CREAM, FACE),
            ("disabled labels", DISABLED_TEXT, DISABLED_FACE),
        ] {
            let a = luminance(text);
            let b = luminance(background);
            let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
            assert!(
                contrast >= 4.5,
                "{role} contrast {contrast:.2}:1 must be at least4.5:1"
            );
        }
    }

    #[test]
    fn focus_rings_stay_visible_against_every_chassis() {
        for palette in [
            Palette::Utility,
            Palette::Marine,
            Palette::Granite,
            Palette::Red,
            Palette::Olive,
        ] {
            let ratio = contrast(palette.focus(), palette.body());
            assert!(
                ratio >= 3.0,
                "{palette:?} focus ring contrast {ratio:.2}:1 must be at least 3:1"
            );
        }
        assert_eq!(Palette::Red.focus(), CREAM);
        assert_eq!(Palette::Marine.focus(), Palette::Marine.accent());
    }

    fn spec() -> RotarySpec<'static> {
        RotarySpec {
            name: "Test dial",
            low: 0.0,
            high: 2.0,
            reset: 1.0,
            step: 0.01,
            decimals: 3,
            logarithmic: false,
            choices: &[],
        }
    }

    #[test]
    fn absolute_buttons_do_not_advance_parent_layout() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (bounds, _) = ui.allocate_exact_size(Vec2::new(400.0, 44.0), Sense::hover());
                let cursor = ui.cursor();
                let occupied = ui.min_rect();
                for index in 0..3 {
                    let rect = Rect::from_min_size(
                        bounds.min + Vec2::new(index as f32 * 34.0, 4.0),
                        Vec2::splat(28.0),
                    );
                    let response = button_at(
                        ui,
                        rect,
                        &format!("header-{index}"),
                        "?",
                        ButtonRole::Secondary,
                        index != 1,
                    );
                    assert_eq!(response.enabled(), index != 1);
                }
                assert_eq!(ui.cursor(), cursor);
                assert_eq!(ui.min_rect(), occupied);
            });
        });
    }

    #[test]
    fn rotary_steps_boundaries_fine_and_discrete() {
        let linear = spec();
        assert_eq!(linear.stepped(2.0, 1.0, false), 2.0);
        assert_eq!(linear.stepped(0.0, -1.0, false), 0.0);
        assert_eq!(linear.stepped(1.0, 0.0, false), 1.0);
        assert_eq!(linear.stepped(1.0, 1.0, true), 1.001);
        let discrete = RotarySpec {
            low: 120.0,
            high: 180.0,
            choices: &[120.0, 140.0, 160.0, 180.0],
            ..linear
        };
        assert_eq!(discrete.bounded(150.0), 140.0);
        assert_eq!(discrete.stepped(140.0, 1.0, true), 160.0);
        assert_eq!(discrete.stepped(180.0, 1.0, false), 180.0);
        let logarithmic = RotarySpec {
            low: 20.0,
            high: 20_000.0,
            decimals: 0,
            step: 1.0,
            logarithmic: true,
            ..linear
        };
        assert_eq!(logarithmic.stepped(2000.0, 1.0, false), 2143.0);
    }

    #[test]
    fn supported_materials_decode_and_fade_to_solid_chassis() {
        for (index, density) in [1.0_f32, 1.5, 2.0].into_iter().enumerate() {
            let corner = image::load_from_memory(material!("utility", "corner-tl")[index])
                .unwrap()
                .into_rgba8();
            assert_eq!(
                corner.dimensions(),
                ((112.0 * density) as u32, (64.0 * density) as u32)
            );
            assert!(corner.pixels().any(|pixel| pixel[3] > 0));
            for x in 0..corner.width() {
                assert_eq!(corner.get_pixel(x, corner.height() - 1)[3], 0);
            }
            for y in 0..corner.height() {
                assert_eq!(corner.get_pixel(corner.width() - 1, y)[3], 0);
            }
            let knob = image::load_from_memory(variants!("knob")[index]).unwrap();
            assert_eq!(
                (knob.width(), knob.height()),
                ((36.0 * density) as u32, (36.0 * density) as u32)
            );
            let rgba = knob.to_rgba8();
            assert!(
                rgba.pixels()
                    .filter(|pixel| pixel[3] > 0 && pixel[3] < 255)
                    .count()
                    > knob.width() as usize,
                "The native-size dial silhouette must retain antialiased edge coverage at {density}x."
            );
        }
    }

    #[test]
    fn material_slices_have_no_outer_fringe_or_strip_endpoint_seams() {
        for palette in [
            parts!("utility"),
            parts!("marine"),
            parts!("granite"),
            parts!("red"),
            parts!("olive"),
        ] {
            for (part, variants) in palette.into_iter().enumerate() {
                for (density, bytes) in variants.into_iter().enumerate() {
                    let image = image::load_from_memory(bytes).unwrap().into_rgba8();
                    let (width, height) = image.dimensions();
                    assert!(image.pixels().any(|pixel| pixel[3] > 0));
                    for x in 0..width {
                        assert_eq!(image.get_pixel(x, 0)[3], 0);
                        assert_eq!(image.get_pixel(x, height - 1)[3], 0);
                    }
                    for y in 0..height {
                        assert_eq!(image.get_pixel(0, y)[3], 0);
                        assert_eq!(image.get_pixel(width - 1, y)[3], 0);
                    }
                    let margin = [4, 6, 8][density];
                    for y in 0..height {
                        for x in 0..width {
                            let side = match part {
                                0 | 2 | 6 => x < margin,
                                1 | 3 | 7 => x >= width - margin,
                                _ => false,
                            };
                            let top_bottom = match part {
                                0 | 1 | 4 => y < margin,
                                2 | 3 | 5 => y >= height - margin,
                                _ => false,
                            };
                            if side || top_bottom {
                                assert_eq!(image.get_pixel(x, y)[3], 0);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn paint_grain_preserves_palette_mean_and_monochrome_gaussian_statistics() {
        for (palette, variants) in [
            (Palette::Utility, material!("utility", "paint-grain")),
            (Palette::Marine, material!("marine", "paint-grain")),
            (Palette::Granite, material!("granite", "paint-grain")),
            (Palette::Red, material!("red", "paint-grain")),
            (Palette::Olive, material!("olive", "paint-grain")),
        ] {
            let base = palette.body();
            for (index, bytes) in variants.into_iter().enumerate() {
                let image = image::load_from_memory(bytes).unwrap().into_rgba8();
                let size = [256, 384, 512][index];
                assert_eq!(image.dimensions(), (size, size));
                let mut sum = 0.0_f64;
                let mut squares = 0.0_f64;
                let mut fourth = 0.0_f64;
                for pixel in image.pixels() {
                    let delta = i16::from(pixel[0]) - i16::from(base.r());
                    assert_eq!(delta, i16::from(pixel[1]) - i16::from(base.g()));
                    assert_eq!(delta, i16::from(pixel[2]) - i16::from(base.b()));
                    assert_eq!(pixel[3], 255);
                    assert!(delta.abs() <= 8, "Rare grain tails must remain subtle.");
                    let delta = f64::from(delta);
                    sum += delta;
                    squares += delta * delta;
                    fourth += delta.powi(4);
                }
                let count = f64::from(size * size);
                assert_eq!(sum, 0.0, "Grain must not shift a palette's mean color.");
                let variance = squares / count;
                assert!(variance.is_finite() && (1.2..=1.8).contains(&variance.sqrt()));
                let kurtosis = (fourth / count) / (variance * variance);
                assert!(
                    (2.7..=3.3).contains(&kurtosis),
                    "Grain must retain a Gaussian distribution, rather than two-level dithering."
                );
            }
        }
    }

    #[test]
    fn disabled_rotary_rejects_accessible_edits() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut value = 1.0;
        let mut response_id = egui::Id::NULL;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_enabled_ui(false, |ui| {
                    let response = rotary(
                        ui,
                        Rect::from_min_size(Pos2::new(30.0, 30.0), Vec2::splat(48.0)),
                        &mut value,
                        spec(),
                    );
                    assert!(!response.enabled());
                    response_id = response.id;
                });
            });
        });
        let input = egui::RawInput {
            events: vec![egui::Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action: egui::accesskit::Action::SetValue,
                    target: response_id.value().into(),
                    data: Some(egui::accesskit::ActionData::NumericValue(2.0)),
                },
            )],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_enabled_ui(false, |ui| {
                    rotary(
                        ui,
                        Rect::from_min_size(Pos2::new(30.0, 30.0), Vec2::splat(48.0)),
                        &mut value,
                        spec(),
                    );
                });
            });
        });
        assert_eq!(value, 1.0);
    }
}
