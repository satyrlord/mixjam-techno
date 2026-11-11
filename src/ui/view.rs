use super::*;
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
use mixjam_techno::application::options::WorkspaceLayout;
use mixjam_techno::application::{
    Fade, FadeCurve, MAX_PITCH_CENTS, MAX_SECTION_NAME, MAX_TICKS, Section, amplitude_to_db,
    db_to_amplitude,
};

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
/// The song name's size at the left of the transport, and the floor that it
/// shrinks to before an absurdly long name goes below it.
const TRANSPORT_TITLE_SIZE: f32 = 15.0;
const TRANSPORT_TITLE_FLOOR: f32 = 9.0;
/// The transport row inside the panel's 10-point vertical margins.
const TRANSPORT_ROW_HEIGHT: f32 = 48.0;
/// Transport buttons are square. In a compact window they narrow when the song
/// name cannot otherwise keep its full size.
const TRANSPORT_BUTTON: f32 = 42.0;
const TRANSPORT_BUTTON_NARROW: f32 = 34.0;
const TRANSPORT_BUTTON_COUNT: f32 = 8.0;
/// The position readout's container in the top bar, and in a narrow window.
const POSITION_SIZE: Vec2 = Vec2::new(112.0, 42.0);
const POSITION_SIZE_COMPACT: Vec2 = Vec2::new(76.0, 40.0);
/// The top bar: the transport row inside its vertical margins.
const TOP_BAR_MARGIN: i8 = 8;
const TOP_BAR_HEIGHT: f32 = TRANSPORT_ROW_HEIGHT + 2.0 * TOP_BAR_MARGIN as f32;
/// Below this width the top bar packs tighter: a stacked brand, narrower
/// buttons, and no bar and beat beside the position.
const TOP_BAR_COMPACT_WIDTH: f32 = 1240.0;

/// Section bookmark colors, in section order; a song with more sections
/// repeats them.
pub(super) fn section_color(index: usize) -> Color32 {
    let sections = &roles().sections;
    sections[index % sections.len()]
}

/// The timeline zoom: the Zoom slider's range and the factor of one wheel step.
const ZOOM_RANGE: std::ops::RangeInclusive<f32> = 0.55..=2.0;
const ZOOM_WHEEL_STEP: f32 = 1.1;
/// A clip gain at or below this level is silence.
const CLIP_SILENCE_DB: f32 = -60.0;

/// A clip's gain in dB, from `CLIP_SILENCE_DB` for silence to +6 dB.
pub(super) fn clip_gain_db(gain: f32) -> f32 {
    if gain > 0.0 {
        amplitude_to_db(gain).max(CLIP_SILENCE_DB)
    } else {
        CLIP_SILENCE_DB
    }
}

