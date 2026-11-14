//! Board geometry and the fixed facts about each pedal: its names, knob grid,
//! lamps, and width, and where each pedal sits along the row.
use super::*;

pub(super) const MARGIN: f32 = 8.0;
pub(super) const INPUT_WIDTH: f32 = 150.0;
pub(super) const OUTPUT_WIDTH: f32 = 200.0;
pub(super) const CABLE: f32 = 26.0;
pub(super) const PEDAL_TOP: f32 = 10.0;
pub(super) const PEDAL_HEIGHT: f32 = 304.0;
pub(super) const HEADER: f32 = 36.0;
pub(super) const FOOTER_TOP: f32 = 260.0;
/// The height of the cables and sockets, through the middle of the pedals.
pub(super) const SIGNAL_Y: f32 = PEDAL_TOP + 150.0;
/// One knob with its value well and label, and the pitch of knob rows.
pub(super) const CELL: f32 = 62.0;
pub(super) const ROW: f32 = 96.0;
pub(super) const PEDAL_PADDING: f32 = 14.0;
/// Room under the pedals for their shadows and the row's scroll bar.
pub(super) const ROW_HEIGHT: f32 = PEDAL_TOP + PEDAL_HEIGHT + 20.0;
/// One row of chips under the pedals and their shadows.
pub(super) const STRIP_HEIGHT: f32 = 30.0;
pub(super) const BOARD_HEIGHT: f32 = ROW_HEIGHT + STRIP_HEIGHT;
/// The gain-reduction lamps above a dynamics pedal's knobs.
pub(super) const LAMP_ROW: f32 = 28.0;
pub(super) const WELL: Vec2 = Vec2::new(56.0, 20.0);

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

/// The line under the pedal's name: where it comes from or what it does.
pub(super) fn family(kind: PedalKind) -> &'static str {
    match kind {
        PedalKind::BoosterEq | PedalKind::BoosterCompressor | PedalKind::BoosterWide => "Booster",
        PedalKind::SoftClip | PedalKind::Tube | PedalKind::Tape => "Saturation",
        PedalKind::TrimEq | PedalKind::LiftEq => "Equaliser",
        PedalKind::Imaging => "Stereo",
        PedalKind::Limiter => "Dynamics, always last",
        _ => "Dynamics",
    }
}

/// Knob columns and rows of a pedal.
pub(super) fn grid(kind: PedalKind) -> (usize, usize) {
    match kind {
        PedalKind::BoosterEq => (5, 2),
        PedalKind::BoosterWide | PedalKind::Maximizer => (1, 1),
        PedalKind::BusCompressor => (3, 2),
        PedalKind::BoosterCompressor
        | PedalKind::TrimEq
        | PedalKind::Multiband
        | PedalKind::Limiter => (2, 2),
        _ => (2, 1),
    }
}

/// The dials on a pedal's face: its knobs, and the tape's speed selector.
pub(super) fn dial_count(kind: PedalKind) -> usize {
    kind.knobs().len() + usize::from(kind == PedalKind::Tape)
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
    let (columns, _) = grid(kind);
    (columns as f32 * CELL + 2.0 * PEDAL_PADDING).max(132.0)
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
