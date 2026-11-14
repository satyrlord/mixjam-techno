//! The application struct and its frame hooks. `MixJamApp` owns the view state and
//! submits commands to the headless session; this module builds it, polls the
//! session once per frame, and implements the `eframe` application trait.
use super::*;

/// The boards of the lower panel, one tab each. The tab row owns navigation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum BottomModule {
    #[default]
    Mixer,
    Mastering,
    FxReturns,
    StepSynth,
}

impl BottomModule {
    /// Tab order.
    pub(super) const ALL: [Self; 4] = [
        Self::Mixer,
        Self::Mastering,
        Self::FxReturns,
        Self::StepSynth,
    ];

    /// The score toolbar, its ruler, and a 150-point lane viewport.
    pub(super) const ARRANGEMENT_RESERVE: f32 = 105.0 + 150.0;

    /// The panel minimum for a window whose content area is `available` points
    /// high. Mixer prefers 400 points, and Mastering and Effects the height of
    /// their whole boards. A short window reduces each, down to 370, so the
    /// arrangement keeps its toolbar and ruler and a 150-point lane viewport;
    /// Mixer's gain rail shortens and the boards scroll.
    pub(super) fn minimum_height(self, available: f32) -> f32 {
        match self {
            Self::Mixer => (available - Self::ARRANGEMENT_RESERVE).clamp(370.0, 400.0),
            Self::Mastering => {
                (available - Self::ARRANGEMENT_RESERVE).clamp(370.0, master::MODULE_HEIGHT)
            }
            Self::FxReturns => {
                (available - Self::ARRANGEMENT_RESERVE).clamp(370.0, fx::MODULE_HEIGHT)
            }
            Self::StepSynth => {
                (available - Self::ARRANGEMENT_RESERVE).clamp(370.0, step_synth::MODULE_HEIGHT)
            }
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Mixer => "Mixer",
            Self::Mastering => "Mastering",
            Self::FxReturns => "FX returns",
            Self::StepSynth => "Step synth",
        }
    }

    /// The tab's accessible name. The rail and the status bar already use
    /// the bare labels.
    pub(super) fn tab_name(self) -> &'static str {
        match self {
            Self::Mixer => "Mixer tab",
            Self::Mastering => "Mastering tab",
            Self::FxReturns => "FX returns tab",
            Self::StepSynth => "Step synth tab",
        }
    }
}

/// Native frontend. It owns view state and submits commands to the headless session.
pub struct MixJamApp {
    pub(super) session: Session,
    pub(super) status: String,
    pub(super) status_error: bool,
    /// The selected sound, clips, and track.
    pub(super) selection: selection::SelectionState,
    /// The folding panels around the score.
    pub(super) panels: room::Panels,
    /// The score's zoom, follow mode, and layout as last drawn.
    pub(super) score: ScoreState,
    /// What the score toolbar's lane menu and Erase switch show.
    pub(super) score_tools: ScoreTools,
    /// The pointer drags on the score.
    pub(super) gestures: drag::Gestures,
    /// The lane context menu.
    pub(super) clip_menu: ClipMenu,
    /// The waveforms and Step synth patterns that clips paint.
    pub(super) clip_paint: ClipPaintCache,
    /// The song overview above the score.
    pub(super) overview: overview::OverviewState,
    /// The section bookmarks on the bar ruler.
    pub(super) sections: SectionState,
    /// The score's Mix view.
    pub(super) mix: mix_view::MixViewState,
    /// The lower panel's tabs and boards.
    pub(super) boards: BoardsState,
    /// The Step synth board's pattern and file.
    pub(super) step_synth: StepSynthBoard,
    /// The Sounds panel's filters, sort, and list.
    pub(super) sounds: sounds::SoundsPanel,
    /// The command list (Ctrl+K) while it is open.
    pub(super) commands: Option<commands::CommandList>,
    /// The tool rail's icons, once loaded, or why they could not load.
    pub(super) rail_icons: Option<Result<Arc<RailIcons>, String>>,
    /// The Options window's draft while it is open.
    pub(super) options: Option<options::OptionsDraft>,
    /// The sample libraries and their scan.
    pub(super) libraries: libraries::LibrariesState,
    /// The Help windows.
    pub(super) help: modals::HelpWindows,
    /// The sample conflict resolver.
    pub(super) conflicts: modals::SampleConflicts,
    /// The Import sound window and the Import details window.
    pub(super) sample_import: SampleImport,
    /// The Effect Studio window.
    pub(super) effects: EffectsWindow,
    /// The Sample Editor window.
    pub(super) sample_edit: SampleEditWindow,
    /// The Time Stretcher window.
    pub(super) stretch: StretchWindow,
    /// The track whose channel EQ window is open.
    pub(super) eq_track: Option<usize>,
    /// The Album Review window.
    pub(super) album_review: AlbumReviewWindow,
    /// The recording destination and the recorded take's discard.
    pub(super) recording: RecordingState,
    /// The unsaved-changes flow and the typed path prompt.
    pub(super) files: FileRequests,
    /// The UI capture that a capture run writes.
    pub(super) capture: CaptureState,
    /// The agent control channel, when the application was started with one.
    pub(super) agent: Option<agent::Driver>,
}

