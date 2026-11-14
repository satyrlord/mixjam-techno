//! UI behavior tests that drive the window through its accessible controls, grouped by
//! the part of the window they exercise. Shared drivers live in `helpers`, and fixtures
//! and assertions that several groups use live in `scenarios`.

use super::*;
use crate::{
    engine::{Editor, ImportNotice, Section, Track, amplitude_to_db},
    library::Library,
};

mod helpers;
mod scenarios;
use helpers::*;
use scenarios::*;

mod agent_tests;
mod arrangement_drag_tests;
mod arrangement_tests;
mod automation_access_tests;
mod automation_tests;
mod chrome_tests;
mod clip_handles_tests;
mod clips_tests;
mod commands_tests;
mod dialogs_tests;
mod effects_tests;
mod eq_window_tests;
mod inspector_tests;
mod keyboard_tests;
mod layout_tests;
mod libraries_tests;
mod mastering_board_tests;
mod mastering_tests;
mod mix_view_tests;
mod mixer_gain_tests;
mod mixer_tests;
mod overview_tests;
mod rail_tests;
mod sample_conflicts_tests;
mod sample_editor_tests;
mod sounds_tests;
mod step_synth_files_tests;
mod step_synth_tests;
mod stretch_status_tests;
mod stretch_tests;
mod transport_tests;
