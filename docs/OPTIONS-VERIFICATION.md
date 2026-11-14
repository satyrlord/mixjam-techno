# Options verification

Implementation date: 2026-11-02. Application version: v1.062. Song format
remains 23. Application options use schema 1 in a separate `options.json`.
The owner's implementation request accepted the recommended baseline in
[OPTIONS-PAGE-SPEC.md](OPTIONS-PAGE-SPEC.md).

## Delivered behavior

Options opens from the gear at the foot of the tool rail or Ctrl+,. Audio, Workspace,
Files, and Sample libraries share one window. Application preferences share
one draft, Apply, Cancel, Restore options defaults, and stale-draft Reload.
Session owns validation, persistence, active values, and audio changes.
Options do not enter song history. Follow and the starting module take effect
at the next launch; section visibility shares its current value with the toolbar.

Drive and master saturation have separate playback quality choices. Exports
always prepare full quality. Preview gain affects preview playbacks only. Reduced
meters hold display values for at least 1/15 second, retain peak events, and
leave audio analysis active. New fixed folders must exist when selected;
an accepted folder that later disappears keeps its path and uses the documented
fallback without blocking unrelated preference changes.

The owner's follow-up folded the complete Sample Libraries feature into the
fourth page. Folder and provider actions use their existing operations;
Apply and Cancel affect the options draft only. The standalone window, its
rail action, and the books PNGs are retired. The gear uses the existing
canonical-raster pipeline and its prepared 20-pixel PNG.

Audio changes require stopped playback, previews, and recording. Buffer Apply
prepares a silent stream on the same device and format, writes settings, then
adopts the stream. Any preparation or write failure retains active values.
Invalid files need an explicit Restore before replacement; future schemas block
persistent writes. The old section-bookmark file remains intact.

## Automated and native checks

| Check | Result |
|---|---|
| Default-feature workspace and all-target tests | 909 passed; 17 optional tests ignored |
| No-default-feature workspace and all-target tests | 640 passed; 17 optional tests ignored |
| Clippy, workspace and all targets, warnings denied | Pass |
| Cargo format check and the independent Rust format check | Pass; 91 Rust files |
| Python tools tests | 145 passed |
| Release build | Pass |
| Fonts, rail icons, app icon, Folded Metal, mixer assets, raster rule | Pass |
| Options captures at 960x720, 1024x768, and 1440x1000 | 24 captures passed and inspected, including library states and first launch |
| Built-app agent checks | 72 requests passed |
| Full-quality artifact comparison | 20 renders matched the pre-change release byte for byte |
| Stem export isolation | Four quality combinations produced identical stem sets |
| Corrupt and future settings under headless rendering | Output unchanged; files preserved |
| Song conversion | Format-23 save/reopen preserved lane data |
| Representative source resolution in the release executable | Zero runtime missing sources; input file unchanged |

The native requests covered draft cancellation, repeated activation, Apply,
no-op Apply, Restore, both quality choices, all buffer targets, Workspace and
Files controls, preview settings, and lower Audio controls through scrolling.
They added a second library through the page's path chooser and verified that
Cancel and Restore options defaults retained the adopted folders. Only Options
was listed as the owning window. They also forced a settings-file write failure
after stream preparation. Active
options and their revision stayed unchanged; the existing output remained selected.
Song data stayed unchanged throughout the requests. The library projection
changed only when the separate library operations adopted the new folders.

Keyboard tests cover page selection, value editing, Apply, Restore, Cancel,
Tab and Shift+Tab containment, higher-window Escape ownership, and focus
restoration. Accessibility-tree checks cover names, selected values, disabled
reasons, retained descriptions, and the meter's OVER state. These checks do not
establish screen-reader behavior.

An independent review found three pointer hints that bypassed the tooltip
preference. Those sites were corrected and tested. A follow-up review found
no further confirmed defects in the revised focus and meter-display paths.
The full suite also found and verified repairs for the minimum-size rail and
an overwritten accessible meter description. A separate review of the library
fold found no feature loss. All 239 UI tests passed after that integration.

The close-out review inspected the modified and new files, recent history,
and whitespace checks. It found no additional confirmed defects. The changes
remain uncommitted. The verified release executable's SHA256 is
`2174fc0c0c8c1a47352718e5d97766227812347f780f25bc745ebd98d839a218`.

## Buffering, Drive, and playback performance

The physical output buffering, Drive tradeoff, and playback performance measurements are in
[their own file](options-verification/performance.md).

## Manual limits and handoff

| Procedure | Status |
|---|---|
| Physical output stream opening and silent buffer probe at 48 kHz | Passed |
| Physical listening to bass, bright tones, drums, and dense mixes | Not available |
| Physical playback at 44.1 kHz | Not available; modeled conversion is tested |
| Microphone and recorded-input operation | Not available; recording guards use fixtures |
| Native Windows file pickers | Not available; the agent path chooser is tested |
| Screen reader | Not available; the accessibility tree is tested |
| Display scaling of 150% and 200% | Out of scope by the owner's decision |
| Clean minimum-spec machine | Not available; the owner has not selected one |

The largest remaining blind spot is audible heavy-song playback on the owner's
minimum supported hardware. Check both quality modes with bass, bright tones,
drums, and dense mixes at 44.1 and 48 kHz. Listen during starts, seeks, and short
loops. Test a screen reader and the native pickers separately. No hardware
capacity or release-readiness claim follows from these checks.

## Lane height follow-up, 2026-11-08

The Lane height preference adds 20, 30, and 40 px choices, with 30 px as the
default. Existing schema 1 files without `laneHeight` load that default.
Options remain at schema 1; the current song format is 24. The earlier results
above describe the 2026-11-02 implementation.

The review corrected overlapping clip names and reverse marks in compact
rows, and a new-track drop hint that disappeared at 20 px. Both new rendering
tests failed before the repairs and passed afterward. They check visible
glyph bounds in every theme at every lane height. The persistence field and
its migration default are now included in the specification's stored fields.

| Check | Result |
|---|---|
| Default-feature workspace and all-target tests | 986 passed; 26 optional tests ignored |
| No-default-feature workspace and all-target tests | 686 passed; 26 optional tests ignored |
| Clippy, warnings denied | Pass |
| Both Rust format checks | Pass; independent check covered 98 Rust files |
| Python tools tests | 152 passed |
| Fonts and all documented raster-asset checks | Pass |
| Release build | Pass; application v1.080 |
| Arrangement and Workspace captures | 18 passed and inspected: all three themes at 960x720, 1024x768, and 1440x1000 |
| Compact and tall lane captures | Six passed and inspected: 20 and 40 px in all three themes at 960x720 |
| Built-app agent verification | 45 checks passed through 117 requests |
| Independent final review | No further confirmed defects after the stored-fields correction |

Native checks used isolated settings on Windows 11 Pro build 26300. They
covered draft isolation, Apply, saved values, unchanged song content, Cancel
with discarded edits, Restore defaults, and persistence after restart.
Native interaction did not start playback or recording. Screen readers remain
unverified. Check compact labels and track controls with a screen reader
before claiming that form of accessibility. These checks do not establish minimum-machine support
or audio parity.
