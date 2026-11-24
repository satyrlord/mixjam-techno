//! Every word that the Options window shows or names, in one place. The
//! window and its tests both read these, so a renamed control changes here
//! only. The backend's option values carry no labels.
use super::draft::Page;
use mixjam_techno::application::options::{
    DriveOversampling, LaneHeight, MeterDisplayRate, Oversampling, PlaybackBufferTarget,
    SampleQuality, SongFolderMode, StartingModule, Theme,
};

/// The window's title and identity.
pub(crate) const WINDOW: &str = "Options";

// Footer and prompt actions.
pub(crate) const APPLY: &str = "Apply";
pub(crate) const CANCEL: &str = "Cancel";
/// Restore puts the defaults of every page in the draft, not only the
/// page on show; the name says so.
pub(crate) const RESTORE: &str = "Restore all defaults";
pub(crate) const RELOAD: &str = "Reload active options";
pub(crate) const DISCARD: &str = "Discard changes?";
pub(crate) const APPLY_AND_CLOSE: &str = "Apply changes";
pub(crate) const DISCARD_CHANGES: &str = "Discard changes";
pub(crate) const KEEP_EDITING: &str = "Keep editing";

// Footer status and Apply's reasons.
pub(crate) const NOTHING_TO_APPLY: &str = "No unapplied changes.";
pub(crate) const APPLIED: &str = "Options applied.";
pub(crate) const DEFAULTS_PENDING: &str = "Defaults not applied yet";
pub(crate) const DEFAULTS_PROMPT: &str = "Options defaults are not applied yet.";

/// The footer's count, and the page marker's description.
pub(crate) fn unapplied(count: usize) -> String {
    match count {
        1 => "1 unapplied change".into(),
        _ => format!("{count} unapplied changes"),
    }
}

pub(crate) fn page(page: Page) -> &'static str {
    match page {
        Page::Audio => "Audio",
        Page::Workspace => "Workspace",
        Page::Files => "Files",
    }
}

// Audio page.
pub(crate) const PLAYBACK_QUALITY: &str = "Playback quality";
pub(crate) const DRIVE_OVERSAMPLING: &str = "Drive playback oversampling";
pub(crate) const MASTER_OVERSAMPLING: &str = "Master playback oversampling";
pub(crate) const SAMPLE_QUALITY: &str = "Sample playback quality";
pub(crate) const QUALITY_REDUCED: &str = "Playback quality reduced; exports use full quality";
pub(crate) const BUFFER_TARGET: &str = "Playback buffer target";
pub(crate) const DEVICE_DETAILS: &str = "Device details";
pub(crate) const SAVE_CAPTURE: &str = "Save audio capture";
pub(crate) const MONITORING: &str = "Monitoring";
pub(crate) const PREVIEW_LEVEL: &str = "Preview level";
pub(crate) const MUTE_PREVIEWS: &str = "Mute previews";
pub(crate) const METER_RATE: &str = "Meter display rate";

pub(crate) const DRIVE_OVERSAMPLING_CHOICES: [(DriveOversampling, &str); 2] = [
    (DriveOversampling::TwoX, "2x"),
    (DriveOversampling::Off, "Off"),
];
pub(crate) const OVERSAMPLING: [(Oversampling, &str); 2] =
    [(Oversampling::FourX, "4x"), (Oversampling::Off, "Off")];
pub(crate) const SAMPLE_QUALITIES: [(SampleQuality, &str); 2] =
    [(SampleQuality::High, "High"), (SampleQuality::Low, "Low")];
pub(crate) const BUFFER_TARGETS: [(PlaybackBufferTarget, &str); 3] = [
    (PlaybackBufferTarget::Automatic, "Automatic"),
    (PlaybackBufferTarget::Ms20, "20 ms"),
    (PlaybackBufferTarget::Ms40, "40 ms"),
];
pub(crate) const METER_RATES: [(MeterDisplayRate, &str); 2] = [
    (MeterDisplayRate::Normal, "Normal"),
    (MeterDisplayRate::Reduced, "Reduced"),
];

// Workspace page.
pub(crate) const WORKSPACE: &str = "Workspace";
pub(crate) const THEME: &str = "Theme";
pub(crate) const LANE_HEIGHT: &str = "Lane height";
pub(crate) const FOLLOW_ON_STARTUP: &str = "Follow playhead on startup";
pub(crate) const SECTION_BOOKMARKS: &str = "Show section bookmarks";
pub(crate) const STARTING_MODULE: &str = "Starting module";
pub(crate) const SHOW_TOOLTIPS: &str = "Show tooltips";

pub(crate) fn theme(theme: Theme) -> &'static str {
    match theme {
        Theme::Studio => "Studio",
        Theme::Aero => "Aero",
        Theme::Vintage => "Vintage",
    }
}

pub(crate) fn lane_height(height: LaneHeight) -> &'static str {
    match height {
        LaneHeight::Px20 => "20 px",
        LaneHeight::Px30 => "30 px",
        LaneHeight::Px40 => "40 px",
    }
}

pub(crate) const STARTING_MODULES: [(StartingModule, &str); 4] = [
    (StartingModule::Sounds, "Sounds"),
    (StartingModule::Mixer, "Mixer"),
    (StartingModule::Mastering, "Mastering"),
    (StartingModule::Effects, "FX returns"),
];

// Files page.
pub(crate) const SONG_FOLDERS: &str = "Song folders";
pub(crate) const FOLDER_MODE: &str = "Song dialog folder";
pub(crate) const FIXED_FOLDER: &str = "Fixed song folder";
pub(crate) const CHOOSE_FOLDER: &str = "Choose fixed song folder";
pub(crate) const LIBRARIES: &str = "Sample libraries";
pub(crate) const OPEN_LIBRARIES: &str = "Open sample libraries";

pub(crate) const FOLDER_MODES: [(SongFolderMode, &str); 2] = [
    (SongFolderMode::LastUsed, "Last used"),
    (SongFolderMode::Fixed, "Fixed folder"),
];
