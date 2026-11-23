//! The song and layout edits that a UI capture view starts from. Only a capture
//! run (`MIXJAM_TECHNO_CAPTURE_TO` with a view name) reaches this code; a normal
//! start never edits the session here.
use super::*;

/// Prepare `session` for the capture `view`: the playhead, the layout of the
/// panels, and the song edits that the view shows.
pub(super) fn prepare_session(session: &mut Session, view: &str) {
    if view.is_empty() {
        return;
    }
    if view == "follow" {
        session.seek(40 * TICKS_PER_BAR);
    }
    // Captures run in their own data folder, so the saved layout stays there.
    // Every capture sets the whole layout, so one view never inherits another's
    // panels.
    let mut layout = session.layout();
    layout.tool_names = view != "rail-icons";
    // The Mix view folds the lower panel, as it does when chosen.
    layout.boards = view != "boards-folded" && !view.starts_with("mix-view");
    layout.maximised = view == "maximised";
    layout.sounds = matches!(view, "sounds-panel" | "sample-details");
    layout.overview = view != "overview-folded";
    layout.inspector = view.starts_with("inspector");
    if let Err(error) = session.set_layout(layout) {
        eprintln!("{error}");
    }
    master_board(session, view);
    if matches!(view, "mixer-eq" | "mix-view-eq") {
        tamed_hi_hats(session);
    }
    if view == "unsaved" {
        let bpm = if session.project().bpm == 160.0 {
            140.0
        } else {
            160.0
        };
        let _ = session.edit(EditCommand::SetTempo(bpm));
    }
}

const MASTERING_EDITORS: &[(&str, PedalKind)] = &[
    ("master-deep-edit", PedalKind::SoftClip),
    ("master-deep-edit-eq", PedalKind::BoosterEq),
    ("master-deep-edit-trim", PedalKind::TrimEq),
    ("master-deep-edit-lift", PedalKind::LiftEq),
    ("master-deep-edit-imaging", PedalKind::Imaging),
    ("master-deep-edit-tape", PedalKind::Tape),
    ("master-deep-edit-comp", PedalKind::BusCompressor),
    ("master-deep-edit-multiband", PedalKind::Multiband),
    ("master-deep-edit-limiter", PedalKind::Limiter),
];

pub(super) fn mastering_editor(view: &str) -> Option<PedalKind> {
    MASTERING_EDITORS
        .iter()
        .find(|(name, _)| *name == view)
        .map(|(_, kind)| *kind)
}

/// The Booster settings of a D4 factory song, and every mastering pedal at its
/// default.
fn master_board(session: &mut Session, view: &str) {
    let editor = mastering_editor(view);
    if editor.is_none()
        && !matches!(
            view,
            "master-active"
                | "master-reordered"
                | "master-end"
                | "master-metering"
                | "master-bypass"
        )
    {
        return;
    }
    let mut chain = session.project().master;
    for kind in PedalKind::ALL {
        chain.set_on(kind, true);
    }
    chain.trim = -3.0;
    chain.booster_eq.bands = [62, 58, 50, 44, 50, 54, 60, 64, 66, 70];
    (
        chain.booster_compressor.drive,
        chain.booster_compressor.speed,
    ) = (1000, 1000);
    chain.booster_compressor.gain = 278;
    chain.booster_wide.spread = 444;
    apply_master_capture_case(&mut chain, editor, view);
    let _ = session.edit(EditCommand::SetMaster(chain));
}

fn apply_editor_capture_preset(chain: &mut MasterChain, editor: PedalKind) {
    match editor {
        PedalKind::SoftClip => {
            chain.soft_clip.amount = 6.0;
            chain.soft_clip.ceiling = -1.0;
        }
        PedalKind::TrimEq => {
            chain.trim_eq.high_pass = 45.0;
            chain.trim_eq.mud = -2.5;
            chain.trim_eq.harsh = -3.0;
        }
        PedalKind::LiftEq => {
            chain.lift_eq.low = 1.5;
            chain.lift_eq.air = 2.0;
        }
        PedalKind::Imaging => {
            chain.imaging.width = 135.0;
            chain.imaging.mono_below = 120.0;
        }
        PedalKind::Tape => {
            chain.tape.drive = 6.5;
            chain.tape.speed = 15;
        }
        other => apply_dynamics_capture_preset(chain, other),
    }
}

fn apply_dynamics_capture_preset(chain: &mut MasterChain, editor: PedalKind) {
    if editor == PedalKind::BusCompressor {
        chain.bus_compressor.threshold = -11.0;
        chain.bus_compressor.ratio = 2.5;
        chain.bus_compressor.sc_hpf_hz = 90.0;
        chain.bus_compressor.mix = 85.0;
        chain.bus_compressor.makeup_db = 0.8;
    } else if editor == PedalKind::Multiband {
        chain.multiband.low = 55.0;
        chain.multiband.mid = 45.0;
        chain.multiband.high = 50.0;
    } else if editor == PedalKind::Limiter {
        chain.limiter.gain = 3.0;
        chain.limiter.ceiling = -1.0;
        chain.limiter.style = mixjam_techno::application::LimiterStyle::Modern;
    }
}

fn apply_master_capture_case(chain: &mut MasterChain, editor: Option<PedalKind>, view: &str) {
    if let Some(kind) = editor {
        apply_editor_capture_preset(chain, kind);
    } else if view == "master-reordered" {
        chain.move_pedal(9, 0);
        chain.move_pedal(3, 5);
        chain.booster_compressor.on = false;
        chain.tube.on = false;
    } else if view == "master-bypass" {
        chain.bypass = true;
    }
}

/// Hi-hats tamed: a low cut, a dip at the harsh band, and less top.
fn tamed_hi_hats(session: &mut Session) {
    let mut tracks = session.project().tracks.clone();
    tracks[0].eq = mixjam_techno::application::eq::ChannelEq {
        on: true,
        low_cut: 180.0,
        mid_hz: 3500.0,
        mid_db: -4.5,
        mid_q: 1.6,
        high_db: -9.0,
        ..Default::default()
    };
    let master_gain = session.project().master_gain;
    let _ = session.edit(EditCommand::SetMixer {
        tracks,
        master_gain,
    });
}
