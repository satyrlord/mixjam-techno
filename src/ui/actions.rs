//! Whole-song and library actions: playback and seeking, opening, saving and
//! exporting songs, library folders, and the unsaved-changes flow that runs
//! New, Open, and Close after the song is safe.
use super::*;

#[derive(Clone)]
pub(super) enum PendingAction {
    New,
    Open,
    /// A song file dropped on the window.
    OpenPath(PathBuf),
    Close,
}

/// The unsaved-changes flow and the typed path prompt that file actions wait on.
#[derive(Default)]
pub(super) struct FileRequests {
    pub(super) pending_action: Option<PendingAction>,
    pub(super) allow_close: bool,
    pub(super) path_prompt: Option<PathPrompt>,
    /// A typed path finished one save of the unsaved-changes modal.
    pub(super) resume_save_changes: bool,
}

impl MixJamApp {
    pub(super) fn start_playback(&mut self) {
        self.session.start_playback();
        self.sync_notice();
    }

    pub(super) fn pause(&mut self) {
        self.session.pause();
        self.sync_notice();
    }

    pub(super) fn stop(&mut self) {
        let recording = self.session.is_recording();
        self.session.stop();
        self.sync_notice();
        if recording && self.session.has_recorded_take() {
            self.save_recorded_take();
        }
    }

    pub(super) fn seek(&mut self, tick: u32) {
        self.session.seek(tick);
        self.sync_notice();
    }

    pub(super) fn preview(&mut self, sample_id: u32) {
        if let Some(missing) = self.session.missing_sample(sample_id) {
            self.error(format!(
                "Cannot preview missing sample {}. Open Import details to see its saved reference.",
                missing.name
            ));
            return;
        }
        self.session.preview(sample_id);
        self.sync_notice();
    }

    pub(super) fn resolve_missing_sample(&mut self, sample_id: u32) {
        if self.session.missing_sample(sample_id).is_some() {
            self.ask_path(FileRequest::ResolveMissing(sample_id));
        }
    }

    /// The application validates the chosen audio and rejects a reference
    /// that no longer exists.
    pub(super) fn resolve_missing_sample_with(&mut self, sample_id: u32, path: &Path) {
        match self.session.resolve_missing_sample(sample_id, path) {
            Ok(()) => {
                self.clip_menu.missing_sample = None;
                self.reconcile_selection();
                self.sync_notice();
            }
            Err(error) => self.error(error),
        }
    }

    pub(super) fn open(&mut self) {
        self.ask_path(FileRequest::OpenSong);
    }

    /// The mix folder of the sample library, until the user has chosen a song.
    pub(super) fn default_song_folder(&self) -> PathBuf {
        let root = self
            .session
            .library()
            .wav_roots()
            .first()
            .map(PathBuf::as_path)
            .or_else(|| self.session.library().source_root(Product::T2))
            .map(Path::to_path_buf)
            .or_else(|| {
                self.session
                    .library()
                    .bindings()
                    .first()
                    .map(|binding| binding.root.clone())
            })
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        [
            root.join("\x65Jay/mix"),
            root.join("MIX"),
            root.join("D2/MIX"),
        ]
        .into_iter()
        .find(|path| path.is_dir())
        .unwrap_or(root)
    }

    pub(super) fn open_path(&mut self, path: &Path) {
        // A changed catalog folder rescans the sample folders; the song opens after.
        if self.session.refresh_catalogs() {
            match self.session.open_after_scan(path) {
                Ok(()) => self.message(format!(
                    "{} opens after the sample folder scan finishes.",
                    path.display()
                )),
                Err(error) => self.error(format!("Cannot open mix: {error}")),
            }
            return;
        }
        match self.session.open(path) {
            Ok(()) => {
                self.clear_selection();
                self.message(with_import_summary(
                    format!("Opened {}.", path.display()),
                    self.session.project(),
                ));
            }
            Err(error) => self.error(format!("Cannot open mix: {error}")),
        }
    }

    pub(super) fn save(&mut self, choose_path: bool) {
        match self.session.file().map(Path::to_path_buf) {
            Some(path) if !choose_path => self.save_to(&path),
            _ => self.ask_path(FileRequest::SaveSong),
        }
    }

    pub(super) fn save_to(&mut self, path: &Path) {
        match self.session.save(path) {
            Ok(()) => self.message(format!("Saved {}.", path.display())),
            Err(error) => self.error(format!("Cannot save mix: {error}")),
        }
    }

    pub(super) fn export(&mut self) {
        self.ask_path(FileRequest::ExportMix);
    }

    pub(super) fn new_mix(&mut self) {
        match self.session.new_mix() {
            Ok(()) => {
                self.clear_selection();
                self.files.pending_action = None;
                self.message("New mix.");
            }
            Err(error) => self.error(error),
        }
    }

    pub(super) fn add_library(&mut self, product: Product, root: &Path) {
        match self.session.add_library(product, root) {
            Ok(()) => {
                self.reconcile_selection();
                self.message(format!("Added {} samples.", product.label()));
            }
            Err(error) => self.error(format!("Cannot add library: {error}")),
        }
    }

