//! Step synth board. The settings sit in a fixed column at the left; the grid
//! column fills the rest of the lower panel and shows as many bars as keep
//! the steps wide enough. Shared hardware controls own presentation.
use super::*;
pub(super) struct Source<'a> {
    pub name: &'a str,
    pub available: bool,
    pub missing: bool,
    pub explicit_file: bool,
}

pub(super) struct Feedback<'a> {
    pub message: Option<&'a str>,
    pub error: bool,
}

pub(super) enum Action {
    None,
    ChooseSource,
    PaletteSource,
    Listen,
    SaveSequence,
    SavePattern,
    LoadPattern,
    Details,
}

/// The board's editing view: what the grid shows and the selected steps.
/// None of it is saved with the pattern.
#[derive(Clone, Debug, Default)]
pub(super) struct Editor {
    /// Octaves the 25 pitch rows are shifted by, from -3 to 3.
    pub octave: i8,
    pub scale: Scale,
    /// The scale's root in semitones above the source pitch, 0 to 11.
    pub root: u8,
    /// The first and last selected step. Lane edits apply to all of them.
    pub selection: Option<(usize, usize)>,
    /// The step shared by the pitch cursor, ruler, and value lanes.
    step: usize,
    seed: u64,
}

/// The scale that shades the pitch rows. Notes outside it stay playable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Scale {
    Chromatic,
    Major,
    #[default]
    Minor,
    Dorian,
    Phrygian,
    HarmonicMinor,
    MinorPentatonic,
    MajorPentatonic,
}
impl Scale {
    const ALL: [Self; 8] = [
        Self::Chromatic,
        Self::Major,
        Self::Minor,
        Self::Dorian,
        Self::Phrygian,
        Self::HarmonicMinor,
        Self::MinorPentatonic,
        Self::MajorPentatonic,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Chromatic => "Chromatic",
            Self::Major => "Major",
            Self::Minor => "Minor",
            Self::Dorian => "Dorian",
            Self::Phrygian => "Phrygian",
            Self::HarmonicMinor => "Harmonic minor",
            Self::MinorPentatonic => "Minor pentatonic",
            Self::MajorPentatonic => "Major pentatonic",
        }
    }
    fn degrees(self) -> &'static [i8] {
        match self {
            Self::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            Self::Major => &[0, 2, 4, 5, 7, 9, 11],
            Self::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Self::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Self::MinorPentatonic => &[0, 3, 5, 7, 10],
            Self::MajorPentatonic => &[0, 2, 4, 7, 9],
        }
    }
    fn contains(self, root: u8, note: i8) -> bool {
        let degree = (i32::from(note) - i32::from(root)).rem_euclid(12) as i8;
        self.degrees().contains(&degree)
    }
}
/// Root names, from the source pitch A upward.
const ROOTS: [&str; 12] = [
    "A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#",
];

/// The gap between the board's painted edge and its content.
const BOARD_INSET: f32 = 2.0;
/// The lower panel's height that shows the whole board: the 512-point board,
/// and the panel's margins, top row, and the spacing under it.
pub(super) const MODULE_HEIGHT: f32 = 512.0 + 2.0 * BOARD_INSET + 64.0;
/// The settings column's width, and the height of its content.
const SETTINGS_WIDTH: f32 = 232.0;
const SETTINGS_HEIGHT: f32 = 726.0;
/// Steps a pattern stores, and the steps in a bar.
const STEPS: usize = 64;
const BAR: usize = 16;
/// The narrowest step the grid draws. Wider boards show two or four bars.
const MIN_STEP_WIDTH: f32 = 28.0;
/// The pitch row's pitch and its cell's height.
const ROW: f32 = 16.0;
const CELL_HEIGHT: f32 = 14.0;
/// The pitch rows: 25 semitones around the octave in view.
const ROWS: i8 = 12;
/// The width of the label column at the left of the grid and lanes.
const GUTTER: f32 = 50.0;
/// The Accent, Slide, and Velocity lanes together.
const LANES_HEIGHT: f32 = 94.0;

/// Show the Step synth board in the space left in `ui`. The board is a tab
/// panel named Step synth, so agents can scope to it by that name.
/// `playing` is the pattern step Listen sequence is playing.
pub(super) fn show(
    ui: &mut egui::Ui,
    source: Source<'_>,
    state: &mut StepSynthState,
    view_bar: &mut usize,
    show_effects: &mut bool,
    editor: &mut Editor,
    playing: Option<usize>,
) -> Action {
    let ctx = ui.ctx().clone();
    let area = ui.available_rect_before_wrap();
    ui.advance_cursor_after_rect(area);
    let mut board = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("step-synth-board")
            .max_rect(area),
    );
    dialog::hardware_style(&mut board, controls::Palette::Olive);
    board.set_clip_rect(area.intersect(ui.clip_rect()));
    controls::panel(&board, area);
    ctx.accesskit_node_builder(board.unique_id(), |node| {
        node.set_role(egui::accesskit::Role::TabPanel);
        node.set_label("Step synth");
        node.set_bounds(egui::accesskit::Rect {
            x0: area.left().into(),
            y0: area.top().into(),
            x1: area.right().into(),
            y1: area.bottom().into(),
        });
    });
    let inner = area.shrink(BOARD_INSET + 8.0);
    let settings_rect = Rect::from_min_size(inner.min, Vec2::new(SETTINGS_WIDTH, inner.height()));
    let grid_rect = Rect::from_min_max(
        Pos2::new(settings_rect.right() + 8.0, inner.top()),
        inner.max,
    );
    controls::inset(&board, settings_rect);
    controls::inset(&board, grid_rect);
    let action = settings(
        &mut board,
        settings_rect.shrink(6.0),
        source,
        state,
        show_effects,
        editor,
    );
    if *show_effects {
        effects_pane(&mut board, grid_rect.shrink(8.0), state);
    } else {
        grid_column(
            &mut board,
            grid_rect.shrink(8.0),
            state,
            view_bar,
            editor,
            playing,
        );
    }
    action
}

