---
name: operate-app
description: >
  Operate the running MixJam Techno window through its agent control channel.
  Use to build or edit a song, use a tool window or modal, or check a UI change
  in the built application.
---

# Operate MixJam Techno

An application that starts with `--agent` or `--agent-port PORT` accepts
requests. The `agent` command of the same executable sends them. `agent help`
is the authoritative list of requests and fields.

Read `docs/AGENT-CONTROL.md` when your task needs one of these:

- the panels
- the Options window
- the Choose path modal
- the session key

Read `docs/agent-control/song-editing.md` when your task needs one of these:

- the Step synth board
- an arrangement edit or an automation lane
- the Track EQ window

## 1. Start an isolated session

1. Build the executable that has the behavior under test.
   Completion criterion: The executable is newer than the source change.
2. Tell the owner that a visible window opens, when, if it plays sound, and for
   how long.
   Completion criterion: The owner has the warning before the window opens.
3. Set `MIXJAM_TECHNO_DATA_DIR` to an empty folder in `tmp/`.
   Completion criterion: The session cannot change the settings or the scan
   cache of the owner.
4. Start `mixjam-techno gui --agent-port PORT --samples FOLDER` as a background
   process. Use a port that no other session uses.
   Completion criterion: Standard output shows `Agent control: 127.0.0.1:PORT`.
5. Send `agent --port PORT wait idle=true` with the same
   `MIXJAM_TECHNO_DATA_DIR`. The command reads the session key from that folder.
   Completion criterion: The reply has `"ok":true`, and the folder scan is complete.

Do not start a session on the settings of the owner unless the owner asks.

## 2. Read before you act

1. Send `state` for the song, the selection, the open windows, the modal, and
   the status.
   Completion criterion: You know which modal, if any, takes the input.
2. Send `ui` for the controls that you can operate now. To list one window or
   modal, add `window=TITLE`. To read text and disabled controls, add `all=true`.
   Completion criterion: You have the exact name of each control that you will use.
3. If a control is in a folded panel, open the panel as the section "Controls
   and input" of `docs/AGENT-CONTROL.md` states. Then send `ui` again.
   Completion criterion: `ui` lists the control.
4. If `ui` does not list a control that is on screen, report it as a defect.
   Each control needs an accessible name.
   Completion criterion: Your report names each missing control.
5. Send `samples search=TEXT` to find a sound and its id.
   Completion criterion: Each sound that you will place has one id.

## 3. Act

1. Use `click` and `set` with the control name for each control that `ui` lists.
   Completion criterion: The reply has `"ok":true` and no error status. A
   value control that stays on screen shows the new value. If the control is
   no longer present, the status, the modal, the windows, or `state` confirms
   the action.
2. If a name matches more than one control, add `window=`, `role=`, or
   `index=`. The error message lists the matches.
   Completion criterion: The request selects one control.
3. Use `place`, `move_clips`, `delete_clips`, `select`, `seek`, `set_loop`, and
   `set_sections` to edit the arrangement lanes. Tracks and bars count from 1.
   Completion criterion: The reply and `state` confirm the clips, the
   selection, the transport position, the loop, or the sections.
4. Use `automate track=N lane=LANE` to replace a lane of a track. Use
   `automate bus=N lane=return` to replace the return lane of an FX bus.
   Completion criterion: `state` shows the new points.
5. Use `clip` to change one clip. A field that you omit keeps its value.
   Completion criterion: `state` shows the new clip values.
6. Before you open Effect Studio, Step synth, Sample editor, or Time Stretcher,
   send `select sample=`. The selected sound is the source of these tools.
   Completion criterion: `state` shows the sound in `selection.sample`.
7. If a reply has `"modal":"Choose path"`, send
   `set target=Path value=FULL_PATH`.
   Completion criterion: The `Path` field shows the full path.
8. Send `click target="Confirm path"`.
   Completion criterion: The reply has no modal, and the status line names the file.
9. After an export or a folder choice, send `wait idle=true`.
   Completion criterion: `state` shows no scan and no export.
10. Use `pointer`, `key`, `type`, and `scroll` only if the gesture has no
    named control. A drag and a context menu are examples. Take the positions
    from `state`.
    Completion criterion: `state` shows the result of the gesture.

To send many requests, write one request on each line and pipe the lines to
`agent -`. It stops at the first failure.

Do not start playback or recording on the owner's computer unless the task
needs sound. Playback uses the audio device.

## 4. Check the result

1. Read `status` in each reply. `"error":true` means that the application
   refused the action, although the request itself succeeded.
   Completion criterion: No action has an unexplained error status.
2. Send `state` and compare the song with the intended result.
   Completion criterion: The tempo, the tracks, the clips, and the mixer
   values agree.
3. If the appearance matters, send `screenshot path=FULL_PATH.png` and examine
   the image.
   Completion criterion: The image shows the expected window state.

## 5. End the session

1. Send `quit`.
   Completion criterion: The reply has `"closing":true`, or the reply has
   `"modal":"Save your changes?"`, or the process has ended.
2. If the reply has `"modal":"Save your changes?"`, click `Save changes` or
   `Discard changes`.
   Completion criterion: The process has ended.
3. Remove the temporary files of this session that you do not need. Keep the
   requested output and the evidence that verifies the task.
   Completion criterion: Unrelated files and the necessary evidence are intact.

## Completion criterion

The work is complete when these conditions are true:

- The requested song or window state exists.
- Each action has a checked reply.
- The application process has ended.