/// A clip's gain amplitude from dB. `CLIP_SILENCE_DB` and below are silence.
pub(super) fn clip_gain_from_db(db: f32) -> f32 {
    if db <= CLIP_SILENCE_DB {
        0.0
    } else {
        db_to_amplitude(db).min(2.0)
    }
}
pub(super) const DEFAULT_STATUS: &str = "Ready";
/// Fits the longest module name, MASTERING, in each theme's 16-point heading
/// face beside the combo arrow.
pub(super) const BOTTOM_MODULE_WIDTH: f32 = 168.0;
/// The lower panel's margins. The bottom margin is smaller than the top one
/// because the boards end in their own scroll bar or inset.
const BOTTOM_PANEL_MARGIN: egui::Margin = egui::Margin {
    left: 18,
    right: 18,
    top: 10,
    bottom: 6,
};
/// The height of the lower panel's toolbar row.
const TOOLBAR_HEIGHT: f32 = 28.0;
/// The space between the module toolbar and a board (Mixer, Mastering, or
/// Effects). The margins give it up, so a board keeps its height.
const BOARD_GAP: f32 = 8.0;
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
    /// Load the bundled artwork, again when the display density changes. A
    /// failure is reported once; the header then falls back to text actions
    /// and the tool rail stays hidden.
    fn icons(&mut self, ctx: &egui::Context) -> Option<Arc<RailIcons>> {
        if self.rail_icons.as_ref().is_none_or(|loaded| {
            loaded
                .as_ref()
                .is_ok_and(|icons| !icons.matches_density(ctx))
        }) {
            let loaded = RailIcons::load(ctx).map(Arc::new);
            if let Err(error) = &loaded {
                self.error(error.clone());
            }
            self.rail_icons = Some(loaded);
        }
        self.rail_icons.as_ref()?.as_ref().ok().cloned()
    }

    /// The top bar across the window: the brand, the song name, the
    /// transport, the recording track, the position, the tempo, and Undo,
    /// Redo and Help at the right end.
    pub(super) fn header(&mut self, ctx: &egui::Context) {
        let icons = self.icons(ctx);
        egui::TopBottomPanel::top("header")
            .exact_height(TOP_BAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(roles().canvas)
                    .inner_margin(egui::Margin::symmetric(18, TOP_BAR_MARGIN)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let compact = ui.available_width() < TOP_BAR_COMPACT_WIDTH;
                    ui.spacing_mut().item_spacing.x = if compact { 4.0 } else { 6.0 };
                    if let Some(icons) = &icons {
                        icons.brand_image(ui);
                    }
                    stacked_brand(ui, compact);
                    // A narrow bar gives the separator's room to the name.
                    if compact {
                        ui.add_space(6.0);
                    } else {
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(2.0);
                    }
                    self.transport(ui, icons.as_deref(), compact);
                });
            });
    }

    pub(super) fn side_rail(&mut self, ctx: &egui::Context) {
        let Some(icons) = self.icons(ctx) else {
            return;
        };
        let layout = self.session.layout();
        if layout.maximised {
            return;
        }
        let names = layout.tool_names;
        egui::SidePanel::left("tools")
            .exact_width(rail_width(names))
            .resizable(false)
            .frame(egui::Frame::new().fill(roles().canvas).inner_margin(6))
            .show(ctx, |ui| {
                controls::apply_palette(ui, controls::Palette::Utility);
                controls::rail_panel(ui, ui.max_rect().expand(4.0));
                ui.spacing_mut().item_spacing.y = 2.0;
                // Options sits at the foot. Its place is kept first, but it is
                // drawn last, so Tab still reaches the tools from the top.
                let area = ui.available_rect_before_wrap();
                let foot_height = RAIL_FOOT_GAP + rail_button_size(names).y;
                let (body, foot) = area.split_top_bottom_at_y(area.bottom() - foot_height);
                ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("tool-rail")
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| self.rail_tools(ui, ctx, &icons, names));
                        });
                });
                ui.scope_builder(egui::UiBuilder::new().max_rect(foot), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(RAIL_FOOT_GAP - 8.0);
                        controls::rail_separator(ui);
                        if rail_tool(ui, &icons, RailIcon::Options, self.options.is_some(), names)
                            .clicked()
                        {
                            self.open_options(ctx);
                        }
                    });
                });
            });
    }

    /// The rail's tools in their groups: File, Clips and Tools. With names
    /// off, each group heading is a separator.
    fn rail_tools(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        icons: &RailIcons,
        names: bool,
    ) {
        let tool = |ui: &mut egui::Ui, icon: RailIcon, active: bool| {
            rail_tool(ui, icons, icon, active, names)
        };
        if rail_names_switch(ui, names).clicked() {
            self.set_layout(|layout| layout.tool_names = !layout.tool_names);
        }
        rail_group(ui, "File", names, true);
        if tool(ui, RailIcon::New, false).clicked() {
            self.request_action(PendingAction::New, ctx);
        }
        if tool(ui, RailIcon::Open, false).clicked() {
            self.request_action(PendingAction::Open, ctx);
        }
        if tool(ui, RailIcon::Save, false).clicked() {
            self.save(false);
        }
        if tool(ui, RailIcon::SaveAs, false).clicked() {
            self.save(true);
        }
        if tool(ui, RailIcon::Import, false).clicked() {
            self.import_wav();
        }
        if tool(ui, RailIcon::Export, false).clicked() {
            self.export();
        }
        rail_group(ui, "Clips", names, false);
        if tool(ui, RailIcon::Erase, self.erase).clicked() {
            self.erase = !self.erase;
        }
        if tool(ui, RailIcon::Delete, false).clicked() {
            self.delete_selection();
        }
        rail_group(ui, "Tools", names, false);
        // Each module button shows its module, or Sounds again when its
        // module already shows. A folded lower panel opens again.
        for (icon, module) in [
            (RailIcon::Mixer, BottomModule::Mixer),
            (RailIcon::Master, BottomModule::Mastering),
        ] {
            let shown = self.bottom_module == module && self.session.layout().boards;
            if tool(ui, icon, shown).clicked() {
                self.bottom_module = if shown { BottomModule::Sounds } else { module };
                self.set_layout(|layout| layout.boards = true);
            }
        }
        if tool(ui, RailIcon::Effects, self.show_effects).clicked() {
            self.show_effects = !self.show_effects;
        }
        if tool(ui, RailIcon::StepSynth, self.show_step_synth).clicked() {
            self.show_step_synth = !self.show_step_synth;
        }
        if tool(ui, RailIcon::Edit, self.show_sample_edit).clicked() {
            self.show_sample_edit = !self.show_sample_edit;
            if self.show_sample_edit {
                self.edit_start = 0;
                self.edit_end = 0;
            }
        }
        if tool(ui, RailIcon::Stretch, self.show_stretch).clicked() {
            self.show_stretch = !self.show_stretch;
            if self.show_stretch {
                self.stretch_source_bpm = self.source_bpm().clamp(30.0, 300.0);
            } else {
                self.close_stretch_request();
            }
        }
        if tool(ui, RailIcon::AlbumReview, self.show_album_review).clicked() {
            self.show_album_review = !self.show_album_review;
        }
    }

    /// Change the workspace layout and keep it for later sessions.
    pub(super) fn set_layout(&mut self, change: impl FnOnce(&mut WorkspaceLayout)) {
        let mut layout = self.session.layout();
        change(&mut layout);
        if let Err(error) = self.session.set_layout(layout) {
            self.error(error);
        }
    }

    /// Keep the status bar's row across the whole window, under the rail.
    /// Its controls are drawn by `status_bar` after the rest of the window,
    /// so Tab reaches them last.
    pub(super) fn status_bar_space(&self, ctx: &egui::Context) -> Rect {
        egui::TopBottomPanel::bottom("status")
            .exact_height(30.0)
            .frame(egui::Frame::new().fill(roles().canvas))
            .show(ctx, |_| {})
            .response
            .rect
    }

    pub(super) fn status_bar(&mut self, ctx: &egui::Context, rect: Rect) {
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
                    self.show_sample_conflicts = true;
                    self.sample_conflict_error = None;
                }
                if (!self.session.project().import_notices.is_empty()
                    || !self.session.project().source_notices.is_empty()
                    || !self.session.project().source_settings.is_empty()
                    || !self.session.missing_samples().is_empty())
                    && notice(ui, "Import details").clicked()
                {
                    self.show_import_details = true;
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
                    ui.colored_label(
                        roles().warning,
                        "Playback quality reduced; exports use full quality",
                    );
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
                let layout = self.session.layout();
                let shown = !layout.maximised;
                if panel_switch(ui, "Tools", shown && layout.tool_names)
                    .hover_hint("Tool rail: names under the icons, or icons only")
                    .clicked()
                {
                    self.set_layout(|layout| {
                        if shown {
                            layout.tool_names = !layout.tool_names;
                        }
                        layout.maximised = false;
                    });
                }
                if panel_switch(ui, "Boards", shown && layout.boards)
                    .hover_hint("Lower panel: its board, or only its toolbar row")
                    .clicked()
                {
                    self.set_layout(|layout| {
                        if shown {
                            layout.boards = !layout.boards;
                        }
                        layout.maximised = false;
                    });
                }
                if panel_switch(ui, "Sounds", self.sounds_panel_open())
                    .hover_hint(
                        "Sounds panel beside the score, or its folded strip. Opening it \
                         changes the lower panel's Sounds module to Mixer.",
                    )
                    .clicked()
                {
                    if !shown {
                        self.set_layout(|layout| layout.maximised = false);
                    } else if self.sounds_panel_open() {
                        self.set_layout(|layout| layout.sounds = false);
                    } else {
                        self.open_sounds_panel();
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

    pub(super) fn bottom(&mut self, ctx: &egui::Context) {
        let layout = self.session.layout();
        if layout.maximised {
            return;
        }
        if layout.boards {
            self.lower_panel(ctx);
        } else {
            // Folded: only the toolbar row, so the module choice and the
            // filters stay in reach. Its own panel id keeps the open panel's
            // height for when it opens again.
            egui::TopBottomPanel::bottom("boards-folded")
                .exact_height(
                    TOOLBAR_HEIGHT
                        + f32::from(BOTTOM_PANEL_MARGIN.top)
                        + f32::from(BOTTOM_PANEL_MARGIN.bottom),
                )
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(roles().panel)
                        .inner_margin(BOTTOM_PANEL_MARGIN),
                )
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    self.bottom_toolbar(ui);
                });
        }
    }

    fn lower_panel(&mut self, ctx: &egui::Context) {
        let height = (ctx.content_rect().height() * 0.28).clamp(220.0, 290.0);
        // Reserve the score toolbar, its ruler and a usable lane viewport.
        let available = ctx.available_rect().height();
        let min_height = self.bottom_module.minimum_height(available);
        // Keep the resized section stable when switching to a shorter module.
        let shared_minimum = BottomModule::ALL
            .into_iter()
            .map(|module| module.minimum_height(available))
            .fold(0.0_f32, f32::max);
        let max_height = (available - 290.0).max(shared_minimum);
        let sounds = egui::TopBottomPanel::bottom("sounds")
            .default_height(height)
            .height_range(min_height..=max_height)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(roles().panel)
                    .inner_margin(BOTTOM_PANEL_MARGIN),
            )
            .show(ctx, |ui| {
                ui.set_min_height(ui.available_height());
                let row_spacing = ui.spacing().item_spacing.y;
                ui.spacing_mut().item_spacing.y = 0.0;
                self.bottom_toolbar(ui);
                ui.spacing_mut().item_spacing.y = row_spacing;
                if self.bottom_module != BottomModule::Sounds {
                    ui.add_space(BOARD_GAP);
                }
                match self.bottom_module {
                    BottomModule::Mixer => self.mixer(ui),
                    BottomModule::Mastering => self.mastering(ui),
                    BottomModule::Effects => self.effects_module(ui),
                    BottomModule::Sounds => self.palette(ui),
                }
            });
        let grip = Rect::from_center_size(
            Pos2::new(
                sounds.response.rect.center().x,
                sounds.response.rect.top() + 2.0,
            ),
            Vec2::new(48.0, 3.0),
        );
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(grip, 2.0, roles().muted);
    }

    pub(super) fn transport(
        &mut self,
        ui: &mut egui::Ui,
        icons: Option<&RailIcons>,
        compact: bool,
    ) {
        // The controls after the name keep their own widths: the name gets
        // the rest of the row, measured in the previous pass, and is never
        // clipped. Its slot keeps room for the unsaved mark, so the first
        // edit does not move the controls.
        let rest_id = egui::Id::new(("transport-rest-width", compact));
        let rest = ui.ctx().data(|data| data.get_temp::<f32>(rest_id));
        let available = ui.available_width();
        let room =
            |button: f32| available - rest.unwrap_or_default() - TRANSPORT_BUTTON_COUNT * button;
        let title = &self.session.project().title;
        let sizing = format!("{title} *");
        let mut button = TRANSPORT_BUTTON;
        let mut fit = transport_title_fit(ui, &sizing, room(button));
        if compact && fit.size < TRANSPORT_TITLE_SIZE {
            button = TRANSPORT_BUTTON_NARROW;
            fit = transport_title_fit(ui, &sizing, room(button));
        }
        let width = room(button);
        let slot = transport_title_galley(ui, &sizing, fit, width).size();
        let shown = if self.session.unsaved() {
            sizing.clone()
        } else {
            title.clone()
        };
        let galley = transport_title_galley(ui, &shown, fit, width);
        // Under a name on one line at full size, the song's length, where it
        // fits the name's room.
        let meta = (fit.size >= TRANSPORT_TITLE_SIZE && galley.rows.len() == 1)
            .then(|| song_meta(self.session.project()))
            .flatten()
            .map(|text| {
                ui.painter()
                    .layout_no_wrap(text, FontId::proportional(11.0), roles().muted)
            })
            .filter(|meta| meta.size().x <= slot.x);
        let block = galley.size().y + meta.as_ref().map_or(0.0, |meta| meta.size().y + 1.0);
        let (name, response) =
            ui.allocate_exact_size(Vec2::new(slot.x, slot.y.max(30.0)), Sense::hover());
        let top = name.center().y - block / 2.0;
        let title_height = galley.size().y;
        ui.painter()
            .galley(Pos2::new(name.left(), top), galley, roles().text);
        if let Some(meta) = meta {
            ui.painter().galley(
                Pos2::new(name.left(), top + title_height + 1.0),
                meta,
                roles().muted,
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, true, title_text(&shown))
        });
        ui.add_space(if compact { 4.0 } else { 12.0 });
        let sections = self.section_bookmarks && !self.session.project().sections.is_empty();
        if transport_button(
            ui,
            button,
            TransportIcon::Loop,
            self.session.project().loop_region.is_some(),
            if sections {
                "Loop the section at the playhead. Drag the bar ruler to loop a range."
            } else {
                "Loop the whole song. Drag the bar ruler to loop a range."
            },
        )
        .clicked()
        {
            let project = self.session.project();
            let region = if project.loop_region.is_some() {
                None
            } else if sections {
                project.section_span(self.session.cursor_tick())
            } else {
                Some([0, project.end_tick().max(TICKS_PER_BAR)])
            };
            let result = self.session.edit(EditCommand::SetLoop(region));
            self.apply(result);
        }
        if transport_button(ui, button, TransportIcon::Start, false, "Go to start").clicked() {
            self.seek(0);
        }
        if transport_button(ui, button, TransportIcon::Back, false, "Back one bar").clicked() {
            self.seek(self.session.cursor_tick().saturating_sub(TICKS_PER_BAR));
        }
        if transport_button(ui, button, TransportIcon::Stop, false, "Stop").clicked() {
            self.stop();
        }
        let play = transport_button(
            ui,
            button,
            if self.session.wants_playback() {
                TransportIcon::Pause
            } else {
                TransportIcon::Play
            },
            self.session.wants_playback(),
            "Play / pause (Space)",
        );
        if self.capture_transport_focus {
            play.request_focus();
            self.capture_focus_ready = play.has_focus();
            if !self.capture_focus_ready {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        if play.clicked() {
            if self.session.wants_playback() {
                self.pause();
            } else {
                self.start_playback();
            }
        }
        if transport_button(ui, button, TransportIcon::Forward, false, "Forward one bar").clicked()
        {
            self.seek(self.session.cursor_tick().saturating_add(TICKS_PER_BAR));
        }
        if transport_button(ui, button, TransportIcon::End, false, "Go to end").clicked() {
            self.seek(self.session.project().end_tick());
        }
        if transport_button(
            ui,
            button,
            TransportIcon::Record,
            self.session.is_recording(),
            "Record into the armed track",
        )
        .clicked()
        {
            if self.session.is_recording() {
                self.stop();
            } else {
                self.start_recording();
            }
        }
        // The recording track sits beside Record.
        if let Some(icons) = icons {
            icon_heading(
                ui,
                icons,
                RailIcon::Record,
                if compact { 18.0 } else { 24.0 },
            );
        }
        let occupied = occupied_tracks(self.session.project());
        recording_destination(ui, &mut self.armed_track, &occupied);
        ui.add_space(if compact { 2.0 } else { 10.0 });
        transport_position(
            ui,
            if compact {
                POSITION_SIZE_COMPACT
            } else {
                POSITION_SIZE
            },
            self.session.cursor_tick(),
            self.session.project().bpm,
            !compact,
        );
        ui.add_space(if compact { 2.0 } else { 10.0 });
        ui.label(egui::RichText::new("BPM").color(roles().muted));
        let mut bpm = self.session.project().bpm;
        if name_control(
            bpm_picker(
                ui,
                "Project BPM",
                &mut bpm,
                if compact { 64.0 } else { 88.0 },
            ),
            "Project BPM",
        )
        .changed()
        {
            let result = self.session.edit(EditCommand::SetTempo(bpm));
            self.apply(result);
        }
        // Undo, Redo and Help against the right edge.
        let action_width = if compact { 32.0 } else { 40.0 };
        let help_gap = if compact { 4.0 } else { 8.0 };
        let spacing = ui.spacing().item_spacing.x;
        let actions = 3.0 * action_width + 2.0 * spacing + help_gap;
        // Remember what follows the name, less the transport buttons. A
        // changed width redoes this pass before it is shown.
        let measured = ui.min_rect().right() - name.right() - TRANSPORT_BUTTON_COUNT * button
            + spacing
            + actions;
        ui.add_space((ui.available_width() - actions).max(0.0));
        let (can_undo, can_redo) = (
            self.session.has_history_step(false),
            self.session.has_history_step(true),
        );
        let action = |ui: &mut egui::Ui, icon: RailIcon, label: &str, enabled: bool| match icons {
            Some(icons) => {
                action_icon_button(ui, icons, icon, label, enabled, icon.hint(), action_width)
            }
            None => action_button(ui, label, false).hover_hint(label),
        };
        if action(ui, RailIcon::Undo, "Undo", can_undo).clicked() {
            self.history_step(false);
        }
        if action(ui, RailIcon::Redo, "Redo", can_redo).clicked() {
            self.history_step(true);
        }
        ui.add_space(help_gap);
        if action(ui, RailIcon::Help, "Help", true).clicked() {
            self.show_help = !self.show_help;
        }
        if rest.is_none_or(|rest| (rest - measured).abs() > 0.5) {
            ui.ctx()
                .data_mut(|data| data.insert_temp(rest_id, measured));
            ui.ctx().request_discard("The transport layout changed.");
        }
    }

    /// The score's toolbar: its heading and counts, then the automation lane,
    /// Follow, Sections, Zoom and Maximise against the right edge.
    fn score_toolbar(&mut self, ui: &mut egui::Ui) {
        // Keep the toolbar in one row beside the Sounds panel, so its
        // controls do not take height from the lane viewport.
        let narrow = ui.available_width() < 760.0;
        let counts = format!(
            "{} tracks  /  {} clips",
            occupied_tracks(self.session.project()).len(),
            self.session.project().clips.len()
        );
        ui.horizontal(|ui| {
            if narrow {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(heading("Arrangement", 12.0));
                    ui.label(egui::RichText::new(&counts).size(10.0).color(roles().muted));
                });
            } else {
                ui.label(heading("Arrangement", 18.0));
                ui.add_space(10.0);
                ui.label(egui::RichText::new(&counts).size(12.0).color(roles().muted));
            }
            let compact = ui.available_width() < 700.0;
            // The controls' width is measured in the previous pass; a change
            // redoes this pass before it is shown.
            let width_id = ui.id().with(("score-toolbar-width", compact));
            let width = ui.ctx().data(|data| data.get_temp::<f32>(width_id));
            ui.add_space((ui.available_width() - width.unwrap_or_default()).max(16.0));
            let start = ui.cursor().left();
            self.automation_selector(ui, !narrow);
            ui.add_space(if compact { 4.0 } else { 10.0 });
            if ui
                .toggle_value(&mut self.follow_playhead, "Follow")
                .hover_hint("Keep the playhead in view during playback and seeking.")
                .changed()
            {
                self.last_follow_position = None;
            }
            if ui
                .toggle_value(&mut self.section_bookmarks, "Sections")
                .hover_hint(
                    "Show the song's section bookmarks on the bar ruler and the lanes. With \
                     sections shown, double-click the bar ruler to add a section, double-click \
                     a name to rename it, right-click a name to delete it, and Loop loops the \
                     section at the playhead.",
                )
                .changed()
                && let Err(error) = self
                    .session
                    .remember_section_bookmarks(self.section_bookmarks)
            {
                self.section_bookmarks = self.session.options().show_section_bookmarks;
                self.error(error);
            }
            ui.add_space(if compact { 4.0 } else { 10.0 });
            ui.label(egui::RichText::new("Zoom").size(12.0).color(roles().muted));
            ui.spacing_mut().slider_width = if compact { 48.0 } else { 72.0 };
            let zoom = name_control(
                ui.add(
                    egui::Slider::new(&mut self.zoom, ZOOM_RANGE)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Always),
                ),
                "Zoom",
            );
            // An egui slider ignores the wheel; each notch scales the zoom by
            // a tenth, as the other wheel-driven controls take one step.
            let turn = controls::wheel(ui, &zoom);
            if turn != 0.0 {
                self.zoom = (self.zoom * ZOOM_WHEEL_STEP.powf(turn))
                    .clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end());
            }
            let maximised = self.session.layout().maximised;
            if maximise_button(ui, maximised).clicked() {
                self.set_layout(|layout| layout.maximised = !layout.maximised);
            }
            let measured = ui.min_rect().right() - start;
            if width.is_none_or(|width| (width - measured).abs() > 0.5) {
                ui.ctx()
                    .data_mut(|data| data.insert_temp(width_id, measured));
                ui.ctx().request_discard("The score toolbar changed.");
            }
        });
    }

    fn bottom_toolbar(&mut self, ui: &mut egui::Ui) {
        let mut picked = false;
        ui.push_id("bottom-module-toolbar", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().interact_size.y = TOOLBAR_HEIGHT;
                let button_padding = ui.spacing().button_padding;
                ui.spacing_mut().button_padding.y = 3.0;
                // ComboBox width is a minimum. Bound its available space as
                // well, so a longer module name cannot move adjacent controls.
                let response = ui
                    .allocate_ui(Vec2::new(BOTTOM_MODULE_WIDTH, 28.0), |ui| {
                        egui::ComboBox::from_id_salt("bottom_module")
                            .selected_text(heading(self.bottom_module.label(), 16.0))
                            .width(BOTTOM_MODULE_WIDTH)
                            .truncate()
                            .show_ui(ui, |ui| {
                                for module in BottomModule::ALL {
                                    if ui
                                        .selectable_value(
                                            &mut self.bottom_module,
                                            module,
                                            module.label(),
                                        )
                                        .clicked()
                                    {
                                        picked = true;
                                        ui.close();
                                    }
                                }
                            })
                            .response
                    })
                    .inner;
                // Choosing a module opens a folded lower panel.
                if picked {
                    self.set_layout(|layout| layout.boards = true);
                }
                name_control(response, "Bottom module").hover_hint(
                    "Choose the module shown in this section. Sounds selects source audio; Mixer \
                     adjusts track levels, pan, EQ, sends, polarity and outputs; Mastering \
                     processes the whole mix on the master pedalboard; Effects holds the four FX \
                     buses that the sends feed.",
                );
                ui.spacing_mut().button_padding = button_padding;
                match self.bottom_module {
                    BottomModule::Sounds => self.palette_filters(ui),
                    BottomModule::Mastering => self.mastering_toolbar(ui),
                    BottomModule::Mixer => self.mixer_toolbar(ui),
                    BottomModule::Effects => {}
                }
            });
        });
    }

    /// The Mixer's toolbar: Export tracks, which writes each track and FX
    /// return to its own WAV file for mixing elsewhere.
    fn mixer_toolbar(&mut self, ui: &mut egui::Ui) {
        let response = ui.add_enabled(
            !self.session.is_exporting(),
            egui::Button::new(egui::RichText::new("Export tracks…").size(13.0)),
        );
        if name_control(response, "Export tracks")
            .hover_hint(
                "Write each track and each FX return to its own stereo WAV file in a folder, \
                 for mixing in another application. Tracks play alone after their EQ, \
                 automation, fader, pan, and outputs, without their sends; the FX returns \
                 hold every send. The master pedalboard is left out. Together the files sum \
                 to the mix with the board bypassed when that sum does not clip.",
            )
            .disabled_hint("Wait for the current export to finish.")
            .clicked()
        {
            self.ask_path(FileRequest::ExportTracks);
        }
    }

    fn palette_filters(&mut self, ui: &mut egui::Ui) {
        let (samples, groups) = self.palette_view();
        self.sound_search(ui, 170.0);
        self.library_filter(ui, 145.0, false);
        self.category_filter(ui, 105.0, &groups, false);
        self.sort_order(ui, 105.0);
        self.sort_direction(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{} sounds", samples.len()))
                    .size(12.0)
                    .color(roles().muted),
            );
        });
    }

    pub(super) fn palette(&mut self, ui: &mut egui::Ui) {
        let (samples, _) = self.palette_view();
        ui.add_space(7.0);
        egui::ScrollArea::horizontal()
            .id_salt("sound_categories")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if category_button(ui, "All", self.group.is_none()).clicked() {
                        self.group = None;
                    }
                    for name in GROUPS {
                        if category_button(ui, name, self.group.as_deref() == Some(name)).clicked()
                        {
                            self.group = Some(name.into());
                        }
                    }
                });
            });
        ui.add_space(10.0);
        if samples.is_empty() {
            ui.add_space(12.0);
            self.no_sounds(ui, 17.0);
        }
        // Tile width follows the bar count, so rows hold a varying number of tiles.
        let row_width = ui.available_width();
        let starts = {
            let library = self.session.library().samples;
            tile_rows(
                samples
                    .iter()
                    .map(|&index| tile_width(library[index].bars, row_width)),
                row_width,
            )
        };
        let spacing =
            std::mem::replace(&mut ui.spacing_mut().item_spacing, Vec2::new(TILE_GAP, 6.0));
        egui::ScrollArea::vertical()
            .id_salt("sample_palette")
            .max_height(ui.available_height())
            .show_rows(ui, 46.0, starts.len(), |ui, rows| {
                for row in rows {
                    let end = starts.get(row + 1).copied().unwrap_or(samples.len());
                    ui.horizontal(|ui| {
                        for &sample_index in &samples[starts[row]..end] {
                            let (id, response) = {
                                let sample = &self.session.library().samples[sample_index];
                                let product = self.sample_product(sample.id);
                                let response = ui
                                    .push_id(sample.id, |ui| {
                                        sample_button(
                                            ui,
                                            sample,
                                            product,
                                            self.selected_sample == Some(sample.id),
                                            tile_width(sample.bars, row_width),
                                        )
                                    })
                                    .inner;
                                if self.capture_view_is("sample-details")
                                    && self.selected_sample == Some(sample.id)
                                {
                                    response.show_tooltip_text(sample_details(sample, product));
                                }
                                (sample.id, response)
                            };
                            self.sound_actions(ui, id, &response);
                        }
                    });
                }
            });
        ui.spacing_mut().item_spacing = spacing;
    }

    pub(super) fn mixer(&mut self, ui: &mut egui::Ui) {
        let mut tracks = self.session.project().tracks.clone();
        let mut master = self.session.project().master_gain;
        let mut buses = self.session.project().fx;
        let mut board = self.session.project().master;
        let changes = ui
            .push_id(self.master_view_generation, |ui| {
                mixer::show(
                    ui,
                    self.session.project(),
                    self.session.library(),
                    &mut tracks,
                    &mut master,
                    &mut buses,
                    &mut board.bypass,
                )
            })
            .inner;
        if changes.tracks {
            let result = self.session.edit(EditCommand::SetMixer {
                tracks,
                master_gain: master,
            });
            self.apply(result);
            ui.ctx().request_repaint();
        }
        if changes.fx {
            let result = self.session.edit(EditCommand::SetEffects(buses));
            self.apply(result);
            ui.ctx().request_repaint();
        }
        if changes.bypass {
            let result = self.session.edit(EditCommand::SetMaster(board));
            self.apply(result);
            ui.ctx().request_repaint();
        }
        if let Some(bus) = changes.open {
            self.bottom_module = BottomModule::Effects;
            self.fx_reveal = Some(bus);
            ui.ctx().request_repaint();
        }
        if let Some(track) = changes.eq {
            self.eq_track = Some(track);
            ui.ctx().request_repaint();
        }
    }

    fn mastering_toolbar(&mut self, ui: &mut egui::Ui) {
        if let Some(chain) = master::preset_selector(ui, &self.session.project().master) {
            let result = self.session.edit(EditCommand::SetMaster(chain));
            self.apply(result);
        }
        let mut chain = self.session.project().master;
        if master::bypass_switch(ui, &mut chain.bypass) {
            let result = self.session.edit(EditCommand::SetMaster(chain));
            self.apply(result);
        }
        let (status, path) = master::status(&self.session.project().master);
        let room = (ui.available_width() - 40.0).max(0.0);
        ui.allocate_ui(Vec2::new(room, 28.0), |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&status).size(12.0)).truncate())
                .hover_hint(path);
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
            if controls::header_button(
                ui,
                rect,
                "mastering-help",
                "Mastering help",
                controls::HeaderIcon::Help,
            )
            .clicked()
            {
                self.show_master_help = true;
            }
        });
    }

    pub(super) fn effects_module(&mut self, ui: &mut egui::Ui) {
        let reveal = self.fx_reveal.take();
        let buses = self.session.project().fx;
        let action = ui
            .push_id(self.master_view_generation, |ui| {
                fx::show(ui, &buses, reveal)
            })
            .inner;
        match action {
            fx::Action::Change(buses) => {
                let result = self.session.edit(EditCommand::SetEffects(*buses));
                self.apply(result);
            }
            fx::Action::ClearTail(bus) => self.session.clear_fx_tail(bus),
            fx::Action::None => {}
        }
    }

    pub(super) fn mastering(&mut self, ui: &mut egui::Ui) {
        // A capture of the meters shows a fixed, representative reading.
        let sample = self.capture_view_is("master-metering");
        let metering = master::Metering {
            reduced: self.session.options().meter_display_rate
                == mixjam_techno::application::options::MeterDisplayRate::Reduced,
            reading: if sample {
                MeterReading {
                    run: 1,
                    input_vu: -1.5,
                    input_peaks: [false, true],
                    input_peak_counts: [0, 1],
                    compressor_reduction: 1.7,
                    limiter_reduction: 2.6,
                    momentary: -12.6,
                    short_term: -13.4,
                    integrated: -13.9,
                    true_peak: -2.4,
                    true_peak_max: -0.8,
                    overs: 1,
                }
            } else {
                self.meter_display.sample(
                    self.session.master_meters(),
                    self.session.options().meter_display_rate
                        == mixjam_techno::application::options::MeterDisplayRate::Reduced,
                    ui.input(|input| input.time),
                )
            },
            playing: sample || self.session.playing_mix(),
        };
        let reveal = self.master_reveal.take();
        let chain = self.session.project().master;
        let action = ui
            .push_id(self.master_view_generation, |ui| {
                master::show(ui, &chain, &metering, reveal)
            })
            .inner;
        if let master::Action::Change(chain) = action {
            let result = self.session.edit(EditCommand::SetMaster(chain));
            self.apply(result);
        }
    }

    /// Exact values for one clip, in its context menu. Drags on the clip's
    /// edges, fade handles, and gain line change the same values.
    fn clip_properties(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some(clip) = self
            .session
            .project()
            .clips
            .iter()
            .find(|c| c.id == id)
            .cloned()
        else {
            return;
        };
        let Some(max_frames) = self
            .session
            .library()
            .sample_by_id(clip.sample_id)
            .map(|sample| sample.frame_count)
        else {
            return;
        };
        ui.separator();
        // The menu style hides widget frames; the value fields need theirs.
        ui.style_mut().visuals.widgets = ui.ctx().style().visuals.widgets.clone();
        let mut gain = clip.gain;
        let mut offset = clip.offset_frames;
        let mut length = clip.length_ticks;
        let mut pitch = clip.pitch_cents;
        let mut reverse = clip.reverse;
        let mut change = false;
        let mut fades = clip.fades;
        egui::Grid::new("clip_properties")
            .num_columns(2)
            .spacing(Vec2::new(10.0, 6.0))
            .show(ui, |ui| {
                // The mixer shows dB, so the clip does too. An untouched value
                // keeps its exact saved amplitude.
                ui.label("Gain (dB)");
                let mut db = clip_gain_db(gain);
                if name_control(
                    ui.add(
                        egui::DragValue::new(&mut db)
                            .range(CLIP_SILENCE_DB..=amplitude_to_db(2.0))
                            .speed(0.1)
                            .custom_formatter(|value, _| {
                                if value <= f64::from(CLIP_SILENCE_DB) {
                                    "-inf".into()
                                } else {
                                    format!("{value:+.1}")
                                }
                            })
                            .custom_parser(|text| {
                                let text = text.trim().to_lowercase();
                                let text = text.trim_end_matches("db").trim();
                                if matches!(text, "-inf" | "-infinity" | "silence") {
                                    Some(f64::from(CLIP_SILENCE_DB))
                                } else {
                                    text.parse().ok()
                                }
                            }),
                    ),
                    "Clip gain",
                )
                .hover_hint("Clip gain in dB. 0 is unity; -inf is silence; the maximum is +6 dB.")
                .changed()
                {
                    gain = clip_gain_from_db(db);
                    change = true;
                }
                ui.end_row();
                ui.label("Trim start (frames)");
                change |= name_control(
                    ui.add(
                        egui::DragValue::new(&mut offset)
                            .range(0..=max_frames.saturating_sub(1))
                            .speed(100),
                    ),
                    "Clip trim start (frames)",
                )
                .hover_hint("Source frames that the clip skips. The clip does not move.")
                .changed();
                ui.end_row();
                ui.label("Length (bars)");
                let mut bars = length as f64 / TICKS_PER_BAR as f64;
                if name_control(
                    ui.add(
                        egui::DragValue::new(&mut bars)
                            .range(1.0 / 16.0..=256.0)
                            .speed(1.0 / 16.0),
                    ),
                    "Clip length (bars)",
                )
                .changed()
                {
                    length = (bars * TICKS_PER_BAR as f64).round().max(1.0) as u32;
                    change = true;
                }
                ui.end_row();
                // Apply the model's shortening order before the controls
                // constrain their individual ranges.
                fades = fades.fit(length);
                let room = length.saturating_sub(fades.fade_out.ticks);
                change |= fade_controls(ui, "in", &mut fades.fade_in, room);
                ui.end_row();
                let room = length.saturating_sub(fades.fade_in.ticks);
                change |= fade_controls(ui, "out", &mut fades.fade_out, room);
                ui.end_row();
                change |= pitch_controls(ui, &mut pitch);
                ui.end_row();
                ui.label("Reverse");
                change |= name_control(ui.checkbox(&mut reverse, "Play backwards"), "Clip reverse")
                    .hover_hint(
                        "Play the clip's sound backwards within the clip, for reversed swells \
                         and risers. Its fades, pitch, and gain stay where they are.",
                    )
                    .changed();
                ui.end_row();
            });
        if change {
            let result = self.session.edit(EditCommand::SetClip {
                id,
                start_tick: clip.start_tick,
                gain,
                offset_frames: offset,
                length_ticks: length,
                fades,
                pitch_cents: pitch,
                reverse,
            });
            self.apply(result);
        }
    }

    /// Open the lane context menu for the selected clip, at the clip, as a
    /// right-click there does. Shift+F10 asks for it.
    fn open_clip_menu(&mut self, ctx: &egui::Context, response: &Response, rect: Rect) {
        let Some(clip) = self.selected_clip.and_then(|id| {
            self.session
                .project()
                .clips
                .iter()
                .find(|clip| clip.id == id)
        }) else {
            return;
        };
        self.context_clip = Some(clip.id);
        self.context_missing_sample = self
            .session
            .missing_sample(clip.sample_id)
            .map(|missing| missing.id);
        self.context_anchor = self.clip_rect(clip, rect).left_bottom();
        egui::Popup::open_id(ctx, egui::Popup::default_response_id(response));
    }

    /// The clip and handle under `pos`. A clip whose sound is missing has none.
    pub(super) fn clip_handle_at(&self, pos: Pos2, rect: Rect) -> Option<(u64, ClipHandle)> {
        let clip = self
            .session
            .project()
            .clips
            .iter()
            .rev()
            .find(|clip| self.clip_rect(clip, rect).contains(pos))?;
        let sample = self.session.library().sample_by_id(clip.sample_id)?;
        let handle = handle_at(
            pos,
            self.clip_rect(clip, rect),
            self.timeline_drop_rect.unwrap_or(rect),
            clip,
            sample,
            self.timeline_bar_width,
            self.session
                .project()
                .sample_playback_bpm(sample.id, sample.source_bpm),
        )?;
        Some((clip.id, handle))
    }

    /// Start, follow, and finish a drag of a clip handle. The clip shows the
    /// change during the drag, and the release makes one edit.
    fn handle_drag_step(&mut self, ui: &egui::Ui, response: &Response, rect: Rect) {
        if self.handle_drag.is_none()
            && response.drag_started_by(egui::PointerButton::Primary)
            && !ui.input(|i| i.modifiers.alt || i.modifiers.shift)
            && let Some(pos) = ui.input(|i| i.pointer.press_origin())
            && let Some((id, handle)) = self.clip_handle_at(pos, rect)
            && let Some(clip) = self
                .session
                .project()
                .clips
                .iter()
                .find(|clip| clip.id == id)
                .cloned()
        {
            if !self.selected_clips.contains(&id) {
                self.select_clip(id, false);
            }
            self.place_on_click = false;
            self.handle_drag = Some(HandleDrag {
                handle,
                origin: pos,
                preview: clip.clone(),
                clip,
            });
        }
        let Some(drag) = &self.handle_drag else {
            return;
        };
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            let preview = self.reshape(drag, pos, rect, ui.input(|i| i.modifiers.ctrl));
            drag_label(ui.ctx(), pos, &handle_readout(drag.handle, &preview));
            if let Some(drag) = self.handle_drag.as_mut() {
                drag.preview = preview;
            }
        }
        if (response.drag_stopped() || ui.input(|i| i.pointer.any_released()))
            && let Some(HandleDrag { clip, preview, .. }) = self.handle_drag.take()
            && preview != clip
        {
            let result = self.session.edit_discrete(EditCommand::SetClip {
                id: preview.id,
                start_tick: preview.start_tick,
                gain: preview.gain,
                offset_frames: preview.offset_frames,
                length_ticks: preview.length_ticks,
                fades: preview.fades,
                pitch_cents: preview.pitch_cents,
                reverse: preview.reverse,
            });
            self.apply(result);
        }
    }

    /// The dragged clip with its handle at `pos`. Edges snap to bars, or to
    /// sixteenths with Ctrl; fades snap to sixteenths, or to ticks with Ctrl.
    /// The clip's neighbors and its sound bound the edges.
    fn reshape(&self, drag: &HandleDrag, pos: Pos2, rect: Rect, fine: bool) -> Clip {
        let clip = &drag.clip;
        let mut shaped = clip.clone();
        let Some(sample) = self.session.library().sample_by_id(clip.sample_id) else {
            return shaped;
        };
        let tick = f64::from((pos.x - rect.left()).max(0.0) / self.timeline_bar_width)
            * f64::from(TICKS_PER_BAR);
        let snap = |ticks: f64, step: u32| {
            ((ticks / f64::from(step)).round() * f64::from(step)).max(0.0) as u32
        };
        let (edge_step, fade_step) = if fine {
            (TICKS_PER_BAR / 16, 1)
        } else {
            (TICKS_PER_BAR, TICKS_PER_BAR / 16)
        };
        let end = clip.start_tick + clip.length_ticks;
        let shortest = (TICKS_PER_BAR / 16).min(clip.length_ticks);
        let others = self
            .session
            .project()
            .clips
            .iter()
            .filter(|other| other.track == clip.track && other.id != clip.id);
        match drag.handle {
            ClipHandle::Start => {
                // The left edge can reveal the trimmed start of the sound.
                let frames_per_tick = frames_per_tick(
                    sample,
                    self.session
                        .project()
                        .sample_playback_bpm(sample.id, sample.source_bpm),
                );
                let revealable = if frames_per_tick > 0.0 {
                    (f64::from(clip.offset_frames) / frames_per_tick) as u32
                } else {
                    0
                };
                let previous_end = others
                    .filter(|other| other.start_tick < clip.start_tick)
                    .map(|other| other.start_tick + other.length_ticks)
                    .max()
                    .unwrap_or(0)
                    .min(clip.start_tick);
                let first = previous_end.max(clip.start_tick.saturating_sub(revealable));
                let remaining = sample
                    .frame_count
                    .saturating_sub(1)
                    .saturating_sub(clip.offset_frames);
                let trimmable = if frames_per_tick > 0.0 {
                    (f64::from(remaining) / frames_per_tick) as u32
                } else {
                    0
                };
                let last = (end - shortest).min(clip.start_tick.saturating_add(trimmable));
                let start = snap(tick, edge_step).clamp(first, last);
                let moved = f64::from(start) - f64::from(clip.start_tick);
                shaped.start_tick = start;
                shaped.length_ticks = end - start;
                shaped.offset_frames = (f64::from(clip.offset_frames) + moved * frames_per_tick)
                    .round()
                    .clamp(0.0, f64::from(sample.frame_count.saturating_sub(1)))
                    as u32;
                shaped.fades = clip.fades.fit(shaped.length_ticks);
            }
            ClipHandle::End => {
                let next_start = others
                    .filter(|other| other.start_tick > clip.start_tick)
                    .map(|other| other.start_tick.max(end))
                    .min()
                    .unwrap_or(MAX_TICKS);
                let new_end = snap(tick, edge_step).clamp(clip.start_tick + shortest, next_start);
                shaped.length_ticks = new_end - clip.start_tick;
                shaped.fades = clip.fades.fit(shaped.length_ticks);
            }
            ClipHandle::FadeIn => {
                shaped.fades.fade_in.ticks = snap(tick - f64::from(clip.start_tick), fade_step)
                    .min(clip.length_ticks - clip.fades.fade_out.ticks);
            }
            ClipHandle::FadeOut => {
                let sound_end = clip.start_tick
                    + sound_ticks(
                        clip,
                        sample,
                        self.session
                            .project()
                            .sample_playback_bpm(sample.id, sample.source_bpm),
                    );
                shaped.fades.fade_out.ticks = snap(f64::from(sound_end) - tick, fade_step)
                    .min(clip.length_ticks - clip.fades.fade_in.ticks);
            }
            ClipHandle::Gain => {
                // Each `GAIN_PIXELS` of travel up or down is one dB, or a
                // tenth with Ctrl, from the saved gain.
                let steps = ((drag.origin.y - pos.y) / GAIN_PIXELS).trunc();
                if steps != 0.0 {
                    let step = if fine { 0.1 } else { 1.0 };
                    shaped.gain = clip_gain_from_db(
                        (clip_gain_db(clip.gain) + steps * step).clamp(CLIP_SILENCE_DB, 6.0),
                    );
                }
            }
        }
        shaped
    }

    pub(super) fn timeline(&mut self, ui: &mut egui::Ui) {
        if !self.section_bookmarks {
            self.cancel_section_edit();
        }
        self.score_toolbar(ui);
        ui.add_space(12.0);
        self.timeline_rect = None;
        self.timeline_drop_rect = None;
        self.timeline_ruler_rect = None;
        let occupied = occupied_tracks(self.session.project());
        let occupied_count = occupied.len();
        self.timeline_add_track =
            if self.armed_track < TRACK_COUNT && !occupied.contains(&self.armed_track) {
                Some(self.armed_track)
            } else {
                (0..TRACK_COUNT).find(|index| !occupied.contains(index))
            };
        self.timeline_tracks = occupied;
        self.timeline_tracks.extend(self.timeline_add_track);
        self.timeline_row_by_track.fill(None);
        for (row, &track) in self.timeline_tracks.iter().enumerate() {
            self.timeline_row_by_track[track] = Some(row);
        }
        let visible_tracks = self.timeline_tracks.clone();
        let add_track = self.timeline_add_track;
        let row_count = visible_tracks.len();
        // The Lane height option sets it; it stays the same at any window or
        // panel size, so more room shows more rows.
        let row_height = self.session.options().lane_height.points();
        let bar_width = 62.0 * self.zoom;
        self.timeline_row_height = row_height;
        self.timeline_bar_width = bar_width;
        let track_height = row_height * row_count as f32;
        let cursor_tick = self.session.cursor_tick();
        let project = self.session.project();
        let last_section = project
            .sections
            .last()
            .map_or(0, |section| section.start_tick);
        let bars =
            (project.end_tick().max(cursor_tick).max(last_section) / TICKS_PER_BAR + 16).max(64);
        let width = bars as f32 * bar_width;
        let label_width = 122.0;
        let lane_inset = label_width + ui.spacing().item_spacing.x;
        let viewport_height = (ui.available_height() - 26.0 - 8.0).max(80.0);
        let (ruler_strip, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
        let body_left = ruler_strip.left();
        let mut route_change = None;
        let mut painted_offset_x = 0.0;
        let mut scroll_area = egui::ScrollArea::both()
            .id_salt("arrangement_scroll")
            .scroll_source(egui::scroll_area::ScrollSource {
                drag: false,
                ..Default::default()
            })
            .auto_shrink([false, false])
            .max_height(viewport_height);
        // Only a position or geometry change can move a paused viewport. Manual
        // scrolling stays put until the playhead moves or Follow is enabled again.
        let visible_width =
            (ui.available_width() - lane_inset - ui.spacing().scroll.allocated_width()).max(1.0);
        let follow_position = (cursor_tick, bar_width, visible_width);
        if self.follow_playhead && self.last_follow_position != Some(follow_position) {
            let id = ui.make_persistent_id(egui::Id::new("arrangement_scroll"));
            let offset = egui::scroll_area::State::load(ui.ctx(), id)
                .unwrap_or_default()
                .offset
                .x;
            let playhead = cursor_tick as f32 / TICKS_PER_BAR as f32 * bar_width;
            if let Some(offset) = follow_offset(playhead, offset, visible_width, width) {
                scroll_area = scroll_area.horizontal_scroll_offset(offset);
            }
        }
        self.last_follow_position = Some(follow_position);
        if self.capture_last_track {
            scroll_area = scroll_area.vertical_scroll_offset(track_height);
            self.capture_last_track = false;
        }
        let scroll = scroll_area.show_viewport(ui, |ui, viewport| {
            painted_offset_x = viewport.left();
            let (whole, _) =
                ui.allocate_exact_size(Vec2::new(width + lane_inset, track_height), Sense::hover());
            let rect = Rect::from_min_size(
                whole.min + Vec2::new(lane_inset, 0.0),
                Vec2::new(width, track_height),
            );
            let canvas_clip = ui.clip_rect().intersect(Rect::from_min_max(
                Pos2::new(body_left + lane_inset, whole.top()),
                Pos2::new(ui.clip_rect().right(), whole.bottom()),
            ));
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(rect).id_salt("lanes"),
                |ui| {
                    ui.set_clip_rect(canvas_clip);
                    let response = name_control(
                        ui.interact(rect, ui.id().with("canvas"), Sense::click_and_drag()),
                        "Arrangement lanes",
                    );
                    if response.has_focus() {
                        self.lane_keys(ui, &response, rect);
                    }
                    if std::mem::take(&mut self.clip_menu_key) {
                        self.open_clip_menu(ui.ctx(), &response, rect);
                    }
                    self.timeline_rect = Some(rect);
                    // A clip paints its own name, so only a missing sound has a
                    // tooltip. The canvas is one widget, so it follows the pointer.
                    if response.hovered()
                        && let Some(pos) = ui.input(|i| i.pointer.latest_pos())
                        && let Some(clip) = self
                            .session
                            .project()
                            .clips
                            .iter()
                            .rev()
                            .find(|clip| self.clip_rect(clip, rect).contains(pos))
                        && let Some(missing) = self.session.missing_sample(clip.sample_id)
                    {
                        response.clone().hover_hint_at_pointer(format!(
                            "{}\n{}\nRight-click to resolve. Import details has copyable \
                             metadata.",
                            missing_sample_details(missing),
                            missing_clip_details(clip)
                        ));
                    }
                    self.timeline_drop_rect = Some(rect.intersect(ui.clip_rect()));
                    let painter = ui.painter_at(rect).with_clip_rect(canvas_clip);
                    painter.rect_filled(rect, 0.0, roles().canvas);
                    for track in 0..occupied_count {
                        let row = Rect::from_min_size(
                            Pos2::new(rect.left(), rect.top() + track as f32 * row_height),
                            Vec2::new(width, row_height),
                        );
                        painter.rect_filled(
                            row,
                            0.0,
                            if track % 2 == 0 {
                                roles().canvas
                            } else {
                                roles().row_alternate
                            },
                        );
                        painter.line_segment(
                            [row.left_bottom(), row.right_bottom()],
                            Stroke::new(1.0_f32, roles().line.gamma_multiply(0.6)),
                        );
                    }
                    let visible = ui.clip_rect().intersect(rect);
                    let first_bar = ((visible.left() - rect.left()) / bar_width)
                        .floor()
                        .max(0.0) as u32;
                    let last_bar = ((visible.right() - rect.left()) / bar_width)
                        .ceil()
                        .min(bars as f32) as u32;
                    for bar in first_bar..=last_bar {
                        let x = rect.left() + bar as f32 * bar_width;
                        painter.line_segment(
                            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                            Stroke::new(
                                1.0_f32,
                                if bar % 4 == 0 {
                                    roles().line
                                } else {
                                    roles().bar_line
                                },
                            ),
                        );
                    }
                    if add_track.is_some() {
                        let target = Rect::from_min_size(
                            Pos2::new(rect.left(), rect.top() + occupied_count as f32 * row_height),
                            Vec2::new(width, row_height),
                        )
                        .intersect(canvas_clip);
                        let hint = painter.layout_no_wrap(
                            "Drop a sound here".into(),
                            FontId::proportional(13.0),
                            roles().muted,
                        );
                        if target.height() >= hint.size().y + 2.0 {
                            // Clear the bar lines behind the hint.
                            let area = Rect::from_center_size(target.center(), hint.size());
                            painter.rect_filled(
                                area.expand2(Vec2::new(6.0, 1.0)),
                                0.0,
                                roles().canvas,
                            );
                            painter.galley(area.min, hint, roles().muted);
                        }
                    }
                    let pointer = ui.input(|i| i.pointer.latest_pos());
                    // Clip handles work while the lanes edit clips.
                    let handles = !self.automating() && !self.erase;
                    if handles {
                        self.handle_drag_step(ui, &response, rect);
                    } else {
                        self.handle_drag = None;
                    }
                    let hovered_handle = pointer
                        .filter(|_| handles && response.hovered() && self.drag.is_none())
                        .and_then(|pos| self.clip_handle_at(pos, rect));
                    if let Some(handle) = self
                        .handle_drag
                        .as_ref()
                        .map(|drag| drag.handle)
                        .or(hovered_handle.map(|(_, handle)| handle))
                    {
                        ui.ctx().set_cursor_icon(handle.cursor());
                    }
                    let preview = self.handle_drag.as_ref().map(|drag| &drag.preview);
                    // The clip that shows its fade handles: the reshaped or hovered one.
                    let shows_handles = preview.map(|clip| clip.id).or_else(|| {
                        pointer
                            .filter(|_| handles && response.hovered())
                            .and_then(|pos| {
                                self.session
                                    .project()
                                    .clips
                                    .iter()
                                    .rev()
                                    .find(|clip| self.clip_rect(clip, rect).contains(pos))
                                    .map(|clip| clip.id)
                            })
                    });
                    for clip in &self.session.project().clips {
                        let clip = preview.filter(|p| p.id == clip.id).unwrap_or(clip);
                        let clip_rect = self.clip_rect(clip, rect);
                        if visible.intersects(clip_rect) {
                            let sample = self.session.library().sample_by_id(clip.sample_id);
                            draw_clip(
                                &painter,
                                clip_rect,
                                sample,
                                self.session.missing_sample(clip.sample_id),
                                self.selected_clips.contains(&clip.id),
                                clip.reverse,
                            );
                            let bpm = sample.map_or(self.session.project().bpm, |sample| {
                                self.session
                                    .project()
                                    .sample_playback_bpm(sample.id, sample.source_bpm)
                            });
                            draw_fades(&painter, clip_rect, clip, sample, bar_width, bpm);
                            let handles = shows_handles == Some(clip.id);
                            if handles || self.selected_clips.contains(&clip.id) {
                                draw_gain_line(&painter, clip_rect, clip.gain);
                            }
                            if handles && let Some(sample) = sample {
                                draw_fade_handles(
                                    &painter, clip_rect, clip, sample, bar_width, bpm,
                                );
                                draw_gain_handle(&painter, clip_rect, visible, clip.gain);
                            }
                        }
                    }
                    // Dim the lanes that Mute or another track's solo silences.
                    let project = self.session.project();
                    let any_solo = project.any_solo();
                    for (row, &track) in visible_tracks.iter().enumerate().take(occupied_count) {
                        if !project.tracks[track].audible(any_solo) {
                            painter.rect_filled(
                                Rect::from_min_size(
                                    Pos2::new(rect.left(), rect.top() + row as f32 * row_height),
                                    Vec2::new(width, row_height),
                                ),
                                0.0,
                                roles().canvas.gamma_multiply(0.6),
                            );
                        }
                    }
                    // Show where a dragged or click-placed sound will land.
                    let incoming = match &self.drag {
                        Some(DragItem::Sample(id)) => Some(*id),
                        Some(DragItem::Clip { .. }) => None,
                        None => self.selected_sample.filter(|_| {
                            self.place_on_click
                                && !self.erase
                                && !self.automating()
                                && response.hovered()
                        }),
                    };
                    if let Some(sample) =
                        incoming.and_then(|id| self.session.library().sample_by_id(id))
                        && let Some(pos) = ui.input(|i| i.pointer.latest_pos())
                        && visible.contains(pos)
                        && let Some(track) = self.timeline_track_at(pos)
                        && let Some(length_ticks) = self.session.placement_ticks(sample.id)
                        && !self
                            .session
                            .project()
                            .clips
                            .iter()
                            .any(|clip| self.clip_rect(clip, rect).contains(pos))
                    {
                        let fine = ui.input(|i| i.modifiers.ctrl);
                        let ghost = self.clip_rect(
                            &Clip {
                                id: 0,
                                sample_id: sample.id,
                                track,
                                start_tick: self.position_tick(pos.x, rect, fine),
                                length_ticks,
                                offset_frames: 0,
                                gain: 1.0,
                                fades: Default::default(),
                                pitch_cents: 0,
                                reverse: false,
                            },
                            rect,
                        );
                        painter.rect_filled(
                            ghost,
                            3.0,
                            group_color(&sample.group_name).gamma_multiply(0.3),
                        );
                        painter.rect_stroke(
                            ghost.shrink(0.5),
                            3.0,
                            Stroke::new(1.0_f32, roles().text),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if response.has_focus() {
                        painter.rect_stroke(
                            visible.shrink(1.0),
                            0.0,
                            Stroke::new(1.0_f32, roles().accent),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if self.section_bookmarks {
                        for (index, section) in self.session.project().sections.iter().enumerate() {
                            let x = rect.left()
                                + section.start_tick as f32 / TICKS_PER_BAR as f32 * bar_width;
                            painter.line_segment(
                                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                                Stroke::new(1.5_f32, section_color(index).gamma_multiply(0.8)),
                            );
                        }
                    }
                    let play_x = rect.left()
                        + self.session.cursor_tick() as f32 / TICKS_PER_BAR as f32 * bar_width;
                    painter.line_segment(
                        [
                            Pos2::new(play_x, rect.top()),
                            Pos2::new(play_x, rect.bottom()),
                        ],
                        Stroke::new(1.7_f32, roles().accent),
                    );
                    if let Some(lane) = self.automation_lane {
                        let rows = &visible_tracks[..occupied_count];
                        self.automation_lanes(ui, &response, &painter, rect, lane, rows);
                        return;
                    }
                    if let Some(bus) = self.return_lane {
                        self.return_lane(ui, &response, &painter, rect, visible, bus);
                        return;
                    }
                    if (response.clicked() || response.double_clicked())
                        && let Some(pos) = response.interact_pointer_pos()
                    {
                        let hit = self
                            .session
                            .project()
                            .clips
                            .iter()
                            .rev()
                            .find(|c| self.clip_rect(c, rect).contains(pos))
                            .cloned();
                        if let Some(clip) = hit {
                            if self.erase {
                                let result = self.session.delete(clip.id);
                                // The erased clip may be part of the selection.
                                self.reconcile_selection();
                                self.apply(result);
                            } else {
                                self.select_clip(clip.id, ui.input(|i| i.modifiers.shift));
                                self.place_on_click = false;
                                if response.double_clicked() {
                                    self.preview(clip.sample_id);
                                }
                            }
                        } else if let Some(sample) = self
                            .selected_sample
                            .filter(|_| self.place_on_click && !ui.input(|i| i.modifiers.shift))
                        {
                            let Some(track) = self.timeline_track_at(pos) else {
                                return;
                            };
                            let tick =
                                self.position_tick(pos.x, rect, ui.input(|i| i.modifiers.ctrl));
                            self.place(sample, track, tick);
                        } else {
                            self.selected_clip = None;
                            if !ui.input(|i| i.modifiers.shift) {
                                self.selected_clips.clear();
                            }
                        }
                    }
                    if response.secondary_clicked() {
                        let hit = response.interact_pointer_pos().and_then(|pos| {
                            self.session
                                .project()
                                .clips
                                .iter()
                                .rev()
                                .find(|clip| self.clip_rect(clip, rect).contains(pos))
                                .map(|clip| clip.id)
                        });
                        self.context_clip = hit;
                        if let Some(pos) = response.interact_pointer_pos() {
                            self.context_anchor = pos;
                        }
                        if let Some(id) = hit.filter(|id| !self.selected_clips.contains(id)) {
                            self.select_clip(id, false);
                            self.place_on_click = false;
                        }
                        self.context_missing_sample =
                            response.interact_pointer_pos().and_then(|pos| {
                                self.session
                                    .project()
                                    .clips
                                    .iter()
                                    .rev()
                                    .find(|clip| self.clip_rect(clip, rect).contains(pos))
                                    .and_then(|clip| self.session.missing_sample(clip.sample_id))
                                    .map(|missing| missing.id)
                            });
                    }
                    if self.context_clip.is_some() || self.context_missing_sample.is_some() {
                        // A context menu that Shift+F10 can also open at the
                        // selected clip. Clicks inside it edit the clip's values.
                        let open = if response.secondary_clicked() {
                            Some(egui::SetOpenCommand::Bool(true))
                        } else if response.clicked() {
                            Some(egui::SetOpenCommand::Bool(false))
                        } else {
                            None
                        };
                        egui::Popup::menu(&response)
                            .open_memory(open)
                            .at_position(self.context_anchor)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show(|ui| {
                                if let Some(sample_id) = self.context_missing_sample
                                    && ui.button("Resolve manually").clicked()
                                {
                                    ui.close();
                                    self.resolve_missing_sample(sample_id);
                                }
                                if let Some(id) = self.context_clip {
                                    if ui.button("Preview").clicked() {
                                        ui.close();
                                        // The menu's clip need not be the primary one.
                                        self.preview_clip(id);
                                    }
                                    if ui.button("Duplicate (Ctrl+D)").clicked() {
                                        ui.close();
                                        self.duplicate_selection();
                                    }
                                    if ui.button("Delete (Delete)").clicked() {
                                        ui.close();
                                        self.delete_selection();
                                    }
                                    self.clip_properties(ui, id);
                                }
                            });
                    }
                    if response.drag_started()
                        && self.handle_drag.is_none()
                        && let Some(pos) = ui.input(|i| i.pointer.press_origin())
                    {
                        if let Some(clip) = self
                            .session
                            .project()
                            .clips
                            .iter()
                            .rev()
                            .find(|c| self.clip_rect(c, rect).contains(pos))
                            .cloned()
                        {
                            let offset = self
                                .position_tick(pos.x, rect, true)
                                .saturating_sub(clip.start_tick);
                            if !self.selected_clips.contains(&clip.id) {
                                self.select_clip(clip.id, false);
                            }
                            self.drag = Some(DragItem::Clip {
                                id: clip.id,
                                offset,
                                copy: ui.input(|i| {
                                    i.modifiers.alt
                                        || i.modifiers.shift
                                        || i.pointer.secondary_down()
                                }),
                            });
                        } else {
                            self.selection_origin = Some(pos);
                            self.selection_base = if ui.input(|i| i.modifiers.shift) {
                                self.selected_clips.clone()
                            } else {
                                BTreeSet::new()
                            };
                        }
                    }
                    if let Some(origin) = self.selection_origin {
                        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                            let selection = Rect::from_two_pos(origin, pos).intersect(rect);
                            painter.rect_filled(selection, 0.0, roles().text.gamma_multiply(0.12));
                            outline(&painter, selection, roles().text, 1.0);
                            self.selected_clips = self.selection_base.clone();
                            let hits: Vec<_> = self
                                .session
                                .project()
                                .clips
                                .iter()
                                .filter(|c| self.clip_rect(c, rect).intersects(selection))
                                .map(|c| c.id)
                                .collect();
                            self.selected_clips.extend(hits);
                            self.selected_clip = self.selected_clips.iter().next().copied();
                        }
                        if response.drag_stopped() || ui.input(|i| i.pointer.any_released()) {
                            self.selection_origin = None;
                        }
                    }
                },
            );
            let labels = Rect::from_min_size(
                Pos2::new(body_left, whole.top()),
                Vec2::new(label_width, track_height),
            );
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(labels)
                    .id_salt("track_labels"),
                |ui| {
                    ui.set_clip_rect(labels.intersect(ui.clip_rect()));
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for &index in &visible_tracks {
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(label_width, row_height),
                            Sense::hover(),
                        );
                        if add_track == Some(index) {
                            let response = ui.interact(
                                rect,
                                ui.id().with(("add_track", index)),
                                Sense::click(),
                            );
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    ui.is_enabled(),
                                    format!("Add track {}", index + 1),
                                )
                            });
                            if response.clicked() {
                                self.armed_track = index;
                                self.message(format!("Recording will create track {}.", index + 1));
                            }
                            if response.has_focus() {
                                ui.painter().rect_stroke(
                                    rect.shrink(1.0),
                                    4.0,
                                    Stroke::new(1.0_f32, roles().accent),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            response
                                .hover_hint("Drop a sound on this row or select it for recording.");
                            continue;
                        }
                        ui.painter()
                            .rect_filled(rect.shrink(1.0), 4.0, roles().panel);
                        let project = self.session.project();
                        let silenced = !project.tracks[index].audible(project.any_solo());
                        // From the left: M, S, L, the track number, and R.
                        let number = Pos2::new(rect.left() + 85.0, rect.center().y);
                        ui.painter().rect_filled(
                            Rect::from_center_size(number, Vec2::new(25.0, row_height - 3.0)),
                            0.0,
                            roles().panel,
                        );
                        ui.painter().text(
                            number + Vec2::new(0.0, -1.0),
                            Align2::CENTER_CENTER,
                            format!("{}", index + 1),
                            FontId::proportional(14.0),
                            if silenced {
                                roles().muted
                            } else {
                                roles().text
                            },
                        );
                        let arm = ui.interact(
                            Rect::from_center_size(number, Vec2::new(26.0, row_height)),
                            ui.id().with(("arm", index)),
                            Sense::click(),
                        );
                        arm.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::RadioButton,
                                ui.is_enabled(),
                                self.armed_track == index,
                                format!("Record on track {}", index + 1),
                            )
                        });
                        if self.armed_track == index {
                            ui.painter().circle_filled(
                                number + Vec2::new(0.0, (row_height / 2.0 - 4.0).min(9.0)),
                                2.0,
                                roles().coral,
                            );
                        }
                        if arm.has_focus() {
                            ui.painter().rect_stroke(
                                arm.rect.shrink2(Vec2::new(0.0, 2.0)),
                                3.0,
                                Stroke::new(1.0_f32, roles().accent),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if arm.clicked() {
                            self.armed_track = index;
                            self.message(format!("Track {} is ready for recording.", index + 1));
                        }
                        arm.hover_hint("Select this track for recording");
                        let track = &self.session.project().tracks[index];
                        for (switch, x) in
                            TrackSwitch::ALL.into_iter().zip([12.0, 36.0, 60.0, 110.0])
                        {
                            let button = Rect::from_center_size(
                                Pos2::new(rect.left() + x, rect.center().y),
                                Vec2::new(24.0, row_height - 3.0),
                            );
                            let mut response = ui.interact(
                                button,
                                ui.id().with(("switch", index, switch)),
                                Sense::click(),
                            );
                            let mut on = switch.get(track);
                            if response.clicked() {
                                on = !on;
                                route_change = Some((index, switch, on));
                                response.mark_changed();
                            }
                            let label =
                                format!("Arrangement track {} {}", index + 1, switch.name());
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::Checkbox,
                                    ui.is_enabled(),
                                    on,
                                    &label,
                                )
                            });
                            switch_letter(ui.painter(), button, on, switch);
                            if response.has_focus() {
                                ui.painter().rect_stroke(
                                    button.shrink(1.0),
                                    2.0,
                                    Stroke::new(1.0_f32, roles().accent),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            response.hover_hint(switch_hint(&label, switch, on));
                        }
                    }
                },
            );
        });
        let ruler_clip = Rect::from_min_max(
            ruler_strip.min + Vec2::new(lane_inset, 0.0),
            Pos2::new(scroll.inner_rect.right(), ruler_strip.bottom()),
        );
        self.timeline_ruler_rect = Some(ruler_clip);
        let ruler = Rect::from_min_size(
            ruler_strip.min + Vec2::new(lane_inset - painted_offset_x, 0.0),
            Vec2::new(width, 26.0),
        );
        let ruler_response = name_region(
            ui.interact(
                ruler_clip,
                ui.id().with("bar_ruler"),
                Sense::CLICK | Sense::DRAG,
            ),
            "Bar ruler",
        );
        let ruler_painter = ui.painter().with_clip_rect(ruler_clip);
        ruler_painter.rect_filled(ruler, 0.0, roles().panel);
        if let Some([start, end]) = self.session.project().loop_region {
            let rect = Rect::from_min_max(
                Pos2::new(
                    ruler.left() + start as f32 / TICKS_PER_BAR as f32 * bar_width,
                    ruler.top(),
                ),
                Pos2::new(
                    ruler.left() + end as f32 / TICKS_PER_BAR as f32 * bar_width,
                    ruler.bottom(),
                ),
            );
            ruler_painter.rect_filled(rect, 0.0, roles().accent.gamma_multiply(0.14));
        }
        let tags = if self.section_bookmarks {
            section_tags(
                ui,
                &ruler_painter,
                ruler,
                bar_width,
                &self.session.project().sections,
            )
        } else {
            Vec::new()
        };
        let first_bar = ((ruler_clip.left() - ruler.left()) / bar_width)
            .floor()
            .max(0.0) as u32;
        let last_bar = ((ruler_clip.right() - ruler.left()) / bar_width)
            .ceil()
            .min(bars as f32) as u32;
        for bar in first_bar..last_bar {
            let x = ruler.left() + bar as f32 * bar_width;
            let number_x = x + bar_width * 0.5;
            if tags
                .iter()
                .any(|(_, tag)| (tag.left() - 8.0..=tag.right() + 8.0).contains(&number_x))
            {
                continue;
            }
            ruler_painter.text(
                Pos2::new(x + bar_width * 0.5, ruler.center().y),
                Align2::CENTER_CENTER,
                format!("{}", bar + 1),
                FontId::proportional(13.0),
                if bar == self.session.cursor_tick() / TICKS_PER_BAR {
                    roles().coral
                } else {
                    roles().text
                },
            );
        }
        if ruler_response.clicked()
            && let Some(pos) = ruler_response.interact_pointer_pos()
        {
            self.seek(self.position_tick(pos.x, ruler, false));
        }
        if ruler_response.drag_started() {
            self.loop_start = ui
                .input(|i| i.pointer.press_origin())
                .map(|p| self.position_tick(p.x, ruler, false));
        }
        if ruler_response.drag_stopped()
            && let (Some(start), Some(pos)) = (
                self.loop_start.take(),
                ruler_response.interact_pointer_pos(),
            )
        {
            let end = self.position_tick(pos.x, ruler, false);
            let region = [start.min(end), start.max(end).saturating_add(TICKS_PER_BAR)];
            let result = self.session.edit(EditCommand::SetLoop(Some(region)));
            self.apply(result);
        }
        if self.section_bookmarks {
            self.section_gestures(ui, &ruler_response, ruler, &tags);
        }
        if let Some((index, switch, on)) = route_change {
            let result = self.session.edit(EditCommand::SetSwitch {
                track: index,
                switch,
                on,
            });
            self.apply(result);
            // The mixer is laid out before the arrangement. Redraw it with the
            // new switch state even when the pointer stops after this click.
            ui.ctx().request_repaint();
        }
    }

    /// Keys for the focused lanes: arrows choose a clip, Alt+arrows move the
    /// selection, Enter previews it, Delete removes it, and Shift+F10 opens
    /// its context menu.
    fn lane_keys(&mut self, ui: &egui::Ui, response: &Response, rect: Rect) {
        controls::keep_transport_key(ui.ctx(), response.id);
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
        let (mut step, mut shift) = ((0, 0), (0, 0));
        let (mut preview, mut delete, mut fine, mut menu) = (false, false, false, false);
        ui.input_mut(|input| {
            for (key, x, y) in [
                (egui::Key::ArrowLeft, -1, 0),
                (egui::Key::ArrowRight, 1, 0),
                (egui::Key::ArrowUp, 0, -1),
                (egui::Key::ArrowDown, 0, 1),
            ] {
                // egui ignores an extra Alt when it matches a key, so read it here.
                let modifiers = input.modifiers;
                if input.consume_key(modifiers, key) {
                    if modifiers.alt {
                        shift = (x, y);
                        fine = modifiers.ctrl;
                    } else {
                        step = (x, y);
                    }
                }
            }
            preview = input.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
            delete = input.consume_key(egui::Modifiers::NONE, egui::Key::Delete);
            menu = input.consume_key(egui::Modifiers::SHIFT, egui::Key::F10);
        });
        self.clip_menu_key |= menu;
        if step != (0, 0) {
            self.step_clip_selection(step.0, step.1);
        }
        if shift != (0, 0) && !self.selected_clips.is_empty() {
            let ids: Vec<_> = self.selected_clips.iter().copied().collect();
            let ticks = i64::from(if fine {
                TICKS_PER_BAR / 16
            } else {
                TICKS_PER_BAR
            });
            if let Err(error) =
                self.try_move_clips(&ids, shift.1, i64::from(shift.0) * ticks, false)
            {
                self.error(error);
            }
        }
        if preview {
            self.preview_selection();
        }
        if delete {
            self.delete_selection();
        }
        if (step != (0, 0) || shift != (0, 0))
            && let Some(clip) = self.selected_clip.and_then(|id| {
                self.session
                    .project()
                    .clips
                    .iter()
                    .find(|clip| clip.id == id)
            })
        {
            ui.scroll_to_rect(self.clip_rect(clip, rect), None);
        }
    }

    pub(super) fn draw(&mut self, ctx: &egui::Context) {
        self.poll_application(ctx);
        // An applied theme repaints the next frame; songs, history, and audio
        // are untouched.
        theme::adopt(ctx, self.session.options().theme);
        self.section_bookmarks = self.session.options().show_section_bookmarks;
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::new("show-hover-hints"),
                self.session.options().show_tooltips,
            )
        });
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
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
        self.side_rail(ctx);
        // The lower panel's Sounds module folds the Sounds panel, whichever
        // way it was chosen, so the two never show at once.
        if self.bottom_module == BottomModule::Sounds && self.session.layout().sounds {
            self.set_layout(|layout| layout.sounds = false);
        }
        self.bottom(ctx);
        self.sounds_side(ctx);
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
        self.sample_conflicts_dialog(ctx);
        self.path_prompt_dialog(ctx);
        self.capture(ctx);
    }
}