/// The settings column: source, tempo, voice, scale, and the actions. It
/// scrolls on its own, so the grid keeps the panel's height.
fn settings(
    ui: &mut egui::Ui,
    rect: Rect,
    source: Source<'_>,
    state: &mut StepSynthState,
    show_effects: &mut bool,
    editor: &mut Editor,
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
            let point = |x: f32, y: f32| bounds.min + Vec2::new(x, y);
            let rect = |x, y, w, h| Rect::from_min_size(point(x, y), Vec2::new(w, h));
            let half = (width - 6.0) / 2.0;
            let ctx = ui.ctx().clone();
            let missing = source.missing && !source.explicit_file;
            let available = source.available && !missing;

            controls::section(ui, rect(0.0, 0.0, width, 20.0), "Source");
            let source_rect = rect(0.0, 26.0, width, 28.0);
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
            ctx.accesskit_node_builder(label.id, |node| {
                node.set_label("Step synth source sound");
                node.set_value(source.name);
            });
            label.hover_hint(source.name);
            left_text(
                ui,
                point(2.0, 64.0),
                if source.explicit_file {
                    "File source"
                } else {
                    "Palette sound"
                },
                11.0,
                roles().legend.gamma_multiply(0.8),
            );
            for (y, text, next) in [
                (74.0, "Choose source file", Action::ChooseSource),
                (108.0, "Use palette sound", Action::PaletteSource),
            ] {
                if button(ui, rect(0.0, y, width, 28.0), text, true, false).clicked() {
                    action = next;
                }
            }

            controls::section(ui, rect(0.0, 316.0, width, 20.0), "Tempo");
            left_text(ui, point(2.0, 354.0), "BPM", 13.0, roles().legend);
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(rect(44.0, 342.0, width - 44.0, 24.0)),
                |ui| {
                    form_style(ui);
                    name_control(
                        bpm_picker(ui, "Step synth BPM", &mut state.bpm, width - 44.0),
                        "Step synth BPM",
                    );
                },
            );
            let left = width / 4.0;
            let right = width * 3.0 / 4.0;
            let mut swing = f64::from(state.swing);
            dial(
                ui,
                [point(left, 382.0), point(left, 416.0), point(left, 450.0)],
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
                [
                    point(right, 382.0),
                    point(right, 416.0),
                    point(right, 450.0),
                ],
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

            controls::section(ui, rect(0.0, 474.0, width, 20.0), "Voice");
            dial(
                ui,
                [point(left, 504.0), point(left, 538.0), point(left, 572.0)],
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
                [
                    point(right, 504.0),
                    point(right, 538.0),
                    point(right, 572.0),
                ],
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
            toggle(
                ui,
                point(14.0, 606.0),
                point(38.0, 606.0),
                "Loop source",
                &mut state.loop_source,
            );

            controls::section(ui, rect(0.0, 630.0, width, 20.0), "Scale");
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(rect(0.0, 656.0, width, 26.0)),
                |ui| {
                    form_style(ui);
                    ui.horizontal(|ui| {
                        let response = egui::ComboBox::from_id_salt("step-synth-scale")
                            .selected_text(editor.scale.label())
                            .width(width - 76.0)
                            .show_ui(ui, |ui| {
                                for scale in Scale::ALL {
                                    ui.selectable_value(&mut editor.scale, scale, scale.label());
                                }
                            })
                            .response;
                        name_control(response, "Scale").hover_hint(
                            "The scale shades the pitch rows. Every note stays playable.",
                        );
                        let response = egui::ComboBox::from_id_salt("step-synth-root")
                            .selected_text(ROOTS[usize::from(editor.root % 12)])
                            .width(54.0)
                            .show_ui(ui, |ui| {
                                for (root, name) in ROOTS.iter().enumerate() {
                                    ui.selectable_value(&mut editor.root, root as u8, *name);
                                }
                            })
                            .response;
                        name_control(response, "Scale root")
                            .hover_hint("The scale's root. A is the source pitch.");
                    });
                },
            );
            if button(ui, rect(0.0, 690.0, width, 28.0), "Randomise", true, false)
                .hover_hint("Write a new line in the scale over the pattern's length.")
                .clicked()
            {
                randomise(state, editor);
            }

            controls::section(ui, rect(0.0, 146.0, width, 20.0), "Pattern");
            for (x, y, w, text, next, enabled, primary) in [
                (
                    0.0,
                    172.0,
                    width,
                    "Listen sequence",
                    Action::Listen,
                    available,
                    false,
                ),
                (
                    0.0,
                    206.0,
                    width,
                    "Save sequence",
                    Action::SaveSequence,
                    available,
                    true,
                ),
                (
                    0.0,
                    240.0,
                    half,
                    "Save pattern",
                    Action::SavePattern,
                    true,
                    false,
                ),
                (
                    half + 6.0,
                    240.0,
                    half,
                    "Load pattern",
                    Action::LoadPattern,
                    true,
                    false,
                ),
            ] {
                let response = button(ui, rect(x, y, w, 28.0), text, enabled, primary);
                let hint = match text {
                    "Listen sequence" | "Save sequence" if !available => {
                        "Choose an available source file or palette sound."
                    }
                    "Save sequence" => "Save a WAV and add it to Step synth.",
                    "Save pattern" => "Save the editable pattern as JSON.",
                    "Load pattern" => "Load an editable JSON pattern.",
                    _ => "Listen to the pattern.",
                };
                let response = if enabled {
                    response.hover_hint(hint)
                } else {
                    response.disabled_hint(hint)
                };
                if response.clicked() {
                    action = next;
                }
            }
            if keyed_button(
                ui,
                rect(0.0, 274.0, width, 28.0),
                "pane",
                if *show_effects {
                    "Note grid"
                } else {
                    "Sequence effects"
                },
                true,
                false,
            )
            .clicked()
            {
                *show_effects = !*show_effects;
            }
        });
    action
}

fn effects_pane(ui: &mut egui::Ui, rect: Rect, state: &mut StepSynthState) {
    left_text(
        ui,
        Pos2::new(rect.left() + 4.0, rect.top() + 10.0),
        "Sequence effects",
        13.0,
        roles().legend,
    );
    let effects_rect = Rect::from_min_max(rect.min + Vec2::new(0.0, 28.0), rect.max);
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
        .show(&mut form, |ui| effects_controls(ui, &mut state.effects));
}

/// Where the grid column's steps sit: the first step in view, how many show,
/// and where each one starts.
#[derive(Clone, Copy)]
struct Columns {
    first: usize,
    count: usize,
    left: f32,
    width: f32,
}
impl Columns {
    fn x(self, step: usize) -> f32 {
        self.left + (step as f32 - self.first as f32) * self.width
    }
    fn contains(self, step: usize) -> bool {
        (self.first..self.first + self.count).contains(&step)
    }
    /// The step under `x`, clamped to the steps in view.
    fn at(self, x: f32) -> usize {
        let offset = ((x - self.left) / self.width).floor().max(0.0) as usize;
        self.first + offset.min(self.count - 1)
    }
    /// The step boundary nearest `x`, from 0 to 64.
    fn boundary(self, x: f32) -> usize {
        let offset = ((x - self.left) / self.width).round().max(0.0) as usize;
        (self.first + offset.min(self.count)).min(STEPS)
    }
    fn span(self, from: usize, to: usize, top: f32, bottom: f32) -> Rect {
        Rect::from_min_max(Pos2::new(self.x(from), top), Pos2::new(self.x(to), bottom))
    }
}

/// For each step, the note that sounds there: its first step and pitch.
fn covers(state: &StepSynthState) -> [Option<(usize, i8)>; STEPS] {
    let mut covers = [None; STEPS];
    let mut current = None;
    for (index, step) in state.steps.iter().enumerate().take(STEPS) {
        current = match step.trig {
            Trig::Note(note) => Some((index, note)),
            Trig::Tie => current,
            Trig::Rest => None,
        };
        covers[index] = current;
    }
    covers
}

/// The last step of the note that starts at `head`.
fn note_end(steps: &[Step], head: usize) -> usize {
    let mut end = head;
    while end + 1 < steps.len() && steps[end + 1].trig == Trig::Tie {
        end += 1;
    }
    end
}

/// Clear the note that starts at `head`, with its ties.
fn clear_note(state: &mut StepSynthState, head: usize) {
    let end = note_end(&state.steps, head);
    state.steps[head] = Step::default();
    for step in &mut state.steps[head + 1..=end] {
        step.trig = Trig::Rest;
    }
}

