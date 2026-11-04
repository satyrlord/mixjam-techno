# Options page specification

Status: implemented, 2026-11-02. The owner requested
implementation of this specification on that date, including its recommended
baseline. The implementation adds the options below without changing the song
format. Verification results and manual-check limits must accompany delivery;
this status does not establish release readiness or a supported hardware limit.
See [OPTIONS-VERIFICATION.md](OPTIONS-VERIFICATION.md) for results and manual limits.

## Purpose and boundaries

Give the user one place to select playback quality and app preferences.
Keep the current sound and behavior as the default. Offer a lower CPU mode
when a project cannot play reliably on the user's computer.

Use a native Options window containing Audio, Workspace, Files, and Sample libraries pages.
Use the shared Utility shell and controls in the current theme. This is not a web
page, a replacement for the Mixer, or a new song editor.

In scope: the controls below, preference persistence, safe application of
changes, existing-control synchronization, agent access, and verification.
Out of scope: device selection, ASIO or exclusive audio, sample-rate changes,
plugins, autosave, theme editing or custom themes, arbitrary UI
scaling, new song DSP parameters, and a change to song format 24. Do not expose
an internal processing block size as a device latency control. The current
song format is 24; options use their own schema.

## Current evidence

Revalidate these facts before implementation; they describe the source at the
time of this proposal.

| Area | Current behavior and owner |
|---|---|
| Track Drive | Four-times oversampling in `src/drive.rs`; the shared oversampler has a 69-frame delay. `TrackChain` in `src/audio.rs` compensates that delay and bounds reads at playback loop ends. |
| Master saturation | Soft clip, Tube, Maximizer, and Tape use `Oversampler` in `src/master/pedals.rs`. Other master processors have their own algorithms and timing. |
| Master timing | `MasterProcessor` in `src/master.rs` keeps a fixed live delay across pedal changes. Export compensates active delays. |
| Playback work | `PlaybackSource` renders 256-frame blocks. This is not the audio device's callback size. |
| Output buffering | `src/audio/output.rs` requests a 40 ms CPAL output buffer, bounded by reported device limits, with system-default fallback. The request and callback interval are separate values. Revalidate the device probe before choosing the page's defaults. |
| Preferences | `src/project_io.rs` stores sample folders, the last song folder, and section bookmark visibility. `Session` exposes their application operations. Follow currently starts enabled and lasts for the current session. |
| Native controls | `src/ui/dialog.rs` owns window shells; `src/ui/controls.rs` owns shared controls. The frontend's application API is `Session`. |

The review measured a 22-Drive workload on Windows 11 with an Intel i7-11700.
For 48 kHz output with 10 ms callbacks, removing duplicate work reduced mean
callback time from 13.52 to 8.49 ms; the optimized case still exceeded the
deadline in 45.3% of 1,000 measured callbacks. These are workload-specific
measurements, not a supported track-count limit or a minimum hardware claim.
The measurement started at a song position, so it does not establish the cost
of uninterrupted playback from the song's start. Keep private fixture paths
out of public documentation.

Oversampling costs CPU and also introduces a signal delay. The existing delay
is compensated. Turning oversampling off is intended to reduce CPU work;
the page must not promise lower device latency.

A silent probe on the default Steinberg UR-RT2 at 48 kHz observed 1,056
initial writable frames with system-default buffering and 1,920 with the
40 ms request. Both runs averaged about 10.1 ms between callbacks. This
confirms a change in the initial writable buffer on that device, not a
40 ms callback interval or glitch-free heavy-song playback. The backend
request is already implemented; this proposal adds user control over it.

## Options

### Audio

| Control | Choices and default | Required behavior |
|---|---|---|
| Drive playback oversampling | `4x` (default), `Off` | Applies to all track Drives during song playback, including separate sidechain-key chains. Does not switch the Drives off or change their saved knobs. |
| Master playback oversampling | `4x` (default), `Off` | Applies only to Soft clip, Tube, Maximizer, and Tape during playback. Does not disable limiter lookahead, true-peak detection, metering filters, or other processors. |
| Sample playback quality | `High` (default), `Low` | High uses band-limited sample reads. Low uses linear interpolation during playback. Exports always use High. This preference does not change source identity, song timing, or saved song data. |
| Playback buffer target | `Automatic`, `20 ms`, `40 ms` (default where supported) | Request a supported output-buffer configuration. Larger buffers can absorb processing bursts and increase control-to-sound delay. Show the actual negotiated buffer or measured callback period separately; never relabel a 10 ms callback as 20 or 40 ms. |
| Preview level | -30 to 0 dB; default 0 dB, plus `Mute previews` off by default | Attenuates sound-browser, clip, tool, and recorded-take previews at the preview output. Does not change source PCM, saved tool output, song gains, recording input, or exports. |
| Meter display rate | `Normal` (default), `Reduced` | Reduced draws meter values at most 15 times per second. It does not reduce audio analysis, change meter integration windows, or miss peaks and latched OVER events. Do not promise a DSP saving from a display-only option. |

