//! One registry of the app's commands: name, group, shortcut and action.
//! The tool rail draws its tools from it, the status bar's panel switches
//! and the keyboard run through it, and the command list (Ctrl+K) shows all
//! of it, so a tool cannot exist in one place and be missing from another.

use super::*;
use mixjam_techno::application::{automation::Lane, fx};

/// Everything the app can do from one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command {
    New,
    Open,
    Save,
    SaveAs,
    Import,
    Export,
    Erase,
    Delete,
    Mixer,
    Mastering,
    FxReturns,
    Effects,
    StepSynth,
    Edit,
    Stretch,
    AlbumReview,
    Options,
    Help,
    PlayPause,
    Undo,
    Redo,
    Duplicate,
    SelectAll,
    Maximise,
    ToolNames,
    Boards,
    Sounds,
    Overview,
    Inspector,
    /// An automation lane: a track lane, a bus's return lane, or Off.
    Lane(Option<Lane>, Option<usize>),
}

/// The rail's groups and their tools, top to bottom. Options and All
/// commands sit at the rail's foot.
pub(super) const RAIL: [(&str, &[Command]); 3] = [
    (
        "File",
        &[
            Command::New,
            Command::Open,
            Command::Save,
            Command::SaveAs,
            Command::Import,
            Command::Export,
        ],
    ),
    ("Clips", &[Command::Erase, Command::Delete]),
    (
        "Tools",
        &[
            Command::Mixer,
            Command::Mastering,
            Command::FxReturns,
            Command::Effects,
            Command::StepSynth,
            Command::Edit,
            Command::Stretch,
            Command::AlbumReview,
        ],
    ),
];

/// The status bar's panel switches, left to right.
pub(super) const PANELS: [Command; 5] = [
    Command::ToolNames,
    Command::Boards,
    Command::Sounds,
    Command::Overview,
    Command::Inspector,
];

impl Command {
    /// Every command, in the list's order: its groups as the rail shows
    /// them, then the rest.
    pub(super) fn all() -> Vec<Self> {
        let mut all: Vec<Self> = RAIL
            .iter()
            .flat_map(|(_, tools)| tools.iter().copied())
            .collect();
        all.extend([
            Self::Options,
            Self::Help,
            Self::PlayPause,
            Self::Undo,
            Self::Redo,
            Self::Duplicate,
            Self::SelectAll,
            Self::Maximise,
        ]);
        all.extend(PANELS);
        all.push(Self::Lane(None, None));
        all.extend(view::TRACK_LANES.map(|lane| Self::Lane(Some(lane), None)));
        all.extend((0..fx::BUSES).map(|bus| Self::Lane(Some(Lane::Send(bus)), None)));
        all.extend((0..fx::BUSES).map(|bus| Self::Lane(None, Some(bus))));
        all
    }

    /// The rail icon, for commands the rail or the top bar shows.
    pub(super) fn icon(self) -> Option<RailIcon> {
        Some(match self {
            Self::New => RailIcon::New,
            Self::Open => RailIcon::Open,
            Self::Save => RailIcon::Save,
            Self::SaveAs => RailIcon::SaveAs,
            Self::Import => RailIcon::Import,
            Self::Export => RailIcon::Export,
            Self::Erase => RailIcon::Erase,
            Self::Delete => RailIcon::Delete,
            Self::Mixer => RailIcon::Mixer,
            Self::Mastering => RailIcon::Master,
            Self::FxReturns => RailIcon::FxReturns,
            Self::Effects => RailIcon::Effects,
            Self::StepSynth => RailIcon::StepSynth,
            Self::Edit => RailIcon::Edit,
            Self::Stretch => RailIcon::Stretch,
            Self::AlbumReview => RailIcon::AlbumReview,
            Self::Options => RailIcon::Options,
            Self::Help => RailIcon::Help,
            Self::Undo => RailIcon::Undo,
            Self::Redo => RailIcon::Redo,
            _ => return None,
        })
    }