fn grid_column(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    view_bar: &mut usize,
    editor: &mut Editor,
    playing: Option<usize>,
) {
    let grid_width = rect.width() - GUTTER - 10.0;
    let bars = [4, 2, 1]
        .into_iter()
        .find(|bars| grid_width / (*bars * BAR) as f32 >= MIN_STEP_WIDTH)
        .unwrap_or(1);
    // The edited bar is the one picked last; the grid shows its page of bars.
    *view_bar = (*view_bar).min(3);
    let columns = Columns {
        first: *view_bar / bars * bars * BAR,
        count: bars * BAR,
        left: rect.left() + GUTTER,
        width: grid_width / (bars * BAR) as f32,
    };
    bar_strip(
        ui,
        Rect::from_min_size(rect.min, Vec2::new(rect.width(), 28.0)),
        state,
        view_bar,
        bars,
        editor,
        playing,
    );
    let ruler = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + 36.0),
        Pos2::new(rect.right(), rect.top() + 58.0),
    );
    let lanes = Rect::from_min_max(
        // A short panel keeps more of its height for the pitch rows.
        Pos2::new(
            rect.left(),
            rect.bottom()
                - if rect.height() < 320.0 {
                    72.0
                } else {
                    LANES_HEIGHT
                },
        ),
        rect.max,
    );
    let grid = Rect::from_min_max(
        Pos2::new(rect.left(), ruler.bottom() + 4.0),
        Pos2::new(rect.right(), lanes.top() - 6.0),
    );
    step_ruler(ui, ruler, state, columns, editor, playing);
    if bars > 1 {
        // The viewed bar's mark over the ruler, for Copy bar and Clear bar.
        let first = *view_bar * BAR;
        ui.painter().rect_filled(
            columns
                .span(first, first + BAR, ruler.top() - 4.0, ruler.top() - 1.0)
                .shrink2(Vec2::new(1.0, 0.0)),
            1.0,
            controls::Palette::Olive.accent(),
        );
    }
    let octave_y = ruler.center().y;
    for (x, text, name, change) in [(0.0, "-", "Octave down", -1), (24.0, "+", "Octave up", 1)] {
        let enabled = (-3..=3).contains(&(editor.octave + change));
        if keyed_button(
            ui,
            Rect::from_min_size(
                Pos2::new(rect.left() + x, octave_y - 11.0),
                Vec2::new(22.0, 22.0),
            ),
            name,
            text,
            enabled,
            false,
        )
        .hover_hint(name)
        .clicked()
        {
            editor.octave += change;
        }
    }
    note_grid(ui, grid, state, view_bar, columns, editor);
    value_lanes(ui, lanes, state, columns, editor);
}

/// The bar strip: a button per bar that brings it into view and marks it in
/// the loop or playing, then Length and the bar actions.
fn bar_strip(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    view_bar: &mut usize,
    bars: usize,
    editor: &mut Editor,
    playing: Option<usize>,
) {
    let length = usize::from(state.length);
    for bar in 0..4 {
        let cell = Rect::from_min_size(
            rect.min + Vec2::new(bar as f32 * 40.0, 0.0),
            Vec2::new(36.0, 28.0),
        );
        let in_view = bar / bars == *view_bar / bars;
        let response = select_button(
            ui,
            cell,
            &format!("{}", bar + 1),
            &format!("View bar {}", bar + 1),
            bar == *view_bar,
        );
        if in_view && bar != *view_bar {
            // The other bars in view keep an outline.
            ui.painter().rect_stroke(
                cell.shrink(1.0),
                3.0,
                Stroke::new(1.5_f32, controls::Palette::Olive.accent()),
                egui::StrokeKind::Inside,
            );
        }
        if response.clicked() {
            *view_bar = bar;
        }
        // The loop mark under the bar: full when the whole bar plays.
        let looped = length.saturating_sub(bar * BAR).min(BAR) as f32 / BAR as f32;
        if looped > 0.0 {
            let mark = Rect::from_min_size(
                cell.left_bottom() + Vec2::new(2.0, 2.0),
                Vec2::new((cell.width() - 4.0) * looped, 3.0),
            );
            ui.painter()
                .rect_filled(mark, 1.0, controls::Palette::Olive.accent());
        }
        if playing.is_some_and(|step| step / BAR == bar) {
            ui.painter()
                .circle_filled(cell.right_top() + Vec2::new(-6.0, 6.0), 3.0, roles().coral);
        }
    }
    let x = rect.left() + 172.0;
    left_text(
        ui,
        Pos2::new(x, rect.center().y),
        "Length",
        13.0,
        roles().legend,
    );
    length_control(
        ui,
        Rect::from_min_size(Pos2::new(x + 48.0, rect.top() + 2.0), Vec2::new(44.0, 24.0)),
        &mut state.length,
    );
    for (offset, text, name, next) in [
        (
            98.0,
            "x2",
            "Double length",
            state.length.saturating_mul(2).min(64),
        ),
        (136.0, "/2", "Halve length", (state.length / 2).max(1)),
    ] {
        if keyed_button(
            ui,
            Rect::from_min_size(Pos2::new(x + offset, rect.top()), Vec2::new(34.0, 28.0)),
            name,
            text,
            true,
            false,
        )
        .hover_hint(name)
        .clicked()
        {
            state.length = next;
        }
    }
    let right = rect.right();
    let first = *view_bar * BAR;
    if button(
        ui,
        Rect::from_min_size(Pos2::new(right - 168.0, rect.top()), Vec2::new(80.0, 28.0)),
        "Copy bar",
        *view_bar < 3,
        false,
    )
    .hover_hint("Copy the viewed bar onto the next bar, replacing it.")
    .clicked()
    {
        let source = state.steps[first..first + BAR].to_vec();
        state.steps[first + BAR..first + 2 * BAR].copy_from_slice(&source);
    }
    if button(
        ui,
        Rect::from_min_size(Pos2::new(right - 82.0, rect.top()), Vec2::new(82.0, 28.0)),
        "Clear bar",
        true,
        false,
    )
    .hover_hint("Clear the steps of the viewed bar.")
    .clicked()
    {
        state.steps[first..first + BAR].fill(Step::default());
        editor.selection = None;
    }
}

/// The pattern length as a value well: drag sideways or scroll to change it,
/// or use the arrow keys, Home, and End.
fn length_control(ui: &mut egui::Ui, rect: Rect, length: &mut u8) {
    let response = ui.interact(rect, ui.id().with("length"), Sense::click_and_drag());
    let mut value = i32::from(*length);
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    let drag_id = response.id.with("drag");
    if response.drag_started() {
        ui.ctx()
            .data_mut(|data| data.insert_temp(drag_id, (value, 0.0_f32)));
    }
    if response.dragged()
        && let Some((start, travel)) = ui.ctx().data(|data| data.get_temp::<(i32, f32)>(drag_id))
    {
        let travel = travel + response.drag_delta().x - response.drag_delta().y;
        ui.ctx()
            .data_mut(|data| data.insert_temp(drag_id, (start, travel)));
        value = start + (travel / 6.0).round() as i32;
    }
    value += controls::wheel(ui, &response) as i32;
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
        ui.input_mut(|input| {
            for key in [egui::Key::ArrowUp, egui::Key::ArrowRight] {
                if input.consume_key(egui::Modifiers::NONE, key) {
                    value += 1;
                }
            }
            for key in [egui::Key::ArrowDown, egui::Key::ArrowLeft] {
                if input.consume_key(egui::Modifiers::NONE, key) {
                    value -= 1;
                }
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
                value = 1;
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
                value = 64;
            }
        });
    }
    ui.input(|input| {
        use egui::accesskit::{Action, ActionData};
        for request in input.accesskit_action_requests(response.id, Action::SetValue) {
            if let Some(ActionData::NumericValue(draft)) = request.data
                && draft.is_finite()
            {
                value = draft.round() as i32;
            }
        }
        value += input.num_accesskit_action_requests(response.id, Action::Increment) as i32
            - input.num_accesskit_action_requests(response.id, Action::Decrement) as i32;
    });
    *length = value.clamp(1, 64) as u8;
    controls::well(ui, rect);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("{length}"),
        FontId::proportional(13.0),
        roles().legend,
    );
    focus(ui, &response, response.has_focus(), roles().legend);
    response.widget_info(|| egui::WidgetInfo::slider(true, f64::from(*length), "Length"));
    name_control(response, "Length").hover_hint(
        "The steps the pattern repeats over, from 1 to 64. Drag the end marker on the step \
         ruler, or drag or scroll here. Steps after the length keep their notes.",
    );
}

