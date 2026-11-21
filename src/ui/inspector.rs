//! The Inspector at the right of the score: the song when nothing is
//! selected, the selected track, the selected clip, or a summary of several
//! selected clips. Its fields edit through the clip menu's and the Mixer's
//! operations, so every change is an undo step. Its controls carry their own
//! names ("Inspector track ...", "Inspector clip ..."), so they never share a
//! name with the Mixer, the Mix view, or the clip menu shown beside them.
use super::mixer::{
    Look, center_button, gain_value, pan_slider, polarity_button, row_surface, send_knob,
    switch_button, unity_button,
};
use super::room::Panel;
use super::sounds::{fold_button, panel_strip};
use super::view::{GROUPS, group_color, section_color, title_text};
use super::*;
use mixjam_techno::application::fx::BUSES;

#[path = "inspector/clip/mod.rs"]
mod clip;

/// The open panel's width, and the folded strip's.
pub(super) const INSPECTOR_WIDTH: f32 = 280.0;
pub(super) const INSPECTOR_STRIP_WIDTH: f32 = 28.0;
/// The name that every track page control starts with.
const TRACK: &str = "Inspector track";
/// The name that every clip page control starts with.
pub(super) const CLIP: &str = "Inspector clip";

/// A sound's category as the Sounds panel names it.
fn group_label(group: &str) -> &str {
    GROUPS
        .iter()
        .find(|name| name.eq_ignore_ascii_case(group))
        .copied()
        .unwrap_or(group)
}

/// The first and last bar that a span of ticks touches, counted from 1.
fn bar_span(start: u32, end: u32) -> (u32, u32) {
    let first = start / TICKS_PER_BAR + 1;
    (first, end.div_ceil(TICKS_PER_BAR).max(first))
}

/// What the Inspector shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Page {
    Song,
    Track(usize),
    Clip(u64),
    Clips(usize),
}

impl MixJamApp {
    /// The page the selection asks for: clips first, then a track, else the
    /// song.
    pub(super) fn inspector_page(&self) -> Page {
        match self.selection.clips.len() {
            0 => self.selection.track.map_or(Page::Song, Page::Track),
            1 => self.selection.clip.map_or(Page::Song, Page::Clip),
            count => Page::Clips(count),
        }
    }

    /// Open the Inspector.
    pub(super) fn open_inspector(&mut self) {
        self.set_layout(|layout| {
            layout.inspector = true;
            layout.maximised = false;
        });
        self.claim(Panel::Inspector);
    }

    /// Select `track` for the Inspector's track page; the clip selection
    /// clears.
    pub(super) fn select_track(&mut self, track: usize) {
        self.selection.track = Some(track);
        self.selection.clip = None;
        self.selection.clips.clear();
    }

