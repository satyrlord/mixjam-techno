//! Mastering window body: the master pedalboard, laid out as the signal
//! flows. The mixer's output enters the input unit at the left, with its
//! trim and VU meter. It passes each pedal in order along a row that
//! scrolls, and reaches the output unit at the right, with the loudness
//! meter. A strip below the board names every pedal in signal order and
//! brings the one chosen into view. Shared hardware controls and shell own
//! presentation; the caller turns a changed chain into an edit.
use super::controls::{CREAM, INK};
use super::*;
use mixjam_techno::application::{Knob, KnobRange, MeterReading, OVER_DBTP};

pub(super) enum Action {
    None,
    Change(MasterChain),
    Help,
}

/// What the meters show: the latest reading, and whether a mix plays.
pub(super) struct Metering {
    pub reading: MeterReading,
    pub playing: bool,
}

const MARGIN: f32 = 8.0;
const INPUT_WIDTH: f32 = 150.0;
const OUTPUT_WIDTH: f32 = 200.0;
const CABLE: f32 = 26.0;
const PEDAL_TOP: f32 = 10.0;
const PEDAL_HEIGHT: f32 = 304.0;
const HEADER: f32 = 36.0;
const FOOTER_TOP: f32 = 260.0;
/// The height of the cables and sockets, through the middle of the pedals.
const SIGNAL_Y: f32 = PEDAL_TOP + 150.0;
/// One knob with its value well and label, and the pitch of knob rows.
const CELL: f32 = 62.0;
const ROW: f32 = 96.0;
const PEDAL_PADDING: f32 = 14.0;
/// Room under the pedals for their shadows and the row's scroll bar.
const ROW_HEIGHT: f32 = PEDAL_TOP + PEDAL_HEIGHT + 20.0;
const STRIP_HEIGHT: f32 = 62.0;
const BOARD_HEIGHT: f32 = ROW_HEIGHT + 8.0 + STRIP_HEIGHT;
/// The gain-reduction lamps above a dynamics pedal's knobs.
const LAMP_ROW: f32 = 28.0;
const WELL: Vec2 = Vec2::new(56.0, 20.0);
const LIME: Color32 = Color32::from_rgb(184, 227, 126);
const AMBER: Color32 = Color32::from_rgb(246, 178, 64);
const RED: Color32 = Color32::from_rgb(232, 72, 56);
const GREEN: Color32 = Color32::from_rgb(110, 196, 112);
const DARK_LAMP: Color32 = Color32::from_rgb(44, 48, 42);

fn name(kind: PedalKind) -> &'static str {
    match kind {
        PedalKind::EjayEq => "Booster EQ",
        PedalKind::EjayCompressor => "Booster compressor",
        PedalKind::EjayWide => "Booster wide",
        PedalKind::SoftClip => "Soft clip",
        PedalKind::Tube => "Tube",
        PedalKind::TrimEq => "Trim EQ",
        PedalKind::BusCompressor => "Bus compressor",
        PedalKind::Maximizer => "Maximizer",
        PedalKind::LiftEq => "Lift EQ",
        PedalKind::Tape => "Tape",
        PedalKind::Imaging => "Stereo imaging",
        PedalKind::Multiband => "Multiband",
        PedalKind::Limiter => "Limiter",
    }
}

/// The name printed on the pedal. The Booster pedals print the original's
/// names; their family line says where they come from.
fn title(kind: PedalKind) -> &'static str {
    match kind {
        PedalKind::EjayEq => "EQ",
        PedalKind::EjayCompressor => "Compressor",
        PedalKind::EjayWide => "Stereo wide",
        _ => name(kind),
    }
}

/// The pedal's name on the strip below the board.
fn short_name(kind: PedalKind) -> &'static str {
    match kind {
        PedalKind::EjayEq => "EQ",
        PedalKind::EjayCompressor => "Comp",
        PedalKind::EjayWide => "Wide",
        PedalKind::SoftClip => "Clip",
        PedalKind::Tube => "Tube",
        PedalKind::TrimEq => "Trim",
        PedalKind::BusCompressor => "Bus",
        PedalKind::Maximizer => "Max",
        PedalKind::LiftEq => "Lift",
        PedalKind::Tape => "Tape",
        PedalKind::Imaging => "Image",
        PedalKind::Multiband => "Multi",
        PedalKind::Limiter => "Limit",
    }
}

/// The line under the pedal's name: where it comes from or what it does.
fn family(kind: PedalKind) -> &'static str {
    match kind {
        PedalKind::EjayEq | PedalKind::EjayCompressor | PedalKind::EjayWide => "eJay Booster",
        PedalKind::SoftClip | PedalKind::Tube | PedalKind::Tape => "Saturation",
        PedalKind::TrimEq | PedalKind::LiftEq => "Equaliser",
        PedalKind::Imaging => "Stereo",
        PedalKind::Limiter => "Dynamics, always last",
        _ => "Dynamics",
    }
}

/// Knob columns and rows of a pedal.
fn grid(kind: PedalKind) -> (usize, usize) {
    match kind {
        PedalKind::EjayEq => (5, 2),
        PedalKind::EjayWide | PedalKind::Maximizer => (1, 1),
        PedalKind::EjayCompressor
        | PedalKind::TrimEq
        | PedalKind::BusCompressor
        | PedalKind::Multiband => (2, 2),
        _ => (2, 1),
    }
}

/// The dials on a pedal's face: its knobs, and the tape's speed selector.
fn dial_count(kind: PedalKind) -> usize {
    kind.knobs().len() + usize::from(kind == PedalKind::Tape)
}

/// The reductions in dB at which a dynamics pedal's lamps light.
fn reduction_lamps(kind: PedalKind) -> Option<[f32; 6]> {
    match kind {
        PedalKind::BusCompressor => Some([0.5, 1.0, 1.5, 2.0, 3.0, 4.0]),
        PedalKind::Limiter => Some([0.5, 1.0, 2.0, 3.0, 4.5, 6.0]),
        _ => None,
    }
}

fn pedal_width(kind: PedalKind) -> f32 {
    let (columns, _) = grid(kind);
    (columns as f32 * CELL + 2.0 * PEDAL_PADDING).max(132.0)
}

/// Each pedal's left edge along the row, after its cable, and the end of
/// the last cable.
fn row_layout(order: &[PedalKind]) -> (Vec<f32>, f32) {
    let mut x = 0.0;
    let lefts = order
        .iter()
        .map(|kind| {
            x += CABLE;
            let left = x;
            x += pedal_width(*kind);
            left
        })
        .collect();
    (lefts, x + CABLE)
}

/// Bands 0 and 9 are shelves with a 12 dB range; the others reach 18 dB.
fn band_range_db(band: usize) -> f64 {
    if band == 0 || band == 9 { 12.0 } else { 18.0 }
}

fn band_db(band: usize, value: f64) -> f64 {
    let range = band_range_db(band);
    value / f64::from(EQ_BAND_MAX) * 2.0 * range - range
}

fn band_label(hz: f64) -> String {
    if hz >= 1000.0 {
        format!("{} kHz", hz / 1000.0)
    } else {
        format!("{hz} Hz")
    }
}

/// How a dial's value reads in its window.
#[derive(Clone, Copy)]
enum Format {
    /// The eJay pedals' 0 to 1000 positions.
    Plain,
    /// The gain of an eJay equaliser band, shown in decibels.
    Band(usize),
    /// A number in a unit.
    Unit {
        unit: &'static str,
        decimals: usize,
        signed: bool,
    },
    /// The tape speed, in inches per second.
    Speed,
}
impl Format {
    const fn db(signed: bool) -> Self {
        Self::Unit {
            unit: "dB",
            decimals: 1,
            signed,
        }
    }
    const fn tenths(unit: &'static str) -> Self {
        Self::Unit {
            unit,
            decimals: 1,
            signed: false,
        }
    }
    const fn whole(unit: &'static str) -> Self {
        Self::Unit {
            unit,
            decimals: 0,
            signed: false,
        }
    }
    fn unit(self) -> &'static str {
        match self {
            Self::Plain => "",
            Self::Band(_) => "dB",
            Self::Unit { unit, .. } => unit,
            Self::Speed => "ips",
        }
    }
    fn show(self, value: f64) -> String {
        // Rounding can leave a negative zero.
        let value = value + 0.0;
        match self {
            Self::Plain => format!("{value:.0}"),
            Self::Band(band) => format!("{:+.1}", band_db(band, value) + 0.0),
            Self::Speed => format!("{value:.0} ips"),
            Self::Unit {
                unit,
                decimals,
                signed,
            } => {
                let number = if signed {
                    format!("{value:+.decimals$}")
                } else {
                    format!("{value:.decimals$}")
                };
                match unit {
                    "" => number,
                    "%" | ":1" => format!("{number}{unit}"),
                    _ => format!("{number} {unit}"),
                }
            }
        }
    }
    /// A typed value, with or without its unit.
    fn read(self, text: &str, high: f64) -> Option<f64> {
        let text = text.trim();
        let unit = self.unit();
        let text = if !unit.is_empty() && text.to_lowercase().ends_with(&unit.to_lowercase()) {
            &text[..text.len() - unit.len()]
        } else {
            text
        };
        let number: f64 = text.trim().parse().ok()?;
        Some(match self {
            Self::Band(band) => {
                let range = band_range_db(band);
                ((number + range) / (2.0 * range) * high).round()
            }
            _ => number,
        })
    }
}

/// Everything a dial shows besides its value.
struct Dial<'a> {
    name: &'a str,
    label: &'a str,
    hint: &'a str,
    range: KnobRange,
    format: Format,
    choices: &'a [f64],
}

