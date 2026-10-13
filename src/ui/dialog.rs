//! Shared Folded Metal chassis and window lifecycle for every in-app surface.
use super::*;

const HEADER: f32 = 44.0;
const MIN_SIZE: Vec2 = Vec2::new(300.0, 160.0);

#[derive(Clone, Copy)]
struct Geometry {
    position: Pos2,
    size: Vec2,
}

pub(super) fn is_collapsed(ctx: &egui::Context, identity: &str) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new(("hardware-collapsed", identity))))
        .unwrap_or(false)
}
pub(super) fn expand(ctx: &egui::Context, identity: &str) {
    set_collapsed(ctx, identity, false);
}
fn set_collapsed(ctx: &egui::Context, identity: &str, value: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(("hardware-collapsed", identity)), value));
    if value && let Some(id) = ctx.memory(|m| m.focused()) {
        ctx.memory_mut(|m| m.surrender_focus(id));
    }
    ctx.request_repaint();
}

fn style(ui: &mut egui::Ui, palette: controls::Palette) {
    let s = ui.style_mut();
    s.text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(13.0));
    s.text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(13.0));
    s.text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(11.0));
    s.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(18.0, egui::FontFamily::Name("Syncopate Bold".into())),
    );
    s.spacing.item_spacing = Vec2::new(10.0, 10.0);
    s.spacing.button_padding = Vec2::new(12.0, 6.0);
    s.spacing.interact_size.y = 28.0;
    s.wrap_mode = Some(egui::TextWrapMode::Wrap);
    s.visuals.override_text_color = None;
    // Components communicate disabled state with their face/indicator. Keep
    // labels readable instead of applying a second opacity fade to the text.
    s.visuals.disabled_alpha = 1.0;
    s.visuals.panel_fill = controls::PANEL;
    s.visuals.extreme_bg_color = controls::FACE;
    s.visuals.faint_bg_color = Color32::from_black_alpha(12);
    s.visuals.widgets.noninteractive.fg_stroke.color = controls::CREAM;
    s.visuals.widgets.noninteractive.bg_stroke.color = Color32::from_black_alpha(70);
    s.visuals.selection.bg_fill = controls::ACCENT;
    s.visuals.selection.stroke.color = controls::INK;
    for w in [
        &mut s.visuals.widgets.inactive,
        &mut s.visuals.widgets.hovered,
        &mut s.visuals.widgets.active,
    ] {
        w.bg_fill = controls::FACE;
        w.weak_bg_fill = controls::FACE;
        w.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(93, 91, 82));
        w.fg_stroke = Stroke::new(1.0_f32, controls::CREAM);
        w.corner_radius = egui::CornerRadius::same(3);
    }
    s.visuals.widgets.hovered.bg_fill = Color32::from_rgb(58, 59, 55);
    s.visuals.widgets.hovered.weak_bg_fill = s.visuals.widgets.hovered.bg_fill;
    s.visuals.widgets.active.bg_stroke.color = controls::ACCENT;
    controls::apply_palette(ui, palette);
}

struct Shell<'a> {
    identity: &'a str,
    title: &'a str,
    size: Vec2,
    viewport: Rect,
    help: bool,
    close: bool,
    blocking: bool,
    palette: controls::Palette,
    inset_body: bool,
    /// Follow the height of the content until the user resizes the window.
    fit: bool,
}
struct ShellResponse<R> {
    response: Response,
    inner: Option<R>,
    help: bool,
    close: bool,
}

fn constrain(mut g: Geometry, viewport: Rect, collapsed: bool) -> Geometry {
    g.size = g.size.max(MIN_SIZE).min(viewport.size());
    let visible = Vec2::new(g.size.x, if collapsed { HEADER } else { g.size.y });
    g.position.x = g.position.x.clamp(
        viewport.left(),
        (viewport.right() - visible.x).max(viewport.left()),
    );
    g.position.y = g.position.y.clamp(
        viewport.top(),
        (viewport.bottom() - visible.y).max(viewport.top()),
    );
    g
}

