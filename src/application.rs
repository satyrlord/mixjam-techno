//! Headless application boundary. UI clients read immutable views and submit typed commands.
pub use crate::{
    APP_VERSION, agent, album_review,
    audio::Pcm,
    effects::EffectsState,
    engine::{
        Clip, Fade, FadeCurve, Fades, MAX_MIXER_GAIN_DB, MAX_PITCH_CENTS, MAX_SECTION_NAME,
        MAX_TICKS, MissingReference, MissingSample, Project, SUPPORTED_BPMS, SampleConflict,
        Section, TICKS_PER_BAR, TRACK_COUNT, TRACK_WIDTH, Track, amplitude_to_db, db_to_amplitude,
        playback_bpm,
    },
    library::{MAX_LIBRARY_FOLDERS, Sample},
    master::{
        EQ_BAND_MAX, EQ_FREQUENCIES, KNOB_MAX, Knob, KnobRange, MasterChain, MeterReading,
        OVER_DBTP, PedalKind, presets,
    },
    peaks::{PeakState, Peaks},
    products::Product,
    synth::{StepSynthState, pattern_beside},
};
/// The channel EQ of each track and its controls.
pub mod eq {
    pub use crate::eq::{ChannelEq, EqParam};
}
/// Validated application preferences, independent of the song model.
pub mod options {
    pub use crate::options::{
        AppOptions, LaneHeight, MeterDisplayRate, Oversampling, PlaybackBufferTarget,
        SampleQuality, SongFolderMode, StartingModule, Theme, WorkspaceLayout,
    };
}
/// The drive of each track and its controls.
pub mod drive {
    pub use crate::drive::{Drive, DriveParam};
}
/// The compressor of each track and its controls.
pub mod compressor {
    pub use crate::compressor::{Compressor, CompressorParam};
}
/// The ducker of each track and its controls.
pub mod ducker {
    pub use crate::ducker::{Ducker, DuckerParam};
}
/// Track automation lanes and their values.
pub mod automation {
    pub use crate::automation::{
        Automation, Lane, Point, REPEAT_GATE_STEP, REPEAT_STEPS, RETURN_CEILING, VOLUME_FLOOR_DB,
        filter_cutoff, return_label, value_at,
    };
}
/// The FX buses, their effects, and the controls of both.
pub mod fx {
    pub use crate::fx::{
        Aetherform, BUSES, DelayCharacter, DelayParam, Division, Echoform, FxBus, FxBuses, Module,
        Range, ReverbCharacter, ReverbParam, SHIMMER_INTERVALS, SpaceModel, TimeMode,
    };
}
use crate::{
    audio::{self, Player},
    engine::Editor,
    library::Library,
    master::MasterControl,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[cfg(test)]
mod boundary_tests;
mod edit;
mod files;
mod gesture;
mod library;
mod recording;
mod references;
mod render_worker;
#[cfg(test)]
mod seam_scan;
#[cfg(test)]
mod sequence_scan;
mod settings;
mod sources;
#[cfg(test)]
mod tests;
mod transport;
mod wave_scan;
pub use edit::{EditCommand, TrackSwitch};
use files::Export;
use gesture::{GESTURE_GAP, Gesture};
pub use library::LibraryView;
use render_worker::RenderWorker;
use sources::StretchJob;
pub use sources::StretchSettings;
use wave_scan::WaveScan;
pub use wave_scan::WaveScanStatus;

/// Owns project history, sources, audio transport, rendering, recording, and persistence.
/// No mutable project, catalog, device, or renderer is exposed to a frontend.
pub struct Session {
    library: Arc<Library>,
    catalog_revision: u64,
    wave_scan: Option<WaveScan>,
    /// Keep a changed catalog pending until a replacement library is adopted.
    catalog_rescan_needed: bool,
    /// A deferred song, with the render revision and editor generation when queued.
    pending_open: Option<(PathBuf, u64, u64)>,
    /// The deferred song, once opened, until a frontend reports it.
    opened_after_scan: Option<PathBuf>,
    /// A successful folder adoption, until the frontend resets its source selection.
    wave_folder_adopted: bool,
    export: Option<Export>,
    stretch_job: Option<StretchJob>,
    stretch_result: Option<(StretchSettings, Result<Pcm, String>)>,
    /// Products whose provider replaces the same product found in a WAV folder.
    preferred_providers: Vec<Product>,
    /// Where the sample folder setting and scan cache live.
    settings_root: PathBuf,
    options: crate::options::AppOptions,
    options_revision: u64,
    options_error: Option<String>,
    options_invalid: bool,
    options_future: bool,
    /// Which workspace panels are open; saved in `layout.json` once the
    /// settings folder is loaded, so headless sessions never write it.
    layout: crate::options::WorkspaceLayout,
    layout_saves: bool,
    editor: Editor,
    player: Option<Player>,
    /// The prepared mix of the current revision. With audio output, each
    /// revision is prepared as it appears; otherwise when it is needed.
    render: Option<Arc<audio::Mix>>,
    render_dirty: bool,
    render_worker: RenderWorker,
    /// Waveform peaks of library sounds, filled by their own worker thread.
    peaks: std::cell::RefCell<crate::peaks::PeakStore>,
    revision: u64,
    requested_revision: Option<u64>,
    /// Whether each revision is prepared before Play asks for it.
    prepares_ahead: bool,
    /// A revision whose preparation failed, which waits for Play to try again.
    failed_revision: Option<u64>,
    wants_playback: bool,
    /// Whether the window has shown that playback runs without an audio
    /// thread's priority. It says so once.
    priority_reported: bool,
    playback_frames_per_tick: f64,
    seek_pending: bool,
    previewing: bool,
    preview_revision: u64,
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
    /// An arrangement point drag has an explicit lifetime, including pauses.
    automation_gesture: Option<(usize, crate::automation::Lane)>,
    /// The FX bus whose return lane a point drag edits, as one undo step.
    return_gesture: Option<usize>,
    /// The pause that ends a gesture: `GESTURE_GAP`, except in some tests.
    gesture_gap: std::time::Duration,
    /// The master pedalboard and FX bus settings that mix playback reads as it
    /// plays, with its Clear Tail requests and meter readings.
    master: Arc<MasterControl>,
}

impl Session {
    pub fn headless(library: Arc<Library>, project: Project) -> Result<Self, String> {
        project.validate()?;
        let master = MasterControl::new(&project.master);
        master.set_returns(&project.fx);
        Ok(Self {
            library,
            catalog_revision: 0,
            wave_scan: None,
            catalog_rescan_needed: false,
            pending_open: None,
            opened_after_scan: None,
            wave_folder_adopted: false,
            export: None,
            stretch_job: None,
            stretch_result: None,
            preferred_providers: Vec::new(),
            settings_root: crate::project_io::data_root(),
            options: crate::options::AppOptions::default(),
            options_revision: 0,
            options_error: None,
            options_invalid: false,
            options_future: false,
            layout: crate::options::WorkspaceLayout::default(),
            layout_saves: false,
            editor: Editor::new(project),
            player: None,
            render: None,
            render_dirty: true,
            render_worker: RenderWorker::new(),
            peaks: Default::default(),
            revision: 0,
            requested_revision: None,
            prepares_ahead: false,
            failed_revision: None,
            wants_playback: false,
            priority_reported: false,
            playback_frames_per_tick: 1.0,
            seek_pending: false,
            previewing: false,
            preview_revision: 0,
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
            automation_gesture: None,
            return_gesture: None,
            gesture_gap: GESTURE_GAP,
            master,
        })
    }
    pub fn new(library: Arc<Library>, project: Project) -> Result<Self, String> {
        let mut session = Self::headless(library, project)?;
        session.load_options();
        match Player::new_with_buffer(session.options.playback_buffer_target) {
            Ok(mut player) => {
                player.set_preview_gain(session.options.preview_gain());
                session.player = Some(player);
                session.prepares_ahead = true;
                session.prepare_ahead();
            }
            Err(error) => session.error(format!("Audio output is unavailable: {error}")),
        }
        Ok(session)
    }
    pub fn project(&self) -> &Project {
        self.editor.project()
    }
    /// Identify the project state for a deferred frontend action. Render-only
    /// and history-only changes both invalidate an earlier request.
    pub fn project_change_token(&self) -> (u64, u64) {
        (self.revision, self.editor.generation())
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

    pub fn take_notice(&mut self) -> Option<(String, bool)> {
        self.notice.take()
    }
    fn message(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), false));
    }
    fn error(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), true));
    }
    /// Start editing another project: a new history, and no gesture of the
    /// old project continues into it.
    fn replace_project(&mut self, project: Project) {
        self.editor = Editor::new(project);
        self.gesture = None;
        self.automation_gesture = None;
        self.return_gesture = None;
        self.master.set(&self.editor.project().master);
        self.master.set_returns(&self.editor.project().fx);
        self.master.set_levels(self.editor.project());
        self.revision = self.revision.wrapping_add(1);
        self.render = None;
        self.render_dirty = true;
    }
    fn changed(&mut self) {
        self.gesture = None;
        self.unsaved = true;
        self.render_dirty = true;
        self.revision = self.revision.wrapping_add(1);
        self.master.set(&self.editor.project().master);
        self.master.set_returns(&self.editor.project().fx);
        self.master.set_levels(self.editor.project());
        if self.wants_playback {
            self.start_playback();
        } else {
            self.prepare_ahead();
        }
    }
    /// A change of the song's sections only. They do not change the sound, so
    /// the prepared mix and playback continue.
    fn sections_changed(&mut self) {
        self.gesture = None;
        self.unsaved = true;
    }
    /// A change of track faders, pans, polarity, or the master gain only.
    /// Playback reads the levels as it plays, so the prepared mix stays
    /// valid for it and glides to them. Export checks the mix's own levels.
    fn levels_changed(&mut self) {
        self.gesture = None;
        self.unsaved = true;
        self.master.set_levels(self.editor.project());
    }
    /// A change of the master pedalboard or the FX buses only. The prepared
    /// mix stays valid, and playback continues with the new settings.
    fn master_changed(&mut self) {
        self.gesture = None;
        self.unsaved = true;
        self.master.set(&self.editor.project().master);
        self.master.set_returns(&self.editor.project().fx);
    }
    /// Advances devices and the render queue. Returns a suggested polling delay in milliseconds.
    pub fn poll(&mut self) -> Option<u64> {
        self.poll_capture();
        self.poll_stretch();
        let scan_delay = match (self.poll_wave_scan(), self.poll_export()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.poll_render();
        let peaks_pending = self.peaks.get_mut().poll();
        self.prepare_ahead();
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
        if self.player.is_some()
            && !self.priority_reported
            && let Some(failure) = audio::priority_failure()
        {
            self.priority_reported = true;
            self.error(failure);
        }
        let delay = if self.render_worker.in_flight || self.is_stretching() || peaks_pending {
            Some(20)
        } else if self.playing_mix() || self.is_previewing() {
            Some(30)
        } else if self.is_recording() {
            Some(40)
        } else if self.player.is_some() {
            // Refresh capture context and report worker results even when
            // neither transport nor user input wakes the window.
            Some(250)
        } else {
            None
        };
        match (delay, scan_delay) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

fn duration_ticks(frames: u64, rate: u32, bpm: f64) -> u32 {
    (frames as f64 * bpm / rate as f64 / 240.0 * TICKS_PER_BAR as f64)
        .round()
        .max(1.0) as u32
}
