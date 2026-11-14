//! Agent control channel: the wire protocol, a loopback server, and the
//! command-line client. It carries typed requests to a frontend and typed
//! replies back. The frontend decides what each request does.
mod album;
mod client;
mod reply;
mod request;
mod server;
#[cfg(test)]
mod tests;
mod views;

pub use album::album_reply;
pub use client::{request_line, run_client, send};
pub use reply::{
    ArrangementView, Body, BusView, Busy, ClipView, ConflictView, LibraryState, MissingView,
    Outcome, PathPromptView, ProviderView, ReviewProblem, SampleView, ScanView, SectionArgs,
    SectionView, Selection, SongView, StateView, Status, Tools, TrackView, Transport, UiNode,
    UnloadedView, encode,
};
pub use request::{Modifier, PointerAction, Request, Value};
pub use server::{Responder, Server};
pub use views::{busy, position_tick, sample_view, song_view, transport, unloaded_views};

pub const DEFAULT_PORT: u16 = 47800;

pub const HELP: &str = "\
MixJam Techno agent control. Start the app with `gui --agent` or
`gui --agent-port PORT`. Send one request per call:
`mixjam-techno agent COMMAND [name=value ...]`. A value is
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
  set_sections sections=[{\"bar\":N,\"name\":\"Intro\"},...]
                         Replace the song's sections; [] removes them. Each
                         section starts at its bar (or \"tick\") and lasts until
                         the next one. Names are 1 to 48 characters.
  automate track=N lane=volume|send1..send4|filter|pan|repeat|repeatgate|pitch
                         points=[[BAR,VALUE],...]
                         Replace one automation lane of a track; [] clears it.
                         Volume is dB over the fader, -60 (silence) to 6;
                         a send is 0 to 1 and replaces its knob; the filter
                         is -1 (low-pass) to 1 (high-pass), open at 0; the
                         pan is -1 (left) to 1 (right), center at 0, and
                         replaces the pan knob. Repeat is 0 (off), or 4, 8,
                         16, 32, or 64 to repeat 1/4 to 1/64 notes of the
                         track's sound from where it turns on; it holds each
                         value until the next point and is off before the
                         first. Repeat gate is 5 to 100, the percent of each
                         repeat that sounds before it falls silent; it holds
                         each value like Repeat and is 100 before the first.
                         Pitch is cents, -4800 to 2400, added to each clip's
                         pitch as a speed change, like a tape: -1200 plays
                         an octave down at half speed, so a fall to -4800
                         is a tape-stop. It is 0 before the first point.
  automate bus=N lane=return points=[[BAR,VALUE],...]
                         Replace FX bus N's return lane (1 to 4); [] clears
                         it. The return is 0 to 2 (1 is 100%, 2 about +6 dB)
                         and replaces the bus's Return knob.
  clip clip=ID [gain_db= offset_frames= length_ticks= fade_in_ticks=
                         fade_in_curve= fade_out_ticks= fade_out_curve=
                         pitch_cents= reverse=true|false]
                         Change one clip; a field left out keeps its value.
                         gain_db is -60 (silence) to 6; state reports gain as
                         amplitude. offset_frames skips that many source frames.
                         Curves are linear, equal-power, or exponential. The
                         fade out ends where the clip's sound ends. Pitch is
                         -2400 to 2400 cents and keeps the timing. reverse
                         plays the clip's slice backwards within the clip.
                         Use move_clips to move a clip.

Libraries
  libraries              The library folders in order, and why any did not load.
  set_libraries folders=[\"PATH\",...] [fallback_bpm=N]   Replace the list with
                         1 to 9 folders and scan them, as the Sample libraries
                         window does. The sounds change when the scan ends;
                         `wait idle=true` waits for it.

Album Review (reviews of albums and of your own songs; no window needed)
  reviews [search= artist= tag= energy= kind= sort= offset= limit=]
                         The reviews, each with its genre tags and track
                         count. search= looks in titles, artists, and genre
                         tags; artist= is part of a name; tag= is a genre or
                         track tag; energy= is low, mid, high, or peak; kind=
                         is reference or own. A track filter keeps a review
                         with one such track. sort= is year, year_desc,
                         artist, title, or tracks.
  review id=             One review in full: metadata, overview, links, and
                         each track with its energy, tags, role, and events.
  review_tracks [id= artist= tag= energy= kind= offset= limit=]
                         The tracks that match, with their album and their
                         timestamped events, such as the arc of every peak
                         track tagged Acid.
  add_review review={...} | file=REVIEW.json   Save a new review. The error
                         lists the validation problems the application finds.
  update_review review={...} | file=REVIEW.json   Replace the review with
                         that id, under the same rules.

Other
  capture [reason=]      Save the last 30 seconds that playback sent to the
                         audio device, as F12 does: device.wav, source.wav,
                         and state.json in a new diagnostics/<local time>
                         folder of the data folder. The reply has the folder
                         and the state.
  quit                   Close the window. Unsaved work opens the save modal.

An action replies with the status line, the open modal, and the open windows.
A modal blocks the controls behind it. The application writes a session key
to its data folder. Use the same MIXJAM_TECHNO_DATA_DIR for this command as
for the application. File choices open a Choose path modal while an agent is
attached: set the Path field, then click Confirm path.";
