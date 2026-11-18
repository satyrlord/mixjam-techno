//! The Inspector's clip pages. One clip shows a picture of its length and
//! fades, whose handles drag, above cards of exact values: its fades, its
//! sound, and its place. Several clips share the fade and sound cards; a
//! change there sets every selected clip at once, as one undo step.
use super::super::view::{CLIP_SILENCE_DB, TICKS_PER_BEAT, clip_gain_db, clip_gain_from_db};
use super::*;
use mixjam_techno::application::{
    ClipSound, FadeCurve, Fades, MAX_PITCH_CENTS, MAX_TICKS, amplitude_to_db,
};

/// The height of one field row in a card.
const ROW: f32 = 28.0;
/// The height of the clip's picture.
const PICTURE: f32 = 56.0;
/// Where a card's value column starts, from the card's left edge.
const VALUES: f32 = 72.0;
/// The text that a field shows when the selected clips differ.
const MIXED: &str = "–";

/// What the fade and sound cards edit.
#[derive(Clone, Copy, PartialEq)]
struct Sound {
    gain: f32,
    fades: Fades,
    semitones: i32,
    cents: i32,
    reverse: bool,
}

impl Sound {
    fn of(clip: &Clip) -> Self {
        Self {
            gain: clip.gain,
            fades: clip.fades,
            semitones: clip.pitch_cents / 100,
            cents: clip.pitch_cents % 100,
            reverse: clip.reverse,
        }
    }

    fn pitch(self, original: i32, touched: Fields) -> i32 {
        let semitones = if touched.semitones {
            self.semitones
        } else {
            original / 100
        };
        let cents = if touched.cents {
            self.cents
        } else {
            original % 100
        };
        (semitones * 100 + cents).clamp(-MAX_PITCH_CENTS, MAX_PITCH_CENTS)
    }
}

/// One flag per field of the fade and sound cards: which values differ
/// between the selected clips, or which fields an edit touched.
#[derive(Clone, Copy, Default)]
struct Fields {
    gain: bool,
    fade_in: bool,
    fade_in_curve: bool,
    fade_out: bool,
    fade_out_curve: bool,
    semitones: bool,
    cents: bool,
    reverse: bool,
}

impl Fields {
    fn any(self) -> bool {
        self.gain || self.fades() || self.pitch() || self.reverse
    }
    fn fades(self) -> bool {
        self.fade_in || self.fade_in_curve || self.fade_out || self.fade_out_curve
    }
    fn pitch(self) -> bool {
        self.semitones || self.cents
    }
    /// A pick or a switch is an undo step of its own.
    fn discrete(self) -> bool {
        self.fade_in_curve || self.fade_out_curve || self.reverse
    }
}

