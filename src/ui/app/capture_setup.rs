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
    if matches!(
        view,
        "master-active" | "master-reordered" | "master-end" | "master-metering" | "master-bypass"
    ) {
        master_board(session, view);
    }
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

/// The Booster settings of a D4 factory song, and every mastering pedal at its
/// default.
fn master_board(session: &mut Session, view: &str) {
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
    if view == "master-reordered" {
        chain.move_pedal(9, 0);
        chain.move_pedal(3, 5);
        chain.booster_compressor.on = false;
        chain.tube.on = false;
    }
    if view == "master-bypass" {
        chain.bypass = true;
    }
    let _ = session.edit(EditCommand::SetMaster(chain));
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
