//! Time Stretcher presentation. Rendering and file operations stay in Session.
use super::*;

const CREAM: Color32 = Color32::from_rgb(245, 232, 205);
const INK: Color32 = Color32::from_rgb(24, 25, 23);
const UV: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));

macro_rules! pngs {
    ($name:literal) => {
        [
            include_bytes!(concat!("../../assets/stretch/100/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/stretch/150/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/stretch/200/", $name, ".png")).as_slice(),
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
        let cache_id = egui::Id::new(("stretch-artwork", density));
        if let Some(cached) = ctx.data(|data| data.get_temp::<Arc<Self>>(cache_id)) {
            return Ok(cached);
        }
        let textures = [pngs!("panel")]
            .iter()
            .enumerate()
            .map(|(index, variants)| {
                let image = image::load_from_memory(variants[density])
                    .map_err(|error| format!("Cannot load Time Stretcher artwork: {error}"))?
                    .into_rgba8();
                Ok(ctx.load_texture(
                    format!("stretch-{density}-{index}"),
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

    fn paint(&self, ui: &egui::Ui, index: usize, center: Pos2, tint: Color32) {
        let texture = &self.textures[index];
        let ppp = ui.ctx().pixels_per_point();
        let size = texture.size_vec2() / ppp;
        let min = ((center - size / 2.0).to_vec2() * ppp).round() / ppp;
        ui.painter().image(
            texture.id(),
            Rect::from_min_size(min.to_pos2(), size),
            UV,
            tint,
        );
    }
}

pub(super) struct Source<'a> {
    pub name: &'a str,
    pub id: Option<u32>,
    pub samples: &'a [Sample],
    pub missing: bool,
}

pub(super) enum Action {
    None,
    Select(u32),
    Listen,
    Save,
    Details,
}

/// Design coordinates map to a compact 640 by 320-point panel at 100% scaling.
pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    original: &mut f64,
    target: &mut f64,
) -> Result<Action, String> {
    let art = Artwork::load(ctx)?;
    let scale = (640.0 / 880.0) * [1.0, 1.5, 2.0][art.density] / ctx.pixels_per_point();
    let collapse_id = egui::Id::new("stretch-collapsed");
    let mut collapsed = ctx
        .data(|data| data.get_temp::<bool>(collapse_id))
        .unwrap_or(false);
    let mut action = Action::None;
    egui::Window::new("TIME STRETCHER")
        .id(egui::Id::new("Time Stretcher"))
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .default_pos(ctx.content_rect().center() - Vec2::new(440.0, 220.0) * scale)
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let (bounds, _) = ui.allocate_exact_size(
                Vec2::new(880.0, if collapsed { 76.0 } else { 440.0 }) * scale,
                Sense::hover(),
            );
            let rect = |x, y, w, h| {
                Rect::from_min_size(
                    bounds.min + Vec2::new(x, y) * scale,
                    Vec2::new(w, h) * scale,
                )
            };
            let point = |x, y| bounds.min + Vec2::new(x, y) * scale;
            // A collapsed panel clips the same artwork instead of resampling it.
            let painter = ui.painter().with_clip_rect(bounds);
            let mut panel_ui = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
            panel_ui.set_clip_rect(bounds);
            art.paint(&panel_ui, 0, point(440.0, 220.0), Color32::WHITE);
            label(
                ui,
                point(452.0, 42.0),
                "TIME STRETCHER",
                40.0 * scale,
                CREAM,
                true,
                Align2::CENTER_CENTER,
            );
            let close_rect = rect(778.0, 20.0, 44.0, 44.0);
            let close = ui.interact(close_rect, ui.id().with("close"), Sense::click());
            named(&close, egui::WidgetType::Button, "Close Time Stretcher");
            focus(ui, &close, scale);
            if close.on_hover_text("Close Time Stretcher (Esc)").clicked()
                || ctx.input(|input| input.key_pressed(egui::Key::Escape))
            {
                *open = false;
            }
            let collapse = ui.interact(
                rect(134.0, 23.0, 48.0, 40.0),
                ui.id().with("collapse"),
                Sense::click(),
            );
            named(
                &collapse,
                egui::WidgetType::Button,
                if collapsed {
                    "Expand Time Stretcher"
                } else {
                    "Collapse Time Stretcher"
                },
            );
            focus(ui, &collapse, scale);
            if collapse.clicked() {
                collapsed = !collapsed;
                ctx.data_mut(|data| data.insert_temp(collapse_id, collapsed));
            }
            if collapsed {
                return;
            }
            let mut selected = source.id;
            at(ui, rect(76.0, 84.0, 731.0, 37.0), |ui| {
                control_style(ui, 24.0 * scale, CREAM);
                let sounds = egui::ComboBox::from_id_salt("stretch-source")
                    .width(727.0 * scale)
                    .selected_text(source.name)
                    .show_ui(ui, |ui| {
                        ui.set_max_width(710.0 * scale);
                        ui.style_mut().override_font_id = Some(FontId::proportional(16.0 * scale));
                        for sample in source.samples {
                            ui.selectable_value(&mut selected, Some(sample.id), &sample.name);
                        }
                    })
                    .response;
                name_control(sounds, "Source sound").on_hover_text(source.name);
            });
            if selected != source.id
                && let Some(id) = selected
            {
                action = Action::Select(id);
            }
            label(
                ui,
                point(97.0, 164.0),
                "Original BPM",
                25.0 * scale,
                CREAM,
                false,
                Align2::LEFT_CENTER,
            );
            label(
                ui,
                point(478.0, 164.0),
                "Target BPM",
                25.0 * scale,
                CREAM,
                false,
                Align2::LEFT_CENTER,
            );
            ui.scope(|ui| {
                control_style(ui, 36.0 * scale, CREAM);
                ui.put(
                    rect(293.0, 145.0, 90.0, 39.0),
                    egui::DragValue::new(original)
                        .range(30.0..=300.0)
                        .speed(1.0),
                )
                .on_hover_text("Original BPM: drag, or click to enter a value.")
                .widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, "Original BPM")
                });
            });
            let target_response = ui.interact(
                rect(636.0, 139.0, 161.0, 51.0),
                ui.id().with("target-picker"),
                Sense::click(),
            );
            named(&target_response, egui::WidgetType::ComboBox, "Target BPM");
            focus(ui, &target_response, scale);
            label(
                ui,
                point(698.0, 164.0),
                &format!("{target:.0}"),
                36.0 * scale,
                CREAM,
                false,
                Align2::CENTER_CENTER,
            );
            egui::Popup::menu(&target_response).show(|ui| {
                for bpm in SUPPORTED_BPMS {
                    if ui
                        .selectable_value(target, bpm, format!("{bpm:.0} BPM"))
                        .clicked()
                    {
                        ui.close();
                    }
                }
            });
            target_response.on_hover_text("Target BPM: 120, 140, 160, or 180.");
            rotary(ui, rect(278.0, 195.0, 120.0, 114.0), original, false, scale);
            rotary(ui, rect(636.0, 195.0, 120.0, 114.0), target, true, scale);
            let warning = if source.missing {
                "Missing audio: processing is unavailable."
            } else {
                "Pitch preservation is approximate."
            };
            label(
                ui,
                point(76.0, 307.0),
                warning,
                21.0 * scale,
                CREAM,
                false,
                Align2::LEFT_CENTER,
            );
            if source.missing {
                at(ui, rect(651.0, 294.0, 147.0, 28.0), |ui| {
                    if ui.button("Import details").clicked() {
                        action = Action::Details;
                    }
                });
            }
            let enabled = source.id.is_some() && !source.missing;
            if action_button(
                ui,
                rect(62.0, 328.0, 374.0, 80.0),
                "Listen stretch",
                false,
                enabled,
                scale,
            )
            .clicked()
            {
                action = Action::Listen;
            }
            if action_button(
                ui,
                rect(446.0, 328.0, 374.0, 80.0),
                "Save stretched sound",
                true,
                enabled,
                scale,
            )
            .clicked()
            {
                action = Action::Save;
            }
            // Keep the generated lamps within the disabled/pressed treatment.
            if !enabled {
                for button in [
                    rect(64.0, 330.0, 370.0, 76.0),
                    rect(448.0, 330.0, 370.0, 76.0),
                ] {
                    painter.rect_filled(button, 10.0 * scale, Color32::from_black_alpha(95));
                }
            }
        });
    Ok(action)
}