/// A UI capture: where it goes, its progress, and the views it prepares.
#[derive(Default)]
pub(super) struct CaptureState {
    pub(super) path: Option<PathBuf>,
    requested: bool,
    pass: u32,
    pub(super) last_track: bool,
    pub(super) transport_focus: bool,
    pub(super) focus_ready: bool,
}

/// The first status line: a startup notice, or the idle text.
pub(super) fn startup_status(session: &mut Session) -> (String, bool) {
    session
        .take_notice()
        .unwrap_or_else(|| (DEFAULT_STATUS.into(), false))
}

impl MixJamApp {
    #[cfg_attr(test, allow(dead_code))]
    pub fn new(cc: &eframe::CreationContext<'_>, mut session: Session) -> Self {
        theme::adopt(&cc.egui_ctx, session.options().theme);
        let (status, status_error) = startup_status(&mut session);
        let app = Self::from_session(session, status, status_error);
        if app.capture.path.is_some() {
            cc.egui_ctx.style_mut(|style| {
                style.animation_time = 0.0;
                style.scroll_animation = egui::style::ScrollAnimation::none();
            });
        }
        app
    }

    /// Show the fallback tempo that the current sample folder was scanned with.
    #[cfg_attr(test, allow(dead_code))]
    pub fn set_fallback_bpm(&mut self, bpm: f64) {
        self.libraries.fallback_bpm = bpm;
    }

    pub fn set_startup_messages(&mut self, messages: &[String]) {
        if !messages.is_empty() {
            let messages = messages.join(" ");
            self.status = if self.status == DEFAULT_STATUS {
                messages
            } else {
                format!("{} {messages}", self.status)
            };
            self.status_error = true;
        }
    }

