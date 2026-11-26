//! The Accent and Slide flag rows and the Velocity bars. A click toggles a
//! flag and a drag paints it; a drag on Velocity draws the level. In a
//! selection, a flag edit sets every selected note, and a level edit shifts
//! each by the same amount, or with Shift sets each to the same level (see
//! `pattern::set_lane`).
use super::*;
use pattern::Lane;

pub(super) fn value_lanes(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    columns: Columns,
    editor: &mut Editor,
) {
    // A short panel gets slimmer flag rows.
    let flag = if rect.height() < LANES_HEIGHT {
        18.0
    } else {
        22.0
    };
    for (lane, top, height) in [
        (Lane::Accent, rect.top(), flag),
        (Lane::Slide, rect.top() + flag + 2.0, flag),
        (
            Lane::Velocity,
            rect.top() + 2.0 * flag + 4.0,
            rect.height() - 2.0 * flag - 4.0,
        ),
    ] {
        let row = Rect::from_min_max(
            Pos2::new(columns.left, top),
            Pos2::new(columns.x(columns.end()), top + height),
        );
        value_lane(ui, rect.left(), row, lane, state, columns, editor);
    }
}

fn value_lane(
    ui: &mut egui::Ui,
    left: f32,
    row: Rect,
    lane: Lane,
    state: &mut StepSynthState,
    columns: Columns,
    editor: &mut Editor,
) {
    let name = lane.label();
    let top = row.top();
    controls::well(ui, row);
    left_text(
        ui,
        Pos2::new(left + 2.0, top + 11.0),
        name,
        11.0,
        roles().legend,
    );
    let mixed = pattern::lane_mixed(state, lane, editor.selection);
    if mixed {
        // Selected notes differ here; an edit shifts them together.
        ui.painter().circle_filled(
            Pos2::new(columns.left - 6.0, top + 11.0),
            3.0,
            roles().coral,
        );
    }
    // The row holds the keyboard focus; each step's cell takes the pointer
    // and is a control of its own for assistive tools and agents.
    let row_id = ui.id().with(("lane", name));
    let response = ui.interact(row, row_id, Sense::focusable_noninteractive());
    if response.has_focus() {
        step_keys(
            ui,
            &response,
            columns,
            usize::from(state.length),
            &mut editor.step,
        );
    }
    let step = editor.step;
    let current = lane.value(&state.steps[step]);
    let draft = lane_draft(ui, &response, lane, current);
    if draft != current && pattern::is_note(&state.steps[step]) {
        let targets = pattern::lane_targets(state, editor.selection, step);
        pattern::set_lane(state, lane, &targets, Some(current), draft);
    }
    response.widget_info(|| {
        let value = lane.value(&state.steps[step]);
        if lane.is_flag() {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                true,
                value != 0,
                format!("{name} lane"),
            )
        } else {
            egui::WidgetInfo::slider(true, f64::from(value), format!("{name} lane"))
        }
    });
    if mixed {
        // The dot has no control of its own: the row says it for agents.
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_description("Mixed: the selected notes differ in this lane.")
        });
    }
    focus(ui, &response, response.has_focus(), roles().legend);
    let hint = match lane {
        Lane::Accent => "Click a note's dot to accent it, or drag along the row to paint.",
        Lane::Slide => {
            "Click a note's dot to slide it into the next note, or drag along the row to paint."
        }
        Lane::Velocity => {
            "Drag up or down to set each note's level. In a selection, Shift-drag gives every selected note the same level."
        }
    };
    let before = state.steps.clone();
    let mut active = None;
    for step in columns.first..columns.end() {
        let cell = columns.span(step, step + 1, row.top(), row.bottom());
        let cell = ui
            .interact(cell, row_id.with(step), Sense::click_and_drag())
            .hover_hint(hint);
        let start = controls::primary_drag_start(ui, &cell);
        if cell.clicked() || start.is_some() {
            response.request_focus();
        }
        if controls::primary_down_on(ui, &cell) {
            if let Some(pointer) = controls::drag_position(ui, &cell) {
                active = Some((Some(pointer), step));
            }
        } else if controls::primary_clicked(ui, &cell) {
            active = Some((cell.interact_pointer_pos(), step));
        }
        step_cell(ui, &cell, lane, step, state, editor);
    }
    lane_pointer(ui, row_id, active, row, lane, state, columns, editor);
    if state.steps != before || draft != current {
        editor.edit(match lane {
            Lane::Accent => "the accent edit",
            Lane::Slide => "the slide edit",
            Lane::Velocity => "the velocity edit",
        });
    }
    paint_lane(ui, row, lane, state, columns, editor);
}