fn show_shell<R>(
    ctx: &egui::Context,
    spec: Shell<'_>,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> ShellResponse<R> {
    let id = egui::Id::new(spec.identity);
    let gid = id.with("hardware-geometry");
    let collapsed = is_collapsed(ctx, spec.identity);
    let mut geometry = ctx
        .data(|d| d.get_temp::<Geometry>(gid))
        .unwrap_or_else(|| {
            let size = spec.size.max(MIN_SIZE).min(spec.viewport.size());
            Geometry {
                position: spec.viewport.center() - size / 2.0,
                size,
            }
        });
    geometry = constrain(geometry, spec.viewport, collapsed);
    let blocking = spec.blocking && !collapsed;
    let order = if spec.blocking {
        egui::Order::Foreground
    } else {
        egui::Order::Middle
    };
    let layer = egui::LayerId::new(order, id);
    let mut backdrop_clicked = false;
    if blocking {
        ctx.memory_mut(|m| m.set_modal_layer(layer));
        ctx.move_to_top(layer);
    }
    let mut close = false;
    let mut help = false;
    let popup_open = egui::Popup::is_any_open(ctx);
    let guarded = ctx
        .data(|d| d.get_temp::<bool>(id.with("hardware-escape-guard")))
        .unwrap_or(false);
    let shown = egui::Area::new(id)
        .kind(egui::UiKind::Window)
        .order(order)
        .fixed_pos(geometry.position)
        .movable(false)
        .sense(Sense::CLICK | Sense::DRAG)
        .fade_in(false)
        .show(ctx, |ui| {
            style(ui, spec.palette);
            if blocking {
                let rect = ctx.content_rect();
                let mut backdrop = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("backdrop")
                        .sense(Sense::CLICK | Sense::DRAG)
                        .max_rect(rect),
                );
                backdrop.set_min_size(rect.size());
                backdrop
                    .painter()
                    .rect_filled(rect, 0.0, Color32::from_black_alpha(85));
                backdrop_clicked = backdrop.response().clicked();
            }
            let display_title = title_text(spec.title);
            name_dialog(
                ui,
                if spec.blocking {
                    spec.identity
                } else {
                    &display_title
                },
            );
            let visible = Vec2::new(
                geometry.size.x,
                if collapsed { HEADER } else { geometry.size.y },
            );
            let (bounds, _) = ui.allocate_exact_size(visible, Sense::CLICK | Sense::DRAG);
            ctx.accesskit_node_builder(ui.unique_id(), |node| {
                node.set_bounds(egui::accesskit::Rect {
                    x0: bounds.left().into(),
                    y0: bounds.top().into(),
                    x1: bounds.right().into(),
                    y1: bounds.bottom().into(),
                })
            });
            ui.set_clip_rect(spec.viewport);
            ui.painter().add(
                egui::epaint::Shadow {
                    offset: [0, 5],
                    blur: 12,
                    spread: 0,
                    color: Color32::from_black_alpha(85),
                }
                .as_shape(bounds, egui::CornerRadius::same(9)),
            );
            controls::panel(ui, bounds);
            let header = Rect::from_min_size(bounds.min, Vec2::new(bounds.width(), HEADER));
            let drag_rect = Rect::from_min_max(
                header.min + Vec2::new(44.0, 2.0),
                header.max - Vec2::new(84.0, 3.0),
            );
            let drag = ui.interact(drag_rect, id.with("title-drag"), Sense::DRAG);
            if drag.dragged() && !drag.drag_started() {
                geometry.position += ctx.input(|i| i.pointer.delta());
            }
            let left = Rect::from_center_size(
                Pos2::new(header.left() + 24.0, header.center().y),
                Vec2::splat(28.0),
            );
            let label = format!(
                "{} {}",
                if collapsed { "Expand" } else { "Collapse" },
                spec.identity
            );
            if controls::header_button(
                ui,
                left,
                "collapse",
                &label,
                if collapsed {
                    controls::HeaderIcon::Expand
                } else {
                    controls::HeaderIcon::Collapse
                },
            )
            .clicked()
            {
                set_collapsed(ctx, spec.identity, !collapsed);
            }
            let title_rect = Rect::from_min_max(
                header.min + Vec2::new(48.0, 0.0),
                header.max - Vec2::new(88.0, 0.0),
            );
            let font = FontId::new(18.0, egui::FontFamily::Name("Syncopate Bold".into()));
            let galley = ui
                .painter()
                .layout_no_wrap(title_text(spec.title), font, controls::CREAM);
            ui.painter().with_clip_rect(title_rect).galley(
                Pos2::new(
                    (header.center().x - galley.size().x / 2.0).max(title_rect.left()),
                    header.center().y - galley.size().y / 2.0,
                ),
                galley,
                controls::CREAM,
            );
            if spec.help {
                let rect = Rect::from_center_size(
                    Pos2::new(header.right() - 61.0, header.center().y),
                    Vec2::splat(28.0),
                );
                help = controls::header_button(
                    ui,
                    rect,
                    "help",
                    &format!("{} help", spec.identity),
                    controls::HeaderIcon::Help,
                )
                .clicked();
            }
            if spec.close {
                let rect = Rect::from_center_size(
                    Pos2::new(header.right() - 24.0, header.center().y),
                    Vec2::splat(28.0),
                );
                close = controls::header_button(
                    ui,
                    rect,
                    "close",
                    &format!("Close {}", spec.identity),
                    controls::HeaderIcon::Close,
                )
                .clicked();
            }
            if collapsed {
                None
            } else {
                ui.painter().line_segment(
                    [
                        Pos2::new(bounds.left() + 10.0, header.bottom()),
                        Pos2::new(bounds.right() - 10.0, header.bottom()),
                    ],
                    Stroke::new(1.0_f32, Color32::from_black_alpha(150)),
                );
                let body = Rect::from_min_max(
                    bounds.min + Vec2::new(12.0, HEADER + 12.0),
                    bounds.max - Vec2::splat(12.0),
                );
                if spec.inset_body {
                    controls::inset(ui, body.expand(4.0));
                }
                // Text in a bay keeps clear of its edge.
                let body = body.shrink(if spec.inset_body { 6.0 } else { 0.0 });
                let mut content = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("hardware-content")
                        .max_rect(body),
                );
                content.set_clip_rect(body.intersect(spec.viewport));
                let scrolled = egui::ScrollArea::both()
                    .id_salt("hardware-scroll")
                    .auto_shrink([false, false])
                    .show(&mut content, contents);
                let resized_id = id.with("hardware-resized");
                if spec.fit && !ctx.data(|d| d.get_temp::<bool>(resized_id).unwrap_or(false)) {
                    // A short dialog has no empty bay below its last control.
                    let chrome = bounds.height() - body.height();
                    geometry.size.y = (scrolled.content_size.y + chrome)
                        .max(MIN_SIZE.y.min(spec.size.y))
                        .min(spec.viewport.height());
                }
                let before = geometry.size;
                resize_handles(ui, bounds, &mut geometry);
                if geometry.size != before {
                    ctx.data_mut(|d| d.insert_temp(resized_id, true));
                }
                Some(scrolled.inner)
            }
        });
    let top = ctx.memory(|m| {
        if blocking {
            m.top_modal_layer() == Some(layer)
        } else {
            m.top_modal_layer().is_none()
                && m.layer_ids()
                    .filter(|other| other.order == order && m.areas().is_visible(other))
                    .last()
                    == Some(layer)
        }
    });
    if popup_open
        && !egui::Popup::is_any_open(ctx)
        && let Some(focused) = ctx.memory(|m| m.focused())
    {
        ctx.memory_mut(|m| m.surrender_focus(focused));
    }
    if spec.close
        && !collapsed
        && top
        && !popup_open
        && (spec.blocking || !guarded)
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        close = true;
    }
    if blocking && backdrop_clicked && !popup_open {
        close = true;
    }
    if close {
        expand(ctx, spec.identity);
    }
    let wants_keyboard = ctx
        .memory(|m| m.focused())
        .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
    ctx.data_mut(|d| {
        d.insert_temp(gid, constrain(geometry, spec.viewport, collapsed));
        d.insert_temp(id.with("hardware-escape-guard"), wants_keyboard);
    });
    ShellResponse {
        response: shown.response,
        inner: shown.inner,
        help,
        close,
    }
}

