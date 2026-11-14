//! The score: the lane headers, the ruler, the lanes with their clips, and the
//! gestures on them. `timeline` is one frame's drawing and input handling for
//! the whole arrangement. It lays the frame out here, draws the lanes in
//! `score_lanes.rs` and the lane headers in `track_headers.rs`, then draws the
//! ruler and applies the headers' changes.
use super::*;

/// The score's zoom, follow mode, and loop drag, and its layout as last drawn
/// for the drags, the song overview, and the agent.
pub(in super::super) struct ScoreState {
    pub(in super::super) rect: Option<Rect>,
    pub(in super::super) drop_rect: Option<Rect>,
    pub(in super::super) ruler_rect: Option<Rect>,
    pub(in super::super) tracks: Vec<usize>,
    pub(in super::super) row_by_track: [Option<usize>; TRACK_COUNT],
    pub(in super::super) add_track: Option<usize>,
    pub(in super::super) row_height: f32,
    pub(in super::super) bar_width: f32,
    pub(in super::super) zoom: f32,
    pub(in super::super) follow_playhead: bool,
    pub(in super::super) last_follow_position: Option<(u32, f32, f32)>,
    /// The score's horizontal view as last laid out, for the song overview.
    pub(in super::super) view: Option<overview::ScoreView>,
    pub(in super::super) loop_start: Option<u32>,
}

impl Default for ScoreState {
    fn default() -> Self {
        Self {
            rect: None,
            drop_rect: None,
            ruler_rect: None,
            tracks: Vec::new(),
            row_by_track: [None; TRACK_COUNT],
            add_track: None,
            row_height: 30.0,
            bar_width: 62.0,
            zoom: 1.0,
            follow_playhead: false,
            last_follow_position: None,
            view: None,
            loop_start: None,
        }
    }
}

/// One frame's score layout, shared by the lanes, the lane headers, and the ruler.
pub(super) struct ScoreFrame {
    /// The tracks shown as rows, in order: the used ones, then the add row.
    pub(super) tracks: Vec<usize>,
    /// The empty track that the last row offers, if any.
    pub(super) add_track: Option<usize>,
    /// The number of rows that hold clips.
    pub(super) occupied_count: usize,
    pub(super) row_height: f32,
    pub(super) bar_width: f32,
    /// The bars the score shows.
    pub(super) bars: u32,
    /// The width of the lanes.
    pub(super) width: f32,
    /// The height of all rows.
    pub(super) track_height: f32,
    pub(super) cursor_tick: u32,
    /// The Mix view widens the lane headers into mixer rows.
    pub(super) mix: bool,
    pub(super) label_width: f32,
    pub(super) mix_offset: f32,
    /// The space left of the lanes: the headers and the item spacing.
    pub(super) lane_inset: f32,
    pub(super) viewport_height: f32,
    /// The width of the lanes that the viewport shows.
    pub(super) visible_width: f32,
    /// The strip above the lanes that holds the ruler.
    pub(super) ruler_strip: Rect,
    pub(super) any_solo: bool,
}

impl ScoreFrame {
    /// The left edge of the headers and the ruler strip.
    pub(super) fn body_left(&self) -> f32 {
        self.ruler_strip.left()
    }
}

/// What the lane headers changed this frame. The changes apply after the score
/// is drawn.
#[derive(Default)]
pub(super) struct HeaderChanges {
    /// A Mute, Solo, or other switch that was clicked: the track, the switch, and its new state.
    pub(super) route: Option<(usize, TrackSwitch, bool)>,
    /// A lane fader that moved: the track and its new gain.
    pub(super) gain: Option<(usize, f32)>,
    /// In the Mix view, each track's sound summary, built in one pass.
    pub(super) summaries: Vec<(String, String)>,
    pub(super) mix: mixer::Changes,
    /// Horizontal wheel input that the Mix view columns took.
    pub(super) mix_wheel: f32,
}

