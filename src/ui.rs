use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::Duration,
};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Vec2};

use crate::{
    audio::{self, Pcm, Player},
    engine::{Clip, Editor, Project, SUPPORTED_BPMS, TICKS_PER_BAR, TRACK_COUNT},
    library::{Library, Sample},
    products::Product,
    ui_icons::{RailIcon, RailIcons},
};

const NAVY: Color32 = Color32::from_rgb(5, 9, 42);
const COBALT: Color32 = Color32::from_rgb(20, 33, 116);
const ICE: Color32 = Color32::from_rgb(213, 230, 247);
const GREEN: Color32 = Color32::from_rgb(103, 235, 88);
const RED: Color32 = Color32::from_rgb(241, 61, 54);
const GROUPS: [&str; 10] = [
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

struct RenderRequest {
    revision: u64,
    project: Project,
    library: Arc<Library>,
}

struct RenderResult {
    revision: u64,
    frames_per_tick: f64,
    pcm: Result<Arc<Pcm>, String>,
}

struct RenderWorker {
    sender: mpsc::Sender<RenderRequest>,
    receiver: mpsc::Receiver<RenderResult>,
    in_flight: bool,
    current_revision: Option<u64>,
    pending: Option<RenderRequest>,
}

impl RenderWorker {
    fn new() -> Self {
        let (sender, requests) = mpsc::channel::<RenderRequest>();
        let (results, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("mixjam-techno-render".into())
            .spawn(move || {
                while let Ok(request) = requests.recv() {
                    let pcm =
                        audio::render_project(&request.project, &request.library).map(Arc::new);
                    let frames_per_tick = pcm
                        .as_ref()
                        .map_or(1.0, |pcm| request.project.frames_per_tick(pcm.sample_rate));
                    if results
                        .send(RenderResult {
                            revision: request.revision,
                            frames_per_tick,
                            pcm,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("Cannot create the audio render worker.");
        Self {
            sender,
            receiver,
            in_flight: false,
            current_revision: None,
            pending: None,
        }
    }

    fn request(&mut self, request: RenderRequest) -> Result<(), String> {
        if self.current_revision == Some(request.revision) {
            return Ok(());
        }
        if self.in_flight {
            self.pending = Some(request);
        } else {
            self.current_revision = Some(request.revision);
            self.sender
                .send(request)
                .map_err(|_| "The audio render worker stopped.".to_string())?;
            self.in_flight = true;
        }
        Ok(())
    }

    fn poll(&mut self) -> Option<RenderResult> {
        let result = self.receiver.try_recv().ok()?;
        self.in_flight = false;
        self.current_revision = None;
        if let Some(request) = self.pending.take() {
            let revision = request.revision;
            if self.sender.send(request).is_ok() {
                self.in_flight = true;
                self.current_revision = Some(revision);
            }
        }
        Some(result)
    }

    fn cancel_pending(&mut self) {
        self.pending = None;
    }
}

/// Native sequencer client. All arrangement mutations pass through the editor.
pub struct MixJamApp {
    library: Arc<Library>,
    editor: Editor,
    player: Option<Player>,
    render: Option<Arc<Pcm>>,
    render_dirty: bool,
    render_worker: RenderWorker,
    revision: u64,
    requested_revision: Option<u64>,
    wants_playback: bool,
    playback_frames_per_tick: f64,
    seek_pending: bool,
    previewing: bool,
    cursor_tick: u32,
    selected_sample: Option<u32>,
    selected_clip: Option<u64>,
    selected_clips: BTreeSet<u64>,
    group: Option<String>,
    palette_product: Option<Product>,
    search: String,
    show_mixer: bool,
    show_help: bool,
    show_libraries: bool,
    show_import_details: bool,
    library_product: Product,
    rail_icons: Option<Result<Arc<RailIcons>, String>>,
    erase: bool,
    zoom: f32,
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
    file: Option<PathBuf>,
    status: String,
    status_error: bool,
    unsaved: bool,
    pending_action: Option<PendingAction>,
    allow_close: bool,
    selection_origin: Option<Pos2>,
    selection_base: BTreeSet<u64>,
    capture_path: Option<PathBuf>,
    capture_requested: bool,
    capture_pass: u32,
    capture_last_track: bool,
    show_effects: bool,
    show_step_synth: bool,
    show_sample_edit: bool,
    show_stretch: bool,
    effects: crate::effects::EffectsState,
    step_synth: crate::synth::StepSynthState,
    step_synth_bar: usize,
    step_synth_file: Option<PathBuf>,
    step_synth_effects_expanded: bool,
    edit_start: u64,
    edit_end: u64,
    edit_pitch: f64,
    stretch_source_bpm: f64,
    stretch_target_bpm: f64,
    armed_track: usize,
    record_track: usize,
    record_bpm: f64,
    recorder: Option<crate::recording::Recorder>,
    record_start_tick: u32,
    record_mix_started: bool,
    recorded_take: Option<Pcm>,
    import_path: Option<PathBuf>,
    import_bpm: f64,
}

impl MixJamApp {
    pub fn new(cc: &eframe::CreationContext<'_>, library: Arc<Library>, project: Project) -> Self {
        configure_style(&cc.egui_ctx);
        let (player, status, status_error) = match Player::new() {
            Ok(player) => (
                Some(player),
                "Choose a sound, then drag it onto a track. Double-click to listen.".into(),
                false,
            ),
            Err(error) => (None, format!("Audio output is unavailable: {error}"), true),
        };
        Self::from_parts(library, project, player, status, status_error)
    }

    pub fn set_startup_messages(&mut self, messages: &[String]) {
        if !messages.is_empty() {
            self.status = format!("{} {}", self.status, messages.join(" "));
            self.status_error = true;
        }
    }

    fn from_parts(
        library: Arc<Library>,
        project: Project,
        player: Option<Player>,
        status: String,
        status_error: bool,
    ) -> Self {
        let capture_path = std::env::var_os("\x54ECHNOREA_CAPTURE_TO").map(PathBuf::from);
        let capture_view = if capture_path.is_some() {
            std::env::var("\x54ECHNOREA_CAPTURE_VIEW").unwrap_or_default()
        } else {
            String::new()
        };
        let status = with_import_summary(status, &project);
        let needs_sample_folder = library.samples.is_empty();
        Self {
            library,
            editor: Editor::new(project),
            player,
            render: None,
            render_dirty: true,
            render_worker: RenderWorker::new(),
            revision: 0,
            requested_revision: None,
            wants_playback: false,
            playback_frames_per_tick: 1.0,
            seek_pending: false,
            previewing: false,
            cursor_tick: 0,
            selected_sample: None,
            selected_clip: None,
            selected_clips: BTreeSet::new(),
            group: None,
            palette_product: None,
            search: String::new(),
            show_mixer: capture_view == "mixer",
            show_help: false,
            show_libraries: capture_view == "libraries" || needs_sample_folder,
            show_import_details: false,
            library_product: Product::T1,
            rail_icons: None,
            erase: false,
            zoom: 1.0,
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
            file: None,
            status,
            status_error,
            unsaved: false,
            pending_action: None,
            allow_close: false,
            selection_origin: None,
            selection_base: BTreeSet::new(),
            capture_path,
            capture_requested: false,
            capture_pass: 0,
            capture_last_track: capture_view == "track64",
            show_effects: capture_view == "effects",
            show_step_synth: capture_view == "step-synth" || capture_view == "step-synth-effects",
            show_sample_edit: capture_view == "edit",
            show_stretch: capture_view == "stretch",
            effects: Default::default(),
            step_synth: if capture_view == "step-synth-effects" {
                crate::synth::StepSynthState {
                    effects: crate::effects::EffectsState {
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
            record_track: 0,
            record_bpm: 140.0,
            recorder: None,
            record_start_tick: 0,
            record_mix_started: false,
            recorded_take: None,
            import_path: None,
            import_bpm: 140.0,
        }
    }

    fn message(&mut self, text: impl Into<String>) {
        self.status = text.into();
        self.status_error = false;
    }

    fn error(&mut self, text: impl Into<String>) {
        self.status = text.into();
        self.status_error = true;
    }

    fn changed(&mut self) {
        self.unsaved = true;
        self.render_dirty = true;
        self.revision = self.revision.wrapping_add(1);
        if self.wants_playback {
            self.start_playback();
        }
    }

    fn playing_mix(&self) -> bool {
        !self.previewing && self.player.as_ref().is_some_and(Player::is_playing)
    }

    fn tick_to_frame(&self, tick: u32, rate: u32) -> u64 {
        (tick as f64 * self.editor.project.frames_per_tick(rate)).round() as u64
    }

    fn start_playback(&mut self) {
        if self.player.is_none() {
            self.error("Audio output is unavailable. You can still edit and export your mix.");
            return;
        }
        if self.editor.project.clips.is_empty() {
            self.pause();
            self.message("Add a sound to a track before you press Play.");
            return;
        }
        self.wants_playback = true;
        if self.render_dirty || self.render.is_none() {
            if self.requested_revision != Some(self.revision) {
                let request = RenderRequest {
                    revision: self.revision,
                    project: self.editor.project.clone(),
                    library: self.library.clone(),
                };
                match self.render_worker.request(request) {
                    Ok(()) => {
                        self.requested_revision = Some(self.revision);
                        self.message("Preparing mix audio. You can continue editing.");
                    }
                    Err(error) => {
                        self.wants_playback = false;
                        self.error(error);
                    }
                }
            }
            return;
        }
        self.apply_playback_buffer();
    }

    fn apply_playback_buffer(&mut self) {
        let pcm = self.render.as_ref().unwrap().clone();
        self.playback_frames_per_tick = self.editor.project.frames_per_tick(pcm.sample_rate);
        let start = self.tick_to_frame(self.cursor_tick, pcm.sample_rate);
        let region = self.editor.project.loop_region.map(|[a, b]| {
            [
                self.tick_to_frame(a, pcm.sample_rate),
                self.tick_to_frame(b, pcm.sample_rate),
            ]
        });
        match self.player.as_mut().unwrap().play(pcm, start, region) {
            Ok(()) => {
                self.previewing = false;
                self.seek_pending = false;
                self.message("Playing mix. Space pauses playback.");
            }
            Err(error) => {
                self.wants_playback = false;
                self.error(format!("Cannot start audio: {error}"));
            }
        }
    }

    fn poll_render(&mut self, ctx: &egui::Context) {
        if self.render_worker.in_flight {
            ctx.request_repaint_after(Duration::from_millis(20));
        }
        let Some(result) = self.render_worker.poll() else {
            return;
        };
        if result.revision != self.revision {
            return;
        }
        self.requested_revision = None;
        match result.pcm {
            Ok(pcm) => {
                if self.playing_mix() && !self.seek_pending {
                    self.cursor_tick = (self.player.as_ref().unwrap().position_frames() as f64
                        / self.playback_frames_per_tick)
                        as u32;
                }
                self.render = Some(pcm);
                self.render_dirty = false;
                if self.wants_playback {
                    self.playback_frames_per_tick = result.frames_per_tick;
                    self.apply_playback_buffer();
                }
            }
            Err(error) => {
                self.wants_playback = false;
                self.error(format!("Cannot render this mix: {error}"));
            }
        }
    }

    fn pause(&mut self) {
        self.wants_playback = false;
        self.render_worker.cancel_pending();
        self.requested_revision = None;
        if let Some(player) = &mut self.player {
            player.pause();
        }
        self.previewing = false;
        self.message("Paused.");
    }

    fn stop(&mut self) {
        self.wants_playback = false;
        self.render_worker.cancel_pending();
        self.requested_revision = None;
        if let Some(player) = &mut self.player {
            player.stop();
        }
        self.previewing = false;
        self.cursor_tick = 0;
        if let Some(recorder) = self.recorder.take() {
            match recorder.finish() {
                Ok(pcm) => {
                    self.recorded_take = Some(pcm);
                    self.save_recorded_take();
                }
                Err(error) => self.error(error),
            }
            return;
        }
        self.message("Stopped.");
    }

    fn seek(&mut self, tick: u32) {
        let was_playing = self.wants_playback;
        self.cursor_tick = tick;
        self.seek_pending = true;
        if was_playing {
            self.start_playback();
        }
    }

    fn preview(&mut self, sample_id: u32) {
        self.wants_playback = false;
        self.render_worker.cancel_pending();
        self.requested_revision = None;
        let Some(sample) = self.library.sample_by_id(sample_id) else {
            return;
        };
        let name = sample.name.clone();
        match self.library.read_pcm(sample_id) {
            Ok(pcm) => match self.player.as_mut() {
                Some(player) => match player.play(Arc::new(pcm), 0, None) {
                    Ok(()) => {
                        self.previewing = true;
                        self.message(format!("Listening: {name}"));
                    }
                    Err(error) => self.error(error),
                },
                None => self.error("Audio output is unavailable."),
            },
            Err(error) => self.error(format!("Cannot open {name}: {error}")),
        }
    }

    fn apply(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => self.changed(),
            Err(error) => self.error(error),
        }
    }

    fn place(&mut self, sample_id: u32, track: usize, tick: u32) {
        let Some(sample) = self.library.sample_by_id(sample_id) else {
            return;
        };
        let length = if Product::sample_key(sample_id).is_some() {
            sample.bars.saturating_mul(TICKS_PER_BAR)
        } else {
            (sample.frame_count as f64 * sample.source_bpm / sample.sample_rate as f64 / 240.0
                * TICKS_PER_BAR as f64)
                .round()
                .max(1.0) as u32
        };
        match self.editor.place(sample_id, track, tick, length) {
            Ok(id) => {
                self.selected_clip = Some(id);
                self.selected_clips.clear();
                self.selected_clips.insert(id);
                self.changed();
            }
            Err(error) => self.error(error),
        }
    }

    fn open(&mut self) {
        let root = self
            .library
            .wav_root()
            .or_else(|| self.library.source_root(Product::T2))
            .map(Path::to_path_buf)
            .or_else(|| {
                self.library
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
        match crate::project_io::open(path, &self.library) {
            Ok((project, library)) => {
                let status = with_import_summary(format!("Opened {}.", path.display()), &project);
                self.stop();
                self.library = Arc::new(library);
                self.editor = Editor::new(project);
                self.revision = self.revision.wrapping_add(1);
                self.render = None;
                self.render_dirty = true;
                self.selected_clip = None;
                self.selected_clips.clear();
                self.selected_sample = None;
                self.unsaved = false;
                self.file = if path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                {
                    Some(path.to_path_buf())
                } else {
                    None
                };
                self.message(status);
            }
            Err(error) => self.error(format!("Cannot open mix: {error}")),
        }
    }

    fn save(&mut self, choose_path: bool) {
        let path = if choose_path || self.file.is_none() {
            rfd::FileDialog::new()
                .add_filter("MixJam Techno project", &["json"])
                .set_file_name(format!("{}.json", self.editor.project.title))
                .save_file()
        } else {
            self.file.clone()
        };
        let Some(path) = path else { return };
        match crate::project_io::save(&self.editor.project, &path) {
            Ok(()) => {
                self.file = Some(path.clone());
                self.unsaved = false;
                self.message(format!("Saved {}.", path.display()));
            }
            Err(error) => self.error(format!("Cannot save mix: {error}")),
        }
    }

    fn export(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Stereo WAV", &["wav"])
            .set_file_name(format!("{}.wav", self.editor.project.title))
            .save_file()
        else {
            return;
        };
        match audio::render_project(&self.editor.project, &self.library)
            .and_then(|pcm| pcm.write_wav(&path))
        {
            Ok(()) => self.message(format!("Exported {}.", path.display())),
            Err(error) => self.error(format!("Cannot export mix: {error}")),
        }
    }

    fn new_mix(&mut self) {
        self.stop();
        self.editor = Editor::new(Project {
            libraries: self.library.bindings().to_vec(),
            imported_samples: self.library.imported_samples(),
            ..Project::default()
        });
        self.revision = self.revision.wrapping_add(1);
        self.render = None;
        self.file = None;
        self.selected_clip = None;
        self.selected_clips.clear();
        self.selected_sample = None;
        self.render_dirty = true;
        self.unsaved = false;
        self.pending_action = None;
        self.message("New mix. Choose a sound and drag it onto a track.");
    }

    fn commit_library(&mut self, library: Library) -> Result<(), String> {
        let bindings = library.bindings().to_vec();
        let mut checked = self.editor.project.clone();
        checked.libraries = bindings.clone();
        checked.validate()?;
        // Products identified inside the selected WAV folder are re-identified
        // from the saved sample folder at startup; persist only added libraries.
        let persisted: Vec<_> = bindings
            .iter()
            .filter(|binding| library.wav_root() != Some(binding.root.as_path()))
            .cloned()
            .collect();
        crate::project_io::save_library_config(&persisted)?;
        self.editor.project.libraries = bindings;
        self.library = Arc::new(library);
        if self
            .selected_sample
            .is_some_and(|id| self.library.sample_by_id(id).is_none())
        {
            self.selected_sample = None;
        }
        self.changed();
        Ok(())
    }

    fn add_library(&mut self, product: Product, root: &Path) {
        let mut library = (*self.library).clone();
        let result = library
            .register(product, root, &crate::project_io::reference_samples_root())
            .and_then(|()| self.commit_library(library));
        match result {
            Ok(()) => self.message(format!("Added {} samples.", product.label())),
            Err(error) => self.error(format!("Cannot add library: {error}")),
        }
    }

    fn select_wave_folder(&mut self, root: &Path) {
        if self.recorder.is_some() {
            self.error("Stop recording before you change the sample folder.");
            return;
        }
        let result = (|| {
            let library = Library::open_wav_folder(root, self.import_bpm)?;
            let (project, library) =
                crate::project_io::resolve(self.editor.project.clone(), &library)?;
            crate::project_io::save_wave_config(root, self.import_bpm)?;
            Ok::<_, String>((project, library))
        })();
        match result {
            Ok((project, library)) => {
                self.stop();
                self.library = Arc::new(library);
                self.editor = Editor::new(project);
                self.selected_sample = None;
                self.selected_clip = None;
                self.selected_clips.clear();
                self.changed();
                self.message("Sample folder loaded. Choose a sound or open a song.");
            }
            Err(error) => self.error(format!("Cannot use this sample folder: {error}")),
        }
    }

    fn remove_library(&mut self, product: Product) {
        if self.editor.project.clips.iter().any(|clip| {
            Product::sample_key(clip.sample_id).is_some_and(|(source, _)| source == product)
        }) {
            self.error(format!(
                "This mix uses {} samples. Start a new mix before you remove this library.",
                product.label()
            ));
            return;
        }
        let mut library = (*self.library).clone();
        let result = library
            .remove_product(product)
            .and_then(|()| self.commit_library(library));
        match result {
            Ok(()) => {
                if self.palette_product == Some(product) {
                    self.palette_product = None;
                }
                self.message(format!(
                    "Removed {} from the sample browser.",
                    product.label()
                ));
            }
            Err(error) => self.error(format!("Cannot remove library: {error}")),
        }
    }

    fn request_action(&mut self, action: PendingAction, ctx: &egui::Context) {
        if self.recorder.is_some() {
            self.error("Stop recording before you open, close, or start a new mix.");
            return;
        }
        if self.unsaved || self.recorded_take.is_some() {
            self.pending_action = Some(action);
        } else {
            self.run_action(action, ctx);
        }
    }

    fn run_action(&mut self, action: PendingAction, ctx: &egui::Context) {
        self.pending_action = None;
        self.recorded_take = None;
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
        if self.recorder.is_some() {
            self.error("Stop recording before you undo or redo.");
            return;
        }
        let changed = if redo {
            self.editor.redo()
        } else {
            self.editor.undo()
        };
        if !changed {
            return;
        }
        let mut library = (*self.library).clone();
        let result = library
            .apply_imports(&self.editor.project.imported_samples)
            .and_then(|()| {
                if let Some(clip) = self.editor.project.clips.iter()
                    .find(|clip| library.sample_by_id(clip.sample_id).is_none())
                {
                    return Err(format!(
                        "Cannot restore this edit because sample {} is unavailable. Add its sample library and try again.",
                        clip.sample_id
                    ));
                }
                Ok(())
            });
        match result {
            Ok(()) => {
                self.editor.project.libraries = library.bindings().to_vec();
                self.library = Arc::new(library);
                self.selected_clips
                    .retain(|id| self.editor.project.clips.iter().any(|c| c.id == *id));
                self.selected_clip = self.selected_clips.iter().next().copied();
                if self
                    .selected_sample
                    .is_some_and(|id| self.library.sample_by_id(id).is_none())
                {
                    self.selected_sample = None;
                }
                self.changed();
            }
            Err(error) => {
                if redo {
                    self.editor.undo();
                } else {
                    self.editor.redo();
                }
                self.error(error);
            }
        }
    }

    fn select_clip(&mut self, id: u64, additive: bool) {
        if let Some(clip) = self.editor.project.clips.iter().find(|c| c.id == id) {
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
        let ids = self.selected_clips.clone();
        if ids.is_empty() {
            return;
        }
        let result = self
            .editor
            .transact(|p| p.clips.retain(|c| !ids.contains(&c.id)));
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
            if self.wants_playback {
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

    fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .exact_height(58.0)
            .frame(
                egui::Frame::new()
                    .fill(COBALT)
                    .inner_margin(egui::Margin::symmetric(18, 8)),
            )
            .show(ctx, |ui| {
                metallic(
                    ui.painter(),
                    ui.max_rect(),
                    Color32::from_rgb(31, 52, 140),
                    Color32::from_rgb(8, 13, 64),
                );
                ui.horizontal_centered(|ui| {
                    ui.label(
                        egui::RichText::new("MixJam")
                            .size(31.0)
                            .strong()
                            .italics()
                            .color(Color32::from_rgb(247, 207, 79)),
                    );
                    ui.add_space(9.0);
                    ui.label(egui::RichText::new("Techno").size(20.0).strong().color(ICE));
                    ui.add_space(22.0);
                    let title = if self.unsaved {
                        format!("{} *", self.editor.project.title)
                    } else {
                        self.editor.project.title.clone()
                    };
                    ui.label(egui::RichText::new(title).size(15.0).color(Color32::WHITE));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if metal_button(ui, "Help", false).clicked() {
                            self.show_help = !self.show_help;
                        }
                        if metal_button(ui, "Redo", false).clicked() {
                            self.history_step(true);
                        }
                        if metal_button(ui, "Undo", false).clicked() {
                            self.history_step(false);
                        }
                        ui.label(
                            egui::RichText::new(format!("{} sounds", self.library.samples.len()))
                                .color(ICE),
                        );
                    });
                });
            });
    }

    fn side_rail(&mut self, ctx: &egui::Context) {
        let icons = self
            .rail_icons
            .get_or_insert_with(|| RailIcons::load(ctx).map(Arc::new))
            .clone();
        let icons = match icons {
            Ok(icons) => icons,
            Err(error) => {
                self.error(error);
                return;
            }
        };
        egui::SidePanel::right("tools")
            .exact_width(76.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(COBALT).inner_margin(8))
            .show(ctx, |ui| {
                metallic(
                    ui.painter(),
                    ui.max_rect(),
                    Color32::from_rgb(169, 195, 223),
                    Color32::from_rgb(69, 99, 156),
                );
                ui.add_space(9.0);
                ui.vertical_centered(|ui| {
                    rail_heading(ui, &icons, RailIcon::Mix);
                    ui.add_space(9.0);
                    if rail_button(ui, &icons, RailIcon::Open, false).clicked() {
                        self.request_action(PendingAction::Open, ctx);
                    }
                    if rail_button(ui, &icons, RailIcon::Save, false).clicked() {
                        self.save(false);
                    }
                    if rail_button(ui, &icons, RailIcon::SaveAs, false).clicked() {
                        self.save(true);
                    }
                    if rail_button(ui, &icons, RailIcon::Export, false).clicked() {
                        self.export();
                    }
                    if rail_button(ui, &icons, RailIcon::Import, false).clicked() {
                        self.import_wav();
                    }
                    if rail_button(ui, &icons, RailIcon::Libraries, false).clicked() {
                        self.show_libraries = true;
                    }
                    ui.add_space(18.0);
                    if rail_button(ui, &icons, RailIcon::New, false).clicked() {
                        self.request_action(PendingAction::New, ctx);
                    }
                    ui.add_space(18.0);
                    if rail_button(ui, &icons, RailIcon::Mixer, self.show_mixer).clicked() {
                        self.show_mixer = !self.show_mixer;
                    }
                    if rail_button(ui, &icons, RailIcon::Erase, self.erase).clicked() {
                        self.erase = !self.erase;
                    }
                    if rail_button(ui, &icons, RailIcon::Delete, false).clicked() {
                        self.delete_selection();
                    }
                    ui.add_space(12.0);
                    if rail_button(ui, &icons, RailIcon::Effects, false).clicked() {
                        self.show_effects = true;
                    }
                    if rail_button(ui, &icons, RailIcon::StepSynth, false).clicked() {
                        self.show_step_synth = true;
                    }
                    if rail_button(ui, &icons, RailIcon::Edit, false).clicked() {
                        self.show_sample_edit = true;
                        self.edit_start = 0;
                        self.edit_end = 0;
                    }
                    if rail_button(ui, &icons, RailIcon::Stretch, false).clicked() {
                        self.show_stretch = true;
                    }
                    ui.add_space(12.0);
                    rail_heading(ui, &icons, RailIcon::Record);
                    let occupied = occupied_tracks(&self.editor.project);
                    recording_destination(ui, &mut self.armed_track, &occupied);
                });
            });
    }

    fn bottom(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(28.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(5, 8, 35))
                    .inner_margin(egui::Margin::symmetric(12, 5)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if (!self.editor.project.import_notices.is_empty()
                        || !self.editor.project.source_notices.is_empty())
                        && ui.small_button("Import details").clicked()
                    {
                        self.show_import_details = true;
                    }
                    ui.add(
                        egui::Label::new(egui::RichText::new(&self.status).size(12.0).color(
                            if self.status_error {
                                Color32::from_rgb(255, 176, 159)
                            } else {
                                ICE
                            },
                        ))
                        .truncate(),
                    )
                    .on_hover_text(&self.status);
                });
            });
        let height = (ctx.content_rect().height() * 0.27).clamp(180.0, 290.0);
        egui::TopBottomPanel::bottom("sounds")
            .exact_height(height)
            .frame(egui::Frame::new().fill(COBALT).inner_margin(10))
            .show(ctx, |ui| {
                if self.show_mixer {
                    self.mixer(ui);
                } else {
                    self.palette(ui);
                }
            });
        egui::TopBottomPanel::bottom("transport")
            .exact_height(70.0)
            .frame(
                egui::Frame::new()
                    .fill(COBALT)
                    .inner_margin(egui::Margin::symmetric(16, 9)),
            )
            .show(ctx, |ui| self.transport(ui));
    }

    fn transport(&mut self, ui: &mut egui::Ui) {
        let bars = self.cursor_tick / TICKS_PER_BAR + 1;
        let beat = (self.cursor_tick % TICKS_PER_BAR) / (TICKS_PER_BAR / 4) + 1;
        ui.horizontal_centered(|ui| {
            egui::Frame::new()
                .fill(NAVY)
                .inner_margin(egui::Margin::symmetric(13, 10))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("{bars:03} : {beat}"))
                            .size(21.0)
                            .strong()
                            .color(ICE),
                    );
                });
            ui.add_space(13.0);
            if transport_button(
                ui,
                TransportIcon::Loop,
                self.editor.project.loop_region.is_some(),
                "Loop selected bars",
            )
            .clicked()
            {
                let end = self.editor.project.end_tick().max(TICKS_PER_BAR);
                let result = self.editor.transact(|p| {
                    p.loop_region = if p.loop_region.is_some() {
                        None
                    } else {
                        Some([0, end])
                    }
                });
                self.apply(result);
            }
            if transport_button(ui, TransportIcon::Start, false, "Go to start").clicked() {
                self.seek(0);
            }
            if transport_button(ui, TransportIcon::Back, false, "Back one bar").clicked() {
                self.seek(self.cursor_tick.saturating_sub(TICKS_PER_BAR));
            }
            if transport_button(ui, TransportIcon::Stop, false, "Stop").clicked() {
                self.stop();
            }
            if transport_button(
                ui,
                if self.wants_playback {
                    TransportIcon::Pause
                } else {
                    TransportIcon::Play
                },
                self.wants_playback,
                "Play / pause (Space)",
            )
            .clicked()
            {
                if self.wants_playback {
                    self.pause();
                } else {
                    self.start_playback();
                }
            }
            if transport_button(ui, TransportIcon::Forward, false, "Forward one bar").clicked() {
                self.seek(self.cursor_tick.saturating_add(TICKS_PER_BAR));
            }
            if transport_button(ui, TransportIcon::End, false, "Go to end").clicked() {
                self.seek(self.editor.project.end_tick());
            }
            if transport_button(
                ui,
                TransportIcon::Record,
                self.recorder.is_some(),
                "Record into the armed track",
            )
            .clicked()
            {
                if self.recorder.is_some() {
                    self.stop();
                } else {
                    self.start_recording();
                }
            }
            ui.add_space(18.0);
            ui.label("BPM");
            let mut bpm = self.editor.project.bpm;
            if bpm_picker(ui, "project_bpm", &mut bpm).changed() {
                let result = self.editor.transact(|p| p.bpm = bpm);
                self.apply(result);
            }
            ui.add_space(10.0);
            ui.label("Zoom");
            ui.add(
                egui::Slider::new(&mut self.zoom, 0.55..=2.0)
                    .show_value(false)
                    .clamping(egui::SliderClamping::Always),
            );
        });
    }

    fn sample_product(&self, id: u32) -> Option<Product> {
        Product::sample_key(id)
            .map(|(product, _)| product)
            .or_else(|| {
                self.editor
                    .project
                    .imported_samples
                    .iter()
                    .find(|sample| sample.id == id)
                    .and_then(|sample| sample.origin)
            })
    }

    fn palette_indices(&self) -> Vec<usize> {
        let query = self.search.to_lowercase();
        self.library
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

    fn palette(&mut self, ui: &mut egui::Ui) {
        let samples = self.palette_indices();
        let group_names: BTreeSet<&str> = self
            .library
            .samples
            .iter()
            .map(|sample| sample.group_name.as_str())
            .collect();
        let groups: BTreeMap<String, &str> = group_names
            .into_iter()
            .map(|name| (name.to_lowercase(), name))
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("SOUND GROUPS").strong().color(ICE));
            egui::ComboBox::from_id_salt("palette_product")
                .selected_text(self.palette_product.map_or("All libraries", Product::label))
                .width(160.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.palette_product, None, "All libraries");
                    for product in Product::ALL {
                        ui.selectable_value(
                            &mut self.palette_product,
                            Some(product),
                            product.label(),
                        );
                    }
                });
            let group_label = self.group.as_deref().map_or("All sounds", |group| {
                if group.is_empty() {
                    "Uncategorized"
                } else {
                    group
                }
            });
            egui::ComboBox::from_id_salt("palette_category")
                .selected_text(group_label)
                .width(110.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.group, None, "All sounds");
                    for name in groups.values() {
                        ui.selectable_value(
                            &mut self.group,
                            Some((*name).to_owned()),
                            if name.is_empty() {
                                "Uncategorized"
                            } else {
                                name
                            },
                        );
                    }
                });
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Find a sound")
                    .desired_width(180.0),
            );
            ui.label(format!("{} sounds", samples.len()));
        });
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(92.0);
                for name in &GROUPS[..5] {
                    if category_button(ui, name, self.group.as_deref() == Some(*name)).clicked() {
                        self.group = Some((*name).into());
                    }
                }
            });
            let right_width = 98.0;
            let palette_width = (ui.available_width() - right_width - 12.0).max(100.0);
            egui::Frame::new()
                .fill(NAVY)
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.set_width(palette_width);
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let count = (palette_width / 126.0).floor().max(1.0) as usize;
                    if samples.is_empty() {
                        ui.add_space(22.0);
                        ui.label("No sounds match this library, category, and search.");
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("sample_palette")
                        .max_height(ui.available_height())
                        .show_rows(ui, 39.0, samples.len().div_ceil(count), |ui, rows| {
                            egui::Grid::new("sound_grid")
                                .spacing(Vec2::new(6.0, 6.0))
                                .show(ui, |ui| {
                                    for row in rows {
                                        for &sample_index in &samples
                                            [row * count..((row + 1) * count).min(samples.len())]
                                        {
                                            let sample = self.library.samples[sample_index].clone();
                                            let source = self
                                                .sample_product(sample.id)
                                                .map_or("User WAV", Product::label);
                                            let response = ui
                                                .push_id(sample.id, |ui| {
                                                    sample_button(
                                                        ui,
                                                        &sample,
                                                        self.selected_sample == Some(sample.id),
                                                    )
                                                })
                                                .inner
                                                .on_hover_text(format!(
                                                    "{} / {}",
                                                    source, sample.group_name
                                                ));
                                            if response.clicked() {
                                                self.selected_sample = Some(sample.id);
                                            }
                                            if response.double_clicked() {
                                                self.preview(sample.id);
                                            }
                                            if response.drag_started() {
                                                self.selected_sample = Some(sample.id);
                                                self.drag = Some(DragItem::Sample(sample.id));
                                            }
                                        }
                                        ui.end_row();
                                    }
                                });
                        });
                });
            ui.vertical(|ui| {
                ui.set_width(92.0);
                for name in &GROUPS[5..] {
                    if category_button(ui, name, self.group.as_deref() == Some(*name)).clicked() {
                        self.group = Some((*name).into());
                    }
                }
            });
        });
    }

    fn mixer(&mut self, ui: &mut egui::Ui) {
        metallic(
            ui.painter(),
            ui.max_rect(),
            Color32::from_rgb(192, 215, 237),
            Color32::from_rgb(105, 137, 179),
        );
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Track Mixer")
                    .size(20.0)
                    .strong()
                    .italics()
                    .color(NAVY),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if metal_button(ui, "Sounds", false).clicked() {
                    self.show_mixer = false;
                }
            });
        });
        let mut tracks = self.editor.project.tracks.clone();
        let occupied = occupied_tracks(&self.editor.project);
        let mut gain = self.editor.project.master_gain;
        let mut change = false;
        let fader_height = (ui.available_height() - 86.0).max(52.0);
        ui.scope(|ui| {
            ui.visuals_mut().override_text_color = Some(NAVY);
            ui.visuals_mut().selection.bg_fill = RED;
            egui::ScrollArea::horizontal()
                .id_salt("mixer_scroll")
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        for &index in &occupied {
                            let track = &mut tracks[index];
                            ui.allocate_ui_with_layout(
                                Vec2::new(64.0, fader_height + 84.0),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().slider_width = fader_height;
                                    ui.horizontal(|ui| {
                                        change |= route_button(ui, "L", &mut track.left_enabled)
                                            .changed();
                                        change |= route_button(ui, "R", &mut track.right_enabled)
                                            .changed();
                                    });
                                    change |= ui
                                        .add_sized(
                                            [25.0, fader_height],
                                            egui::Slider::new(&mut track.gain, -2.0..=4.0)
                                                .vertical()
                                                .show_value(false),
                                        )
                                        .changed();
                                    ui.label(
                                        egui::RichText::new(format!("{}", index + 1))
                                            .strong()
                                            .color(NAVY),
                                    );
                                    ui.spacing_mut().slider_width = 52.0;
                                    change |= pan_control(ui, index, &mut track.pan).changed();
                                    ui.small("L  Pan  R");
                                },
                            );
                        }
                        ui.add_space(18.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(66.0, fader_height + 48.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.spacing_mut().slider_width = fader_height;
                                ui.label("Master");
                                change |= ui
                                    .add_sized(
                                        [25.0, fader_height],
                                        egui::Slider::new(&mut gain, 0.0..=2.0)
                                            .vertical()
                                            .show_value(false),
                                    )
                                    .changed();
                                ui.label(format!("{:.0}%", gain * 100.0));
                            },
                        );
                    });
                });
        });
        if change {
            let result = self.editor.transact(|p| {
                p.tracks = tracks;
                p.master_gain = gain;
            });
            self.apply(result);
        }
    }

    fn selected_inspector(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected_clip else { return };
        let Some(clip) = self
            .editor
            .project
            .clips
            .iter()
            .find(|c| c.id == id)
            .cloned()
        else {
            self.selected_clip = None;
            return;
        };
        let Some(sample) = self.library.sample_by_id(clip.sample_id) else {
            return;
        };
        let name = sample.name.clone();
        let max_frames = sample.frame_count;
        let mut gain = clip.gain;
        let mut offset = clip.offset_frames;
        let mut length = clip.length_ticks;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(name)
                    .color(Color32::from_rgb(255, 230, 161))
                    .strong(),
            );
            ui.add_space(12.0);
            ui.label("Gain");
            let mut change = ui
                .add(egui::DragValue::new(&mut gain).range(0.0..=2.0).speed(0.01))
                .changed();
            ui.label("Trim start (frames)");
            change |= ui
                .add(
                    egui::DragValue::new(&mut offset)
                        .range(0..=max_frames.saturating_sub(1))
                        .speed(100),
                )
                .changed();
            ui.label("Length (bars)");
            let mut bars = length as f64 / TICKS_PER_BAR as f64;
            if ui
                .add(
                    egui::DragValue::new(&mut bars)
                        .range(1.0 / 16.0..=256.0)
                        .speed(1.0 / 16.0),
                )
                .changed()
            {
                length = (bars * TICKS_PER_BAR as f64).round().max(1.0) as u32;
                change = true;
            }
            if change {
                let result = self.editor.transact(|p| {
                    if let Some(c) = p.clips.iter_mut().find(|c| c.id == id) {
                        c.gain = gain;
                        c.offset_frames = offset;
                        c.length_ticks = length;
                    }
                });
                self.apply(result);
            }
        });
    }

    fn timeline(&mut self, ui: &mut egui::Ui) {
        self.timeline_rect = None;
        self.timeline_drop_rect = None;
        self.timeline_ruler_rect = None;
        let occupied = occupied_tracks(&self.editor.project);
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
        let row_height =
            ((ui.available_height() - 69.0) / row_count.clamp(1, 16) as f32).clamp(24.0, 43.0);
        let bar_width = 62.0 * self.zoom;
        self.timeline_row_height = row_height;
        self.timeline_bar_width = bar_width;
        let track_height = row_height * row_count as f32;
        let bars = (self.editor.project.end_tick() / TICKS_PER_BAR + 16).max(64);
        let width = bars as f32 * bar_width;
        let label_width = 74.0;
        let lane_inset = label_width + ui.spacing().item_spacing.x;
        let inspector_height = if self.selected_clip.is_some() {
            34.0
        } else {
            0.0
        };
        let viewport_height = (ui.available_height() - 26.0 - inspector_height - 8.0).max(80.0);
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
                    let response =
                        ui.interact(rect, ui.id().with("canvas"), Sense::click_and_drag());
                    self.timeline_rect = Some(rect);
                    if response.hovered()
                        && let Some(pos) = ui.input(|i| i.pointer.latest_pos())
                        && let Some(clip) = self
                            .editor
                            .project
                            .clips
                            .iter()
                            .rev()
                            .find(|clip| self.clip_rect(clip, rect).contains(pos))
                        && let Some(sample) = self.library.sample_by_id(clip.sample_id)
                    {
                        response.clone().on_hover_text(&sample.name);
                    }
                    self.timeline_drop_rect = Some(rect.intersect(ui.clip_rect()));
                    let painter = ui.painter_at(rect).with_clip_rect(canvas_clip);
                    painter.rect_filled(rect, 0.0, NAVY);
                    for track in 0..occupied_count {
                        let row = Rect::from_min_size(
                            Pos2::new(rect.left(), rect.top() + track as f32 * row_height),
                            Vec2::new(width, row_height),
                        );
                        metallic(
                            &painter,
                            row.shrink2(Vec2::new(0.0, 1.0)),
                            Color32::from_rgb(13, 24, 67),
                            Color32::from_rgb(4, 6, 33),
                        );
                        painter.line_segment(
                            [row.left_bottom(), row.right_bottom()],
                            Stroke::new(1.0_f32, Color32::from_rgb(83, 106, 169)),
                        );
                    }
                    if let Some(track) = add_track {
                        let target = Rect::from_min_size(
                            Pos2::new(rect.left(), rect.top() + occupied_count as f32 * row_height),
                            Vec2::new(width, row_height),
                        )
                        .intersect(canvas_clip);
                        painter.text(
                            target.center(),
                            Align2::CENTER_CENTER,
                            format!("Drop a sound to add track {}", track + 1),
                            FontId::proportional(13.0),
                            ICE,
                        );
                    }
                    let visible = ui.clip_rect().intersect(rect);
                    painter.text(
                        visible.center(),
                        Align2::CENTER_CENTER,
                        "MixJam",
                        FontId::proportional(150.0),
                        Color32::from_rgba_unmultiplied(72, 83, 160, 30),
                    );
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
                            Stroke::new(0.5_f32, Color32::from_rgb(22, 33, 78)),
                        );
                    }
                    for clip in &self.editor.project.clips {
                        let clip_rect = self.clip_rect(clip, rect);
                        if visible.intersects(clip_rect) {
                            let sample = self.library.sample_by_id(clip.sample_id);
                            draw_clip(
                                &painter,
                                clip_rect,
                                sample,
                                self.selected_clips.contains(&clip.id),
                                clip.gain,
                            );
                        }
                    }
                    let play_x =
                        rect.left() + self.cursor_tick as f32 / TICKS_PER_BAR as f32 * bar_width;
                    painter.line_segment(
                        [
                            Pos2::new(play_x, rect.top()),
                            Pos2::new(play_x, rect.bottom()),
                        ],
                        Stroke::new(1.7_f32, RED),
                    );
                    if (response.clicked() || response.double_clicked())
                        && let Some(pos) = response.interact_pointer_pos()
                    {
                        let hit = self
                            .editor
                            .project
                            .clips
                            .iter()
                            .rev()
                            .find(|c| self.clip_rect(c, rect).contains(pos))
                            .cloned();
                        if let Some(clip) = hit {
                            if self.erase {
                                let result = self.editor.delete(clip.id);
                                self.apply(result);
                            } else {
                                self.select_clip(clip.id, ui.input(|i| i.modifiers.shift));
                                if response.double_clicked() {
                                    self.preview(clip.sample_id);
                                }
                            }
                        } else if let Some(sample) = self
                            .selected_sample
                            .filter(|_| !ui.input(|i| i.modifiers.shift))
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
                    if response.drag_started()
                        && let Some(pos) = ui.input(|i| i.pointer.press_origin())
                    {
                        if let Some(clip) = self
                            .editor
                            .project
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
                                copy: ui.input(|i| i.modifiers.alt || i.pointer.secondary_down()),
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
                            painter.rect_filled(
                                selection,
                                0.0,
                                Color32::from_rgba_unmultiplied(123, 170, 246, 35),
                            );
                            outline(&painter, selection, ICE, 1.0);
                            self.selected_clips = self.selection_base.clone();
                            let hits: Vec<_> = self
                                .editor
                                .project
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
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(74.0, row_height), Sense::hover());
                        if add_track == Some(index) {
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                format!("Add {}", index + 1),
                                FontId::proportional(13.0),
                                ICE,
                            );
                            let response = ui.interact(
                                rect,
                                ui.id().with(("add_track", index)),
                                Sense::click(),
                            );
                            if response.clicked() {
                                self.armed_track = index;
                                self.message(format!("Recording will create track {}.", index + 1));
                            }
                            response.on_hover_text(
                                "Drop a sound on this row or select it for recording.",
                            );
                            continue;
                        }
                        metallic(
                            ui.painter(),
                            rect.shrink(1.0),
                            Color32::from_rgb(159, 193, 222),
                            Color32::from_rgb(51, 83, 154),
                        );
                        ui.painter().rect_filled(
                            Rect::from_center_size(
                                rect.center(),
                                Vec2::new(25.0, row_height - 3.0),
                            ),
                            0.0,
                            Color32::from_rgb(28, 55, 115),
                        );
                        ui.painter().text(
                            rect.center() + Vec2::new(0.0, -1.0),
                            Align2::CENTER_CENTER,
                            format!("{}", index + 1),
                            FontId::proportional(17.0),
                            Color32::WHITE,
                        );
                        let arm = ui.interact(
                            Rect::from_center_size(rect.center(), Vec2::new(28.0, row_height)),
                            ui.id().with(("arm", index)),
                            Sense::click(),
                        );
                        if self.armed_track == index {
                            ui.painter().circle_filled(
                                rect.center() + Vec2::new(0.0, 9.0),
                                2.0,
                                RED,
                            );
                        }
                        if arm.clicked() {
                            self.armed_track = index;
                            self.message(format!("Track {} is ready for recording.", index + 1));
                        }
                        arm.on_hover_text("Select this track for recording");
                        let track = &self.editor.project.tracks[index];
                        for (left, enabled) in
                            [(true, track.left_enabled), (false, track.right_enabled)]
                        {
                            let x = if left {
                                rect.left() + 11.0
                            } else {
                                rect.right() - 11.0
                            };
                            let button = Rect::from_center_size(
                                Pos2::new(x, rect.center().y),
                                Vec2::new(19.0, row_height - 3.0),
                            );
                            let response = ui.interact(
                                button,
                                ui.id().with(("route", index, left)),
                                Sense::click(),
                            );
                            speaker(ui.painter(), button, enabled, left);
                            if response.clicked() {
                                route_change = Some((index, left, !enabled));
                            }
                            response.on_hover_text(if left {
                                "Left output"
                            } else {
                                "Right output"
                            });
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
        let ruler_response = ui.interact(
            ruler_clip,
            ui.id().with("bar_ruler"),
            Sense::click_and_drag(),
        );
        let ruler_painter = ui.painter().with_clip_rect(ruler_clip);
        metallic(
            &ruler_painter,
            ruler,
            Color32::from_rgb(35, 55, 135),
            Color32::from_rgb(7, 12, 54),
        );
        if let Some([start, end]) = self.editor.project.loop_region {
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
            ruler_painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(54, 175, 72, 100));
        }
        let first_bar = ((ruler_clip.left() - ruler.left()) / bar_width)
            .floor()
            .max(0.0) as u32;
        let last_bar = ((ruler_clip.right() - ruler.left()) / bar_width)
            .ceil()
            .min(bars as f32) as u32;
        for bar in first_bar..last_bar {
            let x = ruler.left() + bar as f32 * bar_width;
            ruler_painter.text(
                Pos2::new(x + bar_width * 0.5, ruler.center().y),
                Align2::CENTER_CENTER,
                format!("{}", bar + 1),
                FontId::proportional(13.0),
                if bar == self.cursor_tick / TICKS_PER_BAR {
                    RED
                } else {
                    ICE
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
            let result = self.editor.transact(|p| p.loop_region = Some(region));
            self.apply(result);
        }
        if let Some((index, left, enabled)) = route_change {
            let result = self.editor.transact(|p| {
                if left {
                    p.tracks[index].left_enabled = enabled;
                } else {
                    p.tracks[index].right_enabled = enabled;
                }
            });
            self.apply(result);
        }
        ui.add_space(3.0);
        self.selected_inspector(ui);
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
                    .library
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
                NAVY,
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
                    if let Some(anchor) = self.editor.project.clips.iter().find(|c| c.id == id) {
                        let ids: Vec<_> = self.selected_clips.iter().copied().collect();
                        let track_delta = track as i32 - anchor.track as i32;
                        let tick_delta = i64::from(tick) - i64::from(anchor.start_tick);
                        match self.editor.move_clips(&ids, track_delta, tick_delta, copy) {
                            Ok(ids) => {
                                self.selected_clip = ids.first().copied();
                                self.selected_clips = ids.into_iter().collect();
                                self.changed();
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
                ui.heading("Save your changes?");
                ui.label("The current mix or recorded take has unsaved changes.");
                ui.horizontal(|ui| {
                    if ui.button("Save changes").clicked() {
                        if self.recorded_take.is_some() {
                            self.save_recorded_take();
                        }
                        if self.recorded_take.is_none() && self.unsaved {
                            self.save(false);
                        }
                        if !self.unsaved && self.recorded_take.is_none() {
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
        egui::Window::new("Using MixJam Techno").open(&mut self.show_help).resizable(false).default_width(450.0).show(ctx, |ui| {
            ui.label("Drag a sound from the palette onto one of up to 64 tracks. You can also select a sound and click an empty part of a track.");
            ui.add_space(8.0);
            for text in [
                "Double-click a sound or clip to listen to it.",
                "Drag a clip to move it. Hold Alt while dragging to copy.",
                "Hold Shift to select several clips. Drag on an empty track area to select a group.",
                "Hold Ctrl for placement at sixteenth-note intervals.",
                "Select a clip and press Delete to remove it.",
                "Click a bar number to seek. Drag across bar numbers to set a loop.",
                "L and R control each track's left and right output. Turn both off to mute.",
                "Mixer opens track volume, pan, routing, and master volume controls.",
                "Space plays or pauses. Ctrl+Z undoes. Ctrl+Shift+Z redoes.",
                "Open accepts supported original song files and saved JSON mixes. Save writes JSON; Export writes stereo WAV.",
            ] { ui.label(text); ui.add_space(4.0); }
        });
    }

    fn capture(&mut self, ctx: &egui::Context) {
        let Some(path) = self.capture_path.clone() else {
            return;
        };
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
                self.editor
                    .project
                    .clips
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.sample_id)
            })
        })
    }

    fn source_name(&self) -> String {
        self.source_id()
            .and_then(|id| self.library.sample_by_id(id))
            .map_or_else(
                || "Choose a sound in the palette.".into(),
                |s| s.name.clone(),
            )
    }

    fn source_pcm(&self) -> Result<Pcm, String> {
        self.library.read_pcm(
            self.source_id()
                .ok_or("Choose a sound in the palette first.")?,
        )
    }

    fn source_bpm(&self) -> f64 {
        self.source_id()
            .and_then(|id| self.library.sample_by_id(id))
            .map_or(140.0, |s| s.source_bpm)
    }

    fn play_pcm(&mut self, pcm: Pcm, label: &str) {
        self.wants_playback = false;
        self.render_worker.cancel_pending();
        self.requested_revision = None;
        let result = self
            .player
            .as_mut()
            .ok_or("Audio output is unavailable.".to_string())
            .and_then(|p| p.play(Arc::new(pcm), 0, None));
        match result {
            Ok(()) => {
                self.previewing = true;
                self.message(label);
            }
            Err(error) => self.error(error),
        }
    }

    fn register_file(&mut self, path: &Path, group: &str, source_bpm: f64) -> Result<u32, String> {
        let mut library = (*self.library).clone();
        let imported = library.import_wav_with_tempo(path, None, group, source_bpm)?;
        let id = imported.id;
        self.editor.transact(|p| {
            if let Some(existing) = p.imported_samples.iter_mut().find(|s| s.id == id) {
                *existing = imported;
            } else {
                p.imported_samples.push(imported);
            }
        })?;
        self.library = Arc::new(library);
        self.group = Some(group.into());
        self.palette_product = None;
        self.selected_sample = Some(id);
        self.changed();
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
        pcm.write_wav(&path)?;
        let id = self.register_file(&path, group, source_bpm)?;
        self.message(format!("Saved {} and added it to {group}.", path.display()));
        Ok(Some(id))
    }

    fn start_recording(&mut self) {
        if self.recorded_take.is_some() {
            self.error("Save or discard the previous recorded take first.");
            return;
        }
        self.record_start_tick = self.cursor_tick;
        self.record_track = self.armed_track;
        self.record_bpm = self.editor.project.bpm;
        self.record_mix_started = false;
        match crate::recording::Recorder::start(self.record_bpm) {
            Ok(recorder) => {
                self.recorder = Some(recorder);
                match crate::recording::count_in(self.record_bpm) {
                    Ok(pcm) => self.play_pcm(
                        pcm,
                        "Recording count-in: eight beats. Press Stop to finish.",
                    ),
                    Err(error) => self.error(error),
                }
            }
            Err(error) => self.error(error),
        }
    }

    fn poll_recording(&mut self, ctx: &egui::Context) {
        let Some(recorder) = &self.recorder else {
            return;
        };
        let status = recorder.status();
        ctx.request_repaint_after(Duration::from_millis(40));
        if let Some(error) = status.error {
            self.error(error);
            return;
        }
        if status.limit_reached {
            self.stop();
            return;
        }
        if status.count_in_frames_remaining > 0 {
            self.message(format!(
                "Count-in: {:.1} seconds. Recording track {}.",
                status.count_in_frames_remaining as f64 / status.sample_rate as f64,
                self.record_track + 1
            ));
        } else {
            if !self.record_mix_started {
                self.record_mix_started = true;
                self.cursor_tick = self.record_start_tick;
                if !self.editor.project.clips.is_empty() {
                    self.start_playback();
                }
            }
            self.message(format!(
                "Recording track {}: {:.1} seconds. Press Stop to save the take.",
                self.record_track + 1,
                status.captured_frames as f64 / status.sample_rate as f64
            ));
        }
    }

    fn save_recorded_take(&mut self) {
        let Some(pcm) = self.recorded_take.clone() else {
            return;
        };
        match self.save_pcm(&pcm, "Recording", "Wave", self.record_bpm) {
            Ok(Some(id)) => {
                let ticks = (pcm.frames() as f64 * self.record_bpm / pcm.sample_rate as f64 / 240.0
                    * TICKS_PER_BAR as f64)
                    .round()
                    .max(1.0) as u32;
                match self
                    .editor
                    .place(id, self.record_track, self.record_start_tick, ticks)
                {
                    Ok(clip_id) => {
                        self.select_clip(clip_id, false);
                        self.changed();
                        self.recorded_take = None;
                    }
                    Err(error) => {
                        self.recorded_take = None;
                        self.error(format!(
                            "The take is saved in Wave. Cannot place it on the armed track: {error}"
                        ));
                    }
                }
            }
            Ok(None) => self
                .message("The recorded take is kept in memory. Save it before you close the app."),
            Err(error) => self.error(error),
        }
    }

    fn tool_windows(&mut self, ctx: &egui::Context) {
        if self.show_libraries {
            let mut open = true;
            egui::Window::new("Sample libraries")
                .open(&mut open)
                .default_width(540.0)
                .show(ctx, |ui| {
                    ui.label("Choose a folder of WAV files. No original installation is required.");
                    ui.horizontal(|ui| {
                        ui.label("Source BPM for other WAVs");
                        ui.add(egui::DragValue::new(&mut self.import_bpm).range(1.0..=999.0));
                        if ui
                            .add_enabled(
                                self.recorder.is_none(),
                                egui::Button::new("Choose WAV folder"),
                            )
                            .on_disabled_hover_text("Stop recording before you change the sample folder.")
                            .clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder()
                        {
                            self.select_wave_folder(&path);
                        }
                    });
                    if let Some(root) = self.library.wav_root() {
                        ui.label(root.display().to_string());
                    }
                    ui.label("Known extracted filenames are matched automatically. Other WAVs use the source BPM above.");
                    ui.collapsing("Optional product providers", |ui| {
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
                    let bindings = self.library.bindings().to_vec();
                    if bindings.is_empty() {
                        ui.label("No recognized product samples are configured.");
                    }
                    for binding in bindings {
                        ui.horizontal(|ui| {
                            ui.label(binding.product.label());
                            if ui.small_button("Remove").clicked() {
                                self.remove_library(binding.product);
                            }
                        });
                        ui.add(egui::Label::new(binding.root.display().to_string()).truncate())
                            .on_hover_text(binding.root.display().to_string());
                        ui.add_space(6.0);
                    }
                    });
                });
            self.show_libraries = open;
        }
        if self.show_import_details {
            let mut open = true;
            egui::Window::new("Import details")
                .open(&mut open)
                .default_width(540.0)
                .vscroll(true)
                .show(ctx, |ui| {
                    for notice in &self.editor.project.import_notices {
                        ui.label(format!(
                            "{} BPM changed to {} BPM.",
                            notice.source_bpm, notice.target_bpm
                        ));
                    }
                    for notice in &self.editor.project.source_notices {
                        if notice.starts_with("MixJam v7 source controls:") {
                            ui.collapsing("Saved MixJam source controls", |ui| {
                                ui.label(notice);
                            });
                        } else {
                            ui.label(notice);
                        }
                    }
                    ui.small("These changes are saved with the mix.");
                });
            self.show_import_details = open;
        }
        if let Some(path) = self.import_path.clone() {
            egui::Modal::new(egui::Id::new("import_sound")).show(ctx, |ui| {
                ui.heading("Import sound");
                ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                ui.horizontal(|ui| {
                    ui.label("Source BPM");
                    ui.add(egui::DragValue::new(&mut self.import_bpm).range(7.5..=1200.0));
                });
                ui.label(
                    "Enter the sound's known source tempo. Project tempo is selected separately.",
                );
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
            egui::Window::new("Effect Studio")
                .open(&mut open)
                .default_width(560.0)
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new(self.source_name())
                            .strong()
                            .color(Color32::from_rgb(255, 224, 151)),
                    );
                    effects_controls(ui, &mut self.effects);
                    ui.horizontal(|ui| {
                        if ui.button("Listen").clicked() {
                            match self.source_pcm().and_then(|p| {
                                crate::effects::process(&p, &self.effects, self.source_bpm())
                            }) {
                                Ok(pcm) => self.play_pcm(pcm, "Listening to the processed sound."),
                                Err(error) => self.error(error),
                            }
                        }
                        if ui.button("Save sound").clicked() {
                            let result = self.source_pcm().and_then(|p| {
                                crate::effects::process(&p, &self.effects, self.source_bpm())
                            });
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
                    ui.small(
                        "Reconstructed sound processing. Original DSP equivalence is unmeasured.",
                    );
                });
            self.show_effects = open;
        }
        if self.show_step_synth {
            let mut open = true;
            egui::Window::new("Step synth").open(&mut open).default_width(820.0).max_height((ctx.content_rect().height() - 80.0).max(240.0)).vscroll(true).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(self.step_synth_file.as_ref().map_or_else(|| self.source_name(), |p| p.file_stem().unwrap_or_default().to_string_lossy().into_owned())).strong());
                    if ui.button("Choose source file").clicked()
                        && let Some(path) = rfd::FileDialog::new().add_filter("Generator sound / WAV", &["\x70xd", "wav"]).set_directory(self.library.wav_root().or_else(|| self.library.source_root(Product::T2)).unwrap_or_else(|| Path::new("."))).pick_file() { self.step_synth_file = Some(path); }
                    if ui.button("Use palette sound").clicked() { self.step_synth_file = None; }
                });
                ui.horizontal(|ui| {
                    ui.label("View bar");
                    for bar in 0..4 { if ui.selectable_label(self.step_synth_bar == bar, format!("{}", bar + 1)).clicked() { self.step_synth_bar = bar; } }
                    ui.add_space(12.0);
                    for bar in 0..4 { ui.checkbox(&mut self.step_synth.active_bars[bar], format!("Active {}", bar + 1)); }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.step_synth.cut_sixteenth, "1/16 cut");
                    ui.checkbox(&mut self.step_synth.loop_source, "Loop source");
                    ui.label("BPM");
                    bpm_picker(ui, "step_synth_bpm", &mut self.step_synth.bpm);
                    if ui.button("Clear bar").clicked() { self.step_synth.steps[self.step_synth_bar * 16..self.step_synth_bar * 16 + 16].fill(None); }
                });
                egui::Grid::new("step_synth_notes").spacing(Vec2::new(2.0, 2.0)).show(ui, |ui| {
                    for note in (-12_i8..=12).rev() {
                        ui.label(if note == 0 { "A  0".to_string() } else { format!("{note:+}") });
                        for step in 0..16 {
                            let index = self.step_synth_bar * 16 + step;
                            let selected = self.step_synth.steps[index] == Some(note);
                            let fill = if selected { RED } else if matches!((note + 24) % 12, 0 | 2 | 3 | 5 | 7 | 8 | 10) { Color32::from_rgb(46, 71, 127) } else { NAVY };
                            if ui.add_sized([33.0, 13.0], egui::Button::new("").fill(fill)).on_hover_text(format!("Step {}, {note:+} semitones", step + 1)).clicked() { self.step_synth.steps[index] = if selected { None } else { Some(note) }; }
                        }
                        ui.end_row();
                    }
                    ui.label("Step"); for step in 1..=16 { ui.label(format!("{step}")); } ui.end_row();
                });
                egui::CollapsingHeader::new("Effects").default_open(self.step_synth_effects_expanded).show(ui, |ui| effects_controls(ui, &mut self.step_synth.effects));
                ui.horizontal(|ui| {
                    if ui.button("Listen sequence").clicked() {
                        match self.step_synth_pcm() { Ok(pcm) => self.play_pcm(pcm, "Listening to Step synth sequence."), Err(error) => self.error(error) }
                    }
                    if ui.button("Save sequence").clicked() {
                        match self.step_synth_pcm() { Ok(pcm) => { if let Err(error) = self.save_pcm(&pcm, "Step synth sequence", "Step synth", self.step_synth.bpm) { self.error(error); } }, Err(error) => self.error(error) }
                    }
                    if ui.button("Save pattern").clicked() && let Some(path) = rfd::FileDialog::new().add_filter("Step synth pattern", &["json"]).set_file_name("Step synth pattern.json").save_file() { match serde_json::to_vec_pretty(&self.step_synth).map_err(|e| e.to_string()).and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string())) { Ok(()) => self.message("Saved Step synth pattern."), Err(error) => self.error(error) } }
                    if ui.button("Load pattern").clicked() && let Some(path) = rfd::FileDialog::new().add_filter("Step synth pattern", &["json"]).pick_file() { match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| { if bytes.len() > 1_048_576 { Err("The pattern file is too large.".into()) } else { serde_json::from_slice::<crate::synth::StepSynthState>(&bytes).map_err(|e| e.to_string()) } }).and_then(|mut state| { state.validate()?; state.bpm = crate::engine::nearest_supported_bpm(state.bpm)?; Ok(state) }) { Ok(state) => { self.step_synth = state; self.message("Loaded Step synth pattern."); }, Err(error) => self.error(error) } }
                });
                ui.small("Reconstructed generator and effects. Original synthesis equivalence is unmeasured.");
                if self.capture_path.is_some() && self.step_synth_effects_expanded { ui.scroll_to_cursor(Some(egui::Align::BOTTOM)); }
            });
            self.show_step_synth = open;
        }
        if self.show_sample_edit {
            let mut open = true;
            egui::Window::new("Sample editor")
                .open(&mut open)
                .default_width(510.0)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(self.source_name()).strong());
                    let frames = self
                        .source_id()
                        .and_then(|id| self.library.sample_by_id(id))
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
                    );
                    ui.small("Pitch changes playback speed and sample length.");
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
            egui::Window::new("Time Stretcher").open(&mut open).default_width(470.0).show(ctx, |ui| {
                ui.label(egui::RichText::new(self.source_name()).strong());
                ui.horizontal(|ui| { ui.label("Original BPM"); ui.add(egui::DragValue::new(&mut self.stretch_source_bpm).range(30.0..=300.0)); ui.label("Target BPM"); bpm_picker(ui, "stretch_bpm", &mut self.stretch_target_bpm); });
                ui.small("Enter the known source tempo. This conversion approximates pitch preservation.");
                ui.horizontal(|ui| {
                    if ui.button("Listen stretch").clicked() { match self.stretched_pcm() { Ok(pcm) => self.play_pcm(pcm, "Listening to the stretched sound."), Err(error) => self.error(error) } }
                    if ui.button("Save stretched sound").clicked() { match self.stretched_pcm() { Ok(pcm) => { let name = format!("{} - stretch", self.source_name()); if let Err(error) = self.save_pcm(&pcm, &name, "Wave", self.stretch_target_bpm) { self.error(error); } }, Err(error) => self.error(error) } }
                });
            });
            self.show_stretch = open;
        }
        if self.recorded_take.is_some() && self.recorder.is_none() {
            egui::Window::new("Recorded take")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Your take is kept in memory until you save or discard it.");
                    ui.horizontal(|ui| {
                        if ui.button("Listen").clicked()
                            && let Some(pcm) = self.recorded_take.clone()
                        {
                            self.play_pcm(pcm, "Listening to the recorded take.");
                        }
                        if ui.button("Save take").clicked() {
                            self.save_recorded_take();
                        }
                        if ui.button("Discard take").clicked() {
                            self.recorded_take = None;
                        }
                    });
                });
        }
    }

    fn step_synth_pcm(&self) -> Result<Pcm, String> {
        let source = if let Some(path) = &self.step_synth_file {
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
            {
                audio::read_wav(path)?
            } else {
                crate::library::read_packed_file(path)?
            }
        } else {
            self.source_pcm()?
        };
        crate::synth::render(&source, &self.step_synth)
    }

    fn edited_pcm(&self) -> Result<Pcm, String> {
        let source = self.source_pcm()?;
        let trimmed = crate::sample_edit::trim(&source, self.edit_start, self.edit_end)?;
        crate::sample_edit::pitch_shift(&trimmed, self.edit_pitch)
    }

    fn stretched_pcm(&self) -> Result<Pcm, String> {
        crate::sample_edit::time_stretch(
            &self.source_pcm()?,
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

fn recording_destination(ui: &mut egui::Ui, selected: &mut usize, occupied: &[usize]) -> Response {
    egui::ComboBox::from_id_salt("record_destination")
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
        .response
        .on_hover_text(
            "Choose a track for recording. Unused tracks appear only when you add a sound.",
        )
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

fn bpm_picker(ui: &mut egui::Ui, id: &str, bpm: &mut f64) -> Response {
    let before = *bpm;
    let mut response = egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{bpm:.0}"))
        .width(64.0)
        .show_ui(ui, |ui| {
            for value in SUPPORTED_BPMS {
                ui.selectable_value(bpm, value, format!("{value:.0}"));
            }
        })
        .response;
    if *bpm != before {
        response.mark_changed();
    }
    response
}

fn pan_control(ui: &mut egui::Ui, index: usize, pan: &mut f32) -> Response {
    ui.add(egui::Slider::new(pan, -1.0..=1.0).show_value(false))
        .on_hover_text(format!("Track {} pan: {:.0}%", index + 1, *pan * 100.0))
}

fn effects_controls(ui: &mut egui::Ui, state: &mut crate::effects::EffectsState) {
    ui.horizontal(|ui| {
        let mut enabled = state.filter.is_some();
        if ui.checkbox(&mut enabled, "Filter").changed() {
            state.filter = enabled.then(Default::default);
        }
        if let Some(filter) = &mut state.filter {
            ui.add(
                egui::Slider::new(&mut filter.cutoff_hz, 20.0..=20000.0)
                    .logarithmic(true)
                    .text("Cutoff"),
            );
        }
    });
    if let Some(filter) = &mut state.filter {
        ui.add(egui::Slider::new(&mut filter.resonance, 0.0..=1.0).text("Resonance"));
        ui.add(egui::Slider::new(&mut filter.drive, 0.0..=1.0).text("Overdrive"));
        ui.add(egui::Slider::new(&mut filter.wet, 0.0..=1.0).text("Filter volume"));
    }
    let mut enabled = state.echo.is_some();
    if ui.checkbox(&mut enabled, "Echo").changed() {
        state.echo = enabled.then(Default::default);
    }
    if let Some(echo) = &mut state.echo {
        ui.add(egui::Slider::new(&mut echo.delay_beats, 0.001..=2.0).text("Delay (beats)"));
        ui.add(egui::Slider::new(&mut echo.feedback, 0.0..=0.95).text("Feedback"));
        ui.add(egui::Slider::new(&mut echo.wet, 0.0..=1.0).text("Echo volume"));
    }
    let mut enabled = state.robotize.is_some();
    if ui.checkbox(&mut enabled, "Robotize").changed() {
        state.robotize = enabled.then(Default::default);
    }
    if let Some(robot) = &mut state.robotize {
        ui.add(
            egui::Slider::new(&mut robot.frequency_hz, 1.0..=2000.0)
                .logarithmic(true)
                .text("Color"),
        );
        ui.add(egui::Slider::new(&mut robot.wet, 0.0..=1.0).text("Robotize volume"));
    }
    ui.add(egui::Slider::new(&mut state.dirty, 0.0..=1.0).text("Dirty distortion"));
    ui.add(egui::Slider::new(&mut state.noise, 0.0..=1.0).text("Noise distortion"));
    ui.add(egui::Slider::new(&mut state.volume, 0.0..=2.0).text("Output volume"));
}

fn write_bmp(path: &Path, image: &egui::ColorImage) -> std::io::Result<()> {
    let width = image.width() as u32;
    let height = image.height() as u32;
    let bytes = width * height * 4;
    let mut file = std::fs::File::create(path)?;
    file.write_all(b"BM")?;
    file.write_all(&(54 + bytes).to_le_bytes())?;
    file.write_all(&[0; 4])?;
    file.write_all(&54_u32.to_le_bytes())?;
    file.write_all(&40_u32.to_le_bytes())?;
    file.write_all(&width.to_le_bytes())?;
    file.write_all(&(-(height as i32)).to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&32_u16.to_le_bytes())?;
    file.write_all(&0_u32.to_le_bytes())?;
    file.write_all(&bytes.to_le_bytes())?;
    file.write_all(&[0; 16])?;
    for color in &image.pixels {
        file.write_all(&[color.b(), color.g(), color.r(), 255])?;
    }
    Ok(())
}

impl eframe::App for MixJamApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

impl MixJamApp {
    fn draw(&mut self, ctx: &egui::Context) {
        self.poll_render(ctx);
        if self.wants_playback
            && !self.render_dirty
            && self.requested_revision.is_none()
            && !self.playing_mix()
        {
            self.wants_playback = false;
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && (self.unsaved || self.recorded_take.is_some() || self.recorder.is_some())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request_action(PendingAction::Close, ctx);
        }
        if self.playing_mix() {
            if let Some(player) = &self.player
                && !self.seek_pending
            {
                self.cursor_tick =
                    (player.position_frames() as f64 / self.playback_frames_per_tick) as u32;
            }
            ctx.request_repaint_after(Duration::from_millis(30));
        }
        self.poll_recording(ctx);
        if self.pending_action.is_none() {
            self.keyboard(ctx);
        }
        self.header(ctx);
        self.side_rail(ctx);
        self.bottom(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(COBALT).inner_margin(8))
            .show(ctx, |ui| self.timeline(ui));
        self.finish_drag(ctx);
        self.dialogs(ctx);
        self.tool_windows(ctx);
        self.capture(ctx);
    }
}

fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = COBALT;
    style.visuals.window_fill = Color32::from_rgb(15, 25, 75);
    style.visuals.extreme_bg_color = NAVY;
    style.visuals.override_text_color = Some(ICE);
    style.visuals.selection.bg_fill = Color32::from_rgb(52, 89, 177);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 58, 119);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(65, 94, 166);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(78, 113, 186);
    style.spacing.item_spacing = Vec2::new(7.0, 5.0);
    style.spacing.slider_width = 90.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(13.0));
    ctx.set_style(style);
}

