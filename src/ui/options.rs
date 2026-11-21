//! The Options window. The draft belongs to this window: Session validates,
//! prepares, and persists it, and closing the window cannot change an active
//! preference.
//!
//! - `draft`: `OptionsDraft`, a plain controller with no egui and no
//!   Session. `OptionsFacts` in, `Action`s in, `Effect`s out.
//! - `pages`: the Audio, Workspace, and Files pages as free functions over
//!   the draft's values and the facts.
//! - `labels`: every word the window shows or names, shared with the tests.
//!
//! This file holds the window shell: the page selector, the fixed footer,
//! the discard prompt, and the effects that reach the Session.
#[path = "options/draft.rs"]
pub(super) mod draft;
#[path = "options/labels.rs"]
pub(super) mod labels;
#[path = "options/pages.rs"]
pub(super) mod pages;

use super::*;
pub(super) use draft::{Action, Effect, OptionsDraft, OptionsFacts, Page};
pub(super) use labels::DISCARD;
use labels::*;

/// A page selector button.
const PAGE_BUTTON: Vec2 = Vec2::new(132.0, 34.0);
/// The selector's column, with room for the button's focus ring.
const SELECTOR_WIDTH: f32 = 136.0;
/// The footer's height and the gap above it.
const FOOTER: f32 = 38.0;
const FOOTER_GAP: f32 = 12.0;

fn pending_focus_id() -> egui::Id {
    egui::Id::new("options-pending-focus")
}

fn take_pending_focus(ctx: &egui::Context) -> Option<egui::Id> {
    ctx.data_mut(|data| {
        data.remove_temp::<Option<egui::Id>>(pending_focus_id())
            .flatten()
    })
}

/// The open window: its draft, and where the keyboard goes.
pub(super) struct OptionsWindow {
    pub(super) draft: OptionsDraft,
    /// The focus before the window opened, given back when it closes.
    previous_focus: Option<egui::Id>,
    /// Focus the selected page's button on the next frame.
    focus_page: bool,
    /// The tallest page body seen since the window opened. The window keeps
    /// this height, so its footer stays in place as the pages change.
    tallest: Option<f32>,
}

impl std::ops::Deref for OptionsWindow {
    type Target = OptionsDraft;
    fn deref(&self) -> &OptionsDraft {
        &self.draft
    }
}

impl std::ops::DerefMut for OptionsWindow {
    fn deref_mut(&mut self) -> &mut OptionsDraft {
        &mut self.draft
    }
}

impl OptionsWindow {
    pub(super) fn new(session: &Session, page: Page) -> Self {
        Self {
            draft: OptionsDraft::new(&facts(session), page),
            previous_focus: None,
            focus_page: true,
            tallest: None,
        }
    }
}

/// What the Session says this frame, read once.
pub(super) fn facts(session: &Session) -> OptionsFacts {
    OptionsFacts {
        active: session.options().clone(),
        revision: session.options_revision(),
        invalid: session.options_invalid(),
        error: session.options_error().map(str::to_owned),
        audio_blocked: session.audio_options_blocked_reason().map(str::to_owned),
        buffer_errors: BUFFER_TARGETS
            .iter()
            .filter_map(|(target, _)| {
                session
                    .buffer_target_error(*target)
                    .map(|error| (*target, error))
            })
            .collect(),
        buffer_status: session.buffer_status(),
        folder_warning: session.song_folder_warning(),
    }
}

