//! The pitch grid: 25 rows of the octave in view over the steps in view. A
//! tie draws as one joined bar, an accent as a bright top edge, and a slide
//! as a line to the next note. Notes outside the rows mark the grid's edge.
//! The grid turns clicks, keys, and drags into `pattern::NoteEdit`s.
use super::*;

/// The rows in view and where each one sits.
#[derive(Clone, Copy)]
struct Rows {
    top_note: i8,
    bottom_note: i8,
    /// The top of the grid's content.
    top: f32,
}

impl Rows {
    fn at(octave: i8, top: f32) -> Self {
        Self {
            top_note: ROWS + 12 * octave,
            bottom_note: -ROWS + 12 * octave,
            top,
        }
    }

    fn row_top(self, note: i8) -> f32 {
        self.top + f32::from((self.top_note - note) as u8) * ROW
    }

    fn contains(self, note: i8) -> bool {
        (self.bottom_note..=self.top_note).contains(&note)
    }
}

/// What the grid's keys asked for this frame.
#[derive(Default)]
struct GridKeys {
    toggle: bool,
    octave_change: i8,
    bar_change: i32,
}

pub(super) fn note_grid(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    view_bar: &mut usize,
    columns: Columns,
    editor: &mut Editor,
) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    form_style(&mut child);
    scroll_style(&mut child);
    // Open on the notes: the octave nearest their middle, scrolled to it.
    let centre = editor.centre_on_notes(state);
    let mut keys = GridKeys::default();
    let scroll = egui::ScrollArea::vertical()
        .id_salt("step-synth-notes")
        .auto_shrink([false, false])
        .max_height(rect.height())
        .show(&mut child, |ui| {
            let (content, _) = ui.allocate_exact_size(
                Vec2::new(rect.width() - 10.0, f32::from(2 * ROWS as u8 + 1) * ROW),
                Sense::hover(),
            );
            let rows = Rows::at(editor.octave, content.top());
            if let Some(note) = centre {
                ui.scroll_to_rect(
                    Rect::from_min_size(
                        Pos2::new(content.left(), rows.row_top(note)),
                        Vec2::new(content.width(), ROW),
                    ),
                    Some(egui::Align::Center),
                );
            }
            // The grid is one Tab stop. It holds keyboard focus and moves a
            // cursor among its cells, which stay pointer targets.
            let id = ui.id();
            let cell_id = move |note: i8, step: usize| id.with(("note", note, step));
            let cursor_id = ui.id().with("note-cursor");
            let mut cursor = ui
                .ctx()
                .data(|data| data.get_temp::<(i8, usize)>(cursor_id))
                .unwrap_or((12 * editor.octave, columns.first));
            cursor.1 = editor.step;
            cursor.0 = cursor.0.clamp(rows.bottom_note, rows.top_note);
            cursor.1 = cursor.1.clamp(columns.first, columns.last());
            let before = cursor;
            let grid = ui.interact(content, ui.id().with("pitch-grid"), Sense::FOCUSABLE);
            // Assistive technology can focus any cell directly.
            let requested = ui.input(|input| {
                use egui::accesskit::Action;
                let any = input.events.iter().any(|event| {
                    matches!(event, egui::Event::AccessKitActionRequest(request)
                        if request.action == Action::Focus)
                });
                any.then(|| {
                    (rows.bottom_note..=rows.top_note)
                        .flat_map(|note| {
                            (columns.first..columns.end()).map(move |step| (note, step))
                        })
                        .find(|(note, step)| {
                            input.has_accesskit_action_request(cell_id(*note, *step), Action::Focus)
                        })
                })
                .flatten()
            });
            if let Some(target) = requested {
                cursor = target;
                grid.request_focus();
            }
            if grid.has_focus() {
                keys = grid_keys(
                    ui,
                    &grid,
                    rows,
                    columns,
                    usize::from(state.length),
                    &mut cursor,
                );
            }
            let reveal = requested.is_some()
                || (grid.has_focus() && (cursor != before || grid.gained_focus()));
            grid.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Other,
                    true,
                    format!("Pitch grid: step {}, {}", cursor.1 + 1, semitones(cursor.0)),
                )
            });
            let painter = ui.painter().clone();
            let covers = pattern::covers(&state.steps);
            let modifiers = ui.input(|input| input.modifiers);
            let drag_id = ui.id().with("tie-drag");
            let (held, reset, pointer) = ui.input(|input| {
                use egui::PointerButton::Primary;
                let reset = input.key_pressed(egui::Key::Escape);
                let pointer = (input.pointer.primary_down()
                    || input.pointer.button_released(Primary))
                .then(|| input.pointer.latest_pos())
                .flatten();
                (
                    input.pointer.primary_down(),
                    reset || input.pointer.button_pressed(Primary),
                    pointer,
                )
            });
            let previous = ui.data_mut(|data| data.remove_temp::<(usize, Vec<Step>)>(drag_id));
            let mut tying = previous.filter(|_| !reset);
            let mut edit = None;
            for note in (rows.bottom_note..=rows.top_note).rev() {
                let y = rows.row_top(note);
                let in_scale = state.scale.contains(state.root, note);
                row_label(&painter, content.left(), y, note, in_scale);
                for (step, cover) in covers
                    .iter()
                    .enumerate()
                    .take(columns.end())
                    .skip(columns.first)
                {
                    let cell = columns
                        .span(step, step + 1, y + 1.0, y + 1.0 + CELL_HEIGHT)
                        .shrink2(Vec2::new(1.0, 0.0));
                    let held = cover.filter(|(_, pitch)| *pitch == note);
                    let response =
                        ui.interact(cell, cell_id(note, step), Sense::CLICK | Sense::DRAG);
                    if response.clicked() {
                        cursor = (note, step);
                        grid.request_focus();
                    }
                    let current = grid.has_focus() && cursor == (note, step);
                    let fill = if response.hovered() && held.is_none() {
                        roles().cell_hover
                    } else if in_scale {
                        roles().face
                    } else {
                        roles().cell_alternate
                    };
                    painter.rect_filled(cell, 2.0, fill);
                    painter.rect_stroke(
                        cell,
                        2.0,
                        Stroke::new(1.0_f32, roles().cell_edge),
                        egui::StrokeKind::Inside,
                    );
                    let label = format!("Step {}, {}", step + 1, semitones(note));
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            response.enabled(),
                            held.is_some(),
                            &label,
                        )
                    });
                    ui.ctx().accesskit_node_builder(response.id, |node| {
                        node.add_action(egui::accesskit::Action::Focus);
                        // The name keeps the semitones; the note name and
                        // the scale are the description.
                        node.set_description(format!(
                            "Note {}, {}.",
                            pattern::ROOTS[usize::from(note.rem_euclid(12) as u8)],
                            if in_scale { "in scale" } else { "not in scale" }
                        ));
                    });
                    focus(ui, &response, current, roles().legend);
                    if cursor == (note, step) && reveal {
                        ui.scroll_to_rect(cell, None);
                    }
                    // A drag that starts on a note's last step ties it on.
                    let start = controls::primary_drag_start(ui, &response);
                    if tying.is_none()
                        && start.is_some()
                        && let Some((head, _)) = held
                        && pattern::note_end(&state.steps, head) == step
                    {
                        tying = Some((head, state.steps.clone()));
                    }
                    let clicked = controls::primary_clicked(ui, &response.hover_hint(label))
                        || (current && keys.toggle);
                    if clicked {
                        edit = Some(match held {
                            Some((head, _)) if modifiers.alt => pattern::NoteEdit::Slide(head),
                            Some((head, _)) if modifiers.shift => pattern::NoteEdit::Accent(head),
                            Some((head, _)) => pattern::NoteEdit::Clear(head),
                            None => pattern::NoteEdit::Place { step, note },
                        });
                    }
                }
            }
            if let Some((head, snapshot)) = tying {
                if held {
                    ui.data_mut(|data| data.insert_temp(drag_id, (head, snapshot.clone())));
                }
                edit = pointer.map(|pointer| pattern::NoteEdit::Tie {
                    snapshot,
                    head,
                    end: columns.at(pointer.x).max(head),
                });
            }
            paint_notes(ui, &painter, content, state, rows, columns, editor);
            if let Some(edit) = edit {
                editor.edit(match edit {
                    pattern::NoteEdit::Tie { .. } => "the tie edit",
                    pattern::NoteEdit::Slide(_) => "the slide edit",
                    pattern::NoteEdit::Accent(_) => "the accent edit",
                    _ => "the note edit",
                });
                edit.apply(state);
            }
            ui.ctx()
                .data_mut(|data| data.insert_temp(cursor_id, cursor));
            editor.step = cursor.1;
        });
    // egui leaves the scroll bar unnamed; its vertical bar's id is the
    // area's id with axis 1.
    ui.ctx()
        .accesskit_node_builder(scroll.id.with(1_usize), |node| {
            node.set_label("Pitch grid scroll bar")
        });
    // Its drag surface is a Tab stop too, while the rows overflow.
    let surface = scroll.id.with("area");
    if ui
        .ctx()
        .read_response(surface)
        .is_some_and(|response| response.sense.is_focusable())
    {
        ui.ctx().accesskit_node_builder(surface, |node| {
            node.set_role(egui::accesskit::Role::ScrollView);
            node.set_label("Pitch grid drag to scroll");
        });
    }
    if keys.octave_change != 0 {
        editor.octave =
            (editor.octave + keys.octave_change).clamp(*OCTAVES.start(), *OCTAVES.end());
    }
    if keys.bar_change != 0 {
        // Only a move to another page changes the bar in view: the bar that
        // Copy to and Clear bar act on does not change out of sight.
        let bars = columns.bars();
        let bar = (*view_bar as i32 + keys.bar_change * bars as i32)
            .clamp(0, pattern::BARS as i32 - 1) as usize;
        if bar / bars != *view_bar / bars {
            *view_bar = bar;
        }
    }
}

