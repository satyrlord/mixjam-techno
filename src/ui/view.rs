use super::*;
#[path = "view/boards.rs"]
mod boards;
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
impl HoverHints for Response {
    fn hover_hint_at_pointer(self, text: impl Into<egui::WidgetText>) -> Self {
        let text = text.into();
        self.ctx.accesskit_node_builder(self.id, |node| {
            if node.description().is_none() {
                node.set_description(text.text());
            }
        });
        if self.ctx.data(|data| {
            data.get_temp::<bool>(egui::Id::new("show-hover-hints"))
                .unwrap_or(true)
        }) {
            self.on_hover_text_at_pointer(text)
        } else {
            self
        }
    }
    fn hover_hint(self, text: impl Into<egui::WidgetText>) -> Self {
        let text = text.into();
        self.ctx.accesskit_node_builder(self.id, |node| {
            if node.description().is_none() {
                node.set_description(text.text());
            }
        });
        if self.ctx.data(|data| {
            data.get_temp::<bool>(egui::Id::new("show-hover-hints"))
                .unwrap_or(true)
        }) {
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
        if self.ctx.data(|data| {
            data.get_temp::<bool>(egui::Id::new("show-hover-hints"))
                .unwrap_or(true)
        }) {
            self.on_disabled_hover_text(text)
        } else {
            self
        }
    }
    fn hover_details(self, contents: impl FnOnce(&mut egui::Ui)) -> Self {
        if self.ctx.data(|data| {
            data.get_temp::<bool>(egui::Id::new("show-hover-hints"))
                .unwrap_or(true)
        }) {
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

pub(super) const DEFAULT_STATUS: &str = "Ready";
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
        let (pressed, held) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.any_down()));
        self.session.pointer_gesture(pressed, held);
        // An applied theme repaints the next frame; songs, history, and audio
        // are untouched.
        theme::adopt(ctx, self.session.options().theme);
        self.sections.bookmarks = self.session.options().show_section_bookmarks;
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::new("show-hover-hints"),
                self.session.options().show_tooltips,
            )
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
        self.capture(ctx);
    }
}
