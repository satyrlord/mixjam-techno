//! Effect Studio hardware presentation. Processing and file operations stay in Session.
use super::*;

const CREAM: Color32 = Color32::from_rgb(245, 232, 205);
const INK: Color32 = Color32::from_rgb(24, 25, 23);
const BODY: Color32 = Color32::from_rgb(44, 46, 46);
const WELL: Color32 = Color32::from_rgb(30, 23, 21);
const ERROR: Color32 = Color32::from_rgb(255, 167, 143);
const UV: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));

macro_rules! pngs {
    ($name:literal) => {
        [
            include_bytes!(concat!("../../assets/effects/100/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/effects/150/", $name, ".png")).as_slice(),
            include_bytes!(concat!("../../assets/effects/200/", $name, ".png")).as_slice(),
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
        let cache_id = egui::Id::new(("effects-artwork", density));
        if let Some(cached) = ctx.data(|data| data.get_temp::<Arc<Self>>(cache_id)) {
            return Ok(cached);
        }
        let textures = [
            pngs!("panel"),
            pngs!("collapsed"),
            pngs!("toggle-off"),
            pngs!("toggle-on"),
            pngs!("knob"),
        ]
        .iter()
        .enumerate()
        .map(|(index, variants)| {
            let image = image::load_from_memory(variants[density])
                .map_err(|error| format!("Cannot load Effect Studio artwork: {error}"))?
                .into_rgba8();
            Ok(ctx.load_texture(
                format!("effects-{density}-{index}"),
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
}

pub(super) struct Feedback<'a> {
    pub message: Option<&'a str>,
    pub error: bool,
}

pub(super) enum Action {
    None,
    Listen,
    Save,
    Reset,
    Details,
    Help,
}

/// The 640 by 352-point panel has a 48-point status footer.
pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    feedback: Feedback<'_>,
    state: &mut EffectsState,
) -> Result<Action, String> {
    let art = Artwork::load(ctx)?;
    let scale = [1.0, 1.5, 2.0][art.density] / ctx.pixels_per_point();
    let collapse_id = egui::Id::new("effects-collapsed");
    let collapsed = ctx
        .data(|data| data.get_temp::<bool>(collapse_id))
        .unwrap_or(false);
    let mut action = Action::None;
    egui::Window::new("EFFECT STUDIO")
        .id(egui::Id::new("Effect Studio"))
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .default_pos(ctx.content_rect().center() - Vec2::new(320.0, 200.0) * scale)
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let (bounds, _) = ui.allocate_exact_size(
                Vec2::new(640.0, if collapsed { 48.0 } else { 400.0 }) * scale,
                Sense::hover(),
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
            if !collapsed {
                panel.painter().rect_filled(
                    rect(0.0, 336.0, 640.0, 64.0),
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
                point(320.0, if collapsed { 24.0 } else { 176.0 }),
            );
            lettering(
                &panel,
                point(320.0, 24.0),
                "EFFECT STUDIO",
                22.0 * scale,
                CREAM,
                true,
            );
            let guard_id = egui::Id::new("effects-escape-guard");
            let guarded = ctx
                .data(|data| data.get_temp::<bool>(guard_id))
                .unwrap_or(false);
            let mut editing = false;
            if !collapsed {
                panel.scope(|ui| {
                    let text = egui::RichText::new(source.name)
                        .font(FontId::proportional(13.0 * scale))
                        .color(CREAM);
                    let label = ui.put(
                        rect(27.0, 48.0, 586.0, 22.0),
                        egui::Label::new(text).truncate(),
                    );
                    named(&label, egui::WidgetType::Label, "Source sound");
                    ui.ctx().accesskit_node_builder(label.id, |builder| {
                        builder.set_label("Source sound");
                        builder.set_value(source.name);
                    });
                    label.on_hover_text(source.name);
                });

                let mut filter_enabled = state.filter.is_some();
                if toggle(
                    &mut panel,
                    &art,
                    point(98.0, 89.0),
                    scale,
                    "Filter",
                    &mut filter_enabled,
                ) {
                    state.filter = filter_enabled.then(Default::default);
                }
                section(&panel, point(30.0, 89.0), "Filter", scale);
                let mut filter = state.filter.clone().unwrap_or_default();
                for (x, value, name, low, high, reset, step, decimals, logarithmic) in [
                    (
                        65.5,
                        &mut filter.cutoff_hz,
                        "Cutoff",
                        20.0,
                        20000.0,
                        2000.0,
                        1.0,
                        0,
                        true,
                    ),
                    (
                        147.0,
                        &mut filter.resonance,
                        "Resonance",
                        0.0,
                        1.0,
                        0.0,
                        0.001,
                        3,
                        false,
                    ),
                    (
                        229.5,
                        &mut filter.drive,
                        "Overdrive",
                        0.0,
                        1.0,
                        0.0,
                        0.001,
                        3,
                        false,
                    ),
                    (
                        312.0,
                        &mut filter.wet,
                        "Filter volume",
                        0.0,
                        1.0,
                        1.0,
                        0.001,
                        3,
                        false,
                    ),
                ] {
                    editing |= control(
                        &mut panel,
                        &art,
                        [point(x, 117.0), point(x, 144.0), point(x, 163.0)],
                        scale,
                        value,
                        Control {
                            name,
                            low,
                            high,
                            reset,
                            step,
                            decimals,
                            logarithmic,
                        },
                        filter_enabled,
                    );
                }
                if filter_enabled {
                    state.filter = Some(filter);
                }

                let mut echo_enabled = state.echo.is_some();
                if toggle(
                    &mut panel,
                    &art,
                    point(452.0, 89.0),
                    scale,
                    "Echo",
                    &mut echo_enabled,
                ) {
                    state.echo = echo_enabled.then(Default::default);
                }
                section(&panel, point(390.0, 89.0), "Echo", scale);
                let mut echo = state.echo.clone().unwrap_or_default();
                for (x, value, name, low, high, reset, step, decimals) in [
                    (
                        423.0,
                        &mut echo.delay_beats,
                        "Delay (beats)",
                        0.001,
                        2.0,
                        0.5,
                        0.01,
                        3,
                    ),
                    (
                        500.0,
                        &mut echo.feedback,
                        "Feedback",
                        0.0,
                        0.95,
                        0.4,
                        0.01,
                        2,
                    ),
                    (577.5, &mut echo.wet, "Echo volume", 0.0, 1.0, 0.35, 0.01, 2),
                ] {
                    editing |= control(
                        &mut panel,
                        &art,
                        [point(x, 117.0), point(x, 144.0), point(x, 163.0)],
                        scale,
                        value,
                        Control {
                            name,
                            low,
                            high,
                            reset,
                            step,
                            decimals,
                            logarithmic: false,
                        },
                        echo_enabled,
                    );
                }
                if echo_enabled {
                    state.echo = Some(echo);
                }

                let mut robot_enabled = state.robotize.is_some();
                if toggle(
                    &mut panel,
                    &art,
                    point(116.0, 190.0),
                    scale,
                    "Robotize",
                    &mut robot_enabled,
                ) {
                    state.robotize = robot_enabled.then(Default::default);
                }
                section(&panel, point(30.0, 185.0), "Robotize", scale);
                let mut robot = state.robotize.clone().unwrap_or_default();
                for (x, value, name, low, high, reset, step, decimals, logarithmic) in [
                    (
                        93.0,
                        &mut robot.frequency_hz,
                        "Color",
                        1.0,
                        2000.0,
                        80.0,
                        1.0,
                        0,
                        true,
                    ),
                    (
                        196.0,
                        &mut robot.wet,
                        "Robotize volume",
                        0.0,
                        1.0,
                        0.5,
                        0.01,
                        2,
                        false,
                    ),
                ] {
                    editing |= control(
                        &mut panel,
                        &art,
                        [point(x, 216.0), point(x, 244.0), point(x, 263.0)],
                        scale,
                        value,
                        Control {
                            name,
                            low,
                            high,
                            reset,
                            step,
                            decimals,
                            logarithmic,
                        },
                        robot_enabled,
                    );
                }
                if robot_enabled {
                    state.robotize = Some(robot);
                }

                section(&panel, point(296.0, 185.0), "Distortion / output", scale);
                for (x, value, name, high, reset, step, decimals) in [
                    (
                        340.5,
                        &mut state.dirty,
                        "Dirty distortion",
                        1.0,
                        0.0,
                        0.001,
                        3,
                    ),
                    (
                        449.0,
                        &mut state.noise,
                        "Noise distortion",
                        1.0,
                        0.0,
                        0.001,
                        3,
                    ),
                    (558.0, &mut state.volume, "Output volume", 2.0, 1.0, 0.01, 2),
                ] {
                    editing |= control(
                        &mut panel,
                        &art,
                        [point(x, 216.0), point(x, 244.0), point(x, 263.0)],
                        scale,
                        value,
                        Control {
                            name,
                            low: 0.0,
                            high,
                            reset,
                            step,
                            decimals,
                            logarithmic: false,
                        },
                        true,
                    );
                }
                let available = source.available && !source.missing;
                for (x, width, text, next, enabled) in [
                    (48.0, 164.0, "Listen", Action::Listen, available),
                    (232.0, 177.0, "Save sound", Action::Save, available),
                    (430.0, 165.0, "Reset", Action::Reset, true),
                ] {
                    if action_button(
                        &mut panel,
                        rect(x, 284.0, width, 36.0),
                        text,
                        enabled,
                        scale,
                    )
                    .clicked()
                    {
                        action = next;
                    }
                }
                let status = if source.missing {
                    "Missing audio. Open Import details to resolve the sound."
                } else if !source.available {
                    "Select a sound in the palette to begin."
                } else {
                    feedback
                        .message
                        .unwrap_or("Listen before saving. Save sound adds a new WAV to Effect.")
                };
                let status_rect = rect(8.0, 358.0, 624.0, 36.0);
                status_footer(
                    &mut panel,
                    status_rect,
                    status,
                    feedback.error,
                    source.missing,
                    scale,
                );
                if source.missing {
                    let details = panel.put(rect(526.0, 361.0, 94.0, 30.0), egui::Button::new(""));
                    named(&details, egui::WidgetType::Button, "Import details");
                    lettering(
                        &panel,
                        details.rect.center(),
                        "Import details",
                        12.0 * scale,
                        CREAM,
                        false,
                    );
                    if details.clicked() {
                        action = Action::Details;
                    }
                }
                let help = panel.interact(
                    rect(571.0, 11.0, 26.0, 26.0),
                    panel.id().with("help"),
                    Sense::click(),
                );
                help_icon(panel.painter(), point(584.0, 24.0), scale);
                named(&help, egui::WidgetType::Button, "Effect Studio help");
                focus(&panel, &help, scale);
                if help.on_hover_text("How to use Effect Studio").clicked() {
                    action = Action::Help;
                }
            }
            let collapse = panel.interact(
                rect(7.0, 11.0, 26.0, 26.0),
                panel.id().with("collapse"),
                Sense::click(),
            );
            let collapse_name = if collapsed {
                "Expand Effect Studio"
            } else {
                "Collapse Effect Studio"
            };
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
            let close = panel.interact(
                rect(607.0, 11.0, 26.0, 26.0),
                panel.id().with("close"),
                Sense::click(),
            );
            named(&close, egui::WidgetType::Button, "Close Effect Studio");
            for diagonal in [-1.0, 1.0] {
                let offset = Vec2::new(6.0, 6.0 * diagonal) * scale;
                panel.painter().line_segment(
                    [point(620.0, 24.0) - offset, point(620.0, 24.0) + offset],
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
            if close.on_hover_text("Close Effect Studio (Esc)").clicked()
                || (on_top && !guarded && ctx.input(|input| input.key_pressed(egui::Key::Escape)))
            {
                *open = false;
            }
            let guard = editing || egui::Popup::is_any_open(ctx);
            ctx.data_mut(|data| data.insert_temp(guard_id, guard));
        });
    Ok(action)
}

fn section(ui: &egui::Ui, at: Pos2, text: &str, scale: f32) {
    ui.painter().text(
        at,
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(13.0 * scale),
        CREAM,
    );
}

fn toggle(
    ui: &mut egui::Ui,
    art: &Artwork,
    center: Pos2,
    scale: f32,
    name: &str,
    enabled: &mut bool,
) -> bool {
    let mut response = ui.interact(
        Rect::from_center_size(center, Vec2::splat(24.0) * scale),
        ui.id().with(("toggle", name)),
        Sense::click(),
    );
    if response.clicked() {
        *enabled = !*enabled;
        response.mark_changed();
    }
    art.paint(ui, if *enabled { 3 } else { 2 }, center);
    let lamp = center + Vec2::new(20.0, 0.0) * scale;
    ui.painter()
        .circle_filled(lamp, 4.0 * scale, Color32::from_rgb(15, 17, 12));
    ui.painter().circle_stroke(
        lamp,
        4.0 * scale,
        Stroke::new(1.0 * scale, Color32::from_rgb(89, 85, 69)),
    );
    ui.painter().circle_filled(
        lamp,
        2.8 * scale,
        if *enabled {
            Color32::from_rgb(153, 240, 83)
        } else {
            Color32::from_rgb(40, 55, 31)
        },
    );
    if *enabled {
        ui.painter().circle_filled(
            lamp + Vec2::new(-0.8, -0.8) * scale,
            1.0 * scale,
            Color32::from_rgb(224, 255, 198),
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            response.enabled(),
            *enabled,
            name,
        )
    });
    focus(ui, &response, scale);
    let changed = response.changed();
    response.on_hover_text(format!(
        "{name}: {}. Turning it off discards its settings.",
        if *enabled { "on" } else { "off" }
    ));
    changed
}

#[derive(Clone, Copy)]
struct Control<'a> {
    name: &'a str,
    low: f32,
    high: f32,
    reset: f32,
    step: f32,
    decimals: usize,
    logarithmic: bool,
}

impl Control<'_> {
    fn fraction(self, value: f32) -> f64 {
        let value = value.clamp(self.low, self.high) as f64;
        if self.logarithmic {
            (value / self.low as f64).ln() / (self.high as f64 / self.low as f64).ln()
        } else {
            (value - self.low as f64) / (self.high - self.low) as f64
        }
    }

    fn value(self, fraction: f64) -> f32 {
        let fraction = fraction.clamp(0.0, 1.0);
        if self.logarithmic {
            (self.low as f64 * (self.high as f64 / self.low as f64).powf(fraction)) as f32
        } else {
            (self.low as f64 + fraction * (self.high - self.low) as f64) as f32
        }
    }

    /// One wheel notch, arrow key, or accessible step. A logarithmic control
    /// moves 1% of its dial, so a step turns the knob as far at either end.
    fn stepped(self, value: f32, direction: f32, fine: bool) -> f32 {
        let amount = direction * if fine { 0.1 } else { 1.0 };
        if !self.logarithmic {
            return (value + amount * self.step).clamp(self.low, self.high);
        }
        let next = self.value(self.fraction(value) + f64::from(amount) * 0.01);
        if fine {
            return next;
        }
        let precision = 10.0_f32.powi(self.decimals as i32);
        let next = (next * precision).round() / precision;
        // Rounding must not hold the low end of the dial in place.
        if (next - value).abs() < self.step {
            (value + direction * self.step).clamp(self.low, self.high)
        } else {
            next
        }
    }
}

fn control(
    ui: &mut egui::Ui,
    art: &Artwork,
    positions: [Pos2; 3],
    scale: f32,
    value: &mut f32,
    spec: Control<'_>,
    enabled: bool,
) -> bool {
    let [center, well, label] = positions;
    art.paint(ui, 4, center);
    for index in 0..=18 {
        let vector = Vec2::angled((-225.0 + index as f32 * 15.0).to_radians());
        ui.painter().line_segment(
            [
                center + vector * if index % 3 == 0 { 18.0 } else { 19.5 } * scale,
                center + vector * 21.0 * scale,
            ],
            Stroke::new(1.0 * scale, CREAM),
        );
    }
    let well_rect = Rect::from_center_size(well, Vec2::new(58.0, 18.0) * scale);
    ui.painter()
        .rect_filled(well_rect, 3.0 * scale, Color32::from_rgb(12, 14, 13));
    ui.painter().rect_stroke(
        well_rect,
        3.0 * scale,
        Stroke::new(1.0_f32, Color32::from_rgb(87, 73, 64)),
        egui::StrokeKind::Inside,
    );
    let color = if enabled {
        CREAM
    } else {
        CREAM.gamma_multiply(0.45)
    };
    lettering(ui, label, spec.name, 12.0 * scale, color, false);
    let mut editing = false;
    ui.add_enabled_ui(enabled, |ui| {
        rotary(ui, center, scale, value, spec);
        let style = ui.style_mut();
        style.override_font_id = Some(FontId::proportional(14.0 * scale));
        style.visuals.override_text_color = Some(color);
        style.spacing.button_padding = Vec2::ZERO;
        style.spacing.interact_size.y = 16.0 * scale;
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
        let response = ui.put(
            Rect::from_center_size(well, Vec2::new(56.0, 16.0) * scale),
            egui::DragValue::new(value)
                .range(spec.low..=spec.high)
                .speed(spec.step)
                .fixed_decimals(spec.decimals),
        );
        editing = response.has_focus();
        name_control(response, &format!("{} value", spec.name))
            .on_hover_text(format!("{}: drag, or click to enter a value.", spec.name));
    });
    editing
}

fn rotary(ui: &mut egui::Ui, center: Pos2, scale: f32, value: &mut f32, spec: Control<'_>) {
    let rect = Rect::from_center_size(center, Vec2::splat(42.0) * scale);
    let mut response = ui.interact(
        rect,
        ui.id().with(("knob", spec.name)),
        Sense::click_and_drag(),
    );
    let before = *value;
    if response.enabled() {
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
        let draft_id = response.id.with("draft");
        if response.drag_started() {
            ui.ctx()
                .data_mut(|data| data.insert_temp(draft_id, spec.fraction(before)));
        }
        if response.dragged() {
            let draft = ui
                .ctx()
                .data(|data| data.get_temp::<f64>(draft_id))
                .unwrap_or_else(|| spec.fraction(before));
            let delta = ui.input(|input| input.pointer.delta());
            let fine = ui.input(|input| input.modifiers.shift);
            let draft = (draft
                + f64::from((delta.x - delta.y) / (150.0 * scale)) * if fine { 0.1 } else { 1.0 })
            .clamp(0.0, 1.0);
            ui.ctx().data_mut(|data| data.insert_temp(draft_id, draft));
            let precision = 10.0_f32.powi(spec.decimals as i32);
            *value = (spec.value(draft) * precision).round() / precision;
            *value = value.clamp(spec.low, spec.high);
        }
        let mut direction = 0.0_f32;
        if response.hovered() {
            direction = ui.input_mut(|input| {
                let delta = input.raw_scroll_delta.y;
                input.raw_scroll_delta = Vec2::ZERO;
                input.smooth_scroll_delta = Vec2::ZERO;
                if delta.abs() > 0.0 {
                    delta.signum()
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
                    *value = spec.low;
                }
                if input.consume_key(modifiers, egui::Key::End) {
                    *value = spec.high;
                }
            });
        }
        if direction != 0.0 {
            if response.has_focus() {
                ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
            }
            let fine = ui.input(|input| input.modifiers.shift);
            *value = spec.stepped(*value, direction, fine);
        }
        ui.input(|input| {
            use egui::accesskit::{Action, ActionData};
            for request in input.accesskit_action_requests(response.id, Action::SetValue) {
                if let Some(ActionData::NumericValue(draft)) = request.data
                    && draft.is_finite()
                {
                    *value = (draft as f32).clamp(spec.low, spec.high);
                }
            }
            let increment = input.num_accesskit_action_requests(response.id, Action::Increment);
            let decrement = input.num_accesskit_action_requests(response.id, Action::Decrement);
            let turns = increment as i32 - decrement as i32;
            for _ in 0..turns.abs() {
                *value = spec.stepped(*value, turns.signum() as f32, false);
            }
        });
        if response.double_clicked() {
            *value = spec.reset;
        }
    }
    if before != *value {
        response.mark_changed();
    }
    let vector = Vec2::angled((-225.0 + spec.fraction(*value) as f32 * 270.0).to_radians());
    ui.painter().line_segment(
        [
            center + vector * 6.0 * scale,
            center + vector * 12.0 * scale,
        ],
        Stroke::new(2.5 * scale, CREAM),
    );
    if !response.enabled() {
        ui.painter()
            .circle_filled(center, 23.0 * scale, Color32::from_black_alpha(115));
    } else if response.has_focus() || response.hovered() {
        ui.painter().circle_stroke(
            center,
            17.0 * scale,
            Stroke::new(
                if response.has_focus() {
                    2.0 * scale
                } else {
                    1.0
                },
                CREAM.gamma_multiply(if response.has_focus() { 1.0 } else { 0.45 }),
            ),
        );
    }
    response.widget_info(|| egui::WidgetInfo::slider(response.enabled(), *value as f64, spec.name));
    ui.ctx().accesskit_node_builder(response.id, |builder| {
        use egui::accesskit::Action;
        builder.set_min_numeric_value(spec.low as f64);
        builder.set_max_numeric_value(spec.high as f64);
        builder.set_numeric_value_step(spec.step as f64);
        if response.enabled() {
            builder.add_action(Action::SetValue);
            if *value < spec.high {
                builder.add_action(Action::Increment);
            }
            if *value > spec.low {
                builder.add_action(Action::Decrement);
            }
        }
    });
    response.on_hover_text(format!(
        "{}: drag or use arrow keys. Hold Shift for fine movement. Double-click to reset.",
        spec.name
    ));
}

fn lettering(ui: &egui::Ui, center: Pos2, text: &str, size: f32, color: Color32, title: bool) {
    let font = if title {
        FontId::new(size, egui::FontFamily::Name("Syncopate Bold".into()))
    } else {
        FontId::proportional(size)
    };
    let galley = ui.painter().layout_no_wrap(text.into(), font, color);
    let ppp = ui.ctx().pixels_per_point();
    let shadow = Vec2::new(0.0, ppp.round().max(1.0) / ppp);
    let origin = center - galley.mesh_bounds.center().to_vec2() - shadow / 2.0;
    ui.painter()
        .galley_with_override_text_color(origin + shadow, galley.clone(), Color32::BLACK);
    ui.painter().galley(origin, galley, color);
}

fn named(response: &Response, kind: egui::WidgetType, text: &str) {
    response.widget_info(|| egui::WidgetInfo::labeled(kind, response.enabled(), text));
}

fn focus(ui: &egui::Ui, response: &Response, scale: f32) {
    let stroke = if response.has_focus() {
        Stroke::new(2.0 * scale, CREAM)
    } else if response.hovered() && response.enabled() {
        Stroke::new(1.0_f32, CREAM.gamma_multiply(0.45))
    } else {
        return;
    };
    ui.painter().rect_stroke(
        response.rect.shrink(2.0 * scale),
        3.0 * scale,
        stroke,
        egui::StrokeKind::Inside,
    );
}

fn action_button(ui: &mut egui::Ui, rect: Rect, text: &str, enabled: bool, scale: f32) -> Response {
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.interact(rect, ui.id().with(text), Sense::click())
        })
        .inner;
    if !enabled || response.is_pointer_button_down_on() {
        ui.painter().rect_filled(
            rect.shrink(3.0 * scale),
            5.0 * scale,
            Color32::from_black_alpha(if enabled {
                45
            } else if text == "Save sound" {
                150
            } else {
                95
            }),
        );
    }
    let color = if text == "Save sound" { INK } else { CREAM };
    lettering(
        ui,
        rect.center(),
        text,
        13.0 * scale,
        if enabled {
            color
        } else {
            color.gamma_multiply(0.55)
        },
        false,
    );
    named(&response, egui::WidgetType::Button, text);
    focus(ui, &response, scale);
    response.on_hover_text(if !enabled {
        "Select an available sound to process it."
    } else if text == "Save sound" {
        "Save a new WAV and add it to Effect."
    } else {
        text
    })
}

fn status_footer(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    error: bool,
    missing: bool,
    scale: f32,
) {
    ui.painter().rect_filled(rect, 4.0 * scale, WELL);
    ui.painter().rect_stroke(
        rect,
        4.0 * scale,
        Stroke::new(1.0_f32, Color32::from_rgb(90, 65, 58)),
        egui::StrokeKind::Inside,
    );
    let response = ui.interact(rect, ui.id().with("status"), Sense::hover());
    named(&response, egui::WidgetType::Panel, "Effect Studio status");
    let text_rect = Rect::from_min_max(
        rect.min + Vec2::new(9.0, 6.0) * scale,
        rect.max - Vec2::new(if missing { 116.0 } else { 9.0 }, 6.0) * scale,
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
        .id_salt("effects-status")
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
        let (area, response) = ui.allocate_exact_size(
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
            &response,
            egui::WidgetType::Label,
            &if error {
                format!("Error: {text}")
            } else {
                text.into()
            },
        );
        ui.ctx().accesskit_node_builder(response.id, |builder| {
            builder.set_live(egui::accesskit::Live::Polite)
        });
        response.on_hover_text(text);
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
