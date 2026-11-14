//! The Inspector at the right of the score: the song when nothing is
//! selected, the selected track, the selected clip, or a summary of several
//! selected clips. Its fields edit through the clip menu's and the Mixer's
//! operations, so every change is an undo step. Its controls carry their own
//! names ("Inspector track ...", "Inspector clip ..."), so they never share a
//! name with the Mixer, the Mix view, or the clip menu shown beside them.
use super::mixer::{
    center_button, eq_button, gain_value, pan_slider, polarity_button, row_surface, send_knob,
    switch_button, unity_button,
};
use super::room::Panel;
use super::sounds::{fold_button, panel_strip};
use super::view::{GROUPS, group_color, section_color, title_text};
use super::*;
use mixjam_techno::application::fx::BUSES;

/// The open panel's width, and the folded strip's.
pub(super) const INSPECTOR_WIDTH: f32 = 280.0;
pub(super) const INSPECTOR_STRIP_WIDTH: f32 = 28.0;
/// The name that every track page control starts with.
const TRACK: &str = "Inspector track";
/// The name that every clip page control starts with.
pub(super) const CLIP: &str = "Inspector clip";

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
                ui.push_id((self.boards.master_view_generation, page), |ui| {
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
                });
            });
    }

    fn song_page(&mut self, ui: &mut egui::Ui) {
        let project = self.session.project();
        let meta = view::song_meta(project).unwrap_or_else(|| "Empty".into());
        title(
            ui,
            &project.title,
            &format!("{:.0} BPM  ·  {meta}", project.bpm),
        );
        let clips: Vec<&Clip> = project.clips.iter().collect();
        self.category_counts(ui, &clips);
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
            let first = start / TICKS_PER_BAR + 1;
            let last = next.div_ceil(TICKS_PER_BAR).max(first);
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
    fn category_counts(&self, ui: &mut egui::Ui, clips: &[&Clip]) {
        section_heading(ui, "Categories");
        let library = self.session.library();
        let mut counts: Vec<(String, usize)> = Vec::new();
        let mut missing = 0;
        for clip in clips {
            let Some(sample) = library.sample_by_id(clip.sample_id) else {
                missing += 1;
                continue;
            };
            let group = GROUPS
                .iter()
                .find(|group| group.eq_ignore_ascii_case(&sample.group_name))
                .map_or(sample.group_name.clone(), |group| (*group).to_owned());
            match counts.iter_mut().find(|(name, _)| *name == group) {
                Some((_, count)) => *count += 1,
                None => counts.push((group, 1)),
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
            .map(|(name, count)| (group_color(&name), name, count))
            .chain((missing > 0).then(|| (roles().raised, "Missing sound".to_owned(), missing)));
        for (color, name, count) in rows {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
            ui.painter().rect_filled(
                Rect::from_center_size(
                    rect.left_center() + Vec2::new(7.0, 0.0),
                    Vec2::new(14.0, 10.0),
                ),
                2.0,
                color,
            );
            ui.painter().text(
                rect.left_center() + Vec2::new(22.0, 0.0),
                Align2::LEFT_CENTER,
                &name,
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
            &format!(
                "Track {}  ·  {count} {}",
                index + 1,
                if count == 1 { "clip" } else { "clips" }
            ),
        );
        let any_solo = project.any_solo();
        let mut edited = project.tracks[index].clone();
        let track = &mut edited;
        let width = ui.available_width();
        let mut open_eq = false;
        let mut switched = false;

        section_heading(ui, "Level");
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 64.0), Sense::hover());
        row_surface(ui, rect, false);
        let row = |top: f32, left: f32, right: f32, height: f32| {
            Rect::from_x_y_ranges(
                rect.left() + left..=rect.left() + right,
                rect.top() + top..=rect.top() + top + height,
            )
        };
        lanes::lane_fader(ui, row(6.0, 8.0, 132.0, 22.0), TRACK, &mut track.gain);
        gain_value(
            ui,
            row(6.0, 136.0, 200.0, 22.0),
            1.0,
            TRACK,
            &mut track.gain,
        );
        if unity_button(ui, row(8.0, 204.0, width - 6.0, 18.0), 1.0, TRACK) {
            track.gain = 1.0;
        }
        pan_slider(
            ui,
            row(36.0, 8.0, 124.0, 22.0),
            1.0,
            TRACK,
            &mut track.pan,
            !track.automation.pan.is_empty(),
        );
        if center_button(ui, row(38.0, 130.0, 182.0, 18.0), 1.0, TRACK) {
            track.pan = 0.0;
        }
        ui.painter().text(
            Pos2::new(rect.left() + 190.0, rect.top() + 47.0),
            Align2::LEFT_CENTER,
            // While its lane drives the pan, the stored value is not what plays.
            if track.automation.pan.is_empty() {
                view::pan_description(track.pan)
            } else {
                "Automated".into()
            },
            FontId::proportional(11.0),
            roles().strip_text,
        );

        section_heading(ui, "Outputs");
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::hover());
        row_surface(ui, rect, false);
        let cell = |x: f32, w: f32| {
            Rect::from_center_size(
                Pos2::new(rect.left() + x, rect.center().y),
                Vec2::new(w, 24.0),
            )
        };
        for (switch, x) in TrackSwitch::ALL.into_iter().zip([20.0, 46.0, 72.0, 98.0]) {
            let on = match switch {
                TrackSwitch::Mute => &mut track.muted,
                TrackSwitch::Solo => &mut track.solo,
                TrackSwitch::Left => &mut track.left_enabled,
                TrackSwitch::Right => &mut track.right_enabled,
            };
            switched |= switch_button(ui, cell(x, 24.0), 1.0, TRACK, switch, on);
        }
        switched |= polarity_button(
            ui,
            cell(126.0, 24.0),
            1.0,
            TRACK,
            &mut track.polarity_inverted,
        );
        ui.painter().text(
            Pos2::new(rect.left() + 148.0, rect.center().y),
            Align2::LEFT_CENTER,
            mixer::track_state(
                track.muted,
                track.left_enabled || track.right_enabled,
                any_solo && !track.solo,
                !track.automation.pan.is_empty(),
                track.pan,
            ),
            FontId::proportional(11.0),
            roles().strip_text,
        );

        section_heading(ui, "Sends");
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 52.0), Sense::hover());
        row_surface(ui, rect, false);
        let step = rect.width() / BUSES as f32;
        for bus in 0..BUSES {
            let x = rect.left() + step * (bus as f32 + 0.5);
            send_knob(
                ui,
                Rect::from_center_size(Pos2::new(x, rect.top() + 18.0), Vec2::splat(24.0)),
                1.0,
                &format!("{TRACK} send FX {}", bus + 1),
                &mut track.sends[bus],
                !track.automation.sends[bus].is_empty(),
            );
            ui.painter().text(
                Pos2::new(x, rect.bottom() - 9.0),
                Align2::CENTER_CENTER,
                format!("FX {}  {:.0}%", bus + 1, track.sends[bus] * 100.0),
                FontId::proportional(10.0),
                roles().strip_text,
            );
        }

        section_heading(ui, "Channel processing");
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 96.0), Sense::hover());
        eq::response_curve(ui, rect, &track.eq, &format!("{TRACK} EQ"));
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
        if track.ducker.on && usize::from(track.ducker.key) == index + 1 {
            track.ducker.key = if index == 0 { 2 } else { 1 };
        }
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
        open_eq |= eq_button(
            ui,
            Rect::from_min_size(rect.min, Vec2::new(64.0, 24.0)),
            1.0,
            TRACK,
            mixer::processing_on(track),
        );
        ui.painter().text(
            rect.min + Vec2::new(72.0, 12.0),
            Align2::LEFT_CENTER,
            "Opens the channel EQ window",
            FontId::proportional(11.0),
            roles().muted,
        );

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

    fn clip_page(&mut self, ui: &mut egui::Ui, id: u64) {
        let project = self.session.project();
        let Some(clip) = project.clips.iter().find(|clip| clip.id == id).cloned() else {
            return;
        };
        let sample = self.session.library().sample_by_id(clip.sample_id);
        let name = sample.map_or_else(
            || {
                self.session
                    .missing_sample(clip.sample_id)
                    .map_or_else(|| "Missing sound".to_owned(), |m| m.name.clone())
            },
            |sample| sample.name.clone(),
        );
        let first = clip.start_tick / TICKS_PER_BAR + 1;
        let last = (clip.start_tick + clip.length_ticks)
            .div_ceil(TICKS_PER_BAR)
            .max(first);
        let category = sample.map_or("Missing", |sample| sample.group_name.as_str());
        title(
            ui,
            &name,
            &format!(
                "{category}  ·  {}  ·  track {}",
                if first == last {
                    format!("bar {first}")
                } else {
                    format!("bars {first} to {last}")
                },
                clip.track + 1
            ),
        );
        section_heading(ui, "Clip");
        if sample.is_none() {
            note(
                ui,
                "Its sound is missing. Resolve it from the clip's menu, or with Import details \
                 in the status bar.",
            );
            return;
        }
        // Tighter than the clip menu, so the fade curves fit the panel.
        ui.spacing_mut().button_padding.x = 4.0;
        ui.spacing_mut().item_spacing.x = 4.0;
        self.clip_properties(ui, id, CLIP);
    }

    fn clips_page(&mut self, ui: &mut egui::Ui, count: usize) {
        let project = self.session.project();
        let clips: Vec<&Clip> = project
            .clips
            .iter()
            .filter(|clip| self.selection.clips.contains(&clip.id))
            .collect();
        let start = clips.iter().map(|clip| clip.start_tick).min().unwrap_or(0);
        let end = clips
            .iter()
            .map(|clip| clip.start_tick + clip.length_ticks)
            .max()
            .unwrap_or(0);
        let tracks: BTreeSet<usize> = clips.iter().map(|clip| clip.track).collect();
        let first = start / TICKS_PER_BAR + 1;
        let last = end.div_ceil(TICKS_PER_BAR).max(first);
        title(
            ui,
            &format!("{count} clips"),
            &format!(
                "bars {first} to {last}  ·  {} {}",
                tracks.len(),
                if tracks.len() == 1 { "track" } else { "tracks" }
            ),
        );
        self.category_counts(ui, &clips);
        note(
            ui,
            "Drag any of them to move them together; Delete removes them all. Select one clip \
             to edit its gain, fades, pitch, and length here.",
        );
    }
}

/// The page's title in the heading face, and a muted line below it.
fn title(ui: &mut egui::Ui, text: &str, detail: &str) {
    let width = ui.available_width();
    ui.add_sized(
        Vec2::new(width, 22.0),
        egui::Label::new(view::heading(text, 16.0)).truncate(),
    );
    ui.add(egui::Label::new(egui::RichText::new(detail).size(12.0).color(roles().muted)).wrap());
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