impl MixJamApp {
    pub(super) fn clip_page(&mut self, ui: &mut egui::Ui, id: u64) {
        let project = self.session.project();
        let Some(clip) = project.clips.iter().find(|clip| clip.id == id).cloned() else {
            return;
        };
        let sample = self.session.library().sample_by_id(clip.sample_id).cloned();
        let place = format!(
            "{}  ·  Track {}",
            bars_text(clip.start_tick, clip.start_tick + clip.length_ticks),
            clip.track + 1
        );
        let Some(sample) = sample else {
            let name = self
                .session
                .missing_sample(clip.sample_id)
                .map(|missing| missing.name.clone());
            title(
                ui,
                name.as_deref().unwrap_or("Missing sound"),
                &["Missing sound", &place],
            );
            note(
                ui,
                "The sound's file is not in the libraries, so the clip plays silence. Locate \
                 the file to hear it again; every clip of the sound follows.",
            );
            let response = ui.button("Locate the sound…");
            if name_control(response, &format!("{CLIP} locate sound"))
                .hover_hint(
                    "Choose the sound's audio file. Import details in the status bar list \
                     every missing sound.",
                )
                .clicked()
            {
                self.resolve_missing_sample(clip.sample_id);
            }
            return;
        };
        title(
            ui,
            &sample.name,
            &[&group_label(&sample.group_name), &place],
        );
        let bpm = self
            .session
            .project()
            .sample_playback_bpm(sample.id, sample.source_bpm);
        let mut next = clip.clone();
        self.clip_picture(ui, &clip, &sample, bpm, &mut next.fades);

        let mut sound = Sound::of(&next);
        let rooms = [
            clip.length_ticks.saturating_sub(next.fades.fade_out.ticks),
            clip.length_ticks.saturating_sub(next.fades.fade_in.ticks),
        ];
        let (touched, mut discrete) = sound_cards(ui, &mut sound, Fields::default(), rooms);
        next.gain = sound.gain;
        next.fades = sound.fades;
        next.pitch_cents = sound.pitch(clip.pitch_cents, touched);
        next.reverse = sound.reverse;

        section_heading(ui, "Position");
        let rows = card(ui, 3);
        let at = row(rows, 0);
        row_label(ui, at, "Start");
        let mut bar = f64::from(next.start_tick) / f64::from(TICKS_PER_BAR) + 1.0;
        if number_field(
            ui,
            values(at),
            &format!("{CLIP} start (bar)"),
            &mut bar,
            1.0..=(f64::from(MAX_TICKS - next.length_ticks) / f64::from(TICKS_PER_BAR) + 1.0),
            1.0 / 16.0,
            ("bar ", ""),
            false,
            "The bar the clip starts on; 1.5 is halfway through bar 1. Its lane stays the same.",
            &mut discrete,
        ) {
            next.start_tick = ((bar - 1.0) * f64::from(TICKS_PER_BAR)).round().max(0.0) as u32;
        }
        let at = row(rows, 1);
        row_label(ui, at, "Length");
        let mut bars = f64::from(next.length_ticks) / f64::from(TICKS_PER_BAR);
        let unit = if bars == 1.0 { " bar" } else { " bars" };
        if number_field(
            ui,
            values(at),
            &format!("{CLIP} length (bars)"),
            &mut bars,
            1.0 / f64::from(TICKS_PER_BAR)
                ..=f64::from(MAX_TICKS - next.start_tick) / f64::from(TICKS_PER_BAR),
            1.0 / 16.0,
            ("", unit),
            false,
            "The clip's length in bars. A clip longer than its sound plays silence after it.",
            &mut discrete,
        ) {
            next.length_ticks = (bars * f64::from(TICKS_PER_BAR)).round().max(1.0) as u32;
            next.fades = next.fades.fit(next.length_ticks);
        }
        let at = row(rows, 2);
        row_label(ui, at, "Trim start");
        // The trim is a count of source frames; beats at the song's tempo
        // read like the clip's other times.
        let frames_per_beat = view::frames_per_tick(&sample, bpm) * f64::from(TICKS_PER_BEAT);
        if frames_per_beat > 0.0 {
            let mut beats = f64::from(next.offset_frames) / frames_per_beat;
            let last = f64::from(sample.frame_count.saturating_sub(1)) / frames_per_beat;
            let unit = if beats == 1.0 { " beat" } else { " beats" };
            if number_field(
                ui,
                values(at),
                &format!("{CLIP} trim start (beats)"),
                &mut beats,
                0.0..=last,
                1.0 / 16.0,
                ("", unit),
                false,
                "How much of the sound's start the clip skips, in beats at the song's tempo. \
                 The clip stays where it is.",
                &mut discrete,
            ) {
                next.offset_frames = ((beats * frames_per_beat).round().max(0.0) as u32)
                    .min(sample.frame_count.saturating_sub(1));
            }
        }

        if next != clip {
            let command = EditCommand::SetClip {
                id,
                start_tick: next.start_tick,
                gain: next.gain,
                offset_frames: next.offset_frames,
                length_ticks: next.length_ticks,
                fades: next.fades,
                pitch_cents: next.pitch_cents,
                reverse: next.reverse,
            };
            let result = if discrete {
                self.session.edit_discrete(command)
            } else {
                self.session.edit(command)
            };
            self.apply(result);
            ui.ctx().request_repaint();
        }
    }

