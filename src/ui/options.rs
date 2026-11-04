//! The Options draft belongs to this window. Session validates, prepares, and
//! persists it; closing this surface cannot change an active preference.
use super::*;
const STALE_DRAFT: &str = "Options changed outside this draft. \
    Reload active options before Apply.";
const RELOAD_ACTION: &str = "Reload active options";
pub(super) const DISCARD: &str = "Discard changes?";
use mixjam_techno::application::options::{
    AppOptions, MeterDisplayRate, Oversampling, PlaybackBufferTarget, SampleQuality,
    SongFolderMode, StartingModule, Theme,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(super) enum Page {
    #[default]
    Audio,
    Workspace,
    Files,
    Libraries,
}
impl Page {
    const ALL: [Self; 4] = [Self::Audio, Self::Workspace, Self::Files, Self::Libraries];
    pub(super) fn from_capture(view: &str) -> Self {
        match view {
            "options-workspace" | "options-unapplied" | "options-discard" => Self::Workspace,
            "options-files" => Self::Files,
            "options-libraries"
            | "options-library-tempos"
            | "options-library-providers"
            | "options-library-expanded" => Self::Libraries,
            _ => Self::Audio,
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|page| *page == self).unwrap()
    }
    fn label(self) -> &'static str {
        match self {
            Self::Audio => "Audio",
            Self::Workspace => "Workspace",
            Self::Files => "Files",
            Self::Libraries => "Sample libraries",
        }
    }
}

pub(super) struct OptionsDraft {
    pub values: AppOptions,
    pub(super) revision: u64,
    pub(super) page: Page,
    pub(super) restored: bool,
    error: Option<String>,
    saved: bool,
    previous_focus: Option<egui::Id>,
    focus_page: bool,
    focus_order: Vec<egui::Id>,
    /// Close, Escape, or Cancel found unapplied edits and asks first.
    pub(super) confirm_discard: bool,
    /// Each page's last measured content height; the window fits it.
    content_heights: [Option<f32>; 4],
}

fn unapplied(count: usize) -> String {
    match count {
        1 => "1 unapplied change".into(),
        _ => format!("{count} unapplied changes"),
    }
}

/// Unapplied edits per page, in `Page::ALL` order. The destructuring is
/// exhaustive, so a new option field must be given a page here.
fn page_changes(draft: &AppOptions, active: &AppOptions) -> [usize; 4] {
    let AppOptions {
        schema_version: _,
        drive_playback_oversampling,
        master_playback_oversampling,
        sample_playback_quality,
        preview_level_db,
        preview_muted,
        playback_buffer_target,
        meter_display_rate,
        follow_on_startup,
        show_section_bookmarks,
        starting_module,
        show_tooltips,
        theme,
        song_folder_mode,
        fixed_song_folder,
    } = draft;
    let count = |changes: &[bool]| changes.iter().filter(|changed| **changed).count();
    [
        count(&[
            *drive_playback_oversampling != active.drive_playback_oversampling,
            *master_playback_oversampling != active.master_playback_oversampling,
            *sample_playback_quality != active.sample_playback_quality,
            *preview_level_db != active.preview_level_db,
            *preview_muted != active.preview_muted,
            *playback_buffer_target != active.playback_buffer_target,
            *meter_display_rate != active.meter_display_rate,
        ]),
        count(&[
            *follow_on_startup != active.follow_on_startup,
            *show_section_bookmarks != active.show_section_bookmarks,
            *starting_module != active.starting_module,
            *show_tooltips != active.show_tooltips,
            *theme != active.theme,
        ]),
        count(&[
            *song_folder_mode != active.song_folder_mode,
            *fixed_song_folder != active.fixed_song_folder,
        ]),
        0,
    ]
}

