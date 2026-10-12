//! Step synth hardware presentation. Session owns processing and file operations.
use super::*;

const CREAM: Color32 = Color32::from_rgb(245, 232, 205);
const INK: Color32 = Color32::from_rgb(24, 25, 23);
const BODY: Color32 = Color32::from_rgb(44, 46, 41);
const WELL: Color32 = Color32::from_rgb(25, 29, 22);
const FACE: Color32 = Color32::from_rgb(36, 40, 34);
const EDGE: Color32 = Color32::from_rgb(91, 99, 75);
const AMBER: Color32 = Color32::from_rgb(245, 193, 86);
const ERROR: Color32 = Color32::from_rgb(255, 167, 143);
const UV: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));

macro_rules! pngs {
    ($name:literal) => {
        [
            include_bytes!(concat!("../../assets/step-synth/100/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/step-synth/150/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/step-synth/200/", $name, ".png")).as_slice(),
        ]
    };
}

struct Artwork {
    density: usize,
    textures: Vec<egui::TextureHandle>,
}

impl Artwork {
    fn load(ctx: &egui::Context) -> Result<Arc<Self>, String> {
        let density = [1.0_f32, 1.5, 2.0]
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (**a - ctx.pixels_per_point())
                    .abs()
                    .total_cmp(&(**b - ctx.pixels_per_point()).abs())
            })
            .map_or(0, |(index, _)| index);
        let cache_id = egui::Id::new(("step-synth-artwork", density));
        if let Some(cached) = ctx.data(|data| data.get_temp::<Arc<Self>>(cache_id)) {
            return Ok(cached);
        }
        let textures = [pngs!("panel"), pngs!("collapsed")]
            .iter()
            .enumerate()
            .map(|(index, variants)| {
                let image = image::load_from_memory(variants[density])
                    .map_err(|error| format!("Cannot load Step synth artwork: {error}"))?
                    .into_rgba8();
                Ok(ctx.load_texture(
                    format!("step-synth-{density}-{index}"),
                    egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    ),
                    egui::TextureOptions::NEAREST,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let artwork = Arc::new(Self { density, textures });
        ctx.data_mut(|data| data.insert_temp(cache_id, artwork.clone()));
        Ok(artwork)
    }

    fn paint(&self, ui: &egui::Ui, index: usize, center: Pos2) {
        let texture = &self.textures[index];
        let ppp = ui.ctx().pixels_per_point();
        let size = texture.size_vec2() / ppp;
        let min = ((center - size / 2.0).to_vec2() * ppp).round() / ppp;
        ui.painter().image(
            texture.id(),
            Rect::from_min_size(min.to_pos2(), size),
            UV,
            Color32::WHITE,
        );
    }
}

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

/// The 820 by 556-point skin has a fixed 44-point status footer.
pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    feedback: Feedback<'_>,
    state: &mut StepSynthState,
    view_bar: &mut usize,
    show_effects: &mut bool,
) -> Result<Action, String> {
    let art = Artwork::load(ctx)?;
    let scale = [1.0, 1.5, 2.0][art.density] / ctx.pixels_per_point();
    let collapse_id = egui::Id::new("step-synth-collapsed");
    let collapsed = ctx
        .data(|data| data.get_temp::<bool>(collapse_id))
        .unwrap_or(false);
    let guard_id = egui::Id::new("step-synth-escape-guard");
    let guarded = ctx
        .data(|data| data.get_temp::<bool>(guard_id))
        .unwrap_or(false);
    let mut action = Action::None;
    egui::Window::new("STEP SYNTH")
        .id(egui::Id::new("Step synth"))
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .constrain_to(ctx.content_rect().shrink(12.0))
        .default_pos(ctx.content_rect().center() - Vec2::new(410.0, 300.0) * scale)
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let (allocated, _) = ui.allocate_exact_size(
                Vec2::new(820.0, if collapsed { 48.0 } else { 600.0 }) * scale,
                Sense::hover(),
            );
            let ppp = ctx.pixels_per_point();
            let bounds = Rect::from_min_size(
                (allocated.min.to_vec2() * ppp).round().to_pos2() / ppp,
                allocated.size(),
            );
            let rect = |x, y, w, h| {
                Rect::from_min_size(
                    bounds.min + Vec2::new(x, y) * scale,
                    Vec2::new(w, h) * scale,
                )
            };
            let point = |x, y| bounds.min + Vec2::new(x, y) * scale;
            let mut panel = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
            panel.set_clip_rect(bounds);
            panel.style_mut().override_font_id = Some(FontId::proportional(13.0 * scale));
            panel.style_mut().visuals.override_text_color = Some(CREAM);
            if !collapsed {
                panel.painter().rect_filled(
                    rect(0.0, 540.0, 820.0, 60.0),
                    egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: (12.0 * scale).round() as u8,
                        se: (12.0 * scale).round() as u8,
                    },
                    BODY,
                );
            }
            art.paint(
                &panel,
                usize::from(collapsed),
                point(410.0, if collapsed { 24.0 } else { 278.0 }),
            );
            lettering(
                &panel,
                point(410.0, 24.0),
                "STEP SYNTH",
                22.0 * scale,
                CREAM,
                true,
            );
            if !collapsed {
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
                well(&panel, main_rect, scale);
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
                    form.set_clip_rect(effects_rect);
                    form_style(&mut form, scale);
                    form.style_mut().spacing.scroll.floating = false;
                    form.style_mut().spacing.scroll.bar_width = 6.0 * scale;
                    form.style_mut().spacing.scroll.bar_inner_margin = 4.0 * scale;
                    form.style_mut().spacing.scroll.bar_outer_margin = 0.0;
                    egui::ScrollArea::vertical()
                        .id_salt("step-synth-effects")
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
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
                            "Select at least one generator bar."
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
                    "Select at least one generator bar."
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
                        rect(700.0, 566.0, 104.0, 26.0),
                        "Import details",
                        true,
                        false,
                        scale,
                    )
                    .clicked()
                {
                    action = Action::Details;
                }
            }
            let collapse_name = if collapsed {
                "Expand Step synth"
            } else {
                "Collapse Step synth"
            };
            let collapse = panel.interact(
                rect(7.0, 11.0, 26.0, 26.0),
                panel.id().with("collapse"),
                Sense::click(),
            );
            named(&collapse, egui::WidgetType::Button, collapse_name);
            let direction = if collapsed { -1.0 } else { 1.0 };
            panel.painter().add(egui::Shape::convex_polygon(
                vec![
                    point(20.0, 24.0) + Vec2::new(-5.5, -3.0 * direction) * scale,
                    point(20.0, 24.0) + Vec2::new(5.5, -3.0 * direction) * scale,
                    point(20.0, 24.0) + Vec2::new(0.0, 4.5 * direction) * scale,
                ],
                CREAM,
                Stroke::NONE,
            ));
            focus(&panel, &collapse, scale);
            if collapse.on_hover_text(collapse_name).clicked() {
                ctx.data_mut(|data| data.insert_temp(collapse_id, !collapsed));
            }
            let help = panel.interact(
                rect(751.0, 11.0, 26.0, 26.0),
                panel.id().with("help"),
                Sense::click(),
            );
            help_icon(panel.painter(), point(764.0, 24.0), scale);
            named(&help, egui::WidgetType::Button, "Step synth help");
            focus(&panel, &help, scale);
            if help.on_hover_text("How to use Step synth").clicked() {
                action = Action::Help;
            }
            let close = panel.interact(
                rect(787.0, 11.0, 26.0, 26.0),
                panel.id().with("close"),
                Sense::click(),
            );
            named(&close, egui::WidgetType::Button, "Close Step synth");
            for diagonal in [-1.0, 1.0] {
                let offset = Vec2::new(6.0, 6.0 * diagonal) * scale;
                panel.painter().line_segment(
                    [point(800.0, 24.0) - offset, point(800.0, 24.0) + offset],
                    Stroke::new(1.6 * scale, CREAM),
                );
            }
            focus(&panel, &close, scale);
            let layer = panel.layer_id();
            let on_top = ctx.memory(|memory| {
                memory.top_modal_layer().is_none()
                    && memory
                        .layer_ids()
                        .filter(|other| {
                            other.order == egui::Order::Middle && memory.areas().is_visible(other)
                        })
                        .last()
                        == Some(layer)
            });
            if close.on_hover_text("Close Step synth (Esc)").clicked()
                || (on_top && !guarded && ctx.input(|input| input.key_pressed(egui::Key::Escape)))
            {
                *open = false;
            }
            let editing = ctx
                .memory(|memory| memory.focused())
                .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
            let guard = editing || egui::Popup::is_any_open(ctx);
            ctx.data_mut(|data| data.insert_temp(guard_id, guard));
        });
    Ok(action)
}