    /// The clip at the panel's width: its category fill and waveform, its
    /// fades shaded, and its gain line. The fade handles drag, as on the
    /// clip in the score, and change `fades`.
    fn clip_picture(
        &mut self,
        ui: &mut egui::Ui,
        clip: &Clip,
        sample: &Sample,
        bpm: f64,
        fades: &mut Fades,
    ) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, PICTURE), Sense::hover());
        let length = clip.length_ticks.max(1);
        let tick_width = rect.width() / length as f32;
        let bar_width = tick_width * TICKS_PER_BAR as f32;
        let color = group_color(&sample.group_name);
        let painter = ui.painter().clone();
        theme::surface(&painter, rect, view::clip_radius(rect), color);
        let waves = &mut self.clip_paint.inspector_waves;
        waves.retain(|id, _| *id == clip.id);
        if let Some(wave) = view::clip_wave(waves, &self.session, clip, sample, tick_width, bpm) {
            waveform::paint_clip_wave(&painter, rect, rect.top(), wave, color);
        }
        // A tick on the bottom edge at each bar line, when bars are wide
        // enough to count.
        if bar_width >= 8.0 {
            let ink = roles().on_light.gamma_multiply(0.5);
            for bar in 1..clip.length_ticks.div_ceil(TICKS_PER_BAR) {
                let x = rect.left() + bar as f32 * bar_width;
                painter.line_segment(
                    [
                        Pos2::new(x, rect.bottom() - 5.0),
                        Pos2::new(x, rect.bottom() - 1.0),
                    ],
                    Stroke::new(1.0_f32, ink),
                );
            }
        }
        view::draw_fades(&painter, rect, clip, Some(sample), bar_width, bpm);
        view::draw_gain_line(&painter, rect, clip.gain);
        view::draw_fade_handles(&painter, rect, clip, sample, bar_width, bpm);
        if clip.reverse {
            painter.text(
                rect.right_bottom() + Vec2::new(-6.0, -4.0),
                Align2::RIGHT_BOTTOM,
                "rev",
                FontId::proportional(10.0),
                roles().on_light,
            );
        }

        let handles = view::fade_handles(rect, clip, sample, bar_width, bpm);
        let sound_end = view::sound_ticks(clip, sample, bpm);
        let split = (handles[0].x + handles[1].x) / 2.0;
        let left = usize::from(handles[1].x < handles[0].x);
        for (index, (edge, center)) in ["in", "out"].into_iter().zip(handles).enumerate() {
            let name = format!("{CLIP} fade {edge} handle");
            let mut hit = rect.intersect(Rect::from_center_size(
                center,
                Vec2::splat(view::FADE_HANDLE + 8.0),
            ));
            // Split overlapping targets at the midpoint. At the same x, the
            // left and right halves still select different handles.
            if index == left {
                hit.max.x = hit.max.x.min(split);
            } else {
                hit.min.x = hit.min.x.max(split);
            }
            let response = ui.interact(hit, ui.id().with(&name), Sense::click_and_drag());
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if response.clicked() || response.drag_started() {
                response.request_focus();
            }
            let fine = ui.input(|input| input.modifiers.ctrl);
            let step = if fine { 1 } else { TICKS_PER_BAR / 16 };
            let (fade, room) = if index == 0 {
                (&mut fades.fade_in, clip.length_ticks - fades.fade_out.ticks)
            } else {
                (&mut fades.fade_out, clip.length_ticks - fades.fade_in.ticks)
            };
            let mut edited = None;
            if (response.dragged() || response.drag_stopped())
                && let Some(pos) = response.interact_pointer_pos()
            {
                let tick = f64::from((pos.x - rect.left()) / tick_width);
                let ticks = if index == 0 {
                    tick
                } else {
                    f64::from(sound_end) - tick
                };
                let snapped = ((ticks / f64::from(step)).round() * f64::from(step)) as u32;
                edited = Some(snapped);
            }
            let mut direction = 0;
            if response.has_focus() {
                controls::keep_transport_key(ui.ctx(), response.id);
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        response.id,
                        egui::EventFilter {
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            ..Default::default()
                        },
                    );
                });
                ui.input_mut(|input| {
                    let modifiers = input.modifiers;
                    for key in [egui::Key::ArrowUp, egui::Key::ArrowRight] {
                        direction += input.count_and_consume_key(modifiers, key) as i32;
                    }
                    for key in [egui::Key::ArrowDown, egui::Key::ArrowLeft] {
                        direction -= input.count_and_consume_key(modifiers, key) as i32;
                    }
                    if input.consume_key(modifiers, egui::Key::Home) {
                        edited = Some(0);
                    }
                    if input.consume_key(modifiers, egui::Key::End) {
                        edited = Some(room);
                    }
                });
                view::focus_outline(ui, &response, roles().accent);
            }
            ui.input(|input| {
                use egui::accesskit::{Action, ActionData};
                for request in input.accesskit_action_requests(response.id, Action::SetValue) {
                    if let Some(ActionData::NumericValue(beats)) = request.data
                        && beats.is_finite()
                    {
                        edited = Some((beats * f64::from(TICKS_PER_BEAT)).round() as u32);
                    }
                }
                direction += input.num_accesskit_action_requests(response.id, Action::Increment)
                    as i32
                    - input.num_accesskit_action_requests(response.id, Action::Decrement) as i32;
            });
            if direction != 0 {
                edited = Some(
                    edited
                        .unwrap_or(fade.ticks)
                        .saturating_add_signed(direction * step as i32),
                );
                if response.has_focus() {
                    ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
                }
            }
            if let Some(ticks) = edited {
                fade.ticks = ticks.min(room);
            }
            let beats = |ticks| f64::from(ticks) / f64::from(TICKS_PER_BEAT);
            response.widget_info(|| {
                let value = beats(fade.ticks);
                let mut info = egui::WidgetInfo::slider(response.enabled(), value, &name);
                info.current_text_value = Some(format!("{value:.3} beats"));
                info
            });
            ui.ctx().accesskit_node_builder(response.id, |node| {
                use egui::accesskit::Action;
                node.set_min_numeric_value(0.0);
                node.set_max_numeric_value(beats(room));
                node.set_numeric_value_step(beats(TICKS_PER_BAR / 16));
                node.add_action(Action::SetValue);
                node.add_action(Action::Increment);
                node.add_action(Action::Decrement);
            });
            response.hover_hint(format!(
                "Drag or use the arrow keys to set fade {edge} in sixteenths. \
                 Hold Ctrl for single ticks."
            ));
        }
    }

    pub(super) fn clips_page(&mut self, ui: &mut egui::Ui, count: usize) {
        let project = self.session.project();
        let clips: Vec<Clip> = project
            .clips
            .iter()
            .filter(|clip| self.selection.clips.contains(&clip.id))
            .cloned()
            .collect();
        let Some(first) = clips.first() else {
            return;
        };
        let start = clips.iter().map(|clip| clip.start_tick).min().unwrap_or(0);
        let end = clips
            .iter()
            .map(|clip| clip.start_tick + clip.length_ticks)
            .max()
            .unwrap_or(0);
        let tracks: BTreeSet<usize> = clips.iter().map(|clip| clip.track).collect();
        title(
            ui,
            &format!("{count} clips"),
            &[&format!(
                "{}  ·  {} {}",
                bars_text(start, end),
                tracks.len(),
                if tracks.len() == 1 { "track" } else { "tracks" }
            )],
        );
        let before = Sound::of(first);
        let differs =
            |value: &dyn Fn(&Clip) -> i64| clips.iter().any(|clip| value(clip) != value(first));
        let mixed = Fields {
            gain: clips.iter().any(|clip| clip.gain != first.gain),
            fade_in: differs(&|clip| i64::from(clip.fades.fade_in.ticks)),
            fade_in_curve: clips
                .iter()
                .any(|clip| clip.fades.fade_in.curve != first.fades.fade_in.curve),
            fade_out: differs(&|clip| i64::from(clip.fades.fade_out.ticks)),
            fade_out_curve: clips
                .iter()
                .any(|clip| clip.fades.fade_out.curve != first.fades.fade_out.curve),
            semitones: differs(&|clip| i64::from(clip.pitch_cents / 100)),
            cents: differs(&|clip| i64::from(clip.pitch_cents % 100)),
            reverse: clips.iter().any(|clip| clip.reverse != first.reverse),
        };
        // The fades fit each clip when they are set; the fields allow the
        // longest clip's length.
        let longest = clips
            .iter()
            .map(|clip| clip.length_ticks)
            .max()
            .unwrap_or(0);
        let mut sound = before;
        let (touched, discrete) = sound_cards(ui, &mut sound, mixed, [longest, longest]);
        if touched.any() {
            let sounds = clips
                .iter()
                .map(|clip| {
                    let mut fades = clip.fades;
                    if touched.fade_in {
                        fades.fade_in.ticks = sound.fades.fade_in.ticks;
                    }
                    if touched.fade_in_curve {
                        fades.fade_in.curve = sound.fades.fade_in.curve;
                    }
                    if touched.fade_out {
                        fades.fade_out.ticks = sound.fades.fade_out.ticks;
                    }
                    if touched.fade_out_curve {
                        fades.fade_out.curve = sound.fades.fade_out.curve;
                    }
                    ClipSound {
                        id: clip.id,
                        gain: if touched.gain { sound.gain } else { clip.gain },
                        fades,
                        pitch_cents: sound.pitch(clip.pitch_cents, touched),
                        reverse: if touched.reverse {
                            sound.reverse
                        } else {
                            clip.reverse
                        },
                    }
                })
                .collect();
            let command = EditCommand::SetClipSounds(sounds);
            let result = if discrete {
                self.session.edit_discrete(command)
            } else {
                self.session.edit(command)
            };
            self.apply(result);
            ui.ctx().request_repaint();
        }
        let refs: Vec<&Clip> = clips.iter().collect();
        self.category_counts(ui, &refs);
        note(
            ui,
            "A change above sets every selected clip, as one undo step; a dash marks a value \
             that differs between them. Drag any clip to move them together; Delete removes \
             them all.",
        );
    }
}

