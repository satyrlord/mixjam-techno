//! The status bar at the foot of the window: Options and All commands, the
//! notices, the status line, the panel switches, and the version.
use super::*;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A confirmation shows this long, then the bar goes back to its idle text.
/// Errors stay until another message replaces them.
const STATUS_SETTLE: Duration = Duration::from_secs(8);

/// An error shows in Coral this long, then in Muted after a Coral dot, so an
/// old error does not read as a new one. It stays until it is replaced or
/// cleared.
const STATUS_ERROR_FADE: Duration = Duration::from_secs(60);

/// The idle text where the whole of `DEFAULT_STATUS` does not fit. The
/// command list comes before the drag hint, so it is the last to go.
const IDLE_SHORT: &str = "Space plays. Ctrl+K lists every command. Drag sounds onto the score.";

/// The status line's popup lists this many messages.
const STATUS_HISTORY: usize = 8;

/// The width a message keeps before the notices beside it shorten.
const STATUS_MESSAGE_ROOM: f32 = 280.0;

/// Text size of everything in the bar but the PANELS heading.
const STATUS_TEXT: f32 = 12.0;

/// Space between the panel switches and the version link.
const STATUS_VERSION_GAP: f32 = 14.0;

/// Space between Options and All commands and the status that follows.
const STATUS_ICONS_GAP: f32 = 10.0;

/// Space on each side of a notice's text.
const NOTICE_PAD: f32 = 6.0;

/// The repository the version link opens.
const REPOSITORY: &str = "https://github.com/satyrlord/mixjam-techno";

/// The messages the status line showed, newest first, and when each came.
#[derive(Default)]
pub(in super::super) struct StatusLog {
    entries: VecDeque<StatusEntry>,
    /// The status and error flag as last seen, to notice a new one.
    seen: Option<(String, bool)>,
    /// A message was set again, maybe with the same text.
    renewed: bool,
    /// The newest entry is the status that shows now.
    current: bool,
    /// The user cleared the message that shows now.
    cleared: bool,
}

struct StatusEntry {
    text: String,
    error: bool,
    at: Instant,
    /// An error the popup has not shown yet.
    unread: bool,
}

impl StatusLog {
    /// A message or error was set: count it as new even with the same text.
    pub(in super::super) fn renew(&mut self) {
        self.renewed = true;
    }

    /// Note the status of this frame. Give how long it has shown, or None
    /// when it is not a logged message (the idle text or scan progress).
    fn observe(&mut self, text: &str, error: bool, logged: bool, now: Instant) -> Option<Duration> {
        let changed = std::mem::take(&mut self.renewed)
            || self
                .seen
                .as_ref()
                .is_none_or(|(seen, was_error)| seen != text || *was_error != error);
        if changed {
            self.seen = Some((text.to_owned(), error));
            self.current = logged;
            self.cleared = false;
            if logged {
                self.entries.push_front(StatusEntry {
                    text: text.to_owned(),
                    error,
                    at: now,
                    unread: error,
                });
                self.entries.truncate(STATUS_HISTORY);
            }
        }
        self.current
            .then(|| now.saturating_duration_since(self.entries[0].at))
    }

    /// The bar goes back to its idle text until the next message. The
    /// message stays in the log, and the status itself does not change.
    fn clear(&mut self) {
        self.cleared = true;
    }

    /// The errors in the log that the popup has not shown yet. With
    /// `showing`, the newest one is on the line in Coral and does not count
    /// until it fades or another message replaces it.
    fn unread_errors(&self, showing: bool) -> usize {
        self.entries
            .iter()
            .skip(usize::from(showing))
            .filter(|entry| entry.unread)
            .count()
    }

    /// The popup shows every message in the log.
    fn mark_read(&mut self) {
        for entry in &mut self.entries {
            entry.unread = false;
        }
    }
}

/// What the notices at the left of the status line report.
struct Notices {
    conflicts: bool,
    details: bool,
    missing: usize,
    reduced: bool,
}