/// The printed label, accessible name, description, and value format of a knob.
fn face(knob: Knob) -> (String, String, &'static str, Format) {
    let (label, name, hint, format) = match knob {
        Knob::EjayBand(band) => {
            let hz = EQ_FREQUENCIES[band];
            return (
                band_label(hz),
                format!("Booster EQ {hz} Hz"),
                "The middle position is flat.",
                Format::Band(band),
            );
        }
        Knob::Trim => (
            "Trim",
            "Trim",
            "Gain into the board. The meter reads 0 VU at -18 dBFS, where the mastering pedals work best.",
            Format::db(true),
        ),
        Knob::EjayDrive => ("Drive", "Booster compressor drive", "", Format::Plain),
        Knob::EjaySpeed => ("Speed", "Booster compressor speed", "", Format::Plain),
        Knob::EjayGain => ("Gain", "Booster compressor gain", "", Format::Plain),
        Knob::EjaySpread => ("Width", "Booster width", "", Format::Plain),
        Knob::SoftClipAmount => (
            "Amount",
            "Soft clip amount",
            "How far below the ceiling the curve starts to round peaks off.",
            Format::db(false),
        ),
        Knob::SoftClipCeiling => (
            "Ceiling",
            "Soft clip ceiling",
            "The level that rounded peaks approach.",
            Format::db(false),
        ),
        Knob::TubeDrive => (
            "Drive",
            "Tube drive",
            "How hard the mix drives the tube curve.",
            Format::tenths(""),
        ),
        Knob::TubeMix => (
            "Mix",
            "Tube mix",
            "The saturated share of the output.",
            Format::whole("%"),
        ),
        Knob::TrimEqHighPass => (
            "High-pass",
            "Trim EQ high-pass",
            "Removes rumble below this frequency.",
            Format::whole("Hz"),
        ),
        Knob::TrimEqMud => (
            "Mud 250",
            "Trim EQ mud",
            "A narrow cut at 250 Hz.",
            Format::db(false),
        ),
        Knob::TrimEqHarsh => (
            "Harsh 3.5k",
            "Trim EQ harsh",
            "A narrow cut at 3.5 kHz.",
            Format::db(false),
        ),
        Knob::CompressorThreshold => (
            "Threshold",
            "Bus compressor threshold",
            "Compression starts above this level.",
            Format::db(false),
        ),
        Knob::CompressorRatio => (
            "Ratio",
            "Bus compressor ratio",
            "How strongly the level above the threshold comes down.",
            Format::tenths(":1"),
        ),
        Knob::CompressorAttack => (
            "Attack",
            "Bus compressor attack",
            "How fast compression starts.",
            Format::tenths("ms"),
        ),
        Knob::CompressorRelease => (
            "Release",
            "Bus compressor release",
            "How fast compression lets go.",
            Format::whole("ms"),
        ),
        Knob::MaximizerBoost => (
            "Boost",
            "Maximizer boost",
            "Louder, with peaks held below -1 dBFS. Each percent adds a quarter decibel.",
            Format::whole("%"),
        ),
        Knob::LiftLow => (
            "Low 90",
            "Lift EQ low",
            "A wide shelf of weight at 90 Hz.",
            Format::db(true),
        ),
        Knob::LiftAir => (
            "Air 12k",
            "Lift EQ air",
            "A wide shelf of air at 12 kHz.",
            Format::db(true),
        ),
        Knob::TapeDrive => (
            "Drive",
            "Tape drive",
            "How hard the mix hits the tape: saturation, a low-end bump, and a softer top.",
            Format::tenths(""),
        ),
        Knob::ImagingWidth => (
            "Width",
            "Stereo imaging width",
            "The side signal: below 100% narrows the image, above widens it.",
            Format::whole("%"),
        ),
        Knob::ImagingMonoBelow => (
            "Mono below",
            "Stereo imaging mono below",
            "The image is mono below this frequency.",
            Format::whole("Hz"),
        ),
        Knob::MultibandLow => (
            "Low",
            "Multiband low",
            "Compression below 120 Hz.",
            Format::whole("%"),
        ),
        Knob::MultibandMid => (
            "Mid",
            "Multiband mid",
            "Compression from 120 Hz to 2 kHz.",
            Format::whole("%"),
        ),
        Knob::MultibandHigh => (
            "High",
            "Multiband high",
            "Compression above 2 kHz.",
            Format::whole("%"),
        ),
        Knob::LimiterGain => (
            "Gain",
            "Limiter gain",
            "Gain into the limiter.",
            Format::db(true),
        ),
        Knob::LimiterCeiling => (
            "Ceiling",
            "Limiter ceiling",
            "The highest true peak out, in dBTP.",
            Format::db(false),
        ),
    };
    (label.into(), name.into(), hint, format)
}

/// The pedal being dragged to a new place, by its position on the board.
#[derive(Clone, Copy)]
struct Drag {
    from: usize,
}

/// Lamp states that outlast a single reading: when each input peak lamp
/// last lit, and whether the OVER lamp has latched since it was cleared.
#[derive(Clone, Copy)]
struct Lamps {
    peaks: [f64; 2],
    run: u32,
    overs: u32,
    over: bool,
}
impl Default for Lamps {
    fn default() -> Self {
        Self {
            peaks: [f64::NEG_INFINITY; 2],
            run: 0,
            overs: 0,
            over: false,
        }
    }
}

/// What the scrolling row reports: the part of it in view, as fractions
/// of its length, and the scroll offset that centres each pedal.
struct RowView {
    visible: [f32; 2],
    centres: Vec<f32>,
}

/// Show the window. `reveal` brings a pedal into view, as its chip does.
pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    chain: &MasterChain,
    metering: &Metering,
    reveal: Option<PedalKind>,
) -> Action {
    let mut draft = *chain;
    let (_, row_end) = row_layout(&chain.order);
    let natural = Vec2::new(
        2.0 * MARGIN + INPUT_WIDTH + OUTPUT_WIDTH + row_end,
        BOARD_HEIGHT,
    );
    if metering.playing && *open {
        ctx.request_repaint_after(Duration::from_millis(33));
    }
    let response = dialog::tool(
        ctx,
        "Mastering",
        open,
        natural,
        controls::Palette::Indigo,
        true,
        |ui| {
            let minimum = 2.0 * MARGIN + INPUT_WIDTH + OUTPUT_WIDTH + 160.0;
            let size = Vec2::new(ui.available_width().max(minimum), BOARD_HEIGHT);
            let (bounds, _) = ui.allocate_exact_size(size, Sense::hover());
            let mut panel = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
            let top = bounds.top();
            let input = Rect::from_min_size(
                Pos2::new(bounds.left() + MARGIN, top + PEDAL_TOP),
                Vec2::new(INPUT_WIDTH, PEDAL_HEIGHT),
            );
            let output = Rect::from_min_size(
                Pos2::new(bounds.right() - MARGIN - OUTPUT_WIDTH, top + PEDAL_TOP),
                Vec2::new(OUTPUT_WIDTH, PEDAL_HEIGHT),
            );
            let lamps_id = panel.id().with("master-lamps");
            let mut lamps = panel
                .ctx()
                .data(|data| data.get_temp::<Lamps>(lamps_id))
                .unwrap_or_default();
            input_unit(&mut panel, input, &mut draft, metering, &mut lamps);
            output_unit(&mut panel, output, metering, &mut lamps);
            panel
                .ctx()
                .data_mut(|data| data.insert_temp(lamps_id, lamps));

            let viewport = Rect::from_min_max(
                Pos2::new(input.right(), top),
                Pos2::new(output.left(), top + ROW_HEIGHT),
            );
            let jump_id = panel.id().with("pedal-jump");
            let row = pedal_row(&mut panel, viewport, jump_id, chain, &mut draft, metering);
            let strip_rect = Rect::from_min_size(
                Pos2::new(bounds.left() + MARGIN, top + ROW_HEIGHT + 8.0),
                Vec2::new(bounds.width() - 2.0 * MARGIN, STRIP_HEIGHT),
            );
            let revealed = reveal.and_then(|kind| chain.order.iter().position(|k| *k == kind));
            if let Some(index) = strip(&mut panel, strip_rect, &draft, row.visible).or(revealed)
                && let Some(offset) = row.centres.get(index)
            {
                panel
                    .ctx()
                    .data_mut(|data| data.insert_temp(jump_id, *offset));
                panel.ctx().request_repaint();
            }
        },
    );
    if response.help {
        Action::Help
    } else if draft != *chain {
        Action::Change(draft)
    } else {
        Action::None
    }
}

