//! Time Stretcher presentation. Rendering and file operations stay in Session.
use super::*;

const CREAM: Color32 = Color32::from_rgb(245, 232, 205);
const INK: Color32 = Color32::from_rgb(24, 25, 23);
const UV: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
/// The base under the artwork continues the skin's outer bezel.
const BODY: Color32 = Color32::from_rgb(44, 46, 46);
const BODY_EDGE: Color32 = Color32::from_rgb(12, 12, 11);
/// Inset display wells: the status footer and the selector lists.
const WELL: Color32 = Color32::from_rgb(24, 37, 38);
const WELL_LINE: Color32 = Color32::from_rgb(68, 95, 96);
const WELL_DEEP: Color32 = Color32::from_rgb(14, 15, 15);
const WELL_HOVER: Color32 = Color32::from_rgb(40, 60, 61);
const WELL_SELECTED: Color32 = Color32::from_rgb(58, 84, 85);
const ERROR: Color32 = Color32::from_rgb(255, 167, 143);
const LAMP_RIM: Color32 = Color32::from_rgb(128, 123, 110);
/// From a button's left edge to where lettering may begin: the lamp sits 42
/// design units in, its rim reaches 12.25 further, and 10 stay clear.
const LAMP_CLEARANCE: f32 = 64.25;

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
        let textures = [pngs!("panel"), pngs!("collapsed")]
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
    pub bpm: f64,
}

pub(super) struct Feedback<'a> {
    pub processing: bool,
    pub listening: bool,
    pub saved: bool,
    pub audition: Option<(u32, f64, f64)>,
    pub message: Option<&'a str>,
    pub error: bool,
}

pub(super) enum Action {
    None,
    Select(u32),
    Listen,
    Stop,
    Save,
    Details,
    Help,
}

