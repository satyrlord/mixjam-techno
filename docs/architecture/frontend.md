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
tools, utility windows, and blocking dialogs. It owns these items:

- The header layout, the position, and the edge resize.
- The open and collapsed geometry.
- The focus layers, the stable identities, and the accessible modal names.

The caller owns the task and decides what an action or a cancellation means.
Native operating-system windows and file pickers are outside this shell.

## Shared controls and the chassis

`src/ui/controls.rs`, with its `src/ui/controls/` modules, owns the
presentation and interaction of these reusable controls:

- Buttons and header icons.
- Rotaries, sliders, numeric values, and switches.
- The scrolling status text of tool footers.

Roles and states share the same rendering and the same keyboard and
accessibility behavior. The full travel of a rotary is 150 logical points.
Shift applies fine adjustment to new movement. Parameter specifications keep
the ranges, reset values, logarithmic mapping, and discrete choices. Tool
bodies own their content and drafts. They have no window skins or button
implementations of their own.

The shared chassis across Utility, Marine, Granite, Red, Olive, and Indigo
variants renders procedurally in code via `src/ui/controls/` and
`src/ui/theme.rs`, with no chassis bitmap assets. Content reflows or scrolls
when resized, keeping geometry and interaction identical across all three
themes.

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
the recording again when it continues. If a recording starts while the
confirmation is collapsed, the action stays pending. The recording stays intact
until the user stops it.

## File dialogs and artwork

Native file dialogs give paths to application operations. The code that
encodes screenshots is separate presentation verification code.

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
poll replace the queue of one worker thread. They keep the order of the
requests. The worker thread starts on the first request, and reads the sounds
through `Library::read_pcm`. If a pending sound leaves the screen before the
worker gets to it, the worker forgets it. If the sound returns, the view asks
for it again. A decoder panic marks that sound as unavailable. If the worker
stops, a new worker replaces it with its list.

The catalog can change, for example when the user adds or removes a library
folder. Then a sound keeps its table if these items stay the same:

- Its source.
- Its place in the file.
- The size and modified time of the file.

The UI thread reads only that metadata. The worker reads all other sounds
again. `Session::poll` collects finished tables. It keeps the view updating
while tables are pending.

`src/ui/sounds.rs` draws the Sounds panel from those tables, with the search,
filters, sort, and sound actions of the library. `src/ui/waveform.rs` turns a
ready table into one column pair for each point of an arrangement clip. It
follows the trim, Reverse, the fades, and the end of the sound, and paints the
visible columns as one mesh. `src/ui/view/score_lanes.rs` builds these shapes
for each clip, and `MixJamApp` keeps them. The app builds a shape again only if
its key changes. The key holds these items:

- The sound, the trim, the length, Reverse, and the fades.
- The points for each tick (zoom) and the frames for each tick (tempo).
- The catalog revision and the peak table.

The catalog revision prevents a replacement sound from using a stale waveform
when its peak table uses the same address again.

A clip of a Step synth sound with a pattern beside its WAV draws notes
instead. `src/ui/view/clip_paint.rs` loads each such pattern one time for each
catalog revision. `waveform::ClipNotes` puts the notes of
`StepSynthState::notes` on the ticks of the clip. For source frames, the note
positions use the render tempo of the pattern. For song ticks, they use the
effective playback tempo of the catalog.

## Command registry, lanes, Inspector, and overview

`src/ui/commands.rs` holds the command registry: the name, group, shortcut,
rail icon, and action of each command. The rail draws its groups from `RAIL`.
The switches of the status bar and the keyboard run their commands through
`run_command`. The command list shows `Command::all()`. A test fails when a
rail icon has no registered command.

`src/ui/lanes.rs` owns these items:

- The layout constants of the Arrange lane header.
- The lane name, which is the sound family of the first clip.
- The lane pan knob.

In each frame, `src/ui/room.rs` decides which of these show: the Sounds panel,
the Inspector, and the board of the lower panel. The saved layout holds the
panels that the user wants. The measured window shows which panels fit
together. If two panels do not fit, the panel that the user opened last shows.
`src/ui/room.rs` never changes the saved layout.

`src/ui/inspector.rs` draws the Inspector, and `src/ui/inspector/clip/` its clip
pages. Its page follows the clip and track selection; its track page keeps the
EQ response curve button and channel processing power switches. The clip card
fields and the fade handles are in `src/ui/controls/clip_fields.rs`, which the
clip context menu also uses. The fields edit through `SetTrack`, `SetClip`,
and `SetClipSounds`. `SetClipSounds` sets the gain, fades, pitch, and reverse
of several clips as one step. `SetTrack` replaces one track with its
processing. The Mixer and arrangement lane controls send `SetTrackMix`, which
replaces only the mixer settings of one track. Thus a view that edits one
track copies only that track in each frame, not all tracks.
The page and control identities include the selection and the frontend
generation. Thus pending value entries and held controls cannot edit another
selection or a replacement project.

`src/ui/overview.rs` draws the song overview above the score. Its layout comes
before the score toolbar, so a click moves the scroll offset of this frame. It
paints after the lanes, from the offset that they used. Its clip lines are one
mesh. The mesh stays until one of these items changes:

- The project change token and the catalog revision.
- The theme.
- The strip, the span, and the section bookmarks.

The overview reads the song and never edits it. If neither the strip nor its
folded bar fits beside the necessary score area, the overview hides until there
is space again. It keeps the saved visibility choice.

## Sample libraries

`src/ui/libraries.rs` renders the library management as the Sample libraries
tool window. The Files page of Options opens it. Folder scans and provider
changes use their own Session operations and storage. Thus the window has no
draft and no footer. A successful scan adopts its folder changes. Add library
folder and Scan again use the fallback BPM. The options draft and its Cancel
action cannot commit, stop, or undo these operations.

With an empty library at startup, the window opens by itself. It closes after
the initial scan fills the library, unless the scan failed. Opening it by hand
cancels that close.

A new fixed song folder must be an absolute folder that exists when the user
applies it. If a configured folder later becomes unavailable, it stays
configured.
The song dialogs report it and use the last-used or default location. While
that folder is unavailable, unrelated preferences and an Apply that changes
nothing stay available. The draft checks the typed folder a maximum of once
each second for the same values. Apply always checks it again.
