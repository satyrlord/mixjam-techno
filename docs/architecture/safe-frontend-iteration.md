# Safe frontend iteration

Put each frontend change in the module that owns it:

- Workspace layout: `src/ui/view/`.
- Mixer presentation: `src/ui/mixer/`.
- The rows of the Mix view: `src/ui/mix_view.rs`.
- The geometry and lifecycle of the floating shell: `src/ui/dialog.rs`.
- Shared control rendering and interaction: `src/ui/controls/`.
- Tool window bodies: `src/ui/stretch.rs`, `src/ui/effects.rs`, and
  `src/ui/sample_editor.rs`.
- Board bodies: `src/ui/mixer.rs`, `src/ui/master.rs`, `src/ui/fx.rs`, and
  `src/ui/step_synth/`. The Step synth pattern edits are in
  `src/synth/edit.rs`, apart from the board.
- Tool action adapters: `src/ui/tools.rs`.
- Board action adapters: `src/ui/view/boards.rs`.
- File-dialog adapters: `src/ui/paths.rs`.

Use the same shell and controls for each in-app window and modal. Keep the
domain behavior in its caller.

If a design needs new application behavior, add a typed operation in
`application.rs`. Do not expose a mutable project. Do not add a callback that
can change arbitrary application state.

## Checks

Run the commands in [Checks](../DEVELOPMENT.md#checks):

- The headless suite (`--no-default-features`) checks the application without
  the GUI feature.
- The full suite checks native pointer and keyboard input, selection,
  scrolling, and accessible actions.

Source guards read each non-test file under `src/ui/`, and also `src/ui.rs` and
`src/ui_icons.rs`. Thus the guards cover a new frontend file immediately. The
guards reject these items:

- Frontend imports and inline paths that name an implementation module, also
  in `src/ui/capture.rs`.
- Selected mutation and I/O patterns. `src/ui/capture.rs` is the exception,
  because it writes its own image.
- Public GUI exports.

Private session state and read-only accessors enforce state ownership at
compile time. The headless build checks the operation without GUI
dependencies. The source guard does not check the full dependency graph.

PowerShell removes a bare `--` from positional script arguments. For each
command that gives arguments after `--`, use the `-CargoArguments` form.

After asset or source changes, run the raster asset check in
[Visual assets](../DEVELOPMENT.md#visual-assets).

## Captures

To check the built window, use `tools/verify-ui.ps1` as
[UI verification](../DEVELOPMENT.md#ui-verification) tells. Native egui tests
check control input and the accessibility semantics that the controls emit.
They do not replace a screen reader check of the built executable.

## Fonts and titles

[Typography](../design/typography.md) and [Themes](../design/themes.md) own
the faces of each theme, the transliteration and uppercase rules for titles,
and the pinned heading line height. The frontend formats titles without a
change to saved project names, paths, or window IDs. The font check is in
[assets/fonts/README.md](../../assets/fonts/README.md).
