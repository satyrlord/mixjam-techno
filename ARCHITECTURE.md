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
edits update history and mark the render for update.

The frontend owns selection, drag gestures, zoom, layout, open windows, search
filters, and tool form drafts. It converts pointer positions into requested
positions and sends them to the application. It does not own an `Editor` or
change the live project. It does not decode audio, write projects, or control a
device directly.
Native file dialogs return paths for application operations. Screenshot encoding
is isolated presentation verification code.

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
and accessible actions. Source guards reject direct frontend imports of
implementation modules. They also reject selected mutation and I/O patterns
and public GUI exports. Private session state and read-only accessors enforce
state ownership at compile time. The headless build checks operation without
GUI dependencies. The source guard does not check the full dependency graph.

```powershell
.\tools\runtime\cargo.ps1 test --workspace --all-targets --no-default-features --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --locked
.\tools\runtime\cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
.\tools\runtime\cargo.ps1 fmt --all -- --check
```

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