impl Notices {
    /// The text parts of each shown notice at `level`: 0 is the full form,
    /// and each level after it is shorter, so a message keeps its room.
    fn parts(&self, level: usize) -> Vec<(Notice, Vec<(String, Color32)>)> {
        let mut notices = Vec::new();
        if self.conflicts {
            notices.push((
                Notice::Conflicts,
                vec![(
                    if level < 3 {
                        "Sample conflicts"
                    } else {
                        "Conflicts"
                    }
                    .to_owned(),
                    roles().warning,
                )],
            ));
        }
        if self.details {
            let mut parts = Vec::new();
            if self.missing > 0 {
                let count = if level == 0 {
                    format!("{} missing · silent", self.missing)
                } else {
                    format!("{} missing", self.missing)
                };
                parts.push((count, roles().coral));
            }
            if self.missing == 0 || level < 2 {
                parts.push(("Import details".to_owned(), roles().text));
            }
            notices.push((Notice::Details, parts));
        }
        if self.reduced {
            let text = if level < 3 {
                "Reduced quality"
            } else {
                "Reduced"
            };
            notices.push((Notice::Reduced, vec![(text.to_owned(), roles().warning)]));
        }
        notices
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Notice {
    Conflicts,
    Details,
    Reduced,
}

/// The full text of the reduced-quality notice.
const REDUCED: &str = "Playback quality reduced; exports use full quality";

impl MixJamApp {
    /// Keep the status bar's row across the whole window, under the rail.
    /// Its controls are drawn by `status_bar` after the rest of the window,
    /// so Tab reaches them last.
    pub(in super::super) fn status_bar_space(&self, ctx: &egui::Context) -> Rect {
        egui::TopBottomPanel::bottom("status")
            .exact_height(30.0)
            .frame(egui::Frame::new().fill(roles().canvas))
            .show(ctx, |_| {})
            .response
            .rect
    }

    pub(in super::super) fn status_bar(&mut self, ctx: &egui::Context, rect: Rect) {
        self.reconcile_step_synth_feedback();
        let inner = rect - egui::Margin::symmetric(18, 3);
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("status-bar"),
            egui::UiBuilder::new()
                .layer_id(egui::LayerId::background())
                .max_rect(inner),
        );
        ui.set_clip_rect(rect);
        let ui = &mut ui;
        // A group heading, styled like the rail's: small, tracked uppercase,
        // so it never reads as one of the switches beside it.
        let panels = egui::RichText::new("PANELS")
            .size(10.0)
            .extra_letter_spacing(0.6)
            .color(roles().muted);
        let version = format!("v{}", mixjam_techno::application::APP_VERSION);
        let gap = ui.spacing().item_spacing.x;
        // The right-hand group is drawn left to right, after the status, so
        // Tab reads the bar in the order it is seen; its width is measured
        // first to place it against the right edge.
        let heading = egui::WidgetText::from(panels.clone())
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Body,
            )
            .size()
            .x;
        let group = heading
            + PANEL_SWITCHES
                .iter()
                .map(|(_, label, _)| gap + panel_switch_width(ui, label))
                .sum::<f32>()
            + gap
            + STATUS_VERSION_GAP
            + text_width(ui, &version);
        let split = (inner.right() - group).max(inner.left());
        let left = Rect::from_min_max(inner.min, Pos2::new(split - gap, inner.bottom()));
        let right = Rect::from_min_max(Pos2::new(split, inner.top()), inner.max);

        // The message: a logged one settles back to the idle text after a
        // while, unless it is an error or reports what is going on now.
        let now = Instant::now();
        let idle = self.status == DEFAULT_STATUS;
        let progress = self.libraries.scan_status.as_deref() == Some(self.status.as_str());
        let age = self.status_log.observe(
            &self.status,
            self.status_error,
            !idle && !progress && !self.status.is_empty(),
            now,
        );
        let live = progress
            || self.session.playing_mix()
            || self.session.is_previewing()
            || self.session.is_recording();
        let settles = !self.status_error && !live;
        let settled =
            self.status_log.cleared || settles && age.is_some_and(|age| age >= STATUS_SETTLE);
        if settles && let Some(age) = age.filter(|age| *age < STATUS_SETTLE) {
            ctx.request_repaint_after(STATUS_SETTLE - age);
        }
        if self.status_error
            && let Some(age) = age.filter(|age| *age < STATUS_ERROR_FADE)
        {
            ctx.request_repaint_after(STATUS_ERROR_FADE - age);
        }
        let missing = self.session.missing_samples().len();
        // At rest, the Step synth board in view gives its own hint.
        // The general idle text would contradict a missing-sound notice.
        let board_hint = self.step_synth.hint.take();
        let (status, error) = if !idle && !settled {
            (status_text(&self.status), self.status_error)
        } else if let Some(hint) = board_hint {
            (hint.to_owned(), false)
        } else if missing > 0 {
            (String::new(), false)
        } else {
            (DEFAULT_STATUS.to_owned(), false)
        };

        // The notices shorten, in steps, before the message loses its room.
        let project = self.session.project();
        let notices = Notices {
            conflicts: !self.session.sample_conflicts().is_empty(),
            details: !project.import_notices.is_empty()
                || !project.source_notices.is_empty()
                || !project.source_settings.is_empty()
                || missing > 0,
            missing,
            reduced: self.session.options().quality_reduced(),
        };
        // The history mark leads the status once there is a message to show.
        let logged = !self.status_log.entries.is_empty();
        let showing = error && age.is_some_and(|age| age < STATUS_ERROR_FADE);
        let unread = self.status_log.unread_errors(showing);
        // The clock, or in its place the count of unread errors.
        let mark_width = 14.0;
        let room = left.width()
            - 2.0 * (28.0 + gap)
            - STATUS_ICONS_GAP
            - if logged { mark_width + gap } else { 0.0 };
        let wanted = if !status.is_empty() && status != DEFAULT_STATUS {
            text_width(ui, &status).min(STATUS_MESSAGE_ROOM)
        } else {
            0.0
        };
        let level = (0..3)
            .find(|level| {
                let width: f32 = notices
                    .parts(*level)
                    .iter()
                    .map(|(_, parts)| notice_width(ui, parts) + gap)
                    .sum();
                width + wanted <= room
            })
            .unwrap_or(3);
        // The idle text takes its short form where the whole does not fit.
        let shown_notices: f32 = notices
            .parts(level)
            .iter()
            .map(|(_, parts)| notice_width(ui, parts) + gap)
            .sum();
        let short_idle = status == DEFAULT_STATUS && text_width(ui, &status) > room - shown_notices;

        let icons = self.icons(ctx);
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(left)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                // Options and All commands lead the bar, so the rail's whole
                // height goes to the tools.
                let options = status_icon_button(
                    ui,
                    RailIcon::Options.label(),
                    RailIcon::Options.hint(),
                    Some(self.options.is_some()),
                    |ui, centre, color| {
                        if let Some(icons) = &icons {
                            ui.painter().image(
                                icons.texture_id(RailIcon::Options),
                                icons.icon_rect(ui.ctx(), centre),
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                color,
                            );
                        }
                    },
                );
                if options.clicked() {
                    self.run_command(commands::Command::Options, ctx);
                }
                if status_icon_button(
                    ui,
                    commands::ALL_COMMANDS,
                    commands::ALL_COMMANDS_HINT,
                    None,
                    |ui, centre, color| commands::paint_magnifier(ui.painter(), centre, color),
                )
                .clicked()
                {
                    self.commands = Some(commands::CommandList::new());
                }
                ui.add_space(STATUS_ICONS_GAP);
                for (notice, parts) in notices.parts(level) {
                    let (name, value, hint) = match notice {
                        Notice::Conflicts => (
                            "Sample conflicts",
                            None,
                            "Sample choices are not confirmed. Opens Sample conflicts.".to_owned(),
                        ),
                        Notice::Details if missing > 0 => (
                            "Import details",
                            Some(format!("{} play silence", plural_samples(missing))),
                            "Missing samples play silence. Import details shows their saved \
                             references."
                                .to_owned(),
                        ),
                        Notice::Details => (
                            "Import details",
                            None,
                            "Opens the import notices and the song's source details.".to_owned(),
                        ),
                        Notice::Reduced => (
                            "Reduced quality",
                            Some(REDUCED.to_owned()),
                            format!("{REDUCED}. Opens Options."),
                        ),
                    };
                    if notice_chip(ui, &parts, name, value.as_deref())
                        .hover_hint(hint)
                        .clicked()
                    {
                        match notice {
                            Notice::Conflicts => {
                                self.conflicts.open = true;
                                self.conflicts.error = None;
                            }
                            Notice::Details => self.sample_import.details_open = true,
                            Notice::Reduced => {
                                self.run_command(commands::Command::Options, ctx);
                            }
                        }
                    }
                }
                // A clock mark leads to the earlier messages whenever there
                // are any, also when the line itself has nothing to say. It
                // counts the errors the popup has not shown yet.
                if !status.is_empty() || logged {
                    let alone = status.is_empty();
                    let mark = logged.then(|| {
                        // Beside the status it shares the status's Tab stop;
                        // alone it is the Tab stop.
                        let sense = if alone { Sense::click() } else { Sense::CLICK };
                        let (rect, mark) =
                            ui.allocate_exact_size(Vec2::new(mark_width, 24.0), sense);
                        let name = match unread {
                            0 => "Recent messages".to_owned(),
                            1 => "Recent messages, 1 unread error".to_owned(),
                            count => format!("Recent messages, {count} unread errors"),
                        };
                        mark.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
                        });
                        (rect, mark)
                    });
                    // An old error keeps its Coral in a dot before it, unless
                    // the mark's count already shows it.
                    let faded = error && age.is_some_and(|age| age >= STATUS_ERROR_FADE);
                    if faded && unread == 0 {
                        let (dot, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 3.0, roles().coral);
                    }
                    let color = if error && !faded {
                        roles().coral
                    } else {
                        roles().muted
                    };
                    let line = (!alone).then(|| {
                        let text = if short_idle {
                            IDLE_SHORT
                        } else {
                            status.as_str()
                        };
                        // The faded-error dot and notices can change the
                        // preceding widget count. Keep the status's focus ID.
                        let line = ui
                            .scope_builder(egui::UiBuilder::new().id("status-bar-message"), |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(text).size(STATUS_TEXT).color(color),
                                    )
                                    .truncate()
                                    .sense(Sense::click()),
                                )
                            })
                            .inner;
                        let spoken = if error {
                            format!("Error: {status}")
                        } else {
                            status.clone()
                        };
                        ui.ctx().accesskit_node_builder(line.id, |node| {
                            node.set_role(egui::accesskit::Role::Button);
                            node.set_label("Status");
                            node.set_value(spoken);
                            node.set_live(egui::accesskit::Live::Polite);
                            match unread {
                                0 => {}
                                1 => node.set_description("1 unread error"),
                                count => node.set_description(format!("{count} unread errors")),
                            }
                        });
                        line
                    });
                    let response = match (line, &mark) {
                        (Some(line), Some((_, mark))) => line | mark.clone(),
                        (Some(line), None) => line,
                        (None, Some((_, mark))) => mark.clone(),
                        (None, None) => unreachable!("the status or the log has text"),
                    }
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                    let popup_id = ui.make_persistent_id("status-history-popup");
                    let open = egui::Popup::is_id_open(ui.ctx(), popup_id);
                    if let Some((rect, _)) = mark {
                        // Unread errors take the clock's place until the
                        // popup has shown them.
                        if unread > 0 {
                            paint_count(ui, rect.center(), unread);
                        } else {
                            let lit = response.hovered() || response.has_focus() || open;
                            let ink = if lit { roles().text } else { roles().muted };
                            paint_clock(ui.painter(), rect.center(), ink);
                        }
                    }
                    focus_outline(ui, &response, roles().accent);
                    // A click shows the message whole, with the ones before it.
                    let response = response.hover_hint(if alone {
                        "Click for recent messages.".to_owned()
                    } else {
                        format!("{status}\n\nClick for recent messages.")
                    });
                    let shown = (!settled).then_some(age).flatten().is_some();
                    let current = (!alone).then_some((status.as_str(), error));
                    let popup = egui::Popup::from_toggle_button_response(&response)
                        .id(popup_id)
                        .align(egui::RectAlign::TOP_START)
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .width(440.0)
                        .show(|ui| {
                            let recent = egui::ScrollArea::vertical()
                                .id_salt("status-history")
                                .max_height(ui.ctx().content_rect().height() * 0.65)
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    self.status_log.show_recent(ui, current, shown, now)
                                });
                            name_scroll_area(ui, "status-history", "Recent messages");
                            recent.inner
                        });
                    if let Some(popup) = popup {
                        self.status_log.mark_read();
                        if popup.inner {
                            self.status_log.clear();
                        }
                    }
                }
            },
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(right)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.label(panels);
                // In a maximised score all read off but Overview; each switch
                // restores the score with the previous panel choices.
                for (command, label, hint) in PANEL_SWITCHES {
                    if panel_switch(ui, label, self.command_active(command))
                        .hover_hint(hint)
                        .clicked()
                    {
                        self.run_command(command, ctx);
                    }
                }
                // A short rule keeps the version apart from the switches.
                // The item spacing on each side makes up the rest of the gap.
                let (rule, _) = ui.allocate_exact_size(
                    Vec2::new((STATUS_VERSION_GAP - gap).max(1.0), 12.0),
                    Sense::hover(),
                );
                ui.painter().vline(
                    rule.center().x.round() + 0.5,
                    rule.y_range(),
                    egui::Stroke::new(1.0_f32, roles().line),
                );
                version_link(ui, &version).hover_hint("Open MixJam Techno on GitHub");
            },
        );
    }
}

