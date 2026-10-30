//! The channel EQ window of one track: its power switch, Reset, the
//! response curve of its settings, and a dial for each control, in signal
//! order from the low cut to the high cut; below them the track's
//! compressor, with its switch and dials, and the track's width; at the
//! foot the track's ducker, with its switch, its key track, and its dials.
//! The window edits copies of the track's settings; the caller turns
//! changed copies into a mixer edit.
use super::controls::CREAM;
use super::master::{Dial, Format, decimal, dial};
use super::*;
use mixjam_techno::application::{
    KnobRange, TRACK_WIDTH,
    compressor::{Compressor, CompressorParam},
    ducker::{Ducker, DuckerParam},
    eq::{ChannelEq, EqParam},
};

/// The track settings that the window edits.
#[derive(Clone, Copy, PartialEq)]
struct Channel {
    eq: ChannelEq,
    compressor: Compressor,
    width: f32,
    ducker: Ducker,
}

/// What the window changed. Turns of one dial continue one undo step; the
/// power switches and Reset are steps of their own.
enum Action {
    None,
    Turn(Channel),
    Switch(Channel),
}

/// The content's width, the response curve's height, and the pitch of the
/// dial columns and rows.
const WIDTH: f32 = 520.0;
const CURVE_HEIGHT: f32 = 132.0;
const COLUMN: f32 = 86.0;
const ROW: f32 = 92.0;
/// Where the first row of dials starts below the curve.
const DIALS_TOP: f32 = 36.0 + CURVE_HEIGHT + 10.0;
/// Where the compressor's section starts: below the EQ's three rows of
/// dials, with room between the Q label and the section divider.
const COMPRESSOR_TOP: f32 = DIALS_TOP + 48.0 + 2.0 * ROW + 80.0;
/// A section's switch and row of dials, and the gap to the next divider.
const SECTION: f32 = 48.0 + 70.0;
const SECTION_GAP: f32 = 14.0;
/// Where the ducker's section starts, below the compressor's.
const DUCKER_TOP: f32 = COMPRESSOR_TOP + SECTION + SECTION_GAP;
/// The content's height: the controls row, the curve, the column headings,
/// three rows of dials, and the compressor's and the ducker's sections.
const HEIGHT: f32 = DUCKER_TOP + SECTION;
/// The key dial: a track from 1 to 64.
const KEY_RANGE: KnobRange = KnobRange {
    low: 1.0,
    high: 64.0,
    default: 1.0,
    step: 1.0,
    logarithmic: false,
};
/// The curve spans 20 Hz to 20 kHz and this many decibels each way.
const CURVE_DB: f64 = 24.0;

/// Each band's heading and its controls from top to bottom.
const COLUMNS: [(&str, &[EqParam]); 6] = [
    ("Low cut", &[EqParam::LowCut]),
    ("Low shelf", &[EqParam::LowHz, EqParam::LowDb]),
    ("Mid peak", &[EqParam::MidHz, EqParam::MidDb, EqParam::MidQ]),
    (
        "Peak 2",
        &[EqParam::Peak2Hz, EqParam::Peak2Db, EqParam::Peak2Q],
    ),
    ("High shelf", &[EqParam::HighHz, EqParam::HighDb]),
    ("High cut", &[EqParam::HighCut]),
];