/// Paint each section's tag on the bar ruler: a colored line at its start and
/// its name on a chip that ends before the next section. Returns each chip
/// with its section's start.
fn section_tags(
    ui: &egui::Ui,
    painter: &egui::Painter,
    ruler: Rect,
    bar_width: f32,
    sections: &[Section],
) -> Vec<(u32, Rect)> {
    let x_of = |tick: u32| ruler.left() + tick as f32 / TICKS_PER_BAR as f32 * bar_width;
    let mut chips = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        let color = section_color(index);
        let x = x_of(section.start_tick);
        let limit = sections
            .get(index + 1)
            .map_or(f32::INFINITY, |next| x_of(next.start_tick) - 2.0);
        painter.line_segment(
            [Pos2::new(x, ruler.top()), Pos2::new(x, ruler.bottom())],
            Stroke::new(2.0_f32, color),
        );
        let galley = ui.painter().layout_no_wrap(
            section.name.clone(),
            FontId::proportional(11.0),
            roles().on_light,
        );
        let chip = Rect::from_min_size(
            Pos2::new(x, ruler.top() + 4.0),
            Vec2::new(galley.size().x + 10.0, ruler.height() - 8.0),
        );
        let visible =
            Rect::from_min_max(chip.min, Pos2::new(chip.right().min(limit), chip.bottom()));
        if visible.width() < 4.0 {
            continue;
        }
        let chip_painter = painter.with_clip_rect(visible.intersect(painter.clip_rect()));
        chip_painter.rect_filled(
            chip,
            egui::CornerRadius {
                nw: 0,
                sw: 0,
                ne: 4,
                se: 4,
            },
            color,
        );
        chip_painter.galley(
            Pos2::new(chip.left() + 5.0, chip.center().y - galley.size().y / 2.0),
            galley,
            roles().on_light,
        );
        chips.push((section.start_tick, visible));
    }
    chips
}

