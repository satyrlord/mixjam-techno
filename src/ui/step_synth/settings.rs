//! The settings column at the left of the board, and the effects pane that
//! takes the grid's place.
use super::*;

/// The settings column: source, pattern, tempo, and voice. It
/// scrolls on its own, so the grid keeps the panel's height.
pub(super) fn settings(
    ui: &mut egui::Ui,
    rect: Rect,
    source: Source<'_>,
    state: &mut StepSynthState,
    editor: &mut Editor,
    icons: Option<&RailIcons>,
) -> Action {
    let mut action = Action::None;
    let mut column = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("step-synth-settings")
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    column.set_clip_rect(rect.intersect(ui.clip_rect()));
    scroll_style(&mut column);
    egui::ScrollArea::vertical()
        .id_salt("step-synth-scroll")
        .auto_shrink([false, false])
        .max_height(rect.height())
        .show(&mut column, |ui| {
            let width = rect.width() - 12.0;
            let (bounds, _) =
                ui.allocate_exact_size(Vec2::new(width, SETTINGS_HEIGHT), Sense::hover());
            let at = Place {
                origin: bounds.min,
                width,
            };
            if let Some(next) = source_section(ui, at, &source, state, editor) {
                action = next;
            }
            if let Some(next) = pattern_section(ui, at, state, editor, icons, source.playable()) {
                action = next;
            }
            tempo_section(ui, at, state, editor);
            voice_section(ui, at, state, editor);
        });
    super::super::view::name_scroll_area(&column, "step-synth-scroll", "Step synth settings");
    action
}

/// Where the column's content sits: its top left and its width.
#[derive(Clone, Copy)]
struct Place {
    origin: Pos2,
    width: f32,
}

impl Place {
    fn point(self, x: f32, y: f32) -> Pos2 {
        self.origin + Vec2::new(x, y)
    }

    fn rect(self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.point(x, y), Vec2::new(w, h))
    }

    /// The width of a key in a row of two.
    fn half(self) -> f32 {
        (self.width - 6.0) / 2.0
    }
}

/// Name the edit when a setting changed, for the undo message.
fn named_change<T: PartialEq>(editor: &mut Editor, before: T, after: T, what: &'static str) {
    if before != after {
        editor.edit(what);
    }
}