/// The fade and sound cards. `mixed` marks the values that differ between
/// the selected clips; `rooms` are the longest fade in and fade out. Returns
/// the fields that changed.
fn sound_cards(
    ui: &mut egui::Ui,
    sound: &mut Sound,
    mixed: Fields,
    rooms: [u32; 2],
) -> (Fields, bool) {
    let mut touched = Fields::default();
    let mut discrete = false;
    section_heading(ui, "Fades (beats)");
    let rows = card(ui, 2);
    for (index, edge) in ["in", "out"].into_iter().enumerate() {
        let at = row(rows, index);
        row_label(ui, at, if index == 0 { "Fade in" } else { "Fade out" });
        let (fade, mixed_ticks, mixed_curve) = if index == 0 {
            (&mut sound.fades.fade_in, mixed.fade_in, mixed.fade_in_curve)
        } else {
            (
                &mut sound.fades.fade_out,
                mixed.fade_out,
                mixed.fade_out_curve,
            )
        };
        let mut field = values(at);
        field.min.x -= 8.0;
        let split = field.left() + 78.0;
        let beats_rect = Rect::from_x_y_ranges(field.left()..=split - 2.0, field.y_range());
        let curve_rect = Rect::from_x_y_ranges(split + 2.0..=field.right(), field.y_range());
        let mut beats = f64::from(fade.ticks) / f64::from(TICKS_PER_BEAT);
        if number_field(
            ui,
            beats_rect,
            &format!("{CLIP} fade {edge} (beats)"),
            &mut beats,
            0.0..=f64::from(rooms[index]) / f64::from(TICKS_PER_BEAT),
            1.0 / 16.0,
            ("", ""),
            mixed_ticks,
            if index == 0 {
                "Fade in from the clip's start, in beats. 0 is no fade."
            } else {
                "Fade out to the end of the clip's sound, in beats: the clip's end, or \
                 earlier where the sound runs out. 0 is no fade."
            },
            &mut discrete,
        ) {
            fade.ticks = ((beats * f64::from(TICKS_PER_BEAT)).round() as u32).min(rooms[index]);
            if index == 0 {
                touched.fade_in = true;
            } else {
                touched.fade_out = true;
            }
        }
        let name = format!("{CLIP} fade {edge} curve");
        if let Some(curve) = curve_field(ui, curve_rect, &name, fade.curve, mixed_curve) {
            fade.curve = curve;
            if index == 0 {
                touched.fade_in_curve = true;
            } else {
                touched.fade_out_curve = true;
            }
        }
    }

    section_heading(ui, "Sound");
    let rows = card(ui, 3);
    let at = row(rows, 0);
    row_label(ui, at, "Gain");
    let field = values(at);
    let fader = Rect::from_x_y_ranges(field.left()..=field.right() - 70.0, field.y_range());
    let value = Rect::from_x_y_ranges(field.right() - 66.0..=field.right(), field.y_range());
    let mut gain = sound.gain;
    let mut fader_ui = ui.new_child(egui::UiBuilder::new().id_salt("gain-fader").max_rect(fader));
    if mixed.gain {
        fader_ui.multiply_opacity(0.45);
    }
    let set_gain = lanes::clip_fader(&mut fader_ui, fader, CLIP, &mut gain);
    if set_gain && (mixed.gain || gain != sound.gain) {
        sound.gain = gain;
        touched.gain = true;
    }
    if gain_field(
        ui,
        value,
        &mut sound.gain,
        mixed.gain && !set_gain,
        &mut discrete,
    ) {
        touched.gain = true;
    }

    let at = row(rows, 1);
    row_label(ui, at, "Pitch");
    let field = values(at);
    let split = field.center().x;
    touched.semitones = number_field(
        ui,
        Rect::from_x_y_ranges(field.left()..=split - 2.0, field.y_range()),
        &format!("{CLIP} pitch (semitones)"),
        &mut sound.semitones,
        -24..=24,
        0.1,
        ("", " st"),
        mixed.semitones,
        "Transpose the clip in semitones. Its length and timing stay the same.",
        &mut discrete,
    );
    touched.cents = number_field(
        ui,
        Rect::from_x_y_ranges(split + 2.0..=field.right(), field.y_range()),
        &format!("{CLIP} pitch (cents)"),
        &mut sound.cents,
        -99..=99,
        0.5,
        ("", " ct"),
        mixed.cents,
        "Fine-tune the clip in cents, hundredths of a semitone.",
        &mut discrete,
    );
    let at = row(rows, 2);
    row_label(ui, at, "Reverse");
    let field = values(at);
    let name = format!("{CLIP} reverse");
    let mut reverse = sound.reverse && !mixed.reverse;
    let response = controls::toggle_at(
        ui,
        Rect::from_x_y_ranges(field.left()..=field.left() + 30.0, field.y_range()),
        &name,
        "",
        &mut reverse,
    );
    use egui::accesskit::{Action, ActionData, Toggled};
    let requested = ui.input(|input| {
        input
            .accesskit_action_requests(response.id, Action::SetValue)
            .filter_map(|request| match request.data {
                Some(ActionData::NumericValue(0.0)) => Some(false),
                Some(ActionData::NumericValue(1.0)) => Some(true),
                _ => None,
            })
            .last()
    });
    if let Some(requested) = requested {
        reverse = requested;
    }
    let changed = response.clicked() || requested.is_some();
    let still_mixed = mixed.reverse && !changed;
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_toggled(if still_mixed {
            Toggled::Mixed
        } else {
            reverse.into()
        });
        if still_mixed {
            node.set_description("Some selected clips play backwards.");
        }
        node.add_action(Action::SetValue);
    });
    ui.painter().text(
        Pos2::new(field.left() + 38.0, field.center().y),
        Align2::LEFT_CENTER,
        if still_mixed {
            "Some play backwards"
        } else if reverse {
            "Plays backwards"
        } else {
            "Plays forwards"
        },
        FontId::proportional(12.0),
        roles().strip_text,
    );
    if changed {
        sound.reverse = reverse;
        touched.reverse = true;
    }
    name_control(response, &name).hover_hint(
        "Play the clip's sound backwards within the clip, for reversed swells and risers. \
         Its fades, pitch, and gain stay where they are.",
    );
    (touched, discrete || touched.discrete())
}