/// The scrolling row of pedals between the two units.
fn pedal_row(
    panel: &mut egui::Ui,
    viewport: Rect,
    jump_id: egui::Id,
    chain: &MasterChain,
    draft: &mut MasterChain,
    metering: &Metering,
) -> RowView {
    let (lefts, natural) = row_layout(&chain.order);
    let row_width = natural.max(viewport.width());
    let jump = panel
        .ctx()
        .data_mut(|data| data.remove_temp::<f32>(jump_id));
    let mut row = panel.new_child(
        egui::UiBuilder::new()
            .id_salt("pedal-row")
            .max_rect(viewport),
    );
    let mut area = egui::ScrollArea::horizontal()
        .id_salt("pedal-row-scroll")
        .auto_shrink([false, false]);
    if let Some(offset) = jump {
        area = area.horizontal_scroll_offset(offset);
    }
    let shown = area.show(&mut row, |ui| {
        let (content, _) = ui.allocate_exact_size(Vec2::new(row_width, ROW_HEIGHT), Sense::hover());
        let drag_id = ui.id().with("pedal-drag");
        let mut drag = ui
            .ctx()
            .data(|data| data.get_temp::<Drag>(drag_id).map(|d| d.from));
        let pedals: Vec<(PedalKind, Rect)> = chain
            .order
            .iter()
            .zip(&lefts)
            .map(|(kind, left)| {
                let min = Pos2::new(content.left() + left, content.top() + PEDAL_TOP);
                (
                    *kind,
                    Rect::from_min_size(min, Vec2::new(pedal_width(*kind), PEDAL_HEIGHT)),
                )
            })
            .collect();
        let mut ends = vec![content.left()];
        for (_, rect) in &pedals {
            ends.push(rect.left());
            ends.push(rect.right());
        }
        ends.push(content.right());
        let last = ends.len() / 2 - 1;
        for (index, [from, to]) in ends.as_chunks::<2>().0.iter().enumerate() {
            cable(
                ui,
                *from,
                *to,
                content.top() + SIGNAL_Y,
                [index != 0, index != last],
            );
        }

        let count = pedals.len();
        for (index, (kind, rect)) in pedals.iter().enumerate() {
            if let Some(to) = pedal(ui, draft, *kind, *rect, index, metering) {
                draft.move_pedal(index, to);
            }
            // The header is the handle that drags a pedal along the board.
            // The limiter has none: it stays last, just before the output.
            let handle = Rect::from_min_size(rect.min, Vec2::new(rect.width() - 30.0, HEADER));
            if !kind.movable() {
                let header = ui.interact(handle, ui.id().with(("pedal-grip", *kind)), Sense::hover());
                name_region(header, &format!("{} always runs last", name(*kind))).on_hover_text(
                    "The limiter always runs last, just before the output, so no pedal can raise the peaks it holds.",
                );
                continue;
            }
            let grip = ui.interact(handle, ui.id().with(("pedal-grip", *kind)), Sense::drag());
            let grip = name_control(grip, &format!("Drag {}", name(*kind)))
                .on_hover_cursor(egui::CursorIcon::Grab)
                .on_hover_text(
                    "Drag along the board to change where this pedal sits in the signal path.",
                );
            if grip.drag_started() {
                drag = Some(index);
            }
            if grip.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
        }

        // While a pedal is dragged, mark where it would land, and scroll
        // the row when the pointer nears either end of it.
        if let Some(from) = drag {
            let accent = ui.visuals().selection.bg_fill;
            let pointer = ui.ctx().pointer_latest_pos();
            let released = ui.input(|input| input.pointer.any_released());
            if let Some(pointer) = pointer
                && !released
            {
                let edge = 48.0;
                let push = if pointer.x < viewport.left() + edge {
                    (viewport.left() + edge - pointer.x).min(edge)
                } else if pointer.x > viewport.right() - edge {
                    -(pointer.x - (viewport.right() - edge)).min(edge)
                } else {
                    0.0
                };
                if push != 0.0 {
                    ui.scroll_with_delta_animation(
                        Vec2::new(push * 0.4, 0.0),
                        egui::style::ScrollAnimation::none(),
                    );
                    ui.ctx().request_repaint();
                }
            }
            let target = pointer.map(|pointer| {
                pedals
                    .iter()
                    .enumerate()
                    .filter(|(index, (_, rect))| *index != from && rect.center().x < pointer.x)
                    .count()
                    // Nothing lands after the limiter.
                    .min(count - 2)
            });
            if let Some(to) = target {
                let (_, dragged) = pedals[from];
                ui.painter().rect_stroke(
                    dragged,
                    6.0,
                    Stroke::new(2.0_f32, accent),
                    egui::StrokeKind::Outside,
                );
                let others: Vec<Rect> = pedals
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != from)
                    .map(|(_, (_, rect))| *rect)
                    .collect();
                let marker_x = match (
                    to.checked_sub(1).and_then(|i| others.get(i)),
                    others.get(to),
                ) {
                    (Some(before), _) => before.right() + CABLE / 2.0,
                    (None, Some(after)) => after.left() - CABLE / 2.0,
                    (None, None) => dragged.center().x,
                };
                ui.painter().line_segment(
                    [
                        Pos2::new(marker_x, content.top() + PEDAL_TOP),
                        Pos2::new(marker_x, content.top() + PEDAL_TOP + PEDAL_HEIGHT),
                    ],
                    Stroke::new(3.0_f32, accent),
                );
                if released {
                    draft.move_pedal(from, to);
                }
            }
            if released || pointer.is_none() {
                drag = None;
            }
        }
        ui.ctx().data_mut(|data| match drag {
            Some(from) => data.insert_temp(drag_id, Drag { from }),
            None => data.remove::<Drag>(drag_id),
        });
    });
    let width = shown.inner_rect.width();
    let offset = shown.state.offset.x;
    let most = (row_width - width).max(0.0);
    // Shadows at the row's ends show that it continues past them.
    let view = shown.inner_rect;
    for (side, more) in [(-1.0, offset > 0.5), (1.0, offset < most - 0.5)] {
        if !more {
            continue;
        }
        let edge = if side < 0.0 {
            view.left()
        } else {
            view.right()
        };
        for step in 0..8 {
            let inner = edge - side * step as f32 * 2.0;
            let strip = Rect::from_x_y_ranges(
                inner.min(inner - side * 2.0)..=inner.max(inner - side * 2.0),
                view.top() + PEDAL_TOP..=view.top() + PEDAL_TOP + PEDAL_HEIGHT,
            );
            panel
                .painter()
                .rect_filled(strip, 0.0, Color32::from_black_alpha(70 - 8 * step as u8));
        }
    }
    RowView {
        visible: [offset / row_width, ((offset + width) / row_width).min(1.0)],
        centres: chain
            .order
            .iter()
            .zip(&lefts)
            .map(|(kind, left)| (left + pedal_width(*kind) / 2.0 - width / 2.0).clamp(0.0, most))
            .collect(),
    }
}

/// The input unit: the socket from the mixer, the trim, and the VU meter
/// with a peak lamp for each side. The meter reads the trimmed signal.
fn input_unit(
    ui: &mut egui::Ui,
    rect: Rect,
    chain: &mut MasterChain,
    metering: &Metering,
    lamps: &mut Lamps,
) {
    controls::inset(ui, rect);
    heading(ui, rect, "Input", "From the mixer", CREAM);
    let now = ui.input(|input| input.time);
    let reading = &metering.reading;
    let live = metering.playing && reading.run > 0;
    for (side, peak) in reading.input_peaks.iter().enumerate() {
        if live && *peak {
            lamps.peaks[side] = now;
        }
    }
    let vu = if live {
        reading.input_vu
    } else {
        f32::NEG_INFINITY
    };
    let meter = Rect::from_min_size(
        rect.min + Vec2::new(12.0, 44.0),
        Vec2::new(rect.width() - 24.0, 74.0),
    );
    let needle = ui
        .ctx()
        .animate_value_with_time(ui.id().with("vu-needle"), vu_position(vu), 0.06);
    vu_meter(ui, meter, needle);
    let lit = [0, 1].map(|side| live && now - lamps.peaks[side] < 0.4);
    for (side, label) in ["L", "R"].into_iter().enumerate() {
        let center = Pos2::new(
            meter.center().x - 22.0 + 44.0 * side as f32,
            meter.bottom() + 16.0,
        );
        led(ui, center, 4.5, lit[side], RED);
        ui.painter().text(
            center + Vec2::new(if side == 0 { -12.0 } else { 12.0 }, 0.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0),
            CREAM,
        );
    }
    ui.painter().text(
        Pos2::new(meter.center().x, meter.bottom() + 16.0),
        Align2::CENTER_CENTER,
        "PEAK",
        FontId::proportional(9.5),
        CREAM.gamma_multiply(0.75),
    );
    let level = ui.interact(
        meter.union(Rect::from_center_size(
            Pos2::new(meter.center().x, meter.bottom() + 16.0),
            Vec2::new(meter.width(), 12.0),
        )),
        ui.id().with("input-level"),
        Sense::hover(),
    );
    let text = if live && vu.is_finite() {
        format!(
            "Input level {} VU{}",
            show_level(vu),
            match lit {
                [true, true] => ", peaks on both sides",
                [true, false] => ", peaks on the left",
                [false, true] => ", peaks on the right",
                _ => "",
            }
        )
    } else {
        "Input level: no mix playing".to_owned()
    };
    named(&level, egui::WidgetType::Label, &text);
    level.on_hover_text("0 VU is -18 dBFS. Peak lamps light at -1 dBFS.");

    let x = rect.center().x - 8.0;
    let before = decimal(chain.trim);
    let mut value = before;
    let (label, name, hint, format) = face(Knob::Trim);
    dial(
        ui,
        [
            Pos2::new(x, rect.top() + 172.0),
            Pos2::new(x, rect.top() + 210.0),
            Pos2::new(x, rect.top() + 232.0),
        ],
        &mut value,
        Dial {
            name: &name,
            label: &label,
            hint,
            range: Knob::Trim.range(),
            format,
            choices: &[],
        },
        true,
    );
    if value != before {
        Knob::Trim.set(chain, value as f32);
    }

    // The socket at the right edge sends the trimmed mix into the row.
    let socket = Pos2::new(rect.right() - 12.0, rect.top() - PEDAL_TOP + SIGNAL_Y);
    cable(ui, socket.x, rect.right() + 1.0, socket.y, [false, false]);
    jack(ui, socket);
}

