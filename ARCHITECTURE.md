# Application boundary

The Rust library is the backend. The native executable contains the frontend.
`src/application.rs` is the frontend's only application API. It has no egui,
window, file-dialog, or screenshot dependency.

```text
Native views and input                  Headless application
src/ui/view.rs + src/ui.rs               src/application.rs
        |                                       |
        | immutable project/catalog views       | Editor and history
        | typed edits and service calls         | Library and project files
        +-------------------------------------->| Render queue and transport
                                                | Recording and sample processing
```

## Ownership

`Session` owns the editor, catalog, project file, dirty state, render revisions,
audio player, microphone capture, and pending recorded take. Its project and
catalog accessors are read-only. It validates the project when it starts and
after each change. Typed `EditCommand` values change tempo, loop regions, clips,
routing, and mixer settings. Separate operations place and move clips. Successful
edits update history and mark the render for update. An accepted command that
changes nothing is not an edit: it leaves history, the saved state, and the render
unchanged. Changes to one gain, pan, or clip control that arrive within 400 ms
of each other form one history entry, so a slider drag is one undo step. Saving
ends that group, so undo can return to the saved state after the next edit. A clip
cannot be lengthened over the next clip on its track.

Project copies share one imported-sample list until an edit changes it, so a
history entry or render request costs the same for any palette size.

`audio::Mix` is a project prepared for rendering: its clips resolved to output
frames, with their decoded sources. It mixes any range of the song on demand.
The player reads it in blocks on the audio thread, without allocation or locks,
and export writes it block by block, so neither holds a buffer of the whole
song and song length is bounded only by the timeline. The render worker prepares
the mix, keeps the decoded sources of the last one, and discards them when the
library is replaced. `Session::start_export` renders and writes on its own
thread and reports through `poll`; `Session::export` stays synchronous for
headless callers. `Session::catalog_revision` advances when the library is
replaced; the frontend keeps its filtered palette until then. Folder adoption
takes providers that are already loaded from the current library and reads a
provider from disk only when its folder is gone.

`src/atomic_file.rs` owns file replacement. Projects, settings, the scan cache,
synth patterns, and WAV exports are staged beside the destination and renamed
over it. A failed write leaves the existing file intact.

MIX import caches resolved paths and audio metadata. Only the pending asset list
owns retained audio bytes, and distinct payloads share its 500 MB limit. Saving
a song reads the previous document header through a bounded stream to preserve
its creation time. The completed serialized song must fit the reader's 1 GiB
limit before it can replace the destination.

The frontend owns selection, drag gestures, zoom, layout, open windows, search
filters, and tool form drafts. It converts pointer positions into requested
positions and sends them to the application. It does not own an `Editor` or
change the live project. It does not decode audio, write projects, or control a
device directly.
Native file dialogs return paths for application operations. Screenshot encoding
is isolated presentation verification code.

Visual artwork uses transparent PNGs only. Embedded SVG artwork and SVG files
are prohibited in tracked and non-ignored project files.
`tools/assets/check-raster-assets.py` uses Git's file inventory and respects
ignore rules. Original archives and the Rust dependency
cache reside in `local-data/`; temporary files reside in `tmp/`. Runtime PNG textures
use prepared density variants with one texel per physical screen pixel; views
snap their bounds to the pixel grid and do not resize them.

Sample-processing forms hold drafts until the user requests playback or saving.
The application validates and processes those drafts. It also owns synth-pattern
file parsing, generated audio registration, import resolution, and settings files.

Missing audio is explicit project state. The renderer leaves those placements
silent. The frontend reads the retained source metadata and sends a chosen WAV
path to `Session::resolve_missing_sample`. That operation resolves every matching
source identity in one history change, preserves musical trims, and retains the
chosen audio. The picker and context menu remain frontend responsibilities.

Duplicate file matches are stored in `Project.sample_conflicts`, with a stable
default, candidate paths, and source IDs. Readers select the first candidate.
`Session::resolve_sample_conflict` changes all affected placements atomically,
and updates history and rendering. The native modal exposes choices and playback;
confirmation and selected paths persist in the project. Transactions remove
conflicts that no longer affect any clip.

`src/main.rs` is the composition root. It resolves command-line options and builds
the initial library/project before it creates a session. Normal GUI startup then
scans the WAV folder through `Session`; a queued song opens only if no intervening
project change or unsaved take requires its cancellation. Successful folder
adoption tells the frontend to clear selections and active drags. Headless CLI
commands can use library services directly. The GUI modules are compiled into the native
binary, not exported by the production Rust library.

Folder scans stream validated WAV samples into the catalog identity hash. Each
worker retains a reader buffer, not the decoded audio. File lists, metadata, and
the probe cache still grow with the corpus. A scan writes the cache only after
new probes change it. `source_tempo.rs` resolves folder tempo from matching
original audio before the duration-based fallback is used. Optional supplemental
tempo identities are embedded at build time, like the native identity catalogs.

## Safe frontend iteration

Change layout, typography, colors, and controls in `src/ui/view.rs`. Keep tool
input and file-dialog adapters in `src/ui.rs`. Add a typed operation in
`application.rs` when a design needs new application behavior. Do not expose a
mutable project or add a callback that can change arbitrary application state.

Run the headless suite to check the application without the GUI feature. Run the
full suite to check native pointer and keyboard input, selection, scrolling,
and accessible actions. Source guards reject frontend imports and inline paths
that name an implementation module, including those in `src/ui/capture.rs` and
`src/ui_icons.rs`. They also reject selected mutation and I/O patterns
and public GUI exports. Private session state and read-only accessors enforce
state ownership at compile time. The headless build checks operation without
GUI dependencies. The source guard does not check the full dependency graph.

```powershell
.\tools\runtime\cargo.ps1 test --workspace --all-targets --no-default-features --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --locked
.\tools\runtime\cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
.\tools\runtime\cargo.ps1 -CargoArguments @('fmt','--all','--','--check')
```

PowerShell removes a bare `--` from positional script arguments. Use the
`-CargoArguments` form for any command that passes arguments after `--`.

`tools/verify-ui.ps1` checks the built native executable at 1440 by 1000,
1024 by 768, and 960 by 720. It creates synthetic WAV/project fixtures and uses
isolated settings. These captures need visual inspection. They do not test
physical audio, native file-picker interaction, high DPI, or screen readers.
Mixer capture cases include a full 64-track bank, an empty arrangement, and
negative/zero/unity gain, pan, and output-routing states. Native egui tests check
control input and emitted accessibility semantics. They do not replace a screen
reader check of the built executable.

Syncopate Bold 700 is bundled unchanged from a pinned Google Fonts commit.
The frontend transliterates branding and title letters to Latin counterparts
and formats them in uppercase without changing saved project names or paths.
Inter Medium 500 serves all other text. Window IDs remain independent of visible
title formatting.
Run `python tools/assets/prepare-fonts.py --check` to verify both font and license
hashes, static weights, and ASCII title coverage.