/// A card of `rows` field rows on the strip surface, at the page's width.
fn card(ui: &mut egui::Ui, rows: usize) -> Rect {
    let width = ui.available_width();
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(width, rows as f32 * ROW + 8.0), Sense::hover());
    row_surface(ui, rect, false);
    rect
}

/// The field row `index` of a card.
fn row(card: Rect, index: usize) -> Rect {
    Rect::from_min_size(
        card.min + Vec2::new(0.0, 4.0 + index as f32 * ROW),
        Vec2::new(card.width(), ROW),
    )
}

/// A row's value column: every field starts at the same place.
fn values(row: Rect) -> Rect {
    Rect::from_x_y_ranges(
        row.left() + VALUES..=row.right() - 8.0,
        row.center().y - 11.0..=row.center().y + 11.0,
    )
}

fn row_label(ui: &egui::Ui, row: Rect, text: &str) {
    ui.painter().text(
        row.left_center() + Vec2::new(10.0, 0.0),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(12.0),
        roles().strip_text,
    );
}

/// Show `add` in `rect` as a strip field: the face of the Mixer's gain value,
/// with the value in its ink.
fn strip_field<R>(
    ui: &mut egui::Ui,
    rect: Rect,
    salt: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.painter().rect_filled(rect, 3.0, roles().strip_face);
    let ink = roles().legend;
    let mut field = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("inspector-field", salt))
            .max_rect(rect),
    );
    field.set_clip_rect(rect.intersect(field.clip_rect()));
    field.spacing_mut().interact_size = rect.size();
    field.spacing_mut().button_padding = Vec2::new(6.0, 0.0);
    field.style_mut().override_text_style = Some(egui::TextStyle::Body);
    field
        .style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(12.0));
    field.visuals_mut().override_text_color = Some(ink);
    let widgets = &mut field.style_mut().visuals.widgets;
    for widget in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.bg_fill = Color32::TRANSPARENT;
        widget.weak_bg_fill = Color32::TRANSPARENT;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke.color = ink;
    }
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ink.gamma_multiply(0.5));
    widgets.active.bg_stroke = Stroke::new(1.0_f32, ink);
    widgets.open.bg_stroke = Stroke::new(1.0_f32, ink);
    add(&mut field)
}

