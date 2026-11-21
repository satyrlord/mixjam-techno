//! The grid column: the bar strip, the step ruler with the octave keys, the
//! pitch grid, and the value lanes, from the top.
use super::*;

pub(super) fn grid_column(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    view_bar: &mut usize,
    editor: &mut Editor,
    playing: Option<usize>,
    available: bool,
) -> Action {
    // The edited bar is the one picked last; the grid shows its page of bars.
    *view_bar = (*view_bar).min(pattern::BARS - 1);
    let columns = Columns::fit(
        rect.left() + GUTTER,
        rect.width() - GUTTER - 10.0,
        *view_bar,
    );
    let action = bar_strip(
        ui,
        Rect::from_min_size(rect.min, Vec2::new(rect.width(), 28.0)),
        state,
        view_bar,
        columns.bars(),
        editor,
        playing,
        available,
    );
    let ruler = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + 36.0),
        Pos2::new(rect.right(), rect.top() + 58.0),
    );
    let lanes = Rect::from_min_max(
        // A short panel keeps more of its height for the pitch rows.
        Pos2::new(
            rect.left(),
            rect.bottom()
                - if rect.height() < 320.0 {
                    72.0
                } else {
                    LANES_HEIGHT
                },
        ),
        rect.max,
    );
    let grid = Rect::from_min_max(
        Pos2::new(rect.left(), ruler.bottom() + 4.0),
        Pos2::new(rect.right(), lanes.top() - 6.0),
    );
    step_ruler(ui, ruler, state, columns, editor, playing);
    if columns.bars() > 1 {
        // The viewed bar's mark over the ruler, for Copy bar and Clear bar.
        let first = *view_bar * BAR;
        ui.painter().rect_filled(
            columns
                .span(first, first + BAR, ruler.top() - 4.0, ruler.top() - 1.0)
                .shrink2(Vec2::new(1.0, 0.0)),
            1.0,
            controls::Palette::Olive.accent(),
        );
    }
    octave_keys(ui, rect.left(), ruler.center().y, editor);
    note_grid(ui, grid, state, view_bar, columns, editor);
    value_lanes(ui, lanes, state, columns, editor);
    action
}

/// The octave in view between its − and + keys, in the gutter over the rows.
fn octave_keys(ui: &mut egui::Ui, left: f32, y: f32, editor: &mut Editor) {
    for (x, text, name, change) in [(0.0, "−", "Octave down", -1), (32.0, "+", "Octave up", 1)] {
        let enabled = OCTAVES.contains(&(editor.octave + change));
        if key(
            ui,
            Rect::from_min_size(Pos2::new(left + x, y - 11.0), Vec2::new(18.0, 22.0)),
            Key {
                enabled,
                ..Key::new(name, text)
            },
        )
        .hover_hint(name)
        .clicked()
        {
            editor.octave += change;
        }
    }
    lettering(
        ui,
        Pos2::new(left + 25.0, y),
        &if editor.octave == 0 {
            "0".to_owned()
        } else {
            format!("{:+}", editor.octave)
        },
        11.0,
        roles().legend,
    );
}

