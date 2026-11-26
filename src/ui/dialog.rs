//! The shared tool chassis, in the current theme, and window lifecycle for every in-app surface.
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
/// The user resized the window, so it no longer follows its content.
pub(super) fn user_resized(ctx: &egui::Context, identity: &str) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new(identity).with("hardware-resized")))
        .unwrap_or(false)
}
/// Where the window `identity` shows, or last showed.
pub(super) fn window_rect(ctx: &egui::Context, identity: &str) -> Option<Rect> {
    let geometry =
        ctx.data(|d| d.get_temp::<Geometry>(egui::Id::new(identity).with("hardware-geometry")))?;
    let height = if is_collapsed(ctx, identity) {
        HEADER
    } else {
        geometry.size.y
    };
    Some(Rect::from_min_size(
        geometry.position,
        Vec2::new(geometry.size.x, height),
    ))
}
/// Drop the place and the size of the window `identity`, so that it opens
/// at its default place again.
pub(super) fn forget_geometry(ctx: &egui::Context, identity: &str) {
    let id = egui::Id::new(identity);
    ctx.data_mut(|d| {
        d.remove::<Geometry>(id.with("hardware-geometry"));
        d.remove::<bool>(id.with("hardware-resized"));
    });
}
/// Give the window `to` the place, size, and collapse state of the window
/// `from`, for a window that changes its title while it stays open.
pub(super) fn carry_geometry(ctx: &egui::Context, from: &str, to: &str) {
    let (old, new) = (egui::Id::new(from), egui::Id::new(to));
    let collapsed = is_collapsed(ctx, from);
    ctx.data_mut(|d| {
        if let Some(geometry) = d.get_temp::<Geometry>(old.with("hardware-geometry")) {
            d.insert_temp(new.with("hardware-geometry"), geometry);
        }
        if let Some(resized) = d.get_temp::<bool>(old.with("hardware-resized")) {
            d.insert_temp(new.with("hardware-resized"), resized);
        }
        if let Some(fitted) = d.get_temp::<f32>(old.with("hardware-fitted")) {
            d.insert_temp(new.with("hardware-fitted"), fitted);
        }
    });
    set_collapsed(ctx, to, collapsed);
}
/// A window of `size` beside `anchor`: at its right or left when there is
/// room, else above or below it, made shorter to fit when needed.
fn beside(anchor: Rect, size: Vec2, viewport: Rect, min_height: f32) -> Geometry {
    const GAP: f32 = 8.0;
    let top = anchor.top().clamp(
        viewport.top(),
        (viewport.bottom() - size.y).max(viewport.top()),
    );
    if viewport.right() - anchor.right() - GAP >= size.x {
        return Geometry {
            position: Pos2::new(anchor.right() + GAP, top),
            size,
        };
    }
    if anchor.left() - viewport.left() - GAP >= size.x {
        return Geometry {
            position: Pos2::new(anchor.left() - GAP - size.x, top),
            size,
        };
    }
    let left = (anchor.right() - size.x).clamp(
        viewport.left(),
        (viewport.right() - size.x).max(viewport.left()),
    );
    let above = anchor.top() - viewport.top() - GAP;
    let below = viewport.bottom() - anchor.bottom() - GAP;
    if above >= below && above >= min_height {
        let height = size.y.min(above);
        return Geometry {
            position: Pos2::new(left, anchor.top() - GAP - height),
            size: Vec2::new(size.x, height),
        };
    }
    if below >= min_height {
        return Geometry {
            position: Pos2::new(left, anchor.bottom() + GAP),
            size: Vec2::new(size.x, size.y.min(below)),
        };
    }
    Geometry {
        position: viewport.center() - size / 2.0,
        size,
    }
}
pub(super) fn top_window(ctx: &egui::Context, identity: &str) -> bool {
    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new(identity));
    ctx.memory(|memory| {
        memory.top_modal_layer().is_none()
            && memory
                .layer_ids()
                .filter(|other| {
                    other.order == egui::Order::Middle && memory.areas().is_visible(other)
                })
                .last()
                == Some(layer)
    })
}
fn set_collapsed(ctx: &egui::Context, identity: &str, value: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(("hardware-collapsed", identity)), value));
    if value && let Some(id) = ctx.memory(|m| m.focused()) {
        ctx.memory_mut(|m| m.surrender_focus(id));
    }
    ctx.request_repaint();
}

