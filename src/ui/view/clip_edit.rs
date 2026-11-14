//! Editing a clip in the arrangement: its property fields, the context menu,
//! the handle drags that reshape it, and the keys of the focused lanes.
use super::*;
use mixjam_techno::application::{
    Fade, FadeCurve, MAX_PITCH_CENTS, MAX_TICKS, amplitude_to_db, db_to_amplitude,
};

/// A clip gain at or below this level is silence.
const CLIP_SILENCE_DB: f32 = -60.0;

/// The lane context menu: what it was opened on, and where.
#[derive(Default)]
pub(in super::super) struct ClipMenu {
    pub(in super::super) missing_sample: Option<u32>,
    /// The clip that the lane context menu was opened on.
    pub(in super::super) clip: Option<u64>,
    /// Where the lane context menu opens: the pointer, or the clip for a key.
    pub(in super::super) anchor: Pos2,
    /// Shift+F10 asked for the selected clip's menu; the lanes open it.
    pub(in super::super) by_key: bool,
}

/// A clip's gain in dB, from `CLIP_SILENCE_DB` for silence to +6 dB.
pub(in super::super) fn clip_gain_db(gain: f32) -> f32 {
    if gain > 0.0 {
        amplitude_to_db(gain).max(CLIP_SILENCE_DB)
    } else {
        CLIP_SILENCE_DB
    }
}

/// A clip's gain amplitude from dB. `CLIP_SILENCE_DB` and below are silence.
pub(in super::super) fn clip_gain_from_db(db: f32) -> f32 {
    if db <= CLIP_SILENCE_DB {
        0.0
    } else {
        db_to_amplitude(db).min(2.0)
    }
}