/// A section name being edited on the bar ruler.
pub(super) struct SectionRename {
    /// The start of the section, which identifies it while the list changes.
    pub(super) start_tick: u32,
    pub(super) name: String,
    /// The editor takes the keyboard focus on its first frame.
    pub(super) focused: bool,
}

/// The first "Section N" name that no section of `sections` has.
fn new_section_name(sections: &[Section]) -> String {
    (1..)
        .map(|n| format!("Section {n}"))
        .find(|name| sections.iter().all(|section| section.name != *name))
        .unwrap_or_default()
}

impl MixJamApp {
    /// Double-click the bar ruler to add a section at that bar, or a tag to
    /// rename it; right-click for the same choices and Delete.
    fn section_gestures(
        &mut self,
        ui: &mut egui::Ui,
        ruler_response: &Response,
        ruler: Rect,
        tags: &[(u32, Rect)],
    ) {
        let tag_at = |pos: Pos2| {
            tags.iter()
                .find(|(_, rect)| rect.contains(pos))
                .map(|(tick, _)| *tick)
        };
        if ruler_response.double_clicked()
            && let Some(pos) = ruler_response.interact_pointer_pos()
        {
            // The first click of a double-click starts no loop.
            self.loop_start = None;
            match tag_at(pos) {
                Some(tick) => self.rename_section(tick),
                None => self.add_section(self.position_tick(pos.x, ruler, false)),
            }
        }
        if ruler_response.secondary_clicked() {
            let pos = ruler_response.interact_pointer_pos();
            self.section_menu = (
                pos.and_then(tag_at),
                pos.map_or(self.session.cursor_tick(), |pos| {
                    self.position_tick(pos.x, ruler, false)
                }),
            );
        }
        let (tag, bar) = self.section_menu;
        ruler_response.context_menu(|ui| match tag {
            Some(tick) => {
                if ui.button("Rename section").clicked() {
                    self.rename_section(tick);
                    ui.close();
                }
                if ui.button("Delete section").clicked() {
                    let mut sections = self.session.project().sections.clone();
                    sections.retain(|section| section.start_tick != tick);
                    let result = self.session.edit(EditCommand::SetSections(sections));
                    self.apply(result);
                    ui.close();
                }
            }
            None => {
                if ui.button("Add section here").clicked() {
                    self.add_section(bar);
                    ui.close();
                }
            }
        });
        self.section_rename_editor(ui, ruler);
    }

