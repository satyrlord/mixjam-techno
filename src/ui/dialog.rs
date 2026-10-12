//! Shared utility-dialog presentation. Native containers retain input and close semantics.
use super::*;

const CREAM: Color32 = Color32::from_rgb(245, 232, 205);
const INK: Color32 = Color32::from_rgb(24, 25, 23);
const BODY: Color32 = Color32::from_rgb(42, 46, 48);
const FACE: Color32 = Color32::from_rgb(23, 27, 28);
const EDGE: Color32 = Color32::from_rgb(102, 110, 110);
const SECONDARY: Color32 = Color32::from_rgb(197, 199, 191);

macro_rules! pngs {
    ($name:literal) => {
        [
            include_bytes!(concat!("../../assets/dialog/100/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/dialog/150/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/dialog/200/", $name, ".png")).as_slice(),
        ]
    };
}

struct Artwork {
    density: usize,
    textures: Vec<egui::TextureHandle>,
}

impl Artwork {
    fn load(ctx: &egui::Context) -> Result<Arc<Self>, String> {
        let density = [1.0_f32, 1.5, 2.0]
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (**a - ctx.pixels_per_point())
                    .abs()
                    .total_cmp(&(**b - ctx.pixels_per_point()).abs())
            })
            .map_or(0, |(index, _)| index);
        let id = egui::Id::new(("dialog-artwork", density));
        if let Some(art) = ctx.data(|data| data.get_temp::<Arc<Self>>(id)) {
            return Ok(art);
        }
        let textures = [
            pngs!("top-left"),
            pngs!("top"),
            pngs!("top-right"),
            pngs!("left"),
            pngs!("body"),
            pngs!("right"),
            pngs!("bottom-left"),
            pngs!("bottom"),
            pngs!("bottom-right"),
        ]
        .iter()
        .enumerate()
        .map(|(index, variants)| {
            let image = image::load_from_memory(variants[density])
                .map_err(|error| format!("Cannot load dialog artwork: {error}"))?
                .into_rgba8();
            Ok(ctx.load_texture(
                format!("dialog-{density}-{index}"),
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

    fn shape(&self, ctx: &egui::Context, rect: Rect) -> egui::Shape {
        let ppp = ctx.pixels_per_point();
        let rect = Rect::from_min_max(
            (rect.min.to_vec2() * ppp).round().to_pos2() / ppp,
            (rect.max.to_vec2() * ppp).round().to_pos2() / ppp,
        );
        let scale = [1.0, 1.5, 2.0][self.density] / ppp;
        let xs = [
            rect.left(),
            rect.left() + 16.0 * scale,
            rect.right() - 16.0 * scale,
            rect.right(),
        ];
        // Collapsed utilities keep a complete rim rather than clipping the lower bezel.
        let top = (64.0 * scale).min(rect.height() - 16.0 * scale);
        let ys = [
            rect.top(),
            rect.top() + top,
            rect.bottom() - 16.0 * scale,
            rect.bottom(),
        ];
        let mut shapes = Vec::new();
        for row in 0..3 {
            for col in 0..3 {
                let target = Rect::from_min_max(
                    Pos2::new(xs[col], ys[row]),
                    Pos2::new(xs[col + 1], ys[row + 1]),
                );
                let texture = &self.textures[row * 3 + col];
                let size = texture.size_vec2() / ppp;
                let mut y = target.top();
                while y < target.bottom() {
                    let mut x = target.left();
                    while x < target.right() {
                        let end = Pos2::new(
                            (x + size.x).min(target.right()),
                            (y + size.y).min(target.bottom()),
                        );
                        let dest = Rect::from_min_max(Pos2::new(x, y), end);
                        let uv = Rect::from_min_max(
                            Pos2::ZERO,
                            Pos2::new(dest.width() / size.x, dest.height() / size.y),
                        );
                        let mut mesh = egui::Mesh::with_texture(texture.id());
                        mesh.add_rect_with_uv(dest, uv, Color32::WHITE);
                        shapes.push(egui::Shape::mesh(mesh));
                        x += size.x;
                    }
                    y += size.y;
                }
            }
        }
        egui::Shape::Vec(shapes)
    }
}

fn style(ctx: &egui::Context) -> Arc<egui::Style> {
    let saved = ctx.style();
    let mut style = (*saved).clone();
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(11.0));
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(18.0, egui::FontFamily::Name("Syncopate Bold".into())),
    );
    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.spacing.interact_size.y = 28.0;
    style.wrap_mode = Some(egui::TextWrapMode::Wrap);
    style.visuals.override_text_color = Some(CREAM);
    style.visuals.window_highlight_topmost = false;
    style.visuals.panel_fill = BODY;
    style.visuals.extreme_bg_color = FACE;
    style.visuals.faint_bg_color = FACE;
    style.visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 16,
        spread: 2,
        color: Color32::from_black_alpha(130),
    };
    style.visuals.widgets.noninteractive.fg_stroke.color = SECONDARY;
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.bg_fill = FACE;
        widget.weak_bg_fill = FACE;
        widget.bg_stroke = Stroke::new(1.0_f32, EDGE);
        widget.fg_stroke = Stroke::new(1.0_f32, CREAM);
        widget.corner_radius = egui::CornerRadius::same(3);
    }
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(53, 59, 60);
    style.visuals.widgets.hovered.weak_bg_fill = style.visuals.widgets.hovered.bg_fill;
    style.visuals.widgets.active.bg_stroke.color = ACCENT;
    ctx.set_style(style);
    saved
}

