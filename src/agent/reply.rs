//! The replies of the control channel: state views, outcomes, reply bodies, and their JSON encoding.
use crate::album_review;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Status {
    pub text: String,
    pub error: bool,
}

/// One control or text of the window, from its accessibility tree.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UiNode {
    pub id: String,
    pub role: &'static str,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    pub enabled: bool,
    pub clickable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
    /// Left, top, width, and height in points.
    pub rect: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SampleView {
    pub id: u32,
    pub name: String,
    pub group: String,
    /// The product library, or `None` for a user WAV.
    pub library: Option<&'static str>,
    pub bars: u32,
    pub bpm: f64,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClipView {
    pub id: u64,
    pub sample: u32,
    pub name: String,
    pub missing: bool,
    pub track: usize,
    pub bar: f64,
    pub start_tick: u32,
    pub length_ticks: u32,
    pub offset_frames: u32,
    pub gain: f32,
    pub fades: crate::engine::Fades,
    pub pitch_cents: i32,
    /// Whether the clip plays backwards.
    pub reverse: bool,
    /// Where the clip is in the window, when the arrangement shows its track.
    pub rect: Option<[f32; 4]>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TrackView {
    pub track: usize,
    pub gain: f32,
    pub polarity_inverted: bool,
    pub pan: f32,
    pub left: bool,
    pub right: bool,
    pub muted: bool,
    pub solo: bool,
    pub sends: [f32; crate::fx::BUSES],
    /// Only lanes with points. A send or pan lane with points replaces its
    /// knob, `sends` or `pan`. Repeat and repeat gate points are steps. Pitch
    /// points are cents of playback speed.
    pub automation: crate::automation::Automation,
    /// Whether the track's channel EQ is on.
    pub eq_on: bool,
    /// The track's drive: its switch and settings.
    pub drive: crate::drive::Drive,
    /// Whether the track's compressor is on.
    pub compressor_on: bool,
    /// The track's stereo width in percent.
    pub width: f32,
    pub clips: usize,
}

/// One FX bus: its effect, its Return knob, and its return lane.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BusView {
    pub bus: usize,
    pub effect: &'static str,
    pub powered: bool,
    pub return_level: f32,
    /// The return lane's points, ticks and values from 0 to 2. With points
    /// it replaces `return_level`.
    pub return_automation: Vec<crate::automation::Point>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MissingView {
    pub sample: u32,
    pub name: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConflictView {
    pub reference: String,
    pub candidates: Vec<PathBuf>,
    pub selected: usize,
    pub confirmed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProviderView {
    pub library: &'static str,
    pub root: PathBuf,
}

/// A library folder that the last scan could not load.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UnloadedView {
    pub folder: PathBuf,
    pub error: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LibraryState {
    /// The library folders of the last scan, in order, loaded or not.
    pub folders: Vec<PathBuf>,
    pub unloaded: Vec<UnloadedView>,
    pub samples: usize,
    pub groups: Vec<String>,
    pub providers: Vec<ProviderView>,
}

/// One section of a `set_sections` request: where it starts and its name.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionArgs {
    pub bar: Option<f64>,
    pub tick: Option<u32>,
    pub name: String,
}

/// A section of the song: its first bar, its start, and its name.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SectionView {
    pub bar: f64,
    pub tick: u32,
    pub name: String,
}

/// The song and its sources, as the session holds them.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SongView {
    pub title: String,
    pub file: Option<PathBuf>,
    pub unsaved: bool,
    pub bpm: f64,
    pub master_gain: f32,
    /// Whether the master pedalboard is bypassed.
    pub master_bypass: bool,
    /// The first and last looped bar.
    pub loop_bars: Option<[u32; 2]>,
    pub sections: Vec<SectionView>,
    pub end_tick: u32,
    /// Tracks that hold clips or have Solo on, including empty soloed tracks.
    pub tracks: Vec<TrackView>,
    pub clips: Vec<ClipView>,
    /// The four FX buses in order.
    pub fx_buses: Vec<BusView>,
    pub missing_samples: Vec<MissingView>,
    pub sample_conflicts: Vec<ConflictView>,
    pub import_notices: Vec<String>,
    pub library: LibraryState,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Transport {
    pub playing: bool,
    pub recording: bool,
    pub cursor_tick: u32,
    pub bar: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Selection {
    pub sample: Option<u32>,
    pub clips: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Tools {
    pub erase: bool,
    pub follow: bool,
    pub zoom: f32,
    pub record_track: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScanView {
    pub folders: Vec<PathBuf>,
    pub files_done: usize,
    pub files_total: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Busy {
    pub scan: Option<ScanView>,
    pub exporting: bool,
    pub unsaved_take: bool,
}

/// The file choice that the Choose path modal is waiting for.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PathPromptView {
    pub title: String,
    pub kind: &'static str,
    pub extensions: Vec<&'static str>,
    pub suggested_name: Option<String>,
}

/// Where the lanes are in the window. A position maps to a tick through
/// `left` and `bar_width`, and to a track through `tracks` and `row_height`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ArrangementView {
    /// The visible part of the lanes: left, top, width, and height.
    pub visible: [f32; 4],
    /// Where tick 0 and the first row start. Scrolling moves them.
    pub left: f32,
    pub top: f32,
    pub bar_width: f32,
    pub row_height: f32,
    /// The track of each row, top to bottom. The last can be the empty Add row.
    pub tracks: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StateView {
    pub version: &'static str,
    pub song: SongView,
    /// Application preferences are independent of song state.
    pub options: crate::options::AppOptions,
    pub options_revision: u64,
    pub options_error: Option<String>,
    pub audio_apply_blocked_reason: Option<String>,
    pub playback_buffer: String,
    pub transport: Transport,
    pub status: Status,
    pub modal: Option<&'static str>,
    pub path_prompt: Option<PathPromptView>,
    pub windows: Vec<String>,
    pub selection: Selection,
    pub tools: Tools,
    pub busy: Busy,
    pub arrangement: Option<ArrangementView>,
}

/// What every action reports once the window has settled.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Outcome {
    pub status: Status,
    pub modal: Option<&'static str>,
    pub windows: Vec<String>,
    /// The action closed the application.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub closing: bool,
    /// Clips that the action created or moved.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<u64>,
    /// The control that the action used, as it is now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<UiNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Body {
    Help {
        help: &'static str,
    },
    State(Box<StateView>),
    Samples {
        total: usize,
        samples: Vec<SampleView>,
    },
    Ui {
        modal: Option<&'static str>,
        nodes: Vec<UiNode>,
    },
    Screenshot {
        path: PathBuf,
        /// The image size in pixels. One point covers this many pixels.
        width: usize,
        height: usize,
        pixels_per_point: f32,
    },
    Libraries {
        folders: Vec<PathBuf>,
        unloaded: Vec<UnloadedView>,
        /// The most folders that the list can hold.
        max_folders: usize,
        fallback_bpm: f64,
        scan: Option<ScanView>,
    },
    Reviews {
        /// The reviews folder, or none when the application has none.
        folder: Option<PathBuf>,
        total: usize,
        reviews: Vec<album_review::Summary>,
        /// Files of the folder that did not load, with the reason.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        problems: Vec<ReviewProblem>,
    },
    Review {
        review: Box<album_review::Review>,
    },
    ReviewTracks {
        total: usize,
        tracks: Vec<album_review::TrackMatch>,
    },
    ReviewSaved {
        id: String,
        path: PathBuf,
        added: bool,
    },
    /// A saved playback capture: its folder and its `state.json`.
    Capture {
        folder: PathBuf,
        state: serde_json::Value,
    },
    Done(Box<Outcome>),
}

/// A review file that did not load.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ReviewProblem {
    pub file: String,
    pub error: String,
}

/// The reply line for a request.
pub fn encode(reply: &Result<Body, String>) -> String {
    let failure = |error: &str| serde_json::json!({"ok": false, "error": error});
    match reply {
        Ok(body) => match serde_json::to_value(body) {
            Ok(serde_json::Value::Object(mut fields)) => {
                fields.insert("ok".into(), true.into());
                serde_json::Value::Object(fields)
            }
            _ => failure("The reply could not be encoded."),
        },
        Err(error) => failure(error),
    }
    .to_string()
}
