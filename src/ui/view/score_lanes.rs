//! The score's lanes: the grid, the clips with their waveforms or notes, the
//! overlays and the playhead, and the clicks, menus, and drags on the lanes.
use super::timeline::ScoreFrame;
use super::*;
use mixjam_techno::application::PeakState;
use std::ops::ControlFlow;

impl MixJamApp {
    /// Draw the lanes and handle their input for one frame.
    pub(super) fn score_lanes(
        &mut self,
        ui: &mut egui::Ui,
        frame: &ScoreFrame,
        rect: Rect,
        canvas_clip: Rect,
    ) {
        ui.set_clip_rect(canvas_clip);
        let response = self.lanes_response(ui, rect);
        let painter = ui.painter_at(rect).with_clip_rect(canvas_clip);
        let visible = ui.clip_rect().intersect(rect);
        paint_lane_grid(&painter, frame, rect, canvas_clip, visible);
        let shows_handles = self.clip_handles(ui, &response, rect);
        self.paint_clips(&painter, rect, visible, frame.bar_width, shows_handles);
        self.paint_lane_overlays(ui, &response, &painter, frame, rect, visible);
        if let Some(lane) = self.score_tools.automation_lane {
            let rows = &frame.tracks[..frame.occupied_count];
            self.automation_lanes(ui, &response, &painter, rect, lane, rows);
            return;
        }
        if let Some(bus) = self.score_tools.return_lane {
            self.return_lane(ui, &response, &painter, rect, visible, bus);
            return;
        }
        if self.lane_clicks(ui, &response, rect).is_break() {
            return;
        }
        self.clip_context_menu(&response, rect);
        self.lane_drags(ui, &response, &painter, rect);
    }

    /// The lanes' one widget: its keys, its clip menu key, and the tooltip of a
    /// clip whose sound is missing.
    fn lanes_response(&mut self, ui: &mut egui::Ui, rect: Rect) -> Response {
        let response = name_control(
            ui.interact(rect, ui.id().with("canvas"), Sense::click_and_drag()),
            "Arrangement lanes",
        );
        if response.has_focus() {
            self.lane_keys(ui, &response, rect);
        }
        if std::mem::take(&mut self.clip_menu.by_key) {
            self.open_clip_menu(ui.ctx(), &response, rect);
        }
        self.score.rect = Some(rect);
        // A clip paints its own name, so only a missing sound has a
        // tooltip. The canvas is one widget, so it follows the pointer.
        if response.hovered()
            && let Some(pos) = ui.input(|i| i.pointer.latest_pos())
            && let Some(clip) = self
                .session
                .project()
                .clips
                .iter()
                .rev()
                .find(|clip| self.clip_rect(clip, rect).contains(pos))
            && let Some(missing) = self.session.missing_sample(clip.sample_id)
        {
            response.clone().hover_hint_at_pointer(format!(
                "{}\n{}\nRight-click to resolve. Import details has copyable \
                 metadata.",
                missing_sample_details(missing),
                missing_clip_details(clip)
            ));
        }
        self.score.drop_rect = Some(rect.intersect(ui.clip_rect()));
        response
    }

    /// Run a clip handle drag, set the pointer's cursor over a handle, and
    /// return the clip that shows its fade handles: the reshaped or hovered one.
    fn clip_handles(&mut self, ui: &egui::Ui, response: &Response, rect: Rect) -> Option<u64> {
        let pointer = ui.input(|i| i.pointer.latest_pos());
        // Clip handles work while the lanes edit clips.
        let handles = !self.automating() && !self.score_tools.erase;
        if handles {
            self.handle_drag_step(ui, response, rect);
        } else {
            self.gestures.handle_drag = None;
        }
        let hovered_handle = pointer
            .filter(|_| handles && response.hovered() && self.gestures.drag.is_none())
            .and_then(|pos| self.clip_handle_at(pos, rect));
        if let Some(handle) = self
            .gestures
            .handle_drag
            .as_ref()
            .map(|drag| drag.handle)
            .or(hovered_handle.map(|(_, handle)| handle))
        {
            ui.ctx().set_cursor_icon(handle.cursor());
        }
        let preview = self.gestures.handle_drag.as_ref().map(|drag| &drag.preview);
        preview.map(|clip| clip.id).or_else(|| {
            pointer
                .filter(|_| handles && response.hovered())
                .and_then(|pos| {
                    self.session
                        .project()
                        .clips
                        .iter()
                        .rev()
                        .find(|clip| self.clip_rect(clip, rect).contains(pos))
                        .map(|clip| clip.id)
                })
        })
    }