impl MixJamApp {
    pub(in super::super) fn timeline(&mut self, ui: &mut egui::Ui) {
        if !self.sections.bookmarks {
            self.cancel_section_edit();
        }
        let overview = self.song_overview(ui);
        self.score_toolbar(ui);
        ui.add_space(12.0);
        self.score.rect = None;
        self.score.drop_rect = None;
        self.score.ruler_rect = None;
        let frame = self.score_frame(ui);
        let mut headers = HeaderChanges {
            summaries: if frame.mix {
                mixer_sample_summaries(self.session.project(), self.session.library())
            } else {
                Vec::new()
            },
            ..Default::default()
        };
        let mut painted_offset_x = 0.0;
        let scroll_area = self.score_scroll_area(ui, &frame);
        let scroll = scroll_area.show_viewport(ui, |ui, viewport| {
            painted_offset_x = viewport.left();
            self.score_body(ui, &frame, &mut headers);
        });
        self.score.view = Some(overview::ScoreView {
            offset: scroll.state.offset.x,
            visible: frame.visible_width,
            bar_width: frame.bar_width,
        });
        if frame.mix {
            ui.push_id(self.boards.master_view_generation, |ui| {
                self.mix_block(
                    ui,
                    Rect::from_min_size(frame.ruler_strip.min, Vec2::new(frame.label_width, 26.0)),
                    Rect::from_min_size(
                        Pos2::new(frame.body_left(), scroll.inner_rect.bottom() + 6.0),
                        Vec2::new(frame.label_width, mix_view::BLOCK_ROW * 2.0 + 2.0),
                    ),
                    (
                        frame.mix_offset,
                        frame.occupied_count == 0,
                        headers.mix_wheel,
                    ),
                    std::mem::take(&mut headers.mix),
                );
            });
        }
        if let Some(slot) = &overview {
            self.paint_overview(ui, slot);
        }
        self.bar_ruler(ui, &frame, scroll.inner_rect.right(), painted_offset_x);
        self.apply_header_changes(ui, headers);
    }