/// The step ruler: step numbers, the end marker, the playing step, and the
/// selected range, which a drag along it sets.
fn step_ruler(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    columns: Columns,
    editor: &mut Editor,
    playing: Option<usize>,
) {
    let length = usize::from(state.length);
    let area = Rect::from_min_max(
        Pos2::new(columns.left, rect.top()),
        Pos2::new(columns.x(columns.end()), rect.bottom()),
    );
    let response = ui.interact(area, ui.id().with("step-ruler"), Sense::click_and_drag());
    let start = controls::primary_drag_start(ui, &response);
    if response.clicked() || start.is_some() {
        response.request_focus();
    }
    if response.has_focus() {
        step_keys(
            ui,
            &response,
            columns,
            usize::from(state.length),
            &mut editor.step,
        );
        let step = editor.step;
        ui.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
            {
                editor.selection = Some((step, step));
            }
            for (key, next) in [
                (
                    egui::Key::ArrowLeft,
                    step.saturating_sub(1).max(columns.first),
                ),
                (egui::Key::ArrowRight, (step + 1).min(columns.last())),
            ] {
                if input.consume_key(egui::Modifiers::SHIFT, key) {
                    let anchor = editor
                        .selection
                        .map_or(step, |(from, to)| if step == from { to } else { from });
                    editor.step = next;
                    editor.selection = Some((anchor.min(next), anchor.max(next)));
                }
            }
        });
    }
    let anchor_id = response.id.with("anchor");
    if let Some(origin) = start {
        let step = columns.at(origin.x);
        editor.step = step;
        ui.ctx().data_mut(|data| data.insert_temp(anchor_id, step));
        editor.selection = Some((step, step));
    }
    let dragging = controls::primary_drag_or_stop(ui, &response);
    if dragging
        && let Some(pointer) = controls::drag_position(ui, &response)
        && let Some(anchor) = ui.ctx().data(|data| data.get_temp::<usize>(anchor_id))
    {
        let step = columns.at(pointer.x);
        editor.step = step;
        editor.selection = Some((anchor.min(step), anchor.max(step)));
    }
    if !dragging
        && controls::primary_clicked(ui, &response)
        && response.interact_pointer_pos().is_some()
    {
        editor.selection = None;
    }
    if ui.input(|input| input.key_pressed(egui::Key::Escape)) && editor.selection.is_some() {
        editor.selection = None;
    }
    name_region(response, "Step ruler").hover_hint(
        "Drag to select steps; lane edits then apply to all of them. Click to clear the selection.",
    );
    for step in columns.first..columns.end() {
        let cell = columns
            .span(step, step + 1, rect.top(), rect.bottom())
            .shrink2(Vec2::new(1.0, 0.0));
        if editor.selected(step) {
            ui.painter().rect_filled(
                cell,
                2.0,
                controls::Palette::Olive.accent().gamma_multiply(0.45),
            );
        }
        if playing == Some(step) {
            ui.painter().rect_filled(cell, 2.0, roles().coral);
        }
        let beat = step % 4 == 0;
        let color = if step >= length {
            roles().legend.gamma_multiply(0.35)
        } else if beat {
            roles().legend
        } else {
            roles().legend.gamma_multiply(0.7)
        };
        lettering(ui, cell.center(), &format!("{}", step + 1), 11.0, color);
    }
    // The end marker sits on the boundary after the last step.
    if (columns.first..=columns.end()).contains(&length) {
        let x = columns.x(length);
        let handle = Rect::from_center_size(
            Pos2::new(x, rect.center().y),
            Vec2::new(10.0, rect.height()),
        );
        let marker = ui.interact(handle, ui.id().with("length-end"), Sense::DRAG);
        let dragged = controls::primary_drag_or_stop(ui, &marker);
        if dragged && let Some(pointer) = controls::drag_position(ui, &marker) {
            editor.edit("the length change");
            state.length = pattern::clamp_length(columns.boundary(pointer.x) as i64);
        }
        let color = if marker.hovered() || dragged {
            roles().legend
        } else {
            roles().coral
        };
        ui.painter().rect_filled(
            Rect::from_center_size(Pos2::new(x, rect.center().y), Vec2::new(4.0, rect.height())),
            2.0,
            color,
        );
        marker
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
            .hover_hint("Drag to set the pattern length.");
    }
}

/// Move the shared step cursor without leaving a focused grid control. End
/// goes to the last step in view that plays, inside `length`.
pub(super) fn step_keys(
    ui: &mut egui::Ui,
    response: &Response,
    columns: Columns,
    length: usize,
    step: &mut usize,
) {
    *step = (*step).clamp(columns.first, columns.last());
    lock_arrows(ui, response);
    ui.input_mut(|input| {
        if !input.modifiers.shift {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                *step = step.saturating_sub(1).max(columns.first);
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                *step = (*step + 1).min(columns.last());
            }
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
            *step = columns.first;
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
            *step = columns
                .last()
                .min(length.saturating_sub(1))
                .max(columns.first);
        }
    });
}