    /// Paint the clips on screen, each with its waveform or notes, its fades,
    /// and its gain line and handles when it shows them.
    fn paint_clips(
        &mut self,
        painter: &egui::Painter,
        rect: Rect,
        visible: Rect,
        bar_width: f32,
        shows_handles: Option<u64>,
    ) {
        let tick_width = bar_width / TICKS_PER_BAR as f32;
        self.load_step_patterns();
        let preview = self.gestures.handle_drag.as_ref().map(|drag| &drag.preview);
        for clip in &self.session.project().clips {
            let clip = preview.filter(|p| p.id == clip.id).unwrap_or(clip);
            let clip_rect = self.clip_rect(clip, rect);
            if visible.intersects(clip_rect) {
                let sample = self.session.library().sample_by_id(clip.sample_id);
                let missing = self.session.missing_sample(clip.sample_id);
                let bpm = sample.map_or(self.session.project().bpm, |sample| {
                    self.session
                        .project()
                        .sample_playback_bpm(sample.id, sample.source_bpm)
                });
                // A Step synth sound saved with its pattern draws notes.
                let notes = sample.filter(|_| missing.is_none()).and_then(|sample| {
                    let pattern = self
                        .clip_paint
                        .step_patterns
                        .get(&sample.id)
                        .cloned()
                        .flatten()?;
                    let frames_per_step = f64::from(sample.sample_rate) * 60.0 / pattern.bpm / 4.0;
                    Some(waveform::ClipNotes::new(
                        clip,
                        &pattern,
                        frames_per_step,
                        frames_per_tick(sample, bpm),
                        f64::from(sample.frame_count),
                    ))
                });
                // Only clips on screen ask for their sound's peaks.
                let wave = match (sample, missing) {
                    _ if notes.is_some() => None,
                    (Some(sample), None) => clip_wave(
                        &mut self.clip_paint.waves,
                        &self.session,
                        clip,
                        sample,
                        tick_width,
                        bpm,
                    ),
                    _ => None,
                };
                draw_clip(
                    painter,
                    clip_rect,
                    sample,
                    missing,
                    self.selection.clips.contains(&clip.id),
                    clip.reverse,
                    match &notes {
                        Some(notes) => Some(waveform::ClipBody::Notes(notes, tick_width)),
                        None => wave.map(waveform::ClipBody::Wave),
                    },
                );
                draw_fades(painter, clip_rect, clip, sample, bar_width, bpm);
                let handles = shows_handles == Some(clip.id);
                if handles || self.selection.clips.contains(&clip.id) {
                    draw_gain_line(painter, clip_rect, clip.gain);
                }
                if handles && let Some(sample) = sample {
                    draw_fade_handles(painter, clip_rect, clip, sample, bar_width, bpm);
                    draw_gain_handle(painter, clip_rect, visible, clip.gain);
                }
            }
        }
        // Forget the waveforms of deleted clips.
        if self.clip_paint.waves.len() > self.session.project().clips.len() {
            let ids: BTreeSet<u64> = self
                .session
                .project()
                .clips
                .iter()
                .map(|clip| clip.id)
                .collect();
            self.clip_paint.waves.retain(|id, _| ids.contains(id));
        }
    }