impl MixJamApp {
    /// Exact values for one clip, in its context menu. Drags on the clip's
    /// edges, fade handles, and gain line change the same values.
    /// A clip's gain, trim, length, fades, pitch, and reverse, as a grid of
    /// fields named "{prefix} gain" and so on: "Clip" in the clip menu,
    /// "Inspector clip" in the Inspector, which also shows its start.
    pub(in super::super) fn clip_properties(&mut self, ui: &mut egui::Ui, id: u64, prefix: &str) {
        let Some(clip) = self
            .session
            .project()
            .clips
            .iter()
            .find(|c| c.id == id)
            .cloned()
        else {
            return;
        };
        let Some(max_frames) = self
            .session
            .library()
            .sample_by_id(clip.sample_id)
            .map(|sample| sample.frame_count)
        else {
            return;
        };
        ui.separator();
        // The menu style hides widget frames; the value fields need theirs.
        ui.style_mut().visuals.widgets = ui.ctx().style().visuals.widgets.clone();
        let mut gain = clip.gain;
        let mut offset = clip.offset_frames;
        let mut length = clip.length_ticks;
        let mut pitch = clip.pitch_cents;
        let mut reverse = clip.reverse;
        let mut start = clip.start_tick;
        let mut change = false;
        let mut fades = clip.fades;
        egui::Grid::new("clip_properties")
            .num_columns(2)
            .spacing(Vec2::new(if prefix == "Clip" { 10.0 } else { 6.0 }, 6.0))
            .show(ui, |ui| {
                if prefix != "Clip" {
                    ui.label("Start (bar)");
                    let mut bar = f64::from(start) / f64::from(TICKS_PER_BAR) + 1.0;
                    if name_control(
                        ui.add(
                            egui::DragValue::new(&mut bar)
                                .range(1.0..=f64::INFINITY)
                                .speed(1.0 / 16.0)
                                .max_decimals(3),
                        ),
                        &format!("{prefix} start (bar)"),
                    )
                    .hover_hint(
                        "The bar the clip starts on; 1.5 is halfway through bar 1. Its lane \
                         stays the same.",
                    )
                    .changed()
                    {
                        start = ((bar - 1.0) * f64::from(TICKS_PER_BAR)).round().max(0.0) as u32;
                        change = true;
                    }
                    ui.end_row();
                }
                // The mixer shows dB, so the clip does too. An untouched value
                // keeps its exact saved amplitude.
                ui.label("Gain (dB)");
                let mut db = clip_gain_db(gain);
                if name_control(
                    ui.add(
                        egui::DragValue::new(&mut db)
                            .range(CLIP_SILENCE_DB..=amplitude_to_db(2.0))
                            .speed(0.1)
                            .custom_formatter(|value, _| {
                                if value <= f64::from(CLIP_SILENCE_DB) {
                                    "-inf".into()
                                } else {
                                    format!("{value:+.1}")
                                }
                            })
                            .custom_parser(|text| {
                                let text = text.trim().to_lowercase();
                                let text = text.trim_end_matches("db").trim();
                                if matches!(text, "-inf" | "-infinity" | "silence") {
                                    Some(f64::from(CLIP_SILENCE_DB))
                                } else {
                                    text.parse().ok()
                                }
                            }),
                    ),
                    &format!("{prefix} gain"),
                )
                .hover_hint("Clip gain in dB. 0 is unity; -inf is silence; the maximum is +6 dB.")
                .changed()
                {
                    gain = clip_gain_from_db(db);
                    change = true;
                }
                ui.end_row();
                ui.label("Trim start (frames)");
                change |= name_control(
                    ui.add(
                        egui::DragValue::new(&mut offset)
                            .range(0..=max_frames.saturating_sub(1))
                            .speed(100),
                    ),
                    &format!("{prefix} trim start (frames)"),
                )
                .hover_hint("Source frames that the clip skips. The clip does not move.")
                .changed();
                ui.end_row();
                ui.label("Length (bars)");
                let mut bars = length as f64 / TICKS_PER_BAR as f64;
                if name_control(
                    ui.add(
                        egui::DragValue::new(&mut bars)
                            .range(1.0 / 16.0..=256.0)
                            .speed(1.0 / 16.0),
                    ),
                    &format!("{prefix} length (bars)"),
                )
                .changed()
                {
                    length = (bars * TICKS_PER_BAR as f64).round().max(1.0) as u32;
                    change = true;
                }
                ui.end_row();
                // Apply the model's shortening order before the controls
                // constrain their individual ranges.
                fades = fades.fit(length);
                let room = length.saturating_sub(fades.fade_out.ticks);
                change |= fade_controls(ui, prefix, "in", &mut fades.fade_in, room);
                ui.end_row();
                let room = length.saturating_sub(fades.fade_in.ticks);
                change |= fade_controls(ui, prefix, "out", &mut fades.fade_out, room);
                ui.end_row();
                change |= pitch_controls(ui, prefix, &mut pitch);
                ui.end_row();
                ui.label("Reverse");
                change |= name_control(
                    ui.checkbox(&mut reverse, "Play backwards"),
                    &format!("{prefix} reverse"),
                )
                .hover_hint(
                    "Play the clip's sound backwards within the clip, for reversed swells \
                         and risers. Its fades, pitch, and gain stay where they are.",
                )
                .changed();
                ui.end_row();
            });
        if change {
            let result = self.session.edit(EditCommand::SetClip {
                id,
                start_tick: start,
                gain,
                offset_frames: offset,
                length_ticks: length,
                fades,
                pitch_cents: pitch,
                reverse,
            });
            self.apply(result);
        }
    }

    /// Open the lane context menu for the selected clip, at the clip, as a
    /// right-click there does. Shift+F10 asks for it.
    pub(super) fn open_clip_menu(&mut self, ctx: &egui::Context, response: &Response, rect: Rect) {
        let Some(clip) = self.selection.clip.and_then(|id| {
            self.session
                .project()
                .clips
                .iter()
                .find(|clip| clip.id == id)
        }) else {
            return;
        };
        self.clip_menu.clip = Some(clip.id);
        self.clip_menu.missing_sample = self
            .session
            .missing_sample(clip.sample_id)
            .map(|missing| missing.id);
        self.clip_menu.anchor = self.clip_rect(clip, rect).left_bottom();
        egui::Popup::open_id(ctx, egui::Popup::default_response_id(response));
    }