/// The type, spacing, and widget colours of painted hardware in `palette`.
pub(super) fn hardware_style(ui: &mut egui::Ui, palette: controls::Palette) {
    let s = ui.style_mut();
    s.text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(13.0));
    s.text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(13.0));
    s.text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(11.0));
    s.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(18.0, theme::heading_family()),
    );
    s.spacing.item_spacing = Vec2::new(10.0, 10.0);
    s.spacing.button_padding = Vec2::new(12.0, 6.0);
    s.spacing.interact_size.y = 28.0;
    s.wrap_mode = Some(egui::TextWrapMode::Wrap);
    // Components communicate disabled state with their face/indicator. Keep
    // labels readable instead of applying a second opacity fade to the text.
    s.visuals.disabled_alpha = 1.0;
    s.visuals.extreme_bg_color = roles().face;
    s.visuals.faint_bg_color = Color32::from_black_alpha(12);
    s.visuals.widgets.noninteractive.fg_stroke.color = roles().legend;
    s.visuals.widgets.noninteractive.bg_stroke.color = Color32::from_black_alpha(70);
    for w in [
        &mut s.visuals.widgets.inactive,
        &mut s.visuals.widgets.hovered,
        &mut s.visuals.widgets.active,
    ] {
        w.bg_fill = roles().face;
        w.weak_bg_fill = roles().face;
        w.bg_stroke = Stroke::new(1.0_f32, roles().widget_edge);
        w.fg_stroke = Stroke::new(1.0_f32, roles().legend);
        w.corner_radius = egui::CornerRadius::same(roles().corner(3.0) as u8);
    }
    s.visuals.widgets.hovered.bg_fill = roles().widget_hover;
    s.visuals.widgets.hovered.weak_bg_fill = s.visuals.widgets.hovered.bg_fill;
    s.visuals.widgets.active.bg_stroke.color = roles().tools[0].1;
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
    scroll: bool,
    /// The window takes the size of its content on every frame. It has no
    /// resize handles and no scroll bars, and it can extend past the
    /// viewport: only its header must stay in reach.
    fixed: bool,
    focus_controls: Option<fn(&Response)>,
    /// The window opens beside this rectangle, not at the centre.
    anchor: Option<Rect>,
}
struct ShellResponse {
    help: bool,
    close: bool,
}

fn constrain(mut g: Geometry, viewport: Rect, collapsed: bool, min: Vec2) -> Geometry {
    g.size = g.size.max(min).min(viewport.size());
    place(g, viewport, collapsed)
}