    /// Add a section named "Section N" at `tick` and edit its name. A section
    /// that already starts there is renamed instead.
    pub(super) fn add_section(&mut self, tick: u32) {
        let sections = &self.session.project().sections;
        if !sections.iter().any(|section| section.start_tick == tick) {
            let mut sections = sections.clone();
            sections.push(Section {
                start_tick: tick,
                name: new_section_name(&sections),
            });
            sections.sort_by_key(|section| section.start_tick);
            let result = self.session.edit(EditCommand::SetSections(sections));
            if result.is_err() {
                self.apply(result);
                return;
            }
        }
        self.rename_section(tick);
    }

    fn rename_section(&mut self, tick: u32) {
        if let Some(section) = self
            .session
            .project()
            .sections
            .iter()
            .find(|section| section.start_tick == tick)
        {
            self.section_rename = Some(SectionRename {
                start_tick: tick,
                name: section.name.clone(),
                focused: false,
            });
        }
    }

    /// The name editor over a section's tag. Enter or a click elsewhere keeps
    /// the name; Escape or an empty name keeps the old one.
    fn section_rename_editor(&mut self, ui: &mut egui::Ui, ruler: Rect) {
        let Some(rename) = &mut self.section_rename else {
            return;
        };
        let x = ruler.left()
            + rename.start_tick as f32 / TICKS_PER_BAR as f32 * self.timeline_bar_width;
        let mut done = None;
        egui::Area::new(ui.id().with("section_rename"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(x, ruler.top() + 1.0))
            .show(ui.ctx(), |ui| {
                let response = name_control(
                    ui.add(
                        egui::TextEdit::singleline(&mut rename.name)
                            .char_limit(MAX_SECTION_NAME)
                            .desired_width(180.0)
                            .font(FontId::proportional(12.0)),
                    ),
                    "Section name",
                );
                if !rename.focused {
                    response.request_focus();
                    rename.focused = true;
                }
                if response.lost_focus() {
                    done = Some(!ui.input(|input| input.key_pressed(egui::Key::Escape)));
                }
            });
        let Some(keep) = done else {
            return;
        };
        let Some(rename) = self.section_rename.take() else {
            return;
        };
        let name = rename.name.trim();
        if !keep || name.is_empty() {
            return;
        }
        let mut sections = self.session.project().sections.clone();
        if let Some(section) = sections
            .iter_mut()
            .find(|section| section.start_tick == rename.start_tick)
            && section.name != name
        {
            section.name = name.into();
            let result = self.session.edit(EditCommand::SetSections(sections));
            self.apply(result);
        }
    }
}

/// Keep the viewport still while the playhead is comfortably visible.
fn follow_offset(
    playhead: f32,
    offset: f32,
    visible_width: f32,
    content_width: f32,
) -> Option<f32> {
    let margin = (visible_width * 0.1).min(48.0);
    if playhead >= offset + margin && playhead <= offset + visible_width - margin {
        return None;
    }
    let next =
        (playhead - visible_width * 0.25).clamp(0.0, (content_width - visible_width).max(0.0));
    ((next - offset).abs() > 0.5).then_some(next)
}

fn transport_position(
    ui: &mut egui::Ui,
    size: Vec2,
    tick: u32,
    bpm: f64,
    bar_and_beat: bool,
) -> Response {
    let bars = tick / TICKS_PER_BAR + 1;
    let beat = (tick % TICKS_PER_BAR) / (TICKS_PER_BAR / 4) + 1;
    let seconds = (tick as f64 * 240.0 / bpm / TICKS_PER_BAR as f64) as u64;
    let text = format!("{:02} : {:02}", seconds / 60, seconds % 60);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    // Vintage reads time from an amber display window; the other themes from
    // the header itself.
    let display = roles().material == theme::Material::Metal;
    if display {
        ui.painter().rect_filled(rect.shrink(2.0), 4.0, roles().ink);
        ui.painter().rect_stroke(
            rect.shrink(2.0),
            4.0,
            Stroke::new(1.0_f32, roles().line),
            egui::StrokeKind::Inside,
        );
    } else {
        ui.painter()
            .rect_filled(rect, roles().corner(6.0), roles().canvas);
    }
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .text(
            rect.center(),
            Align2::CENTER_CENTER,
            &text,
            FontId::proportional(if size.x < POSITION_SIZE.x { 19.0 } else { 23.0 }),
            if display {
                roles().accent
            } else {
                roles().text
            },
        );
    // Bar and beat beside the time, where the top bar has room.
    if bar_and_beat {
        let (beside, _) = ui.allocate_exact_size(Vec2::new(44.0, size.y), Sense::hover());
        ui.painter().text(
            beside.left_center(),
            Align2::LEFT_CENTER,
            format!("{bars} . {beat}"),
            FontId::proportional(12.0),
            roles().muted,
        );
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
    response.hover_hint(format!(
        "{text} (minutes : seconds). Bar {bars}, beat {beat}"
    ))
}

/// How the song name is laid out at the left of the transport.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TitleFit {
    size: f32,
    rows: usize,
    break_anywhere: bool,
}

fn transport_title_galley(
    ui: &egui::Ui,
    text: &str,
    fit: TitleFit,
    width: f32,
) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(
        title_text(text),
        transport_title_font(fit.size),
        roles().text,
        width.max(1.0),
    );
    job.wrap.max_rows = fit.rows;
    job.wrap.break_anywhere = fit.break_anywhere;
    for section in &mut job.sections {
        section.format.line_height = Some(fit.size * theme::HEADING_LINE);
    }
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

fn transport_title_font(size: f32) -> FontId {
    FontId::new(size, theme::heading_family())
}

/// The largest layout that shows the whole song name within `width`: from 15
/// points down to the 9-point floor, on one line or two lines wrapped between
/// words, then at the floor in as many lines as the row holds. Only a name too
/// long for that goes below the floor, breaking words if it must, so the name
/// is never clipped.
fn transport_title_fit(ui: &egui::Ui, text: &str, width: f32) -> TitleFit {
    let fits = |fit: TitleFit| {
        let galley = transport_title_galley(ui, text, fit, width);
        !galley.elided && galley.size().x <= width + 0.5 && galley.size().y <= TRANSPORT_ROW_HEIGHT
    };
    let rows = |size: f32| {
        let row = size * theme::HEADING_LINE;
        ((TRANSPORT_ROW_HEIGHT / row).floor() as usize).max(1)
    };
    let upper = title_text(text);
    // Shrink the name rather than break one of its words.
    let words = |size: f32| {
        upper.split_whitespace().all(|word| {
            let galley = ui.fonts_mut(|fonts| {
                fonts.layout_no_wrap(word.into(), transport_title_font(size), roles().text)
            });
            galley.size().x <= width + 0.5
        })
    };
    let mut size = TRANSPORT_TITLE_SIZE;
    while size >= TRANSPORT_TITLE_FLOOR {
        let fit = TitleFit {
            size,
            rows: 2,
            break_anywhere: false,
        };
        if words(size) && fits(fit) {
            return fit;
        }
        size -= 1.0;
    }
    // At the floor, a very long name takes as many lines as the row holds.
    size = TRANSPORT_TITLE_FLOOR;
    let fit = TitleFit {
        size,
        rows: rows(size),
        break_anywhere: false,
    };
    if words(size) && fits(fit) {
        return fit;
    }
    loop {
        size -= 0.5;
        let fit = TitleFit {
            size,
            rows: rows(size),
            break_anywhere: true,
        };
        if size <= 1.0 {
            return TitleFit {
                rows: usize::MAX,
                ..fit
            };
        }
        if fits(fit) {
            return fit;
        }
    }
}

pub(super) fn effects_controls(ui: &mut egui::Ui, state: &mut EffectsState) {
    egui::Frame::new().fill(roles().face).show(ui, |ui| {
        ui.visuals_mut().override_text_color = Some(roles().legend);
        ui.vertical(|ui| {
            let mut enabled = state.filter.is_some();
            if embedded_effect_group(ui, "Filter", &mut enabled) {
                state.filter = enabled.then(Default::default);
            }
            let mut filter = state.filter.clone().unwrap_or_default();
            ui.add_enabled_ui(state.filter.is_some(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    embedded_effect_dial(
                        ui,
                        &mut filter.cutoff_hz,
                        "Cutoff",
                        [20.0, 20000.0, 2000.0, 1.0],
                        0,
                        true,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut filter.resonance,
                        "Resonance",
                        [0.0, 1.0, 0.0, 0.001],
                        3,
                        false,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut filter.drive,
                        "Overdrive",
                        [0.0, 1.0, 0.0, 0.001],
                        3,
                        false,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut filter.wet,
                        "Filter volume",
                        [0.0, 1.0, 1.0, 0.001],
                        3,
                        false,
                    );
                });
            });
            if state.filter.is_some() {
                state.filter = Some(filter);
            }
            let mut enabled = state.echo.is_some();
            if embedded_effect_group(ui, "Echo", &mut enabled) {
                state.echo = enabled.then(Default::default);
            }
            let mut echo = state.echo.clone().unwrap_or_default();
            ui.add_enabled_ui(state.echo.is_some(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    embedded_effect_dial(
                        ui,
                        &mut echo.delay_beats,
                        "Delay (beats)",
                        [0.001, 2.0, 0.5, 0.01],
                        3,
                        false,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut echo.feedback,
                        "Feedback",
                        [0.0, 0.95, 0.4, 0.01],
                        2,
                        false,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut echo.wet,
                        "Echo volume",
                        [0.0, 1.0, 0.35, 0.01],
                        2,
                        false,
                    );
                });
            });
            if state.echo.is_some() {
                state.echo = Some(echo);
            }
            let mut enabled = state.robotize.is_some();
            if embedded_effect_group(ui, "Robotize", &mut enabled) {
                state.robotize = enabled.then(Default::default);
            }
            let mut robot = state.robotize.clone().unwrap_or_default();
            ui.add_enabled_ui(state.robotize.is_some(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    embedded_effect_dial(
                        ui,
                        &mut robot.frequency_hz,
                        "Color",
                        [1.0, 2000.0, 80.0, 1.0],
                        0,
                        true,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut robot.wet,
                        "Robotize volume",
                        [0.0, 1.0, 0.5, 0.01],
                        2,
                        false,
                    );
                });
            });
            if state.robotize.is_some() {
                state.robotize = Some(robot);
            }
            let (band, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
            controls::section(ui, band, "Distortion / output");
            ui.horizontal_wrapped(|ui| {
                embedded_effect_dial(
                    ui,
                    &mut state.dirty,
                    "Dirty distortion",
                    [0.0, 1.0, 0.0, 0.001],
                    3,
                    false,
                );
                embedded_effect_dial(
                    ui,
                    &mut state.noise,
                    "Noise distortion",
                    [0.0, 1.0, 0.0, 0.001],
                    3,
                    false,
                );
                embedded_effect_dial(
                    ui,
                    &mut state.volume,
                    "Output volume",
                    [0.0, 2.0, 1.0, 0.01],
                    2,
                    false,
                );
            });
        });
    });
}

fn embedded_effect_group(ui: &mut egui::Ui, name: &str, enabled: &mut bool) -> bool {
    let (band, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    controls::section(ui, band, "");
    controls::toggle_at(ui, band.shrink2(Vec2::new(8.0, 0.0)), name, name, enabled)
        .hover_hint(format!("{name}: turning it off discards its settings."))
        .changed()
}

fn embedded_effect_dial(
    ui: &mut egui::Ui,
    value: &mut f32,
    name: &str,
    settings: [f64; 4],
    decimals: usize,
    logarithmic: bool,
) {
    let [low, high, reset, step] = settings;
    let (bounds, _) = ui.allocate_exact_size(Vec2::new(126.0, 112.0), Sense::hover());
    // Absolute value fields must only advance their own column. Adding them
    // to the horizontal parent shifts the next dial down to the field baseline.
    let mut column = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(name)
            .max_rect(bounds)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    column.set_clip_rect(bounds.intersect(ui.clip_rect()));
    let ui = &mut column;
    let mut draft = f64::from(*value);
    let dial = Rect::from_center_size(
        bounds.center_top() + Vec2::new(0.0, 29.0),
        Vec2::splat(48.0),
    );
    let response = controls::rotary(
        ui,
        dial,
        &mut draft,
        controls::RotarySpec {
            name,
            low,
            high,
            reset,
            step,
            decimals,
            logarithmic,
            choices: &[],
        },
    );
    if response.changed() {
        *value = draft as f32;
    }
    let well = Rect::from_min_size(bounds.min + Vec2::new(13.0, 61.0), Vec2::new(100.0, 24.0));
    controls::well(ui, well);
    ui.scope(|ui| {
        controls::value_style(ui);
        name_control(
            ui.put(
                well.shrink2(Vec2::new(2.0, 0.0)),
                egui::DragValue::new(value)
                    .range(low..=high)
                    .speed(step)
                    .fixed_decimals(decimals),
            ),
            &format!("{name} value"),
        );
    });
    ui.painter().text(
        bounds.center_top() + Vec2::new(0.0, 99.0),
        Align2::CENTER_CENTER,
        name,
        FontId::proportional(13.0),
        roles().legend,
    );
}

/// A track's pan knob. While its pan lane has points, the knob is
/// `automated`: the lane sets the pan while the song plays. The knob leaves
/// the wheel to the view behind it, so a scroll over the strips never moves
/// a pan.
pub(super) fn pan_control(
    ui: &mut egui::Ui,
    index: usize,
    pan: &mut f32,
    automated: bool,
) -> Response {
    let response = ui.add(egui::Slider::new(pan, -1.0..=1.0).show_value(false));
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::slider(
            ui.is_enabled(),
            f64::from(*pan),
            format!("Track {} pan", index + 1),
        );
        info.current_text_value = Some(pan_description(*pan));
        info
    });
    response.hover_hint(format!(
        "Track {} pan: {}. Drag or use the arrow keys; the wheel scrolls. Pan does not enable \
         a disabled L/R output.{}",
        index + 1,
        pan_description(*pan),
        if automated {
            "\nAutomated: its automation lane sets the pan while the song plays."
        } else {
            ""
        }
    ))
}

pub(super) fn pan_description(pan: f32) -> String {
    if pan == 0.0 {
        "Center".into()
    } else {
        format!(
            "{:.0}% {}",
            pan.abs() * 100.0,
            if pan < 0.0 { "left" } else { "right" }
        )
    }
}

pub(super) fn mixer_sample_summary(
    project: &Project,
    library: LibraryView<'_>,
    index: usize,
) -> (String, String) {
    let ids: BTreeSet<u32> = project
        .clips
        .iter()
        .filter(|clip| clip.track == index)
        .map(|clip| clip.sample_id)
        .collect();
    let names: Vec<String> = ids
        .iter()
        .map(|id| {
            if let Some(missing) = project
                .missing_samples
                .iter()
                .find(|sample| sample.id == *id)
            {
                return format!("Missing: {}", missing.name);
            }
            library.sample_by_id(*id).map_or_else(
                || format!("Missing sound {id}"),
                |sample| sample.name.clone(),
            )
        })
        .collect();
    let summary = match names.as_slice() {
        [] => "No sounds".into(),
        [name] => name.clone(),
        _ => format!("{} sounds", names.len()),
    };
    (summary, names.join("\n"))
}

/// The compact tempo slider: one notch per supported tempo, value at the end.
pub(super) fn bpm_picker(ui: &mut egui::Ui, name: &str, bpm: &mut f64, width: f32) -> Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::hover());
    controls::stepped_slider(ui, rect, bpm, &SUPPORTED_BPMS, name, false, None)
}

pub(super) fn recording_destination(
    ui: &mut egui::Ui,
    selected: &mut usize,
    occupied: &[usize],
) -> Response {
    let response = egui::ComboBox::from_id_salt("record_destination")
        .selected_text(format!("{}", *selected + 1))
        .width(48.0)
        .show_ui(ui, |ui| {
            for index in 0..TRACK_COUNT {
                let label = if occupied.contains(&index) {
                    format!("Track {}", index + 1)
                } else {
                    format!("New track {}", index + 1)
                };
                ui.selectable_value(selected, index, label);
            }
        })
        .response;
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    name_control(response, "Recording track")
        .hover_hint("Choose a track for recording. Unused tracks appear only when you add a sound.")
}

/// The workspace style, menus, tooltips, and fonts of the current theme.
pub(super) fn configure_style(ctx: &egui::Context) {
    theme::configure_fonts(ctx);
    let roles = roles();
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    // egui's dark-mode curve (close to gamma 0.5) suits light text on dark
    // fields but bloats dark legends on light button faces; linear thins the
    // light text. One curve serves both in the shared glyph atlas.
    style.visuals.text_alpha_from_coverage = egui::epaint::AlphaFromCoverage::Gamma(0.7);
    style.visuals.override_text_color = Some(roles.text);
    style.visuals.panel_fill = roles.canvas;
    style.visuals.window_fill = roles.panel;
    style.visuals.window_stroke = Stroke::new(1.0_f32, roles.line);
    style.visuals.window_corner_radius = egui::CornerRadius::same(roles.corner(10.0) as u8);
    style.visuals.menu_corner_radius = egui::CornerRadius::same(roles.corner(6.0) as u8);
    style.visuals.faint_bg_color = roles.raised;
    style.visuals.extreme_bg_color = roles.canvas;
    style.visuals.selection.bg_fill = roles.selection;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, roles.accent);
    style.visuals.hyperlink_color = roles.accent;
    style.visuals.warn_fg_color = roles.warning;
    style.visuals.error_fg_color = roles.coral;
    if roles.material == theme::Material::Flat {
        // Studio keeps menus and tooltips flat: an edge, no cast shadow.
        style.visuals.window_shadow = egui::epaint::Shadow::NONE;
        style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
    }
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(roles.corner(5.0) as u8);
        widget.fg_stroke = Stroke::new(1.0_f32, roles.text);
        widget.bg_stroke = Stroke::new(1.0_f32, roles.line);
        widget.bg_fill = roles.raised;
        widget.weak_bg_fill = roles.raised;
    }
    style.visuals.widgets.hovered.bg_fill = roles.hover;
    style.visuals.widgets.hovered.weak_bg_fill = roles.hover;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, roles.muted);
    style.visuals.widgets.active.bg_fill = roles.hover;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, roles.accent);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, roles.line);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, roles.muted);
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.interact_size = Vec2::new(32.0, 28.0);
    style.spacing.slider_width = 84.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(11.0));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::proportional(13.0));
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(20.0, theme::heading_family()),
    );
    ctx.set_style(style);
}