    /// The clip and handle under `pos`. A clip whose sound is missing has none.
    pub(in super::super) fn clip_handle_at(
        &self,
        pos: Pos2,
        rect: Rect,
    ) -> Option<(u64, ClipHandle)> {
        let clip = self
            .session
            .project()
            .clips
            .iter()
            .rev()
            .find(|clip| self.clip_rect(clip, rect).contains(pos))?;
        let sample = self.session.library().sample_by_id(clip.sample_id)?;
        let handle = handle_at(
            pos,
            self.clip_rect(clip, rect),
            self.score.drop_rect.unwrap_or(rect),
            clip,
            sample,
            self.score.bar_width,
            self.session
                .project()
                .sample_playback_bpm(sample.id, sample.source_bpm),
        )?;
        Some((clip.id, handle))
    }

    /// Start, follow, and finish a drag of a clip handle. The clip shows the
    /// change during the drag, and the release makes one edit.
    pub(super) fn handle_drag_step(&mut self, ui: &egui::Ui, response: &Response, rect: Rect) {
        if self.gestures.handle_drag.is_none()
            && response.drag_started_by(egui::PointerButton::Primary)
            && !ui.input(|i| i.modifiers.alt || i.modifiers.shift)
            && let Some(pos) = ui.input(|i| i.pointer.press_origin())
            && let Some((id, handle)) = self.clip_handle_at(pos, rect)
            && let Some(clip) = self
                .session
                .project()
                .clips
                .iter()
                .find(|clip| clip.id == id)
                .cloned()
        {
            if !self.selection.clips.contains(&id) {
                self.select_clip(id, false);
            }
            self.selection.place_on_click = false;
            self.gestures.handle_drag = Some(HandleDrag {
                handle,
                origin: pos,
                preview: clip.clone(),
                clip,
            });
        }
        let Some(drag) = &self.gestures.handle_drag else {
            return;
        };
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            let preview = self.reshape(drag, pos, rect, ui.input(|i| i.modifiers.ctrl));
            drag_label(ui.ctx(), pos, &handle_readout(drag.handle, &preview));
            if let Some(drag) = self.gestures.handle_drag.as_mut() {
                drag.preview = preview;
            }
        }
        if (response.drag_stopped() || ui.input(|i| i.pointer.any_released()))
            && let Some(HandleDrag { clip, preview, .. }) = self.gestures.handle_drag.take()
            && preview != clip
        {
            let result = self.session.edit_discrete(EditCommand::SetClip {
                id: preview.id,
                start_tick: preview.start_tick,
                gain: preview.gain,
                offset_frames: preview.offset_frames,
                length_ticks: preview.length_ticks,
                fades: preview.fades,
                pitch_cents: preview.pitch_cents,
                reverse: preview.reverse,
            });
            self.apply(result);
        }
    }

    /// The dragged clip with its handle at `pos`. Edges snap to bars, or to
    /// sixteenths with Ctrl; fades snap to sixteenths, or to ticks with Ctrl.
    /// The clip's neighbors and its sound bound the edges.
    fn reshape(&self, drag: &HandleDrag, pos: Pos2, rect: Rect, fine: bool) -> Clip {
        let clip = &drag.clip;
        let mut shaped = clip.clone();
        let Some(sample) = self.session.library().sample_by_id(clip.sample_id) else {
            return shaped;
        };
        let tick = f64::from((pos.x - rect.left()).max(0.0) / self.score.bar_width)
            * f64::from(TICKS_PER_BAR);
        let snap = |ticks: f64, step: u32| {
            ((ticks / f64::from(step)).round() * f64::from(step)).max(0.0) as u32
        };
        let (edge_step, fade_step) = if fine {
            (TICKS_PER_BAR / 16, 1)
        } else {
            (TICKS_PER_BAR, TICKS_PER_BAR / 16)
        };
        let end = clip.start_tick + clip.length_ticks;
        let shortest = (TICKS_PER_BAR / 16).min(clip.length_ticks);
        let others = self
            .session
            .project()
            .clips
            .iter()
            .filter(|other| other.track == clip.track && other.id != clip.id);
        match drag.handle {
            ClipHandle::Start => {
                // The left edge can reveal the trimmed start of the sound.
                let frames_per_tick = frames_per_tick(
                    sample,
                    self.session
                        .project()
                        .sample_playback_bpm(sample.id, sample.source_bpm),
                );
                let revealable = if frames_per_tick > 0.0 {
                    (f64::from(clip.offset_frames) / frames_per_tick) as u32
                } else {
                    0
                };
                let previous_end = others
                    .filter(|other| other.start_tick < clip.start_tick)
                    .map(|other| other.start_tick + other.length_ticks)
                    .max()
                    .unwrap_or(0)
                    .min(clip.start_tick);
                let first = previous_end.max(clip.start_tick.saturating_sub(revealable));
                let remaining = sample
                    .frame_count
                    .saturating_sub(1)
                    .saturating_sub(clip.offset_frames);
                let trimmable = if frames_per_tick > 0.0 {
                    (f64::from(remaining) / frames_per_tick) as u32
                } else {
                    0
                };
                let last = (end - shortest).min(clip.start_tick.saturating_add(trimmable));
                let start = snap(tick, edge_step).clamp(first, last);
                let moved = f64::from(start) - f64::from(clip.start_tick);
                shaped.start_tick = start;
                shaped.length_ticks = end - start;
                shaped.offset_frames = (f64::from(clip.offset_frames) + moved * frames_per_tick)
                    .round()
                    .clamp(0.0, f64::from(sample.frame_count.saturating_sub(1)))
                    as u32;
                shaped.fades = clip.fades.fit(shaped.length_ticks);
            }
            ClipHandle::End => {
                let next_start = others
                    .filter(|other| other.start_tick > clip.start_tick)
                    .map(|other| other.start_tick.max(end))
                    .min()
                    .unwrap_or(MAX_TICKS);
                let new_end = snap(tick, edge_step).clamp(clip.start_tick + shortest, next_start);
                shaped.length_ticks = new_end - clip.start_tick;
                shaped.fades = clip.fades.fit(shaped.length_ticks);
            }
            ClipHandle::FadeIn => {
                shaped.fades.fade_in.ticks = snap(tick - f64::from(clip.start_tick), fade_step)
                    .min(clip.length_ticks - clip.fades.fade_out.ticks);
            }
            ClipHandle::FadeOut => {
                let sound_end = clip.start_tick
                    + sound_ticks(
                        clip,
                        sample,
                        self.session
                            .project()
                            .sample_playback_bpm(sample.id, sample.source_bpm),
                    );
                shaped.fades.fade_out.ticks = snap(f64::from(sound_end) - tick, fade_step)
                    .min(clip.length_ticks - clip.fades.fade_in.ticks);
            }
            ClipHandle::Gain => {
                // Each `GAIN_PIXELS` of travel up or down is one dB, or a
                // tenth with Ctrl, from the saved gain.
                let steps = ((drag.origin.y - pos.y) / GAIN_PIXELS).trunc();
                if steps != 0.0 {
                    let step = if fine { 0.1 } else { 1.0 };
                    shaped.gain = clip_gain_from_db(
                        (clip_gain_db(clip.gain) + steps * step).clamp(CLIP_SILENCE_DB, 6.0),
                    );
                }
            }
        }
        shaped
    }

    /// Keys for the focused lanes: arrows choose a clip, Alt+arrows move the
    /// selection, Enter previews it, Delete removes it, and Shift+F10 opens
    /// its context menu.
    pub(super) fn lane_keys(&mut self, ui: &egui::Ui, response: &Response, rect: Rect) {
        controls::keep_transport_key(ui.ctx(), response.id);
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            )
        });
        let (mut step, mut shift) = ((0, 0), (0, 0));
        let (mut preview, mut delete, mut fine, mut menu) = (false, false, false, false);
        ui.input_mut(|input| {
            for (key, x, y) in [
                (egui::Key::ArrowLeft, -1, 0),
                (egui::Key::ArrowRight, 1, 0),
                (egui::Key::ArrowUp, 0, -1),
                (egui::Key::ArrowDown, 0, 1),
            ] {
                // egui ignores an extra Alt when it matches a key, so read it here.
                let modifiers = input.modifiers;
                if input.consume_key(modifiers, key) {
                    if modifiers.alt {
                        shift = (x, y);
                        fine = modifiers.ctrl;
                    } else {
                        step = (x, y);
                    }
                }
            }
            preview = input.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
            delete = input.consume_key(egui::Modifiers::NONE, egui::Key::Delete);
            menu = input.consume_key(egui::Modifiers::SHIFT, egui::Key::F10);
        });
        self.clip_menu.by_key |= menu;
        if step != (0, 0) {
            self.step_clip_selection(step.0, step.1);
        }
        if shift != (0, 0) && !self.selection.clips.is_empty() {
            let ids: Vec<_> = self.selection.clips.iter().copied().collect();
            let ticks = i64::from(if fine {
                TICKS_PER_BAR / 16
            } else {
                TICKS_PER_BAR
            });
            if let Err(error) =
                self.try_move_clips(&ids, shift.1, i64::from(shift.0) * ticks, false)
            {
                self.error(error);
            }
        }
        if preview {
            self.preview_selection();
        }
        if delete {
            self.delete_selection();
        }
        if (step != (0, 0) || shift != (0, 0))
            && let Some(clip) = self.selection.clip.and_then(|id| {
                self.session
                    .project()
                    .clips
                    .iter()
                    .find(|clip| clip.id == id)
            })
        {
            ui.scroll_to_rect(self.clip_rect(clip, rect), None);
        }
    }
}

