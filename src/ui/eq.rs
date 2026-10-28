//! The channel EQ window of one track: its power switch, Reset, the
//! response curve of its settings, and a dial for each control, in signal
//! order from the low cut to the high cut. The window edits a copy of the
//! track's EQ; the caller turns a changed copy into a mixer edit.
use super::controls::CREAM;
use super::master::{Dial, Format, decimal, dial};
use super::*;
use mixjam_techno::application::eq::{ChannelEq, EqParam};

/// What the window changed. Turns of one dial continue one undo step; the
/// power switch and Reset are steps of their own.
enum Action {
    None,
    Turn(ChannelEq),
    Switch(ChannelEq),
}

/// The content's width, the response curve's height, and the pitch of the
/// dial columns and rows.
const WIDTH: f32 = 440.0;
const CURVE_HEIGHT: f32 = 132.0;
const COLUMN: f32 = 86.0;
const ROW: f32 = 92.0;
/// Where the first row of dials starts below the curve.
const DIALS_TOP: f32 = 36.0 + CURVE_HEIGHT + 10.0;
/// The content's height: the controls row, the curve, the column headings,
/// and three rows of dials.
const HEIGHT: f32 = DIALS_TOP + 48.0 + 2.0 * ROW + 70.0;
/// The curve spans 20 Hz to 20 kHz and this many decibels each way.
const CURVE_DB: f64 = 24.0;

/// Each band's heading and its controls from top to bottom.
const COLUMNS: [(&str, &[EqParam]); 5] = [
    ("Low cut", &[EqParam::LowCut]),
    ("Low shelf", &[EqParam::LowHz, EqParam::LowDb]),
    ("Mid peak", &[EqParam::MidHz, EqParam::MidDb, EqParam::MidQ]),
    ("High shelf", &[EqParam::HighHz, EqParam::HighDb]),
    ("High cut", &[EqParam::HighCut]),
];

/// The printed label, description, and value format of a control.
fn face(param: EqParam) -> (&'static str, &'static str, Format) {
    match param {
        EqParam::LowCut => (
            "Cut",
            "A 12 dB per octave high-pass that removes rumble. At 20 Hz it is off.",
            Format::Hertz,
        ),
        EqParam::LowHz => ("Freq", "Where the low shelf turns.", Format::Hertz),
        EqParam::LowDb => (
            "Gain",
            "Raises or lowers the sound below the low shelf's frequency.",
            Format::db(true),
        ),
        EqParam::MidHz => ("Freq", "The centre of the peak.", Format::Hertz),
        EqParam::MidDb => (
            "Gain",
            "Raises or lowers the band around the peak's frequency.",
            Format::db(true),
        ),
        EqParam::MidQ => (
            "Q",
            "The width of the peak: a higher Q is narrower.",
            Format::tenths(""),
        ),
        EqParam::HighHz => ("Freq", "Where the high shelf turns.", Format::Hertz),
        EqParam::HighDb => (
            "Gain",
            "Raises or lowers the sound above the high shelf's frequency. Lower it to tame \
             harsh hi-hats.",
            Format::db(true),
        ),
        EqParam::HighCut => (
            "Cut",
            "A 12 dB per octave low-pass that darkens the top. At 20 kHz it is off.",
            Format::Hertz,
        ),
    }
}

impl MixJamApp {
    /// The EQ window of the track whose EQ button was pressed last. Its
    /// edits apply while the song plays and are undo steps.
    pub(super) fn eq_window(&mut self, ctx: &egui::Context) {
        let Some(track) = self.eq_track else {
            return;
        };
        let title = format!("Track {} EQ", track + 1);
        let eq = self.session.project().tracks[track].eq;
        // A replaced project starts new dial gestures.
        let generation = self.master_view_generation;
        let mut open = true;
        let mut action = Action::None;
        // The shell's margins and bay take 36 by 12 points of the size.
        dialog::window(&title)
            .open(&mut open)
            .default_width(WIDTH + 40.0)
            .default_height(HEIGHT + 16.0)
            .max_height((ctx.viewport_rect().height() - 40.0).max(240.0))
            .constrain_to(ctx.viewport_rect().shrink(12.0))
            .show(ctx, |ui| {
                action = ui.push_id(generation, |ui| show(ui, &title, &eq)).inner;
            });
        if !open {
            self.eq_track = None;
        }
        let (changed, discrete) = match action {
            Action::None => return,
            Action::Turn(changed) => (changed, false),
            Action::Switch(changed) => (changed, true),
        };
        let mut tracks = self.session.project().tracks.clone();
        tracks[track].eq = changed;
        let command = EditCommand::SetMixer {
            tracks,
            master_gain: self.session.project().master_gain,
        };
        let result = if discrete {
            self.session.edit_discrete(command)
        } else {
            self.session.edit(command)
        };
        self.apply(result);
        ctx.request_repaint();
    }
}