    /// Choose this frame's rows and lay out the score, and allocate the ruler strip.
    fn score_frame(&mut self, ui: &mut egui::Ui) -> ScoreFrame {
        let occupied = occupied_tracks(self.session.project());
        let occupied_count = occupied.len();
        self.score.add_track =
            if self.recording.track < TRACK_COUNT && !occupied.contains(&self.recording.track) {
                Some(self.recording.track)
            } else {
                (0..TRACK_COUNT).find(|index| !occupied.contains(index))
            };
        self.score.tracks = occupied;
        self.score.tracks.extend(self.score.add_track);
        self.score.row_by_track.fill(None);
        for (row, &track) in self.score.tracks.iter().enumerate() {
            self.score.row_by_track[track] = Some(row);
        }
        let tracks = self.score.tracks.clone();
        let add_track = self.score.add_track;
        let row_count = tracks.len();
        // The Lane height option sets it; it stays the same at any window or
        // panel size, so more room shows more rows.
        let row_height = self.session.options().lane_height.points();
        let bar_width = 62.0 * self.score.zoom;
        self.score.row_height = row_height;
        self.score.bar_width = bar_width;
        let track_height = row_height * row_count as f32;
        let cursor_tick = self.session.cursor_tick();
        let project = self.session.project();
        let last_section = project
            .sections
            .last()
            .map_or(0, |section| section.start_tick);
        let bars =
            (project.end_tick().max(cursor_tick).max(last_section) / TICKS_PER_BAR + 16).max(64);
        let width = bars as f32 * bar_width;
        // The Mix view widens the lane headers into mixer rows and keeps the
        // Master and the returns below the lanes.
        let mix = self.mix.shown;
        let label_width = if mix {
            mix_view::column_width(ui.available_width())
        } else {
            lanes::LANE_HEADER_WIDTH
        };
        let mix_offset = self
            .mix
            .scroll
            .clamp(0.0, mix_view::max_offset(label_width));
        let lane_inset = label_width + ui.spacing().item_spacing.x;
        let viewport_height =
            (ui.available_height() - 26.0 - 8.0 - if mix { mix_view::BLOCK_HEIGHT } else { 0.0 })
                .max(80.0);
        let (ruler_strip, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
        let visible_width =
            (ui.available_width() - lane_inset - ui.spacing().scroll.allocated_width()).max(1.0);
        ScoreFrame {
            tracks,
            add_track,
            occupied_count,
            row_height,
            bar_width,
            bars,
            width,
            track_height,
            cursor_tick,
            mix,
            label_width,
            mix_offset,
            lane_inset,
            viewport_height,
            visible_width,
            ruler_strip,
            any_solo: self.session.project().any_solo(),
        }
    }

    /// The score's scroll area, moved to follow the playhead, to the overview's
    /// target, or to the last track for a capture.
    fn score_scroll_area(&mut self, ui: &egui::Ui, frame: &ScoreFrame) -> egui::ScrollArea {
        let bar_width = frame.bar_width;
        let visible_width = frame.visible_width;
        let width = frame.width;
        let mut scroll_area = egui::ScrollArea::both()
            .id_salt("arrangement_scroll")
            .scroll_source(egui::scroll_area::ScrollSource {
                drag: false,
                ..Default::default()
            })
            .auto_shrink([false, false])
            .max_height(frame.viewport_height);
        // Only a position or geometry change can move a paused viewport. Manual
        // scrolling stays put until the playhead moves or Follow is enabled again.
        let follow_position = (frame.cursor_tick, bar_width, visible_width);
        if self.score.follow_playhead && self.score.last_follow_position != Some(follow_position) {
            let id = ui.make_persistent_id(egui::Id::new("arrangement_scroll"));
            let offset = egui::scroll_area::State::load(ui.ctx(), id)
                .unwrap_or_default()
                .offset
                .x;
            let playhead = frame.cursor_tick as f32 / TICKS_PER_BAR as f32 * bar_width;
            if let Some(offset) = follow_offset(playhead, offset, visible_width, width) {
                scroll_area = scroll_area.horizontal_scroll_offset(offset);
            }
        }
        self.score.last_follow_position = Some(follow_position);
        // The song overview centres the view on the tick it was clicked at.
        if let Some(tick) = self.overview.target.take() {
            // Whole points, so the song's start is exactly 0.
            let offset = (tick / TICKS_PER_BAR as f32 * bar_width - visible_width / 2.0).round();
            scroll_area = scroll_area
                .horizontal_scroll_offset(offset.clamp(0.0, (width - visible_width).max(0.0)));
        }
        if self.capture.last_track {
            scroll_area = scroll_area.vertical_scroll_offset(frame.track_height);
            self.capture.last_track = false;
        }
        scroll_area
    }

    /// The scrolled content: the lanes, then the lane headers beside them.
    fn score_body(&mut self, ui: &mut egui::Ui, frame: &ScoreFrame, headers: &mut HeaderChanges) {
        let (whole, _) = ui.allocate_exact_size(
            Vec2::new(frame.width + frame.lane_inset, frame.track_height),
            Sense::hover(),
        );
        let rect = Rect::from_min_size(
            whole.min + Vec2::new(frame.lane_inset, 0.0),
            Vec2::new(frame.width, frame.track_height),
        );
        let canvas_clip = ui.clip_rect().intersect(Rect::from_min_max(
            Pos2::new(frame.body_left() + frame.lane_inset, whole.top()),
            Pos2::new(ui.clip_rect().right(), whole.bottom()),
        ));
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect).id_salt("lanes"),
            |ui| self.score_lanes(ui, frame, rect, canvas_clip),
        );
        let labels = Rect::from_min_size(
            Pos2::new(frame.body_left(), whole.top()),
            Vec2::new(frame.label_width, frame.track_height),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(labels)
                .id_salt(("track_labels", self.boards.master_view_generation)),
            |ui| {
                ui.set_clip_rect(labels.intersect(ui.clip_rect()));
                ui.spacing_mut().item_spacing.y = 0.0;
                let first_sounds = lanes::first_sounds(self.session.project());
                for &index in &frame.tracks {
                    self.lane_header(ui, frame, index, first_sounds[index], headers);
                }
            },
        );
        // The columns own horizontal wheel input over the headers. Take
        // it after their controls, before the score's scroll area does.
        if frame.mix && ui.rect_contains_pointer(labels) {
            headers.mix_wheel =
                ui.input_mut(|input| std::mem::take(&mut input.smooth_scroll_delta.x));
        }
    }

    /// The bar ruler above the lanes: the loop, the section tags, and the bar
    /// numbers. A click seeks, a drag sets the loop, and the tags take their gestures.
    fn bar_ruler(
        &mut self,
        ui: &mut egui::Ui,
        frame: &ScoreFrame,
        scroll_right: f32,
        painted_offset_x: f32,
    ) {
        let bar_width = frame.bar_width;
        let ruler_strip = frame.ruler_strip;
        let ruler_clip = Rect::from_min_max(
            ruler_strip.min + Vec2::new(frame.lane_inset, 0.0),
            Pos2::new(scroll_right, ruler_strip.bottom()),
        );
        self.score.ruler_rect = Some(ruler_clip);
        let ruler = Rect::from_min_size(
            ruler_strip.min + Vec2::new(frame.lane_inset - painted_offset_x, 0.0),
            Vec2::new(frame.width, 26.0),
        );
        let ruler_response = name_region(
            ui.interact(
                ruler_clip,
                ui.id().with("bar_ruler"),
                Sense::CLICK | Sense::DRAG,
            ),
            "Bar ruler",
        );
        let ruler_painter = ui.painter().with_clip_rect(ruler_clip);
        ruler_painter.rect_filled(ruler, 0.0, roles().panel);
        if let Some([start, end]) = self.session.project().loop_region {
            let rect = Rect::from_min_max(
                Pos2::new(
                    ruler.left() + start as f32 / TICKS_PER_BAR as f32 * bar_width,
                    ruler.top(),
                ),
                Pos2::new(
                    ruler.left() + end as f32 / TICKS_PER_BAR as f32 * bar_width,
                    ruler.bottom(),
                ),
            );
            ruler_painter.rect_filled(rect, 0.0, roles().accent.gamma_multiply(0.14));
        }
        let tags = if self.sections.bookmarks {
            section_tags(
                ui,
                &ruler_painter,
                ruler,
                bar_width,
                &self.session.project().sections,
            )
        } else {
            Vec::new()
        };
        let first_bar = ((ruler_clip.left() - ruler.left()) / bar_width)
            .floor()
            .max(0.0) as u32;
        let last_bar = ((ruler_clip.right() - ruler.left()) / bar_width)
            .ceil()
            .min(frame.bars as f32) as u32;
        for bar in first_bar..last_bar {
            let x = ruler.left() + bar as f32 * bar_width;
            let number_x = x + bar_width * 0.5;
            if tags
                .iter()
                .any(|(_, tag)| (tag.left() - 8.0..=tag.right() + 8.0).contains(&number_x))
            {
                continue;
            }
            ruler_painter.text(
                Pos2::new(x + bar_width * 0.5, ruler.center().y),
                Align2::CENTER_CENTER,
                format!("{}", bar + 1),
                FontId::proportional(13.0),
                if bar == self.session.cursor_tick() / TICKS_PER_BAR {
                    roles().coral
                } else {
                    roles().text
                },
            );
        }
        if ruler_response.clicked()
            && let Some(pos) = ruler_response.interact_pointer_pos()
        {
            self.seek(self.position_tick(pos.x, ruler, false));
        }
        if ruler_response.drag_started() {
            self.score.loop_start = ui
                .input(|i| i.pointer.press_origin())
                .map(|p| self.position_tick(p.x, ruler, false));
        }
        if ruler_response.drag_stopped()
            && let (Some(start), Some(pos)) = (
                self.score.loop_start.take(),
                ruler_response.interact_pointer_pos(),
            )
        {
            let end = self.position_tick(pos.x, ruler, false);
            let region = [start.min(end), start.max(end).saturating_add(TICKS_PER_BAR)];
            let result = self.session.edit(EditCommand::SetLoop(Some(region)));
            self.apply(result);
        }
        if self.sections.bookmarks {
            self.section_gestures(ui, &ruler_response, ruler, &tags);
        }
    }

    /// Apply a switch click and a lane fader move from the lane headers.
    fn apply_header_changes(&mut self, ui: &egui::Ui, headers: HeaderChanges) {
        if let Some((index, switch, on)) = headers.route {
            let result = self.session.edit(EditCommand::SetSwitch {
                track: index,
                switch,
                on,
            });
            self.apply(result);
            // The mixer is laid out before the arrangement. Redraw it with the
            // new switch state even when the pointer stops after this click.
            ui.ctx().request_repaint();
        }
        if let Some((index, gain)) = headers.gain {
            let mut track = self.session.project().tracks[index].clone();
            track.gain = gain;
            let result = self.session.edit(EditCommand::SetTrack {
                index,
                track: Box::new(track),
            });
            self.apply(result);
            ui.ctx().request_repaint();
        }
    }
}

/// Keep the viewport still while the playhead is comfortably visible.
fn follow_offset(
    playhead: f32,
    offset: f32,
    visible_width: f32,
    content_width: f32,
) -> Option<f32> {
    let margin = (visible_width * 0.1).min(48.0);
    if playhead >= offset + margin && playhead <= offset + visible_width - margin {
        return None;
    }
    let next =
        (playhead - visible_width * 0.25).clamp(0.0, (content_width - visible_width).max(0.0));
    ((next - offset).abs() > 0.5).then_some(next)
}
