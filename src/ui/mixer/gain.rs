//! The gain fader: its taper between level and rail position, the text a
//! level reads and is typed in, the rail, and its keyboard and accessibility
//! behavior.
use super::*;

const SILENCE_DB: f32 = -96.0;
pub(in super::super) const GAIN_STEP_DB: f32 = 0.5;
pub(in super::super) const GAIN_FINE_STEP_DB: f32 = 0.1;
// The useful -24..+12 dB interval occupies 72% of the travel. The short
// bottom section is linear amplitude, so the rail reaches exact zero.
const TAPER: [(f32, f32); 6] = [
    (0.08, -60.0),
    (0.16, -36.0),
    (0.28, -24.0),
    (0.46, -12.0),
    (0.70, 0.0),
    (1.0, 12.0),
];

/// Percentage of rail travel and the dB bounds of the main mixing range.
#[cfg(test)]
pub(in super::super) const fn useful_taper_span() -> (f32, f32, f32) {
    ((TAPER[5].0 - TAPER[2].0) * 100.0, TAPER[2].1, TAPER[5].1)
}

#[derive(Clone, Copy)]
pub(crate) enum FaderRange {
    Mixer,
    Clip,
}

impl FaderRange {
    fn silence_db(self) -> f32 {
        match self {
            Self::Mixer => SILENCE_DB,
            Self::Clip => super::super::view::CLIP_SILENCE_DB,
        }
    }

    fn maximum_db(self) -> f32 {
        match self {
            Self::Mixer => MAX_MIXER_GAIN_DB,
            Self::Clip => super::super::view::clip_gain_max_db(),
        }
    }

    fn travel(self) -> std::ops::RangeInclusive<f32> {
        match self {
            Self::Mixer => 0.0..=1.0,
            Self::Clip => {
                gain_fraction(db_to_amplitude(self.silence_db()))
                    ..=gain_fraction(super::super::view::MAX_CLIP_GAIN)
            }
        }
    }

    pub(crate) fn fraction(self, gain: f32) -> f32 {
        let travel = self.travel();
        ((gain_fraction(gain) - travel.start()) / (travel.end() - travel.start())).clamp(0.0, 1.0)
    }

    pub(crate) fn gain(self, fraction: f32) -> f32 {
        if fraction <= 0.0 {
            0.0
        } else if matches!(self, Self::Clip) && fraction >= 1.0 {
            super::super::view::MAX_CLIP_GAIN
        } else {
            fraction_gain(egui::lerp(self.travel(), fraction.clamp(0.0, 1.0)))
        }
    }

    fn gain_at_db(self, db: f32) -> f32 {
        match self {
            Self::Mixer => db_to_amplitude(db.min(self.maximum_db())),
            Self::Clip => super::super::view::clip_gain_from_db(db),
        }
    }
}

pub(crate) fn gain_fraction(gain: f32) -> f32 {
    let low = db_to_amplitude(TAPER[0].1);
    if gain <= low {
        return TAPER[0].0 * gain / low;
    }
    let db = amplitude_to_db(gain);
    for pair in TAPER.windows(2) {
        let [(a, lo), (b, hi)] = [pair[0], pair[1]];
        if db <= hi {
            return egui::lerp(a..=b, (db - lo) / (hi - lo));
        }
    }
    1.0
}

pub(crate) fn fraction_gain(fraction: f32) -> f32 {
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= TAPER[0].0 {
        return db_to_amplitude(TAPER[0].1) * fraction / TAPER[0].0;
    }
    for pair in TAPER.windows(2) {
        let [(a, lo), (b, hi)] = [pair[0], pair[1]];
        if fraction <= b {
            return db_to_amplitude(egui::lerp(lo..=hi, (fraction - a) / (b - a)));
        }
    }
    db_to_amplitude(MAX_MIXER_GAIN_DB)
}

fn db_text(db: f32) -> String {
    if db == f32::NEG_INFINITY {
        "-inf dB".into()
    } else if db.abs() < 0.05 {
        "0.0 dB".into()
    } else {
        format!("{db:+.1} dB")
    }
}

/// The gain field's text while it is being edited: one decimal, like `db_text`, without the unit.
fn db_edit_text(db: f32) -> String {
    if db == f32::NEG_INFINITY {
        "-inf".into()
    } else if db.abs() < 0.05 {
        "0.0".into()
    } else {
        format!("{db:.1}")
    }
}

fn parse_db(text: &str) -> Option<f64> {
    let value = text.trim().to_lowercase();
    let value = value.trim_end_matches("db").trim();
    match value {
        "-inf" | "-infinity" | "-∞" | "silence" => Some(f64::NEG_INFINITY),
        _ => value.parse::<f64>().ok().filter(|value| value.is_finite()),
    }
}

