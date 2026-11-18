# Native frontend

## Themes

`src/ui/theme.rs` owns the three application themes. For each theme, it holds
the color roles, the material (flat, plastic, or metal), the corner radii, and
the fonts. It also holds the code painters for plastic surfaces, gradients,
flat and molded dials, and fader caps. Each frame sets a thread-local current
theme from the option. `theme::adopt` builds the egui style and fonts again
only when the theme changes. Renderers read roles, and only shared painters
branch on the material. Interaction and parameter code never depend on the
theme.

## Shared tool shell

`src/ui/dialog.rs` owns the shared tool shell and its lifecycle for all in-app
tools, utility windows, and blocking dialogs. It owns the header layout, the
position, the edge resize, the open and collapsed geometry, the focus layers,
the stable identities, and the accessible modal names. The caller owns the
task and decides what an action or a cancellation means. Native
operating-system windows and file pickers are outside this shell.

## Shared controls and the chassis

`src/ui/controls.rs`, with its `src/ui/controls/` modules, owns the
presentation and interaction of reusable controls: buttons, header icons,
rotaries, sliders, numeric values, switches, and the scrolling status text of
tool footers. Roles and states share the same rendering and the same keyboard
and accessibility behavior. The full travel of a rotary is 150 logical points.
Shift applies fine adjustment to new movement. Parameter specifications keep
the ranges, reset values, logarithmic mapping, and discrete choices. Tool
bodies own their content and drafts. They have no window skins or button
implementations of their own.

The shared chassis uses folded painted metal with a small quantity of age,
dark inset control bays, cream lettering, polished silver knobs, and pale metal
primary buttons. `controls::Palette` supplies the Utility, Marine, Granite,
Red, Olive, and Indigo variants:

- Time Stretcher and the FX returns board use Marine.
- Sample Editor uses Granite.
- Effect Studio uses Red.
- Step synth uses Olive.
- Mastering uses Indigo.
- Utilities use neutral metal with orange-yellow accents.

Prepared material and control PNGs are in `assets/folded-metal/<palette>/100/`
and in the shared `assets/folded-metal/controls/100/`.

The chassis has no decorative screws or fasteners. Its continuous antialiased
perimeter and bevel are native geometry. Raster material fades before the
outer outline and at strip ends, so no joints are visible. Knob edges use 8x
supersampling and one premultiplied-alpha Lanczos reduction. Fixed details fade
to transparent over a solid chassis color that matches them. A resize extends
the solid regions. Painting aligns one texel to one physical pixel. Text and
input stay live. Content reflows or scrolls, and the artwork and controls do
not scale. The palette variants share geometry and interaction.

## Collapse and blockers

Collapse keeps the position, the expanded size, and the task state. A
collapsed blocking dialog releases workspace and agent input, and does not do
its pending action. `src/ui/modals.rs` owns the blocker precedence and the
paused save continuation. `src/ui/paths.rs` prevents the replacement of pending
file requests. Choose path suspends its parent confirmation until the path
request resolves. A close action cancels a confirmation, and does not save or
discard implicitly.

Recorded take can collapse while its unsaved in-memory take stays available for
an explicit Save take or Discard take. A deferred file or close action examines
the recording again when it continues. If a recording started while its
confirmation was collapsed, the action stays pending. The recording stays
intact until the user stops it.

## File dialogs and artwork

Native file dialogs give paths to application operations. Screenshot encoding
is isolated presentation verification code.

