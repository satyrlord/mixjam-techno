//! Agent control channel: the wire protocol, a loopback server, and the
//! command-line client. It carries typed requests to a frontend and typed
//! replies back. The frontend decides what each request does.
use crate::application::{Product, Sample, Session, TICKS_PER_BAR};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

pub const DEFAULT_PORT: u16 = 47800;
/// How long a request may wait for the window to take it.
const START_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REQUEST_BYTES: u64 = 1 << 20;

pub const HELP: &str = "\
MixJam Techno agent control. Start the app with `gui --agent`, then send one
request per call: `mixjam-techno agent COMMAND [name=value ...]`. A value is
JSON when it parses as JSON, otherwise text. `agent -` reads requests from
standard input, one per line, and stops at the first failure. A line can also
be a JSON object: {\"command\":\"place\",\"sample\":12,\"track\":1,\"bar\":1}.
Quote a value that has spaces: target=\"Track 5 gain\", clips=\"[1, 2]\".
Each reply is one JSON object with \"ok\". Tracks and bars count from 1.
One bar has 256 ticks. Screen positions are in points from the top left.

Read
  state                  Song, transport, selection, open windows, modal, status.
  samples [search= group= library= offset= limit=]   Sounds in the library.
  ui [all=true window= contains=]   Controls on screen. all=true adds text and
                         disabled controls. window= takes a window or modal title.
  screenshot path=FILE.png   The reply has pixels_per_point for positions.
  wait [ms=N | idle=true timeout_ms=N]   idle waits for a scan or export.

Use controls (target is a label from `ui`, or an id such as \"#123\")
  click target= [window= role= index= double=true right=true pointer=true
                         modifiers=]
  set target= value= [window= role= index=]   Slider, number, text, checkbox,
                         radio button, toggle, or combo box.
  key key=Enter [modifiers=[\"ctrl\",\"shift\",\"alt\"]]
  type text=
  pointer action=move|down|up|click|double_click|drag x= y= [to_x= to_y=
                         right=true modifiers=]
  scroll x= y= [dx= dy=]   dy=-200 shows content further down.

Arrangement (the lanes have no separate controls)
  place sample=ID|NAME track=N bar=N|tick=N
  move_clips clips=[ID,...] [tracks=N bars=N ticks=N copy=true]
  delete_clips clips=[ID,...]
  select [clips=[ID,...]] [sample=ID|NAME]   The selected sound feeds the tools.
  seek bar=N|tick=N
  set_loop first_bar=N last_bar=N   Omit both to turn the loop off.

Other
  quit                   Close the window. Unsaved work opens the save modal.

An action replies with the status line, the open modal, and the open windows.
A modal blocks the controls behind it. The application writes a session key
to its data folder. Use the same MIXJAM_TECHNO_DATA_DIR for this command as
for the application. File choices open a Choose path modal while an agent is
attached: set the Path field, then click Confirm path.";

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
    Quit,
}

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
    pub clips: usize,
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

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LibraryState {
    pub folder: Option<PathBuf>,
    pub samples: usize,
    pub groups: Vec<String>,
    pub providers: Vec<ProviderView>,
}

/// The song and its sources, as the session holds them.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SongView {
    pub title: String,
    pub file: Option<PathBuf>,
    pub unsaved: bool,
    pub bpm: f64,
    pub master_gain: f32,
    /// The first and last looped bar.
    pub loop_bars: Option<[u32; 2]>,
    pub end_tick: u32,
    /// Tracks that hold clips. The other tracks keep their default settings.
    pub tracks: Vec<TrackView>,
    pub clips: Vec<ClipView>,
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
    pub panel: &'static str,
    pub erase: bool,
    pub follow: bool,
    pub zoom: f32,
    pub record_track: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScanView {
    pub root: PathBuf,
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
    pub transport: Transport,
    pub status: Status,
    pub modal: Option<&'static str>,
    pub path_prompt: Option<PathPromptView>,
    pub windows: Vec<&'static str>,
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
    pub windows: Vec<&'static str>,
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
    Done(Box<Outcome>),
}

fn bar_of(tick: u32) -> f64 {
    f64::from(tick) / f64::from(TICKS_PER_BAR) + 1.0
}

/// The tick of a position given as a bar counted from 1, or as a tick.
pub fn position_tick(bar: Option<f64>, tick: Option<u32>) -> Result<u32, String> {
    match (bar, tick) {
        (Some(_), Some(_)) => Err("Give a bar or a tick, not both.".into()),
        (None, Some(tick)) => Ok(tick),
        (Some(bar), None) => {
            let tick = ((bar - 1.0) * f64::from(TICKS_PER_BAR)).round();
            if bar.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&tick) {
                Ok(tick as u32)
            } else {
                Err("Bars count from 1.".into())
            }
        }
        (None, None) => Err("Give a bar or a tick.".into()),
    }
}

fn sample_library(session: &Session, id: u32) -> Option<Product> {
    Product::sample_key(id)
        .map(|(product, _)| product)
        .or_else(|| {
            session
                .project()
                .imported_samples
                .iter()
                .find(|sample| sample.id == id)
                .and_then(|sample| sample.origin)
        })
}

pub fn sample_view(session: &Session, sample: &Sample) -> SampleView {
    SampleView {
        id: sample.id,
        name: sample.name.clone(),
        group: sample.group_name.clone(),
        library: sample_library(session, sample.id).map(Product::label),
        bars: sample.bars,
        bpm: sample.source_bpm,
        path: sample.wav_path.clone(),
    }
}

pub fn song_view(session: &Session) -> SongView {
    let project = session.project();
    let library = session.library();
    let mut tracks: Vec<TrackView> = Vec::new();
    for clip in &project.clips {
        match tracks
            .iter_mut()
            .find(|track| track.track == clip.track + 1)
        {
            Some(track) => track.clips += 1,
            None => {
                let Some(track) = project.tracks.get(clip.track) else {
                    continue;
                };
                tracks.push(TrackView {
                    track: clip.track + 1,
                    gain: track.gain,
                    polarity_inverted: track.polarity_inverted,
                    pan: track.pan,
                    left: track.left_enabled,
                    right: track.right_enabled,
                    clips: 1,
                });
            }
        }
    }
    tracks.sort_by_key(|track| track.track);
    let mut groups: Vec<String> = library
        .samples
        .iter()
        .map(|sample| sample.group_name.clone())
        .collect();
    groups.sort();
    groups.dedup();
    SongView {
        title: project.title.clone(),
        file: session.file().map(PathBuf::from),
        unsaved: session.unsaved(),
        bpm: project.bpm,
        master_gain: project.master_gain,
        loop_bars: project.loop_region.map(|[start, end]| {
            [
                start / TICKS_PER_BAR + 1,
                end.div_ceil(TICKS_PER_BAR).max(start / TICKS_PER_BAR + 1),
            ]
        }),
        end_tick: project.end_tick(),
        tracks,
        clips: project
            .clips
            .iter()
            .map(|clip| {
                let missing = session.missing_sample(clip.sample_id);
                ClipView {
                    id: clip.id,
                    sample: clip.sample_id,
                    name: missing.map_or_else(
                        || {
                            library
                                .sample_by_id(clip.sample_id)
                                .map(|sample| sample.name.clone())
                                .unwrap_or_default()
                        },
                        |missing| missing.name.clone(),
                    ),
                    missing: missing.is_some(),
                    track: clip.track + 1,
                    bar: bar_of(clip.start_tick),
                    start_tick: clip.start_tick,
                    length_ticks: clip.length_ticks,
                    offset_frames: clip.offset_frames,
                    gain: clip.gain,
                    rect: None,
                }
            })
            .collect(),
        missing_samples: session
            .missing_samples()
            .iter()
            .map(|missing| MissingView {
                sample: missing.id,
                name: missing.name.clone(),
                reason: missing.reason.clone(),
            })
            .collect(),
        sample_conflicts: session
            .sample_conflicts()
            .iter()
            .map(|conflict| ConflictView {
                reference: conflict.reference.clone(),
                candidates: conflict.candidates.clone(),
                selected: conflict.selected,
                confirmed: conflict.confirmed,
            })
            .collect(),
        import_notices: project
            .import_notices
            .iter()
            .map(|notice| {
                format!(
                    "{} BPM changed to {} BPM.",
                    notice.source_bpm, notice.target_bpm
                )
            })
            .chain(project.source_notices.iter().cloned())
            .collect(),
        library: LibraryState {
            folder: library.wav_root().map(PathBuf::from),
            samples: library.samples.len(),
            groups,
            providers: library
                .bindings()
                .iter()
                .map(|binding| ProviderView {
                    library: binding.product.label(),
                    root: binding.root.clone(),
                })
                .collect(),
        },
    }
}

pub fn transport(session: &Session) -> Transport {
    Transport {
        playing: session.wants_playback(),
        recording: session.is_recording(),
        cursor_tick: session.cursor_tick(),
        bar: bar_of(session.cursor_tick()),
    }
}

pub fn busy(session: &Session) -> Busy {
    Busy {
        scan: session.wave_scan().map(|scan| ScanView {
            root: scan.root,
            files_done: scan.files_done,
            files_total: scan.files_total,
        }),
        exporting: session.is_exporting(),
        unsaved_take: session.has_recorded_take(),
    }
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

/// Returns the reply of one request to the client that sent it.
pub struct Responder(mpsc::Sender<String>);

impl Responder {
    pub fn send(self, reply: Result<Body, String>) {
        // The client may have disconnected. Its reply then has no reader.
        let _ = self.0.send(encode(&reply));
    }
}

struct Incoming {
    request: Request,
    /// Set by whichever side acts first: the window when it takes the
    /// request, or the connection when it stops waiting.
    claimed: Arc<AtomicBool>,
    reply: mpsc::Sender<String>,
}

type Waker = Arc<Mutex<Option<Box<dyn Fn() + Send>>>>;

/// Where a session on `port` keeps its key, in the application's data folder.
fn key_path(folder: &Path, port: u16) -> PathBuf {
    folder.join(format!("agent-{port}.key"))
}

/// A key that another program cannot guess. The standard library seeds each
/// hasher from the operating system's random source.
fn new_key() -> String {
    use std::hash::{BuildHasher, Hasher};
    (0..4)
        .map(|_| {
            let state = std::collections::hash_map::RandomState::new();
            format!("{:016x}", state.build_hasher().finish())
        })
        .collect()
}

/// Accepts requests on the loopback interface and queues them for a frontend.
pub struct Server {
    port: u16,
    key: String,
    key_file: PathBuf,
    incoming: mpsc::Receiver<Incoming>,
    waker: Waker,
}

impl Drop for Server {
    fn drop(&mut self) {
        // The key of a session that has ended opens nothing.
        let _ = std::fs::remove_file(&self.key_file);
    }
}

impl Server {
    /// Listen on `port` and write the session key to `folder`. A client must
    /// send the key first, so only a program that can read the user's data
    /// folder can operate the application.
    pub fn start(port: u16, folder: &Path) -> Result<Self, String> {
        Self::start_with(port, folder, START_TIMEOUT)
    }

    fn start_with(port: u16, folder: &Path, start_timeout: Duration) -> Result<Self, String> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .map_err(|error| format!("Cannot listen for agents on port {port}: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        let key = new_key();
        let key_file = key_path(folder, port);
        std::fs::create_dir_all(folder)
            .map_err(|error| error.to_string())
            .and_then(|()| crate::atomic_file::write(&key_file, key.as_bytes()))
            .map_err(|error| format!("Cannot write the agent key: {error}"))?;
        let (sender, incoming) = mpsc::channel();
        let waker = Waker::default();
        let wake = waker.clone();
        let expected = key.clone();
        std::thread::Builder::new()
            .name("mixjam-techno-agent".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let sender = sender.clone();
                    let wake = wake.clone();
                    let expected = expected.clone();
                    let _ = std::thread::Builder::new()
                        .name("mixjam-techno-agent-client".into())
                        .spawn(move || serve(stream, &expected, &sender, &wake, start_timeout));
                }
            })
            .map_err(|error| format!("Cannot start the agent listener: {error}"))?;
        Ok(Self {
            port,
            key,
            key_file,
            incoming,
            waker,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    /// Call `waker` from the connection thread whenever a request arrives.
    pub fn set_waker(&self, waker: impl Fn() + Send + 'static) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(Box::new(waker));
        }
    }

    /// The next request that a client still waits for.
    pub fn next(&self) -> Option<(Request, Responder)> {
        loop {
            let incoming = self.incoming.try_recv().ok()?;
            if !incoming.claimed.swap(true, Ordering::SeqCst) {
                return Some((incoming.request, Responder(incoming.reply)));
            }
        }
    }
}

fn serve(
    stream: TcpStream,
    key: &str,
    requests: &mpsc::Sender<Incoming>,
    waker: &Waker,
    start_timeout: Duration,
) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    // The first line is the session key. Anything else ends the connection,
    // which also stops a request that another program was tricked into
    // sending, such as an HTTP form post.
    let mut first = String::new();
    if (&mut reader)
        .take(MAX_REQUEST_BYTES)
        .read_line(&mut first)
        .is_err()
        || first.trim_end() != key
    {
        let _ = writeln!(
            writer,
            "{}",
            encode(&Err(
                "The session key is wrong. Send requests with the `agent` command.".into()
            ))
        );
        return;
    }
    loop {
        let mut line = String::new();
        match (&mut reader).take(MAX_REQUEST_BYTES).read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if line.trim().is_empty() {
            continue;
        }
        // A malformed line ends the connection.
        let request = if line.ends_with('\n') || (line.len() as u64) < MAX_REQUEST_BYTES {
            serde_json::from_str::<Request>(&line)
                .map_err(|error| format!("Invalid request: {error}"))
        } else {
            Err("The request is too long.".into())
        };
        let request = match request {
            Ok(request) => request,
            Err(error) => {
                let _ = writeln!(writer, "{}", encode(&Err(error)));
                return;
            }
        };
        let (reply, replies) = mpsc::channel();
        let claimed = Arc::new(AtomicBool::new(false));
        let queued = requests
            .send(Incoming {
                request,
                claimed: claimed.clone(),
                reply,
            })
            .is_ok();
        if let Ok(waker) = waker.lock()
            && let Some(wake) = waker.as_ref()
        {
            wake();
        }
        let closed = || encode(&Err("The application closed before it replied.".into()));
        let reply = if queued {
            loop {
                match replies.recv_timeout(start_timeout) {
                    Ok(reply) => break reply,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break closed(),
                    // The window has the request and reports when it is done.
                    Err(mpsc::RecvTimeoutError::Timeout)
                        if claimed.swap(true, Ordering::SeqCst) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        break encode(&Err(
                            "The window did not take the request. Restore it if it is minimized."
                                .into(),
                        ));
                    }
                }
            }
        } else {
            closed()
        };
        if writeln!(writer, "{reply}").is_err() {
            return;
        }
    }
}

/// Split a request line into words. Double quotes keep spaces in a word.
fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    for character in line.chars() {
        match character {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            character if character.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            character => {
                word.push(character);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// The JSON line for `COMMAND name=value ...`, or for one JSON object.
pub fn request_line(words: &[String]) -> Result<String, String> {
    let (first, rest) = words
        .split_first()
        .ok_or("Give a command. Use `agent help` for the list.")?;
    let value = if first.trim_start().starts_with('{') {
        if !rest.is_empty() {
            return Err("Give one JSON object, or a command with name=value fields.".into());
        }
        serde_json::from_str(first).map_err(|error| format!("Invalid JSON: {error}"))?
    } else {
        let mut fields = serde_json::Map::new();
        fields.insert("command".into(), first.as_str().into());
        for word in rest {
            let (name, value) = word
                .split_once('=')
                .ok_or_else(|| format!("Write `{word}` as name=value."))?;
            fields.insert(
                name.into(),
                serde_json::from_str(value).unwrap_or_else(|_| value.into()),
            );
        }
        serde_json::Value::Object(fields)
    };
    serde_json::from_value::<Request>(value.clone())
        .map_err(|error| format!("Invalid request: {error}. Use `agent help` for the list."))?;
    Ok(value.to_string())
}

/// Send request lines and write each reply. Stops at the first failed
/// request and returns whether every request succeeded.
pub fn send(
    port: u16,
    key: &str,
    lines: impl IntoIterator<Item = Result<String, String>>,
    output: &mut impl Write,
) -> Result<bool, String> {
    let mut connection: Option<(TcpStream, BufReader<TcpStream>)> = None;
    for line in lines {
        let line = line?;
        if connection.is_none() {
            let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).map_err(|error| {
                format!(
                    "Cannot reach MixJam Techno on port {port}: {error}. Start it with `gui --agent`."
                )
            })?;
            let reader = BufReader::new(stream.try_clone().map_err(|error| error.to_string())?);
            writeln!(&stream, "{key}")
                .map_err(|error| format!("Cannot send the session key: {error}"))?;
            connection = Some((stream, reader));
        }
        let (stream, reader) = connection
            .as_mut()
            .expect("The connection was just opened.");
        writeln!(stream, "{line}").map_err(|error| format!("Cannot send the request: {error}"))?;
        let mut reply = String::new();
        reader
            .read_line(&mut reply)
            .map_err(|error| format!("Cannot read the reply: {error}"))?;
        if reply.is_empty() {
            return Err("The application closed the connection.".into());
        }
        output
            .write_all(reply.as_bytes())
            .and_then(|()| output.flush())
            .map_err(|error| error.to_string())?;
        let ok = serde_json::from_str::<serde_json::Value>(&reply)
            .ok()
            .and_then(|reply| reply.get("ok")?.as_bool())
            .unwrap_or(false);
        if !ok {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The `agent` command: `[--port N] COMMAND [name=value ...]`, or `-` to read
/// request lines from standard input.
pub fn run_client(arguments: &[String]) -> Result<bool, String> {
    let mut port = DEFAULT_PORT;
    let mut rest = arguments;
    if let [flag, value, tail @ ..] = rest
        && flag == "--port"
    {
        port = value
            .parse()
            .map_err(|_| "--port needs a port number.".to_string())?;
        rest = tail;
    }
    let mut output = std::io::stdout().lock();
    if matches!(rest, [word] if word == "help") {
        writeln!(output, "{HELP}").map_err(|error| error.to_string())?;
        return Ok(true);
    }
    let key_file = key_path(&crate::project_io::data_root(), port);
    let key = std::fs::read_to_string(&key_file).map_err(|_| {
        format!(
            "No session key at {}. Start the application with `gui --agent`, with the same port and the same MIXJAM_TECHNO_DATA_DIR as this command.",
            key_file.display()
        )
    })?;
    let key = key.trim();
    if matches!(rest, [word] if word == "-") {
        let lines = std::io::stdin().lines().filter_map(|line| match line {
            Ok(line) if line.trim().is_empty() || line.trim_start().starts_with('#') => None,
            Ok(line) if line.trim_start().starts_with('{') => Some(request_line(&[line])),
            Ok(line) => Some(request_line(&words(&line))),
            Err(error) => Some(Err(error.to_string())),
        });
        return send(port, key, lines, &mut output);
    }
    send(port, key, [request_line(rest)], &mut output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_string()).collect()
    }

    fn key_folder(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mixjam-agent-{}-{tag}", std::process::id()))
    }

    #[test]
    fn command_words_become_typed_requests_and_reject_unknown_fields() {
        let line = request_line(&strings(&[
            "place",
            "sample=Analog kick",
            "track=2",
            "bar=1.5",
        ]))
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Request>(&line).unwrap(),
            Request::Place {
                sample: Value::Text("Analog kick".into()),
                track: 2,
                bar: Some(1.5),
                tick: None,
            }
        );
        // A label that looks like a number stays a label.
        let line = request_line(&strings(&["click", "target=160", "double=true"])).unwrap();
        assert!(matches!(
            serde_json::from_str::<Request>(&line).unwrap(),
            Request::Click { target, double: true, pointer: false, .. } if target == "160"
        ));
        let line = request_line(&strings(&["move_clips", "clips=[3,4]", "bars=-2"])).unwrap();
        assert!(matches!(
            serde_json::from_str::<Request>(&line).unwrap(),
            Request::MoveClips { clips, bars, copy: false, .. } if clips == [3, 4] && bars == -2.0
        ));
        let line = request_line(&strings(&[
            r#"{"command":"set_loop","first_bar":1,"last_bar":8}"#,
        ]))
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Request>(&line).unwrap(),
            Request::SetLoop {
                first_bar: Some(1),
                last_bar: Some(8)
            }
        );
        assert_eq!(
            serde_json::from_str::<Request>(&request_line(&strings(&["state"])).unwrap()).unwrap(),
            Request::State
        );
        for invalid in [
            &["place", "sample=1", "track=1", "bars=1"][..],
            &["frobnicate"],
            &["click"],
            &["state", "loose"],
            &[],
        ] {
            assert!(request_line(&strings(invalid)).is_err(), "{invalid:?}");
        }
        assert_eq!(
            words(r#"set target="Find a sound" value="deep  bass" window=Sounds"#),
            [
                "set",
                "target=Find a sound",
                "value=deep  bass",
                "window=Sounds"
            ]
        );
        assert_eq!(words(r#"type text="""#), ["type", "text="]);
    }

    #[test]
    fn positions_count_bars_from_one_and_ticks_from_zero() {
        assert_eq!(position_tick(Some(1.0), None), Ok(0));
        assert_eq!(position_tick(Some(3.25), None), Ok(2 * 256 + 64));
        assert_eq!(position_tick(None, Some(96)), Ok(96));
        assert!(position_tick(Some(0.5), None).is_err());
        assert!(position_tick(Some(f64::NAN), None).is_err());
        assert!(position_tick(Some(1.0), Some(0)).is_err());
        assert!(position_tick(None, None).is_err());
    }

    #[test]
    fn replies_are_one_flat_object_with_the_result() {
        let reply: serde_json::Value = serde_json::from_str(&encode(&Ok(Body::Samples {
            total: 0,
            samples: Vec::new(),
        })))
        .unwrap();
        assert_eq!(
            reply,
            serde_json::json!({"ok": true, "total": 0, "samples": []})
        );
        let reply: serde_json::Value =
            serde_json::from_str(&encode(&Err("No such control.".into()))).unwrap();
        assert_eq!(
            reply,
            serde_json::json!({"ok": false, "error": "No such control."})
        );
    }

    #[test]
    fn server_queues_requests_wakes_the_frontend_and_returns_its_replies() {
        let folder = key_folder("queue");
        let server = Server::start_with(0, &folder, Duration::from_millis(150)).unwrap();
        let key = server.key().to_string();
        assert_eq!(
            std::fs::read_to_string(key_path(&folder, server.port())).unwrap(),
            key
        );
        let (woken, wakes) = mpsc::channel();
        server.set_waker(move || {
            let _ = woken.send(());
        });
        let port = server.port();
        let client_key = key.clone();
        let client = std::thread::spawn(move || {
            let mut output = Vec::new();
            let ok = send(
                port,
                &client_key,
                [
                    request_line(&strings(&["help"])),
                    request_line(&strings(&["seek", "bar=5"])),
                    // The client stops before this one.
                    request_line(&strings(&["state"])),
                ],
                &mut output,
            )
            .unwrap();
            (ok, String::from_utf8(output).unwrap())
        });
        for expected in [
            Request::Help,
            Request::Seek {
                bar: Some(5.0),
                tick: None,
            },
        ] {
            wakes.recv_timeout(Duration::from_secs(10)).unwrap();
            let (request, respond) = server.next().unwrap();
            assert_eq!(request, expected);
            respond.send(match request {
                Request::Help => Ok(Body::Help { help: HELP }),
                _ => Err("The song is empty.".into()),
            });
        }
        let (ok, output) = client.join().unwrap();
        let replies: Vec<serde_json::Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        // The client stops at the failed request.
        assert!(!ok);
        assert_eq!(replies.len(), 2);
        assert_eq!(replies[0]["ok"], true);
        assert_eq!(replies[1]["error"], "The song is empty.");

        // A request that the frontend does not take in time fails, and the
        // frontend can no longer run it.
        let mut output = Vec::new();
        assert!(
            !send(
                port,
                &key,
                [request_line(&strings(&["state"]))],
                &mut output
            )
            .unwrap()
        );
        assert!(String::from_utf8(output).unwrap().contains("did not take"));
        assert!(server.next().is_none());

        // The key file lasts as long as the session.
        drop(server);
        assert!(!key_path(&folder, port).exists());
        std::fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn a_connection_without_the_key_or_with_a_malformed_line_runs_nothing_more() {
        let folder = key_folder("refuse");
        let server = Server::start_with(0, &folder, Duration::from_secs(5)).unwrap();
        let exchange = |bytes: &[u8]| {
            let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, server.port())).unwrap();
            stream.write_all(bytes).unwrap();
            let mut reply = String::new();
            BufReader::new(stream).read_to_string(&mut reply).unwrap();
            reply
        };
        for intruder in [
            &b"POST / HTTP/1.1\r\nHost: localhost\r\n\r\n{\"command\":\"quit\"}\n"[..],
            b"{\"command\":\"quit\"}\n",
            b"0000000000000000\n{\"command\":\"quit\"}\n",
        ] {
            let reply = exchange(intruder);
            assert_eq!(reply.lines().count(), 1);
            assert!(reply.contains("session key is wrong"), "{reply}");
        }
        let reply = exchange(
            format!(
                "{}\nnot a request\n{{\"command\":\"quit\"}}\n",
                server.key()
            )
            .as_bytes(),
        );
        assert_eq!(reply.lines().count(), 1);
        assert!(reply.contains("Invalid request"), "{reply}");
        assert!(server.next().is_none());
        drop(server);
        std::fs::remove_dir(folder).unwrap();
    }
}