Show one short help line under each quality control, and one under the
section title: "Lower settings use less CPU during playback. Exports always
use full quality." (2026-11-03, after the Options critique). Show the buffer
request and the measured callback inside a closed **Device details**
disclosure under the buffer target.

The two switches must remain separate. A user may need cheaper track Drives
while keeping the master at full quality. Do not offer a single global
"Disable all oversampling" switch that also changes limiter or meter safety.

The owner also requested a longer processing deadline during this review.
Treat the buffer target as a request to the audio backend, not a relaxed
benchmark threshold. The implementing agent must inspect the current device
API and supported buffer range. At 48 kHz, 20 ms corresponds to 960 frames;
at 44.1 kHz it corresponds to 882 frames. A 40 ms target is 1,920 or 1,764
frames respectively. These are target sizes, not evidence
that Windows or the selected device will deliver those periods. If the
backend cannot negotiate the requested size, retain the existing working
configuration and show the reason. Do not silently claim success or offer
unsupported values as working choices. Keep Automatic available.

Report the requested size and actual size or measured callback interval. If
the backend cannot expose negotiated capacity, label that value unavailable;
do not present the accepted request as a measured capacity.
Recreate the output stream while stopped when needed, and retain the previous
configuration if opening the replacement fails. Re-measure the same workload
with the actual callback cadence and buffering. A larger requested capacity
does not by itself establish a longer callback interval. Neither changing the callback benchmark's
constant nor enlarging `PlaybackSource`'s internal render block changes the
device deadline by itself. The final default must follow the owner's accepted
buffer change and verified backend behavior; revalidate it before implementation.

**Export rule:** normal mix exports, stems, and song-tool renders always use
4x for Drive and the four master saturation processors. They ignore playback
quality preferences. Preview attenuation and display preferences also have no
effect on export. Headless rendering must remain independent of the user's
settings folder. A future explicit draft-export mode is outside this version.
Show `Playback quality reduced; exports use full quality` in Options and as an
accessible workspace status when either playback switch is Off or Sample
playback quality is Low. Exports always read samples at High.

### Workspace

| Control | Default | Required behavior |
|---|---|---|
| Follow playhead on startup | On | Sets the initial Follow state on the next app launch. The existing toolbar toggle still controls the current session. Do not silently turn Follow back on after a user turns it off. |
| Show section bookmarks | Existing saved value, otherwise On | Shares one current value with the Sections toolbar toggle. Both update the same preference; neither edits the song or its section data. |
| Starting module | Sounds | Choices: Sounds, Mixer, Mastering, Effects. Applies on the next launch, not when a project opens. Capture and verification modes retain their explicit view overrides. |
| Theme | Studio | Choices: Studio, Aero, Vintage; the first control on the page. Apply repaints the whole application at once, with no restart and no live preview while the draft is open. Allowed during playback, previews, and recording: it is not an audio option. It never changes song data, undo history, renders, the audio device, or parameter ranges. Every theme keeps the same layout, controls, and features. |
| Show tooltips | On | Controls hover hints. Never removes accessible names, descriptions, focus indication, errors, or essential visible instructions. |

### Files

| Control | Default | Required behavior |
|---|---|---|
| Song dialog folder | Last used | Choices: Last used, Fixed folder. Last used keeps current behavior. Fixed folder uses a user-selected absolute folder as the initial location for song Open and Save as. |
| Fixed song folder | None | Visible only with Fixed folder selected. Validate an existing directory when applying. If it later becomes unavailable, keep the configured path, show a useful message, and fall back to the current last-used/default-folder behavior. Never relocate existing songs. |

Choosing a path does not commit a preference. Cancelling the picker leaves the
draft unchanged. Library management uses the Sample libraries page in this same window. Its
folder and provider actions run separately through existing Session services
and storage. A successful scan adopts its folder changes; fallback BPM is used
by Add library folder and Scan again. Options Apply and Cancel do not commit or
roll back library changes.

### Sample libraries

Move the complete former Sample libraries window into this Options page. Keep
folder registration and removal, configured order, scan progress and cancellation,
fallback BPM, individual folder-tempo reports, skipped WAV and unreadable-folder
messages, and optional product-provider management. Use `Cancel library scan` to
distinguish scan cancellation from Options Cancel. Retire the standalone window
and its rail action. The Options rail entry is a gear with prepared PNG variants.
Show `Changes here take effect right away.` Use a single Done action on this
page. Done follows the same draft-close rules as Cancel on the other pages.
Keep the options preference draft intact while navigating to this page.

## Interaction and application rules

