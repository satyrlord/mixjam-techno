//! The gain fader: its taper between level and rail position, the text a
//! level reads and is typed in, the rail, and its keyboard and accessibility
//! behavior.
use super::*;

const SILENCE_DB: f32 = -96.0;
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

pub(crate) fn gain_value(
    ui: &mut egui::Ui,
    rect: Rect,
    scale: f32,
    name: &str,
    gain: &mut f32,
) -> bool {
    ui.painter().rect_filled(rect, 3.0, roles().strip_face);
    let before = *gain;
    let db = amplitude_to_db(before);
    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt(("gain-value", name))
            .max_rect(rect),
        |ui| {
            ui.spacing_mut().interact_size = rect.size();
            ui.spacing_mut().button_padding = Vec2::ZERO;
            ui.style_mut().override_text_style = Some(egui::TextStyle::Body);
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(12.0 * scale));
            let widgets = &mut ui.style_mut().visuals.widgets;
            for widget in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
            ] {
                widget.bg_fill = Color32::TRANSPARENT;
                widget.weak_bg_fill = Color32::TRANSPARENT;
                widget.bg_stroke = Stroke::NONE;
                widget.fg_stroke.color = roles().legend;
            }
            // A focused value shows an edge before it is typed in.
            widgets.active.bg_stroke = Stroke::new(1.0 * scale, roles().legend);
            let id = ui.next_auto_id();
            let context = ui.ctx().clone();
            // DragValue needs a finite origin for relative edits. Keep silence's
            // text and parser independent from that origin, so passive focus and
            // explicit finite entry retain their exact meaning.
            let has_focus = context.memory(|memory| memory.has_focus(id));
            let relative_edit = ui.ctx().is_being_dragged(id)
                || ui.input(|input| {
                    use egui::accesskit::Action;
                    (input.key_pressed(egui::Key::ArrowUp)
                        || input.key_pressed(egui::Key::ArrowDown))
                        && has_focus
                        || input.num_accesskit_action_requests(id, Action::Increment) > 0
                        || input.num_accesskit_action_requests(id, Action::Decrement) > 0
                });
            // The edit text shows one decimal. Leaving the field without typing
            // commits that text, which must not round the saved amplitude.
            let typed_id = id.with("typed");
            let typed = context.data(|data| data.get_temp::<bool>(typed_id)) == Some(true)
                || has_focus
                    && ui.input(|input| {
                        input.events.iter().any(|event| {
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
                        })
                    });
            let shown = parse_db(&db_edit_text(db)).map(|shown| shown as f32);
            let response = ui.add(
                egui::DragValue::from_get_set(|value| {
                    if let Some(value) = value {
                        let value = value as f32;
                        let lowering_silence =
                            before == 0.0 && relative_edit && value <= SILENCE_DB;
                        let untouched_text = !typed && !relative_edit && Some(value) == shown;
                        if !lowering_silence
                            && !untouched_text
                            && value != db
                            && (value.is_finite() || value == f32::NEG_INFINITY)
                        {
                            *gain = db_to_amplitude(value.min(MAX_MIXER_GAIN_DB));
                        }
                    }
                    f64::from(if *gain == 0.0 {
                        SILENCE_DB
                    } else {
                        amplitude_to_db(*gain)
                    })
                })
                .range(f64::NEG_INFINITY..=f64::from(amplitude_to_db(4.0)))
                .clamp_existing_to_range(false)
                .speed(0.1)
                .update_while_editing(false)
                .custom_formatter(move |value, _| {
                    let value = if before == 0.0
                        && (value as f32 == SILENCE_DB
                            || relative_edit && value as f32 <= SILENCE_DB)
                    {
                        f32::NEG_INFINITY
                    } else {
                        value as f32
                    };
                    if context.memory(|memory| memory.has_focus(id)) {
                        db_edit_text(value)
                    } else {
                        db_text(value)
                    }
                })
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
            response.widget_info(|| {
                let mut info =
                    egui::WidgetInfo::drag_value(ui.is_enabled(), f64::from(db.max(SILENCE_DB)));
                info.label = Some(format!("{name} gain value"));
                info.current_text_value = Some(db_text(db));
                info
            });
            response.hover_hint(format!(
                "{name} gain: {}. Click to enter dB, then press Enter. Negative dB attenuates. \
                 Enter -inf for silence. Polarity stays unchanged.",
                db_text(db)
            ));
        },
    );
    before != *gain
}

