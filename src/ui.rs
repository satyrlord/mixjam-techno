#[path = "ui/capture.rs"]
mod capture;
use capture::write_bmp;
#[path = "ui/view.rs"]
mod view;
use view::*;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Vec2};

use crate::ui_icons::{RailIcon, RailIcons};
use mixjam_techno::application::{
    Clip, EditCommand, EffectsState, MissingReference, MissingSample, Pcm, Product, Project,
    SUPPORTED_BPMS, Sample, Session, StepSynthState, TICKS_PER_BAR, TRACK_COUNT,
};

#[derive(Clone)]
enum DragItem {
    Sample(u32),
    Clip { id: u64, offset: u32, copy: bool },
}

#[derive(Clone, Copy)]
enum PendingAction {
    New,
    Open,
    Close,
}

/// Native frontend. It owns view state and submits commands to the headless session.
pub struct MixJamApp {
    session: Session,
    selected_sample: Option<u32>,
    selected_clip: Option<u64>,
    selected_clips: BTreeSet<u64>,
    context_missing_sample: Option<u32>,
    group: Option<String>,
    palette_product: Option<Product>,
    search: String,
    show_mixer: bool,
    show_help: bool,
    show_libraries: bool,
    show_import_details: bool,
    show_sample_conflicts: bool,
    /// Escape hid the resolver without confirming; the status bar reopens it.
    sample_conflicts_dismissed: bool,
    sample_conflict_error: Option<String>,
    library_product: Product,
    rail_icons: Option<Result<Arc<RailIcons>, String>>,
    erase: bool,
    zoom: f32,
    follow_playhead: bool,
    last_follow_position: Option<(u32, f32, f32)>,
    drag: Option<DragItem>,
    timeline_rect: Option<Rect>,
    timeline_drop_rect: Option<Rect>,
    timeline_ruler_rect: Option<Rect>,
    timeline_tracks: Vec<usize>,
    timeline_row_by_track: [Option<usize>; TRACK_COUNT],
    timeline_add_track: Option<usize>,
    timeline_row_height: f32,
    timeline_bar_width: f32,
    loop_start: Option<u32>,
    status: String,
    status_error: bool,
    /// The scan progress line last written to the status, while a scan runs.
    scan_status: Option<String>,
    pending_action: Option<PendingAction>,
    allow_close: bool,
    selection_origin: Option<Pos2>,
    selection_base: BTreeSet<u64>,
    capture_path: Option<PathBuf>,
    capture_requested: bool,
    capture_pass: u32,
    capture_last_track: bool,
    capture_transport_focus: bool,
    capture_focus_ready: bool,
    show_effects: bool,
    show_step_synth: bool,
    show_sample_edit: bool,
    show_stretch: bool,
    effects: EffectsState,
    step_synth: StepSynthState,
    step_synth_bar: usize,
    step_synth_file: Option<PathBuf>,
    step_synth_effects_expanded: bool,
    edit_start: u64,
    edit_end: u64,
    edit_pitch: f64,
    stretch_source_bpm: f64,
    stretch_target_bpm: f64,
    armed_track: usize,
    import_path: Option<PathBuf>,
    import_bpm: f64,
}

/// The first status line: a startup notice, or the idle text.
fn startup_status(session: &mut Session) -> (String, bool) {
    session
        .take_notice()
        .unwrap_or_else(|| (DEFAULT_STATUS.into(), false))
}

