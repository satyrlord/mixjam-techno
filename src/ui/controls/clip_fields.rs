//! Shared clip parameter fields and fade handle interaction used by both the
//! Inspector's clip cards and the arrangement clip context menu.
use super::super::{name_control, view};
use super::*;
use mixjam_techno::application::{
    CLIP_SILENCE_DB, FadeCurve, TICKS_PER_BAR, TICKS_PER_BEAT, clip_gain_db, clip_gain_from_db,
    clip_gain_max_db,
};

const MIXED: &str = "–";
const FADE_CURVE_HINT: &str = "The fade's shape: Linear; Equal power, for crossfades that keep \
     their loudness; or Exponential, even steps in decibels like a natural decay.";

/// Which side of a clip a fade handle or field belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FadeEdge {
    In,
    Out,
}

impl FadeEdge {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Out => "out",
        }
    }
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

/// What a number field shows and allows, apart from its value.
pub(crate) struct NumberField<'a, N> {
    /// The control's accessible name.
    pub name: &'a str,
    pub range: RangeInclusive<N>,
    /// How far one point of a drag moves the value.
    pub speed: f64,
    /// The unit around the number: prefix and suffix.
    pub affixes: (&'a str, &'a str),
    /// The selected clips differ, so the field shows a dash until it changes.
    pub mixed: bool,
    pub hint: &'a str,
}

/// A number field inside an Inspector strip-field rect, tracking discrete vs
/// continuous undo commits.
pub(crate) fn number_field<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    rect: Rect,
    field: NumberField<'_, N>,
    value: &mut N,
    discrete: &mut bool,
) -> bool {
    let salt = field.name;
    strip_field(ui, rect, salt, |ui| {
        run_number_field(ui, field, value, Some(discrete))
    })
}

/// An inline number field rendered at the current layout cursor (for menus).
pub(crate) fn inline_number_field<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    field: NumberField<'_, N>,
    value: &mut N,
) -> bool {
    run_number_field(ui, field, value, None)
}

fn run_number_field<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    field: NumberField<'_, N>,
    value: &mut N,
    discrete: Option<&mut bool>,
) -> bool {
    let NumberField {
        name,
        range,
        speed,
        affixes: (prefix, suffix),
        mixed,
        hint,
    } = field;
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
        .update_while_editing(discrete.is_none())
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
    let labeled = name_control(response, name);
    if !hint.is_empty() {
        labeled.hover_hint(hint);
    }
    edited && (mixed || *value != before)
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

    fn finish(
        self,
        ui: &egui::Ui,
        response: &Response,
        edited: bool,
        discrete: Option<&mut bool>,
    ) -> bool {
        let lost_focus = response.lost_focus();
        if lost_focus || response.gained_focus() {
            ui.data_mut(|data| data.remove::<bool>(self.id));
        }
        if response.has_focus() && response.changed() && !edited {
            ui.data_mut(|data| data.insert_temp(self.id, true));
        }
        let requested = field_value_requested(ui, response);
        if let Some(discrete) = discrete {
            *discrete |= requested || (edited && lost_focus);
        }
        edited || requested
    }
}

/// Record a numeric edit even when it sets a mixed field to the first value.
fn field_drag<'a, N: egui::emath::Numeric>(
    value: &'a mut N,
    range: RangeInclusive<N>,
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

fn format_clip_gain_db(value: f64, mixed: bool, with_unit: bool) -> String {
    if mixed {
        MIXED.into()
    } else if value <= f64::from(CLIP_SILENCE_DB) {
        if with_unit { "-inf dB" } else { "-inf" }.into()
    } else if with_unit && value.abs() < 0.05 {
        "0.0 dB".into()
    } else if with_unit {
        format!("{value:+.1} dB")
    } else {
        format!("{value:+.1}")
    }
}

fn parse_clip_gain_db(text: &str) -> Option<f64> {
    let text = text.trim().to_lowercase().replace('−', "-");
    let text = text.trim_end_matches("db").trim();
    if matches!(text, "-inf" | "-infinity" | "silence") {
        Some(f64::from(CLIP_SILENCE_DB))
    } else {
        text.parse::<f64>().ok().filter(|value| value.is_finite())
    }
}

fn run_gain_field(
    ui: &mut egui::Ui,
    name: &str,
    gain: &mut f32,
    mixed: bool,
    discrete: Option<&mut bool>,
    with_unit: bool,
    hint: &str,
) -> bool {
    let before = clip_gain_db(*gain);
    let mut db = before;
    let mut edited = false;
    let draft = FieldDraft::new(ui);
    let response = ui.add(
        field_drag(&mut db, CLIP_SILENCE_DB..=clip_gain_max_db(), &mut edited)
            .update_while_editing(discrete.is_none())
            .speed(0.1)
            .custom_formatter(move |value, _| format_clip_gain_db(value, mixed, with_unit))
            .custom_parser(move |text| {
                if !draft.typed {
                    return None;
                }
                parse_clip_gain_db(text)
            }),
    );
    edited = draft.finish(ui, &response, edited, discrete);
    name_control(response, name).hover_hint(hint);
    if !edited {
        return false;
    }
    let before_gain = *gain;
    if db != before || db <= CLIP_SILENCE_DB {
        *gain = clip_gain_from_db(db);
    }
    mixed || *gain != before_gain
}

