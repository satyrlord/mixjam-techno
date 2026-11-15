# Development

## Contents

- [Toolchain and folders](#toolchain-and-folders)
- [Version](#version)
- [Checks](#checks)
- [Audio output verification](#audio-output-verification)
- [UI verification](#ui-verification)
- [Visual assets](#visual-assets)

[ARCHITECTURE.md](ARCHITECTURE.md) describes the application boundaries.
[DESIGN.md](DESIGN.md) describes the visual contract. The repository skills in
`.github/skills/` hold the agent procedures, for example the quality gate.

## Toolchain and folders

The build needs these items:

- Git with a complete checkout history.
- Rust.
- The Visual Studio C++ build tools.
- The Windows SDK.

The checks also need Python and Node.js with npm. The application does not use
Node.js. Run `npm ci` once to install the pinned Markdown linter in the
gitignored `node_modules/` folder.

For a shallow clone, run `git fetch --unshallow` before the build.
`tools/runtime/install-rust.ps1` installs Rust in `local-data/toolchain/`, and
does not change the user PATH. `tools/runtime/cargo.ps1` runs that toolchain.

The repository holds the public files. The gitignored `local-data/` folder holds
the necessary private project data and tools. The gitignored `tmp/` folder holds
temporary files. Preserved project snapshots and bundles are in
`local-data/legacy/`. If `local-data/research/PRODUCT.md` and
`local-data/research/COMPATIBILITY.md` exist, they are the product and
compatibility authorities. A clone works without them.

## Version

The status bar shows the application version at its right margin, with a `v`
prefix, for example `v1.005`. Select it to open the GitHub repository in the
default browser of the operating system. The link requests a new tab, and the
browser controls the tab or window placement.

`build.rs` combines the manually kept `APP_MAJOR_VERSION` (value 1) with
`git rev-list --count HEAD`, padded to a minimum of three digits. Five commits
give `1.005`. Cargo calculates the version again when the Git references
change. Saved `.mixjam` songs use the same version. The installed application
does not need Git.

Cargo needs a semantic package version. Thus `tools/runtime/cargo.ps1` writes
the same version in that form to `Cargo.toml` and `Cargo.lock` before each run:
`1.005` is `1.5.0`. The major number of the package version must be equal to
`APP_MAJOR_VERSION`.

## Checks

~~~powershell
.\tools\runtime\cargo.ps1 build --release --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --no-default-features --locked
.\tools\runtime\cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
.\tools\runtime\cargo.ps1 -CargoArguments @('fmt','--all','--','--check')
python tools/check-rust-format.py
python -m unittest discover -s tools/tests -v
npm run lint:md
~~~

`npm run lint:md` runs markdownlint-cli2 on each Markdown file that Git tracks
or does not ignore. `.markdownlint-cli2.jsonc` holds the rules. Prose lines
have a maximum of 80 columns. Tables, code blocks, and headings have no limit.
`npm run lint:md:fix` corrects the issues that have an automatic fix.

The format check passes some code that rustfmt does not format:

- Stable rustfmt keeps a statement as written when the statement cannot fit a
  line, for example a statement with a long string literal.
- Rustfmt does not visit a file that the crate gets to only through
  `include!`.

`src/lib.rs` includes `src/ui.rs` for the native UI tests. `src/main.rs` also
declares `src/ui.rs` as a module, so rustfmt formats it and follows its
`#[path]` test modules. `tools/check-rust-format.py` formats each tracked or
new Rust file separately, and reports each region that rustfmt skips. Thus it
also checks a file that only `include!` gets to. Split a long literal with `\`
line continuations, so that each line fits in 100 columns.

The Python tests check these items:

- The Rust literal handling of the format checker.
- The song tools in `tools/song/`.
- The Album Review lookup tool in `tools/album-review/`, offline.
- The byte preservation of the pinned font assets through the Git line-ending
  conversion.
- The `.mixjam` projection that artifact verification uses.

The projection reads formats 8 through 25. When a verification script writes a
schema-4 JSON project for a render, the projection keeps these items:

- The sections, the master pedalboard and its bypass.
- The FX buses and their return lanes, the sends, and the automation.
- The channel EQs, and the clip fades, pitch, and reverse.
- The lane duckers, drives, compressors, and widths.
- The master keys and the FX keys.

It gives the Booster pedals their format-16 keys.

## Audio output verification

The optional `audio::output::tests::native_output_buffer_probe` test opens the
default physical output for nine seconds of silence. Run it separately with
`cargo.ps1 -CargoArguments @('test','--lib','native_output_buffer_probe',
'--','--ignored','--nocapture')`. It compares the Automatic, 20 ms, and 40 ms
requests. It reports these items separately: unsupported targets, open
failures, callback sizes, and measured intervals.

The production output status also reports the frames of the last callback and
the last measured callback interval. These values are not the negotiated
buffer capacity, which CPAL does not expose. CPAL calculates its timestamp lead
from the available frames. The lead is not an independent measurement of the
output latency. This probe does not prove that a heavy song plays without
underruns. Report benchmark limits against the measured or explicitly modeled
callback period.

[OPTIONS-PAGE-SPEC.md](OPTIONS-PAGE-SPEC.md) gives the Options contract. The
preference tests cover these items:

- The defaults and the section-bookmarks reading.
- Malformed and future files, and the validation.
- An atomic write failure.
- The independence of the preferences from the song and the history.

The playback quality tests cover the Drive and master timing, and the
separation of the export. For CPU claims, run the callback benchmark in
`src/audio/realtime_bench.rs` in an optimized build. Record the workload, rate,
cadence, mean, p99, maximum, and missed deadlines. An accepted larger buffer
request does not give a longer processing deadline.

Physical listening, microphone operation, native pickers, screen readers, and
minimum-machine capacity need separate results. Automated tests and screenshots
do not give those results.

The ignored `realtime_options_bench` test covers these synthetic arrangements:
sparse, dense Drive, master-heavy, and 64-track. It uses all four quality
combinations at 44.1 and 48 kHz. It measures starts, seeks, replacements, and
uninterrupted playback after eight seconds.

The representative `realtime_bench` accepts `MIXJAM_BENCH_DRIVE` and
`MIXJAM_BENCH_MASTER` (`4x` or `off`). Set `MIXJAM_BENCH_CONTINUOUS=1` to play
from the start of the song before the measurement of the position that
`MIXJAM_BENCH_START` selects. The measured sample excludes the warmup. The
benchmark reports the audio and wall duration of the warmup. Do not change the
10 ms modeled deadline. [OPTIONS-VERIFICATION.md](OPTIONS-VERIFICATION.md)
gives the checks and limits of the implementation.

## UI verification

`tools/verify-ui.ps1` captures the built native executable at the three
supported window sizes: 1440x1000, 1024x768, and 960x720. It makes synthetic WAV
and project fixtures, and uses isolated settings. Inspect each capture
visually. It has these parameters:

- `-Executable PATH` selects another build.
- `-OutputDirectory PATH` selects the capture folder. The default is
  `tmp\evidence\ui\redesign`.
- `-Views` selects a subset of the cases.
- `-Themes` selects a subset of `studio`, `aero`, and `vintage`. The default is
  all three.
- `-Sizes` selects a subset of the three window sizes.
- `-Jobs N` sets how many cases run at the same time. The default is half the
  processors, with a maximum of 8.

Each case runs in its own process, with its own data folder. Each Aero and
Vintage case starts from an options file with that theme. Studio cases start
without an options file, which checks the default of a fresh start. Captures
have the name `theme-view-size`. Captures run at a display scale of 100% only.
Display scaling of 150% and 200% is out of scope.

The capture cases include these views:

- `sections` has named bookmarks and a long section name.
- `options-audio`, `options-workspace`, and `options-files` show the Options
  pages. `options-unapplied` adds unapplied edits on two pages.
  `options-discard` shows its Discard changes? prompt.
- `rail-icons` shows the tool rail without names.
- `boards-folded` folds the lower panel.
- `maximised` hides the tool rail, the Sounds panel, the Inspector, and the
  lower panel, so that the score fills the window.
- `sounds-panel` opens the Sounds panel with the lower panel on Mixer.
- `overview-folded` folds the song overview to its bar.
- `inspector-song`, `inspector-track`, `inspector-clip`, and `inspector-clips`
  open the Inspector on each of its pages.
- `automation-menu` opens the Automation lane menu over the song with fades and
  automation. Track 9 of that song plays a Step synth sequence that was saved
  with its pattern, and the clip shows the notes.
- `command-list` opens the command list over the default song.
- `mix-view` shows the Mix view of the score, with the lower panel on
  Mastering. `mix-view-dense` fills it with the full song. `mix-view-empty`
  shows it with no tracks. `mix-view-states` uses the song with the Mixer
  states. `mix-view-eq` adds the EQ window of the first track.

The Mixer cases cover these states:

- An ordinary arrangement and a full 64-track bank.
- An empty arrangement.
- Attenuation, silence, and unity gain.
- Independent polarity, pan, and output routing.
- The EQ window of a track.

The Mastering cases include a bypassed board. The capture set also covers Help,
Sample libraries, Import sound, Import details, Save your changes?, and Sample
conflicts. The Album Review cases are the grid, a filtered grid, an album page,
and a missing reviews folder. The set covers the three tool windows and the
Step synth board, with expanded effects, provider settings, and the processing
states of a missing source. It has no case for the Help windows of the tools or
for Choose path. Recorded-take captures check the window layout with a
synthetic presentation state. They do not record audio.

Through the agent channel of the built application, verify keyboard page
selection, draft edits, Apply, Cancel, Restore options defaults, and focus
restoration. Inspect the accessibility tree separately from the capture
appearance. [Native frontend](architecture/frontend.md) describes the themes,
the shared shell and controls, and the collapse behavior that the captures
show.

The ignored `arrangement_frame_time` test times the Arrange view on a song of
64 tracks by 256 bars with waveforms, at each lane height and at the zoom
limits. Run it optimized with `cargo.ps1 -CargoArguments @('test','--release',
'--locked','--lib','arrangement_frame_time','--','--ignored','--nocapture')`.
The p95 frame time of each case must stay below 16.7 ms.

A body that overflows its window shows a scroll bar. Short blocking dialogs
follow the height of their content. The captures do not verify screen readers
or other accessibility, physical audio, or native file pickers.

## Visual assets

Visual assets are transparent PNGs, painted at one image pixel for each screen
pixel. Each asset has one prepared PNG size, for 100% display scaling, the only
scale in scope. SVG files and embedded SVG artwork are prohibited in tracked and
non-ignored project files.

Run `python tools/assets/check-raster-assets.py` after asset or source changes,
and as part of the quality gate. The asset check obeys the Git ignore rules.
Original legacy archives and necessary dependency caches stay in the ignored
`local-data/` folder. Temporary files stay in the ignored `tmp/` folder.