/// A pitch as its accessible name says it, such as "+1 semitone".
fn semitones(note: i8) -> String {
    if note.abs() == 1 {
        format!("{note:+} semitone")
    } else {
        format!("{note:+} semitones")
    }
}

/// Read the focused grid's keys: arrows, Home, and End move the cursor,
/// Page Up and Page Down the octave, Ctrl with Left or Right the bar page,
/// and Space or Enter puts or clears the note. End goes to the last step in
/// view that plays, inside `length`.
fn grid_keys(
    ui: &mut egui::Ui,
    grid: &Response,
    rows: Rows,
    columns: Columns,
    length: usize,
    cursor: &mut (i8, usize),
) -> GridKeys {
    let mut keys = GridKeys::default();
    lock_arrows(ui, grid);
    let last = columns.last();
    let last_played = last.min(length.saturating_sub(1)).max(columns.first);
    ui.input_mut(|input| {
        let mut pressed = |key| input.consume_key(egui::Modifiers::NONE, key);
        if pressed(egui::Key::ArrowUp) {
            cursor.0 = (cursor.0 + 1).min(rows.top_note);
        }
        if pressed(egui::Key::ArrowDown) {
            cursor.0 = (cursor.0 - 1).max(rows.bottom_note);
        }
        if pressed(egui::Key::ArrowLeft) {
            cursor.1 = cursor.1.saturating_sub(1).max(columns.first);
        }
        if pressed(egui::Key::ArrowRight) {
            cursor.1 = (cursor.1 + 1).min(last);
        }
        if pressed(egui::Key::Home) {
            cursor.1 = columns.first;
        }
        if pressed(egui::Key::End) {
            cursor.1 = last_played;
        }
        if pressed(egui::Key::PageUp) {
            keys.octave_change = 1;
        }
        if pressed(egui::Key::PageDown) {
            keys.octave_change = -1;
        }
        keys.toggle = pressed(egui::Key::Enter) | pressed(egui::Key::Space);
        if input.consume_key(egui::Modifiers::COMMAND, egui::Key::ArrowLeft) {
            keys.bar_change = -1;
        }
        if input.consume_key(egui::Modifiers::COMMAND, egui::Key::ArrowRight) {
            keys.bar_change = 1;
        }
    });
    keys
}