/// The step ruler: step numbers, the end marker, the playing step, and the
/// selected range, which a drag along it sets.
fn step_ruler(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    columns: Columns,
    editor: &mut Editor,
    playing: Option<usize>,
) {
    let length = usize::from(state.length);
    let area = Rect::from_min_max(
        Pos2::new(columns.left, rect.top()),
        Pos2::new(columns.x(columns.first + columns.count), rect.bottom()),
    );
    let response = ui.interact(area, ui.id().with("step-ruler"), Sense::click_and_drag());
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }
    if response.has_focus() {
        step_keys(ui, &response, columns, &mut editor.step);
        let step = editor.step;
        ui.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
            {
                editor.selection = Some((step, step));
            }
            for (key, next) in [
                (
                    egui::Key::ArrowLeft,
                    step.saturating_sub(1).max(columns.first),
                ),
                (
                    egui::Key::ArrowRight,
                    (step + 1).min(columns.first + columns.count - 1),
                ),
            ] {
                if input.consume_key(egui::Modifiers::SHIFT, key) {
                    let anchor = editor
                        .selection
                        .map_or(step, |(from, to)| if step == from { to } else { from });
                    editor.step = next;
                    editor.selection = Some((anchor.min(next), anchor.max(next)));
                }
            }
        });
    }
    let anchor_id = response.id.with("anchor");
    if response.drag_started()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let step = columns.at(pointer.x);
        editor.step = step;
        ui.ctx().data_mut(|data| data.insert_temp(anchor_id, step));
        editor.selection = Some((step, step));
    }
    if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
        && let Some(anchor) = ui.ctx().data(|data| data.get_temp::<usize>(anchor_id))
    {
        let step = columns.at(pointer.x);
        editor.step = step;
        editor.selection = Some((anchor.min(step), anchor.max(step)));
    }
    if response.clicked() && response.interact_pointer_pos().is_some() {
        editor.selection = None;
    }
    if ui.input(|input| input.key_pressed(egui::Key::Escape)) && editor.selection.is_some() {
        editor.selection = None;
    }
    name_region(response, "Step ruler").hover_hint(
        "Drag to select steps; lane edits then apply to all of them. Click to clear the selection.",
    );
    for step in columns.first..columns.first + columns.count {
        let cell = columns
            .span(step, step + 1, rect.top(), rect.bottom())
            .shrink2(Vec2::new(1.0, 0.0));
        let selected = editor
            .selection
            .is_some_and(|(from, to)| (from..=to).contains(&step));
        if selected {
            ui.painter().rect_filled(
                cell,
                2.0,
                controls::Palette::Olive.accent().gamma_multiply(0.45),
            );
        }
        if playing == Some(step) {
            ui.painter().rect_filled(cell, 2.0, roles().coral);
        }
        let beat = step % 4 == 0;
        let color = if step >= length {
            roles().legend.gamma_multiply(0.35)
        } else if beat {
            roles().legend
        } else {
            roles().legend.gamma_multiply(0.7)
        };
        lettering(ui, cell.center(), &format!("{}", step + 1), 11.0, color);
    }
    // The end marker sits on the boundary after the last step.
    if (columns.first..=columns.first + columns.count).contains(&length) {
        let x = columns.x(length);
        let handle = Rect::from_center_size(
            Pos2::new(x, rect.center().y),
            Vec2::new(10.0, rect.height()),
        );
        let marker = ui.interact(handle, ui.id().with("length-end"), Sense::DRAG);
        if marker.dragged()
            && let Some(pointer) = marker.interact_pointer_pos()
        {
            state.length = columns.boundary(pointer.x).clamp(1, STEPS) as u8;
        }
        let color = if marker.hovered() || marker.dragged() {
            roles().legend
        } else {
            roles().coral
        };
        ui.painter().rect_filled(
            Rect::from_center_size(Pos2::new(x, rect.center().y), Vec2::new(4.0, rect.height())),
            2.0,
            color,
        );
        marker
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
            .hover_hint("Drag to set the pattern length.");
    }
}

