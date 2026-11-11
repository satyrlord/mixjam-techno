//! The Sounds panel at the left of the score: the library's sounds as a list
//! with waveform thumbnails, and the filters the lower panel's Sounds module
//! also shows. The two never show at once (owner answer P3), so every control
//! keeps one name. The filter state and the sound actions are shared.
use super::view::{
    GROUPS, TILE_DETAIL_ALPHA, action_button, elided, focus_outline, group_color, heading,
    name_control, sample_details, tile_tint,
};
use super::*;
use mixjam_techno::application::PeakState;

/// The open panel's width, and the folded strip's.
pub(super) const SOUNDS_PANEL_WIDTH: f32 = 250.0;
pub(super) const SOUNDS_STRIP_WIDTH: f32 = 28.0;
/// One sound row and the gap below it.
const ROW_HEIGHT: f32 = 44.0;
const ROW_GAP: f32 = 4.0;
/// The waveform thumbnail at a row's right end.
const THUMBNAIL: Vec2 = Vec2::new(84.0, 30.0);
/// Below this panel height the category chips share one scrolling row, so
/// the list keeps room.
const WRAPPED_CHIPS_HEIGHT: f32 = 440.0;
/// The quietest peak a thumbnail scales up to full height; quieter sounds
/// stay small rather than show their noise floor.
const THUMBNAIL_FLOOR: f32 = 0.05;

impl MixJamApp {
    /// Whether the open Sounds panel shows: chosen, not maximised, and the
    /// lower panel not on its own Sounds module.
    pub(super) fn sounds_panel_open(&self) -> bool {
        let layout = self.session.layout();
        layout.sounds && !layout.maximised && self.bottom_module != BottomModule::Sounds
    }

    /// Open the Sounds panel. The lower panel leaves its Sounds module for
    /// Mixer, so the two never show at once.
    pub(super) fn open_sounds_panel(&mut self) {
        if self.bottom_module == BottomModule::Sounds {
            self.bottom_module = BottomModule::Mixer;
        }
        self.set_layout(|layout| {
            layout.sounds = true;
            layout.maximised = false;
        });
    }