fn at(ui: &mut egui::Ui, rect: Rect, add: impl FnOnce(&mut egui::Ui)) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        add,
    );
}

fn control_style(ui: &mut egui::Ui, size: f32, color: Color32) {
    let style = ui.style_mut();
    style.override_font_id = Some(FontId::proportional(size));
    style.visuals.override_text_color = Some(color);
    style.spacing.button_padding = Vec2::ZERO;
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.bg_fill = Color32::TRANSPARENT;
        widget.weak_bg_fill = Color32::TRANSPARENT;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke.color = color;
    }
}

fn label(
    ui: &egui::Ui,
    pos: Pos2,
    text: &str,
    size: f32,
    color: Color32,
    title: bool,
    align: Align2,
) {
    let font = if title {
        FontId::new(size, egui::FontFamily::Name("Syncopate Bold".into()))
    } else {
        FontId::proportional(size)
    };
    ui.painter().text(
        pos + Vec2::new(0.0, 1.0),
        align,
        text,
        font.clone(),
        Color32::BLACK,
    );
    ui.painter().text(pos, align, text, font, color);
}

fn named(response: &Response, kind: egui::WidgetType, text: &str) {
    response.widget_info(|| egui::WidgetInfo::labeled(kind, response.enabled(), text));
}

fn focus(ui: &egui::Ui, response: &Response, scale: f32) {
    if response.has_focus() || (response.hovered() && response.enabled()) {
        ui.painter().rect_stroke(
            response.rect.shrink(4.0 * scale),
            6.0 * scale,
            Stroke::new(1.5 * scale, CREAM),
            egui::StrokeKind::Inside,
        );
    }
}