impl OptionsDraft {
    /// Apply has work: an edited value, or defaults that replace an invalid file.
    pub(super) fn pending(&self, session: &Session) -> bool {
        self.values != *session.options() || (self.restored && session.options_invalid())
    }
    pub(super) fn changes(&self, session: &Session) -> [usize; 4] {
        page_changes(&self.values, session.options())
    }
    /// The footer's account of the draft, empty when nothing is pending.
    fn status(&self, session: &Session) -> String {
        let count: usize = self.changes(session).iter().sum();
        match count {
            0 if self.pending(session) => "Defaults not applied yet".into(),
            0 if self.saved => "Options applied.".into(),
            0 => String::new(),
            _ => unapplied(count),
        }
    }
    pub(super) fn new(session: &Session, page: Page) -> Self {
        Self {
            values: session.options().clone(),
            revision: session.options_revision(),
            page,
            restored: false,
            error: None,
            saved: false,
            previous_focus: None,
            focus_page: true,
            focus_order: Vec::new(),
            confirm_discard: false,
            content_heights: [None; 4],
        }
    }
    /// Capture views that show unapplied edits on two pages, and their prompt.
    pub(super) fn for_capture(mut self, view: &str) -> Self {
        if matches!(view, "options-unapplied" | "options-discard") {
            self.values.preview_muted = !self.values.preview_muted;
            self.values.follow_on_startup = !self.values.follow_on_startup;
            self.values.starting_module = StartingModule::Mixer;
            self.confirm_discard = view == "options-discard";
        }
        self
    }
    fn reload(&mut self, session: &Session) {
        self.values = session.options().clone();
        self.revision = session.options_revision();
        self.restored = false;
        self.error = None;
        self.saved = false;
        self.confirm_discard = false;
    }
    fn audio_changed(&self, active: &AppOptions) -> bool {
        self.values.audio_changed(active)
    }
    pub(super) fn cycle_focus(&self, ctx: &egui::Context, reverse: bool) {
        if self.focus_order.is_empty() {
            return;
        }
        let focused = ctx.memory(|memory| memory.focused());
        let index = self.focus_order.iter().position(|id| Some(*id) == focused);
        let next = match (index, reverse) {
            (Some(0), true) | (None, true) => self.focus_order.len() - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % self.focus_order.len(),
            (None, false) => 0,
        };
        ctx.memory_mut(|memory| {
            memory.move_focus(egui::FocusDirection::None);
            memory.request_focus(self.focus_order[next]);
        });
    }
}

pub(super) fn remember_focus(response: &Response) {
    if response.enabled() {
        response.ctx.data_mut(|data| {
            let key = egui::Id::new("options-focus-order");
            let mut order = data.get_temp::<Vec<egui::Id>>(key).unwrap_or_default();
            order.push(response.id);
            data.insert_temp(key, order);
        });
        if response.gained_focus() {
            response.scroll_to_me(None);
        }
    }
}

#[derive(Default)]
pub(super) struct MeterDisplay {
    shown: MeterReading,
    pending: MeterReading,
    next: f64,
}
impl MeterDisplay {
    /// Keep transient peaks between draws. Audio still measures every block.
    pub(super) fn sample(
        &mut self,
        reading: MeterReading,
        reduced: bool,
        now: f64,
    ) -> MeterReading {
        if reading.run != self.pending.run {
            self.pending = reading;
            self.pending.input_peaks = std::array::from_fn(|side| {
                reading.input_peaks[side] || reading.input_peak_counts[side] > 0
            });
            self.shown = reading;
            self.next = now;
        } else {
            let peaks = std::array::from_fn::<_, 2, _>(|side| {
                self.pending.input_peaks[side]
                    || self.pending.input_peak_counts[side] != reading.input_peak_counts[side]
            });
            let true_peak = self.pending.true_peak;
            let compressor = self.pending.compressor_reduction;
            let limiter = self.pending.limiter_reduction;
            self.pending = reading;
            self.pending.input_peaks =
                std::array::from_fn(|side| peaks[side] || reading.input_peaks[side]);
            self.pending.true_peak = true_peak.max(reading.true_peak);
            self.pending.compressor_reduction = compressor.max(reading.compressor_reduction);
            self.pending.limiter_reduction = limiter.max(reading.limiter_reduction);
        }
        if !reduced || now >= self.next {
            self.shown = self.pending;
            self.pending = reading;
            self.pending.input_peaks = [false; 2];
            self.pending.true_peak = f32::NEG_INFINITY;
            self.pending.compressor_reduction = 0.0;
            self.pending.limiter_reduction = 0.0;
            self.next = now + 1.0 / 15.0;
        }
        self.shown
    }
}

fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    name: &str,
    value: &mut T,
    choices: &[(T, &str)],
) {
    ui.push_id(name, |ui| {
        ui.label(name);
        let selected = choices
            .iter()
            .find(|(item, _)| item == value)
            .map_or("", |(_, label)| *label);
        let response = egui::ComboBox::from_id_salt("choice")
            .width(210.0)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (item, label) in choices {
                    ui.selectable_value(value, *item, *label);
                }
            });
        named(&response.response, egui::WidgetType::ComboBox, name);
        remember_focus(&response.response);
        ui.ctx()
            .accesskit_node_builder(response.response.id, |node| node.set_value(selected));
    });
}

/// A page section title in uppercase, with more room above it than below.
/// The first one clears the bay's top edge.
pub(super) fn section(ui: &mut egui::Ui, title: &str) {
    let first = ui.cursor().top() <= ui.min_rect().top() + 0.5;
    ui.add_space(if first { 4.0 } else { 8.0 });
    ui.label(heading(title, 15.0).color(roles().legend));
}

/// One short line of explanation that belongs to the control above it.
pub(super) fn help(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add_space(-4.0);
    ui.label(egui::RichText::new(text).size(12.0).color(roles().help));
    // Keep the next control's group apart from this one.
    ui.add_space(4.0);
}

/// The shared Folded Metal switch, laid out in the page flow.
fn switch(ui: &mut egui::Ui, name: &str, value: &mut bool) {
    let label =
        ui.painter()
            .layout_no_wrap(name.into(), FontId::proportional(13.0), roles().legend);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(label.size().x + 36.0, 24.0), Sense::hover());
    remember_focus(&controls::toggle_at(ui, rect, name, name, value));
}