    /// The group the list shows the command under.
    pub(super) fn group(self) -> &'static str {
        if let Some((group, _)) = RAIL.iter().find(|(_, tools)| tools.contains(&self)) {
            return group;
        }
        match self {
            Self::Options | Self::Help => "Application",
            Self::PlayPause => "Transport",
            Self::Undo | Self::Redo | Self::Duplicate | Self::SelectAll => "Edit",
            Self::Lane(..) => "Automation",
            _ => "Panels",
        }
    }

    /// The command's name in the list. A rail tool keeps its rail name.
    pub(super) fn name(self) -> String {
        if let Some(icon) = self.icon() {
            return icon.label().to_owned();
        }
        match self {
            Self::PlayPause => "Play or pause".into(),
            Self::Duplicate => "Duplicate selection".into(),
            Self::SelectAll => "Select every clip".into(),
            Self::Maximise => "Maximise the score".into(),
            Self::ToolNames => "Tool names in the rail".into(),
            Self::Boards => "Lower panel".into(),
            Self::Sounds => "Sounds panel".into(),
            Self::Overview => "Song overview".into(),
            Self::Inspector => "Inspector".into(),
            Self::Lane(None, None) => "Off: edit clips".into(),
            Self::Lane(Some(lane), _) => format!("{} lane", lane.label()),
            Self::Lane(None, Some(bus)) => format!("{} lane", view::return_lane_label(bus)),
            _ => unreachable!("every rail command has an icon"),
        }
    }

    /// What the command does, in one line after its name in the list.
    pub(super) fn description(self) -> &'static str {
        match self {
            Self::New => "Start an empty song",
            Self::Open => "Open a song file",
            Self::Save => "Save this song",
            Self::SaveAs => "Save the song to a new file",
            Self::Import => "Bring in a WAV file as a sound",
            Self::Export => "Render the song to a stereo WAV through the master board",
            Self::Erase => "Click clips to delete them; Esc ends",
            Self::Delete => "Delete the selected clips",
            Self::Mixer => "The tracks become mixer strips",
            Self::Mastering => "The pedalboard on the master bus",
            Self::FxReturns => "Four send and return effect buses",
            Self::Effects => "Process a sound into a new one",
            Self::StepSynth => "Write a note sequence and save it as a sound",
            Self::Edit => "Trim, fade and level a sound",
            Self::Stretch => "Fit a sound to the tempo",
            Self::AlbumReview => "Listening notes on an album",
            Self::Options => "Audio device, theme, workspace and folders",
            Self::Help => "Manual and shortcuts",
            Self::PlayPause => "Play or pause the song",
            Self::Undo => "Take back the last edit",
            Self::Redo => "Put back the edit you undid",
            Self::Duplicate => "Copy the selected clips to the bars after them",
            Self::SelectAll => "Select all clips in the song",
            Self::Maximise => "Hide the side and lower panels around the score",
            Self::ToolNames => "Show names under the rail icons, or icons only",
            Self::Boards => "Show or fold Mixer, Mastering, FX returns and Step synth",
            Self::Sounds => "The sound list beside the score",
            Self::Overview => "The whole song in one strip above the score",
            Self::Inspector => "Settings of the song, a track or the selected clips",
            Self::Lane(None, None) => "Hide automation and edit clips again",
            Self::Lane(Some(Lane::Volume), _) => "Decibels over the fader, on every track",
            Self::Lane(Some(Lane::Pan), _) => "Left to right, on every track",
            Self::Lane(Some(Lane::Filter), _) => "Low-pass below the middle, high-pass above",
            Self::Lane(Some(Lane::Pitch), _) => "Bend the playback speed, like a tape machine",
            Self::Lane(Some(Lane::Repeat), _) => "Rolls and ratchets from the track's own sound",
            Self::Lane(Some(Lane::RepeatGate), _) => "How much of each repeat sounds",
            Self::Lane(Some(Lane::Send(_)), _) => "How much each track sends to this bus",
            Self::Lane(None, Some(_)) => "The bus's return level, one line across the song",
        }
    }

    /// The command's shortcut, as the list prints it.
    pub(super) fn shortcut(self) -> Option<&'static str> {
        Some(match self {
            Self::New => "Ctrl+N",
            Self::Open => "Ctrl+O",
            Self::Save => "Ctrl+S",
            Self::SaveAs => "Ctrl+Shift+S",
            Self::Delete => "Delete",
            Self::Options => "Ctrl+,",
            Self::Help => "F1",
            Self::PlayPause => "Space",
            Self::Undo => "Ctrl+Z",
            Self::Redo => "Ctrl+Y",
            Self::Duplicate => "Ctrl+D",
            Self::SelectAll => "Ctrl+A",
            Self::Maximise => "F",
            _ => return None,
        })
    }
}