/// One step's cell: its accessible name and value, and the assistive
/// requests that set it.
fn step_cell(
    ui: &egui::Ui,
    cell: &Response,
    lane: Lane,
    step: usize,
    state: &mut StepSynthState,
    editor: &Editor,
) {
    let is_note = pattern::is_note(&state.steps[step]);
    let current = lane.value(&state.steps[step]);
    if !lane.is_flag() && is_note {
        let mut draft = i32::from(current);
        ui.input(|input| {
            use egui::accesskit::{Action, ActionData};
            draft += input.num_accesskit_action_requests(cell.id, Action::Increment) as i32
                - input.num_accesskit_action_requests(cell.id, Action::Decrement) as i32;
            for request in input.accesskit_action_requests(cell.id, Action::SetValue) {
                if let Some(ActionData::NumericValue(value)) = request.data
                    && value.is_finite()
                {
                    draft = value.round() as i32;
                }
            }
        });
        let range = lane.range();
        let draft = draft.clamp(*range.start(), *range.end()) as u8;
        if draft != current {
            let targets = pattern::lane_targets(state, editor.selection, step);
            pattern::set_lane(state, lane, &targets, Some(current), draft);
        }
    }
    let value = lane.value(&state.steps[step]);
    let label = format!("{}, step {}", lane.label(), step + 1);
    cell.widget_info(|| {
        if lane.is_flag() {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, is_note, value != 0, &label)
        } else {
            egui::WidgetInfo::slider(is_note, f64::from(value), &label)
        }
    });
}

/// The lane's value after this frame's keys and assistive requests: Up and
/// Down step it, and Space, Enter, or a click toggles a flag.
fn lane_draft(ui: &mut egui::Ui, response: &Response, lane: Lane, current: u8) -> u8 {
    let mut draft = i32::from(current);
    if response.has_focus() {
        ui.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                draft += 1;
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                draft -= 1;
            }
            if lane.is_flag()
                && (input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
            {
                draft = i32::from(current == 0);
            }
        });
    }
    ui.input(|input| {
        use egui::accesskit::{Action, ActionData};
        draft += input.num_accesskit_action_requests(response.id, Action::Increment) as i32
            - input.num_accesskit_action_requests(response.id, Action::Decrement) as i32;
        if lane.is_flag() && input.has_accesskit_action_request(response.id, Action::Click) {
            draft = i32::from(current == 0);
        }
        for request in input.accesskit_action_requests(response.id, Action::SetValue) {
            if let Some(ActionData::NumericValue(value)) = request.data
                && value.is_finite()
            {
                draft = value.round() as i32;
            }
        }
    });
    let range = lane.range();
    draft.clamp(*range.start(), *range.end()) as u8
}