/// The source's name, the keys that pick it, where its notes start, and
/// Loop source.
fn source_section(
    ui: &mut egui::Ui,
    at: Place,
    source: &Source<'_>,
    state: &mut StepSynthState,
    editor: &mut Editor,
) -> Option<Action> {
    let (width, half) = (at.width, at.half());
    controls::section(ui, at.rect(0.0, 0.0, width, 20.0), "Source");
    let source_rect = at.rect(0.0, 26.0, width, 28.0);
    controls::well(ui, source_rect);
    let label = ui.put(
        source_rect.shrink2(Vec2::new(8.0, 4.0)),
        egui::Label::new(
            egui::RichText::new(source.name)
                .font(FontId::proportional(12.5))
                .color(roles().legend),
        )
        .truncate(),
    );
    named(&label, egui::WidgetType::Label, "Step synth source sound");
    ui.ctx().accesskit_node_builder(label.id, |node| {
        node.set_label("Step synth source sound");
        node.set_value(source.name);
    });
    label.hover_hint(source.name);
    // Where the source comes from: the palette's selected sound, or
    // a file. File… opens the chooser each time.
    let mut action = None;
    for (x, text, selected, next, hint) in [
        (
            0.0,
            "Palette sound",
            !source.explicit_file,
            Action::PaletteSource,
            "Play the sound selected in the palette.",
        ),
        (
            half + 6.0,
            "Source file…",
            source.explicit_file,
            Action::ChooseSource,
            "Choose a WAV file to play instead of the palette sound. \
             The pattern file keeps the choice.",
        ),
    ] {
        if key(
            ui,
            at.rect(x, 60.0, half, 28.0),
            Key {
                selected: Some(selected),
                ..Key::new(text, text)
            },
        )
        .hover_hint(hint)
        .clicked()
        {
            action = Some(next);
        }
    }
    // Where each note starts to read the source: a sound with silence
    // before its hit plays late on every step without it.
    let before = state.start_ms;
    // Start ends 1 ms before the source does, so a note always has audio.
    let high = source
        .length_ms
        .map_or(*pattern::START_MS_RANGE.end(), |ms| {
            (ms - 1.0).floor().clamp(
                *pattern::START_MS_RANGE.start(),
                *pattern::START_MS_RANGE.end(),
            )
        });
    dial(
        ui,
        at.rect(0.0, 94.0, half, 48.0),
        &mut state.start_ms,
        Dial {
            name: "Start",
            low: *pattern::START_MS_RANGE.start(),
            high,
            reset: 0.0,
            step: 1.0,
            decimals: 0,
            logarithmic: false,
        },
        "Start is where each note starts to read the source, in ms. Raise it to \
         cut the silence before the sound's first hit, so notes play on the beat.",
    );
    named_change(editor, before, state.start_ms, "the start change");
    let response = key(
        ui,
        at.rect(half + 6.0, 94.0, half, 28.0),
        Key {
            enabled: source.playable(),
            ..Key::new("Find start", "Find start")
        },
    );
    let response = if source.playable() {
        response.hover_hint("Set Start to the source's first hit.")
    } else {
        response.disabled_hint(NO_SOURCE)
    };
    if response.clicked() {
        action = Some(Action::FindStart);
    }
    // Whether a note repeats its source: like Start, it suits one source.
    let looped = state.loop_source;
    toggle(
        ui,
        at.point(half + 14.0, 135.0),
        at.point(half + 30.0, 135.0),
        "Loop source",
        &mut state.loop_source,
    );
    named_change(editor, looped, state.loop_source, "the loop source change");
    action
}

/// The pattern's length, its file keys, and Save sound.
fn pattern_section(
    ui: &mut egui::Ui,
    at: Place,
    state: &mut StepSynthState,
    editor: &mut Editor,
    icons: Option<&RailIcons>,
    available: bool,
) -> Option<Action> {
    let width = at.width;
    controls::section(ui, at.rect(0.0, 158.0, width, 20.0), "Pattern");
    // The pattern's length, with the keys that double and halve it.
    let before = state.length;
    left_text(ui, at.point(2.0, 198.0), "Length", 13.0, roles().legend);
    length_control(
        ui,
        at.rect(width - 132.0, 186.0, 52.0, 24.0),
        &mut state.length,
    );
    for (x, text, name, double, next, hint, disabled) in [
        (
            width - 72.0,
            "×2",
            "Double length",
            true,
            pattern::doubled_length(state.length),
            "Double length: the new steps repeat the loop when they are empty.",
            "The pattern is at its longest length, 64 steps.",
        ),
        (
            width - 34.0,
            "÷2",
            "Halve length",
            false,
            pattern::halved_length(state.length),
            "Halve length: the steps past the new end keep their notes.",
            "The pattern is at its shortest length, 1 step.",
        ),
    ] {
        let enabled = next != state.length;
        let response = key(
            ui,
            at.rect(x, 184.0, 34.0, 28.0),
            Key {
                enabled,
                ..Key::new(name, text)
            },
        );
        let response = if enabled {
            response.hover_hint(hint)
        } else {
            response.disabled_hint(disabled)
        };
        if controls::primary_clicked(ui, &response) {
            if double {
                pattern::double_length(state);
            } else {
                state.length = next;
            }
        }
    }
    if state.length != before {
        editor.edit("the length change");
    }
    let mut action = None;
    let third = (width - 12.0) / 3.0;
    for (x, text, name, icon, next, hint) in [
        (
            0.0,
            "Load",
            "Load pattern",
            RailIcon::Open,
            Action::LoadPattern,
            "Load an editable pattern from a JSON file.",
        ),
        (
            third + 6.0,
            "Save",
            "Save pattern",
            RailIcon::Save,
            Action::SavePattern,
            "Save the editable pattern as a JSON file: its notes, settings, \
             effects, scale, and source file.",
        ),
    ] {
        if key(
            ui,
            at.rect(x, 218.0, third, 28.0),
            Key {
                glyph: Glyph::rail(icons, icon),
                ..Key::new(name, text)
            },
        )
        .hover_hint(hint)
        .clicked()
        {
            action = Some(next);
        }
    }
    let empty = pattern::pattern_is_empty(state);
    let response = key(
        ui,
        at.rect(2.0 * third + 12.0, 218.0, third, 28.0),
        Key {
            enabled: !empty,
            ..Key::new("Clear pattern", "Clear")
        },
    );
    let response = if empty {
        response.disabled_hint("The pattern has no notes.")
    } else {
        response.hover_hint(
            "Clear pattern: empty all four bars. The settings and effects stay. \
             Ctrl+Z over the board takes it back.",
        )
    };
    if response.clicked() {
        pattern::clear_pattern(state);
        editor.selection = None;
        editor.edit_saying("Clear pattern", "Cleared the pattern.".into());
    }
    let response = key(
        ui,
        at.rect(0.0, 252.0, width, 28.0),
        Key {
            enabled: available,
            primary: true,
            ..Key::new("Save sound", "Save sound")
        },
    );
    let response = if available {
        response.hover_hint("Render the pattern to a WAV file and add it to Step synth.")
    } else {
        response.disabled_hint(NO_SOURCE)
    };
    if response.clicked() {
        action = Some(Action::SaveSound);
    }
    action
}