/// The 640 by 320-point hardware skin has an 80-point native status footer.
pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    feedback: Feedback<'_>,
    original: &mut f64,
    target: &mut f64,
) -> Result<Action, String> {
    let art = Artwork::load(ctx)?;
    let scale = (640.0 / 880.0) * [1.0, 1.5, 2.0][art.density] / ctx.pixels_per_point();
    let collapse_id = egui::Id::new("stretch-collapsed");
    let collapsed = ctx
        .data(|data| data.get_temp::<bool>(collapse_id))
        .unwrap_or(false);
    let mut action = Action::None;
    egui::Window::new("TIME STRETCHER")
        .id(egui::Id::new("Time Stretcher"))
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .default_pos(ctx.content_rect().center() - Vec2::new(440.0, 275.0) * scale)
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            let (bounds, _) = ui.allocate_exact_size(
                Vec2::new(880.0, if collapsed { 88.0 } else { 550.0 }) * scale,
                Sense::hover(),
            );
            let rect = |x, y, w, h| {
                Rect::from_min_size(
                    bounds.min + Vec2::new(x, y) * scale,
                    Vec2::new(w, h) * scale,
                )
            };
            let point = |x, y| bounds.min + Vec2::new(x, y) * scale;
            // Each state has its own complete bezel at the selected density.
            let painter = ui.painter().with_clip_rect(bounds);
            let mut panel_ui = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
            panel_ui.set_clip_rect(bounds);
            if !collapsed {
                // One body: the base continues the bezel under the footer and
                // fills the skin's transparent lower corners.
                let radius = (24.75 * scale).round() as u8;
                let corners = egui::CornerRadius { nw: 0, ne: 0, sw: radius, se: radius };
                let body = rect(0.0, 400.0, 880.0, 150.0);
                painter.rect_filled(body, corners, BODY);
                painter.rect_stroke(body, corners, Stroke::new(1.0_f32, BODY_EDGE), egui::StrokeKind::Inside);
            }
            if collapsed {
                art.paint(&panel_ui, 1, point(440.0, 44.0), Color32::WHITE);
            } else {
                art.paint(&panel_ui, 0, point(440.0, 220.0), Color32::WHITE);
            }
            label(
                ui,
                point(452.0, 42.0),
                "TIME STRETCHER",
                40.0 * scale,
                CREAM,
                true,
                Align2::CENTER_CENTER,
            );
            // Esc belongs to the innermost thing in use. A list, a menu, or a
            // value being typed takes it first; so does a window above this one.
            let guard_id = egui::Id::new("stretch-escape-guard");
            let guarded = ctx.data(|data| data.get_temp::<bool>(guard_id)).unwrap_or(false);
            let mut editing = false;
            if !collapsed {
            let mut selected = source.id;
            at(ui, rect(76.0, 84.0, 731.0, 37.0), |ui| {
                control_style(ui, 24.0 * scale, CREAM);
                let sounds = egui::ComboBox::from_id_salt("stretch-source")
                    .width(727.0 * scale)
                    .truncate()
                    .height(380.0 * scale)
                    .popup_style(list_style(scale))
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .selected_text(source.name)
                    .show_ui(ui, |ui| {
                        ui.set_width(710.0 * scale);
                        // Every opening starts with an empty, focused search.
                        let search_id = ui.id().with("source-search");
                        let seen_id = ui.id().with("source-search-seen");
                        let frame = ui.ctx().cumulative_frame_nr();
                        let opened = ui.ctx().data(|data| data.get_temp::<u64>(seen_id)).is_none_or(|seen| frame > seen + 1);
                        ui.ctx().data_mut(|data| data.insert_temp(seen_id, frame));
                        let mut search = if opened { String::new() } else {
                            ui.ctx().data(|data| data.get_temp::<String>(search_id)).unwrap_or_default()
                        };
                        let search_response = ui.add(
                            egui::TextEdit::singleline(&mut search)
                                .hint_text("Find source sound")
                                .desired_width(ui.available_width()),
                        );
                        if opened {
                            search_response.request_focus();
                        }
                        name_control(search_response, "Find source sound");
                        ui.ctx().data_mut(|data| data.insert_temp(search_id, search.clone()));
                        let query = search.trim().to_lowercase();
                        let matches: Vec<_> = source.samples.iter().filter(|sample| query.is_empty() || sample.name.to_lowercase().contains(&query)).collect();
                        if matches.is_empty() {
                            ui.label("No matching sounds. Clear the search or enter another name.");
                        } else {
                            let row_height = ui.spacing().interact_size.y.max(30.0 * scale);
                            egui::ScrollArea::vertical()
                                .id_salt(("filtered-sources", &query))
                                .max_height(300.0 * scale)
                                .show_rows(ui, row_height, matches.len(), |ui, rows| {
                                    for index in rows {
                                        let sample = matches[index];
                                        if ui.add_sized(
                                            [ui.available_width(), row_height],
                                            egui::Button::selectable(selected == Some(sample.id), &sample.name).truncate(),
                                        ).on_hover_text(&sample.name).clicked() {
                                            selected = Some(sample.id);
                                            ui.close();
                                        }
                                    }
                                });
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
                let value = ui.put(
                    rect(293.0, 145.0, 90.0, 39.0),
                    egui::DragValue::new(original)
                        .range(30.0..=300.0)
                        .speed(1.0),
                );
                editing = value.has_focus();
                value
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
            egui::Popup::menu(&target_response).style(list_style(scale)).show(|ui| {
                ui.set_min_width(target_response.rect.width() - 2.0 * ui.spacing().menu_margin.leftf());
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
            rotary(ui, rect(278.0, 195.0, 120.0, 114.0), original, false, source.bpm, scale);
            rotary(ui, rect(636.0, 195.0, 120.0, 114.0), target, true, 140.0, scale);
            let previewed = feedback.audition.is_some_and(|(id, audition_original, audition_target)| {
                source.id == Some(id) && *original == audition_original && *target == audition_target
            });
            let mut idle = false;
            let status = if source.missing {
                "Missing audio. Open Import details to resolve the sound.".to_owned()
            } else if let Some(message) = feedback.message {
                message.to_owned()
            } else if feedback.processing {
                "Processing sound...".to_owned()
            } else if feedback.listening {
                let (_, audition_original, audition_target) = feedback.audition.unwrap_or((0, *original, *target));
                format!("Playing {audition_original:.0} to {audition_target:.0} BPM.{}",
                    if previewed { "" } else { " Settings changed since this preview." })
            } else if source.id.is_none() {
                "Select a source sound to begin.".to_owned()
            } else if *original == *target {
                "Same tempo. Duration is unchanged.".to_owned()
            } else {
                idle = true;
                format!("Duration {:+.0}%. Pitch preservation is approximate.{}", (*original / *target - 1.0) * 100.0,
                    if previewed { "" } else { " Listen before saving." })
            };
            let footer_rect = rect(16.0, 452.0, 848.0, 82.0);
            painter.rect_filled(footer_rect, 6.0 * scale, WELL);
            painter.rect_stroke(footer_rect, 6.0 * scale,
                Stroke::new(1.0_f32, WELL_LINE), egui::StrokeKind::Inside);
            let footer = ui.interact(footer_rect, ui.id().with("status-footer"), Sense::hover());
            named(&footer, egui::WidgetType::Panel, "Time Stretcher status");
            // An error keeps its centered text and adds a mark at the left edge.
            let status_rect = if source.missing {
                rect(30.0, 464.0, 650.0, 58.0)
            } else if feedback.error {
                rect(66.0, 464.0, 748.0, 58.0)
            } else {
                rect(30.0, 464.0, 820.0, 58.0)
            };
            if feedback.error && !source.missing {
                let mark = point(44.0, 493.0);
                painter.circle_stroke(mark, 11.0 * scale, Stroke::new(2.0 * scale, ERROR));
                painter.line_segment([mark - Vec2::new(0.0, 6.0) * scale, mark + Vec2::new(0.0, 1.5) * scale],
                    Stroke::new(2.5 * scale, ERROR));
                painter.circle_filled(mark + Vec2::new(0.0, 5.5) * scale, 1.5 * scale, ERROR);
            }
            let mut status_ui = ui.new_child(egui::UiBuilder::new()
                .max_rect(status_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            status_ui.set_clip_rect(status_rect);
            let previous_id = ui.id().with("previous-status");
            let changed = ui.ctx().data(|data| data.get_temp::<String>(previous_id).as_deref() != Some(status.as_str()));
            ui.ctx().data_mut(|data| data.insert_temp(previous_id, status.clone()));
            let mut scroll = egui::ScrollArea::vertical().id_salt("stretch-status")
                .max_height(status_rect.height()).auto_shrink([false, false]);
            if changed { scroll = scroll.vertical_scroll_offset(0.0); }
            scroll.show(&mut status_ui, |ui| {
                let color = if feedback.error { ERROR } else { CREAM };
                let mut job = egui::text::LayoutJob::simple(status.clone(),
                    FontId::proportional(20.0 * scale), color, ui.available_width());
                job.halign = egui::Align::Center;
                let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
                let (text_rect, response) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), galley.mesh_bounds.height().max(status_rect.height())), Sense::hover());
                let origin = if galley.mesh_bounds.height() <= status_rect.height() {
                    text_rect.center() - galley.mesh_bounds.center().to_vec2()
                } else {
                    Pos2::new(text_rect.center().x - galley.mesh_bounds.center().x,
                        text_rect.top() - galley.mesh_bounds.top())
                };
                ui.painter().galley(origin, galley, color);
                // Assistive technology hears each change, and an error by name.
                let spoken = if feedback.error { format!("Error: {status}") } else { status.clone() };
                named(&response, egui::WidgetType::Label, &spoken);
                ui.ctx().accesskit_node_builder(response.id, |builder| {
                    builder.set_live(egui::accesskit::Live::Polite);
                });
                response.on_hover_text(if idle {
                    format!("{status}\nThe result can change in tone or develop audible artifacts. Open Help for details.")
                } else {
                    status.clone()
                });
            });
            if source.missing {
                ui.scope(|ui| {
                    ui.spacing_mut().button_padding = Vec2::new(3.0 * scale, 0.0);
                    ui.spacing_mut().interact_size.y = 18.0 * scale;
                    let details_rect = rect(695.0, 475.0, 139.0, 36.0);
                    let details = ui.put(details_rect, egui::Button::new(""));
                    named(&details, egui::WidgetType::Button, "Import details");
                    let text = ui.painter().layout_no_wrap("Import details".into(), FontId::proportional(17.0 * scale), CREAM);
                    ui.painter().galley(details_rect.center() - text.mesh_bounds.center().to_vec2(), text, CREAM);
                    if details.clicked() {
                        action = Action::Details;
                    }
                });
            }
            let enabled = source.id.is_some() && !source.missing && !feedback.processing;
            let listen_enabled = enabled || feedback.listening;
            let unavailable = if feedback.processing {
                "Wait for sound processing to finish."
            } else {
                "Select an available sound to process it."
            };
            if action_button(
                ui,
                rect(62.0, 328.0, 374.0, 80.0),
                if feedback.listening { "Stop listening" } else { "Listen to stretched sound" },
                if feedback.listening { "Stop listening" } else if feedback.processing { "Processing..." } else { "Listen" },
                listen_enabled,
                unavailable,
                scale,
            )
            .clicked()
            {
                action = if feedback.listening { Action::Stop } else { Action::Listen };
            }
            if action_button(
                ui,
                rect(446.0, 328.0, 374.0, 80.0),
                "Save stretched sound",
                "Save stretched sound",
                enabled,
                unavailable,
                scale,
            )
            .clicked()
            {
                action = Action::Save;
            }
            // The ivory face needs a deeper veil than the black one to read as off.
            for (button, enabled, veil) in [
                (rect(64.0, 330.0, 370.0, 76.0), listen_enabled, 95),
                (rect(448.0, 330.0, 370.0, 76.0), enabled, 150),
            ] {
                if !enabled {
                    painter.rect_filled(button, 10.0 * scale, Color32::from_black_alpha(veil));
                }
            }
            lamp(&painter, point(104.0, 365.0), scale, feedback.processing || feedback.listening, false);
            lamp(&painter, point(488.0, 365.0), scale, feedback.saved, true);
            }
            // Window controls come last in the tab order, after the work.
            let help = ui.interact(
                rect(772.0, 20.0, 44.0, 44.0),
                ui.id().with("help"),
                Sense::click(),
            );
            help_icon(ui.painter(), help.rect.center(), scale);
            named(&help, egui::WidgetType::Button, "Time Stretcher help");
            focus(ui, &help, scale);
            if help.on_hover_text("How stretching changes duration and sound").clicked() {
                action = Action::Help;
            }
            let collapse = ui.interact(
                rect(12.0, 20.0, 44.0, 44.0),
                ui.id().with("collapse"),
                Sense::click(),
            );
            let collapse_name = if collapsed {
                "Expand Time Stretcher"
            } else {
                "Collapse Time Stretcher"
            };
            named(&collapse, egui::WidgetType::Button, collapse_name);
            let center = collapse.rect.center();
            let direction = if collapsed { -1.0 } else { 1.0 };
            ui.painter().add(egui::Shape::convex_polygon(
                vec![
                    center + Vec2::new(-11.0, -6.0 * direction) * scale,
                    center + Vec2::new(11.0, -6.0 * direction) * scale,
                    center + Vec2::new(0.0, 9.0 * direction) * scale,
                ],
                CREAM,
                Stroke::NONE,
            ));
            focus(ui, &collapse, scale);
            if collapse.on_hover_text(collapse_name).clicked() {
                ctx.data_mut(|data| data.insert_temp(collapse_id, !collapsed));
            }
            let close = ui.interact(rect(824.0, 20.0, 44.0, 44.0), ui.id().with("close"), Sense::click());
            named(&close, egui::WidgetType::Button, "Close Time Stretcher");
            for diagonal in [-1.0, 1.0] {
                let offset = Vec2::new(10.0, 10.0 * diagonal) * scale;
                ui.painter().line_segment([close.rect.center() - offset, close.rect.center() + offset],
                    Stroke::new(2.5 * scale, CREAM));
            }
            focus(ui, &close, scale);
            let layer = ui.layer_id();
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
            if close.on_hover_text("Close Time Stretcher (Esc)").clicked()
                || (on_top && !guarded && ctx.input(|input| input.key_pressed(egui::Key::Escape)))
            {
                *open = false;
            }
            let guard = editing || egui::Popup::is_any_open(ctx);
            ctx.data_mut(|data| data.insert_temp(guard_id, guard));
        });
    Ok(action)
}

/// A drawn question mark keeps Help independent of text glyphs and baked artwork.
fn help_icon(painter: &egui::Painter, center: Pos2, scale: f32) {
    let stroke = Stroke::new(2.0 * scale, CREAM);
    painter.circle_stroke(center, 12.0 * scale, stroke);
    let points = [
        (-4.5, -4.0),
        (-4.0, -6.5),
        (-1.0, -7.5),
        (2.5, -7.0),
        (4.5, -5.0),
        (4.5, -2.5),
        (2.5, -0.5),
        (0.0, 1.0),
        (0.0, 3.0),
    ];
    painter.add(egui::Shape::line(
        points
            .into_iter()
            .map(|(x, y)| center + Vec2::new(x, y) * scale)
            .collect(),
        stroke,
    ));
    painter.circle_filled(center + Vec2::new(0.0, 7.0) * scale, 1.3 * scale, CREAM);
}

/// Lists and menus keep the display-well material and the panel's type size.
fn list_style(scale: f32) -> egui::style::StyleModifier {
    egui::style::StyleModifier::new(move |style| {
        style.override_font_id = Some(FontId::proportional(20.0 * scale));
        style.wrap_mode = Some(egui::TextWrapMode::Truncate);
        style.spacing.interact_size.y = 30.0 * scale;
        let visuals = &mut style.visuals;
        visuals.override_text_color = Some(CREAM);
        visuals.window_fill = WELL;
        visuals.panel_fill = WELL;
        visuals.window_stroke = Stroke::new(1.0_f32, WELL_LINE);
        visuals.extreme_bg_color = WELL_DEEP;
        visuals.selection.bg_fill = WELL_SELECTED;
        visuals.selection.stroke = Stroke::new(1.0_f32, CREAM);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, WELL_LINE);
        visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        for widget in [&mut visuals.widgets.hovered, &mut visuals.widgets.active] {
            widget.bg_fill = WELL_HOVER;
            widget.weak_bg_fill = WELL_HOVER;
            widget.bg_stroke = Stroke::new(1.0_f32, WELL_LINE);
        }
    })
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
        pos + shadow(ui.ctx()),
        align,
        text,
        font.clone(),
        Color32::BLACK,
    );
    ui.painter().text(pos, align, text, font, color);
}

/// One lettering shadow for the title, labels, and buttons: a whole number of
/// physical pixels at every prepared density.
fn shadow(ctx: &egui::Context) -> Vec2 {
    let ppp = ctx.pixels_per_point();
    Vec2::new(0.0, ppp.round().max(1.0) / ppp)
}

fn named(response: &Response, kind: egui::WidgetType, text: &str) {
    response.widget_info(|| egui::WidgetInfo::labeled(kind, response.enabled(), text));
}

/// Keyboard focus is a solid ring. Pointer hover is a faint one, so the two
/// never look alike when they rest on different controls.
fn focus(ui: &egui::Ui, response: &Response, scale: f32) {
    let stroke = if response.has_focus() {
        Stroke::new(2.5 * scale, CREAM)
    } else if response.hovered() && response.enabled() {
        Stroke::new(1.0_f32, CREAM.gamma_multiply(0.45))
    } else {
        return;
    };
    ui.painter().rect_stroke(
        response.rect.shrink(4.0 * scale),
        6.0 * scale,
        stroke,
        egui::StrokeKind::Inside,
    );
}

fn action_button(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    visible_text: &str,
    enabled: bool,
    unavailable: &str,
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
    // The lower bevel is wider than the upper bevel in the prepared skin.
    // Center the visible glyphs in the flat face, rather than its hit rectangle
    // or the font's line box, which includes unused ascender/descender space.
    let face = Rect::from_min_max(
        rect.min + Vec2::new(12.0, 12.0) * scale,
        rect.max - Vec2::new(12.0, 20.0) * scale,
    );
    let color = if text == "Save stretched sound" {
        INK
    } else {
        CREAM
    };
    // Unavailable lettering fades with its face.
    let color = if enabled {
        color
    } else {
        color.gamma_multiply(0.55)
    };
    let galley = ui.painter().layout_no_wrap(
        visible_text.into(),
        FontId::proportional(25.0 * scale),
        color,
    );
    let shadow = shadow(ui.ctx());
    let mut origin = face.center() - galley.mesh_bounds.center().to_vec2() - shadow / 2.0;
    // A long label moves right only as far as it must to stay clear of the lamp.
    let clear = rect.left() + LAMP_CLEARANCE * scale;
    origin.x += (clear - (origin.x + galley.mesh_bounds.left())).max(0.0);
    ui.painter()
        .galley_with_override_text_color(origin + shadow, galley.clone(), Color32::BLACK);
    ui.painter().galley(origin, galley, color);
    focus(ui, &response, scale);
    named(&response, egui::WidgetType::Button, text);
    response.on_hover_text(if enabled { text } else { unavailable })
}

fn lamp(painter: &egui::Painter, center: Pos2, scale: f32, active: bool, green: bool) {
    let color = match (active, green) {
        (true, true) => Color32::from_rgb(174, 231, 55),
        (true, false) => Color32::from_rgb(255, 184, 47),
        (false, true) => Color32::from_rgb(74, 94, 38),
        (false, false) => Color32::from_rgb(112, 76, 26),
    };
    // A metal rim locates the lamp on the black face; the dark bezel inside
    // it carries a lit lamp against the ivory one.
    painter.circle_filled(center, 11.5 * scale, Color32::from_rgb(19, 20, 16));
    painter.circle_stroke(center, 11.5 * scale, Stroke::new(1.5 * scale, LAMP_RIM));
    painter.circle_filled(center, 7.5 * scale, color);
    painter.circle_filled(
        center + Vec2::new(-2.0, -2.5) * scale,
        2.0 * scale,
        if active {
            CREAM
        } else {
            color.gamma_multiply(1.35)
        },
    );
}

fn step_target(value: f64, direction: f64) -> f64 {
    let index = SUPPORTED_BPMS
        .iter()
        .position(|bpm| *bpm == value)
        .unwrap_or(1);
    SUPPORTED_BPMS[(index as isize + direction.signum() as isize).clamp(0, 3) as usize]
}

fn rotary(
    ui: &mut egui::Ui,
    rect: Rect,
    value: &mut f64,
    target: bool,
    reset: f64,
    scale: f32,
) -> Response {
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
            .data_mut(|data| data.insert_temp(response.id.with("draft"), before));
    }
    if response.dragged() {
        let draft = ui
            .ctx()
            .data(|data| data.get_temp::<f64>(response.id.with("draft")))
            .unwrap_or(before);
        // Accumulate the unrounded value. Fine movement survives rounding, and
        // changing Shift affects only new movement during the same drag.
        let delta = response.drag_delta();
        // Target holds only a short overshoot, so reversing a drag past either
        // end takes effect almost at once.
        let (low, high) = if target {
            (110.0, 190.0)
        } else {
            (30.0, 300.0)
        };
        let draft = (draft
            + f64::from((delta.x - delta.y) / scale)
                * if ui.input(|i| i.modifiers.shift) {
                    0.1
                } else {
                    1.0
                })
        .clamp(low, high);
        ui.ctx()
            .data_mut(|data| data.insert_temp(response.id.with("draft"), draft));
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
        *value = reset.clamp(30.0, 300.0);
    }
    if before != *value {
        response.mark_changed();
    }
    let center = rect.center();
    let angle = if target {
        -225.0 + ((*value - 120.0) / 60.0) as f32 * 270.0
    } else {
        // One degree per BPM across the skin's evenly spaced 270-degree ring.
        -225.0 + (*value - 30.0) as f32
    };
    let vector = Vec2::angled(angle.to_radians());
    if target {
        for (index, _) in SUPPORTED_BPMS.iter().enumerate() {
            let tick = Vec2::angled((-225.0 + index as f32 * 90.0).to_radians());
            ui.painter().line_segment(
                [center + tick * 49.0 * scale, center + tick * 57.0 * scale],
                Stroke::new(3.0 * scale, CREAM),
            );
        }
    }
    ui.painter().line_segment(
        [
            center + vector * 19.0 * scale,
            center + vector * 41.0 * scale,
        ],
        Stroke::new(4.0 * scale, CREAM),
    );
    if response.has_focus() {
        ui.painter()
            .circle_stroke(center, 47.0 * scale, Stroke::new(2.5 * scale, CREAM));
    } else if response.hovered() {
        ui.painter().circle_stroke(
            center,
            47.0 * scale,
            Stroke::new(1.0_f32, CREAM.gamma_multiply(0.45)),
        );
    }
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
    let reset_help = if target {
        "Double-click to reset the target to 140 BPM.".to_owned()
    } else {
        format!(
            "Double-click to restore the selected source tempo ({:.0} BPM).",
            reset.clamp(30.0, 300.0)
        )
    };
    response.on_hover_text(format!(
        "{name}: {value:.0}. Drag or use arrow keys. Hold Shift for fine movement. {reset_help}"
    ))
}