impl MixJamApp {
    #[cfg_attr(test, allow(dead_code))]
    pub fn new(cc: &eframe::CreationContext<'_>, mut session: Session) -> Self {
        configure_style(&cc.egui_ctx);
        let (status, status_error) = startup_status(&mut session);
        let app = Self::from_session(session, status, status_error);
        if app.capture_path.is_some() {
            if std::env::var("\x54ECHNOREA_CAPTURE_VIEW").as_deref() == Ok("library-expanded") {
                cc.egui_ctx.data_mut(|data| {
                    data.insert_persisted(
                        egui::Id::new("sounds"),
                        egui::containers::panel::PanelState {
                            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 480.0)),
                        },
                    )
                });
            }
            cc.egui_ctx.style_mut(|style| {
                style.animation_time = 0.0;
                style.scroll_animation = egui::style::ScrollAnimation::none();
            });
        }
        app
    }

    pub fn set_startup_messages(&mut self, messages: &[String]) {
        if !messages.is_empty() {
            self.status = format!("{} {}", self.status, messages.join(" "));
            self.status_error = true;
        }
    }

    fn from_session(mut session: Session, status: String, status_error: bool) -> Self {
        let capture_path = std::env::var_os("\x54ECHNOREA_CAPTURE_TO").map(PathBuf::from);
        let capture_view = if capture_path.is_some() {
            std::env::var("\x54ECHNOREA_CAPTURE_VIEW").unwrap_or_default()
        } else {
            String::new()
        };
        let status = with_import_summary(status, session.project());
        if capture_view == "follow" {
            session.seek(40 * TICKS_PER_BAR);
        }
        if capture_view == "unsaved" {
            let bpm = if session.project().bpm == 160.0 {
                140.0
            } else {
                160.0
            };
            let _ = session.edit(EditCommand::SetTempo(bpm));
        }
        let import_path = (capture_view == "import")
            .then(|| {
                session
                    .library()
                    .samples
                    .first()
                    .and_then(|sample| sample.wav_path.clone())
            })
            .flatten();
        let needs_sample_folder =
            session.library().samples.is_empty() && session.project().clips.is_empty();
        let selected_sample = if capture_view.starts_with("missing") {
            session.missing_samples().first().map(|sample| sample.id)
        } else if capture_view == "sample-details" {
            session
                .project()
                .imported_samples
                .first()
                .map(|sample| sample.id)
        } else {
            (!capture_view.is_empty())
                .then(|| session.library().samples.first().map(|sample| sample.id))
                .flatten()
        };
        Self {
            session,
            selected_sample,
            selected_clip: None,
            selected_clips: BTreeSet::new(),
            context_missing_sample: None,
            group: None,
            palette_product: None,
            search: String::new(),
            show_mixer: capture_view.starts_with("mixer"),
            show_help: capture_view == "help",
            show_libraries: matches!(
                capture_view.as_str(),
                "libraries" | "library-providers" | "library-tempos"
            ) || needs_sample_folder,
            show_import_details: matches!(
                capture_view.as_str(),
                "missing-details" | "import-details"
            ),
            show_sample_conflicts: capture_view == "sample-conflicts",
            sample_conflicts_dismissed: false,
            sample_conflict_error: None,
            library_product: Product::T1,
            rail_icons: None,
            erase: false,
            zoom: 1.0,
            follow_playhead: true,
            last_follow_position: None,
            drag: None,
            timeline_rect: None,
            timeline_drop_rect: None,
            timeline_ruler_rect: None,
            timeline_tracks: Vec::new(),
            timeline_row_by_track: [None; TRACK_COUNT],
            timeline_add_track: None,
            timeline_row_height: 30.0,
            timeline_bar_width: 62.0,
            loop_start: None,
            status,
            status_error,
            scan_status: None,
            pending_action: (capture_view == "unsaved").then_some(PendingAction::New),
            allow_close: false,
            selection_origin: None,
            selection_base: BTreeSet::new(),
            capture_path,
            capture_requested: false,
            capture_pass: 0,
            capture_last_track: capture_view == "track64",
            capture_transport_focus: capture_view == "transport-focus",
            capture_focus_ready: false,
            show_effects: matches!(
                capture_view.as_str(),
                "effects" | "effects-expanded" | "missing-effects"
            ),
            show_step_synth: matches!(
                capture_view.as_str(),
                "step-synth" | "step-synth-effects" | "missing-step-synth"
            ),
            show_sample_edit: matches!(capture_view.as_str(), "edit" | "missing-edit"),
            show_stretch: matches!(capture_view.as_str(), "stretch" | "missing-stretch"),
            effects: if capture_view == "effects-expanded" {
                EffectsState {
                    filter: Some(Default::default()),
                    echo: Some(Default::default()),
                    robotize: Some(Default::default()),
                    ..Default::default()
                }
            } else {
                Default::default()
            },
            step_synth: if capture_view == "step-synth-effects" {
                StepSynthState {
                    effects: EffectsState {
                        filter: Some(Default::default()),
                        echo: Some(Default::default()),
                        robotize: Some(Default::default()),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            } else {
                Default::default()
            },
            step_synth_bar: 0,
            step_synth_file: None,
            step_synth_effects_expanded: capture_view == "step-synth-effects",
            edit_start: 0,
            edit_end: 0,
            edit_pitch: 0.0,
            stretch_source_bpm: 140.0,
            stretch_target_bpm: 140.0,
            armed_track: if capture_view == "track64" {
                TRACK_COUNT - 1
            } else {
                0
            },
            import_path,
            import_bpm: 140.0,
        }
    }

    fn capture_view_is(&self, view: &str) -> bool {
        self.capture_path.is_some()
            && std::env::var("\x54ECHNOREA_CAPTURE_VIEW").as_deref() == Ok(view)
    }

    fn clear_selection(&mut self) {
        self.selected_sample = None;
        self.selected_clip = None;
        self.selected_clips.clear();
        self.context_missing_sample = None;
        self.show_sample_conflicts = false;
        self.sample_conflicts_dismissed = false;
        self.sample_conflict_error = None;
    }
    fn reconcile_selection(&mut self) {
        self.selected_clips.retain(|id| {
            self.session
                .project()
                .clips
                .iter()
                .any(|clip| clip.id == *id)
        });
        self.selected_clip = self.selected_clips.iter().next().copied();
        if self.selected_sample.is_some_and(|id| {
            self.session.library().sample_by_id(id).is_none()
                && self.session.missing_sample(id).is_none()
        }) {
            self.selected_sample = None;
        }
    }
    fn sync_notice(&mut self) -> bool {
        let Some((status, error)) = self.session.take_notice() else {
            return false;
        };
        self.status = status;
        self.status_error = error;
        true
    }
    fn poll_application(&mut self, ctx: &egui::Context) {
        let recording = self.session.is_recording();
        if let Some(delay) = self.session.poll() {
            ctx.request_repaint_after(Duration::from_millis(delay));
        }
        if self.session.take_wave_folder_adopted() {
            self.clear_selection();
            self.drag = None;
            self.selection_origin = None;
            self.selection_base.clear();
            self.edit_start = 0;
            self.edit_end = 0;
        }
        let noticed = self.sync_notice();
        if let Some(path) = self.session.take_opened_after_scan() {
            self.clear_selection();
            if !self.status_error {
                self.message(with_import_summary(
                    format!("Opened {}.", path.display()),
                    self.session.project(),
                ));
            }
        }
        match self.session.wave_scan() {
            // Progress owns the status line until another message replaces it.
            Some(scan)
                if !self.status_error
                    && !noticed
                    && self
                        .scan_status
                        .as_ref()
                        .is_none_or(|shown| *shown == self.status) =>
            {
                self.status = if scan.files_total == 0 {
                    format!("Listing {}…", scan.root.display())
                } else {
                    format!(
                        "Scanning {}: {} of {} WAVs",
                        scan.root.display(),
                        scan.files_done,
                        scan.files_total
                    )
                };
                self.scan_status = Some(self.status.clone());
            }
            Some(_) => {}
            None => self.scan_status = None,
        }
        if recording && !self.session.is_recording() && self.session.has_recorded_take() {
            self.save_recorded_take();
        }
    }

    fn message(&mut self, text: impl Into<String>) {
        let _ = self.session.take_notice();
        self.status = text.into();
        self.status_error = false;
    }

    fn error(&mut self, text: impl Into<String>) {
        let _ = self.session.take_notice();
        self.status = text.into();
        self.status_error = true;
    }

    fn start_playback(&mut self) {
        self.session.start_playback();
        self.sync_notice();
    }

    fn pause(&mut self) {
        self.session.pause();
        self.sync_notice();
    }

    fn stop(&mut self) {
        let recording = self.session.is_recording();
        self.session.stop();
        self.sync_notice();
        if recording && self.session.has_recorded_take() {
            self.save_recorded_take();
        }
    }

    fn seek(&mut self, tick: u32) {
        self.session.seek(tick);
        self.sync_notice();
    }

    fn preview(&mut self, sample_id: u32) {
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

    fn resolve_missing_sample(&mut self, sample_id: u32) {
        let Some(missing) = self.session.missing_sample(sample_id) else {
            return;
        };
        let title = format!("Resolve missing sample: {}", missing.name);
        let path = rfd::FileDialog::new()
            .set_title(title_text(&title))
            .add_filter("WAV audio", &["wav"])
            .pick_file();
        self.resolve_missing_sample_with(sample_id, path);
    }

    /// A cancelled picker leaves the project unchanged. The application
    /// validates the chosen audio and rejects a reference that no longer exists.
    fn resolve_missing_sample_with(&mut self, sample_id: u32, path: Option<PathBuf>) {
        let Some(path) = path else {
            return;
        };
        match self.session.resolve_missing_sample(sample_id, &path) {
            Ok(_) => {
                self.context_missing_sample = None;
                self.reconcile_selection();
                self.sync_notice();
            }
            Err(error) => self.error(error),
        }
    }

    fn apply(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.error(error);
        } else {
            self.sync_notice();
        }
    }

    fn place(&mut self, sample_id: u32, track: usize, tick: u32) {
        match self.session.place(sample_id, track, tick) {
            Ok(id) => {
                self.selected_clip = Some(id);
                self.selected_clips = [id].into_iter().collect();
                self.sync_notice();
            }
            Err(error) => self.error(error),
        }
    }

    fn open(&mut self) {
        let root = self
            .session
            .library()
            .wav_root()
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
        let folder = [
            root.join("\x65Jay/mix"),
            root.join("MIX"),
            root.join("D2/MIX"),
        ]
        .into_iter()
        .find(|path| path.is_dir())
        .unwrap_or(root);
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Songs", &["mix", "mixjam", "json"])
            .set_directory(folder)
            .pick_file()
        else {
            return;
        };
        self.open_path(&path);
    }

    fn open_path(&mut self, path: &Path) {
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

    fn save(&mut self, choose_path: bool) {
        let path = if choose_path || self.session.file().is_none() {
            rfd::FileDialog::new()
                .add_filter("MixJam Techno project", &["json"])
                .set_file_name(format!("{}.json", self.session.project().title))
                .save_file()
        } else {
            self.session.file().map(Path::to_path_buf)
        };
        let Some(path) = path else {
            return;
        };
        match self.session.save(&path) {
            Ok(()) => self.message(format!("Saved {}.", path.display())),
            Err(error) => self.error(format!("Cannot save mix: {error}")),
        }
    }

    fn export(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Stereo WAV", &["wav"])
            .set_file_name(format!("{}.wav", self.session.project().title))
            .save_file()
        else {
            return;
        };
        match self.session.export(&path) {
            Ok(()) => self.message(format!("Exported {}.", path.display())),
            Err(error) => self.error(format!("Cannot export mix: {error}")),
        }
    }

    fn new_mix(&mut self) {
        match self.session.new_mix() {
            Ok(()) => {
                self.clear_selection();
                self.pending_action = None;
                self.message("New mix.");
            }
            Err(error) => self.error(error),
        }
    }

    fn add_library(&mut self, product: Product, root: &Path) {
        match self.session.add_library(product, root) {
            Ok(()) => {
                self.reconcile_selection();
                self.message(format!("Added {} samples.", product.label()));
            }
            Err(error) => self.error(format!("Cannot add library: {error}")),
        }
    }

    fn select_wave_folder(&mut self, root: &Path) {
        match self.session.start_wave_scan(root, self.import_bpm) {
            Ok(()) => {
                self.clear_selection();
                self.message(format!("Scanning {}…", root.display()));
            }
            Err(error) => self.error(error),
        }
    }

    fn remove_library(&mut self, product: Product) {
        match self.session.remove_library(product) {
            Ok(()) => {
                if self.palette_product == Some(product) {
                    self.palette_product = None;
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

    fn request_action(&mut self, action: PendingAction, ctx: &egui::Context) {
        if self.session.is_recording() {
            self.error("Stop recording before you open, close, or start a new mix.");
            return;
        }
        if self.session.needs_save_confirmation() {
            self.pending_action = Some(action);
        } else {
            self.run_action(action, ctx);
        }
    }

    fn run_action(&mut self, action: PendingAction, ctx: &egui::Context) {
        self.pending_action = None;
        self.session.discard_recorded_take();
        match action {
            PendingAction::New => self.new_mix(),
            PendingAction::Open => self.open(),
            PendingAction::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn history_step(&mut self, redo: bool) {
        match self.session.history_step(redo) {
            Ok(true) => {
                self.reconcile_selection();
                self.sync_notice();
            }
            Ok(false) => {}
            Err(error) => self.error(error),
        }
    }

    fn select_clip(&mut self, id: u64, additive: bool) {
        if let Some(clip) = self.session.project().clips.iter().find(|c| c.id == id) {
            self.selected_sample = Some(clip.sample_id);
        }
        if additive {
            if !self.selected_clips.insert(id) {
                self.selected_clips.remove(&id);
            }
        } else {
            self.selected_clips.clear();
            self.selected_clips.insert(id);
        }
        self.selected_clip = self
            .selected_clips
            .contains(&id)
            .then_some(id)
            .or_else(|| self.selected_clips.iter().next().copied());
    }

    fn delete_selection(&mut self) {
        if self.selected_clips.is_empty() {
            return;
        }
        let result = self.session.edit(EditCommand::DeleteClips(
            self.selected_clips.iter().copied().collect(),
        ));
        if result.is_ok() {
            self.selected_clip = None;
            self.selected_clips.clear();
        }
        self.apply(result);
    }

    fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            if self.session.wants_playback() {
                self.pause();
            } else {
                self.start_playback();
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.delete_selection();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            let redo = ctx.input(|i| i.modifiers.shift);
            self.history_step(redo);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            self.save(false);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            self.request_action(PendingAction::Open, ctx);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.drag = None;
            self.erase = false;
        }
    }

    fn sample_product(&self, id: u32) -> Option<Product> {
        Product::sample_key(id)
            .map(|(product, _)| product)
            .or_else(|| {
                self.session
                    .project()
                    .imported_samples
                    .iter()
                    .find(|sample| sample.id == id)
                    .and_then(|sample| sample.origin)
            })
    }

    fn palette_indices(&self) -> Vec<usize> {
        let query = self.search.to_lowercase();
        self.session
            .library()
            .samples
            .iter()
            .enumerate()
            .filter_map(|(index, sample)| {
                (self
                    .group
                    .as_deref()
                    .is_none_or(|group| sample.group_name.eq_ignore_ascii_case(group))
                    && self
                        .palette_product
                        .is_none_or(|product| self.sample_product(sample.id) == Some(product))
                    && (query.is_empty() || sample.name.to_lowercase().contains(&query)))
                .then_some(index)
            })
            .collect()
    }

    fn position_tick(&self, x: f32, rect: Rect, fine: bool) -> u32 {
        let raw =
            ((x - rect.left()).max(0.0) / self.timeline_bar_width * TICKS_PER_BAR as f32) as u32;
        let step = if fine {
            TICKS_PER_BAR / 16
        } else {
            TICKS_PER_BAR
        };
        (raw / step) * step
    }

    fn timeline_track_at(&self, pos: Pos2) -> Option<usize> {
        let rect = self.timeline_rect?;
        if pos.y < rect.top() || pos.y >= rect.bottom() {
            return None;
        }
        let row = ((pos.y - rect.top()) / self.timeline_row_height) as usize;
        self.timeline_tracks.get(row).copied()
    }

    fn clip_rect(&self, clip: &Clip, timeline: Rect) -> Rect {
        let Some(row) = self
            .timeline_row_by_track
            .get(clip.track)
            .copied()
            .flatten()
        else {
            return Rect::NOTHING;
        };
        Rect::from_min_size(
            Pos2::new(
                timeline.left()
                    + clip.start_tick as f32 / TICKS_PER_BAR as f32 * self.timeline_bar_width,
                timeline.top() + row as f32 * self.timeline_row_height + 2.0,
            ),
            Vec2::new(
                (clip.length_ticks as f32 / TICKS_PER_BAR as f32 * self.timeline_bar_width - 1.0)
                    .max(3.0),
                self.timeline_row_height - 4.0,
            ),
        )
    }

    fn finish_drag(&mut self, ctx: &egui::Context) {
        if self.drag.is_none() {
            return;
        }
        if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("drag_sound"),
            ));
            let label = match self.drag.as_ref().unwrap() {
                DragItem::Sample(id) => self
                    .session
                    .library()
                    .sample_by_id(*id)
                    .map(|s| s.name.clone())
                    .unwrap_or_default(),
                DragItem::Clip { copy, .. } => {
                    if *copy { "Copy sound" } else { "Move sound" }.into()
                }
            };
            let rect = Rect::from_min_size(pos + Vec2::new(14.0, 14.0), Vec2::new(150.0, 30.0));
            painter.rect_filled(rect, 4.0, Color32::from_rgb(245, 220, 146));
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(12.0),
                CANVAS,
            );
        }
        if !ctx.input(|i| i.pointer.any_released()) {
            return;
        }
        let item = self.drag.take().unwrap();
        let pos = ctx.input(|i| i.pointer.latest_pos());
        if let (Some(pos), Some(rect)) = (pos, self.timeline_rect)
            && self
                .timeline_drop_rect
                .is_some_and(|visible| visible.contains(pos))
        {
            let Some(track) = self.timeline_track_at(pos) else {
                return;
            };
            let fine = ctx.input(|i| i.modifiers.ctrl);
            match item {
                DragItem::Sample(id) => {
                    self.place(id, track, self.position_tick(pos.x, rect, fine))
                }
                DragItem::Clip { id, offset, copy } => {
                    let raw_tick = self.position_tick(pos.x, rect, true).saturating_sub(offset);
                    let step = if fine {
                        TICKS_PER_BAR / 16
                    } else {
                        TICKS_PER_BAR
                    };
                    let tick = raw_tick / step * step;
                    if let Some(anchor) = self.session.project().clips.iter().find(|c| c.id == id) {
                        let ids: Vec<_> = self.selected_clips.iter().copied().collect();
                        let track_delta = track as i32 - anchor.track as i32;
                        let tick_delta = i64::from(tick) - i64::from(anchor.start_tick);
                        match self.session.move_clips(&ids, track_delta, tick_delta, copy) {
                            Ok(ids) => {
                                self.selected_clip = ids.first().copied();
                                self.selected_clips = ids.into_iter().collect();
                                self.sync_notice();
                            }
                            Err(error) => self.error(error),
                        }
                    }
                }
            }
            return;
        }
        if let DragItem::Clip { copy, .. } = item {
            let action = if copy { "Copy" } else { "Move" };
            let destination = if self.timeline_add_track.is_some() {
                "a track or the Add row"
            } else {
                "a track"
            };
            self.message(format!("{action} canceled. Drop on {destination}."));
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.pending_action {
            egui::Modal::new(egui::Id::new("unsaved_changes")).show(ctx, |ui| {
                ui.heading("SAVE YOUR CHANGES?");
                ui.label(
                    if self.session.has_recorded_take() && self.session.unsaved() {
                        "The mix and recorded take are unsaved."
                    } else if self.session.has_recorded_take() {
                        "The recorded take is unsaved."
                    } else {
                        "The mix is unsaved."
                    },
                );
                ui.horizontal(|ui| {
                    if ui.button("Save changes").clicked() {
                        if self.session.has_recorded_take() {
                            self.save_recorded_take();
                        }
                        if !self.session.has_recorded_take() && self.session.unsaved() {
                            self.save(false);
                        }
                        if !self.session.unsaved() && !self.session.has_recorded_take() {
                            self.run_action(action, ctx);
                        }
                    }
                    if ui.button("Discard changes").clicked() {
                        self.run_action(action, ctx);
                    }
                    if ui.button("Cancel").clicked() {
                        self.pending_action = None;
                    }
                });
            });
        }
        titled_window("Help").open(&mut self.show_help).resizable(false).default_width(450.0)
            .max_height((ctx.viewport_rect().height() - 80.0).max(240.0))
            .constrain_to(ctx.viewport_rect().shrink(12.0)).vscroll(true).show(ctx, |ui| {
                egui::Grid::new("help_gestures").num_columns(2).spacing(Vec2::new(18.0, 8.0)).show(ui, |ui| {
                    for (action, gesture) in [
                        ("Place sound", "Drag to a track, or select a sound and click a track. Up to 64 tracks."),
                        ("Preview", "Double-click a sound or clip."),
                        ("Move / copy", "Drag a clip. Alt-drag copies."),
                        ("Select group", "Shift-click clips or drag an empty area."),
                        ("Fine snap", "Hold Ctrl for sixteenth-note positions."),
                        ("Delete", "Select clips, then press Delete."),
                        ("Seek / loop", "Click a bar number to seek. Drag the ruler to loop."),
                        ("Play / pause", "Space"),
                        ("Undo / redo", "Ctrl+Z / Ctrl+Shift+Z"),
                        ("Open / save", "Ctrl+O / Ctrl+S. Open MIX, MIXJAM, or JSON; save JSON."),
                        ("Export", "Stereo WAV"),
                    ] {
                        ui.label(action);
                        ui.add(egui::Label::new(gesture).wrap());
                        ui.end_row();
                    }
                });
                ui.separator();
                ui.label("Mixer: gain, pan, L/R outputs, and master gain. Both outputs off mutes a track.");
                ui.label("Gain: 0 is silence; +1 is unity (unchanged level). Negative gain inverts polarity; its magnitude controls level.");
                ui.label("Click a gain value for exact entry, then Enter. Unity resets gain; Center resets pan. Master stays visible while tracks scroll.");
                ui.label("Tab focuses controls. Arrow keys adjust sliders. Space toggles outputs.");
            });
    }

    fn sample_conflicts_open(&self) -> bool {
        !self.session.sample_conflicts().is_empty()
            && (self.show_sample_conflicts
                || (!self.sample_conflicts_dismissed
                    && self
                        .session
                        .sample_conflicts()
                        .iter()
                        .any(|conflict| !conflict.confirmed)))
    }

    fn sample_conflicts_dialog(&mut self, ctx: &egui::Context) {
        if !self.sample_conflicts_open() || self.pending_action.is_some() {
            return;
        }
        self.show_sample_conflicts = true;
        let conflicts = self.session.sample_conflicts().to_vec();
        let recording = self.session.is_recording();
        let mut choice = None;
        let mut confirm = false;
        let response = egui::Modal::new(egui::Id::new("sample_conflicts")).show(ctx, |ui| {
            ui.set_width((ctx.viewport_rect().width() - 96.0).clamp(280.0, 700.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.heading(title_text("Sample conflicts"));
            ui.label("Several files match a saved sample. The first match is selected until you choose another.");
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
                            ui.add(egui::Label::new(display_paths(&conflict.reference)).wrap().selectable(true));
                            let instances = self.session.project().clips.iter()
                                .filter(|clip| conflict.sample_ids.contains(&clip.sample_id)).count();
                            ui.label(egui::RichText::new(format!("{instances} instances · {} matches", conflict.candidates.len())).color(MUTED));
                            ui.add_enabled_ui(!recording, |ui| {
                                for (candidate_index, path) in conflict.candidates.iter().enumerate() {
                                    let path = display_paths(&path.to_string_lossy());
                                    let selected = candidate_index == conflict.selected;
                                    let label = format!("{}{}\n{path}",
                                        if selected { "Selected match " } else { "Use match " },
                                        candidate_index + 1);
                                    if ui.add(egui::RadioButton::new(selected, label)).clicked() && !selected {
                                        choice = Some((index, candidate_index));
                                    }
                                }
                            });
                        });
                    }
                });
            ui.add_space(10.0);
            if recording {
                ui.colored_label(CORAL, "Stop recording before changing or confirming sample choices.");
                if ui.button("Stop recording").clicked() {
                    self.stop();
                }
            }
            if let Some(error) = &self.sample_conflict_error {
                ui.colored_label(CORAL, display_paths(error));
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(!recording, egui::Button::new("Keep choices")).clicked() {
                    confirm = true;
                }
                let playing = self.session.wants_playback();
                if ui.add_enabled(!recording, egui::Button::new(if playing { "Pause song" } else { "Play song" })).clicked() {
                    if playing { self.pause(); } else { self.start_playback(); }
                }
            });
            ui.small("Esc closes this window without confirming. Reopen from Sample conflicts in the status bar.");
        });
        if let Some((conflict, candidate)) = choice {
            match self.session.resolve_sample_conflict(conflict, candidate) {
                Ok(()) => {
                    self.sample_conflict_error = None;
                    self.reconcile_selection();
                    self.message("Updated all instances of the selected source.");
                }
                Err(error) => self.sample_conflict_error = Some(error),
            }
        }
        if confirm && !recording {
            match self.session.confirm_sample_conflicts() {
                Ok(()) => {
                    self.show_sample_conflicts = false;
                    self.sample_conflicts_dismissed = false;
                    self.sample_conflict_error = None;
                    self.message("Sample choices retained.");
                }
                Err(error) => self.sample_conflict_error = Some(error),
            }
        } else if response.should_close() {
            // Escape or a click outside only hides the window. Confirmation is
            // an undoable project change, so it needs the explicit button.
            self.show_sample_conflicts = false;
            self.sample_conflicts_dismissed = true;
            self.sample_conflict_error = None;
            self.message(
                "Sample choices are not confirmed. Reopen from Sample conflicts in the status bar.",
            );
        }
    }

    fn capture(&mut self, ctx: &egui::Context) {
        let Some(path) = self.capture_path.clone() else {
            return;
        };
        if self.capture_transport_focus && !self.capture_focus_ready {
            ctx.request_repaint_after(Duration::from_millis(20));
            return;
        }
        self.capture_pass += 1;
        let image = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            if let Err(error) = write_bmp(&path, &image) {
                eprintln!("Cannot save UI capture: {error}");
            }
            self.allow_close = true;
            self.capture_path = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if !self.capture_requested && self.capture_pass >= 3 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.capture_requested = true;
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(20));
        }
    }

    fn source_id(&self) -> Option<u32> {
        self.selected_sample.or_else(|| {
            self.selected_clip.and_then(|id| {
                self.session
                    .project()
                    .clips
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.sample_id)
            })
        })
    }

    fn source_name(&self) -> String {
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

    fn source_bpm(&self) -> f64 {
        self.session.source_bpm(self.source_id())
    }

    fn show_missing_source(&mut self, ui: &mut egui::Ui) -> bool {
        if self
            .source_id()
            .and_then(|id| self.session.missing_sample(id))
            .is_none()
        {
            return false;
        }
        ui.label("Missing audio: clips play silence; processing is unavailable.");
        if ui.button("Import details").clicked() {
            self.show_import_details = true;
        }
        true
    }

    fn play_pcm(&mut self, pcm: Pcm, label: &str) {
        self.session.play_pcm(pcm, label);
        self.sync_notice();
    }

    fn register_file(&mut self, path: &Path, group: &str, source_bpm: f64) -> Result<u32, String> {
        let id = self.session.register_file(path, group, source_bpm)?;
        self.group = Some(group.into());
        self.palette_product = None;
        self.selected_sample = Some(id);
        self.sync_notice();
        Ok(id)
    }

    fn import_wav(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("WAV audio", &["wav"])
            .pick_file()
        else {
            return;
        };
        self.import_path = Some(path);
        self.import_bpm = 140.0;
    }

    fn save_pcm(
        &mut self,
        pcm: &Pcm,
        name: &str,
        group: &str,
        source_bpm: f64,
    ) -> Result<Option<u32>, String> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("WAV audio", &["wav"])
            .set_file_name(format!("{name}.wav"))
            .save_file()
        else {
            return Ok(None);
        };
        let id = self.session.save_pcm(pcm, &path, group, source_bpm)?;
        self.group = Some(group.into());
        self.palette_product = None;
        self.selected_sample = Some(id);
        self.message(format!("Saved {} and added it to {group}.", path.display()));
        Ok(Some(id))
    }

    fn start_recording(&mut self) {
        let result = self.session.start_recording(self.armed_track);
        self.apply(result);
    }

    fn save_recorded_take(&mut self) {
        if !self.session.has_recorded_take() {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("WAV audio", &["wav"])
            .set_file_name("Recording.wav")
            .save_file()
        else {
            self.message("Unsaved take: stored in memory. Save it before closing.");
            return;
        };
        match self.session.save_recorded_take(&path) {
            Ok(clip) => {
                self.group = Some("Wave".into());
                self.palette_product = None;
                if let Some(id) = clip {
                    self.select_clip(id, false);
                    self.message(format!("Saved {}.", path.display()));
                } else {
                    self.sync_notice();
                }
            }
            Err(error) => self.error(error),
        }
    }

    fn tool_windows(&mut self, ctx: &egui::Context) {
        if self.show_libraries {
            let mut open = true;
            titled_window("Sample libraries")
                .open(&mut open)
                .default_width(540.0)
                .max_height((ctx.viewport_rect().height() - 80.0).max(240.0))
                .constrain_to(ctx.viewport_rect().shrink(12.0))
                .vscroll(true)
                .show(ctx, |ui| {
                    let scan = self.session.wave_scan();
                    ui.horizontal(|ui| {
                        ui.label("Fallback BPM");
                        ui.add(egui::DragValue::new(&mut self.import_bpm).range(1.0..=999.0))
                            .on_hover_text("Recognized product audio keeps its original tempo. Folders use matching original audio when enough sources agree, then loop lengths, then this fallback. Unresolved half/double matches prefer 88–176 BPM unless the fallback selects the other tempo.");
                        if ui
                            .add_enabled(
                                !self.session.is_recording() && scan.is_none(),
                                egui::Button::new("Choose WAV folder"),
                            )
                            .on_disabled_hover_text(if scan.is_some() {
                                "Wait for the current scan to finish or cancel it."
                            } else {
                                "Stop recording before you change the sample folder."
                            })
                            .clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder()
                        {
                            self.select_wave_folder(&path);
                        }
                    });
                    if let Some(scan) = &scan {
                        ui.horizontal(|ui| {
                            let (text, fraction) = if scan.files_total == 0 {
                                ("Listing files…".to_string(), 0.0)
                            } else {
                                (
                                    format!("{} of {} WAVs", scan.files_done, scan.files_total),
                                    scan.files_done as f32 / scan.files_total as f32,
                                )
                            };
                            ui.add(
                                egui::ProgressBar::new(fraction)
                                    .desired_width(300.0)
                                    .animate(true)
                                    .text(text),
                            );
                            if ui.button("Cancel").clicked() {
                                self.session.cancel_wave_scan();
                                self.sync_notice();
                            }
                        });
                        ui.small(format!("Scanning {}", display_paths(&scan.root.to_string_lossy())));
                    } else if let Some(root) = self.session.library().wav_root() {
                        ui.label(display_paths(&root.to_string_lossy()));
                    }
                    let library = self.session.library();
                    if let Some(root) = library.wav_root()
                        && !library.folder_tempos().is_empty()
                    {
                        egui::CollapsingHeader::new(format!("Folder tempos ({})", library.folder_tempos().len()))
                            .default_open(self.capture_view_is("library-tempos"))
                            .show(ui, |ui| {
                                for (folder, bpm, count) in library.folder_tempos() {
                                    let relative = folder.strip_prefix(root).unwrap_or(folder);
                                    let name = if relative.as_os_str().is_empty() {
                                        ".".to_string()
                                    } else {
                                        display_paths(&relative.to_string_lossy())
                                    };
                                    ui.small(format!("{name}: {bpm} BPM, {count} WAVs"));
                                }
                            });
                    }
                    egui::CollapsingHeader::new("Optional product providers")
                        .default_open(self.capture_view_is("library-providers")).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("library_product")
                            .selected_text(self.library_product.label())
                            .show_ui(ui, |ui| {
                                for product in Product::ALL {
                                    ui.selectable_value(
                                        &mut self.library_product,
                                        product,
                                        product.label(),
                                    );
                                }
                            });
                        if ui.button("Choose folder").clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder()
                        {
                            self.add_library(self.library_product, &path);
                        }
                    });
                    ui.separator();
                    let bindings = self.session.library().bindings().to_vec();
                    if bindings.is_empty() {
                        ui.label("No providers");
                    }
                    for binding in bindings {
                        ui.horizontal(|ui| {
                            ui.label(binding.product.label());
                            if ui.small_button("Remove").clicked() {
                                self.remove_library(binding.product);
                            }
                        });
                        let path = display_paths(&binding.root.to_string_lossy());
                        ui.add(egui::Label::new(&path).truncate())
                            .on_hover_text(&path);
                        ui.add_space(6.0);
                    }
                    });
                });
            self.show_libraries = open;
        }
        if self.show_import_details {
            let mut open = true;
            titled_window("Import details")
                .open(&mut open)
                .default_width(540.0)
                .max_height((ctx.viewport_rect().height() - 80.0).max(240.0))
                .constrain_to(ctx.viewport_rect().shrink(12.0))
                .vscroll(true)
                .show(ctx, |ui| {
                    if !self.session.missing_samples().is_empty() {
                        ui.heading("MISSING SAMPLES");
                        ui.label("Missing samples play silence. Select details to copy.");
                        for missing in self.session.missing_samples() {
                            let selected = self.source_id() == Some(missing.id);
                            egui::CollapsingHeader::new(format!("Missing: {}", missing.name))
                                .id_salt(("missing_sample", missing.id))
                                .default_open(selected || self.session.missing_samples().len() == 1)
                                .show(ui, |ui| {
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
                            ui.collapsing("Saved MixJam source controls", |ui| {
                                ui.label(display_paths(notice));
                            });
                        } else {
                            ui.label(display_paths(notice));
                        }
                    }
                });
            self.show_import_details = open;
        }
        if let Some(path) = self.import_path.clone() {
            egui::Modal::new(egui::Id::new("import_sound")).show(ctx, |ui| {
                ui.heading("IMPORT SOUND");
                ui.label(path.file_name().unwrap_or_default().to_string_lossy())
                    .on_hover_text(display_paths(&path.to_string_lossy()));
                ui.horizontal(|ui| {
                    ui.label("Source BPM");
                    ui.add(egui::DragValue::new(&mut self.import_bpm).range(7.5..=1200.0))
                        .on_hover_text("The sound's original tempo, separate from project BPM.");
                });
                ui.horizontal(|ui| {
                    if ui.button("Import into Wave").clicked() {
                        match self.register_file(&path, "Wave", self.import_bpm) {
                            Ok(_) => {
                                self.import_path = None;
                                self.message(format!("Imported {}.", path.display()));
                            }
                            Err(error) => self.error(error),
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        self.import_path = None;
                    }
                });
            });
        }
        if self.show_effects {
            let mut open = true;
            titled_window("Effect Studio")
                .open(&mut open)
                .default_width(560.0)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(self.source_name()).strong().color(TEXT));
                    if self.show_missing_source(ui) {
                        return;
                    }
                    effects_controls(ui, &mut self.effects);
                    ui.horizontal(|ui| {
                        if ui.button("Listen").clicked() {
                            match self.session.processed_pcm(self.source_id(), &self.effects) {
                                Ok(pcm) => self.play_pcm(pcm, "Listening to the processed sound."),
                                Err(error) => self.error(error),
                            }
                        }
                        if ui
                            .button("Save sound")
                            .on_hover_text("Save a new WAV and add it to Effect.")
                            .clicked()
                        {
                            let result =
                                self.session.processed_pcm(self.source_id(), &self.effects);
                            match result {
                                Ok(pcm) => {
                                    let name = format!("{} - effect", self.source_name());
                                    if let Err(error) =
                                        self.save_pcm(&pcm, &name, "Effect", self.source_bpm())
                                    {
                                        self.error(error);
                                    }
                                }
                                Err(error) => self.error(error),
                            }
                        }
                        if ui.button("Reset").clicked() {
                            self.effects = Default::default();
                        }
                    });
                });
            self.show_effects = open;
        }
        if self.show_step_synth {
            let mut open = true;
            titled_window("Step synth")
                .open(&mut open)
                .default_width(820.0)
                .max_height((ctx.viewport_rect().height() - 80.0).max(240.0))
                .constrain_to(ctx.viewport_rect().shrink(12.0))
                .vscroll(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(self.step_synth_file.as_ref().map_or_else(
                                || self.source_name(),
                                |p| {
                                    p.file_stem()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned()
                                },
                            ))
                            .strong(),
                        );
                        if ui.button("Choose source file").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Generator sound / WAV", &["\x70xd", "wav"])
                                .set_directory(
                                    self.session
                                        .library()
                                        .wav_root()
                                        .or_else(|| self.session.library().source_root(Product::T2))
                                        .unwrap_or_else(|| Path::new(".")),
                                )
                                .pick_file()
                        {
                            self.step_synth_file = Some(path);
                        }
                        if ui.button("Use palette sound").clicked() {
                            self.step_synth_file = None;
                        }
                    });
                    if self.step_synth_file.is_none() && self.show_missing_source(ui) {
                        return;
                    }
                    ui.horizontal(|ui| {
                        ui.label("View bar");
                        for bar in 0..4 {
                            if ui
                                .selectable_label(
                                    self.step_synth_bar == bar,
                                    format!("{}", bar + 1),
                                )
                                .clicked()
                            {
                                self.step_synth_bar = bar;
                            }
                        }
                        ui.add_space(12.0);
                        for bar in 0..4 {
                            ui.checkbox(
                                &mut self.step_synth.active_bars[bar],
                                format!("Active {}", bar + 1),
                            );
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.step_synth.cut_sixteenth, "1/16 cut");
                        ui.checkbox(&mut self.step_synth.loop_source, "Loop source");
                        ui.label("BPM");
                        bpm_picker(ui, "step_synth_bpm", &mut self.step_synth.bpm);
                        if ui.button("Clear bar").clicked() {
                            self.step_synth.steps
                                [self.step_synth_bar * 16..self.step_synth_bar * 16 + 16]
                                .fill(None);
                        }
                    });
                    ui.scope(|ui| {
                        ui.spacing_mut().interact_size.y = 16.0;
                        ui.spacing_mut().button_padding.y = 0.0;
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Body, FontId::proportional(12.0));
                        egui::Grid::new("step_synth_notes")
                            .spacing(Vec2::new(2.0, 2.0))
                            .show(ui, |ui| {
                                for note in (-12_i8..=12).rev() {
                                    ui.label(if note == 0 {
                                        "A  0".to_string()
                                    } else {
                                        format!("{note:+}")
                                    });
                                    for step in 0..16 {
                                        let index = self.step_synth_bar * 16 + step;
                                        let selected = self.step_synth.steps[index] == Some(note);
                                        let fill = if selected {
                                            CORAL
                                        } else if matches!(
                                            (note + 24) % 12,
                                            0 | 2 | 3 | 5 | 7 | 8 | 10
                                        ) {
                                            Color32::from_rgb(39, 45, 51)
                                        } else {
                                            CANVAS
                                        };
                                        let label =
                                            format!("Step {}, {note:+} semitones", step + 1);
                                        let response = ui.add_sized(
                                            [33.0, 16.0],
                                            egui::Button::new("").fill(fill),
                                        );
                                        response.widget_info(|| {
                                            egui::WidgetInfo::selected(
                                                egui::WidgetType::SelectableLabel,
                                                ui.is_enabled(),
                                                selected,
                                                &label,
                                            )
                                        });
                                        if response.on_hover_text(label).clicked() {
                                            self.step_synth.steps[index] =
                                                if selected { None } else { Some(note) };
                                        }
                                    }
                                    ui.end_row();
                                }
                                ui.label("Step");
                                for step in 1..=16 {
                                    ui.label(format!("{step}"));
                                }
                                ui.end_row();
                            });
                    });
                    egui::CollapsingHeader::new("Effects")
                        .default_open(self.step_synth_effects_expanded)
                        .show(ui, |ui| effects_controls(ui, &mut self.step_synth.effects));
                    ui.horizontal(|ui| {
                        if ui.button("Listen sequence").clicked() {
                            match self.step_synth_pcm() {
                                Ok(pcm) => self.play_pcm(pcm, "Listening to Step synth sequence."),
                                Err(error) => self.error(error),
                            }
                        }
                        if ui
                            .button("Save sequence")
                            .on_hover_text("Save a WAV and add it to Step synth.")
                            .clicked()
                        {
                            match self.step_synth_pcm() {
                                Ok(pcm) => {
                                    if let Err(error) = self.save_pcm(
                                        &pcm,
                                        "Step synth sequence",
                                        "Step synth",
                                        self.step_synth.bpm,
                                    ) {
                                        self.error(error);
                                    }
                                }
                                Err(error) => self.error(error),
                            }
                        }
                        if ui
                            .button("Save pattern")
                            .on_hover_text("Save the editable pattern as JSON.")
                            .clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Step synth pattern", &["json"])
                                .set_file_name("Step synth pattern.json")
                                .save_file()
                        {
                            match self.session.save_pattern(&path, &self.step_synth) {
                                Ok(()) => self.message("Saved Step synth pattern."),
                                Err(error) => self.error(error),
                            }
                        }
                        if ui.button("Load pattern").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Step synth pattern", &["json"])
                                .pick_file()
                        {
                            match self.session.load_pattern(&path) {
                                Ok(state) => {
                                    self.step_synth = state;
                                    self.message("Loaded Step synth pattern.");
                                }
                                Err(error) => self.error(error),
                            }
                        }
                    });
                    if self.capture_path.is_some() && self.step_synth_effects_expanded {
                        ui.scroll_to_cursor_animation(
                            Some(egui::Align::BOTTOM),
                            egui::style::ScrollAnimation::none(),
                        );
                    }
                });
            self.show_step_synth = open;
        }
        if self.show_sample_edit {
            let mut open = true;
            titled_window("Sample editor")
                .open(&mut open)
                .default_width(510.0)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(self.source_name()).strong());
                    if self.show_missing_source(ui) {
                        return;
                    }
                    let frames = self
                        .source_id()
                        .and_then(|id| self.session.library().sample_by_id(id))
                        .map_or(0, |s| u64::from(s.frame_count));
                    if self.edit_end == 0 {
                        self.edit_end = frames;
                    }
                    ui.horizontal(|ui| {
                        ui.label("Start frame");
                        ui.add(
                            egui::DragValue::new(&mut self.edit_start)
                                .range(0..=frames.saturating_sub(1))
                                .speed(100),
                        );
                        ui.label("End frame");
                        ui.add(
                            egui::DragValue::new(&mut self.edit_end)
                                .range(1..=frames.max(1))
                                .speed(100),
                        );
                    });
                    ui.add(
                        egui::Slider::new(&mut self.edit_pitch, -24.0..=24.0)
                            .text("Pitch (semitones)"),
                    )
                    .on_hover_text("Pitch changes playback speed and sample length.");
                    ui.horizontal(|ui| {
                        if ui.button("Listen edit").clicked() {
                            match self.edited_pcm() {
                                Ok(pcm) => self.play_pcm(pcm, "Listening to the sample edit."),
                                Err(error) => self.error(error),
                            }
                        }
                        if ui.button("Save edited sound").clicked() {
                            match self.edited_pcm() {
                                Ok(pcm) => {
                                    let name = format!("{} - edit", self.source_name());
                                    if let Err(error) =
                                        self.save_pcm(&pcm, &name, "Wave", self.source_bpm())
                                    {
                                        self.error(error);
                                    }
                                }
                                Err(error) => self.error(error),
                            }
                        }
                    });
                });
            self.show_sample_edit = open;
        }
        if self.show_stretch {
            let mut open = true;
            titled_window("Time Stretcher")
                .open(&mut open)
                .default_width(470.0)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(self.source_name()).strong());
                    if self.show_missing_source(ui) {
                        return;
                    }
                    ui.horizontal(|ui| {
                        ui.label("Original BPM");
                        ui.add(
                            egui::DragValue::new(&mut self.stretch_source_bpm).range(30.0..=300.0),
                        );
                        ui.label("Target BPM");
                        bpm_picker(ui, "stretch_bpm", &mut self.stretch_target_bpm);
                    });
                    ui.small("Pitch preservation is approximate.");
                    ui.horizontal(|ui| {
                        if ui.button("Listen stretch").clicked() {
                            match self.stretched_pcm() {
                                Ok(pcm) => self.play_pcm(pcm, "Listening to the stretched sound."),
                                Err(error) => self.error(error),
                            }
                        }
                        if ui.button("Save stretched sound").clicked() {
                            match self.stretched_pcm() {
                                Ok(pcm) => {
                                    let name = format!("{} - stretch", self.source_name());
                                    if let Err(error) =
                                        self.save_pcm(&pcm, &name, "Wave", self.stretch_target_bpm)
                                    {
                                        self.error(error);
                                    }
                                }
                                Err(error) => self.error(error),
                            }
                        }
                    });
                });
            self.show_stretch = open;
        }
        if (self.session.has_recorded_take() && !self.session.is_recording())
            || self.capture_view_is("recorded-take")
        {
            titled_window("Recorded take")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Unsaved take: stored in memory.");
                    ui.horizontal(|ui| {
                        if ui.button("Listen").clicked() {
                            self.session.preview_recorded_take();
                            self.sync_notice();
                        }
                        if ui.button("Save take").clicked() {
                            self.save_recorded_take();
                        }
                        if ui.button("Discard take").clicked() {
                            self.session.discard_recorded_take();
                        }
                    });
                });
        }
    }

    fn step_synth_pcm(&self) -> Result<Pcm, String> {
        self.session.step_synth_pcm(
            self.source_id(),
            self.step_synth_file.as_deref(),
            &self.step_synth,
        )
    }

    fn edited_pcm(&self) -> Result<Pcm, String> {
        self.session.edited_pcm(
            self.source_id(),
            self.edit_start,
            self.edit_end,
            self.edit_pitch,
        )
    }

    fn stretched_pcm(&self) -> Result<Pcm, String> {
        self.session.stretched_pcm(
            self.source_id(),
            self.stretch_source_bpm,
            self.stretch_target_bpm,
        )
    }
}

fn occupied_tracks(project: &Project) -> Vec<usize> {
    project
        .clips
        .iter()
        .map(|clip| clip.track)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn with_import_summary(text: String, project: &Project) -> String {
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

impl eframe::App for MixJamApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}
