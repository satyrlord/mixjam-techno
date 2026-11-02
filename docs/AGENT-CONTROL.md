# Agent control

An AI agent, or any script, can operate the window as a person does. Start the
application with `--agent`, or `.\run.ps1 -Agent`. It then accepts requests from
programs on the same computer on port 47800. `--agent-port PORT` selects another
port. Agent control is off unless you start the application this way.

Send one request with the `agent` command of the same executable:

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

`agent help` lists every request. `agent -` reads requests from standard input,
one per line, and stops at the first failure. Each reply is one JSON object;
the exit code is nonzero when a request fails. `agent --port PORT` selects the
port.

- `ui` lists each control with its name, role, value, and position. `click` and
  `set` take that name. `set` works for sliders, number fields, text fields,
  checkboxes, toggles, and combo boxes. `window=`, `role=`, and `index=` select
  one control when several have the same name.
- `key`, `type`, `pointer`, and `scroll` send keyboard and mouse input to a
  position. `state` gives the position of the arrangement and of each clip.
- The arrangement lanes are one painted surface. `place`, `move_clips`,
  `delete_clips`, `select`, `seek`, `set_loop`, `set_sections`, `automate`, and
  `clip` edit them without positions. `set_sections` replaces the song's named
  sections, each given by its first `bar` (or `tick`) and its `name`; `state`
  lists them under `song.sections`. `automate track=N lane=LANE
  points=[[BAR,VALUE],...]` replaces one automation lane: `volume`, `send1` to
  `send4`, `filter`, `pan` (-1 hard left to 1 hard right, 0 center), or
  `repeat` (0 off, or 4, 8, 16, 32, or 64 for 1/4 to 1/64 repeats), and
  `points=[]` clears it. The repeat lane steps: it is off before its first
  point and holds each value until the next point, so
  `points=[[5,8],[5.25,16],[5.375,32],[5.5,0]]` rolls from 1/8 through 1/32 on
  the slice that starts at bar 5. `state` lists each track's lanes with points
  under its `automation`; a send or pan lane there replaces the knob in `sends`
  or `pan`.
  `clip` sets gain, trim, length, fades, and pitch for one clip.
  These edits use the same history as edits made by hand.
- A track's channel EQ, drive, compressor, width, and ducker are controls in
  its EQ window. Click `Track N EQ` in the Mixer to open it, then `set` the
  control by name: `set target="Track 1 drive power" value=true`, then
  `set target="Track 1 drive amount" value=18`, and likewise `drive
  character`, `drive mix`, `drive focus` (Hz), and `drive output` (dB). `state`
  lists each track's `eq_on`, `compressor_on`, and `width`, and its `drive`
  with its switch and settings.
- `libraries` lists the library folders in order and the reason any did not
  load. `set_libraries folders='["D:/drums","D:/loops"]'` replaces the list with
  1 to 9 folders and scans them, as the Libraries window does. One folder can be
  given as text: `folders=D:\drums`. The sounds change when the scan ends; send
  `wait idle=true` before you use them. `state` also has the folders under
  `song.library`.
- `reviews`, `review`, and `review_tracks` read the Album Review folder;
  `add_review` and `update_review` write it. They do not need the Album Review
  window. `reviews tag=Acid energy=peak` lists the albums that have such a
  track, and `review_tracks` gives each matching track with its role and its
  timestamped events, so a song-building agent can use the reviews as
  reference. Before saving, the application checks the review's structure and
  its implemented validation rules. It reports the validation issues it finds.
  It does not verify factual or listening claims. Give a long review as
  `file=PATH`. The
  [album-analysis skill](../.github/skills/album-analysis/SKILL.md) has the
  format and the procedure.
- While an agent is attached, a file choice opens a **Choose path** modal in the
  window in place of the native dialog. Set its **Path** field and select
  **Confirm path**. A person can type the path too. If the song changes while
  an Open path choice is collapsed, cancel that choice and start Open again.
  The old choice keeps its path draft and cannot replace the later edits.
- A modal blocks the controls behind it, as it does for a person. Each reply
  names the open modal and the open tool windows, and has the status line.

## Security

The port accepts connections from this computer only, and each connection must
start with a session key. The application writes a new key to
`agent-PORT.key` in its data folder at each start and removes it on exit. The
`agent` command reads it there, so it needs the same `MIXJAM_TECHNO_DATA_DIR` as
the application. The key file uses the file system's default permissions; the
application does not set a more restrictive access control list. Any user or
program that can read the key can control the window, including saving
files to a supplied path. A request without the key cannot authenticate.