impl StatusLog {
    /// The status popup: the message shown now in full, if any, then the
    /// earlier ones with their age. `shown` says the newest entry is the one
    /// shown. Give true when the user clears the message shown now with
    /// Clear. Escape only closes the popup.
    fn show_recent(
        &self,
        ui: &mut egui::Ui,
        current: Option<(&str, bool)>,
        shown: bool,
        now: Instant,
    ) -> bool {
        ui.spacing_mut().item_spacing.y = 4.0;
        let mut clear = false;
        if let Some((status, error)) = current {
            let color = if error { roles().coral } else { roles().text };
            ui.add(
                egui::Label::new(egui::RichText::new(status).size(STATUS_TEXT).color(color)).wrap(),
            );
            // Any logged message can be cleared; the idle text and work in
            // progress cannot.
            if shown {
                clear = notice_chip(
                    ui,
                    &[("Clear".to_owned(), roles().text)],
                    "Clear message",
                    None,
                )
                .on_hover_text("The bar shows its usual hint until the next message.")
                .clicked();
                if clear {
                    ui.close();
                }
            }
        }
        let earlier: Vec<_> = self.entries.iter().skip(usize::from(shown)).collect();
        if earlier.is_empty() {
            return clear;
        }
        if current.is_some() {
            ui.add_space(4.0);
            ui.separator();
        }
        for entry in earlier {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(age_text(now.saturating_duration_since(entry.at)))
                    .size(10.0)
                    .color(roles().muted),
            );
            let color = if entry.error {
                roles().coral
            } else {
                roles().muted
            };
            ui.add(
                egui::Label::new(
                    egui::RichText::new(status_text(&entry.text))
                        .size(STATUS_TEXT)
                        .color(color),
                )
                .wrap(),
            );
        }
        clear
    }
}