fn metallic(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let steps = 24;
    for step in 0..steps {
        let t = step as f32 / (steps - 1) as f32;
        let highlight = (1.0 - ((t - 0.27) * 5.0).abs()).max(0.0) * 0.16;
        let mix = |a: u8, b: u8| {
            ((a as f32 * (1.0 - t) + b as f32 * t) * (1.0 - highlight) + 255.0 * highlight) as u8
        };
        let color = Color32::from_rgb(
            mix(top.r(), bottom.r()),
            mix(top.g(), bottom.g()),
            mix(top.b(), bottom.b()),
        );
        let y0 = rect.top() + rect.height() * step as f32 / steps as f32;
        let y1 = rect.top() + rect.height() * (step + 1) as f32 / steps as f32;
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(rect.left(), y0), Pos2::new(rect.right(), y1)),
            0.0,
            color,
        );
    }
}

fn outline(painter: &egui::Painter, rect: Rect, color: Color32, width: f32) {
    let stroke = Stroke::new(width, color);
    painter.line_segment([rect.left_top(), rect.right_top()], stroke);
    painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
    painter.line_segment([rect.right_bottom(), rect.left_bottom()], stroke);
    painter.line_segment([rect.left_bottom(), rect.left_top()], stroke);
}

fn metal_button(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    let font = FontId::proportional(13.0);
    let text_width = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), NAVY)
        .size()
        .x;
    let width = (text_width + 18.0)
        .max(51.0)
        .min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 29.0), Sense::click());
    metallic(
        ui.painter(),
        rect.shrink(1.0),
        if response.hovered() {
            Color32::from_rgb(234, 246, 255)
        } else {
            Color32::from_rgb(190, 214, 235)
        },
        Color32::from_rgb(93, 127, 170),
    );
    outline(
        ui.painter(),
        rect.shrink(0.5),
        if active || response.has_focus() {
            GREEN
        } else {
            Color32::from_rgb(62, 86, 137)
        },
        if active { 2.0 } else { 1.0 },
    );
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, label, font, NAVY);
    response
}

