//! The Inspector at the right of the score: the song when nothing is
//! selected, the selected track's channel processing, the selected clip, or a
//! summary of several selected clips. Its controls carry their own names
//! ("Inspector track ...", "Inspector clip ..."), so they never share a name
//! with the Mixer or the clip menu.
use super::mixer::row_surface;
use super::room::Panel;
use super::sounds::{fold_button, panel_strip};
use super::view::{GROUPS, group_color, section_color, title_text};
use super::*;
use mixjam_techno::application::Track;

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
            self.inspector_folded(ctx);
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
                self.inspector_header(ui);
                let page = self.inspector_page();
                self.inspector_scroll(ui, page);
            });
    }

    fn inspector_folded(&mut self, ctx: &egui::Context) {
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
                    "Show the Inspector: the song, the selected track, or the selected clips. In \
                     a narrow window the Sounds panel folds to make room.",
                )
                .clicked()
                {
                    self.open_inspector();
                }
            });
    }

    fn inspector_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.set_height(26.0);
            // Painted in the heading face; the status bar's switch is the
            // one control named Inspector.
            let heading = ui.painter().layout_no_wrap(
                title_text("Inspector"),
                FontId::new(15.0, theme::heading_family()),
                roles().text,
            );
            let (rect, _) = ui.allocate_exact_size(heading.size(), Sense::hover());
            ui.painter().galley(rect.min, heading, roles().text);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if fold_button(ui, "Fold the Inspector", 1.0).clicked() {
                    self.set_layout(|layout| layout.inspector = false);
                }
            });
        });
    }

    fn inspector_scroll(&mut self, ui: &mut egui::Ui, page: Page) {
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
        let mut edited = project.tracks[index].clone();
        let track = &mut edited;
        let open_eq = channel_eq_button(ui, track);
        let switched = channel_power_switches(ui, track, index);

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

fn channel_eq_button(ui: &mut egui::Ui, track: &Track) -> bool {
    section_heading(ui, "Channel processing");
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 80.0), Sense::hover());
    let name = format!("{TRACK} EQ");
    eq::response_curve(ui, rect, &track.eq, &name);
    let response = ui.interact(rect, ui.id().with(&name), Sense::click());
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
        Pos2::new(rect.right(), rect.top() - 11.0),
        Align2::RIGHT_CENTER,
        "Open EQ",
        FontId::proportional(10.0),
        if lit { roles().text } else { roles().muted },
    );
    view::named(&response, egui::WidgetType::Button, &name);
    response
        .hover_hint(format!(
            "Open the channel strip for EQ, drive, compressor, stereo, filter and ducker. \
             Processing is {}.",
            if track.processing_on() { "on" } else { "off" }
        ))
        .clicked()
}

fn channel_power_switches(ui: &mut egui::Ui, track: &mut Track, index: usize) -> bool {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 56.0), Sense::hover());
    let half = (width - 6.0) / 2.0;
    let mut switched = false;
    let ducker_was_on = track.ducker.on;
    let inserts = [
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
            "{label} on this track: {}. Its settings stay when it is off.",
            if *on { "on" } else { "off" }
        ));
    }
    if !ducker_was_on && track.ducker.on {
        track.ducker.set_key(index, f64::from(track.ducker.key));
    }
    switched
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