/// The command list's state while it is open.
#[derive(Default)]
pub(super) struct CommandList {
    query: String,
    current: usize,
    focus: bool,
}

impl CommandList {
    pub(super) fn new() -> Self {
        Self {
            focus: true,
            ..Self::default()
        }
    }
}

/// The commands whose name or group holds every typed word, in list order.
pub(super) fn matching(query: &str) -> Vec<Command> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    Command::all()
        .into_iter()
        .filter(|command| {
            let text = format!("{} {}", command.name(), command.group()).to_lowercase();
            words.iter().all(|word| text.contains(word))
        })
        .collect()
}

impl MixJamApp {
    /// Run `command` as its rail tool, switch or shortcut does.
    pub(super) fn run_command(&mut self, command: Command, ctx: &egui::Context) {
        match command {
            Command::New => self.request_action(PendingAction::New, ctx),
            Command::Open => self.request_action(PendingAction::Open, ctx),
            Command::Save => self.save(false),
            Command::SaveAs => self.save(true),
            Command::Import => self.import_wav(),
            Command::Export => self.export(),
            Command::Erase => self.score_tools.erase = !self.score_tools.erase,
            Command::Delete => self.delete_selection(),
            // Mixer switches the score between its Arrange and Mix views.
            Command::Mixer => self.set_mix_view(!self.mix.shown),
            // Mastering, FX returns and Step synth open their tab, and fold
            // the lower panel when their tab already shows.
            Command::Mastering => self.toggle_board(BottomModule::Mastering),
            Command::FxReturns => self.toggle_board(BottomModule::FxReturns),
            Command::StepSynth => self.toggle_board(BottomModule::StepSynth),
            Command::Effects => self.effects.open = !self.effects.open,
            Command::Edit => {
                self.sample_edit.open = !self.sample_edit.open;
                if self.sample_edit.open {
                    self.sample_edit.start = 0;
                    self.sample_edit.end = 0;
                }
            }
            Command::Stretch => {
                self.stretch.open = !self.stretch.open;
                if self.stretch.open {
                    self.stretch.source_bpm = self.source_bpm().clamp(30.0, 300.0);
                } else {
                    self.close_stretch_request();
                }
            }
            Command::AlbumReview => self.album_review.open = !self.album_review.open,
            Command::Options => self.open_options(ctx),
            Command::Help => self.help.main = true,
            Command::PlayPause => {
                if self.session.wants_playback() {
                    self.pause();
                } else {
                    self.start_playback();
                }
            }
            Command::Undo => self.history_step(false),
            Command::Redo => self.history_step(true),
            Command::Duplicate => self.duplicate_selection(),
            Command::SelectAll => self.select_all_clips(),
            Command::Maximise => self.set_layout(|layout| layout.maximised = !layout.maximised),
            // In a maximised score each panel switch restores the score with
            // the previous panel choices.
            Command::ToolNames => {
                let shown = !self.session.layout().maximised;
                self.set_layout(|layout| {
                    if shown {
                        layout.tool_names = !layout.tool_names;
                    }
                    layout.maximised = false;
                });
            }
            Command::Boards => {
                if self.session.layout().maximised {
                    self.set_layout(|layout| layout.maximised = false);
                } else if self.panels.room.boards {
                    self.set_layout(|layout| layout.boards = false);
                } else {
                    self.set_layout(|layout| layout.boards = true);
                    self.claim(super::room::Panel::Boards);
                }
            }
            Command::Sounds => {
                if self.session.layout().maximised {
                    self.set_layout(|layout| layout.maximised = false);
                } else if self.sounds_panel_open() {
                    self.set_layout(|layout| layout.sounds = false);
                } else {
                    self.open_sounds_panel();
                }
            }
            // The overview belongs to the score, so it keeps its place when
            // the score is maximised.
            Command::Overview => self.set_layout(|layout| layout.overview = !layout.overview),
            Command::Inspector => {
                if self.session.layout().maximised {
                    self.set_layout(|layout| layout.maximised = false);
                } else if self.panels.room.inspector {
                    self.set_layout(|layout| layout.inspector = false);
                } else {
                    self.open_inspector();
                }
            }
            Command::Lane(lane, bus) => self.choose_lane((lane, bus)),
        }
    }