    /// Paint over the clips: the dimmed silenced lanes, where an incoming sound
    /// will land, the focus outline, the section lines, and the playhead.
    fn paint_lane_overlays(
        &self,
        ui: &egui::Ui,
        response: &Response,
        painter: &egui::Painter,
        frame: &ScoreFrame,
        rect: Rect,
        visible: Rect,
    ) {
        let bar_width = frame.bar_width;
        let row_height = frame.row_height;
        // Dim the lanes that Mute or another track's solo silences.
        let project = self.session.project();
        let any_solo = project.any_solo();
        for (row, &track) in frame.tracks.iter().enumerate().take(frame.occupied_count) {
            if !project.tracks[track].audible(any_solo) {
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(rect.left(), rect.top() + row as f32 * row_height),
                        Vec2::new(frame.width, row_height),
                    ),
                    0.0,
                    roles().canvas.gamma_multiply(0.6),
                );
            }
        }
        // Show where a dragged or click-placed sound will land.
        let incoming = match &self.gestures.drag {
            Some(DragItem::Sample(id)) => Some(*id),
            Some(DragItem::Clip { .. }) => None,
            None => self.selection.sample.filter(|_| {
                self.selection.place_on_click
                    && !self.score_tools.erase
                    && !self.automating()
                    && response.hovered()
            }),
        };
        if let Some(sample) = incoming.and_then(|id| self.session.library().sample_by_id(id))
            && let Some(pos) = ui.input(|i| i.pointer.latest_pos())
            && visible.contains(pos)
            && let Some(track) = self.timeline_track_at(pos)
            && let Some(length_ticks) = self.session.placement_ticks(sample.id)
            && !self
                .session
                .project()
                .clips
                .iter()
                .any(|clip| self.clip_rect(clip, rect).contains(pos))
        {
            let fine = ui.input(|i| i.modifiers.ctrl);
            let ghost = self.clip_rect(
                &Clip {
                    id: 0,
                    sample_id: sample.id,
                    track,
                    start_tick: self.position_tick(pos.x, rect, fine),
                    length_ticks,
                    offset_frames: 0,
                    gain: 1.0,
                    fades: Default::default(),
                    pitch_cents: 0,
                    reverse: false,
                },
                rect,
            );
            painter.rect_filled(
                ghost,
                3.0,
                group_color(&sample.group_name).gamma_multiply(0.3),
            );
            painter.rect_stroke(
                ghost.shrink(0.5),
                3.0,
                Stroke::new(1.0_f32, roles().text),
                egui::StrokeKind::Inside,
            );
        }
        if response.has_focus() {
            painter.rect_stroke(
                visible.shrink(1.0),
                0.0,
                Stroke::new(1.0_f32, roles().accent),
                egui::StrokeKind::Inside,
            );
        }
        if self.sections.bookmarks {
            for (index, section) in self.session.project().sections.iter().enumerate() {
                let x = rect.left() + section.start_tick as f32 / TICKS_PER_BAR as f32 * bar_width;
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(1.5_f32, section_color(index).gamma_multiply(0.8)),
                );
            }
        }
        let play_x =
            rect.left() + self.session.cursor_tick() as f32 / TICKS_PER_BAR as f32 * bar_width;
        painter.line_segment(
            [
                Pos2::new(play_x, rect.top()),
                Pos2::new(play_x, rect.bottom()),
            ],
            Stroke::new(1.7_f32, roles().accent),
        );
    }

    /// A click on a clip selects, previews, or erases it, and a click on an
    /// empty lane places the selected sound or clears the selection. A
    /// placement outside every track ends the frame's lane input.
    fn lane_clicks(&mut self, ui: &egui::Ui, response: &Response, rect: Rect) -> ControlFlow<()> {
        if (response.clicked() || response.double_clicked())
            && let Some(pos) = response.interact_pointer_pos()
        {
            let hit = self
                .session
                .project()
                .clips
                .iter()
                .rev()
                .find(|c| self.clip_rect(c, rect).contains(pos))
                .cloned();
            if let Some(clip) = hit {
                if self.score_tools.erase {
                    let result = self.session.delete(clip.id);
                    // The erased clip may be part of the selection.
                    self.reconcile_selection();
                    self.apply(result);
                } else {
                    self.select_clip(clip.id, ui.input(|i| i.modifiers.shift));
                    self.selection.place_on_click = false;
                    if response.double_clicked() {
                        self.preview(clip.sample_id);
                    }
                }
            } else if let Some(sample) = self
                .selection
                .sample
                .filter(|_| self.selection.place_on_click && !ui.input(|i| i.modifiers.shift))
            {
                let Some(track) = self.timeline_track_at(pos) else {
                    return ControlFlow::Break(());
                };
                let tick = self.position_tick(pos.x, rect, ui.input(|i| i.modifiers.ctrl));
                self.place(sample, track, tick);
            } else if !ui.input(|i| i.modifiers.shift) {
                self.selection.clip = None;
                self.selection.clips.clear();
                self.selection.track = None;
            }
        }
        ControlFlow::Continue(())
    }

    /// A right-click picks the clip under the pointer, and the clip menu opens
    /// there or at the selected clip.
    fn clip_context_menu(&mut self, response: &Response, rect: Rect) {
        if response.secondary_clicked() {
            let hit = response.interact_pointer_pos().and_then(|pos| {
                self.session
                    .project()
                    .clips
                    .iter()
                    .rev()
                    .find(|clip| self.clip_rect(clip, rect).contains(pos))
                    .map(|clip| clip.id)
            });
            self.clip_menu.clip = hit;
            if let Some(pos) = response.interact_pointer_pos() {
                self.clip_menu.anchor = pos;
            }
            if let Some(id) = hit.filter(|id| !self.selection.clips.contains(id)) {
                self.select_clip(id, false);
                self.selection.place_on_click = false;
            }
            self.clip_menu.missing_sample = response.interact_pointer_pos().and_then(|pos| {
                self.session
                    .project()
                    .clips
                    .iter()
                    .rev()
                    .find(|clip| self.clip_rect(clip, rect).contains(pos))
                    .and_then(|clip| self.session.missing_sample(clip.sample_id))
                    .map(|missing| missing.id)
            });
        }
        if self.clip_menu.clip.is_some() || self.clip_menu.missing_sample.is_some() {
            // A context menu that Shift+F10 can also open at the
            // selected clip. Clicks inside it edit the clip's values.
            let open = if response.secondary_clicked() {
                Some(egui::SetOpenCommand::Bool(true))
            } else if response.clicked() {
                Some(egui::SetOpenCommand::Bool(false))
            } else {
                None
            };
            egui::Popup::menu(response)
                .open_memory(open)
                .at_position(self.clip_menu.anchor)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    if let Some(sample_id) = self.clip_menu.missing_sample
                        && ui.button("Resolve manually").clicked()
                    {
                        ui.close();
                        self.resolve_missing_sample(sample_id);
                    }
                    if let Some(id) = self.clip_menu.clip {
                        if ui.button("Preview").clicked() {
                            ui.close();
                            // The menu's clip need not be the primary one.
                            self.preview_clip(id);
                        }
                        if ui.button("Duplicate (Ctrl+D)").clicked() {
                            ui.close();
                            self.duplicate_selection();
                        }
                        if ui.button("Delete (Delete)").clicked() {
                            ui.close();
                            self.delete_selection();
                        }
                        self.clip_properties(ui, id, "Clip");
                    }
                });
        }
    }

    /// A drag that starts on a clip moves or copies the selection; one that
    /// starts on an empty lane draws a selection rectangle.
    fn lane_drags(
        &mut self,
        ui: &egui::Ui,
        response: &Response,
        painter: &egui::Painter,
        rect: Rect,
    ) {
        if response.drag_started()
            && self.gestures.handle_drag.is_none()
            && let Some(pos) = ui.input(|i| i.pointer.press_origin())
        {
            if let Some(clip) = self
                .session
                .project()
                .clips
                .iter()
                .rev()
                .find(|c| self.clip_rect(c, rect).contains(pos))
                .cloned()
            {
                let offset = self
                    .position_tick(pos.x, rect, true)
                    .saturating_sub(clip.start_tick);
                if !self.selection.clips.contains(&clip.id) {
                    self.select_clip(clip.id, false);
                }
                self.gestures.drag = Some(DragItem::Clip {
                    id: clip.id,
                    offset,
                    copy: ui.input(|i| {
                        i.modifiers.alt || i.modifiers.shift || i.pointer.secondary_down()
                    }),
                });
            } else {
                self.gestures.marquee_origin = Some(pos);
                self.gestures.marquee_base = if ui.input(|i| i.modifiers.shift) {
                    self.selection.clips.clone()
                } else {
                    BTreeSet::new()
                };
            }
        }
        if let Some(origin) = self.gestures.marquee_origin {
            if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                let selection = Rect::from_two_pos(origin, pos).intersect(rect);
                painter.rect_filled(selection, 0.0, roles().text.gamma_multiply(0.12));
                outline(painter, selection, roles().text, 1.0);
                self.selection.clips = self.gestures.marquee_base.clone();
                let hits: Vec<_> = self
                    .session
                    .project()
                    .clips
                    .iter()
                    .filter(|c| self.clip_rect(c, rect).intersects(selection))
                    .map(|c| c.id)
                    .collect();
                self.selection.clips.extend(hits);
                self.selection.clip = self.selection.clips.iter().next().copied();
            }
            if response.drag_stopped() || ui.input(|i| i.pointer.any_released()) {
                self.gestures.marquee_origin = None;
            }
        }
    }
}