impl MixJamApp {
    pub(super) fn open_options(&mut self, ctx: &egui::Context) {
        self.close_options_after_scan = false;
        if let Some(draft) = &mut self.options {
            draft.focus_page = true;
        } else {
            let mut draft = OptionsDraft::new(&self.session, Page::Audio);
            draft.previous_focus = ctx.memory(|memory| memory.focused());
            self.options = Some(draft);
        }
        dialog::expand(ctx, "Options");
        ctx.move_to_top(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("Options"),
        ));
    }

    pub(super) fn open_sample_libraries(&mut self, ctx: &egui::Context) {
        self.open_options(ctx);
        self.options.as_mut().unwrap().page = Page::Libraries;
    }

    pub(super) fn sample_libraries_open(&self) -> bool {
        self.options
            .as_ref()
            .is_some_and(|draft| draft.page == Page::Libraries)
    }

    pub(super) fn close_options(&mut self, ctx: &egui::Context) {
        if let Some(draft) = self.options.take() {
            if let Some(id) = draft.previous_focus {
                ctx.memory_mut(|memory| memory.request_focus(id));
            } else if let Some(id) = ctx.memory(|memory| memory.focused()) {
                ctx.memory_mut(|memory| memory.surrender_focus(id));
            }
        }
    }

    pub(super) fn options_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.options.take() else {
            return;
        };
        let mut open = true;
        let mut cancel = false;
        let mut apply = false;
        let mut pick_folder = false;
        let stale = draft.revision != self.session.options_revision();
        let audio_blocked = draft
            .audio_changed(self.session.options())
            .then(|| self.session.audio_options_blocked_reason())
            .flatten()
            .map(str::to_owned);
        let pending = draft.pending(&self.session);
        let changes = draft.changes(&self.session);
        let status = draft.status(&self.session);
        let apply_blocked = if stale {
            Some(STALE_DRAFT.to_owned())
        } else if !pending {
            Some("No unapplied changes.".to_owned())
        } else {
            audio_blocked.clone()
        };
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new("options-focus-order"), Vec::<egui::Id>::new())
        });
        let page_index = draft.page.index();
        // Until the user resizes it, the window follows the page's height up
        // to the screen; the page scrolls below that.
        let fitted = !dialog::user_resized(ctx, "Options");
        dialog::window("Options")
            .open(&mut open)
            .default_width(760.0)
            .default_height(500.0)
            .scroll(false)
            .fit(true)
            .focus_controls(remember_focus)
            .show(ctx, |ui| {
                let mut area = ui.available_rect_before_wrap();
                if fitted && let Some(content) = draft.content_heights[page_index] {
                    let selector = Page::ALL.len() as f32 * (34.0 + ui.spacing().item_spacing.y);
                    let wanted = content.max(selector) + 12.0 + 38.0;
                    // The shell sizes the window from this allocation.
                    ui.allocate_space(Vec2::new(area.width(), wanted));
                    area.set_height(wanted.min(area.height()));
                } else {
                    ui.advance_cursor_after_rect(area);
                }
                let actions =
                    Rect::from_min_max(Pos2::new(area.left(), area.bottom() - 38.0), area.max);
                let body =
                    Rect::from_min_max(area.min, Pos2::new(area.right(), actions.top() - 12.0));
                let mut body_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("options-body")
                        .max_rect(body),
                );
                body_ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(136.0, body.height()),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            for (page, changed) in Page::ALL.into_iter().zip(changes) {
                                let response = ui.add_sized(
                                    [132.0, 34.0],
                                    egui::Button::selectable(draft.page == page, page.label()),
                                );
                                if changed > 0 {
                                    // Screen readers hear the marker as text.
                                    ctx.accesskit_node_builder(response.id, |node| {
                                        node.set_description(unapplied(changed))
                                    });
                                    // The selected face is the accent colour.
                                    ui.painter().circle_filled(
                                        Pos2::new(
                                            response.rect.right() - 12.0,
                                            response.rect.center().y,
                                        ),
                                        3.5,
                                        if draft.page == page {
                                            roles().ink
                                        } else {
                                            roles().tools[0].1
                                        },
                                    );
                                }
                                remember_focus(&response);
                                if draft.focus_page && draft.page == page {
                                    response.request_focus();
                                    draft.focus_page = false;
                                }
                                if response.clicked() {
                                    draft.page = page;
                                    self.close_options_after_scan = false;
                                }
                                if response.has_focus() {
                                    let delta = ui.input_mut(|input| {
                                        if input.consume_key(
                                            egui::Modifiers::NONE,
                                            egui::Key::ArrowDown,
                                        ) {
                                            1
                                        } else if input
                                            .consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
                                        {
                                            -1
                                        } else {
                                            0
                                        }
                                    });
                                    if delta != 0 {
                                        ctx.memory_mut(|memory| {
                                            memory.move_focus(egui::FocusDirection::None)
                                        });
                                        let pages = Page::ALL;
                                        let index = pages
                                            .iter()
                                            .position(|current| *current == page)
                                            .unwrap();
                                        draft.page = pages[(index as isize + delta)
                                            .rem_euclid(pages.len() as isize)
                                            as usize];
                                        draft.focus_page = true;
                                        self.close_options_after_scan = false;
                                    }
                                }
                            }
                        },
                    );
                    ui.separator();
                    let width = ui.available_width();
                    ui.allocate_ui_with_layout(
                        Vec2::new(width, body.height()),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            let scrolled = egui::ScrollArea::vertical()
                                .id_salt(("options-page", draft.page))
                                .auto_shrink([false, false])
                                .max_height(body.height())
                                .show(ui, |ui| {
                                    ui.set_width(width - ui.spacing().scroll.allocated_width());
                                    if stale {
                                        ui.colored_label(roles().tools[0].1, STALE_DRAFT);
                                        let reload = controls::button(
                                            ui,
                                            RELOAD_ACTION,
                                            controls::ButtonRole::Secondary,
                                        );
                                        remember_focus(&reload);
                                        if reload.clicked() {
                                            draft.reload(&self.session);
                                        }
                                        ui.separator();
                                    }
                                    if let Some(error) = self.session.options_error() {
                                        ui.colored_label(roles().coral, error);
                                    }
                                    if let Some(error) = &draft.error {
                                        ui.colored_label(roles().coral, error);
                                    }
                                    if let Some(reason) = &audio_blocked {
                                        ui.colored_label(roles().tools[0].1, reason);
                                    }
                                    match draft.page {
                                        Page::Audio => self.audio_options(ui, &mut draft.values),
                                        Page::Workspace => workspace_options(ui, &mut draft.values),
                                        Page::Files => {
                                            pick_folder = self.file_options(ui, &mut draft.values);
                                        }
                                        Page::Libraries => self.sample_libraries_page(ui),
                                    }
                                });
                            draft.content_heights[draft.page.index()] =
                                Some(scrolled.content_size.y);
                        },
                    );
                });
                let mut row = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("options-actions")
                        .max_rect(actions)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                // Library actions take effect at once, so their page has no
                // draft to apply or cancel.
                let libraries = draft.page == Page::Libraries;
                if !libraries {
                    let restore = controls::button(
                        &mut row,
                        "Restore options defaults",
                        controls::ButtonRole::Tertiary,
                    );
                    remember_focus(&restore);
                    if restore.clicked() {
                        draft.values = AppOptions::default();
                        draft.restored = true;
                        draft.error = None;
                        draft.saved = false;
                    }
                }
                row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if libraries {
                        let done = controls::button(ui, "Done", controls::ButtonRole::Primary);
                        remember_focus(&done);
                        cancel = done.clicked();
                    } else {
                        let cancel_button =
                            controls::button(ui, "Cancel", controls::ButtonRole::Secondary);
                        remember_focus(&cancel_button);
                        cancel = cancel_button.clicked();
                        ui.add_enabled_ui(apply_blocked.is_none(), |ui| {
                            let response =
                                controls::button(ui, "Apply", controls::ButtonRole::Primary);
                            remember_focus(&response);
                            apply = response.clicked();
                            if let Some(reason) = &apply_blocked {
                                response.disabled_hint(reason);
                            }
                        });
                    }
                    if !status.is_empty() {
                        let label = ui.add(
                            egui::Label::new(egui::RichText::new(&status).color(if pending {
                                roles().tools[0].1
                            } else {
                                roles().legend
                            }))
                            .truncate(),
                        );
                        // Screen readers hear the count as it changes.
                        ctx.accesskit_node_builder(label.id, |node| {
                            node.set_live(egui::accesskit::Live::Polite)
                        });
                    }
                });
            });
        let pending = draft.pending(&self.session);
        if !pending {
            draft.confirm_discard = false;
        }
        let mut close = !open || cancel;
        if close && pending {
            draft.confirm_discard = true;
            close = false;
        }
        if draft.confirm_discard {
            let count: usize = changes.iter().sum();
            let response = dialog::modal(ctx, DISCARD, DISCARD, |ui| {
                ui.label(match count {
                    0 => "Options defaults are not applied yet.".to_owned(),
                    _ => format!("Options has {}.", unapplied(count)),
                });
                ui.horizontal(|ui| {
                    let apply_close = ui
                        .add_enabled_ui(apply_blocked.is_none(), |ui| {
                            dialog::primary(ui, "Apply changes")
                        })
                        .inner;
                    if let Some(reason) = &apply_blocked {
                        apply_close.disabled_hint(reason);
                    } else if apply_close.clicked() {
                        apply = true;
                        draft.confirm_discard = false;
                        close = true;
                    }
                    if dialog::danger(ui, "Discard changes").clicked() {
                        draft.confirm_discard = false;
                        close = true;
                    }
                    if controls::button(ui, "Keep editing", controls::ButtonRole::Secondary)
                        .clicked()
                    {
                        draft.confirm_discard = false;
                    }
                });
            });
            if response.should_close() {
                draft.confirm_discard = false;
            }
        }
        draft.focus_order = ctx.data(|data| {
            data.get_temp::<Vec<egui::Id>>(egui::Id::new("options-focus-order"))
                .unwrap_or_default()
        });
        if apply {
            match self
                .session
                .apply_options(draft.values.clone(), draft.revision, draft.restored)
            {
                Ok(_) => {
                    draft.reload(&self.session);
                    draft.saved = true;
                    self.section_bookmarks = self.session.options().show_section_bookmarks;
                    self.message("Options applied.");
                }
                Err(error) => {
                    self.error(error.clone());
                    draft.error = Some(error);
                    close = false;
                }
            }
        }
        self.options = Some(draft);
        if close {
            self.close_options(ctx);
        }
        if pick_folder {
            self.ask_path(FileRequest::OptionsFolder);
        }
    }

    fn audio_options(&self, ui: &mut egui::Ui, values: &mut AppOptions) {
        section(ui, "Playback quality");
        help(
            ui,
            "Lower settings use less CPU during playback. Exports always use full quality.",
        );
        choice(
            ui,
            "Drive playback oversampling",
            &mut values.drive_playback_oversampling,
            &[(Oversampling::FourX, "4x"), (Oversampling::Off, "Off")],
        );
        help(
            ui,
            "4x keeps track drive saturation clean. Off can sound harsher.",
        );
        choice(
            ui,
            "Master playback oversampling",
            &mut values.master_playback_oversampling,
            &[(Oversampling::FourX, "4x"), (Oversampling::Off, "Off")],
        );
        help(
            ui,
            "4x keeps Soft clip, Tube, Maximizer, and Tape clean. Off can sound harsher.",
        );
        choice(
            ui,
            "Sample playback quality",
            &mut values.sample_playback_quality,
            &[(SampleQuality::High, "High"), (SampleQuality::Low, "Low")],
        );
        help(
            ui,
            "High reduces unwanted tones at changed tempo or rate. Low uses less CPU.",
        );
        if values.quality_reduced() {
            ui.colored_label(
                roles().tools[0].1,
                "Playback quality reduced; exports use full quality",
            );
        }
        ui.add_space(8.0);
        ui.label("Playback buffer target");
        let targets = [
            (PlaybackBufferTarget::Automatic, "Automatic"),
            (PlaybackBufferTarget::Ms20, "20 ms"),
            (PlaybackBufferTarget::Ms40, "40 ms"),
        ];
        let selected = targets
            .iter()
            .find(|(target, _)| *target == values.playback_buffer_target)
            .unwrap()
            .1;
        let buffer = egui::ComboBox::from_id_salt("playback-buffer-target")
            .width(210.0)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (target, label) in targets {
                    let reason = self.session.buffer_target_error(target);
                    let response = ui.add_enabled(
                        reason.is_none(),
                        egui::Button::selectable(values.playback_buffer_target == target, label),
                    );
                    if response.clicked() {
                        values.playback_buffer_target = target;
                    }
                    if let Some(reason) = reason {
                        response.disabled_hint(&reason);
                        ui.small(reason);
                    }
                }
            });
        named(
            &buffer.response,
            egui::WidgetType::ComboBox,
            "Playback buffer target",
        );
        remember_focus(&buffer.response);
        ui.ctx()
            .accesskit_node_builder(buffer.response.id, |node| node.set_value(selected));
        if let Some(reason) = self
            .session
            .buffer_target_error(values.playback_buffer_target)
        {
            ui.colored_label(roles().coral, reason);
        }
        help(
            ui,
            "A larger buffer absorbs processing bursts but adds delay between a control and its sound.",
        );
        // The request and the measured callback, apart from each other (Options
        // spec), out of the way until someone needs them.
        let details = controls::disclosure("Device details")
            .default_open(false)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(self.session.buffer_status())
                        .size(12.0)
                        .color(roles().help),
                );
            });
        remember_focus(&details.header_response);
        ui.separator();
        section(ui, "Monitoring");
        ui.horizontal(|ui| {
            ui.label("Preview level");
            let level = controls::slider(
                ui,
                &mut values.preview_level_db,
                -30.0..=0.0,
                "Preview level",
            );
            remember_focus(&level);
            ui.ctx().accesskit_node_builder(level.id, |node| {
                node.set_description("Preview output level in dB, from -30 to 0.")
            });
            ui.label(format!("{:.1} dB", values.preview_level_db));
        });
        switch(ui, "Mute previews", &mut values.preview_muted);
        choice(
            ui,
            "Meter display rate",
            &mut values.meter_display_rate,
            &[
                (MeterDisplayRate::Normal, "Normal"),
                (MeterDisplayRate::Reduced, "Reduced"),
            ],
        );
        help(
            ui,
            "Reduced draws meters at most 15 times a second. Measurement and peaks stay exact.",
        );
    }

    /// Returns whether the fixed song folder picker was requested.
    fn file_options(&self, ui: &mut egui::Ui, values: &mut AppOptions) -> bool {
        section(ui, "Song folders");
        choice(
            ui,
            "Song dialog folder",
            &mut values.song_folder_mode,
            &[
                (SongFolderMode::LastUsed, "Last used"),
                (SongFolderMode::Fixed, "Fixed folder"),
            ],
        );
        help(
            ui,
            "Where Open and Save as start. Existing songs stay in place.",
        );
        let mut choose = false;
        if values.song_folder_mode == SongFolderMode::Fixed {
            ui.label("Fixed song folder");
            let mut path = values
                .fixed_song_folder
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default();
            let response =
                ui.add(egui::TextEdit::singleline(&mut path).desired_width(ui.available_width()));
            named(&response, egui::WidgetType::TextEdit, "Fixed song folder");
            remember_focus(&response);
            if response.changed() {
                values.fixed_song_folder = (!path.is_empty()).then(|| PathBuf::from(path));
            }
            let picker = controls::button(
                ui,
                "Choose fixed song folder",
                controls::ButtonRole::Secondary,
            );
            remember_focus(&picker);
            choose = picker.clicked();
            help(ui, "An existing folder, chosen or typed as a full path.");
        }
        if let Some(warning) = self.session.song_folder_warning() {
            ui.colored_label(roles().coral, warning);
        }
        choose
    }
}

