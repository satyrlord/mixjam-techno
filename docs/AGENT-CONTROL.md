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
  `delete_clips`, `select`, `seek`, `set_loop`, `automate`, and `clip` edit them
  without positions. `clip` sets gain, trim, length, fades, and pitch for one clip.
  These edits use the same history as edits made by hand.
- While an agent is attached, a file choice opens a **Choose path** modal in the
  window in place of the native dialog. Set its **Path** field and select
  **Confirm path**. A person can type the path too.
- A modal blocks the controls behind it, as it does for a person. Each reply
  names the open modal and the open tool windows, and has the status line.

## Security

The port accepts connections from this computer only, and each connection must
start with a session key. The application writes a new key to
`agent-PORT.key` in its data folder at each start and removes it on exit. The
`agent` command reads it there, so it needs the same `MIXJAM_TECHNO_DATA_DIR` as
the application. Another user of the computer, or a web page, cannot send
requests. A program that runs as you can read the key. It can then do what the
window can do, which includes saving files to a path that it supplies.
