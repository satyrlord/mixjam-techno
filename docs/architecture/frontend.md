# Native frontend

## Themes

`src/ui/theme.rs` owns the three application themes: each theme's color roles,
its material (flat, plastic, or metal), corner radii, and fonts, and the code
painters for plastic surfaces, gradients, flat and molded dials, and fader
caps. Each frame sets a thread-local current theme from the option;
`theme::adopt` rebuilds the egui style and fonts only when the theme changes.
Renderers read roles, and only shared painters branch on the material;
interaction and parameter code never depend on the theme.

## Shared tool shell

`src/ui/dialog.rs` owns the shared tool shell and lifecycle for all
in-app tools, utility windows, and blocking dialogs. It owns header layout,
position, edge resizing, open/collapsed geometry, focus layers, stable identities,
and accessible modal naming. The caller owns the task and decides what an action
or cancellation means. Native operating-system windows and file pickers are outside
this shell.

## Shared controls and the chassis

`src/ui/controls.rs`, with its `src/ui/controls/` modules, owns reusable control
presentation and interaction: buttons, header icons, rotaries, sliders,
numeric values, switches, and the scrolling status text of tool footers. Roles
and states share the same rendering and keyboard/accessibility behavior.
Rotary full travel is 150 logical points; Shift applies fine adjustment to
new movement. Parameter specifications retain ranges, reset values,
logarithmic mapping, and discrete choices. Tool bodies own content and drafts,
without independent window skins or button implementations.

The shared chassis uses subtly aged folded painted metal, dark inset control
bays, cream lettering, polished silver knobs, and pale metal primary buttons.
`controls::Palette` supplies Utility, Marine, Granite, Red, Olive, and Indigo variants.
Time Stretcher uses Marine; Sample Editor uses Granite; Effect Studio uses Red;
Step synth uses Olive; Mastering uses Indigo. Utilities use neutral metal with orange-yellow accents.
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

## Collapse and blockers

Collapse retains position, expanded size, and task state. Collapsed blocking
dialogs release workspace and agent input without executing their pending action.
`src/ui/modals.rs` owns blocker precedence and the paused save continuation.
`src/ui/paths.rs` guards against replacement of pending file requests. Choose
path suspends its parent confirmation
until the path request resolves. A close action cancels a confirmation, without
implicitly saving or discarding. Recorded take can collapse while its unsaved
in-memory take remains available for explicit Save take or Discard take.
A deferred file or close action checks recording again when it resumes. If a
recording started while its confirmation was collapsed, the action stays
pending and the recording remains intact until the user stops it.

## File dialogs and artwork

Native file dialogs return paths for application operations. Screenshot encoding
is isolated presentation verification code.

Visual artwork uses transparent PNGs only. Embedded SVG artwork and SVG files
are prohibited in tracked and non-ignored project files.
`tools/assets/check-raster-assets.py` uses Git's file inventory and respects
ignore rules. Original archives and the Rust dependency
cache reside in `local-data/`; temporary files reside in `tmp/`. Runtime PNG textures
use prepared density variants with one texel per physical screen pixel; views
snap their bounds to the pixel grid and do not resize them.

## Waveform peaks and the Sounds panel

`src/peaks.rs` holds read-only waveform peaks for library sounds: the lowest and
highest sample of every 64 frames, all channels together, and coarser tables
of four times as many frames each until a few buckets remain. Views ask
`Session::sample_peaks(id)`, which answers `Pending`, `Ready`, or `Unavailable`
and never decodes on the UI thread. At each `Session::poll`, the pending
sounds that views asked for since the last poll, in the order asked, replace the
queue of one lazily started worker thread, which reads them through
`Library::read_pcm`. A pending sound that left the screen before the worker
reached it is forgotten and asked for again when it returns. A decoder panic
marks that sound unavailable; a worker that stops is replaced with its list.
When the catalog changes, as when a library folder is added or removed, a
sound keeps its table while its source, its place in the file, and the file's
size and modified time are the same; only that metadata is read on the UI
thread. Any other sound is read again. `Session::poll` collects
finished tables and keeps the view refreshing while any are pending.
`src/ui/sounds.rs` draws the Sounds panel from those tables, with the
library's search, filters, sort, and sound actions.
`src/ui/waveform.rs` turns a ready table into one column pair per point for an
arrangement clip, following the trim, Reverse, the fades, and where the sound
ends, and paints the visible columns as one mesh. `src/ui/view/score_lanes.rs`
builds these shapes per clip, `MixJamApp` keeps them, and one is rebuilt only
when its key changes: the sound, trim,
length, Reverse, fades, points per tick (zoom), frames per tick (tempo),
catalog revision, or peak table. The catalog revision prevents a replacement
sound from reusing a stale waveform when its peak table reuses an address.
A clip of a Step synth sound with a pattern beside its WAV draws notes instead:
`src/ui/view/clip_paint.rs` loads each such pattern once per catalog
revision, and `waveform::ClipNotes` places the active bars' notes on the clip's ticks. Note
positions use the pattern's render tempo for source frames and the catalog's
effective playback tempo for song ticks.

## Command registry, lanes, Inspector, and overview

`src/ui/commands.rs` holds the command registry: each command's name, group,
shortcut, rail icon and action. The rail draws its groups from `RAIL`, the
status bar's switches and the keyboard run their commands through
`run_command`, and the command list shows `Command::all()`. A test fails when a
rail icon has no registered command.
`src/ui/lanes.rs` owns the Arrange lane header's layout constants, the lane
name derived from the first clip's sound family, and the lane fader.
`src/ui/room.rs` decides each frame which of the Sounds panel, the Inspector,
and the lower panel's board show: the saved layout says which are wanted, the
measured window says which fit together, and the order they were opened in
breaks a tie. It never changes the saved layout.
`src/ui/inspector.rs` draws the Inspector. Its page follows the clip and track
selection, and its fields reuse the Mixer's controls and the clip menu's
fields under their own names, editing through `SetTrack` and `SetClip`.
`SetTrack` replaces one track, so a view that edits one track copies only that
track each frame, not all of them.
Its page and control identities include the selection and frontend generation,
so pending value entries and held controls cannot edit another selection or a
replacement project.
`src/ui/overview.rs` draws the song overview above the score. It is laid out
before the score toolbar, so a click moves this frame's scroll offset, and
painted after the lanes from the offset they just used. Its clip lines are one
mesh, kept until the project change token, catalog revision, theme, strip, or
span changes. It reads the song and never edits it. When neither the strip nor
its folded bar fits beside the required score area, it hides until space
returns and keeps the saved visibility choice.

## Sample libraries in Options

`src/ui/libraries.rs` renders library management as the Sample libraries page of
Options. Folder scans and provider changes still use their existing Session
operations and storage. A successful scan adopts its folder changes; fallback
BPM is used by Add library folder and Scan again. The options draft and its
Cancel action cannot commit, stop, or roll back these operations. There is no separate library window.
Untouched startup Options may close after its initial scan fills the library;
manual navigation, a preference draft, or Restore defaults keeps it open.
New fixed song folders must be absolute existing directories when applied. A folder
that later becomes unavailable remains configured; song dialogs report it and
use the last-used/default location. Unrelated preferences and no-op Apply remain
available while that configured folder is unavailable.
