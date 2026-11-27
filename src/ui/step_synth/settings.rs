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

fn source_mode_keys(ui: &mut egui::Ui, at: Place, state: &mut StepSynthState, editor: &mut Editor) {
    let half = at.half();
    for (x, text, mode, hint) in [
        (
            0.0,
            "Sample",
            pattern::SourceMode::Sample,
            "Play the selected WAV or palette sound.",
        ),
        (
            half + 6.0,
            "Synth",
            pattern::SourceMode::Synth,
            "Play the built-in oversampled oscillator without an audio sample.",
        ),
    ] {
        let selected = state.source_mode == mode;
        if key(
            ui,
            at.rect(x, 24.0, half, 24.0),
            Key {
                selected: Some(selected),
                ..Key::new(&format!("Source mode {text}"), text)
            },
        )
        .hover_hint(hint)
        .clicked()
            && !selected
        {
            state.source_mode = mode;
            editor.edit("the source mode change");
        }
    }
}

fn sample_source_controls(
    ui: &mut egui::Ui,
    at: Place,
    source: &Source<'_>,
    state: &mut StepSynthState,
    editor: &mut Editor,
) -> Option<Action> {
    let (width, half) = (at.width, at.half());
    let source_rect = at.rect(0.0, 54.0, width, 28.0);
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
            at.rect(x, 88.0, half, 28.0),
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
    let before = state.start_ms;
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
        at.rect(0.0, 122.0, half, 48.0),
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
        at.rect(half + 6.0, 122.0, half, 28.0),
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
    let looped = state.loop_source;
    toggle(
        ui,
        at.point(half + 14.0, 163.0),
        at.point(half + 30.0, 163.0),
        "Loop source",
        &mut state.loop_source,
    );
    named_change(editor, looped, state.loop_source, "the loop source change");
    action
}

fn synth_source_controls(
    ui: &mut egui::Ui,
    at: Place,
    state: &mut StepSynthState,
    editor: &mut Editor,
) {
    let (width, half) = (at.width, at.half());
    let third = (width - 12.0) / 3.0;
    for (i, (text, name, wave)) in [
        ("Saw", "Oscillator Saw", pattern::OscWaveform::Saw),
        ("Sq", "Oscillator Square", pattern::OscWaveform::Square),
        ("Tri", "Oscillator Triangle", pattern::OscWaveform::Triangle),
    ]
    .into_iter()
    .enumerate()
    {
        let selected = state.osc_waveform == wave;
        if key(
            ui,
            at.rect(i as f32 * (third + 6.0), 54.0, third, 28.0),
            Key {
                selected: Some(selected),
                ..Key::new(name, text)
            },
        )
        .hover_hint("Built-in oscillator waveform.")
        .clicked()
            && !selected
        {
            state.osc_waveform = wave;
            editor.edit("the waveform change");
        }
    }
    left_text(ui, at.point(2.0, 102.0), "Octave", 13.0, roles().legend);
    let (min_oct, max_oct) = (
        *pattern::OSC_OCTAVE_RANGE.start(),
        *pattern::OSC_OCTAVE_RANGE.end(),
    );
    let before_oct = state.osc_octave;
    if key(
        ui,
        at.rect(width - 108.0, 88.0, 28.0, 28.0),
        Key {
            enabled: state.osc_octave > min_oct,
            ..Key::new("Synth octave down", "−")
        },
    )
    .hover_hint("Play the oscillator one octave lower.")
    .disabled_hint("The oscillator is at its lowest octave.")
    .clicked()
    {
        state.osc_octave = (state.osc_octave - 1).max(min_oct);
    }
    let oct_well = at.rect(width - 76.0, 90.0, 44.0, 24.0);
    controls::well(ui, oct_well);
    let oct_label = if state.osc_octave > 0 {
        format!("+{}", state.osc_octave)
    } else {
        format!("{}", state.osc_octave)
    };
    lettering(ui, oct_well.center(), &oct_label, 12.5, roles().legend);
    if key(
        ui,
        at.rect(width - 28.0, 88.0, 28.0, 28.0),
        Key {
            enabled: state.osc_octave < max_oct,
            ..Key::new("Synth octave up", "+")
        },
    )
    .hover_hint("Play the oscillator one octave higher.")
    .disabled_hint("The oscillator is at its highest octave.")
    .clicked()
    {
        state.osc_octave = (state.osc_octave + 1).min(max_oct);
    }
    named_change(editor, before_oct, state.osc_octave, "the octave change");

    let mut sub_pct = f64::from(state.osc_sub) * 100.0;
    dial(
        ui,
        at.rect(0.0, 122.0, half, 48.0),
        &mut sub_pct,
        Dial {
            name: "Sub",
            low: 0.0,
            high: 100.0,
            reset: 0.0,
            step: 2.0,
            decimals: 0,
            logarithmic: false,
        },
        "Sub mixes a square oscillator one octave below the primary waveform.",
    );
    let next_sub = (sub_pct / 100.0).clamp(0.0, 1.0) as f32;
    named_change(editor, state.osc_sub, next_sub, "the sub level change");
    state.osc_sub = next_sub;

    let pitch_well = at.rect(half + 6.0, 132.0, half, 28.0);
    controls::well(ui, pitch_well);
    let hz = 65.406 * 2.0_f64.powi(i32::from(state.osc_octave));
    let pitch_text = format!("C{} · {:.0} Hz", 2 + state.osc_octave, hz);
    lettering(ui, pitch_well.center(), &pitch_text, 12.0, roles().legend);
}

