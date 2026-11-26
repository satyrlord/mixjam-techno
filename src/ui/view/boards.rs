//! The lower panel and its boards: the top row, the toolbars, and the Mixer,
//! Mastering, FX returns, and Step synth boards.
use super::*;
use mixjam_techno::application::{fx::FxBuses, options::WorkspaceLayout};

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
    pub(in super::super) meter_display: MeterDisplay,
    pub(in super::super) fx_meter_display: fx::MeterDisplay,
    /// A pedal that the Mastering module brings into view on its next frame.
    pub(in super::super) master_reveal: Option<PedalKind>,
    /// Replacing the project invalidates the board's active widget gestures.
    pub(in super::super) master_view_generation: u64,
    /// An FX bus that the FX returns board brings into view on its next frame.
    pub(in super::super) fx_reveal: Option<usize>,
    /// Each track's sound summary, kept until the song or the catalog changes.
    pub(in super::super) summaries: SummaryCache,
}

/// The sound summaries of every track with what they were built from.
#[derive(Default)]
pub(in super::super) struct SummaryCache {
    key: Option<((u64, u64), u64, u64)>,
    summaries: Arc<Vec<(String, String)>>,
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
    /// The lower panel's least height for the open board in a content area
    /// `available` points high.
    pub(in super::super) fn board_minimum(&self, module: BottomModule, available: f32) -> f32 {
        module.minimum_height(available)
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
        if layout == before {
            return;
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
        let mut max_height = (available - 290.0).max(shared_minimum);
        if self.panels.room.sounds {
            // An open Sounds panel keeps its least height above the board, so
            // a drag never folds one of them.
            max_height = max_height.min((available - room::SOUNDS_MIN_HEIGHT).max(min_height));
        }
        let sounds = egui::TopBottomPanel::bottom(LOWER_PANEL_ID)
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
        Self::paint_lower_panel_grip(ctx, sounds.response.rect);
    }

    /// Paint the lower panel resize splitter and name its drag edge for screen readers.
    fn paint_lower_panel_grip(ctx: &egui::Context, rect: Rect) {
        let edge = egui::Id::new(LOWER_PANEL_ID).with("__resize");
        if ctx.read_response(edge).is_some() {
            ctx.accesskit_node_builder(edge, |node| {
                node.set_role(egui::accesskit::Role::Splitter);
                node.set_label("Lower panel height");
            });
        }
        let grip = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + 2.0),
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
                    BottomModule::FxReturns => {
                        if controls::button(
                            ui,
                            "Clear all FX tails",
                            controls::ButtonRole::Secondary,
                        )
                        .hover_hint("Fade every active FX return tail cleanly to silence.")
                        .clicked()
                        {
                            self.session.clear_all_fx_tails();
                        }
                    }
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
        self.export_tracks_button(ui, theme::type_size::BODY);
    }

