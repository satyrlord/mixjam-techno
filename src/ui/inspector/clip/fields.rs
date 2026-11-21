//! The fields of the clip pages: a number, a gain, and a fade curve in a strip
//! field, the face of the Mixer's gain value.
use super::super::super::view::{
    CLIP_SILENCE_DB, clip_gain_db, clip_gain_from_db, clip_gain_max_db,
};
use super::*;
use mixjam_techno::application::FadeCurve;

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

/// What a number field shows and allows, apart from its value.
pub(super) struct NumberField<'a, N> {
    /// The control's accessible name.
    pub name: &'a str,
    pub range: std::ops::RangeInclusive<N>,
    /// How far one point of a drag moves the value.
    pub speed: f64,
    /// The unit around the number: prefix and suffix.
    pub affixes: (&'a str, &'a str),
    /// The selected clips differ, so the field shows a dash until it changes.
    pub mixed: bool,
    pub hint: &'a str,
}

/// A number in a strip field, with its unit as `affixes` (prefix, suffix).
/// A `mixed` field shows a dash until it changes.
pub(super) fn number_field<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    rect: Rect,
    field: NumberField<'_, N>,
    value: &mut N,
    discrete: &mut bool,
) -> bool {
    let NumberField {
        name,
        range,
        speed,
        affixes: (prefix, suffix),
        mixed,
        hint,
    } = field;
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
pub(super) fn gain_field(
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
            field_drag(&mut db, CLIP_SILENCE_DB..=clip_gain_max_db(), &mut edited)
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
pub(super) fn curve_field(
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