/// The output unit: the socket from the last pedal, the loudness readings,
/// a loudness bar with the -14 LUFS delivery band, a true-peak bar with its
/// -1 dBTP line, and the OVER lamp, which latches until it is clicked.
fn output_unit(ui: &mut egui::Ui, rect: Rect, metering: &Metering, lamps: &mut Lamps) {
    controls::inset(ui, rect);
    heading(ui, rect, "Output", "To playback and export", CREAM);
    let reading = &metering.reading;
    let live = metering.playing && reading.run > 0;
    if reading.run != lamps.run {
        (lamps.run, lamps.overs) = (reading.run, 0);
    }
    if reading.overs > lamps.overs {
        (lamps.overs, lamps.over) = (reading.overs, true);
    }

    // The OVER lamp sits where a pedal's lamp would.
    let over = Rect::from_center_size(
        rect.right_top() + Vec2::new(-34.0, 19.0),
        Vec2::new(56.0, 26.0),
    );
    let response = ui.interact(over, ui.id().with("over-lamp"), Sense::click());
    if response.clicked() {
        lamps.over = false;
    }
    led(
        ui,
        over.left_center() + Vec2::new(10.0, 0.0),
        5.5,
        lamps.over,
        RED,
    );
    ui.painter().text(
        over.left_center() + Vec2::new(22.0, 0.0),
        Align2::LEFT_CENTER,
        "OVER",
        FontId::proportional(11.0),
        CREAM,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            over.expand(1.0),
            4.0,
            Stroke::new(2.0_f32, controls::focus_color(ui)),
            egui::StrokeKind::Outside,
        );
    }
    named(&response, egui::WidgetType::Button, "Clear OVER");
    let state = if lamps.over {
        "OVER: detected."
    } else {
        "OVER: clear."
    };
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_description(state));
    response.on_hover_text(format!(
        "{state} Lights when the output's true peak passes {OVER_DBTP:.0} dBTP, and stays lit until you click it."
    ));

    let rows: [(&str, f32, &str); 4] = [
        (
            "Momentary",
            if live {
                reading.momentary
            } else {
                f32::NEG_INFINITY
            },
            "LUFS",
        ),
        (
            "Short-term",
            if live {
                reading.short_term
            } else {
                f32::NEG_INFINITY
            },
            "LUFS",
        ),
        ("Integrated", reading.integrated, "LUFS"),
        ("True peak", reading.true_peak_max, "dBTP"),
    ];
    for (index, (label, value, _)) in rows.iter().enumerate() {
        let y = rect.top() + 56.0 + index as f32 * 28.0;
        ui.painter().text(
            Pos2::new(rect.left() + 30.0, y),
            Align2::LEFT_CENTER,
            *label,
            FontId::proportional(11.5),
            CREAM,
        );
        let well = Rect::from_center_size(Pos2::new(rect.right() - 42.0, y), WELL);
        controls::well(ui, well);
        let hot = index == 3 && *value > OVER_DBTP;
        ui.painter().text(
            well.center(),
            Align2::CENTER_CENTER,
            show_level(*value),
            FontId::proportional(13.0),
            if hot { RED } else { CREAM },
        );
    }
    let summary = rows
        .iter()
        .map(|(label, value, unit)| {
            format!("{} {} {unit}", label.to_lowercase(), show_level(*value))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let readings = ui.interact(
        Rect::from_min_max(
            Pos2::new(rect.left() + 26.0, rect.top() + 44.0),
            Pos2::new(rect.right() - 8.0, rect.top() + 152.0),
        ),
        ui.id().with("output-loudness"),
        Sense::hover(),
    );
    named(
        &readings,
        egui::WidgetType::Label,
        &format!("Output loudness: {summary}"),
    );
    readings.on_hover_text(
        "Momentary and short-term loudness follow the last 400 ms and 3 s. Integrated loudness and the true peak cover the playback so far.",
    );

    // Bars: loudness from -36 to 0 LUFS with the -14 LUFS band, true peak
    // from -24 to 0 dBTP with the -1 dBTP line.
    let left = rect.left() + 30.0;
    let right = rect.right() - 14.0;
    let loudness = Rect::from_min_max(
        Pos2::new(left, rect.top() + 178.0),
        Pos2::new(right, rect.top() + 192.0),
    );
    let at = |bar: Rect, low: f32, value: f32| {
        bar.left() + bar.width() * ((value - low) / -low).clamp(0.0, 1.0)
    };
    bar(ui, loudness);
    if live && reading.momentary.is_finite() {
        let fill = Rect::from_min_max(
            loudness.min + Vec2::new(2.0, 3.0),
            Pos2::new(
                at(loudness, -36.0, reading.momentary),
                loudness.bottom() - 3.0,
            ),
        );
        ui.painter()
            .rect_filled(fill, 1.0, CREAM.gamma_multiply(0.85));
    }
    // The delivery band stays visible over the fill.
    let band = Rect::from_x_y_ranges(
        at(loudness, -36.0, -15.0)..=at(loudness, -36.0, -13.0),
        loudness.y_range(),
    );
    ui.painter()
        .rect_filled(band, 1.0, GREEN.gamma_multiply(0.55));
    ui.painter().rect_stroke(
        band,
        1.0,
        Stroke::new(1.0_f32, GREEN),
        egui::StrokeKind::Inside,
    );
    if live && reading.short_term.is_finite() {
        let x = at(loudness, -36.0, reading.short_term);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(x - 4.0, loudness.bottom() + 6.0),
                Pos2::new(x + 4.0, loudness.bottom() + 6.0),
                Pos2::new(x, loudness.bottom()),
            ],
            controls::print_color(ui),
            Stroke::NONE,
        ));
    }
    scale(
        ui,
        loudness,
        &[
            (-36.0, "-36"),
            (-24.0, "-24"),
            (-14.0, "-14"),
            (0.0, "0 LUFS"),
        ],
        -36.0,
    );

    let peak = Rect::from_min_max(
        Pos2::new(left, rect.top() + 222.0),
        Pos2::new(right, rect.top() + 232.0),
    );
    bar(ui, peak);
    if live && reading.true_peak.is_finite() {
        let fill = Rect::from_min_max(
            peak.min + Vec2::new(2.0, 2.5),
            Pos2::new(at(peak, -24.0, reading.true_peak), peak.bottom() - 2.5),
        );
        ui.painter()
            .rect_filled(fill, 1.0, CREAM.gamma_multiply(0.85));
    }
    if reading.true_peak_max.is_finite() {
        let x = at(peak, -24.0, reading.true_peak_max);
        ui.painter().line_segment(
            [
                Pos2::new(x, peak.top() + 1.0),
                Pos2::new(x, peak.bottom() - 1.0),
            ],
            Stroke::new(2.0_f32, CREAM),
        );
    }
    let line = at(peak, -24.0, OVER_DBTP);
    ui.painter().line_segment(
        [
            Pos2::new(line, peak.top() - 3.0),
            Pos2::new(line, peak.bottom() + 3.0),
        ],
        Stroke::new(2.0_f32, RED),
    );
    scale(
        ui,
        peak,
        &[(-24.0, "-24"), (-12.0, "-12"), (-1.0, "-1 dBTP")],
        -24.0,
    );

    // The socket at the left edge takes the last pedal's output.
    let socket = Pos2::new(rect.left() + 12.0, rect.top() - PEDAL_TOP + SIGNAL_Y);
    cable(ui, rect.left() - 1.0, socket.x, socket.y, [false, false]);
    jack(ui, socket);
}

/// A unit's or pedal's name with the smaller line under it.
fn heading(ui: &egui::Ui, rect: Rect, title: &str, line: &str, color: Color32) {
    let at = |y: f32| Pos2::new(rect.left() + PEDAL_PADDING, rect.top() + y);
    // A long name shrinks until it clears the lamp.
    let room = rect.width() - PEDAL_PADDING - 34.0;
    let size = [14.0, 13.0, 12.0, 11.0]
        .into_iter()
        .find(|size| {
            ui.painter()
                .layout_no_wrap(title.into(), FontId::proportional(*size), color)
                .size()
                .x
                <= room
        })
        .unwrap_or(11.0);
    ui.painter().text(
        at(15.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(size),
        color,
    );
    ui.painter().text(
        at(30.0),
        Align2::LEFT_CENTER,
        line,
        FontId::proportional(10.0),
        color,
    );
}

/// A meter's dark track.
fn bar(ui: &egui::Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 2.0, INK);
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0_f32, CREAM.gamma_multiply(0.25)),
        egui::StrokeKind::Inside,
    );
}

/// Marks and figures under a meter bar whose scale runs from `low` to 0.
fn scale(ui: &egui::Ui, bar: Rect, marks: &[(f32, &str)], low: f32) {
    for (value, label) in marks {
        let x = bar.left() + bar.width() * ((value - low) / -low);
        ui.painter().line_segment(
            [
                Pos2::new(x, bar.bottom() + 1.0),
                Pos2::new(x, bar.bottom() + 4.0),
            ],
            Stroke::new(1.0_f32, CREAM.gamma_multiply(0.6)),
        );
        let align = if *value == low {
            Align2::LEFT_TOP
        } else if x > bar.right() - 30.0 {
            Align2::RIGHT_TOP
        } else {
            Align2::CENTER_TOP
        };
        let x = if *value == low {
            bar.left()
        } else if x > bar.right() - 30.0 {
            bar.right()
        } else {
            x
        };
        ui.painter().text(
            Pos2::new(x, bar.bottom() + 6.0),
            align,
            *label,
            FontId::proportional(9.5),
            CREAM.gamma_multiply(0.75),
        );
    }
}

/// A level as the meters print it.
fn show_level(value: f32) -> String {
    if value.is_finite() {
        format!("{:.1}", value + 0.0)
    } else {
        "-".to_owned()
    }
}

/// The needle's travel from -20 to +3 VU, spread by voltage as VU scales are.
fn vu_position(vu: f32) -> f32 {
    if !vu.is_finite() {
        return 0.0;
    }
    let volts = |db: f32| 10f32.powf(db / 20.0);
    ((volts(vu.min(4.0)) - volts(-20.0)) / (volts(3.0) - volts(-20.0))).clamp(0.0, 1.03)
}

/// An ivory VU face with its scale, red zone, and needle.
fn vu_meter(ui: &egui::Ui, face: Rect, position: f32) {
    const IVORY: Color32 = Color32::from_rgb(240, 228, 190);
    let painter = ui.painter().with_clip_rect(face.intersect(ui.clip_rect()));
    painter.rect_filled(face, 4.0, IVORY);
    let pivot = Pos2::new(face.center().x, face.bottom() + 22.0);
    let radius = face.height() + 4.0;
    let point = |position: f32, reach: f32| {
        let angle = (-40.0 + 80.0 * position).to_radians();
        pivot + Vec2::new(angle.sin(), -angle.cos()) * reach
    };
    // The red zone above 0 VU.
    let zone: Vec<Pos2> = (0..=12)
        .map(|step| point(vu_position(3.0 * step as f32 / 12.0), radius - 6.0))
        .collect();
    painter.add(egui::Shape::line(
        zone,
        Stroke::new(4.0_f32, Color32::from_rgb(196, 52, 40)),
    ));
    for (value, label) in [
        (-20.0, Some("20")),
        // The scale is crowded at its low end, so -10 has a mark only.
        (-10.0, None),
        (-7.0, None),
        (-5.0, Some("5")),
        (-3.0, None),
        (-2.0, None),
        (-1.0, None),
        (0.0, Some("0")),
        (1.0, None),
        (2.0, None),
        (3.0, Some("+3")),
    ] {
        let position = vu_position(value);
        let long = label.is_some();
        painter.line_segment(
            [
                point(position, radius - if long { 10.0 } else { 7.0 }),
                point(position, radius - 2.0),
            ],
            Stroke::new(1.0_f32, INK),
        );
        if let Some(label) = label {
            painter.text(
                point(position, radius - 17.0),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(9.5),
                INK,
            );
        }
    }
    painter.text(
        Pos2::new(face.center().x, face.bottom() - 11.0),
        Align2::CENTER_CENTER,
        "VU",
        FontId::proportional(11.0),
        INK,
    );
    painter.line_segment([pivot, point(position, radius)], Stroke::new(1.5_f32, INK));
    ui.painter().rect_stroke(
        face,
        4.0,
        Stroke::new(1.5_f32, INK),
        egui::StrokeKind::Outside,
    );
}