/// The pitch grid: 25 rows of the octave in view over the steps in view. A
/// tie draws as one joined bar, an accent as a bright top edge, and a slide
/// as a line to the next note. Notes outside the rows mark the grid's edge.
fn note_grid(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    view_bar: &mut usize,
    columns: Columns,
    editor: &mut Editor,
) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    form_style(&mut child);
    scroll_style(&mut child);
    let top_note = ROWS + 12 * editor.octave;
    let bottom_note = -ROWS + 12 * editor.octave;
    let length = usize::from(state.length);
    let mut octave_change = 0;
    let mut bar_change = 0_i32;
    egui::ScrollArea::vertical()
        .id_salt("step-synth-notes")
        .auto_shrink([false, false])
        .max_height(rect.height())
        .show(&mut child, |ui| {
            let (content, _) = ui.allocate_exact_size(
                Vec2::new(rect.width() - 10.0, f32::from(2 * ROWS as u8 + 1) * ROW),
                Sense::hover(),
            );
            let row_top = |note: i8| content.top() + f32::from((top_note - note) as u8) * ROW;
            // The grid is one Tab stop. It holds keyboard focus and moves a
            // cursor among its cells, which stay pointer targets.
            let cell_id = |note: i8, step: usize| ui.id().with(("note", note, step));
            let cursor_id = ui.id().with("note-cursor");
            let mut cursor = ui
                .ctx()
                .data(|data| data.get_temp::<(i8, usize)>(cursor_id))
                .unwrap_or((12 * editor.octave, columns.first));
            cursor.1 = editor.step;
            cursor.0 = cursor.0.clamp(bottom_note, top_note);
            cursor.1 = cursor
                .1
                .clamp(columns.first, columns.first + columns.count - 1);
            let before = cursor;
            let grid = ui.interact(content, ui.id().with("pitch-grid"), Sense::FOCUSABLE);
            // Assistive technology can focus any cell directly.
            let requested = ui.input(|input| {
                use egui::accesskit::Action;
                let any = input.events.iter().any(|event| {
                    matches!(event, egui::Event::AccessKitActionRequest(request)
                        if request.action == Action::Focus)
                });
                any.then(|| {
                    (bottom_note..=top_note)
                        .flat_map(|note| {
                            (columns.first..columns.first + columns.count)
                                .map(move |step| (note, step))
                        })
                        .find(|(note, step)| {
                            input.has_accesskit_action_request(cell_id(*note, *step), Action::Focus)
                        })
                })
                .flatten()
            });
            if let Some(target) = requested {
                cursor = target;
                grid.request_focus();
            }
            let mut toggle = false;
            if grid.has_focus() {
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        grid.id,
                        egui::EventFilter {
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            ..Default::default()
                        },
                    )
                });
                let last = columns.first + columns.count - 1;
                ui.input_mut(|input| {
                    let mut pressed = |key| input.consume_key(egui::Modifiers::NONE, key);
                    if pressed(egui::Key::ArrowUp) {
                        cursor.0 = (cursor.0 + 1).min(top_note);
                    }
                    if pressed(egui::Key::ArrowDown) {
                        cursor.0 = (cursor.0 - 1).max(bottom_note);
                    }
                    if pressed(egui::Key::ArrowLeft) {
                        cursor.1 = cursor.1.saturating_sub(1).max(columns.first);
                    }
                    if pressed(egui::Key::ArrowRight) {
                        cursor.1 = (cursor.1 + 1).min(last);
                    }
                    if pressed(egui::Key::Home) {
                        cursor.1 = columns.first;
                    }
                    if pressed(egui::Key::End) {
                        cursor.1 = last;
                    }
                    if pressed(egui::Key::PageUp) {
                        octave_change = 1;
                    }
                    if pressed(egui::Key::PageDown) {
                        octave_change = -1;
                    }
                    toggle = pressed(egui::Key::Enter) | pressed(egui::Key::Space);
                    if input.consume_key(egui::Modifiers::COMMAND, egui::Key::ArrowLeft) {
                        bar_change = -1;
                    }
                    if input.consume_key(egui::Modifiers::COMMAND, egui::Key::ArrowRight) {
                        bar_change = 1;
                    }
                });
            }
            let reveal = requested.is_some()
                || (grid.has_focus() && (cursor != before || grid.gained_focus()));
            grid.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Other,
                    true,
                    format!(
                        "Pitch grid: step {}, {:+} semitones",
                        cursor.1 + 1,
                        cursor.0
                    ),
                )
            });
            let painter = ui.painter().clone();
            let covers = covers(state);
            let modifiers = ui.input(|input| input.modifiers);
            let drag_id = ui.id().with("tie-drag");
            let mut edit = None;
            for note in (bottom_note..=top_note).rev() {
                let y = row_top(note);
                let in_scale = editor.scale.contains(editor.root, note);
                painter.text(
                    Pos2::new(content.left() + GUTTER - 8.0, y + ROW / 2.0),
                    Align2::RIGHT_CENTER,
                    if note == 0 {
                        "A  0".into()
                    } else {
                        format!("{note:+}")
                    },
                    FontId::proportional(12.0),
                    if in_scale {
                        roles().legend
                    } else {
                        roles().legend.gamma_multiply(0.6)
                    },
                );
                let view = columns.first..columns.first + columns.count;
                for (step, cover) in covers.iter().enumerate().take(view.end).skip(view.start) {
                    let cell = columns
                        .span(step, step + 1, y + 1.0, y + 1.0 + CELL_HEIGHT)
                        .shrink2(Vec2::new(1.0, 0.0));
                    let held = cover.filter(|(_, pitch)| *pitch == note);
                    let selected = held.is_some();
                    let response =
                        ui.interact(cell, cell_id(note, step), Sense::CLICK | Sense::DRAG);
                    if response.clicked() {
                        cursor = (note, step);
                        grid.request_focus();
                    }
                    let current = grid.has_focus() && cursor == (note, step);
                    let fill = if response.hovered() && held.is_none() {
                        roles().cell_hover
                    } else if in_scale {
                        roles().face
                    } else {
                        roles().cell_alternate
                    };
                    painter.rect_filled(cell, 2.0, fill);
                    painter.rect_stroke(
                        cell,
                        2.0,
                        Stroke::new(1.0_f32, roles().cell_edge),
                        egui::StrokeKind::Inside,
                    );
                    let label = format!("Step {}, {note:+} semitones", step + 1);
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            response.enabled(),
                            selected,
                            &label,
                        )
                    });
                    ui.ctx().accesskit_node_builder(response.id, |node| {
                        node.add_action(egui::accesskit::Action::Focus)
                    });
                    focus(ui, &response, current, roles().legend);
                    if cursor == (note, step) && reveal {
                        ui.scroll_to_rect(cell, None);
                    }
                    // A drag that starts on a note's last step ties it on.
                    if response.drag_started()
                        && let Some((head, _)) = held
                        && note_end(&state.steps, head) == step
                    {
                        ui.ctx().data_mut(|data| {
                            data.insert_temp(drag_id, (head, state.steps.clone()))
                        });
                    }
                    if response.dragged()
                        && let Some(pointer) = response.interact_pointer_pos()
                        && let Some((head, snapshot)) = ui
                            .ctx()
                            .data(|data| data.get_temp::<(usize, Vec<Step>)>(drag_id))
                    {
                        let end = columns.at(pointer.x).max(head);
                        edit = Some(GridEdit::Tie(snapshot, head, end));
                    }
                    if response.drag_stopped() {
                        ui.ctx()
                            .data_mut(|data| data.remove::<(usize, Vec<Step>)>(drag_id));
                    }
                    let clicked = response.hover_hint(label).clicked() || (current && toggle);
                    if clicked {
                        edit = Some(match held {
                            Some((head, _)) if modifiers.alt => GridEdit::Slide(head),
                            Some((head, _)) if modifiers.shift => GridEdit::Accent(head),
                            Some((head, _)) => GridEdit::Clear(head),
                            None => GridEdit::Place(step, note),
                        });
                    }
                }
            }
            // Bar and beat lines, then the notes over the cells.
            for step in (columns.first..=columns.first + columns.count).step_by(4) {
                let x = columns.x(step);
                painter.line_segment(
                    [Pos2::new(x, content.top()), Pos2::new(x, content.bottom())],
                    Stroke::new(
                        if step % BAR == 0 { 1.5 } else { 1.0_f32 },
                        roles().grid_line,
                    ),
                );
            }
            for (head, step) in state.steps.iter().enumerate() {
                let Trig::Note(note) = step.trig else {
                    continue;
                };
                let end = note_end(&state.steps, head);
                if end < columns.first || head >= columns.first + columns.count {
                    continue;
                }
                let from = head.max(columns.first);
                let to = (end + 1).min(columns.first + columns.count);
                let past = head >= length;
                if !(bottom_note..=top_note).contains(&note) {
                    // A note outside the rows marks the grid's top or bottom.
                    if columns.contains(head) {
                        let x = columns.x(head) + columns.width / 2.0;
                        let edge = if note > top_note {
                            ui.clip_rect().top().max(content.top())
                        } else {
                            ui.clip_rect().bottom().min(content.bottom()) - 4.0
                        };
                        painter.rect_filled(
                            Rect::from_min_size(Pos2::new(x - 6.0, edge), Vec2::new(12.0, 4.0)),
                            1.0,
                            roles().coral,
                        );
                    }
                    continue;
                }
                let y = row_top(note);
                let bar = columns
                    .span(from, to, y + 1.0, y + 1.0 + CELL_HEIGHT)
                    .shrink2(Vec2::new(1.0, 0.0));
                let color = if past {
                    roles().coral.gamma_multiply(0.45)
                } else {
                    roles().coral
                };
                painter.rect_filled(bar, 3.0, color);
                painter.rect_stroke(
                    bar,
                    3.0,
                    Stroke::new(1.0_f32, roles().coral.lerp_to_gamma(roles().legend, 0.4)),
                    egui::StrokeKind::Inside,
                );
                if step.accent {
                    painter.rect_filled(
                        Rect::from_min_size(bar.min, Vec2::new(bar.width(), 3.0)),
                        egui::CornerRadius {
                            nw: 3,
                            ne: 3,
                            sw: 0,
                            se: 0,
                        },
                        roles().legend,
                    );
                }
                if !editor.scale.contains(editor.root, note) && columns.contains(head) {
                    // An off-scale note keeps its cell, with a notched corner.
                    let corner = bar.right_top();
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            corner + Vec2::new(-6.0, 1.0),
                            corner + Vec2::new(-1.0, 1.0),
                            corner + Vec2::new(-1.0, 6.0),
                        ],
                        roles().ink,
                        Stroke::NONE,
                    ));
                }
                if step.slide
                    && let Some(Step {
                        trig: Trig::Note(next),
                        ..
                    }) = state.steps.get(end + 1)
                    && columns.contains(end + 1)
                    && (bottom_note..=top_note).contains(next)
                {
                    let from = Pos2::new(bar.right() - 4.0, y + 1.0 + CELL_HEIGHT / 2.0);
                    let to = Pos2::new(
                        columns.x(end + 1) + 4.0,
                        row_top(*next) + 1.0 + CELL_HEIGHT / 2.0,
                    );
                    painter.line_segment([from, to], Stroke::new(2.5_f32, roles().legend));
                }
            }
            // Steps past the length are dimmed.
            if length < columns.first + columns.count {
                let from = length.max(columns.first);
                painter.rect_filled(
                    columns.span(
                        from,
                        columns.first + columns.count,
                        content.top(),
                        content.bottom(),
                    ),
                    0.0,
                    roles().ink.gamma_multiply(0.35),
                );
                if columns.contains(length) || length == columns.first + columns.count {
                    let x = columns.x(length);
                    painter.line_segment(
                        [Pos2::new(x, content.top()), Pos2::new(x, content.bottom())],
                        Stroke::new(2.0_f32, roles().coral),
                    );
                }
            }
            if let Some((from, to)) = editor.selection {
                let from = from.max(columns.first);
                let to = (to + 1).min(columns.first + columns.count);
                if from < to {
                    painter.rect_filled(
                        columns.span(from, to, content.top(), content.bottom()),
                        0.0,
                        controls::Palette::Olive.accent().gamma_multiply(0.12),
                    );
                }
            }
            match edit {
                Some(GridEdit::Place(step, note)) => state.steps[step].trig = Trig::Note(note),
                Some(GridEdit::Clear(head)) => clear_note(state, head),
                Some(GridEdit::Accent(head)) => state.steps[head].accent ^= true,
                Some(GridEdit::Slide(head)) => state.steps[head].slide ^= true,
                Some(GridEdit::Tie(snapshot, head, end)) => tie_to(state, &snapshot, head, end),
                None => {}
            }
            ui.ctx()
                .data_mut(|data| data.insert_temp(cursor_id, cursor));
            editor.step = cursor.1;
        });
    if octave_change != 0 {
        editor.octave = (editor.octave + octave_change).clamp(-3, 3);
    }
    if bar_change != 0 {
        let bars = columns.count / BAR;
        let bar = *view_bar as i32 + bar_change * bars as i32;
        *view_bar = bar.clamp(0, 3) as usize;
    }
}

