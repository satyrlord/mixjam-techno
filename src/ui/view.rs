use super::*;
#[path = "view/boards.rs"]
mod boards;
/// The id of the lower panel. egui keeps the panel's height under it, so the
/// value stays the name that the panel had when it held only the Sounds list.
pub(super) const LOWER_PANEL_ID: &str = "sounds";
/// Only the library's UI tests use it.
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use boards::MeterDisplay;
pub(super) use boards::{BoardsState, StepSynthBoard};
#[path = "view/clip_edit.rs"]
mod clip_edit;
pub(super) use clip_edit::*;
#[path = "view/clip_paint.rs"]
mod clip_paint;
pub(super) use clip_paint::*;
#[path = "view/header.rs"]
mod header;
pub(super) use header::*;
#[path = "view/lane_automation.rs"]
mod lane_automation;
pub(super) use lane_automation::*;
#[path = "view/rail.rs"]
mod rail;
pub(super) use rail::*;
#[path = "view/score_lanes.rs"]
mod score_lanes;
#[path = "view/score_toolbar.rs"]
mod score_toolbar;
pub(super) use score_toolbar::*;
#[path = "view/sections.rs"]
mod sections;
pub(super) use sections::*;
#[path = "view/status_bar.rs"]
mod status_bar;
pub(super) use status_bar::StatusLog;
#[path = "view/timeline.rs"]
mod timeline;
pub(super) use timeline::ScoreState;
#[path = "view/track_headers.rs"]
mod track_headers;
#[path = "view/widgets.rs"]
mod widgets;
pub(super) use widgets::*;
/// Hover hints can be hidden without removing a control's accessible help.
pub(super) trait HoverHints {
    fn hover_hint(self, text: impl Into<egui::WidgetText>) -> Self;
    fn hover_hint_at_pointer(self, text: impl Into<egui::WidgetText>) -> Self;
    fn disabled_hint(self, text: impl Into<egui::WidgetText>) -> Self;
    fn hover_details(self, contents: impl FnOnce(&mut egui::Ui)) -> Self;
}
/// Where the frame keeps whether hover hints show.
fn hover_hints_id() -> egui::Id {
    egui::Id::new("show-hover-hints")
}

/// Whether hover hints show: the Options page can hide them.
fn hints_shown(response: &Response) -> bool {
    response
        .ctx
        .data(|data| data.get_temp::<bool>(hover_hints_id()))
        .unwrap_or(true)
}

/// Give the control `text` as its accessible description, unless it has one.
fn describe_once(response: &Response, text: &egui::WidgetText) {
    response.ctx.accesskit_node_builder(response.id, |node| {
        if node.description().is_none() {
            node.set_description(text.text());
        }
    });
}

impl HoverHints for Response {
    fn hover_hint_at_pointer(self, text: impl Into<egui::WidgetText>) -> Self {
        let text = text.into();
        describe_once(&self, &text);
        if hints_shown(&self) {
            self.on_hover_text_at_pointer(text)
        } else {
            self
        }
    }
    fn hover_hint(self, text: impl Into<egui::WidgetText>) -> Self {
        let text = text.into();
        describe_once(&self, &text);
        if hints_shown(&self) {
            self.on_hover_text(text)
        } else {
            self
        }
    }
    fn disabled_hint(self, text: impl Into<egui::WidgetText>) -> Self {
        let text = text.into();
        if !self.enabled() {
            self.ctx
                .accesskit_node_builder(self.id, |node| node.set_description(text.text()));
        }
        if hints_shown(&self) {
            self.on_disabled_hover_text(text)
        } else {
            self
        }
    }
    fn hover_details(self, contents: impl FnOnce(&mut egui::Ui)) -> Self {
        if hints_shown(&self) {
            self.on_hover_ui(contents)
        } else {
            self
        }
    }
}

