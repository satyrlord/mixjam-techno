//! Input outside the widgets: the window's keyboard shortcuts, the capture
//! keys, and song files dropped on the window.
use super::*;

impl MixJamApp {
    /// Open the first song file dropped on the window, and mark the window
    /// while a file hovers over it.
    pub(super) fn file_drop(&mut self, ctx: &egui::Context) {
        let is_song = |path: &Path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    SONG_EXTENSIONS
                        .iter()
                        .any(|song| extension.eq_ignore_ascii_case(song))
                })
        };
        let (hovered, dropped) = ctx.input(|input| {
            (
                input
                    .raw
                    .hovered_files
                    .iter()
                    .map(|file| file.path.clone())
                    .collect::<Vec<_>>(),
                input
                    .raw
                    .dropped_files
                    .iter()
                    .filter_map(|file| file.path.clone())
                    .collect::<Vec<_>>(),
            )
        });
        if !hovered.is_empty() {
            let song = hovered.iter().flatten().any(|path| is_song(path));
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("file-drop"),
            ));
            let screen = ctx.viewport_rect();
            painter.rect_filled(screen, 0.0, Color32::from_black_alpha(150));
            painter.rect_stroke(
                screen.shrink(12.0),
                8.0,
                Stroke::new(3.0_f32, roles().accent),
                egui::StrokeKind::Inside,
            );
            painter.text(
                screen.center(),
                Align2::CENTER_CENTER,
                if song {
                    "Drop to open the song"
                } else {
                    "Only song files open here"
                },
                FontId::proportional(24.0),
                Color32::WHITE,
            );
        }
        if dropped.is_empty() {
            return;
        }
        let Some(path) = dropped.into_iter().find(|path| is_song(path)) else {
            self.error(format!(
                "Cannot open the dropped file: songs are .{} files.",
                SONG_EXTENSIONS.join(", .")
            ));
            return;
        };
        if let Some(identity) = self.blocking_modal(ctx) {
            dialog::expand(ctx, identity);
            self.error(format!(
                "Finish or cancel {identity} before starting another file action."
            ));
            return;
        }
        self.request_action(PendingAction::OpenPath(path), ctx);
    }

    pub(super) fn capture_keyboard(&mut self, ctx: &egui::Context) {
        // F12 saves what playback just played, wherever the focus is: the
        // user presses it while hearing a fault. Once per press.
        let capture = ctx.input_mut(|input| {
            let pressed = input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::F12, pressed: true, repeat: false, modifiers, ..
                    } if modifiers.is_none()
                )
            });
            input.consume_key(egui::Modifiers::NONE, egui::Key::F12);
            pressed
        });
        if capture {
            let _ = self.session.save_playback_capture("Saved with F12");
        }
    }

    pub(super) fn keyboard(&mut self, ctx: &egui::Context) {
        // The command list owns the keys while it is open.
        if self.commands.is_some() {
            return;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.run_command(commands::Command::Options, ctx);
            return;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::K)) {
            self.commands = Some(commands::CommandList::new());
            return;
        }
        if let Some(options) = &self.options
            && !dialog::is_collapsed(ctx, "Options")
            && dialog::top_window(ctx, "Options")
        {
            if !egui::Popup::is_any_open(ctx) {
                let direction = ctx.input_mut(|input| {
                    if input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab) {
                        Some(true)
                    } else if input.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
                        Some(false)
                    } else {
                        None
                    }
                });
                if let Some(reverse) = direction {
                    options.cycle_focus(ctx, reverse);
                }
            }
            // The top utility owns its keys. Its shell handles Escape after
            // text fields and popups have had their normal input pass.
            return;
        }
        let focused = ctx.memory(|memory| memory.focused());
        // A value that is being typed keeps every key.
        if focused.is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some()) {
            return;
        }
        // A focused button or switch takes Space itself. A dial or fader uses
        // only arrows, Home, and End, so the transport keeps Space there.
        let free = focused.is_none();
        let transport = free || focused == controls::arrow_key_focus(ctx);
        // Held keys repeat. A toggle or a file action must run once per press.
        let pressed_once = |key: egui::Key| {
            ctx.input(|i| {
                i.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key { key: pressed, pressed: true, repeat: false, .. }
                            if *pressed == key
                    )
                })
            })
        };
        if transport && pressed_once(egui::Key::Space) {
            self.run_command(commands::Command::PlayPause, ctx);
        }
        if free && ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.run_command(commands::Command::Delete, ctx);
        }
        if free && ctx.input(|i| i.modifiers.shift) && pressed_once(egui::Key::F10) {
            self.clip_menu.by_key = true;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            let redo = ctx.input(|i| i.modifiers.shift);
            self.history_step(redo);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Y)) {
            self.run_command(commands::Command::Redo, ctx);
        }
        if ctx.input(|i| i.modifiers.command) && pressed_once(egui::Key::S) {
            self.run_command(
                if ctx.input(|i| i.modifiers.shift) {
                    commands::Command::SaveAs
                } else {
                    commands::Command::Save
                },
                ctx,
            );
        }
        if ctx.input(|i| i.modifiers.command) && pressed_once(egui::Key::N) {
            self.run_command(commands::Command::New, ctx);
        }
        if ctx.input(|i| i.modifiers.command) && pressed_once(egui::Key::A) {
            self.run_command(commands::Command::SelectAll, ctx);
        }
        if ctx.input(|i| i.modifiers.command) && pressed_once(egui::Key::D) {
            self.run_command(commands::Command::Duplicate, ctx);
        }
        if pressed_once(egui::Key::F1) {
            self.run_command(commands::Command::Help, ctx);
        }
        // F maximises the score or restores it; typing already returned.
        if ctx.input(|i| i.modifiers.is_none())
            && !egui::Popup::is_any_open(ctx)
            && pressed_once(egui::Key::F)
        {
            self.run_command(commands::Command::Maximise, ctx);
        }
        if ctx.input(|i| i.modifiers.command) && pressed_once(egui::Key::O) {
            self.run_command(commands::Command::Open, ctx);
        }
        if transport && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.gestures.drag = None;
            self.gestures.handle_drag = None;
            self.gestures.automation_drag = None;
            self.session.end_automation_gesture();
            self.score_tools.erase = false;
            self.selection.place_on_click = false;
        }
    }
}