/// A socket for a patch cable.
fn jack(ui: &egui::Ui, center: Pos2) {
    ui.painter().circle_filled(center, 9.0, INK);
    ui.painter()
        .circle_stroke(center, 9.0, Stroke::new(2.0_f32, CREAM.gamma_multiply(0.6)));
    ui.painter().circle_filled(center, 3.5, Color32::BLACK);
}

/// A patch cable from one point to the next, with an arrow for the
/// direction and a metal plug at each end that has one.
fn cable(ui: &egui::Ui, from: f32, to: f32, y: f32, plugs: [bool; 2]) {
    let painter = ui.painter();
    painter.line_segment(
        [Pos2::new(from, y + 1.0), Pos2::new(to, y + 1.0)],
        Stroke::new(6.0_f32, Color32::from_black_alpha(120)),
    );
    painter.line_segment(
        [Pos2::new(from, y), Pos2::new(to, y)],
        Stroke::new(4.0_f32, INK),
    );
    painter.line_segment(
        [Pos2::new(from, y - 1.0), Pos2::new(to, y - 1.0)],
        Stroke::new(1.0_f32, CREAM.gamma_multiply(0.25)),
    );
    for (left, right) in [(from, from + 8.0), (to - 8.0, to)]
        .into_iter()
        .zip(plugs)
        .filter_map(|(ends, plug)| plug.then_some(ends))
    {
        let plug = Rect::from_min_max(Pos2::new(left, y - 4.5), Pos2::new(right, y + 4.5));
        painter.rect_filled(plug, 1.5, Color32::from_rgb(198, 194, 180));
        painter.rect_stroke(
            plug,
            1.5,
            Stroke::new(1.0_f32, INK),
            egui::StrokeKind::Inside,
        );
        painter.line_segment(
            [
                plug.left_top() + Vec2::new(1.5, 2.0),
                plug.right_top() + Vec2::new(-1.5, 2.0),
            ],
            Stroke::new(1.0_f32, Color32::from_white_alpha(170)),
        );
    }
    if to - from > 20.0 {
        let tip = Pos2::new((from + to) / 2.0 + 4.0, y - 10.0);
        painter.add(egui::Shape::convex_polygon(
            vec![
                tip + Vec2::new(-7.0, -4.0),
                tip + Vec2::new(1.0, 0.0),
                tip + Vec2::new(-7.0, 4.0),
            ],
            CREAM.gamma_multiply(0.7),
            Stroke::NONE,
        ));
    }
}

/// One pedal: header with name and lamp, gain-reduction lamps on dynamics
/// pedals, the dials, then the move buttons and the switch. Returns the
/// position a move button sends the pedal to.
fn pedal(
    ui: &mut egui::Ui,
    chain: &mut MasterChain,
    kind: PedalKind,
    rect: Rect,
    index: usize,
    metering: &Metering,
) -> Option<usize> {
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y);
    let on = chain.is_on(kind);
    enclosure(ui, rect, kind, on);
    heading(ui, rect, title(kind), family(kind), INK);
    lamp(ui, at(rect.width() - 18.0, 19.0), on);
    for y in [HEADER + 2.0, FOOTER_TOP - 2.0] {
        ui.painter().line_segment(
            [at(10.0, y), at(rect.width() - 10.0, y)],
            Stroke::new(1.0_f32, INK.gamma_multiply(0.35)),
        );
    }

    let (columns, rows) = grid(kind);
    let lamps = reduction_lamps(kind);
    let stack = lamps.map_or(0.0, |_| LAMP_ROW) + rows as f32 * ROW - 8.0;
    let top = HEADER + ((FOOTER_TOP - HEADER) - stack) / 2.0;
    let dials = dial_count(kind);
    let cell = |slot: usize| {
        let (column, row) = (slot % columns, slot / columns);
        // A short last row is centered under the one above.
        let in_row = if row + 1 == rows {
            dials - row * columns
        } else {
            columns
        };
        let left = (rect.width() - in_row as f32 * CELL) / 2.0;
        let x = left + (column as f32 + 0.5) * CELL;
        let y = top + lamps.map_or(0.0, |_| LAMP_ROW) + 22.0 + row as f32 * ROW;
        [at(x, y), at(x, y + 38.0), at(x, y + 60.0)]
    };
    if let Some(thresholds) = lamps {
        let reduction = match kind {
            PedalKind::BusCompressor => metering.reading.compressor_reduction,
            _ => metering.reading.limiter_reduction,
        };
        let reduction = if on && metering.playing {
            reduction
        } else {
            0.0
        };
        reduction_meter(
            ui,
            at(rect.width() / 2.0, top + 10.0),
            name(kind),
            reduction,
            &thresholds,
        );
    }
    // The dials sit on the enamel itself, which they read as a light surface.
    ui.scope(|ui| {
        ui.visuals_mut().dark_mode = false;
        for (slot, knob) in kind.knobs().iter().enumerate() {
            let (label, name, hint, format) = face(*knob);
            let before = decimal(knob.get(chain));
            let mut value = before;
            dial(
                ui,
                cell(slot),
                &mut value,
                Dial {
                    name: &name,
                    label: &label,
                    hint,
                    range: knob.range(),
                    format,
                    choices: &[],
                },
                on,
            );
            if value != before {
                knob.set(chain, value as f32);
            }
        }
        if kind == PedalKind::Tape {
            let mut speed = f64::from(chain.tape.speed);
            dial(
                ui,
                cell(1),
                &mut speed,
                Dial {
                    name: "Tape speed",
                    label: "Speed",
                    hint: "15 or 30 inches per second: moves the low-end bump and the treble roll-off.",
                    range: KnobRange {
                        low: 15.0,
                        high: 30.0,
                        default: 30.0,
                        step: 15.0,
                        logarithmic: false,
                    },
                    format: Format::Speed,
                    choices: &[15.0, 30.0],
                },
                on,
            );
            chain.tape.speed = if speed < 22.5 { 15 } else { 30 };
        }
    });

    let mut moved = None;
    // The limiter has no move buttons, and the pedal before it cannot pass it.
    let buttons = if kind.movable() {
        vec![
            (PEDAL_PADDING - 4.0, -1.0, "left", index > 0),
            (
                rect.width() - PEDAL_PADDING - 26.0,
                1.0,
                "right",
                chain
                    .order
                    .get(index + 1)
                    .is_some_and(|next| next.movable()),
            ),
        ]
    } else {
        Vec::new()
    };
    for (offset, direction, step, enabled) in buttons {
        let button = Rect::from_min_size(at(offset, FOOTER_TOP + 6.0), Vec2::new(30.0, 28.0));
        let label = format!("Move {} {step}", name(kind));
        let response = controls::button_at(
            ui,
            button,
            &label,
            "",
            controls::ButtonRole::Secondary,
            enabled,
        );
        arrow(ui, button.center(), direction, enabled);
        let response = reveal(name_control(response, &label)).on_hover_text(format!(
            "Move {} one place {step} in the signal path.",
            name(kind)
        ));
        if response.clicked() {
            moved = Some(if step == "left" { index - 1 } else { index + 1 });
        }
    }
    let mut switch = on;
    let response = controls::toggle_at(
        ui,
        Rect::from_center_size(
            at(rect.width() / 2.0 + 8.0, FOOTER_TOP + 20.0),
            Vec2::new(44.0, 24.0),
        ),
        name(kind),
        "",
        &mut switch,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            response.enabled(),
            switch,
            name(kind),
        )
    });
    reveal(response).on_hover_text(format!(
        "{}: {}. A pedal that is off keeps its settings and passes the signal on.",
        name(kind),
        if switch { "on" } else { "off" }
    ));
    chain.set_on(kind, switch);
    moved
}

/// Bring a control into view at once when it gains keyboard focus, so Tab
/// can walk the whole row.
fn reveal(response: Response) -> Response {
    if response.gained_focus() {
        response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
    }
    response
}

/// A row of lamps for the gain reduction of a dynamics pedal; each lights
/// once the reduction reaches its step.
fn reduction_meter(ui: &egui::Ui, center: Pos2, pedal: &str, reduction: f32, steps: &[f32; 6]) {
    let first = center.x - 2.5 * 14.0 + 10.0;
    ui.painter().text(
        Pos2::new(first - 22.0, center.y),
        Align2::CENTER_CENTER,
        "GR",
        FontId::proportional(10.5),
        INK,
    );
    for (index, step) in steps.iter().enumerate() {
        led(
            ui,
            Pos2::new(first + index as f32 * 14.0, center.y),
            4.0,
            reduction >= *step,
            AMBER,
        );
    }
    let response = ui.interact(
        Rect::from_center_size(center + Vec2::new(-6.0, 0.0), Vec2::new(112.0, 14.0)),
        ui.id().with(("reduction", pedal)),
        Sense::hover(),
    );
    named(
        &response,
        egui::WidgetType::Label,
        &format!("{pedal} gain reduction {reduction:.1} dB"),
    );
    response.on_hover_text(format!(
        "Gain reduction. The lamps light at {} dB.",
        steps.map(|step| step.to_string()).join(", ")
    ));
}

/// A small round lamp.
fn led(ui: &egui::Ui, center: Pos2, radius: f32, lit: bool, color: Color32) {
    let painter = ui.painter();
    if lit {
        painter.circle_filled(center, radius + 4.0, color.gamma_multiply(0.18));
    }
    painter.circle_filled(center, radius + 1.5, INK);
    painter.circle_filled(center, radius, if lit { color } else { DARK_LAMP });
    if lit {
        painter.circle_filled(
            center + Vec2::new(-1.2, -1.2),
            radius * 0.3,
            Color32::from_white_alpha(200),
        );
    }
}