fn rail_button(ui: &mut egui::Ui, icons: &RailIcons, icon: RailIcon, active: bool) -> Response {
    let width = 58.0_f32.min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 29.0), Sense::click());
    metallic(
        ui.painter(),
        rect.shrink(1.0),
        if response.hovered() {
            Color32::from_rgb(234, 246, 255)
        } else {
            Color32::from_rgb(190, 214, 235)
        },
        Color32::from_rgb(93, 127, 170),
    );
    outline(
        ui.painter(),
        rect.shrink(0.5),
        if active || response.has_focus() {
            GREEN
        } else {
            Color32::from_rgb(62, 86, 137)
        },
        if active { 2.0 } else { 1.0 },
    );
    let image = Rect::from_center_size(rect.center(), Vec2::splat(22.0));
    ui.painter().image(
        icons.texture_id(icon),
        image,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        if ui.is_enabled() {
            Color32::WHITE
        } else {
            Color32::from_white_alpha(96)
        },
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
    response.on_hover_text(icon.label())
}

fn rail_heading(ui: &mut egui::Ui, icons: &RailIcons, icon: RailIcon) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(58.0, 22.0), Sense::hover());
    ui.painter().image(
        icons.texture_id(icon),
        Rect::from_center_size(rect.center(), Vec2::splat(22.0)),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Image, ui.is_enabled(), icon.label())
    });
    response.on_hover_text(icon.label());
}