/// A press or drag along the lane: a flag paints the value its first note
/// toggled to, and Velocity draws the level under the pointer. A fast drag
/// fills the steps it skipped between two frames. Shift on Velocity gives
/// every selected note the drawn level instead of shifting them together.
#[allow(clippy::too_many_arguments)]
fn lane_pointer(
    ui: &egui::Ui,
    row_id: egui::Id,
    active: Option<(Option<Pos2>, usize)>,
    row: Rect,
    lane: Lane,
    state: &mut StepSynthState,
    columns: Columns,
    editor: &mut Editor,
) {
    let paint_id = row_id.with("paint");
    let last_id = row_id.with("last");
    let Some((pointer, cell)) = active else {
        ui.ctx().data_mut(|data| {
            data.remove::<bool>(paint_id);
            data.remove::<(usize, u8)>(last_id);
        });
        return;
    };
    let Some(pointer) = pointer else {
        // An assistive click has no pointer: it toggles the cell's flag.
        editor.step = cell;
        if lane.is_flag() && pattern::is_note(&state.steps[cell]) {
            let targets = pattern::lane_targets(state, editor.selection, cell);
            let next = u8::from(lane.value(&state.steps[cell]) == 0);
            pattern::set_lane(state, lane, &targets, Some(0), next);
        }
        return;
    };
    let step = columns.at(pointer.x);
    editor.step = step;
    let level = ((row.bottom() - 2.0 - pointer.y) / (row.height() - 4.0) * 100.0)
        .round()
        .clamp(1.0, 100.0) as u8;
    let (last, last_level) = ui
        .ctx()
        .data(|data| data.get_temp::<(usize, u8)>(last_id))
        .unwrap_or((step, level));
    ui.ctx()
        .data_mut(|data| data.insert_temp(last_id, (step, level)));
    // In a selection one edit already reaches every selected note.
    let steps = if pattern::in_selection(editor.selection, step) {
        step..=step
    } else {
        last.min(step)..=last.max(step)
    };
    let absolute = ui.input(|input| input.modifiers.shift);
    for at in steps {
        let is_note = pattern::is_note(&state.steps[at]);
        let targets = pattern::lane_targets(state, editor.selection, at);
        if lane.is_flag() {
            let paint = match ui.ctx().data(|data| data.get_temp::<bool>(paint_id)) {
                Some(paint) => Some(paint),
                None if is_note => {
                    let next = lane.value(&state.steps[at]) == 0;
                    ui.ctx().data_mut(|data| data.insert_temp(paint_id, next));
                    Some(next)
                }
                None => None,
            };
            if let Some(paint) = paint {
                pattern::set_lane(state, lane, &targets, Some(0), u8::from(paint));
            }
        } else if is_note {
            let level = if last == step {
                level
            } else {
                let t = (at as f32 - last as f32) / (step as f32 - last as f32);
                (f32::from(last_level) + (f32::from(level) - f32::from(last_level)) * t).round()
                    as u8
            };
            let from = (!absolute).then_some(state.steps[at].velocity);
            pattern::set_lane(state, lane, &targets, from, level);
        }
    }
}

/// Each step's dot or level bar, with the selection behind it.
fn paint_lane(
    ui: &egui::Ui,
    row: Rect,
    lane: Lane,
    state: &StepSynthState,
    columns: Columns,
    editor: &Editor,
) {
    let length = usize::from(state.length);
    let shown = state
        .steps
        .iter()
        .copied()
        .enumerate()
        .take(columns.end())
        .skip(columns.first);
    for (step, note) in shown {
        let cell = columns
            .span(step, step + 1, row.top(), row.bottom())
            .shrink2(Vec2::new(2.0, 2.0));
        if editor.selected(step) {
            ui.painter().rect_filled(
                cell.expand2(Vec2::new(1.0, 1.0)),
                1.0,
                controls::Palette::Olive.accent().gamma_multiply(0.3),
            );
        }
        let is_note = pattern::is_note(&note);
        let alpha = if step >= length { 0.45 } else { 1.0 };
        if lane.is_flag() {
            if is_note && lane.value(&note) != 0 {
                ui.painter()
                    .circle_filled(cell.center(), 5.0, roles().coral.gamma_multiply(alpha));
            } else if is_note {
                ui.painter().circle_stroke(
                    cell.center(),
                    4.5,
                    Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.6 * alpha)),
                );
            } else {
                ui.painter()
                    .circle_filled(cell.center(), 1.5, roles().legend.gamma_multiply(0.25));
            }
        } else if is_note {
            let level = f32::from(note.velocity) / 100.0;
            let bar = Rect::from_min_max(
                Pos2::new(cell.left() + 2.0, cell.bottom() - cell.height() * level),
                Pos2::new(cell.right() - 2.0, cell.bottom()),
            );
            ui.painter().rect_filled(
                bar,
                1.0,
                controls::Palette::Olive.accent().gamma_multiply(alpha),
            );
        }
    }
}