pub(super) fn outline(painter: &egui::Painter, rect: Rect, color: Color32, width: f32) {
    let stroke = Stroke::new(width, color);
    painter.line_segment([rect.left_top(), rect.right_top()], stroke);
    painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
    painter.line_segment([rect.right_bottom(), rect.left_bottom()], stroke);
    painter.line_segment([rect.left_bottom(), rect.left_top()], stroke);
}

/// These buttons override egui's fill and stroke, so focus needs its own mark.
pub(super) fn focus_outline(ui: &egui::Ui, response: &Response, color: Color32) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            5.0,
            Stroke::new(2.0_f32, color),
            egui::StrokeKind::Inside,
        );
    }
}

pub(super) fn action_button(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).color(if active {
            roles().on_light
        } else {
            roles().text
        }))
        .fill(if active {
            roles().accent
        } else {
            roles().raised
        })
        .stroke(Stroke::NONE)
        .min_size(Vec2::new(48.0, 30.0)),
    );
    focus_outline(
        ui,
        &response,
        if active {
            roles().on_light
        } else {
            roles().accent
        },
    );
    response
}

fn action_icon_button(
    ui: &mut egui::Ui,
    icons: &RailIcons,
    icon: RailIcon,
    label: &str,
    enabled: bool,
    hint: &str,
    width: f32,
) -> Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .fill(roles().raised)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(width, 30.0)),
    );
    ui.painter().image(
        icons.texture_id(icon),
        icons.icon_rect(ui.ctx(), response.rect.center()),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        if enabled {
            roles().text
        } else {
            roles().muted.gamma_multiply(0.5)
        },
    );
    focus_outline(ui, &response, roles().accent);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response
        .hover_hint(hint)
        .disabled_hint(format!("{hint}: nothing to {}.", label.to_lowercase()))
}

pub(super) fn rail_button(
    ui: &mut egui::Ui,
    icons: &RailIcons,
    icon: RailIcon,
    active: bool,
) -> Response {
    let width = 48.0_f32.min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
    controls::icon_button_surface(ui, &response, active);
    let color = rail_icon_color(ui, active);
    ui.painter().image(
        icons.texture_id(icon),
        icons.icon_rect(ui.ctx(), rect.center()),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        color,
    );
    response.widget_info(|| {
        if icon.is_toggle() {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                ui.is_enabled(),
                active,
                icon.label(),
            )
        } else {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), icon.label())
        }
    });
    response.hover_hint(icon.hint())
}

/// A muted icon that names the control beside it, as the microphone does
/// for the recording track.
pub(super) fn icon_heading(ui: &mut egui::Ui, icons: &RailIcons, icon: RailIcon, width: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::hover());
    ui.painter().image(
        icons.texture_id(icon),
        icons.icon_rect(ui.ctx(), rect.center()),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        roles().muted,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Image, ui.is_enabled(), icon.label())
    });
    response.hover_hint(icon.label());
}

/// The brand: MixJam over Techno, smaller in a narrow top bar.
fn stacked_brand(ui: &mut egui::Ui, compact: bool) {
    let galley = |text: egui::RichText| {
        egui::WidgetText::from(text).into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
    };
    let (large, small) = if compact { (14.0, 9.0) } else { (20.0, 11.0) };
    let top = galley(brand("MixJam", large));
    let bottom = galley(brand("Techno", small).color(roles().muted));
    let size = Vec2::new(
        top.size().x.max(bottom.size().x),
        top.size().y + bottom.size().y,
    );
    let (_, rect) = ui.allocate_space(size);
    // Two labels, so each word keeps its own accessible name.
    let mut stack = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    stack.spacing_mut().item_spacing.y = 0.0;
    stack.label(top);
    stack.label(bottom);
}

/// The song's length under its name: bars and minutes. None for an empty
/// song.
fn song_meta(project: &Project) -> Option<String> {
    let end = project.end_tick();
    if end == 0 {
        return None;
    }
    let bars = end.div_ceil(TICKS_PER_BAR);
    let seconds = (f64::from(end) * 240.0 / project.bpm / f64::from(TICKS_PER_BAR)).round() as u64;
    Some(format!(
        "{bars} {}  ·  {}:{:02}",
        if bars == 1 { "bar" } else { "bars" },
        seconds / 60,
        seconds % 60
    ))
}

/// The score toolbar's Maximise switch: corners pointing out, or in while
/// the score fills the window.
fn maximise_button(ui: &mut egui::Ui, on: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(30.0, 26.0), Sense::click());
    controls::icon_button_surface(ui, &response, on);
    let color = rail_icon_color(ui, on);
    let stroke = Stroke::new(1.5_f32, color);
    let mark = Rect::from_center_size(rect.center(), Vec2::splat(12.0));
    let arm = 4.0;
    for (corner, x, y) in [
        (mark.left_top(), 1.0, 1.0),
        (mark.right_top(), -1.0, 1.0),
        (mark.left_bottom(), 1.0, -1.0),
        (mark.right_bottom(), -1.0, -1.0),
    ] {
        // Restored, each corner is an outward bracket; maximised, the
        // brackets turn inward.
        let (tip, dx, dy) = if on {
            (corner + Vec2::new(x * arm, y * arm), -x, -y)
        } else {
            (corner, x, y)
        };
        ui.painter()
            .line_segment([tip, tip + Vec2::new(dx * arm, 0.0)], stroke);
        ui.painter()
            .line_segment([tip, tip + Vec2::new(0.0, dy * arm)], stroke);
    }
    focus_outline(ui, &response, roles().accent);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            on,
            "Maximise the score",
        )
    });
    response.hover_hint(if on {
        "Restore the tool rail, the Sounds panel and the lower panel (F)"
    } else {
        "Maximise the score: hide the tool rail, the Sounds panel and the lower panel (F)"
    })
}

/// The rail's width: icons with names, or icons only.
pub(super) fn rail_width(names: bool) -> f32 {
    if names { 84.0 } else { 64.0 }
}

/// One rail tool's target: an icon over its name, or the icon alone.
pub(super) fn rail_button_size(names: bool) -> Vec2 {
    if names {
        Vec2::new(72.0, 42.0)
    } else {
        Vec2::new(48.0, 30.0)
    }
}

/// The space above Options at the rail's foot, its separator included.
const RAIL_FOOT_GAP: f32 = 12.0;

fn rail_icon_color(ui: &egui::Ui, active: bool) -> Color32 {
    if !ui.is_enabled() {
        roles().muted.gamma_multiply(0.5)
    } else if active {
        roles().tools[0].1
    } else {
        roles().legend
    }
}

/// A rail tool. With names on, the tool's accessible name is printed under
/// its icon in the text face; the name, toggle state and hint stay the same
/// in both states.
pub(super) fn rail_tool(
    ui: &mut egui::Ui,
    icons: &RailIcons,
    icon: RailIcon,
    active: bool,
    names: bool,
) -> Response {
    if !names {
        return rail_button(ui, icons, icon, active);
    }
    let size = rail_button_size(true);
    let width = size.x.min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, size.y), Sense::click());
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    controls::icon_button_surface(ui, &response, active);
    let color = rail_icon_color(ui, active);
    ui.painter().image(
        icons.texture_id(icon),
        icons.icon_rect(ui.ctx(), Pos2::new(rect.center().x, rect.top() + 13.0)),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        color,
    );
    let text_color = if !ui.is_enabled() {
        color
    } else if active || response.hovered() {
        roles().text
    } else {
        roles().legend
    };
    // A long name steps down half a point at a time rather than clipping.
    let mut font = 11.0;
    let galley = loop {
        let galley = ui.painter().layout_no_wrap(
            icon.label().to_owned(),
            egui::FontId::proportional(font),
            text_color,
        );
        if galley.size().x <= rect.width() - 4.0 || font <= 9.0 {
            break galley;
        }
        font -= 0.5;
    };
    let origin = Pos2::new(
        rect.center().x - galley.size().x / 2.0,
        rect.bottom() - 4.0 - galley.size().y,
    );
    ui.painter().galley(origin, galley, text_color);
    response.widget_info(|| {
        if icon.is_toggle() {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                ui.is_enabled(),
                active,
                icon.label(),
            )
        } else {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), icon.label())
        }
    });
    response.hover_hint(icon.hint())
}

/// The button at the rail's top that shows or hides the tool names.
pub(super) fn rail_names_switch(ui: &mut egui::Ui, names: bool) -> Response {
    let label = if names {
        "Shrink the tool rail to icons"
    } else {
        "Show the tool names"
    };
    let width = rail_button_size(names).x.min(ui.available_width().max(1.0));
    let height = if names { 20.0 } else { 16.0 };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    focus_outline(ui, &response, roles().accent);
    // A chevron that points where the rail goes: left to shrink, right to widen.
    let direction = if names { -1.0 } else { 1.0 };
    let color = if response.hovered() {
        roles().text
    } else {
        roles().muted
    };
    let stroke = Stroke::new(1.5_f32, color);
    for offset in [-3.0, 3.0] {
        let tip = rect.center() + Vec2::new(offset + direction * 2.5, 0.0);
        let back = tip - Vec2::new(direction * 4.0, 0.0);
        ui.painter()
            .line_segment([back + Vec2::new(0.0, -4.0), tip], stroke);
        ui.painter()
            .line_segment([back + Vec2::new(0.0, 4.0), tip], stroke);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response.hover_hint(label)
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

pub(super) fn panel_switch(ui: &mut egui::Ui, label: &str, on: bool) -> Response {
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

/// A group heading over the rail's tools: a short uppercase title with
/// names on, a separator with icons only (none above the first group).
pub(super) fn rail_group(ui: &mut egui::Ui, title: &str, names: bool, first: bool) {
    if !names {
        if !first {
            controls::rail_separator(ui);
        }
        return;
    }
    let height = if first { 14.0 } else { 20.0 };
    let width = rail_button_size(true).x.min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let galley = ui.painter().layout_no_wrap(
        title.to_uppercase(),
        egui::FontId::proportional(9.5),
        roles().muted,
    );
    ui.painter().galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.bottom() - 1.0 - galley.size().y,
        ),
        galley,
        roles().muted,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, title));
}

pub(super) fn category_button(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    // A category can have the name of a tool, such as Step synth.
    let action = format!("Show {label} sounds");
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).color(if active {
            roles().text
        } else {
            roles().muted
        }))
        .fill(if active { roles().hover } else { roles().panel })
        .stroke(Stroke::new(
            1.0_f32,
            if active { roles().muted } else { roles().panel },
        ))
        .min_size(Vec2::new(40.0, 26.0)),
    );
    focus_outline(ui, &response, roles().accent);
    name_control(response, &action).hover_hint(action)
}

pub(super) fn group_color(group: &str) -> Color32 {
    match group.to_ascii_lowercase().as_str() {
        "loop" => Color32::from_rgb(212, 193, 130),
        "drum" => Color32::from_rgb(217, 157, 120),
        "bass" => Color32::from_rgb(162, 196, 133),
        "sequence" => Color32::from_rgb(137, 176, 210),
        "sphere" | "wave" => Color32::from_rgb(127, 190, 185),
        "effect" => Color32::from_rgb(217, 145, 146),
        "voice" => Color32::from_rgb(185, 158, 203),
        _ => Color32::from_rgb(187, 179, 159),
    }
}

/// A sound tile's category tint over its fill.
pub(super) fn tile_tint(category: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(category.r(), category.g(), category.b(), 51)
}
/// The opacity of a sound tile's category and length line.
pub(super) const TILE_DETAIL_ALPHA: f32 = 0.9;

/// Width of a one-bar sound tile and the gap between tiles.
pub(super) const TILE_BAR: f32 = 96.0;
pub(super) const TILE_GAP: f32 = 8.0;

/// A tile spans one bar unit per bar plus the gaps between them, so tiles
/// of different lengths align. A sound longer than a row fills the row.
pub(super) fn tile_width(bars: u32, row_width: f32) -> f32 {
    (bars.max(1) as f32 * (TILE_BAR + TILE_GAP) - TILE_GAP).min(row_width.max(TILE_BAR))
}

/// The index of the first tile in each row when tiles wrap at `row_width`.
pub(super) fn tile_rows(widths: impl IntoIterator<Item = f32>, row_width: f32) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut x = 0.0;
    for (index, width) in widths.into_iter().enumerate() {
        if index == 0 || x + width > row_width {
            starts.push(index);
            x = 0.0;
        }
        x += width + TILE_GAP;
    }
    starts
}

pub(super) fn sample_button(
    ui: &mut egui::Ui,
    sample: &Sample,
    product: Option<Product>,
    selected: bool,
    width: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 46.0), Sense::click_and_drag());
    let color = group_color(&sample.group_name);
    let painter = ui.painter_at(rect);
    let radius = roles().corner(5.0);
    theme::surface(
        &painter,
        rect,
        radius,
        if response.hovered() || selected {
            roles().tile_hover
        } else {
            roles().tile
        },
    );
    painter.rect_filled(rect, radius, tile_tint(color));
    if selected || response.has_focus() {
        painter.rect_stroke(
            rect.shrink(0.5),
            radius,
            Stroke::new(1.0_f32, color),
            egui::StrokeKind::Inside,
        );
    }
    let text = painter.layout_job(elided(
        &sample.name,
        12.0,
        roles().text,
        rect.width() - 16.0,
        1,
    ));
    let clip = painter.with_clip_rect(rect.shrink2(Vec2::new(8.0, 3.0)));
    clip.galley(rect.min + Vec2::new(8.0, 5.0), text, roles().text);
    let bars = format!(
        "{} bar{}",
        sample.bars,
        if sample.bars == 1 { "" } else { "s" }
    );
    let font = FontId::proportional(10.0);
    let muted = roles().text.gamma_multiply(TILE_DETAIL_ALPHA);
    let mut metadata = painter.layout_no_wrap(
        format!("{}  /  {bars}", sample.group_name),
        font.clone(),
        muted,
    );
    // A narrow tile keeps the bar count; its tint still names the category.
    if metadata.size().x > rect.width() - 16.0 {
        metadata = painter.layout_no_wrap(bars, font, muted);
    }
    clip.galley(
        rect.min + Vec2::new(8.0, 32.0 - metadata.size().y / 2.0),
        metadata,
        muted,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &sample.name)
    });
    // Build the details only for the tile under the pointer.
    response.hover_details(|ui| {
        ui.label(sample_details(sample, product));
    })
}

pub(super) fn sample_details(sample: &Sample, product: Option<Product>) -> String {
    let mut rows = vec![
        sample.name.clone(),
        format!(
            "{} / {}",
            product.map_or("User WAV", Product::label),
            sample.group_name
        ),
        format!(
            "{} bar{} · {} BPM",
            sample.bars,
            if sample.bars == 1 { "" } else { "s" },
            sample.source_bpm
        ),
        format!(
            "{} Hz · {} channel{} · {} frames",
            sample.sample_rate,
            sample.channels,
            if sample.channels == 1 { "" } else { "s" },
            sample.frame_count
        ),
    ];
    if sample.sample_rate > 0 {
        rows.push(format!(
            "Audio duration: {:.3} s",
            f64::from(sample.frame_count) / f64::from(sample.sample_rate)
        ));
    }
    if !sample.code.is_empty() {
        rows.push(format!("Code: {}", sample.code));
    }
    rows.push(format!("Project sample ID: {}", sample.id));
    if let Some((_, native_id)) = Product::sample_key(sample.id) {
        rows.push(format!("Native sample ID: {native_id}"));
    }
    if let Some(path) = &sample.wav_path {
        rows.push(format!("WAV: {}", path.display()));
    }
    display_paths(&rows.join("\n"))
}

