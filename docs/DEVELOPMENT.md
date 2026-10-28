# Development

## Contents

- [Toolchain and folders](#toolchain-and-folders)
- [Version](#version)
- [Checks](#checks)
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
tools in `tools/song/`, and the `.mixjam` projection used by artifact
verification. The projection reads formats 8 through 16 and preserves the
master pedalboard and its bypass, FX buses, sends, automation, channel EQs, and
clip fades and pitch when a verification script writes a schema-4 JSON project
for rendering; it gives the Booster pedals their format-16 keys.

## UI verification

`tools/verify-ui.ps1` captures the built native interface at all three
supported window sizes with isolated synthetic fixtures and settings.
`-Executable PATH` selects another build and `-Views` a subset of the cases.
Mixer cases cover ordinary, 64-track, empty, and gain/polarity/routing states,
and a track's EQ window; Mastering cases include a bypassed board.
The capture set also covers Help, Sample libraries, Import sound, Import details,
Save your changes?, Sample conflicts, and the four tool windows, with expanded
effects and provider settings and missing-source processing states. It has no
case for the tool Help windows or Choose path. Recorded-take captures
check the window layout with a synthetic presentation state; they do not record audio.

Every in-app tool window and modal uses Folded Metal: subtly aged painted metal,
dark inset control bays, cream text, polished silver knobs, and pale metal primary
buttons. Time Stretcher stays dark marine cyan, Sample Editor dark granite gray,
Effect Studio synth red, and Step synth dark olive. Utility dialogs use neutral
metal with orange-yellow accents. The chassis has no decorative screws; a smooth
folded lip, texture, and light wear provide its hardware character.
`src/ui/dialog.rs` owns the shared resizable shell; `src/ui/controls.rs` owns
reusable buttons, header icons, rotaries, sliders, value fields, and switches.
Collapse keeps a window in place and preserves its expanded size and edits.
Collapsing a blocking dialog releases the workspace and pauses its pending action.
The main graphite workspace, Mixer, and native operating-system file pickers
retain their existing presentation.

A body that overflows its window shows a scroll bar, and short blocking dialogs
follow the height of their content. The captures do not verify high-DPI
scaling, screen readers or other accessibility, physical audio, or native file
pickers.

## Visual assets

Visual assets are transparent PNGs painted at one image pixel per screen pixel.
Prepared density variants support Windows scaling. SVG files and embedded SVG
artwork are prohibited in tracked and non-ignored project files. Run
`python tools/assets/check-raster-assets.py` as part of the quality gate. Original
legacy archives and required dependency caches stay in ignored `local-data/`;
temporary files stay in ignored `tmp/`. The asset check respects Git ignore rules.