/// The window's content for the EQ named `name`, such as "Track 1 EQ".
fn show(ui: &mut egui::Ui, name: &str, eq: &ChannelEq) -> Action {
    let mut draft = *eq;
    let (bounds, _) = ui.allocate_exact_size(Vec2::new(WIDTH, HEIGHT), Sense::hover());
    // The controls are placed in the allocated bounds; their own scopes must
    // not lengthen the window's content.
    let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(bounds));
    let mut switched = false;

    let power_name = format!("{name} power");
    let response = controls::toggle_at(
        ui,
        Rect::from_min_size(bounds.min, Vec2::new(120.0, 28.0)),
        &power_name,
        "Power",
        &mut draft.on,
    );
    switched |= response.clicked();
    name_control(response, &power_name).on_hover_text(format!(
        "{power_name}: {}. An EQ that is off keeps its settings and passes the track on \
         unchanged.",
        if draft.on { "on" } else { "off" }
    ));
    let reset_name = format!("Reset {name}");
    let response = controls::button_at(
        ui,
        Rect::from_min_size(
            Pos2::new(bounds.right() - 84.0, bounds.top()),
            Vec2::new(84.0, 28.0),
        ),
        &reset_name,
        "Reset",
        controls::ButtonRole::Secondary,
        true,
    );
    if name_control(response, &reset_name)
        .on_hover_text("Return every band to flat. The power switch stays as it is.")
        .clicked()
    {
        draft = ChannelEq {
            on: draft.on,
            ..ChannelEq::default()
        };
        switched = true;
    }

    let curve = Rect::from_min_size(
        bounds.min + Vec2::new(0.0, 36.0),
        Vec2::new(WIDTH, CURVE_HEIGHT),
    );
    response_curve(ui, curve, &draft, name);

    let top = bounds.top() + DIALS_TOP;
    let first = bounds.left() + (WIDTH - COLUMNS.len() as f32 * COLUMN) / 2.0;
    for (column, (heading, params)) in COLUMNS.iter().enumerate() {
        let x = first + (column as f32 + 0.5) * COLUMN;
        ui.painter().text(
            Pos2::new(x, top + 8.0),
            Align2::CENTER_CENTER,
            heading,
            FontId::proportional(12.0),
            CREAM,
        );
        for (row, param) in params.iter().enumerate() {
            let y = top + 48.0 + row as f32 * ROW;
            let (label, hint, format) = face(*param);
            let before = decimal(param.get(&draft));
            let mut value = before;
            dial(
                ui,
                [
                    Pos2::new(x, y),
                    Pos2::new(x, y + 38.0),
                    Pos2::new(x, y + 60.0),
                ],
                &mut value,
                Dial {
                    name: &format!("{name} {}", param.name()),
                    label,
                    hint,
                    range: param.range(),
                    format,
                    choices: &[],
                },
                draft.on,
            );
            if value != before {
                param.set(&mut draft, value as f32);
            }
        }
    }
    if draft == *eq {
        Action::None
    } else if switched {
        Action::Switch(draft)
    } else {
        Action::Turn(draft)
    }
}

/// The EQ's magnitude response, on a frequency scale from 20 Hz to 20 kHz
/// that spreads octaves evenly. An EQ that is off draws its curve faint.
fn response_curve(ui: &egui::Ui, rect: Rect, eq: &ChannelEq, name: &str) {
    controls::inset(ui, rect);
    let plot = Rect::from_min_max(
        rect.min + Vec2::new(30.0, 8.0),
        rect.max - Vec2::new(10.0, 18.0),
    );
    let x_of = |hz: f64| plot.left() + ((hz / 20.0).log10() / 3.0) as f32 * plot.width();
    let y_of = |db: f64| {
        plot.center().y - (db.clamp(-CURVE_DB, CURVE_DB) / CURVE_DB) as f32 * plot.height() / 2.0
    };
    let grid = CREAM.gamma_multiply(0.18);
    let painter = ui.painter().with_clip_rect(rect);
    for hz in [50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0] {
        let x = x_of(hz);
        painter.line_segment(
            [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
            Stroke::new(1.0_f32, grid),
        );
        if let Some(label) = match hz as u32 {
            100 => Some("100"),
            1000 => Some("1k"),
            10_000 => Some("10k"),
            _ => None,
        } {
            painter.text(
                Pos2::new(x, plot.bottom() + 9.0),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(10.0),
                CREAM,
            );
        }
    }
    for (db, label) in [(12.0, "+12"), (0.0, "0"), (-12.0, "−12")] {
        let y = y_of(db);
        painter.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(
                1.0_f32,
                if db == 0.0 {
                    CREAM.gamma_multiply(0.4)
                } else {
                    grid
                },
            ),
        );
        painter.text(
            Pos2::new(plot.left() - 5.0, y),
            Align2::RIGHT_CENTER,
            label,
            FontId::proportional(10.0),
            CREAM,
        );
    }
    let points: Vec<Pos2> = (0..=160)
        .map(|step| {
            let hz = 20.0 * 1000.0_f64.powf(f64::from(step) / 160.0);
            Pos2::new(x_of(hz), y_of(eq.response_db(hz)))
        })
        .collect();
    let color = if eq.on {
        controls::print_color(ui)
    } else {
        CREAM.gamma_multiply(0.35)
    };
    painter.add(egui::Shape::line(points, Stroke::new(2.0_f32, color)));
    let response = ui.interact(rect, ui.id().with(("eq-curve", name)), Sense::hover());
    named(
        &response,
        egui::WidgetType::Other,
        &format!("{name} response curve"),
    );
    response.on_hover_text(format!(
        "{name}: the change in level across the spectrum{}. The grid marks 100 Hz, 1 kHz, \
         and 10 kHz, and 12 dB steps.",
        if eq.on { "" } else { " once the EQ is on" }
    ));
}
