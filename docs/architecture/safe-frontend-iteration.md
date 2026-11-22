# Safe frontend iteration

Put each frontend change in the module that owns it:

- Workspace layout: `src/ui/view/`.
- Mixer presentation: `src/ui/mixer/`.
- The Mix view: `src/ui/mix_view/`. Its column table is `layout.rs`. A row
  takes plain values and reports what it changed. Thus a layout change needs no
  change to the session.
- The type sizes: `theme::type_size`.
- The geometry and lifecycle of the tool shell: `src/ui/dialog.rs`.
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

A mixer widget draws in a `Look`. It uses `Look::Strip(scale)` on a Mixer strip
or in the Inspector, and `Look::Quiet` in a Mix view row. Change how a control
looks in one place. Keep its name, keys, and hint the same in both looks.

A mixer view edits a `TrackMix`, not a whole `Track`, and sends it as
`EditCommand::SetTrackMix`. Domain rules such as `TrackMix::silence` and
`Track::processing_on` are in the backend, so a view only shows them.

The backend also owns these rules. A view uses them and does not restate
them:

- Ranges and units: `TEMPO_RANGE`, `SOURCE_BPM`, `MAX_PITCH_CENTS`,
  `MAX_CLIP_GAIN` with the clip gain decibel functions, `TICKS_PER_BEAT`,
  `frames_per_tick_at`, `tick_seconds`, and `MIX_SAMPLE_RATE`.
- Sections: `Project::sections_with_new`, `sections_without`, and
  `sections_renamed` give the list for `EditCommand::SetSections`.
  `Project::song_span` is the whole-song loop.
- Clip edges: `Project::clip_room` is the space that a clip can grow into
  before it overlaps a neighbor. `SetClip` checks the same rule.
- Rows: `Project::occupied_tracks` gives the tracks that the views show.
- Automation: `Lane::snap` and `automation::snap_return` give the value grid.
  `automation::set_point` and `moved_point` keep one point for each tick, in
  order.
- FX presets: `FxBus::load_preset` and `FxBus::matching_preset`.
- FX monitoring and the Drive transfer curve: `Session::fx_meters`,
  `Session::fx_solo`, `Session::toggle_fx_solo`, and `Session::fx_drive_curve`.
- Duckers: `Ducker::set_key` stops a ducker from using its own track as its
  key.
- The board delay: `MasterChain::latency_ms`.

If a design needs new application behavior, add a typed operation in
`application.rs`. Do not expose a mutable project. Do not add a callback that
can change arbitrary application state.

## Checks

Run the commands in [Checks](../DEVELOPMENT.md#checks):

- The headless suite (`--no-default-features`) checks the application without
  the GUI feature.
- The full suite checks native pointer and keyboard input, selection,
  scrolling, and accessible actions.

The source guard is the test
`frontend_source_can_only_reach_the_application_boundary` in
`src/application/boundary_tests/frontend.rs`. It reads each non-test file under
`src/ui/`, and also `src/ui.rs` and `src/ui_icons.rs`. Thus the guard covers a
new frontend file immediately. The guard rejects these items:

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

## Test layers

Write each test at the lowest layer that can show the behavior:

1. Pure logic: a layout function or a domain rule, with no `egui::Ui`. For
   example, `mix_view::layout` tests and `TrackMix::silence`.
2. Component: one widget or row on a bare `egui::Context`, from plain values,
   with no app and no session. For example,
   `src/ui/tests/mix_view_row_tests.rs` paints a track row from a `TrackMix`
   and a `RowFacts`.
3. Session contract: a typed command on a headless `Session`, in
   `src/application/boundary_tests/`.
4. Window: the full app in `src/ui/tests/`, for input, focus, and layout that
   need the whole window.

In tests, query a painted frame through `src/ui/tests/screen.rs`, not through
its shapes: `screen(&output).text("Muted").color()`,
`screen(&output).within(row).shows("35")`, and
`screen(&output).node("Track 1 EQ").toggled()`. `Text::color` gives the
color that the code asks for. `Text::opacity` gives the opacity that the
painter applies to the glyphs.

## Captures

To check the built window, use `tools/verify-ui.ps1` as
[UI verification](../development/verification.md#ui-verification) tells.
Native egui tests check control input and the accessibility semantics that the
controls emit. They do not replace a screen reader check of the built
executable.

## Fonts and titles

[Typography](../design/typography.md) and [Themes](../design/themes.md) own
these rules:

- The faces of each theme.
- The transliteration and uppercase rules for titles.
- The pinned heading line height.

The frontend formats titles. It does not change saved project names, paths, or
window IDs. The font check is in
[assets/fonts/README.md](../../assets/fonts/README.md).