    /// Add a library folder after the current ones and scan them all.
    pub(super) fn add_library_folder(&mut self, root: &Path) {
        let mut roots = self.session.library().wav_roots().to_vec();
        roots.push(root.to_path_buf());
        self.scan_library_folders(roots);
    }

    /// Scan the library folders without the one at `index`.
    pub(super) fn remove_library_folder(&mut self, index: usize) {
        let mut roots = self.session.library().wav_roots().to_vec();
        if index < roots.len() {
            roots.remove(index);
            self.scan_library_folders(roots);
        }
    }

    /// Scan these library folders. The Sounds panel changes when the scan ends.
    pub(super) fn scan_library_folders(&mut self, roots: Vec<PathBuf>) {
        if let Err(error) = self.try_scan_library_folders(roots, self.libraries.fallback_bpm) {
            self.error(error);
        }
    }

    pub(super) fn try_scan_library_folders(
        &mut self,
        roots: Vec<PathBuf>,
        fallback_bpm: f64,
    ) -> Result<(), String> {
        self.session.start_wave_scan(&roots, fallback_bpm)?;
        self.libraries.fallback_bpm = fallback_bpm;
        self.libraries.close_options_after_scan = false;
        self.clear_selection();
        self.message(format!("Scanning {}…", scan_subject(&roots)));
        Ok(())
    }

    pub(super) fn remove_library(&mut self, product: Product) {
        match self.session.remove_library(product) {
            Ok(()) => {
                if self.sounds.product == Some(product) {
                    self.sounds.product = None;
                }
                self.reconcile_selection();
                self.message(format!(
                    "Removed {} from the sample browser.",
                    product.label()
                ));
            }
            Err(error) => self.error(error),
        }
    }

    pub(super) fn request_action(&mut self, action: PendingAction, ctx: &egui::Context) {
        let pending = if self.files.path_prompt.is_some() {
            Some("Choose path")
        } else if self.files.pending_action.is_some() {
            Some("Save your changes?")
        } else if self.sample_import.path.is_some() {
            Some("Import sound")
        } else {
            None
        };
        if let Some(identity) = pending {
            dialog::expand(ctx, identity);
            self.error(format!(
                "Finish or cancel {identity} before starting another file action."
            ));
            return;
        }
        if self.session.is_recording() {
            self.error("Stop recording before you open, close, or start a new mix.");
            return;
        }
        if self.session.needs_save_confirmation() {
            self.files.pending_action = Some(action);
        } else {
            self.run_action(action, ctx);
        }
    }

    pub(super) fn run_action(&mut self, action: PendingAction, ctx: &egui::Context) {
        // A collapsed confirmation permits recording after the initial request.
        if self.session.is_recording() {
            self.error("Stop recording before you open, close, or start a new mix.");
            return;
        }
        self.files.pending_action = None;
        self.session.discard_recorded_take();
        match action {
            PendingAction::New => self.new_mix(),
            PendingAction::Open => self.open(),
            PendingAction::OpenPath(path) => {
                self.session.remember_song_folder(&path);
                self.open_path(&path);
            }
            PendingAction::Close => {
                self.files.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Save what is unsaved, then run `action`. A cancelled or failed save
    /// leaves the unsaved-changes modal open.
    pub(super) fn save_changes(&mut self, action: PendingAction, ctx: &egui::Context) {
        if self.session.has_recorded_take() {
            self.save_recorded_take();
        }
        // A typed path arrives in a later frame. `path_prompt_dialog` then resumes here.
        if self.files.path_prompt.is_none()
            && !self.session.has_recorded_take()
            && self.session.unsaved()
        {
            self.save(false);
        }
        if self.files.path_prompt.is_none()
            && !self.session.unsaved()
            && !self.session.has_recorded_take()
        {
            self.run_action(action, ctx);
        }
    }
}

/// What a scan reads, for the status line: one library folder, or a count.
pub(super) fn scan_subject(roots: &[PathBuf]) -> String {
    match roots {
        [root] => root.display().to_string(),
        _ => format!("{} library folders", roots.len()),
    }
}

/// A path below a library folder as the Libraries window shows it: relative
/// to its folder, after the folder's own name when there are several.
pub(super) fn library_relative(library: LibraryView<'_>, path: &Path) -> String {
    let Some(root) = library.wav_root_of(path) else {
        return display_paths(&path.to_string_lossy());
    };
    let base = if library.wav_roots().len() > 1 {
        root.parent().unwrap_or(root)
    } else {
        root
    };
    let relative = path.strip_prefix(base).unwrap_or(path);
    if relative.as_os_str().is_empty() {
        ".".into()
    } else {
        display_paths(&relative.to_string_lossy())
    }
}

pub(super) fn with_import_summary(text: String, project: &Project) -> String {
    let mut text = project
        .import_notices
        .last()
        .map_or(text.clone(), |notice| {
            format!(
                "{text} Tempo {} BPM changed to {} BPM.",
                notice.source_bpm, notice.target_bpm
            )
        });
    for notice in &project.source_notices {
        if notice.starts_with("MixJam v7 source controls:") {
            continue;
        }
        text.push(' ');
        text.push_str(notice);
    }
    text
}