fn category_button(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(90.0, 30.0), Sense::click());
    metallic(
        ui.painter(),
        rect.shrink(1.0),
        Color32::from_rgb(137, 182, 224),
        Color32::from_rgb(43, 85, 153),
    );
    outline(
        ui.painter(),
        rect.shrink(0.5),
        if active {
            Color32::from_rgb(249, 220, 124)
        } else {
            Color32::from_rgb(165, 202, 236)
        },
        if active { 2.0 } else { 1.0 },
    );
    ui.painter().rect_filled(
        Rect::from_center_size(rect.center(), Vec2::new(80.0, 21.0)),
        0.0,
        Color32::from_rgb(28, 55, 115),
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        Color32::WHITE,
    );
    response
}

fn group_color(group: &str) -> Color32 {
    match group.to_ascii_lowercase().as_str() {
        "loop" => Color32::from_rgb(244, 218, 132),
        "drum" => Color32::from_rgb(229, 180, 133),
        "bass" => Color32::from_rgb(184, 221, 184),
        "sequence" => Color32::from_rgb(180, 211, 239),
        "sphere" => Color32::from_rgb(178, 218, 238),
        "effect" => Color32::from_rgb(235, 164, 163),
        "voice" => Color32::from_rgb(217, 184, 222),
        _ => Color32::from_rgb(231, 195, 152),
    }
}