/// A number in a strip field, with its unit as `affixes` (prefix, suffix).
/// A `mixed` field shows a dash until it changes.
#[allow(clippy::too_many_arguments)]
fn number_field<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
    speed: f64,
    (prefix, suffix): (&str, &str),
    mixed: bool,
    hint: &str,
    discrete: &mut bool,
) -> bool {
    strip_field(ui, rect, name, |ui| {
        let before = *value;
        let mut edited = false;
        let draft = FieldDraft::new(ui);
        let format = move |value: f64| {
            if mixed {
                MIXED.to_owned()
            } else {
                egui::emath::format_with_decimals_in_range(value, 0..=3)
            }
        };
        let drag = field_drag(value, range, &mut edited)
            .speed(speed)
            .max_decimals(if N::INTEGRAL { 0 } else { 3 })
            .prefix(if mixed { "" } else { prefix })
            .suffix(if mixed { "" } else { suffix })
            .custom_formatter(move |value, _| format(value))
            .custom_parser(move |text| {
                if !draft.typed {
                    return None;
                }
                text.replace(char::is_whitespace, "")
                    .replace('−', "-")
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
            });
        let response = ui.add(drag);
        edited = draft.finish(ui, &response, edited, discrete);
        name_control(response, name).hover_hint(hint);
        edited && (mixed || *value != before)
    })
}