#[derive(Clone, Copy)]
struct GainEdit {
    before: f32,
    db: f32,
    shown: Option<f32>,
    typed: bool,
    relative_edit: bool,
    accesskit_entry: bool,
}

impl GainEdit {
    fn value(&self, gain: &mut f32, value: Option<f64>) -> f64 {
        if let Some(value) = value {
            self.apply_value(gain, value as f32);
        }
        // Relative input needs a finite origin. An absolute SetValue must
        // distinguish that origin (-96 dB) from actual silence.
        f64::from(if *gain == 0.0 && !self.accesskit_entry {
            SILENCE_DB
        } else {
            amplitude_to_db(*gain)
        })
    }

    fn apply_value(&self, gain: &mut f32, value: f32) {
        if self.suppressed_edit(value) || value == self.db {
            return;
        }
        if value.is_finite() || value == f32::NEG_INFINITY {
            *gain = db_to_amplitude(value.min(MAX_MIXER_GAIN_DB));
        }
    }

    fn suppressed_edit(&self, value: f32) -> bool {
        let lowering_silence = self.before == 0.0 && self.relative_edit && value <= SILENCE_DB;
        let untouched = !self.typed && !self.relative_edit && Some(value) == self.shown;
        !self.accesskit_entry && (lowering_silence || untouched)
    }

    fn format_value(&self, context: &egui::Context, id: egui::Id, value: f64) -> String {
        let value = value as f32;
        let shown = if self.silenced(value) {
            f32::NEG_INFINITY
        } else {
            value
        };
        if context.memory(|memory| memory.has_focus(id)) {
            db_edit_text(shown)
        } else {
            db_text(shown)
        }
    }

    fn silenced(&self, value: f32) -> bool {
        self.before == 0.0
            && !self.accesskit_entry
            && (value == SILENCE_DB || self.relative_edit && value <= SILENCE_DB)
    }
}

pub(crate) fn gain_value(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    gain: &mut f32,
) -> bool {
    ui.painter().rect_filled(rect, 3.0, roles().strip_face);
    let before = *gain;
    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt(("gain-value", name))
            .max_rect(rect),
        |ui| gain_value_widget(ui, rect, scale, name, gain, roles().legend),
    );
    before != *gain
}

fn gain_value_widget(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    gain: &mut f32,
    ink: Color32,
) {
    style_gain_value(ui, rect, scale, ink);
    let before = *gain;
    let db = amplitude_to_db(before);
    let id = ui.next_auto_id();
    let context = ui.ctx().clone();
    let has_focus = context.memory(|memory| memory.has_focus(id));
    let relative_edit = is_relative_gain_edit(ui, id, has_focus);
    let typed_id = id.with("typed");
    let typed = was_gain_typed(ui, &context, typed_id, has_focus);
    let accesskit_entry = ui.input(|input| {
        input.num_accesskit_action_requests(id, egui::accesskit::Action::SetValue) != 0
    });
    let edit = GainEdit {
        before,
        db,
        shown: parse_db(&db_edit_text(db)).map(|shown| shown as f32),
        typed,
        relative_edit,
        accesskit_entry,
    };
    let fmt_context = context.clone();
    let enabled = controls::layer_allows_input(ui);
    let response = ui.add(
        egui::DragValue::from_get_set(|value| edit.value(gain, value.filter(|_| enabled)))
            .range(f64::NEG_INFINITY..=f64::from(amplitude_to_db(4.0)))
            .clamp_existing_to_range(false)
            .speed(0.1)
            .update_while_editing(false)
            .custom_formatter(move |value, _| edit.format_value(&fmt_context, id, value))
            .custom_parser(parse_db),
    );
    let focused = response.has_focus();
    ui.ctx().data_mut(|data| {
        if focused {
            data.insert_temp(typed_id, typed);
        } else {
            data.remove::<bool>(typed_id);
        }
    });
    gain_value_semantics(ui, response, name, amplitude_to_db(*gain), enabled);
}

fn style_gain_value(ui: &mut egui::Ui, rect: Rect, scale: f32, ink: Color32) {
    ui.spacing_mut().interact_size = rect.size();
    // The value starts a little inside its face, so its sign clears the edge.
    ui.spacing_mut().button_padding = Vec2::new(4.0 * scale, 0.0);
    ui.style_mut().override_text_style = Some(egui::TextStyle::Body);
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(12.0 * scale));
    // The workspace style overrides text color; use the face's ink instead.
    ui.visuals_mut().override_text_color = Some(ink);
    let widgets = &mut ui.style_mut().visuals.widgets;
    for widget in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
    ] {
        widget.bg_fill = Color32::TRANSPARENT;
        widget.weak_bg_fill = Color32::TRANSPARENT;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke.color = ink;
    }
    widgets.active.bg_stroke = Stroke::new(1.0 * scale, ink);
}