/// The strip under the board: every pedal in signal order between the mixer
/// and the output, lit when on, with the part of the row in view outlined.
/// Clicking a pedal brings it into view. Returns the pedal clicked.
fn strip(ui: &mut egui::Ui, rect: Rect, chain: &MasterChain, visible: [f32; 2]) -> Option<usize> {
    controls::well(ui, rect);
    let chips_top = rect.top() + 8.0;
    let ends = 52.0;
    let gap = 5.0;
    let room = rect.width() - 2.0 * ends - 16.0;
    let width = ((room - gap * 12.0) / 13.0).clamp(34.0, 70.0);
    let total = 13.0 * width + 12.0 * gap;
    let left = rect.center().x - total / 2.0;
    for (text, x, align) in [
        ("Mixer", left - 8.0, Align2::RIGHT_CENTER),
        ("Output", left + total + 8.0, Align2::LEFT_CENTER),
    ] {
        ui.painter().text(
            Pos2::new(x, chips_top + 11.0),
            align,
            text,
            FontId::proportional(11.5),
            CREAM.gamma_multiply(0.8),
        );
    }
    let (lefts, end) = row_layout(&chain.order);
    let mut chosen = None;
    let mut seen: Option<Rect> = None;
    for (index, kind) in chain.order.iter().enumerate() {
        let chip = Rect::from_min_size(
            Pos2::new(left + index as f32 * (width + gap), chips_top),
            Vec2::new(width, 22.0),
        );
        let on = chain.is_on(*kind);
        let label = format!("Show {}", name(*kind));
        let response = ui.interact(chip, ui.id().with(("chip", *kind)), Sense::click());
        ui.painter().rect_filled(chip, 4.0, enamel(*kind, on));
        if response.hovered() {
            ui.painter()
                .rect_filled(chip, 4.0, Color32::from_white_alpha(30));
        }
        ui.painter().rect_stroke(
            chip,
            4.0,
            Stroke::new(1.0_f32, INK),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            chip.center(),
            Align2::CENTER_CENTER,
            short_name(*kind),
            FontId::proportional(11.0),
            INK,
        );
        if response.has_focus() {
            ui.painter().rect_stroke(
                chip.expand(2.0),
                5.0,
                Stroke::new(2.0_f32, controls::focus_color(ui)),
                egui::StrokeKind::Outside,
            );
        }
        named(&response, egui::WidgetType::Button, &label);
        let response = response.on_hover_text(format!(
            "{}: {}. Click to bring it into view.",
            name(*kind),
            if on { "on" } else { "off" }
        ));
        if response.clicked() {
            chosen = Some(index);
        }
        // The chips whose pedals are in view share an outline.
        let middle = (lefts[index] + pedal_width(*kind) / 2.0) / end;
        if (visible[0]..=visible[1]).contains(&middle) {
            seen = Some(seen.map_or(chip, |seen| seen.union(chip)));
        }
    }
    if let Some(seen) = seen {
        ui.painter().rect_stroke(
            seen.expand(3.0),
            6.0,
            Stroke::new(1.5_f32, CREAM),
            egui::StrokeKind::Outside,
        );
    }

    let on = chain
        .order
        .iter()
        .filter(|kind| chain.is_on(**kind))
        .count();
    let latency = chain.latency() as f32 / 44.1;
    let status = if chain.is_active() {
        let pedals = if on == 1 {
            "1 pedal on".to_owned()
        } else {
            format!("{on} of 13 pedals on")
        };
        let trim = if chain.trim != 0.0 {
            format!(", trim {:+.1} dB", chain.trim)
        } else {
            String::new()
        };
        let delay = if latency > 0.0 {
            format!(". Latency {latency:.1} ms, which exports remove.")
        } else {
            ".".to_owned()
        };
        format!("{pedals}{trim}{delay}")
    } else {
        "Every pedal is off and the trim is at 0 dB: the mix plays unprocessed.".to_owned()
    };
    let status_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 8.0, chips_top + 28.0),
        Pos2::new(rect.right() - 8.0, rect.bottom() - 4.0),
    );
    let response = ui.interact(status_rect, ui.id().with("master-status"), Sense::hover());
    named(&response, egui::WidgetType::Label, &status);
    let path: Vec<String> = chain
        .order
        .iter()
        .map(|kind| {
            if chain.is_on(*kind) {
                name(*kind).to_owned()
            } else {
                format!("{} (off)", name(*kind))
            }
        })
        .collect();
    response.on_hover_text(format!("Signal: mixer → {} → output.", path.join(" → ")));
    ui.painter().text(
        status_rect.center(),
        Align2::CENTER_CENTER,
        &status,
        FontId::proportional(12.0),
        CREAM,
    );
    chosen
}

/// The painted enclosure of each pedal. Like stompboxes, each kind has its
/// own enamel; it always appears with the pedal's printed name. The families
/// share hues: golds for equalisers, blues and greens for dynamics, warm
/// oranges for saturation, and violets for stereo. A pedal that is off
/// loses most of its color and some light, so the pedals in use stand out.
/// Ink lettering keeps at least 5:1 contrast in both states.
fn enamel(kind: PedalKind, on: bool) -> Color32 {
    let [red, green, blue] = match kind {
        PedalKind::EjayEq => [222, 172, 64],
        PedalKind::EjayCompressor => [84, 184, 166],
        PedalKind::EjayWide => [200, 150, 220],
        PedalKind::SoftClip => [240, 150, 120],
        PedalKind::Tube => [240, 166, 80],
        PedalKind::TrimEq => [196, 204, 92],
        PedalKind::BusCompressor => [120, 178, 228],
        PedalKind::Maximizer => [130, 206, 150],
        PedalKind::LiftEq => [236, 208, 108],
        PedalKind::Tape => [206, 180, 140],
        PedalKind::Imaging => [166, 160, 232],
        PedalKind::Multiband => [100, 196, 204],
        PedalKind::Limiter => [238, 140, 160],
    };
    if on {
        return Color32::from_rgb(red, green, blue);
    }
    let gray = 0.2126 * f32::from(red) + 0.7152 * f32::from(green) + 0.0722 * f32::from(blue);
    let toward = |channel: u8| {
        ((f32::from(channel) + (gray - f32::from(channel)) * 0.7) * 0.83).round() as u8
    };
    Color32::from_rgb(toward(red), toward(green), toward(blue))
}

/// The pedal's corner radius, the width of its rounded bevel, and how far its
/// side walls show below and to the right of the face.
const CORNER: f32 = 9.0;
const BEVEL: f32 = 6.0;
const DEPTH: f32 = 4.0;
/// Light falls on the board from the upper left, at 135 degrees, as it does
/// on the knob caps. This points from a pedal toward it.
const TO_LIGHT: Vec2 = Vec2::new(
    -std::f32::consts::FRAC_1_SQRT_2,
    -std::f32::consts::FRAC_1_SQRT_2,
);

/// How far the metal finish darkens the enamel at most: the grain's deepest
/// streak and the sheen's darker reflection. Ink keeps its contrast there.
const GRAIN_DIP: f32 = 0.05;
const SHEEN_DIP: f32 = 0.08;
/// The grain's typical depth, and the size of its tile in screen pixels.
const GRAIN_DEPTH: f32 = 0.02;
const GRAIN: [usize; 2] = [512, 256];
/// How far the sheen leans from upright, as a share of the pedal's height.
const SHEEN_LEAN: f32 = 0.2;
const ALUMINIUM: Color32 = Color32::from_rgb(236, 238, 240);

/// A die-cast enclosure under light from the upper left. The body stands off
/// the board: its side walls show below and to the right of the face, and it
/// casts its shadow that way. The rounded bevel catches the light along the
/// upper and left edges and falls into shade along the lower and right ones,
/// and the face lightens a little toward the light. Text sits inside the
/// bevel.
///
/// A pedal that is on shows its anodised aluminium: brushed grain, a metal
/// sheen, and scratches where the bare metal shows. One that is off stays
/// matte. The finish darkens the enamel by `GRAIN_DIP` and `SHEEN_DIP` at
/// most, where the ink still keeps its contrast.
fn enclosure(ui: &egui::Ui, rect: Rect, kind: PedalKind, on: bool) {
    let painter = ui.painter();
    let enamel = enamel(kind, on);
    let radius = egui::CornerRadius::same(CORNER as u8);
    let body = Rect::from_min_max(rect.min, rect.max + Vec2::splat(DEPTH));
    // A wide soft shadow, and a close dark one where the body meets the board.
    for (offset, blur, alpha) in [([5, 8], 16, 160), ([1, 2], 4, 150)] {
        painter.add(
            egui::epaint::Shadow {
                offset,
                blur,
                spread: 0,
                color: Color32::from_black_alpha(alpha),
            }
            .as_shape(body, radius),
        );
    }
    // The outline of the face swept down and right: the walls facing away
    // from the light show where the face does not cover them.
    let along = Vec2::splat(DEPTH);
    let mut outline = Vec::new();
    for (point, facing) in contour(rect, 0.0) {
        let away = facing.x + facing.y;
        if away > 1e-3 {
            outline.push(point + along);
        } else if away < -1e-3 {
            outline.push(point);
        } else if facing.x > 0.0 {
            outline.extend([point, point + along]);
        } else {
            outline.extend([point + along, point]);
        }
    }
    painter.add(egui::Shape::convex_polygon(
        outline.clone(),
        lit(enamel, -0.62),
        Stroke::NONE,
    ));
    let face = bevel(rect, enamel);
    if on {
        let finish = finish(ui.ctx());
        let seed = kind as u64 + 1;
        // The grain keeps one texel to a screen pixel, from a different
        // part of the tile on each pedal.
        let pixels = ui.ctx().pixels_per_point();
        let start = Vec2::new((seed * 97 % 512) as f32, (seed * 61 % 256) as f32);
        let grain = overlay(&face, finish.grain.id(), |point| {
            let texel = (point - rect.min) * pixels + start;
            Pos2::new(texel.x / GRAIN[0] as f32, texel.y / GRAIN[1] as f32)
        });
        let sheen = overlay(&face, finish.sheen.id(), |point| {
            let lean = SHEEN_LEAN * (rect.bottom() - point.y);
            let across =
                (point.x - rect.left() + lean) / (rect.width() + SHEEN_LEAN * rect.height());
            Pos2::new(across, 0.5)
        });
        painter.add(face);
        painter.add(grain);
        painter.add(sheen);
        // Scratches keep clear of the printed name.
        let below = rect.shrink(BEVEL + 3.0);
        let below =
            Rect::from_x_y_ranges(below.x_range(), rect.top() + HEADER + 4.0..=below.bottom());
        scratches(painter, below, seed);
    } else {
        painter.add(face);
    }
    painter.add(egui::Shape::closed_line(outline, Stroke::new(1.5_f32, INK)));
}

