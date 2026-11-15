//! The board's top row in the lower panel: the Notes and Effects switch, the
//! board's undo and redo keys, the status line and, for missing audio,
//! Import details.
use super::*;

/// The toolbar, `width` points wide, as the Mastering status sits.
pub(in super::super) fn toolbar(
    ui: &mut egui::Ui,
    width: f32,
    source: Source<'_>,
    feedback: Feedback<'_>,
    show_effects: &mut bool,
    history: History<'_>,
) -> Action {
    let missing = source.missing_audio();
    let status = feedback.message.unwrap_or(if missing {
        "Missing audio. Choose a source file or open Import details."
    } else if !source.available {
        "Choose a source file or select a sound in the palette."
    } else if *show_effects {
        "Adjust the pattern effects. Notes returns to the grid."
    } else {
        "Click places or clears a note. Drag a note's end to hold it."
    });
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width.max(0.0), 28.0), Sense::hover());
    let mut strip = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    dialog::hardware_style(&mut strip, controls::Palette::Olive);
    pane_switch(&mut strip, rect.min, show_effects);
    let (action, x) = history_keys(&mut strip, rect.min + Vec2::new(154.0, 0.0), &history);
    let rect = Rect::from_min_max(Pos2::new(x + 12.0, rect.top()), rect.max);
    status_footer(&mut strip, rect, status, feedback.error, missing);
    let details = Rect::from_min_size(
        Pos2::new(rect.right() - 130.0, rect.top() + 1.0),
        Vec2::new(128.0, 26.0),
    );
    if missing && button(&mut strip, details, "Import details", true, false).clicked() {
        Action::Details
    } else {
        action
    }
}

/// Notes and Effects, the choice keys that switch the board's pane.
fn pane_switch(ui: &mut egui::Ui, at: Pos2, show_effects: &mut bool) {
    for (x, text, name, effects, hint) in [
        (
            0.0,
            "Notes",
            "Notes pane",
            false,
            "Edit the notes in the grid.",
        ),
        (
            74.0,
            "Effects",
            "Effects pane",
            true,
            "Shape the pattern's sound with its effects.",
        ),
    ] {
        if key(
            ui,
            Rect::from_min_size(at + Vec2::new(x, 0.0), Vec2::new(72.0, 28.0)),
            Key {
                selected: Some(*show_effects == effects),
                ..Key::new(name, text)
            },
        )
        .hover_hint(hint)
        .clicked()
        {
            *show_effects = effects;
        }
    }
}

/// The board's own undo and redo keys, apart from the song's, from `at`.
/// Returns the action and where the keys end.
fn history_keys(ui: &mut egui::Ui, at: Pos2, history: &History<'_>) -> (Action, f32) {
    let mut action = Action::None;
    let mut x = at.x;
    for (redo, enabled, icon, text, name, hint) in [
        (
            false,
            history.undo,
            RailIcon::Undo,
            "Undo",
            "Undo pattern edit",
            "Undo the last pattern edit. Ctrl+Z over the board does the same.",
        ),
        (
            true,
            history.redo,
            RailIcon::Redo,
            "Redo",
            "Redo pattern edit",
            "Redo the pattern edit undone last. Ctrl+Y over the board does the same.",
        ),
    ] {
        let width = if history.icons.is_some() { 30.0 } else { 52.0 };
        let response = key(
            ui,
            Rect::from_min_size(Pos2::new(x, at.y), Vec2::new(width, 28.0)),
            Key {
                enabled,
                glyph: Glyph::rail(history.icons, icon),
                ..Key::new(name, if history.icons.is_some() { "" } else { text })
            },
        );
        let response = if enabled {
            response.hover_hint(hint)
        } else if redo {
            response.disabled_hint("No pattern edit to redo.")
        } else {
            response.disabled_hint("No pattern edit to undo.")
        };
        if response.clicked() {
            action = if redo { Action::Redo } else { Action::Undo };
        }
        x += width + 2.0;
    }
    (action, x)
}