fn sample_button(ui: &mut egui::Ui, sample: &Sample, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(118.0, 39.0), Sense::click_and_drag());
    draw_clip(
        ui.painter(),
        rect,
        Some(sample),
        selected || response.hovered(),
        1.0,
    );
    response.on_hover_text(format!(
        "{}\n{} bars | {} Hz\nDouble-click to listen. Drag to a track.",
        sample.name, sample.bars, sample.sample_rate
    ))
}

fn draw_clip(
    painter: &egui::Painter,
    rect: Rect,
    sample: Option<&Sample>,
    selected: bool,
    gain: f32,
) {
    let color = sample
        .map(|s| group_color(&s.group_name))
        .unwrap_or(Color32::LIGHT_GRAY);
    painter.rect_filled(rect, 0.0, color);
    let clipped = painter.with_clip_rect(painter.clip_rect().intersect(rect.shrink(2.0)));
    for x in (0..(rect.width() / 16.0) as usize).map(|n| rect.left() + n as f32 * 16.0) {
        let points = vec![
            Pos2::new(x, rect.center().y),
            Pos2::new(x + 4.0, rect.top() + 3.0),
            Pos2::new(x + 8.0, rect.center().y),
            Pos2::new(x + 12.0, rect.bottom() - 3.0),
            Pos2::new(x + 16.0, rect.center().y),
        ];
        clipped.add(egui::Shape::line(
            points,
            Stroke::new(0.5_f32, Color32::from_white_alpha(45)),
        ));
    }
    outline(
        painter,
        rect.shrink(0.5),
        if selected {
            Color32::WHITE
        } else {
            Color32::from_rgb(117, 108, 111)
        },
        if selected { 2.0 } else { 1.0 },
    );
    if let Some(sample) = sample {
        let size = if rect.height() < 24.0 { 9.0 } else { 11.0 };
        let text = clipped.layout(
            visible_clip_label(&sample.name, rect.width() - 8.0, size, rect.height()),
            FontId::proportional(size),
            Color32::from_rgb(20, 25, 37),
            rect.width() - 8.0,
        );
        clipped.galley(
            rect.left_top() + Vec2::new(4.0, 2.0),
            text,
            Color32::from_rgb(20, 25, 37),
        );
    }
    if selected {
        let y = rect.bottom() - (gain.min(2.0) / 2.0 * rect.height()).max(2.0);
        clipped.line_segment(
            [
                Pos2::new(rect.left() + 3.0, y),
                Pos2::new(rect.right() - 3.0, y),
            ],
            Stroke::new(1.0_f32, Color32::from_black_alpha(100)),
        );
    }
}