    /// True while the command's tool, panel or lane is on.
    pub(super) fn command_active(&self, command: Command) -> bool {
        match command {
            Command::Erase => self.score_tools.erase,
            Command::Mixer => self.mix.shown,
            Command::Mastering => self.board_shown(BottomModule::Mastering),
            Command::FxReturns => self.board_shown(BottomModule::FxReturns),
            Command::StepSynth => self.board_shown(BottomModule::StepSynth),
            Command::Effects => self.effects.open,
            Command::Edit => self.sample_edit.open,
            Command::Stretch => self.stretch.open,
            Command::AlbumReview => self.album_review.open,
            Command::Options => self.options.is_some(),
            Command::Maximise => self.session.layout().maximised,
            Command::ToolNames => {
                let layout = self.session.layout();
                !layout.maximised && layout.tool_names
            }
            Command::Boards => self.panels.room.boards,
            Command::Sounds => self.sounds_panel_open(),
            Command::Overview => self.session.layout().overview,
            Command::Inspector => self.panels.room.inspector,
            Command::Lane(lane, bus) => {
                (
                    self.score_tools.automation_lane,
                    self.score_tools.return_lane,
                ) == (lane, bus)
            }
            _ => false,
        }
    }

    /// Show the command list over the window: every command in its groups,
    /// narrowed as words are typed. Arrows move, Enter runs, Esc closes.
    pub(super) fn command_list(&mut self, ctx: &egui::Context) {
        if self
            .blocking_modal(ctx)
            .is_some_and(|title| title != "All commands")
        {
            self.commands = None;
            return;
        }
        let Some(mut list) = self.commands.take() else {
            return;
        };
        let (up, down, enter) = ctx.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            )
        });
        if up || down {
            // egui queues focus movement before widgets consume their keys.
            ctx.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        let mut run = None;
        const NO_MATCH: &str = "Nothing matches. Clear the field to see the whole list again.";
        let width = (ctx.content_rect().width() - 32.0).clamp(280.0, 580.0);
        let modal = egui::Modal::new(egui::Id::new("command-list"))
            .frame(
                egui::Frame::new()
                    .fill(roles().panel)
                    .stroke(Stroke::new(1.0_f32, roles().line))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                name_dialog(ui, "All commands");
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 0.0;
                let field = egui::Frame::new()
                    .fill(roles().raised)
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut list.query)
                                .hint_text(
                                    egui::RichText::new(
                                        "Everything is listed below. Type to narrow it.",
                                    )
                                    .color(roles().muted),
                                )
                                .desired_width(f32::INFINITY)
                                .frame(false)
                                .font(egui::FontId::proportional(14.0)),
                        )
                    })
                    .inner;
                let field = name_control(field, "Find a command");
                if list.focus || up || down {
                    field.request_focus();
                    list.focus = false;
                }
                if field.changed() {
                    list.current = 0;
                }
                let shown = matching(&list.query);
                if down {
                    list.current = (list.current + 1).min(shown.len().saturating_sub(1));
                }
                if up {
                    list.current = list.current.saturating_sub(1);
                }
                if enter {
                    run = shown.get(list.current).copied();
                }
                let height = (ctx.content_rect().height() * 0.66).max(200.0);
                if field.changed() {
                    egui::scroll_area::State::default().store(
                        ctx,
                        ui.make_persistent_id(egui::Id::new("command-list-results")),
                    );
                }
                egui::ScrollArea::vertical()
                    .id_salt("command-list-results")
                    .max_height(height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.add_space(4.0);
                        if shown.is_empty() {
                            ui.add_space(12.0);
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new(NO_MATCH).color(roles().muted));
                            });
                            ui.add_space(12.0);
                        }
                        let mut group = "";
                        for (index, command) in shown.iter().enumerate() {
                            if command.group() != group {
                                group = command.group();
                                // Aligned with the names below it.
                                ui.horizontal(|ui| {
                                    ui.add_space(26.0 - ui.spacing().item_spacing.x);
                                    ui.vertical(|ui| view::lane_menu_heading(ui, group));
                                });
                            }
                            let current = index == list.current;
                            let response = command_row(ui, self, *command, current);
                            if current && (up || down) {
                                response.scroll_to_me(None);
                            }
                            if response.clicked() {
                                run = Some(*command);
                            }
                        }
                        ui.add_space(6.0);
                    });
            });
        if run.is_none() && modal.should_close() {
            return;
        }
        if let Some(command) = run {
            self.run_command(command, ctx);
            return;
        }
        self.commands = Some(list);
    }
}