    /// Export tracks…, in the Mixer's toolbar.
    pub(in super::super) fn export_tracks_button(&mut self, ui: &mut egui::Ui, size: f32) {
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

    /// Each track's sound summary and the list of its sounds, in one pass
    /// over the clips for as long as the song and the catalog stay as they are.
    pub(in super::super) fn sample_summaries(&mut self) -> Arc<Vec<(String, String)>> {
        let key = (
            self.session.project_change_token(),
            self.session.catalog_revision(),
            // A replacement session starts again from revision 0.
            self.session.instance_id(),
        );
        if self.boards.summaries.key != Some(key) {
            self.boards.summaries = SummaryCache {
                key: Some(key),
                summaries: Arc::new(mixer_sample_summaries(
                    self.session.project(),
                    self.session.library(),
                )),
            };
        }
        self.boards.summaries.summaries.clone()
    }

    pub(in super::super) fn mixer(&mut self, ui: &mut egui::Ui) {
        let summaries = self.sample_summaries();
        let mut master = self.session.project().master_gain;
        let mut buses = self.session.project().fx;
        let mut board = self.session.project().master;
        let metering = self.master_metering(ui.ctx());
        let pedals = PedalKind::ALL
            .into_iter()
            .filter(|kind| board.is_on(*kind))
            .count();
        let changes = ui
            .push_id(self.boards.master_view_generation, |ui| {
                mixer::show(
                    ui,
                    self.session.project(),
                    &summaries,
                    &mut master,
                    &mut buses,
                    mixer::MasterState {
                        bypass: &mut board.bypass,
                        pedals,
                        metering: &metering,
                    },
                )
            })
            .inner;
        self.apply_mixer_changes(ui, changes, master, buses, board);
    }

    /// Applies what the Mixer's strips changed: the tracks and master level,
    /// the FX buses, the master bypass, a bus to show on the FX returns board,
    /// and a track's EQ window to open.
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
            1 if !changes.master => changes
                .tracks
                .pop()
                .map(|(index, mix)| EditCommand::SetTrackMix { index, mix }),
            _ => {
                let mut tracks = self.session.project().tracks.clone();
                for (index, mix) in changes.tracks.drain(..) {
                    tracks[index].set_mix(mix);
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
        if let Some(chain) =
            master::preset_selector(ui, &self.session.project().master, "Mastering preset")
        {
            // A preset is a whole choice, so it never joins a knob turn.
            let result = self.session.edit_discrete(EditCommand::SetMaster(chain));
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
        let meters = self.boards.fx_meter_display.read();
        let solos = std::array::from_fn(|i| self.session.fx_solo(i));
        let action = ui
            .push_id(self.boards.master_view_generation, |ui| {
                fx::show(ui, &buses, reveal, &meters, solos)
            })
            .inner;
        match action {
            fx::Action::Change(buses) => {
                let result = self.session.edit(EditCommand::SetEffects(*buses));
                self.apply(result);
            }
            fx::Action::ClearTail(bus) => self.session.clear_fx_tail(bus),
            fx::Action::ToggleSolo(bus) => self.session.toggle_fx_solo(bus),
            fx::Action::OpenDeepEdit(bus) => self.fx_deep_edit = Some(bus),
            fx::Action::None => {}
        }
    }

    fn master_metering(&mut self, ctx: &egui::Context) -> master::Metering {
        const CAPTURE_SAMPLE_VIEWS: &[&str] = &[
            "master-metering",
            "mixer-states",
            "master-deep-edit-comp",
            "master-deep-edit-multiband",
            "master-deep-edit-limiter",
            "master-deep-edit-imaging",
        ];
        let sample = CAPTURE_SAMPLE_VIEWS
            .iter()
            .any(|view| self.capture_view_is(view));
        let reduced = self.session.options().meter_display_rate
            == mixjam_techno::application::options::MeterDisplayRate::Reduced;
        let (reading, vectorscope) = if sample {
            (
                MeterReading {
                    run: 1,
                    input_vu: -1.5,
                    input_peaks: [false, true],
                    input_peak_counts: [0, 1],
                    compressor_reduction: 1.7,
                    multiband_reduction: [2.1, 1.4, 0.9],
                    limiter_reduction: 2.6,
                    stereo_correlation: 0.78,
                    momentary: -12.6,
                    short_term: -13.4,
                    integrated: -13.9,
                    true_peak: -2.4,
                    true_peak_max: -0.8,
                    overs: 1,
                },
                sample_vectorscope(),
            )
        } else {
            self.boards.meter_display.sample(
                self.session.master_meters(),
                self.session.master_vectorscope(),
                reduced,
                ctx.input(|input| input.time),
            )
        };
        master::Metering {
            reduced,
            reading,
            vectorscope,
            playing: sample || self.session.playing_mix(),
        }
    }

    pub(in super::super) fn mastering(&mut self, ui: &mut egui::Ui) {
        let metering = self.master_metering(ui.ctx());
        let reveal = self.boards.master_reveal.take();
        let chain = self.session.project().master;
        let action = ui
            .push_id(self.boards.master_view_generation, |ui| {
                master::show(ui, &chain, &metering, reveal)
            })
            .inner;
        match action {
            master::Action::Change(chain) => {
                let result = self.session.edit(EditCommand::SetMaster(chain));
                self.apply(result);
            }
            master::Action::OpenDeepEdit(kind) => {
                self.master_deep_edit = Some(kind);
                dialog::expand(ui.ctx(), master::DEEP_EDIT_TITLE);
                ui.ctx().move_to_top(egui::LayerId::new(
                    egui::Order::Middle,
                    egui::Id::new(master::DEEP_EDIT_TITLE),
                ));
                ui.ctx().request_repaint();
            }
            master::Action::None => {}
        }
    }

    pub(in super::super) fn master_deep_edit_window(&mut self, ctx: &egui::Context) {
        let Some(mut active) = self.master_deep_edit else {
            return;
        };
        let mut open = true;
        let chain = self.session.project().master;
        let metering = self.master_metering(ctx);
        let outcome = master::show_deep_edit(
            ctx,
            &mut open,
            &mut active,
            &chain,
            &metering,
            self.boards.master_view_generation,
        );
        if let Some(updated) = outcome.chain {
            let cmd = EditCommand::SetMaster(updated);
            let result = if outcome.discrete {
                self.session.edit_discrete(cmd)
            } else {
                self.session.edit(cmd)
            };
            self.apply(result);
        }
        if outcome.help {
            self.help.open_tool(help::Topic::Mastering);
        }
        self.master_deep_edit = open.then_some(active);
    }
}

/// The decimated `(left, right)` points of one vectorscope frame.
type Vectorscope = [(f32, f32); mixjam_techno::application::VECTORSCOPE_POINTS];

fn sample_vectorscope() -> Vectorscope {
    std::array::from_fn(|i| {
        let t = i as f32 * 0.37;
        let mid = 0.55 * t.sin() + 0.20 * (2.0 * t).cos();
        let side = 0.24 * (1.7 * t + 0.8).cos();
        (
            (mid + side).clamp(-0.95, 0.95),
            (mid - side).clamp(-0.95, 0.95),
        )
    })
}

/// What the master meters show. A reduced display rate holds a reading
/// between draws, and keeps every peak that happened in between. It holds
/// the vectorscope points with the reading, so the scope and its
/// correlation bar change together.
pub(in super::super) struct MeterDisplay {
    shown: MeterReading,
    pending: MeterReading,
    next: f64,
    scope: Vectorscope,
}

impl Default for MeterDisplay {
    fn default() -> Self {
        Self {
            shown: MeterReading::default(),
            pending: MeterReading::default(),
            next: 0.0,
            scope: [(0.0, 0.0); mixjam_techno::application::VECTORSCOPE_POINTS],
        }
    }
}

impl MeterDisplay {
    fn merge_pending(&mut self, reading: MeterReading) {
        let peaks = std::array::from_fn::<_, 2, _>(|side| {
            self.pending.input_peaks[side]
                || self.pending.input_peak_counts[side] != reading.input_peak_counts[side]
        });
        let true_peak = self.pending.true_peak;
        let compressor = self.pending.compressor_reduction;
        let multiband = self.pending.multiband_reduction;
        let limiter = self.pending.limiter_reduction;
        self.pending = reading;
        self.pending.input_peaks =
            std::array::from_fn(|side| peaks[side] || reading.input_peaks[side]);
        self.pending.true_peak = true_peak.max(reading.true_peak);
        self.pending.compressor_reduction = compressor.max(reading.compressor_reduction);
        self.pending.multiband_reduction =
            std::array::from_fn(|band| multiband[band].max(reading.multiband_reduction[band]));
        self.pending.limiter_reduction = limiter.max(reading.limiter_reduction);
    }

    /// Keep transient peaks between draws. Audio still measures every block.
    /// The vectorscope `scope` changes only when the reading does.
    pub(in super::super) fn sample(
        &mut self,
        reading: MeterReading,
        scope: Vectorscope,
        reduced: bool,
        now: f64,
    ) -> (MeterReading, Vectorscope) {
        if self.advance(reading, reduced, now) {
            self.scope = scope;
        }
        (self.shown, self.scope)
    }

    /// Take a reading, and return true when the shown reading changes to it.
    fn advance(&mut self, reading: MeterReading, reduced: bool, now: f64) -> bool {
        if reading.run != self.pending.run {
            self.pending = reading;
            self.pending.input_peaks = std::array::from_fn(|side| {
                reading.input_peaks[side] || reading.input_peak_counts[side] > 0
            });
            self.shown = reading;
            self.next = now;
        } else {
            self.merge_pending(reading);
        }
        let due = !reduced || now >= self.next;
        if due {
            self.shown = self.pending;
            self.pending = reading;
            self.pending.input_peaks = [false; 2];
            self.pending.true_peak = f32::NEG_INFINITY;
            self.pending.compressor_reduction = 0.0;
            self.pending.multiband_reduction = [0.0; 3];
            self.pending.limiter_reduction = 0.0;
            self.next = now + 1.0 / 15.0;
        }
        due
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
