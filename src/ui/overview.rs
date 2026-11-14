//! The song overview above the score: the whole song at a glance, with its
//! sections, one thin line per clip in its category color, the score's
//! visible window as a frame, the loop, and the playhead. Clicking or
//! dragging it moves the score's view; it never edits the song. Folded, it
//! leaves a named bar that brings it back.
use super::*;

/// The open strip's height.
pub(super) const OVERVIEW_HEIGHT: f32 = 60.0;
/// The folded bar's height.
pub(super) const OVERVIEW_BAR_HEIGHT: f32 = 20.0;
/// The space between the overview and the score toolbar.
pub(super) const OVERVIEW_GAP: f32 = 8.0;
/// The song's span is inset this far from the strip's sides.
const INSET: f32 = 12.0;
const HIDE_SIZE: f32 = 22.0;

/// The song overview's rect, the tick it asks the score to show, and its
/// cached clip lines.
#[derive(Default)]
pub(super) struct OverviewState {
    /// A tick the song overview asked the score to centre on.
    pub(super) target: Option<f32>,
    /// The song overview's rect, open or folded, as last laid out.
    pub(super) rect: Option<Rect>,
    /// The song overview's clip lines and what they were built from.
    pub(super) lines: Option<(ClipLinesKey, Arc<egui::Mesh>)>,
}

/// The score's horizontal view, as the arrangement last laid it out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ScoreView {
    /// The scroll offset of the lanes, in points from the song's start.
    pub(super) offset: f32,
    /// The width of the visible lanes.
    pub(super) visible: f32,
    pub(super) bar_width: f32,
}

impl ScoreView {
    pub(super) fn start_tick(self) -> f32 {
        self.offset / self.bar_width * TICKS_PER_BAR as f32
    }

    pub(super) fn ticks(self) -> f32 {
        self.visible / self.bar_width * TICKS_PER_BAR as f32
    }
}

/// What the overview's clip lines depend on. The mesh is rebuilt only when
/// one of them changes, so a long song costs one copy per frame.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct ClipLinesKey {
    project: (u64, u64),
    clips: usize,
    catalog: u64,
    rect: Rect,
    span: f32,
    sections: bool,
    /// The theme, by its canvas color.
    canvas: Color32,
}

/// The open strip, laid out before the score and painted after it, when the
/// score's view for this frame is known.
pub(super) struct OverviewSlot {
    rect: Rect,
    strip: Response,
    hide: Response,
}

/// The ticks the overview spans: the song, its last section, and the
/// playhead, and at least the score's visible window, so the frame always
/// fits and a short or empty song keeps its true size.
pub(super) fn overview_span(project: &Project, cursor: u32, view_ticks: f32) -> f32 {
    let last_section = project.sections.last().map_or(0, |section| {
        section.start_tick.saturating_add(TICKS_PER_BAR)
    });
    (project.end_tick().max(last_section).max(cursor) as f32)
        .max(view_ticks)
        .max(TICKS_PER_BAR as f32)
}

/// The points the folded bar takes above the score toolbar.
pub(super) fn overview_bar_reserve(ui_spacing: f32) -> f32 {
    OVERVIEW_BAR_HEIGHT + ui_spacing + OVERVIEW_GAP
}

/// The score's toolbar, ruler, and a 150-point lane viewport: the lower
/// panel's arrangement reserve less the score's 14-point top and bottom
/// margins.
const SCORE_ROOM: f32 = BottomModule::ARRANGEMENT_RESERVE - 28.0;

/// Whether the open strip fits above a score `height` points high and still
/// leaves the score its room.
pub(super) fn overview_fits(height: f32, ui_spacing: f32) -> bool {
    height >= OVERVIEW_HEIGHT + ui_spacing + OVERVIEW_GAP + SCORE_ROOM
}