fn resize_handles(ui: &mut egui::Ui, rect: Rect, g: &mut Geometry) {
    let edge = 6.0;
    for (index, h, v) in [
        (0, -1, -1),
        (1, 0, -1),
        (2, 1, -1),
        (3, -1, 0),
        (4, 1, 0),
        (5, -1, 1),
        (6, 0, 1),
        (7, 1, 1),
    ] {
        let x = match h {
            -1 => rect.left()..=rect.left() + edge,
            1 => rect.right() - edge..=rect.right(),
            _ => rect.left() + edge..=rect.right() - edge,
        };
        let y = match v {
            -1 => rect.top()..=rect.top() + edge,
            1 => rect.bottom() - edge..=rect.bottom(),
            _ => rect.top() + edge..=rect.bottom() - edge,
        };
        let cursor = if h == 0 {
            egui::CursorIcon::ResizeVertical
        } else if v == 0 {
            egui::CursorIcon::ResizeHorizontal
        } else if h == v {
            egui::CursorIcon::ResizeNwSe
        } else {
            egui::CursorIcon::ResizeNeSw
        };
        let response = ui
            .interact(
                Rect::from_x_y_ranges(x, y),
                ui.id().with(("resize", index)),
                Sense::DRAG,
            )
            .on_hover_cursor(cursor);
        if response.dragged() && !response.drag_started() {
            let delta = ui.input(|i| i.pointer.delta());
            let before = g.size;
            if h != 0 {
                g.size.x = (before.x + delta.x * h as f32).max(MIN_SIZE.x);
            }
            if v != 0 {
                g.size.y = (before.y + delta.y * v as f32).max(MIN_SIZE.y);
            }
            if h < 0 {
                g.position.x += before.x - g.size.x;
            }
            if v < 0 {
                g.position.y += before.y - g.size.y;
            }
            ui.ctx().request_repaint();
        }
    }
    for offset in [0.0, 4.0, 8.0] {
        ui.painter().line_segment(
            [
                rect.right_bottom() - Vec2::new(7.0 + offset, 7.0),
                rect.right_bottom() - Vec2::new(7.0, 7.0 + offset),
            ],
            Stroke::new(1.0_f32, controls::CREAM),
        );
    }
}