fn is_relative_gain_edit(ui: &egui::Ui, id: egui::Id, has_focus: bool) -> bool {
    if ui.ctx().is_being_dragged(id) {
        return true;
    }
    ui.input(|input| {
        use egui::accesskit::Action;
        let arrow = has_focus
            && (input.key_pressed(egui::Key::ArrowUp) || input.key_pressed(egui::Key::ArrowDown));
        let acc = input.num_accesskit_action_requests(id, Action::Increment) > 0
            || input.num_accesskit_action_requests(id, Action::Decrement) > 0;
        arrow || acc
    })
}

fn is_typing_event(event: &egui::Event) -> bool {
    matches!(
        event,
        egui::Event::Text(_)
            | egui::Event::Paste(_)
            | egui::Event::Key {
                key: egui::Key::Backspace | egui::Key::Delete,
                pressed: true,
                ..
            }
    )
}

fn was_gain_typed(
    ui: &egui::Ui,
    context: &egui::Context,
    typed_id: egui::Id,
    has_focus: bool,
) -> bool {
    if context.data(|data| data.get_temp::<bool>(typed_id)) == Some(true) {
        return true;
    }
    has_focus && ui.input(|input| input.events.iter().any(is_typing_event))
}

fn gain_value_semantics(ui: &egui::Ui, response: Response, name: &str, db: f32, enabled: bool) {
    response.widget_info(|| {
        let number = if db.is_finite() { db } else { SILENCE_DB };
        let mut info = egui::WidgetInfo::drag_value(enabled, f64::from(number));
        info.label = Some(format!("{name} gain value"));
        info.current_text_value = Some(db_text(db));
        info
    });
    if !enabled {
        ui.ctx().accesskit_node_builder(response.id, |node| {
            use egui::accesskit::Action;
            for action in [Action::SetValue, Action::Increment, Action::Decrement] {
                node.remove_action(action);
            }
        });
    }
    response.hover_hint(format!(
        "{name} gain: {}. Click to enter dB, then press Enter. Negative dB attenuates. \
         Enter -inf for silence. Polarity stays unchanged.",
        db_text(db)
    ));
}

pub(super) fn gain_rail(
    ui: &mut egui::Ui,
    scale: f32,
    rect: Rect,
    name: &str,
    gain: &mut f32,
) -> bool {
    let before = *gain;
    let response = ui.interact(
        // The whole cap stays draggable at either end. Position mapping
        // below still uses the physical rail and clamps beyond its ends.
        rect.expand2(Vec2::new(0.0, 16.0 * scale)),
        ui.id().with(("gain-rail", name)),
        Sense::click_and_drag(),
    );
    let slot = Rect::from_center_size(
        rect.center(),
        Vec2::new(8.0 * scale, rect.height() + 4.0 * scale),
    );
    ui.painter().rect_filled(slot, 4.0, roles().strip_slot);
    ui.painter().rect_stroke(
        slot.expand(1.0 * scale),
        4.0,
        Stroke::new(1.0 * scale, roles().strip_edge),
        egui::StrokeKind::Outside,
    );
    // A pointer edit leaves keyboard focus unchanged. Tab focuses the rail
    // for key adjustments.
    let dragged = controls::primary_dragged(ui, &response);
    let pointer = if dragged {
        controls::drag_position(ui, &response)
    } else {
        response
            .interact_pointer_pos()
            .filter(|_| controls::primary_clicked(ui, &response))
    };
    if let Some(pos) = pointer {
        let fine = ui.input(|input| input.modifiers.shift);
        let fraction = if fine && dragged {
            gain_fraction(*gain) - ui.input(|input| input.pointer.delta().y) / rect.height() * 0.1
        } else {
            (rect.bottom() - pos.y) / rect.height()
        };
        *gain = fraction_gain(fraction);
    }
    gain_keys(ui, &response, gain, FaderRange::Mixer);
    let cap = Pos2::new(
        rect.center().x,
        egui::lerp(rect.bottom()..=rect.top(), gain_fraction(*gain)),
    );
    gain_cap(ui.painter(), cap, scale);
    focus_ring(ui, &response, rect, scale);
    gain_semantics(ui, response, name, before, *gain, FaderRange::Mixer);
    before != *gain
}