/// Ticks in one beat, the unit of the fade controls.
pub(super) const TICKS_PER_BEAT: u32 = TICKS_PER_BAR / 4;

/// A clip fade's length in beats, at most `room` ticks, and its curve: one
/// row of the clip properties grid.
fn fade_controls(ui: &mut egui::Ui, prefix: &str, edge: &str, fade: &mut Fade, room: u32) -> bool {
    ui.label(format!("Fade {edge} (beats)"));
    ui.horizontal(|ui| {
        let mut beats = f64::from(fade.ticks) / f64::from(TICKS_PER_BEAT);
        let mut change = false;
        if name_control(
            ui.add(
                egui::DragValue::new(&mut beats)
                    .range(0.0..=f64::from(room) / f64::from(TICKS_PER_BEAT))
                    .speed(1.0 / 16.0)
                    .max_decimals(3),
            ),
            &format!("{prefix} fade {edge} (beats)"),
        )
        .hover_hint(if edge == "in" {
            "Fade in from the clip's start, in beats. 0 is no fade."
        } else {
            "Fade out to the end of the clip's sound, in beats: the clip's end, or earlier \
         where the sound runs out. 0 is no fade."
        })
        .changed()
        {
            fade.ticks = ((beats * f64::from(TICKS_PER_BEAT)).round() as u32).min(room);
            change = true;
        }
        // A submenu, because a combo box inside the context menu would close it.
        let name = format!("{prefix} fade {edge} curve");
        let response = ui
            .menu_button(fade.curve.label(), |ui| {
                for curve in FadeCurve::ALL {
                    if ui
                        .selectable_label(fade.curve == curve, curve.label())
                        .clicked()
                    {
                        change |= fade.curve != curve;
                        fade.curve = curve;
                        ui.close();
                    }
                }
            })
            .response;
        name_control(response, &name).hover_hint(
            "Linear, Equal power for crossfades that keep their loudness, or Exponential, \
         even steps in decibels like a natural decay.",
        );
        change
    })
    .inner
}

