//! Board geometry and the fixed facts about each pedal: its names, knob grid,
//! lamps, and width, and where each pedal sits along the row.
use super::*;

pub(super) const MARGIN: f32 = 8.0;
pub(super) const INPUT_WIDTH: f32 = 136.0;
pub(super) const OUTPUT_WIDTH: f32 = 180.0;
pub(super) const CABLE: f32 = 22.0;
pub(super) const PEDAL_TOP: f32 = 8.0;
pub(super) const PEDAL_HEIGHT: f32 = 228.0;
pub(super) const HEADER: f32 = 36.0;
pub(super) const FOOTER_TOP: f32 = 190.0;
/// The height of the cables and sockets, through the middle of the pedals.
pub(super) const SIGNAL_Y: f32 = PEDAL_TOP + PEDAL_HEIGHT / 2.0;
/// One knob with its value well and label.
pub(super) const CELL: f32 = 64.0;
pub(super) const PEDAL_PADDING: f32 = 14.0;
/// Room under the pedals for their shadows and the row's scroll bar.
pub(super) const ROW_HEIGHT: f32 = PEDAL_TOP + PEDAL_HEIGHT + 12.0;
/// One row of chips under the pedals and their shadows.
pub(super) const STRIP_HEIGHT: f32 = 30.0;
pub(super) const BOARD_HEIGHT: f32 = ROW_HEIGHT + STRIP_HEIGHT;
pub(crate) const WELL: Vec2 = Vec2::new(56.0, 20.0);

pub(super) fn name(kind: PedalKind) -> &'static str {
    match kind {
        PedalKind::BoosterEq => "Booster EQ",
        PedalKind::BoosterCompressor => "Booster compressor",
        PedalKind::BoosterWide => "Booster wide",
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
pub(super) fn title(kind: PedalKind) -> &'static str {
    match kind {
        PedalKind::BoosterEq => "EQ",
        PedalKind::BoosterCompressor => "Compressor",
        PedalKind::BoosterWide => "Stereo wide",
        _ => name(kind),
    }
}

/// The pedal's name on the strip below the board.
pub(super) fn short_name(kind: PedalKind) -> &'static str {
    match kind {
        PedalKind::BoosterEq => "EQ",
        PedalKind::BoosterCompressor => "Comp",
        PedalKind::BoosterWide => "Wide",
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

const HERO_KNOBS: [(PedalKind, &[Knob]); 13] = [
    (PedalKind::BoosterEq, &[]),
    (
        PedalKind::BoosterCompressor,
        &[Knob::BoosterDrive, Knob::BoosterSpeed, Knob::BoosterGain],
    ),
    (PedalKind::BoosterWide, &[Knob::BoosterSpread]),
    (
        PedalKind::SoftClip,
        &[Knob::SoftClipAmount, Knob::SoftClipCeiling],
    ),
    (PedalKind::Tube, &[Knob::TubeDrive, Knob::TubeMix]),
    (
        PedalKind::TrimEq,
        &[Knob::TrimEqHighPass, Knob::TrimEqMud, Knob::TrimEqHarsh],
    ),
    (
        PedalKind::BusCompressor,
        &[
            Knob::CompressorThreshold,
            Knob::CompressorRatio,
            Knob::CompressorAttack,
        ],
    ),
    (PedalKind::Maximizer, &[Knob::MaximizerBoost]),
    (PedalKind::LiftEq, &[Knob::LiftLow, Knob::LiftAir]),
    (PedalKind::Tape, &[Knob::TapeDrive]),
    (
        PedalKind::Imaging,
        &[Knob::ImagingWidth, Knob::ImagingMonoBelow],
    ),
    (
        PedalKind::Multiband,
        &[Knob::MultibandLow, Knob::MultibandMid, Knob::MultibandHigh],
    ),
    (
        PedalKind::Limiter,
        &[Knob::LimiterGain, Knob::LimiterCeiling],
    ),
];

/// Hero dials shown on a pedal's compact lower-board card (1–3 per card).
pub(super) fn hero_knobs(kind: PedalKind) -> &'static [Knob] {
    HERO_KNOBS
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or(&[], |(_, knobs)| *knobs)
}

/// The dials on a compact pedal card: its hero knobs, plus tape speed.
pub(super) fn card_dial_count(kind: PedalKind) -> usize {
    hero_knobs(kind).len() + usize::from(kind == PedalKind::Tape)
}

/// The reductions in dB at which a dynamics pedal's lamps light.
pub(super) fn reduction_lamps(kind: PedalKind) -> Option<[f32; 6]> {
    match kind {
        PedalKind::BusCompressor => Some([0.5, 1.0, 1.5, 2.0, 3.0, 4.0]),
        PedalKind::Limiter => Some([0.5, 1.0, 2.0, 3.0, 4.5, 6.0]),
        _ => None,
    }
}

pub(super) fn pedal_width(kind: PedalKind) -> f32 {
    if kind == PedalKind::BoosterEq {
        return 184.0;
    }
    let cols = card_dial_count(kind).max(1);
    (cols as f32 * CELL + 2.0 * PEDAL_PADDING).max(184.0)
}

/// Each pedal's left edge along the row, after its cable, and the end of
/// the last cable.
pub(super) fn row_layout(order: &[PedalKind]) -> (Vec<f32>, f32) {
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

/// The gap between the board's painted edge and its content.
pub(super) const BOARD_INSET: f32 = 2.0;
/// The lower panel's height that shows the whole board: the board, and the
/// panel's margins, module toolbar, and the spacing under the toolbar.
pub(crate) const MODULE_HEIGHT: f32 = BOARD_HEIGHT + 2.0 * BOARD_INSET + 64.0;