/// The mark of the message history: a 12-point clock face with its hands at
/// three o'clock, in the 1.5-point stroke of the bar's painted marks.
fn paint_clock(painter: &egui::Painter, centre: Pos2, color: Color32) {
    let stroke = egui::Stroke::new(1.5_f32, color);
    painter.circle_stroke(centre, 5.5, stroke);
    painter.line_segment([centre, centre + Vec2::new(0.0, -3.0)], stroke);
    painter.line_segment([centre, centre + Vec2::new(2.5, 0.0)], stroke);
}

/// The unread-error count in the clock's place: a Coral disc as wide as the
/// clock face, with the count in Canvas. The log holds at most
/// `STATUS_HISTORY` messages, so the count is one digit.
fn paint_count(ui: &egui::Ui, centre: Pos2, count: usize) {
    let galley = ui.painter().layout_no_wrap(
        count.to_string(),
        egui::FontId::proportional(9.0),
        roles().canvas,
    );
    ui.painter().circle_filled(centre, 6.0, roles().coral);
    let at = centre - galley.mesh_bounds.center().to_vec2();
    ui.painter().galley(at, galley, roles().canvas);
}

/// A message as the bar shows it: paths without their long-path prefix.
/// Typed file-error reasons are formatted by their producers.
fn status_text(text: &str) -> String {
    display_paths(text)
}