/// BPM, Swing, and Gate.
fn tempo_section(ui: &mut egui::Ui, at: Place, state: &mut StepSynthState, editor: &mut Editor) {
    let (width, half) = (at.width, at.half());
    let before = (state.bpm, state.swing, state.gate);
    controls::section(ui, at.rect(0.0, 294.0, width, 20.0), "Tempo");
    left_text(ui, at.point(2.0, 332.0), "BPM", 13.0, roles().legend);
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(at.rect(44.0, 320.0, width - 44.0, 24.0)),
        |ui| {
            form_style(ui);
            name_control(
                bpm_picker(ui, "Step synth BPM", &mut state.bpm, width - 44.0),
                "Step synth BPM",
            );
        },
    );
    let mut swing = f64::from(state.swing);
    dial(
        ui,
        at.rect(0.0, 354.0, half, 48.0),
        &mut swing,
        Dial {
            name: "Swing",
            low: 50.0,
            high: 75.0,
            reset: 50.0,
            step: 1.0,
            decimals: 0,
            logarithmic: false,
        },
        "Swing delays every second sixteenth, in percent of the eighth: \
         50 is straight, 67 a triplet feel, and 75 a dotted feel.",
    );
    state.swing = swing.round() as u8;
    let mut gate = f64::from(state.gate);
    dial(
        ui,
        at.rect(half + 6.0, 354.0, half, 48.0),
        &mut gate,
        Dial {
            name: "Gate",
            low: 5.0,
            high: 100.0,
            reset: 100.0,
            step: 5.0,
            decimals: 0,
            logarithmic: false,
        },
        "Gate is how much of its last step a note sounds for, in percent. \
         A note that slides into the next one ignores it.",
    );
    state.gate = gate.round() as u8;
    named_change(editor, before.0, state.bpm, "the BPM change");
    named_change(editor, before.1, state.swing, "the swing change");
    named_change(editor, before.2, state.gate, "the gate change");
}

