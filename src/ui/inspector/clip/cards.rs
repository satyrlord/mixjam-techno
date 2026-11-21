//! The cards of the clip pages: the fade and sound cards that one clip and
//! several clips share, and the row layout of every card.
use super::super::super::controls::{NumberField, curve_field, gain_field, number_field};
use super::super::super::view::TICKS_PER_BEAT;
use super::*;
use mixjam_techno::application::{Fades, MAX_PITCH_CENTS};

/// What the fade and sound cards edit.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Sound {
    pub gain: f32,
    pub fades: Fades,
    pub semitones: i32,
    pub cents: i32,
    pub reverse: bool,
}

impl Sound {
    pub(super) fn of(clip: &Clip) -> Self {
        Self {
            gain: clip.gain,
            fades: clip.fades,
            semitones: clip.pitch_cents / 100,
            cents: clip.pitch_cents % 100,
            reverse: clip.reverse,
        }
    }

    pub(super) fn pitch(self, original: i32, touched: Fields) -> i32 {
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
pub(super) struct Fields {
    pub gain: bool,
    pub fade_in: bool,
    pub fade_in_curve: bool,
    pub fade_out: bool,
    pub fade_out_curve: bool,
    pub semitones: bool,
    pub cents: bool,
    pub reverse: bool,
}

impl Fields {
    pub(super) fn any(self) -> bool {
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

/// The fade and sound cards. `mixed` marks the values that differ between
/// the selected clips; `rooms` are the longest fade in and fade out. Returns
/// the fields that changed.
pub(super) fn sound_cards(
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
            NumberField {
                name: &format!("{CLIP} fade {edge} (beats)"),
                range: 0.0..=f64::from(rooms[index]) / f64::from(TICKS_PER_BEAT),
                speed: 1.0 / 16.0,
                affixes: ("", ""),
                mixed: mixed_ticks,
                hint: if index == 0 {
                    "Fade in from the clip's start, in beats. 0 is no fade."
                } else {
                    "Fade out to the end of the clip's sound, in beats: the clip's end, or \
                     earlier where the sound runs out. 0 is no fade."
                },
            },
            &mut beats,
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
        &format!("{CLIP} gain value"),
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
        NumberField {
            name: &format!("{CLIP} pitch (semitones)"),
            range: -MAX_PITCH_CENTS / 100..=MAX_PITCH_CENTS / 100,
            speed: 0.1,
            affixes: ("", " st"),
            mixed: mixed.semitones,
            hint: "Transpose the clip in semitones. Its length and timing stay the same.",
        },
        &mut sound.semitones,
        &mut discrete,
    );
    touched.cents = number_field(
        ui,
        Rect::from_x_y_ranges(split + 2.0..=field.right(), field.y_range()),
        NumberField {
            name: &format!("{CLIP} pitch (cents)"),
            range: -99..=99,
            speed: 0.5,
            affixes: ("", " ct"),
            mixed: mixed.cents,
            hint: "Fine-tune the clip in cents, hundredths of a semitone.",
        },
        &mut sound.cents,
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
pub(super) fn card(ui: &mut egui::Ui, rows: usize) -> Rect {
    let width = ui.available_width();
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(width, rows as f32 * ROW + 8.0), Sense::hover());
    row_surface(ui, rect, false);
    rect
}

/// The field row `index` of a card.
pub(super) fn row(card: Rect, index: usize) -> Rect {
    Rect::from_min_size(
        card.min + Vec2::new(0.0, 4.0 + index as f32 * ROW),
        Vec2::new(card.width(), ROW),
    )
}

/// A row's value column: every field starts at the same place.
pub(super) fn values(row: Rect) -> Rect {
    Rect::from_x_y_ranges(
        row.left() + VALUES..=row.right() - 8.0,
        row.center().y - 11.0..=row.center().y + 11.0,
    )
}

pub(super) fn row_label(ui: &egui::Ui, row: Rect, text: &str) {
    ui.painter().text(
        row.left_center() + Vec2::new(10.0, 0.0),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(12.0),
        roles().strip_text,
    );
}