pub(super) struct Window<'a> {
    title: &'a str,
    open: Option<&'a mut bool>,
    size: Vec2,
    max_height: f32,
    viewport: Option<Rect>,
}
pub(super) fn window(title: &str) -> Window<'_> {
    Window {
        title,
        open: None,
        size: Vec2::new(460.0, 280.0),
        max_height: f32::INFINITY,
        viewport: None,
    }
}
impl<'a> Window<'a> {
    pub(super) fn open(mut self, open: &'a mut bool) -> Self {
        self.open = Some(open);
        self
    }
    pub(super) fn default_width(mut self, value: f32) -> Self {
        self.size.x = value;
        self
    }
    pub(super) fn default_height(mut self, value: f32) -> Self {
        self.size.y = value + HEADER + 24.0;
        self
    }
    pub(super) fn max_height(mut self, value: f32) -> Self {
        self.max_height = value;
        self
    }
    pub(super) fn constrain_to(mut self, value: Rect) -> Self {
        self.viewport = Some(value);
        self
    }
    pub(super) fn show<R>(
        self,
        ctx: &egui::Context,
        contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<egui::InnerResponse<Option<R>>> {
        if self.open.as_deref() == Some(&false) {
            return None;
        }
        let result = show_shell(
            ctx,
            Shell {
                identity: self.title,
                title: self.title,
                size: Vec2::new(self.size.x, self.size.y.min(self.max_height)),
                viewport: self
                    .viewport
                    .unwrap_or_else(|| ctx.content_rect().shrink(8.0)),
                help: false,
                close: self.open.is_some(),
                blocking: false,
                palette: match self.title {
                    "Time Stretcher help" => controls::Palette::Marine,
                    "Effect Studio help" => controls::Palette::Red,
                    "Step synth help" => controls::Palette::Olive,
                    "Sample editor help" => controls::Palette::Granite,
                    _ => controls::Palette::Utility,
                },
                inset_body: true,
                fit: self.title == "Recorded take",
            },
            contents,
        );
        if result.close
            && let Some(open) = self.open
        {
            *open = false;
        }
        Some(egui::InnerResponse {
            inner: result.inner,
            response: result.response,
        })
    }
}
pub(super) struct ToolResponse {
    pub help: bool,
}
pub(super) fn tool<R>(
    ctx: &egui::Context,
    identity: &str,
    open: &mut bool,
    size: Vec2,
    palette: controls::Palette,
    has_help: bool,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> ToolResponse {
    if !*open {
        return ToolResponse { help: false };
    }
    let result = show_shell(
        ctx,
        Shell {
            identity,
            title: identity,
            size: size + Vec2::new(24.0, HEADER + 24.0),
            viewport: ctx.content_rect().shrink(8.0),
            help: has_help,
            close: true,
            blocking: false,
            palette,
            inset_body: false,
            fit: false,
        },
        contents,
    );
    if result.close {
        *open = false;
    }
    ToolResponse { help: result.help }
}
pub(super) struct ModalResponse {
    close: bool,
}
impl ModalResponse {
    pub(super) fn should_close(&self) -> bool {
        self.close
    }
}
pub(super) fn modal<R>(
    ctx: &egui::Context,
    _id: egui::Id,
    identity: &str,
    title: &str,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> ModalResponse {
    let size = match identity {
        "Sample conflicts" => Vec2::new(760.0, 520.0),
        "Choose path" => Vec2::new(700.0, 330.0),
        "Save your changes?" => Vec2::new(470.0, 160.0),
        _ => Vec2::new(470.0, 190.0),
    };
    let result = show_shell(
        ctx,
        Shell {
            identity,
            title,
            size,
            viewport: ctx.content_rect().shrink(8.0),
            help: false,
            close: true,
            blocking: true,
            palette: controls::Palette::Utility,
            inset_body: true,
            fit: true,
        },
        contents,
    );
    ModalResponse {
        close: result.close,
    }
}
pub(super) fn primary(ui: &mut egui::Ui, label: &str) -> Response {
    controls::button(ui, label, controls::ButtonRole::Primary)
}
pub(super) fn danger(ui: &mut egui::Ui, label: &str) -> Response {
    controls::button(ui, label, controls::ButtonRole::Danger)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn resized_window_collapses_in_place_and_restores_its_size_and_draft() {
        for density in [1.0, 1.5, 2.0] {
            let ctx = egui::Context::default();
            configure_style(&ctx);
            ctx.set_pixels_per_point(density);
            let mut open = true;
            let mut draft = String::from("Unfinished edit");
            let mut body_calls = 0;
            let mut frame = |events: Vec<egui::Event>| {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                        focused: true,
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        window("Resize fixture")
                            .open(&mut open)
                            .default_width(420.0)
                            .show(ctx, |ui| {
                                body_calls += 1;
                                ui.text_edit_singleline(&mut draft);
                            });
                    },
                );
            };
            for _ in 0..3 {
                frame(vec![]);
            }
            let geometry = || {
                ctx.data(|d| {
                    d.get_temp::<Geometry>(
                        egui::Id::new("Resize fixture").with("hardware-geometry"),
                    )
                })
                .unwrap()
            };
            let before = geometry();
            let start = before.position + before.size - Vec2::splat(2.0);
            frame(vec![egui::Event::PointerMoved(start), pointer(start, true)]);
            let end = start + Vec2::new(80.0, 50.0);
            frame(vec![egui::Event::PointerMoved(end)]);
            frame(vec![pointer(end, false)]);
            frame(vec![]);
            let resized = geometry();
            assert!(
                resized.size.x > before.size.x + 60.0,
                "Right edge must resize at {density}x"
            );
            assert!(resized.size.y > before.size.y + 30.0);
            let collapse = resized.position + Vec2::new(24.0, HEADER / 2.0);
            frame(vec![
                egui::Event::PointerMoved(collapse),
                pointer(collapse, true),
            ]);
            frame(vec![pointer(collapse, false)]);
            for _ in 0..3 {
                frame(vec![]);
            }
            assert!(is_collapsed(&ctx, "Resize fixture"));
            let small = ctx
                .memory(|m| m.area_rect(egui::Id::new("Resize fixture")))
                .unwrap();
            assert!((small.height() - HEADER).abs() < 1.0);
            assert_eq!(geometry().position, resized.position);
            assert_eq!(geometry().size, resized.size);
            frame(vec![
                egui::Event::PointerMoved(collapse),
                pointer(collapse, true),
            ]);
            frame(vec![pointer(collapse, false)]);
            for _ in 0..3 {
                frame(vec![]);
            }
            assert!(!is_collapsed(&ctx, "Resize fixture"));
            assert_eq!(geometry().position, resized.position);
            assert_eq!(geometry().size, resized.size);
            assert_eq!(draft, "Unfinished edit");
            assert!(body_calls > 3);
            assert!(open);
        }
    }

    #[test]
    fn shrinking_viewport_keeps_collapsed_header_and_expanded_body_reachable() {
        let original = Geometry {
            position: Pos2::new(900.0, 600.0),
            size: Vec2::new(900.0, 650.0),
        };
        let viewport = Rect::from_min_size(Pos2::new(8.0, 8.0), Vec2::new(944.0, 704.0));
        for collapsed in [false, true] {
            let bounded = constrain(original, viewport, collapsed);
            let visible = Vec2::new(
                bounded.size.x,
                if collapsed { HEADER } else { bounded.size.y },
            );
            assert!(viewport.contains_rect(Rect::from_min_size(bounded.position, visible)));
            assert_eq!(bounded.size, original.size);
        }
    }
}