/// The metal finish shared by every pedal that is on.
struct Finish {
    grain: egui::TextureHandle,
    sheen: egui::TextureHandle,
}

fn finish(ctx: &egui::Context) -> Arc<Finish> {
    let id = egui::Id::new("pedal-metal-finish");
    if let Some(finish) = ctx.data(|data| data.get_temp::<Arc<Finish>>(id)) {
        return finish;
    }
    let finish = Arc::new(Finish {
        grain: ctx.load_texture(
            "pedal-grain",
            grain_image(),
            egui::TextureOptions::NEAREST_REPEAT,
        ),
        sheen: ctx.load_texture("pedal-sheen", sheen_image(), egui::TextureOptions::LINEAR),
    });
    ctx.data_mut(|data| data.insert_temp(id, finish.clone()));
    finish
}

/// A texel that lightens what lies under it by a positive amount, adding
/// light, or darkens it by a negative one.
fn light_texel(amount: f32) -> Color32 {
    let level = (amount.abs().min(1.0) * 255.0).round() as u8;
    if amount > 0.0 {
        Color32::from_rgba_premultiplied(level, level, level, 0)
    } else {
        Color32::from_rgba_premultiplied(0, 0, 0, level)
    }
}

/// Brushed aluminium: fine streaks along the rows, each row a little lighter
/// or darker than the next and changing slowly along its length. The tile
/// repeats without a seam.
fn grain_image() -> egui::ColorImage {
    const KNOT: usize = 32;
    let [width, height] = GRAIN;
    let mut random = Random(0x6d61_7374_6572);
    let mut values = Vec::with_capacity(width * height);
    for _ in 0..height {
        let row = random.normal();
        let knots: Vec<f32> = (0..width / KNOT).map(|_| random.normal()).collect();
        for x in 0..width {
            let (knot, part) = (x / KNOT, (x % KNOT) as f32 / KNOT as f32);
            let blend = (1.0 - (part * std::f32::consts::PI).cos()) / 2.0;
            let next = knots[(knot + 1) % knots.len()];
            let along = knots[knot] + (next - knots[knot]) * blend;
            values.push(0.6 * row + 0.45 * along + 0.25 * random.normal());
        }
    }
    let deviation =
        (values.iter().map(|value| value * value).sum::<f32>() / values.len() as f32).sqrt();
    let pixels = values
        .iter()
        .map(|value| light_texel((value / deviation * GRAIN_DEPTH).clamp(-GRAIN_DIP, GRAIN_DIP)))
        .collect();
    egui::ColorImage::new(GRAIN, pixels)
}

/// The sheen across a pedal from its left edge to its right. Brushed metal
/// stretches a highlight across its grain: a bright streak toward the light,
/// a darker reflection, then a fainter streak.
fn sheen(across: f32) -> f32 {
    let bump = |centre: f32, width: f32| (-((across - centre) / width).powi(2)).exp();
    0.09 * bump(0.24, 0.16) + 0.13 * bump(0.24, 0.05) + 0.06 * bump(0.74, 0.07)
        - SHEEN_DIP * bump(0.5, 0.14)
}

fn sheen_image() -> egui::ColorImage {
    const TEXELS: usize = 256;
    let pixels = (0..TEXELS)
        .map(|texel| light_texel(sheen((texel as f32 + 0.5) / TEXELS as f32)))
        .collect();
    egui::ColorImage::new([TEXELS, 1], pixels)
}

/// The face's mesh again, painted with a texture laid across it.
fn overlay(face: &egui::Mesh, texture: egui::TextureId, uv: impl Fn(Pos2) -> Pos2) -> egui::Mesh {
    let mut mesh = egui::Mesh::with_texture(texture);
    mesh.indices.clone_from(&face.indices);
    mesh.vertices = face
        .vertices
        .iter()
        .map(|vertex| egui::epaint::Vertex {
            pos: vertex.pos,
            uv: uv(vertex.pos),
            color: Color32::WHITE,
        })
        .collect();
    mesh
}

/// Hairline scratches through the anodising, where the bare aluminium shows.
/// Each pedal keeps its own, seeded by its kind, so they stay with it as it
/// moves. Most are short and run in the pedal's few usual directions, some
/// in pairs or threes; a few are long. In each groove the wall toward the
/// light falls dark and the far wall catches it, brightest on scratches
/// across the light.
fn scratches(painter: &egui::Painter, face: Rect, seed: u64) {
    let mut random = Random(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let usual = [random.unit(), random.unit()].map(|turn| turn * std::f32::consts::PI);
    let count = 8 + (face.area() / 4500.0) as usize;
    for _ in 0..count {
        let heading = if random.unit() < 0.3 {
            random.unit() * std::f32::consts::PI
        } else {
            usual[usize::from(random.unit() < 0.5)] + (random.unit() - 0.5) * 0.4
        };
        let length = if random.unit() < 0.2 {
            40.0 + 90.0 * random.unit()
        } else {
            5.0 + 25.0 * random.unit()
        };
        let depth = 0.35 + 0.65 * random.unit().powi(2);
        let bend = (random.unit() - 0.5) * 0.5;
        let start = face.min + Vec2::new(random.unit(), random.unit()) * face.size();
        let siblings = if random.unit() < 0.3 {
            1 + usize::from(random.unit() < 0.5)
        } else {
            0
        };
        let gap = 1.5 + 1.5 * random.unit();
        for sibling in 0..=siblings {
            let shift = Vec2::angled(heading + std::f32::consts::FRAC_PI_2) * gap * sibling as f32;
            let share = 1.0 - 0.25 * sibling as f32;
            scratch(
                painter,
                face,
                start + shift,
                heading,
                length * share,
                bend,
                depth * share,
            );
        }
    }
}

/// One scratch along a gentle curve, fading in and out at its ends. It stops
/// where it would leave the face.
fn scratch(
    painter: &egui::Painter,
    face: Rect,
    start: Pos2,
    heading: f32,
    length: f32,
    bend: f32,
    depth: f32,
) {
    const SEGMENTS: usize = 5;
    let mut points = vec![start];
    for segment in 0..SEGMENTS {
        let turn = heading + bend * (segment as f32 / SEGMENTS as f32 - 0.5);
        let next = points[segment] + Vec2::angled(turn) * (length / SEGMENTS as f32);
        if !face.contains(next) {
            break;
        }
        points.push(next);
    }
    if points.len() < 3 {
        return;
    }
    let glint = 0.55 + 0.45 * Vec2::angled(heading).rot90().dot(TO_LIGHT).abs();
    let last = (points.len() - 1) as f32;
    for (index, pair) in points.windows(2).enumerate() {
        let middle = (index as f32 + 0.5) / last;
        let fade = (middle * std::f32::consts::PI).sin().sqrt() * depth;
        let [from, to] = [pair[0], pair[1]];
        let shadow = TO_LIGHT * 0.6;
        painter.line_segment(
            [from + shadow, to + shadow],
            Stroke::new(0.8_f32, INK.gamma_multiply(0.35 * fade)),
        );
        painter.line_segment(
            [from - TO_LIGHT * 0.2, to - TO_LIGHT * 0.2],
            Stroke::new(0.8_f32, ALUMINIUM.gamma_multiply(0.9 * fade * glint)),
        );
    }
}

/// A small seeded generator, so the grain and each pedal's scratches come out
/// the same on every frame and every run.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    /// Uniform in [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1_u64 << 24) as f32
    }

    /// Standard normal.
    fn normal(&mut self) -> f32 {
        let radius = (-2.0 * (1.0 - self.unit()).ln()).sqrt();
        radius * (std::f32::consts::TAU * self.unit()).cos()
    }
}

/// The face as one mesh: rings from the edge in to the level face, then a
/// fan across it. Light on the bevel follows how steep it is at each ring and
/// which way that stretch of edge faces; the face brightens a little toward
/// the upper left.
fn bevel(rect: Rect, enamel: Color32) -> egui::Mesh {
    const RINGS: usize = 4;
    let span = rect.width() + rect.height();
    let shade = |point: Pos2, light: f32| {
        let sheen = 0.07 * (1.0 - (point.x - rect.left() + point.y - rect.top()) / span);
        lit(enamel, sheen + light)
    };
    let mut mesh = egui::Mesh::default();
    let mut around = 0;
    for ring in 0..=RINGS {
        let inward = ring as f32 / RINGS as f32;
        // A rounded edge: steep at the outline, level where it meets the face.
        let slope = (1.0 - inward).powf(1.1);
        let points = contour(rect, BEVEL * inward);
        around = points.len();
        for (point, facing) in points {
            let facing = facing.dot(TO_LIGHT);
            let light = slope * if facing > 0.0 { 0.68 } else { 0.6 } * facing;
            mesh.colored_vertex(point, shade(point, light));
        }
    }
    let ring = |ring: usize, index: usize| (ring * around + index % around) as u32;
    for outer in 0..RINGS {
        for index in 0..around {
            let [a, b] = [ring(outer, index), ring(outer, index + 1)];
            let [c, d] = [ring(outer + 1, index), ring(outer + 1, index + 1)];
            mesh.add_triangle(a, b, d);
            mesh.add_triangle(a, d, c);
        }
    }
    let centre = mesh.vertices.len() as u32;
    mesh.colored_vertex(rect.center(), shade(rect.center(), 0.0));
    for index in 0..around {
        mesh.add_triangle(centre, ring(RINGS, index), ring(RINGS, index + 1));
    }
    mesh
}