fn clip_label(name: &str, width: f32, font_size: f32) -> String {
    let capacity = (width / (font_size * 0.51)).floor().max(1.0) as usize;
    if name.chars().count() <= capacity {
        return name.into();
    }
    let split = name
        .char_indices()
        .enumerate()
        .take(capacity + 1)
        .filter(|(_, (_, c))| c.is_whitespace())
        .last()
        .map(|(_, (byte, _))| byte)
        .unwrap_or_else(|| {
            name.char_indices()
                .nth(capacity)
                .map_or(name.len(), |(byte, _)| byte)
        });
    let (first, tail) = name.split_at(split);
    let tail = tail.trim_start();
    let second = if tail.chars().count() > capacity {
        format!(
            "{}…",
            tail.chars()
                .take(capacity.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        tail.into()
    };
    format!("{first}\n{second}")
}

fn visible_clip_label(name: &str, width: f32, font_size: f32, height: f32) -> String {
    if height >= 24.0 {
        return clip_label(name, width, font_size);
    }
    let capacity = (width / (font_size * 0.51)).floor().max(1.0) as usize;
    if name.chars().count() <= capacity {
        name.into()
    } else {
        format!(
            "{}…",
            name.chars()
                .take(capacity.saturating_sub(1))
                .collect::<String>()
        )
    }
}

fn route_button(ui: &mut egui::Ui, text: &str, active: &mut bool) -> Response {
    let mut response = ui.add(
        egui::Button::new(egui::RichText::new(text).color(if *active { NAVY } else { ICE }))
            .fill(if *active {
                GREEN
            } else {
                Color32::from_rgb(45, 72, 137)
            })
            .min_size(Vec2::new(17.0, 20.0)),
    );
    if response.clicked() {
        *active = !*active;
        response.mark_changed();
    }
    response
}

fn speaker(painter: &egui::Painter, rect: Rect, enabled: bool, left: bool) {
    let center = rect.center();
    let sign = if left { -1.0 } else { 1.0 };
    let color = if enabled {
        GREEN
    } else {
        Color32::from_rgb(57, 94, 174)
    };
    painter.add(egui::Shape::convex_polygon(
        vec![
            center + Vec2::new(-sign * 5.0, -3.0),
            center + Vec2::new(sign * 1.0, -3.0),
            center + Vec2::new(sign * 7.0, -8.0),
            center + Vec2::new(sign * 7.0, 8.0),
            center + Vec2::new(sign * 1.0, 3.0),
            center + Vec2::new(-sign * 5.0, 3.0),
        ],
        color,
        Stroke::new(1.0_f32, NAVY),
    ));
}

#[derive(Clone, Copy)]
enum TransportIcon {
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

fn transport_button(
    ui: &mut egui::Ui,
    icon: TransportIcon,
    active: bool,
    tooltip: &str,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(48.0, 39.0), Sense::click());
    metallic(
        ui.painter(),
        rect.shrink(1.0),
        if response.hovered() {
            Color32::from_rgb(239, 249, 255)
        } else {
            Color32::from_rgb(191, 214, 235)
        },
        Color32::from_rgb(99, 125, 171),
    );
    let center = rect.center();
    let painter = ui.painter();
    let color = if active { GREEN } else { NAVY };
    let triangle = |x: f32, direction: f32| {
        painter.add(egui::Shape::convex_polygon(
            vec![
                center + Vec2::new(x - direction * 5.0, -9.0),
                center + Vec2::new(x - direction * 5.0, 9.0),
                center + Vec2::new(x + direction * 7.0, 0.0),
            ],
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
            painter.circle_filled(center, 10.0, RED);
            if active {
                painter.circle_stroke(center, 13.0, Stroke::new(1.0_f32, Color32::WHITE));
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
            let points: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let a = 0.4 + i as f32 / 20.0 * 5.3;
                    center + Vec2::new(a.cos() * 12.0, a.sin() * 8.0)
                })
                .collect();
            painter.add(egui::Shape::line(points, Stroke::new(2.2_f32, color)));
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + Vec2::new(11.0, 3.0),
                    center + Vec2::new(4.0, 2.0),
                    center + Vec2::new(10.0, -4.0),
                ],
                color,
                Stroke::NONE,
            ));
        }
    }
    outline(
        painter,
        rect.shrink(0.5),
        if response.has_focus() {
            GREEN
        } else {
            Color32::from_rgb(58, 80, 132)
        },
        1.0,
    );
    response.on_hover_text(tooltip)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (egui::Context, MixJamApp) {
        let context = egui::Context::default();
        configure_style(&context);
        let sample = Sample {
            id: 1,
            code: "FIXTURE".into(),
            name: "good grasp 2".into(),
            bars: 1,
            group: 1,
            group_name: "Loop".into(),
            offset: 0,
            length: 0,
            frame_count: 75600,
            sample_rate: 44100,
            channels: 1,
            source_bpm: 140.0,
            wav_path: None,
        };
        let library = Arc::new(Library::fixture(vec![sample]));
        let mut editor = Editor::new(Project::default());
        editor.place(1, 0, 0, TICKS_PER_BAR).unwrap();
        editor.place(1, 1, TICKS_PER_BAR, TICKS_PER_BAR).unwrap();
        let mut app =
            MixJamApp::from_parts(library, editor.project, None, "UI fixture".into(), false);
        app.capture_path = None;
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        frame(&context, &mut app, vec![], egui::Modifiers::NONE);
        (context, app)
    }

    fn frame(
        context: &egui::Context,
        app: &mut MixJamApp,
        events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 1000.0))),
            events,
            modifiers,
            focused: true,
            ..Default::default()
        };
        context.run(input, |ctx| app.draw(ctx))
    }

    fn pointer(
        context: &egui::Context,
        app: &mut MixJamApp,
        pos: Pos2,
        pressed: bool,
        modifiers: egui::Modifiers,
    ) {
        frame(
            context,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers,
                },
            ],
            modifiers,
        );
    }

    fn click(context: &egui::Context, app: &mut MixJamApp, pos: Pos2, modifiers: egui::Modifiers) {
        pointer(context, app, pos, true, modifiers);
        pointer(context, app, pos, false, modifiers);
    }

    #[test]
    fn native_canvas_shift_selection_group_drag_and_delete_are_wired() {
        let (ctx, mut app) = fixture();
        let canvas = app.timeline_rect.unwrap();
        let first = app.clip_rect(&app.editor.project.clips[0], canvas).center();
        let second = app.clip_rect(&app.editor.project.clips[1], canvas).center();
        click(&ctx, &mut app, first, egui::Modifiers::NONE);
        click(&ctx, &mut app, second, egui::Modifiers::SHIFT);
        assert_eq!(app.selected_clips.len(), 2, "Shift must add a second clip.");
        let target = first + Vec2::new(app.timeline_bar_width, 0.0);
        pointer(&ctx, &mut app, first, true, egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
        assert_eq!(app.editor.project.clips[0].start_tick, TICKS_PER_BAR);
        assert_eq!(app.editor.project.clips[1].start_tick, TICKS_PER_BAR * 2);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        );
        assert!(
            app.editor.project.clips.is_empty(),
            "Delete must remove the selected group."
        );
        app.history_step(false);
        assert_eq!(app.editor.project.clips.len(), 2);
    }

    #[test]
    fn native_canvas_alt_drag_copies_group_and_undo_restores_once() {
        let (ctx, mut app) = fixture();
        let canvas = app.timeline_rect.unwrap();
        let first = app.clip_rect(&app.editor.project.clips[0], canvas).center();
        app.selected_clips = [1, 2].into_iter().collect();
        app.selected_clip = Some(1);
        let target = first + Vec2::new(app.timeline_bar_width * 3.0, 0.0);
        pointer(&ctx, &mut app, first, true, egui::Modifiers::ALT);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::ALT,
        );
        pointer(&ctx, &mut app, target, false, egui::Modifiers::ALT);
        assert_eq!(app.editor.project.clips.len(), 4);
        assert_eq!(app.selected_clips.len(), 2);
        app.history_step(false);
        assert_eq!(app.editor.project.clips.len(), 2);
    }

    #[test]
    fn fine_snap_and_two_line_labels_preserve_unicode_boundaries() {
        let (_, app) = fixture();
        let rect = app.timeline_rect.unwrap();
        let x = rect.left() + app.timeline_bar_width * 1.59;
        assert_eq!(app.position_tick(x, rect, false), 256);
        assert_eq!(app.position_tick(x, rect, true), 400);
        assert_eq!(clip_label("good grasp 2", 54.0, 11.0), "good\ngrasp 2");
        assert_eq!(clip_label("électronique", 10.0, 11.0).lines().count(), 2);
        let short = visible_clip_label("good grasp 2", 54.0, 9.0, 16.0);
        assert_eq!(
            short.lines().count(),
            1,
            "Short clip rows must keep the text on one visible line."
        );
        assert!(short.ends_with('…'));
        assert_eq!(
            visible_clip_label("good grasp 2", 54.0, 11.0, 26.0),
            "good\ngrasp 2"
        );
    }

    #[test]
    fn imported_catalog_tracks_history_atomically_and_keeps_source_tempo() {
        let (_, mut app) = fixture();
        app.palette_product = Some(Product::T1);
        let path = std::env::temp_dir().join(format!(
            "mixjam-techno-ui-import-{}.wav",
            std::process::id()
        ));
        let pcm = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.25; 1000],
        };
        pcm.write_wav(&path).unwrap();
        let id = app.register_file(&path, "Wave", 90.0).unwrap();
        assert_eq!(app.library.sample_by_id(id).unwrap().source_bpm, 90.0);
        assert_eq!(app.editor.project.imported_samples.len(), 1);
        assert!(app.palette_product.is_none());
        assert!(
            app.palette_indices()
                .iter()
                .any(|&index| app.library.samples[index].id == id)
        );
        app.history_step(false);
        assert!(app.library.sample_by_id(id).is_none());
        app.history_step(true);
        assert!(app.library.sample_by_id(id).is_some());
        app.history_step(false);
        std::fs::remove_file(&path).unwrap();
        let before = app.editor.project.clone();
        app.history_step(true);
        assert_eq!(
            app.editor.project, before,
            "A missing imported file must reject redo atomically."
        );
        assert!(app.library.sample_by_id(id).is_none());
        assert!(app.status_error);
    }

    #[test]
    fn recording_blocks_sample_folder_changes_before_scanning_or_replacing_state() {
        let (_, mut app) = fixture();
        app.editor
            .transact(|project| project.title = "Before recording".into())
            .unwrap();
        app.selected_sample = Some(1);
        app.select_clip(1, false);
        app.cursor_tick = TICKS_PER_BAR;
        app.recorder = Some(crate::recording::Recorder::fixture());
        let project = app.editor.project.clone();
        let library = app.library.clone();
        let revision = app.revision;
        let unsaved = app.unsaved;

        // An empty path cannot be scanned. The recording guard must run first.
        app.select_wave_folder(Path::new(""));

        assert_eq!(
            app.status,
            "Stop recording before you change the sample folder."
        );
        assert!(app.status_error);
        assert_eq!(app.editor.project, project);
        assert!(Arc::ptr_eq(&app.library, &library));
        assert_eq!(app.revision, revision);
        assert_eq!(app.unsaved, unsaved);
        assert_eq!(app.cursor_tick, TICKS_PER_BAR);
        assert_eq!(app.selected_sample, Some(1));
        assert_eq!(app.selected_clip, Some(1));
        assert_eq!(app.selected_clips, [1].into_iter().collect());
        assert!(app.recorded_take.is_none());
        assert_eq!(app.recorder.as_ref().unwrap().status().captured_frames, 2);
        assert!(
            app.editor.undo(),
            "The blocked action must retain edit history."
        );
        assert_eq!(
            app.recorder.take().unwrap().finish().unwrap().samples,
            vec![0.25, -0.25]
        );
    }

    #[test]
    fn unsaved_mix_and_take_require_a_pending_action_before_replacement() {
        let (ctx, mut app) = fixture();
        app.unsaved = true;
        app.request_action(PendingAction::New, &ctx);
        assert!(matches!(app.pending_action, Some(PendingAction::New)));
        assert_eq!(app.editor.project.clips.len(), 2);
        app.pending_action = None;
        app.unsaved = false;
        app.recorded_take = Some(Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.0; 10],
        });
        app.request_action(PendingAction::Close, &ctx);
        assert!(matches!(app.pending_action, Some(PendingAction::Close)));
        assert!(!app.allow_close);
        assert!(app.recorded_take.is_some());
    }

    #[test]
    fn render_worker_keeps_only_the_latest_pending_revision_and_cancels_it() {
        let (_, app) = fixture();
        let (sender, requests) = mpsc::channel();
        let (results, receiver) = mpsc::channel();
        let mut worker = RenderWorker {
            sender,
            receiver,
            in_flight: false,
            current_revision: None,
            pending: None,
        };
        let request = |revision| RenderRequest {
            revision,
            project: app.editor.project.clone(),
            library: app.library.clone(),
        };
        worker.request(request(1)).unwrap();
        worker.request(request(2)).unwrap();
        worker.request(request(3)).unwrap();
        assert_eq!(requests.try_recv().unwrap().revision, 1);
        assert!(requests.try_recv().is_err());
        results
            .send(RenderResult {
                revision: 1,
                frames_per_tick: 1.0,
                pcm: Err("Fixture result".into()),
            })
            .unwrap();
        assert_eq!(worker.poll().unwrap().revision, 1);
        assert_eq!(
            requests.try_recv().unwrap().revision,
            3,
            "Intermediate fader revisions must not fill the worker queue."
        );
        worker.request(request(4)).unwrap();
        worker.cancel_pending();
        results
            .send(RenderResult {
                revision: 3,
                frames_per_tick: 1.0,
                pcm: Err("Fixture result".into()),
            })
            .unwrap();
        worker.poll().unwrap();
        assert!(requests.try_recv().is_err());
        assert!(!worker.in_flight);
    }

    #[test]
    fn render_results_cannot_replace_a_new_project_or_autoplay_after_pause() {
        let (ctx, mut app) = fixture();
        let (sender, _requests) = mpsc::channel();
        let (results, receiver) = mpsc::channel();
        app.render_worker = RenderWorker {
            sender,
            receiver,
            in_flight: false,
            current_revision: None,
            pending: None,
        };
        app.revision = 5;
        app.wants_playback = true;
        app.pause();
        let pcm = Arc::new(Pcm {
            sample_rate: 1000,
            channels: 2,
            samples: vec![0.0; 20],
        });
        results
            .send(RenderResult {
                revision: 5,
                frames_per_tick: 1.0,
                pcm: Ok(pcm.clone()),
            })
            .unwrap();
        app.poll_render(&ctx);
        assert!(!app.wants_playback);
        assert!(
            app.render.is_some(),
            "A paused render may be cached without starting an audio device."
        );
        app.new_mix();
        results
            .send(RenderResult {
                revision: 5,
                frames_per_tick: 1.0,
                pcm: Ok(pcm),
            })
            .unwrap();
        app.poll_render(&ctx);
        assert!(
            app.render.is_none(),
            "A previous project's late result must be discarded."
        );
        assert!(app.render_dirty);
    }

    #[test]
    fn scrolling_reaches_track_64_and_keeps_labels_and_ruler_interactive() {
        let (ctx, mut app) = fixture();
        for track in 2..TRACK_COUNT {
            app.editor.place(1, track, 0, TICKS_PER_BAR).unwrap();
        }
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.timeline_tracks.len(), 64);
        assert!(app.timeline_add_track.is_none());
        let visible = app.timeline_drop_rect.unwrap();
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(visible.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -2500.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
        );
        for _ in 0..5 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let canvas = app.timeline_rect.unwrap();
        let visible = app.timeline_drop_rect.unwrap();
        let row_y = canvas.top() + 63.5 * app.timeline_row_height;
        assert!(visible.contains(Pos2::new(visible.center().x, row_y)));
        click(
            &ctx,
            &mut app,
            Pos2::new(visible.left() - 44.0, row_y),
            egui::Modifiers::NONE,
        );
        assert_eq!(
            app.armed_track, 63,
            "Track labels must share the lane scroll offset."
        );
        click(
            &ctx,
            &mut app,
            Pos2::new(visible.left() - 70.0, row_y),
            egui::Modifiers::NONE,
        );
        assert!(
            !app.editor.project.tracks[63].left_enabled,
            "route click at {row_y}; current canvas {:?}, visible {:?}, row {}; disabled tracks {:?}",
            app.timeline_rect,
            app.timeline_drop_rect,
            app.timeline_row_height,
            app.editor
                .project
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, track)| !track.left_enabled)
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
        );
        assert!(app.editor.project.tracks[0].left_enabled);
        let ruler = app.timeline_ruler_rect.unwrap();
        let seek_x = ruler.left() + 2.5 * app.timeline_bar_width;
        click(
            &ctx,
            &mut app,
            Pos2::new(seek_x, ruler.center().y),
            egui::Modifiers::NONE,
        );
        assert_eq!(app.cursor_tick, 2 * TICKS_PER_BAR);
        app.selected_sample = Some(1);
        click(
            &ctx,
            &mut app,
            Pos2::new(seek_x, row_y),
            egui::Modifiers::NONE,
        );
        assert!(
            app.editor
                .project
                .clips
                .iter()
                .any(|clip| clip.track == 63 && clip.start_tick == 2 * TICKS_PER_BAR)
        );
        let ruler_before = app.timeline_ruler_rect.unwrap();
        let visible = app.timeline_drop_rect.unwrap();
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(visible.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(-400.0, 0.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            egui::Modifiers::NONE,
        );
        for _ in 0..5 {
            frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        }
        let ruler = app.timeline_ruler_rect.unwrap();
        let canvas = app.timeline_rect.unwrap();
        assert!(canvas.left() < ruler.left() - 100.0);
        assert_eq!(
            ruler.left(),
            ruler_before.left(),
            "Ruler viewport stays fixed while bars scroll."
        );
        let pos = Pos2::new(ruler.left() + 30.0, ruler.center().y);
        click(&ctx, &mut app, pos, egui::Modifiers::NONE);
        let expected = app.position_tick(pos.x, app.timeline_rect.unwrap(), false);
        assert_eq!(app.cursor_tick, expected);
    }

    #[test]
    fn browser_keeps_unknown_categories_and_product_identity_accessible() {
        let (_, mut app) = fixture();
        let first = app.library.samples[0].clone();
        let mut t1 = first.clone();
        t1.id = Product::T1.global_sample_id(731).unwrap();
        t1.group_name = "Rap".into();
        let mut kick = first.clone();
        kick.id = Product::D1.global_sample_id(73).unwrap();
        kick.group_name = "Kick".into();
        let mut unnamed = first.clone();
        unnamed.id = Product::H1.global_sample_id(74).unwrap();
        unnamed.group_name.clear();
        app.library = Arc::new(Library::fixture(vec![first, t1, kick, unnamed]));
        assert_eq!(app.palette_indices(), vec![0, 1, 2, 3]);
        app.group = Some("kick".into());
        assert_eq!(app.palette_indices(), vec![2]);
        app.group = None;
        app.palette_product = Some(Product::T1);
        assert_eq!(app.palette_indices(), vec![1]);
        app.palette_product = None;
        app.group = Some(String::new());
        assert_eq!(app.palette_indices(), vec![3]);
    }

    #[test]
    fn failed_open_and_in_use_library_removal_preserve_the_active_project() {
        let (_, mut app) = fixture();
        let previous = app.editor.project.clone();
        let library = app.library.clone();
        app.open_path(Path::new("missing-ui-fixture.invalid"));
        assert!(app.status_error);
        assert_eq!(app.editor.project, previous);
        assert!(Arc::ptr_eq(&library, &app.library));
        app.remove_library(Product::T2);
        assert!(app.status_error);
        assert!(app.status.contains("This mix uses"));
        assert_eq!(app.editor.project, previous);
        assert!(Arc::ptr_eq(&library, &app.library));
    }

    #[test]
    fn persisted_import_notices_are_visible_at_startup_and_keep_details() {
        let (_, app) = fixture();
        let mut project = app.editor.project.clone();
        project.import_notices.push(crate::engine::ImportNotice {
            source_bpm: 146.0,
            target_bpm: 140.0,
        });
        let mut reopened =
            MixJamApp::from_parts(app.library.clone(), project, None, "Ready.".into(), false);
        assert!(reopened.status.contains("146 BPM changed to 140 BPM"));
        reopened.message("Ready to edit.");
        assert_eq!(reopened.editor.project.import_notices.len(), 1);
        reopened.set_startup_messages(&["One library folder is unavailable.".into()]);
        assert!(reopened.status_error);
        assert!(
            reopened
                .status
                .contains("One library folder is unavailable")
        );
    }

    #[test]
    fn native_tempo_menu_and_pan_controls_accept_user_input() {
        let ctx = egui::Context::default();
        configure_style(&ctx);
        let mut bpm = 140.0;
        let mut pan = 0.0;
        let mut draw = |events: Vec<egui::Event>| {
            let mut tempo_rect = Rect::NOTHING;
            let mut pan_rect = Rect::NOTHING;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        tempo_rect = bpm_picker(ui, "fixture_tempo", &mut bpm).rect;
                        pan_rect = pan_control(ui, 0, &mut pan).rect;
                    });
                },
            );
            (output, tempo_rect, pan_rect)
        };
        let (_, tempo, _) = draw(vec![]);
        let mouse = |pos, pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        draw(mouse(tempo.center(), true));
        draw(mouse(tempo.center(), false));
        let (output, _, _) = draw(vec![]);
        let choice = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "180"
                {
                    Some(Rect::from_min_size(text.pos, text.galley.size()).center())
                } else {
                    None
                }
            })
            .expect("The open tempo menu must offer 180 BPM.");
        draw(mouse(choice, true));
        draw(mouse(choice, false));
        let (_, _, pan_rect) = draw(vec![]);
        draw(mouse(
            Pos2::new(pan_rect.right() - 2.0, pan_rect.center().y),
            true,
        ));
        draw(mouse(
            Pos2::new(pan_rect.right() - 2.0, pan_rect.center().y),
            false,
        ));
        assert_eq!(bpm, 180.0);
        assert!(pan > 0.95 && pan <= 1.0);
    }
    #[test]
    fn unavailable_library_rejects_undo_atomically_until_the_source_returns() {
        let (_, mut app) = fixture();
        let original_library = app.library.clone();
        app.selected_clips = [1, 2].into_iter().collect();
        app.delete_selection();
        let empty_project = app.editor.project.clone();
        assert!(empty_project.clips.is_empty());
        let mut removed = (*app.library).clone();
        removed.remove_product(Product::T2).unwrap();
        app.library = Arc::new(removed);
        let removed_library = app.library.clone();
        app.history_step(false);
        assert!(app.status_error);
        assert!(app.status.contains("sample 1 is unavailable"));
        assert_eq!(app.editor.project, empty_project);
        assert!(Arc::ptr_eq(&removed_library, &app.library));
        assert!(
            !app.editor.redo(),
            "A rejected undo must not leave a redo entry."
        );
        app.library = original_library;
        app.history_step(false);
        assert_eq!(app.editor.project.clips.len(), 2);
        assert!(app.library.sample_by_id(1).is_some());
        app.history_step(true);
        assert!(app.editor.project.clips.is_empty());
    }

    #[test]
    fn full_catalog_frame_paints_only_visible_sample_rows() {
        let (ctx, mut app) = fixture();
        let base = app.library.samples[0].clone();
        let samples = (0..31_620)
            .map(|index| {
                let mut sample = base.clone();
                sample.id = index + 1;
                sample.name = format!("Corpus sample {index}");
                sample.group_name = "Kick".into();
                sample
            })
            .collect();
        app.library = Arc::new(Library::fixture(samples));
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let painted_samples = output.shapes.iter().filter(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.starts_with("Corpus"))
        }).count();
        assert_eq!(app.palette_indices().len(), 31_620);
        assert!(
            painted_samples > 0 && painted_samples < 100,
            "A 31,620-sound catalog must paint the visible rows, not every sample: {painted_samples}"
        );
    }
    #[test]
    fn sparse_tracks_keep_model_ids_for_drag_copy_selection_and_mixer() {
        let (ctx, mut app) = fixture();
        app.editor
            .transact(|project| {
                project.clips[1].track = 63;
                project.tracks[31].gain = 1.25;
                project.tracks[31].pan = 0.4;
                project.tracks[31].left_enabled = false;
            })
            .unwrap();
        let hidden = app.editor.project.tracks[31].clone();
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.timeline_tracks, vec![0, 63, 1]);
        assert_eq!(app.timeline_add_track, Some(1));
        let canvas = app.timeline_rect.unwrap();
        let first = app.clip_rect(&app.editor.project.clips[0], canvas).center();
        let second = app.clip_rect(&app.editor.project.clips[1], canvas).center();
        assert!((second.y - first.y - app.timeline_row_height).abs() < 0.1);
        let target = Pos2::new(first.x + 3.0 * app.timeline_bar_width, second.y);
        click(&ctx, &mut app, first, egui::Modifiers::NONE);
        pointer(&ctx, &mut app, first, true, egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
        let moved = app
            .editor
            .project
            .clips
            .iter()
            .find(|clip| clip.id == 1)
            .unwrap();
        assert_eq!(moved.track, 63);
        assert_eq!(moved.start_tick, 3 * TICKS_PER_BAR);
        app.history_step(false);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let canvas = app.timeline_rect.unwrap();
        let first = app.clip_rect(&app.editor.project.clips[0], canvas).center();
        let second = app.clip_rect(&app.editor.project.clips[1], canvas).center();
        let target = Pos2::new(first.x + 3.0 * app.timeline_bar_width, second.y);
        click(&ctx, &mut app, first, egui::Modifiers::NONE);
        pointer(&ctx, &mut app, first, true, egui::Modifiers::ALT);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::ALT,
        );
        pointer(&ctx, &mut app, target, false, egui::Modifiers::ALT);
        assert_eq!(app.editor.project.clips.len(), 3);
        assert_eq!(app.editor.project.clips[0].track, 0);
        assert!(
            app.editor
                .project
                .clips
                .iter()
                .any(|clip| clip.track == 63 && clip.start_tick == 3 * TICKS_PER_BAR)
        );
        app.history_step(false);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let canvas = app.timeline_rect.unwrap();
        let origin = canvas.min + Vec2::splat(1.0);
        let target = Pos2::new(
            canvas.left() + 2.5 * app.timeline_bar_width,
            canvas.top() + 2.0 * app.timeline_row_height - 1.0,
        );
        pointer(&ctx, &mut app, origin, true, egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(target)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, target, false, egui::Modifiers::NONE);
        assert_eq!(app.selected_clips, [1, 2].into_iter().collect());
        let visible = app.timeline_drop_rect.unwrap();
        let second_y = canvas.top() + 1.5 * app.timeline_row_height;
        click(
            &ctx,
            &mut app,
            Pos2::new(visible.left() - 70.0, second_y),
            egui::Modifiers::NONE,
        );
        assert!(!app.editor.project.tracks[63].left_enabled);
        assert!(app.editor.project.tracks[1].left_enabled);
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 350.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.mixer(ui));
            },
        );
        let labels: BTreeSet<_> = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    Some(text.galley.job.text.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(labels.contains("1") && labels.contains("64"));
        assert!(!labels.contains("2") && !labels.contains("32"));
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let canvas = app.timeline_rect.unwrap();
        let add = Pos2::new(
            canvas.left() + 3.5 * app.timeline_bar_width,
            canvas.top() + 2.5 * app.timeline_row_height,
        );
        app.selected_sample = Some(1);
        click(&ctx, &mut app, add, egui::Modifiers::NONE);
        assert!(app.editor.project.clips.iter().any(|clip| clip.track == 1));
        assert_eq!(
            app.editor
                .project
                .clips
                .iter()
                .find(|clip| clip.id == 2)
                .unwrap()
                .track,
            63
        );
        assert_eq!(app.editor.project.tracks[31], hidden);
        assert_eq!(app.editor.project.tracks.len(), 64);
    }

    #[test]
    fn empty_project_record_chooser_can_create_track_64_without_empty_rows() {
        let (ctx, mut app) = fixture();
        app.new_mix();
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.timeline_tracks, vec![0]);
        assert_eq!(app.timeline_add_track, Some(0));
        assert!(occupied_tracks(&app.editor.project).is_empty());
        assert!((app.timeline_rect.unwrap().height() - app.timeline_row_height).abs() < 0.1);
        let menu_ctx = egui::Context::default();
        configure_style(&menu_ctx);
        let mut selected = 0;
        let mut menu = |events: Vec<egui::Event>| {
            let mut rect = Rect::NOTHING;
            let output = menu_ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        rect = recording_destination(ui, &mut selected, &[]).rect;
                    });
                },
            );
            (output, rect)
        };
        let (_, rect) = menu(vec![]);
        let mouse = |pos, pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        menu(mouse(rect.center(), true));
        menu(mouse(rect.center(), false));
        let (output, _) = menu(vec![]);
        let first = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "New track 1"
                {
                    Some(Rect::from_min_size(text.pos, text.galley.size()).center())
                } else {
                    None
                }
            })
            .unwrap();
        menu(vec![
            egui::Event::PointerMoved(first),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -5000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        for _ in 0..5 {
            menu(vec![]);
        }
        let (output, _) = menu(vec![]);
        let last = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "New track 64"
                {
                    let pos = Rect::from_min_size(text.pos, text.galley.size()).center();
                    shape.clip_rect.contains(pos).then_some(pos)
                } else {
                    None
                }
            })
            .expect("The recording chooser must reach unused track 64.");
        menu(vec![egui::Event::PointerMoved(last)]);
        menu(mouse(last, true));
        menu(mouse(last, false));
        assert_eq!(selected, 63);
        app.armed_track = selected;
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(app.timeline_tracks, vec![63]);
        assert_eq!(app.timeline_add_track, Some(63));
        let canvas = app.timeline_rect.unwrap();
        app.selected_sample = Some(1);
        let drop = canvas.min + Vec2::new(30.0, app.timeline_row_height / 2.0);
        click(&ctx, &mut app, drop, egui::Modifiers::NONE);
        assert_eq!(app.editor.project.clips.len(), 1);
        assert_eq!(app.editor.project.clips[0].track, 63);
        assert_eq!(app.editor.project.tracks.len(), 64);
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert_eq!(occupied_tracks(&app.editor.project), vec![63]);
        assert!(app.timeline_tracks.len() <= 2);
    }
    #[test]
    fn dropping_a_group_below_the_add_target_cancels_without_deleting_or_history() {
        let (ctx, mut app) = fixture();
        app.selected_clips = [1, 2].into_iter().collect();
        app.selected_clip = Some(1);
        let before = app.editor.project.clone();
        let revision = app.revision;
        let canvas = app.timeline_rect.unwrap();
        let first = app.clip_rect(&app.editor.project.clips[0], canvas).center();
        let below = Pos2::new(first.x, canvas.bottom() + 15.0);
        pointer(&ctx, &mut app, first, true, egui::Modifiers::NONE);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(below)],
            egui::Modifiers::NONE,
        );
        pointer(&ctx, &mut app, below, false, egui::Modifiers::NONE);
        assert_eq!(app.editor.project, before);
        assert_eq!(app.selected_clips, [1, 2].into_iter().collect());
        assert_eq!(app.selected_clip, Some(1));
        assert_eq!(app.revision, revision);
        assert!(app.status.contains("Move canceled"));
        assert!(
            !app.editor.undo(),
            "Canceled movement must not add an edit-history entry."
        );
    }

    #[test]
    fn rail_icons_keep_tooltips_accessible_actions_and_cached_textures() {
        let (ctx, mut app) = fixture();
        let icons = app.rail_icons.as_ref().unwrap().as_ref().unwrap().clone();
        let image_center = |output: &egui::FullOutput, icon: RailIcon| {
            output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Mesh(mesh) = &shape.shape
                        && mesh.texture_id == icons.texture_id(icon)
                    {
                        Some(mesh.calc_bounds().center())
                    } else {
                        None
                    }
                })
                .expect("The rail must paint its bundled icon texture.")
        };
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            !output.shapes.iter().any(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    let label = text.galley.job.text.as_str();
                    text.pos.x > 1364.0
                        && text.pos.y > 50.0
                        && (RailIcon::ALL.iter().any(|icon| icon.label() == label)
                            || matches!(label, "MIX" | "Record"))
                } else {
                    false
                }
            }),
            "Rail action names must not be painted beside the icons."
        );
        let mixer = image_center(&output, RailIcon::Mixer);
        ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(mixer)],
            egui::Modifiers::NONE,
        );
        let hovered = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        assert!(
            hovered.shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Mixer")
            }),
            "The icon must preserve its action name in a hover tooltip."
        );
        pointer(&ctx, &mut app, mixer, true, egui::Modifiers::NONE);
        let clicked = frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerButton {
                pos: mixer,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            egui::Modifiers::NONE,
        );
        assert!(app.show_mixer);
        assert!(clicked.platform_output.events.iter().any(|event| {
            matches!(event, egui::output::OutputEvent::Clicked(info) if info.label.as_deref() == Some("Mixer"))
        }), "The icon must expose its action name to accessibility output.");
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let erase = image_center(&output, RailIcon::Erase);
        click(&ctx, &mut app, erase, egui::Modifiers::NONE);
        assert!(app.erase);
        app.selected_clips = [1, 2].into_iter().collect();
        app.selected_clip = Some(1);
        let output = frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
        let delete = image_center(&output, RailIcon::Delete);
        click(&ctx, &mut app, delete, egui::Modifiers::NONE);
        assert!(app.editor.project.clips.is_empty());
        app.history_step(false);
        assert_eq!(app.editor.project.clips.len(), 2);
        let cached = app.rail_icons.as_ref().unwrap().as_ref().unwrap();
        assert!(
            Arc::ptr_eq(&icons, cached),
            "Rail textures must be reused across frames and actions."
        );
    }
}