fn workspace_options(ui: &mut egui::Ui, values: &mut AppOptions) {
    section(ui, "Workspace");
    choice(
        ui,
        "Theme",
        &mut values.theme,
        &Theme::ALL.map(|theme| (theme, theme.label())),
    );
    help(
        ui,
        "Studio is plain and flat, Aero is colorful plastic, and Vintage is worn painted \
         metal. Layout and controls stay the same. Apply changes it at once, even while a \
         song plays.",
    );
    switch(
        ui,
        "Follow playhead on startup",
        &mut values.follow_on_startup,
    );
    help(
        ui,
        "From the next launch. The Follow button sets this session.",
    );
    switch(
        ui,
        "Show section bookmarks",
        &mut values.show_section_bookmarks,
    );
    help(ui, "The same setting as the Sections button.");
    choice(
        ui,
        "Starting module",
        &mut values.starting_module,
        &[
            (StartingModule::Sounds, "Sounds"),
            (StartingModule::Mixer, "Mixer"),
            (StartingModule::Mastering, "Mastering"),
            (StartingModule::Effects, "Effects"),
        ],
    );
    help(ui, "At the next launch. Opening a song keeps the module.");
    switch(ui, "Show tooltips", &mut values.show_tooltips);
    help(
        ui,
        "Hover hints only. Names, focus, and errors stay available.",
    );
}
