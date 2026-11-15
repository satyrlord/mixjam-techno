# Options verification

This file records the verification evidence for the contract in
[OPTIONS-PAGE-SPEC.md](OPTIONS-PAGE-SPEC.md). Application options use schema 1
in a separate `options.json`. Each table names the application version that
it verified.

## Verified behavior

The checks below confirm these points of the contract:

- Options opens from the gear at the left end of the status bar, or with
  Ctrl+,. Audio, Workspace, Files, and Sample libraries share one window.
- The preferences share one draft, with Apply, Cancel, Restore options
  defaults, and Reload for a stale draft. Options do not enter the song
  history.
- Exports always prepare full quality. The preview level changes only preview
  playback.
- Reduced meters hold display values for a minimum of 1/15 second. They keep
  peak events, and the audio analysis continues.
- A fixed folder must exist when the user selects it. If the folder is not
  available later, the application keeps its path and uses the fallback. This
  does not block changes to other preferences.
- The folder and provider actions of the Sample libraries page use their own
  operations. Apply and Cancel change only the options draft.
- Buffer Apply prepares a silent stream on the same device and format, writes
  the settings, and then adopts the stream. A preparation or write failure
  keeps the active values.
- A file that is not valid needs an explicit Restore before replacement. A
  future schema blocks persistent writes. The older section-bookmark file
  stays as it is.

## Automated and native checks, v1.062

| Check | Result |
|---|---|
| Default-feature workspace and all-target tests | 909 passed; 17 optional tests ignored |
| No-default-feature workspace and all-target tests | 640 passed; 17 optional tests ignored |
| Clippy, workspace and all targets, warnings denied | Pass |
| Cargo format check and the independent Rust format check | Pass; 91 Rust files |
| Python tools tests | 145 passed |
| Release build | Pass |
| Fonts, rail icons, app icon, Folded Metal, mixer assets, raster rule | Pass |
| Options captures at 960x720, 1024x768, and 1440x1000 | 24 captures passed and inspected, with library states and first launch |
| Built-app agent checks | 72 requests passed |
| Full-quality artifact comparison | 20 renders matched the earlier release byte for byte |
| Stem export isolation | Four quality combinations made identical stem sets |
| Settings that are not valid, and future settings, under headless rendering | Output did not change; files kept |
| Song conversion | Save and reopen kept the lane data |
| Source resolution of a representative song in the release executable | Zero missing sources at runtime; input file did not change |

The native requests covered these operations:

- Draft cancellation, repeated activation, Apply, no-op Apply, and Restore.
- Both quality choices and all buffer targets.
- The Workspace and Files controls, and the preview settings.
- The lower Audio controls, reached through scrolling.
- A second library, added through the path chooser of the page. Cancel and
  Restore options defaults kept the adopted folders. Only Options was the
  owning window.
- A forced failure of the settings-file write after stream preparation. The
  active options and their revision did not change, and the existing output
  stayed selected.

The song data did not change during the requests. The library projection
changed only when the library operations adopted the new folders.

Keyboard tests cover page selection, value editing, Apply, Restore, Cancel,
Tab and Shift+Tab containment, Escape ownership by a higher window, and focus
restoration. Accessibility-tree checks cover names, selected values, disabled
reasons, kept descriptions, and the OVER state of the meter. These checks do
not prove screen-reader behavior.

## Lane height checks, v1.080

The Lane height preference has the choices 20, 30, and 40 px, with 30 px as
the default. A schema 1 file without `laneHeight` loads that default.

Two rendering tests check the visible glyph bounds in each theme at each lane
height. They cover clip names and reverse marks in compact rows, and the drop
hint for a new track at 20 px.

| Check | Result |
|---|---|
| Default-feature workspace and all-target tests | 986 passed; 26 optional tests ignored |
| No-default-feature workspace and all-target tests | 686 passed; 26 optional tests ignored |
| Clippy, warnings denied | Pass |
| Both Rust format checks | Pass; the independent check covered 98 Rust files |
| Python tools tests | 152 passed |
| Fonts and all documented raster-asset checks | Pass |
| Release build | Pass |
| Arrangement and Workspace captures | 18 passed and inspected: all three themes at 960x720, 1024x768, and 1440x1000 |
| Compact and tall lane captures | Six passed and inspected: 20 and 40 px in all three themes at 960x720 |
| Built-app agent verification | 45 checks passed through 117 requests |

The native checks used isolated settings on Windows 11 Pro build 26300. They
covered draft isolation, Apply, saved values, song content that did not
change, Cancel with discarded edits, Restore defaults, and persistence after a
restart. The native checks did not start playback or recording.

## Buffering, Drive, and playback performance

The measurements of the physical output buffering, the Drive tradeoff, and the
playback performance are in
[their own file](options-verification/performance.md).

## Manual checks

| Procedure | Status |
|---|---|
| Physical output stream opening and silent buffer probe at 48 kHz | Passed |
| Physical listening to bass, bright tones, drums, and dense mixes | Not available |
| Physical playback at 44.1 kHz | Not available; the modeled conversion is tested |
| Microphone and recorded-input operation | Not available; the recording guards use fixtures |
| Native Windows file pickers | Not available; the agent path chooser is tested |
| Screen reader | Not available; the accessibility tree is tested |
| Clean minimum-spec machine | Not available; the owner has not selected one |

The largest open check is audible playback of a heavy song on the minimum
supported hardware. Check both quality modes with bass, bright tones, drums,
and dense mixes at 44.1 and 48 kHz. Listen during starts, seeks, and short
loops. Test a screen reader and the native pickers separately. Before you
claim screen-reader accessibility, check the compact labels and the track
controls with a screen reader. These checks do not prove a hardware capacity,
audio parity, or release readiness.
