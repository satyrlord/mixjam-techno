# Agent control

An AI agent, or any script, can operate the window as a person does. Start the
application with `--agent`, or with `.\run.ps1 -Agent`. The application then
accepts requests on port 47800 from programs on the same computer.
`--agent-port PORT` selects another port. Agent control is off unless you start
the application in this mode.

To send one request, use the `agent` command of the same executable:

~~~powershell
$app = '.\target\release\mixjam-techno.exe'
& $app agent ui                                  # controls on screen
& $app agent click target="Effects"
& $app agent set target="Cutoff" value=900 window="Effect Studio"
& $app agent set target="Project BPM" value=160
& $app agent place sample="Analog kick" track=1 bar=1
& $app agent state                               # song, selection, modal, status
& $app agent screenshot path=C:\Temp\window.png
~~~

`agent help` lists all requests. `agent -` reads requests from standard input,
one on each line, and stops at the first failure. Each reply is one JSON
object. The exit code is not zero when a request fails. `agent --port PORT`
selects the port.

## Contents

These parts have their own page,
[Agent control: Step synth, arrangement, and Track
EQ](agent-control/song-editing.md):

- [Step synth board](agent-control/song-editing.md#step-synth-board)
- [Arrangement edits](agent-control/song-editing.md#arrangement-edits)
- [Track EQ window](agent-control/song-editing.md#track-eq-window)

## Controls and input

`ui` lists each control with its name, role, value, and position. `click` and
`set` take that name. `set` works for these controls: sliders, number fields,
text fields, checkboxes, toggles, and combo boxes. When more than one control
has the same name, `window=`, `role=`, and `index=` select one control.
The Inspector describes a mixed Reverse selection in the checkbox's
`description`. `set` accepts `true` or `false` and applies it to every
selected clip.

`key`, `type`, `pointer`, and `scroll` send keyboard and mouse input to a
position. `state` gives the position of the arrangement and of each clip.

`ui` lists only the controls on screen, and a folded panel lists none. To open
a panel, click the `Sounds`, `Inspector`, `Boards`, `Overview`, or `Tools`
switch of the status bar. To restore a maximised score, click
`Maximise the score` in the score toolbar. The data folder keeps the layout in
`layout.json`.

The `Arrange view` and `Mix view` labels of the score switch between the lanes
and the Mix view. The track rows of the Mix view use the names of the Mixer
(`Track N mute`, `Track N EQ`). The Mixer board and the Mix view never show
together. Inspector controls start with `Inspector track` or `Inspector clip`.
`key key=K modifiers=["ctrl"]` opens the All commands list. Its search field is
`Find a command`.

A modal blocks the controls behind it, as it does for a person. Each reply
names the open modal and the open tool windows, and has the status line.

## Keyboard focus

`state` and `ui` have a `focus` field. It is the control that has the keyboard
focus, in the form of a `ui` entry. It is `null` when no control has focus. Use
it to check the Tab order: send `key key=Tab`, then read `focus`. A control
without an accessible name is not in `focus`.

`click` without `pointer=true` sends an accessibility action, and no pointer
moves. The driver then takes the focus from the control that had it, as a
pointer click on another control does. The control that you click keeps its
focus. The keys that follow thus go where they go for a person. For example,
Ctrl+Z after `click target="Arrange view"` undoes the song, and not the Step
synth board.

## Options

To open or focus the Options Utility window, click `Options`, or send Ctrl+,
through `key`. Its named `Audio`, `Workspace`, and `Files` selectors show the
controls on each page. Use `set` with labels such as these:

- `Drive playback oversampling`: `4x` or `Off`.
- `Sample playback quality`: `High` or `Low`.
- `Playback buffer target`: `Automatic`, `20 ms`, or `40 ms`.
- `Preview level`: dB.
- `Mute previews`.
- `Lane height`: `20 px`, `30 px`, or `40 px`.
- `Starting module`.
- `Song dialog folder`.

In `window="Options"`, click `Apply`, `Cancel`, `Restore all defaults`, or
`Reload active options`. Apply uses the same validation, stopped-audio
condition, stale-draft check, persistence, and errors as for a person. Inspect
the disabled reasons and `status` after Apply. A direct change to a settings
file does not use the UI path.

With pending changes, Cancel, Escape, or close opens
`window="Discard changes?"`, with `Apply changes`, `Discard changes`, and
`Keep editing`.

To manage folders and providers, click `Open sample libraries` on the Files
page. It opens `window="Sample libraries"`:

- Its actions apply at once; it has no Apply, Cancel, or Done.
- `Close Sample libraries` closes it, and a running scan continues.
- Add library folder and Scan again use `Tempo for files without one`.
- `Cancel library scan` cancels a scan.
- The provider picker is `Choose provider folder`. Its path choice stays a
  modal.

`state` lists the active preferences under `options`, separately from `song`,
with `options_revision`, `options_error`, `audio_apply_blocked_reason`, and
`playback_buffer`. The buffer text keeps the requested frames separate from the
observed callback frames and interval. The negotiated capacity is not
available.

## Libraries, reviews, and captures

`libraries` lists the library folders in order, and the reason when a folder
did not load. `set_libraries folders='["D:/drums","D:/loops"]'` replaces the
list with 1 to 9 folders and scans them, as the Sample libraries window
does. One folder can be text: `folders=D:\drums`. The sounds change
when the scan ends. Send `wait idle=true` before you use them. `state` also has
the folders under `song.library`.

`reviews`, `review`, and `review_tracks` read the Album Review folder.
`add_review` and `update_review` write it. They do not need the Album Review
window. `reviews tag=Acid energy=peak` lists the albums that have such a track.
`review_tracks` gives each matching track with its role and its timestamped
events. Thus a song-building agent can use the reviews as reference.

Before a save, the application checks the structure of the review and its
implemented validation rules. It reports the validation issues that it finds.
It does not verify factual or listening claims. Give a long review as
`file=PATH`. The [album-analysis skill](../.github/skills/album-analysis/SKILL.md)
gives the format and the procedure.

`capture [reason=TEXT]` saves the last 30 seconds that playback sent to the
audio device, as F12 does. It writes `device.wav`, `source.wav`, and
`state.json` to a new `diagnostics/<local time>` folder in the data folder. The
reply has the `folder` and the `state`. Without audio output, the request fails
and gives the reason.

## File choices

While an agent is attached, a file choice opens a **Choose path** modal in the
window, in place of the native dialog. Set its **Path** field and select
**Confirm path**. A person can also type the path. When the path is not valid,
the modal stays open and shows an error under the field, such as "That file
does not exist." `state.path_prompt.error` has that text. It is `null` when
there is no error. If the song changes while an
Open path choice is collapsed, cancel that choice and start Open again. The old
choice keeps its path draft and cannot replace the later edits.

## Security

The port accepts connections from this computer only. Each connection must
start with a session key. At each start, the application writes a new key to
`agent-PORT.key` in its data folder, and removes it on exit. The `agent`
command reads the key there. Thus it needs the same `MIXJAM_TECHNO_DATA_DIR` as
the application.

The key file uses the default permissions of the file system. The application
does not set a more restrictive access control list. Any user or program that
can read the key can control the window, and can save files to a path that it
supplies. A request without the key cannot authenticate.
