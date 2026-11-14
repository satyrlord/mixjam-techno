//! Automation lanes and FX return lanes in the arrangement: their points,
//! their line, and the gestures that add, move, and delete a point.
use super::*;
use mixjam_techno::application::MAX_TICKS;

impl MixJamApp {
    /// Draw FX bus `bus`'s return lane across the visible rows and edit its
    /// points with the pointer, as a track's lane is edited. The lane runs
    /// from silence at the bottom to `RETURN_CEILING` at the top; a dashed
    /// line marks the full return, 100%.
    pub(super) fn return_lane(
        &mut self,
        ui: &egui::Ui,
        response: &Response,
        painter: &egui::Painter,
        rect: Rect,
        visible: Rect,
        bus: usize,
    ) {
        painter.rect_filled(rect, 0.0, roles().canvas.gamma_multiply(0.45));
        let row = return_row(rect, visible);
        let bar_width = self.score.bar_width;
        let x = |tick: u32| rect.left() + tick as f32 / TICKS_PER_BAR as f32 * bar_width;
        let points = self.session.project().return_automation[bus].clone();
        let knob = self.session.project().fx.buses[bus].return_level;
        painter.text(
            Pos2::new(visible.left() + 8.0, visible.top() + 4.0),
            Align2::LEFT_TOP,
            format!("{} (100% dashed)", return_lane_label(bus)),
            FontId::proportional(12.0),
            roles().muted,
        );
        let full = return_y(1.0, row);
        painter.add(egui::Shape::dashed_line(
            &[Pos2::new(rect.left(), full), Pos2::new(rect.right(), full)],
            Stroke::new(1.0_f32, roles().line),
            3.0,
            6.0,
        ));
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let mut hit = None;
        if points.is_empty() {
            // The Return knob, which the bus keeps without points.
            let y = return_y(knob, row);
            painter.add(egui::Shape::dashed_line(
                &[Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(1.0_f32, roles().muted.gamma_multiply(0.6)),
                6.0,
                5.0,
            ));
        } else {
            let markers: Vec<Pos2> = points
                .iter()
                .map(|point| Pos2::new(x(point.tick), return_y(point.value, row)))
                .collect();
            let mut line = vec![Pos2::new(rect.left(), markers[0].y)];
            line.extend(markers.iter().copied());
            line.push(Pos2::new(rect.right(), markers[markers.len() - 1].y));
            painter.add(egui::Shape::line(
                line,
                Stroke::new(1.6_f32, roles().accent),
            ));
            for (index, point) in markers.iter().enumerate() {
                let active = self.gestures.automation_drag == Some((bus, index));
                let hovered = pointer.is_some_and(|pos| point.distance(pos) <= AUTOMATION_REACH);
                if hovered {
                    hit = Some(index);
                }
                painter.circle(
                    *point,
                    if active || hovered { 4.5 } else { 3.5 },
                    if active { roles().text } else { roles().accent },
                    Stroke::new(1.0_f32, roles().canvas),
                );
            }
        }
        if let Some(index) = self
            .gestures
            .automation_drag
            .map(|(_, index)| index)
            .or(hit)
            && let Some(point) = points.get(index)
        {
            response.clone().hover_hint_at_pointer(format!(
                "{}, bar {:.2}: {}",
                return_lane_label(bus),
                f64::from(point.tick) / f64::from(TICKS_PER_BAR) + 1.0,
                automation::return_label(point.value)
            ));
        }
        let free = ui.input(|i| i.modifiers.ctrl);
        let hit_at = |pos: Pos2, points: &[automation::Point]| {
            points.iter().position(|point| {
                Pos2::new(x(point.tick), return_y(point.value, row)).distance(pos)
                    <= AUTOMATION_REACH
            })
        };
        // A click adds a point; a drag from the lane adds one and moves it.
        if response.drag_started()
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            match self.session.begin_return_gesture(bus) {
                Ok(()) => {
                    self.gestures.automation_drag = hit_at(origin, &points)
                        .or_else(|| self.return_add(origin, rect, row, bus, free))
                        .map(|index| (bus, index));
                    if self.gestures.automation_drag.is_none() {
                        self.session.end_automation_gesture();
                    }
                }
                Err(error) => self.error(error),
            }
        }
        if let Some((_, index)) = self.gestures.automation_drag {
            if response.dragged()
                && let Some(pos) = pointer
            {
                let mut points = self.session.project().return_automation[bus].clone();
                if index < points.len() {
                    let low = index
                        .checked_sub(1)
                        .map_or(0, |before| points[before].tick + 1);
                    let high = points
                        .get(index + 1)
                        .map_or(MAX_TICKS, |after| after.tick - 1);
                    let tick = self.automation_tick(pos.x, rect, free);
                    let point = automation::Point {
                        tick: if low <= high {
                            tick.clamp(low, high)
                        } else {
                            points[index].tick
                        },
                        value: return_value(pos.y, row),
                    };
                    if points[index] != point {
                        points[index] = point;
                        let result = self
                            .session
                            .edit(EditCommand::SetReturnAutomation { bus, points });
                        self.apply(result);
                    }
                }
            }
            if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
                self.gestures.automation_drag = None;
                self.session.end_automation_gesture();
            }
        }
        if let Some(pos) = response.interact_pointer_pos() {
            let points = self.session.project().return_automation[bus].clone();
            let hit = hit_at(pos, &points);
            if response.double_clicked() || response.secondary_clicked() {
                if let Some(index) = hit {
                    let mut points = points;
                    points.remove(index);
                    let result = self
                        .session
                        .edit(EditCommand::SetReturnAutomation { bus, points });
                    self.apply(result);
                }
            } else if response.clicked() && hit.is_none() {
                self.return_add(pos, rect, row, bus, free);
            }
        }
    }

    /// Add a point to FX bus `bus`'s return lane where `pos` is, or set the
    /// value of the point already at its tick. Returns the point's index.
    fn return_add(
        &mut self,
        pos: Pos2,
        rect: Rect,
        row: Rect,
        bus: usize,
        free: bool,
    ) -> Option<usize> {
        if !row.y_range().contains(pos.y) {
            return None;
        }
        let tick = self.automation_tick(pos.x, rect, free);
        let value = return_value(pos.y, row);
        let mut points = self.session.project().return_automation[bus].clone();
        let index = points.partition_point(|point| point.tick < tick);
        if points.get(index).is_some_and(|point| point.tick == tick) {
            points[index].value = value;
        } else {
            points.insert(index, automation::Point { tick, value });
        }
        let result = self
            .session
            .edit(EditCommand::SetReturnAutomation { bus, points });
        let added = result.is_ok();
        self.apply(result);
        added.then_some(index)
    }

    /// The lane's row rectangle for a track the arrangement shows.
    fn automation_row(&self, track: usize, rect: Rect) -> Option<Rect> {
        let row = self.score.row_by_track[track]?;
        Some(Rect::from_min_size(
            Pos2::new(rect.left(), rect.top() + row as f32 * self.score.row_height),
            Vec2::new(rect.width(), self.score.row_height),
        ))
    }

    fn automation_tick(&self, x: f32, rect: Rect, free: bool) -> u32 {
        let raw = (x - rect.left()).max(0.0) / self.score.bar_width * TICKS_PER_BAR as f32;
        let step = if free {
            1.0
        } else {
            (TICKS_PER_BAR / 16) as f32
        };
        ((raw / step).round() * step) as u32
    }

    /// Draw `lane` on each row and edit its points with the pointer.
    pub(super) fn automation_lanes(
        &mut self,
        ui: &egui::Ui,
        response: &Response,
        painter: &egui::Painter,
        rect: Rect,
        lane: automation::Lane,
        tracks: &[usize],
    ) {
        // Dim the clips so the lines stand out.
        painter.rect_filled(rect, 0.0, roles().canvas.gamma_multiply(0.45));
        let project = self.session.project();
        let mut hit = None;
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let near = |a: Pos2, b: Pos2| a.distance(b) <= AUTOMATION_REACH;
        for &track in tracks {
            let Some(row) = self.automation_row(track, rect) else {
                continue;
            };
            let points = project.tracks[track].automation.lane(lane);
            let x =
                |tick: u32| rect.left() + tick as f32 / TICKS_PER_BAR as f32 * self.score.bar_width;
            if points.is_empty() {
                // The value the track keeps without points.
                let value = match lane {
                    automation::Lane::Send(bus) => project.tracks[track].sends[bus],
                    automation::Lane::Pan => project.tracks[track].pan,
                    _ => lane.before_first(),
                };
                let y = automation_y(lane, value, row);
                painter.add(egui::Shape::dashed_line(
                    &[Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(1.0_f32, roles().muted.gamma_multiply(0.6)),
                    6.0,
                    5.0,
                ));
                continue;
            }
            let markers: Vec<Pos2> = points
                .iter()
                .map(|point| Pos2::new(x(point.tick), automation_y(lane, point.value, row)))
                .collect();
            // A stepped lane holds its resting value before its first point
            // (Repeat off, the gate at 100%) and each value until the next
            // point; the Pitch lane rests at 0 before its first point and
            // then moves in straight lines; the others hold their first
            // value and move in straight lines.
            let mut line = vec![Pos2::new(
                rect.left(),
                if lane.rests_before_first() {
                    automation_y(lane, lane.before_first(), row)
                } else {
                    markers[0].y
                },
            )];
            for (index, marker) in markers.iter().enumerate() {
                if lane.steps() || (index == 0 && lane.rests_before_first()) {
                    line.push(Pos2::new(marker.x, line[line.len() - 1].y));
                }
                line.push(*marker);
            }
            line.push(Pos2::new(rect.right(), markers[markers.len() - 1].y));
            painter.add(egui::Shape::line(
                line,
                Stroke::new(1.6_f32, roles().accent),
            ));
            for (index, point) in markers.iter().enumerate() {
                let active = self.gestures.automation_drag == Some((track, index));
                let hovered = pointer.is_some_and(|pos| near(*point, pos));
                if hovered {
                    hit = Some((track, index));
                }
                painter.circle(
                    *point,
                    if active || hovered { 4.5 } else { 3.5 },
                    if active { roles().text } else { roles().accent },
                    Stroke::new(1.0_f32, roles().canvas),
                );
            }
        }
        if let Some((track, index)) = self.gestures.automation_drag.or(hit)
            && let Some(point) = project.tracks[track].automation.lane(lane).get(index)
        {
            response.clone().hover_hint_at_pointer(format!(
                "Track {}, bar {:.2}: {}",
                track + 1,
                f64::from(point.tick) / f64::from(TICKS_PER_BAR) + 1.0,
                lane.format(point.value)
            ));
        }
        let free = ui.input(|i| i.modifiers.ctrl);
        // A click on a lane adds a point; a drag from a lane adds one and moves it.
        if response.drag_started()
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            let hit = self.automation_hit(origin, rect, lane, tracks);
            let track = hit.map(|(track, _)| track).or_else(|| {
                self.timeline_track_at(origin)
                    .filter(|track| tracks.contains(track))
            });
            if let Some(track) = track {
                match self.session.begin_automation_gesture(track, lane) {
                    Ok(()) => {
                        self.gestures.automation_drag =
                            hit.or_else(|| self.automation_add(origin, rect, lane, tracks, free));
                        if self.gestures.automation_drag.is_none() {
                            self.session.end_automation_gesture();
                        }
                    }
                    Err(error) => self.error(error),
                }
            }
        }
        if let Some((track, index)) = self.gestures.automation_drag {
            if response.dragged()
                && let Some(pos) = pointer
                && let Some(row) = self.automation_row(track, rect)
            {
                let mut points = self.session.project().tracks[track]
                    .automation
                    .lane(lane)
                    .to_vec();
                if index < points.len() {
                    let low = index
                        .checked_sub(1)
                        .map_or(0, |before| points[before].tick + 1);
                    let high = points
                        .get(index + 1)
                        .map_or(MAX_TICKS, |after| after.tick - 1);
                    let tick = self.automation_tick(pos.x, rect, free);
                    let point = automation::Point {
                        tick: if low <= high {
                            tick.clamp(low, high)
                        } else {
                            points[index].tick
                        },
                        value: automation_value(lane, pos.y, row),
                    };
                    if points[index] != point {
                        points[index] = point;
                        let result = self.session.edit(EditCommand::SetAutomation {
                            track,
                            lane,
                            points,
                        });
                        self.apply(result);
                    }
                }
            }
            if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
                self.gestures.automation_drag = None;
                self.session.end_automation_gesture();
            }
        }
        if let Some(pos) = response.interact_pointer_pos() {
            let hit = self.automation_hit(pos, rect, lane, tracks);
            if response.double_clicked() || response.secondary_clicked() {
                if let Some((track, index)) = hit {
                    let mut points = self.session.project().tracks[track]
                        .automation
                        .lane(lane)
                        .to_vec();
                    points.remove(index);
                    let result = self.session.edit(EditCommand::SetAutomation {
                        track,
                        lane,
                        points,
                    });
                    self.apply(result);
                }
            } else if response.clicked() && hit.is_none() {
                self.automation_add(pos, rect, lane, tracks, free);
            }
        }
    }

    /// The point of `lane` within reach of `pos`, as its track and index.
    fn automation_hit(
        &self,
        pos: Pos2,
        rect: Rect,
        lane: automation::Lane,
        tracks: &[usize],
    ) -> Option<(usize, usize)> {
        tracks.iter().find_map(|&track| {
            let row = self.automation_row(track, rect)?;
            self.session.project().tracks[track]
                .automation
                .lane(lane)
                .iter()
                .position(|point| {
                    let at = Pos2::new(
                        rect.left()
                            + point.tick as f32 / TICKS_PER_BAR as f32 * self.score.bar_width,
                        automation_y(lane, point.value, row),
                    );
                    at.distance(pos) <= AUTOMATION_REACH
                })
                .map(|index| (track, index))
        })
    }

    /// Add a point of `lane` where `pos` is, or set the value of the point
    /// already at its tick. Returns the point's track and index.
    fn automation_add(
        &mut self,
        pos: Pos2,
        rect: Rect,
        lane: automation::Lane,
        tracks: &[usize],
        free: bool,
    ) -> Option<(usize, usize)> {
        let track = self
            .timeline_track_at(pos)
            .filter(|track| tracks.contains(track))?;
        let row = self.automation_row(track, rect)?;
        let tick = self.automation_tick(pos.x, rect, free);
        let value = automation_value(lane, pos.y, row);
        let mut points = self.session.project().tracks[track]
            .automation
            .lane(lane)
            .to_vec();
        let index = points.partition_point(|point| point.tick < tick);
        if points.get(index).is_some_and(|point| point.tick == tick) {
            points[index].value = value;
        } else {
            points.insert(index, automation::Point { tick, value });
        }
        let result = self.session.edit(EditCommand::SetAutomation {
            track,
            lane,
            points,
        });
        let added = result.is_ok();
        self.apply(result);
        added.then_some((track, index))
    }
}

