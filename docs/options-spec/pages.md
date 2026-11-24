# Options pages

## Options

### Audio

The page has two sections: Playback quality and Monitoring. Monitoring holds
Preview level, `Mute previews`, and Meter display rate.

| Control | Choices and default | Required behavior |
| --- | --- | --- |
| Drive playback oversampling | `2x` (default), `Off` | Applies to all track Drives during song playback, also to separate sidechain-key chains. Does not switch the Drives off or change their saved knobs. |
| Master playback oversampling | `4x` (default), `Off` | Applies only to Soft clip, Tube, Maximizer, and Tape during playback. Does not disable limiter lookahead, true-peak detection, metering filters, or other processors. |
| Sample playback quality | `High` (default), `Low` | High uses band-limited sample reads. Low uses linear interpolation during playback. Exports always use High. This preference does not change source identity, song timing, or saved song data. |
| Playback buffer target | `Automatic`, `20 ms`, `40 ms` (default where supported) | Requests a supported output-buffer configuration. Larger buffers can absorb processing bursts and increase the control-to-sound delay. Shows the actual negotiated buffer or the measured callback period separately. Never shows a 10 ms callback as 20 or 40 ms. |
| Preview level | -30 to 0 dB, default 0 dB. `Mute previews` is off by default | Makes sound-browser, clip, tool, and recorded-take previews quieter at the preview output. Does not change source PCM, saved tool output, song gains, recording input, or exports. |
| Meter display rate | `Normal` (default), `Reduced` | Reduced draws meter values a maximum of 15 times each second. It does not reduce audio analysis, change meter integration windows, or miss peaks and latched OVER events. Do not promise a DSP saving from a display-only option. |

Show one short help line under the section title: "Lower settings use less
CPU during playback. Exports always use full quality." Show one short help line
under each quality control. Show the buffer request and the measured callback
in a closed **Device details** disclosure under the buffer target. That
disclosure also holds `Save audio capture`. It saves the last 30 seconds that
playback sent to the audio device. It also saves the state of the song. Both
go to the diagnostics folder in the data folder. F12 does the same anywhere
in the application.

Keep the two oversampling switches separate. A user can need cheaper track
Drives and a master at full quality. Do not offer one global "Disable all
oversampling" switch that also changes the limiter or meter safety.

### Buffer target rules

The buffer target is a request to the audio backend. It is not a relaxed
benchmark threshold. These are the target sizes:

| Target | 44.1 kHz | 48 kHz |
| --- | --- | --- |
| 20 ms | 882 frames | 960 frames |
| 40 ms | 1,764 frames | 1,920 frames |

These sizes are not evidence that Windows or the device delivers these
periods. Offer only the targets that the device supports, and keep Automatic
available. If the backend cannot negotiate the requested size, keep the
existing working configuration and show the reason. Do not claim success
silently, and do not offer unsupported values as working choices.

Report the requested size and the actual size or the measured callback
interval. If the backend cannot expose the negotiated capacity, show that value
as not available. Do not show the accepted request as a measured capacity.

If a new request needs a new output stream, make it while playback is stopped.
If the replacement does not open, keep the previous configuration. A larger
requested capacity alone does not give a longer callback interval. A change to
the constant of the callback benchmark does not change the device deadline.
A larger internal render block of `PlaybackSource` also does not change it.

### Export rule

Normal mix exports, stems, and song-tool renders use `8x` FIR ADAA2 for track
Drive and read samples at High. When a render includes the master pedalboard, its
four saturation pedals use 8x.
They ignore the playback quality preferences. Preview attenuation and display
preferences also have no effect on export. Headless rendering must stay
independent of the settings folder of the user. An explicit draft-export mode
is outside this specification.

When one of the playback switches is Off, or Sample playback quality is Low,
show `Playback quality reduced; exports use full quality` in Options. Show it
also as an accessible workspace status.

### Workspace

| Control | Default | Required behavior |
| --- | --- | --- |
| Theme | Studio | Choices: Studio, Aero, Vintage. The first control on the page. Apply repaints the full application immediately, with no restart and no live preview while the draft is open. Allowed during playback, previews, and recording: it is not an audio option. It never changes song data, undo history, renders, the audio device, or parameter ranges. All themes keep the same layout, controls, and features. |
| Lane height | 30 px | Choices: 20 px, 30 px, 40 px: the height of each arrangement row. Apply resizes the rows immediately, with no restart and no live preview while the draft is open. Allowed during playback, previews, and recording: it is not an audio option. It never changes song data or undo history. |
| Follow playhead on startup | On | Sets the initial Follow state at the next start of the application. The Follow toggle in the toolbar still controls the current session. Do not turn Follow on again silently after a user turns it off. |
| Show section bookmarks | The existing saved value, otherwise On | Shares one current value with the Sections toggle in the toolbar. Both update the same preference. Neither edits the song or its section data. |
| Starting module | Sounds | Choices: Sounds, Mixer, Mastering, FX returns. Sounds opens the Sounds panel and folds the lower panel. Mixer opens the Mix view of the score, with Mastering in the lower panel. Mastering and FX returns open the lower panel on that board. Applies at the next start, not when a project opens. Capture and verification modes keep their explicit view overrides. |
| Show tooltips | On | Controls hover hints. Never removes accessible names, descriptions, focus indication, errors, or necessary visible instructions. |

### Files

| Control | Default | Required behavior |
| --- | --- | --- |
| Song dialog folder | Last used | Choices: Last used, Fixed folder. Last used starts song Open and Save as in the last-used folder. Fixed folder uses a folder that the user selects, as an absolute path, as the initial location of song Open and Save as. |
| Fixed song folder | None | Visible only when Fixed folder is selected. A text field and the `Choose fixed song folder` button set it. Apply validates that the directory exists. If it is not available later, keep the configured path and show a message. Use the last-used or default folder. Never move existing songs. |

A path selection does not commit a preference. If the user cancels the picker,
the draft does not change.

### Sample libraries

Files ends with a Sample libraries section. It has one help line and
`Open sample libraries`. That button opens the Sample libraries tool window
beside Options. The draft stays as it is. That window holds the complete
library feature:

- Folder registration and removal, in the configured order.
- Scan progress and cancellation.
- The fallback BPM (**Tempo for files without one**).
- The folder-tempo report of each folder.
- The messages for skipped WAV files and folders that the application cannot
  read.
- Optional product-provider management.

Sample libraries has no rail action. The Options entry is a gear, a prepared
PNG, at the left end of the status bar. Use `Cancel library scan` to make scan
cancellation different from Options Cancel.

The folder and provider actions run through the existing Session services and
storage, not through the options draft. A successful scan adopts its folder
changes. Add library folder and Scan again use the fallback BPM. Options Apply
and Cancel do not commit library changes, and they do not undo them.

The window shows `Changes take effect right away.` It has no Apply, Cancel,
Restore, or Done. Its close button closes it at once, and a scan that runs
continues. With an empty library at startup, the window opens by itself and
closes when the first scan finds sounds.
