---
name: operate-app
description: >
  Operate the running MixJam Techno window through its agent control channel.
  Use to build or edit a song, use a tool window or modal, or check a UI change
  in the built application.
---

# Operate MixJam Techno

The application accepts requests only when it starts with `--agent`. The
`agent` command of the same executable sends them. `agent help` is the
authoritative request list. Read `docs/AGENT-CONTROL.md` for the model.

## 1. Start an isolated session

1. Build the executable that has the behavior under test.
   Completion criterion: The executable is newer than the source change.
2. Set `MIXJAM_TECHNO_DATA_DIR` to an empty folder in `tmp/`.
   Completion criterion: The session cannot change the owner's settings or scan cache.
3. Start `mixjam-techno gui --agent-port PORT --samples FOLDER` as a background
   process. Use a port that no other session uses.
   Completion criterion: Standard output shows `Agent control: 127.0.0.1:PORT`.
4. Send `agent --port PORT wait idle=true` with the same
   `MIXJAM_TECHNO_DATA_DIR`. The command reads the session key from that folder.
   Completion criterion: The reply has `"ok":true` and the folder scan is complete.

Do not start a session on the owner's settings unless the owner requests it.

## 2. Read before you act

1. Send `state` for the song, selection, open windows, modal, and status.
   Completion criterion: You know which modal, if any, takes the input.
2. Send `ui` for the controls that you can operate now. Add `window=TITLE` to
   list one window or modal. Add `all=true` to read text and disabled controls.
   Completion criterion: You have the exact name of each control that you will use.
3. Send `samples search=TEXT` to find a sound and its id.
   Completion criterion: Each sound that you will place has one id.

## 3. Act

1. Use `click` and `set` with a control name for every control that `ui` lists.
   Completion criterion: The reply has `"ok":true`, and its `control` shows the new value.
2. If a name matches several controls, add `window=`, `role=`, or `index=`. The
   error message lists the matches.
   Completion criterion: The request selects one control.
3. Use `place`, `move_clips`, `delete_clips`, `select`, `seek`, and `set_loop`
   for the arrangement lanes. Tracks and bars count from 1. Use `automate`
   to replace a track's volume, send, or filter automation lane. Use `clip`
   to set one clip's gain, trim, length, fades, and pitch; a field you leave
   out keeps its value.
   Completion criterion: The reply lists the affected clip ids, or `state`
   shows the track's automation.
4. Use `select sample=` before you open Effect Studio, Step synth, Sample
   editor, or Time Stretcher. The selected sound is the source of these tools.
   Completion criterion: `state` shows the sound in `selection.sample`.
5. When a reply has `"modal":"Choose path"`, send `set target=Path value=FULL_PATH`,
   then `click target="Confirm path"`.
   Completion criterion: The reply has no modal and the status line names the file.
6. After an export or a folder choice, send `wait idle=true`.
   Completion criterion: `state` shows no scan and no export.
7. Use `pointer`, `key`, `type`, and `scroll` only for a gesture that has no
   named control, such as a drag or a context menu. Take positions from `state`.
   Completion criterion: `state` shows the result of the gesture.

To send many requests, write one per line and pipe the lines to `agent -`.
It stops at the first failure.

## 4. Check the result

1. Read `status` in each reply. `"error":true` means that the application
   refused the action, although the request itself succeeded.
   Completion criterion: No action has an unexplained error status.
2. Send `state` and compare the song with the intended result.
   Completion criterion: Tempo, tracks, clips, and mixer values agree.
3. Send `screenshot path=FULL_PATH.png` and inspect the image when appearance matters.
   Completion criterion: The image shows the expected window state.

Do not start playback or recording on the owner's computer unless the task
needs sound. Playback uses the audio device.

## 5. End the session

1. Send `quit`. If the reply has `"modal":"Save your changes?"`, select
   `Save changes` or `Discard changes`.
   Completion criterion: The reply has `"closing":true`, or the process has ended.
2. Remove the temporary data folder and files that the task does not need.
   Completion criterion: Only requested output remains in `tmp/`.

## Completion criterion

The work is complete when the requested song or window state exists, each
action has a checked reply, and the session is closed. Report a control that
`ui` does not list as a defect: each control needs an accessible name.