/// Accent and Slide time.
fn voice_section(ui: &mut egui::Ui, at: Place, state: &mut StepSynthState, editor: &mut Editor) {
    let (width, half) = (at.width, at.half());
    let before = (state.accent_db, state.slide_ms);
    controls::section(ui, at.rect(0.0, 416.0, width, 20.0), "Voice");
    dial(
        ui,
        at.rect(0.0, 442.0, half, 48.0),
        &mut state.accent_db,
        Dial {
            name: "Accent",
            low: 0.0,
            high: 12.0,
            reset: 6.0,
            step: 0.5,
            decimals: 1,
            logarithmic: false,
        },
        "Accent is how much louder an accented note plays, in dB.",
    );
    dial(
        ui,
        at.rect(half + 6.0, 442.0, half, 48.0),
        &mut state.slide_ms,
        Dial {
            name: "Slide time",
            low: 1.0,
            high: 1000.0,
            reset: 60.0,
            step: 1.0,
            decimals: 0,
            logarithmic: true,
        },
        "Slide time is how long a slide takes to reach the next note's pitch, in ms.",
    );
    named_change(editor, before.0, state.accent_db, "the accent level change");
    named_change(editor, before.1, state.slide_ms, "the slide time change");
}

/// The pattern effects in place of the note grid. Listen keeps its place at
/// the top, so the effects can be heard as they change.
pub(super) fn effects_pane(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    editor: &mut Editor,
    available: bool,
    playing: Option<usize>,
) -> Action {
    let action = listen_key(
        ui,
        Rect::from_min_size(rect.min, Vec2::new(LISTEN_WIDTH, 28.0)),
        available,
        playing.is_some(),
        true,
    );
    play_strip(
        ui,
        Rect::from_min_max(
            rect.min + Vec2::new(LISTEN_WIDTH + 12.0, 6.0),
            Pos2::new(rect.right() - 10.0, rect.top() + 22.0),
        ),
        usize::from(state.length),
        playing,
    );
    let effects_rect = Rect::from_min_max(rect.min + Vec2::new(0.0, 36.0), rect.max);
    let mut form = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(effects_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    form.set_clip_rect(effects_rect.intersect(ui.clip_rect()));
    form_style(&mut form);
    scroll_style(&mut form);
    egui::ScrollArea::vertical()
        .id_salt("step-synth-effects")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .auto_shrink([false, false])
        .max_height(effects_rect.height())
        .show(&mut form, |ui| {
            let before = state.effects.clone();
            super::super::effects_rack::draw_rack_scrolling(ui, &mut state.effects, true);
            named_change(editor, before, state.effects.clone(), "the effect change");
        });
    super::super::view::name_scroll_area(&form, "step-synth-effects", "Step synth effects");
    action
}

/// One cell for each step of the pattern beside Listen, so the playing step
/// shows while the effects hide the grid. A taller cell starts each bar.
fn play_strip(ui: &mut egui::Ui, rect: Rect, length: usize, playing: Option<usize>) {
    if length == 0 || rect.width() <= 0.0 {
        return;
    }
    let width = rect.width() / length as f32;
    let painter = ui.painter();
    for step in 0..length {
        let left = rect.left() + step as f32 * width;
        let top = if step % BAR == 0 {
            rect.top()
        } else {
            rect.top() + 4.0
        };
        let cell = Rect::from_min_max(Pos2::new(left, top), Pos2::new(left + width, rect.bottom()))
            .shrink2(Vec2::new((width * 0.15).min(1.0), 0.0));
        let fill = if playing == Some(step) {
            roles().coral
        } else if step % 4 == 0 {
            roles().cell_edge
        } else {
            roles().face
        };
        painter.rect_filled(cell, 1.0, fill);
    }
    let response = ui.interact(rect, ui.id().with("play-strip"), Sense::hover());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Label,
            true,
            match playing {
                Some(step) => format!(
                    "Playing step {} of {length}, bar {}",
                    step + 1,
                    step / BAR + 1
                ),
                None => format!("Pattern steps: {length}, not playing"),
            },
        )
    });
}