    pub(super) fn from_session(mut session: Session, status: String, status_error: bool) -> Self {
        let capture_path = std::env::var_os("MIXJAM_TECHNO_CAPTURE_TO").map(PathBuf::from);
        let capture_view = if capture_path.is_some() {
            std::env::var("MIXJAM_TECHNO_CAPTURE_VIEW").unwrap_or_default()
        } else {
            String::new()
        };
        let status = with_import_summary(status, session.project());
        if capture_view == "follow" {
            session.seek(40 * TICKS_PER_BAR);
        }
        if !capture_view.is_empty() {
            // Captures run in their own data folder, so the saved layout
            // stays there. Every capture sets the whole layout, so one view
            // never inherits another's panels.
            let mut layout = session.layout();
            layout.tool_names = capture_view != "rail-icons";
            layout.boards = capture_view != "boards-folded";
            layout.maximised = capture_view == "maximised";
            layout.sounds = matches!(capture_view.as_str(), "sounds-panel" | "sample-details");
            layout.overview = capture_view != "overview-folded";
            layout.inspector = capture_view.starts_with("inspector");
            if let Err(error) = session.set_layout(layout) {
                eprintln!("{error}");
            }
        }
        if matches!(
            capture_view.as_str(),
            "master-active"
                | "master-reordered"
                | "master-end"
                | "master-metering"
                | "master-bypass"
        ) {
            // The Booster settings of a D4 factory song, and
            // every mastering pedal at its default.
            let mut chain = session.project().master;
            for kind in PedalKind::ALL {
                chain.set_on(kind, true);
            }
            chain.trim = -3.0;
            chain.booster_eq.bands = [62, 58, 50, 44, 50, 54, 60, 64, 66, 70];
            (
                chain.booster_compressor.drive,
                chain.booster_compressor.speed,
            ) = (1000, 1000);
            chain.booster_compressor.gain = 278;
            chain.booster_wide.spread = 444;
            if capture_view == "master-reordered" {
                chain.move_pedal(9, 0);
                chain.move_pedal(3, 5);
                chain.booster_compressor.on = false;
                chain.tube.on = false;
            }
            if capture_view == "master-bypass" {
                chain.bypass = true;
            }
            let _ = session.edit(EditCommand::SetMaster(chain));
        }
        if matches!(capture_view.as_str(), "mixer-eq" | "mix-view-eq") {
            // Hi-hats tamed: a low cut, a dip at the harsh band, and less top.
            let mut tracks = session.project().tracks.clone();
            tracks[0].eq = mixjam_techno::application::eq::ChannelEq {
                on: true,
                low_cut: 180.0,
                mid_hz: 3500.0,
                mid_db: -4.5,
                mid_q: 1.6,
                high_db: -9.0,
                ..Default::default()
            };
            let master_gain = session.project().master_gain;
            let _ = session.edit(EditCommand::SetMixer {
                tracks,
                master_gain,
            });
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
        let startup_scan = session.wave_scan().is_some();
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
        let section_bookmarks = session.section_bookmarks_setting();
        let follow_on_startup = session.options().follow_on_startup;
        // Starting with Mixer opens the score's Mix view; the lower panel
        // then shows Mastering, since the Mixer tab never shows with it.
        // Starting with Sounds opens the Sounds panel and folds the lower
        // panel, which keeps Mixer for when it opens. Starting with Mastering
        // or Effects opens the lower panel on that tab.
        use mixjam_techno::application::options::StartingModule;
        let start_in_mix_view = session.options().starting_module == StartingModule::Mixer;
        let starting_module = match session.options().starting_module {
            StartingModule::Sounds => BottomModule::Mixer,
            StartingModule::Mixer | StartingModule::Mastering => BottomModule::Mastering,
            StartingModule::Effects => BottomModule::FxReturns,
        };
        let (status, status_error) = if capture_view.is_empty() {
            let mut layout = session.layout();
            match session.options().starting_module {
                StartingModule::Sounds => {
                    layout.boards = false;
                    layout.sounds = true;
                }
                StartingModule::Mastering | StartingModule::Effects => layout.boards = true,
                StartingModule::Mixer => {}
            }
            match session.set_layout(layout) {
                Ok(()) => (status, status_error),
                Err(error) => (error, true),
            }
        } else {
            (status, status_error)
        };
        let options_draft =
            (capture_view.starts_with("options") || needs_sample_folder).then(|| {
                let page = if needs_sample_folder && !capture_view.starts_with("options") {
                    options::Page::Libraries
                } else {
                    options::Page::from_capture(&capture_view)
                };
                options::OptionsDraft::new(&session, page).for_capture(&capture_view)
            });
        // Select the clip whose fades the capture demonstrates, or the clips
        // that the Inspector's pages describe.
        let capture_clips: BTreeSet<u64> = match capture_view.as_str() {
            "clip-fades" | "inspector-clip" => session
                .project()
                .clips
                .first()
                .map(|clip| clip.id)
                .into_iter()
                .collect(),
            "inspector-clips" => session
                .project()
                .clips
                .iter()
                .take(6)
                .map(|clip| clip.id)
                .collect(),
            _ => BTreeSet::new(),
        };
        let inspector_capture = capture_view.starts_with("inspector");
        let mut app = Self {
            session,
            status,
            status_error,
            selection: selection::SelectionState {
                sample: selected_sample,
                clip: capture_clips.iter().next().copied(),
                clips: capture_clips,
                track: (capture_view == "inspector-track").then_some(0),
                ..Default::default()
            },
            panels: room::Panels {
                order: if inspector_capture {
                    [
                        room::Panel::Inspector,
                        room::Panel::Boards,
                        room::Panel::Sounds,
                    ]
                } else if matches!(capture_view.as_str(), "sounds-panel" | "sample-details") {
                    [
                        room::Panel::Sounds,
                        room::Panel::Boards,
                        room::Panel::Inspector,
                    ]
                } else {
                    [
                        room::Panel::Boards,
                        room::Panel::Sounds,
                        room::Panel::Inspector,
                    ]
                },
                inspector_opened: inspector_capture,
                ..Default::default()
            },
            score: ScoreState {
                follow_playhead: follow_on_startup,
                ..Default::default()
            },
            score_tools: ScoreTools {
                automation_lane: matches!(capture_view.as_str(), "automation" | "automation-menu")
                    .then_some(automation::Lane::Volume),
                ..Default::default()
            },
            gestures: Default::default(),
            clip_menu: Default::default(),
            clip_paint: Default::default(),
            overview: Default::default(),
            sections: SectionState {
                bookmarks: section_bookmarks,
                ..Default::default()
            },
            mix: mix_view::MixViewState {
                shown: capture_view.starts_with("mix-view")
                    || (capture_view.is_empty() && start_in_mix_view),
                ..Default::default()
            },
            boards: BoardsState {
                module: if capture_view.starts_with("mixer") || capture_view == "sounds-panel" {
                    BottomModule::Mixer
                } else if capture_view.starts_with("master") || capture_view.starts_with("mix-view")
                {
                    BottomModule::Mastering
                } else if capture_view.starts_with("fx") {
                    BottomModule::FxReturns
                } else if capture_view.contains("step-synth") {
                    BottomModule::StepSynth
                } else if capture_view.is_empty() {
                    starting_module
                } else {
                    BottomModule::Mixer
                },
                master_reveal: match capture_view.as_str() {
                    "master-end" => Some(PedalKind::Limiter),
                    "master-metering" => Some(PedalKind::BusCompressor),
                    _ => None,
                },
                ..Default::default()
            },
            step_synth: StepSynthBoard {
                state: if capture_view == "step-synth-effects" {
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
                effects_expanded: capture_view == "step-synth-effects",
                ..Default::default()
            },
            sounds: Default::default(),
            commands: (capture_view == "command-list").then(commands::CommandList::new),
            rail_icons: None,
            options: options_draft,
            libraries: libraries::LibrariesState {
                close_options_after_scan: needs_sample_folder
                    && startup_scan
                    && capture_view.is_empty(),
                ..Default::default()
            },
            help: modals::HelpWindows {
                main: capture_view == "help",
                ..Default::default()
            },
            conflicts: modals::SampleConflicts {
                open: capture_view == "sample-conflicts",
                ..Default::default()
            },
            sample_import: SampleImport {
                path: import_path,
                details_open: matches!(capture_view.as_str(), "missing-details" | "import-details"),
                ..Default::default()
            },
            effects: EffectsWindow {
                open: matches!(
                    capture_view.as_str(),
                    "effects" | "effects-expanded" | "missing-effects"
                ),
                state: if capture_view == "effects-expanded" {
                    EffectsState {
                        filter: Some(Default::default()),
                        echo: Some(Default::default()),
                        robotize: Some(Default::default()),
                        ..Default::default()
                    }
                } else {
                    Default::default()
                },
                ..Default::default()
            },
            sample_edit: SampleEditWindow {
                open: matches!(capture_view.as_str(), "edit" | "missing-edit"),
                ..Default::default()
            },
            stretch: StretchWindow {
                open: matches!(capture_view.as_str(), "stretch" | "missing-stretch"),
                ..Default::default()
            },
            eq_track: matches!(capture_view.as_str(), "mixer-eq" | "mix-view-eq").then_some(0),
            album_review: AlbumReviewWindow {
                open: capture_view.starts_with("album-review"),
                state: album_review::capture_state(&capture_view),
            },
            recording: RecordingState {
                track: if capture_view == "track64" {
                    TRACK_COUNT - 1
                } else {
                    0
                },
                ..Default::default()
            },
            files: FileRequests {
                pending_action: (capture_view == "unsaved").then_some(PendingAction::New),
                ..Default::default()
            },
            capture: CaptureState {
                path: capture_path,
                last_track: capture_view == "track64",
                transport_focus: capture_view == "transport-focus",
                ..Default::default()
            },
            agent: None,
        };
        app.resolve_room();
        app
    }

    pub(super) fn capture_view_is(&self, view: &str) -> bool {
        self.capture.path.is_some()
            && std::env::var("MIXJAM_TECHNO_CAPTURE_VIEW").as_deref() == Ok(view)
    }

    pub(super) fn sync_notice(&mut self) -> bool {
        let Some((status, error)) = self.session.take_notice() else {
            return false;
        };
        self.status = status;
        self.status_error = error;
        true
    }

    pub(super) fn poll_application(&mut self, ctx: &egui::Context) {
        let recording = self.session.is_recording();
        if let Some(delay) = self.session.poll() {
            ctx.request_repaint_after(Duration::from_millis(delay));
        }
        self.poll_stretch();
        if self.session.take_wave_folder_adopted() {
            self.clear_selection();
            self.sample_edit.start = 0;
            self.sample_edit.end = 0;
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
                        .libraries
                        .scan_status
                        .as_ref()
                        .is_none_or(|shown| *shown == self.status) =>
            {
                self.status = if scan.files_total == 0 {
                    format!("Listing {}…", scan_subject(&scan.roots))
                } else {
                    format!(
                        "Scanning {}: {} of {} WAVs",
                        scan_subject(&scan.roots),
                        scan.files_done,
                        scan.files_total
                    )
                };
                self.libraries.scan_status = Some(self.status.clone());
            }
            Some(_) => {}
            None => {
                self.libraries.scan_status = None;
                if std::mem::take(&mut self.libraries.close_options_after_scan)
                    && !self.status_error
                    && !self.session.library().samples.is_empty()
                    && self.sample_libraries_open()
                    && self.options.as_ref().is_some_and(|draft| {
                        !draft.restored && draft.values == *self.session.options()
                    })
                {
                    self.close_options(ctx);
                }
            }
        }
        if recording && !self.session.is_recording() && self.session.has_recorded_take() {
            self.save_recorded_take();
        }
    }

    pub(super) fn message(&mut self, text: impl Into<String>) {
        let _ = self.session.take_notice();
        self.status = text.into();
        self.status_error = false;
    }

    pub(super) fn error(&mut self, text: impl Into<String>) {
        let _ = self.session.take_notice();
        self.status = text.into();
        self.status_error = true;
    }

    pub(super) fn capture(&mut self, ctx: &egui::Context) {
        let Some(path) = self.capture.path.clone() else {
            return;
        };
        if self.capture.transport_focus && !self.capture.focus_ready {
            ctx.request_repaint_after(Duration::from_millis(20));
            return;
        }
        self.capture.pass += 1;
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
            self.files.allow_close = true;
            self.capture.path = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if !self.capture.requested && self.capture.pass >= 3 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.capture.requested = true;
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(20));
        }
    }
}

impl eframe::App for MixJamApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.agent_frame(ctx, raw_input);
    }
}
