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
   - Restore defaults puts the defaults of every page in the draft, not only
     the page on show. Label it `Restore all defaults`. It changes only the
     draft, and does not reset songs, libraries, or caches.
3. The user can edit audio changes in the draft during playback, preview, or
   recording. Apply cannot save them until all three stop. Show `Stop
   playback, previews, and recording to apply audio options.` Do not stop them
   automatically. Changes only to Workspace or Files can apply during
   playback.
4. A successful Apply does not mark the song dirty or add a project undo step.
   It does not change song metadata or discard an unsaved project. A no-op
   Apply does not rebuild audio or write settings.
5. A DSP quality change invalidates the prepared playback mixes and any
   pending playback preparation. Build the replacement state off the audio
   thread before the next Play. A completed job from an older preference
   generation cannot replace the new one. Export jobs keep their own
   full-quality configuration.
6. Detect a preference change from outside the open draft: for example, the
   Sections toggle in the toolbar. Do not let an old draft overwrite it
   silently. Keep the unsaved draft edits, and show `Reload active options`.
   Apply stays disabled until the user reloads.
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
use the playback configuration of the last playback.

With 4x selected, keep the reference algorithm, sample values, latency, block
independence, and seek and loop behavior. Off still runs the saturation curve
and the matching, mix, focus, output, and DC-removal stages of the Drive.
Keep the antiderivative anti-aliasing of the Drive at the base rate. Define
and test its reference path at that rate. Off is not a bypass, and its
sound does not have to match 4x. Record the alias and frequency-response
tradeoffs. Do not make the 4x acceptance thresholds weaker.

Keep the integer alignment in both playback modes:

- If the oversampling filters are off, keep the 69-frame stage delay as a
  simple delay. Keep the compensation at the chain boundary.
- Account for a fractional phase difference in the base-rate curve and
  reference pair. Matching integer delays do not prove phase equivalence.
- Track audio, parallel dry paths, sidechain keys, sends, and master inputs
  must stay aligned.
- Keep the fixed live master delay, its pedal-switch crossfades, and the
  limiter timing.

Test impulses, zero-Mix identity at unity Output, seeks, very short loops, and
both output channels.

These options must not change any of these items:

- The mix sample rate.
- The tempo interpretation and the pitch.
- The source identity and the export length.
- The saved Drive settings and the song schema.

Do not allocate, lock, read files, or design filter tables in the audio
callback. Quality changes apply only if playback is stopped, so no live
quality crossfade is necessary.

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
  Restore all defaults, followed by Apply, can replace the settings that
  are not valid. A file with an unsupported future schema must stay as it is.
  Report the incompatibility and block persistent writes. The Sections toggle in
  the toolbar still works for the session if the file is not valid. It also
  works if the file is from a newer schema or the application cannot write the
  file. Then the application loses only the saved choice. It does not write a
  file that is not valid or newer, and a failed write shows an error.
- Apply must validate the full draft and prepare all fallible resources before
  one atomic replacement of `options.json`. Adopt the active values only after
  the write is successful. A write error or a preparation error does not
  change the active preferences, the song, or the existing file, and the draft
  stays available.
- The panel layout is not an option. It has its own `layout.json` file beside
  `options.json`, and an Options draft does not include it.
- A reset applies only to the options in this specification. Never delete
  audio, retained manual sources, sample caches, library registrations, or
  preservation data.

Stored fields:

- `schemaVersion`
- `drivePlaybackOversampling`: `4x` or `off`. A draft file with `2x` loads at
  `4x` and saves as `4x`.
- `masterPlaybackOversampling`: `4x` or `off`.
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

- A field that you add to schema 1 is optional, with a defined default. Thus
  each older valid schema 1 file still loads and keeps its explicit values.
- Each present field stays strictly typed, and the reader rejects unknown
  fields. The reader also rejects a file larger than 64 KiB.
- A file with a newer schema stays as it is and blocks persistent writes.
- A change that cannot obey these rules needs schema 2 and a migration.

The cost of this policy is on downgrades. A build that is older than a field
rejects a file that holds that field as an unknown field. That build then runs
on defaults and treats the file as not valid, until Restore all defaults
replaces it.
