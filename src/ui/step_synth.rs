//! Step synth body. Shared hardware shell and controls own presentation.
use super::controls::{CREAM, FACE};
use super::*;
const WELL: Color32 = FACE;
const EDGE: Color32 = Color32::from_rgb(100, 105, 101);
/// Beat-group lines, idle cell edges, and the alternate pitch rows of the grid.
const GRID_LINE: Color32 = Color32::from_rgb(120, 128, 108);
const CELL_EDGE: Color32 = Color32::from_rgb(108, 116, 98);
const CELL_HOVER: Color32 = Color32::from_rgb(62, 68, 53);
const CELL_ALTERNATE: Color32 = Color32::from_rgb(29, 33, 28);
pub(super) struct Source<'a> {
    pub name: &'a str,
    pub available: bool,
    pub missing: bool,
    pub explicit_file: bool,
}

pub(super) struct Feedback<'a> {
    pub message: Option<&'a str>,
    pub error: bool,
}

pub(super) enum Action {
    None,
    ChooseSource,
    PaletteSource,
    Listen,
    SaveSequence,
    SavePattern,
    LoadPattern,
    Details,
    Help,
}

pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    feedback: Feedback<'_>,
    state: &mut StepSynthState,
    view_bar: &mut usize,
    show_effects: &mut bool,
) -> Result<Action, String> {
    let mut action = Action::None;
    let response = dialog::tool(
        ctx,
        "Step synth",
        open,
        Vec2::new(820.0, 552.0),
        controls::Palette::Olive,
        true,
        |ui| {
            let scale = 1.0;
            let (bounds, _) = ui.allocate_exact_size(Vec2::new(820.0, 552.0), Sense::hover());
            let point = |x: f32, y: f32| bounds.min + Vec2::new(x, y - 48.0);
            let rect = |x, y, w, h| Rect::from_min_size(point(x, y), Vec2::new(w, h));
            let mut panel = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
            controls::inset(&panel, rect(12.0, 48.0, 796.0, 28.0));
            controls::inset(&panel, rect(12.0, 82.0, 796.0, 70.0));
            let missing = source.missing && !source.explicit_file;
            let available = source.available && !missing;
            let source_rect = rect(18.0, 48.0, 222.0, 28.0);
            well(&panel, source_rect, scale);
            let label = panel.put(
                source_rect.shrink2(Vec2::new(8.0, 4.0) * scale),
                egui::Label::new(
                    egui::RichText::new(source.name)
                        .font(FontId::proportional(12.5 * scale))
                        .color(CREAM),
                )
                .truncate(),
            );
            named(&label, egui::WidgetType::Label, "Step synth source sound");
            ctx.accesskit_node_builder(label.id, |node| {
                node.set_label("Step synth source sound");
                node.set_value(source.name);
            });
            label.on_hover_text(source.name);
            for (x, width, text, next) in [
                (252.0, 164.0, "Choose source file", Action::ChooseSource),
                (426.0, 154.0, "Use palette sound", Action::PaletteSource),
            ] {
                if button(
                    &mut panel,
                    rect(x, 48.0, width, 28.0),
                    text,
                    true,
                    false,
                    scale,
                )
                .clicked()
                {
                    action = next;
                }
            }
            left_text(
                &panel,
                point(598.0, 62.0),
                if source.explicit_file {
                    "File source"
                } else {
                    "Palette sound"
                },
                12.0 * scale,
                CREAM,
            );
            left_text(&panel, point(20.0, 100.0), "View bar", 13.0 * scale, CREAM);
            for bar in 0..4 {
                let selected = *view_bar == bar;
                let response = select_button(
                    &mut panel,
                    rect(94.0 + bar as f32 * 38.0, 86.0, 32.0, 28.0),
                    &format!("{}", bar + 1),
                    &format!("View bar {}", bar + 1),
                    selected,
                    scale,
                );
                if response.clicked() {
                    *view_bar = bar;
                }
                toggle(
                    &mut panel,
                    point(278.0 + bar as f32 * 130.0, 100.0),
                    point(308.0 + bar as f32 * 130.0, 100.0),
                    scale,
                    &format!("Active {}", bar + 1),
                    &mut state.active_bars[bar],
                );
            }
            toggle(
                &mut panel,
                point(32.0, 134.0),
                point(56.0, 134.0),
                scale,
                "1/16 cut",
                &mut state.cut_sixteenth,
            );
            toggle(
                &mut panel,
                point(178.0, 134.0),
                point(202.0, 134.0),
                scale,
                "Loop source",
                &mut state.loop_source,
            );
            left_text(&panel, point(330.0, 134.0), "BPM", 13.0 * scale, CREAM);
            panel.scope_builder(
                egui::UiBuilder::new().max_rect(rect(370.0, 120.0, 76.0, 28.0)),
                |ui| {
                    form_style(ui, scale);
                    name_control(
                        bpm_picker(ui, "step_synth_bpm", &mut state.bpm),
                        "Step synth BPM",
                    );
                },
            );
            if button(
                &mut panel,
                rect(468.0, 120.0, 96.0, 28.0),
                "Clear bar",
                true,
                false,
                scale,
            )
            .on_hover_text("Clear the notes in the viewed bar.")
            .clicked()
            {
                let start = *view_bar * 16;
                if let Some(steps) = state.steps.get_mut(start..start + 16) {
                    steps.fill(None);
                }
            }
            let main_rect = rect(16.0, 160.0, 788.0, 344.0);
            controls::inset(&panel, main_rect);
            if *show_effects {
                left_text(
                    &panel,
                    point(30.0, 177.0),
                    "Sequence effects",
                    13.0 * scale,
                    CREAM,
                );
                let effects_rect = rect(28.0, 198.0, 764.0, 294.0);
                let mut form = panel.new_child(
                    egui::UiBuilder::new()
                        .max_rect(effects_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                form.set_clip_rect(effects_rect.intersect(panel.clip_rect()));
                form_style(&mut form, scale);
                form.style_mut().spacing.scroll.floating = false;
                form.style_mut().spacing.scroll.bar_width = 6.0 * scale;
                form.style_mut().spacing.scroll.bar_inner_margin = 4.0 * scale;
                form.style_mut().spacing.scroll.bar_outer_margin = 0.0;
                egui::ScrollArea::vertical()
                    .id_salt("step-synth-effects")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .auto_shrink([false, false])
                    .max_height(effects_rect.height())
                    .show(&mut form, |ui| effects_controls(ui, &mut state.effects));
            } else {
                for step in 0..16 {
                    lettering(
                        &panel,
                        point(83.0 + step as f32 * 46.0, 174.0),
                        &format!("{}", step + 1),
                        12.0 * scale,
                        CREAM,
                        false,
                    );
                }
                left_text(&panel, point(25.0, 174.0), "Pitch", 11.0 * scale, CREAM);
                note_grid(
                    &mut panel,
                    rect(20.0, 190.0, 784.0, 306.0),
                    scale,
                    state,
                    *view_bar,
                );
            }
            let active = state.active_bars.iter().any(|active| *active);
            for (x, width, text, next, enabled, ivory) in [
                (
                    16.0,
                    156.0,
                    "Listen sequence",
                    Action::Listen,
                    available && active,
                    false,
                ),
                (
                    182.0,
                    156.0,
                    "Save sequence",
                    Action::SaveSequence,
                    available && active,
                    true,
                ),
                (
                    470.0,
                    160.0,
                    "Save pattern",
                    Action::SavePattern,
                    true,
                    false,
                ),
                (
                    640.0,
                    164.0,
                    "Load pattern",
                    Action::LoadPattern,
                    true,
                    false,
                ),
            ] {
                let response = button(
                    &mut panel,
                    rect(x, 516.0, width, 28.0),
                    text,
                    enabled,
                    ivory,
                    scale,
                );
                let hint = match text {
                    "Listen sequence" | "Save sequence" if !active => {
                        "Switch on at least one Active bar."
                    }
                    "Listen sequence" | "Save sequence" if !available => {
                        "Choose an available source file or palette sound."
                    }
                    "Save sequence" => "Save a WAV and add it to Step synth.",
                    "Save pattern" => "Save the editable pattern as JSON.",
                    "Load pattern" => "Load an editable JSON pattern.",
                    _ => "Listen to the active bars in sequence.",
                };
                let response = if enabled {
                    response.on_hover_text(hint)
                } else {
                    response.on_disabled_hover_text(hint)
                };
                if response.clicked() {
                    action = next;
                }
            }
            if keyed_button(
                &mut panel,
                rect(348.0, 516.0, 112.0, 28.0),
                "pane",
                if *show_effects {
                    "Note grid"
                } else {
                    "Effects"
                },
                true,
                false,
                scale,
            )
            .clicked()
            {
                *show_effects = !*show_effects;
            }
            let status = feedback.message.unwrap_or(if missing {
                "Missing audio. Choose a source file or open Import details."
            } else if !source.available {
                "Choose a source file or select a sound in the palette."
            } else if !active {
                "Switch on at least one Active bar."
            } else if *show_effects {
                "Adjust sequence effects. Note grid returns to the notes."
            } else {
                "Click a note to set it. Click it again to clear it."
            });
            status_footer(
                &mut panel,
                rect(8.0, 564.0, 804.0, 30.0),
                status,
                feedback.error,
                missing,
                scale,
            );
            if missing
                && button(
                    &mut panel,
                    rect(676.0, 566.0, 128.0, 26.0),
                    "Import details",
                    true,
                    false,
                    scale,
                )
                .clicked()
            {
                action = Action::Details;
            }
        },
    );
    if response.help {
        action = Action::Help;
    }
    Ok(action)
}
fn note_grid(ui: &mut egui::Ui, rect: Rect, scale: f32, state: &mut StepSynthState, bar: usize) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    form_style(&mut child, scale);
    child.style_mut().spacing.scroll.floating = false;
    child.style_mut().spacing.scroll.bar_width = 6.0 * scale;
    child.style_mut().spacing.scroll.bar_inner_margin = 4.0 * scale;
    child.style_mut().spacing.scroll.bar_outer_margin = 0.0;
    egui::ScrollArea::vertical()
        .id_salt("step-synth-notes")
        .auto_shrink([false, false])
        .max_height(rect.height())
        .show(&mut child, |ui| {
            let (content, _) =
                ui.allocate_exact_size(Vec2::new(774.0, 400.0) * scale, Sense::hover());
            for group in 1..4 {
                let x = content.left() + (40.0 + group as f32 * 184.0) * scale;
                ui.painter().line_segment(
                    [Pos2::new(x, content.top()), Pos2::new(x, content.bottom())],
                    Stroke::new(1.0_f32, GRID_LINE),
                );
            }
            // The grid is one Tab stop. It holds keyboard focus and moves a
            // cursor among its 400 cells, which stay pointer targets.
            let cell_id = |note: i8, step: usize| ui.id().with(("note", bar, note, step));
            let cursor_id = ui.id().with("note-cursor");
            let mut cursor = ui
                .ctx()
                .data(|data| data.get_temp::<(i8, usize)>(cursor_id))
                .filter(|(note, step)| (-12..=12).contains(note) && *step < 16)
                .unwrap_or((0, 0));
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
                    (-12_i8..=12)
                        .flat_map(|note| (0..16).map(move |step| (note, step)))
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
            let mut toggle = false;
            if grid.has_focus() {
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        grid.id,
                        egui::EventFilter {
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            ..Default::default()
                        },
                    )
                });
                ui.input_mut(|input| {
                    let mut pressed = |key| input.consume_key(egui::Modifiers::NONE, key);
                    if pressed(egui::Key::ArrowUp) {
                        cursor.0 = (cursor.0 + 1).min(12);
                    }
                    if pressed(egui::Key::ArrowDown) {
                        cursor.0 = (cursor.0 - 1).max(-12);
                    }
                    if pressed(egui::Key::ArrowLeft) {
                        cursor.1 = cursor.1.saturating_sub(1);
                    }
                    if pressed(egui::Key::ArrowRight) {
                        cursor.1 = (cursor.1 + 1).min(15);
                    }
                    if pressed(egui::Key::Home) {
                        cursor.1 = 0;
                    }
                    if pressed(egui::Key::End) {
                        cursor.1 = 15;
                    }
                    toggle = pressed(egui::Key::Enter) | pressed(egui::Key::Space);
                });
            }
            let reveal = grid.has_focus() && (cursor != before || grid.gained_focus());
            grid.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Other,
                    true,
                    format!(
                        "Pitch grid: step {}, {:+} semitones",
                        cursor.1 + 1,
                        cursor.0
                    ),
                )
            });
            for (row, note) in (-12_i8..=12).rev().enumerate() {
                let y = content.top() + row as f32 * 16.0 * scale;
                ui.painter().text(
                    Pos2::new(content.left() + 32.0 * scale, y + 8.0 * scale),
                    Align2::RIGHT_CENTER,
                    if note == 0 {
                        "A  0".into()
                    } else {
                        format!("{note:+}")
                    },
                    FontId::proportional(12.0 * scale),
                    CREAM,
                );
                for step in 0..16 {
                    let index = bar * 16 + step;
                    let selected = state
                        .steps
                        .get(index)
                        .is_some_and(|value| *value == Some(note));
                    let cell = Rect::from_min_size(
                        Pos2::new(
                            content.left() + (42.0 + step as f32 * 46.0) * scale,
                            y + scale,
                        ),
                        Vec2::new(42.0, 14.0) * scale,
                    );
                    let response = ui.interact(cell, cell_id(note, step), Sense::CLICK);
                    if response.clicked() {
                        cursor = (note, step);
                    }
                    let current = grid.has_focus() && cursor == (note, step);
                    let fill = if selected {
                        CORAL
                    } else if response.hovered() {
                        CELL_HOVER
                    } else if matches!((note + 24) % 12, 0 | 2 | 3 | 5 | 7 | 8 | 10) {
                        FACE
                    } else {
                        CELL_ALTERNATE
                    };
                    ui.painter().rect_filled(cell, 2.0 * scale, fill);
                    ui.painter().rect_stroke(
                        cell,
                        2.0 * scale,
                        Stroke::new(
                            1.0_f32,
                            if selected {
                                CORAL.lerp_to_gamma(CREAM, 0.4)
                            } else {
                                CELL_EDGE
                            },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    let label = format!("Step {}, {note:+} semitones", step + 1);
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            response.enabled(),
                            selected,
                            &label,
                        )
                    });
                    ui.ctx().accesskit_node_builder(response.id, |node| {
                        node.add_action(egui::accesskit::Action::Focus)
                    });
                    focus(
                        ui,
                        &response,
                        current,
                        scale,
                        if selected { controls::INK } else { CREAM },
                    );
                    if current && reveal {
                        ui.scroll_to_rect(cell, None);
                    }
                    if (response.on_hover_text(label).clicked() || (current && toggle))
                        && let Some(value) = state.steps.get_mut(index)
                    {
                        *value = if selected { None } else { Some(note) };
                    }
                }
            }
            ui.ctx()
                .data_mut(|data| data.insert_temp(cursor_id, cursor));
        });
}
fn form_style(ui: &mut egui::Ui, scale: f32) {
    let style = ui.style_mut();
    style.override_font_id = Some(FontId::proportional(13.0 * scale));
    style.spacing.interact_size.y = 26.0 * scale;
    style.spacing.item_spacing = Vec2::new(8.0, 6.0) * scale;
    style.spacing.slider_width = 180.0 * scale;
    style.visuals.override_text_color = Some(CREAM);
    style.visuals.panel_fill = WELL;
    style.visuals.extreme_bg_color = WELL;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, CREAM);
    for widgets in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widgets.bg_fill = FACE;
        widgets.weak_bg_fill = FACE;
        widgets.bg_stroke = Stroke::new(1.0_f32, EDGE);
        widgets.fg_stroke = Stroke::new(1.0_f32, CREAM);
    }
}
fn toggle(
    ui: &mut egui::Ui,
    center: Pos2,
    label_at: Pos2,
    _scale: f32,
    name: &str,
    value: &mut bool,
) {
    let hit = Rect::from_min_size(
        center - Vec2::new(14.0, 13.0),
        Vec2::new((label_at.x - center.x + 90.0).max(110.0), 26.0),
    );
    let response = controls::toggle_at(ui, hit, name, name, value);
    let status = format!("{name}: {}", if *value { "on" } else { "off" });
    let tooltip = match name {
        "1/16 cut" => format!(
            "{status}\n\n\
             On: Limits each note's source audio to one sixteenth-note step at the sequence BPM. \
             Short sounds can end sooner when Loop source is off.\n\n\
             Off: A note can continue across empty steps until the next note or the end of the active-bar sequence. \
             With Loop source off, it also stops when the source audio ends.\n\n\
             Every new note restarts the source from the beginning. At 140 BPM, one step lasts about 107 ms. \
             This switch gates the source audio; effects such as echo can still sound after the cut."
        ),
        "Loop source" => format!(
            "{status}\n\n\
             On: Repeats the entire source audio from its beginning whenever it reaches the end, \
             for as long as the current note is allowed to play. It does not use embedded loop points.\n\n\
             Off: Plays the source once per note. If it ends before the next note, the remaining interval has no source audio.\n\n\
             Every new note restarts the source. With 1/16 cut on, repetition stops after one step. \
             With 1/16 cut off, it can continue across empty steps until the next note or the end of the active-bar sequence. \
             Loop source does not repeat the pattern or extend the sequence."
        ),
        _ => status,
    };
    response.on_hover_text(tooltip);
}
fn button(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    enabled: bool,
    primary: bool,
    scale: f32,
) -> Response {
    keyed_button(ui, rect, text, text, enabled, primary, scale)
}
fn keyed_button(
    ui: &mut egui::Ui,
    rect: Rect,
    key: &str,
    text: &str,
    enabled: bool,
    primary: bool,
    _scale: f32,
) -> Response {
    controls::button_at(
        ui,
        rect,
        key,
        text,
        if primary {
            controls::ButtonRole::Primary
        } else {
            controls::ButtonRole::Secondary
        },
        enabled,
    )
}
fn select_button(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    name: &str,
    selected: bool,
    _scale: f32,
) -> Response {
    let response = controls::button_at(
        ui,
        rect,
        name,
        text,
        if selected {
            controls::ButtonRole::Primary
        } else {
            controls::ButtonRole::Secondary
        },
        true,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            response.enabled(),
            selected,
            name,
        )
    });
    response.on_hover_text(name)
}
fn well(ui: &egui::Ui, rect: Rect, _scale: f32) {
    controls::well(ui, rect);
}
fn lettering(ui: &egui::Ui, center: Pos2, text: &str, size: f32, color: Color32, title: bool) {
    let font = if title {
        FontId::new(size, egui::FontFamily::Name("Syncopate Bold".into()))
    } else {
        FontId::proportional(size)
    };
    let galley = ui.painter().layout_no_wrap(text.into(), font, color);
    ui.painter().galley(
        center - galley.mesh_bounds.center().to_vec2(),
        galley,
        color,
    );
}

