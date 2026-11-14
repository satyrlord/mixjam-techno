//! The tool windows and what they do with sounds. `tool_windows` shows each
//! open window: Options, the channel EQ, Album Review, Import details, Import
//! sound, Effect Studio, Sample Editor, Time Stretcher, and the recorded take.
//! The rest is what they act on: the source that a tool edits, Listen and Save,
//! importing and recording, and the Time Stretcher's background renders.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum StretchPurpose {
    Listen,
    Save,
}

pub(super) struct StretchRequest {
    pub(super) settings: StretchSettings,
    purpose: StretchPurpose,
    pub(super) name: String,
    pub(super) catalog_revision: u64,
    preview_revision: u64,
}

pub(super) struct StretchAudition {
    pub(super) settings: StretchSettings,
    pub(super) name: String,
    preview_revision: u64,
}

#[derive(Default)]
pub(super) struct StretchFeedback {
    pub(super) pending: Option<StretchRequest>,
    pub(super) audition: Option<StretchAudition>,
    pub(super) saved: Option<StretchSettings>,
    pub(super) message: Option<String>,
    pub(super) error: bool,
}

#[derive(Default)]
pub(super) struct EffectsFeedback {
    pub(super) source: Option<u32>,
    pub(super) message: Option<String>,
    pub(super) error: bool,
}

#[derive(Default)]
pub(super) struct StepSynthFeedback {
    /// The palette source that the message describes. An explicit file has none.
    pub(super) source: Option<u32>,
    pub(super) message: Option<String>,
    pub(super) error: bool,
}

/// The Effect Studio window: whether it is open, its effects, and its result.
#[derive(Default)]
pub(super) struct EffectsWindow {
    pub(super) open: bool,
    pub(super) state: EffectsState,
    pub(super) feedback: EffectsFeedback,
}

/// The Sample Editor window: whether it is open, and its range and pitch.
#[derive(Default)]
pub(super) struct SampleEditWindow {
    pub(super) open: bool,
    pub(super) start: u64,
    pub(super) end: u64,
    /// The sound and catalog revision that the edit range was last set for.
    pub(super) source: Option<(u32, u64)>,
    pub(super) pitch: f64,
}

/// The Time Stretcher window: whether it is open, its tempos, and its renders.
pub(super) struct StretchWindow {
    pub(super) open: bool,
    pub(super) source_bpm: f64,
    pub(super) target_bpm: f64,
    pub(super) feedback: StretchFeedback,
}

impl Default for StretchWindow {
    fn default() -> Self {
        Self {
            open: false,
            source_bpm: 140.0,
            target_bpm: 140.0,
            feedback: StretchFeedback::default(),
        }
    }
}

/// The Album Review window: whether it is open, and its view, reviews, and
/// loaded covers.
pub(super) struct AlbumReviewWindow {
    pub(super) open: bool,
    pub(super) state: album_review::AlbumReviewState,
}

/// The Import sound window's WAV and tempo, and the Import details window.
pub(super) struct SampleImport {
    pub(super) path: Option<PathBuf>,
    pub(super) bpm: f64,
    pub(super) details_open: bool,
}

impl Default for SampleImport {
    fn default() -> Self {
        Self {
            path: None,
            bpm: 140.0,
            details_open: false,
        }
    }
}

/// The recording destination and the recorded take's discard confirmation.
#[derive(Default)]
pub(super) struct RecordingState {
    /// The track that a recording goes to.
    pub(super) track: usize,
    /// Discard take was pressed once and waits for its confirmation.
    pub(super) discard_armed: bool,
}

