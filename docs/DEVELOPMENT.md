# Development

## Contents

- [Toolchain and folders](#toolchain-and-folders)
- [Version](#version)
- [Checks](#checks)
- [Audio output verification](development/verification.md#audio-output-verification)
- [UI verification](development/verification.md#ui-verification)
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
Node.js. Run `npm ci` once to install the pinned Markdown linter and the
duplication tool in the gitignored `node_modules/` folder.

For a shallow clone, run `git fetch --unshallow` before the build.
`tools/runtime/install-rust.ps1` installs Rust in `local-data/toolchain/`, and
does not change the user PATH. `tools/runtime/cargo.ps1` runs that toolchain.
`tools/runtime/install-quality-tools.ps1` adds the pinned measurement tools of
[Change measurements](#change-measurements) to it.
`tools/runtime/install-rust.ps1` and `tools/verify-ui.ps1` need PowerShell 7.

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
`1.005` is `1.5.0`. It replaces a file whole, and only if the version in the
file is different. The major number of the package version must be equal to
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

- Stable rustfmt keeps a statement as written if the statement cannot fit a
  line. An example is a statement with a long string literal.
- Rustfmt does not visit a file that the crate reaches only through
  `include!`.

`src/lib.rs` includes `src/ui.rs` for the native UI tests, in the module
`ui_test_harness`. To run only the UI tests, use that name as the test filter.
`src/main.rs` also declares `src/ui.rs` as a module, so rustfmt formats it and
follows its `#[path]` test modules. `tools/check-rust-format.py` formats each
tracked or new Rust file separately, and reports each region that rustfmt
skips. Thus it also checks a file that only `include!` reaches. Split a long
literal with `\` line continuations, so that each line fits in 100 columns.

The Python tests check these items:

- The Rust literal handling of the format checker.
- The raster asset check.
- The song tools in `tools/song/`.
- The Album Review lookup tool in `tools/album-review/`, offline.
- The byte preservation of the pinned font assets through the Git line-ending
  conversion.
- The `.mixjam` projection that artifact verification uses.
- The change measurements of `tools/measure-change.py`.

The projection reads formats 8 through 29. When a verification script writes a
schema-4 JSON project for a render, the projection keeps these items:

- The sections, the master pedalboard and its bypass.
- The FX buses and their return lanes, the sends, and the automation.
- The channel EQs, and the clip fades, pitch, and reverse.
- The lane duckers, drives, compressors, widths, and Filter settings.
- The master keys and the FX keys.

It gives the Booster pedals their format-16 keys.

### Change measurements

Run this command to measure complexity and duplication on an uncommitted
change without running coverage:

~~~powershell
python tools/measure-change.py
~~~

When the owner explicitly invokes `run-quality-gate` and asks for a full gate
or coverage run, unlock `cargo llvm-cov` and pass `--lcov` for CRAP scoring:

~~~powershell
$env:MIXJAM_ALLOW_LLVM_COV = '1'
.\tools\runtime\cargo.ps1 llvm-cov --workspace --all-targets --locked --lcov --output-path tmp/measure/lcov.info
python tools/measure-change.py --lcov tmp/measure/lcov.info
~~~

`tools/runtime/cargo.ps1` blocks `llvm-cov` unless `MIXJAM_ALLOW_LLVM_COV`
equals `1`.

| Tool | Pinned version | Measures |
| --- | --- | --- |
| cargo-llvm-cov | 0.9.2 | Explicit-only line coverage of the default run |
| rust-code-analysis-cli | 0.0.25 | The cyclomatic complexity of each function |
| jscpd | 5.4.3 | Copied token runs of 50 tokens or more in `src/` |

`tools/measure-change.py` compares the worktree with `HEAD`, and includes the
untracked Rust files. In default fast mode it checks cyclomatic complexity
against limit 10 and reports each jscpd clone with a changed line. With
`--lcov`, it computes `complexity² × (1 − coverage)³ + complexity` (limit 10).
It skips test files, `#[cfg(test)]` modules, and inline `#[cfg(test)]` items.
It warns if a changed file is newer than the coverage file. It exits 1 if it
finds an issue, Git cannot read the change, or a required tool or requested
coverage file is missing. Use `--base REV` to compare with a different revision.

## Visual assets

Visual assets are transparent PNGs, painted at one image pixel for each screen
pixel. Each asset has one prepared PNG size, for 100% display scaling, the only
scale in scope. Do not use SVG files or embedded SVG artwork in tracked and
non-ignored project files.

Run `python tools/assets/check-raster-assets.py` after asset or source changes,
and as part of the quality gate. The asset check obeys the Git ignore rules.
Original legacy archives and necessary dependency caches stay in the ignored
`local-data/` folder. Temporary files stay in the ignored `tmp/` folder.