/// Paint the lane rows, the bar lines, and the hint on the empty add row.
fn paint_lane_grid(
    painter: &egui::Painter,
    frame: &ScoreFrame,
    rect: Rect,
    canvas_clip: Rect,
    visible: Rect,
) {
    let (width, row_height, bar_width) = (frame.width, frame.row_height, frame.bar_width);
    painter.rect_filled(rect, 0.0, roles().canvas);
    for track in 0..frame.occupied_count {
        let row = Rect::from_min_size(
            Pos2::new(rect.left(), rect.top() + track as f32 * row_height),
            Vec2::new(width, row_height),
        );
        painter.rect_filled(
            row,
            0.0,
            if track % 2 == 0 {
                roles().canvas
            } else {
                roles().row_alternate
            },
        );
        painter.line_segment(
            [row.left_bottom(), row.right_bottom()],
            Stroke::new(1.0_f32, roles().line.gamma_multiply(0.6)),
        );
    }
    let first_bar = ((visible.left() - rect.left()) / bar_width)
        .floor()
        .max(0.0) as u32;
    let last_bar = ((visible.right() - rect.left()) / bar_width)
        .ceil()
        .min(frame.bars as f32) as u32;
    for bar in first_bar..=last_bar {
        let x = rect.left() + bar as f32 * bar_width;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(
                1.0_f32,
                if bar % 4 == 0 {
                    roles().line
                } else {
                    roles().bar_line
                },
            ),
        );
    }
    if frame.add_track.is_some() {
        let target = Rect::from_min_size(
            Pos2::new(
                rect.left(),
                rect.top() + frame.occupied_count as f32 * row_height,
            ),
            Vec2::new(width, row_height),
        )
        .intersect(canvas_clip);
        let hint = painter.layout_no_wrap(
            "Drop a sound here for a new track".into(),
            FontId::proportional(13.0),
            roles().muted,
        );
        if target.height() >= hint.size().y + 2.0 {
            // Clear the bar lines behind the hint.
            let area = Rect::from_center_size(target.center(), hint.size());
            painter.rect_filled(area.expand2(Vec2::new(6.0, 1.0)), 0.0, roles().canvas);
            painter.galley(area.min, hint, roles().muted);
        }
    }
}

/// The waveform of an on-screen clip, kept per clip and rebuilt only when its
/// key changes. None while the sound's peaks are pending or unavailable.
fn clip_wave<'a>(
    waves: &'a mut std::collections::HashMap<u64, waveform::ClipWave>,
    session: &Session,
    clip: &Clip,
    sample: &Sample,
    tick_width: f32,
    bpm: f64,
) -> Option<&'a waveform::ClipWave> {
    match session.sample_peaks(sample.id) {
        PeakState::Ready(peaks) => {
            let key = waveform::ClipWaveKey::new(
                clip,
                &peaks,
                session.catalog_revision(),
                tick_width,
                frames_per_tick(sample, bpm),
            );
            let wave = waves
                .entry(clip.id)
                .or_insert_with(|| waveform::ClipWave::new(key, &peaks));
            if wave.key != key {
                *wave = waveform::ClipWave::new(key, &peaks);
            }
            Some(&*wave)
        }
        PeakState::Pending | PeakState::Unavailable => None,
    }
}
