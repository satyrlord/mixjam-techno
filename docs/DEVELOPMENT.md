# Development

## Contents

- [Toolchain and folders](#toolchain-and-folders)
- [Version](#version)
- [Checks](#checks)
- [Audio output verification](#audio-output-verification)
- [UI verification](#ui-verification)
- [Visual assets](#visual-assets)

[ARCHITECTURE.md](ARCHITECTURE.md) describes the application boundaries and
[DESIGN.md](DESIGN.md) the visual contract. The repository skills in
`.github/skills/` hold the agent procedures, such as the quality gate.

## Toolchain and folders

The build needs Git with a complete checkout history, Rust, the Visual Studio
C++ build tools, and the Windows SDK. For a shallow clone, run
`git fetch --unshallow` before building.
`tools/runtime/install-rust.ps1` installs Rust in `local-data/toolchain/`, without
changing the user PATH. `tools/runtime/cargo.ps1` runs that toolchain.

The repository holds the public files. Gitignored `local-data/` holds required
private project data and tools, and gitignored `tmp/` holds temporary files.
Preserved project snapshots and bundles remain in `local-data/legacy/`. If
`local-data/research/PRODUCT.md` and `local-data/research/COMPATIBILITY.md`
exist, they are the product and compatibility authorities; a clone works
without them.

## Version

The bottom bar shows the application version with a `v` prefix, such as
`v1.005`, at its right margin. Select it to
open the GitHub repository in the operating system's default browser. The link
requests a new tab; the browser controls tab or window placement. `build.rs`
combines the manually maintained `APP_MAJOR_VERSION` (currently 1) with
`git rev-list --count HEAD`, padded to at least three digits. Five commits gives
`1.005`. Cargo recalculates it when Git references change. Saved `.mixjam` songs
use the same version. The installed application does not need Git. Cargo needs a
semantic package version, so `tools/runtime/cargo.ps1` writes the same version to
`Cargo.toml` and `Cargo.lock` in that form before each run: `1.005` is `1.5.0`.
The major of the package version must equal `APP_MAJOR_VERSION`.

## Checks

~~~powershell
.\tools\runtime\cargo.ps1 build --release --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --no-default-features --locked
.\tools\runtime\cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
.\tools\runtime\cargo.ps1 -CargoArguments @('fmt','--all','--','--check')
python tools/check-rust-format.py
python -m unittest discover -s tools/tests -v
~~~

The format check passes code that rustfmt does not format. Stable rustfmt
leaves a statement as written when it cannot fit a line, such as one with a
long string literal, and it does not visit `src/ui/tests.rs`, which reaches the
crate through `include!`. `tools/check-rust-format.py` formats each Rust file on
its own and reports each region that rustfmt skips. Split a long literal with
`\` line continuations so that every line fits in 100 columns.

The Python tests check Rust literal handling in the format checker, the song
tools in `tools/song/`, the Album Review lookup tool in `tools/album-review/`
(offline), byte preservation of pinned font assets through Git line-ending
conversion, and the `.mixjam` projection used by artifact
verification. The projection reads formats 8 through 24 and preserves sections, the
master pedalboard and its bypass, FX buses and their return lanes, sends, automation,
channel EQs, clip fades, pitch, and reverse, lane duckers, drives, compressors, and
widths, and master and FX
keys when a verification script writes a schema-4 JSON project
for rendering; it gives the Booster pedals their format-16 keys.

## Audio output verification

The optional `audio::output::tests::native_output_buffer_probe` test opens
the default physical output for nine seconds of silence. Run it separately
with `cargo.ps1 -CargoArguments @('test','--lib','native_output_buffer_probe',
'--','--ignored','--nocapture')`. It compares Automatic, 20 ms, and 40 ms
requests and reports unsupported targets, opening failures, callback sizes,
and measured intervals separately. Production output status also reports
the last callback's frames and the last measured callback interval; neither
value is negotiated buffer capacity, which CPAL does not expose. CPAL's
reported timestamp lead is derived from available frames, not an independent
measurement of output latency. This probe does not prove that a heavy song
plays without underruns. Report benchmark limits against the measured or
explicitly modeled callback period.

The Options contract is in [OPTIONS-PAGE-SPEC.md](OPTIONS-PAGE-SPEC.md).
Preference tests cover defaults, section-visibility migration, malformed and
future files, validation, atomic failure, and song/history independence.
Playback quality tests cover Drive and master timing and export separation.
Run the callback benchmark in `src/audio/realtime_bench.rs` in an optimized
build for CPU claims; record workload, rate, cadence, mean, p99, maximum, and
missed deadlines. An accepted larger buffer request does not establish a
longer processing deadline. Physical listening, microphone operation, native
pickers, screen readers, high DPI, and minimum-machine capacity need separate
results. Automated tests and screenshots do not establish those results;
scaled captures cover high-DPI painting only.

The ignored `realtime_options_bench` test covers sparse, dense Drive,
master-heavy, and 64-track synthetic arrangements, with all four quality
combinations at 44.1 and 48 kHz. It measures starts, seeks, replacements, and
uninterrupted playback after eight seconds. The representative `realtime_bench`
accepts `MIXJAM_BENCH_DRIVE` and `MIXJAM_BENCH_MASTER` (`4x` or `off`). Set
`MIXJAM_BENCH_CONTINUOUS=1` to play from the song's start before measuring the
position selected by `MIXJAM_BENCH_START`. Warmup is excluded from the measured
sample; its audio and wall duration are reported. Leave the 10 ms modeled
deadline unchanged. See [OPTIONS-VERIFICATION.md](OPTIONS-VERIFICATION.md)
for the implementation's checks and limitations.

## UI verification

`tools/verify-ui.ps1` captures the built native interface at all three
supported window sizes with isolated synthetic fixtures and settings.
The sections case includes named bookmarks and a long section name.
Options capture views are `options-audio`, `options-workspace`, and `options-files`;
`options-unapplied` adds unapplied edits on two pages, and `options-discard`
shows its Discard changes? prompt. `rail-icons` shows the tool rail without
names, `boards-folded` folds the lower panel to its toolbar row, and
`maximised` hides the tool rail, the Sounds panel and the lower panel so the
score fills the window, `sounds-panel` opens the Sounds panel with the lower
panel on Mixer, and `overview-folded` folds the song overview to its bar.
`inspector-song`, `inspector-track`, `inspector-clip`, and `inspector-clips`
open the Inspector on each of its pages. `automation-menu` opens the Automation
lane menu over the fades-and-automation song, whose track 9 plays a Step synth
sequence saved with its pattern, drawn as notes. Inspect each at 960x720, 1024x768, and 1440x1000. Verify keyboard page selection,
draft editing, Apply, Cancel, Restore options defaults, and focus restoration
through the built app's agent channel. Inspect its accessibility tree separately
from capture appearance.
`-Executable PATH` selects another build, `-Views` a subset of the cases, and
`-Themes` a subset of `studio`, `aero`, and `vintage` (all three by default).
Each Aero and Vintage case starts from an options file with that theme; Studio
cases start without one, which checks the fresh-start default. Captures are
named `theme-view-size`. `-Scales` adds display scales (`1.5`, `1.75`, `2`):
each scaled case sets `MIXJAM_TECHNO_CAPTURE_ZOOM`, which the application reads
only during a capture. It multiplies egui's pixels per point and the window
size, so the capture keeps the case's size in points, paints with the matching
artwork density, and is named `theme-view-size@scalex`. On a 3440x1440 display
at 100%, 1024x768 fits at 1.75 and 960x720 at 2. A scaled capture does not
change the Windows display scale, so it does not test the operating system's
DPI messages or a monitor change.
Mixer cases cover ordinary, 64-track, empty, and gain/polarity/routing states,
and a track's EQ window; Mastering cases include a bypassed board.
The capture set also covers Help, Sample libraries, Import sound, Import details,
Save your changes?, Sample conflicts, Album Review (the grid, a filtered grid,
an album page, and a missing reviews folder), and the four tool windows, with expanded
effects and provider settings and missing-source processing states. It has no
case for the tool Help windows or Choose path. Recorded-take captures
check the window layout with a synthetic presentation state; they do not record audio.

Every in-app tool window, modal, board, and the Mixer use the current theme.
Studio paints flat software panels, Aero molded plastic, and Vintage the Folded
Metal artwork. Each tool keeps its palette in every theme: Time Stretcher
marine, Sample Editor granite, Effect Studio red, Step synth olive, Mastering
indigo, and neutral Utility dialogs. `src/ui/theme.rs` owns the roles, materials,
and fonts; `src/ui/dialog.rs` owns the shared resizable shell; `src/ui/controls.rs`
owns reusable buttons, header icons, rotaries, sliders, value fields, and switches.
Collapse keeps a window in place and preserves its expanded size and edits.
Collapsing a blocking dialog releases the workspace and pauses its pending action.
Native operating-system file pickers retain their system presentation.

The ignored `arrangement_frame_time` test times the Arrange view on a song of
64 tracks by 256 bars with waveforms at each lane height and at the zoom limits.
Run it optimized with `cargo.ps1 -CargoArguments @('test','--release','--locked',
'--lib','arrangement_frame_time','--','--ignored','--nocapture')`. Each case's
p95 frame time must stay under 16.7 ms.

A body that overflows its window shows a scroll bar, and short blocking dialogs
follow the height of their content. Scaled captures verify high-DPI painting,
not Windows display-scale changes. The captures do not verify screen readers or
other accessibility, physical audio, or native file pickers.

## Visual assets

Visual assets are transparent PNGs painted at one image pixel per screen pixel.
Prepared density variants support Windows scaling. SVG files and embedded SVG
artwork are prohibited in tracked and non-ignored project files. Run
`python tools/assets/check-raster-assets.py` as part of the quality gate. Original
legacy archives and required dependency caches stay in ignored `local-data/`;
temporary files stay in ignored `tmp/`. The asset check respects Git ignore rules.