/// Format Windows filesystem paths for labels only. Stored paths and messages
/// retain their original spelling for file operations and diagnostics.
pub(super) fn display_paths(text: &str) -> String {
    let mut display = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(start) = remaining.find(r"\\?\") {
        display.push_str(&remaining[..start]);
        let tail = &remaining[start + 4..];
        if let Some(unc) = tail.strip_prefix(r"UNC\") {
            display.push_str(r"\\");
            remaining = unc;
        } else {
            let bytes = tail.as_bytes();
            if !(bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && bytes[2] == b'\\')
            {
                // Device and volume identifiers have no equivalent drive path.
                display.push_str(r"\\?\");
            }
            remaining = tail;
        }
    }
    display.push_str(remaining);
    display
}

// Colors come from the theme roles in `theme.rs`. Application state and rules
// live outside this module.

/// The idle status line: how to start, until the first notice replaces it.
pub(super) const DEFAULT_STATUS: &str = "Space plays the song. Drag a sound from the Sounds panel \
     onto the score. Ctrl+K lists every command.";
pub(super) const GROUPS: [&str; 10] = [
    "Loop",
    "Drum",
    "Bass",
    "Sequence",
    "Wave",
    "Voice",
    "Effect",
    "Xtra",
    "Sphere",
    "Step synth",
];

pub(super) fn title_text(text: &str) -> String {
    deunicode::deunicode(text).to_ascii_uppercase()
}

pub(super) fn heading(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(title_text(&text.into()))
        .font(FontId::new(size, theme::heading_family()))
        .line_height(Some(size * theme::HEADING_LINE))
        .color(roles().text)
}

/// The product name, in the brand face of every theme.
pub(super) fn brand(text: &str, size: f32) -> egui::RichText {
    egui::RichText::new(title_text(text))
        .font(FontId::new(size, theme::brand_family()))
        .color(roles().text)
}

/// Name a control whose visible label is separate text, for assistive
/// technology and agents.
pub(super) fn name_control(response: Response, name: &str) -> Response {
    response
        .ctx
        .accesskit_node_builder(response.id, |node| node.set_label(name));
    response
}

/// Name the parts of a scroll area that egui makes Tab stops: its bars and
/// its drag surface, each only while it is shown. Call it after the area,
/// in the same `ui`, with the area's `id_salt`.
pub(super) fn name_scroll_area(ui: &egui::Ui, id_salt: impl std::hash::Hash, name: &str) {
    name_scroll_parts(
        ui.ctx(),
        ui.make_persistent_id(egui::Id::new(id_salt)),
        name,
    );
}

/// Name the parts of the scroll area with this egui id that are Tab stops.
fn name_scroll_parts(ctx: &egui::Context, id: egui::Id, name: &str) {
    use egui::accesskit::Role;
    for (part, role, label) in [
        (
            id.with(0_usize),
            Role::ScrollBar,
            format!("{name} horizontal scroll bar"),
        ),
        (
            id.with(1_usize),
            Role::ScrollBar,
            format!("{name} vertical scroll bar"),
        ),
        (
            id.with("area"),
            Role::ScrollView,
            format!("{name} drag to scroll"),
        ),
    ] {
        if ctx
            .read_response(part)
            .is_some_and(|response| response.sense.is_focusable())
        {
            ctx.accesskit_node_builder(part, |node| {
                node.set_role(role);
                node.set_label(label);
            });
        }
    }
}

/// Name the surface of each menu that shows. egui makes the surface of a
/// menu, a context menu, and a drop-down list a Tab stop, and gives it no
/// name. A menu that opens from another menu gets a number, because names on
/// screen together are unique. A surface with keyboard focus gets the Accent
/// outline of the other controls. The list of a combo box scrolls when it is
/// long, and its scroll parts get the name of the menu. Call it after the
/// last menu of the frame.
fn name_open_menus(ctx: &egui::Context) {
    let layers: Vec<_> = ctx.memory(|memory| {
        memory
            .layer_ids()
            .filter(|layer| layer.order == egui::Order::Foreground)
            .collect()
    });
    let mut menus = 0;
    for layer in layers {
        // egui's own id for the surface of an area.
        let surface = layer.id.with("move");
        let shown =
            ctx.graphics(|graphics| graphics.get(layer).is_some_and(|list| !list.is_empty()));
        if shown
            && let Some(response) = ctx.read_response(surface)
            && response.sense.is_focusable()
        {
            menus += 1;
            if response.has_focus() {
                // Over the rows of the menu, in the layer of the menu.
                ctx.layer_painter(layer).rect_stroke(
                    response.rect.shrink(1.0),
                    ctx.style().visuals.menu_corner_radius,
                    Stroke::new(2.0_f32, roles().accent),
                    egui::StrokeKind::Inside,
                );
            }
            let name = if menus == 1 {
                "Menu".to_owned()
            } else {
                format!("Menu {menus}")
            };
            ctx.accesskit_node_builder(surface, |node| {
                node.set_role(egui::accesskit::Role::Menu);
                node.set_label(name.as_str());
            });
            // egui's own id for the scroll area of a combo box list: in the
            // frame of the menu, with the default salt of a scroll area.
            let list = layer
                .id
                .with(egui::Id::new("child"))
                .with(egui::Id::new("scroll_area"));
            name_scroll_parts(ctx, list, &name);
        }
    }
}

/// Show a menu that can be taller than egui's default area height. egui gives
/// a new menu that height at most, and a combo box has no setting for it. A
/// taller list would cut its last rows and scroll. The window still limits
/// the menu.
pub(super) fn tall_menu<R>(ctx: &egui::Context, show: impl FnOnce() -> R) -> R {
    let mut height = f32::INFINITY;
    ctx.style_mut(|style| {
        std::mem::swap(&mut style.spacing.default_area_size.y, &mut height);
    });
    let shown = show();
    ctx.style_mut(|style| style.spacing.default_area_size.y = height);
    shown
}

/// Name a pointer surface that is not a Tab stop. egui describes only
/// focusable widgets by itself; the widget information adds the bounds.
pub(super) fn name_region(response: Response, name: &str) -> Response {
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, name));
    response
}