impl MixJamApp {
    pub(super) fn source_id(&self) -> Option<u32> {
        self.selection.sample.or_else(|| {
            self.selection.clip.and_then(|id| {
                self.session
                    .project()
                    .clips
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.sample_id)
            })
        })
    }

    pub(super) fn source_name(&self) -> String {
        if let Some(missing) = self
            .source_id()
            .and_then(|id| self.session.missing_sample(id))
        {
            return format!("Missing: {}", missing.name);
        }
        self.source_id()
            .and_then(|id| self.session.library().sample_by_id(id))
            .map_or_else(|| "Select a sound".into(), |s| s.name.clone())
    }

    pub(super) fn source_bpm(&self) -> f64 {
        self.session.source_bpm(self.source_id())
    }

    pub(super) fn play_pcm(&mut self, pcm: Pcm, label: &str) {
        self.session.play_pcm(pcm, label);
        self.sync_notice();
    }

    pub(super) fn register_file(
        &mut self,
        path: &Path,
        group: &str,
        source_bpm: f64,
    ) -> Result<u32, String> {
        let id = self.session.register_file(path, group, source_bpm)?;
        self.sounds.group = Some(group.into());
        self.sounds.product = None;
        self.selection.sample = Some(id);
        self.sync_notice();
        Ok(id)
    }

    pub(super) fn import_wav(&mut self) {
        self.ask_path(FileRequest::ImportSound);
    }

    /// Ask where to save `pcm`, then add the saved WAV to `group`.
    pub(super) fn save_pcm(&mut self, pcm: Pcm, name: &str, group: &'static str, source_bpm: f64) {
        self.ask_path(FileRequest::SaveSound {
            pcm,
            name: name.into(),
            group,
            source_bpm,
            stretch: None,
        });
    }

    pub(super) fn start_recording(&mut self) {
        let result = self.session.start_recording(self.recording.track);
        self.apply(result);
    }

    pub(super) fn save_recorded_take(&mut self) {
        if self.session.has_recorded_take() {
            self.ask_path(FileRequest::SaveTake);
        }
    }

    pub(super) fn tool_windows(&mut self, ctx: &egui::Context) {
        self.options_window(ctx);
        self.eq_window(ctx);
        album_review::show(
            ctx,
            &mut self.album_review.open,
            &mut self.album_review.state,
        );
        if self.sample_import.details_open {
            let mut open = true;
            dialog::window("Import details")
                .default_height(240.0)
                .open(&mut open)
                .default_width(540.0)
                .max_height((ctx.viewport_rect().height() - 80.0).max(240.0))
                .constrain_to(ctx.viewport_rect().shrink(12.0))
                .show(ctx, |ui| {
                    if !self.session.missing_samples().is_empty() {
                        ui.label(heading("Missing samples", 20.0));
                        ui.label("Missing samples play silence. Select details to copy.");
                        let mut resolve = None;
                        for missing in self.session.missing_samples() {
                            let selected = self.source_id() == Some(missing.id);
                            controls::disclosure(format!("Missing: {}", missing.name))
                                .id_salt(("missing_sample", missing.id))
                                .default_open(selected || self.session.missing_samples().len() == 1)
                                .show(ui, |ui| {
                                    let action = name_control(
                                        controls::button(
                                            ui,
                                            "Resolve manually",
                                            controls::ButtonRole::Secondary,
                                        ),
                                        &format!("Resolve manually: {}", missing.name),
                                    );
                                    if action
                                        .hover_hint(
                                            "Choose the WAV file for this sound. Every clip that \
                                             uses it keeps its position, length, and gain.",
                                        )
                                        .clicked()
                                    {
                                        resolve = Some(missing.id);
                                    }
                                    ui.add(
                                        egui::Label::new(missing_sample_details(missing))
                                            .wrap()
                                            .selectable(true),
                                    );
                                    for clip in self
                                        .session
                                        .project()
                                        .clips
                                        .iter()
                                        .filter(|clip| clip.sample_id == missing.id)
                                    {
                                        ui.add(
                                            egui::Label::new(missing_clip_details(clip))
                                                .wrap()
                                                .selectable(true),
                                        );
                                    }
                                });
                        }
                        if let Some(id) = resolve {
                            self.resolve_missing_sample(id);
                        }
                        ui.separator();
                    }
                    for notice in &self.session.project().import_notices {
                        ui.label(format!(
                            "{} BPM changed to {} BPM.",
                            notice.source_bpm, notice.target_bpm
                        ));
                    }
                    for notice in &self.session.project().source_notices {
                        if notice.starts_with("MixJam v7 source controls:") {
                            controls::disclosure("Saved MixJam source controls").show(ui, |ui| {
                                ui.label(display_paths(notice));
                            });
                        } else {
                            ui.label(display_paths(notice));
                        }
                    }
                    let settings = &self.session.project().source_settings;
                    if !settings.is_empty() {
                        controls::disclosure(format!("Saved settings ({})", settings.len()))
                            .id_salt("source_settings")
                            .show(ui, |ui| {
                                ui.label(
                                    "The original product saved these values with the song. \
                                     Select them to copy.",
                                );
                                let rows: Vec<_> = settings
                                    .iter()
                                    .map(|(key, value)| format!("{key}: {value}"))
                                    .collect();
                                ui.add(egui::Label::new(rows.join("\n")).wrap().selectable(true));
                            });
                    }
                });
            self.sample_import.details_open = open;
        }
        if let Some(path) = self.sample_import.path.clone()
            && self.files.path_prompt.is_none()
            && (self.files.pending_action.is_none()
                || dialog::is_collapsed(ctx, "Save your changes?"))
        {
            let response = dialog::modal(ctx, "Import sound", "Import sound", |ui| {
                ui.label(path.file_name().unwrap_or_default().to_string_lossy())
                    .hover_hint(display_paths(&path.to_string_lossy()));
                ui.horizontal(|ui| {
                    ui.label("Source BPM");
                    controls::number(ui, &mut self.sample_import.bpm, 7.5..=1200.0, "Source BPM")
                        .hover_hint("The sound's original tempo, separate from project BPM.");
                });
                ui.horizontal(|ui| {
                    if dialog::primary(ui, "Import into Wave").clicked() {
                        match self.register_file(&path, "Wave", self.sample_import.bpm) {
                            Ok(_) => {
                                self.sample_import.path = None;
                                self.message(format!("Imported {}.", path.display()));
                            }
                            Err(error) => self.error(error),
                        }
                    }
                    if controls::button(ui, "Cancel", controls::ButtonRole::Secondary).clicked() {
                        self.sample_import.path = None;
                    }
                });
            });
            if response.should_close() {
                self.sample_import.path = None;
            }
        }
        if self.effects.open {
            let mut open = true;
            let source_id = self.source_id();
            if self.effects.feedback.source != source_id {
                self.effects.feedback = EffectsFeedback {
                    source: source_id,
                    ..Default::default()
                };
            }
            let name = self.source_name();
            let missing = source_id.is_some_and(|id| self.session.missing_sample(id).is_some());
            let available =
                source_id.is_some_and(|id| self.session.library().sample_by_id(id).is_some());
            let action = effects::show(
                ctx,
                &mut open,
                effects::Source {
                    name: &name,
                    available,
                    missing,
                },
                effects::Feedback {
                    message: self.effects.feedback.message.as_deref(),
                    error: self.effects.feedback.error,
                },
                &mut self.effects.state,
            );
            match action {
                effects::Action::Listen => {
                    match self.session.processed_pcm(source_id, &self.effects.state) {
                        Ok(pcm) => self.play_pcm(pcm, "Listening to the processed sound."),
                        Err(error) => self.error(error),
                    }
                    self.effects.feedback.message = Some(self.status.clone());
                    self.effects.feedback.error = self.status_error;
                }
                effects::Action::Save => {
                    match self.session.processed_pcm(source_id, &self.effects.state) {
                        Ok(pcm) => {
                            self.effects.feedback.message =
                                Some("Choose a WAV path to save the processed sound.".into());
                            self.effects.feedback.error = false;
                            let name = format!("{name} - effect");
                            self.save_pcm(pcm, &name, "Effect", self.source_bpm());
                        }
                        Err(error) => {
                            self.effects.feedback.message = Some(error.clone());
                            self.effects.feedback.error = true;
                            self.error(error);
                        }
                    }
                }
                effects::Action::Reset => {
                    self.effects.state = Default::default();
                    self.effects.feedback.message =
                        Some("Effects reset. Filter, Echo, and Robotize are off.".into());
                    self.effects.feedback.error = false;
                }
                effects::Action::Details => self.sample_import.details_open = true,
                effects::Action::Help => self.help.effects = true,
                effects::Action::None => {}
            }
            self.effects.open = open;
        }
        if self.sample_edit.open {
            let mut open = true;
            let source_id = self.source_id();
            let name = self.source_name();
            let metadata = source_id
                .and_then(|id| self.session.library().sample_by_id(id))
                .map(|sample| (u64::from(sample.frame_count), sample.sample_rate));
            let (frames, sample_rate) = metadata.unwrap_or((0, 0));
            // A replacement can keep its source ID. Reset only the old trim draft.
            let source = source_id.map(|id| (id, self.session.catalog_revision()));
            if !dialog::is_collapsed(ctx, "Sample editor")
                && (self.sample_edit.end == 0 || self.sample_edit.source != source)
            {
                self.sample_edit.source = source;
                self.sample_edit.start = 0;
                self.sample_edit.end = frames;
            }
            match sample_editor::show(
                ctx,
                &mut open,
                sample_editor::Source {
                    name: &name,
                    frames,
                    sample_rate,
                    available: metadata.is_some(),
                    missing: source_id.is_some_and(|id| self.session.missing_sample(id).is_some()),
                },
                &mut self.sample_edit.start,
                &mut self.sample_edit.end,
                &mut self.sample_edit.pitch,
            ) {
                sample_editor::Action::Listen => match self.edited_pcm() {
                    Ok(pcm) => self.play_pcm(pcm, "Listening to the sample edit."),
                    Err(error) => self.error(error),
                },
                sample_editor::Action::Save => match self.edited_pcm() {
                    Ok(pcm) => {
                        let name = format!("{} - edit", self.source_name());
                        self.save_pcm(pcm, &name, "Wave", self.source_bpm());
                    }
                    Err(error) => self.error(error),
                },
                sample_editor::Action::Details => self.sample_import.details_open = true,
                sample_editor::Action::Help => self.help.sample_edit = true,
                sample_editor::Action::None => {}
            }
            self.sample_edit.open = open;
        }
        if self.stretch.open {
            let source = self.source_id();
            let source_name = self.source_name();
            let source_bpm = self.source_bpm();
            let missing = source.is_some_and(|id| self.session.missing_sample(id).is_some());
            let settings = self.stretch_settings();
            let listening = self.stretch_is_listening();
            let audition = self.stretch.feedback.audition.as_ref();
            let audition_changed = audition.is_some_and(|heard| Some(heard.settings) != settings);
            // Only a running preview has a message. Afterwards the panel
            // returns to the duration of the current settings.
            let playback_message = audition.filter(|_| listening).map(|heard| {
                format!(
                    "Playing {} at {:.0} to {:.0} BPM.{}",
                    heard.name,
                    heard.settings.source_bpm,
                    heard.settings.target_bpm,
                    if audition_changed {
                        " Settings changed since this preview."
                    } else {
                        ""
                    },
                )
            });
            match stretch::show(
                ctx,
                &mut self.stretch.open,
                stretch::Source {
                    name: &source_name,
                    id: source,
                    samples: self.session.library().samples,
                    catalog_revision: self.session.catalog_revision(),
                    missing,
                    bpm: source_bpm,
                },
                stretch::Feedback {
                    processing: self.session.is_stretching(),
                    listening,
                    saved: settings.is_some() && self.stretch.feedback.saved == settings,
                    audition: audition.map(|heard| {
                        (
                            heard.settings.source_id,
                            heard.settings.source_bpm,
                            heard.settings.target_bpm,
                        )
                    }),
                    message: self
                        .stretch
                        .feedback
                        .message
                        .as_deref()
                        .or(playback_message.as_deref()),
                    error: self.stretch.feedback.error,
                },
                &mut self.stretch.source_bpm,
                &mut self.stretch.target_bpm,
            ) {
                stretch::Action::Select(id) => {
                    self.selection.sample = Some(id);
                    self.stretch.source_bpm = self.source_bpm().clamp(30.0, 300.0);
                    self.stretch.feedback.saved = None;
                    self.stretch.feedback.message = None;
                    self.stretch.feedback.error = false;
                }
                stretch::Action::Listen => self.start_stretch(StretchPurpose::Listen),
                stretch::Action::Save => self.start_stretch(StretchPurpose::Save),
                stretch::Action::Stop => {
                    self.stop();
                    self.stretch.feedback.message = Some("Preview stopped.".into());
                    self.stretch.feedback.error = false;
                }
                stretch::Action::Help => self.help.stretch = true,
                stretch::Action::Details => self.sample_import.details_open = true,
                stretch::Action::None => {}
            }
            if self.stretch_settings() != settings {
                self.stretch.feedback.saved = None;
                self.stretch.feedback.message = None;
                self.stretch.feedback.error = false;
            }
            if !self.stretch.open {
                self.close_stretch_request();
            }
            if self.session.is_stretching() {
                ctx.request_repaint_after(Duration::from_millis(16));
            }
        }
        if (self.session.has_recorded_take() && !self.session.is_recording())
            || self.capture_view_is("recorded-take")
        {
            dialog::window("Recorded take")
                .default_width(400.0)
                .default_height(92.0)
                .show(ctx, |ui| {
                    ui.label("Unsaved take: stored in memory.");
                    if self.recording.discard_armed {
                        ui.colored_label(
                            roles().coral,
                            "Discarding erases this recording. It cannot be undone.",
                        );
                    }
                    ui.horizontal(|ui| {
                        if self.recording.discard_armed {
                            if controls::button(ui, "Keep take", controls::ButtonRole::Secondary)
                                .clicked()
                            {
                                self.recording.discard_armed = false;
                            }
                            if dialog::danger(ui, "Confirm discard").clicked() {
                                self.session.discard_recorded_take();
                                self.recording.discard_armed = false;
                                self.message("Recorded take discarded.");
                            }
                            return;
                        }
                        if dialog::primary(ui, "Save take").clicked() {
                            self.save_recorded_take();
                        }
                        if controls::button(ui, "Listen", controls::ButtonRole::Secondary).clicked()
                        {
                            self.session.preview_recorded_take();
                            self.sync_notice();
                        }
                        ui.add_space(16.0);
                        if dialog::danger(ui, "Discard take").clicked() {
                            self.recording.discard_armed = true;
                        }
                    });
                });
        } else {
            self.recording.discard_armed = false;
        }
    }

    pub(super) fn step_synth_pcm(&self) -> Result<Pcm, String> {
        self.session.step_synth_pcm(
            self.source_id(),
            self.step_synth.file.as_deref(),
            &self.step_synth.state,
        )
    }

    pub(super) fn step_synth_notice(&mut self) {
        self.step_synth.feedback.message = Some(self.status.clone());
        self.step_synth.feedback.error = self.status_error;
    }

    pub(super) fn edited_pcm(&self) -> Result<Pcm, String> {
        self.session.edited_pcm(
            self.source_id(),
            self.sample_edit.start,
            self.sample_edit.end,
            self.sample_edit.pitch,
        )
    }

    fn stretch_settings(&self) -> Option<StretchSettings> {
        Some(StretchSettings {
            source_id: self.source_id()?,
            source_bpm: self.stretch.source_bpm,
            target_bpm: self.stretch.target_bpm,
        })
    }

    fn stretch_is_listening(&self) -> bool {
        self.stretch
            .feedback
            .audition
            .as_ref()
            .is_some_and(|heard| {
                self.session.is_previewing()
                    && heard.preview_revision == self.session.preview_revision()
            })
    }

    pub(super) fn start_stretch(&mut self, purpose: StretchPurpose) {
        let Some(settings) = self.stretch_settings() else {
            return;
        };
        match self.session.start_stretch(settings) {
            Ok(()) => {
                self.stretch.feedback.pending = Some(StretchRequest {
                    settings,
                    purpose,
                    name: self.source_name(),
                    catalog_revision: self.session.catalog_revision(),
                    preview_revision: self.session.preview_revision(),
                });
                self.stretch.feedback.message = Some("Processing sound...".into());
                self.stretch.feedback.error = false;
                self.stretch.feedback.saved = None;
            }
            Err(error) => {
                self.stretch.feedback.message = Some(format!("Cannot process the sound: {error}"));
                self.stretch.feedback.error = true;
                self.error(error);
            }
        }
    }

    pub(super) fn poll_stretch(&mut self) {
        let Some((settings, result)) = self.session.take_stretch_result() else {
            return;
        };
        let Some(request) = self.stretch.feedback.pending.take() else {
            return;
        };
        if !self.stretch.open
            || settings != request.settings
            || Some(settings) != self.stretch_settings()
            || request.catalog_revision != self.session.catalog_revision()
        {
            self.stretch.feedback.message =
                Some("Settings changed. Process the sound again.".into());
            self.stretch.feedback.error = false;
            return;
        }
        if matches!(request.purpose, StretchPurpose::Listen)
            && request.preview_revision != self.session.preview_revision()
        {
            self.stretch.feedback.message =
                Some("Preview cancelled by another playback action.".into());
            self.stretch.feedback.error = false;
            return;
        }
        match result {
            Ok(pcm) => match request.purpose {
                StretchPurpose::Listen => {
                    self.play_pcm(pcm, "Listening to the stretched sound.");
                    self.stretch.feedback.error = self.status_error;
                    self.stretch.feedback.message =
                        Some(format!("Cannot play the preview: {}", self.status));
                    if !self.status_error {
                        self.stretch.feedback.message = None;
                        self.stretch.feedback.audition = Some(StretchAudition {
                            settings,
                            name: request.name,
                            preview_revision: self.session.preview_revision(),
                        });
                    }
                }
                StretchPurpose::Save => {
                    self.stretch.feedback.message =
                        Some("Choose where to save the stretched sound.".into());
                    self.ask_path(FileRequest::SaveSound {
                        pcm,
                        name: format!("{} - stretch", request.name),
                        group: "Wave",
                        source_bpm: settings.target_bpm,
                        stretch: Some(settings),
                    });
                }
            },
            Err(error) => {
                self.stretch.feedback.message = Some(format!("Cannot process the sound: {error}"));
                self.stretch.feedback.error = true;
                self.error(error);
            }
        }
    }

    pub(super) fn close_stretch_request(&mut self) {
        if self.stretch.feedback.pending.take().is_some() {
            self.stretch.feedback.message = Some("Processing request cancelled.".into());
            self.stretch.feedback.error = false;
        }
    }
}