/// A text draft commits only after the user edits it. Opening and leaving the
/// editor keeps the original value, including precision the field cannot show.
#[derive(Clone, Copy)]
struct FieldDraft {
    id: egui::Id,
    typed: bool,
}

impl FieldDraft {
    fn new(ui: &egui::Ui) -> Self {
        let control = ui.next_auto_id();
        let id = control.with("inspector-number-draft");
        let typed_now = ui.memory(|memory| memory.has_focus(control))
            && ui.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Text(_) | egui::Event::Paste(_)))
            });
        Self {
            id,
            typed: typed_now || ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false)),
        }
    }

    fn finish(self, ui: &egui::Ui, response: &Response, edited: bool, discrete: &mut bool) -> bool {
        let lost_focus = response.lost_focus();
        if lost_focus || response.gained_focus() {
            ui.data_mut(|data| data.remove::<bool>(self.id));
        }
        if response.has_focus() && response.changed() && !edited {
            ui.data_mut(|data| data.insert_temp(self.id, true));
        }
        let requested = field_value_requested(ui, response);
        *discrete |= requested || (edited && lost_focus);
        edited || requested
    }
}

/// Record a numeric edit even when it sets a mixed field to the first value.
fn field_drag<'a, N: egui::emath::Numeric>(
    value: &'a mut N,
    range: std::ops::RangeInclusive<N>,
    edited: &'a mut bool,
) -> egui::DragValue<'a> {
    let (min, max) = (range.start().to_f64(), range.end().to_f64());
    egui::DragValue::from_get_set(move |requested| {
        if let Some(requested) = requested.filter(|value| value.is_finite()) {
            *value = N::from_f64(requested.clamp(min, max));
            *edited = true;
        }
        value.to_f64()
    })
    .range(range)
    .clamp_existing_to_range(false)
    .update_while_editing(false)
}