/// One pitch-grid edit, applied after the grid is drawn.
enum GridEdit {
    /// Place a note at a step.
    Place(usize, i8),
    /// Clear the note that starts at a step, with its ties.
    Clear(usize),
    Accent(usize),
    Slide(usize),
    /// Tie a note through a step, from the steps the drag began with.
    Tie(Vec<Step>, usize, usize),
}

/// Tie the note at `head` through `end`, from the steps as they were when the
/// drag began. The tie replaces the notes it covers, and their remaining ties
/// become rests.
fn tie_to(state: &mut StepSynthState, snapshot: &[Step], head: usize, end: usize) {
    state.steps.copy_from_slice(snapshot);
    let old_end = note_end(snapshot, head);
    for step in &mut state.steps[head + 1..=old_end] {
        step.trig = Trig::Rest;
    }
    for step in &mut state.steps[head + 1..=end] {
        step.trig = Trig::Tie;
    }
    let mut after = end + 1;
    while after < STEPS && state.steps[after].trig == Trig::Tie {
        state.steps[after].trig = Trig::Rest;
        after += 1;
    }
}

/// Which value lane a row of the lanes edits.
#[derive(Clone, Copy, PartialEq)]
enum Lane {
    Accent,
    Slide,
    Velocity,
}

/// Move the shared step cursor without leaving a focused grid control.
fn step_keys(ui: &mut egui::Ui, response: &Response, columns: Columns, step: &mut usize) {
    *step = (*step).clamp(columns.first, columns.first + columns.count - 1);
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
    ui.input_mut(|input| {
        if !input.modifiers.shift {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                *step = step.saturating_sub(1).max(columns.first);
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                *step = (*step + 1).min(columns.first + columns.count - 1);
            }
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
            *step = columns.first;
        }
        if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
            *step = columns.first + columns.count - 1;
        }
    });
}

