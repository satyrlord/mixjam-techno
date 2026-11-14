# Safe frontend iteration

Change workspace layout in `src/ui/view/`, Mixer presentation in
`src/ui/mixer/`, and the Mix view's rows in `src/ui/mix_view.rs`. Change
floating shell geometry and lifecycle in `src/ui/dialog.rs`, and shared control
rendering and interaction in `src/ui/controls/`. Tool bodies remain in
`src/ui/stretch.rs`, `src/ui/effects.rs`, `src/ui/step_synth.rs`, and
`src/ui/sample_editor.rs`. Keep tool action adapters in `src/ui/tools.rs` (the
lower-panel boards' in `src/ui/view/boards.rs`) and file-dialog adapters in
`src/ui/paths.rs`. Reuse the same shell and controls for every in-app window
and modal; keep domain behavior in its caller.

Add a typed operation in
`application.rs` when a design needs new application behavior. Do not expose a
mutable project or add a callback that can change arbitrary application state.

Run the headless suite to check the application without the GUI feature. Run the
full suite to check native pointer and keyboard input, selection, scrolling,
and accessible actions. Source guards read every non-test file under `src/ui/`, plus `src/ui.rs` and
`src/ui_icons.rs`, so a new frontend file is covered at once. They reject
frontend imports and inline paths that name an implementation module,
including those in `src/ui/capture.rs`. They also reject selected mutation and
I/O patterns, except in `src/ui/capture.rs`, which writes its own image, and
public GUI exports. Private session state and read-only accessors enforce
state ownership at compile time. The headless build checks operation without
GUI dependencies. The source guard does not check the full dependency graph.

```powershell
.\tools\runtime\cargo.ps1 test --workspace --all-targets --no-default-features --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --locked
.\tools\runtime\cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
.\tools\runtime\cargo.ps1 -CargoArguments @('fmt','--all','--','--check')
python tools/check-rust-format.py
```

`tools/check-rust-format.py` reports Rust code that the format check passes but
rustfmt does not format: statements with a line too long to fit, and files that
reach the crate only through `include!`.

PowerShell removes a bare `--` from positional script arguments. Use the
`-CargoArguments` form for any command that passes arguments after `--`.

Run `python tools/assets/check-raster-assets.py` after asset or source changes.

`tools/verify-ui.ps1` checks the built native executable at 1440 by 1000,
1024 by 768, and 960 by 720. It creates synthetic WAV/project fixtures and uses
isolated settings. These captures need visual inspection. They do not test
physical audio, native file-picker interaction, high DPI, or screen readers.
Mixer capture cases include a full 64-track bank, an empty arrangement, and
attenuation/silence/unity gain, independent polarity, pan, and output-routing
states. Native egui tests check
control input and emitted accessibility semantics. They do not replace a screen
reader check of the built executable.

Syncopate Bold 700 sets the product name in every theme. Each theme adds a
heading face and a text face (see `docs/DESIGN.md`): IBM Plex Sans SemiBold
with Inter Medium for Studio, Michroma with Fira Sans Medium for Aero, and
Barlow Condensed SemiBold with Barlow Medium for Vintage. Inter is every
theme's symbol fallback. All files are unchanged from pinned sources. The
frontend transliterates branding and title letters to Latin counterparts and
formats them in uppercase without changing saved project names or paths.
Heading rows are pinned to one line height in every theme, so the layout does
not move. Window IDs remain independent of visible title formatting.
Run `python tools/assets/prepare-fonts.py --check` to verify font and license
hashes, static weights, families, embedding permission, ASCII coverage, and
Romanian coverage of text faces.