/// Keep the window's top-left corner in `viewport`, and all of the window
/// when it fits.
fn place(mut g: Geometry, viewport: Rect, collapsed: bool) -> Geometry {
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

/// One shell window on this frame: its spec, its memory id, and whether it
/// is collapsed.
struct Frame<'s, 'a> {
    ctx: &'s egui::Context,
    spec: &'s Shell<'a>,
    id: egui::Id,
    collapsed: bool,
}

impl Frame<'_, '_> {
    fn key(&self, name: &str) -> egui::Id {
        self.id.with(name)
    }
}

/// The layer of a shell window, and what it knew when the frame started.
struct ShellState {
    blocking: bool,
    layer: egui::LayerId,
    popup_open: bool,
    guarded: bool,
}

fn show_shell<R>(
    ctx: &egui::Context,
    spec: Shell<'_>,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> ShellResponse {
    let frame = Frame {
        ctx,
        spec: &spec,
        id: egui::Id::new(spec.identity),
        collapsed: is_collapsed(ctx, spec.identity),
    };
    let mut min = shell_min(&frame);
    let mut geometry = shell_geometry(&frame, min);
    let state = shell_state(&frame);
    let mut backdrop_clicked = false;
    let mut buttons = ShellResponse {
        help: false,
        close: false,
    };
    egui::Area::new(frame.id)
        .kind(egui::UiKind::Window)
        .order(state.layer.order)
        .fixed_pos(geometry.position)
        .movable(false)
        .sense(Sense::CLICK | Sense::DRAG)
        .fade_in(false)
        .show(ctx, |ui| {
            hardware_style(ui, spec.palette);
            backdrop_clicked = state.blocking && backdrop(ui);
            let bounds = shell_frame(ui, &frame, geometry.size);
            let header = Rect::from_min_size(bounds.min, Vec2::new(bounds.width(), HEADER));
            buttons = shell_header(ui, &frame, header, &mut geometry);
            if !frame.collapsed {
                shell_body(ui, &frame, bounds, (&mut geometry, &mut min), contents);
            }
        });
    if shell_dismissed(&frame, &state, backdrop_clicked) {
        buttons.close = true;
    }
    if buttons.close {
        expand(ctx, spec.identity);
    }
    store_shell(&frame, geometry, min);
    buttons
}

/// The smallest size of the window on this frame. A window that follows its
/// content can be shorter than MIN_SIZE: its floor is the height that its
/// content last needed.
fn shell_min(frame: &Frame) -> Vec2 {
    let mut min = MIN_SIZE;
    if frame.spec.anchor.is_some() {
        // Help can use a short scrolling page above a tool in a compact
        // viewport. Keep the header, body margins, and 44 points of text.
        min.y = HEADER + 80.0;
    }
    if frame.spec.fit
        && let Some(fitted) = frame
            .ctx
            .data(|d| d.get_temp::<f32>(frame.key("hardware-fitted")))
    {
        min.y = fitted.min(MIN_SIZE.y);
    }
    min
}

/// Keep `g` in reach: a fixed window keeps the size of its content, another
/// window also fits its size to the viewport.
fn settle(frame: &Frame, g: Geometry, min: Vec2) -> Geometry {
    if frame.spec.fixed {
        place(g, frame.spec.viewport, frame.collapsed)
    } else {
        constrain(g, frame.spec.viewport, frame.collapsed, min)
    }
}

/// The stored geometry of the window, or where a new window opens.
fn shell_geometry(frame: &Frame, min: Vec2) -> Geometry {
    let spec = frame.spec;
    let mut geometry = frame
        .ctx
        .data(|d| d.get_temp::<Geometry>(frame.key("hardware-geometry")))
        .unwrap_or_else(|| {
            let size = spec.size.max(min).min(spec.viewport.size());
            match spec.anchor {
                Some(anchor) => beside(anchor, size, spec.viewport, min.y),
                None => Geometry {
                    position: spec.viewport.center() - size / 2.0,
                    size,
                },
            }
        });
    if spec.fixed {
        geometry.size = frame
            .ctx
            .data(|d| d.get_temp::<Vec2>(frame.key("hardware-measured")))
            .unwrap_or(spec.size);
    }
    settle(frame, geometry, min)
}

/// The window's layer, raised above the others while it blocks them.
fn shell_state(frame: &Frame) -> ShellState {
    let ctx = frame.ctx;
    let blocking = frame.spec.blocking && !frame.collapsed;
    let order = if frame.spec.blocking {
        egui::Order::Foreground
    } else {
        egui::Order::Middle
    };
    let layer = egui::LayerId::new(order, frame.id);
    if blocking {
        ctx.memory_mut(|m| m.set_modal_layer(layer));
        ctx.move_to_top(layer);
    }
    ShellState {
        blocking,
        layer,
        popup_open: egui::Popup::is_any_open(ctx),
        guarded: ctx
            .data(|d| d.get_temp::<bool>(frame.key("hardware-escape-guard")))
            .unwrap_or(false),
    }
}

/// Dim everything behind a blocking window. Returns whether the user clicked
/// the dimmed area.
fn backdrop(ui: &mut egui::Ui) -> bool {
    let rect = ui.ctx().content_rect();
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
    backdrop.response().clicked()
}

/// Name the window, take its space, and paint its shadow and panel. Returns
/// the window's bounds.
fn shell_frame(ui: &mut egui::Ui, frame: &Frame, size: Vec2) -> Rect {
    let spec = frame.spec;
    let display_title = title_text(spec.title);
    name_dialog(
        ui,
        if spec.blocking {
            spec.identity
        } else {
            &display_title
        },
    );
    let visible = Vec2::new(size.x, if frame.collapsed { HEADER } else { size.y });
    let (bounds, _) = ui.allocate_exact_size(visible, Sense::CLICK | Sense::DRAG);
    frame.ctx.accesskit_node_builder(ui.unique_id(), |node| {
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
        .as_shape(bounds, egui::CornerRadius::same(roles().corner(9.0) as u8)),
    );
    controls::panel(ui, bounds);
    bounds
}

/// A header key's square, `from_right` points from the header's right edge
/// to its centre.
fn header_key(header: Rect, from_right: f32) -> Rect {
    Rect::from_center_size(
        Pos2::new(header.right() - from_right, header.center().y),
        Vec2::splat(28.0),
    )
}

/// The title bar: drag, collapse, the title, and the help and close keys.
fn shell_header(
    ui: &mut egui::Ui,
    frame: &Frame,
    header: Rect,
    geometry: &mut Geometry,
) -> ShellResponse {
    drag_title(ui, frame, header, geometry);
    collapse_key(ui, frame, header);
    paint_title(ui, frame.spec.title, header);
    ShellResponse {
        help: frame.spec.help && help_key(ui, frame.spec.identity, header),
        close: frame.spec.close && close_key(ui, frame, header),
    }
}

fn drag_title(ui: &mut egui::Ui, frame: &Frame, header: Rect, geometry: &mut Geometry) {
    let drag_rect = Rect::from_min_max(
        header.min + Vec2::new(44.0, 2.0),
        header.max - Vec2::new(84.0, 3.0),
    );
    let drag = ui.interact(drag_rect, frame.key("title-drag"), Sense::DRAG);
    if drag.dragged() && !drag.drag_started() {
        geometry.position += frame.ctx.input(|i| i.pointer.delta());
    }
}

fn collapse_key(ui: &mut egui::Ui, frame: &Frame, header: Rect) {
    let left = Rect::from_center_size(
        Pos2::new(header.left() + 24.0, header.center().y),
        Vec2::splat(28.0),
    );
    let (verb, icon) = if frame.collapsed {
        ("Expand", controls::HeaderIcon::Expand)
    } else {
        ("Collapse", controls::HeaderIcon::Collapse)
    };
    let label = format!("{verb} {}", frame.spec.identity);
    let collapse = controls::header_button(ui, left, "collapse", &label, icon);
    if let Some(focus) = frame.spec.focus_controls {
        focus(&collapse);
    }
    if collapse.clicked() {
        set_collapsed(frame.ctx, frame.spec.identity, !frame.collapsed);
    }
}

fn paint_title(ui: &mut egui::Ui, title: &str, header: Rect) {
    let title_rect = Rect::from_min_max(
        header.min + Vec2::new(48.0, 0.0),
        header.max - Vec2::new(88.0, 0.0),
    );
    let mut job = egui::text::LayoutJob::simple_singleline(
        title_text(title),
        FontId::new(18.0, theme::heading_family()),
        roles().legend,
    );
    job.sections[0].format.line_height = Some(18.0 * theme::HEADING_LINE);
    let galley = ui.painter().layout_job(job);
    ui.painter().with_clip_rect(title_rect).galley(
        Pos2::new(
            (header.center().x - galley.size().x / 2.0).max(title_rect.left()),
            header.center().y - galley.size().y / 2.0,
        ),
        galley,
        roles().legend,
    );
}

fn help_key(ui: &mut egui::Ui, identity: &str, header: Rect) -> bool {
    controls::header_button(
        ui,
        header_key(header, 61.0),
        "help",
        &format!("{identity} help"),
        controls::HeaderIcon::Help,
    )
    .clicked()
}

fn close_key(ui: &mut egui::Ui, frame: &Frame, header: Rect) -> bool {
    let close_button = controls::header_button(
        ui,
        header_key(header, 24.0),
        "close",
        &format!("Close {}", frame.spec.identity),
        controls::HeaderIcon::Close,
    );
    if let Some(focus) = frame.spec.focus_controls {
        focus(&close_button);
    }
    close_button.clicked()
}

/// The body under the header: the content, then the fitted height and the
/// resize handles. `sizes` holds the window's geometry and its smallest size.
fn shell_body<R>(
    ui: &mut egui::Ui,
    frame: &Frame,
    bounds: Rect,
    sizes: (&mut Geometry, &mut Vec2),
    contents: impl FnOnce(&mut egui::Ui) -> R,
) {
    let (geometry, min) = sizes;
    let spec = frame.spec;
    let header_bottom = bounds.top() + HEADER;
    ui.painter().line_segment(
        [
            Pos2::new(bounds.left() + 10.0, header_bottom),
            Pos2::new(bounds.right() - 10.0, header_bottom),
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
    let chrome = bounds.size() - body.size();
    if spec.fixed {
        fixed_body(frame, &mut content, chrome, geometry, contents);
        return;
    }
    let scrolled = scrolled_body(&mut content, spec, contents);
    let resized_id = frame.key("hardware-resized");
    let resized = frame
        .ctx
        .data(|d| d.get_temp::<bool>(resized_id).unwrap_or(false));
    if spec.fit && !resized {
        fit_height(frame, &content, &scrolled, chrome.y, (geometry, min));
    }
    let before = geometry.size;
    resize_handles(ui, bounds, geometry, *min);
    if geometry.size != before {
        frame.ctx.data_mut(|d| d.insert_temp(resized_id, true));
    }
}

/// Draw the content of a fixed window, which takes the size of what its
/// content drew.
fn fixed_body<R>(
    frame: &Frame,
    content: &mut egui::Ui,
    chrome: Vec2,
    geometry: &mut Geometry,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) {
    contents(content);
    let measured = (content.min_rect().size() + chrome).round();
    if measured != geometry.size {
        geometry.size = measured;
        *geometry = place(*geometry, frame.spec.viewport, frame.collapsed);
        frame
            .ctx
            .data_mut(|d| d.insert_temp(frame.key("hardware-measured"), measured));
        frame.ctx.request_repaint();
    }
}

/// Draw the content in a scroll area. Solid bars show whenever the body
/// overflows; floating bars stay hidden until hovered. The handle takes the
/// cream text colour, because its default fill is the bay's own face. The
/// content keeps its own style.
fn scrolled_body<R>(
    content: &mut egui::Ui,
    spec: &Shell,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::scroll_area::ScrollAreaOutput<R> {
    let inherited = content.spacing().scroll;
    content.spacing_mut().scroll = egui::style::ScrollStyle {
        foreground_color: true,
        ..egui::style::ScrollStyle::solid()
    };
    let scrolled = egui::ScrollArea::new([spec.scroll; 2])
        .id_salt("hardware-scroll")
        .auto_shrink([false, false])
        .min_scrolled_height(0.0)
        .show(content, |ui| {
            ui.spacing_mut().scroll = inherited;
            contents(ui)
        });
    view::name_scroll_area(content, "hardware-scroll", spec.title);
    scrolled
}

/// A short dialog has no empty bay below its last control.
fn fit_height<R>(
    frame: &Frame,
    content: &egui::Ui,
    scrolled: &egui::scroll_area::ScrollAreaOutput<R>,
    chrome: f32,
    sizes: (&mut Geometry, &mut Vec2),
) {
    let (geometry, min) = sizes;
    let horizontal_bar = if scrolled.content_size.x > scrolled.inner_rect.width() {
        content.spacing().scroll.allocated_width()
    } else {
        0.0
    };
    let fitted =
        (scrolled.content_size.y + chrome + horizontal_bar).min(frame.spec.viewport.height());
    geometry.size.y = fitted;
    min.y = fitted.min(MIN_SIZE.y);
    frame
        .ctx
        .data_mut(|d| d.insert_temp(frame.key("hardware-fitted"), fitted));
}

/// Whether the window is the top one of its kind.
fn shell_on_top(ctx: &egui::Context, layer: egui::LayerId, blocking: bool) -> bool {
    ctx.memory(|m| {
        if blocking {
            m.top_modal_layer() == Some(layer)
        } else {
            m.top_modal_layer().is_none()
                && m.layer_ids()
                    .filter(|other| other.order == layer.order && m.areas().is_visible(other))
                    .last()
                    == Some(layer)
        }
    })
}

/// A popup that closed on this frame gives up the focus it held.
fn release_popup_focus(ctx: &egui::Context, popup_open: bool) {
    if popup_open
        && !egui::Popup::is_any_open(ctx)
        && let Some(focused) = ctx.memory(|m| m.focused())
    {
        ctx.memory_mut(|m| m.surrender_focus(focused));
    }
}

/// Whether Escape closes the window: it must be the top window, and no popup
/// or text field may hold the key.
fn escape_pressed(frame: &Frame, state: &ShellState, top: bool) -> bool {
    let spec = frame.spec;
    spec.close
        && !frame.collapsed
        && top
        && !state.popup_open
        && (spec.blocking || !state.guarded)
        && frame
            .ctx
            .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
}

/// Whether Escape or a click on the backdrop closes the window.
fn shell_dismissed(frame: &Frame, state: &ShellState, backdrop_clicked: bool) -> bool {
    let top = shell_on_top(frame.ctx, state.layer, state.blocking);
    release_popup_focus(frame.ctx, state.popup_open);
    let escaped = escape_pressed(frame, state, top);
    escaped || (state.blocking && backdrop_clicked && !state.popup_open)
}

/// Keep the window's geometry, and whether a text field held the keyboard.
fn store_shell(frame: &Frame, geometry: Geometry, min: Vec2) {
    let ctx = frame.ctx;
    let wants_keyboard = ctx
        .memory(|m| m.focused())
        .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
    let stored = settle(frame, geometry, min);
    ctx.data_mut(|d| {
        d.insert_temp(frame.key("hardware-geometry"), stored);
        d.insert_temp(frame.key("hardware-escape-guard"), wants_keyboard);
    });
}

fn resize_handles(ui: &mut egui::Ui, rect: Rect, g: &mut Geometry, min: Vec2) {
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
                g.size.x = (before.x + delta.x * h as f32).max(min.x);
            }
            if v != 0 {
                g.size.y = (before.y + delta.y * v as f32).max(min.y);
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
            Stroke::new(1.0_f32, roles().legend),
        );
    }
}

pub(super) struct Window<'a> {
    title: &'a str,
    open: Option<&'a mut bool>,
    help: Option<&'a mut bool>,
    size: Vec2,
    max_height: f32,
    viewport: Option<Rect>,
    scroll: bool,
    fit: bool,
    fixed: bool,
    focus_controls: Option<fn(&Response)>,
    palette: controls::Palette,
    anchor: Option<Rect>,
}
pub(super) fn window(title: &str) -> Window<'_> {
    Window {
        title,
        open: None,
        help: None,
        size: Vec2::new(460.0, 280.0),
        max_height: f32::INFINITY,
        viewport: None,
        scroll: true,
        fit: false,
        fixed: false,
        focus_controls: None,
        palette: controls::Palette::Utility,
        anchor: None,
    }
}
impl<'a> Window<'a> {
    /// Show `?` in the header and set `requested` when the user clicks it.
    pub(super) fn help(mut self, requested: &'a mut bool) -> Self {
        self.help = Some(requested);
        self
    }
    /// The chassis palette: a tool's own, for its Help.
    pub(super) fn palette(mut self, palette: controls::Palette) -> Self {
        self.palette = palette;
        self
    }
    /// Open beside `rect` instead of at the centre, until moved.
    pub(super) fn anchor(mut self, rect: Option<Rect>) -> Self {
        self.anchor = rect;
        self
    }
    pub(super) fn focus_controls(mut self, register: fn(&Response)) -> Self {
        self.focus_controls = Some(register);
        self
    }
    /// Let a surface provide its own scrolling body and fixed action row.
    pub(super) fn scroll(mut self, enabled: bool) -> Self {
        self.scroll = enabled;
        self
    }
    /// Follow the height of the content until the user resizes the window.
    pub(super) fn fit(mut self, enabled: bool) -> Self {
        self.fit = enabled;
        self
    }
    /// Take the size of the content on every frame, with no resize handles
    /// and no scroll bars. The window can extend past the screen.
    pub(super) fn fixed(mut self) -> Self {
        self.fixed = true;
        self
    }
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
    pub(super) fn show<R>(self, ctx: &egui::Context, contents: impl FnOnce(&mut egui::Ui) -> R) {
        if self.open.as_deref() == Some(&false) {
            return;
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
                help: self.help.is_some(),
                close: self.open.is_some(),
                blocking: false,
                palette: self.palette,
                inset_body: true,
                fit: self.fit || self.title == "Recorded take",
                scroll: self.scroll,
                fixed: self.fixed,
                focus_controls: self.focus_controls,
                anchor: self.anchor,
            },
            contents,
        );
        if result.help
            && let Some(requested) = self.help
        {
            *requested = true;
        }
        if result.close
            && let Some(open) = self.open
        {
            *open = false;
        }
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
            scroll: true,
            fixed: false,
            focus_controls: None,
            anchor: None,
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
            scroll: true,
            fixed: false,
            focus_controls: None,
            anchor: None,
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
        let ctx = egui::Context::default();
        configure_style(&ctx);
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
                d.get_temp::<Geometry>(egui::Id::new("Resize fixture").with("hardware-geometry"))
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
            "Right edge must resize"
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

    #[test]
    fn shrinking_viewport_keeps_collapsed_header_and_expanded_body_reachable() {
        let original = Geometry {
            position: Pos2::new(900.0, 600.0),
            size: Vec2::new(900.0, 650.0),
        };
        let viewport = Rect::from_min_size(Pos2::new(8.0, 8.0), Vec2::new(944.0, 704.0));
        for collapsed in [false, true] {
            let bounded = constrain(original, viewport, collapsed, MIN_SIZE);
            let visible = Vec2::new(
                bounded.size.x,
                if collapsed { HEADER } else { bounded.size.y },
            );
            assert!(viewport.contains_rect(Rect::from_min_size(bounded.position, visible)));
            assert_eq!(bounded.size, original.size);
        }
    }

    #[test]
    fn short_modal_follows_its_content_below_the_minimum_height() {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        let frame = || {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                    ..Default::default()
                },
                |ctx| {
                    modal(ctx, "Short fixture", "Short fixture", |ui| {
                        ui.label("One line of text.");
                    });
                },
            );
        };
        let height = || {
            ctx.data(|d| {
                d.get_temp::<Geometry>(egui::Id::new("Short fixture").with("hardware-geometry"))
            })
            .unwrap()
            .size
            .y
        };
        for _ in 0..3 {
            frame();
        }
        let fitted = height();
        assert!(
            fitted < MIN_SIZE.y,
            "A short modal keeps an empty bay: {fitted}"
        );
        for _ in 0..3 {
            frame();
        }
        assert_eq!(height(), fitted);
        let painted = ctx
            .memory(|m| m.area_rect(egui::Id::new("Short fixture")))
            .unwrap();
        assert!(
            (painted.height() - fitted).abs() < 1.0,
            "{painted:?} / {fitted}"
        );
    }

    #[test]
    fn fitted_modal_keeps_short_wide_content_above_the_horizontal_scrollbar() {
        for height in [20.0, 40.0, 100.0] {
            let ctx = egui::Context::default();
            configure_style(&ctx);
            ctx.style_mut(|style| style.animation_time = 0.0);
            let mut content = Rect::NOTHING;
            let mut clip = Rect::NOTHING;
            let mut visible_bar = false;
            for _ in 0..12 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
                        ..Default::default()
                    },
                    |ctx| {
                        modal(ctx, "Wide fixture", "Wide fixture", |ui| {
                            content = ui
                                .allocate_exact_size(Vec2::new(800.0, height), Sense::hover())
                                .0;
                            clip = ui.clip_rect();
                        });
                    },
                );
                visible_bar = output.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Rect(rect)
                        if rect.fill == roles().legend
                            && rect.rect.width() > 100.0
                            && rect.rect.height() < 16.0
                            && rect.rect.top() >= content.bottom()
                            && shape.clip_rect.contains_rect(rect.rect))
                });
            }
            assert!(
                content.width() > clip.width(),
                "The fixture must scroll horizontally."
            );
            assert!(
                content.bottom() <= clip.bottom(),
                "A fitted dialog must leave room for its horizontal scrollbar: \
                 {content:?} / {clip:?}"
            );
            assert!(
                visible_bar,
                "The horizontal scrollbar must stay visible below {height}-point content."
            );
        }
    }
}