    /// The Inspector at the right, or its folded strip. A maximised score
    /// hides both.
    pub(super) fn inspector_side(&mut self, ctx: &egui::Context) {
        if self.session.layout().maximised {
            return;
        }
        if !self.panels.room.inspector {
            egui::SidePanel::right("inspector-folded")
                .exact_width(INSPECTOR_STRIP_WIDTH)
                .resizable(false)
                .frame(egui::Frame::new().fill(roles().panel))
                .show(ctx, |ui| {
                    if panel_strip(
                        ui,
                        "Show the Inspector",
                        "INSPECTOR",
                        -1.0,
                        "Show the Inspector: the song, the selected track, or the selected \
                         clips. In a narrow window the Sounds panel folds to make room.",
                    )
                    .clicked()
                    {
                        self.open_inspector();
                    }
                });
            return;
        }
        egui::SidePanel::right("inspector")
            .exact_width(INSPECTOR_WIDTH)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(roles().panel)
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(20.0);
                    ui.label(
                        egui::RichText::new("Inspector")
                            .size(12.0)
                            .color(roles().muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if fold_button(ui, "Fold the Inspector", 1.0).clicked() {
                            self.set_layout(|layout| layout.inspector = false);
                        }
                    });
                });
                let page = self.inspector_page();
                let mut page_id = ui.id().with((self.boards.master_view_generation, page));
                if matches!(page, Page::Clips(_)) {
                    page_id = page_id.with(&self.selection.clips);
                }
                ui.push_id(page_id, |ui| {
                    // The bar takes its own room, so it never covers a card.
                    ui.spacing_mut().scroll.floating = false;
                    egui::ScrollArea::vertical()
                        .id_salt("inspector-page")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                            match page {
                                Page::Song => self.song_page(ui),
                                Page::Track(track) => self.track_page(ui, track),
                                Page::Clip(id) => self.clip_page(ui, id),
                                Page::Clips(count) => self.clips_page(ui, count),
                            }
                        });
                    view::name_scroll_area(ui, "inspector-page", "Inspector");
                });
            });
    }

    fn song_page(&mut self, ui: &mut egui::Ui) {
        let project = self.session.project();
        let meta = view::song_meta(project).unwrap_or_else(|| "Empty".into());
        title(
            ui,
            &project.title,
            &[&format!("{:.0} BPM  ·  {meta}", project.bpm)],
        );
        self.category_counts(ui, &project.clips);
        section_heading(ui, "Sections");
        let project = self.session.project();
        if project.sections.is_empty() {
            note(
                ui,
                "No sections. Add one from the bar ruler's menu; each one shows here and in \
                 the song overview.",
            );
            return;
        }
        let end = project.end_tick();
        let sections: Vec<(u32, u32, String)> = project
            .sections
            .iter()
            .enumerate()
            .map(|(index, section)| {
                let next = project
                    .sections
                    .get(index + 1)
                    .map_or(end.max(section.start_tick + TICKS_PER_BAR), |next| {
                        next.start_tick
                    });
                (section.start_tick, next, section.name.clone())
            })
            .collect();
        for (index, (start, next, name)) in sections.into_iter().enumerate() {
            let (first, last) = bar_span(start, next);
            let label = format!("Go to {name}, bar {first}");
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
            if response.hovered() || response.has_focus() {
                ui.painter()
                    .rect_filled(rect, 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
            }
            view::focus_outline(ui, &response, roles().accent);
            ui.painter().rect_filled(
                Rect::from_min_size(rect.left_top() + Vec2::new(2.0, 4.0), Vec2::new(3.0, 16.0)),
                1.0,
                section_color(index),
            );
            let bars = ui.painter().text(
                rect.right_center() - Vec2::new(4.0, 0.0),
                Align2::RIGHT_CENTER,
                if first == last {
                    format!("bar {first}")
                } else {
                    format!("bars {first} to {last}")
                },
                FontId::proportional(12.0),
                roles().muted,
            );
            let name_left = rect.left() + 12.0;
            let galley = ui.fonts_mut(|fonts| {
                fonts.layout_job(view::elided(
                    &title_text(&name),
                    12.0,
                    roles().text,
                    bars.left() - 10.0 - name_left,
                    1,
                ))
            });
            ui.painter().galley(
                egui::pos2(name_left, rect.center().y - galley.size().y / 2.0),
                galley,
                roles().text,
            );
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
            if response
                .hover_hint("Scroll the score to this section. The playhead stays where it is.")
                .clicked()
            {
                self.overview.target = Some((start + next) as f32 / 2.0);
            }
        }
    }

    /// Clip counts per category, in the Sounds panel's category order.
    fn category_counts<'a>(&self, ui: &mut egui::Ui, clips: impl IntoIterator<Item = &'a Clip>) {
        section_heading(ui, "Categories");
        let library = self.session.library();
        // Count the clips of each sound first, so a sound's category is
        // looked up once; its place keeps the order of first appearance.
        let mut sounds: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
        for (place, clip) in clips.into_iter().enumerate() {
            sounds.entry(clip.sample_id).or_insert((place, 0)).1 += 1;
        }
        let mut sounds: Vec<(u32, (usize, usize))> = sounds.into_iter().collect();
        sounds.sort_by_key(|(_, (place, _))| *place);
        let mut counts: Vec<(&str, usize)> = Vec::new();
        let mut missing = 0;
        for (id, (_, clips)) in sounds {
            let Some(sample) = library.sample_by_id(id) else {
                missing += clips;
                continue;
            };
            let group = group_label(&sample.group_name);
            match counts.iter_mut().find(|(name, _)| *name == group) {
                Some((_, count)) => *count += clips,
                None => counts.push((group, clips)),
            }
        }
        counts.sort_by_key(|(name, _)| {
            GROUPS
                .iter()
                .position(|group| group == name)
                .unwrap_or(GROUPS.len())
        });
        if counts.is_empty() && missing == 0 {
            note(
                ui,
                "No clips yet. Drag a sound from the Sounds panel onto a lane.",
            );
            return;
        }
        let rows = counts
            .into_iter()
            .map(|(name, count)| (group_color(name), name, count, false))
            .chain((missing > 0).then(|| (roles().raised, "Missing sound", missing, true)));
        for (color, name, count, is_missing) in rows {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
            let swatch = Rect::from_center_size(
                rect.left_center() + Vec2::new(7.0, 0.0),
                Vec2::new(14.0, 10.0),
            );
            ui.painter().rect_filled(swatch, 2.0, color);
            // As on a missing clip in the score: a coral edge.
            if is_missing {
                ui.painter().rect_stroke(
                    swatch,
                    2.0,
                    Stroke::new(1.0_f32, roles().coral),
                    egui::StrokeKind::Inside,
                );
            }
            ui.painter().text(
                rect.left_center() + Vec2::new(22.0, 0.0),
                Align2::LEFT_CENTER,
                name,
                FontId::proportional(12.0),
                roles().text,
            );
            ui.painter().text(
                rect.right_center() - Vec2::new(4.0, 0.0),
                Align2::RIGHT_CENTER,
                format!("{count} {}", if count == 1 { "clip" } else { "clips" }),
                FontId::proportional(12.0),
                roles().muted,
            );
        }
    }

    fn track_page(&mut self, ui: &mut egui::Ui, index: usize) {
        let project = self.session.project();
        let first = lanes::first_sounds(project)[index];
        let name = lanes::lane_name(
            first,
            self.session.library(),
            |id| {
                self.session
                    .missing_sample(id)
                    .map(|missing| missing.name.clone())
            },
            index,
        );
        let count = project
            .clips
            .iter()
            .filter(|clip| clip.track == index)
            .count();
        title(
            ui,
            &name,
            &[&format!(
                "Track {}  ·  {count} {}",
                index + 1,
                if count == 1 { "clip" } else { "clips" }
            )],
        );
        let any_solo = project.any_solo();
        let returns: Vec<(&'static str, bool)> = project
            .fx
            .buses
            .iter()
            .map(|bus| {
                let label = bus.module.label();
                (
                    // "Echoform Delay" reads "Delay" under a small knob.
                    label.rsplit(' ').next().unwrap_or(label),
                    !bus.module.is_empty() && bus.powered,
                )
            })
            .collect();
        let mut edited = project.tracks[index].clone();
        let track = &mut edited;
        let width = ui.available_width();
        let mut switched = false;

        section_heading(ui, "Channel processing");
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 80.0), Sense::hover());
        eq::response_curve(ui, rect, &track.eq, &format!("{TRACK} EQ"));
        // The curve opens the channel EQ window, where every dial is.
        let eq_name = format!("{TRACK} EQ");
        let response = ui.interact(rect, ui.id().with(&eq_name), Sense::click());
        let lit = response.hovered() || response.has_focus();
        if lit {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.painter().rect_stroke(
                rect,
                3.0,
                Stroke::new(1.0_f32, roles().accent),
                egui::StrokeKind::Inside,
            );
        }
        view::focus_outline(ui, &response, roles().accent);
        ui.painter().text(
            rect.right_top() + Vec2::new(-6.0, 5.0),
            Align2::RIGHT_TOP,
            "Open EQ",
            FontId::proportional(10.0),
            if lit { roles().text } else { roles().muted },
        );
        view::named(&response, egui::WidgetType::Button, &eq_name);
        let open_eq = response
            .hover_hint(format!(
                "Open the channel EQ window: low and high cuts, shelves, and two peaks; the \
                 drive; the compressor and the stereo width; and the ducker. Processing is {}.",
                if track.processing_on() { "on" } else { "off" }
            ))
            .clicked();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 56.0), Sense::hover());
        let half = (width - 6.0) / 2.0;
        let inserts: [(&str, &str, &mut bool); 4] = [
            ("EQ", "EQ power", &mut track.eq.on),
            ("Drive", "drive power", &mut track.drive.on),
            ("Compressor", "compressor power", &mut track.compressor.on),
            ("Ducker", "ducker power", &mut track.ducker.on),
        ];
        for (slot, (label, suffix, on)) in inserts.into_iter().enumerate() {
            let at = Rect::from_min_size(
                rect.min + Vec2::new((slot % 2) as f32 * (half + 6.0), (slot / 2) as f32 * 30.0),
                Vec2::new(half, 26.0),
            );
            let control = format!("{TRACK} {suffix}");
            let response = controls::toggle_at(ui, at, &control, label, on);
            switched |= response.clicked();
            name_control(response, &control).hover_hint(format!(
                "{label} on this track: {}. Its settings stay when it is off; the channel EQ \
                 window holds every dial.",
                if *on { "on" } else { "off" }
            ));
        }
        // A track never keys its own ducker, as in the channel EQ window.
        track.ducker.set_key(index, f64::from(track.ducker.key));

        // Level, pan, and outputs share one card, a row each.
        section_heading(ui, "Channel");
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 98.0), Sense::hover());
        row_surface(ui, rect, false);
        // The card sets the page's height. Its positioned controls have their own cursor.
        let mut channel_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("inspector-channel")
                .max_rect(rect),
        );
        let w = rect.width();
        let row = |top: f32, left: f32, right: f32, height: f32| {
            Rect::from_x_y_ranges(
                rect.left() + left..=rect.left() + right,
                rect.top() + top..=rect.top() + top + height,
            )
        };
        let pan_automated = !track.automation.pan.is_empty();
        lanes::strip_fader(
            &mut channel_ui,
            row(6.0, 6.0, w - 122.0, 22.0),
            TRACK,
            &mut track.gain,
        );
        gain_value(
            &mut channel_ui,
            row(6.0, w - 118.0, w - 60.0, 22.0),
            1.0,
            TRACK,
            &mut track.gain,
        );
        if unity_button(
            &mut channel_ui,
            row(8.0, w - 56.0, w - 6.0, 18.0),
            Look::Strip(1.0),
            TRACK,
        ) {
            track.gain = 1.0;
        }
        // Inset, so the automation ring stays inside the card.
        pan_slider(
            &mut channel_ui,
            row(38.0, 10.0, w - 126.0, 22.0),
            Look::Strip(1.0),
            TRACK,
            &mut track.pan,
            pan_automated,
        );
        if center_button(
            &mut channel_ui,
            row(40.0, w - 118.0, w - 60.0, 18.0),
            Look::Strip(1.0),
            TRACK,
        ) {
            track.pan = 0.0;
        }
        channel_ui.painter().text(
            Pos2::new(rect.left() + w - 54.0, rect.top() + 49.0),
            Align2::LEFT_CENTER,
            // While its lane drives the pan, the stored value is not what plays.
            if pan_automated {
                "Automated".into()
            } else {
                view::pan_description(track.pan)
            },
            FontId::proportional(11.0),
            roles().strip_text,
        );
        let cell = |x: f32| {
            Rect::from_center_size(
                Pos2::new(rect.left() + x, rect.top() + 81.0),
                Vec2::new(24.0, 22.0),
            )
        };
        for (switch, x) in TrackSwitch::ALL.into_iter().zip([18.0, 44.0, 70.0, 96.0]) {
            let on = match switch {
                TrackSwitch::Mute => &mut track.muted,
                TrackSwitch::Solo => &mut track.solo,
                TrackSwitch::Left => &mut track.left_enabled,
                TrackSwitch::Right => &mut track.right_enabled,
            };
            switched |= switch_button(
                &mut channel_ui,
                cell(x),
                Look::Strip(1.0),
                TRACK,
                switch,
                on,
            );
        }
        switched |= polarity_button(
            &mut channel_ui,
            cell(124.0),
            Look::Strip(1.0),
            TRACK,
            &mut track.polarity_inverted,
        );
        // Only a silent track says why here; the pan row says the rest.
        if let Some(silence) = track.silence(any_solo) {
            channel_ui.painter().text(
                Pos2::new(rect.left() + 144.0, rect.top() + 81.0),
                Align2::LEFT_CENTER,
                mixer::track_state(Some(silence), pan_automated, track.pan),
                FontId::proportional(11.0),
                roles().strip_text,
            );
        }

        section_heading(ui, "Sends");
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 64.0), Sense::hover());
        row_surface(ui, rect, false);
        let step = rect.width() / BUSES as f32;
        for (bus, (effect, live)) in returns.into_iter().enumerate().take(BUSES) {
            let x = rect.left() + step * (bus as f32 + 0.5);
            ui.scope(|ui| {
                // A send to an empty or switched-off return sounds nothing.
                if !live {
                    ui.multiply_opacity(0.45);
                }
                send_knob(
                    ui,
                    Rect::from_center_size(Pos2::new(x, rect.top() + 18.0), Vec2::splat(24.0)),
                    Look::Strip(1.0),
                    &format!("{TRACK} send FX {}", bus + 1),
                    &mut track.sends[bus],
                    !track.automation.sends[bus].is_empty(),
                );
                ui.painter().text(
                    Pos2::new(x, rect.top() + 41.0),
                    Align2::CENTER_CENTER,
                    format!("{} {effect}", bus + 1),
                    FontId::proportional(10.0),
                    roles().strip_text,
                );
                ui.painter().text(
                    Pos2::new(x, rect.top() + 54.0),
                    Align2::CENTER_CENTER,
                    format!("{:.0}%", track.sends[bus] * 100.0),
                    FontId::proportional(10.0),
                    roles().strip_text,
                );
            });
        }

        if open_eq {
            self.eq_track = Some(index);
        }
        if edited != self.session.project().tracks[index] {
            let command = EditCommand::SetTrack {
                index,
                track: Box::new(edited),
            };
            let result = if switched {
                self.session.edit_discrete(command)
            } else {
                self.session.edit(command)
            };
            self.apply(result);
            ui.ctx().request_repaint();
        }
    }
}

/// The page's title in the heading face, at the left, and a muted line
/// below it for each of `lines`.
fn title(ui: &mut egui::Ui, text: &str, lines: &[&str]) {
    ui.add(egui::Label::new(view::heading(text, 16.0)).truncate());
    for line in lines {
        ui.add(
            egui::Label::new(egui::RichText::new(*line).size(12.0).color(roles().muted)).truncate(),
        );
    }
}

/// A group heading: small uppercase muted text.
fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(title_text(text))
            .size(11.0)
            .color(roles().muted),
    );
}

fn note(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).size(12.0).color(roles().muted)).wrap());
}