    /// The Sounds panel at the left, or its folded strip. A maximised score
    /// hides both.
    pub(super) fn sounds_side(&mut self, ctx: &egui::Context) {
        if self.session.layout().maximised {
            return;
        }
        if !self.sounds_panel_open() {
            egui::SidePanel::left("sounds-folded")
                .exact_width(SOUNDS_STRIP_WIDTH)
                .resizable(false)
                .frame(egui::Frame::new().fill(roles().panel))
                .show(ctx, |ui| {
                    if sounds_strip(ui).clicked() {
                        self.open_sounds_panel();
                    }
                });
            return;
        }
        egui::SidePanel::left("sounds-panel")
            .exact_width(SOUNDS_PANEL_WIDTH)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(roles().panel)
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| self.sounds_panel(ui));
    }

    fn sounds_panel(&mut self, ui: &mut egui::Ui) {
        let (samples, groups) = self.palette_view();
        let total = self.session.library().samples.len();
        let tall = ui.available_height() >= WRAPPED_CHIPS_HEIGHT;
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        ui.horizontal(|ui| {
            ui.set_height(26.0);
            // Painted, not a label: the status bar's switch is the one
            // control named Sounds.
            let title = ui.painter().layout_no_wrap(
                "Sounds".to_owned(),
                FontId::proportional(16.0),
                roles().text,
            );
            let (rect, _) = ui.allocate_exact_size(title.size(), Sense::hover());
            ui.painter().galley(rect.min, title, roles().text);
            ui.label(
                egui::RichText::new(format!("{} of {total}", samples.len()))
                    .size(12.0)
                    .color(roles().muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if fold_button(ui).clicked() {
                    self.set_layout(|layout| layout.sounds = false);
                }
            });
        });
        let width = ui.available_width();
        self.sound_search(ui, width);
        ui.horizontal(|ui| {
            let half = (width - ui.spacing().item_spacing.x) / 2.0;
            self.library_filter(ui, half - 18.0, true);
            self.category_filter(ui, half - 18.0, &groups, true);
        });
        let chips = |app: &mut Self, ui: &mut egui::Ui| {
            let mut chosen = None;
            if category_chip(ui, "All", None, app.group.is_none()).clicked() {
                chosen = Some(None);
            }
            for name in GROUPS {
                let active = app.group.as_deref() == Some(name);
                if category_chip(ui, name, Some(group_color(name)), active).clicked() {
                    chosen = Some(Some(name.to_owned()));
                }
            }
            if let Some(group) = chosen {
                app.group = group;
            }
        };
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            if tall {
                ui.horizontal_wrapped(|ui| chips(self, ui));
            } else {
                egui::ScrollArea::horizontal()
                    .id_salt("sounds-panel-categories")
                    .show(ui, |ui| ui.horizontal(|ui| chips(self, ui)));
            }
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Sort").size(12.0).color(roles().muted));
            self.sort_order(ui, 120.0);
            self.sort_direction(ui);
        });
        ui.add_space(2.0);
        if samples.is_empty() {
            self.no_sounds(ui, 15.0);
            return;
        }
        let row_width = ui.available_width();
        ui.spacing_mut().item_spacing = Vec2::new(0.0, ROW_GAP);
        egui::ScrollArea::vertical()
            .id_salt("sounds-panel-list")
            .auto_shrink(false)
            .show_rows(ui, ROW_HEIGHT, samples.len(), |ui, rows| {
                for row in rows {
                    let sample_index = samples[row];
                    let (id, response) = {
                        let sample = &self.session.library().samples[sample_index];
                        let product = self.sample_product(sample.id);
                        let peaks = self.session.sample_peaks(sample.id);
                        let response = ui
                            .push_id(("sound-row", sample.id), |ui| {
                                sound_row(
                                    ui,
                                    sample,
                                    product,
                                    &peaks,
                                    self.selected_sample == Some(sample.id),
                                    row_width,
                                )
                            })
                            .inner;
                        if self.capture_view_is("sample-details")
                            && self.selected_sample == Some(sample.id)
                        {
                            response.show_tooltip_text(sample_details(sample, product));
                        }
                        (sample.id, response)
                    };
                    self.sound_actions(ui, id, &response);
                }
            });
    }

    /// What a sound tile or row does: a click selects it for placement,
    /// Enter places it at the playhead on the armed track, a double-click
    /// previews it, and a drag carries it to a track.
    pub(super) fn sound_actions(&mut self, ui: &egui::Ui, id: u32, response: &Response) {
        let place = response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if response.clicked() {
            self.selected_sample = Some(id);
            self.place_on_click = true;
        }
        if place {
            self.place(id, self.armed_track, self.session.cursor_tick());
        }
        if response.double_clicked() {
            self.preview(id);
        }
        if response.drag_started() {
            self.selected_sample = Some(id);
            self.place_on_click = true;
            self.drag = Some(DragItem::Sample(id));
        }
    }

    /// The empty list: no library at all, or no sound that the filters let through.
    pub(super) fn no_sounds(&mut self, ui: &mut egui::Ui, size: f32) {
        let empty = self.session.library().samples.is_empty();
        ui.label(heading(
            if empty {
                "No sounds"
            } else {
                "No matching sounds"
            },
            size,
        ));
        if empty {
            if action_button(ui, "Open sample libraries", true).clicked() {
                self.open_sample_libraries(ui.ctx());
            }
        } else if action_button(ui, "Clear filters", false).clicked() {
            self.search.clear();
            self.group = None;
            self.palette_product = None;
        }
    }

    pub(super) fn sound_search(&mut self, ui: &mut egui::Ui, width: f32) {
        name_control(
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    // The workspace overrides text color; a hint needs its own.
                    .hint_text(egui::RichText::new("Find a sound").color(roles().muted))
                    .desired_width(width),
            ),
            "Find a sound",
        );
    }

    /// The filters' boxes truncate a long choice in the narrow panel; the
    /// lower panel's toolbar lets it widen the box as before.
    pub(super) fn library_filter(&mut self, ui: &mut egui::Ui, width: f32, truncate: bool) {
        let mut libraries = egui::ComboBox::from_id_salt("palette_product")
            .selected_text(self.palette_product.map_or("All libraries", Product::label))
            .width(width);
        if truncate {
            libraries = libraries.truncate();
        }
        let libraries = libraries
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.palette_product, None, "All libraries");
                for product in Product::ALL {
                    ui.selectable_value(&mut self.palette_product, Some(product), product.label());
                }
            })
            .response;
        name_control(libraries, "Library filter");
    }

    pub(super) fn category_filter(
        &mut self,
        ui: &mut egui::Ui,
        width: f32,
        groups: &[String],
        truncate: bool,
    ) {
        let group_label = self.group.as_deref().map_or("All sounds", |g| {
            if g.is_empty() { "Uncategorized" } else { g }
        });
        let mut categories = egui::ComboBox::from_id_salt("palette_category")
            .selected_text(group_label)
            .width(width);
        if truncate {
            categories = categories.truncate();
        }
        let categories = categories
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.group, None, "All sounds");
                for name in groups {
                    ui.selectable_value(
                        &mut self.group,
                        Some(name.clone()),
                        if name.is_empty() {
                            "Uncategorized"
                        } else {
                            name
                        },
                    );
                }
            })
            .response;
        name_control(categories, "Category filter");
    }

    pub(super) fn sort_order(&mut self, ui: &mut egui::Ui, width: f32) {
        let sorting = egui::ComboBox::from_id_salt("palette_sort")
            .selected_text(self.palette_sort.label())
            .width(width)
            .show_ui(ui, |ui| {
                for sort in PaletteSort::ALL {
                    ui.selectable_value(&mut self.palette_sort, sort, sort.label());
                }
            })
            .response;
        name_control(sorting, "Sort order");
    }

    pub(super) fn sort_direction(&mut self, ui: &mut egui::Ui) {
        let [forward, reversed] = self.palette_sort.directions();
        let (now, next) = if self.palette_descending {
            (reversed, forward)
        } else {
            (forward, reversed)
        };
        let direction = ui
            .add(
                egui::Button::new(if self.palette_descending {
                    "\u{2193}"
                } else {
                    "\u{2191}"
                })
                .min_size(Vec2::new(24.0, 0.0)),
            )
            .hover_hint(format!("{now}. Click for {}.", next.to_lowercase()));
        ui.ctx()
            .accesskit_node_builder(direction.id, |node| node.set_value(now));
        if name_control(direction, "Sort direction").clicked() {
            self.palette_descending = !self.palette_descending;
        }
    }
}