/// Name a painted surface with the widget role that the caller selects.
pub(super) fn named(response: &Response, kind: egui::WidgetType, text: &str) {
    response.widget_info(|| egui::WidgetInfo::labeled(kind, response.enabled(), text));
}

/// Group the controls of a modal under its title, for assistive technology
/// and agents.
pub(super) fn name_dialog(ui: &egui::Ui, title: &str) {
    ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
        node.set_role(egui::accesskit::Role::Dialog);
        node.set_label(title);
    });
}

impl MixJamApp {
    pub(super) fn draw(&mut self, ctx: &egui::Context) {
        self.poll_application(ctx);
        let (pressed, held) = ctx.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.primary_down(),
            )
        });
        self.session.pointer_gesture(pressed, held);
        // An applied theme repaints the next frame; songs, history, and audio
        // are untouched.
        theme::adopt(ctx, self.session.options().theme);
        self.sections.bookmarks = self.session.options().show_section_bookmarks;
        ctx.data_mut(|data| {
            data.insert_temp(hover_hints_id(), self.session.options().show_tooltips)
        });
        if ctx.input(|i| i.viewport().close_requested())
            && !self.files.allow_close
            && (self.session.unsaved()
                || self.session.has_recorded_take()
                || self.session.is_recording())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request_action(PendingAction::Close, ctx);
        }
        self.capture_keyboard(ctx);
        if self.blocking_modal(ctx).is_none() {
            self.keyboard(ctx);
        }
        self.file_drop(ctx);
        self.header(ctx);
        let status = self.status_bar_space(ctx);
        self.measure_room(ctx);
        self.side_rail(ctx);
        self.bottom(ctx);
        self.sounds_side(ctx);
        self.inspector_side(ctx);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(roles().canvas)
                    .inner_margin(egui::Margin::symmetric(18, 14)),
            )
            .show(ctx, |ui| self.timeline(ui));
        self.status_bar(ctx, status);
        self.finish_drag(ctx);
        self.dialogs(ctx);
        self.tool_windows(ctx);
        self.command_list(ctx);
        self.sample_conflicts_dialog(ctx);
        self.path_prompt_dialog(ctx);
        name_open_menus(ctx);
        self.capture(ctx);
    }
}