/// One row of the command list: the state mark, the name, what it does,
/// and the shortcut at the right.
fn command_row(ui: &mut egui::Ui, app: &MixJamApp, command: Command, current: bool) -> Response {
    let width = ui.available_width();
    // The arrows choose the row, so Tab never lands on one: a focused row
    // that Enter ignores would run a different command.
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::CLICK);
    let rect = rect.shrink2(Vec2::new(6.0, 0.0));
    let active = app.command_active(command);
    let name = command.name();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, &name)
    });
    let description = command.description();
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_description(description));
    if current || response.hovered() {
        ui.painter().rect_filled(rect, 4.0, roles().hover);
    }
    if current {
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
            1.5,
            roles().accent,
        );
    }
    let mut x = rect.left() + 12.0;
    // An on tool, panel or lane shows a filled dot; the others leave room.
    if active {
        ui.painter()
            .circle_filled(Pos2::new(x + 3.0, rect.center().y), 3.0, roles().accent);
    }
    x += 14.0;
    let shortcut = command.shortcut().map(|keys| {
        ui.painter().layout_no_wrap(
            keys.to_owned(),
            egui::FontId::monospace(11.0),
            roles().muted,
        )
    });
    let right = shortcut.as_ref().map_or(rect.right() - 10.0, |galley| {
        let origin = Pos2::new(
            rect.right() - 10.0 - galley.size().x,
            rect.center().y - galley.size().y / 2.0,
        );
        ui.painter().galley(origin, galley.clone(), roles().muted);
        origin.x - 12.0
    });
    let mut job = egui::text::LayoutJob::single_section(
        name,
        egui::TextFormat::simple(egui::FontId::proportional(13.0), roles().text),
    );
    job.wrap.max_width = (right - x).max(1.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    let after = x + galley.size().x + 8.0;
    ui.painter().galley(
        Pos2::new(x, rect.center().y - galley.size().y / 2.0),
        galley,
        roles().text,
    );
    // The description takes what the name leaves and ends in an ellipsis.
    if right - after >= 24.0 {
        let mut job = egui::text::LayoutJob::single_section(
            description.to_owned(),
            egui::TextFormat::simple(egui::FontId::proportional(12.0), roles().muted),
        );
        job.wrap.max_width = right - after;
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.overflow_character = Some('…');
        let galley = ui.painter().layout_job(job);
        ui.painter().galley(
            Pos2::new(after, rect.center().y - galley.size().y / 2.0),
            galley,
            roles().muted,
        );
    }
    response
}

/// The rail's All commands button: a magnifier drawn over its name, or the
/// magnifier alone when the rail shows icons only.
pub(super) fn rail_commands_button(ui: &mut egui::Ui, names: bool, open: bool) -> Response {
    // The rail prints the short "Commands"; the accessible name stays full.
    let response = view::rail_item(ui, names, open, "Commands", |ui, centre, color| {
        // The rail icons' 1.6-point stroke throughout, with a round-capped
        // handle, balanced in the 20-point icon square.
        let stroke = Stroke::new(1.6_f32, color);
        let lens = centre - Vec2::splat(1.5);
        let tip = centre + Vec2::splat(7.5);
        ui.painter().circle_stroke(lens, 6.0, stroke);
        ui.painter()
            .line_segment([lens + Vec2::splat(6.0 / std::f32::consts::SQRT_2), tip], stroke);
        ui.painter().circle_filled(tip, 0.8, color);
    });
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "All commands"));
    response.hover_hint("All commands: everything the app can do, in one list (Ctrl+K)")
}