fn note_grid(ui: &mut egui::Ui, rect: Rect, scale: f32, state: &mut StepSynthState, bar: usize) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect);
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
                    Stroke::new(1.0_f32, Color32::from_rgb(63, 70, 53)),
                );
            }
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
                    let response = ui.interact(
                        cell,
                        ui.id().with(("note", bar, note, step)),
                        Sense::click(),
                    );
                    let fill = if selected {
                        CORAL
                    } else if response.hovered() {
                        Color32::from_rgb(62, 68, 53)
                    } else if matches!((note + 24) % 12, 0 | 2 | 3 | 5 | 7 | 8 | 10) {
                        FACE
                    } else {
                        Color32::from_rgb(29, 33, 28)
                    };
                    ui.painter().rect_filled(cell, 2.0 * scale, fill);
                    ui.painter().rect_stroke(
                        cell,
                        2.0 * scale,
                        Stroke::new(
                            1.0_f32,
                            if selected {
                                ERROR
                            } else {
                                Color32::from_rgb(70, 76, 64)
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
                    focus(ui, &response, scale);
                    if response.gained_focus() {
                        ui.scroll_to_rect(cell, None);
                    }
                    if response.on_hover_text(label).clicked()
                        && let Some(value) = state.steps.get_mut(index)
                    {
                        *value = if selected { None } else { Some(note) };
                    }
                }
            }
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
    style.visuals.selection.bg_fill = EDGE;
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
    scale: f32,
    name: &str,
    value: &mut bool,
) {
    let hit = Rect::from_center_size(center, Vec2::new(30.0, 26.0) * scale);
    let mut response = ui.interact(hit, ui.id().with(("toggle", name)), Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let track = Rect::from_center_size(center, Vec2::new(26.0, 16.0) * scale);
    ui.painter().rect_filled(track, 8.0 * scale, INK);
    ui.painter().rect_stroke(
        track,
        8.0 * scale,
        Stroke::new(1.0_f32, EDGE),
        egui::StrokeKind::Inside,
    );
    if *value {
        ui.painter()
            .rect_filled(track.shrink(2.0 * scale), 6.0 * scale, AMBER);
    }
    let knob = center + Vec2::new(if *value { 5.0 } else { -5.0 }, 0.0) * scale;
    ui.painter()
        .circle_filled(knob + Vec2::new(0.0, scale), 6.0 * scale, Color32::BLACK);
    ui.painter()
        .circle_filled(knob, 5.5 * scale, Color32::from_rgb(205, 203, 183));
    ui.painter()
        .circle_stroke(knob, 5.5 * scale, Stroke::new(1.0_f32, CREAM));
    let label_rect = ui.painter().text(
        label_at,
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(13.0 * scale),
        CREAM,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, response.enabled(), *value, name)
    });
    focus(ui, &response, scale);
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
    ui.interact(
        label_rect,
        ui.id().with(("toggle-help", name)),
        Sense::hover(),
    )
    .on_hover_text(tooltip.clone());
    response.on_hover_text(tooltip);
}