/// A clip's pitch as semitones and cents: one row of the clip properties
/// grid. Its timing does not change.
fn pitch_controls(ui: &mut egui::Ui, prefix: &str, cents: &mut i32) -> bool {
    let mut semitones = *cents / 100;
    let mut fine = *cents % 100;
    ui.label("Pitch");
    let change = ui
        .horizontal(|ui| {
            let semitone_change = name_control(
                ui.add(
                    egui::DragValue::new(&mut semitones)
                        .range(-24..=24)
                        .speed(0.1)
                        .suffix(" st"),
                ),
                &format!("{prefix} pitch (semitones)"),
            )
            .hover_hint("Transpose the clip in semitones. Its length and timing stay the same.")
            .changed();
            let cent_change = name_control(
                ui.add(
                    egui::DragValue::new(&mut fine)
                        .range(-99..=99)
                        .speed(0.5)
                        .suffix(" ct"),
                ),
                &format!("{prefix} pitch (cents)"),
            )
            .hover_hint("Fine-tune the clip in cents, hundredths of a semitone.")
            .changed();
            semitone_change || cent_change
        })
        .inner;
    if change {
        *cents = (semitones * 100 + fine).clamp(-MAX_PITCH_CENTS, MAX_PITCH_CENTS);
    }
    change
}