/// How long ago a message came, in its largest whole unit.
fn age_text(age: Duration) -> String {
    let seconds = age.as_secs();
    match seconds {
        0..5 => "Just now".to_owned(),
        5..60 => format!("{seconds} s ago"),
        60..3600 => format!("{} min ago", seconds / 60),
        _ => format!("{} h ago", seconds / 3600),
    }
}

fn plural_samples(count: usize) -> String {
    if count == 1 {
        "1 missing sample".to_owned()
    } else {
        format!("{count} missing samples")
    }
}

/// The status bar's panel switches with their hover hints.
const PANEL_SWITCHES: [(commands::Command, &str, &str); 5] = [
    (
        commands::Command::ToolNames,
        "Tools",
        "Tool rail: names under the icons, or icons only",
    ),
    (
        commands::Command::Boards,
        "Boards",
        "Lower panel: open with its board, or folded away",
    ),
    (
        commands::Command::Sounds,
        "Sounds",
        "Sounds panel beside the score, or its folded strip",
    ),
    (
        commands::Command::Overview,
        "Overview",
        "Song overview above the score, or its named bar",
    ),
    (
        commands::Command::Inspector,
        "Inspector",
        "Inspector beside the score, or its folded strip",
    ),
];

/// The width of `text` in the bar's text size.
fn text_width(ui: &egui::Ui, text: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(
            text.to_owned(),
            egui::FontId::proportional(STATUS_TEXT),
            roles().text,
        )
        .size()
        .x
}

