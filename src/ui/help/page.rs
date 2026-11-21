//! The drawing of one page of the manual: its blocks, its keycaps, and its
//! keyboard scrolling.
use super::content::Block::*;
use super::*;
use egui::accesskit::Role;

/// A page in its own scroll area. Page Up, Page Down, Home, and End scroll
/// it while the pointer is over it or the page list has the focus.
pub(super) fn page_scroll(
    ui: &mut egui::Ui,
    identity: &str,
    topic: Topic,
    title: bool,
    list_focus: bool,
) {
    let salt = ("help-page", identity);
    let shown = egui::Id::new(salt).with("topic");
    let metrics = egui::Id::new(salt).with("metrics");
    let mut area = egui::ScrollArea::vertical()
        .id_salt(salt)
        .auto_shrink([false, false])
        .min_scrolled_height(0.0)
        .max_height(ui.available_height());
    if ui.data(|data| data.get_temp::<Topic>(shown)) != Some(topic) {
        ui.data_mut(|data| data.insert_temp(shown, topic));
        area = area.vertical_scroll_offset(0.0);
    } else if let Some(last) = ui.data(|data| data.get_temp::<PageMetrics>(metrics))
        && let Some(offset) = keyboard_offset(ui, identity, last, list_focus)
    {
        area = area.vertical_scroll_offset(offset);
    }
    let output = area.show(ui, |ui| page(ui, topic, title));
    ui.data_mut(|data| {
        data.insert_temp(
            metrics,
            PageMetrics {
                view: output.inner_rect,
                offset: output.state.offset.y,
                end: (output.content_size.y - output.inner_rect.height()).max(0.0),
            },
        )
    });
    view::name_scroll_area(ui, salt, &format!("{} page", topic.name()));
}

/// Where a page showed last frame and how far it scrolled.
#[derive(Clone, Copy)]
struct PageMetrics {
    view: Rect,
    offset: f32,
    end: f32,
}

/// The offset that Page Up, Page Down, Home, or End asks for. The keys
/// jump at once, as a reader's do.
fn keyboard_offset(
    ui: &egui::Ui,
    identity: &str,
    last: PageMetrics,
    list_focus: bool,
) -> Option<f32> {
    let ctx = ui.ctx();
    let focused = ctx.memory(|memory| memory.focused());
    let own_focus = focused
        .and_then(|id| ctx.read_response(id))
        .is_some_and(|response| response.layer_id == ui.layer_id());
    let hovered = ctx
        .pointer_hover_pos()
        .is_some_and(|at| last.view.contains(at) && ctx.layer_id_at(at) == Some(ui.layer_id()));
    if !(list_focus || own_focus || focused.is_none() && hovered)
        || !dialog::top_window(ctx, identity)
        || egui::Popup::is_any_open(ctx)
    {
        return None;
    }
    let page = (last.view.height() - 40.0).max(40.0);
    ctx.input_mut(|input| {
        [
            (egui::Key::PageDown, last.offset + page),
            (egui::Key::PageUp, last.offset - page),
            (egui::Key::End, last.end),
            (egui::Key::Home, 0.0),
        ]
        .into_iter()
        .find(|(key, _)| input.consume_key(egui::Modifiers::NONE, *key))
        .map(|(_, offset)| offset.clamp(0.0, last.end))
    })
}

fn page(ui: &mut egui::Ui, topic: Topic, title: bool) {
    let width = ui.available_width().min(MEASURE);
    ui.set_max_width(width);
    ui.spacing_mut().item_spacing = Vec2::new(8.0, 7.0);
    if title {
        heading(ui, topic.name(), 16.0, 1);
        ui.add_space(2.0);
    }
    for (index, block) in topic.blocks().iter().enumerate() {
        match block {
            Heading(text) => {
                if index > 0 || title {
                    ui.add_space(10.0);
                }
                heading(ui, text, 13.0, 2);
            }
            Facts(lines) => {
                for line in *lines {
                    ui.add(egui::Label::new(*line).wrap());
                }
            }
            Steps(steps) => {
                for (number, step) in steps.iter().enumerate() {
                    columns(
                        ui,
                        22.0,
                        |ui| {
                            ui.label(action_text(&format!("{}.", number + 1)));
                        },
                        |ui| {
                            ui.add(egui::Label::new(*step).wrap());
                        },
                    );
                }
            }
            Keys(rows) => {
                for (action, keys, note) in *rows {
                    columns(
                        ui,
                        ACTION_WIDTH,
                        |ui| {
                            // The same wrapped row as the keys, so that both
                            // columns centre on one line.
                            ui.horizontal_wrapped(|ui| {
                                ui.add(egui::Label::new(action_text(action)).wrap());
                            });
                        },
                        |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                for (index, combo) in keys.iter().enumerate() {
                                    if index > 0 {
                                        ui.label(egui::RichText::new("/").color(roles().help));
                                    }
                                    keycap(ui, combo);
                                }
                                if !note.is_empty() {
                                    ui.add(egui::Label::new(*note).wrap());
                                }
                            });
                        },
                    );
                }
            }
            Terms(rows) => {
                for (term, meaning) in *rows {
                    columns(
                        ui,
                        ACTION_WIDTH,
                        |ui| {
                            ui.add(egui::Label::new(action_text(term)).wrap());
                        },
                        |ui| {
                            ui.add(egui::Label::new(*meaning).wrap());
                        },
                    );
                }
            }
        }
    }
    ui.add_space(6.0);
}

/// A heading that assistive technology announces as one, at `level`.
fn heading(ui: &mut egui::Ui, text: &str, size: f32, level: usize) {
    let response = ui.add(egui::Label::new(view::heading(text, size).color(roles().legend)).wrap());
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(Role::Heading);
        node.set_level(level);
    });
}

/// The action or term of a row, in the heading face, so the eye finds it.
fn action_text(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .font(FontId::new(13.0, theme::heading_family()))
        .color(roles().legend)
}

fn columns(
    ui: &mut egui::Ui,
    left: f32,
    first: impl FnOnce(&mut egui::Ui),
    second: impl FnOnce(&mut egui::Ui),
) {
    let left = left.min(ui.available_width() * 0.4);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(left, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(left);
                first(ui);
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(ui.available_width());
                second(ui);
            },
        );
    });
}

/// A key or key combination, drawn as a key cap.
fn keycap(ui: &mut egui::Ui, keys: &str) {
    let galley =
        ui.painter()
            .layout_no_wrap(keys.to_owned(), FontId::proportional(12.0), roles().legend);
    let size = Vec2::new(galley.size().x + 12.0, galley.size().y.max(14.0) + 5.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, keys));
    let corner = roles().corner(3.0);
    let painter = ui.painter();
    painter.rect(
        rect,
        corner,
        Color32::from_white_alpha(8),
        Stroke::new(1.0_f32, roles().widget_edge),
        egui::StrokeKind::Inside,
    );
    // The cap's lower edge, as on a keyboard.
    painter.line_segment(
        [
            Pos2::new(rect.left() + corner + 1.0, rect.bottom() - 1.0),
            Pos2::new(rect.right() - corner - 1.0, rect.bottom() - 1.0),
        ],
        Stroke::new(1.0_f32, Color32::from_black_alpha(110)),
    );
    painter.galley(rect.center() - galley.size() / 2.0, galley, roles().legend);
}
