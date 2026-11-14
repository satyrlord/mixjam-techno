# Options rules: interaction, DSP, and persistence

## Interaction and application rules

1. Add a clearly named `Options` utility action and `Ctrl+,` shortcut. Place it
   at the foot of the tool rail. Use the gear icon as a prepared PNG; do not
   add SVG. A second activation focuses the existing window.
2. Use one draft per open window, with Apply, Cancel, and Restore defaults.
   Apply validates and saves, updates active preferences, and keeps the window
   open. Disable Apply when the draft has no pending changes. Cancel, Escape,
   or the close button closes a clean draft immediately. With pending changes,
   show Discard changes? with Apply changes, Discard changes, and Keep editing.
   Apply changes uses the same validation as Apply and closes only on success.
   Escape or close on the confirmation keeps editing. Restore defaults changes
   the draft only. Label it
   `Restore options defaults`; it does not reset songs, libraries, or caches.
3. Audio changes may be drafted during playback, preview, or recording, but
   cannot be applied until all three stop. Show `Stop playback, previews, and
   recording to apply audio options.` Do not stop them automatically. Pure
   Workspace or Files changes can apply while playback continues.
4. Successful application does not mark the song dirty, enter project undo,
   alter song metadata, or discard an unsaved project. No-op Apply does not
   rebuild audio or write settings.
5. A DSP quality change invalidates prepared playback mixes and any pending
   playback preparation. Build replacement state off the audio thread before
   the next Play. A completed job from an older preference generation cannot
   replace the new one. Export jobs retain their own full-quality configuration.
6. Detect a preference change made outside the open draft, such as the Sections
   toolbar toggle. Do not let an old draft silently overwrite it. Preserve
   unsaved draft edits and offer Reload before another Apply.
7. Closing or collapsing Options must not affect the transport or apply drafts.
   Keep page selection and scroll position while the window remains open.

Use a vertical page selector and a scrollable body with a fixed action row.
At 960x720, 1024x768, and 1440x1000, all options and actions must be reachable
without horizontal scrolling. Use the Utility palette and existing title,
close, collapse, focus, disabled-state, and error presentation. Keep labels
short, and put the quality explanation beside the controls rather than only
in tooltips.

## DSP implementation contract

Introduce an explicit runtime quality configuration outside `Project` and
`MasterChain`. Pass it through preparation and processor construction. Avoid
process-global mutable switches and reads of preference files on audio threads.
Export construction must select full quality explicitly rather than inheriting
the most recently used playback configuration.

With 4x selected, preserve the current algorithm, sample values, latency,
block independence, and seek/loop behavior. Off still runs the saturation
curve and the Drive's matching, mix, focus, output, and DC-removal stages.
Keep the Drive's antiderivative anti-aliasing method at the base rate; define
and test its reference path at that rate as well. Off is not a bypass, and its
sound need not match 4x. Record the resulting alias and frequency-response
tradeoffs instead of weakening the 4x acceptance thresholds.

Keep the existing integer alignment in both playback modes. The recommended
implementation keeps the 69-frame stage delay with a cheap delay when its
oversampling filters are off, and retains compensation at the chain boundary.
Account for any fractional phase difference in the base-rate curve/reference
pair; do not assert that matching integer delays proves phase equivalence.
Track audio, parallel dry paths, sidechain keys, sends, and master inputs must
remain aligned. Keep the fixed live master delay, its pedal-switch crossfades,
and limiter timing intact. Test impulses, zero-Mix identity at unity Output,
seeks, very short loops, and both output channels.

Do not change the mix sample rate, tempo interpretation, pitch, source identity,
export length, saved Drive settings, or song schema to implement these options.
Do not allocate, lock, read files, or design filter tables in the audio callback.
Changing quality while stopped avoids a new live quality-crossfade feature.

## Preference ownership and persistence

- `Session` owns validated active preferences and applies typed preference
  requests. The UI owns only its editable draft and presentation. Persistence
  stays in the backend, using the existing settings root and atomic file helper.
- Add a versioned `options.json` under `project_io::data_root()`; honor
  `MIXJAM_TECHNO_DATA_DIR` for isolated operation. Use a separate schema version
  from `.mixjam`. Define a typed enum for each choice and finite, bounded values
  for preview level. Reject invalid types and ranges before changing state.
- Missing settings use the table defaults. On first migration, import the
  existing `section-bookmarks.json` value. Once options exist, their section
  visibility is authoritative; route the existing toolbar through the same
  API. Keep the old file intact but stop writing it. Library and last-used song
  folder storage remain under their existing owners.
- Invalid settings load safe defaults with a visible, nonblocking error. Do
  not overwrite the original on startup. An explicit Restore options defaults
  followed by Apply may replace invalid settings. An unsupported future schema
  must remain intact; report the incompatibility and block persistent writes.
- Apply must validate the whole draft and prepare any fallible resources
  before one atomic replacement of `options.json`. Adopt active values only
  after persistence succeeds. A write or preparation error leaves active
  preferences, song, and existing file unchanged and keeps the draft available.
- Reset concerns only options in this document. Never delete audio, retained
  manual sources, sample caches, library registrations, or preservation data.

Stored fields: `schemaVersion`, `drivePlaybackOversampling`,
`masterPlaybackOversampling`, `samplePlaybackQuality` (`high` or `low`; a
schema 1 file without it loads at `high`), `previewLevelDb`, `previewMuted`,
`playbackBufferTarget`, `meterDisplayRate`, `followOnStartup`, `showSectionBookmarks`, `startingModule`,
`showTooltips`, `theme` (`studio`, `aero`, or `vintage`; a schema 1 file
without it loads `studio`), `laneHeight` (`20px`, `30px`, or `40px`; a schema 1
file without it loads `30px`), `songFolderMode`, and `fixedSongFolder`. Do not
store transient drafts or current playback state. Add migration tests before
changing this schema later.

Schema policy: a field added to schema 1 is optional with a defined default,
so every older valid schema 1 file still loads and keeps its explicit values.
Every present field stays strictly typed, unknown fields are still rejected,
and a file with a newer schema stays intact and blocks persistent writes. A
change that cannot follow this rule needs schema 2 and a migration. The
cost falls on downgrades: a build older than a field rejects a file that holds
it as an unknown field, so that build runs on defaults and treats the file as
invalid until Restore options defaults replaces it.