impl MixJamApp {
    /// Lay out the overview at the top of the score and take its clicks,
    /// drags, and keys. A move of the view takes effect in this frame's
    /// score. Returns the open strip, which `paint_overview` draws after the
    /// score. In a score too short for the strip, it shows its bar until
    /// there is room again. If even the bar cannot fit, it hides entirely;
    /// the saved layout stays as it is.
    pub(super) fn song_overview(&mut self, ui: &mut egui::Ui) -> Option<OverviewSlot> {
        self.overview.rect = None;
        if ui.available_height() < overview_bar_reserve(ui.spacing().item_spacing.y) + SCORE_ROOM {
            return None;
        }
        let wanted = self.session.layout().overview;
        if !wanted || !overview_fits(ui.available_height(), ui.spacing().item_spacing.y) {
            let (rect, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), OVERVIEW_BAR_HEIGHT),
                Sense::hover(),
            );
            self.overview.rect = Some(rect);
            if overview_bar(ui, rect, wanted).clicked() && !wanted {
                self.set_layout(|layout| layout.overview = true);
            }
            ui.add_space(OVERVIEW_GAP);
            return None;
        }
        let (rect, strip) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), OVERVIEW_HEIGHT),
            Sense::click_and_drag(),
        );
        self.overview.rect = Some(rect);
        // The hide button sits over the strip's lower right corner and takes
        // the pointer there.
        let hide = ui.interact(
            Rect::from_min_size(
                rect.right_bottom() - Vec2::new(5.0 + HIDE_SIZE, 4.0 + HIDE_SIZE),
                Vec2::splat(HIDE_SIZE),
            ),
            ui.id().with("hide-song-overview"),
            Sense::click(),
        );
        if let Some(view) = self.score.view {
            let span = overview_span(
                self.session.project(),
                self.session.cursor_tick(),
                view.ticks(),
            );
            let lane = rect.x_range().shrink(INSET);
            if (strip.is_pointer_button_down_on() || strip.clicked())
                && let Some(pos) = strip.interact_pointer_pos()
            {
                let tick = (pos.x - lane.min) / lane.span() * span;
                self.overview.target = Some(tick.clamp(0.0, span));
                strip.request_focus();
            }
            if strip.has_focus() {
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        strip.id,
                        egui::EventFilter {
                            horizontal_arrows: true,
                            ..Default::default()
                        },
                    )
                });
                // Left and Right move the view by a screen; Home and End go
                // to the song's start and end.
                let half = view.ticks() / 2.0;
                let centre = view.start_tick() + half;
                let target = ui.input_mut(|input| {
                    let modifiers = input.modifiers;
                    if input.consume_key(modifiers, egui::Key::ArrowLeft) {
                        Some(centre - view.ticks())
                    } else if input.consume_key(modifiers, egui::Key::ArrowRight) {
                        Some(centre + view.ticks())
                    } else if input.consume_key(modifiers, egui::Key::Home) {
                        Some(half)
                    } else if input.consume_key(modifiers, egui::Key::End) {
                        Some(span - half)
                    } else {
                        None
                    }
                });
                if let Some(target) = target {
                    self.overview.target = Some(target.clamp(0.0, span));
                }
            }
        }
        strip.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Song overview")
        });
        let strip = strip.hover_hint(
            "Song overview: the whole song, its sections, the visible part of the score, the \
             loop, and the playhead. Click or drag to move the score's view; Left and Right \
             move it by a screen, Home and End go to the start and the end. It never edits \
             the song.",
        );
        hide.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Hide the song overview")
        });
        let hide = hide.hover_hint("Hide the song overview");
        if hide.clicked() {
            self.set_layout(|layout| layout.overview = false);
        }
        ui.add_space(OVERVIEW_GAP);
        Some(OverviewSlot { rect, strip, hide })
    }

    /// One thin line per clip, a row per shown track, from the same ticks the
    /// score's clip rectangles use.
    fn clip_lines(&self, rect: Rect, span: f32, sections: bool) -> egui::Mesh {
        let project = self.session.project();
        let lane = rect.x_range().shrink(INSET);
        let x = |tick: f32| lane.min + tick / span * lane.span();
        let tracks = occupied_tracks(project);
        let mut row_of = [None; TRACK_COUNT];
        for (row, &track) in tracks.iter().enumerate() {
            row_of[track] = Some(row);
        }
        let top = rect.top() + if sections { 22.0 } else { 6.0 };
        let row_height = (rect.bottom() - 8.0 - top) / tracks.len().max(1) as f32;
        let any_solo = project.any_solo();
        let library = self.session.library();
        let mut colors = std::collections::HashMap::new();
        let mut mesh = egui::Mesh::default();
        for clip in &project.clips {
            let Some(row) = row_of.get(clip.track).copied().flatten() else {
                continue;
            };
            let mut color = *colors.entry(clip.sample_id).or_insert_with(|| {
                library
                    .sample_by_id(clip.sample_id)
                    .filter(|_| self.session.missing_sample(clip.sample_id).is_none())
                    .map_or(roles().raised, |sample| group_color(&sample.group_name))
            });
            if !project.tracks[clip.track].audible(any_solo) {
                color = roles().canvas.lerp_to_gamma(color, 0.3);
            }
            let left = x(clip.start_tick as f32);
            let right = x(clip.start_tick.saturating_add(clip.length_ticks) as f32);
            let y = top + row as f32 * row_height;
            mesh.add_colored_rect(
                Rect::from_min_size(
                    Pos2::new(left, y),
                    Vec2::new((right - left - 1.0).max(1.5), (row_height - 0.6).max(1.0)),
                ),
                color,
            );
        }
        mesh
    }

    /// Paint the open strip from the song and the score's view of this frame.
    pub(super) fn paint_overview(&mut self, ui: &egui::Ui, slot: &OverviewSlot) {
        let rect = slot.rect;
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, roles().panel);
        let Some(view) = self.score.view else {
            return;
        };
        let project = self.session.project();
        let cursor = self.session.cursor_tick();
        let span = overview_span(project, cursor, view.ticks());
        let lane = rect.x_range().shrink(INSET);
        let x = |tick: f32| lane.min + tick / span * lane.span();
        let sections = self.sections.bookmarks && !project.sections.is_empty();
        if sections {
            let current = project
                .sections
                .iter()
                .rposition(|section| section.start_tick <= cursor);
            for (index, section) in project.sections.iter().enumerate() {
                let left = x(section.start_tick as f32);
                let right = project
                    .sections
                    .get(index + 1)
                    .map_or(lane.max, |next| x(next.start_tick as f32));
                // Every second section is shaded, so neighbours read apart.
                if index % 2 == 1 {
                    painter.rect_filled(
                        Rect::from_x_y_ranges(left..=right, rect.y_range()),
                        0.0,
                        roles().canvas.gamma_multiply(0.55),
                    );
                }
                painter.rect_filled(
                    Rect::from_x_y_ranges(
                        left..=(right - 1.0).max(left + 1.0),
                        rect.top()..=rect.top() + 2.0,
                    ),
                    0.0,
                    section_color(index),
                );
                let color = if current == Some(index) {
                    roles().text
                } else {
                    roles().muted
                };
                let name = painter.layout_job(elided(
                    &section.name.to_uppercase(),
                    10.0,
                    color,
                    right - left - 6.0,
                    1,
                ));
                painter.galley(Pos2::new(left + 4.0, rect.top() + 5.0), name, color);
            }
        }
        let key = ClipLinesKey {
            project: self.session.project_change_token(),
            clips: project.clips.len(),
            catalog: self.session.catalog_revision(),
            rect,
            span,
            sections,
            canvas: roles().canvas,
        };
        if self
            .overview
            .lines
            .as_ref()
            .is_none_or(|(old, _)| *old != key)
        {
            self.overview.lines = Some((key, Arc::new(self.clip_lines(rect, span, sections))));
        }
        if let Some((_, mesh)) = &self.overview.lines
            && !mesh.is_empty()
        {
            painter.add(egui::Shape::Mesh(mesh.clone()));
        }
        if let Some([start, end]) = project.loop_region {
            painter.rect_filled(
                Rect::from_x_y_ranges(
                    x(start as f32)..=x(end as f32),
                    rect.bottom() - 3.0..=rect.bottom() - 1.0,
                ),
                0.0,
                roles().accent,
            );
        }
        // The score's visible window. Past the song's end it stays at the
        // strip's right edge.
        let right = x(view.start_tick() + view.ticks()).min(rect.right() - 4.0);
        let left = x(view.start_tick()).min(right - 8.0);
        let frame = Rect::from_x_y_ranges(left..=right, rect.top() + 4.0..=rect.bottom() - 4.0);
        painter.rect_filled(frame, 3.0, roles().text.gamma_multiply(0.08));
        painter.rect_stroke(
            frame,
            3.0,
            Stroke::new(1.0_f32, roles().text.gamma_multiply(0.85)),
            egui::StrokeKind::Inside,
        );
        let playhead = x(cursor as f32).round();
        painter.rect_filled(
            Rect::from_x_y_ranges(playhead..=playhead + 1.5, rect.y_range()),
            0.0,
            roles().accent,
        );
        // The hide button, over the corner.
        let hide = slot.hide.rect;
        painter.rect_filled(
            hide,
            4.0,
            if slot.hide.hovered() || slot.hide.has_focus() {
                ui.visuals().widgets.hovered.weak_bg_fill
            } else {
                roles().panel
            },
        );
        painter.rect_stroke(
            hide,
            4.0,
            Stroke::new(1.0_f32, roles().line),
            egui::StrokeKind::Inside,
        );
        vertical_chevron(
            &painter,
            hide.center(),
            -1.0,
            if slot.hide.hovered() {
                roles().text
            } else {
                roles().muted
            },
        );
        focus_outline(ui, &slot.hide, roles().accent);
        focus_outline(ui, &slot.strip, roles().accent);
    }
}