/// The centre of a dial column, from 0, in the grid that the EQ's columns
/// and the sections below share.
fn column_x(bounds: Rect, column: usize) -> f32 {
    bounds.left() + (WIDTH - COLUMNS.len() as f32 * COLUMN) / 2.0 + (column as f32 + 0.5) * COLUMN
}

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
        EqParam::Peak2Hz => (
            "Freq",
            "The centre of the second peak, from 40 Hz to 16 kHz.",
            Format::Hertz,
        ),
        EqParam::Peak2Db => (
            "Gain",
            "Raises or lowers the band around the second peak's frequency. A narrow cut tames \
             one ringing line while the first peak does another job.",
            Format::db(true),
        ),
        EqParam::Peak2Q => (
            "Q",
            "The width of the second peak: a higher Q is narrower.",
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

/// The printed label, description, and value format of a compressor control.
fn compressor_face(param: CompressorParam) -> (&'static str, &'static str, Format) {
    match param {
        CompressorParam::Threshold => (
            "Thresh",
            "Compression starts above this level of the track after its EQ.",
            Format::db(false),
        ),
        CompressorParam::Ratio => (
            "Ratio",
            "How strongly the level above the threshold comes down.",
            Format::tenths(":1"),
        ),
        CompressorParam::Attack => (
            "Attack",
            "How fast compression starts. A short attack catches drum hits.",
            Format::tenths("ms"),
        ),
        CompressorParam::Release => (
            "Release",
            "How fast compression lets go.",
            Format::whole("ms"),
        ),
        CompressorParam::Makeup => (
            "Makeup",
            "Raises the track after the compression.",
            Format::db(true),
        ),
    }
}

/// The printed label, description, and value format of a ducker control.
fn ducker_face(param: DuckerParam) -> (&'static str, &'static str, Format) {
    match param {
        DuckerParam::Threshold => (
            "Thresh",
            "The key's level above which this track ducks.",
            Format::db(true),
        ),
        DuckerParam::Depth => (
            "Depth",
            "The most the track ducks. It ducks by as much as the key rises over the \
             threshold, up to this.",
            Format::db(false),
        ),
        DuckerParam::Attack => ("Attack", "How fast the track ducks.", Format::tenths("ms")),
        DuckerParam::Release => (
            "Release",
            "How fast the track comes back after the key.",
            Format::whole("ms"),
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
        let settings = &self.session.project().tracks[track];
        let channel = Channel {
            eq: settings.eq,
            compressor: settings.compressor,
            width: settings.width,
            ducker: settings.ducker,
        };
        // A replaced project starts new dial gestures.
        let generation = self.master_view_generation;
        let mut open = true;
        let mut action = Action::None;
        // The shell's margins and bay take 36 by 12 points of the size.
        // Reserve the solid vertical scrollbar too, so a short viewport
        // does not also force the fixed-width controls to scroll sideways.
        let scrollbar = egui::style::ScrollStyle::solid().allocated_width();
        dialog::window(&title)
            .open(&mut open)
            .default_width(WIDTH + 40.0 + scrollbar)
            .default_height(HEIGHT + 16.0)
            .max_height((ctx.viewport_rect().height() - 40.0).max(240.0))
            .constrain_to(ctx.viewport_rect().shrink(12.0))
            .show(ctx, |ui| {
                action = ui
                    .push_id(generation, |ui| show(ui, &title, track, &channel))
                    .inner;
            });
        if !open {
            self.eq_track = None;
        }
        let (changed, discrete) = match action {
            Action::None => return,
            Action::Turn(channel) => (channel, false),
            Action::Switch(channel) => (channel, true),
        };
        let mut tracks = self.session.project().tracks.clone();
        tracks[track].eq = changed.eq;
        tracks[track].compressor = changed.compressor;
        tracks[track].width = changed.width;
        tracks[track].ducker = changed.ducker;
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

/// The window's content for the EQ named `name`, such as "Track 1 EQ", of
/// `track`, from 0.
fn show(ui: &mut egui::Ui, name: &str, track: usize, channel: &Channel) -> Action {
    let mut draft = channel.eq;
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
    for (column, (heading, params)) in COLUMNS.iter().enumerate() {
        let x = column_x(bounds, column);
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
                dial_points(x, y),
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
    let (compressor, width, compressor_switched) =
        compressor_section(ui, bounds, track, &channel.compressor, channel.width);
    let (ducker, ducker_switched) = ducker_section(ui, bounds, track, &channel.ducker);
    let changed = Channel {
        eq: draft,
        compressor,
        width,
        ducker,
    };
    if changed == *channel {
        Action::None
    } else if switched || compressor_switched || ducker_switched {
        Action::Switch(changed)
    } else {
        Action::Turn(changed)
    }
}

/// Three points of a dial at `x`: the knob's centre at `y`, its label,
/// and its value.
fn dial_points(x: f32, y: f32) -> [Pos2; 3] {
    [
        Pos2::new(x, y),
        Pos2::new(x, y + 38.0),
        Pos2::new(x, y + 60.0),
    ]
}

/// The compressor's switch and dials below the EQ, and the track's width
/// at the end of their row; whether the switch changed. The width has no
/// switch: at 100% it leaves the track as it is.
fn compressor_section(
    ui: &mut egui::Ui,
    bounds: Rect,
    track: usize,
    compressor: &Compressor,
    width: f32,
) -> (Compressor, f32, bool) {
    let mut draft = *compressor;
    let label = format!("Track {}", track + 1);
    let top = bounds.top() + COMPRESSOR_TOP;
    controls::inset(
        ui,
        Rect::from_min_size(Pos2::new(bounds.left(), top - 8.0), Vec2::new(WIDTH, 1.0)),
    );
    let power_name = format!("{label} compressor power");
    let response = controls::toggle_at(
        ui,
        Rect::from_min_size(Pos2::new(bounds.left(), top), Vec2::new(120.0, 28.0)),
        &power_name,
        "Compressor",
        &mut draft.on,
    );
    let switched = response.clicked();
    name_control(response, &power_name).on_hover_text(format!(
        "{power_name}: {}. The compressor evens out this track's level after its EQ, such as \
         the hits of a drum loop, before its width, automation, and fader.",
        if draft.on { "on" } else { "off" }
    ));
    let y = top + 48.0;
    for (column, param) in CompressorParam::ALL.into_iter().enumerate() {
        let (label_text, hint, format) = compressor_face(param);
        let before = decimal(param.get(&draft));
        let mut value = before;
        dial(
            ui,
            dial_points(column_x(bounds, column), y),
            &mut value,
            Dial {
                name: &format!("{label} {}", param.name()),
                label: label_text,
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
    let x = column_x(bounds, COLUMNS.len() - 1);
    ui.painter().text(
        Pos2::new(x, top + 14.0),
        Align2::CENTER_CENTER,
        "Stereo",
        FontId::proportional(12.0),
        CREAM,
    );
    let before = decimal(width);
    let mut value = before;
    dial(
        ui,
        dial_points(x, y),
        &mut value,
        Dial {
            name: &format!("{label} width"),
            label: "Width",
            hint: "The track's stereo width, before its pan: 0% is mono, 100% leaves it as it \
                   is, and 200% doubles the difference between its sides. It needs both \
                   outputs on.",
            range: TRACK_WIDTH,
            format: Format::whole("%"),
            choices: &[],
        },
        true,
    );
    let width = if value == before {
        width
    } else {
        (value as f32).clamp(TRACK_WIDTH.low, TRACK_WIDTH.high)
    };
    (draft, width, switched)
}

/// The ducker's switch and dials at the foot of the window, and whether
/// the switch changed. The key never names the track itself: the dial
/// steps over it.
fn ducker_section(
    ui: &mut egui::Ui,
    bounds: Rect,
    track: usize,
    ducker: &Ducker,
) -> (Ducker, bool) {
    let mut draft = *ducker;
    let own = track as f64 + 1.0;
    let label = format!("Track {}", track + 1);
    let top = bounds.top() + DUCKER_TOP;
    controls::inset(
        ui,
        Rect::from_min_size(Pos2::new(bounds.left(), top - 8.0), Vec2::new(WIDTH, 1.0)),
    );
    let power_name = format!("{label} ducker power");
    let response = controls::toggle_at(
        ui,
        Rect::from_min_size(Pos2::new(bounds.left(), top), Vec2::new(120.0, 28.0)),
        &power_name,
        "Ducker",
        &mut draft.on,
    );
    let switched = response.clicked();
    name_control(response, &power_name).on_hover_text(format!(
        "{power_name}: {}. The ducker lowers this track while its key track sounds, as a \
         bass makes room for the kick. It works after the fader and before the sends.",
        if draft.on { "on" } else { "off" }
    ));
    if draft.on && f64::from(draft.key) == own {
        draft.key = if track == 0 { 2 } else { 1 };
    }
    let y = top + 48.0;
    let at = |column: usize| column_x(bounds, column);
    let before = f64::from(draft.key);
    let mut key = before;
    dial(
        ui,
        dial_points(at(0), y),
        &mut key,
        Dial {
            name: &format!("{label} ducker key"),
            label: "Key",
            hint: "The track whose sound ducks this one, as it sounds after its fader.",
            range: KEY_RANGE,
            format: Format::whole(""),
            choices: &[],
        },
        draft.on,
    );
    let mut key = key.round().clamp(1.0, 64.0);
    if key != before && key == own {
        key = if key > before { own + 1.0 } else { own - 1.0 };
        if !(1.0..=64.0).contains(&key) {
            key = before;
        }
    }
    draft.key = key as u8;
    for (column, param) in DuckerParam::ALL.into_iter().enumerate() {
        let x = at(column + 1);
        let (label_text, hint, format) = ducker_face(param);
        let before = decimal(param.get(&draft));
        let mut value = before;
        dial(
            ui,
            dial_points(x, y),
            &mut value,
            Dial {
                name: &format!("{label} {}", param.name()),
                label: label_text,
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
    (draft, switched)
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