impl MixJamApp {
    pub(super) fn open_options(&mut self, ctx: &egui::Context) {
        if let Some(window) = &mut self.options {
            window.focus_page = true;
        } else {
            let mut window = OptionsWindow::new(&self.session, Page::Audio);
            let pending = take_pending_focus(ctx);
            window.previous_focus = ctx.memory(|memory| memory.focused()).or(pending);
            self.options = Some(window);
        }
        dialog::expand(ctx, WINDOW);
        ctx.move_to_top(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new(WINDOW),
        ));
    }

    pub(super) fn close_options(&mut self, ctx: &egui::Context) {
        if let Some(window) = self.options.take() {
            if let Some(id) = window.previous_focus {
                if ctx.memory(|memory| memory.top_modal_layer().is_none()) {
                    ctx.memory_mut(|memory| memory.request_focus(id));
                    return;
                }
                // On the first pass after Discard, egui still blocks the
                // workspace because the modal was drawn in the previous pass.
                ctx.data_mut(|data| data.insert_temp(pending_focus_id(), Some(id)));
                ctx.request_repaint();
            }
            if let Some(id) = ctx.memory(|memory| memory.focused()) {
                ctx.memory_mut(|memory| memory.surrender_focus(id));
            }
        }
    }

    fn restore_options_focus(&self, ctx: &egui::Context) {
        let pending = pending_focus_id();
        if !ctx.data(|data| data.get_temp::<Option<egui::Id>>(pending).is_some())
            || self.blocking_modal(ctx).is_some()
        {
            return;
        }
        if ctx.memory(|memory| memory.top_modal_layer().is_some()) {
            ctx.request_repaint();
        } else if let Some(id) = take_pending_focus(ctx) {
            ctx.memory_mut(|memory| {
                if memory.focused().is_none() {
                    memory.request_focus(id);
                }
            });
            ctx.request_repaint();
        }
    }

    pub(super) fn options_window(&mut self, ctx: &egui::Context) {
        let Some(mut window) = self.options.take() else {
            self.restore_options_focus(ctx);
            return;
        };
        let facts = facts(&self.session);
        let mut actions = Vec::new();
        let mut open = true;
        form::ring::begin(ctx, WINDOW);
        let fitted = !dialog::user_resized(ctx, WINDOW);
        dialog::window(WINDOW)
            .open(&mut open)
            .default_width(760.0)
            .default_height(500.0)
            .scroll(false)
            .fit(true)
            .focus_controls(form::register)
            .show(ctx, |ui| {
                draw(ui, &mut window, &facts, fitted, &mut actions)
            });
        form::ring::end(ctx);
        if !open {
            actions.push(Action::Close);
        }
        let close_requested = actions.iter().any(|action| matches!(action, Action::Close));
        let mut effects: Vec<Effect> = actions
            .into_iter()
            .filter_map(|action| window.update(action, &facts))
            .collect();
        window.settle(&facts);
        if close_requested && window.confirm_discard {
            dialog::expand(ctx, DISCARD);
        }
        if let Some(wait) = draft::folder_check_retry_in() {
            ctx.request_repaint_after(wait);
        }
        if window.confirm_discard {
            let mut answer = None;
            let response = dialog::modal(ctx, DISCARD, DISCARD, |ui| {
                answer = discard_prompt(ui, &window, &facts);
            });
            if response.should_close() {
                answer = Some(Action::KeepEditing);
            }
            effects.extend(answer.and_then(|action| window.update(action, &facts)));
        }
        self.options = Some(window);
        for effect in effects {
            self.options_effect(ctx, effect);
        }
    }

    fn options_effect(&mut self, ctx: &egui::Context, effect: Effect) {
        match effect {
            Effect::Apply {
                values,
                revision,
                restored,
                close,
            } => match self.session.apply_options(values, revision, restored) {
                Ok(_) => {
                    let facts = facts(&self.session);
                    if let Some(window) = &mut self.options {
                        window.applied(&facts);
                    }
                    self.sections.bookmarks = self.session.options().show_section_bookmarks;
                    self.message(APPLIED);
                    if close {
                        self.close_options(ctx);
                    }
                }
                Err(error) => {
                    self.error(error.clone());
                    if let Some(window) = &mut self.options {
                        window.failed(error);
                    }
                }
            },
            Effect::Close => self.close_options(ctx),
            Effect::PickFolder => self.ask_path(FileRequest::OptionsFolder),
            Effect::SaveCapture => {
                // The session puts the result in the status line, error or not.
                let _ = self.session.save_playback_capture("Saved from Options");
            }
            Effect::OpenLibraries => self.open_sample_libraries(ctx),
        }
    }
}

/// The window's body: the selector and the page above a fixed footer.
fn draw(
    ui: &mut egui::Ui,
    window: &mut OptionsWindow,
    facts: &OptionsFacts,
    fitted: bool,
    actions: &mut Vec<Action>,
) {
    let mut area = ui.available_rect_before_wrap();
    // Until the user resizes it, the window is as tall as the tallest page
    // seen, up to the screen; a page scrolls below that.
    if fitted && let Some(content) = window.tallest {
        let selector = Page::ALL.len() as f32 * (PAGE_BUTTON.y + ui.spacing().item_spacing.y);
        let wanted = content.max(selector) + FOOTER_GAP + FOOTER;
        // The shell sizes the window from this allocation.
        ui.allocate_space(Vec2::new(area.width(), wanted));
        area.set_height(wanted.min(area.height()));
    } else {
        ui.advance_cursor_after_rect(area);
    }
    let footer = Rect::from_min_max(Pos2::new(area.left(), area.bottom() - FOOTER), area.max);
    let body = Rect::from_min_max(area.min, Pos2::new(area.right(), footer.top() - FOOTER_GAP));
    let mut body_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("options-body")
            .max_rect(body),
    );
    body_ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(SELECTOR_WIDTH, body.height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| selector(ui, window, facts, actions),
        );
        ui.separator();
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(width, body.height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                let page = window.page;
                let scrolled = egui::ScrollArea::vertical()
                    .id_salt(("options-page", page))
                    .auto_shrink([false, false])
                    .max_height(body.height())
                    .show(ui, |ui| {
                        ui.set_width(width - ui.spacing().scroll.allocated_width());
                        pages::notices(ui, window, facts, actions);
                        pages::page(ui, page, &mut window.draft.values, facts, actions);
                    });
                view::name_scroll_area(ui, ("options-page", page), "Options page");
                let content = scrolled.content_size.y;
                window.tallest = Some(window.tallest.map_or(content, |seen| seen.max(content)));
            },
        );
    });
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("options-actions")
            .max_rect(footer)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    actions_row(&mut row, window, facts, actions);
}