/// Points around the pedal's rounded outline, moved in by `inset`, each with
/// the outward direction it faces, clockwise from the lower right corner.
/// Each corner turns in even steps, so one point there faces diagonally.
fn contour(rect: Rect, inset: f32) -> Vec<(Pos2, Vec2)> {
    const STEPS: usize = 8;
    let centres = [
        rect.right_bottom() + Vec2::new(-CORNER, -CORNER),
        rect.left_bottom() + Vec2::new(CORNER, -CORNER),
        rect.left_top() + Vec2::new(CORNER, CORNER),
        rect.right_top() + Vec2::new(-CORNER, CORNER),
    ];
    let mut points = Vec::with_capacity(4 * (STEPS + 1));
    for (corner, centre) in centres.into_iter().enumerate() {
        for step in 0..=STEPS {
            let turn = corner as f32 + step as f32 / STEPS as f32;
            let facing = Vec2::angled(turn * std::f32::consts::FRAC_PI_2);
            points.push((centre + facing * (CORNER - inset), facing));
        }
    }
    points
}

/// The enamel as the light finds it: a positive amount lightens it toward
/// cream, a negative one darkens it.
fn lit(enamel: Color32, amount: f32) -> Color32 {
    let toward = if amount > 0.0 { CREAM } else { Color32::BLACK };
    let mix = |from: u8, to: u8| {
        (f32::from(from) + (f32::from(to) - f32::from(from)) * amount.abs().min(1.0)).round() as u8
    };
    Color32::from_rgb(
        mix(enamel.r(), toward.r()),
        mix(enamel.g(), toward.g()),
        mix(enamel.b(), toward.b()),
    )
}

/// The pedal's lamp. Lit, it glows in the application's active lime.
fn lamp(ui: &egui::Ui, center: Pos2, on: bool) {
    let painter = ui.painter();
    if on {
        for (radius, alpha) in [(12.0, 28), (9.0, 54)] {
            painter.circle_filled(center, radius, LIME.gamma_multiply(alpha as f32 / 255.0));
        }
    }
    painter.circle_filled(center, 6.5, INK);
    painter.circle_filled(center, 4.5, if on { LIME } else { DARK_LAMP });
    if on {
        painter.circle_filled(
            center + Vec2::new(-1.5, -1.5),
            1.5,
            Color32::from_white_alpha(200),
        );
    }
}

/// A drawn arrow for the move buttons.
fn arrow(ui: &egui::Ui, center: Pos2, direction: f32, enabled: bool) {
    let color = if enabled {
        CREAM
    } else {
        CREAM.gamma_multiply(0.35)
    };
    let stroke = Stroke::new(1.8_f32, color);
    let tip = center + Vec2::new(5.0 * direction, 0.0);
    ui.painter()
        .line_segment([center - Vec2::new(5.0 * direction, 0.0), tip], stroke);
    for rise in [-4.0, 4.0] {
        ui.painter()
            .line_segment([tip, tip + Vec2::new(-4.0 * direction, rise)], stroke);
    }
}

/// A setting as the decimal it was written as: `0.1_f32` is 0.1, not the
/// binary value just above it, so grid steps count true.
fn decimal(value: f32) -> f64 {
    (f64::from(value) * 1e6).round() / 1e6
}

/// The value on the knob's grid nearest to `value`, or one step on in the
/// direction it moved from `before`, so a fine step still moves one.
fn on_grid(value: f64, before: Option<f64>, range: &KnobRange) -> f64 {
    let (low, high, step) = (decimal(range.low), decimal(range.high), decimal(range.step));
    let steps = (value - low) / step;
    let steps = match before {
        Some(before) if value > before => (steps - 1e-4).ceil(),
        Some(before) if value < before => (steps + 1e-4).floor(),
        _ => steps.round(),
    };
    let decimals = if step < 1.0 { 10f64.powi(2) } else { 1.0 };
    ((low + steps * step).clamp(low, high) * decimals).round() / decimals
}

/// A rotary with its value window and label. The value moves on the knob's
/// grid; the window takes typed values with or without the unit.
fn dial(ui: &mut egui::Ui, positions: [Pos2; 3], value: &mut f64, spec: Dial<'_>, enabled: bool) {
    let [center, well, label] = positions;
    let range = spec.range;
    ui.painter().text(
        label,
        Align2::CENTER_CENTER,
        spec.label,
        FontId::proportional(11.5),
        if ui.visuals().dark_mode { CREAM } else { INK },
    );
    controls::well(ui, Rect::from_center_size(well, WELL));
    let (low, high) = (decimal(range.low), decimal(range.high));
    let hint = format!(
        "{}: {} to {}. {}",
        spec.name,
        spec.format.show(low),
        spec.format.show(high),
        if spec.hint.is_empty() {
            "Drag, or click the value to enter one."
        } else {
            spec.hint
        }
    );
    ui.add_enabled_ui(enabled, |ui| {
        let before = *value;
        let response = controls::rotary(
            ui,
            Rect::from_center_size(center, Vec2::splat(44.0)),
            value,
            controls::RotarySpec {
                name: spec.name,
                low,
                high,
                reset: decimal(range.default),
                step: decimal(range.step),
                decimals: if range.step < 1.0 { 1 } else { 0 },
                logarithmic: range.logarithmic,
                choices: spec.choices,
            },
        );
        reveal(response).on_hover_text(&hint);
        if spec.choices.is_empty() && *value != before {
            *value = on_grid(*value, Some(before), &range);
        }
        ui.scope(|ui| {
            controls::value_style(ui);
            let typed = *value;
            let format = spec.format;
            let entry = egui::DragValue::new(value)
                .range(low..=high)
                .speed(decimal(range.step))
                .custom_formatter(move |value, _| format.show(value))
                .custom_parser(move |text| format.read(text, high));
            let response = ui.put(
                Rect::from_center_size(well, WELL + Vec2::new(-2.0, 4.0)),
                entry,
            );
            reveal(name_control(response, &format!("{} value", spec.name))).on_hover_text(&hint);
            if *value != typed {
                *value = if spec.choices.is_empty() {
                    on_grid(*value, None, &range)
                } else if *value < (low + high) / 2.0 {
                    low
                } else {
                    high
                };
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_stays_legible_on_every_enamel_lit_or_off() {
        for kind in PedalKind::ALL {
            for on in [true, false] {
                // A pedal that is on is checked where its finish is darkest.
                let darkest = if on {
                    lit(enamel(kind, on), -(GRAIN_DIP + SHEEN_DIP))
                } else {
                    enamel(kind, on)
                };
                let ratio = controls::contrast(darkest, INK);
                assert!(ratio >= 5.0, "{} ({on}) gives {ratio:.2}:1", name(kind));
            }
        }
    }

    #[test]
    fn the_metal_finish_is_brushed_along_the_rows_and_keeps_within_its_dip() {
        let [width, height] = GRAIN;
        let amounts: Vec<f32> = grain_image()
            .pixels
            .iter()
            .map(|texel| (f32::from(texel.r()) - f32::from(texel.a())) / 255.0)
            .collect();
        assert!(
            amounts
                .iter()
                .all(|amount| amount.abs() <= GRAIN_DIP + 0.5 / 255.0)
        );
        let variance = |means: &[f32]| {
            let mean = means.iter().sum::<f32>() / means.len() as f32;
            means
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f32>()
                / means.len() as f32
        };
        let rows: Vec<f32> = amounts
            .chunks(width)
            .map(|row| row.iter().sum::<f32>() / width as f32)
            .collect();
        let columns: Vec<f32> = (0..width)
            .map(|x| (0..height).map(|y| amounts[y * width + x]).sum::<f32>() / height as f32)
            .collect();
        assert!(variance(&rows) > 20.0 * variance(&columns));
        assert!((0..=100).all(|step| sheen(step as f32 / 100.0) >= -SHEEN_DIP));
    }

    #[test]
    fn values_read_in_their_units_and_typed_units_are_optional() {
        assert_eq!(Format::db(true).show(1.0), "+1.0 dB");
        assert_eq!(Format::db(false).show(-0.0), "0.0 dB");
        assert_eq!(Format::whole("%").show(105.0), "105%");
        assert_eq!(Format::tenths(":1").show(2.0), "2.0:1");
        assert_eq!(Format::tenths("").show(2.5), "2.5");
        assert_eq!(Format::Speed.show(30.0), "30 ips");
        assert_eq!(Format::Band(4).show(50.0), "+0.0");
        assert_eq!(Format::db(false).read("-3 dB", 0.0), Some(-3.0));
        assert_eq!(Format::db(false).read("-1.5db", 0.0), Some(-1.5));
        assert_eq!(Format::whole("Hz").read(" 120 ", 300.0), Some(120.0));
        assert_eq!(Format::Band(4).read("+18", 100.0), Some(100.0));
        assert_eq!(Format::whole("ms").read("fast", 1200.0), None);
    }

    #[test]
    fn knobs_move_whole_steps_of_their_own_size() {
        let trim = Knob::Trim.range();
        // A tenth of a decibel stays a tenth, however far from the low end.
        let mut value = 0.0;
        for expected in [-0.1, -0.2, -0.3] {
            let next = value - 0.100_000_001_49;
            value = on_grid(next, Some(value), &trim);
            assert_eq!(value, expected);
        }
        // A fine step still moves one step; a typed value goes to the nearest.
        assert_eq!(on_grid(0.01, Some(0.0), &trim), 0.1);
        assert_eq!(on_grid(1.23, None, &trim), 1.2);
        let release = Knob::CompressorRelease.range();
        assert_eq!(on_grid(301.0, Some(300.0), &release), 310.0);
        assert_eq!(on_grid(1500.0, None, &release), 1200.0);
        assert_eq!(decimal(0.1), 0.1);
    }

    #[test]
    fn the_vu_scale_spreads_by_voltage() {
        assert_eq!(vu_position(f32::NEG_INFINITY), 0.0);
        assert_eq!(vu_position(-20.0), 0.0);
        assert!((vu_position(0.0) - 0.685).abs() < 0.01);
        assert!((vu_position(3.0) - 1.0).abs() < 1e-6);
        assert!(vu_position(12.0) <= 1.03);
    }
}
