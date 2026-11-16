//! The lower panel and its boards: the top row, the toolbars, and the Mixer,
//! Mastering, FX returns, and Step synth boards, with the Mix view's block of
//! Master and return rows below the lanes.
use super::*;
use mixjam_techno::application::{
    fx::{BUSES, FxBuses},
    options::WorkspaceLayout,
};

/// The lower panel's margins. The bottom margin is smaller than the top one
/// because the boards end in their own scroll bar or inset.
const BOTTOM_PANEL_MARGIN: egui::Margin = egui::Margin {
    left: 18,
    right: 18,
    top: 10,
    bottom: 6,
};

/// The height of the lower panel's toolbar row.
const TOOLBAR_HEIGHT: f32 = 28.0;

/// The space between the module toolbar and a board (Mixer, Mastering, or
/// FX returns). The margins give it up, so a board keeps its height.
const BOARD_GAP: f32 = 8.0;

/// The lower panel: its shown board, its meters, and what its boards bring into
/// view.
#[derive(Default)]
pub(in super::super) struct BoardsState {
    pub(in super::super) module: BottomModule,
    pub(in super::super) meter_display: options::MeterDisplay,
    /// A pedal that the Mastering module brings into view on its next frame.
    pub(in super::super) master_reveal: Option<PedalKind>,
    /// Replacing the project invalidates the board's active widget gestures.
    pub(in super::super) master_view_generation: u64,
    /// An FX bus that the FX returns board brings into view on its next frame.
    pub(in super::super) fx_reveal: Option<usize>,
}

/// The Step synth board: its pattern with its source, the bar it shows, and the
/// result of its last action.
#[derive(Default)]
pub(in super::super) struct StepSynthBoard {
    pub(in super::super) state: StepSynthState,
    pub(in super::super) bar: usize,
    pub(in super::super) effects_expanded: bool,
    pub(in super::super) feedback: StepSynthFeedback,
    /// The idle hint for the status bar, set while the board's toolbar draws.
    pub(in super::super) hint: Option<&'static str>,
    /// The grid's view: octave, scale, and the selected steps.
    pub(in super::super) grid: step_synth::Editor,
    pub(in super::super) listening: Option<StepSynthAudition>,
    /// The toolbar's rectangle and layer, for the board's keyboard shortcuts.
    pub(in super::super) toolbar: Option<(Rect, egui::LayerId)>,
    /// The source that Start and Loop source were last set for, and its
    /// length in ms. A load clears it, so the loaded Start stays.
    pub(in super::super) adopted: Option<(SourceKey, Option<f64>)>,
    /// The catalog observed for the cached source, and its decoded audio.
    /// A catalog change refreshes the cache without resetting an unchanged sound.
    pub(in super::super) adopted_revision: u64,
    pub(in super::super) adopted_audio: Option<u64>,
    /// What Save sound tells after the save, such as a level change.
    pub(in super::super) save_note: Option<String>,
}

/// Which audio the pattern plays: a source file, or a palette sound.
#[derive(Clone, Debug, PartialEq)]
pub(in super::super) enum SourceKey {
    File(std::path::PathBuf),
    Palette(Option<u32>),
}

/// Identify decoded audio only when the source or its catalog changes.
fn source_audio_signature(source: &Pcm) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    source.sample_rate.hash(&mut hash);
    source.channels.hash(&mut hash);
    source.samples.len().hash(&mut hash);
    for sample in &source.samples {
        sample.to_bits().hash(&mut hash);
    }
    hash.finish()
}

/// Timing and identity of the rendered preview, independent of later draft edits.
pub(in super::super) struct StepSynthAudition {
    pub(in super::super) start: std::time::Instant,
    pub(in super::super) bpm: f64,
    pub(in super::super) length: u8,
    pub(in super::super) swing: u8,
    pub(in super::super) preview_revision: u64,
}

impl StepSynthAudition {
    pub(in super::super) fn is_current(&self, previewing: bool, revision: u64) -> bool {
        previewing && revision == self.preview_revision
    }

