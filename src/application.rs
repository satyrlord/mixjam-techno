//! Headless application boundary. UI clients read immutable views and submit typed commands.
pub use crate::{
    audio::Pcm,
    effects::EffectsState,
    engine::{Clip, Project, SUPPORTED_BPMS, TICKS_PER_BAR, TRACK_COUNT, Track},
    library::Sample,
    products::Product,
    synth::StepSynthState,
};
use crate::{
    audio::{self, Player},
    engine::Editor,
    library::Library,
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};

/// A read-only catalog projection. It cannot decode, register, or remove sources.
#[derive(Clone, Copy)]
pub struct LibraryView<'a> {
    pub samples: &'a [Sample],
    library: &'a Library,
}
impl<'a> LibraryView<'a> {
    pub fn sample_by_id(self, id: u32) -> Option<&'a Sample> {
        self.library.sample_by_id(id)
    }
    pub fn bindings(self) -> &'a [crate::products::LibraryBinding] {
        self.library.bindings()
    }
    pub fn wav_root(self) -> Option<&'a Path> {
        self.library.wav_root()
    }
    pub fn source_root(self, product: Product) -> Option<&'a Path> {
        self.library.source_root(product)
    }
}

#[derive(Clone, Debug)]
pub enum EditCommand {
    SetTempo(f64),
    SetLoop(Option<[u32; 2]>),
    SetMixer {
        tracks: Vec<Track>,
        master_gain: f32,
    },
    SetClip {
        id: u64,
        gain: f32,
        offset_frames: u32,
        length_ticks: u32,
    },
    SetRouting {
        track: usize,
        left: bool,
        enabled: bool,
    },
    DeleteClips(Vec<u64>),
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

/// Owns project history, sources, audio transport, rendering, recording, and persistence.
/// No mutable project, catalog, device, or renderer is exposed to a frontend.
pub struct Session {
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
    file: Option<PathBuf>,
    unsaved: bool,
    notice: Option<(String, bool)>,
    record_track: usize,
    record_bpm: f64,
    recorder: Option<crate::recording::Recorder>,
    record_start_tick: u32,
    record_mix_started: bool,
    recorded_take: Option<Pcm>,
}

impl Session {
    pub fn headless(library: Arc<Library>, project: Project) -> Result<Self, String> {
        project.validate()?;
        Ok(Self {
            library,
            editor: Editor::new(project),
            player: None,
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
            file: None,
            unsaved: false,
            notice: None,
            record_track: 0,
            record_bpm: 140.0,
            recorder: None,
            record_start_tick: 0,
            record_mix_started: false,
            recorded_take: None,
        })
    }
    pub fn new(library: Arc<Library>, project: Project) -> Result<Self, String> {
        let mut session = Self::headless(library, project)?;
        match Player::new() {
            Ok(player) => session.player = Some(player),
            Err(error) => session.error(format!("Audio output is unavailable: {error}")),
        }
        Ok(session)
    }
    pub fn project(&self) -> &Project {
        &self.editor.project
    }
    pub fn library(&self) -> LibraryView<'_> {
        LibraryView {
            samples: &self.library.samples,
            library: &self.library,
        }
    }
    pub fn cursor_tick(&self) -> u32 {
        self.cursor_tick
    }
    pub fn wants_playback(&self) -> bool {
        self.wants_playback
    }
    pub fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }
    pub fn has_recorded_take(&self) -> bool {
        self.recorded_take.is_some()
    }
    pub fn unsaved(&self) -> bool {
        self.unsaved
    }
    pub fn needs_save_confirmation(&self) -> bool {
        self.unsaved || self.recorded_take.is_some()
    }
    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }
    pub fn record_bpm(&self) -> f64 {
        self.record_bpm
    }
    pub fn take_notice(&mut self) -> Option<(String, bool)> {
        self.notice.take()
    }
    fn message(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), false));
    }
    fn error(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), true));
    }
    fn changed(&mut self) {
        self.unsaved = true;
        self.render_dirty = true;
        self.revision = self.revision.wrapping_add(1);
        if self.wants_playback {
            self.start_playback();
        }
    }
    pub fn edit(&mut self, command: EditCommand) -> Result<(), String> {
        // Validate identifiers before a transaction. A stale UI command must fail visibly.
        match &command {
            EditCommand::SetClip {
                id, offset_frames, ..
            } => {
                let clip = self
                    .project()
                    .clips
                    .iter()
                    .find(|clip| clip.id == *id)
                    .ok_or("The clip does not exist.")?;
                let sample = self
                    .library
                    .sample_by_id(clip.sample_id)
                    .ok_or("The sample is unavailable.")?;
                if *offset_frames > sample.frame_count.saturating_sub(1) {
                    return Err("The trim start must be within the source audio.".into());
                }
            }
            EditCommand::SetRouting { track, .. } if *track >= TRACK_COUNT => {
                return Err("The track is outside the project.".into());
            }
            EditCommand::DeleteClips(ids)
                if ids
                    .iter()
                    .any(|id| !self.project().clips.iter().any(|c| c.id == *id)) =>
            {
                return Err("A selected clip does not exist.".into());
            }
            _ => {}
        }
        self.editor.transact(|project| match command {
            EditCommand::SetTempo(bpm) => project.bpm = bpm,
            EditCommand::SetLoop(region) => project.loop_region = region,
            EditCommand::SetMixer {
                tracks,
                master_gain,
            } => {
                project.tracks = tracks;
                project.master_gain = master_gain;
            }
            EditCommand::SetClip {
                id,
                gain,
                offset_frames,
                length_ticks,
            } => {
                if let Some(clip) = project.clips.iter_mut().find(|clip| clip.id == id) {
                    clip.gain = gain;
                    clip.offset_frames = offset_frames;
                    clip.length_ticks = length_ticks;
                }
            }
            EditCommand::SetRouting {
                track,
                left,
                enabled,
            } => {
                if left {
                    project.tracks[track].left_enabled = enabled;
                } else {
                    project.tracks[track].right_enabled = enabled;
                }
            }
            EditCommand::DeleteClips(ids) => {
                let ids: BTreeSet<_> = ids.into_iter().collect();
                project.clips.retain(|clip| !ids.contains(&clip.id));
            }
        })?;
        self.changed();
        Ok(())
    }
    pub fn place(&mut self, sample_id: u32, track: usize, tick: u32) -> Result<u64, String> {
        let sample = self
            .library
            .sample_by_id(sample_id)
            .ok_or("The sample is unavailable.")?;
        let length = if Product::sample_key(sample_id).is_some() {
            sample.bars.saturating_mul(TICKS_PER_BAR)
        } else {
            duration_ticks(
                u64::from(sample.frame_count),
                sample.sample_rate,
                sample.source_bpm,
            )
        };
        let id = self.editor.place(sample_id, track, tick, length)?;
        self.changed();
        Ok(id)
    }
    pub fn delete(&mut self, id: u64) -> Result<(), String> {
        self.edit(EditCommand::DeleteClips(vec![id]))
    }
    pub fn move_clips(
        &mut self,
        ids: &[u64],
        track_delta: i32,
        tick_delta: i64,
        copy: bool,
    ) -> Result<Vec<u64>, String> {
        let ids = self.editor.move_clips(ids, track_delta, tick_delta, copy)?;
        self.changed();
        Ok(ids)
    }
    pub fn history_step(&mut self, redo: bool) -> Result<bool, String> {
        if self.is_recording() {
            return Err("Stop recording before you undo or redo.".into());
        }
        let changed = if redo {
            self.editor.redo()
        } else {
            self.editor.undo()
        };
        if !changed {
            return Ok(false);
        }
        let mut library = (*self.library).clone();
        let result = library.apply_imports(&self.project().imported_samples).and_then(|()| {
            if let Some(clip) = self.project().clips.iter().find(|clip| library.sample_by_id(clip.sample_id).is_none()) {
                return Err(format!("Cannot restore this edit because sample {} is unavailable. Add its sample library and try again.", clip.sample_id));
            } Ok(())
        });
        if let Err(error) = result {
            if redo {
                self.editor.undo();
            } else {
                self.editor.redo();
            }
            return Err(error);
        }
        self.editor.project.libraries = library.bindings().to_vec();
        self.library = Arc::new(library);
        self.changed();
        Ok(true)
    }
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        self.require_idle_recording()?;
        let (project, library) = crate::project_io::open(path, &self.library)?;
        self.stop();
        self.library = Arc::new(library);
        self.editor = Editor::new(project);
        self.revision = self.revision.wrapping_add(1);
        self.render = None;
        self.render_dirty = true;
        self.unsaved = false;
        self.file = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            .then(|| path.to_path_buf());
        Ok(())
    }
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        crate::project_io::save(self.project(), path)?;
        self.file = Some(path.to_path_buf());
        self.unsaved = false;
        Ok(())
    }
    pub fn export(&self, path: &Path) -> Result<(), String> {
        audio::render_project(self.project(), &self.library)?.write_wav(path)
    }
    pub fn new_mix(&mut self) -> Result<(), String> {
        self.require_idle_recording()?;
        self.stop();
        self.editor = Editor::new(Project {
            libraries: self.library.bindings().to_vec(),
            imported_samples: self.library.imported_samples(),
            ..Project::default()
        });
        self.revision = self.revision.wrapping_add(1);
        self.render = None;
        self.file = None;
        self.render_dirty = true;
        self.unsaved = false;
        Ok(())
    }
    fn require_idle_recording(&self) -> Result<(), String> {
        if self.is_recording() {
            Err("Stop recording before you open, close, or start a new mix.".into())
        } else {
            Ok(())
        }
    }
    fn commit_library(&mut self, library: Library) -> Result<(), String> {
        let bindings = library.bindings().to_vec();
        let mut checked = self.project().clone();
        checked.libraries = bindings.clone();
        checked.validate()?;
        let persisted: Vec<_> = bindings
            .iter()
            .filter(|binding| library.wav_root() != Some(binding.root.as_path()))
            .cloned()
            .collect();
        crate::project_io::save_library_config(&persisted)?;
        self.editor.project.libraries = bindings;
        self.library = Arc::new(library);
        self.changed();
        Ok(())
    }
    pub fn add_library(&mut self, product: Product, root: &Path) -> Result<(), String> {
        let mut library = (*self.library).clone();
        library.register(product, root, &crate::project_io::reference_samples_root())?;
        self.commit_library(library)
    }
    pub fn remove_library(&mut self, product: Product) -> Result<(), String> {
        if self.project().clips.iter().any(|clip| {
            Product::sample_key(clip.sample_id).is_some_and(|(source, _)| source == product)
        }) {
            return Err(format!(
                "This mix uses {} samples. Start a new mix before you remove this library.",
                product.label()
            ));
        }
        let mut library = (*self.library).clone();
        library.remove_product(product)?;
        self.commit_library(library)
    }
    pub fn select_wave_folder(&mut self, root: &Path, source_bpm: f64) -> Result<(), String> {
        if self.is_recording() {
            return Err("Stop recording before you change the sample folder.".into());
        }
        let library = Library::open_wav_folder(root, source_bpm)?;
        let (project, library) = crate::project_io::resolve(self.project().clone(), &library)?;
        crate::project_io::save_wave_config(root, source_bpm)?;
        self.stop();
        self.library = Arc::new(library);
        self.editor = Editor::new(project);
        self.changed();
        Ok(())
    }
    pub fn register_file(
        &mut self,
        path: &Path,
        group: &str,
        source_bpm: f64,
    ) -> Result<u32, String> {
        let mut library = (*self.library).clone();
        let imported = library.import_wav_with_tempo(path, None, group, source_bpm)?;
        let id = imported.id;
        self.editor.transact(|project| {
            if let Some(existing) = project
                .imported_samples
                .iter_mut()
                .find(|sample| sample.id == id)
            {
                *existing = imported;
            } else {
                project.imported_samples.push(imported);
            }
        })?;
        self.library = Arc::new(library);
        self.changed();
        Ok(id)
    }
    pub fn save_pcm(
        &mut self,
        pcm: &Pcm,
        path: &Path,
        group: &str,
        source_bpm: f64,
    ) -> Result<u32, String> {
        pcm.validate()?;
        crate::engine::validate_import_metadata(&crate::engine::ImportedSample {
            id: crate::products::IMPORTED_ID_START,
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .trim()
                .to_string(),
            group_name: group.trim().to_string(),
            path: path.to_path_buf(),
            source_bpm,
            origin: None,
        })?;
        if pcm.frames() == 0 {
            return Err("The audio source is empty.".into());
        }
        pcm.write_wav(path)?;
        self.register_file(path, group, source_bpm)
    }
    pub fn source_pcm(&self, id: Option<u32>) -> Result<Pcm, String> {
        self.library
            .read_pcm(id.ok_or("Choose a sound in the palette first.")?)
    }
    pub fn source_bpm(&self, id: Option<u32>) -> f64 {
        id.and_then(|id| self.library.sample_by_id(id))
            .map_or(140.0, |sample| sample.source_bpm)
    }
    pub fn processed_pcm(&self, id: Option<u32>, effects: &EffectsState) -> Result<Pcm, String> {
        crate::effects::process(&self.source_pcm(id)?, effects, self.source_bpm(id))
    }
    pub fn step_synth_pcm(
        &self,
        id: Option<u32>,
        path: Option<&Path>,
        state: &StepSynthState,
    ) -> Result<Pcm, String> {
        let source = if let Some(path) = path {
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
            {
                audio::read_wav(path)?
            } else {
                crate::library::read_packed_file(path)?
            }
        } else {
            self.source_pcm(id)?
        };
        crate::synth::render(&source, state)
    }
    pub fn edited_pcm(
        &self,
        id: Option<u32>,
        start: u64,
        end: u64,
        pitch: f64,
    ) -> Result<Pcm, String> {
        crate::sample_edit::pitch_shift(
            &crate::sample_edit::trim(&self.source_pcm(id)?, start, end)?,
            pitch,
        )
    }
    pub fn stretched_pcm(
        &self,
        id: Option<u32>,
        source_bpm: f64,
        target_bpm: f64,
    ) -> Result<Pcm, String> {
        crate::sample_edit::time_stretch(&self.source_pcm(id)?, source_bpm, target_bpm)
    }
    pub fn save_pattern(&self, path: &Path, state: &StepSynthState) -> Result<(), String> {
        state.validate()?;
        let bytes = serde_json::to_vec_pretty(state).map_err(|error| error.to_string())?;
        std::fs::write(path, bytes).map_err(|error| error.to_string())
    }
    pub fn load_pattern(&self, path: &Path) -> Result<StepSynthState, String> {
        use std::io::Read;
        let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
        let mut bytes = Vec::new();
        file.take(1_048_577)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() > 1_048_576 {
            return Err("The pattern file is too large.".into());
        }
        let mut state: StepSynthState =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        state.validate()?;
        state.bpm = crate::engine::nearest_supported_bpm(state.bpm)?;
        Ok(state)
    }
    pub fn playing_mix(&self) -> bool {
        !self.previewing && self.player.as_ref().is_some_and(Player::is_playing)
    }

    fn tick_to_frame(&self, tick: u32, rate: u32) -> u64 {
        (tick as f64 * self.editor.project.frames_per_tick(rate)).round() as u64
    }

    pub fn start_playback(&mut self) {
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

    fn poll_render(&mut self) {
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

    pub fn pause(&mut self) {
        self.wants_playback = false;
        self.render_worker.cancel_pending();
        self.requested_revision = None;
        if let Some(player) = &mut self.player {
            player.pause();
        }
        self.previewing = false;
        self.message("Paused.");
    }

    pub fn seek(&mut self, tick: u32) {
        let was_playing = self.wants_playback;
        self.cursor_tick = tick;
        self.seek_pending = true;
        if was_playing {
            self.start_playback();
        }
    }

    pub fn preview(&mut self, sample_id: u32) {
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

    pub fn play_pcm(&mut self, pcm: Pcm, label: &str) {
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
    pub fn stop(&mut self) {
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
                Ok(pcm) => self.recorded_take = Some(pcm),
                Err(error) => self.error(error),
            }
        } else {
            self.message("Stopped.");
        }
    }
    /// Advances devices and the render queue. Returns a suggested polling delay in milliseconds.
    pub fn poll(&mut self) -> Option<u64> {
        self.poll_render();
        if self.wants_playback
            && !self.render_dirty
            && self.requested_revision.is_none()
            && !self.playing_mix()
        {
            self.wants_playback = false;
        }
        if self.playing_mix()
            && !self.seek_pending
            && let Some(player) = &self.player
        {
            self.cursor_tick =
                (player.position_frames() as f64 / self.playback_frames_per_tick) as u32;
        }
        self.poll_recording();
        if self.render_worker.in_flight {
            Some(20)
        } else if self.playing_mix() {
            Some(30)
        } else if self.is_recording() {
            Some(40)
        } else {
            None
        }
    }
    pub fn start_recording(&mut self, track: usize) -> Result<(), String> {
        if self.is_recording() {
            return Err("Recording has already started.".into());
        }
        if self.recorded_take.is_some() {
            return Err("Save or discard the previous recorded take first.".into());
        }
        if track >= TRACK_COUNT {
            return Err("The track is outside the project.".into());
        }
        let recorder = crate::recording::Recorder::start(self.project().bpm)?;
        self.record_start_tick = self.cursor_tick;
        self.record_track = track;
        self.record_bpm = self.project().bpm;
        self.record_mix_started = false;
        self.recorder = Some(recorder);
        match crate::recording::count_in(self.record_bpm) {
            Ok(pcm) => self.play_pcm(
                pcm,
                "Recording count-in: eight beats. Press Stop to finish.",
            ),
            Err(error) => self.error(error),
        }
        Ok(())
    }
    fn poll_recording(&mut self) {
        let Some(recorder) = &self.recorder else {
            return;
        };
        let status = recorder.status();
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
                if !self.project().clips.is_empty() {
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
    pub fn preview_recorded_take(&mut self) {
        if let Some(pcm) = self.recorded_take.clone() {
            self.play_pcm(pcm, "Listening to the recorded take.");
        }
    }
    pub fn discard_recorded_take(&mut self) {
        self.recorded_take = None;
    }
    /// Saves the take first. A placement collision retains the saved source in the catalog.
    pub fn save_recorded_take(&mut self, path: &Path) -> Result<Option<u64>, String> {
        let pcm = self
            .recorded_take
            .clone()
            .ok_or("No recorded take is available.")?;
        let id = self.save_pcm(&pcm, path, "Wave", self.record_bpm)?;
        let ticks = duration_ticks(pcm.frames(), pcm.sample_rate, self.record_bpm);
        let placed = self
            .editor
            .place(id, self.record_track, self.record_start_tick, ticks);
        self.recorded_take = None;
        match placed {
            Ok(clip_id) => {
                self.changed();
                Ok(Some(clip_id))
            }
            Err(error) => {
                self.error(format!(
                    "The take is saved in Wave. Cannot place it on the armed track: {error}"
                ));
                Ok(None)
            }
        }
    }
}

fn duration_ticks(frames: u64, rate: u32, bpm: f64) -> u32 {
    (frames as f64 * bpm / rate as f64 / 240.0 * TICKS_PER_BAR as f64)
        .round()
        .max(1.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Session {
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
        Session::headless(library, editor.project).unwrap()
    }
    #[test]
    fn recording_blocks_sample_folder_changes_before_scanning_or_replacing_state() {
        let mut app = fixture();
        app.editor
            .transact(|project| project.title = "Before recording".into())
            .unwrap();
        app.cursor_tick = TICKS_PER_BAR;
        app.recorder = Some(crate::recording::Recorder::fixture());
        let project = app.editor.project.clone();
        let library = app.library.clone();
        let revision = app.revision;
        let unsaved = app.unsaved;

        // An empty path cannot be scanned. The recording guard must run first.
        let error = app.select_wave_folder(Path::new(""), 140.0).unwrap_err();

        assert_eq!(error, "Stop recording before you change the sample folder.");
        assert_eq!(app.editor.project, project);
        assert!(Arc::ptr_eq(&app.library, &library));
        assert_eq!(app.revision, revision);
        assert_eq!(app.unsaved, unsaved);
        assert_eq!(app.cursor_tick, TICKS_PER_BAR);
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
    fn render_worker_keeps_only_the_latest_pending_revision_and_cancels_it() {
        let app = fixture();
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
        let mut app = fixture();
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
        app.poll_render();
        assert!(!app.wants_playback);
        assert!(
            app.render.is_some(),
            "A paused render may be cached without starting an audio device."
        );
        app.new_mix().unwrap();
        results
            .send(RenderResult {
                revision: 5,
                frames_per_tick: 1.0,
                pcm: Ok(pcm),
            })
            .unwrap();
        app.poll_render();
        assert!(
            app.render.is_none(),
            "A previous project's late result must be discarded."
        );
        assert!(app.render_dirty);
    }

    #[test]
    fn unavailable_library_rejects_undo_atomically_until_the_source_returns() {
        let mut app = fixture();
        let original_library = app.library.clone();
        app.edit(EditCommand::DeleteClips(vec![1, 2])).unwrap();
        let empty_project = app.editor.project.clone();
        assert!(empty_project.clips.is_empty());
        let mut removed = (*app.library).clone();
        removed.remove_product(Product::T2).unwrap();
        app.library = Arc::new(removed);
        let removed_library = app.library.clone();
        let error = app.history_step(false).unwrap_err();
        assert!(error.contains("sample 1 is unavailable"));
        assert_eq!(app.editor.project, empty_project);
        assert!(Arc::ptr_eq(&removed_library, &app.library));
        assert!(
            !app.editor.redo(),
            "A rejected undo must not leave a redo entry."
        );
        app.library = original_library;
        app.history_step(false).unwrap();
        assert_eq!(app.editor.project.clips.len(), 2);
        assert!(app.library.sample_by_id(1).is_some());
        app.history_step(true).unwrap();
        assert!(app.editor.project.clips.is_empty());
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
    fn fixture_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "mixjam-session-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn constructors_reject_invalid_projects_before_commands_can_index_them() {
        for project in [
            Project {
                tracks: vec![],
                ..Project::default()
            },
            Project {
                bpm: 146.0,
                ..Project::default()
            },
        ] {
            assert!(Session::headless(Arc::new(Library::empty()), project).is_err());
        }
    }
    #[test]
    fn invalid_typed_commands_preserve_project_history_and_dirty_state() {
        let mut session =
            Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
        let before = session.project().clone();
        for command in [
            EditCommand::SetTempo(146.0),
            EditCommand::SetLoop(Some([256, 256])),
            EditCommand::SetRouting {
                track: TRACK_COUNT,
                left: true,
                enabled: false,
            },
            EditCommand::SetMixer {
                tracks: vec![],
                master_gain: 1.0,
            },
            EditCommand::DeleteClips(vec![5]),
        ] {
            assert!(session.edit(command).is_err());
            assert_eq!(session.project(), &before);
            assert!(!session.unsaved());
            assert!(!session.history_step(false).unwrap());
        }
        session.edit(EditCommand::SetTempo(160.0)).unwrap();
        assert_eq!(session.project().bpm, 160.0);
        assert!(session.unsaved());
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project(), &before);
    }
    #[test]
    fn clip_trim_bounds_reject_silent_out_of_source_edits_atomically() {
        let dir = fixture_dir();
        let wave = dir.join("source.wav");
        Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.5; 1000],
        }
        .write_wav(&wave)
        .unwrap();
        let mut session =
            Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
        let source = session.register_file(&wave, "Wave", 140.0).unwrap();
        let clip = session.place(source, 0, 0).unwrap();
        session.save(&dir.join("project.json")).unwrap();
        let before = session.project().clone();
        let revision = session.revision;
        let command = |offset| EditCommand::SetClip {
            id: clip,
            gain: 1.0,
            offset_frames: offset,
            length_ticks: before.clips[0].length_ticks,
        };
        assert!(session.edit(command(1000)).is_err());
        assert_eq!(session.project(), &before);
        assert!(!session.unsaved());
        assert_eq!(session.revision, revision);
        session.edit(command(999)).unwrap();
        assert_eq!(session.project().clips[0].offset_frames, 999);
        session.history_step(false).unwrap();
        assert_eq!(session.project(), &before);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn headless_session_import_save_reopen_render_and_processing_work_without_gui() {
        let dir = fixture_dir();
        let wave = dir.join("source.wav");
        let project = dir.join("song.json");
        Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.25; 1000],
        }
        .write_wav(&wave)
        .unwrap();
        let mut session =
            Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
        let source = session.register_file(&wave, "Wave", 90.0).unwrap();
        session.place(source, 63, 0).unwrap();
        session.edit(EditCommand::SetTempo(160.0)).unwrap();
        session.save(&project).unwrap();
        let expected = session.project().clone();
        session.new_mix().unwrap();
        session.open(&project).unwrap();
        assert_eq!(session.project(), &expected);
        assert_eq!(
            session.library().sample_by_id(source).unwrap().source_bpm,
            90.0
        );
        session.export(&dir.join("export.wav")).unwrap();
        let rendered = audio::read_wav(&dir.join("export.wav")).unwrap();
        assert!(rendered.samples.iter().any(|sample| sample.abs() > 0.1));
        assert_eq!(
            session
                .edited_pcm(Some(source), 10, 20, 0.0)
                .unwrap()
                .frames(),
            10
        );
        assert_eq!(
            session
                .processed_pcm(Some(source), &EffectsState::default())
                .unwrap()
                .frames(),
            1000
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn frontend_source_can_only_reach_the_application_boundary() {
        for (name, source) in [
            ("ui.rs", include_str!("ui.rs")),
            ("ui/view.rs", include_str!("ui/view.rs")),
        ] {
            for forbidden in [
                ".editor",
                ".transact(",
                "crate::audio",
                "crate::engine",
                "crate::library",
                "crate::project_io",
                "crate::recording",
                "crate::effects",
                "crate::synth",
                "crate::sample_edit",
                "std::fs::",
                "serde_json::",
                "render_project(",
                "read_pcm(",
                "Recorder::",
                "Player::",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "{name} bypasses the application boundary through {forbidden}"
                );
            }
            for line in source
                .lines()
                .filter(|line| line.trim_start().starts_with("use mixjam_techno::"))
            {
                assert!(
                    line.contains("use mixjam_techno::application::"),
                    "{name} imports a backend implementation directly"
                );
            }
        }
        let library = include_str!("lib.rs");
        assert!(
            !library.contains("pub mod ui;"),
            "The production library must remain headless."
        );
    }

    #[test]
    fn invalid_audio_save_metadata_never_replaces_the_selected_file() {
        let dir = fixture_dir();
        let target = dir.join("existing.wav");
        std::fs::write(&target, b"unchanged").unwrap();
        let pcm = Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.5; 10],
        };
        let mut session =
            Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
        let before = session.project().clone();
        for (group, bpm) in [("Wave", f64::NAN), ("", 140.0)] {
            assert!(session.save_pcm(&pcm, &target, group, bpm).is_err());
            assert_eq!(std::fs::read(&target).unwrap(), b"unchanged");
            assert_eq!(session.project(), &before);
            assert!(session.library().samples.is_empty());
            assert!(!session.unsaved());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unsaved_mix_and_take_require_a_pending_action_before_replacement() {
        let mut session =
            Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
        assert!(!session.needs_save_confirmation());
        session.edit(EditCommand::SetTempo(160.0)).unwrap();
        assert!(session.needs_save_confirmation());
        session.unsaved = false;
        session.recorded_take = Some(Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.0; 10],
        });
        assert!(session.needs_save_confirmation());
        assert!(session.has_recorded_take());
        session.discard_recorded_take();
        assert!(!session.needs_save_confirmation());
    }
}
