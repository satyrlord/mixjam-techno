//! The bar strip over the grid, in groups: Listen; a key per bar that brings
//! it into view and marks it in the loop or playing; the scale with
//! Randomise; and, at the right, the bar actions. Hairlines part the groups.
//! A narrow strip keeps every control and shows Listen and Randomise as icons.
use super::*;

/// The strip's measures at its width.
struct Strip {
    wide: bool,
    with_text: bool,
    gap: f32,
}

impl Strip {
    fn at(width: f32) -> Self {
        let wide = width >= STRIP_WIDE_WIDTH;
        Self {
            wide,
            with_text: width >= LISTEN_TEXT_WIDTH,
            gap: if wide { 16.0 } else { 12.0 },
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn bar_strip(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    view_bar: &mut usize,
    bars: usize,
    editor: &mut Editor,
    playing: Option<usize>,
    available: bool,
) -> Action {
    let strip = Strip::at(rect.width());
    let gap = strip.gap;
    let listen_width = if strip.with_text { LISTEN_WIDTH } else { 36.0 };
    let action = listen_key(
        ui,
        Rect::from_min_size(rect.min, Vec2::new(listen_width, 28.0)),
        available,
        playing.is_some(),
        strip.with_text,
    );
    let mut x = rect.left() + listen_width + gap;
    divider(ui, rect, x - gap / 2.0);
    x = bar_keys(ui, rect, x, &strip, state.length, view_bar, bars, playing) + gap;
    divider(ui, rect, x - gap / 2.0);
    scale_group(ui, rect, x, &strip, state, editor);
    bar_actions(ui, rect, &strip, state, *view_bar, editor);
    action
}

/// A hairline between two groups, on a pixel's centre.
fn divider(ui: &egui::Ui, rect: Rect, x: f32) {
    let x = egui::emath::GuiRounding::round_to_pixel_center(x, ui.ctx().pixels_per_point());
    ui.painter().line_segment(
        [
            Pos2::new(x, rect.top() + 4.0),
            Pos2::new(x, rect.bottom() - 4.0),
        ],
        Stroke::new(1.0_f32, roles().tool_edge),
    );
}

/// A key per bar from `x`. Returns where the group ends.
#[allow(clippy::too_many_arguments)]
fn bar_keys(
    ui: &mut egui::Ui,
    rect: Rect,
    x: f32,
    strip: &Strip,
    length: u8,
    view_bar: &mut usize,
    bars: usize,
    playing: Option<usize>,
) -> f32 {
    let bar_width = if strip.wide { 36.0 } else { 32.0 };
    for bar in 0..pattern::BARS {
        let cell = Rect::from_min_size(
            Pos2::new(x + bar as f32 * (bar_width + 4.0), rect.top()),
            Vec2::new(bar_width, 28.0),
        );
        let in_view = bar / bars == *view_bar / bars;
        let name = format!("View bar {}", bar + 1);
        let response = key(
            ui,
            cell,
            Key {
                selected: Some(bar == *view_bar),
                ..Key::new(&name, &format!("{}", bar + 1))
            },
        )
        .hover_hint(&name);
        if in_view && bar != *view_bar {
            // The other bars in view keep a faint outline.
            ui.painter().rect_stroke(
                cell.shrink(2.0).translate(Vec2::new(0.0, -1.0)),
                3.0,
                Stroke::new(
                    1.0_f32,
                    controls::Palette::Olive.accent().gamma_multiply(0.55),
                ),
                egui::StrokeKind::Inside,
            );
        }
        if response.clicked() {
            *view_bar = bar;
        }
        // The loop mark under the bar: full when the whole bar plays.
        let looped = pattern::bar_in_loop(length, bar);
        if looped > 0.0 {
            let mark = Rect::from_min_size(
                cell.left_bottom() + Vec2::new(2.0, 2.0),
                Vec2::new((cell.width() - 4.0) * looped, 3.0),
            );
            ui.painter()
                .rect_filled(mark, 1.0, controls::Palette::Olive.accent());
        }
        if playing.is_some_and(|step| step / BAR == bar) {
            ui.painter()
                .circle_filled(cell.right_top() + Vec2::new(-6.0, 6.0), 3.0, roles().coral);
        }
    }
    x + pattern::BARS as f32 * bar_width + 12.0
}

/// Scale, its root, and Randomise from `x`. The scale shades the rows
/// Randomise writes into.
fn scale_group(
    ui: &mut egui::Ui,
    rect: Rect,
    x: f32,
    strip: &Strip,
    state: &mut StepSynthState,
    editor: &mut Editor,
) {
    let (scale_width, root_width) = if strip.wide {
        (136.0, 52.0)
    } else {
        // Narrow enough to leave room for Fill; the list truncates, and its
        // tooltip names the scale.
        (80.0, 42.0)
    };
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(Rect::from_min_size(
            Pos2::new(x, rect.top() + 1.0),
            Vec2::new(scale_width + 4.0 + root_width, 26.0),
        )),
        |ui| {
            form_style(ui);
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.horizontal(|ui| {
                let response = egui::ComboBox::from_id_salt("step-synth-scale")
                    .selected_text(state.scale.label())
                    .width(scale_width)
                    .truncate()
                    .show_ui(ui, |ui| {
                        for scale in pattern::Scale::ALL {
                            // A choice closes the list, also for an agent.
                            if ui
                                .selectable_value(&mut state.scale, scale, scale.label())
                                .clicked()
                            {
                                editor.edit("the scale change");
                                ui.close();
                            }
                        }
                    })
                    .response;
                name_control(response, "Scale").hover_hint(format!(
                    "Scale: {}. It shades the pitch rows, and Randomise writes in it. \
                     Every note stays playable.",
                    state.scale.label()
                ));
                let response = egui::ComboBox::from_id_salt("step-synth-root")
                    .selected_text(pattern::ROOTS[usize::from(state.root % 12)])
                    .width(root_width)
                    .show_ui(ui, |ui| {
                        for (root, name) in pattern::ROOTS.iter().enumerate() {
                            if ui
                                .selectable_value(&mut state.root, root as u8, *name)
                                .clicked()
                            {
                                editor.edit("the root change");
                                ui.close();
                            }
                        }
                    })
                    .response;
                name_control(response, "Scale root")
                    .hover_hint("The scale's root. A is the source pitch.");
            });
        },
    );
    let x = x + scale_width + 4.0 + root_width + 4.0;
    if key(
        ui,
        Rect::from_min_size(
            Pos2::new(x, rect.top()),
            Vec2::new(if strip.wide { 104.0 } else { 30.0 }, 28.0),
        ),
        Key {
            glyph: Glyph::Dice,
            ..Key::new("Randomise", if strip.wide { "Randomise" } else { "" })
        },
    )
    .hover_hint(
        "Randomise: write a new line in the scale over the pattern's length. \
         Ctrl+Z over the board takes it back.",
    )
    .clicked()
    {
        editor.randomise(state);
    }
}

/// Copy to N, Fill, and Clear bar, at the strip's right end, for the viewed
/// bar.
fn bar_actions(
    ui: &mut egui::Ui,
    rect: Rect,
    strip: &Strip,
    state: &mut StepSynthState,
    view_bar: usize,
    editor: &mut Editor,
) {
    let right = rect.right();
    let (copy_width, fill_width, clear_width) = if strip.wide {
        (80.0, 52.0, 82.0)
    } else {
        (72.0, 44.0, 64.0)
    };
    let fill_left = right - clear_width - 4.0 - fill_width;
    let copy_left = fill_left - 4.0 - copy_width;
    divider(ui, rect, copy_left - strip.gap / 2.0);
    let target = pattern::copy_target(view_bar);
    // The name is the visible text, so an agent finds the key by what it
    // reads; on bar 4 the disabled key reads Copy bar.
    let copy = target.map_or("Copy bar".to_owned(), |target| {
        format!("Copy to {}", target + 1)
    });
    let response = key(
        ui,
        Rect::from_min_size(
            Pos2::new(copy_left, rect.top()),
            Vec2::new(copy_width, 28.0),
        ),
        Key {
            enabled: target.is_some(),
            ..Key::new(&copy, &copy)
        },
    );
    let response = match target {
        Some(target) => response.hover_hint(format!(
            "Copy this bar onto bar {}, replacing it. Ctrl+Z over the board takes it back.",
            target + 1
        )),
        None => response.disabled_hint("Bar 4 is the last bar, so there is no bar to copy it to."),
    };
    if response.clicked()
        && let Some(target) = target
        && pattern::copy_bar(state, view_bar)
    {
        editor.edit_saying(
            "Copy bar",
            format!("Copied bar {} to bar {}.", view_bar + 1, target + 1),
        );
    }
    let response = key(
        ui,
        Rect::from_min_size(
            Pos2::new(fill_left, rect.top()),
            Vec2::new(fill_width, 28.0),
        ),
        Key::new("Fill", "Fill"),
    )
    .hover_hint(format!(
        "Fill: copy bar {} onto the other three bars, replacing them. \
         Ctrl+Z over the board takes it back.",
        view_bar + 1
    ));
    if response.clicked() {
        let notice = if pattern::fill_bars(state, view_bar) {
            editor.selection = None;
            format!("Copied bar {} to every bar.", view_bar + 1)
        } else {
            format!("Every bar is already a copy of bar {}.", view_bar + 1)
        };
        editor.edit_saying("Fill", notice);
    }
    let empty = pattern::bar_is_empty(state, view_bar);
    let response = key(
        ui,
        Rect::from_min_size(
            Pos2::new(right - clear_width, rect.top()),
            Vec2::new(clear_width, 28.0),
        ),
        Key {
            enabled: !empty,
            ..Key::new("Clear bar", "Clear bar")
        },
    );
    let response = if empty {
        response.disabled_hint("This bar is already empty.")
    } else {
        response.hover_hint("Clear this bar's steps. Ctrl+Z over the board takes it back.")
    };
    if response.clicked() {
        pattern::clear_bar(state, view_bar);
        editor.selection = None;
        editor.edit_saying("Clear bar", format!("Cleared bar {}.", view_bar + 1));
    }
}