    pub(in super::super) fn step(&self, elapsed: std::time::Duration) -> usize {
        let steps = elapsed.as_secs_f64() * self.bpm / 15.0;
        let pair = (steps / 2.0).floor() as usize;
        let second = steps - pair as f64 * 2.0 >= f64::from(self.swing) / 50.0;
        (pair * 2 + usize::from(second)) % usize::from(self.length.max(1))
    }
}

impl MixJamApp {
    /// Show the score's Mix view or its Arrange view. The Mix view and the
    /// Mixer board never show at once, so the lower panel leaves Mixer for
    /// Mastering. The Mix view is its own mode: it folds the lower panel so
    /// its rows take the whole score, and leaving it opens the panel again
    /// unless the panel was opened or folded in between.
    pub(in super::super) fn set_mix_view(&mut self, on: bool) {
        if on && self.boards.module == BottomModule::Mixer {
            self.boards.module = BottomModule::Mastering;
        }
        if on == self.mix.shown {
            return;
        }
        self.mix.shown = on;
        if on {
            let open = self.session.layout().boards;
            if open {
                self.set_layout(|layout| layout.boards = false);
            }
            self.mix.restore_boards = open;
        } else if std::mem::take(&mut self.mix.restore_boards) && !self.session.layout().boards {
            self.set_layout(|layout| layout.boards = true);
        }
    }

    /// The height the score keeps below its lanes: the Mix view's Master and
    /// returns rows, which stay in view while the tracks scroll.
    pub(in super::super) fn score_block_height(&self) -> f32 {
        if self.mix.shown {
            mix_view::BLOCK_HEIGHT
        } else {
            0.0
        }
    }

    /// The lower panel's least height for the open board in a content area
    /// `available` points high, with the room the score keeps below its lanes.
    pub(in super::super) fn board_minimum(&self, module: BottomModule, available: f32) -> f32 {
        module.minimum_height(available, self.score_block_height())
    }

    /// True when `module`'s board shows in an open lower panel.
    pub(in super::super) fn board_shown(&self, module: BottomModule) -> bool {
        self.boards.module == module && self.panels.room.boards
    }

    /// Show `module`'s board, or fold the lower panel when it already shows.
    pub(in super::super) fn toggle_board(&mut self, module: BottomModule) {
        if self.board_shown(module) {
            self.set_layout(|layout| layout.boards = false);
        } else {
            self.boards.module = module;
            self.set_layout(|layout| {
                layout.boards = true;
                layout.maximised = false;
            });
            self.claim(room::Panel::Boards);
        }
    }

    /// Change the workspace layout and keep it for later sessions.
    /// A panel that this change opens takes the room it shares first.
    pub(in super::super) fn set_layout(&mut self, change: impl FnOnce(&mut WorkspaceLayout)) {
        let before = self.session.layout();
        let mut layout = before;
        change(&mut layout);
        if layout.boards != before.boards {
            // The lower panel was opened or folded by hand, so leaving the
            // Mix view keeps it as it is.
            self.mix.restore_boards = false;
        }
        if let Err(error) = self.session.set_layout(layout) {
            self.error(error);
        }
        for (opened, panel) in [
            (layout.boards && !before.boards, room::Panel::Boards),
            (layout.sounds && !before.sounds, room::Panel::Sounds),
            (
                layout.inspector && !before.inspector,
                room::Panel::Inspector,
            ),
        ] {
            if opened {
                self.claim(panel);
            }
        }
        self.resolve_room();
    }

    pub(in super::super) fn bottom(&mut self, ctx: &egui::Context) {
        if self.session.layout().maximised {
            return;
        }
        // Folded, the lower panel leaves the score its height; the rail's
        // board buttons and the status bar's Boards switch open it again.
        if self.panels.room.boards {
            self.lower_panel(ctx);
        }
    }