/// The clip gain in dB in a strip field, from -inf to +6 dB. An untouched
/// value keeps its exact saved amplitude.
pub(crate) fn gain_field(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    gain: &mut f32,
    mixed: bool,
    discrete: &mut bool,
) -> bool {
    strip_field(ui, rect, name, |ui| {
        run_gain_field(
            ui,
            name,
            gain,
            mixed,
            Some(discrete),
            true,
            "Clip gain in dB. 0 is unity; -inf is silence; the maximum is +6 dB. \
             Double-click the fader for unity.",
        )
    })
}

/// Inline clip gain in dB for context menus. An untouched value keeps its
/// exact saved amplitude.
pub(crate) fn inline_gain_field(ui: &mut egui::Ui, name: &str, gain: &mut f32) -> bool {
    run_gain_field(
        ui,
        name,
        gain,
        false,
        None,
        false,
        "Clip gain in dB. 0 is unity; -inf is silence; the maximum is +6 dB.",
    )
}

/// A fade curve in a strip field, as a drop-down list. Returns the curve
/// picked this frame.
pub(crate) fn curve_field(
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
        name_control(response, name).hover_hint(FADE_CURVE_HINT);
    });
    picked
}

/// A fade curve selector inside a popup menu (a submenu so picking a curve
/// does not fight popup placement).
pub(crate) fn curve_menu(ui: &mut egui::Ui, name: &str, curve: &mut FadeCurve) -> bool {
    let mut changed = false;
    let response = ui
        .menu_button(curve.label(), |ui| {
            for option in FadeCurve::ALL {
                if ui
                    .selectable_label(*curve == option, option.label())
                    .clicked()
                {
                    changed |= *curve != option;
                    *curve = option;
                    ui.close();
                }
            }
        })
        .response;
    name_control(response, name).hover_hint(FADE_CURVE_HINT);
    changed
}

/// Snap a pointer x-coordinate (converted to ticks from the clip start) to a
/// fade length in ticks, clamped within `room`.
pub(crate) fn snapped_fade_ticks(
    edge: FadeEdge,
    pointer_tick: f64,
    sound_end_ticks: u32,
    room: u32,
    fine: bool,
) -> u32 {
    let step = if fine { 1 } else { TICKS_PER_BAR / 16 };
    let ticks = match edge {
        FadeEdge::In => pointer_tick,
        FadeEdge::Out => f64::from(sound_end_ticks) - pointer_tick,
    };
    (((ticks / f64::from(step)).round() * f64::from(step)).max(0.0) as u32).min(room)
}

/// Geometry and constraints for one interactive fade handle in the Inspector.
pub(crate) struct FadeHandleSpec<'a> {
    pub rect: Rect,
    pub tick_width: f32,
    pub sound_end: u32,
    pub split: f32,
    pub is_left: bool,
    pub name: &'a str,
    pub edge: FadeEdge,
    pub center: Pos2,
    pub room: u32,
}

/// One fade handle: its hit area, pointer drag, keyboard arrows, AccessKit
/// slider semantics, and hover hint. Returns updated fade ticks if edited.
pub(crate) fn fade_handle(
    ui: &mut egui::Ui,
    spec: FadeHandleSpec<'_>,
    current_ticks: u32,
) -> Option<u32> {
    let FadeHandleSpec {
        rect,
        tick_width,
        sound_end,
        split,
        is_left,
        name,
        edge,
        center,
        room,
    } = spec;
    let mut hit = rect.intersect(Rect::from_center_size(
        center,
        Vec2::splat(view::FADE_HANDLE + 8.0),
    ));
    // Split overlapping targets at the midpoint. At the same x, the
    // left and right halves still select different handles.
    if is_left {
        hit.max.x = hit.max.x.min(split);
    } else {
        hit.min.x = hit.min.x.max(split);
    }
    let response = ui.interact(hit, ui.id().with(name), Sense::click_and_drag());
    let dragging = primary_drag_or_stop(ui, &response);
    if response.hovered() || dragging {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if response.clicked() || (dragging && response.drag_started()) {
        response.request_focus();
    }
    let fine = ui.input(|input| input.modifiers.ctrl);
    let step = if fine { 1 } else { TICKS_PER_BAR / 16 };
    let mut edited = None;
    if dragging && let Some(pos) = drag_position(ui, &response) {
        let tick = f64::from((pos.x - rect.left()) / tick_width);
        edited = Some(snapped_fade_ticks(edge, tick, sound_end, room, fine));
    }
    let mut direction = 0;
    if response.has_focus() {
        keep_transport_key(ui.ctx(), response.id);
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
        direction += input.num_accesskit_action_requests(response.id, Action::Increment) as i32
            - input.num_accesskit_action_requests(response.id, Action::Decrement) as i32;
    });
    if direction != 0 {
        edited = Some(
            edited
                .unwrap_or(current_ticks)
                .saturating_add_signed(direction * step as i32),
        );
        if response.has_focus() {
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
    }
    let result = edited.map(|ticks| ticks.min(room));
    let shown_ticks = result.unwrap_or(current_ticks);
    let beats = |ticks| f64::from(ticks) / f64::from(TICKS_PER_BEAT);
    response.widget_info(|| {
        let value = beats(shown_ticks);
        let mut info = egui::WidgetInfo::slider(response.enabled(), value, name);
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
        "Drag or use the arrow keys to set fade {} in sixteenths. \
         Hold Ctrl for single ticks.",
        edge.label()
    ));
    result
}
