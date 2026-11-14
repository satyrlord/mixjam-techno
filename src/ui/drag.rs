//! Lane geometry and drags in the arrangement: where a point falls on a lane
//! and tick, the ghosts that a sound or clip drag shows, and the drop that
//! finishes it. A drag of a clip handle reshapes one clip.
use super::*;

pub(super) enum DragItem {
    Sample(u32),
    Clip { id: u64, offset: u32, copy: bool },
}

/// The part of a clip that a drag reshapes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ClipHandle {
    /// The left edge. The start and the trim move together, so the sound
    /// stays where it plays.
    Start,
    End,
    FadeIn,
    FadeOut,
    /// The top edge between the fade handles. It moves the gain line.
    Gain,
}

impl ClipHandle {
    pub(super) fn cursor(self) -> egui::CursorIcon {
        match self {
            Self::Gain => egui::CursorIcon::ResizeVertical,
            _ => egui::CursorIcon::ResizeHorizontal,
        }
    }
}

/// A drag of a clip handle. The lanes show `preview`; the release makes one edit.
pub(super) struct HandleDrag {
    pub(super) handle: ClipHandle,
    pub(super) origin: Pos2,
    pub(super) clip: Clip,
    pub(super) preview: Clip,
}

/// The pointer drags on the score: a sound or clip, a clip handle, an
/// automation point, and the marquee.
#[derive(Default)]
pub(super) struct Gestures {
    pub(super) drag: Option<DragItem>,
    pub(super) handle_drag: Option<HandleDrag>,
    /// The track (or, on a return lane, the bus) and index of the
    /// automation point being dragged.
    pub(super) automation_drag: Option<(usize, usize)>,
    /// Where the marquee started, and the selection it adds to.
    pub(super) marquee_origin: Option<Pos2>,
    pub(super) marquee_base: BTreeSet<u64>,
}

impl MixJamApp {
    pub(super) fn position_tick(&self, x: f32, rect: Rect, fine: bool) -> u32 {
        let raw = ((x - rect.left()).max(0.0) / self.score.bar_width * TICKS_PER_BAR as f32) as u32;
        let step = if fine {
            TICKS_PER_BAR / 16
        } else {
            TICKS_PER_BAR
        };
        (raw / step) * step
    }

    pub(super) fn timeline_track_at(&self, pos: Pos2) -> Option<usize> {
        let rect = self.score.rect?;
        if pos.y < rect.top() || pos.y >= rect.bottom() {
            return None;
        }
        let row = ((pos.y - rect.top()) / self.score.row_height) as usize;
        self.score.tracks.get(row).copied()
    }

    pub(super) fn clip_rect(&self, clip: &Clip, timeline: Rect) -> Rect {
        self.lane_rect(clip.track, clip.start_tick, clip.length_ticks, timeline)
    }

    /// The rectangle of a clip on `track` at `start_tick`; nothing when the
    /// track has no row.
    pub(super) fn lane_rect(
        &self,
        track: usize,
        start_tick: u32,
        length_ticks: u32,
        timeline: Rect,
    ) -> Rect {
        let Some(row) = self.score.row_by_track.get(track).copied().flatten() else {
            return Rect::NOTHING;
        };
        Rect::from_min_size(
            Pos2::new(
                timeline.left() + start_tick as f32 / TICKS_PER_BAR as f32 * self.score.bar_width,
                timeline.top() + row as f32 * self.score.row_height + 2.0,
            ),
            Vec2::new(
                (length_ticks as f32 / TICKS_PER_BAR as f32 * self.score.bar_width - 1.0).max(3.0),
                self.score.row_height - 4.0,
            ),
        )
    }

