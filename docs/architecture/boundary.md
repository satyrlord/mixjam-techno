# Application boundary

The Rust library is the backend. The native executable contains the frontend.
`src/application.rs` is the only application API of the frontend. It has no
egui, window, file-dialog, or screenshot dependency.

```text
Native views and input                  Headless application
src/ui.rs                               src/application.rs
src/ui/ (every module)                  src/application/
src/ui_icons.rs
        |                                       |
        | immutable project and catalog views   | Editor and history
        | typed edits and service calls         | Library and project files
        +-------------------------------------->| Render queue and transport
                                                | Recording and sample processing
```

## Session and edit history

`Session` owns the editor, the catalog, the project file, the dirty state, the
render revisions, the audio player, the microphone capture, and the pending
recorded take. Its project and catalog accessors are read-only. It validates
the project when it starts and after each change.

Typed `EditCommand` values change the tempo, the loop regions, the clips, the
routing, the sections, and the mixer settings. Separate operations place and
move clips. A successful edit updates the history and, when necessary, marks
the render for update. An accepted command that changes nothing is not an
edit: the history, the saved state, and the render do not change. A clip
cannot become longer than the gap to the next clip on its track.

The history groups edits as follows:

- Changes to the tempo, or to one gain, pan, send, channel EQ dial, FX knob, or
  clip control, that arrive within 400 ms of each other make one history
  entry. Thus a slider drag is one undo step.
- A pointer press starts a new entry. Thus a reset, such as Unity or Center,
  is a step of its own. While a pointer button is held, a pause does not end
  the entry.
- Save ends the group. Thus undo can go back to the saved state after the
  next edit.
- An automation point drag has explicit start and end boundaries. A pause in
  a drag does not split its undo step, and separate drags do not merge.
- A clip handle drag keeps a frontend preview and sends one `edit_discrete`
  action on release. This action ends the continuous-control group before and
  after the edit. Thus each drag and each agent `clip` request keeps its own
  undo step.

Project copies share one imported-sample list until an edit changes it. Thus a
history entry or a render request has the same cost for each palette size.

## Sections

Sections are ordered, named timeline markers. `SetSections` replaces the list
as one validated history step. A section edit, and its undo or redo, marks the
song as modified. It keeps the render revision, the prepared mix, and the
pending playback. The native UI owns the section rename drafts.

`Session` owns the bookmark visibility as an application option. The Sections
toolbar and the Options window use the same validated preference API. The
visibility does not change the song. Section positions extend the scrollable
arrangement, but they do not extend the audio render.

## Options and workspace layout

`src/options.rs` owns the typed schema of the application preferences, their
defaults, their validation, and their persistence in `options.json` under
`project_io::data_root()`. `Session` owns the active values, their revision,
and `apply_options`. `src/ui/options.rs` owns one editable window draft and the
page selection.

Apply does these steps:

1. It checks the revision.
2. It validates the complete draft.
3. When necessary, it prepares a replacement output.
4. It replaces the file in one atomic step.

The active values change only after the write succeeds. Cancel and close ask
before they discard pending changes. A clean draft closes immediately. Restore
options defaults changes only the draft. An Apply that changes nothing does no
write and no audio rebuild. Options do not go into the song history and do not
change song data.

If `options.json` is missing, the load takes only the section-bookmark
visibility from `section-bookmarks.json`. The load does not change that file,
and the application does not write to it. An invalid options file loads safe
defaults with a visible error. Before the application can replace that file,
the user must select Restore options defaults. A file with a future schema stays
intact and blocks preference writes.

The library configuration and the last-used song folder have their own
storage. `libraries.json` in the settings root holds the library
configuration. `load_library_config` and `save_library_config` take that root,
and `Session` writes the file in its own settings root.

`WorkspaceLayout` in `src/options.rs` stores the Tools, Boards, Sounds,
Overview, and Inspector switches and the Maximise state. It is in
`layout.json` beside `options.json`. `Session` adopts each layout change
immediately and saves it for the next launch. If the write fails, `Session`
reports an error and keeps the current layout. Headless sessions do not save
layouts. Layout changes do not change the Options revision, the song data, or
the history.

## What the frontend owns

The frontend owns the selection, the drag gestures, the zoom, the layout, the
open windows, the search filters, and the tool form drafts. It converts
pointer positions into requested positions and sends them to the application.
It does not own an `Editor`, and it does not change the live project. It does
not decode audio, write projects, or control a device directly.

When a song is replaced, or when scanned folders are adopted, the held control
gestures in Mixer, Mastering, FX returns, and the channel EQ become invalid.
Movement from an earlier gesture cannot edit the replacement song.
