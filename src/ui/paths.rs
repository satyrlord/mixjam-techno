//! File requests and their paths. A request asks a native dialog for a path, or
//! an agent-driven session's Choose path modal, and `finish_path` carries the
//! chosen path out.
use super::*;

/// The file extensions that Open song accepts.
pub(super) const SONG_EXTENSIONS: &[&str] = &["mixjam", "mix", "json"];

/// What a chosen path is for. While an agent drives the application, the
/// request waits in the Choose path modal instead of a native file dialog.
pub(super) enum FileRequest {
    OpenSong,
    SaveSong,
    ExportMix,
    ExportTracks,
    ImportSound,
    SaveSound {
        pcm: Pcm,
        name: String,
        group: &'static str,
        source_bpm: f64,
        stretch: Option<StretchSettings>,
    },
    SaveTake,
    LibraryFolder,
    ProviderFolder(Product),
    SynthSource,
    SavePattern,
    LoadPattern,
    OptionsFolder,
    ResolveMissing(u32),
}

pub(super) enum PathKind {
    OpenFile,
    SaveFile,
    Folder,
}

/// How to ask for the path of a file request, natively or in the modal.
pub(super) struct PathSpec {
    pub(super) title: String,
    pub(super) kind: PathKind,
    pub(super) filter: Option<(&'static str, &'static [&'static str])>,
    pub(super) file_name: Option<String>,
    pub(super) folder: Option<PathBuf>,
}

impl PathSpec {
    /// The path that a typed entry names, or why it cannot be used.
    pub(super) fn checked(&self, text: &str) -> Result<PathBuf, String> {
        let mut path = PathBuf::from(text.trim().trim_matches('"'));
        if !path.is_absolute() {
            return Err("Enter a full path, such as C:\\Music\\song.mixjam.".into());
        }
        match self.kind {
            PathKind::OpenFile if !path.is_file() => Err("That file does not exist.".into()),
            PathKind::Folder if !path.is_dir() => Err("That folder does not exist.".into()),
            PathKind::SaveFile if path.is_dir() => {
                Err("That path is a folder. Add a file name.".into())
            }
            PathKind::SaveFile => {
                // A native save dialog adds the extension of its filter.
                if path.extension().is_none()
                    && let Some((_, [extension, ..])) = self.filter
                {
                    path.set_extension(extension);
                }
                Ok(path)
            }
            PathKind::OpenFile | PathKind::Folder => Ok(path),
        }
    }
}

/// A file request that waits for a typed path.
pub(super) struct PathPrompt {
    pub(super) request: FileRequest,
    /// The song state whose replacement was approved before Open chose a path.
    pub(super) open_token: Option<(u64, u64)>,
    pub(super) text: String,
    pub(super) error: Option<String>,
    pub(super) focused: bool,
}

impl MixJamApp {
    pub(super) fn path_spec(&self, request: &FileRequest) -> PathSpec {
        use PathKind::{Folder, OpenFile, SaveFile};
        type Filter = Option<(&'static str, &'static [&'static str])>;
        const WAV: Filter = Some(("WAV audio", &["wav"]));
        const PATTERN: Filter = Some(("Step synth pattern", &["json"]));
        let song = &self.session.project().title;
        let (title, kind, filter, file_name, folder): (String, _, Filter, _, _) = match request {
            FileRequest::OpenSong => (
                "Open song".into(),
                OpenFile,
                Some(("Songs", SONG_EXTENSIONS)),
                None,
                Some(
                    self.session
                        .song_folder()
                        .unwrap_or_else(|| self.default_song_folder()),
                ),
            ),
            FileRequest::SaveSong => (
                "Save song".into(),
                SaveFile,
                Some(("MixJam song", &["mixjam"])),
                Some(format!("{song}.mixjam")),
                self.session.song_folder(),
            ),
            FileRequest::ExportMix => (
                "Export mix".into(),
                SaveFile,
                Some(("Stereo WAV", &["wav"])),
                Some(format!("{song}.wav")),
                None,
            ),
            FileRequest::ExportTracks => ("Export tracks".into(), Folder, None, None, None),
            FileRequest::ImportSound => ("Import sound".into(), OpenFile, WAV, None, None),
            FileRequest::SaveSound { name, .. } => (
                "Save sound".into(),
                SaveFile,
                WAV,
                Some(format!("{name}.wav")),
                None,
            ),
            FileRequest::SaveTake => (
                "Save recorded take".into(),
                SaveFile,
                WAV,
                Some("Recording.wav".into()),
                None,
            ),
            FileRequest::LibraryFolder => {
                ("Choose library folder".into(), Folder, None, None, None)
            }
            FileRequest::OptionsFolder => (
                "Choose fixed song folder".into(),
                Folder,
                None,
                None,
                self.options
                    .as_ref()
                    .and_then(|draft| draft.values.fixed_song_folder.clone()),
            ),
            FileRequest::ProviderFolder(product) => (
                format!("Choose {} folder", product.label()),
                Folder,
                None,
                None,
                None,
            ),
            FileRequest::SynthSource => (
                "Choose source file".into(),
                OpenFile,
                Some(("Generator sound / WAV", &["\x70xd", "wav"])),
                None,
                Some(
                    self.session
                        .library()
                        .wav_roots()
                        .first()
                        .map(PathBuf::as_path)
                        .or_else(|| self.session.library().source_root(Product::T2))
                        .unwrap_or_else(|| Path::new("."))
                        .to_path_buf(),
                ),
            ),
            FileRequest::SavePattern => (
                "Save pattern".into(),
                SaveFile,
                PATTERN,
                Some("Step synth pattern.json".into()),
                None,
            ),
            FileRequest::LoadPattern => ("Load pattern".into(), OpenFile, PATTERN, None, None),
            FileRequest::ResolveMissing(id) => (
                format!(
                    "Resolve missing sample: {}",
                    self.session
                        .missing_sample(*id)
                        .map_or("", |missing| missing.name.as_str())
                ),
                OpenFile,
                WAV,
                None,
                None,
            ),
        };
        PathSpec {
            title,
            kind,
            filter,
            file_name,
            folder,
        }
    }

    fn native_path(request: &FileRequest, spec: &PathSpec) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new();
        if matches!(request, FileRequest::ResolveMissing(_)) {
            dialog = dialog.set_title(title_text(&spec.title));
        }
        if let Some((name, extensions)) = spec.filter {
            dialog = dialog.add_filter(name, extensions);
        }
        if let Some(name) = &spec.file_name {
            dialog = dialog.set_file_name(name);
        }
        if let Some(folder) = &spec.folder {
            dialog = dialog.set_directory(folder);
        }
        match spec.kind {
            PathKind::OpenFile => dialog.pick_file(),
            PathKind::SaveFile => dialog.save_file(),
            PathKind::Folder => dialog.pick_folder(),
        }
    }

    /// Ask for the path of `request`, then carry the request out. An agent
    /// cannot operate a native dialog, so its session types the path instead.
    pub(super) fn ask_path(&mut self, request: FileRequest) {
        if matches!(request, FileRequest::OpenSong | FileRequest::SaveSong)
            && let Some(warning) = self.session.song_folder_warning()
        {
            self.error(warning);
        }
        if self.files.path_prompt.is_some() {
            self.error("Finish or cancel Choose path before starting another file action.");
            return;
        }
        if matches!(request, FileRequest::ImportSound) && self.sample_import.path.is_some() {
            self.error("Finish or cancel Import sound before importing another sound.");
            return;
        }
        if self.agent.is_some() {
            let open_token = matches!(request, FileRequest::OpenSong)
                .then(|| self.session.project_change_token());
            self.files.path_prompt = Some(PathPrompt {
                request,
                open_token,
                text: String::new(),
                error: None,
                focused: false,
            });
        } else {
            let path = Self::native_path(&request, &self.path_spec(&request));
            self.finish_path(request, path);
        }
    }

    /// Carry out a file request with its chosen path. `None` is a cancelled choice.
    pub(super) fn finish_path(&mut self, request: FileRequest, path: Option<PathBuf>) {
        let Some(path) = path else {
            if matches!(request, FileRequest::SaveTake) {
                self.message("Unsaved take: stored in memory. Save it before closing.");
            }
            if matches!(
                request,
                FileRequest::SaveSound {
                    stretch: Some(_),
                    ..
                }
            ) {
                self.stretch.feedback.message = Some("Save cancelled.".into());
                self.stretch.feedback.saved = None;
                self.stretch.feedback.error = false;
            }
            if matches!(
                request,
                FileRequest::SaveSound {
                    group: "Effect",
                    stretch: None,
                    ..
                }
            ) {
                self.effects.feedback.message = Some("Save cancelled.".into());
                self.effects.feedback.error = false;
            }
            let step_message = match request {
                FileRequest::SynthSource => Some("Source selection cancelled."),
                FileRequest::SavePattern => Some("Pattern save cancelled."),
                FileRequest::LoadPattern => Some("Pattern load cancelled."),
                FileRequest::SaveSound {
                    group: "Step synth",
                    stretch: None,
                    ..
                } => Some("Sequence save cancelled."),
                _ => None,
            };
            if let Some(message) = step_message {
                self.step_synth.feedback.message = Some(message.into());
                self.step_synth.feedback.error = false;
            }
            return;
        };
        match request {
            FileRequest::OptionsFolder => {
                if let Some(draft) = &mut self.options {
                    draft.values.fixed_song_folder = Some(path);
                }
            }
            FileRequest::OpenSong => {
                self.session.remember_song_folder(&path);
                self.open_path(&path);
            }
            FileRequest::SaveSong => {
                self.session.remember_song_folder(&path);
                self.save_to(&path);
            }
            FileRequest::ExportMix => {
                let result = self.session.start_export(&path);
                self.apply(result);
            }
            FileRequest::ExportTracks => {
                let result = self.session.start_track_export(&path);
                self.apply(result);
            }
            FileRequest::ImportSound => {
                if self.sample_import.path.is_some() {
                    self.error("Finish or cancel Import sound before importing another sound.");
                    return;
                }
                self.sample_import.path = Some(path);
                self.sample_import.bpm = 140.0;
            }
            FileRequest::SaveSound {
                pcm,
                group,
                source_bpm,
                stretch,
                ..
            } => match if group == "Step synth" && stretch.is_none() {
                self.session
                    .save_sequence(&pcm, &path, &self.step_synth.state)
            } else {
                self.session.save_pcm(&pcm, &path, group, source_bpm)
            } {
                Ok(id) => {
                    self.sounds.group = Some(group.into());
                    self.sounds.product = None;
                    self.message(format!("Saved {} and added it to {group}.", path.display()));
                    if let Some(settings) = stretch {
                        // The tool keeps its source and both tempos, so the
                        // next version starts from the same sound.
                        let file = path.file_name().map_or_else(
                            || path.display().to_string(),
                            |name| name.to_string_lossy().into_owned(),
                        );
                        self.stretch.feedback.saved = Some(settings);
                        self.stretch.feedback.message = Some(format!("Saved {file} to {group}."));
                        self.stretch.feedback.error = false;
                    } else {
                        self.selection.sample = Some(id);
                        if group == "Effect" {
                            self.effects.feedback.source = Some(id);
                            self.effects.feedback.message = Some(format!(
                                "Saved {} to Effect.",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ));
                            self.effects.feedback.error = false;
                        }
                        if group == "Step synth" {
                            self.step_synth.feedback.source =
                                self.step_synth.file.is_none().then_some(id);
                            self.step_synth.feedback.message = Some(format!(
                                "Saved {} to Step synth.",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ));
                            self.step_synth.feedback.error = false;
                            self.clip_paint.step_patterns.clear();
                        }
                    }
                }
                Err(error) => {
                    if stretch.is_some() {
                        self.stretch.feedback.message =
                            Some(format!("Cannot save the sound: {error}"));
                        self.stretch.feedback.error = true;
                        self.stretch.feedback.saved = None;
                    }
                    if group == "Effect" && stretch.is_none() {
                        self.effects.feedback.message =
                            Some(format!("Cannot save the sound: {error}"));
                        self.effects.feedback.error = true;
                    }
                    if group == "Step synth" && stretch.is_none() {
                        self.step_synth.feedback.message =
                            Some(format!("Cannot save the sequence: {error}"));
                        self.step_synth.feedback.error = true;
                    }
                    self.error(error);
                }
            },
            FileRequest::SaveTake => match self.session.save_recorded_take(&path) {
                Ok(clip) => {
                    self.sounds.group = Some("Wave".into());
                    self.sounds.product = None;
                    if let Some(id) = clip {
                        self.select_clip(id, false);
                        self.message(format!("Saved {}.", path.display()));
                    } else {
                        self.sync_notice();
                    }
                }
                Err(error) => self.error(error),
            },
            FileRequest::LibraryFolder => self.add_library_folder(&path),
            FileRequest::ProviderFolder(product) => self.add_library(product, &path),
            FileRequest::SynthSource => {
                self.step_synth.file = Some(path);
                self.step_synth.feedback = StepSynthFeedback {
                    source: None,
                    message: Some("Selected a source file. Listen before saving.".into()),
                    error: false,
                };
            }
            FileRequest::SavePattern => {
                match self.session.save_pattern(&path, &self.step_synth.state) {
                    Ok(()) => self.message("Saved Step synth pattern."),
                    Err(error) => self.error(error),
                }
                self.step_synth_notice();
            }
            FileRequest::LoadPattern => {
                match self.session.load_pattern(&path) {
                    Ok(state) => {
                        self.step_synth.state = state;
                        self.message("Loaded Step synth pattern.");
                    }
                    Err(error) => self.error(error),
                }
                self.step_synth_notice();
            }
            FileRequest::ResolveMissing(id) => self.resolve_missing_sample_with(id, &path),
        }
    }

    /// The file choice of an agent-driven session, in place of a native dialog.
    pub(super) fn path_prompt_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut prompt) = self.files.path_prompt.take() else {
            return;
        };
        let spec = self.path_spec(&prompt.request);
        let mut confirm = false;
        let mut cancel = false;
        let modal = dialog::modal(ctx, "Choose path", &spec.title, |ui| {
            ui.set_width((ctx.viewport_rect().width() - 96.0).clamp(280.0, 620.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(match spec.kind {
                PathKind::OpenFile => "Enter the full path of the file to open.",
                PathKind::SaveFile => {
                    "Enter the full path of the file to save. An existing file is replaced."
                }
                PathKind::Folder => "Enter the full path of the folder.",
            });
            if let Some((name, extensions)) = spec.filter {
                let extensions: Vec<_> = extensions
                    .iter()
                    .map(|extension| format!(".{extension}"))
                    .collect();
                ui.label(
                    egui::RichText::new(format!("{name}: {}", extensions.join(", ")))
                        .color(roles().legend),
                );
            }
            if let Some(name) = &spec.file_name {
                ui.label(
                    egui::RichText::new(format!("Suggested name: {name}")).color(roles().legend),
                );
            }
            let path = name_control(
                ui.add(
                    egui::TextEdit::singleline(&mut prompt.text)
                        .hint_text("Path")
                        .desired_width(f32::INFINITY),
                ),
                "Path",
            );
            if !prompt.focused {
                path.request_focus();
                prompt.focused = true;
            }
            confirm = path.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if let Some(error) = &prompt.error {
                ui.colored_label(roles().coral, error);
            }
            ui.horizontal(|ui| {
                confirm |= dialog::primary(ui, "Confirm path").clicked();
                cancel = controls::button(ui, "Cancel", controls::ButtonRole::Secondary).clicked();
            });
        });
        if confirm {
            if prompt.open_token.is_some_and(|token| {
                token != self.session.project_change_token()
                    || self.session.is_recording()
                    || self.session.has_recorded_take()
            }) {
                prompt.error = Some(
                    "The mix changed or recording started after Open. Cancel this choice and \
                     open the song again."
                        .into(),
                );
                self.files.path_prompt = Some(prompt);
                return;
            }
            match spec.checked(&prompt.text) {
                Ok(path) => {
                    let saves = matches!(
                        prompt.request,
                        FileRequest::SaveSong | FileRequest::SaveTake
                    );
                    self.finish_path(prompt.request, Some(path));
                    self.files.resume_save_changes = saves && self.files.pending_action.is_some();
                }
                Err(error) => {
                    prompt.error = Some(error);
                    self.files.path_prompt = Some(prompt);
                }
            }
        } else if cancel || modal.should_close() {
            self.finish_path(prompt.request, None);
        } else {
            self.files.path_prompt = Some(prompt);
        }
    }
}
