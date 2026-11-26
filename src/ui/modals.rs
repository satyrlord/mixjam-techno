//! The modal and help windows: the blocking-modal precedence, the
//! unsaved-changes prompt, the Help windows of the application and its tools,
//! and the sample conflict resolver.
use super::*;

/// Which Help windows are open: the application's and each tool's.
#[derive(Default)]
pub(super) struct HelpWindows {
    pub(super) main: bool,
    pub(super) stretch: bool,
    pub(super) effects: bool,
    pub(super) step_synth: bool,
    pub(super) sample_edit: bool,
    pub(super) master: bool,
    pub(super) fx_returns: bool,
    /// The page that the main Help window shows.
    pub(super) topic: help::Topic,
    /// Tool pages opened since the last frame, to place beside their tool.
    pub(super) placing: Vec<help::Topic>,
}

impl HelpWindows {
    /// Open a tool's or a board's Help beside it.
    pub(super) fn open_tool(&mut self, topic: help::Topic) {
        let open = match topic {
            help::Topic::Stretch => &mut self.stretch,
            help::Topic::Effects => &mut self.effects,
            help::Topic::Mastering => &mut self.master,
            help::Topic::FxReturns => &mut self.fx_returns,
            help::Topic::StepSynth => &mut self.step_synth,
            help::Topic::SampleEditor => &mut self.sample_edit,
            _ => return,
        };
        *open = true;
        self.placing.push(topic);
    }
}

/// The sample conflict resolver: open, dismissed, and its last error.
#[derive(Default)]
pub(super) struct SampleConflicts {
    pub(super) open: bool,
    /// Escape hid the resolver without confirming; the status bar reopens it.
    pub(super) dismissed: bool,
    pub(super) error: Option<String>,
}

