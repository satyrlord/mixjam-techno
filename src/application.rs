//! Headless application boundary. UI clients read immutable views and submit typed commands.
pub use crate::{
    APP_VERSION,
    audio::Pcm,
    effects::EffectsState,
    engine::{
        Clip, MissingReference, MissingSample, Project, SUPPORTED_BPMS, SampleConflict,
        TICKS_PER_BAR, TRACK_COUNT, Track,
    },
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
    pub fn folder_tempos(self) -> &'a [(std::path::PathBuf, f64, usize)] {
        self.library.folder_tempos()
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
    mix: Result<Arc<audio::Mix>, String>,
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
        let (sender, receiver) = Self::spawn();
        Self {
            sender,
            receiver,
            in_flight: false,
            current_revision: None,
            pending: None,
        }
    }

    fn spawn() -> (mpsc::Sender<RenderRequest>, mpsc::Receiver<RenderResult>) {
        let (sender, requests) = mpsc::channel::<RenderRequest>();
        let (results, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("mixjam-techno-render".into())
            .spawn(move || {
                // An edit rarely changes which sounds a mix uses. Keep their
                // decoded audio for the next render of the same library.
                let mut decoded = std::collections::HashMap::new();
                let mut decoded_from = std::sync::Weak::new();
                while let Ok(request) = requests.recv() {
                    let library = Arc::downgrade(&request.library);
                    if !decoded_from.ptr_eq(&library) {
                        decoded.clear();
                        decoded_from = library;
                    }
                    let mix = audio::prepare_project_reusing(
                        &request.project,
                        &request.library,
                        &mut decoded,
                    )
                    .map(Arc::new);
                    if results
                        .send(RenderResult {
                            revision: request.revision,
                            mix,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("Cannot create the audio render worker.");
        (sender, receiver)
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
        let result = match self.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                // The worker died during a render. Report that render as failed
                // and replace the worker, so later requests are not stranded.
                (self.sender, self.receiver) = Self::spawn();
                RenderResult {
                    revision: self.current_revision?,
                    mix: Err("The audio render worker stopped.".into()),
                }
            }
        };
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

/// A sample folder scan running on its own thread. The result replaces the
/// library only when it arrives through `poll`; dropping the job abandons it.
struct WaveScan {
    root: PathBuf,
    source_bpm: f64,
    progress: Arc<crate::library::ScanProgress>,
    receiver: mpsc::Receiver<Result<Library, String>>,
}

/// A mix export running on its own thread.
struct Export {
    path: PathBuf,
    receiver: mpsc::Receiver<Result<(), String>>,
    thread: std::thread::JoinHandle<()>,
}

/// What a frontend may show about a running sample folder scan.
#[derive(Clone, Debug, PartialEq)]
pub struct WaveScanStatus {
    pub root: PathBuf,
    pub files_done: usize,
    pub files_total: usize,
}

/// Owns project history, sources, audio transport, rendering, recording, and persistence.
/// No mutable project, catalog, device, or renderer is exposed to a frontend.
pub struct Session {
    library: Arc<Library>,
    catalog_revision: u64,
    wave_scan: Option<WaveScan>,
    /// A song to open once the startup folder scan has replaced the library.
    pending_open: Option<(PathBuf, u64)>,
    /// The deferred song, once opened, until a frontend reports it.
    opened_after_scan: Option<PathBuf>,
    /// A successful folder adoption, until the frontend resets its source selection.
    wave_folder_adopted: bool,
    export: Option<Export>,
    /// Products whose provider replaces the same product found in a WAV folder.
    preferred_providers: Vec<Product>,
    /// Where the sample folder setting and scan cache live.
    settings_root: PathBuf,
    editor: Editor,
    player: Option<Player>,
    /// The prepared mix of the current revision; it is rendered on demand.
    render: Option<Arc<audio::Mix>>,
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
    recorded_take: Option<Arc<Pcm>>,
    /// The control changed by the last edit, while later edits may continue it.
    gesture: Option<(Gesture, std::time::Instant)>,
}

/// One continuously adjustable control. Edits of the same control in quick
/// succession, such as the frames of a slider drag, form one undo step.
#[derive(Clone, Copy, PartialEq)]
enum Gesture {
    Master,
    Track(usize, TrackControl),
    Clip(u64, ClipControl),
}
#[derive(Clone, Copy, PartialEq)]
enum TrackControl {
    Gain,
    Pan,
}
#[derive(Clone, Copy, PartialEq)]
enum ClipControl {
    Gain,
    Trim,
    Length,
}
const GESTURE_GAP: std::time::Duration = std::time::Duration::from_millis(400);

impl Gesture {
    /// The control a command adjusts, when it adjusts exactly one.
    fn of(command: &EditCommand, project: &Project) -> Option<Self> {
        match command {
            EditCommand::SetClip {
                id,
                gain,
                offset_frames,
                length_ticks,
            } => {
                let clip = project.clips.iter().find(|clip| clip.id == *id)?;
                let mut controls = Vec::new();
                if clip.gain != *gain {
                    controls.push(ClipControl::Gain);
                }
                if clip.offset_frames != *offset_frames {
                    controls.push(ClipControl::Trim);
                }
                if clip.length_ticks != *length_ticks {
                    controls.push(ClipControl::Length);
                }
                match controls[..] {
                    [control] => Some(Self::Clip(*id, control)),
                    _ => None,
                }
            }
            EditCommand::SetMixer {
                tracks,
                master_gain,
            } if tracks.len() == project.tracks.len() => {
                let mut controls = Vec::new();
                for (index, (old, new)) in project.tracks.iter().zip(tracks).enumerate() {
                    if old.left_enabled != new.left_enabled
                        || old.right_enabled != new.right_enabled
                    {
                        return None;
                    }
                    if old.gain != new.gain {
                        controls.push(Self::Track(index, TrackControl::Gain));
                    }
                    if old.pan != new.pan {
                        controls.push(Self::Track(index, TrackControl::Pan));
                    }
                }
                if project.master_gain != *master_gain {
                    controls.push(Self::Master);
                }
                match controls[..] {
                    [control] => Some(control),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl Drop for Session {
    /// Let a running export finish its file before the application exits.
    fn drop(&mut self) {
        if let Some(export) = self.export.take() {
            let _ = export.thread.join();
        }
    }
}

impl Session {
    pub fn headless(library: Arc<Library>, project: Project) -> Result<Self, String> {
        project.validate()?;
        Ok(Self {
            library,
            catalog_revision: 0,
            wave_scan: None,
            pending_open: None,
            opened_after_scan: None,
            wave_folder_adopted: false,
            export: None,
            preferred_providers: Vec::new(),
            settings_root: crate::project_io::data_root(),
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
            gesture: None,
        })
    }
    /// Keep settings and the scan cache in `root` instead of the user's data folder.
    #[cfg(test)]
    pub(crate) fn with_settings_root(mut self, root: PathBuf) -> Self {
        self.settings_root = root;
        self
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
        self.editor.project()
    }
    pub fn missing_samples(&self) -> &[MissingSample] {
        &self.project().missing_samples
    }
    pub fn missing_sample(&self, id: u32) -> Option<&MissingSample> {
        self.project().missing_sample(id)
    }
    pub fn sample_conflicts(&self) -> &[SampleConflict] {
        &self.project().sample_conflicts
    }
    /// Replace every placement of one ambiguous reference with the chosen WAV.
    pub fn resolve_sample_conflict(
        &mut self,
        conflict_index: usize,
        candidate_index: usize,
    ) -> Result<(), String> {
        self.require_idle_recording()?;
        let conflict = self
            .sample_conflicts()
            .get(conflict_index)
            .ok_or("The sample conflict no longer exists.")?
            .clone();
        let path = conflict
            .candidates
            .get(candidate_index)
            .ok_or("The selected sample choice no longer exists.")?;
        let path = std::fs::canonicalize(path)
            .map_err(|error| format!("Cannot open the selected sample: {error}"))?;
        let mut project = self.project().clone();
        let mut next_id = last_imported_id(&project, &self.library);
        for id in &conflict.sample_ids {
            let missing = project.missing_sample(*id);
            let original = project
                .imported_samples
                .iter()
                .find(|sample| sample.id == *id);
            let known = self.library.sample_by_id(*id);
            let (mut imported, previous_rate, preserve_frames) = if let Some(source) = original {
                let rate = source.source_sample_rate.or_else(|| {
                    known
                        .filter(|sample| sample.wav_path.as_deref() == Some(source.path.as_path()))
                        .map(|sample| sample.sample_rate)
                });
                (source.clone(), rate.unwrap_or(44_100), rate.is_none())
            } else if let Some(source) = missing {
                (
                    crate::engine::ImportedSample {
                        id: *id,
                        name: source.name.clone(),
                        group_name: source
                            .metadata
                            .get("group_name")
                            .filter(|name| !name.trim().is_empty())
                            .cloned()
                            .unwrap_or_else(|| "Resolved samples".into()),
                        path: path.clone(),
                        source_bpm: source.source_bpm,
                        source_sample_rate: Some(source.sample_rate),
                        origin: match &source.reference {
                            MissingReference::External { origin, .. } => *origin,
                            _ => None,
                        },
                    },
                    source.sample_rate,
                    source
                        .metadata
                        .get("Trim rate policy")
                        .is_some_and(|policy| policy == "preserve-frames"),
                )
            } else if let Some(source) = known {
                (
                    crate::engine::ImportedSample {
                        id: *id,
                        name: source.name.clone(),
                        group_name: source.group_name.clone(),
                        path: path.clone(),
                        source_bpm: source.source_bpm,
                        source_sample_rate: Some(source.sample_rate),
                        origin: None,
                    },
                    source.sample_rate,
                    false,
                )
            } else {
                return Err("The sample conflict source is unavailable.".into());
            };
            let previous_bpm = imported.source_bpm;
            if *id < crate::products::IMPORTED_ID_START {
                next_id = next_id
                    .checked_add(1)
                    .ok_or("Imported sample ID limit reached.")?;
                imported.id = next_id;
            }
            imported.path = path.clone();
            let ready = Library::check_import(&imported)?;
            if let Some(ticks) = conflict.source_spans.get(id) {
                imported.source_bpm = f64::from(*ticks) * 240.0 * f64::from(ready.sample_rate)
                    / (f64::from(TICKS_PER_BAR) * f64::from(ready.frame_count));
            }
            imported.source_sample_rate = Some(ready.sample_rate);
            for clip in project
                .clips
                .iter_mut()
                .filter(|clip| clip.sample_id == *id)
            {
                let offset = if preserve_frames {
                    f64::from(clip.offset_frames)
                } else {
                    (f64::from(clip.offset_frames) * f64::from(ready.sample_rate)
                        / f64::from(previous_rate)
                        * previous_bpm
                        / imported.source_bpm)
                        .round()
                };
                if !offset.is_finite() || !(0.0..=f64::from(u32::MAX)).contains(&offset) {
                    return Err(
                        "The selected sample trim exceeds the supported frame range.".into(),
                    );
                }
                clip.sample_id = imported.id;
                clip.offset_frames = offset as u32;
            }
            let updated = &mut project.sample_conflicts[conflict_index];
            for source_id in &mut updated.sample_ids {
                if source_id == id {
                    *source_id = imported.id;
                }
            }
            if imported.id != *id
                && let Some(span) = updated.source_spans.remove(id)
            {
                updated.source_spans.insert(imported.id, span);
            }
            project.imported_samples.retain(|sample| sample.id != *id);
            project.missing_samples.retain(|sample| sample.id != *id);
            project.imported_samples.push(imported);
        }
        project.sample_conflicts[conflict_index].selected = candidate_index;
        project.sample_conflicts[conflict_index].confirmed = true;
        project.validate()?;
        let mut library = (*self.library).clone();
        library.apply_imports(&project.imported_samples)?;
        library.reserve_missing_samples(&project.missing_samples);
        self.editor.transact(|current| *current = project)?;
        self.set_library(library);
        self.changed();
        Ok(())
    }
    /// Accept the current choices as one undoable action.
    pub fn confirm_sample_conflicts(&mut self) -> Result<(), String> {
        self.require_idle_recording()?;
        if self
            .sample_conflicts()
            .iter()
            .all(|conflict| conflict.confirmed)
        {
            return Ok(());
        }
        self.editor.transact(|project| {
            for conflict in &mut project.sample_conflicts {
                conflict.confirmed = true;
            }
        })?;
        self.changed();
        Ok(())
    }
    /// Bind every occurrence of one retained source to an explicitly chosen WAV.
    /// Manual choices receive imported identities, never verified factory identities.
    pub fn resolve_missing_sample(&mut self, id: u32, path: &Path) -> Result<(), String> {
        self.require_idle_recording()?;
        let selected = self
            .missing_sample(id)
            .ok_or("The missing sample no longer exists.")?
            .clone();
        let path = std::fs::canonicalize(path)
            .map_err(|error| format!("Cannot open the selected sample: {error}"))?;
        let path = crate::project_io::retain_manual_wave(&path, &self.settings_root)?;
        let mut project = self.project().clone();
        let matched: Vec<_> = project
            .missing_samples
            .iter()
            .filter(|sample| same_missing_reference(&selected.reference, &sample.reference))
            .cloned()
            .collect();
        let mut next_id = last_imported_id(&project, &self.library);
        for missing in matched {
            let new_id = if missing.id >= crate::products::IMPORTED_ID_START {
                missing.id
            } else {
                next_id = next_id
                    .checked_add(1)
                    .ok_or("Imported sample ID limit reached.")?;
                next_id
            };
            let mut imported = crate::engine::ImportedSample {
                id: new_id,
                name: missing.name.clone(),
                group_name: missing
                    .metadata
                    .get("group_name")
                    .filter(|name| !name.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| "Resolved samples".into()),
                path: path.clone(),
                source_bpm: missing.source_bpm,
                source_sample_rate: None,
                origin: None,
            };
            let ready = Library::check_import(&imported)?;
            imported.source_bpm =
                missing.resolved_source_bpm(ready.frame_count, ready.sample_rate)?;
            imported.source_sample_rate = Some(ready.sample_rate);
            for clip in project
                .clips
                .iter_mut()
                .filter(|clip| clip.sample_id == missing.id)
            {
                let offset = if clip.offset_frames == 0
                    || missing
                        .metadata
                        .get("Trim rate policy")
                        .is_some_and(|policy| policy == "preserve-frames")
                {
                    f64::from(clip.offset_frames)
                } else {
                    (f64::from(clip.offset_frames) * f64::from(ready.sample_rate)
                        / f64::from(missing.sample_rate)
                        * (missing.source_bpm / imported.source_bpm))
                        .round()
                };
                if !offset.is_finite() || !(0.0..=f64::from(u32::MAX)).contains(&offset) {
                    return Err(
                        "The selected sample trim exceeds the supported frame range.".into(),
                    );
                }
                clip.sample_id = new_id;
                clip.offset_frames = offset as u32;
            }
            project.imported_samples.push(imported);
            project
                .missing_samples
                .retain(|sample| sample.id != missing.id);
            for conflict in &mut project.sample_conflicts {
                conflict.sample_ids.retain(|id| *id != missing.id);
                conflict.source_spans.remove(&missing.id);
            }
        }
        project
            .sample_conflicts
            .retain(|conflict| !conflict.sample_ids.is_empty());
        project.validate()?;
        let mut library = (*self.library).clone();
        library.apply_imports(&project.imported_samples)?;
        library.reserve_missing_samples(&project.missing_samples);
        self.editor.transact(|current| *current = project)?;
        self.set_library(library);
        self.changed();
        Ok(())
    }
    fn set_library(&mut self, library: Library) {
        self.library = Arc::new(library);
        self.catalog_revision += 1;
    }
    /// A count that advances whenever the sample catalog is replaced. Views
    /// can keep work derived from `library()` until it changes.
    pub fn catalog_revision(&self) -> u64 {
        self.catalog_revision
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

    /// Where a song file dialog starts: the folder of the song last chosen in one.
    pub fn song_folder(&self) -> Option<PathBuf> {
        crate::project_io::load_song_folder(&self.settings_root)
    }
    /// Start later song file dialogs in the folder of `song`. A failed write
    /// only loses that convenience.
    pub fn remember_song_folder(&self, song: &Path) {
        if let Some(folder) = song.parent().filter(|folder| folder.is_dir()) {
            let _ = crate::project_io::save_song_folder(&self.settings_root, folder);
        }
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
        self.gesture = None;
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
                id,
                offset_frames,
                length_ticks,
                ..
            } => {
                let clip = self
                    .project()
                    .clips
                    .iter()
                    .find(|clip| clip.id == *id)
                    .ok_or("The clip does not exist.")?;
                let max_offset = if let Some(sample) = self.missing_sample(clip.sample_id) {
                    (f64::from(sample.nominal_ticks) * f64::from(sample.sample_rate) * 240.0
                        / sample.source_bpm
                        / f64::from(TICKS_PER_BAR))
                    .ceil() as u32
                } else {
                    self.library
                        .sample_by_id(clip.sample_id)
                        .ok_or("The sample is unavailable.")?
                        .frame_count
                };
                if *offset_frames > max_offset.saturating_sub(1) {
                    return Err("The trim start must be within the source audio.".into());
                }
                // A longer clip must not cover its neighbor. Other clip edits
                // stay available for imported arrangements that already overlap.
                if *length_ticks > clip.length_ticks {
                    crate::engine::ensure_space(
                        self.project(),
                        Some(*id),
                        clip.track,
                        clip.start_tick,
                        *length_ticks,
                    )?;
                }
            }
            EditCommand::SetRouting { track, .. } if *track >= TRACK_COUNT => {
                return Err("The track is outside the project.".into());
            }
            EditCommand::DeleteClips(ids) => {
                let existing: std::collections::HashSet<_> =
                    self.project().clips.iter().map(|clip| clip.id).collect();
                if ids.iter().any(|id| !existing.contains(id)) {
                    return Err("A selected clip does not exist.".into());
                }
            }
            _ => {}
        }
        let gesture = Gesture::of(&command, self.project());
        let now = std::time::Instant::now();
        let continuing = gesture.is_some()
            && self.gesture.is_some_and(|(previous, at)| {
                Some(previous) == gesture && now.duration_since(at) < GESTURE_GAP
            });
        let generation = self.editor.generation();
        let apply = |project: &mut Project| match command {
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
        };
        if continuing {
            self.editor.transact_continuing(apply)?;
        } else {
            self.editor.transact(apply)?;
        }
        // An accepted command that changes nothing is not an edit.
        if self.editor.generation() != generation {
            self.changed();
            self.gesture = gesture.map(|gesture| (gesture, now));
        }
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
        let generation = self.editor.generation();
        let ids = self.editor.move_clips(ids, track_delta, tick_delta, copy)?;
        if self.editor.generation() != generation {
            self.changed();
        }
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
        // Most steps leave the imported sources as they are. Rebuild the
        // catalog only for a step that adds, removes, or relinks one.
        let mut library = if self.library.has_imports(&self.project().imported_samples) {
            None
        } else {
            Some((*self.library).clone())
        };
        let result = match &mut library {
            Some(library) => library.apply_imports(&self.project().imported_samples),
            None => Ok(()),
        }
        .and_then(|()| {
            let library = library.as_ref().unwrap_or(&self.library);
            match self.project().clips.iter().find(|clip| {
                library.sample_by_id(clip.sample_id).is_none()
                    && self.project().missing_sample(clip.sample_id).is_none()
            }) {
                Some(clip) => Err(format!(
                    "Cannot restore this edit because sample {} is unavailable. Add its sample library and try again.",
                    clip.sample_id
                )),
                None => Ok(()),
            }
        });
        if let Err(error) = result {
            if redo {
                self.editor.undo();
            } else {
                self.editor.redo();
            }
            return Err(error);
        }
        if let Some(library) = library {
            self.set_library(library);
        }
        self.editor.set_libraries(self.library.bindings().to_vec());
        // Replacing the library discards the render worker's decoded sources.
        if !self
            .library
            .has_reserved_missing_samples(&self.editor.project().missing_samples)
        {
            Arc::make_mut(&mut self.library)
                .reserve_missing_samples(&self.editor.project().missing_samples);
        }
        self.changed();
        Ok(true)
    }
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        self.require_idle_recording()?;
        let (project, library, song) = crate::project_io::open_song(path, &self.library)?;
        self.stop();
        self.set_library(library);
        self.editor = Editor::new(project);
        self.revision = self.revision.wrapping_add(1);
        self.render = None;
        self.render_dirty = true;
        self.unsaved = false;
        // Save replaces a song. An import or earlier JSON project asks for a new file.
        self.file = song.then(|| path.to_path_buf());
        Ok(())
    }
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        crate::project_io::save(self.project(), path)?;
        // An edit after Save must be undoable back to the saved state.
        self.gesture = None;
        self.file = Some(path.to_path_buf());
        self.unsaved = false;
        Ok(())
    }
    /// Render and write the mix on this thread. Headless callers use this;
    /// the GUI calls `start_export` so that the window stays responsive.
    pub fn export(&self, path: &Path) -> Result<(), String> {
        // Playback already rendered an unchanged mix; write that audio.
        match &self.render {
            Some(mix) if !self.render_dirty => mix.write_wav(path),
            _ => audio::prepare_project(self.project(), &self.library)?.write_wav(path),
        }
    }

    /// Export the mix as it is now on another thread. `poll` reports the
    /// result as a notice. Later edits do not change the exported audio.
    pub fn start_export(&mut self, path: &Path) -> Result<(), String> {
        if self.export.is_some() {
            return Err("Wait for the current export to finish.".into());
        }
        let rendered = self.render.clone().filter(|_| !self.render_dirty);
        let project = self.project().clone();
        let library = self.library.clone();
        let target = path.to_path_buf();
        let (sender, receiver) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("mixjam-techno-export".into())
            .spawn(move || {
                let result = match rendered {
                    Some(mix) => mix.write_wav(&target),
                    None => audio::prepare_project(&project, &library)
                        .and_then(|mix| mix.write_wav(&target)),
                };
                let _ = sender.send(result);
            })
            .map_err(|error| format!("Cannot start the export: {error}"))?;
        self.message(format!("Exporting {}.", path.display()));
        self.export = Some(Export {
            path: path.to_path_buf(),
            receiver,
            thread,
        });
        Ok(())
    }

    pub fn is_exporting(&self) -> bool {
        self.export.is_some()
    }

    fn poll_export(&mut self) -> Option<u64> {
        let export = self.export.as_ref()?;
        let result = match export.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return Some(100),
            Err(mpsc::TryRecvError::Disconnected) => Err("The export stopped unexpectedly.".into()),
        };
        let export = self.export.take()?;
        let _ = export.thread.join();
        match result {
            Ok(()) => self.message(format!("Exported {}.", export.path.display())),
            Err(error) => self.error(format!("Cannot export mix: {error}")),
        }
        None
    }
    pub fn new_mix(&mut self) -> Result<(), String> {
        self.require_idle_recording()?;
        self.stop();
        self.editor = Editor::new(Project {
            libraries: self.library.bindings().to_vec(),
            imported_samples: self.library.imported_samples().into(),
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
        self.editor.set_libraries(bindings);
        self.set_library(library);
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
    /// Scan a folder on this thread and adopt it. Headless callers and tests
    /// use this; the GUI starts a background scan instead.
    pub fn select_wave_folder(&mut self, root: &Path, source_bpm: f64) -> Result<(), String> {
        if self.is_recording() {
            return Err("Stop recording before you change the sample folder.".into());
        }
        let library = Library::open_wav_folder(root, source_bpm)?;
        let warnings = self.adopt_wave_folder(library, root, source_bpm)?;
        if !warnings.is_empty() {
            self.error(warnings.join(" "));
        }
        Ok(())
    }

    /// Let these products' providers replace the same products found in a
    /// sample folder. Other providers only fill products the folder lacks.
    pub fn prefer_providers(&mut self, products: &[Product]) {
        self.preferred_providers = products.to_vec();
    }

    /// Replace the library with a scanned folder, keeping the product
    /// providers already in use. Returns the providers that could not be kept.
    fn adopt_wave_folder(
        &mut self,
        mut library: Library,
        root: &Path,
        source_bpm: f64,
    ) -> Result<Vec<String>, String> {
        let mut warnings = Vec::new();
        let previous_root = self.library.wav_root();
        for binding in self.library.bindings() {
            // Bindings at the previous sample folder came from its WAVs.
            if previous_root == Some(binding.root.as_path()) {
                continue;
            }
            let mut candidate = library.clone();
            if candidate.source_root(binding.product).is_some() {
                // A folder that merely shares some audio with this product
                // does not displace its provider.
                if !self.preferred_providers.contains(&binding.product)
                    && !library.is_sparse_wav_product(binding.product)
                {
                    continue;
                }
                candidate.remove_product(binding.product)?;
            }
            // The current library already holds this provider. Read it from
            // disk again only when its folder is gone, to report that.
            let kept = candidate
                .register_loaded(binding.product, &self.library)
                .and_then(|kept| {
                    if kept {
                        return Ok(());
                    }
                    candidate.register(
                        binding.product,
                        &binding.root,
                        &crate::project_io::reference_samples_root(),
                    )
                });
            match kept {
                Ok(()) => library = candidate,
                Err(error) => warnings.push(error),
            }
        }
        let (project, library) = crate::project_io::resolve(self.project().clone(), &library)?;
        crate::project_io::save_wave_config_in(&self.settings_root, root, source_bpm)?;
        self.stop();
        // New sample sources are not an edit: a saved or untouched mix stays so.
        let unsaved = self.unsaved;
        self.set_library(library);
        self.editor = Editor::new(project);
        self.changed();
        self.unsaved = unsaved;
        self.wave_folder_adopted = true;
        Ok(warnings)
    }

    /// Start scanning a sample folder on another thread. A scan already
    /// running is abandoned. The library changes only when `poll` receives
    /// the result; `wave_scan` reports progress until then.
    pub fn start_wave_scan(&mut self, root: &Path, source_bpm: f64) -> Result<(), String> {
        if self.is_recording() {
            return Err("Stop recording before you change the sample folder.".into());
        }
        if !source_bpm.is_finite() || source_bpm <= 0.0 {
            return Err("The source tempo must be finite and positive.".into());
        }
        if let Some(path) = self.abandon_wave_scan() {
            self.error(format!(
                "{} was not opened because another folder was selected.",
                path.display()
            ));
        }
        let progress = Arc::new(crate::library::ScanProgress::default());
        let (sender, receiver) = mpsc::channel();
        let worker_progress = Arc::clone(&progress);
        let worker_root = root.to_path_buf();
        let settings_root = self.settings_root.clone();
        std::thread::Builder::new()
            .name("mixjam-techno-scan".into())
            .spawn(move || {
                let mut cache = crate::project_io::load_scan_cache(&settings_root);
                let result = Library::open_wav_folder_with(
                    &worker_root,
                    source_bpm,
                    Some(&worker_progress),
                    Some(&mut cache),
                );
                // Keep the probes of a failed or cancelled scan for the next one.
                // A cache that cannot be written only costs that scan time.
                if cache.has_changes() {
                    let _ = crate::project_io::save_scan_cache(&settings_root, &cache);
                }
                let _ = sender.send(result);
            })
            .map_err(|error| format!("Cannot start the sample folder scan: {error}"))?;
        self.wave_scan = Some(WaveScan {
            root: root.to_path_buf(),
            source_bpm,
            progress,
            receiver,
        });
        Ok(())
    }

    pub fn wave_scan(&self) -> Option<WaveScanStatus> {
        self.wave_scan.as_ref().map(|scan| {
            let (files_done, files_total) = scan.progress.counts();
            WaveScanStatus {
                root: scan.root.clone(),
                files_done,
                files_total,
            }
        })
    }

    /// Abandon a running scan. Its thread stops at the next file; its result
    /// is never adopted, and a song waiting for it is not opened.
    pub fn cancel_wave_scan(&mut self) {
        if self.wave_scan.is_none() {
            return;
        }
        match self.abandon_wave_scan() {
            Some(path) => self.message(format!(
                "Sample folder scan cancelled. {} was not opened.",
                path.display()
            )),
            None => self.message("Sample folder scan cancelled."),
        }
    }

    /// Stop the running scan and return the song that was waiting for it.
    fn abandon_wave_scan(&mut self) -> Option<PathBuf> {
        if let Some(scan) = self.wave_scan.take() {
            scan.progress.cancel();
        }
        self.pending_open.take().map(|(path, _)| path)
    }

    /// Report a successful folder adoption once, so views can discard stale IDs.
    pub fn take_wave_folder_adopted(&mut self) -> bool {
        std::mem::take(&mut self.wave_folder_adopted)
    }

    /// The song opened by the last completed scan, reported once.
    pub fn take_opened_after_scan(&mut self) -> Option<PathBuf> {
        self.opened_after_scan.take()
    }

    /// Open this song after the running folder scan completes, or now when
    /// no scan is running.
    pub fn open_after_scan(&mut self, path: &Path) -> Result<(), String> {
        if self.wave_scan.is_some() {
            self.pending_open = Some((path.to_path_buf(), self.revision));
            Ok(())
        } else {
            self.open(path)
        }
    }

    fn poll_wave_scan(&mut self) -> Option<u64> {
        let scan = self.wave_scan.as_ref()?;
        if self.is_recording() {
            // Adopting the folder stops the transport; let the take finish first.
            return Some(100);
        }
        let result = match scan.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return Some(100),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("The sample folder scan stopped unexpectedly.".into())
            }
        };
        let scan = self.wave_scan.take()?;
        let pending = self.pending_open.take();
        let project_changed = pending.as_ref().is_some_and(|(_, revision)| {
            *revision != self.revision || self.needs_save_confirmation()
        });
        match result
            .and_then(|library| self.adopt_wave_folder(library, &scan.root, scan.source_bpm))
        {
            Ok(warnings) => {
                let count = self.library.samples.len();
                let mut notice = format!("Sample folder loaded: {count} sounds.");
                let mut error = !warnings.is_empty();
                if !warnings.is_empty() {
                    notice.push_str(&format!(" {}", warnings.join(" ")));
                }
                if let Some((path, _)) = pending {
                    if project_changed {
                        notice.push_str(&format!(
                            " {} was not opened because the current mix changed or has an unsaved take.",
                            path.display()
                        ));
                    } else {
                        match self.open(&path) {
                            Ok(()) => self.opened_after_scan = Some(path),
                            Err(cause) => {
                                error = true;
                                notice
                                    .push_str(&format!(" Cannot open {}: {cause}", path.display()));
                            }
                        }
                    }
                }
                self.notice = Some((notice, error));
            }
            Err(error) => match pending {
                Some((path, _)) => {
                    self.error(format!("{error} {} was not opened.", path.display()))
                }
                None => self.error(error),
            },
        }
        None
    }
    pub fn register_file(
        &mut self,
        path: &Path,
        group: &str,
        source_bpm: f64,
    ) -> Result<u32, String> {
        let mut library = (*self.library).clone();
        library.reserve_missing_samples(&self.project().missing_samples);
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
        self.set_library(library);
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
            source_sample_rate: None,
            origin: None,
        })?;
        if pcm.frames() == 0 {
            return Err("The audio source is empty.".into());
        }
        pcm.write_wav(path)?;
        self.register_file(path, group, source_bpm)
    }
    pub fn source_pcm(&self, id: Option<u32>) -> Result<Pcm, String> {
        if id.is_some_and(|id| self.missing_sample(id).is_some()) {
            return Err("This sample is missing. Select a folder with its source audio.".into());
        }
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
        crate::atomic_file::write(path, &bytes)
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
        (tick as f64 * self.editor.project().frames_per_tick(rate)).round() as u64
    }

    pub fn start_playback(&mut self) {
        if self.player.is_none() {
            self.error("Audio output is unavailable. You can still edit and export your mix.");
            return;
        }
        if self.editor.project().clips.is_empty() {
            self.pause();
            self.message("Add a sound to a track before you press Play.");
            return;
        }
        self.wants_playback = true;
        if self.render_dirty || self.render.is_none() {
            if self.requested_revision != Some(self.revision) {
                match self.request_render() {
                    Ok(()) => self.message("Preparing mix audio."),
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

    /// Queue a render of the current revision. `poll_render` stores the result.
    fn request_render(&mut self) -> Result<(), String> {
        self.render_worker.request(RenderRequest {
            revision: self.revision,
            project: self.editor.project().clone(),
            library: self.library.clone(),
        })?;
        self.requested_revision = Some(self.revision);
        Ok(())
    }

    fn cancel_playback_request(&mut self) {
        self.wants_playback = false;
        self.render_worker.cancel_pending();
        self.requested_revision = None;
    }

    fn apply_playback_buffer(&mut self) {
        let mix = self.render.as_ref().unwrap().clone();
        let rate = audio::Mix::SAMPLE_RATE;
        self.playback_frames_per_tick = self.editor.project().frames_per_tick(rate);
        let start = self.tick_to_frame(self.cursor_tick, rate);
        let region = self
            .editor
            .project()
            .loop_region
            .map(|[a, b]| [self.tick_to_frame(a, rate), self.tick_to_frame(b, rate)]);
        match self.player.as_mut().unwrap().play(mix, start, region) {
            Ok(()) => {
                self.previewing = false;
                self.seek_pending = false;
                self.message("Playing mix.");
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
        match result.mix {
            Ok(mix) => {
                if self.playing_mix() && !self.seek_pending {
                    self.cursor_tick = (self.player.as_ref().unwrap().position_frames() as f64
                        / self.playback_frames_per_tick)
                        as u32;
                }
                self.render = Some(mix);
                self.render_dirty = false;
                if self.wants_playback {
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
        self.cancel_playback_request();
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
        self.cancel_playback_request();
        let Some(sample) = self.library.sample_by_id(sample_id) else {
            return;
        };
        let name = sample.name.clone();
        match self.library.read_pcm(sample_id) {
            Ok(pcm) => match self.player.as_mut() {
                Some(player) => match pcm
                    .validate()
                    .and_then(|()| player.play(Arc::new(pcm), 0, None))
                {
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
        self.play_shared_pcm(Arc::new(pcm), label);
    }
    fn play_shared_pcm(&mut self, pcm: Arc<Pcm>, label: &str) {
        self.cancel_playback_request();
        let result = self
            .player
            .as_mut()
            .ok_or("Audio output is unavailable.".to_string())
            .and_then(|p| pcm.validate().and_then(|()| p.play(pcm, 0, None)));
        match result {
            Ok(()) => {
                self.previewing = true;
                self.message(label);
            }
            Err(error) => self.error(error),
        }
    }
    pub fn stop(&mut self) {
        self.cancel_playback_request();
        if let Some(player) = &mut self.player {
            player.stop();
        }
        self.previewing = false;
        self.cursor_tick = 0;
        if let Some(recorder) = self.recorder.take() {
            match recorder.finish() {
                Ok(pcm) => self.recorded_take = Some(Arc::new(pcm)),
                Err(error) => self.error(error),
            }
        } else {
            self.message("Stopped.");
        }
    }
    /// Advances devices and the render queue. Returns a suggested polling delay in milliseconds.
    pub fn poll(&mut self) -> Option<u64> {
        let scan_delay = match (self.poll_wave_scan(), self.poll_export()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
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
        let delay = if self.render_worker.in_flight {
            Some(20)
        } else if self.playing_mix() {
            Some(30)
        } else if self.is_recording() {
            Some(40)
        } else {
            None
        };
        match (delay, scan_delay) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
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
        // Render an edited mix during the count-in, so that it starts with the take.
        if self.render_dirty
            && !self.project().clips.is_empty()
            && self.player.is_some()
            && let Err(error) = self.request_render()
        {
            self.error(error);
        }
        Ok(())
    }
    fn poll_recording(&mut self) {
        let Some(recorder) = &self.recorder else {
            return;
        };
        let status = recorder.status();
        if let Some(error) = status.error {
            // The input cannot recover. Keep what it captured as the take.
            self.stop();
            self.error(if self.recorded_take.is_some() {
                format!("{error} Recording stopped. The audio captured before that is kept.")
            } else {
                error
            });
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
            self.play_shared_pcm(pcm, "Listening to the recorded take.");
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

/// The highest imported sample ID in use; new imports follow it.
fn last_imported_id(project: &Project, library: &Library) -> u32 {
    project
        .imported_samples
        .iter()
        .map(|sample| sample.id)
        .chain(project.missing_samples.iter().map(|sample| sample.id))
        .chain(library.samples.iter().map(|sample| sample.id))
        .max()
        .unwrap_or(0)
        .max(crate::products::IMPORTED_ID_START - 1)
}

fn same_missing_reference(left: &MissingReference, right: &MissingReference) -> bool {
    match (left, right) {
        (
            MissingReference::External { path: left, .. },
            MissingReference::External { path: right, .. },
        ) => {
            let normalized = |path: &Path| {
                let path = path.to_string_lossy().replace('\\', "/").to_lowercase();
                let path = if let Some(unc) = path.strip_prefix("//?/unc/") {
                    format!("//{unc}")
                } else {
                    path.strip_prefix("//?/").unwrap_or(&path).to_owned()
                };
                path.split('/')
                    .filter(|part| !part.is_empty() && *part != ".")
                    .collect::<Vec<_>>()
                    .join("/")
            };
            normalized(left) == normalized(right)
        }
        _ => left == right,
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

    #[test]
    fn identical_missing_paths_include_windows_verbatim_spellings() {
        let reference = |path: &str| MissingReference::External {
            path: path.into(),
            origin: None,
        };
        assert!(same_missing_reference(
            &reference(r"C:\Lost\Lead.wav"),
            &reference(r"\\?\c:\lost\lead.WAV")
        ));
        assert!(same_missing_reference(
            &reference(r"\\server\share\Lead.wav"),
            &reference(r"\\?\UNC\SERVER\SHARE\lead.wav")
        ));
        assert!(!same_missing_reference(
            &reference(r"C:\Lost\Lead.wav"),
            &reference(r"C:\Other\Lead.wav")
        ));
    }
    fn manual_fixture_dir() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "mixjam-manual-resolution-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }
    fn conflict_fixture() -> (Session, PathBuf) {
        let root = manual_fixture_dir();
        let first = root.join("first.wav");
        let second = root.join("second.wav");
        for (path, rate, frames, value) in [
            (&first, 22_050, 22_050, 0.25),
            (&second, 44_100, 88_200, -0.25),
        ] {
            Pcm {
                sample_rate: rate,
                channels: 1,
                samples: vec![value; frames],
            }
            .write_wav(path)
            .unwrap();
        }
        let first = std::fs::canonicalize(first).unwrap();
        let second = std::fs::canonicalize(second).unwrap();
        let sources: Vec<_> = [256, 512]
            .into_iter()
            .enumerate()
            .map(|(index, ticks)| crate::engine::ImportedSample {
                id: 1_000_000 + index as u32,
                name: "Lead".into(),
                group_name: "MixJam".into(),
                path: first.clone(),
                source_bpm: f64::from(ticks) * 240.0 / f64::from(TICKS_PER_BAR),
                source_sample_rate: Some(22_050),
                origin: None,
            })
            .collect();
        let mut library = Library::empty();
        library.apply_imports(&sources).unwrap();
        let project = Project {
            imported_samples: sources.into(),
            sample_conflicts: vec![SampleConflict {
                reference: "Old/lead.wav".into(),
                sample_ids: vec![1_000_000, 1_000_001],
                candidates: vec![first, second],
                selected: 0,
                confirmed: false,
                source_spans: [(1_000_000, 256), (1_000_001, 512)].into(),
            }],
            clips: [1_000_000, 1_000_001, 1_000_000]
                .into_iter()
                .enumerate()
                .map(|(index, id)| Clip {
                    id: index as u64 + 1,
                    sample_id: id,
                    track: index * 4,
                    start_tick: 17,
                    length_ticks: 256,
                    offset_frames: 11_025,
                    gain: 0.75,
                })
                .collect(),
            ..Project::default()
        };
        (Session::headless(Arc::new(library), project).unwrap(), root)
    }
    #[test]
    fn conflict_switch_updates_all_instances_and_preserves_musical_trims_with_history() {
        let (mut session, root) = conflict_fixture();
        let original = session.project().clone();
        assert_eq!(session.sample_conflicts()[0].selected, 0);
        assert!(!session.sample_conflicts()[0].confirmed);
        assert!(
            audio::render_project(session.project(), &session.library)
                .unwrap()
                .samples
                .iter()
                .any(|sample| *sample > 0.0)
        );
        let revision = session.revision;
        session.resolve_sample_conflict(0, 1).unwrap();
        assert!(session.unsaved());
        assert!(session.revision > revision);
        assert!(session.render_dirty);
        let chosen = &session.sample_conflicts()[0];
        assert_eq!(chosen.selected, 1);
        assert!(chosen.confirmed);
        for source in &session.project().imported_samples {
            assert_eq!(source.path, chosen.candidates[1]);
            assert_eq!(source.source_sample_rate, Some(44_100));
            assert_eq!(
                source.source_bpm,
                if source.id == 1_000_000 { 120.0 } else { 240.0 }
            );
        }
        for (before, after) in original.clips.iter().zip(&session.project().clips) {
            assert_eq!(after.offset_frames, 44_100);
            let mut expected = before.clone();
            expected.offset_frames = 44_100;
            assert_eq!(after, &expected);
        }
        assert!(
            audio::render_project(session.project(), &session.library)
                .unwrap()
                .samples
                .iter()
                .any(|sample| *sample < 0.0)
        );
        let changed = session.project().clone();
        let encoded = serde_json::to_vec(&changed).unwrap();
        assert_eq!(
            serde_json::from_slice::<Project>(&encoded).unwrap(),
            changed
        );
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project(), &original);
        assert!(session.history_step(true).unwrap());
        assert_eq!(session.project(), &changed);
        session.resolve_sample_conflict(0, 0).unwrap();
        assert_eq!(session.project().clips, original.clips);
        for path in &original.sample_conflicts[0].candidates {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn t1_conflict_switch_preserves_decoder_on_return_and_missing_source_recovery() {
        let (base, root) = conflict_fixture();
        let mut project = base.project().clone();
        let candidates = project.sample_conflicts[0].candidates.clone();
        // Distinct stereo lanes and source rates exercise the T1 WAV decoder.
        for (path, rate, left, right) in [
            (&candidates[0], 22_050, 0.25, -0.75),
            (&candidates[1], 44_100, -0.5, 0.125),
        ] {
            Pcm {
                sample_rate: rate,
                channels: 2,
                samples: [left, right].repeat(rate as usize),
            }
            .write_wav(path)
            .unwrap();
        }
        project.sample_conflicts[0].source_spans.clear();
        for source in &mut project.imported_samples {
            source.group_name = "Techno 1".into();
            source.source_bpm = 140.0;
            source.source_sample_rate = Some(44_100);
            source.origin = Some(Product::T1);
        }
        let mut library = Library::empty();
        library.apply_imports(&project.imported_samples).unwrap();
        let mut session = Session::headless(Arc::new(library), project.clone()).unwrap();
        let original = audio::render_project(session.project(), &session.library).unwrap();
        assert!(original.samples.iter().any(|sample| *sample != 0.0));
        session.resolve_sample_conflict(0, 1).unwrap();
        assert!(
            session
                .project()
                .imported_samples
                .iter()
                .all(|source| source.origin == Some(Product::T1))
        );
        let alternative = audio::render_project(session.project(), &session.library).unwrap();
        assert_ne!(alternative.samples, original.samples);
        session.resolve_sample_conflict(0, 0).unwrap();
        assert_eq!(session.project().clips, project.clips);
        let returned = audio::render_project(session.project(), &session.library).unwrap();
        assert_eq!(returned.sample_rate, original.sample_rate);
        assert_eq!(returned.channels, original.channels);
        assert_eq!(returned.samples, original.samples);

        let mut missing_project = session.project().clone();
        let unavailable = missing_project.imported_samples.remove(0);
        missing_project
            .missing_samples
            .push(MissingSample::external(
                &unavailable,
                256,
                "Unavailable source.".into(),
            ));
        let mut missing_library = Library::empty();
        missing_library
            .apply_imports(&missing_project.imported_samples)
            .unwrap();
        missing_library.reserve_missing_samples(&missing_project.missing_samples);
        let mut missing_session =
            Session::headless(Arc::new(missing_library), missing_project).unwrap();
        missing_session.resolve_sample_conflict(0, 1).unwrap();
        assert!(missing_session.missing_samples().is_empty());
        assert!(
            missing_session
                .project()
                .imported_samples
                .iter()
                .all(|source| source.origin == Some(Product::T1))
        );
        for path in &candidates {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn deleting_conflict_placements_prunes_choices_and_spans_with_history() {
        let (mut session, root) = conflict_fixture();
        let original = session.project().clone();
        session.delete(2).unwrap();
        assert_eq!(session.sample_conflicts().len(), 1);
        assert_eq!(session.sample_conflicts()[0].sample_ids, vec![1_000_000]);
        assert_eq!(
            session.sample_conflicts()[0].source_spans,
            [(1_000_000, 256)].into()
        );
        assert!(!session.sample_conflicts()[0].confirmed);
        let partial = session.project().clone();
        session.edit(EditCommand::DeleteClips(vec![1, 3])).unwrap();
        assert!(session.project().clips.is_empty());
        assert!(session.sample_conflicts().is_empty());
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project(), &partial);
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project(), &original);
        assert!(session.history_step(true).unwrap());
        assert_eq!(session.project(), &partial);
        assert!(session.history_step(true).unwrap());
        assert!(session.sample_conflicts().is_empty());
        for path in &original.sample_conflicts[0].candidates {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn invalid_conflict_choice_is_atomic_and_accepting_defaults_is_undoable() {
        let (mut session, root) = conflict_fixture();
        let original = session.project().clone();
        let revision = session.revision;
        assert!(session.resolve_sample_conflict(9, 0).is_err());
        assert!(session.resolve_sample_conflict(0, 9).is_err());
        std::fs::write(&original.sample_conflicts[0].candidates[1], b"invalid WAV").unwrap();
        assert!(session.resolve_sample_conflict(0, 1).is_err());
        assert_eq!(session.project(), &original);
        assert_eq!(session.revision, revision);
        assert!(!session.unsaved());
        assert!(!session.history_step(false).unwrap());
        session.confirm_sample_conflicts().unwrap();
        assert!(session.sample_conflicts()[0].confirmed);
        assert_eq!(session.sample_conflicts()[0].selected, 0);
        assert_eq!(session.project().clips, original.clips);
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project(), &original);
        assert!(session.history_step(true).unwrap());
        assert!(session.sample_conflicts()[0].confirmed);
        for path in &original.sample_conflicts[0].candidates {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn conflict_choice_restores_native_missing_source_as_import_without_copying() {
        let (base, root) = conflict_fixture();
        let mut missing = MissingSample::native(
            Product::T2,
            42,
            "Lead".into(),
            140.0,
            256,
            false,
            "Missing source.".into(),
        )
        .unwrap();
        missing.sample_rate = 22_050;
        let candidates = base.sample_conflicts()[0].candidates.clone();
        let project = Project {
            missing_samples: vec![missing],
            sample_conflicts: vec![SampleConflict {
                reference: "Native lead".into(),
                sample_ids: vec![42],
                candidates: candidates.clone(),
                selected: 0,
                confirmed: false,
                source_spans: Default::default(),
            }],
            clips: vec![Clip {
                id: 1,
                sample_id: 42,
                track: 0,
                start_tick: 17,
                length_ticks: 256,
                offset_frames: 11_025,
                gain: 0.75,
            }],
            ..Project::default()
        };
        let mut session = Session::headless(Arc::new(Library::empty()), project.clone()).unwrap();
        session.resolve_sample_conflict(0, 1).unwrap();
        assert!(session.missing_samples().is_empty());
        let imported = &session.project().imported_samples[0];
        assert!(imported.id >= crate::products::IMPORTED_ID_START);
        assert_eq!(imported.path, candidates[1]);
        assert_eq!(imported.source_bpm, 140.0);
        assert_eq!(imported.origin, None);
        assert_eq!(session.sample_conflicts()[0].sample_ids, vec![imported.id]);
        assert_eq!(session.project().clips[0].sample_id, imported.id);
        assert_eq!(session.project().clips[0].offset_frames, 22_050);
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project(), &project);
        assert!(session.history_step(true).unwrap());
        for path in &candidates {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }

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
            digest: None,
        };
        let library = Arc::new(Library::fixture(vec![sample]));
        let mut editor = Editor::new(Project::default());
        editor.place(1, 0, 0, TICKS_PER_BAR).unwrap();
        editor.place(1, 1, TICKS_PER_BAR, TICKS_PER_BAR).unwrap();
        Session::headless(library, editor.into_project()).unwrap()
    }
    #[test]
    fn manual_resolution_is_atomic_updates_all_exact_references_and_supports_history() {
        let root = manual_fixture_dir();
        let imported = crate::engine::ImportedSample {
            id: 1_000_000,
            name: "Missing lead".into(),
            group_name: "Leads".into(),
            path: PathBuf::from("C:\\Old\\Leads\\lead.wav"),
            source_bpm: 120.0,
            source_sample_rate: None,
            origin: None,
        };
        let first = MissingSample::external(&imported, 512, "File is missing.".into());
        let mut second = first.clone();
        second.id += 1;
        second.reference = MissingReference::External {
            path: PathBuf::from("c:/old/leads/LEAD.wav"),
            origin: Some(Product::D3),
        };
        let mut unrelated = first.clone();
        unrelated.id += 2;
        unrelated.reference = MissingReference::External {
            path: PathBuf::from("C:\\Other\\lead.wav"),
            origin: None,
        };
        let mut project = Project {
            missing_samples: vec![first, second, unrelated],
            ..Project::default()
        };
        for (index, (sample_id, track)) in [
            (1_000_000, 0),
            (1_000_000, 4),
            (1_000_001, 7),
            (1_000_002, 9),
        ]
        .into_iter()
        .enumerate()
        {
            project.clips.push(Clip {
                id: index as u64 + 1,
                sample_id,
                track,
                start_tick: 17,
                length_ticks: 256,
                offset_frames: 4410,
                gain: 0.75,
            });
        }
        let mut session = Session::headless(Arc::new(Library::empty()), project.clone())
            .unwrap()
            .with_settings_root(root.join("settings"));
        let revision = session.revision;
        let bad = root.join("bad.wav");
        std::fs::write(&bad, b"not audio").unwrap();
        assert!(session.resolve_missing_sample(1_000_000, &bad).is_err());
        assert_eq!(session.project(), &project);
        assert_eq!(session.revision, revision);
        assert!(!session.unsaved());
        assert!(!session.history_step(false).unwrap());

        let wave = root.join("chosen.wav");
        Pcm {
            sample_rate: 22_050,
            channels: 1,
            samples: vec![0.5; 22_050],
        }
        .write_wav(&wave)
        .unwrap();
        session.resolve_missing_sample(1_000_000, &wave).unwrap();
        assert_eq!(session.missing_samples().len(), 1);
        assert_eq!(session.missing_samples()[0].id, 1_000_002);
        assert!(session.unsaved());
        assert!(session.revision > revision);
        for (index, clip) in session.project().clips.iter().enumerate() {
            assert_eq!(clip.start_tick, 17);
            assert_eq!(clip.length_ticks, 256);
            assert_eq!(clip.gain, 0.75);
            assert_eq!(clip.track, project.clips[index].track);
            assert_eq!(clip.offset_frames, if index < 3 { 2205 } else { 4410 });
        }
        let resolved = session.project().clone();
        assert!(
            audio::render_project(&resolved, &session.library)
                .unwrap()
                .samples
                .iter()
                .any(|sample| *sample != 0.0)
        );
        session.history_step(false).unwrap();
        assert_eq!(session.project(), &project);
        assert!(
            audio::render_project(session.project(), &session.library)
                .unwrap()
                .samples
                .iter()
                .all(|sample| *sample == 0.0)
        );
        session.history_step(true).unwrap();
        assert_eq!(session.project(), &resolved);
        std::fs::remove_file(&resolved.imported_samples[0].path).unwrap();
        std::fs::remove_file(wave).unwrap();
        std::fs::remove_file(bad).unwrap();
        std::fs::remove_dir(root.join("settings/manual-samples")).unwrap();
        std::fs::remove_dir(root.join("settings")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn manual_resolution_uses_saved_rate_and_actual_mixjam_wave_duration() {
        let root = manual_fixture_dir();
        let wave = root.join("chosen.wav");
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 88_200],
        }
        .write_wav(&wave)
        .unwrap();
        let imported = crate::engine::ImportedSample {
            id: 1_000_000,
            name: "Missing lead".into(),
            group_name: "Lead".into(),
            path: PathBuf::from("lost.wav"),
            source_bpm: 140.0,
            source_sample_rate: Some(22_050),
            origin: None,
        };
        let mut missing = MissingSample::external(&imported, 256, "Missing audio.".into());
        missing
            .metadata
            .insert("Playback timing policy".into(), "mixjam-span".into());
        missing
            .metadata
            .insert("durationSeconds".into(), "99".into());
        let project = Project {
            missing_samples: vec![missing],
            clips: vec![Clip {
                id: 1,
                sample_id: imported.id,
                track: 0,
                start_tick: 0,
                length_ticks: 256,
                offset_frames: 11_025,
                gain: 1.0,
            }],
            ..Project::default()
        };
        let mut session = Session::headless(Arc::new(Library::empty()), project)
            .unwrap()
            .with_settings_root(root.join("settings"));
        session.resolve_missing_sample(imported.id, &wave).unwrap();
        assert_eq!(session.project().clips[0].offset_frames, 25_725);
        assert_eq!(session.project().imported_samples[0].source_bpm, 120.0);
        assert_eq!(
            session.project().imported_samples[0].source_sample_rate,
            Some(44_100)
        );
        std::fs::remove_file(&session.project().imported_samples[0].path).unwrap();
        std::fs::remove_file(wave).unwrap();
        std::fs::remove_dir(root.join("settings/manual-samples")).unwrap();
        std::fs::remove_dir(root.join("settings")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn manual_native_resolution_uses_imported_identity_without_factory_identity() {
        let root = manual_fixture_dir();
        let missing = MissingSample::native(
            Product::T2,
            42,
            "Native lead".into(),
            140.0,
            256,
            false,
            "Missing library.".into(),
        )
        .unwrap();
        let mut project = Project {
            missing_samples: vec![missing],
            ..Project::default()
        };
        project.clips.push(Clip {
            id: 1,
            sample_id: 42,
            track: 0,
            start_tick: 0,
            length_ticks: 256,
            offset_frames: 0,
            gain: 1.0,
        });
        let wave = root.join("chosen.wav");
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.5; 1000],
        }
        .write_wav(&wave)
        .unwrap();
        let mut session = Session::headless(Arc::new(Library::empty()), project)
            .unwrap()
            .with_settings_root(root.join("settings"));
        session.resolve_missing_sample(42, &wave).unwrap();
        assert!(session.missing_samples().is_empty());
        assert!(session.project().clips[0].sample_id >= crate::products::IMPORTED_ID_START);
        assert!(session.library.sample_by_id(42).is_none());
        assert_eq!(session.library.verified_count(), 0);
        std::fs::remove_file(&session.project().imported_samples[0].path).unwrap();
        std::fs::remove_file(wave).unwrap();
        std::fs::remove_dir(root.join("settings/manual-samples")).unwrap();
        std::fs::remove_dir(root.join("settings")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn manual_choice_survives_same_basename_palette_and_original_file_loss() {
        let root = manual_fixture_dir();
        let selected = root.join("selected");
        let outside = root.join("outside");
        std::fs::create_dir_all(&selected).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        for (folder, value) in [(&selected, -0.5), (&outside, 0.5)] {
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![value; 1000],
            }
            .write_wav(&folder.join("lead.wav"))
            .unwrap();
        }
        let library = Library::open_wav_folder(&selected, 140.0).unwrap();
        let missing = MissingSample::native(
            Product::T2,
            42,
            "Native lead".into(),
            140.0,
            256,
            false,
            "Missing source.".into(),
        )
        .unwrap();
        let project = Project {
            missing_samples: vec![missing],
            clips: vec![Clip {
                id: 1,
                sample_id: 42,
                track: 0,
                start_tick: 0,
                length_ticks: 256,
                offset_frames: 0,
                gain: 1.0,
            }],
            ..Project::default()
        };
        let mut session = Session::headless(Arc::new(library.clone()), project)
            .unwrap()
            .with_settings_root(root.join("settings"));
        session
            .resolve_missing_sample(42, &outside.join("lead.wav"))
            .unwrap();
        let retained = session.project().imported_samples[0].path.clone();
        assert_eq!(
            std::fs::read(&retained).unwrap(),
            std::fs::read(outside.join("lead.wav")).unwrap()
        );
        std::fs::remove_file(outside.join("lead.wav")).unwrap();
        let saved = root.join("song.mixjam");
        session.save(&saved).unwrap();
        let (reopened, ready) = crate::project_io::open(&saved, &library).unwrap();
        assert_eq!(reopened.imported_samples[0].path, retained);
        assert!(reopened.missing_samples.is_empty());
        let rendered = audio::render_project(&reopened, &ready).unwrap();
        assert!(rendered.samples.iter().any(|sample| *sample > 0.0));
        assert!(rendered.samples.iter().all(|sample| *sample >= 0.0));
        std::fs::remove_file(retained).unwrap();
        std::fs::remove_file(saved).unwrap();
        std::fs::remove_file(selected.join("lead.wav")).unwrap();
        std::fs::remove_dir(selected).unwrap();
        std::fs::remove_dir(outside).unwrap();
        std::fs::remove_dir(root.join("settings/manual-samples")).unwrap();
        std::fs::remove_dir(root.join("settings")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn recording_blocks_sample_folder_changes_before_scanning_or_replacing_state() {
        let mut app = fixture();
        app.editor
            .transact(|project| project.title = "Before recording".into())
            .unwrap();
        app.cursor_tick = TICKS_PER_BAR;
        app.recorder = Some(crate::recording::Recorder::fixture());
        let project = app.editor.project().clone();
        let library = app.library.clone();
        let revision = app.revision;
        let unsaved = app.unsaved;

        // An empty path cannot be scanned. The recording guard must run first.
        let error = app.select_wave_folder(Path::new(""), 140.0).unwrap_err();

        assert_eq!(error, "Stop recording before you change the sample folder.");
        assert_eq!(*app.editor.project(), project);
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
            project: app.editor.project().clone(),
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
                mix: Err("Fixture result".into()),
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
                mix: Err("Fixture result".into()),
            })
            .unwrap();
        worker.poll().unwrap();
        assert!(requests.try_recv().is_err());
        assert!(!worker.in_flight);
    }

    #[test]
    fn a_stopped_render_worker_fails_its_render_and_is_replaced() {
        let app = fixture();
        let (sender, requests) = mpsc::channel();
        let (results, receiver) = mpsc::channel::<RenderResult>();
        let mut worker = RenderWorker {
            sender,
            receiver,
            in_flight: false,
            current_revision: None,
            pending: None,
        };
        let request = |revision| RenderRequest {
            revision,
            project: app.editor.project().clone(),
            library: app.library.clone(),
        };
        worker.request(request(1)).unwrap();
        worker.request(request(2)).unwrap();
        // The worker thread ends without a result, as it does after a panic.
        drop((requests, results));
        let failed = worker.poll().unwrap();
        assert_eq!(failed.revision, 1);
        assert!(failed.mix.is_err());
        // The replacement worker renders the request that was waiting.
        let finished = loop {
            if let Some(result) = worker.poll() {
                break result;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(finished.revision, 2);
        assert!(!worker.in_flight);
        assert!(worker.poll().is_none());
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
        let mix = Arc::new(audio::prepare_project(&Project::default(), &Library::empty()).unwrap());
        results
            .send(RenderResult {
                revision: 5,
                mix: Ok(mix.clone()),
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
                mix: Ok(mix),
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
        let empty_project = app.editor.project().clone();
        assert!(empty_project.clips.is_empty());
        let mut removed = (*app.library).clone();
        removed.remove_product(Product::T2).unwrap();
        app.library = Arc::new(removed);
        let removed_library = app.library.clone();
        let error = app.history_step(false).unwrap_err();
        assert!(error.contains("sample 1 is unavailable"));
        assert_eq!(*app.editor.project(), empty_project);
        assert!(Arc::ptr_eq(&removed_library, &app.library));
        assert!(
            !app.editor.redo(),
            "A rejected undo must not leave a redo entry."
        );
        app.library = original_library;
        app.history_step(false).unwrap();
        assert_eq!(app.editor.project().clips.len(), 2);
        assert!(app.library.sample_by_id(1).is_some());
        app.history_step(true).unwrap();
        assert!(app.editor.project().clips.is_empty());
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
        session.save(&dir.join("project.mixjam")).unwrap();
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
    fn accepted_commands_that_change_nothing_are_not_edits() {
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
        session.place(source, 1, 0).unwrap();
        session.save(&dir.join("project.mixjam")).unwrap();
        let revision = session.revision;
        session.move_clips(&[clip], 0, 0, false).unwrap();
        session
            .edit(EditCommand::SetMixer {
                tracks: session.project().tracks.clone(),
                master_gain: session.project().master_gain,
            })
            .unwrap();
        session
            .edit(EditCommand::SetTempo(session.project().bpm))
            .unwrap();
        assert!(!session.unsaved());
        assert_eq!(session.revision, revision);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn saving_ends_a_control_gesture_before_the_next_edit() {
        let dir = fixture_dir();
        let mut session =
            Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
        let set_master = |session: &mut Session, master_gain| {
            session.edit(EditCommand::SetMixer {
                tracks: session.project().tracks.clone(),
                master_gain,
            })
        };
        set_master(&mut session, 0.5).unwrap();
        session.save(&dir.join("gesture.mixjam")).unwrap();
        let saved = session.project().clone();
        set_master(&mut session, 0.75).unwrap();
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project(), &saved);
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project(), &Project::default());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn one_control_drag_is_one_undo_step_and_keeps_earlier_history() {
        let mut session =
            Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
        session.edit(EditCommand::SetTempo(160.0)).unwrap();
        let mixer = |session: &Session, track: usize, gain: f32, pan: f32| {
            let mut tracks = session.project().tracks.clone();
            tracks[track].gain = gain;
            tracks[track].pan = pan;
            EditCommand::SetMixer {
                tracks,
                master_gain: session.project().master_gain,
            }
        };
        // More frames than the history holds: the tempo edit must survive them.
        for frame in 0..150 {
            let command = mixer(&session, 3, 1.0 + (frame + 1) as f32 * 0.01, 0.0);
            session.edit(command).unwrap();
        }
        // Another control, and the same control after a pause, start new steps.
        let command = mixer(&session, 3, 2.5, 0.5);
        session.edit(command).unwrap();
        session.gesture = session
            .gesture
            .map(|(gesture, at)| (gesture, at - GESTURE_GAP));
        let command = mixer(&session, 3, 2.5, -0.5);
        session.edit(command).unwrap();
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project().tracks[3].pan, 0.5);
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project().tracks[3].pan, 0.0);
        assert_eq!(session.project().tracks[3].gain, 2.5);
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project().tracks[3].gain, 1.0);
        assert_eq!(session.project().bpm, 160.0);
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project(), &Project::default());
        assert!(!session.history_step(false).unwrap());
    }
    #[test]
    fn a_longer_clip_cannot_cover_the_next_clip_on_its_track() {
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
        let first = session.place(source, 0, 0).unwrap();
        let length = session.project().clips[0].length_ticks;
        session.place(source, 0, length + 16).unwrap();
        let before = session.project().clone();
        let resize = |length_ticks| EditCommand::SetClip {
            id: first,
            gain: 1.0,
            offset_frames: 0,
            length_ticks,
        };
        assert!(session.edit(resize(length + 17)).is_err());
        assert_eq!(session.project(), &before);
        session.edit(resize(length + 16)).unwrap();
        assert_eq!(session.project().clips[0].length_ticks, length + 16);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn each_clip_control_drag_is_its_own_undo_step() {
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
        let length = session.project().clips[0].length_ticks;
        let set = |gain, offset_frames, length_ticks| EditCommand::SetClip {
            id: clip,
            gain,
            offset_frames,
            length_ticks,
        };
        // Frames of a gain drag, then at once frames of a trim drag: two
        // controls, so two steps.
        session.edit(set(0.5, 0, length)).unwrap();
        session.edit(set(0.6, 0, length)).unwrap();
        session.edit(set(0.6, 10, length)).unwrap();
        session.edit(set(0.6, 20, length)).unwrap();
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project().clips[0].offset_frames, 0);
        assert_eq!(session.project().clips[0].gain, 0.6);
        assert!(session.history_step(false).unwrap());
        assert_eq!(session.project().clips[0].gain, 1.0);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[cfg_attr(
        not(bundled_catalog),
        ignore = "Needs the locally generated sample catalogs."
    )]
    fn folder_adoption_keeps_optional_providers_and_respects_explicit_priority() {
        let dir = fixture_dir();
        let provider_root = dir.join("provider");
        let packed = provider_root.join("eJay/PXD");
        std::fs::create_dir_all(&packed).unwrap();
        // A complete one-record native House provider, with five silent frames.
        let mut record = b"tPxD\x02\0\0T".to_vec();
        record.extend(5_u32.to_le_bytes());
        record.extend([0, 0, 0]);
        std::fs::write(packed.join("house10"), &record).unwrap();
        std::fs::write(
            packed.join("house10.inf"),
            format!(
                "[SAMPLES]\n1\n6\n\"HOUSE\"\n0\n{}\n\"House sound\"\n\"1\"\n1\n18\n\"\"\n0\n\"\"\n",
                record.len()
            ),
        )
        .unwrap();
        let mut provider = Library::empty();
        provider
            .register(Product::H1, &provider_root, &dir)
            .unwrap();
        let house_id = Product::H1.global_sample_id(1).unwrap();
        assert_eq!(provider.read_pcm(house_id).unwrap().samples, vec![0.0; 5]);
        let folder = dir.join("Dance4");
        std::fs::create_dir_all(&folder).unwrap();
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 5],
        }
        .write_wav(&folder.join("sound.wav"))
        .unwrap();
        let identity = |native_id| crate::catalog::WaveMetadata {
            native_id,
            code: format!("WAVE{native_id}"),
            name: "Folder sound".into(),
            group_name: "Wave".into(),
            bars: 1,
            source_bpm: 140.0,
            frame_count: 5,
            pcm_digest: Some(crate::catalog::pcm_identity_hex(&[0; 5])),
        };
        let complete_count = crate::catalog::wave_metadata(Product::H1).unwrap().len();
        for (house_count, preferred, expect_provider) in [
            (0, false, true), // A Dance 4 only folder must keep House.
            (1, false, true), // Shared audio must not displace House.
            (complete_count, false, false),
            (complete_count, true, true),
        ] {
            let mut scanned = Library::open_wav_folder(&folder, 140.0).unwrap();
            scanned
                .register_wav_product(Product::D4, &folder, &[identity(1)])
                .unwrap();
            if house_count > 0 {
                let entries: Vec<_> = (1..=house_count as u32).map(identity).collect();
                scanned
                    .register_wav_product(Product::H1, &folder, &entries)
                    .unwrap();
            }
            let mut session = Session::headless(Arc::new(provider.clone()), Project::default())
                .unwrap()
                .with_settings_root(dir.join("settings"));
            if preferred {
                session.prefer_providers(&[Product::H1]);
            }
            assert!(
                session
                    .adopt_wave_folder(scanned, &folder, 140.0)
                    .unwrap()
                    .is_empty()
            );
            let source = session.library().sample_by_id(house_id).unwrap();
            assert_eq!(
                source.wav_path.is_none(),
                expect_provider,
                "count={house_count}, preferred={preferred}"
            );
            assert!(
                session
                    .library()
                    .sample_by_id(Product::D4.global_sample_id(1).unwrap())
                    .is_some()
            );
            assert!(!session.unsaved());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unavailable_optional_provider_keeps_scanned_product_and_reports_warning() {
        let dir = fixture_dir();
        let old = dir.join("provider");
        let new = dir.join("folder");
        let metadata = [crate::catalog::WaveMetadata {
            native_id: 1,
            code: "TEST".into(),
            name: "Test".into(),
            group_name: "Wave".into(),
            bars: 1,
            source_bpm: 140.0,
            frame_count: 1000,
            pcm_digest: Some(crate::catalog::pcm_identity_hex(&vec![0; 1000])),
        }];
        for folder in [&old, &new] {
            std::fs::create_dir_all(folder).unwrap();
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![0.0; 1000],
            }
            .write_wav(&folder.join("sound.wav"))
            .unwrap();
        }
        let mut provider = Library::empty();
        provider.import_wav_folder(&old, 140.0).unwrap();
        provider
            .register_wav_product(Product::T2, &old, &metadata)
            .unwrap();
        let mut scanned = Library::open_wav_folder(&new, 140.0).unwrap();
        scanned
            .register_wav_product(Product::T2, &new, &metadata)
            .unwrap();
        let native_id = Product::T2.global_sample_id(1).unwrap();
        assert!(scanned.sample_by_id(native_id).is_some());
        let mut session = Session::headless(Arc::new(provider), Project::default())
            .unwrap()
            .with_settings_root(dir.join("settings"));
        std::fs::remove_dir_all(&old).unwrap();
        let song = dir.join("queued.json");
        Project::default().save(&song).unwrap();
        let (sender, receiver) = mpsc::channel();
        session.wave_scan = Some(WaveScan {
            root: new,
            source_bpm: 140.0,
            progress: Arc::new(crate::library::ScanProgress::default()),
            receiver,
        });
        session.open_after_scan(&song).unwrap();
        sender.send(Ok(scanned)).unwrap();
        session.poll();
        assert!(session.library().sample_by_id(native_id).is_some());
        assert_eq!(session.take_opened_after_scan(), Some(song));
        let (notice, error) = session.take_notice().unwrap();
        assert!(error && notice.contains("Cannot open"), "{notice}");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn deferred_scan_open_preserves_intervening_work() {
        let dir = fixture_dir();
        let song = dir.join("queued.json");
        Project {
            title: "Queued song".into(),
            ..Project::default()
        }
        .save(&song)
        .unwrap();
        let other = dir.join("other.json");
        Project {
            title: "Other song".into(),
            ..Project::default()
        }
        .save(&other)
        .unwrap();
        for action in ["edit", "edit_then_save", "new", "open", "take"] {
            let mut session = Session::headless(Arc::new(Library::empty()), Project::default())
                .unwrap()
                .with_settings_root(dir.join(action));
            let (sender, receiver) = mpsc::channel();
            session.wave_scan = Some(WaveScan {
                root: dir.clone(),
                source_bpm: 140.0,
                progress: Arc::new(crate::library::ScanProgress::default()),
                receiver,
            });
            session.open_after_scan(&song).unwrap();
            match action {
                "edit" => session.edit(EditCommand::SetTempo(160.0)).unwrap(),
                "edit_then_save" => {
                    session.edit(EditCommand::SetTempo(160.0)).unwrap();
                    session.save(&dir.join("saved.mixjam")).unwrap();
                }
                "new" => session.new_mix().unwrap(),
                "open" => session.open(&other).unwrap(),
                "take" => {
                    session.recorded_take = Some(Arc::new(Pcm {
                        sample_rate: 44_100,
                        channels: 1,
                        samples: vec![0.25; 32],
                    }))
                }
                _ => unreachable!(),
            }
            let expected = session.project().clone();
            let expected_file = session.file.clone();
            sender.send(Ok(Library::empty())).unwrap();
            session.poll();
            assert_eq!(session.project(), &expected, "{action}");
            assert_eq!(session.file, expected_file, "{action}");
            assert!(session.take_opened_after_scan().is_none(), "{action}");
            assert!(
                session.take_notice().unwrap().0.contains("was not opened"),
                "{action}"
            );
            assert_eq!(session.has_recorded_take(), action == "take");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn background_folder_scans_adopt_on_poll_and_cancel_without_replacing_the_library() {
        let dir = fixture_dir();
        std::fs::create_dir_all(dir.join("Loop")).unwrap();
        for (name, frames) in [
            ("Loop/a.wav", 75_600),
            ("Loop/b.wav", 151_200),
            ("Loop/d.wav", 151_200),
            ("c.wav", 9_001),
        ] {
            Pcm {
                sample_rate: 44_100,
                channels: 1,
                samples: vec![0.25; frames],
            }
            .write_wav(&dir.join(name))
            .unwrap();
        }
        let song = dir.join("song.mixjam");
        let data = dir.join("data");
        let mut session = Session::headless(Arc::new(Library::empty()), Project::default())
            .unwrap()
            .with_settings_root(data.clone());
        assert!(session.wave_scan().is_none());
        session.start_wave_scan(&dir, 120.0).unwrap();
        let status = session.wave_scan().unwrap();
        assert_eq!(status.root, dir);
        assert!(status.files_done <= 4);
        // The song opens once the scanned library can resolve it.
        session.open_after_scan(&song).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while session.wave_scan().is_some() {
            assert!(
                std::time::Instant::now() < deadline,
                "the scan did not finish"
            );
            let delay = session.poll();
            if session.wave_scan().is_some() {
                assert_eq!(delay, Some(100));
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(session.library().samples.len(), 4);
        assert_eq!(session.library().folder_tempos().len(), 1);
        let (notice, error) = session.take_notice().unwrap();
        // The pending song did not exist: the folder loaded, the open failed visibly.
        assert!(error && notice.contains("song.mixjam"), "{notice}");
        assert_eq!(crate::project_io::load_scan_cache(&data).len(), 4);
        let cache_path = data.join("wav-scan-cache.json");
        let cache_modified = std::fs::metadata(&cache_path).unwrap().modified().unwrap();
        assert!(data.join("sample-folder.json").is_file());
        // A cancelled scan never replaces the library, even after its thread ends.
        let before = session.library().samples.len();
        session.start_wave_scan(&dir.join("Loop"), 140.0).unwrap();
        session.open_after_scan(&song).unwrap();
        session.cancel_wave_scan();
        assert!(session.wave_scan().is_none());
        let (notice, error) = session.take_notice().unwrap();
        assert!(!error && notice.contains("song.mixjam"), "{notice}");
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(session.poll(), None);
        assert_eq!(session.library().samples.len(), before);
        assert!(session.start_wave_scan(&dir, f64::NAN).is_err());
        // The cancelled scan's song is not opened by a later scan; a queued one is.
        let finish = |session: &mut Session| {
            while session.wave_scan().is_some() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "the scan did not finish"
                );
                session.poll();
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        session.start_wave_scan(&dir, 120.0).unwrap();
        finish(&mut session);
        assert_eq!(
            std::fs::metadata(&cache_path).unwrap().modified().unwrap(),
            cache_modified,
            "An unchanged cached scan must not rewrite its cache."
        );
        let (notice, error) = session.take_notice().unwrap();
        assert!(
            !error && notice.starts_with("Sample folder loaded"),
            "{notice}"
        );
        assert_eq!(session.take_opened_after_scan(), None);
        let source = session.library().samples[0].id;
        session.place(source, 0, 0).unwrap();
        session.save(&song).unwrap();
        session.start_wave_scan(&dir, 120.0).unwrap();
        session.open_after_scan(&song).unwrap();
        finish(&mut session);
        assert_eq!(session.take_opened_after_scan(), Some(song.clone()));
        assert_eq!(session.project().clips.len(), 1);
        assert!(!session.unsaved());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn headless_session_import_save_reopen_render_and_processing_work_without_gui() {
        let dir = fixture_dir();
        let wave = dir.join("source.wav");
        let project = dir.join("song.mixjam");
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
        // The background export writes the same audio and reports through `poll`.
        let background = dir.join("background.wav");
        session.start_export(&background).unwrap();
        assert!(session.start_export(&background).is_err());
        while session.is_exporting() {
            session.poll();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            session.take_notice(),
            Some((format!("Exported {}.", background.display()), false))
        );
        assert_eq!(
            audio::read_wav(&background).unwrap().samples,
            rendered.samples
        );
        // A render reuses decoded sources and drops those no longer used.
        let mut decoded = std::collections::HashMap::new();
        let first =
            audio::prepare_project_reusing(session.project(), &session.library, &mut decoded)
                .unwrap();
        assert_eq!(decoded.keys().copied().collect::<Vec<_>>(), [source]);
        std::fs::remove_file(&wave).unwrap();
        let again =
            audio::prepare_project_reusing(session.project(), &session.library, &mut decoded)
                .unwrap();
        assert_eq!(
            again.to_pcm().unwrap().samples,
            first.to_pcm().unwrap().samples
        );
        let empty = Project::default();
        audio::prepare_project_reusing(&empty, &session.library, &mut decoded).unwrap();
        assert!(decoded.is_empty());
        Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.25; 1000],
        }
        .write_wav(&wave)
        .unwrap();
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
        }
        // The native binary names the backend as `mixjam_techno`. Every such
        // path, in an import or inline, must go through the application API.
        for (name, source) in [
            ("ui.rs", include_str!("ui.rs")),
            ("ui/view.rs", include_str!("ui/view.rs")),
            ("ui/capture.rs", include_str!("ui/capture.rs")),
            ("ui_icons.rs", include_str!("ui_icons.rs")),
        ] {
            for line in source.lines() {
                assert_eq!(
                    line.matches("mixjam_techno::").count(),
                    line.matches("mixjam_techno::application::").count(),
                    "{name} reaches a backend implementation directly: {line}"
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
        session.recorded_take = Some(Arc::new(Pcm {
            sample_rate: 1000,
            channels: 1,
            samples: vec![0.0; 10],
        }));
        assert!(session.needs_save_confirmation());
        assert!(session.has_recorded_take());
        session.discard_recorded_take();
        assert!(!session.needs_save_confirmation());
    }
}
