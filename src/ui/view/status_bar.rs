//! The status bar at the foot of the window: the status line, the panel
//! switches, and the version.
use super::*;

impl MixJamApp {
    /// Keep the status bar's row across the whole window, under the rail.
    /// Its controls are drawn by `status_bar` after the rest of the window,
    /// so Tab reaches them last.
    pub(in super::super) fn status_bar_space(&self, ctx: &egui::Context) -> Rect {
        egui::TopBottomPanel::bottom("status")
            .exact_height(30.0)
            .frame(egui::Frame::new().fill(roles().canvas))
            .show(ctx, |_| {})
            .response
            .rect
    }

    pub(in super::super) fn status_bar(&mut self, ctx: &egui::Context, rect: Rect) {
        let inner = rect - egui::Margin::symmetric(18, 3);
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("status-bar"),
            egui::UiBuilder::new()
                .layer_id(egui::LayerId::background())
                .max_rect(inner),
        );
        ui.set_clip_rect(rect);
        let ui = &mut ui;
        let panels = egui::RichText::new("Panels")
            .size(12.0)
            .color(roles().muted);
        let version = egui::RichText::new(format!("v{}", mixjam_techno::application::APP_VERSION))
            .size(12.0)
            .color(roles().accent);
        // The right-hand group is drawn left to right, after the status, so
        // Tab reads the bar in the order it is seen; its width is measured
        // first to place it against the right edge.
        let text_width = |ui: &egui::Ui, text: &egui::RichText| {
            egui::WidgetText::from(text.clone())
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Extend),
                    f32::INFINITY,
                    egui::TextStyle::Body,
                )
                .size()
                .x
        };
        let gap = ui.spacing().item_spacing.x;
        let group = text_width(ui, &panels)
            + gap
            + panel_switch_width(ui, "Tools")
            + gap
            + panel_switch_width(ui, "Boards")
            + gap
            + panel_switch_width(ui, "Sounds")
            + gap
            + panel_switch_width(ui, "Overview")
            + gap
            + panel_switch_width(ui, "Inspector")
            + gap
            + STATUS_VERSION_GAP
            + text_width(ui, &version);
        let split = (inner.right() - group).max(inner.left());
        let left = Rect::from_min_max(inner.min, Pos2::new(split - gap, inner.bottom()));
        let right = Rect::from_min_max(Pos2::new(split, inner.top()), inner.max);
        let notice = |ui: &mut egui::Ui, label: &str| {
            ui.add(
                egui::Button::new(egui::RichText::new(label).size(12.0))
                    .min_size(Vec2::new(0.0, 24.0)),
            )
        };
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(left)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                if !self.session.sample_conflicts().is_empty()
                    && notice(ui, "Sample conflicts").clicked()
                {
                    self.conflicts.open = true;
                    self.conflicts.error = None;
                }
                if (!self.session.project().import_notices.is_empty()
                    || !self.session.project().source_notices.is_empty()
                    || !self.session.project().source_settings.is_empty()
                    || !self.session.missing_samples().is_empty())
                    && notice(ui, "Import details").clicked()
                {
                    self.sample_import.details_open = true;
                }
                if !self.session.missing_samples().is_empty() {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} missing · silent",
                            self.session.missing_samples().len()
                        ))
                        .size(12.0)
                        .color(roles().coral),
                    )
                    .hover_hint(
                        "Missing samples play silence. Open Import details to see their \
                             saved references.",
                    );
                }
                let color = if self.status_error {
                    roles().coral
                } else {
                    roles().muted
                };
                if self.session.options().quality_reduced() {
                    // It shortens before it would run under the panel switches.
                    let reduced = "Playback quality reduced; exports use full quality";
                    ui.add(
                        egui::Label::new(egui::RichText::new(reduced).color(roles().warning))
                            .truncate(),
                    )
                    .hover_hint(reduced);
                }
                // The idle text would contradict a missing-sound notice.
                let status = if self.status == DEFAULT_STATUS
                    && !self.session.missing_samples().is_empty()
                {
                    String::new()
                } else {
                    display_paths(&self.status)
                };
                if !status.is_empty() {
                    ui.add(
                        egui::Label::new(egui::RichText::new(&status).size(12.0).color(color))
                            .truncate(),
                    )
                    .hover_hint(&status);
                }
            },
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(right)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.label(panels);
                // In a maximised score all read off; each switch restores the
                // score with the previous panel choices.
                // In a maximised score all read off but Overview; each switch
                // restores the score with the previous panel choices.
                for (command, label, hint) in [
                    (
                        commands::Command::ToolNames,
                        "Tools",
                        "Tool rail: names under the icons, or icons only",
                    ),
                    (
                        commands::Command::Boards,
                        "Boards",
                        "Lower panel: its board, or only its toolbar row",
                    ),
                    (
                        commands::Command::Sounds,
                        "Sounds",
                        "Sounds panel beside the score, or its folded strip",
                    ),
                    (
                        commands::Command::Overview,
                        "Overview",
                        "Song overview above the score, or its named bar",
                    ),
                    (
                        commands::Command::Inspector,
                        "Inspector",
                        "Inspector beside the score, or its folded strip",
                    ),
                ] {
                    if panel_switch(ui, label, self.command_active(command))
                        .hover_hint(hint)
                        .clicked()
                    {
                        self.run_command(command, ctx);
                    }
                }
                ui.add_space(STATUS_VERSION_GAP);
                ui.add(
                    egui::Hyperlink::from_label_and_url(
                        version,
                        "https://github.com/satyrlord/mixjam-techno",
                    )
                    .open_in_new_tab(true),
                )
                .hover_hint("Open MixJam Techno on GitHub");
            },
        );
    }
}

/// A status-bar switch that shows or folds a panel. On, its name is bright
/// and underlined in the accent.
/// Space between the panel switches and the version link.
const STATUS_VERSION_GAP: f32 = 14.0;

/// The width `panel_switch` takes for `label`.
fn panel_switch_width(ui: &egui::Ui, label: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(12.0),
            roles().text,
        )
        .size()
        .x
        + 14.0
}

pub(in super::super) fn panel_switch(ui: &mut egui::Ui, label: &str, on: bool) -> Response {
    let color = if on { roles().text } else { roles().muted };
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), egui::FontId::proportional(12.0), color);
    let size = Vec2::new(panel_switch_width(ui, label), 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let text = Pos2::new(
        rect.center().x - galley.size().x / 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text, galley, color);
    if on {
        ui.painter().rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left() + 4.0, rect.bottom() - 2.0),
                Pos2::new(rect.right() - 4.0, rect.bottom()),
            ),
            0.0,
            roles().accent,
        );
    }
    focus_outline(ui, &response, roles().accent);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label)
    });
    response
}