/// The space between the parts of one notice.
const NOTICE_PART_GAP: f32 = 8.0;

/// The parts of a notice, each its own text in its own color.
fn notice_galleys(ui: &egui::Ui, parts: &[(String, Color32)]) -> Vec<std::sync::Arc<egui::Galley>> {
    parts
        .iter()
        .map(|(text, color)| {
            ui.painter().layout_no_wrap(
                text.clone(),
                egui::FontId::proportional(STATUS_TEXT),
                *color,
            )
        })
        .collect()
}

/// The width of `galleys` side by side, without the chip padding.
fn galleys_width(galleys: &[std::sync::Arc<egui::Galley>]) -> f32 {
    let text: f32 = galleys.iter().map(|galley| galley.size().x).sum();
    text + NOTICE_PART_GAP * galleys.len().saturating_sub(1) as f32
}

/// The width `notice_chip` takes for `parts`.
fn notice_width(ui: &egui::Ui, parts: &[(String, Color32)]) -> f32 {
    galleys_width(&notice_galleys(ui, parts)) + 2.0 * NOTICE_PAD
}

/// A flat notice that opens what it reports: colored text, a fill under the
/// pointer, and the accent outline in focus. `name` is its accessible name,
/// whatever the text shows.
fn notice_chip(
    ui: &mut egui::Ui,
    parts: &[(String, Color32)],
    name: &str,
    value: Option<&str>,
) -> Response {
    let galleys = notice_galleys(ui, parts);
    let size = Vec2::new(galleys_width(&galleys) + 2.0 * NOTICE_PAD, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            roles().corner(3.0),
            ui.visuals().widgets.hovered.weak_bg_fill,
        );
    }
    // The parts share one baseline, and their ink, not their line box, sits
    // in the middle of the chip.
    let ink = galleys
        .iter()
        .map(|galley| galley.mesh_bounds)
        .reduce(|all, bounds| all.union(bounds))
        .unwrap_or(egui::Rect::NOTHING);
    let top = rect.center().y - ink.center().y;
    let mut left = rect.left() + NOTICE_PAD;
    for galley in galleys {
        let width = galley.size().x;
        let text = Pos2::new(left, top);
        ui.painter().galley(text, galley, roles().text);
        left += width + NOTICE_PART_GAP;
    }
    focus_outline(ui, &response, roles().accent);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    if let Some(value) = value {
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_value(value));
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The version link: muted at rest, in the accent and underlined under the
/// pointer or in focus. It opens the repository in the system browser.
fn version_link(ui: &mut egui::Ui, version: &str) -> Response {
    let width = text_width(ui, version);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::click());
    let lit = response.hovered() || response.has_focus();
    let color = if lit { roles().accent } else { roles().muted };
    let galley = ui.painter().layout_no_wrap(
        version.to_owned(),
        egui::FontId::proportional(STATUS_TEXT),
        color,
    );
    let text = Pos2::new(rect.left(), rect.center().y - galley.size().y / 2.0);
    let bottom = text.y + galley.size().y;
    ui.painter().galley(text, galley, color);
    if lit {
        ui.painter().line_segment(
            [
                Pos2::new(rect.left(), bottom),
                Pos2::new(rect.right(), bottom),
            ],
            Stroke::new(1.0_f32, color),
        );
    }
    if response.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(REPOSITORY));
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Link, true, "MixJam Techno on GitHub")
    });
    // A stable name; the version itself is the value.
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_value(version));
    name_control(response, "MixJam Techno on GitHub")
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A bare 28 by 24-point icon button at the status bar's left end, with
/// `glyph` painted at its centre: muted like the status text, bright under
/// the pointer, and in the accent when `open` says its window is open.
fn status_icon_button(
    ui: &mut egui::Ui,
    label: &str,
    hint: &str,
    open: Option<bool>,
    glyph: impl FnOnce(&egui::Ui, Pos2, Color32),
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(28.0, 24.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            roles().corner(3.0),
            ui.visuals().widgets.hovered.weak_bg_fill,
        );
    }
    let color = if open == Some(true) {
        roles().accent
    } else if response.hovered() {
        roles().text
    } else {
        roles().muted
    };
    glyph(ui, rect.center(), color);
    focus_outline(ui, &response, roles().accent);
    response.widget_info(|| match open {
        Some(open) => {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, open, label)
        }
        None => egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label),
    });
    response.hover_hint(hint)
}

/// The width `panel_switch` takes for `label`.
fn panel_switch_width(ui: &egui::Ui, label: &str) -> f32 {
    text_width(ui, label) + 14.0
}

/// A status-bar switch that shows or folds a panel. On, its name is bright
/// and underlined in the accent.
pub(in super::super) fn panel_switch(ui: &mut egui::Ui, label: &str, on: bool) -> Response {
    let color = if on { roles().text } else { roles().muted };
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(STATUS_TEXT),
        color,
    );
    let size = Vec2::new(panel_switch_width(ui, label), 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            roles().corner(3.0),
            ui.visuals().widgets.hovered.weak_bg_fill,
        );
    }
    let text = Pos2::new(
        rect.center().x - galley.size().x / 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text, galley, color);
    if on {
        ui.painter().rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left() + 4.0, rect.bottom() - 2.0),
                Pos2::new(rect.right() - 4.0, rect.bottom()),
            ),
            0.0,
            roles().accent,
        );
    }
    focus_outline(ui, &response, roles().accent);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label)
    });
    response
}

#[cfg(test)]
#[path = "status_bar_tests.rs"]
mod tests;