fn button(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    enabled: bool,
    ivory: bool,
    scale: f32,
) -> Response {
    keyed_button(ui, rect, text, text, enabled, ivory, scale)
}

/// A button whose label changes keeps its identity, and so keyboard focus, through `key`.
fn keyed_button(
    ui: &mut egui::Ui,
    rect: Rect,
    key: &str,
    text: &str,
    enabled: bool,
    ivory: bool,
    scale: f32,
) -> Response {
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.interact(rect, ui.id().with(key), Sense::click())
        })
        .inner;
    let color = if ivory { INK } else { CREAM };
    let fill = if ivory {
        CREAM
    } else if response.hovered() {
        Color32::from_rgb(52, 58, 46)
    } else {
        FACE
    };
    ui.painter()
        .rect_filled(rect, 4.0 * scale, Color32::from_rgb(12, 14, 11));
    ui.painter()
        .rect_filled(rect.shrink(2.0 * scale), 3.0 * scale, fill);
    ui.painter().rect_stroke(
        rect,
        4.0 * scale,
        Stroke::new(1.0_f32, EDGE),
        egui::StrokeKind::Inside,
    );
    if !enabled || response.is_pointer_button_down_on() {
        ui.painter().rect_filled(
            rect.shrink(2.0 * scale),
            3.0 * scale,
            Color32::from_black_alpha(if enabled { 35 } else { 95 }),
        );
    }
    lettering(
        ui,
        rect.center(),
        text,
        12.5 * scale,
        if enabled {
            color
        } else {
            color.gamma_multiply(0.6)
        },
        false,
    );
    named(&response, egui::WidgetType::Button, text);
    focus(ui, &response, scale);
    response
}

