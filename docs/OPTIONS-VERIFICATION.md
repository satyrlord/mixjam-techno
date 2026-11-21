# Options verification

This file gives the verification procedure for the contract in
[OPTIONS-PAGE-SPEC.md](OPTIONS-PAGE-SPEC.md). Application options use schema 1
in a separate `options.json`.

## Behavior to verify

The checks below must confirm these points of the contract:

- Options opens from the gear at the left end of the status bar, or with
  Ctrl+,. Audio, Workspace, and Files share one window. Files opens the
  separate Sample libraries window.
- The preferences share one draft, with Apply, Cancel, Restore all defaults,
  and `Reload active options` for a stale draft. Options do not enter the
  song history.
- Exports always prepare full quality. The preview level changes only preview
  playback.
- Reduced meters hold display values for a minimum of 1/15 second. They keep
  peak events, and the audio analysis continues.
- A fixed folder must exist when the user selects it. If the folder is not
  available later, the application keeps its path and uses the fallback. This
  does not block changes to other preferences.
- The folder and provider actions of the Sample libraries window use their own
  operations. Apply and Cancel change only the options draft.
- Buffer Apply prepares a silent stream on the same device and format, writes
  the settings, and then adopts the stream. A preparation or write failure
  keeps the active values.
- A file that is not valid needs an explicit Restore before replacement. A
  future schema blocks persistent writes. The older section-bookmark file
  stays as it is.

## Automated and native checks

Run the commands in [DEVELOPMENT.md](DEVELOPMENT.md#checks). Then run the
Options captures of `tools/verify-ui.ps1` (the `options-audio`,
`options-workspace`, `options-files`, `options-unapplied`, and
`options-discard` views) at the three window sizes. Inspect each capture.
Confirm these items:

- A full-quality artifact comparison with different preference files matches
  the earlier release byte for byte.
- The four quality combinations make identical stem sets.
- A file of settings that is not valid, or a future one, does not change
  headless rendering. The files stay as they are.
- Song conversion keeps the lane data after a save and a reopen.
- A representative song in the release executable has zero missing sources at
  runtime, and the input file does not change.

The native requests through the agent channel must cover these operations:

- Draft cancellation, repeated activation, Apply, no-op Apply, and Restore.
- Both quality choices and all buffer targets.
- The Workspace and Files controls, and the preview settings.
- The lower Audio controls, reached through scrolling.
- A second library, added through the path chooser of the page. Cancel and
  Restore all defaults keep the adopted folders. The Options window
  does not own the library change.
- A forced failure of the settings-file write after stream preparation. The
  active options and their revision do not change, and the existing output
  stays selected.

The song data must not change during the requests. The library projection
changes only when the library operations adopt the new folders.

Keyboard tests cover page selection, value editing, Apply, Restore, and
Cancel. They also cover Tab and Shift+Tab containment, Escape ownership by a
higher window, and focus restoration. Accessibility-tree checks cover names,
selected values, disabled reasons, kept descriptions, and the OVER state of
the meter. These checks do not prove screen-reader behavior.

## Lane height checks

Two rendering tests check the visible glyph bounds in each theme at each lane
height. They cover clip names and reverse marks in compact rows, and the drop
hint for a new track at 20 px. They are
`lane_heights_keep_reversed_clip_names_clear_of_the_marker` and
`lane_heights_keep_the_new_track_drop_hint_visible`.

Run the native checks with isolated settings. Cover these items:

- Draft isolation, Apply, and saved values.
- Song content that does not change.
- Cancel with discarded edits, and Restore defaults.
- Persistence after a restart.

Do not start playback or recording in these checks.

## Buffering, Drive, and playback performance

The procedures for the physical output buffering, the Drive tradeoff, and the
playback performance are in
[their own file](options-verification/performance.md).

## Manual checks

| Procedure | Status |
| --- | --- |
| Physical listening to bass, bright tones, drums, and dense mixes | Not available |
| Physical playback at 44.1 kHz | Not available. A test covers the modeled conversion. |
| Microphone and recorded-input operation | Not available. Fixtures test the recording guards. |
| Native Windows file pickers | Not available. A test covers the agent path chooser. |
| Screen reader | Not available. A test covers the accessibility tree. |
| Clean minimum-spec machine | Not available. The owner has not selected one. |

The largest open check is audible playback of a heavy song on the minimum
supported hardware. Check both quality modes with bass, bright tones, drums,
and dense mixes at 44.1 and 48 kHz. Listen during starts, seeks, and short
loops. Test a screen reader and the native pickers separately. Before you
claim screen-reader accessibility, check the compact labels and the track
controls with a screen reader. These checks do not prove a hardware capacity,
audio parity, or release readiness.