/// The folded overview: a bar with a chevron and the overview's name. The
/// whole bar opens it again. When the overview is set to show but the score
/// is too short (`no_room`), the bar says why and does nothing on a click.
fn overview_bar(ui: &mut egui::Ui, rect: Rect, no_room: bool) -> Response {
    let response = ui.interact(
        rect,
        ui.id().with("show-song-overview"),
        if no_room {
            Sense::hover()
        } else {
            Sense::click()
        },
    );
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        4.0,
        if !no_room && (response.hovered() || response.has_focus()) {
            ui.visuals().widgets.hovered.weak_bg_fill
        } else {
            roles().panel
        },
    );
    focus_outline(ui, &response, roles().accent);
    let color = if response.hovered() && !no_room {
        roles().text
    } else {
        roles().muted
    };
    if !no_room {
        vertical_chevron(
            painter,
            Pos2::new(rect.left() + 14.0, rect.center().y),
            1.0,
            color,
        );
    }
    let name = painter.text(
        Pos2::new(rect.left() + 26.0, rect.center().y),
        Align2::LEFT_CENTER,
        "SONG OVERVIEW",
        FontId::proportional(10.0),
        color,
    );
    if no_room {
        let label = "Song overview, folded for room";
        painter.text(
            Pos2::new(name.right() + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            "Needs a taller score",
            FontId::proportional(10.0),
            roles().muted,
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label));
        response.hover_hint(
            "The song overview folds while the score is too short for it and a 150-point lane \
             view. Make the window taller, fold the lower panel's board, or maximise the score \
             to show it.",
        )
    } else {
        let label = "Show the song overview";
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        response.hover_hint(label)
    }
}