impl MixJamApp {
    /// Pending file choices suspend their parent confirmation, even while collapsed.
    pub(super) fn blocking_modal(&self, ctx: &egui::Context) -> Option<&'static str> {
        if self.files.path_prompt.is_some() {
            return if dialog::is_collapsed(ctx, "Choose path") {
                self.commands.as_ref().map(|_| "All commands")
            } else {
                Some("Choose path")
            };
        }
        if self.files.pending_action.is_some() && !dialog::is_collapsed(ctx, "Save your changes?") {
            return Some("Save your changes?");
        }
        if self.sample_conflicts_open() && !dialog::is_collapsed(ctx, "Sample conflicts") {
            return Some("Sample conflicts");
        }
        if self.sample_import.path.is_some() && !dialog::is_collapsed(ctx, "Import sound") {
            return Some("Import sound");
        }
        if self
            .options
            .as_ref()
            .is_some_and(|draft| draft.confirm_discard)
            && !dialog::is_collapsed(ctx, options::DISCARD)
        {
            return Some(options::DISCARD);
        }
        self.commands.as_ref().map(|_| "All commands")
    }

    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        self.save_changes_dialog(ctx);
        self.help_windows(ctx);
    }

    /// "Save your changes?" while an action waits on unsaved work.
    fn save_changes_dialog(&mut self, ctx: &egui::Context) {
        if !dialog::is_collapsed(ctx, "Save your changes?")
            && self.files.path_prompt.is_none()
            && std::mem::take(&mut self.files.resume_save_changes)
            && let Some(action) = self.files.pending_action.clone()
        {
            self.save_changes(action, ctx);
        }
        let Some(action) = self.files.pending_action.clone() else {
            return;
        };
        if self.files.path_prompt.is_some() {
            return;
        }
        let message = self.unsaved_message();
        let response = dialog::modal(ctx, "Save your changes?", "Save your changes?", |ui| {
            ui.label(message);
            ui.horizontal(|ui| self.save_changes_keys(ui, &action, ctx));
        });
        if response.should_close() {
            self.files.pending_action = None;
            self.files.resume_save_changes = false;
        }
    }

    /// What the unsaved work is.
    fn unsaved_message(&self) -> &'static str {
        if self.session.has_recorded_take() && self.session.unsaved() {
            "The song and the recorded take are unsaved."
        } else if self.session.has_recorded_take() {
            "The recorded take is unsaved."
        } else {
            "The song has unsaved changes."
        }
    }

    fn save_changes_keys(
        &mut self,
        ui: &mut egui::Ui,
        action: &PendingAction,
        ctx: &egui::Context,
    ) {
        if dialog::primary(ui, "Save changes").clicked() {
            self.save_changes(action.clone(), ctx);
        }
        if dialog::danger(ui, "Discard changes").clicked() {
            self.run_action(action.clone(), ctx);
        }
        if controls::button(ui, "Cancel", controls::ButtonRole::Secondary).clicked() {
            self.files.pending_action = None;
        }
    }

    /// The manual, and the Help window of each tool.
    fn help_windows(&mut self, ctx: &egui::Context) {
        for topic in std::mem::take(&mut self.help.placing) {
            if let Some(window) = topic.window() {
                dialog::forget_geometry(ctx, window);
            }
        }
        let viewport = ctx.viewport_rect().shrink(12.0);
        let topic = &mut self.help.topic;
        dialog::window("Help")
            .open(&mut self.help.main)
            .default_width(720.0)
            .default_height((viewport.height() - 160.0).clamp(420.0, 760.0))
            .max_height((viewport.height() - 40.0).max(240.0))
            .constrain_to(viewport)
            .scroll(false)
            .show(ctx, |ui| help::manual(ui, topic));
        let deep_edit_open = [
            (help::Topic::Mastering, self.master_deep_edit.is_some()),
            (help::Topic::FxReturns, self.fx_deep_edit.is_some()),
        ];
        for (topic, open) in [
            (help::Topic::Stretch, &mut self.help.stretch),
            (help::Topic::Effects, &mut self.help.effects),
            (help::Topic::Mastering, &mut self.help.master),
            (help::Topic::FxReturns, &mut self.help.fx_returns),
            (help::Topic::StepSynth, &mut self.help.step_synth),
            (help::Topic::SampleEditor, &mut self.help.sample_edit),
        ] {
            let (Some(title), Some(palette)) = (topic.window(), topic.palette()) else {
                continue;
            };
            dialog::window(title)
                .open(open)
                .palette(palette)
                .anchor(help::anchor(
                    ctx,
                    topic,
                    deep_edit_open.contains(&(topic, true)),
                ))
                .default_width(480.0)
                .default_height((viewport.height() - 200.0).clamp(360.0, 600.0))
                .max_height((viewport.height() - 40.0).max(240.0))
                .constrain_to(viewport)
                .scroll(false)
                .show(ctx, |ui| help::tool_page(ui, topic));
        }
    }

    pub(super) fn sample_conflicts_open(&self) -> bool {
        !self.session.sample_conflicts().is_empty()
            && (self.conflicts.open
                || (!self.conflicts.dismissed
                    && self
                        .session
                        .sample_conflicts()
                        .iter()
                        .any(|conflict| !conflict.confirmed)))
    }

    pub(super) fn sample_conflicts_dialog(&mut self, ctx: &egui::Context) {
        if !self.sample_conflicts_open()
            || self.files.path_prompt.is_some()
            || (self.files.pending_action.is_some()
                && !dialog::is_collapsed(ctx, "Save your changes?"))
        {
            return;
        }
        self.conflicts.open = true;
        let conflicts = self.session.sample_conflicts().to_vec();
        let recording = self.session.is_recording();
        let mut choice = None;
        let mut confirm = false;
        let response = dialog::modal(ctx, "Sample conflicts", "Sample conflicts", |ui| {
            ui.set_width((ctx.viewport_rect().width() - 96.0).clamp(280.0, 700.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(
                "Several files match a saved sample. The first match is selected until you choose \
                 another.",
            );
            ui.label("Each choice updates every instance of that source in this song.");
            ui.add_space(10.0);
            egui::ScrollArea::vertical()
                .id_salt("sample_conflict_choices")
                .max_height((ctx.viewport_rect().height() - 310.0).max(160.0))
                .show(ui, |ui| {
                    for (index, conflict) in conflicts.iter().enumerate() {
                        ui.push_id(index, |ui| {
                            if index > 0 {
                                ui.add_space(8.0);
                                ui.separator();
                                ui.add_space(8.0);
                            }
                            ui.add(
                                egui::Label::new(display_paths(&conflict.reference))
                                    .wrap()
                                    .selectable(true),
                            );
                            let instances = self
                                .session
                                .project()
                                .clips
                                .iter()
                                .filter(|clip| conflict.sample_ids.contains(&clip.sample_id))
                                .count();
                            ui.label(
                                egui::RichText::new(format!(
                                    "{instances} instances · {} matches",
                                    conflict.candidates.len()
                                ))
                                .color(roles().legend),
                            );
                            ui.add_enabled_ui(!recording, |ui| {
                                for (candidate_index, path) in
                                    conflict.candidates.iter().enumerate()
                                {
                                    let path = display_paths(&path.to_string_lossy());
                                    let selected = candidate_index == conflict.selected;
                                    let label = format!(
                                        "{}{}\n{path}",
                                        if selected {
                                            "Selected match "
                                        } else {
                                            "Use match "
                                        },
                                        candidate_index + 1
                                    );
                                    if controls::radio(ui, selected, &label).clicked() && !selected
                                    {
                                        choice = Some((index, candidate_index));
                                    }
                                }
                            });
                        });
                    }
                });
            view::name_scroll_area(ui, "sample_conflict_choices", "Conflict choices");
            ui.add_space(10.0);
            if recording {
                ui.colored_label(
                    roles().coral,
                    "Stop recording before changing or confirming sample choices.",
                );
                if controls::button(ui, "Stop recording", controls::ButtonRole::Secondary).clicked()
                {
                    self.stop();
                }
            }
            if let Some(error) = &self.conflicts.error {
                ui.colored_label(roles().coral, display_paths(error));
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled_ui(!recording, |ui| dialog::primary(ui, "Keep choices"))
                    .inner
                    .clicked()
                {
                    confirm = true;
                }
                let playing = self.session.wants_playback();
                if ui
                    .add_enabled_ui(!recording, |ui| {
                        controls::button(
                            ui,
                            if playing { "Pause song" } else { "Play song" },
                            controls::ButtonRole::Secondary,
                        )
                    })
                    .inner
                    .clicked()
                {
                    if playing {
                        self.pause();
                    } else {
                        self.start_playback();
                    }
                }
            });
            ui.small(
                "Esc closes this window without confirming. Reopen from Sample conflicts in the \
                 status bar.",
            );
        });
        if let Some((conflict, candidate)) = choice {
            match self.session.resolve_sample_conflict(conflict, candidate) {
                Ok(()) => {
                    self.conflicts.error = None;
                    self.reconcile_selection();
                    self.message("Updated all instances of the selected source.");
                }
                Err(error) => self.conflicts.error = Some(error),
            }
        }
        if confirm && !recording {
            match self.session.confirm_sample_conflicts() {
                Ok(()) => {
                    self.conflicts.open = false;
                    self.conflicts.dismissed = false;
                    self.conflicts.error = None;
                    self.message("Sample choices retained.");
                }
                Err(error) => self.conflicts.error = Some(error),
            }
        } else if response.should_close() {
            // Escape or a click outside only hides the window. Confirmation is
            // an undoable project change, so it needs the explicit button.
            self.conflicts.open = false;
            self.conflicts.dismissed = true;
            self.conflicts.error = None;
            self.message(
                "Sample choices are not confirmed. Reopen from Sample conflicts in the status bar.",
            );
        }
    }
}