/// The Accent and Slide flag rows and the Velocity bars. A click toggles a
/// flag and a drag paints it; a drag on Velocity draws the level. In a
/// selection, a flag edit sets every selected note, and a level edit shifts
/// each by the same amount.
fn value_lanes(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut StepSynthState,
    columns: Columns,
    editor: &mut Editor,
) {
    // A short panel gets slimmer flag rows.
    let flag = if rect.height() < LANES_HEIGHT {
        18.0
    } else {
        22.0
    };
    let rows = [
        (Lane::Accent, "Accent", rect.top(), flag),
        (Lane::Slide, "Slide", rect.top() + flag + 2.0, flag),
        (
            Lane::Velocity,
            "Velocity",
            rect.top() + 2.0 * flag + 4.0,
            rect.height() - 2.0 * flag - 4.0,
        ),
    ];
    let length = usize::from(state.length);
    let selected = |step: usize| {
        editor
            .selection
            .is_some_and(|(from, to)| (from..=to).contains(&step))
    };
    for (lane, name, top, height) in rows {
        let row = Rect::from_min_max(
            Pos2::new(columns.left, top),
            Pos2::new(columns.x(columns.first + columns.count), top + height),
        );
        controls::well(ui, row);
        let notes: Vec<usize> = (0..STEPS)
            .filter(|step| matches!(state.steps[*step].trig, Trig::Note(_)))
            .collect();
        let value = |step: &Step| match lane {
            Lane::Accent => u8::from(step.accent),
            Lane::Slide => u8::from(step.slide),
            Lane::Velocity => step.velocity,
        };
        let mixed = {
            let mut values = notes
                .iter()
                .filter(|step| selected(**step))
                .map(|step| value(&state.steps[*step]));
            values
                .next()
                .is_some_and(|first| values.any(|other| other != first))
        };
        left_text(
            ui,
            Pos2::new(rect.left() + 2.0, top + 11.0),
            name,
            11.0,
            roles().legend,
        );
        if mixed {
            // Selected notes differ here; an edit shifts them together.
            ui.painter().circle_filled(
                Pos2::new(columns.left - 6.0, top + 11.0),
                3.0,
                roles().coral,
            );
        }
        let response = ui.interact(row, ui.id().with(("lane", name)), Sense::click_and_drag());
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        if response.has_focus() {
            step_keys(ui, &response, columns, &mut editor.step);
        }
        let step = editor.step;
        let current = value(&state.steps[step]);
        let mut draft = i32::from(current);
        if response.has_focus() {
            ui.input_mut(|input| {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    draft += 1;
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    draft -= 1;
                }
                if lane != Lane::Velocity
                    && (input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
                        || input.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
                {
                    draft = i32::from(current == 0);
                }
            });
        }
        ui.input(|input| {
            use egui::accesskit::{Action, ActionData};
            draft += input.num_accesskit_action_requests(response.id, Action::Increment) as i32
                - input.num_accesskit_action_requests(response.id, Action::Decrement) as i32;
            if lane != Lane::Velocity
                && input.has_accesskit_action_request(response.id, Action::Click)
            {
                draft = i32::from(current == 0);
            }
            for request in input.accesskit_action_requests(response.id, Action::SetValue) {
                if let Some(ActionData::NumericValue(value)) = request.data
                    && value.is_finite()
                {
                    draft = value.round() as i32;
                }
            }
        });
        draft = draft.clamp(
            if lane == Lane::Velocity { 1 } else { 0 },
            if lane == Lane::Velocity { 100 } else { 1 },
        );
        if draft != i32::from(current) && matches!(state.steps[step].trig, Trig::Note(_)) {
            for target in notes.iter().copied().filter(|target| {
                if selected(step) {
                    selected(*target)
                } else {
                    *target == step
                }
            }) {
                let target = &mut state.steps[target];
                match lane {
                    Lane::Accent => target.accent = draft != 0,
                    Lane::Slide => target.slide = draft != 0,
                    Lane::Velocity => {
                        target.velocity = (i32::from(target.velocity) + draft - i32::from(current))
                            .clamp(1, 100) as u8
                    }
                }
            }
        }
        response.widget_info(|| match lane {
            Lane::Velocity => egui::WidgetInfo::slider(
                true,
                f64::from(value(&state.steps[step])),
                format!("{name} lane"),
            ),
            _ => egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                true,
                value(&state.steps[step]) != 0,
                format!("{name} lane"),
            ),
        });
        focus(ui, &response, response.has_focus(), roles().legend);
        let paint_id = response.id.with("paint");
        if let Some(pointer) = response.interact_pointer_pos()
            && (response.is_pointer_button_down_on() || response.clicked())
        {
            let step = columns.at(pointer.x);
            editor.step = step;
            let is_note = matches!(state.steps[step].trig, Trig::Note(_));
            let targets: Vec<usize> = if selected(step) {
                notes
                    .iter()
                    .copied()
                    .filter(|step| selected(*step))
                    .collect()
            } else {
                vec![step]
            };
            match lane {
                Lane::Accent | Lane::Slide => {
                    let paint = ui.ctx().data(|data| data.get_temp::<bool>(paint_id));
                    let paint = match paint {
                        Some(paint) => Some(paint),
                        None if is_note => {
                            let next = value(&state.steps[step]) == 0;
                            ui.ctx().data_mut(|data| data.insert_temp(paint_id, next));
                            Some(next)
                        }
                        None => None,
                    };
                    if let Some(paint) = paint {
                        for target in targets {
                            if !matches!(state.steps[target].trig, Trig::Note(_)) {
                                continue;
                            }
                            let step = &mut state.steps[target];
                            if lane == Lane::Accent {
                                step.accent = paint;
                            } else {
                                step.slide = paint;
                            }
                        }
                    }
                }
                Lane::Velocity if is_note => {
                    let level = ((row.bottom() - 2.0 - pointer.y) / (row.height() - 4.0) * 100.0)
                        .round()
                        .clamp(1.0, 100.0) as i32;
                    let delta = level - i32::from(state.steps[step].velocity);
                    for target in targets {
                        let step = &mut state.steps[target];
                        step.velocity = (i32::from(step.velocity) + delta).clamp(1, 100) as u8;
                    }
                }
                Lane::Velocity => {}
            }
        }
        if !response.is_pointer_button_down_on() {
            ui.ctx().data_mut(|data| data.remove::<bool>(paint_id));
        }
        response.hover_hint(match lane {
            Lane::Accent => "Click a note's dot to accent it, or drag along the row to paint.",
            Lane::Slide => {
                "Click a note's dot to slide it into the next note, or drag along the row to paint."
            }
            Lane::Velocity => "Drag up or down to set each note's level.",
        });
        for step in columns.first..columns.first + columns.count {
            let cell = columns
                .span(step, step + 1, row.top(), row.bottom())
                .shrink2(Vec2::new(2.0, 2.0));
            if selected(step) {
                ui.painter().rect_filled(
                    cell.expand2(Vec2::new(1.0, 1.0)),
                    1.0,
                    controls::Palette::Olive.accent().gamma_multiply(0.3),
                );
            }
            let note = state.steps[step];
            let is_note = matches!(note.trig, Trig::Note(_));
            let alpha = if step >= length { 0.45 } else { 1.0 };
            match lane {
                Lane::Accent | Lane::Slide => {
                    let on = if lane == Lane::Accent {
                        note.accent
                    } else {
                        note.slide
                    };
                    if is_note && on {
                        ui.painter().circle_filled(
                            cell.center(),
                            5.0,
                            roles().coral.gamma_multiply(alpha),
                        );
                    } else if is_note {
                        ui.painter().circle_stroke(
                            cell.center(),
                            4.5,
                            Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.6 * alpha)),
                        );
                    } else {
                        ui.painter().circle_filled(
                            cell.center(),
                            1.5,
                            roles().legend.gamma_multiply(0.25),
                        );
                    }
                }
                Lane::Velocity if is_note => {
                    let level = f32::from(note.velocity) / 100.0;
                    let bar = Rect::from_min_max(
                        Pos2::new(cell.left() + 2.0, cell.bottom() - cell.height() * level),
                        Pos2::new(cell.right() - 2.0, cell.bottom()),
                    );
                    ui.painter().rect_filled(
                        bar,
                        1.0,
                        controls::Palette::Olive.accent().gamma_multiply(alpha),
                    );
                }
                Lane::Velocity => {}
            }
        }
    }
}

/// Write a new line over the pattern's length: notes in the scale near the
/// root of the octave in view, with ties, rests, accents, and slides.
fn randomise(state: &mut StepSynthState, editor: &mut Editor) {
    let mut seed = editor.seed
        ^ std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |time| time.as_nanos() as u64);
    let mut next = move || {
        // xorshift64*: enough to vary a line, never for anything else.
        seed ^= seed >> 12;
        seed ^= seed << 25;
        seed ^= seed >> 27;
        seed = seed.max(1);
        (seed.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    };
    let centre = 12 * editor.octave + editor.root as i8;
    let pool: Vec<i8> = (centre - 7..=centre + 12)
        .filter(|note| (-48..=48).contains(note) && editor.scale.contains(editor.root, *note))
        .collect();
    let mut sounding = false;
    for index in 0..usize::from(state.length) {
        let roll = next();
        let step = &mut state.steps[index];
        *step = Step::default();
        if index == 0 || roll < 0.45 {
            let pick = (next() * pool.len() as f64) as usize;
            step.trig = Trig::Note(pool.get(pick).copied().unwrap_or(centre));
            step.accent = next() < 0.2;
            step.slide = next() < 0.12;
            step.velocity = 70 + (next() * 31.0) as u8;
            sounding = true;
        } else if sounding && roll < 0.62 {
            step.trig = Trig::Tie;
        } else {
            sounding = false;
        }
    }
    editor.seed = next().to_bits();
    editor.selection = None;
}