/// A chevron pointing down (`direction` 1) or up (-1).
fn vertical_chevron(painter: &egui::Painter, at: Pos2, direction: f32, color: Color32) {
    let stroke = Stroke::new(1.5_f32, color);
    let tip = at + Vec2::new(0.0, direction * 2.5);
    let back = tip - Vec2::new(0.0, direction * 5.0);
    painter.line_segment([back + Vec2::new(-5.0, 0.0), tip], stroke);
    painter.line_segment([back + Vec2::new(5.0, 0.0), tip], stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overview_spans_the_song_and_at_least_the_visible_window() {
        let mut project = Project::default();
        let bar = TICKS_PER_BAR as f32;
        // An empty song spans the visible window, or its own length.
        let end = project.end_tick() as f32;
        assert_eq!(overview_span(&project, 0, 64.0 * bar), 64.0 * bar);
        assert_eq!(overview_span(&project, 0, 0.0), end.max(bar));
        // A later section, or the playhead past the end, widens it.
        project.sections.push(mixjam_techno::application::Section {
            start_tick: 40 * TICKS_PER_BAR,
            name: "Drop".to_owned(),
        });
        assert_eq!(overview_span(&project, 0, 16.0 * bar), 41.0 * bar);
        assert_eq!(
            overview_span(&project, 50 * TICKS_PER_BAR, 16.0 * bar),
            50.0 * bar
        );
    }
}