/// The page buttons, each with a marker when its page has unapplied edits.
/// Up and Down move between pages.
fn selector(
    ui: &mut egui::Ui,
    window: &mut OptionsWindow,
    facts: &OptionsFacts,
    actions: &mut Vec<Action>,
) {
    let changes = window.changes(facts);
    // The page whose button takes the focus this frame. An arrow key moves
    // it at once, so a second key in the next frame starts from the new page.
    let mut focus = window.focus_page.then_some(window.page);
    let mut ids = [egui::Id::NULL; Page::ALL.len()];
    for (page, changed) in Page::ALL.into_iter().zip(changes) {
        let selected = window.page == page;
        let response = ui.add_sized(
            PAGE_BUTTON,
            egui::Button::selectable(selected, labels::page(page)),
        );
        if changed > 0 {
            // Screen readers hear the marker as text.
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_description(unapplied(changed))
            });
            // The selected face takes the selection colour, so its marker
            // takes the ink.
            ui.painter().circle_filled(
                Pos2::new(response.rect.right() - 12.0, response.rect.center().y),
                3.5,
                if selected {
                    roles().ink
                } else {
                    roles().warning
                },
            );
        }
        form::register(&response);
        ids[page.index()] = response.id;
        if response.clicked() {
            actions.push(Action::Show(page));
        }
        if response.has_focus() {
            let step = ui.input_mut(|input| {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    1
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    -1
                } else {
                    0
                }
            });
            if step != 0 {
                ui.ctx()
                    .memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
                let count = Page::ALL.len() as isize;
                let next = Page::ALL[(page.index() as isize + step).rem_euclid(count) as usize];
                actions.push(Action::Show(next));
                focus = Some(next);
            }
        }
    }
    if let Some(page) = focus {
        ui.ctx()
            .memory_mut(|memory| memory.request_focus(ids[page.index()]));
        window.focus_page = false;
    }
}

/// Restore at the left; the status, Cancel, and Apply at the right.
fn actions_row(
    ui: &mut egui::Ui,
    window: &OptionsWindow,
    facts: &OptionsFacts,
    actions: &mut Vec<Action>,
) {
    if form::button(ui, RESTORE, controls::ButtonRole::Tertiary)
        .hover_hint("Put the defaults of every page in the draft. Nothing changes until Apply.")
        .clicked()
    {
        actions.push(Action::Restore);
    }
    let pending = window.pending(facts);
    let status = window.status(facts);
    let blocked = window.apply_blocked(facts);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if form::button(ui, CANCEL, controls::ButtonRole::Secondary).clicked() {
            actions.push(Action::Close);
        }
        ui.add_enabled_ui(blocked.is_none(), |ui| {
            let apply = form::button(ui, APPLY, controls::ButtonRole::Primary);
            if let Some(reason) = &blocked {
                apply.disabled_hint(reason);
            } else if apply.clicked() {
                actions.push(Action::Apply);
            }
        });
        if !status.is_empty() {
            let color = if pending {
                roles().warning
            } else {
                roles().legend
            };
            let label =
                ui.add(egui::Label::new(egui::RichText::new(&status).color(color)).truncate());
            // Screen readers hear the count as it changes.
            ui.ctx().accesskit_node_builder(label.id, |node| {
                node.set_live(egui::accesskit::Live::Polite)
            });
        }
    });
}

/// The prompt's text and its three answers.
fn discard_prompt(
    ui: &mut egui::Ui,
    window: &OptionsWindow,
    facts: &OptionsFacts,
) -> Option<Action> {
    let count: usize = window.changes(facts).iter().sum();
    ui.label(match count {
        0 => DEFAULTS_PROMPT.to_owned(),
        _ => format!("Options has {}.", unapplied(count)),
    });
    let blocked = window.apply_blocked(facts);
    let mut answer = None;
    ui.horizontal(|ui| {
        let apply = ui
            .add_enabled_ui(blocked.is_none(), |ui| dialog::primary(ui, APPLY_AND_CLOSE))
            .inner;
        if let Some(reason) = &blocked {
            apply.disabled_hint(reason);
        } else if apply.clicked() {
            answer = Some(Action::ApplyAndClose);
        }
        if dialog::danger(ui, DISCARD_CHANGES).clicked() {
            answer = Some(Action::Discard);
        }
        if controls::button(ui, KEEP_EDITING, controls::ButtonRole::Secondary).clicked() {
            answer = Some(Action::KeepEditing);
        }
    });
    answer
}
