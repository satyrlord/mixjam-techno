//! The requests of the control channel: the values a request carries and the command set.
use super::SectionArgs;
use serde::Deserialize;
use std::path::PathBuf;

/// A scalar that a request can carry in more than one type.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Number(f64),
    Text(String),
}

impl Value {
    pub fn text(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Number(value) => value.to_string(),
            Self::Text(value) => value.clone(),
        }
    }
}

/// Accept `target=160` for a text field: a command line cannot tell them apart.
fn text<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Value::deserialize(deserializer).map(|value| value.text())
}

fn optional_text<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Ok(Option::<Value>::deserialize(deserializer)?.map(|value| value.text()))
}

/// Accept one path as text, or a JSON list of paths. A Windows path with
/// single backslashes is not JSON, so it arrives as text.
fn paths<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<PathBuf>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Paths {
        One(PathBuf),
        Many(Vec<PathBuf>),
    }
    Ok(match Paths::deserialize(deserializer)? {
        Paths::One(path) => vec![path],
        Paths::Many(paths) => paths,
    })
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PointerAction {
    Move,
    Down,
    Up,
    Click,
    DoubleClick,
    Drag,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Help,
    State,
    Samples {
        #[serde(default, deserialize_with = "text")]
        search: String,
        #[serde(default, deserialize_with = "optional_text")]
        group: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        library: Option<String>,
        #[serde(default)]
        offset: usize,
        limit: Option<usize>,
    },
    Ui {
        #[serde(default)]
        all: bool,
        #[serde(default, deserialize_with = "optional_text")]
        window: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        contains: Option<String>,
    },
    Screenshot {
        path: PathBuf,
    },
    Click {
        #[serde(deserialize_with = "text")]
        target: String,
        #[serde(default, deserialize_with = "optional_text")]
        window: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        role: Option<String>,
        index: Option<usize>,
        #[serde(default)]
        double: bool,
        #[serde(default)]
        right: bool,
        #[serde(default)]
        pointer: bool,
        #[serde(default)]
        modifiers: Vec<Modifier>,
    },
    Set {
        #[serde(deserialize_with = "text")]
        target: String,
        #[serde(default, deserialize_with = "optional_text")]
        window: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        role: Option<String>,
        index: Option<usize>,
        value: Value,
    },
    Key {
        #[serde(deserialize_with = "text")]
        key: String,
        #[serde(default)]
        modifiers: Vec<Modifier>,
    },
    Type {
        #[serde(deserialize_with = "text")]
        text: String,
    },
    Pointer {
        action: PointerAction,
        x: f32,
        y: f32,
        to_x: Option<f32>,
        to_y: Option<f32>,
        #[serde(default)]
        right: bool,
        #[serde(default)]
        modifiers: Vec<Modifier>,
    },
    Scroll {
        x: f32,
        y: f32,
        #[serde(default)]
        dx: f32,
        #[serde(default)]
        dy: f32,
    },
    Wait {
        ms: Option<u64>,
        #[serde(default)]
        idle: bool,
        timeout_ms: Option<u64>,
    },
    Place {
        sample: Value,
        track: usize,
        bar: Option<f64>,
        tick: Option<u32>,
    },
    MoveClips {
        clips: Vec<u64>,
        #[serde(default)]
        tracks: i32,
        #[serde(default)]
        bars: f64,
        #[serde(default)]
        ticks: i64,
        #[serde(default)]
        copy: bool,
    },
    DeleteClips {
        clips: Vec<u64>,
    },
    Select {
        #[serde(default)]
        clips: Vec<u64>,
        sample: Option<Value>,
    },
    Seek {
        bar: Option<f64>,
        tick: Option<u32>,
    },
    SetLoop {
        first_bar: Option<u32>,
        last_bar: Option<u32>,
    },
    SetSections {
        sections: Vec<SectionArgs>,
    },
    /// Replace a track's automation lane, or with `bus` an FX bus's return
    /// lane.
    Automate {
        track: Option<usize>,
        bus: Option<usize>,
        #[serde(deserialize_with = "text")]
        lane: String,
        points: Vec<[f64; 2]>,
    },
    /// Change one clip. A field that is left out keeps its value.
    Clip {
        clip: u64,
        gain_db: Option<f32>,
        offset_frames: Option<u32>,
        length_ticks: Option<u32>,
        fade_in_ticks: Option<u32>,
        fade_in_curve: Option<crate::engine::FadeCurve>,
        fade_out_ticks: Option<u32>,
        fade_out_curve: Option<crate::engine::FadeCurve>,
        pitch_cents: Option<i32>,
        reverse: Option<bool>,
    },
    Libraries,
    /// Replace the library folders and scan them.
    SetLibraries {
        #[serde(deserialize_with = "paths")]
        folders: Vec<PathBuf>,
        fallback_bpm: Option<f64>,
    },
    /// List the album reviews that match.
    Reviews {
        #[serde(default, deserialize_with = "optional_text")]
        search: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        artist: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        tag: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        energy: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        kind: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        sort: Option<String>,
        #[serde(default)]
        offset: usize,
        limit: Option<usize>,
    },
    /// One album review in full.
    Review {
        #[serde(deserialize_with = "text")]
        id: String,
    },
    /// The tracks of the album reviews that match.
    ReviewTracks {
        #[serde(default, deserialize_with = "optional_text")]
        id: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        artist: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        tag: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        energy: Option<String>,
        #[serde(default, deserialize_with = "optional_text")]
        kind: Option<String>,
        #[serde(default)]
        offset: usize,
        limit: Option<usize>,
    },
    /// Save a new album review, given inline or as a JSON file.
    AddReview {
        review: Option<serde_json::Value>,
        file: Option<PathBuf>,
    },
    /// Replace an album review, given inline or as a JSON file.
    UpdateReview {
        review: Option<serde_json::Value>,
        file: Option<PathBuf>,
    },
    /// Save a capture of what playback just sent to the audio device.
    Capture {
        #[serde(default, deserialize_with = "optional_text")]
        reason: Option<String>,
    },
    Quit,
}
