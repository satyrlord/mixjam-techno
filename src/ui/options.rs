//! The Options draft belongs to this window. Session validates, prepares, and
//! persists it; closing this surface cannot change an active preference.
use super::*;
const STALE_DRAFT: &str = "Options changed outside this draft. \
    Reload active options before Apply.";
const RELOAD_ACTION: &str = "Reload active options";
use mixjam_techno::application::options::{
    AppOptions, MeterDisplayRate, Oversampling, PlaybackBufferTarget, SongFolderMode,
    StartingModule,
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
            "options-workspace" => Self::Workspace,
            "options-files" => Self::Files,
            "options-libraries"
            | "options-library-tempos"
            | "options-library-providers"
            | "options-library-expanded" => Self::Libraries,
            _ => Self::Audio,
        }
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
}
impl OptionsDraft {
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
        }
    }
    fn reload(&mut self, session: &Session) {
        self.values = session.options().clone();
        self.revision = session.options_revision();
        self.restored = false;
        self.error = None;
        self.saved = false;
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

fn checkbox(ui: &mut egui::Ui, name: &str, value: &mut bool) {
    remember_focus(&ui.checkbox(value, egui::RichText::new(name).color(controls::CREAM)));
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
        let mut navigate_libraries = false;
        let stale = draft.revision != self.session.options_revision();
        let audio_blocked = draft
            .audio_changed(self.session.options())
            .then(|| self.session.audio_options_blocked_reason())
            .flatten()
            .map(str::to_owned);
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new("options-focus-order"), Vec::<egui::Id>::new())
        });
        dialog::window("Options")
            .open(&mut open)
            .default_width(760.0)
            .default_height(500.0)
            .scroll(false)
            .focus_controls(remember_focus)
            .show(ctx, |ui| {
                let area = ui.available_rect_before_wrap();
                let actions =
                    Rect::from_min_max(Pos2::new(area.left(), area.bottom() - 38.0), area.max);
                let body =
                    Rect::from_min_max(area.min, Pos2::new(area.right(), actions.top() - 12.0));
                ui.advance_cursor_after_rect(area);
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
                            for page in Page::ALL {
                                let response = ui.add_sized(
                                    [132.0, 34.0],
                                    egui::Button::selectable(draft.page == page, page.label()),
                                );
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
                            egui::ScrollArea::vertical()
                                .id_salt(("options-page", draft.page))
                                .auto_shrink([false, false])
                                .max_height(body.height())
                                .show(ui, |ui| {
                                    ui.set_width(width - ui.spacing().scroll.allocated_width());
                                    if stale {
                                        ui.colored_label(controls::ACCENT, STALE_DRAFT);
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
                                        ui.colored_label(CORAL, error);
                                    }
                                    if let Some(error) = &draft.error {
                                        ui.colored_label(CORAL, error);
                                    }
                                    if draft.saved {
                                        ui.label("Options applied.");
                                    }
                                    if let Some(reason) = &audio_blocked {
                                        ui.colored_label(controls::ACCENT, reason);
                                    }
                                    match draft.page {
                                        Page::Audio => self.audio_options(ui, &mut draft.values),
                                        Page::Workspace => workspace_options(ui, &mut draft.values),
                                        Page::Files => {
                                            let action = self.file_options(ui, &mut draft.values);
                                            pick_folder = action == FileOptionsAction::ChooseFolder;
                                            navigate_libraries =
                                                action == FileOptionsAction::Libraries;
                                        }
                                        Page::Libraries => self.sample_libraries_page(ui),
                                    }
                                });
                        },
                    );
                });
                let mut row = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("options-actions")
                        .max_rect(actions)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                let restore = controls::button(
                    &mut row,
                    "Restore options defaults",
                    controls::ButtonRole::Secondary,
                );
                remember_focus(&restore);
                if restore.clicked() {
                    draft.values = AppOptions::default();
                    draft.restored = true;
                    draft.error = None;
                    draft.saved = false;
                }
                row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let cancel_button =
                        controls::button(ui, "Cancel", controls::ButtonRole::Secondary);
                    remember_focus(&cancel_button);
                    cancel = cancel_button.clicked();
                    ui.add_enabled_ui(!stale && audio_blocked.is_none(), |ui| {
                        let response = controls::button(ui, "Apply", controls::ButtonRole::Primary);
                        remember_focus(&response);
                        apply = response.clicked();
                        if stale {
                            response.disabled_hint(STALE_DRAFT);
                        } else if let Some(reason) = &audio_blocked {
                            response.disabled_hint(reason);
                        }
                    });
                });
            });
        draft.focus_order = ctx.data(|data| {
            data.get_temp::<Vec<egui::Id>>(egui::Id::new("options-focus-order"))
                .unwrap_or_default()
        });
        if navigate_libraries {
            draft.page = Page::Libraries;
            draft.focus_page = true;
            self.close_options_after_scan = false;
        }
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
                }
            }
        }
        self.options = Some(draft);
        if !open || cancel {
            self.close_options(ctx);
        }
        if pick_folder {
            self.ask_path(FileRequest::OptionsFolder);
        }
    }

    fn audio_options(&self, ui: &mut egui::Ui, values: &mut AppOptions) {
        ui.heading("Playback quality");
        ui.label(
            "4x reduces unwanted high-frequency tones from saturation. \
        Off uses less CPU but can sound harsher. Exports use 4x in \
        both cases.",
        );
        choice(
            ui,
            "Drive playback oversampling",
            &mut values.drive_playback_oversampling,
            &[(Oversampling::FourX, "4x"), (Oversampling::Off, "Off")],
        );
        choice(
            ui,
            "Master playback oversampling",
            &mut values.master_playback_oversampling,
            &[(Oversampling::FourX, "4x"), (Oversampling::Off, "Off")],
        );
        if values.drive_playback_oversampling == Oversampling::Off
            || values.master_playback_oversampling == Oversampling::Off
        {
            ui.colored_label(controls::ACCENT, "Playback quality reduced; exports use 4x");
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
            ui.colored_label(CORAL, reason);
        }
        ui.label(
            "Larger buffers can absorb processing bursts and increase \
        control-to-sound delay. The device may use a different \
        callback period.",
        );
        ui.label(self.session.buffer_status());
        ui.separator();
        ui.heading("Monitoring");
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
        checkbox(ui, "Mute previews", &mut values.preview_muted);
        choice(
            ui,
            "Meter display rate",
            &mut values.meter_display_rate,
            &[
                (MeterDisplayRate::Normal, "Normal"),
                (MeterDisplayRate::Reduced, "Reduced"),
            ],
        );
        ui.label(
            "Reduced updates meter values at most 15 times per second. \
        Audio measurement and peak detection stay active.",
        );
    }

    fn file_options(&mut self, ui: &mut egui::Ui, values: &mut AppOptions) -> FileOptionsAction {
        ui.heading("Song folders");
        choice(
            ui,
            "Song dialog folder",
            &mut values.song_folder_mode,
            &[
                (SongFolderMode::LastUsed, "Last used"),
                (SongFolderMode::Fixed, "Fixed folder"),
            ],
        );
        let mut action = FileOptionsAction::None;
        ui.add_enabled_ui(values.song_folder_mode == SongFolderMode::Fixed, |ui| {
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
            if picker.clicked() {
                action = FileOptionsAction::ChooseFolder;
            }
        });
        if let Some(warning) = self.session.song_folder_warning() {
            ui.colored_label(CORAL, warning);
        }
        ui.label(
            "The folder is the initial location for Open and Save as. \
        Existing songs stay in place.",
        );
        ui.separator();
        ui.heading("Sample libraries");
        ui.label(format!(
            "{} configured sample folders",
            self.session.library().wav_roots().len()
        ));
        let manage = controls::button(
            ui,
            "Go to sample libraries",
            controls::ButtonRole::Secondary,
        );
        remember_focus(&manage);
        if manage.clicked() {
            action = FileOptionsAction::Libraries;
        }
        ui.label(
            "Folder scans, tempo reports, and product providers are on the Sample libraries \
             page. Library actions apply immediately and do not use Options Apply.",
        );
        action
    }
}

#[derive(PartialEq, Eq)]
enum FileOptionsAction {
    None,
    ChooseFolder,
    Libraries,
}

fn workspace_options(ui: &mut egui::Ui, values: &mut AppOptions) {
    ui.heading("Workspace");
    checkbox(
        ui,
        "Follow playhead on startup",
        &mut values.follow_on_startup,
    );
    ui.label(
        "Sets Follow for the next app launch. The toolbar controls \
        this session.",
    );
    checkbox(
        ui,
        "Show section bookmarks",
        &mut values.show_section_bookmarks,
    );
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
    ui.label(
        "Opens this module on the next launch. Opening a song keeps \
        the current module.",
    );
    checkbox(ui, "Show tooltips", &mut values.show_tooltips);
    ui.label(
        "Hover hints only. Control names, focus, errors, and visible \
        instructions stay available.",
    );
}