fn action_button(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    save: bool,
    enabled: bool,
    scale: f32,
) -> Response {
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.interact(rect, ui.id().with(text), Sense::click())
        })
        .inner;
    if response.is_pointer_button_down_on() {
        ui.painter().rect_filled(
            rect.shrink(5.0 * scale),
            8.0 * scale,
            Color32::from_black_alpha(45),
        );
    }
    label(
        ui,
        rect.center() + Vec2::new(20.0 * scale, 0.0),
        text,
        25.0 * scale,
        if save { INK } else { CREAM },
        false,
        Align2::CENTER_CENTER,
    );
    focus(ui, &response, scale);
    named(&response, egui::WidgetType::Button, text);
    response.on_hover_text(if enabled {
        text
    } else {
        "Select an available sound to process it."
    })
}

fn step_target(value: f64, direction: f64) -> f64 {
    let index = SUPPORTED_BPMS
        .iter()
        .position(|bpm| *bpm == value)
        .unwrap_or(1);
    SUPPORTED_BPMS[(index as isize + direction.signum() as isize).clamp(0, 3) as usize]
}

fn rotary(ui: &mut egui::Ui, rect: Rect, value: &mut f64, target: bool, scale: f32) -> Response {
    let mut response = ui.interact(
        rect,
        ui.id().with(if target {
            "target-knob"
        } else {
            "original-knob"
        }),
        Sense::click_and_drag(),
    );
    let before = *value;
    if response.has_focus() {
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
    }
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    if response.drag_started() {
        ui.ctx()
            .data_mut(|data| data.insert_temp(response.id.with("start"), before));
    }
    if response.dragged() {
        let start = ui
            .ctx()
            .data(|data| data.get_temp::<f64>(response.id.with("start")))
            .unwrap_or(before);
        let delta = response.drag_delta();
        let draft = (start
            + f64::from((delta.x - delta.y) / scale)
                * if ui.input(|i| i.modifiers.shift) {
                    0.1
                } else {
                    1.0
                })
        .clamp(30.0, 300.0);
        *value = if target {
            SUPPORTED_BPMS
                .into_iter()
                .min_by(|a, b| (a - draft).abs().total_cmp(&(b - draft).abs()))
                .unwrap_or(140.0)
        } else {
            draft.round()
        };
    }
    let mut direction = 0.0_f64;
    if response.hovered() {
        direction = ui.input_mut(|input| {
            let delta = input.raw_scroll_delta.y;
            input.smooth_scroll_delta = Vec2::ZERO;
            input.raw_scroll_delta = Vec2::ZERO;
            // signum returns +1 for positive zero. Idle hover is no turn.
            if delta.abs() > 0.0 {
                f64::from(delta.signum())
            } else {
                0.0
            }
        });
    }
    if response.has_focus() {
        ui.input_mut(|input| {
            let modifiers = input.modifiers;
            if input.consume_key(modifiers, egui::Key::ArrowUp)
                || input.consume_key(modifiers, egui::Key::ArrowRight)
            {
                direction = 1.0;
            }
            if input.consume_key(modifiers, egui::Key::ArrowDown)
                || input.consume_key(modifiers, egui::Key::ArrowLeft)
            {
                direction = -1.0;
            }
            if input.consume_key(modifiers, egui::Key::Home) {
                *value = if target { 120.0 } else { 30.0 };
            }
            if input.consume_key(modifiers, egui::Key::End) {
                *value = if target { 180.0 } else { 300.0 };
            }
        });
    }
    if direction != 0.0 {
        if response.has_focus() {
            // The first key after a focus request arrives before egui retains
            // the filter. Keep this adjustment from navigating to a neighbor.
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        *value = if target {
            step_target(*value, direction)
        } else {
            (*value + direction).clamp(30.0, 300.0)
        };
    }
    ui.input(|input| {
        use egui::accesskit::{Action, ActionData};
        for request in input.accesskit_action_requests(response.id, Action::SetValue) {
            if let Some(ActionData::NumericValue(draft)) = request.data
                && draft.is_finite()
            {
                *value = if target {
                    SUPPORTED_BPMS
                        .into_iter()
                        .min_by(|a, b| (a - draft).abs().total_cmp(&(b - draft).abs()))
                        .unwrap_or(140.0)
                } else {
                    draft.clamp(30.0, 300.0)
                };
            }
        }
        let increment = input.num_accesskit_action_requests(response.id, Action::Increment);
        let decrement = input.num_accesskit_action_requests(response.id, Action::Decrement);
        for _ in 0..increment {
            *value = if target {
                step_target(*value, 1.0)
            } else {
                (*value + 1.0).min(300.0)
            };
        }
        for _ in 0..decrement {
            *value = if target {
                step_target(*value, -1.0)
            } else {
                (*value - 1.0).max(30.0)
            };
        }
    });
    if response.double_clicked() {
        *value = 140.0;
    }
    if before != *value {
        response.mark_changed();
    }
    let center = rect.center();
    let fraction = (*value - 140.0) / if *value < 140.0 { 110.0 } else { 160.0 };
    let vector = Vec2::angled((-90.0 + fraction as f32 * 135.0).to_radians());
    ui.painter().line_segment(
        [
            center + vector * 19.0 * scale,
            center + vector * 41.0 * scale,
        ],
        Stroke::new(4.0 * scale, CREAM),
    );
    focus(ui, &response, scale);
    let name = if target {
        "Target BPM knob"
    } else {
        "Original BPM knob"
    };
    response.widget_info(|| egui::WidgetInfo::slider(true, *value, name));
    ui.ctx().accesskit_node_builder(response.id, |builder| {
        use egui::accesskit::Action;
        let (min, max, step) = if target {
            (120.0, 180.0, 20.0)
        } else {
            (30.0, 300.0, 1.0)
        };
        builder.set_min_numeric_value(min);
        builder.set_max_numeric_value(max);
        builder.set_numeric_value_step(step);
        builder.add_action(Action::SetValue);
        if *value < max {
            builder.add_action(Action::Increment);
        }
        if *value > min {
            builder.add_action(Action::Decrement);
        }
    });
    response.on_hover_text(format!(
        "{name}: {value:.0}. Drag or use arrow keys. Double-click to reset to 140."
    ))
}
