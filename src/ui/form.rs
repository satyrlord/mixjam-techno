//! Form pieces that tool windows share: section titles, help lines, choice
//! boxes, switches, and notices, and the Tab ring that keeps keyboard focus
//! inside one window.
//!
//! A window opens its ring with `ring::begin`, draws its controls (each piece
//! here registers itself), and closes it with `ring::end`. The input pass
//! then cycles the ring of the window on top.
use super::*;

/// A page section title in uppercase, with more room above it than below.
/// The first one clears the bay's top edge.
pub(super) fn section(ui: &mut egui::Ui, title: &str) {
    let first = ui.cursor().top() <= ui.min_rect().top() + 0.5;
    ui.add_space(if first { 4.0 } else { 8.0 });
    ui.label(heading(title, theme::type_size::SECTION).color(roles().legend));
}

/// One short line of explanation that belongs to the control above it.
pub(super) fn help(ui: &mut egui::Ui, text: impl Into<String>) {
    // Pull the line up to its control; the next group keeps its distance.
    ui.add_space(-4.0);
    ui.label(
        egui::RichText::new(text)
            .size(theme::type_size::CLIP)
            .color(roles().help),
    );
    ui.add_space(4.0);
}

/// A state that needs attention but is not an error, such as an unapplied
/// or reduced setting.
pub(super) fn warning(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.colored_label(roles().warning, text.into());
}

/// A failure, or a value that blocks an action.
pub(super) fn error(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.colored_label(roles().coral, text.into());
}

/// A labelled choice box named `name`. Screen readers hear the choice as its
/// value.
pub(super) fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    name: &str,
    value: &mut T,
    choices: &[(T, &str)],
) {
    ui.push_id(name, |ui| {
        ui.label(name);
        let selected = choices
            .iter()
            .find(|(item, _)| item == value)
            .map_or("", |(_, label)| *label);
        let response = egui::ComboBox::from_id_salt("choice")
            .width(CHOICE_WIDTH)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                for (item, label) in choices {
                    ui.selectable_value(value, *item, *label);
                }
            });
        choice_named(&response.response, name, selected);
    });
}

/// The width of every choice box in a form.
pub(super) const CHOICE_WIDTH: f32 = 210.0;

/// Name a hand-built choice box, give it its value, and register it.
pub(super) fn choice_named(response: &Response, name: &str, selected: &str) {
    named(response, egui::WidgetType::ComboBox, name);
    register(response);
    response
        .ctx
        .accesskit_node_builder(response.id, |node| node.set_value(selected));
}

/// The shared Folded Metal switch, laid out in the form's flow.
pub(super) fn switch(ui: &mut egui::Ui, name: &str, value: &mut bool) -> Response {
    let label = ui.painter().layout_no_wrap(
        name.into(),
        FontId::proportional(theme::type_size::BODY),
        roles().legend,
    );
    let (rect, _) = ui.allocate_exact_size(Vec2::new(label.size().x + 36.0, 24.0), Sense::hover());
    let response = controls::toggle_at(ui, rect, name, name, value);
    register(&response);
    response
}

/// A form button, registered in the ring.
pub(super) fn button(ui: &mut egui::Ui, label: &str, role: controls::ButtonRole) -> Response {
    let response = controls::button(ui, label, role);
    register(&response);
    response
}

/// A disclosure whose header takes focus before the controls in its body.
pub(super) fn disclosure<R>(
    ui: &mut egui::Ui,
    header: egui::CollapsingHeader,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::CollapsingResponse<R> {
    let position = ui.ctx().data(|data| {
        let ring = data.get_temp::<egui::Id>(egui::Id::new(CURRENT))?;
        let order = data.get_temp::<Vec<egui::Id>>(ring)?;
        Some((ring, order.len()))
    });
    let response = header.show(ui, contents);
    register(&response.header_response);
    if response.header_response.enabled()
        && let Some((ring, index)) = position
    {
        // egui returns the header response after drawing the body. Put it
        // before the controls that the body added, including nested groups.
        ui.ctx().data_mut(|data| {
            data.get_temp_mut_or_default::<Vec<egui::Id>>(ring)[index..].rotate_right(1);
        });
    }
    response
}

/// Add an enabled control to the ring of the window being drawn, and keep a
/// newly focused control in view.
pub(super) fn register(response: &Response) {
    if !response.enabled() {
        return;
    }
    response.ctx.data_mut(|data| {
        if let Some(ring) = data.get_temp::<egui::Id>(egui::Id::new(CURRENT)) {
            data.get_temp_mut_or_default::<Vec<egui::Id>>(ring)
                .push(response.id);
        }
    });
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
}

const CURRENT: &str = "form-ring-current";

/// The Tab ring of a window: its enabled controls in drawing order. Tab and
/// Shift+Tab walk it and wrap at both ends.
pub(super) mod ring {
    use super::*;

    fn id(identity: &str) -> egui::Id {
        egui::Id::new(("form-ring", identity))
    }

    /// Start the ring of `identity` for this frame.
    pub(in super::super) fn begin(ctx: &egui::Context, identity: &str) {
        let ring = id(identity);
        ctx.data_mut(|data| {
            data.insert_temp(ring, Vec::<egui::Id>::new());
            data.insert_temp(egui::Id::new(CURRENT), ring);
        });
    }

    /// Stop adding controls to the ring that `begin` started.
    pub(in super::super) fn end(ctx: &egui::Context) {
        ctx.data_mut(|data| data.remove::<egui::Id>(egui::Id::new(CURRENT)));
    }

    /// Move the focus to the next control of the ring of `identity`, or to
    /// the previous one.
    pub(in super::super) fn cycle(ctx: &egui::Context, identity: &str, reverse: bool) {
        let order = ctx
            .data(|data| data.get_temp::<Vec<egui::Id>>(id(identity)))
            .unwrap_or_default();
        if order.is_empty() {
            return;
        }
        let focused = ctx.memory(|memory| memory.focused());
        let index = order.iter().position(|id| Some(*id) == focused);
        let next = match (index, reverse) {
            (Some(0) | None, true) => order.len() - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % order.len(),
            (None, false) => 0,
        };
        ctx.memory_mut(|memory| {
            memory.move_focus(egui::FocusDirection::None);
            memory.request_focus(order[next]);
        });
    }
}