fn select_button(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    name: &str,
    selected: bool,
    scale: f32,
) -> Response {
    let response = ui.interact(rect, ui.id().with(name), Sense::click());
    ui.painter()
        .rect_filled(rect, 4.0 * scale, if selected { AMBER } else { FACE });
    ui.painter().rect_stroke(
        rect,
        4.0 * scale,
        Stroke::new(1.0_f32, if selected { CREAM } else { EDGE }),
        egui::StrokeKind::Inside,
    );
    lettering(
        ui,
        rect.center(),
        text,
        13.0 * scale,
        if selected { INK } else { CREAM },
        false,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            response.enabled(),
            selected,
            name,
        )
    });
    focus(ui, &response, scale);
    response.on_hover_text(name)
}

fn well(ui: &egui::Ui, rect: Rect, scale: f32) {
    ui.painter().rect_filled(rect, 5.0 * scale, WELL);
    ui.painter().rect_stroke(
        rect,
        5.0 * scale,
        Stroke::new(1.0_f32, EDGE),
        egui::StrokeKind::Inside,
    );
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

fn focus(ui: &egui::Ui, response: &Response, scale: f32) {
    if response.has_focus() || (response.hovered() && response.enabled()) {
        ui.painter().rect_stroke(
            response.rect.shrink(scale),
            2.0 * scale,
            Stroke::new(
                if response.has_focus() {
                    2.0 * scale
                } else {
                    1.0
                },
                CREAM,
            ),
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
        rect.max - Vec2::new(if missing { 122.0 } else { 9.0 }, 5.0) * scale,
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(text_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(text_rect);
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
        let color = if error { ERROR } else { CREAM };
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

fn help_icon(painter: &egui::Painter, center: Pos2, scale: f32) {
    let stroke = Stroke::new(1.2 * scale, CREAM);
    painter.circle_stroke(center, 7.0 * scale, stroke);
    painter.add(egui::Shape::line(
        [
            (-2.5, -2.0),
            (-2.0, -3.5),
            (0.0, -4.0),
            (2.0, -3.0),
            (2.5, -1.5),
            (1.5, 0.0),
            (0.0, 1.0),
            (0.0, 2.0),
        ]
        .into_iter()
        .map(|(x, y)| center + Vec2::new(x, y) * scale)
        .collect(),
        stroke,
    ));
    painter.circle_filled(center + Vec2::new(0.0, 4.0) * scale, 0.8 * scale, CREAM);
}
