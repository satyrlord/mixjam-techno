# Options pages

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
| Starting module | Sounds | Choices: Sounds, Mixer, Mastering, FX returns. Sounds opens the Sounds panel and folds the lower panel. Mixer opens the score's Mix view with Mastering in the lower panel. Mastering and FX returns open the lower panel on that tab. Applies on the next launch, not when a project opens. Capture and verification modes retain their explicit view overrides. |
| Theme | Studio | Choices: Studio, Aero, Vintage; the first control on the page. Apply repaints the whole application at once, with no restart and no live preview while the draft is open. Allowed during playback, previews, and recording: it is not an audio option. It never changes song data, undo history, renders, the audio device, or parameter ranges. Every theme keeps the same layout, controls, and features. |
| Lane height | 30 px | Choices: 20 px, 30 px, 40 px; the height of each arrangement row. Apply resizes the rows at once, with no restart and no live preview while the draft is open. Allowed during playback, previews, and recording: it is not an audio option. It never changes song data or undo history. |
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
