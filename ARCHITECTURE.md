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
catalog accessors are read-only. Construction and mutations validate the project.
Typed `EditCommand` values change tempo, loop regions, clips, routing, and mixer
settings. Placement and group movement use explicit operations. Successful edits
update history and invalidate the render in the application layer.

The frontend owns selection, drag gestures, zoom, layout, open windows, search
filters, and tool form drafts. It converts pointer positions into requested
positions and sends them to the application. It does not own an `Editor`, change
the live project, decode audio, write projects, or control a device directly.
Native file dialogs return paths for application operations. Screenshot encoding
is isolated presentation verification code.

Sample-processing forms hold drafts until the user requests playback or saving.
The application validates and processes those drafts. It also owns synth-pattern
file parsing, generated audio registration, import resolution, and settings files.

`src/main.rs` is the composition root. It resolves command-line options and builds
the initial library/project before it creates a session. Headless CLI commands
can use library services directly. The GUI modules are compiled into the native
binary, not exported by the production Rust library.

## Safe frontend iteration

Change layout, typography, colors, and controls in `src/ui/view.rs`. Keep tool
input and file-dialog adapters in `src/ui.rs`. Add a typed operation in
`application.rs` when a design needs new application behavior. Do not expose a
mutable project or add a callback that can change arbitrary application state.

Run the headless suite to verify the application without the GUI feature. Run the
full suite for native pointer, keyboard, selection, scrolling, and accessible
action checks. Source guards reject direct frontend imports of implementation
modules, selected mutation and I/O patterns, and public GUI exports. Private
session state and immutable accessors enforce state ownership at compile time.
The headless build verifies operation without GUI dependencies enabled; the
source guard is not a general dependency-graph checker.

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

Font assets are derived from the imported Space Grotesk WOFF2 files. Run
`python tools/assets/prepare-fonts.py --check` to verify reproducible native TTFs.
