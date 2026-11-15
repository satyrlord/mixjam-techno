# Options rules: interaction, DSP, and persistence

## Interaction and application rules

1. Name the utility action `Options`, with the shortcut `Ctrl+,`. Put it at
   the left end of the status bar. Use the gear icon as a prepared PNG. Do not
   add SVG. A second activation focuses the existing window.
2. Use one draft for each open window, with Apply, Cancel, and Restore
   defaults:
   - Apply validates and saves the draft, updates the active preferences, and
     keeps the window open. Disable Apply when the draft has no pending
     changes.
   - Cancel, Escape, or the close button closes a clean draft immediately.
   - With pending changes, show Discard changes? with Apply changes, Discard
     changes, and Keep editing. Apply changes uses the same validation as Apply
     and closes the window only on success. Escape or close on the
     confirmation keeps editing.
   - Restore defaults changes only the draft. Label it
     `Restore options defaults`. It does not reset songs, libraries, or
     caches.
3. Audio changes can be drafted during playback, preview, or recording. They
   cannot be applied until all three stop. Show `Stop playback, previews, and
   recording to apply audio options.` Do not stop them automatically. Changes
   only to Workspace or Files can apply during playback.
4. A successful Apply does not mark the song dirty, add a project undo step,
   change song metadata, or discard an unsaved project. A no-op Apply does not
   rebuild audio or write settings.
5. A DSP quality change invalidates the prepared playback mixes and any
   pending playback preparation. Build the replacement state off the audio
   thread before the next Play. A completed job from an older preference
   generation cannot replace the new one. Export jobs keep their own
   full-quality configuration.
6. Detect a preference change from outside the open draft: for example, the
   Sections toggle in the toolbar. Do not let an old draft overwrite it
   silently. Keep the unsaved draft edits and offer Reload before the next
   Apply.
7. When Options closes or collapses, the transport does not change and the
   draft does not apply. Keep the page selection and the scroll position while
   the window stays open.

Use a vertical page selector and a scrollable body with a fixed action row.
At 960x720, 1024x768, and 1440x1000, all options and actions must be
reachable without horizontal scrolling. Use the Utility palette and the
existing title, close, collapse, focus, disabled-state, and error
presentation. Keep labels short. Put the quality explanation beside the
controls, not only in tooltips.

## DSP implementation contract

Keep the runtime quality configuration outside `Project` and `MasterChain`.
Pass it through preparation and processor construction. Do not use
process-global mutable switches. Do not read preference files on audio
threads. Export construction must select full quality explicitly. It must not
use the playback configuration that was used last.

With 4x selected, keep the reference algorithm, sample values, latency, block
independence, and seek and loop behavior. Off still runs the saturation curve
and the matching, mix, focus, output, and DC-removal stages of the Drive.
Keep the antiderivative anti-aliasing of the Drive at the base rate, and
define and test its reference path at that rate. Off is not a bypass, and its
sound does not have to match 4x. Record the alias and frequency-response
tradeoffs. Do not make the 4x acceptance thresholds weaker.

Keep the integer alignment in both playback modes:

- Keep the 69-frame stage delay as a simple delay when the oversampling
  filters are off, and keep the compensation at the chain boundary.
- Account for a fractional phase difference in the base-rate curve and
  reference pair. Matching integer delays do not prove phase equivalence.
- Track audio, parallel dry paths, sidechain keys, sends, and master inputs
  must stay aligned.
- Keep the fixed live master delay, its pedal-switch crossfades, and the
  limiter timing.

Test impulses, zero-Mix identity at unity Output, seeks, very short loops, and
both output channels.

Do not change the mix sample rate, the tempo interpretation, the pitch, the
source identity, the export length, the saved Drive settings, or the song
schema for these options. Do not allocate, lock, read files, or design filter
tables in the audio callback. Quality changes apply only while playback is
stopped, so no live quality crossfade is necessary.

## Preference ownership and persistence

- `Session` owns the validated active preferences and applies typed
  preference requests. The UI owns only its editable draft and its
  presentation. Persistence stays in the backend, with the existing settings
  root and the atomic file helper.
- Store the options in a versioned `options.json` under
  `project_io::data_root()`. Use `MIXJAM_TECHNO_DATA_DIR` for isolated
  operation. Use a schema version separate from `.mixjam`. Use a typed enum for
  each choice, and finite, bounded values for the preview level. Reject types
  and ranges that are not valid before the state changes.
- Missing settings use the table defaults. If `options.json` does not exist,
  import the value of the older `section-bookmarks.json` file. When
  `options.json` exists, its section visibility is the authority, and the
  toolbar uses the same API. Keep the older file, but do not write it. The
  library folders and the last-used song folder stay with their existing
  owners.
- If the settings are not valid, load safe defaults with a visible error that
  does not block. Do not overwrite the original file at startup. An explicit
  Restore options defaults, followed by Apply, can replace the settings that
  are not valid. A file with an unsupported future schema must stay as it is.
  Report the incompatibility and block persistent writes.
- Apply must validate the full draft and prepare all fallible resources before
  one atomic replacement of `options.json`. Adopt the active values only after
  the write is successful. A write error or a preparation error does not
  change the active preferences, the song, or the existing file, and the draft
  stays available.
- A reset applies only to the options in this specification. Never delete
  audio, retained manual sources, sample caches, library registrations, or
  preservation data.

Stored fields:

- `schemaVersion`
- `drivePlaybackOversampling`
- `masterPlaybackOversampling`
- `samplePlaybackQuality`: `high` or `low`. A schema 1 file without it loads
  at `high`.
- `previewLevelDb`
- `previewMuted`
- `playbackBufferTarget`
- `meterDisplayRate`
- `followOnStartup`
- `showSectionBookmarks`
- `startingModule`
- `showTooltips`
- `theme`: `studio`, `aero`, or `vintage`. A schema 1 file without it loads
  `studio`.
- `laneHeight`: `20px`, `30px`, or `40px`. A schema 1 file without it loads
  `30px`.
- `songFolderMode`
- `fixedSongFolder`

Do not store transient drafts or the current playback state. Before a later
change to this schema, add migration tests.

Schema policy:

- A field that is added to schema 1 is optional, with a defined default. Thus
  each older valid schema 1 file still loads and keeps its explicit values.
- Each present field stays strictly typed, and unknown fields are rejected.
- A file with a newer schema stays as it is and blocks persistent writes.
- A change that cannot obey these rules needs schema 2 and a migration.

The cost of this policy is on downgrades. A build that is older than a field
rejects a file that holds that field as an unknown field. That build then runs
on defaults and treats the file as not valid, until Restore options defaults
replaces it.