    /// How far a clip drag at `pos` moves the selection: tracks and ticks
    /// from the dragged clip. None off the lanes.
    fn clip_drag_delta(&self, pos: Pos2, fine: bool, id: u64, offset: u32) -> Option<(i32, i64)> {
        let rect = self.score.rect?;
        if !self
            .score
            .drop_rect
            .is_some_and(|visible| visible.contains(pos))
        {
            return None;
        }
        let track = self.timeline_track_at(pos)?;
        let anchor = self.session.project().clips.iter().find(|c| c.id == id)?;
        let pointer_tick = self.position_tick(pos.x, rect, true);
        let step = i64::from(if fine {
            TICKS_PER_BAR / 16
        } else {
            TICKS_PER_BAR
        });
        // Snap the distance dragged to the nearest step. Flooring the
        // position would move a clip a whole bar for a slight leftward
        // drift, and shift an off-bar clip on a track change.
        let start = i64::from(anchor.start_tick);
        let dragged = i64::from(pointer_tick) - i64::from(offset) - start;
        let tick_delta = ((dragged + step / 2).div_euclid(step) * step).max(-start);
        Some((track as i32 - anchor.track as i32, tick_delta))
    }

    /// Why a clip drag that moves the selection by `tracks` cannot drop: the
    /// first destination without a row. The lanes show only occupied tracks
    /// and the Add row, so a drop puts every clip on one of them or none.
    fn unshown_destination(&self, tracks: i32) -> Option<String> {
        let track = self
            .session
            .project()
            .clips
            .iter()
            .filter(|clip| self.selection.clips.contains(&clip.id))
            .map(|clip| clip.track as i64 + i64::from(tracks))
            .filter(|&track| {
                usize::try_from(track)
                    .ok()
                    .and_then(|track| self.score.row_by_track.get(track).copied().flatten())
                    .is_none()
            })
            .min()?;
        Some(if (0..TRACK_COUNT as i64).contains(&track) {
            format!("Track {} has no row", track + 1)
        } else {
            "Outside the timeline".into()
        })
    }

    /// Where a drag at `pos` would land: the rectangle and sound of each
    /// clip it places or moves. Empty off the lanes.
    pub(super) fn drop_ghosts(&self, item: &DragItem, pos: Pos2, fine: bool) -> Vec<(Rect, u32)> {
        let Some(rect) = self.score.rect else {
            return Vec::new();
        };
        match *item {
            DragItem::Sample(id) => {
                if !self
                    .score
                    .drop_rect
                    .is_some_and(|visible| visible.contains(pos))
                {
                    return Vec::new();
                }
                let (Some(track), Some(length)) = (
                    self.timeline_track_at(pos),
                    self.session.placement_ticks(id),
                ) else {
                    return Vec::new();
                };
                let tick = self.position_tick(pos.x, rect, fine);
                vec![(self.lane_rect(track, tick, length, rect), id)]
            }
            DragItem::Clip { id, offset, .. } => {
                let Some((tracks, ticks)) = self.clip_drag_delta(pos, fine, id, offset) else {
                    return Vec::new();
                };
                // A drop that would fill a track without a row is canceled, so
                // no clip shows a ghost.
                if self.unshown_destination(tracks).is_some() {
                    return Vec::new();
                }
                self.session
                    .project()
                    .clips
                    .iter()
                    .filter(|clip| self.selection.clips.contains(&clip.id))
                    .filter_map(|clip| {
                        let track = usize::try_from(clip.track as i32 + tracks).ok()?;
                        let start = u32::try_from(i64::from(clip.start_tick) + ticks).ok()?;
                        Some((
                            self.lane_rect(track, start, clip.length_ticks, rect),
                            clip.sample_id,
                        ))
                    })
                    .collect()
            }
        }
    }