/// A category chip: a small square in the category's colour before its
/// name. It carries the same name and hint as the lower panel's category
/// buttons.
fn category_chip(ui: &mut egui::Ui, label: &str, color: Option<Color32>, active: bool) -> Response {
    let action = format!("Show {label} sounds");
    let text_color = if active { roles().text } else { roles().muted };
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), FontId::proportional(12.0), text_color);
    let swatch = if color.is_some() { 10.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + swatch + 14.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let radius = roles().corner(4.0);
    let fill = if active || response.hovered() {
        roles().hover
    } else {
        roles().panel
    };
    painter.rect_filled(rect, radius, fill);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, if active { roles().muted } else { roles().hover }),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.left() + 7.0;
    if let Some(color) = color {
        painter.rect_filled(
            Rect::from_center_size(Pos2::new(x + 3.0, rect.center().y), Vec2::splat(6.0)),
            1.0,
            color,
        );
        x += swatch;
    }
    painter.galley(
        Pos2::new(x, rect.center().y - galley.size().y / 2.0),
        galley,
        text_color,
    );
    focus_outline(ui, &response, roles().accent);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, &action)
    });
    name_control(response, &action).hover_hint(action)
}

/// One sound in the list: its name, its category and length, and its
/// waveform, on the category's tint. It names itself after the sound, as a
/// tile does.
fn sound_row(
    ui: &mut egui::Ui,
    sample: &Sample,
    product: Option<Product>,
    peaks: &PeakState,
    selected: bool,
    width: f32,
) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::click_and_drag());
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    let color = group_color(&sample.group_name);
    let painter = ui.painter_at(rect);
    let radius = roles().corner(5.0);
    theme::surface(
        &painter,
        rect,
        radius,
        if response.hovered() || selected {
            roles().tile_hover
        } else {
            roles().tile
        },
    );
    painter.rect_filled(rect, radius, tile_tint(color));
    if selected || response.has_focus() {
        painter.rect_stroke(
            rect.shrink(0.5),
            radius,
            Stroke::new(1.0_f32, color),
            egui::StrokeKind::Inside,
        );
    }
    let thumbnail = Rect::from_min_size(
        Pos2::new(
            rect.right() - 8.0 - THUMBNAIL.x,
            rect.center().y - THUMBNAIL.y / 2.0,
        ),
        THUMBNAIL,
    );
    paint_thumbnail(&painter, thumbnail, peaks, color);
    let text_width = thumbnail.left() - rect.left() - 16.0;
    let name = painter.layout_job(elided(&sample.name, 12.5, roles().text, text_width, 1));
    let text = painter.with_clip_rect(Rect::from_min_max(
        rect.min + Vec2::new(8.0, 3.0),
        Pos2::new(thumbnail.left() - 6.0, rect.bottom() - 3.0),
    ));
    text.galley(rect.min + Vec2::new(8.0, 5.0), name, roles().text);
    let bars = format!(
        "{} bar{}",
        sample.bars,
        if sample.bars == 1 { "" } else { "s" }
    );
    let muted = roles().text.gamma_multiply(TILE_DETAIL_ALPHA);
    let font = FontId::proportional(10.5);
    let mut metadata = painter.layout_no_wrap(
        format!("{} · {bars}", sample.group_name),
        font.clone(),
        muted,
    );
    if metadata.size().x > text_width {
        metadata = painter.layout_no_wrap(bars, font, muted);
    }
    text.galley(rect.min + Vec2::new(8.0, 25.0), metadata, muted);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &sample.name)
    });
    // Build the details only for the row under the pointer.
    response.hover_details(|ui| {
        ui.label(sample_details(sample, product));
    })
}

