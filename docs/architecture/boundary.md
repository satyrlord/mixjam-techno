# Application boundary

The Rust library is the backend. The native executable contains the frontend.
`src/application.rs` is the frontend's only application API. It has no egui,
window, file-dialog, or screenshot dependency.

```text
Native views and input                  Headless application
src/ui/view.rs + src/ui/view/            src/application.rs
             + src/ui/stretch.rs         + src/application/
             + src/ui/effects.rs
             + src/ui/eq.rs
             + src/ui/step_synth.rs
             + src/ui/dialog.rs
             + src/ui/controls.rs
             + src/ui/controls/
             + src/ui/sample_editor.rs
             + src/ui/mixer.rs
             + src/ui/mixer/
             + src/ui/mix_view.rs
             + src/ui/master.rs
             + src/ui/master/
             + src/ui/fx.rs
             + src/ui/agent.rs
             + src/ui/agent/
             + src/ui/album_review.rs
             + src/ui/sounds.rs
             + src/ui/waveform.rs
             + src/ui/inspector.rs
             + src/ui/overview.rs
             + src/ui/lanes.rs
             + src/ui/room.rs
             + src/ui/commands.rs
             + src/ui/app.rs
             + src/ui/tools.rs
             + src/ui/modals.rs
             + src/ui/paths.rs
             + src/ui.rs
        |                                       |
        | immutable project/catalog views       | Editor and history
        | typed edits and service calls         | Library and project files
        +-------------------------------------->| Render queue and transport
                                                | Recording and sample processing
```

## Session and edit history

`Session` owns the editor, catalog, project file, dirty state, render revisions,
audio player, microphone capture, and pending recorded take. Its project and
catalog accessors are read-only. It validates the project when it starts and
after each change. Typed `EditCommand` values change tempo, loop regions, clips,
routing, sections, and mixer settings. Separate operations place and move clips.
Successful edits update history and, when needed, mark the render for update. An accepted command that
changes nothing is not an edit: it leaves history, the saved state, and the render
unchanged. Changes to tempo or one gain, pan, send, channel EQ dial, FX knob, or clip control that arrive within 400 ms
of each other form one history entry, so a slider drag is one undo step. A
pointer press starts a new entry, so a reset such as Unity or Center is a step of
its own; while a pointer button is held, a pause does not end the entry. Saving
ends that group, so undo can return to the saved state after the next edit. A clip
cannot be lengthened over the next clip on its track. Automation point drags
use explicit start and end boundaries, so a pause within a drag does not split
its undo step and separate drags do not merge.
Clip handle drags keep a frontend preview and submit one `edit_discrete` action
on release. This ends continuous-control grouping before and after the edit,
so separate drags and agent `clip` requests each keep their own undo step.

Project copies share one imported-sample list until an edit changes it, so a
history entry or render request costs the same for any palette size.

## Sections

Sections are ordered named timeline markers. `SetSections` replaces the list
as one validated history step. Section edits and their undo/redo mark the song
modified but preserve the render revision, prepared mix, and pending playback.
The native UI owns section rename drafts. `Session` owns bookmark visibility
as an application option; the Sections toolbar and Options window use the same
validated preference API. Visibility does not modify the song. Section positions extend
the scrollable arrangement without extending the audio render.

## Options and workspace layout

`src/options.rs` owns the typed application preference schema, defaults,
validation, and persistence in `options.json` under `project_io::data_root()`.
`Session` owns the active values, their revision, and `apply_options`.
`src/ui/options.rs` owns one editable window draft and page selection. Apply
checks the revision, validates the complete draft, and prepares a replacement
output when needed before one atomic file replacement. Active values change
only after the write succeeds. Cancel and close ask before discarding pending
changes; a clean draft closes immediately. Restore options defaults changes
the draft only. No-op Apply performs no write or
audio rebuild. Options do not enter song history or modify song data.

Missing options import only the existing section-bookmark visibility. The old
file remains intact and receives no further writes. Invalid files load safe
defaults with a visible error and need explicit Restore options defaults before
replacement. A future schema remains intact and blocks preference writes.
Library configuration and the last-used song folder retain their own storage.
`libraries.json` holds the library configuration in the settings root.
`load_library_config` and `save_library_config` take that root, and `Session`
writes the file in its own settings root.
`WorkspaceLayout` in `src/options.rs` stores the Tools, Boards, Sounds, Overview,
and Inspector switches and the Maximise state in `layout.json` beside `options.json`. `Session` adopts each
layout change at once and saves it for the next launch. A failed write reports
an error but keeps the current layout. Headless sessions do not save layouts.
Layout changes do not change the Options revision, song data, or history.

## What the frontend owns

The frontend owns selection, drag gestures, zoom, layout, open windows, search
filters, and tool form drafts. It converts pointer positions into requested
positions and sends them to the application. It does not own an `Editor` or
change the live project. It does not decode audio, write projects, or control a
device directly.
Replacing a song or adopting scanned folders invalidates held control gestures
in Mixer, Mastering, Effects, and the channel EQ. Movement from an earlier
gesture cannot edit the replacement song.