/// A row's pitch in the gutter. The source pitch, 0, is in the board's
/// accent, with a tick at the gutter's edge.
fn row_label(painter: &egui::Painter, left: f32, y: f32, note: i8, in_scale: bool) {
    let accent = controls::Palette::Olive.accent();
    painter.text(
        Pos2::new(left + GUTTER - 8.0, y + ROW / 2.0),
        Align2::RIGHT_CENTER,
        if note == 0 {
            "0".into()
        } else {
            format!("{note:+}")
        },
        FontId::proportional(12.0),
        if note == 0 {
            accent
        } else if in_scale {
            roles().legend
        } else {
            roles().legend.gamma_multiply(0.6)
        },
    );
    if note == 0 {
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(left + GUTTER - 3.0, y + 2.0),
                Vec2::new(2.0, CELL_HEIGHT - 2.0),
            ),
            1.0,
            accent,
        );
    }
}

/// Bar and beat lines, then the notes over the cells, the dimmed steps past
/// the length, and the selection.
fn paint_notes(
    ui: &egui::Ui,
    painter: &egui::Painter,
    content: Rect,
    state: &StepSynthState,
    rows: Rows,
    columns: Columns,
    editor: &Editor,
) {
    let length = usize::from(state.length);
    for step in (columns.first..=columns.end()).step_by(4) {
        let x = columns.x(step);
        painter.line_segment(
            [Pos2::new(x, content.top()), Pos2::new(x, content.bottom())],
            Stroke::new(
                if step % BAR == 0 { 1.5 } else { 1.0_f32 },
                roles().grid_line,
            ),
        );
    }
    for (head, step) in state.steps.iter().enumerate() {
        let Trig::Note(note) = step.trig else {
            continue;
        };
        let end = pattern::note_end(&state.steps, head);
        if end < columns.first || head >= columns.end() {
            continue;
        }
        let from = head.max(columns.first);
        let to = (end + 1).min(columns.end());
        if !rows.contains(note) {
            // A note outside the rows marks the grid's top or bottom.
            if columns.contains(head) {
                let x = columns.x(head) + columns.width / 2.0;
                let edge = if note > rows.top_note {
                    ui.clip_rect().top().max(content.top())
                } else {
                    ui.clip_rect().bottom().min(content.bottom()) - 4.0
                };
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x - 6.0, edge), Vec2::new(12.0, 4.0)),
                    1.0,
                    roles().coral,
                );
            }
            continue;
        }
        let y = rows.row_top(note);
        let bar = columns
            .span(from, to, y + 1.0, y + 1.0 + CELL_HEIGHT)
            .shrink2(Vec2::new(1.0, 0.0));
        let color = if head >= length {
            roles().coral.gamma_multiply(0.45)
        } else {
            roles().coral
        };
        painter.rect_filled(bar, 3.0, color);
        painter.rect_stroke(
            bar,
            3.0,
            Stroke::new(1.0_f32, roles().coral.lerp_to_gamma(roles().legend, 0.4)),
            egui::StrokeKind::Inside,
        );
        if step.accent {
            painter.rect_filled(
                Rect::from_min_size(bar.min, Vec2::new(bar.width(), 3.0)),
                egui::CornerRadius {
                    nw: 3,
                    ne: 3,
                    sw: 0,
                    se: 0,
                },
                roles().legend,
            );
        }
        if !state.scale.contains(state.root, note) && columns.contains(head) {
            // An off-scale note keeps its cell, with a notched corner.
            let corner = bar.right_top();
            painter.add(egui::Shape::convex_polygon(
                vec![
                    corner + Vec2::new(-6.0, 1.0),
                    corner + Vec2::new(-1.0, 1.0),
                    corner + Vec2::new(-1.0, 6.0),
                ],
                roles().ink,
                Stroke::NONE,
            ));
        }
        if step.slide
            && let Some(Step {
                trig: Trig::Note(next),
                ..
            }) = state.steps.get(end + 1)
            && columns.contains(end + 1)
            && rows.contains(*next)
        {
            let from = Pos2::new(bar.right() - 4.0, y + 1.0 + CELL_HEIGHT / 2.0);
            let to = Pos2::new(
                columns.x(end + 1) + 4.0,
                rows.row_top(*next) + 1.0 + CELL_HEIGHT / 2.0,
            );
            painter.line_segment([from, to], Stroke::new(2.5_f32, roles().legend));
        }
    }
    // Steps past the length are dimmed.
    if length < columns.end() {
        let from = length.max(columns.first);
        painter.rect_filled(
            columns.span(from, columns.end(), content.top(), content.bottom()),
            0.0,
            roles().ink.gamma_multiply(0.35),
        );
        if columns.contains(length) || length == columns.end() {
            let x = columns.x(length);
            painter.line_segment(
                [Pos2::new(x, content.top()), Pos2::new(x, content.bottom())],
                Stroke::new(2.0_f32, roles().coral),
            );
        }
    }
    if let Some((from, to)) = editor.selection {
        let from = from.max(columns.first);
        let to = (to + 1).min(columns.end());
        if from < to {
            painter.rect_filled(
                columns.span(from, to, content.top(), content.bottom()),
                0.0,
                controls::Palette::Olive.accent().gamma_multiply(0.12),
            );
        }
    }
}