/// The board's status line and, for missing audio, Import details. They sit
/// in the panel's top row, as the Mastering status does, `width` points wide.
pub(super) fn toolbar(
    ui: &mut egui::Ui,
    width: f32,
    source: Source<'_>,
    feedback: Feedback<'_>,
    show_effects: bool,
) -> Action {
    let missing = source.missing && !source.explicit_file;
    let status = feedback.message.unwrap_or(if missing {
        "Missing audio. Choose a source file or open Import details."
    } else if !source.available {
        "Choose a source file or select a sound in the palette."
    } else if show_effects {
        "Adjust sequence effects. Note grid returns to the notes."
    } else {
        "Click places or clears a note. Drag a note's end to hold it."
    });
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width.max(0.0), 28.0), Sense::hover());
    let mut strip = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    dialog::hardware_style(&mut strip, controls::Palette::Olive);
    status_footer(&mut strip, rect, status, feedback.error, missing);
    let details = Rect::from_min_size(
        Pos2::new(rect.right() - 130.0, rect.top() + 1.0),
        Vec2::new(128.0, 26.0),
    );
    if missing && button(&mut strip, details, "Import details", true, false).clicked() {
        Action::Details
    } else {
        Action::None
    }
}

/// A dial with its name above and its value below.
struct Dial<'a> {
    name: &'a str,
    low: f64,
    high: f64,
    reset: f64,
    step: f64,
    decimals: usize,
    logarithmic: bool,
}
fn dial(ui: &mut egui::Ui, positions: [Pos2; 3], value: &mut f64, spec: Dial<'_>, hint: &str) {
    let [label, center, well] = positions;
    ui.painter().text(
        label,
        Align2::CENTER_CENTER,
        spec.name,
        FontId::proportional(12.0),
        roles().legend,
    );
    controls::rotary(
        ui,
        Rect::from_center_size(center, Vec2::splat(44.0)),
        value,
        controls::RotarySpec {
            name: spec.name,
            low: spec.low,
            high: spec.high,
            reset: spec.reset,
            step: spec.step,
            decimals: spec.decimals,
            logarithmic: spec.logarithmic,
            choices: &[],
        },
    )
    .hover_hint(hint);
    controls::well(ui, Rect::from_center_size(well, Vec2::new(60.0, 20.0)));
    ui.scope(|ui| {
        controls::value_style(ui);
        let response = ui.put(
            Rect::from_center_size(well, Vec2::new(58.0, 22.0)),
            egui::DragValue::new(value)
                .range(spec.low..=spec.high)
                .speed(spec.step)
                .fixed_decimals(spec.decimals),
        );
        name_control(response, &format!("{} value", spec.name))
            .hover_hint(format!("{}: drag, or click to enter a value.", spec.name));
    });
}
fn scroll_style(ui: &mut egui::Ui) {
    let scroll = &mut ui.style_mut().spacing.scroll;
    scroll.floating = false;
    scroll.bar_width = 6.0;
    scroll.bar_inner_margin = 4.0;
    scroll.bar_outer_margin = 0.0;
}
fn form_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.override_font_id = Some(FontId::proportional(13.0));
    style.spacing.interact_size.y = 26.0;
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.slider_width = 180.0;
    style.visuals.override_text_color = Some(roles().legend);
    style.visuals.panel_fill = roles().face;
    style.visuals.extreme_bg_color = roles().face;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, roles().legend);
    for widgets in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widgets.bg_fill = roles().face;
        widgets.weak_bg_fill = roles().face;
        widgets.bg_stroke = Stroke::new(1.0_f32, roles().tool_edge);
        widgets.fg_stroke = Stroke::new(1.0_f32, roles().legend);
    }
}
fn toggle(ui: &mut egui::Ui, center: Pos2, label_at: Pos2, name: &str, value: &mut bool) {
    let hit = Rect::from_min_size(
        center - Vec2::new(14.0, 13.0),
        Vec2::new((label_at.x - center.x + 90.0).max(110.0), 26.0),
    );
    let response = controls::toggle_at(ui, hit, name, name, value);
    let status = format!("{name}: {}", if *value { "on" } else { "off" });
    let tooltip = match name {
        "Loop source" => format!(
            "{status}\n\n\
             On: Repeats the entire source audio from its beginning whenever it reaches the end, \
             for as long as the current note is allowed to play. It does not use embedded loop points.\n\n\
             Off: Plays the source once per note. If it ends before the next note, the remaining interval has no source audio.\n\n\
             Every new note restarts the source, unless the note before it slides into it. \
             Repetition lasts through the note's ties and its gate. \
             Loop source does not repeat the pattern or extend the sequence."
        ),
        _ => status,
    };
    response.hover_hint(tooltip);
}
fn button(ui: &mut egui::Ui, rect: Rect, text: &str, enabled: bool, primary: bool) -> Response {
    keyed_button(ui, rect, text, text, enabled, primary)
}
fn keyed_button(
    ui: &mut egui::Ui,
    rect: Rect,
    key: &str,
    text: &str,
    enabled: bool,
    primary: bool,
) -> Response {
    controls::button_at(
        ui,
        rect,
        key,
        text,
        if primary {
            controls::ButtonRole::Primary
        } else {
            controls::ButtonRole::Secondary
        },
        enabled,
    )
}
fn select_button(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    name: &str,
    selected: bool,
) -> Response {
    let response = controls::button_at(
        ui,
        rect,
        name,
        text,
        if selected {
            controls::ButtonRole::Primary
        } else {
            controls::ButtonRole::Secondary
        },
        true,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            response.enabled(),
            selected,
            name,
        )
    });
    response.hover_hint(name)
}
fn lettering(ui: &egui::Ui, center: Pos2, text: &str, size: f32, color: Color32) {
    let font = FontId::proportional(size);
    let galley = ui.painter().layout_no_wrap(text.into(), font, color);
    ui.painter().galley(
        center - galley.mesh_bounds.center().to_vec2(),
        galley,
        color,
    );
}

fn left_text(ui: &egui::Ui, at: Pos2, text: &str, size: f32, color: Color32) {
    ui.painter().text(
        at,
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(size),
        color,
    );
}

fn focus(ui: &egui::Ui, response: &Response, focused: bool, color: Color32) {
    if focused || (response.hovered() && response.enabled()) {
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            2.0,
            Stroke::new(if focused { 2.0 } else { 1.0_f32 }, color),
            egui::StrokeKind::Inside,
        );
    }
}

fn status_footer(ui: &mut egui::Ui, rect: Rect, text: &str, error: bool, missing: bool) {
    controls::well(ui, rect);
    let response = ui.interact(rect, ui.id().with("status"), Sense::hover());
    named(&response, egui::WidgetType::Panel, "Step synth status");
    let text_rect = Rect::from_min_max(
        rect.min + Vec2::new(9.0, 5.0),
        rect.max - Vec2::new(if missing { 144.0 } else { 9.0 }, 5.0),
    );
    controls::status_text(ui, text_rect, "step-synth-status", text, 12.5, error, text);
}