impl MixJamApp {
    /// The arrangement's automation lane: Off, or the lane that the track
    /// rows show and edit.
    fn automation_selector(&mut self, ui: &mut egui::Ui, show_label: bool) {
        if show_label {
            ui.label(
                egui::RichText::new("Automation")
                    .size(12.0)
                    .color(roles().muted),
            );
        }
        let current = match (self.automation_lane, self.return_lane) {
            (_, Some(bus)) => return_lane_label(bus),
            (Some(lane), None) => lane.label(),
            (None, None) => "Off".into(),
        };
        let shown = (self.automation_lane, self.return_lane);
        let mut choice = shown;
        let response = egui::ComboBox::from_id_salt("automation_lane")
            .selected_text(current)
            .width(112.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(choice == (None, None), "Off").clicked() {
                    choice = (None, None);
                    ui.close();
                }
                for lane in automation::Lane::ALL {
                    if ui
                        .selectable_label(choice == (Some(lane), None), lane.label())
                        .clicked()
                    {
                        choice = (Some(lane), None);
                        ui.close();
                    }
                }
                ui.separator();
                for bus in 0..mixjam_techno::application::fx::BUSES {
                    if ui
                        .selectable_label(choice == (None, Some(bus)), return_lane_label(bus))
                        .clicked()
                    {
                        choice = (None, Some(bus));
                        ui.close();
                    }
                }
            })
            .response;
        name_control(response, "Automation lane").hover_hint(
            "Show and edit one automation lane on every track, or the return lane of one FX \
             bus across the arrangement. Click a lane to add a point, drag a point to move it, \
             and double-click or right-click a point to delete it. Points snap to 1/16 bar; \
             hold Ctrl for single ticks. Off edits clips again.",
        );
        if choice != shown {
            (self.automation_lane, self.return_lane) = choice;
            self.automation_drag = None;
            self.session.end_automation_gesture();
        }
    }

    /// Whether the arrangement shows an automation lane, whose points the
    /// lanes then edit instead of clips.
    pub(super) fn automating(&self) -> bool {
        self.automation_lane.is_some() || self.return_lane.is_some()
    }

    /// Draw FX bus `bus`'s return lane across the visible rows and edit its
    /// points with the pointer, as a track's lane is edited. The lane runs
    /// from silence at the bottom to `RETURN_CEILING` at the top; a dashed
    /// line marks the full return, 100%.
    fn return_lane(
        &mut self,
        ui: &egui::Ui,
        response: &Response,
        painter: &egui::Painter,
        rect: Rect,
        visible: Rect,
        bus: usize,
    ) {
        painter.rect_filled(rect, 0.0, roles().canvas.gamma_multiply(0.45));
        let row = return_row(rect, visible);
        let bar_width = self.timeline_bar_width;
        let x = |tick: u32| rect.left() + tick as f32 / TICKS_PER_BAR as f32 * bar_width;
        let points = self.session.project().return_automation[bus].clone();
        let knob = self.session.project().fx.buses[bus].return_level;
        painter.text(
            Pos2::new(visible.left() + 8.0, visible.top() + 4.0),
            Align2::LEFT_TOP,
            format!("{} (100% dashed)", return_lane_label(bus)),
            FontId::proportional(12.0),
            roles().muted,
        );
        let full = return_y(1.0, row);
        painter.add(egui::Shape::dashed_line(
            &[Pos2::new(rect.left(), full), Pos2::new(rect.right(), full)],
            Stroke::new(1.0_f32, roles().line),
            3.0,
            6.0,
        ));
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let mut hit = None;
        if points.is_empty() {
            // The Return knob, which the bus keeps without points.
            let y = return_y(knob, row);
            painter.add(egui::Shape::dashed_line(
                &[Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(1.0_f32, roles().muted.gamma_multiply(0.6)),
                6.0,
                5.0,
            ));
        } else {
            let markers: Vec<Pos2> = points
                .iter()
                .map(|point| Pos2::new(x(point.tick), return_y(point.value, row)))
                .collect();
            let mut line = vec![Pos2::new(rect.left(), markers[0].y)];
            line.extend(markers.iter().copied());
            line.push(Pos2::new(rect.right(), markers[markers.len() - 1].y));
            painter.add(egui::Shape::line(
                line,
                Stroke::new(1.6_f32, roles().accent),
            ));
            for (index, point) in markers.iter().enumerate() {
                let active = self.automation_drag == Some((bus, index));
                let hovered = pointer.is_some_and(|pos| point.distance(pos) <= AUTOMATION_REACH);
                if hovered {
                    hit = Some(index);
                }
                painter.circle(
                    *point,
                    if active || hovered { 4.5 } else { 3.5 },
                    if active { roles().text } else { roles().accent },
                    Stroke::new(1.0_f32, roles().canvas),
                );
            }
        }
        if let Some(index) = self.automation_drag.map(|(_, index)| index).or(hit)
            && let Some(point) = points.get(index)
        {
            response.clone().hover_hint_at_pointer(format!(
                "{}, bar {:.2}: {}",
                return_lane_label(bus),
                f64::from(point.tick) / f64::from(TICKS_PER_BAR) + 1.0,
                automation::return_label(point.value)
            ));
        }
        let free = ui.input(|i| i.modifiers.ctrl);
        let hit_at = |pos: Pos2, points: &[automation::Point]| {
            points.iter().position(|point| {
                Pos2::new(x(point.tick), return_y(point.value, row)).distance(pos)
                    <= AUTOMATION_REACH
            })
        };
        // A click adds a point; a drag from the lane adds one and moves it.
        if response.drag_started()
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            match self.session.begin_return_gesture(bus) {
                Ok(()) => {
                    self.automation_drag = hit_at(origin, &points)
                        .or_else(|| self.return_add(origin, rect, row, bus, free))
                        .map(|index| (bus, index));
                    if self.automation_drag.is_none() {
                        self.session.end_automation_gesture();
                    }
                }
                Err(error) => self.error(error),
            }
        }
        if let Some((_, index)) = self.automation_drag {
            if response.dragged()
                && let Some(pos) = pointer
            {
                let mut points = self.session.project().return_automation[bus].clone();
                if index < points.len() {
                    let low = index
                        .checked_sub(1)
                        .map_or(0, |before| points[before].tick + 1);
                    let high = points
                        .get(index + 1)
                        .map_or(MAX_TICKS, |after| after.tick - 1);
                    let tick = self.automation_tick(pos.x, rect, free);
                    let point = automation::Point {
                        tick: if low <= high {
                            tick.clamp(low, high)
                        } else {
                            points[index].tick
                        },
                        value: return_value(pos.y, row),
                    };
                    if points[index] != point {
                        points[index] = point;
                        let result = self
                            .session
                            .edit(EditCommand::SetReturnAutomation { bus, points });
                        self.apply(result);
                    }
                }
            }
            if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
                self.automation_drag = None;
                self.session.end_automation_gesture();
            }
        }
        if let Some(pos) = response.interact_pointer_pos() {
            let points = self.session.project().return_automation[bus].clone();
            let hit = hit_at(pos, &points);
            if response.double_clicked() || response.secondary_clicked() {
                if let Some(index) = hit {
                    let mut points = points;
                    points.remove(index);
                    let result = self
                        .session
                        .edit(EditCommand::SetReturnAutomation { bus, points });
                    self.apply(result);
                }
            } else if response.clicked() && hit.is_none() {
                self.return_add(pos, rect, row, bus, free);
            }
        }
    }

    /// Add a point to FX bus `bus`'s return lane where `pos` is, or set the
    /// value of the point already at its tick. Returns the point's index.
    fn return_add(
        &mut self,
        pos: Pos2,
        rect: Rect,
        row: Rect,
        bus: usize,
        free: bool,
    ) -> Option<usize> {
        if !row.y_range().contains(pos.y) {
            return None;
        }
        let tick = self.automation_tick(pos.x, rect, free);
        let value = return_value(pos.y, row);
        let mut points = self.session.project().return_automation[bus].clone();
        let index = points.partition_point(|point| point.tick < tick);
        if points.get(index).is_some_and(|point| point.tick == tick) {
            points[index].value = value;
        } else {
            points.insert(index, automation::Point { tick, value });
        }
        let result = self
            .session
            .edit(EditCommand::SetReturnAutomation { bus, points });
        let added = result.is_ok();
        self.apply(result);
        added.then_some(index)
    }

    /// The lane's row rectangle for a track the arrangement shows.
    fn automation_row(&self, track: usize, rect: Rect) -> Option<Rect> {
        let row = self.timeline_row_by_track[track]?;
        Some(Rect::from_min_size(
            Pos2::new(
                rect.left(),
                rect.top() + row as f32 * self.timeline_row_height,
            ),
            Vec2::new(rect.width(), self.timeline_row_height),
        ))
    }

    fn automation_tick(&self, x: f32, rect: Rect, free: bool) -> u32 {
        let raw = (x - rect.left()).max(0.0) / self.timeline_bar_width * TICKS_PER_BAR as f32;
        let step = if free {
            1.0
        } else {
            (TICKS_PER_BAR / 16) as f32
        };
        ((raw / step).round() * step) as u32
    }

    /// Draw `lane` on each row and edit its points with the pointer.
    fn automation_lanes(
        &mut self,
        ui: &egui::Ui,
        response: &Response,
        painter: &egui::Painter,
        rect: Rect,
        lane: automation::Lane,
        tracks: &[usize],
    ) {
        // Dim the clips so the lines stand out.
        painter.rect_filled(rect, 0.0, roles().canvas.gamma_multiply(0.45));
        let project = self.session.project();
        let mut hit = None;
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let near = |a: Pos2, b: Pos2| a.distance(b) <= AUTOMATION_REACH;
        for &track in tracks {
            let Some(row) = self.automation_row(track, rect) else {
                continue;
            };
            let points = project.tracks[track].automation.lane(lane);
            let x = |tick: u32| {
                rect.left() + tick as f32 / TICKS_PER_BAR as f32 * self.timeline_bar_width
            };
            if points.is_empty() {
                // The value the track keeps without points.
                let value = match lane {
                    automation::Lane::Send(bus) => project.tracks[track].sends[bus],
                    automation::Lane::Pan => project.tracks[track].pan,
                    _ => lane.before_first(),
                };
                let y = automation_y(lane, value, row);
                painter.add(egui::Shape::dashed_line(
                    &[Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(1.0_f32, roles().muted.gamma_multiply(0.6)),
                    6.0,
                    5.0,
                ));
                continue;
            }
            let markers: Vec<Pos2> = points
                .iter()
                .map(|point| Pos2::new(x(point.tick), automation_y(lane, point.value, row)))
                .collect();
            // A stepped lane holds its resting value before its first point
            // (Repeat off, the gate at 100%) and each value until the next
            // point; the Pitch lane rests at 0 before its first point and
            // then moves in straight lines; the others hold their first
            // value and move in straight lines.
            let mut line = vec![Pos2::new(
                rect.left(),
                if lane.rests_before_first() {
                    automation_y(lane, lane.before_first(), row)
                } else {
                    markers[0].y
                },
            )];
            for (index, marker) in markers.iter().enumerate() {
                if lane.steps() || (index == 0 && lane.rests_before_first()) {
                    line.push(Pos2::new(marker.x, line[line.len() - 1].y));
                }
                line.push(*marker);
            }
            line.push(Pos2::new(rect.right(), markers[markers.len() - 1].y));
            painter.add(egui::Shape::line(
                line,
                Stroke::new(1.6_f32, roles().accent),
            ));
            for (index, point) in markers.iter().enumerate() {
                let active = self.automation_drag == Some((track, index));
                let hovered = pointer.is_some_and(|pos| near(*point, pos));
                if hovered {
                    hit = Some((track, index));
                }
                painter.circle(
                    *point,
                    if active || hovered { 4.5 } else { 3.5 },
                    if active { roles().text } else { roles().accent },
                    Stroke::new(1.0_f32, roles().canvas),
                );
            }
        }
        if let Some((track, index)) = self.automation_drag.or(hit)
            && let Some(point) = project.tracks[track].automation.lane(lane).get(index)
        {
            response.clone().hover_hint_at_pointer(format!(
                "Track {}, bar {:.2}: {}",
                track + 1,
                f64::from(point.tick) / f64::from(TICKS_PER_BAR) + 1.0,
                lane.format(point.value)
            ));
        }
        let free = ui.input(|i| i.modifiers.ctrl);
        // A click on a lane adds a point; a drag from a lane adds one and moves it.
        if response.drag_started()
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            let hit = self.automation_hit(origin, rect, lane, tracks);
            let track = hit.map(|(track, _)| track).or_else(|| {
                self.timeline_track_at(origin)
                    .filter(|track| tracks.contains(track))
            });
            if let Some(track) = track {
                match self.session.begin_automation_gesture(track, lane) {
                    Ok(()) => {
                        self.automation_drag =
                            hit.or_else(|| self.automation_add(origin, rect, lane, tracks, free));
                        if self.automation_drag.is_none() {
                            self.session.end_automation_gesture();
                        }
                    }
                    Err(error) => self.error(error),
                }
            }
        }
        if let Some((track, index)) = self.automation_drag {
            if response.dragged()
                && let Some(pos) = pointer
                && let Some(row) = self.automation_row(track, rect)
            {
                let mut points = self.session.project().tracks[track]
                    .automation
                    .lane(lane)
                    .to_vec();
                if index < points.len() {
                    let low = index
                        .checked_sub(1)
                        .map_or(0, |before| points[before].tick + 1);
                    let high = points
                        .get(index + 1)
                        .map_or(MAX_TICKS, |after| after.tick - 1);
                    let tick = self.automation_tick(pos.x, rect, free);
                    let point = automation::Point {
                        tick: if low <= high {
                            tick.clamp(low, high)
                        } else {
                            points[index].tick
                        },
                        value: automation_value(lane, pos.y, row),
                    };
                    if points[index] != point {
                        points[index] = point;
                        let result = self.session.edit(EditCommand::SetAutomation {
                            track,
                            lane,
                            points,
                        });
                        self.apply(result);
                    }
                }
            }
            if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
                self.automation_drag = None;
                self.session.end_automation_gesture();
            }
        }
        if let Some(pos) = response.interact_pointer_pos() {
            let hit = self.automation_hit(pos, rect, lane, tracks);
            if response.double_clicked() || response.secondary_clicked() {
                if let Some((track, index)) = hit {
                    let mut points = self.session.project().tracks[track]
                        .automation
                        .lane(lane)
                        .to_vec();
                    points.remove(index);
                    let result = self.session.edit(EditCommand::SetAutomation {
                        track,
                        lane,
                        points,
                    });
                    self.apply(result);
                }
            } else if response.clicked() && hit.is_none() {
                self.automation_add(pos, rect, lane, tracks, free);
            }
        }
    }

    /// The point of `lane` within reach of `pos`, as its track and index.
    fn automation_hit(
        &self,
        pos: Pos2,
        rect: Rect,
        lane: automation::Lane,
        tracks: &[usize],
    ) -> Option<(usize, usize)> {
        tracks.iter().find_map(|&track| {
            let row = self.automation_row(track, rect)?;
            self.session.project().tracks[track]
                .automation
                .lane(lane)
                .iter()
                .position(|point| {
                    let at = Pos2::new(
                        rect.left()
                            + point.tick as f32 / TICKS_PER_BAR as f32 * self.timeline_bar_width,
                        automation_y(lane, point.value, row),
                    );
                    at.distance(pos) <= AUTOMATION_REACH
                })
                .map(|index| (track, index))
        })
    }

    /// Add a point of `lane` where `pos` is, or set the value of the point
    /// already at its tick. Returns the point's track and index.
    fn automation_add(
        &mut self,
        pos: Pos2,
        rect: Rect,
        lane: automation::Lane,
        tracks: &[usize],
        free: bool,
    ) -> Option<(usize, usize)> {
        let track = self
            .timeline_track_at(pos)
            .filter(|track| tracks.contains(track))?;
        let row = self.automation_row(track, rect)?;
        let tick = self.automation_tick(pos.x, rect, free);
        let value = automation_value(lane, pos.y, row);
        let mut points = self.session.project().tracks[track]
            .automation
            .lane(lane)
            .to_vec();
        let index = points.partition_point(|point| point.tick < tick);
        if points.get(index).is_some_and(|point| point.tick == tick) {
            points[index].value = value;
        } else {
            points.insert(index, automation::Point { tick, value });
        }
        let result = self.session.edit(EditCommand::SetAutomation {
            track,
            lane,
            points,
        });
        let added = result.is_ok();
        self.apply(result);
        added.then_some((track, index))
    }
}

/// How far from a point, in points, the pointer still takes it.
const AUTOMATION_REACH: f32 = 7.0;

/// Where a lane value sits in a track row, 4 points in from its edges. The
/// Repeat lane's values sit at even heights, from Off at the bottom to 1/64
/// at the top.
fn automation_y(lane: automation::Lane, value: f32, row: Rect) -> f32 {
    let inner = row.shrink2(Vec2::new(0.0, 4.0));
    let along = if lane == automation::Lane::Repeat {
        let steps = &automation::REPEAT_STEPS;
        let step = steps.iter().position(|step| *step >= value).unwrap_or(0);
        step as f32 / (steps.len() - 1) as f32
    } else {
        let (low, high) = lane.range();
        (value - low) / (high - low)
    };
    inner.bottom() - along * inner.height()
}

/// The lane value at a height in a track row, rounded to the lane's steps.
fn automation_value(lane: automation::Lane, y: f32, row: Rect) -> f32 {
    let (low, high) = lane.range();
    let inner = row.shrink2(Vec2::new(0.0, 4.0));
    let along = (inner.bottom() - y) / inner.height();
    let value = low + along * (high - low);
    let value = match lane {
        automation::Lane::Volume => (value * 2.0).round() / 2.0,
        automation::Lane::Send(_) => (value * 100.0).round() / 100.0,
        // The middle opens the filter, or centers the pan.
        automation::Lane::Filter | automation::Lane::Pan if value.abs() < 0.04 => 0.0,
        automation::Lane::Filter | automation::Lane::Pan => (value * 100.0).round() / 100.0,
        // The nearest of the evenly spaced steps.
        automation::Lane::Repeat => {
            let steps = &automation::REPEAT_STEPS;
            let last = (steps.len() - 1) as f32;
            steps[(along * last).round().clamp(0.0, last) as usize]
        }
        // Whole steps of 5%.
        automation::Lane::RepeatGate => {
            (value / automation::REPEAT_GATE_STEP).round() * automation::REPEAT_GATE_STEP
        }
        // Whole semitones.
        automation::Lane::Pitch => (value / 100.0).round() * 100.0,
    };
    value.clamp(low, high)
}

/// The name of FX bus `bus`'s return lane in the lane menu: "FX 1 return".
fn return_lane_label(bus: usize) -> String {
    format!("FX {} return", bus + 1)
}

/// The return lane's row: the visible rows of the arrangement, below its
/// name and 4 points above the bottom.
pub(super) fn return_row(rect: Rect, visible: Rect) -> Rect {
    let top = visible.top() + 22.0;
    let bottom = (visible.bottom() - 4.0).max(top + 8.0);
    Rect::from_x_y_ranges(rect.x_range(), top..=bottom)
}

/// Where a return lane value sits in its row.
pub(super) fn return_y(value: f32, row: Rect) -> f32 {
    row.bottom() - value / automation::RETURN_CEILING * row.height()
}

/// The return lane value at a height in its row, in whole percent. The
/// full return, 100%, holds the pointer near it.
fn return_value(y: f32, row: Rect) -> f32 {
    let value = (row.bottom() - y) / row.height() * automation::RETURN_CEILING;
    let value = if (value - 1.0).abs() < 0.04 {
        1.0
    } else {
        (value * 100.0).round() / 100.0
    };
    value.clamp(0.0, automation::RETURN_CEILING)
}

/// Mark a clip that plays backwards: "rev" in its bottom right corner.
fn draw_reverse_mark(painter: &egui::Painter, rect: Rect) -> Option<Rect> {
    // The shortest lane height still shows it.
    if rect.width() < 36.0 || rect.height() < 16.0 {
        return None;
    }
    Some(
        painter
            .with_clip_rect(painter.clip_rect().intersect(rect.shrink(2.0)))
            .text(
                rect.right_bottom() + Vec2::new(-6.0, -3.0),
                Align2::RIGHT_BOTTOM,
                "rev",
                FontId::proportional(10.0),
                roles().on_light,
            ),
    )
}

/// Shade a clip's fades above their curves. The fade out ends where the
/// clip's sound ends, which can be before the clip's end.
fn draw_fades(
    painter: &egui::Painter,
    rect: Rect,
    clip: &Clip,
    sample: Option<&Sample>,
    bar_width: f32,
    bpm: f64,
) {
    if clip.fades.is_none() {
        return;
    }
    let tick_width = bar_width / TICKS_PER_BAR as f32;
    let sound_end = sample.map_or(rect.right(), |sample| {
        (rect.left() + sound_ticks(clip, sample, bpm) as f32 * tick_width).min(rect.right())
    });
    let clipped = painter.with_clip_rect(painter.clip_rect().intersect(rect));
    let fade_in = clip.fades.fade_in;
    let fade_out = clip.fades.fade_out;
    let spans = [
        (
            rect.left(),
            rect.left() + fade_in.ticks as f32 * tick_width,
            fade_in,
            true,
        ),
        (
            sound_end - fade_out.ticks as f32 * tick_width,
            sound_end,
            fade_out,
            false,
        ),
    ];
    for (from, to, fade, rising) in spans {
        if fade.ticks == 0 || to <= from {
            continue;
        }
        const STEPS: usize = 24;
        let points: Vec<Pos2> = (0..=STEPS)
            .map(|step| {
                let t = step as f32 / STEPS as f32;
                let gain = fade.curve.gain(if rising { t } else { 1.0 - t });
                Pos2::new(from + (to - from) * t, rect.bottom() - gain * rect.height())
            })
            .collect();
        let mut mesh = egui::Mesh::default();
        let shade = Color32::from_black_alpha(110);
        for pair in points.windows(2) {
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(Pos2::new(pair[0].x, rect.top()), shade);
            mesh.colored_vertex(pair[0], shade);
            mesh.colored_vertex(Pos2::new(pair[1].x, rect.top()), shade);
            mesh.colored_vertex(pair[1], shade);
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 2);
        }
        clipped.add(egui::Shape::mesh(mesh));
        clipped.add(egui::Shape::line(
            points,
            Stroke::new(1.0_f32, Color32::from_black_alpha(190)),
        ));
    }
}

/// Source frames in one tick at the project's effective source tempo.
fn frames_per_tick(sample: &Sample, bpm: f64) -> f64 {
    f64::from(sample.sample_rate) * 240.0 / bpm / f64::from(TICKS_PER_BAR)
}

/// The ticks from a clip's start to where its sound ends, at most its length.
fn sound_ticks(clip: &Clip, sample: &Sample, bpm: f64) -> u32 {
    // Reversed, the sound ends with the clip.
    if sample.sample_rate == 0 || clip.reverse {
        return clip.length_ticks;
    }
    let frames = f64::from(sample.frame_count.saturating_sub(clip.offset_frames));
    ((frames / frames_per_tick(sample, bpm)) as u32).min(clip.length_ticks)
}

/// The height of a clip's gain line: unity in the middle, +6 dB at the top.
fn gain_y(rect: Rect, gain: f32) -> f32 {
    rect.bottom() - (gain.min(2.0) / 2.0 * rect.height()).max(2.0)
}

/// The centers of a clip's fade handles on its top edge: where the fade in
/// ends and where the fade out starts. A handle without a fade sits in its corner.
fn fade_handles(rect: Rect, clip: &Clip, sample: &Sample, bar_width: f32, bpm: f64) -> [Pos2; 2] {
    let tick_width = bar_width / TICKS_PER_BAR as f32;
    let sound_end =
        (rect.left() + sound_ticks(clip, sample, bpm) as f32 * tick_width).min(rect.right());
    let inset = FADE_HANDLE / 2.0;
    let place = |x: f32| {
        Pos2::new(
            x.clamp(
                rect.left() + inset,
                (rect.right() - inset).max(rect.left() + inset),
            ),
            rect.top() + inset + 1.0,
        )
    };
    [
        place(rect.left() + clip.fades.fade_in.ticks as f32 * tick_width),
        place(sound_end - clip.fades.fade_out.ticks as f32 * tick_width),
    ]
}