/// egui skips the setter when an accessibility request repeats the old value.
fn field_value_requested(ui: &egui::Ui, response: &Response) -> bool {
    use egui::accesskit::{Action, ActionData};
    ui.input(|input| {
        input
            .accesskit_action_requests(response.id, Action::SetValue)
            .any(|request| match request.data {
                Some(ActionData::NumericValue(value)) => value.is_finite(),
                _ => false,
            })
    })
}

/// The clip gain in dB in a strip field, from -inf to +6 dB. An untouched
/// value keeps its exact saved amplitude.
fn gain_field(
    ui: &mut egui::Ui,
    rect: Rect,
    gain: &mut f32,
    mixed: bool,
    discrete: &mut bool,
) -> bool {
    let name = format!("{CLIP} gain value");
    let before = clip_gain_db(*gain);
    let mut db = before;
    let mut edited = false;
    let format = move |value: f64| {
        if mixed {
            MIXED.into()
        } else if value <= f64::from(CLIP_SILENCE_DB) {
            "-inf dB".into()
        } else if value.abs() < 0.05 {
            "0.0 dB".into()
        } else {
            format!("{value:+.1} dB")
        }
    };
    strip_field(ui, rect, &name, |ui| {
        let draft = FieldDraft::new(ui);
        let response = ui.add(
            field_drag(&mut db, CLIP_SILENCE_DB..=amplitude_to_db(2.0), &mut edited)
                .speed(0.1)
                .custom_formatter(move |value, _| format(value))
                .custom_parser(move |text| {
                    if !draft.typed {
                        return None;
                    }
                    let text = text.trim().to_lowercase().replace('−', "-");
                    let text = text.trim_end_matches("db").trim();
                    if matches!(text, "-inf" | "-infinity" | "silence") {
                        Some(f64::from(CLIP_SILENCE_DB))
                    } else {
                        text.parse::<f64>().ok().filter(|value| value.is_finite())
                    }
                }),
        );
        edited = draft.finish(ui, &response, edited, discrete);
        name_control(response, &name).hover_hint(
            "Clip gain in dB. 0 is unity; -inf is silence; the maximum is +6 dB. \
             Double-click the fader for unity.",
        );
    });
    if !edited {
        return false;
    }
    let before_gain = *gain;
    if db != before || db <= CLIP_SILENCE_DB {
        *gain = clip_gain_from_db(db);
    }
    mixed || *gain != before_gain
}

/// A fade curve in a strip field, as a drop-down list. Returns the curve
/// picked this frame.
fn curve_field(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    curve: FadeCurve,
    mixed: bool,
) -> Option<FadeCurve> {
    let mut picked = None;
    strip_field(ui, rect, name, |ui| {
        ui.spacing_mut().button_padding.x = 2.0;
        ui.spacing_mut().icon_width = 7.0;
        ui.spacing_mut().icon_spacing = 2.0;
        let response = egui::ComboBox::from_id_salt(name)
            .selected_text(if mixed { MIXED } else { curve.label() })
            .width(rect.width())
            .truncate()
            .show_ui(ui, |ui| {
                for option in FadeCurve::ALL {
                    if ui
                        .selectable_label(!mixed && curve == option, option.label())
                        .clicked()
                        && (mixed || curve != option)
                    {
                        picked = Some(option);
                    }
                }
            })
            .response;
        name_control(response, name).hover_hint(
            "The fade's shape: Linear; Equal power, for crossfades that keep their loudness; \
             or Exponential, even steps in decibels like a natural decay.",
        );
    });
    picked
}

/// "Bar 3" or "Bars 3 to 6" for a span of ticks.
fn bars_text(start: u32, end: u32) -> String {
    let first = start / TICKS_PER_BAR + 1;
    let last = end.div_ceil(TICKS_PER_BAR).max(first);
    if first == last {
        format!("Bar {first}")
    } else {
        format!("Bars {first} to {last}")
    }
}

/// A sound's category as the Sounds panel names it.
fn group_label(group: &str) -> String {
    GROUPS
        .iter()
        .find(|name| name.eq_ignore_ascii_case(group))
        .map_or_else(|| group.to_owned(), |name| (*name).to_owned())
}