    pub(super) fn finish_drag(&mut self, ctx: &egui::Context) {
        if self.gestures.automation_drag.is_some() && !ctx.input(|i| i.pointer.primary_down()) {
            self.gestures.automation_drag = None;
            self.session.end_automation_gesture();
        }
        let Some(drag) = &self.gestures.drag else {
            return;
        };
        let fine = ctx.input(|i| i.modifiers.ctrl);
        if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
            // Over the lanes, a ghost of each clip shows where it lands.
            let ghosts = self.drop_ghosts(drag, pos, fine);
            if let (false, Some(visible)) = (ghosts.is_empty(), self.score.drop_rect) {
                let mut painter = ctx
                    .layer_painter(egui::LayerId::new(
                        egui::Order::Foreground,
                        egui::Id::new("drop_ghosts"),
                    ))
                    .with_clip_rect(visible);
                painter.set_opacity(0.6);
                for (ghost, sample_id) in ghosts {
                    let sample = self.session.library().sample_by_id(sample_id);
                    draw_clip(&painter, ghost, sample, None, true, false, None);
                }
            } else if let DragItem::Sample(id) = drag {
                let name = self
                    .session
                    .library()
                    .sample_by_id(*id)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                drag_label(ctx, pos, &name);
            } else if let DragItem::Clip { id, offset, .. } = *drag
                && let Some((tracks, _)) = self.clip_drag_delta(pos, fine, id, offset)
                && let Some(reason) = self.unshown_destination(tracks)
            {
                // A group that would fill a track without a row names it.
                drag_label(ctx, pos, &reason);
            }
        }
        if !ctx.input(|i| i.pointer.any_released()) {
            return;
        }
        let Some(item) = self.gestures.drag.take() else {
            return;
        };
        let pos = ctx.input(|i| i.pointer.latest_pos());
        if let (Some(pos), Some(rect)) = (pos, self.score.rect)
            && self
                .score
                .drop_rect
                .is_some_and(|visible| visible.contains(pos))
        {
            let Some(track) = self.timeline_track_at(pos) else {
                return;
            };
            match item {
                DragItem::Sample(id) => {
                    self.place(id, track, self.position_tick(pos.x, rect, fine))
                }
                DragItem::Clip { id, offset, copy } => {
                    // Shift or Alt held at the drop copies too, as in the original.
                    let copy = copy || ctx.input(|i| i.modifiers.shift || i.modifiers.alt);
                    let Some((track_delta, tick_delta)) =
                        self.clip_drag_delta(pos, fine, id, offset)
                    else {
                        return;
                    };
                    if let Some(reason) = self.unshown_destination(track_delta) {
                        let action = if copy { "Copy" } else { "Move" };
                        self.message(format!(
                            "{action} canceled. {reason}. Every clip must land on a row."
                        ));
                        return;
                    }
                    let ids: Vec<_> = self.selection.clips.iter().copied().collect();
                    if let Err(error) = self.try_move_clips(&ids, track_delta, tick_delta, copy) {
                        self.error(error);
                    }
                }
            }
            return;
        }
        if let DragItem::Clip { copy, .. } = item {
            let action = if copy { "Copy" } else { "Move" };
            let destination = if self.score.add_track.is_some() {
                "a track or the Add row"
            } else {
                "a track"
            };
            self.message(format!("{action} canceled. Drop on {destination}."));
        }
    }
}

/// The label that follows the pointer during a drag.
pub(super) fn drag_label(ctx: &egui::Context, pos: Pos2, label: &str) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("drag_label"),
    ));
    let rect = Rect::from_min_size(pos + Vec2::new(14.0, 14.0), Vec2::new(150.0, 30.0));
    painter.rect_filled(rect, 4.0, roles().drag_label);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        roles().on_light,
    );
}

/// Tracks with clips, and soloed tracks: a solo silences the other tracks, so
/// its switch must stay visible even on an empty track.
/// Several views ask for it each frame, so it is one pass with a flag per
/// track, not a sorted set.
pub(super) fn occupied_tracks(project: &Project) -> Vec<usize> {
    let mut occupied: Vec<bool> = project.tracks.iter().map(|track| track.solo).collect();
    for clip in &project.clips {
        if let Some(flag) = occupied.get_mut(clip.track) {
            *flag = true;
        }
    }
    (0..occupied.len())
        .filter(|&track| occupied[track])
        .collect()
}