pub(super) fn gain_rail(
    ui: &mut egui::Ui,
    art: Option<&Artwork>,
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
    if (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let fine = ui.input(|input| input.modifiers.shift);
        let fraction = if fine && response.dragged() {
            gain_fraction(*gain) - ui.input(|input| input.pointer.delta().y) / rect.height() * 0.1
        } else {
            (rect.bottom() - pos.y) / rect.height()
        };
        *gain = fraction_gain(fraction);
    }
    gain_keys(ui, &response, gain);
    let cap = Pos2::new(
        rect.center().x,
        egui::lerp(rect.bottom()..=rect.top(), gain_fraction(*gain)),
    );
    if let Some(art) = art {
        art.fader(ui, cap);
    } else {
        // A flat software cap on Studio, a molded one on Aero; both stay
        // inside the bitmap cap's 24 by 32 points.
        let size = if roles().material == theme::Material::Flat {
            Vec2::new(24.0, 16.0)
        } else {
            Vec2::new(24.0, 24.0)
        };
        theme::fader_cap(ui.painter(), cap, size * scale);
    }
    focus_ring(ui, &response, rect, scale);
    gain_semantics(ui, response, name, before, *gain);
    before != *gain
}

/// The edits every gain fader takes besides the pointer: the wheel, the
/// arrow keys, Home and End, the accessibility actions, and a double-click.
pub(crate) fn gain_keys(ui: &egui::Ui, response: &Response, gain: &mut f32) {
    let mut step = controls::wheel(ui, response);
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
            )
        });
        ui.input_mut(|input| {
            let modifiers = input.modifiers;
            if input.consume_key(modifiers, egui::Key::ArrowUp)
                || input.consume_key(modifiers, egui::Key::ArrowRight)
            {
                step = 1.0;
            }
            if input.consume_key(modifiers, egui::Key::ArrowDown)
                || input.consume_key(modifiers, egui::Key::ArrowLeft)
            {
                step = -1.0;
            }
            if input.consume_key(modifiers, egui::Key::Home) {
                *gain = 0.0;
            }
            if input.consume_key(modifiers, egui::Key::End) {
                *gain = db_to_amplitude(MAX_MIXER_GAIN_DB);
            }
        });
    }
    ui.input(|input| {
        use egui::accesskit::{Action, ActionData};
        for request in input.accesskit_action_requests(response.id, Action::SetValue) {
            if let Some(ActionData::NumericValue(value)) = request.data
                && value.is_finite()
            {
                *gain = if value <= f64::from(SILENCE_DB) {
                    0.0
                } else {
                    db_to_amplitude((value as f32).min(MAX_MIXER_GAIN_DB))
                };
            }
        }
        step += input.num_accesskit_action_requests(response.id, Action::Increment) as f32
            - input.num_accesskit_action_requests(response.id, Action::Decrement) as f32;
    });
    if step != 0.0 {
        if response.has_focus() {
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        let previous_db = amplitude_to_db(*gain);
        let db = if *gain == 0.0 {
            if step > 0.0 { -60.0 } else { f32::NEG_INFINITY }
        } else {
            previous_db
                + step
                    * if ui.input(|input| input.modifiers.shift) {
                        0.1
                    } else {
                        0.5
                    }
        };
        *gain = if step < 0.0 && previous_db >= SILENCE_DB && db <= SILENCE_DB {
            0.0
        } else {
            db_to_amplitude(db.min(MAX_MIXER_GAIN_DB))
        };
    }
    if response.double_clicked() {
        *gain = 1.0;
    }
}

/// A gain fader's accessible name, value, range, and hint: "{name} gain".
pub(crate) fn gain_semantics(
    ui: &egui::Ui,
    mut response: Response,
    name: &str,
    before: f32,
    gain: f32,
) -> Response {
    let db = amplitude_to_db(gain);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::slider(
            response.enabled(),
            f64::from(db.max(SILENCE_DB)),
            format!("{name} gain"),
        );
        info.current_text_value = Some(db_text(db));
        info
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::Action;
        node.set_min_numeric_value(f64::from(SILENCE_DB));
        node.set_max_numeric_value(f64::from(MAX_MIXER_GAIN_DB));
        node.set_numeric_value_step(0.5);
        node.add_action(Action::SetValue);
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
    });
    if before != gain {
        response.mark_changed();
    }
    response.hover_hint(format!(
        "{name} gain: {}. Drag to adjust; Shift gives fine movement. \
         The wheel and arrow keys change 0.5 dB \
         (Shift: 0.1 dB). Home silences, End sets +12 dB. Double-click for unity. Negative dB \
         only reduces level.",
        db_text(db)
    ))
}