/// The source's mode (Sample or Synth) and its controls.
fn source_section(
    ui: &mut egui::Ui,
    at: Place,
    source: &Source<'_>,
    state: &mut StepSynthState,
    editor: &mut Editor,
) -> Option<Action> {
    controls::section(ui, at.rect(0.0, 0.0, at.width, 20.0), "Source");
    source_mode_keys(ui, at, state, editor);
    if state.source_mode == pattern::SourceMode::Synth {
        synth_source_controls(ui, at, state, editor);
        None
    } else {
        sample_source_controls(ui, at, source, state, editor)
    }
}

fn pattern_length_row(
    ui: &mut egui::Ui,
    at: Place,
    state: &mut StepSynthState,
    editor: &mut Editor,
) {
    let width = at.width;
    let before = state.length;
    left_text(ui, at.point(2.0, 226.0), "Length", 13.0, roles().legend);
    length_control(
        ui,
        at.rect(width - 132.0, 214.0, 52.0, 24.0),
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
            at.rect(x, 212.0, 34.0, 28.0),
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
}

fn pattern_file_row(
    ui: &mut egui::Ui,
    at: Place,
    state: &mut StepSynthState,
    editor: &mut Editor,
    icons: Option<&RailIcons>,
) -> Option<Action> {
    let mut action = None;
    let third = (at.width - 12.0) / 3.0;
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
            at.rect(x, 246.0, third, 28.0),
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
        at.rect(2.0 * third + 12.0, 246.0, third, 28.0),
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
    controls::section(ui, at.rect(0.0, 186.0, width, 20.0), "Pattern");
    pattern_length_row(ui, at, state, editor);
    let mut action = pattern_file_row(ui, at, state, editor, icons);
    let response = key(
        ui,
        at.rect(0.0, 280.0, width, 28.0),
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
    controls::section(ui, at.rect(0.0, 322.0, width, 20.0), "Tempo");
    left_text(ui, at.point(2.0, 360.0), "BPM", 13.0, roles().legend);
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(at.rect(44.0, 348.0, width - 44.0, 24.0)),
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
        at.rect(0.0, 382.0, half, 48.0),
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
        at.rect(half + 6.0, 382.0, half, 48.0),
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

fn voice_preset_combo(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    editor: &mut Editor,
) {
    let current = pattern::matching_step_synth_preset(state).unwrap_or("Custom");
    let ctx = ui.ctx().clone();
    let mut combo_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    form_style(&mut combo_ui);
    let combo = egui::ComboBox::from_id_salt("step-synth-preset")
        .selected_text(current)
        .width(rect.width())
        .truncate()
        .height(340.0);
    let response = view::tall_menu(&ctx, || {
        combo.show_ui(&mut combo_ui, |ui| {
            for preset in pattern::step_synth_presets() {
                if ui
                    .selectable_label(preset.name == current, preset.name)
                    .clicked()
                {
                    preset.apply(state);
                    editor.edit("the preset change");
                    ui.close();
                }
            }
        })
    })
    .response;
    controls::reveal_focus(ui, &response);
    form::choice_named(&response, "Step synth preset", current);
    response.hover_hint("Load Voice and Effects preset while keeping all notes.");
}

#[inline]
fn snap_amp_decay(before: f64, raw: f64) -> f64 {
    let min = *pattern::AMP_DECAY_MS_RANGE.start();
    let max = *pattern::AMP_DECAY_MS_RANGE.end();
    if raw <= 0.0 || (before >= min && raw < min) {
        0.0
    } else if before == 0.0 && raw > 0.0 && raw < min {
        min
    } else if raw < min * 0.5 {
        0.0
    } else {
        raw.clamp(min, max)
    }
}

/// Presets, Accent, Slide time, Amp decay, Filter envelope depth, and Env decay.
fn voice_section(ui: &mut egui::Ui, at: Place, state: &mut StepSynthState, editor: &mut Editor) {
    let (width, half) = (at.width, at.half());
    controls::section(ui, at.rect(0.0, 444.0, width, 20.0), "Voice");
    left_text(ui, at.point(2.0, 483.0), "Preset", 13.0, roles().legend);
    voice_preset_combo(ui, at.rect(52.0, 470.0, width - 52.0, 26.0), state, editor);

    let before = (
        state.accent_db,
        state.slide_ms,
        state.amp_decay_ms,
        state.filter_env_amount,
        state.filter_env_decay_ms,
    );
    dial(
        ui,
        at.rect(0.0, 504.0, half, 48.0),
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
        at.rect(half + 6.0, 504.0, half, 48.0),
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
    let mut amp_decay = state.amp_decay_ms;
    dial(
        ui,
        at.rect(0.0, 558.0, half, 48.0),
        &mut amp_decay,
        Dial {
            name: "Amp decay",
            low: 0.0,
            high: *pattern::AMP_DECAY_MS_RANGE.end(),
            reset: 0.0,
            step: 5.0,
            decimals: 0,
            logarithmic: false,
        },
        "Exponential volume decay per phrase in ms; 0 keeps full note gate sustain.",
    );
    state.amp_decay_ms = snap_amp_decay(before.2, amp_decay);
    let mut env_pct = f64::from(state.filter_env_amount) * 100.0;
    dial(
        ui,
        at.rect(half + 6.0, 558.0, half, 48.0),
        &mut env_pct,
        Dial {
            name: "Filter env",
            low: -100.0,
            high: 100.0,
            reset: 0.0,
            step: 2.0,
            decimals: 0,
            logarithmic: false,
        },
        "Bipolar filter envelope depth in percent; accented notes boost the sweep.",
    );
    state.filter_env_amount = (env_pct / 100.0).clamp(-1.0, 1.0) as f32;
    dial(
        ui,
        at.rect(0.0, 612.0, half, 48.0),
        &mut state.filter_env_decay_ms,
        Dial {
            name: "Env decay",
            low: *pattern::FILTER_ENV_DECAY_MS_RANGE.start(),
            high: *pattern::FILTER_ENV_DECAY_MS_RANGE.end(),
            reset: pattern::DEFAULT_FILTER_ENV_DECAY_MS,
            step: 5.0,
            decimals: 0,
            logarithmic: true,
        },
        "Filter envelope sweep decay time in ms.",
    );
    named_change(editor, before.0, state.accent_db, "the accent level change");
    named_change(editor, before.1, state.slide_ms, "the slide time change");
    named_change(editor, before.2, state.amp_decay_ms, "the amp decay change");
    named_change(
        editor,
        before.3,
        state.filter_env_amount,
        "the filter env change",
    );
    named_change(
        editor,
        before.4,
        state.filter_env_decay_ms,
        "the env decay change",
    );
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