/// A sound's waveform in `rect`: one bar per point from its lowest to its
/// highest sample, scaled to the sound's own peak. A sound whose peaks are
/// still on their way, or cannot be read, shows only the centre line.
fn paint_thumbnail(painter: &egui::Painter, rect: Rect, peaks: &PeakState, color: Color32) {
    let line = color.gamma_multiply(0.45);
    painter.line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0_f32, line),
    );
    let PeakState::Ready(peaks) = peaks else {
        return;
    };
    let count = rect.width().round().max(1.0) as usize;
    let columns = peaks.columns(count);
    let loudest = columns
        .iter()
        .map(|(low, high)| low.abs().max(high.abs()))
        .fold(0.0_f32, f32::max);
    let scale = rect.height() / 2.0 / loudest.max(THUMBNAIL_FLOOR);
    let mut mesh = egui::Mesh::default();
    for (index, (low, high)) in columns.into_iter().enumerate() {
        let x = rect.left() + index as f32;
        let top = rect.center().y - high * scale;
        let bottom = (rect.center().y - low * scale).max(top + 1.0);
        mesh.add_colored_rect(
            Rect::from_min_max(Pos2::new(x, top), Pos2::new(x + 1.0, bottom)),
            color,
        );
    }
    painter.add(mesh);
}

/// The fold button at the Sounds panel's top right: a chevron toward the edge.
fn fold_button(ui: &mut egui::Ui) -> Response {
    let label = "Fold the Sounds panel";
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    focus_outline(ui, &response, roles().accent);
    chevron(
        ui.painter(),
        rect.center(),
        -1.0,
        if response.hovered() {
            roles().text
        } else {
            roles().muted
        },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response.hover_hint(label)
}

/// The folded panel: a strip with a chevron and the panel's name, read top
/// to bottom. The whole strip opens the panel.
fn sounds_strip(ui: &mut egui::Ui) -> Response {
    let label = "Show the Sounds panel";
    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, Sense::click());
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    focus_outline(ui, &response, roles().accent);
    let color = if response.hovered() {
        roles().text
    } else {
        roles().muted
    };
    chevron(
        ui.painter(),
        Pos2::new(rect.center().x, rect.top() + 18.0),
        1.0,
        color,
    );
    let galley =
        ui.painter()
            .layout_no_wrap("SOUNDS".to_owned(), FontId::proportional(11.0), color);
    // A quarter turn clockwise turns the galley's top edge to the right.
    let at = Pos2::new(rect.center().x + galley.size().y / 2.0, rect.top() + 36.0);
    ui.painter().add(
        egui::epaint::TextShape::new(at, galley, color).with_angle(std::f32::consts::FRAC_PI_2),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response.hover_hint(
        "Show the Sounds panel. When the lower panel shows its Sounds module, it changes to Mixer.",
    )
}

/// A chevron centred on `at`, pointing right for a positive `direction` and
/// left for a negative one.
fn chevron(painter: &egui::Painter, at: Pos2, direction: f32, color: Color32) {
    let stroke = Stroke::new(1.5_f32, color);
    let tip = at + Vec2::new(direction * 2.5, 0.0);
    let back = tip - Vec2::new(direction * 5.0, 0.0);
    painter.line_segment([back + Vec2::new(0.0, -5.0), tip], stroke);
    painter.line_segment([back + Vec2::new(0.0, 5.0), tip], stroke);
}