fn frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::TRANSPARENT)
        .inner_margin(egui::Margin::same(18))
        .corner_radius(12)
        .shadow(egui::epaint::Shadow::NONE)
}

fn finish_background(
    ctx: &egui::Context,
    painter: &egui::Painter,
    index: egui::layers::ShapeIdx,
    rect: Rect,
) {
    let skin = match Artwork::load(ctx) {
        Ok(art) => art.shape(ctx, rect),
        Err(_) => egui::Shape::rect_filled(rect, 12.0, BODY),
    };
    // The shadow must precede the raster bezel, including native collapsed windows.
    painter.set(
        index,
        egui::Shape::Vec(vec![
            ctx.style()
                .visuals
                .window_shadow
                .as_shape(rect, egui::CornerRadius::same(12))
                .into(),
            skin,
        ]),
    );
}

/// A styled native window. Its original ID, focus layer and title-bar controls stay intact.
pub(super) struct Window<'a> {
    inner: egui::Window<'a>,
    id: egui::Id,
}

pub(super) fn window(title: &str) -> Window<'_> {
    let id = egui::Id::new(title);
    Window {
        inner: egui::Window::new(title_text(title))
            .id(id)
            .default_height(220.0)
            .default_width(420.0),
        id,
    }
}

impl<'a> Window<'a> {
    pub(super) fn open(mut self, open: &'a mut bool) -> Self {
        self.inner = self.inner.open(open);
        self
    }
    pub(super) fn resizable(mut self, value: bool) -> Self {
        self.inner = self.inner.resizable(value);
        self
    }
    pub(super) fn collapsible(mut self, value: bool) -> Self {
        self.inner = self.inner.collapsible(value);
        self
    }
    pub(super) fn default_width(mut self, value: f32) -> Self {
        self.inner = self.inner.default_width(value);
        self
    }
    pub(super) fn default_height(mut self, value: f32) -> Self {
        self.inner = self.inner.default_height(value);
        self
    }
    pub(super) fn max_height(mut self, value: f32) -> Self {
        self.inner = self.inner.max_height(value);
        self
    }
    pub(super) fn constrain_to(mut self, value: Rect) -> Self {
        self.inner = self.inner.constrain_to(value);
        self
    }
    pub(super) fn vscroll(mut self, value: bool) -> Self {
        self.inner = self.inner.vscroll(value);
        self
    }
    pub(super) fn show<R>(
        self,
        ctx: &egui::Context,
        contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<egui::InnerResponse<Option<R>>> {
        let saved = style(ctx);
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, self.id));
        let index = painter.add(egui::Shape::Noop);
        let result = self.inner.frame(frame()).show(ctx, contents);
        if let Some(response) = &result {
            finish_background(ctx, &painter, index, response.response.rect);
        }
        ctx.set_style(saved);
        result
    }
}

/// Preserve native modal blocking, focus and dismissal responses; callers decide what closes.
pub(super) fn modal<R>(
    ctx: &egui::Context,
    id: egui::Id,
    identity: &str,
    title: &str,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::ModalResponse<R> {
    let saved = style(ctx);
    let mut background = None;
    let response = egui::Modal::new(id).frame(frame()).show(ctx, |ui| {
        name_dialog(ui, identity);
        ui.set_min_width((ctx.viewport_rect().width() - 96.0).clamp(240.0, 320.0));
        ui.set_max_width((ctx.viewport_rect().width() - 96.0).clamp(240.0, 700.0));
        background = Some((ui.painter().clone(), ui.painter().add(egui::Shape::Noop)));
        ui.heading(title_text(title));
        ui.add_space(12.0);
        contents(ui)
    });
    if let Some((painter, index)) = background {
        finish_background(ctx, &painter, index, response.response.rect);
    }
    ctx.set_style(saved);
    response
}

pub(super) fn primary(ui: &mut egui::Ui, label: &str) -> Response {
    ui.add(egui::Button::new(egui::RichText::new(label).color(INK)).fill(CREAM))
}

pub(super) fn danger(ui: &mut egui::Ui, label: &str) -> Response {
    ui.add(egui::Button::new(egui::RichText::new(label).color(CORAL)))
}
