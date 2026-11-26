//! The Help windows: the application's manual, which opens on a shortcut
//! sheet and lists its pages at the left, and each tool's page of it in the
//! tool's own window and palette, placed beside the tool.
use super::*;
#[path = "help/content.rs"]
mod content;
#[path = "help/page.rs"]
mod page;
#[path = "help/topics.rs"]
mod topics;
use page::page_scroll;
pub(super) use topics::Topic;

/// The width of the page list of the main window.
const LIST_WIDTH: f32 = 156.0;
/// Below this width the page list wraps above the page.
const NARROW: f32 = 560.0;
/// The widest line of a page: about 75 characters of body text.
const MEASURE: f32 = 540.0;
/// The action column of key and term rows.
const ACTION_WIDTH: f32 = 128.0;

/// Remember the lower panel, so that a board's Help opens beside it.
pub(super) fn note_board_rect(ctx: &egui::Context, rect: Rect) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("help-board-rect"), rect));
}

/// What a page's window opens beside: its tool window, its deep-edit window
/// while `deep_edit_open`, or the lower panel. A closed deep-edit window keeps
/// its last place, so that place counts only while the window is open.
pub(super) fn anchor(ctx: &egui::Context, topic: Topic, deep_edit_open: bool) -> Option<Rect> {
    if let Some(tool) = topic.tool_window() {
        return dialog::window_rect(ctx, tool);
    }
    let board = ctx.data(|data| data.get_temp::<Rect>(egui::Id::new("help-board-rect")));
    let deep_edit = match topic {
        Topic::Mastering => Some(master::DEEP_EDIT_TITLE),
        Topic::FxReturns => Some(fx::DEEP_EDIT_TITLE),
        _ => None,
    };
    deep_edit
        .filter(|_| deep_edit_open)
        .and_then(|title| dialog::window_rect(ctx, title))
        .or(board)
}

/// The main Help window's body: the page list and the current page.
pub(super) fn manual(ui: &mut egui::Ui, current: &mut Topic) {
    let identity = "Help";
    let mut list_focus = false;
    if ui.available_width() < NARROW {
        // Keep a page viewport even at the shell's minimum size.
        let list_height = (ui.available_height() * 0.4).max(26.0);
        egui::ScrollArea::vertical()
            .id_salt("help-topics")
            .min_scrolled_height(0.0)
            .max_height(list_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    for topic in Topic::ALL {
                        list_focus |= list_item(ui, topic, current, true);
                    }
                });
            });
        view::name_scroll_area(ui, "help-topics", "Help pages");
        ui.add_space(4.0);
        ui.separator();
        page_scroll(ui, identity, *current, true, list_focus);
        return;
    }
    let height = ui.available_height();
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(LIST_WIDTH, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(LIST_WIDTH);
                egui::ScrollArea::vertical()
                    .id_salt("help-topics")
                    .auto_shrink([false, false])
                    .max_height(height)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for topic in Topic::ALL {
                            if topic == Topic::Stretch {
                                ui.add_space(6.0);
                                ui.separator();
                                ui.add_space(6.0);
                            }
                            list_focus |= list_item(ui, topic, current, false);
                        }
                    });
                view::name_scroll_area(ui, "help-topics", "Help pages");
            },
        );
        let x = ui.cursor().left() + 2.0;
        ui.painter().line_segment(
            [
                Pos2::new(x, ui.min_rect().top()),
                Pos2::new(x, ui.min_rect().top() + height),
            ],
            Stroke::new(1.0_f32, Color32::from_black_alpha(110)),
        );
        ui.add_space(14.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| page_scroll(ui, identity, *current, true, list_focus),
        );
    });
}

/// A tool's Help window body: its page, without the page list.
pub(super) fn tool_page(ui: &mut egui::Ui, topic: Topic) {
    page_scroll(ui, topic.window().unwrap_or("Help"), topic, false, false);
}

/// One entry of the page list. Arrows move between entries; Tab leaves.
/// Returns whether the entry has the keyboard focus.
fn list_item(ui: &mut egui::Ui, topic: Topic, current: &mut Topic, compact: bool) -> bool {
    let name = topic.name();
    let galley =
        ui.painter()
            .layout_no_wrap(name.to_owned(), FontId::proportional(13.0), roles().legend);
    let mark = if topic.palette().is_some() { 14.0 } else { 0.0 };
    let width = if compact {
        galley.size().x + mark + 20.0
    } else {
        ui.available_width()
    };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
    let response = ui.interact(
        rect,
        egui::Id::new(("help-page-entry", name)),
        Sense::click(),
    );
    let focus_id = egui::Id::new("help-list-focus");
    if ui.data(|data| data.get_temp::<Topic>(focus_id)) == Some(topic) {
        response.request_focus();
        response.scroll_to_me(None);
        ui.data_mut(|data| data.remove::<Topic>(focus_id));
    }
    // A click gives the entry the focus, so the arrows go on from it, but
    // only the keyboard shows the focus ring.
    let pointer_id = egui::Id::new("help-list-pointer");
    if response.clicked() {
        *current = topic;
        response.request_focus();
        ui.data_mut(|data| data.insert_temp(pointer_id, true));
    }
    if response.has_focus() {
        // The list keeps Up and Down; egui would move the focus with them.
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    vertical_arrows: true,
                    ..Default::default()
                },
            )
        });
        let index = Topic::ALL.iter().position(|t| *t == topic).unwrap_or(0);
        let step = ui.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                Some(index + 1)
            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                index.checked_sub(1)
            } else {
                None
            }
        });
        if let Some(next) = step.and_then(|index| Topic::ALL.get(index).copied()) {
            *current = next;
            ui.data_mut(|data| {
                data.insert_temp(focus_id, next);
                data.insert_temp(pointer_id, false);
            });
        }
    }
    if response.gained_focus() && ui.input(|input| input.key_pressed(egui::Key::Tab)) {
        response.scroll_to_me(None);
        ui.data_mut(|data| data.insert_temp(pointer_id, false));
    }
    let ring =
        response.has_focus() && !ui.data(|data| data.get_temp::<bool>(pointer_id).unwrap_or(false));
    let selected = *current == topic;
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            format!("Help page: {name}"),
        )
    });
    let corner = roles().corner(3.0);
    let painter = ui.painter();
    if selected {
        painter.rect(
            rect,
            corner,
            roles().face,
            Stroke::new(1.0_f32, roles().widget_edge),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() {
        painter.rect_filled(rect, corner, Color32::from_white_alpha(8));
    }
    if ring {
        painter.rect_stroke(
            rect.shrink(1.0),
            corner,
            Stroke::new(1.5_f32, ui.visuals().selection.bg_fill),
            egui::StrokeKind::Inside,
        );
    }
    let mut x = rect.left() + 10.0;
    if let Some(palette) = topic.palette() {
        painter.rect_filled(
            Rect::from_center_size(Pos2::new(x + 3.5, rect.center().y), Vec2::splat(7.0)),
            1.5,
            palette.accent(),
        );
        x += mark;
    }
    let y = rect.center().y - galley.size().y / 2.0;
    painter.galley(Pos2::new(x, y), galley, roles().legend);
    response.has_focus()
}