/// The edits every gain fader takes besides the pointer: the wheel, the
/// arrow keys, Home and End, the accessibility actions, and a double-click.
/// Returns whether the user set a value, including the current one.
pub(crate) fn gain_keys(
    ui: &egui::Ui,
    response: &Response,
    gain: &mut f32,
    range: FaderRange,
) -> bool {
    if !controls::control_allows_input(ui, response) {
        return false;
    }
    let step_db = if ui.input(|input| input.modifiers.shift) {
        GAIN_FINE_STEP_DB
    } else {
        GAIN_STEP_DB
    };
    let mut edited = gain_wheel_and_keys(ui, response, gain, range, step_db);
    edited |= ui.input(|input| gain_accesskit(input, response.id, range, gain, step_db));
    if edited && response.has_focus() {
        ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
    }
    if response.double_clicked() {
        *gain = 1.0;
        edited = true;
    }
    edited
}

fn gain_wheel_and_keys(
    ui: &egui::Ui,
    response: &Response,
    gain: &mut f32,
    range: FaderRange,
    step_db: f32,
) -> bool {
    let mut step = controls::wheel(ui, response);
    let mut edited = false;
    if response.has_focus() {
        controls::lock_arrows(ui, response.id);
        edited = ui.input_mut(|input| gain_keyboard(input, range, gain, &mut step));
    }
    if step != 0.0 {
        nudge_gain(range, gain, step * step_db);
        edited = true;
    }
    edited
}

fn gain_keyboard(
    input: &mut egui::InputState,
    range: FaderRange,
    gain: &mut f32,
    step: &mut f32,
) -> bool {
    let modifiers = input.modifiers;
    if input.consume_key(modifiers, egui::Key::ArrowUp)
        || input.consume_key(modifiers, egui::Key::ArrowRight)
    {
        *step = 1.0;
    }
    if input.consume_key(modifiers, egui::Key::ArrowDown)
        || input.consume_key(modifiers, egui::Key::ArrowLeft)
    {
        *step = -1.0;
    }
    let home = input.consume_key(modifiers, egui::Key::Home);
    let end = input.consume_key(modifiers, egui::Key::End);
    if home {
        *gain = 0.0;
    }
    if end {
        *gain = range.gain_at_db(range.maximum_db());
    }
    home || end
}

fn gain_accesskit(
    input: &egui::InputState,
    id: egui::Id,
    range: FaderRange,
    gain: &mut f32,
    step_db: f32,
) -> bool {
    use controls::NumericAction;
    let mut edited = false;
    for action in controls::numeric_actions(input, id) {
        match action {
            NumericAction::Set(db) if db <= f64::from(range.silence_db()) => *gain = 0.0,
            NumericAction::Set(db) => *gain = range.gain_at_db(db as f32),
            NumericAction::Increment => nudge_gain(range, gain, step_db),
            NumericAction::Decrement => nudge_gain(range, gain, -step_db),
        }
        edited = true;
    }
    edited
}

fn nudge_gain(range: FaderRange, gain: &mut f32, increment: f32) {
    let previous_db = amplitude_to_db(*gain);
    let db = match range {
        FaderRange::Clip => previous_db.max(range.silence_db()) + increment,
        FaderRange::Mixer if *gain == 0.0 && increment > 0.0 => -60.0,
        FaderRange::Mixer => previous_db + increment,
    };
    *gain = if increment < 0.0 && previous_db >= range.silence_db() && db <= range.silence_db() {
        0.0
    } else {
        range.gain_at_db(db)
    };
}

/// A gain fader's accessible name, value, range, and hint: "{name} gain".
pub(crate) fn gain_semantics(
    ui: &egui::Ui,
    mut response: Response,
    name: &str,
    before: f32,
    gain: f32,
    range: FaderRange,
) -> Response {
    let db = amplitude_to_db(gain);
    let enabled = response.enabled() && controls::layer_allows_input(ui);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::slider(
            enabled,
            f64::from(db.max(range.silence_db())),
            format!("{name} gain"),
        );
        info.current_text_value = Some(db_text(db));
        info
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::Action;
        node.set_min_numeric_value(f64::from(range.silence_db()));
        node.set_max_numeric_value(f64::from(range.maximum_db()));
        node.set_numeric_value_step(f64::from(GAIN_STEP_DB));
        if enabled {
            node.add_action(Action::SetValue);
            node.add_action(Action::Increment);
            node.add_action(Action::Decrement);
        }
    });
    if before != gain {
        response.mark_changed();
    }
    response.hover_hint(format!(
        "{name} gain: {}. Drag to adjust; Shift gives fine movement. The wheel and arrow keys \
         change {GAIN_STEP_DB} dB (Shift: {GAIN_FINE_STEP_DB} dB). Home silences, End sets \
         +{:.0} dB. Double-click for unity. Negative dB only reduces level.",
        db_text(db),
        range.maximum_db(),
    ))
}
