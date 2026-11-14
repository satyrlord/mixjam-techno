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
            Command::Erase => self.erase = !self.erase,
            Command::Delete => self.delete_selection(),
            // Mixer switches the score between its Arrange and Mix views.
            Command::Mixer => self.set_mix_view(!self.mix_view),
            // Mastering, FX returns and Step synth open their tab, and fold
            // the lower panel when their tab already shows.
            Command::Mastering => self.toggle_board(BottomModule::Mastering),
            Command::FxReturns => self.toggle_board(BottomModule::FxReturns),
            Command::StepSynth => self.toggle_board(BottomModule::StepSynth),
            Command::Effects => self.show_effects = !self.show_effects,
            Command::Edit => {
                self.show_sample_edit = !self.show_sample_edit;
                if self.show_sample_edit {
                    self.edit_start = 0;
                    self.edit_end = 0;
                }
            }
            Command::Stretch => {
                self.show_stretch = !self.show_stretch;
                if self.show_stretch {
                    self.stretch_source_bpm = self.source_bpm().clamp(30.0, 300.0);
                } else {
                    self.close_stretch_request();
                }
            }
            Command::AlbumReview => self.show_album_review = !self.show_album_review,
            Command::Options => self.open_options(ctx),
            Command::Help => self.show_help = true,
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
                } else if self.room.boards {
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
                } else if self.room.inspector {
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
            Command::Erase => self.erase,
            Command::Mixer => self.mix_view,
            Command::Mastering => self.board_shown(BottomModule::Mastering),
            Command::FxReturns => self.board_shown(BottomModule::FxReturns),
            Command::StepSynth => self.board_shown(BottomModule::StepSynth),
            Command::Effects => self.show_effects,
            Command::Edit => self.show_sample_edit,
            Command::Stretch => self.show_stretch,
            Command::AlbumReview => self.show_album_review,
            Command::Options => self.options.is_some(),
            Command::Maximise => self.session.layout().maximised,
            Command::ToolNames => {
                let layout = self.session.layout();
                !layout.maximised && layout.tool_names
            }
            Command::Boards => self.room.boards,
            Command::Sounds => self.sounds_panel_open(),
            Command::Overview => self.session.layout().overview,
            Command::Inspector => self.room.inspector,
            Command::Lane(lane, bus) => (self.automation_lane, self.return_lane) == (lane, bus),
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

/// One row of the command list: the name, its group's state mark, and the
/// shortcut at the right.
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
    ui.painter().galley(
        Pos2::new(x, rect.center().y - galley.size().y / 2.0),
        galley,
        roles().text,
    );
    response
}

/// The rail's All commands button: a magnifier drawn over its name, or the
/// magnifier alone when the rail shows icons only.
pub(super) fn rail_commands_button(ui: &mut egui::Ui, names: bool, open: bool) -> Response {
    let size = view::rail_button_size(names);
    let width = size.x.min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, size.y), Sense::click());
    controls::icon_button_surface(ui, &response, open);
    let color = if open {
        roles().tools[0].1
    } else {
        roles().legend
    };
    let centre = if names {
        Pos2::new(rect.center().x - 1.0, rect.top() + 12.0)
    } else {
        rect.center() - Vec2::splat(1.0)
    };
    let stroke = Stroke::new(1.6_f32, color);
    ui.painter().circle_stroke(centre, 5.0, stroke);
    ui.painter().line_segment(
        [centre + Vec2::splat(3.6), centre + Vec2::splat(7.5)],
        Stroke::new(2.0_f32, color),
    );
    if names {
        let text_color = if open || response.hovered() {
            roles().text
        } else {
            roles().legend
        };
        let mut font = 11.0;
        let galley = loop {
            let galley = ui.painter().layout_no_wrap(
                "All commands".to_owned(),
                egui::FontId::proportional(font),
                text_color,
            );
            if galley.size().x <= rect.width() - 4.0 || font <= 9.0 {
                break galley;
            }
            font -= 0.5;
        };
        let origin = Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.bottom() - 4.0 - galley.size().y,
        );
        ui.painter().galley(origin, galley, Color32::PLACEHOLDER);
    }
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "All commands"));
    response.hover_hint("All commands: everything the app can do, in one list (Ctrl+K)")
}
