//! The lower panel and its boards: the tab row, the toolbars, and the Mixer,
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
/// Effects). The margins give it up, so a board keeps its height.
const BOARD_GAP: f32 = 8.0;

/// The lower panel: its open tab, its meters, and what its boards bring into
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

/// The Step synth board: its pattern, the bar it shows, its file, and the
/// result of its last action.
#[derive(Default)]
pub(in super::super) struct StepSynthBoard {
    pub(in super::super) state: StepSynthState,
    pub(in super::super) bar: usize,
    pub(in super::super) file: Option<PathBuf>,
    pub(in super::super) effects_expanded: bool,
    pub(in super::super) feedback: StepSynthFeedback,
}

impl MixJamApp {
    /// Show the score's Mix view or its Arrange view. The Mix view and the
    /// Mixer tab never show at once, so the lower panel leaves Mixer for
    /// Mastering.
    pub(in super::super) fn set_mix_view(&mut self, on: bool) {
        self.mix.shown = on;
        if on && self.boards.module == BottomModule::Mixer {
            self.boards.module = BottomModule::Mastering;
        }
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
        if self.panels.room.boards {
            self.lower_panel(ctx);
        } else {
            // Folded: only the toolbar row, so the module choice and the
            // filters stay in reach. Its own panel id keeps the open panel's
            // height for when it opens again.
            egui::TopBottomPanel::bottom("boards-folded")
                .exact_height(
                    TOOLBAR_HEIGHT
                        + f32::from(BOTTOM_PANEL_MARGIN.top)
                        + f32::from(BOTTOM_PANEL_MARGIN.bottom),
                )
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(roles().panel)
                        .inner_margin(BOTTOM_PANEL_MARGIN),
                )
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    self.bottom_toolbar(ui);
                });
        }
    }

    fn lower_panel(&mut self, ctx: &egui::Context) {
        let height = (ctx.content_rect().height() * 0.28).clamp(220.0, 290.0);
        // Reserve the score toolbar, its ruler and a usable lane viewport.
        let available = ctx.available_rect().height();
        let min_height = self.boards.module.minimum_height(available);
        // Keep the resized section stable when switching to a shorter module.
        let shared_minimum = BottomModule::ALL
            .into_iter()
            .map(|module| module.minimum_height(available))
            .fold(0.0_f32, f32::max);
        let mut max_height = (available - 290.0).max(shared_minimum);
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

    /// The lower panel's tab row: one tab per board, then the shown board's
    /// own controls.
    fn bottom_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.push_id("bottom-module-toolbar", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().interact_size.y = TOOLBAR_HEIGHT;
                let spacing = ui.spacing().item_spacing.x;
                ui.spacing_mut().item_spacing.x = 2.0;
                let boards = self.panels.room.boards;
                for module in BottomModule::ALL {
                    let selected = self.boards.module == module;
                    // Choosing a tab opens a folded lower panel.
                    if board_tab(ui, module, selected, boards).clicked() {
                        self.boards.module = module;
                        if module == BottomModule::Mixer {
                            self.mix.shown = false;
                        }
                        self.set_layout(|layout| layout.boards = true);
                        self.claim(room::Panel::Boards);
                    }
                }
                ui.spacing_mut().item_spacing.x = spacing;
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

    /// The Step synth tab's status line, Import details for missing audio, and
    /// its help at the row's end.
    fn step_synth_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
            let (name, available, missing, explicit_file) = self.step_synth_source();
            let action = step_synth::toolbar(
                ui,
                ui.available_width() - 40.0,
                step_synth::Source {
                    name: &name,
                    available,
                    missing,
                    explicit_file,
                },
                step_synth::Feedback {
                    message: self.step_synth.feedback.message.as_deref(),
                    error: self.step_synth.feedback.error,
                },
                &self.step_synth.state,
                self.step_synth.effects_expanded,
            );
            if matches!(action, step_synth::Action::Details) {
                self.sample_import.details_open = true;
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
                    self.help.step_synth = true;
                }
            });
            let area = ui.min_rect();
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

    /// The Step synth source: its name, and whether it is available, missing,
    /// or an explicit file. A new palette source clears the old feedback.
    fn step_synth_source(&mut self) -> (String, bool, bool, bool) {
        let explicit_file = self.step_synth.file.is_some();
        let source_id = self.source_id();
        let palette_source = source_id.filter(|_| !explicit_file);
        if self.step_synth.feedback.source != palette_source {
            self.step_synth.feedback = StepSynthFeedback {
                source: palette_source,
                ..Default::default()
            };
        }
        let name = self.step_synth.file.as_ref().map_or_else(
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

    /// The Step synth board of the lower panel, and what its controls ask for.
    fn step_synth_board(&mut self, ui: &mut egui::Ui) {
        let (name, available, missing, explicit_file) = self.step_synth_source();
        let action = step_synth::show(
            ui,
            step_synth::Source {
                name: &name,
                available,
                missing,
                explicit_file,
            },
            &mut self.step_synth.state,
            &mut self.step_synth.bar,
            &mut self.step_synth.effects_expanded,
        );
        match action {
            step_synth::Action::ChooseSource => self.ask_path(FileRequest::SynthSource),
            step_synth::Action::PaletteSource => {
                self.step_synth.file = None;
                self.step_synth.feedback = StepSynthFeedback::default();
            }
            step_synth::Action::Listen => {
                match self.step_synth_pcm() {
                    Ok(pcm) => self.play_pcm(pcm, "Listening to Step synth sequence."),
                    Err(error) => self.error(error),
                }
                self.step_synth_notice();
            }
            step_synth::Action::SaveSequence => match self.step_synth_pcm() {
                Ok(pcm) => {
                    self.step_synth.feedback.message =
                        Some("Choose a WAV path to save the sequence.".into());
                    self.step_synth.feedback.error = false;
                    self.save_pcm(
                        pcm,
                        "Step synth sequence",
                        "Step synth",
                        self.step_synth.state.bpm,
                    );
                }
                Err(error) => {
                    self.error(error);
                    self.step_synth_notice();
                }
            },
            step_synth::Action::SavePattern => self.ask_path(FileRequest::SavePattern),
            step_synth::Action::LoadPattern => self.ask_path(FileRequest::LoadPattern),
            step_synth::Action::Details => self.sample_import.details_open = true,
            step_synth::Action::None => {}
        }
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
                self.help.master = true;
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
        let metering = master::Metering {
            reduced: self.session.options().meter_display_rate
                == mixjam_techno::application::options::MeterDisplayRate::Reduced,
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
                    self.session.options().meter_display_rate
                        == mixjam_techno::application::options::MeterDisplayRate::Reduced,
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

/// A tab of the lower panel's tab row. The shown board's tab is bright, on
/// a raised face, and underlined in the accent. On a folded panel the chosen
/// tab keeps its bright name only.
fn board_tab(ui: &mut egui::Ui, module: BottomModule, selected: bool, open: bool) -> Response {
    let label = module.label();
    let color = if selected {
        roles().text
    } else {
        roles().muted
    };
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), egui::FontId::proportional(14.0), color);
    let size = Vec2::new(galley.size().x + 20.0, TOOLBAR_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if selected && open {
        ui.painter().rect_filled(rect, 4.0, roles().raised);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let text = Pos2::new(
        rect.center().x - galley.size().x / 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text, galley, color);
    if selected && open {
        ui.painter().rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left() + 6.0, rect.bottom() - 2.0),
                Pos2::new(rect.right() - 6.0, rect.bottom()),
            ),
            1.0,
            roles().accent,
        );
    }
    focus_outline(ui, &response, roles().accent);
    let name = module.tab_name();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, name)
    });
    response.hover_hint(match module {
        BottomModule::Mixer => {
            "Mixer: track levels, pan, EQ, sends, polarity and outputs, on mixer strips."
        }
        BottomModule::Mastering => "Mastering: the master pedalboard that processes the whole mix.",
        BottomModule::FxReturns => "FX returns: the four FX buses that the sends feed.",
        BottomModule::StepSynth => "Step synth: sequence a sound across up to four bars of notes.",
    })
}