1. Add a clearly named `Options` utility action and `Ctrl+,` shortcut. Reuse
   the rail's utility group. Use the gear icon with the required PNG density
   variants; do not add SVG. A second activation focuses the existing window.
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
without it loads `studio`), `songFolderMode`, and `fixedSongFolder`. Do not
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

## Agent and accessibility contract

Expose the window and each control through the existing named UI channel.
Use stable names matching the option labels, with selected values, units,
disabled reasons, and keyboard operation. Add active preference values and
an audio-apply-blocked reason to agent `state`, separate from song state.
Agent Apply must use the same validation, stop requirements, history exclusion,
and error path as a click. No direct JSON-file manipulation is a UI substitute.

Test keyboard-only page navigation, value editing, Apply, Cancel, Restore
defaults, and focus restoration. Inspect the accessibility tree and test with
a screen reader when available; a screenshot alone does not prove accessibility.

## Delivery slices and exit checks

| Slice | Dependency | Deliverable and observable exit check |
|---|---|---|
| 1. Preference model | None | Typed defaults, validation, atomic persistence, existing section-visibility migration, and Session API. Tests prove corrupt/future files are preserved and failed Apply changes nothing. No new UI or DSP behavior yet. |
| 2. Native shell and app options | 1 | Options navigation, draft lifecycle, Workspace/Files controls, preview level, and meter display cadence. Tests prove project/history independence, toolbar synchronization, stale-draft rejection, and preview/export separation. Capture all three window sizes. |
| 3. Drive playback quality | 1-2 | 4x/Off for audible and key chains. 4x matches the current reference; Off keeps placement/timing/zero-Mix invariants. Full-quality exports are identical regardless of playback preference. Measure callback costs before and after. |
| 4. Output buffering | 1-2 | Extend the existing 40 ms backend request with supported targets, stopped stream replacement, truthful request/cadence reporting, and failure rollback. Record device acceptance and actual callback behavior before making buffering or performance claims. |
| 5. Master playback quality | 3 | Separate quality for the four named saturation pedals. Verify parallel dry paths, live delay, pedal edits, key timing, and limiter/true-peak behavior. The preference does not disable unrelated processing. |
| 6. Integration and handoff | 2-5 | Agent state/control coverage, documentation, release-artifact verification, performance evidence, and stated manual-check limits. No placeholders, disabled future controls, or temporary migration writes remain. |

Each slice must remain independently reviewable. Do not expose an enabled
quality control before its processor and export-isolation tests pass. The
implementing agent should update the current product, architecture, design,
user-guide, agent-control, and development contracts as behavior lands.

## Verification and acceptance

Run the current commands in [DEVELOPMENT.md](DEVELOPMENT.md), including both
Cargo feature sets, warnings-denied Clippy, both format checks, Python tests,
release build, applicable asset checks, and native captures. Follow
[ARCHITECTURE.md](ARCHITECTURE.md), [DESIGN.md](DESIGN.md), and
[AGENT-CONTROL.md](AGENT-CONTROL.md). Preserve the public raster-asset rule.

For each quality combination, test both preparation and active processing:
off Drives, zero Mix, maximal Drive, the four master saturation pedals alone
and together, sidechain keys, short loops, discontinuous seeks, source gaps,
stem summation, and a changed preference while an old job completes. Check
that a sample-rate conversion by the device cannot change the chosen internal
quality or musical timing. Run the saved full-quality artifact fixtures under
different preference files and compare output samples and duration.

Measure an optimized release with the callback harness in
`src/audio/realtime_bench.rs`. Include a representative arrangement, sparse
and dense Drive use, a master-heavy case, and 64-track stress. Measure starts,
seeks, replacements, and uninterrupted playback after all relevant lanes have
sounded. Report CPU/OS, device or callback model, output rate, workload,
duration, mean, p99, maximum, and missed deadlines. Repeat on a quiet machine
to distinguish sustained overload from isolated scheduling spikes. Require
lower measured CPU cost for Off in a DSP-bound case; do not claim it makes
every project fit a 10 ms deadline. Choose the minimum supported test machine
with the owner before making release-readiness claims about hardware capacity.

Listen for the Off-mode tradeoff on bass, bright tones, drums, and dense mixes.
Check clicks, delay alignment, seeks, loop boundaries, and physical playback
at 44.1 and 48 kHz. Record microphone, native-picker, screen-reader, high-DPI,
and clean-machine checks separately as passed, failed, or unavailable.

## Decisions for the owner or assigned agent

Recommended baseline: separate Drive/master playback switches; 4x defaults;
exports always 4x; audio changes apply only while stopped; a supported output
buffer target with truthful actual-period reporting; no device picker in this
version; the complete option list above delivered in slices.

Before expanding this baseline, resolve whether draft exports or live quality
switching are actually needed, and identify the minimum supported performance
machine. Those are additional product decisions. The assigned agent must not
infer approval for them from the existence of this proposal.