/// How far from a point, in points, the pointer still takes it.
const AUTOMATION_REACH: f32 = 7.0;

/// Where a lane value sits in a track row, 4 points in from its edges. The
/// Repeat lane's values sit at even heights, from Off at the bottom to 1/64
/// at the top.
fn automation_y(lane: automation::Lane, value: f32, row: Rect) -> f32 {
    let inner = row.shrink2(Vec2::new(0.0, 4.0));
    let along = if lane == automation::Lane::Repeat {
        let steps = &automation::REPEAT_STEPS;
        let step = steps.iter().position(|step| *step >= value).unwrap_or(0);
        step as f32 / (steps.len() - 1) as f32
    } else {
        let (low, high) = lane.range();
        (value - low) / (high - low)
    };
    inner.bottom() - along * inner.height()
}

/// The lane value at a height in a track row, rounded to the lane's steps.
fn automation_value(lane: automation::Lane, y: f32, row: Rect) -> f32 {
    let (low, high) = lane.range();
    let inner = row.shrink2(Vec2::new(0.0, 4.0));
    let along = (inner.bottom() - y) / inner.height();
    let value = low + along * (high - low);
    let value = match lane {
        automation::Lane::Volume => (value * 2.0).round() / 2.0,
        automation::Lane::Send(_) => (value * 100.0).round() / 100.0,
        // The middle opens the filter, or centers the pan.
        automation::Lane::Filter | automation::Lane::Pan if value.abs() < 0.04 => 0.0,
        automation::Lane::Filter | automation::Lane::Pan => (value * 100.0).round() / 100.0,
        // The nearest of the evenly spaced steps.
        automation::Lane::Repeat => {
            let steps = &automation::REPEAT_STEPS;
            let last = (steps.len() - 1) as f32;
            steps[(along * last).round().clamp(0.0, last) as usize]
        }
        // Whole steps of 5%.
        automation::Lane::RepeatGate => {
            (value / automation::REPEAT_GATE_STEP).round() * automation::REPEAT_GATE_STEP
        }
        // Whole semitones.
        automation::Lane::Pitch => (value / 100.0).round() * 100.0,
    };
    value.clamp(low, high)
}

/// The name of FX bus `bus`'s return lane in the lane menu: "FX 1 return".
pub(in super::super) fn return_lane_label(bus: usize) -> String {
    format!("FX {} return", bus + 1)
}

/// The return lane's row: the visible rows of the arrangement, below its
/// name and 4 points above the bottom.
pub(in super::super) fn return_row(rect: Rect, visible: Rect) -> Rect {
    let top = visible.top() + 22.0;
    let bottom = (visible.bottom() - 4.0).max(top + 8.0);
    Rect::from_x_y_ranges(rect.x_range(), top..=bottom)
}

/// Where a return lane value sits in its row.
pub(in super::super) fn return_y(value: f32, row: Rect) -> f32 {
    row.bottom() - value / automation::RETURN_CEILING * row.height()
}

/// The return lane value at a height in its row, in whole percent. The
/// full return, 100%, holds the pointer near it.
fn return_value(y: f32, row: Rect) -> f32 {
    let value = (row.bottom() - y) / row.height() * automation::RETURN_CEILING;
    let value = if (value - 1.0).abs() < 0.04 {
        1.0
    } else {
        (value * 100.0).round() / 100.0
    };
    value.clamp(0.0, automation::RETURN_CEILING)
}