The artwork rules and their check are in
[Visual assets](../DEVELOPMENT.md#visual-assets). Runtime PNG textures have one
prepared size and paint one texel for each physical screen pixel. Views snap
their bounds to the pixel grid and do not resize the textures.

## Waveform peaks and the Sounds panel

`src/peaks.rs` holds read-only waveform peaks for library sounds. The finest
table has the lowest and highest sample of each 64 frames, all channels
together. Each coarser table has four times as many frames in each bucket,
until only a few buckets remain. Views ask `Session::sample_peaks(id)`, which
gives `Pending`, `Ready`, or `Unavailable` and never decodes on the UI thread.

At each `Session::poll`, the pending sounds that views asked for since the last
poll replace the queue of one worker thread, in the order of the requests. The
worker thread starts on the first request, and reads the sounds through
`Library::read_pcm`. If a pending sound leaves the screen before the worker
gets to it, the worker forgets it. When the sound comes back, the view asks for
it again. A decoder panic marks that sound as unavailable. If the worker stops,
a new worker replaces it with its list.

When the catalog changes, for example when a library folder is added or
removed, a sound keeps its table while these items stay the same: its source,
its place in the file, and the size and modified time of the file. The UI
thread reads only that metadata. The worker reads all other sounds again.
`Session::poll` collects finished tables and keeps the view refreshing while
tables are pending.

`src/ui/sounds.rs` draws the Sounds panel from those tables, with the search,
filters, sort, and sound actions of the library. `src/ui/waveform.rs` turns a
ready table into one column pair for each point of an arrangement clip. It
follows the trim, Reverse, the fades, and the end of the sound, and paints the
visible columns as one mesh. `src/ui/view/score_lanes.rs` builds these shapes
for each clip, and `MixJamApp` keeps them. A shape is built again only when its
key changes: the sound, the trim, the length, Reverse, the fades, the points for
each tick (zoom), the frames for each tick (tempo), the catalog revision, or the
peak table. The catalog revision prevents a replacement sound from using a
stale waveform when its peak table uses the same address again.

A clip of a Step synth sound with a pattern beside its WAV draws notes
instead. `src/ui/view/clip_paint.rs` loads each such pattern one time for each
catalog revision. `waveform::ClipNotes` puts the notes of
`StepSynthState::notes` on the ticks of the clip. The note positions use the
render tempo of the pattern for source frames, and the effective playback tempo
of the catalog for song ticks.

## Command registry, lanes, Inspector, and overview

`src/ui/commands.rs` holds the command registry: the name, group, shortcut,
rail icon, and action of each command. The rail draws its groups from `RAIL`.
The switches of the status bar and the keyboard run their commands through
`run_command`. The command list shows `Command::all()`. A test fails when a
rail icon has no registered command.

`src/ui/lanes.rs` owns the layout constants of the Arrange lane header, the
lane name that comes from the sound family of the first clip, and the lane
fader.

`src/ui/room.rs` decides in each frame which of these show: the Sounds panel,
the Inspector, and the board of the lower panel. The saved layout tells which
are wanted. The measured window tells which fit together. If there is a tie,
the order in which they were opened decides. `src/ui/room.rs` never changes the
saved layout.

`src/ui/inspector.rs` draws the Inspector, and `src/ui/inspector/clip.rs` its
clip pages. Its page follows the clip and track selection. Its fields use the
Mixer controls and the clip painting under their own names, and edit through
`SetTrack`, `SetClip`, and `SetClipSounds`, which sets the gain, fades, pitch,
and reverse of several clips as one step. `SetTrack` replaces
one track with its processing. The Mixer and the Mix view send `SetTrackMix`,
which replaces only the mixer settings of one track. Thus a view that edits one
track copies only that track in each frame, not all tracks. The page and
control identities include the selection and the frontend generation. Thus
pending value entries and held controls cannot edit another selection or a
replacement project.

`src/ui/overview.rs` draws the song overview above the score. Its layout comes
before the score toolbar, so a click moves the scroll offset of this frame. It
paints after the lanes, from the offset that they used. Its clip lines are one
mesh. The mesh stays until one of these changes: the project change token, the
catalog revision, the theme, the strip, or the span. The overview reads the
song and never edits it. When neither the strip nor its folded bar fits beside
the necessary score area, the overview hides until the space is available
again. It keeps the saved visibility choice.

## Sample libraries

`src/ui/libraries.rs` renders the library management as the Sample libraries
tool window. The Files page of Options opens it. Folder scans and provider
changes use their own Session operations and storage, so the window has no
draft and no footer. A successful scan adopts its folder changes. Add library
folder and Scan again use the fallback BPM. The options draft and its Cancel
action cannot commit, stop, or roll back these operations.

With an empty library at startup, the window opens by itself. It closes after
the initial scan fills the library, unless the scan failed. Opening it by hand
cancels that close.

When a new fixed song folder is applied, it must be an absolute folder that
exists. If a configured folder later becomes unavailable, it stays configured.
The song dialogs report it and use the last-used or default location. While
that folder is unavailable, unrelated preferences and an Apply that changes
nothing stay available.