fn left_text(ui: &egui::Ui, at: Pos2, text: &str, size: f32, color: Color32) {
    ui.painter().text(
        at,
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(size),
        color,
    );
}

fn named(response: &Response, kind: egui::WidgetType, text: &str) {
    response.widget_info(|| egui::WidgetInfo::labeled(kind, response.enabled(), text));
}

fn focus(ui: &egui::Ui, response: &Response, focused: bool, scale: f32, color: Color32) {
    if focused || (response.hovered() && response.enabled()) {
        ui.painter().rect_stroke(
            response.rect.shrink(scale),
            2.0 * scale,
            Stroke::new(if focused { 2.0 * scale } else { 1.0 }, color),
            egui::StrokeKind::Inside,
        );
    }
}

fn status_footer(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    error: bool,
    missing: bool,
    scale: f32,
) {
    well(ui, rect, scale);
    let response = ui.interact(rect, ui.id().with("status"), Sense::hover());
    named(&response, egui::WidgetType::Panel, "Step synth status");
    let text_rect = Rect::from_min_max(
        rect.min + Vec2::new(9.0, 5.0) * scale,
        rect.max - Vec2::new(if missing { 144.0 } else { 9.0 }, 5.0) * scale,
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(text_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(text_rect.intersect(ui.clip_rect()));
    let previous_id = ui.id().with("previous-status");
    let changed = ui
        .ctx()
        .data(|data| data.get_temp::<String>(previous_id).as_deref() != Some(text));
    ui.ctx()
        .data_mut(|data| data.insert_temp(previous_id, text.to_owned()));
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("step-synth-status")
        .max_height(text_rect.height())
        .auto_shrink([false, false]);
    if changed {
        scroll = scroll.vertical_scroll_offset(0.0);
    }
    scroll.show(&mut child, |ui| {
        let color = if error { CORAL } else { CREAM };
        let mut job = egui::text::LayoutJob::simple(
            text.into(),
            FontId::proportional(12.5 * scale),
            color,
            ui.available_width(),
        );
        job.halign = egui::Align::Center;
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        let (area, label) = ui.allocate_exact_size(
            Vec2::new(
                ui.available_width(),
                galley.mesh_bounds.height().max(text_rect.height()),
            ),
            Sense::hover(),
        );
        let origin = if galley.mesh_bounds.height() <= text_rect.height() {
            area.center() - galley.mesh_bounds.center().to_vec2()
        } else {
            Pos2::new(
                area.center().x - galley.mesh_bounds.center().x,
                area.top() - galley.mesh_bounds.top(),
            )
        };
        ui.painter().galley(origin, galley, color);
        named(
            &label,
            egui::WidgetType::Label,
            &if error {
                format!("Error: {text}")
            } else {
                text.into()
            },
        );
        ui.ctx().accesskit_node_builder(label.id, |node| {
            node.set_live(egui::accesskit::Live::Polite)
        });
        label.on_hover_text(text);
    });
}