    fn lower_panel(&mut self, ctx: &egui::Context) {
        let height = (ctx.content_rect().height() * 0.28).clamp(220.0, 290.0);
        // Reserve the score toolbar, its ruler and a usable lane viewport.
        let available = ctx.available_rect().height();
        let min_height = self.board_minimum(self.boards.module, available);
        // Keep the resized section stable when switching to a shorter module.
        let shared_minimum = BottomModule::ALL
            .into_iter()
            .map(|module| self.board_minimum(module, available))
            .fold(0.0_f32, f32::max);
        let mut max_height = (available - 290.0 - self.score_block_height()).max(shared_minimum);
        if self.panels.room.sounds {
            // An open Sounds panel keeps its least height above the board, so
            // a drag never folds one of them.
            max_height = max_height.min((available - room::SOUNDS_MIN_HEIGHT).max(min_height));
        }
        let sounds = egui::TopBottomPanel::bottom("sounds")
            .default_height(height)
            .height_range(min_height..=max_height)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(roles().panel)
                    .inner_margin(BOTTOM_PANEL_MARGIN),
            )
            .show(ctx, |ui| {
                ui.set_min_height(ui.available_height());
                let row_spacing = ui.spacing().item_spacing.y;
                ui.spacing_mut().item_spacing.y = 0.0;
                self.bottom_toolbar(ui);
                ui.spacing_mut().item_spacing.y = row_spacing;
                ui.add_space(BOARD_GAP);
                match self.boards.module {
                    BottomModule::Mixer => self.mixer(ui),
                    BottomModule::Mastering => self.mastering(ui),
                    BottomModule::FxReturns => self.effects_module(ui),
                    BottomModule::StepSynth => self.step_synth_board(ui),
                }
            });
        help::note_board_rect(ctx, sounds.response.rect);
        // egui makes the panel's drag edge a Tab stop and leaves it unnamed.
        let edge = egui::Id::new("sounds").with("__resize");
        if ctx.read_response(edge).is_some() {
            ctx.accesskit_node_builder(edge, |node| {
                node.set_role(egui::accesskit::Role::Splitter);
                node.set_label("Lower panel height");
            });
        }
        let grip = Rect::from_center_size(
            Pos2::new(
                sounds.response.rect.center().x,
                sounds.response.rect.top() + 2.0,
            ),
            Vec2::new(48.0, 3.0),
        );
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(grip, 2.0, roles().muted);
    }

    /// The lower panel's top row: the shown board's title, then its own
    /// controls. The tool rail chooses the board.
    fn bottom_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.push_id("bottom-module-toolbar", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().interact_size.y = TOOLBAR_HEIGHT;
                ui.add_space(6.0);
                ui.add(
                    egui::Label::new(heading(self.boards.module.label(), 16.0)).selectable(false),
                );
                ui.add_space(4.0);
                ui.separator();
                match self.boards.module {
                    BottomModule::Mastering => self.mastering_toolbar(ui),
                    BottomModule::Mixer => self.mixer_toolbar(ui),
                    BottomModule::StepSynth => self.step_synth_toolbar(ui),
                    BottomModule::FxReturns => {}
                }
            });
        });
    }

    /// The Step synth board's pane switch, undo keys, Import details for
    /// missing audio, and its help at the row's end. The board's idle hint
    /// goes to the status bar.
    fn step_synth_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
            let (name, available, missing, explicit_file) = self.step_synth_source();
            let icons = self.icons(ui.ctx());
            let source = step_synth::Source {
                name: &name,
                available,
                missing,
                explicit_file,
                length_ms: self.step_synth.adopted.as_ref().and_then(|(_, ms)| *ms),
            };
            self.step_synth.hint =
                Some(step_synth::hint(&source, self.step_synth.effects_expanded));
            let action = step_synth::toolbar(
                ui,
                ui.available_width() - 40.0,
                source,
                &mut self.step_synth.effects_expanded,
                step_synth::History {
                    undo: self.step_synth.grid.can_undo(false),
                    redo: self.step_synth.grid.can_undo(true),
                    icons: icons.as_deref(),
                },
            );
            match action {
                step_synth::Action::Details => self.sample_import.details_open = true,
                step_synth::Action::Undo => self.step_synth_undo(false),
                step_synth::Action::Redo => self.step_synth_undo(true),
                _ => {}
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                if controls::header_button(
                    ui,
                    rect,
                    "step-synth-help",
                    "Step synth help",
                    controls::HeaderIcon::Help,
                )
                .clicked()
                {
                    self.help.open_tool(help::Topic::StepSynth);
                }
            });
            let area = ui.min_rect();
            self.step_synth.toolbar = Some((area, ui.layer_id()));
            ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
                node.set_role(egui::accesskit::Role::Toolbar);
                node.set_label("Step synth");
                node.set_bounds(egui::accesskit::Rect {
                    x0: area.left().into(),
                    y0: area.top().into(),
                    x1: area.right().into(),
                    y1: area.bottom().into(),
                });
            });
        });
    }

    /// A new palette source clears its old feedback, even when the board is
    /// hidden. An unrelated status message and an explicit file stay intact.
    pub(in super::super) fn reconcile_step_synth_feedback(&mut self) {
        let palette_source = self
            .source_id()
            .filter(|_| self.step_synth.state.source.is_none());
        if self.step_synth.feedback.source != palette_source {
            if self.step_synth.feedback.message.as_ref() == Some(&self.status) {
                self.message(DEFAULT_STATUS);
            }
            self.step_synth.feedback = StepSynthFeedback {
                source: palette_source,
                ..Default::default()
            };
        }
    }

    /// The Step synth source: its name, and whether it is available, missing,
    /// or an explicit file.
    fn step_synth_source(&self) -> (String, bool, bool, bool) {
        let explicit_file = self.step_synth.state.source.is_some();
        let source_id = self.source_id();
        let name = self.step_synth.state.source.as_ref().map_or_else(
            || self.source_name(),
            |path| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            },
        );
        let missing =
            !explicit_file && source_id.is_some_and(|id| self.session.missing_sample(id).is_some());
        let available = explicit_file
            || source_id.is_some_and(|id| self.session.library().sample_by_id(id).is_some());
        (name, available, missing, explicit_file)
    }

    /// Set Start and Loop source for a new source: a new sound starts from
    /// its own first hit, and does not loop until asked. The change is one
    /// undo step. The first source after a load or a start keeps the pattern's
    /// settings. Returns the source's length in ms, when it can be read.
    fn step_synth_adopt_source(&mut self) -> Option<f64> {
        let revision = if self.step_synth.state.source.is_some() {
            0
        } else {
            self.session.catalog_revision()
        };
        let key = match &self.step_synth.state.source {
            Some(path) => SourceKey::File(path.clone()),
            None => SourceKey::Palette(self.source_id()),
        };
        if let Some((adopted, length)) = &self.step_synth.adopted
            && *adopted == key
            && self.step_synth.adopted_revision == revision
        {
            return *length;
        }
        let source = self
            .session
            .step_synth_source(self.source_id(), &self.step_synth.state)
            .ok();
        let length = source
            .as_ref()
            .filter(|pcm| pcm.sample_rate > 0)
            .map(|pcm| pcm.frames() as f64 * 1000.0 / f64::from(pcm.sample_rate));
        let first = self.step_synth.adopted.is_none();
        let audio = source.as_ref().map(source_audio_signature);
        let changed = self
            .step_synth
            .adopted
            .as_ref()
            .is_some_and(|(adopted, _)| *adopted != key)
            || self.step_synth.adopted_audio != audio;
        self.step_synth.adopted = Some((key, length));
        self.step_synth.adopted_revision = revision;
        self.step_synth.adopted_audio = audio;
        let Some(source) = source.filter(|_| !first && changed) else {
            return length;
        };
        let start = mixjam_techno::application::pattern::first_hit(&source);
        let state = &mut self.step_synth.state;
        let looped = state.loop_source;
        if state.start_ms == start && !looped {
            return length;
        }
        self.step_synth.grid.edit("the source change");
        state.start_ms = start;
        state.loop_source = false;
        self.step_synth_say(
            if looped {
                format!(
                    "New source: Start set to its first hit, {start:.0} ms, and Loop source \
                     turned off."
                )
            } else {
                format!("New source: Start set to its first hit, {start:.0} ms.")
            },
            false,
        );
        length
    }

    /// The Step synth board of the lower panel, and what its controls ask for.
    fn step_synth_board(&mut self, ui: &mut egui::Ui) {
        let (name, available, missing, explicit_file) = self.step_synth_source();
        let length_ms = self.step_synth_adopt_source();
        let playing = self
            .step_synth_playing()
            .inspect(|_| ui.ctx().request_repaint());
        let icons = self.icons(ui.ctx());
        let action = step_synth::show(
            ui,
            step_synth::Source {
                name: &name,
                available,
                missing,
                explicit_file,
                length_ms,
            },
            &mut self.step_synth.state,
            &mut self.step_synth.bar,
            self.step_synth.effects_expanded,
            &mut self.step_synth.grid,
            playing,
            icons.as_deref(),
        );
        if let Some(notice) = self.step_synth.grid.take_notice() {
            self.step_synth_say(notice, false);
        }
        match action {
            step_synth::Action::ChooseSource => self.ask_path(FileRequest::SynthSource),
            step_synth::Action::PaletteSource => {
                self.step_synth.grid.edit("the source change");
                self.step_synth.state.source = None;
                // The status line goes back to the board's idle hint for the
                // palette source, such as missing audio.
                self.step_synth.feedback = Default::default();
                self.message(DEFAULT_STATUS);
            }
            step_synth::Action::FindStart => self.step_synth_find_start(),
            step_synth::Action::Listen | step_synth::Action::Stop => self.step_synth_listen(),
            step_synth::Action::SaveSound => match self.step_synth_pcm() {
                Ok((pcm, note)) => {
                    self.step_synth_say("Choose a WAV path to save the sound.", false);
                    self.step_synth.save_note = note;
                    self.save_pcm(
                        pcm,
                        "Step synth sound",
                        "Step synth",
                        self.step_synth.state.bpm,
                    );
                }
                Err(error) => self.step_synth_say(error, true),
            },
            step_synth::Action::SavePattern => self.ask_path(FileRequest::SavePattern),
            step_synth::Action::LoadPattern => self.ask_path(FileRequest::LoadPattern),
            step_synth::Action::Details => self.sample_import.details_open = true,
            step_synth::Action::Undo => self.step_synth_undo(false),
            step_synth::Action::Redo => self.step_synth_undo(true),
            step_synth::Action::None => {}
        }
    }

    /// The pattern step Listen is playing, while its preview lasts.
    fn step_synth_playing(&mut self) -> Option<usize> {
        if self.step_synth.listening.as_ref().is_some_and(|heard| {
            !heard.is_current(
                self.session.is_previewing(),
                self.session.preview_revision(),
            )
        }) {
            self.step_synth.listening = None;
            // The preview ended on its own: its Listening text is stale.
            if !self.status_error && self.status.starts_with("Listening to the Step synth") {
                self.message("Preview ended.");
                self.step_synth_notice();
            }
        }
        self.step_synth
            .listening
            .as_ref()
            .map(|heard| heard.step(heard.start.elapsed()))
    }

    /// Take back the Step synth board's last note edit, or with `redo` put
    /// it back. The board's undo is apart from the song's.
    pub(in super::super) fn step_synth_undo(&mut self, redo: bool) {
        let source = self.step_synth.state.source.clone();
        let restored = self.step_synth.grid.undo(&mut self.step_synth.state, redo);
        if restored.is_some() && source != self.step_synth.state.source {
            // History restores Start and Loop source with the source itself.
            // Refresh its length without applying the new-source defaults.
            self.step_synth.adopted = None;
        }
        let text = match restored {
            Some(what) if redo => format!("Redid {what}."),
            Some(what) => format!("Undid {what}."),
            None if redo => "No Step synth edit to redo.".into(),
            None => "No Step synth edit to undo.".into(),
        };
        self.step_synth_say(text, false);
    }

    /// Listen to the Step synth pattern, or stop it while it plays. The
    /// board's Listen key and the L key both run it.
    pub(in super::super) fn step_synth_listen(&mut self) {
        if self.step_synth_playing().is_some() {
            // Pause keeps the song's position, as stopping a preview should.
            self.pause();
            self.step_synth.listening = None;
            self.message("Preview stopped.");
            self.step_synth_notice();
            return;
        }
        match self.step_synth_pcm() {
            Ok((pcm, note)) => {
                let previous = self.session.preview_revision();
                self.play_pcm(
                    pcm,
                    &format!(
                        "Listening to the Step synth pattern.{}",
                        note.unwrap_or_default()
                    ),
                );
                let state = &self.step_synth.state;
                self.step_synth.listening = (self.session.is_previewing()
                    && self.session.preview_revision() != previous)
                    .then(|| StepSynthAudition {
                        start: std::time::Instant::now(),
                        bpm: state.bpm,
                        length: state.length,
                        swing: state.swing,
                        preview_revision: self.session.preview_revision(),
                    });
            }
            Err(error) => self.error(error),
        }
        self.step_synth_notice();
    }

    /// The Mixer's toolbar: Export tracks, which writes each track and FX
    /// return to its own WAV file for mixing elsewhere.
    fn mixer_toolbar(&mut self, ui: &mut egui::Ui) {
        self.export_tracks_button(ui, 13.0);
    }

    /// Export tracks…, in the Mixer's toolbar and the Mix view's Master row.
    fn export_tracks_button(&mut self, ui: &mut egui::Ui, size: f32) {
        let response = ui.add_enabled(
            !self.session.is_exporting(),
            egui::Button::new(egui::RichText::new("Export tracks…").size(size)),
        );
        if name_control(response, "Export tracks")
            .hover_hint(
                "Write each track and each FX return to its own stereo WAV file in a folder, \
                 for mixing in another application. Tracks play alone after their EQ, \
                 automation, fader, pan, and outputs, without their sends; the FX returns \
                 hold every send. The master pedalboard is left out. Together the files sum \
                 to the mix with the board bypassed when that sum does not clip.",
            )
            .disabled_hint("Wait for the current export to finish.")
            .clicked()
        {
            self.ask_path(FileRequest::ExportTracks);
        }
    }

    pub(in super::super) fn mixer(&mut self, ui: &mut egui::Ui) {
        let mut master = self.session.project().master_gain;
        let mut buses = self.session.project().fx;
        let mut board = self.session.project().master;
        let changes = ui
            .push_id(self.boards.master_view_generation, |ui| {
                mixer::show(
                    ui,
                    self.session.project(),
                    self.session.library(),
                    &mut master,
                    &mut buses,
                    &mut board.bypass,
                )
            })
            .inner;
        self.apply_mixer_changes(ui, changes, master, buses, board);
    }

    /// Applies what the Mixer's strips or the Mix view's rows changed: the
    /// tracks and master level, the FX buses, the master bypass, a bus to
    /// show on the FX returns board, and a track's EQ window to open.
    pub(in super::super) fn apply_mixer_changes(
        &mut self,
        ui: &egui::Ui,
        mut changes: mixer::Changes,
        master: f32,
        buses: FxBuses,
        board: MasterChain,
    ) {
        // One track is the usual edit. The master level, or more than one
        // track in a frame, stays one undo step through the whole mixer.
        let command = match changes.tracks.len() {
            0 if !changes.master => None,
            1 if !changes.master => {
                changes
                    .tracks
                    .pop()
                    .map(|(index, track)| EditCommand::SetTrack {
                        index,
                        track: Box::new(track),
                    })
            }
            _ => {
                let mut tracks = self.session.project().tracks.clone();
                for (index, track) in changes.tracks.drain(..) {
                    tracks[index] = track;
                }
                Some(EditCommand::SetMixer {
                    tracks,
                    master_gain: master,
                })
            }
        };
        if let Some(command) = command {
            let result = self.session.edit(command);
            self.apply(result);
            ui.ctx().request_repaint();
        }
        if changes.fx {
            let result = self.session.edit(EditCommand::SetEffects(buses));
            self.apply(result);
            ui.ctx().request_repaint();
        }
        if changes.bypass {
            let result = self.session.edit(EditCommand::SetMaster(board));
            self.apply(result);
            ui.ctx().request_repaint();
        }
        if let Some(bus) = changes.open {
            self.boards.module = BottomModule::FxReturns;
            self.boards.fx_reveal = Some(bus);
            // The Mix view's returns can open a folded lower panel.
            self.set_layout(|layout| {
                layout.boards = true;
                layout.maximised = false;
            });
            self.claim(room::Panel::Boards);
            ui.ctx().request_repaint();
        }
        if let Some(track) = changes.eq {
            self.eq_track = Some(track);
            ui.ctx().request_repaint();
        }
    }

    fn mastering_toolbar(&mut self, ui: &mut egui::Ui) {
        if let Some(chain) = master::preset_selector(ui, &self.session.project().master) {
            let result = self.session.edit(EditCommand::SetMaster(chain));
            self.apply(result);
        }
        let mut chain = self.session.project().master;
        if master::bypass_switch(ui, &mut chain.bypass) {
            let result = self.session.edit(EditCommand::SetMaster(chain));
            self.apply(result);
        }
        let (status, path) = master::status(&self.session.project().master);
        let room = (ui.available_width() - 40.0).max(0.0);
        ui.allocate_ui(Vec2::new(room, 28.0), |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&status).size(12.0)).truncate())
                .hover_hint(path);
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
            if controls::header_button(
                ui,
                rect,
                "mastering-help",
                "Mastering help",
                controls::HeaderIcon::Help,
            )
            .clicked()
            {
                self.help.open_tool(help::Topic::Mastering);
            }
        });
    }

    pub(in super::super) fn effects_module(&mut self, ui: &mut egui::Ui) {
        let reveal = self.boards.fx_reveal.take();
        let buses = self.session.project().fx;
        let action = ui
            .push_id(self.boards.master_view_generation, |ui| {
                fx::show(ui, &buses, reveal)
            })
            .inner;
        match action {
            fx::Action::Change(buses) => {
                let result = self.session.edit(EditCommand::SetEffects(*buses));
                self.apply(result);
            }
            fx::Action::ClearTail(bus) => self.session.clear_fx_tail(bus),
            fx::Action::None => {}
        }
    }

    pub(in super::super) fn mastering(&mut self, ui: &mut egui::Ui) {
        // A capture of the meters shows a fixed, representative reading.
        let sample = self.capture_view_is("master-metering");
        let reduced = self.session.options().meter_display_rate
            == mixjam_techno::application::options::MeterDisplayRate::Reduced;
        let metering = master::Metering {
            reduced,
            reading: if sample {
                MeterReading {
                    run: 1,
                    input_vu: -1.5,
                    input_peaks: [false, true],
                    input_peak_counts: [0, 1],
                    compressor_reduction: 1.7,
                    limiter_reduction: 2.6,
                    momentary: -12.6,
                    short_term: -13.4,
                    integrated: -13.9,
                    true_peak: -2.4,
                    true_peak_max: -0.8,
                    overs: 1,
                }
            } else {
                self.boards.meter_display.sample(
                    self.session.master_meters(),
                    reduced,
                    ui.input(|input| input.time),
                )
            },
            playing: sample || self.session.playing_mix(),
        };
        let reveal = self.boards.master_reveal.take();
        let chain = self.session.project().master;
        let action = ui
            .push_id(self.boards.master_view_generation, |ui| {
                master::show(ui, &chain, &metering, reveal)
            })
            .inner;
        if let master::Action::Change(chain) = action {
            let result = self.session.edit(EditCommand::SetMaster(chain));
            self.apply(result);
        }
    }

    /// The Mix view's parts outside the lanes: the column captions in the
    /// ruler strip, the Master and returns rows below the lanes, the "No
    /// tracks" note, the columns' scrolling, and the rows' edits.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn mix_block(
        &mut self,
        ui: &mut egui::Ui,
        captions: Rect,
        block: Rect,
        (offset, empty, wheel): (f32, bool, f32),
        mut changes: mixer::Changes,
    ) {
        let column = captions.width();
        let mut new_offset = mix_view::captions(ui, captions, offset) - wheel;
        let mut master = self.session.project().master_gain;
        let mut buses = self.session.project().fx;
        let mut board = self.session.project().master;
        let automated: [bool; BUSES] =
            std::array::from_fn(|bus| !self.session.project().return_automation[bus].is_empty());
        let master_rect = Rect::from_min_size(block.min, Vec2::new(column, mix_view::BLOCK_ROW));
        let returns_rect = master_rect.translate(Vec2::new(0.0, mix_view::BLOCK_ROW + 2.0));
        let row = mix_view::master_row(ui, master_rect, offset, &mut master, &mut board.bypass);
        changes.master |= row.changed;
        changes.bypass = row.bypass;
        let mut export = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(row.export)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        export.set_clip_rect(
            Rect::from_min_max(
                Pos2::new(master_rect.left() + mix_view::PINNED, master_rect.top()),
                master_rect.max,
            )
            .intersect(ui.clip_rect()),
        );
        export.spacing_mut().interact_size.y = row.export.height();
        self.export_tracks_button(&mut export, 12.0);
        let on_board = self.board_shown(BottomModule::FxReturns);
        let (fx, open) =
            mix_view::returns_row(ui, returns_rect, offset, &mut buses, automated, on_board);
        mix_view::edge_fade(ui, master_rect, master_rect, offset, false);
        mix_view::edge_fade(ui, returns_rect, returns_rect, offset, true);
        changes.fx = fx;
        changes.open = open;
        if empty {
            let mut note = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(
                captions.left_bottom() + Vec2::new(mix_view::PINNED + 4.0, 34.0),
                Vec2::new((column - mix_view::PINNED - 8.0).max(80.0), 48.0),
            )));
            note.label(heading("No tracks", 15.0));
            note.label("Place a sound in the arrangement.");
        }
        // Shift and the wheel over the rows scroll the columns, after any
        // control under the pointer has taken the wheel.
        let rows = Rect::from_min_max(
            Pos2::new(captions.left(), captions.top()),
            Pos2::new(captions.right(), block.bottom()),
        );
        if ui.rect_contains_pointer(rows) {
            let delta = ui.input(|input| input.smooth_scroll_delta.x);
            if delta != 0.0 {
                new_offset -= delta;
                ui.input_mut(|input| input.smooth_scroll_delta.x = 0.0);
            }
        }
        // A control reached with the keyboard scrolls into view.
        let focused = ui.memory(|memory| memory.focused());
        if focused != self.mix.revealed {
            self.mix.revealed = focused;
            let visible = captions.left() + mix_view::PINNED..=captions.right();
            if let Some(response) = focused.and_then(|id| ui.ctx().read_response(id))
                && response.interact_rect != response.rect
                && response.rect.width() < 200.0
                && (rows.top()..=rows.bottom()).contains(&response.rect.center().y)
            {
                let content = response.rect.left() - captions.left() + offset;
                if content >= mix_view::PINNED - 0.5
                    && content + response.rect.width() <= mix_view::CONTENT_WIDTH + 0.5
                {
                    if response.rect.left() < *visible.start() {
                        new_offset -= *visible.start() - response.rect.left() + 4.0;
                    } else if response.rect.right() > *visible.end() {
                        new_offset += response.rect.right() - *visible.end() + 4.0;
                    }
                }
            }
        }
        let new_offset = new_offset.clamp(0.0, mix_view::max_offset(column));
        if new_offset != self.mix.scroll {
            self.mix.scroll = new_offset;
            ui.ctx().request_repaint();
        }
        self.apply_mixer_changes(ui, changes, master, buses, board);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_synth_audition_marker_follows_swing_and_preview_identity() {
        let mut heard = StepSynthAudition {
            start: std::time::Instant::now(),
            bpm: 120.0,
            length: 3,
            swing: 75,
            preview_revision: 7,
        };
        let step = |heard: &StepSynthAudition, ms| heard.step(std::time::Duration::from_millis(ms));
        assert_eq!(
            step(&heard, 125),
            0,
            "The swung second step has not started."
        );
        assert_eq!(step(&heard, 188), 1);
        assert_eq!(step(&heard, 250), 2);
        assert_eq!(step(&heard, 375), 2, "Swing uses the full render clock.");
        assert_eq!(step(&heard, 438), 0, "The pattern wraps after three steps.");
        heard.swing = 50;
        assert_eq!(step(&heard, 125), 1);
        assert!(heard.is_current(true, 7));
        assert!(
            !heard.is_current(true, 8),
            "A replacement preview has no marker."
        );
        assert!(
            !heard.is_current(false, 7),
            "A stopped preview has no marker."
        );
    }
}