/// The side of a fade handle's square.
const FADE_HANDLE: f32 = 9.0;

/// The vertical travel of one gain step: one dB, or a tenth with Ctrl.
const GAIN_PIXELS: f32 = 4.0;
/// The size of the gain handle.
const GAIN_HANDLE: Vec2 = Vec2::new(16.0, 8.0);

/// The center of a clip's gain handle: on the gain line, in the middle of
/// the part of the clip in view.
pub(super) fn gain_handle(rect: Rect, visible: Rect, gain: f32) -> Pos2 {
    let shown = rect.intersect(visible);
    let x = if shown.width() > 0.0 {
        shown.center().x
    } else {
        rect.center().x
    };
    Pos2::new(x, gain_y(rect, gain))
}

/// The handle under `pos` in a clip's rectangle. The fade handles sit on the
/// top edge, the trims take the side edges, and the gain handle sits on the
/// gain line in the middle of the part in `visible`. The rest moves the clip.
fn handle_at(
    pos: Pos2,
    rect: Rect,
    visible: Rect,
    clip: &Clip,
    sample: &Sample,
    bar_width: f32,
    bpm: f64,
) -> Option<ClipHandle> {
    // A narrow clip is only for moving.
    if rect.width() < 16.0 {
        return None;
    }
    // The gain handle is painted last. It must receive input where it
    // covers a fade handle or a trim edge.
    if Rect::from_center_size(
        gain_handle(rect, visible, clip.gain),
        GAIN_HANDLE + Vec2::splat(2.0),
    )
    .contains(pos)
    {
        return Some(ClipHandle::Gain);
    }
    if pos.y <= rect.top() + 12.0 {
        let [fade_in, fade_out] = fade_handles(rect, clip, sample, bar_width, bpm);
        let (to_in, to_out) = ((pos.x - fade_in.x).abs(), (pos.x - fade_out.x).abs());
        if to_in.min(to_out) <= 7.0 {
            return Some(if to_in <= to_out {
                ClipHandle::FadeIn
            } else {
                ClipHandle::FadeOut
            });
        }
    }
    if pos.x <= rect.left() + 5.0 {
        Some(ClipHandle::Start)
    } else if pos.x >= rect.right() - 5.0 {
        Some(ClipHandle::End)
    } else {
        None
    }
}

/// The gain handle of a hovered clip, which drags the gain line.
fn draw_gain_handle(painter: &egui::Painter, rect: Rect, visible: Rect, gain: f32) {
    if rect.width() < 16.0 {
        return;
    }
    let handle = Rect::from_center_size(gain_handle(rect, visible, gain), GAIN_HANDLE);
    painter.rect_filled(handle, 2.0, roles().on_light);
    painter.rect_stroke(
        handle,
        2.0,
        Stroke::new(1.0_f32, roles().text),
        egui::StrokeKind::Inside,
    );
}

fn draw_fade_handles(
    painter: &egui::Painter,
    rect: Rect,
    clip: &Clip,
    sample: &Sample,
    bar_width: f32,
    bpm: f64,
) {
    if rect.width() < 16.0 {
        return;
    }
    // A light rim keeps the dark squares visible on fills and fade shading.
    for center in fade_handles(rect, clip, sample, bar_width, bpm) {
        let square = Rect::from_center_size(center, Vec2::splat(FADE_HANDLE));
        painter.rect_filled(square, 1.5, roles().on_light);
        painter.rect_stroke(
            square,
            1.5,
            Stroke::new(1.0_f32, roles().text),
            egui::StrokeKind::Inside,
        );
    }
}

/// What a handle drag changes, for the label at the pointer.
fn handle_readout(handle: ClipHandle, clip: &Clip) -> String {
    let beats = |ticks: u32| f64::from(ticks) / f64::from(TICKS_PER_BEAT);
    match handle {
        ClipHandle::Start => format!(
            "Start: bar {:.2}",
            f64::from(clip.start_tick) / f64::from(TICKS_PER_BAR) + 1.0
        ),
        ClipHandle::End => format!(
            "Length: {:.2} bars",
            f64::from(clip.length_ticks) / f64::from(TICKS_PER_BAR)
        ),
        ClipHandle::FadeIn => format!("Fade in: {:.2} beats", beats(clip.fades.fade_in.ticks)),
        ClipHandle::FadeOut => format!("Fade out: {:.2} beats", beats(clip.fades.fade_out.ticks)),
        ClipHandle::Gain if clip.gain == 0.0 => "Gain: -inf dB".into(),
        ClipHandle::Gain => format!("Gain: {:+.1} dB", clip_gain_db(clip.gain)),
    }
}

/// Ticks in one beat, the unit of the fade controls.
const TICKS_PER_BEAT: u32 = TICKS_PER_BAR / 4;

/// A clip fade's length in beats, at most `room` ticks, and its curve: one
/// row of the clip properties grid.
fn fade_controls(ui: &mut egui::Ui, edge: &str, fade: &mut Fade, room: u32) -> bool {
    ui.label(format!("Fade {edge} (beats)"));
    ui.horizontal(|ui| {
        let mut beats = f64::from(fade.ticks) / f64::from(TICKS_PER_BEAT);
        let mut change = false;
        if name_control(
            ui.add(
                egui::DragValue::new(&mut beats)
                    .range(0.0..=f64::from(room) / f64::from(TICKS_PER_BEAT))
                    .speed(1.0 / 16.0)
                    .max_decimals(3),
            ),
            &format!("Clip fade {edge} (beats)"),
        )
        .hover_hint(if edge == "in" {
            "Fade in from the clip's start, in beats. 0 is no fade."
        } else {
            "Fade out to the end of the clip's sound, in beats: the clip's end, or earlier \
         where the sound runs out. 0 is no fade."
        })
        .changed()
        {
            fade.ticks = ((beats * f64::from(TICKS_PER_BEAT)).round() as u32).min(room);
            change = true;
        }
        // A submenu, because a combo box inside the context menu would close it.
        let name = format!("Clip fade {edge} curve");
        let response = ui
            .menu_button(fade.curve.label(), |ui| {
                for curve in FadeCurve::ALL {
                    if ui
                        .selectable_label(fade.curve == curve, curve.label())
                        .clicked()
                    {
                        change |= fade.curve != curve;
                        fade.curve = curve;
                        ui.close();
                    }
                }
            })
            .response;
        name_control(response, &name).hover_hint(
            "Linear, Equal power for crossfades that keep their loudness, or Exponential, \
         even steps in decibels like a natural decay.",
        );
        change
    })
    .inner
}

/// A clip's pitch as semitones and cents: one row of the clip properties
/// grid. Its timing does not change.
fn pitch_controls(ui: &mut egui::Ui, cents: &mut i32) -> bool {
    let mut semitones = *cents / 100;
    let mut fine = *cents % 100;
    ui.label("Pitch");
    let change = ui
        .horizontal(|ui| {
            let semitone_change = name_control(
                ui.add(
                    egui::DragValue::new(&mut semitones)
                        .range(-24..=24)
                        .speed(0.1)
                        .suffix(" st"),
                ),
                "Clip pitch (semitones)",
            )
            .hover_hint("Transpose the clip in semitones. Its length and timing stay the same.")
            .changed();
            let cent_change = name_control(
                ui.add(
                    egui::DragValue::new(&mut fine)
                        .range(-99..=99)
                        .speed(0.5)
                        .suffix(" ct"),
                ),
                "Clip pitch (cents)",
            )
            .hover_hint("Fine-tune the clip in cents, hundredths of a semitone.")
            .changed();
            semitone_change || cent_change
        })
        .inner;
    if change {
        *cents = (semitones * 100 + fine).clamp(-MAX_PITCH_CENTS, MAX_PITCH_CENTS);
    }
    change
}

pub(super) fn draw_clip(
    painter: &egui::Painter,
    rect: Rect,
    sample: Option<&Sample>,
    missing: Option<&MissingSample>,
    selected: bool,
    reverse: bool,
) {
    let color = if missing.is_some() {
        roles().raised
    } else {
        sample
            .map(|s| group_color(&s.group_name))
            .unwrap_or(roles().muted)
    };
    // The clip's silhouette: a tight tab on Studio, a rounded bubble on Aero,
    // and a painted label on Vintage. Category colors stay the same.
    let radius = clip_radius(rect);
    theme::surface(painter, rect, radius, color);
    if missing.is_some() {
        painter.rect_stroke(
            rect.shrink(0.5),
            radius,
            Stroke::new(1.0_f32, roles().coral),
            egui::StrokeKind::Inside,
        );
    }
    let clipped = painter.with_clip_rect(painter.clip_rect().intersect(rect.shrink(2.0)));
    if selected {
        painter.rect_stroke(
            rect.shrink(0.5),
            radius,
            Stroke::new(2.0_f32, roles().text),
            egui::StrokeKind::Inside,
        );
        // A dark inner line keeps the selection visible on every category fill.
        painter.rect_stroke(
            rect.shrink(2.5),
            (radius - 1.0).max(1.0),
            Stroke::new(1.0_f32, roles().on_light),
            egui::StrokeKind::Inside,
        );
    }
    let reverse_mark = reverse.then(|| draw_reverse_mark(painter, rect)).flatten();
    let label = missing
        .map(|missing| format!("Missing: {}", missing.name))
        .or_else(|| sample.map(|sample| sample.name.clone()));
    if let Some(label) = label {
        let text_color = if missing.is_some() {
            roles().text
        } else {
            roles().on_light
        };
        let size = if rect.height() < 28.0 { 10.0 } else { 12.0 };
        let mut text = clipped.layout_job(elided(
            &label,
            size,
            text_color,
            rect.width() - 12.0,
            clip_label_rows(rect.height()),
        ));
        // The 20-point lane leaves no room below the name at the usual inset.
        let top = if rect.height() < 20.0 { 2.0 } else { 4.0 };
        let at = rect.left_top() + Vec2::new(6.0, top);
        let mut label_painter = clipped;
        if let Some(mark) = reverse_mark
            && text.mesh_bounds.translate(at.to_vec2()).intersects(mark)
        {
            let right = mark.left() - 4.0;
            text = label_painter.layout_job(elided(
                &label,
                size,
                text_color,
                right - at.x,
                clip_label_rows(rect.height()),
            ));
            let mut clip = label_painter.clip_rect();
            clip.max.x = clip.max.x.min(right);
            label_painter = label_painter.with_clip_rect(clip);
        }
        label_painter.galley(at, text, text_color);
    }
}

/// The corner radius of a clip in the current theme. Aero's bubble stays
/// small enough that a clip's label clears its curve.
pub(super) fn clip_radius(rect: Rect) -> f32 {
    match roles().material {
        theme::Material::Flat => 2.0,
        theme::Material::Plastic => (rect.height() / 2.0).min(8.0),
        theme::Material::Metal => 3.0,
    }
}

/// The gain line of a selected or hovered clip. Its handle drags it.
fn draw_gain_line(painter: &egui::Painter, rect: Rect, gain: f32) {
    let y = gain_y(rect, gain);
    painter
        .with_clip_rect(painter.clip_rect().intersect(rect.shrink(2.0)))
        .line_segment(
            [
                Pos2::new(rect.left() + 4.0, y),
                Pos2::new(rect.right() - 4.0, y),
            ],
            Stroke::new(1.5_f32, Color32::from_black_alpha(170)),
        );
}

pub(super) fn missing_clip_details(clip: &Clip) -> String {
    format!(
        "Track {}: start {} ticks; length {} ticks ({:.3} bars); source offset {} frames; gain {}.",
        clip.track + 1,
        clip.start_tick,
        clip.length_ticks,
        clip.length_ticks as f64 / TICKS_PER_BAR as f64,
        clip.offset_frames,
        clip.gain
    )
}

pub(super) fn missing_sample_details(missing: &MissingSample) -> String {
    let mut rows = vec![
        format!("Missing: {}", missing.name),
        "Playback: silence".into(),
        format!("Reason: {}", missing.reason),
        format!("Project sample ID: {}", missing.id),
    ];
    match &missing.reference {
        MissingReference::Native { product, native_id } => {
            rows.push(format!("Original product: {}", product.label()));
            rows.push(format!("Native sample ID: {native_id}"));
        }
        MissingReference::External { path, origin } => {
            rows.push(format!("Original path: {}", path.display()));
            if let Some(product) = origin {
                rows.push(format!("Original product: {}", product.label()));
            }
        }
        MissingReference::Generated {
            product,
            kind,
            data,
        } => {
            rows.push(format!("Original product: {}", product.label()));
            rows.push(format!("Generator: {kind}"));
            rows.push(format!("Generator data: {data}"));
        }
    }
    rows.push(format!("Source tempo: {} BPM", missing.source_bpm));
    rows.push(format!("Trim reference rate: {} Hz", missing.sample_rate));
    rows.push(format!(
        "Source length: {} ticks ({:.3} bars; {})",
        missing.nominal_ticks,
        missing.nominal_ticks as f64 / TICKS_PER_BAR as f64,
        if missing.length_inferred {
            "inferred"
        } else {
            "known"
        }
    ));
    for (name, value) in &missing.metadata {
        rows.push(format!("{name}: {value}"));
    }
    display_paths(&rows.join("\n"))
}

/// A clip shows its name on one line below 38 points of height, else on two.
pub(super) fn clip_label_rows(height: f32) -> usize {
    if height < 38.0 { 1 } else { 2 }
}

/// Text limited to `rows` lines of `width`. The measured glyphs decide where
/// it ends, and an ellipsis on the last visible line marks a shortened name.
pub(super) fn elided(
    text: &str,
    size: f32,
    color: Color32,
    width: f32,
    rows: usize,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        FontId::proportional(size),
        color,
        width.max(1.0),
    );
    job.wrap.max_rows = rows;
    job.wrap.break_anywhere = rows == 1;
    job
}

/// The tooltip of a track switch; the arrangement and the mixer share it.
pub(super) fn switch_hint(label: &str, switch: TrackSwitch, on: bool) -> String {
    let state = if on { "on" } else { "off" };
    match switch {
        TrackSwitch::Left | TrackSwitch::Right => format!(
            "{label}: {state}. With both outputs off this track has no output. Pan stays unchanged."
        ),
        TrackSwitch::Mute => {
            format!("{label}: {state}. A muted track is silent, even when soloed.")
        }
        TrackSwitch::Solo => {
            format!("{label}: {state}. While any track is soloed, only soloed tracks play.")
        }
    }
}

fn route_color(active: bool) -> Color32 {
    if active {
        roles().accent
    } else {
        roles().muted
    }
}

fn switch_letter(painter: &egui::Painter, rect: Rect, on: bool, switch: TrackSwitch) {
    // The bar marks a switch that is on, so the state does not rest on color.
    // It moves up in a short row so it stays inside the button.
    if on {
        let below = (rect.height() / 2.0 - 2.5).min(9.0);
        painter.rect_filled(
            Rect::from_center_size(rect.center() + Vec2::new(0.0, below), Vec2::new(9.0, 2.0)),
            1.0,
            roles().accent,
        );
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        switch.letter(),
        FontId::proportional(10.0),
        route_color(on),
    );
}

#[derive(Clone, Copy)]
pub(super) enum TransportIcon {
    Loop,
    Start,
    Back,
    Stop,
    Pause,
    Play,
    Forward,
    End,
    Record,
}

pub(super) fn transport_button(
    ui: &mut egui::Ui,
    size: f32,
    icon: TransportIcon,
    active: bool,
    tooltip: &str,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let primary = matches!(icon, TransportIcon::Play | TransportIcon::Pause);
    let fill = if primary {
        roles().accent
    } else if response.hovered() {
        roles().hover
    } else {
        roles().raised
    };
    theme::surface(ui.painter(), rect, roles().corner(6.0), fill);
    let center = rect.center();
    let painter = ui.painter();
    let color = if primary {
        roles().on_light
    } else if active {
        roles().accent
    } else {
        roles().text
    };
    // Snapped corners keep the flat edge on a pixel boundary; a fractional
    // edge leaves a faint ghost column beside the glyph.
    let triangle = |x: f32, direction: f32| {
        painter.add(egui::Shape::convex_polygon(
            [
                Vec2::new(x - direction * 5.0, -9.0),
                Vec2::new(x - direction * 5.0, 9.0),
                Vec2::new(x + direction * 7.0, 0.0),
            ]
            .map(|offset| {
                egui::emath::GuiRounding::round_to_pixels(
                    center + offset,
                    painter.pixels_per_point(),
                )
            })
            .to_vec(),
            color,
            Stroke::NONE,
        ));
    };
    match icon {
        TransportIcon::Play => triangle(0.0, 1.0),
        TransportIcon::Pause => {
            for x in [-5.0, 5.0] {
                painter.rect_filled(
                    Rect::from_center_size(center + Vec2::new(x, 0.0), Vec2::new(5.0, 18.0)),
                    0.0,
                    color,
                );
            }
        }
        TransportIcon::Stop => {
            painter.rect_filled(
                Rect::from_center_size(center, Vec2::splat(17.0)),
                0.0,
                color,
            );
        }
        TransportIcon::Record => {
            painter.circle_filled(center, 10.0, roles().coral);
            if active {
                painter.circle_stroke(center, 13.0, Stroke::new(1.0_f32, roles().text));
            }
        }
        TransportIcon::Back => {
            triangle(-5.0, -1.0);
            triangle(7.0, -1.0);
        }
        TransportIcon::Forward => {
            triangle(-7.0, 1.0);
            triangle(5.0, 1.0);
        }
        TransportIcon::Start => {
            triangle(3.0, -1.0);
            painter.line_segment(
                [
                    center + Vec2::new(-10.0, -10.0),
                    center + Vec2::new(-10.0, 10.0),
                ],
                Stroke::new(3.0_f32, color),
            );
        }
        TransportIcon::End => {
            triangle(-3.0, 1.0);
            painter.line_segment(
                [
                    center + Vec2::new(10.0, -10.0),
                    center + Vec2::new(10.0, 10.0),
                ],
                Stroke::new(3.0_f32, color),
            );
        }
        TransportIcon::Loop => {
            // Two opposed arrows around a rounded rectangle; the second half
            // is the first turned half a circle.
            for side in [1.0_f32, -1.0] {
                let at = |x: f32, y: f32| center + Vec2::new(x, y) * side;
                let mut points = vec![at(-7.0, 0.5)];
                points.extend((0..=6).map(|i| {
                    let a = std::f32::consts::PI * (1.0 + i as f32 / 12.0);
                    at(-4.0 + 3.0 * a.cos(), -2.0 + 3.0 * a.sin())
                }));
                points.push(at(4.5, -5.0));
                painter.add(egui::Shape::line(points, Stroke::new(2.2_f32, color)));
                painter.add(egui::Shape::convex_polygon(
                    vec![at(10.0, -5.0), at(4.0, -9.5), at(4.0, -0.5)],
                    color,
                    Stroke::NONE,
                ));
            }
        }
    }
    // An outline marks an engaged loop, so the state does not rest on color.
    if active && matches!(icon, TransportIcon::Loop) {
        painter.rect_stroke(
            rect.shrink(0.5),
            6.0,
            Stroke::new(1.0_f32, roles().accent),
            egui::StrokeKind::Inside,
        );
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(2.0),
            6.0,
            Stroke::new(
                2.0_f32,
                if primary {
                    roles().on_light
                } else {
                    roles().accent
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    response.widget_info(|| {
        if matches!(icon, TransportIcon::Loop | TransportIcon::Record) {
            egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), active, tooltip)
        } else {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tooltip)
        }
    });
    response.hover_hint(tooltip)
}
