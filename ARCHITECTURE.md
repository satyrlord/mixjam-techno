# Application boundary

The Rust library is the backend. The native executable contains the frontend.
`src/application.rs` is the frontend's only application API. It has no egui,
window, file-dialog, or screenshot dependency.

```text
Native views and input                  Headless application
src/ui/view.rs + src/ui/stretch.rs       src/application.rs
             + src/ui/effects.rs
             + src/ui/step_synth.rs
             + src/ui/dialog.rs
             + src/ui/controls.rs
             + src/ui/sample_editor.rs
             + src/ui.rs
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

Track gain is a nonnegative amplitude and polarity is a separate persistent
boolean. Playback and export use `Track::effective_gain()` to apply the sign.
The Mixer presents `20 * log10(amplitude)` dB; zero amplitude is negative
infinity. Track and Master model admission is 0 through 4, while new
Mixer edits normally stop at +12 dB (about 3.9811 amplitude). Earlier exact
track values of 4 survive load and focus. Clip gain remains 0 through 2.
Changing polarity ends a gain or pan gesture and creates a discrete undo
step. Silence retains polarity; Unity changes only amplitude.

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

`src/ui/dialog.rs` owns the shared Folded Metal shell and lifecycle for all
in-app tools, utility windows, and blocking dialogs. It owns header layout,
position, edge resizing, open/collapsed geometry, focus layers, stable identities,
and accessible modal naming. The caller owns the task and decides what an action
or cancellation means. Native operating-system windows and file pickers are outside
this shell.

`src/ui/controls.rs` owns reusable control presentation and interaction: buttons,
header icons, rotaries, sliders, numeric values, and switches. Roles and states
share the same rendering and keyboard/accessibility behavior. Rotary full travel
is 150 logical points; Shift applies fine adjustment to new movement. Parameter
specifications retain ranges, reset values, logarithmic mapping, and discrete
choices. Tool bodies own content and drafts, without independent window skins or
button implementations.

The shared chassis uses subtly aged folded painted metal, dark inset control
bays, cream lettering, polished silver knobs, and pale metal primary buttons.
`controls::Palette` supplies Utility, Marine, Granite, Red, and Olive variants.
Time Stretcher uses Marine; Sample Editor uses Granite; Effect Studio uses Red;
Step synth uses Olive. Utilities use neutral metal with orange-yellow accents.
Prepared material and control PNGs live in `assets/folded-metal/<palette>/{100,150,200}/` and the shared
`assets/folded-metal/controls/{100,150,200}/`.
The chassis contains no decorative screws or fasteners. Its continuous
antialiased perimeter and bevel are native geometry. Raster material fades before
the outer outline and at strip ends to avoid visible joints. Knob edges use 8x
supersampling and one premultiplied-alpha Lanczos reduction per density.
Fixed details fade to transparent over a matching solid chassis color. Resizing
extends the solid regions; painting aligns one texel to one physical pixel. Text
and input remain live, and content reflows or scrolls instead of scaling artwork
or controls. Palette variants share geometry and interaction.

Collapse retains position, expanded size, and task state. Collapsed blocking
dialogs release workspace and agent input without executing their pending action.
`src/ui.rs` owns blocker precedence, paused save continuation, and guards against
replacement of pending file requests. Choose path suspends its parent confirmation
until the path request resolves. A close action cancels a confirmation, without
implicitly saving or discarding. Recorded take can collapse while its unsaved
in-memory take remains available for explicit Save take or Discard take.

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
`src/ui/sample_editor.rs` owns the Sample Editor body and native action responses.
It uses the shared shell and controls for frame fields, pitch slider, buttons,
header, and feedback. Numeric formatting does not change the draft.

Frame drafts retain unsigned 64-bit values, with an inclusive start and exclusive end. Pitch keeps the
existing -24 to +24-semitone UI range. Numeric display rounding does not alter
the draft. Background selections do not reset trim while the editor is collapsed.
On expansion, a changed active source ID or catalog revision applies the existing
trim reset. Expanded source-change behavior remains unchanged, so a replaced
source cannot retain stale bounds.
`src/ui.rs` adapts Listen edit, Save edited sound, and Import details. Existing
Session trim and resampling remain synchronous. Preview uses existing PCM
playback; save uses the existing WAV workflow, registers the result in Wave
at the source BPM, and selects it. Neither the panel nor the artwork performs
file or audio operations. Missing sources and invalid ranges disable processing
with explicit tooltip reasons; the footer shows range and duration or recovery
guidance. This presentation does not establish original-product DSP parity.

`src/ui/stretch.rs` owns Time Stretcher's content and action responses inside the
shared shell. Source, tempo, processing, preview, and save feedback use shared
controls. Its Help action stays available in both open and collapsed states.

`src/ui.rs` adapts its source selections, Listen, Save, and Import details
requests to existing application operations; the panel does not render audio or
write files. Opening it from the tool rail initializes the original BPM draft
from the selected sample's source tempo.
The application owns Time Stretcher's background source read and processing job.
Each request captures its source catalog and tempo values, rejects duplicate
work, and retains one consumable result. Library or song replacement invalidates
that result. The frontend retains the requested action and rejects results after
the tool closes or its source, tempos, or catalog change. It opens the existing
save workflow or requests PCM preview only for a current result. Preview state
and a revision identify whether that tool's audition is still playing; a later
preview, transport change, or stop cannot inherit its active indicator.
Save completion updates the local feedback and the selected output's source
tempo; cancelling a file choice does not report success.

`src/ui/mixer.rs` owns Mixer's compact graphite and ivory hardware presentation.
Each strip is 108 logical points wide. Master remains outside the horizontally
scrolling occupied-track bank. Native controls paint live labels, dB values,
rail marks, pan, routing, polarity, focus, and hover. Gain uses a piecewise dB
taper with 72% of travel between -24 and +12 dB; the bottom section maps linear
amplitude to exact silence. Unity sits at 0 dB on the gain rail. Center aligns
with the pan midpoint just below its rail. Polarity has a separate
slashed-circle button. Control-specific tooltips and the Help window carry
detailed guidance.
Prepared material tiles and fader caps live in `assets/mixer/{100,150,200}/`.
The fader is first reduced to a 48 by 64-pixel canonical grid, then prepared
at 24 by 32, 36 by 48, and 48 by 64 pixels. Runtime painting aligns one texel
to one physical pixel, with no texture resizing. The empty bank gives one
instruction and leaves Master available.

The frontend's `BottomModule` enum selects Sounds or Mixer. `src/ui/view.rs`
owns their shared 28-point header and fixed left-side `Bottom module` combo.
Its width is 136 points and the selected module uses 16-point bold Syncopate.
The combo keeps its control ID and horizontal anchor across module changes.
Sounds filters render inline to its right. Each module renders only its body
below that header. The Mixer tool-rail shortcut selects Mixer rather than
toggling between modules. Selection and shared panel height are frontend
session state. Switching preserves resized height; opening Mixer raises a
shorter panel to its 370-point minimum. Sounds retains its 180-point minimum.
Later modules can extend the enum and the same selector.

`src/ui/effects.rs` owns Effect Studio's sections and action responses. It uses
shared rotaries, value entry, switches, action buttons, header, and status styling.
Keep labels, values, and dial marks separate. Verify rendered bounds at all three
prepared densities; source coordinates alone do not establish visual acceptance.

`src/ui.rs` adapts Listen, Save sound, Reset, Help, and Import details. Listen
and Save sound call `Session::processed_pcm` synchronously; Listen then uses the
existing PCM preview. Save uses the existing file request and
`Session::save_pcm`, registers the WAV in Effect at the source BPM, and selects
the saved sound. Source changes clear local feedback; save choice, cancellation,
failure, and success update it. Reset restores `EffectsState::default`.
Turning an optional section off discards its settings; enabling it uses that
section's defaults. This presentation does not change the algorithms or the
shared `effects_controls` form used by Step synth. It does not add a background
processing job or establish original-product DSP parity.

`src/ui/step_synth.rs` owns Step synth's source, bar, grid, effects, and action
content inside the shared shell. Text, grid cells, switches, and input stay live.
The pitch grid and replacement effects form have bounded scrolling; source,
step labels, actions, and feedback remain reachable. The grid exposes sixteen
steps and twenty-five pitch rows. The shared `effects_controls` form retains its
algorithms and ranges and uses the common controls.

`src/ui.rs` owns the source path, viewed bar, effects-view selection, draft,
and local feedback. It adapts the panel's actions to existing `Session`
operations. An explicit WAV or PXD path overrides palette or clip selection,
including missing palette audio. Selecting it does not read audio.
`Session::step_synth_pcm` reads the source and renders synchronously when Listen
sequence or Save sequence is requested. The frontend then requests PCM preview
or the existing WAV save workflow. Save registers the result in Step synth at
the draft BPM and selects it. Save choice, cancellation, failure, and success
update local feedback. Missing palette audio offers Import details.
Pattern save/load uses `Session::save_pattern` and `Session::load_pattern`.
It replaces only the four-bar draft; the source path and viewed bar remain.
Clear bar clears only the viewed sixteen steps. The visible +12 through -12
pitch rows do not narrow the existing -48 through +48 serialized-note range.
This presentation does not add a worker or establish original-product DSP parity.

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

## Agent control

`gui --agent` lets a local program operate the window. The channel has two parts
on the two sides of the application boundary.

`src/agent.rs` is backend code without GUI dependencies. It defines the typed
requests and replies, encodes them as JSON lines, serves them on the loopback
interface, and contains the `agent` command-line client. It queues each request
for the frontend and wakes it. It does not interpret a request. Each session
has a new key in `agent-PORT.key` in the data folder, which only the user can
read. A connection that does not start with the key, or that sends a malformed
line, is closed. A request from another user, or one that another program was
tricked into sending, such as an HTTP form post, never reaches the queue. A request that the
window does not take within 30 seconds fails and can no longer run.

`src/ui/agent.rs` is the frontend driver. `eframe` calls it before each frame.
It reads the controls from the accessibility tree that egui produced for the
previous frame, and it acts through that frame's input: an accessibility action
for a named control, or pointer and keyboard events for a position. A request
replies after the frames that show its result. Arrangement requests call the
same `Session` operations as the lane gestures, because the lanes are one painted
surface without separate controls. The driver holds no project state and
reaches the backend only through `application.rs`; the source guard covers it.

egui does not mark the controls behind a modal as disabled, and an
accessibility action reaches them. Each modal therefore names itself with
`name_dialog`, and the driver refuses a control outside the open modal.

Every file choice is a `FileRequest`. Without an agent, `ask_path` shows the
native dialog and carries the request out at once. With an agent, the request
waits in the Choose path modal, because an agent cannot operate a native dialog
and a blocked window cannot reply. A typed path gets the same result as a
chosen one, including the file extension that a save dialog adds.

A control is usable by an agent, and by a screen reader, only when it has an
accessible name. Give a new control visible label text, `widget_info`, or
`name_control`. Keep each name unique among the controls that can be on screen
together: the shared panel names its combo `Bottom module`, while the tool
rail has a `Mixer` shortcut. Sound-library filters keep their own names.
Add a control that is painted on a shared surface to the
arrangement requests instead.

## Safe frontend iteration

Change workspace layout in `src/ui/view.rs` and Mixer presentation in
`src/ui/mixer.rs`. Change floating shell geometry and lifecycle in
`src/ui/dialog.rs`, and shared control rendering and interaction in
`src/ui/controls.rs`. Tool bodies remain in `src/ui/stretch.rs`,
`src/ui/effects.rs`, `src/ui/step_synth.rs`, and `src/ui/sample_editor.rs`.
Keep tool action and file-dialog adapters in `src/ui.rs`. Reuse the same shell
and controls for every in-app window and modal; keep domain behavior in its caller.

Add a typed operation in
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

Syncopate Bold 700 is bundled unchanged from a pinned Google Fonts commit.
The frontend transliterates branding and title letters to Latin counterparts
and formats them in uppercase without changing saved project names or paths.
Inter Medium 500 serves all other text. Window IDs remain independent of visible
title formatting.
Run `python tools/assets/prepare-fonts.py --check` to verify both font and license
hashes, static weights, and ASCII title coverage.
