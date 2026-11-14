# Tool windows and missing audio

## Sample-processing forms and the Sample Editor

Sample-processing forms hold drafts until the user requests playback or saving.
The application validates and processes those drafts. It also owns synth-pattern
file parsing, generated audio registration, import resolution, and settings files.
`src/ui/sample_editor.rs` owns the Sample Editor body and native action responses.
It uses the shared shell and controls for frame fields, pitch slider, buttons,
header, and feedback. Numeric formatting does not change the draft.

Frame drafts retain unsigned 64-bit values, with an inclusive start and exclusive end. Pitch keeps the
existing -24 to +24-semitone UI range. Numeric display rounding does not alter
the draft. Background selections do not reset trim while the editor is collapsed.
On expansion, a changed active source ID or catalog revision applies the existing
trim reset. Expanded source-change behavior remains unchanged, so a replaced
source cannot retain stale bounds.
`src/ui/tools.rs` adapts Listen edit, Save edited sound, and Import details. Existing
Session trim and resampling remain synchronous. Preview uses existing PCM
playback; save uses the existing WAV workflow, registers the result in Wave
at the source BPM, and selects it. Neither the panel nor the artwork performs
file or audio operations. Missing sources and invalid ranges disable processing
with explicit tooltip reasons; the footer shows range and duration or recovery
guidance.

## Time Stretcher

`src/ui/stretch.rs` owns Time Stretcher's content and action responses inside the
shared shell. Source, tempo, processing, preview, and save feedback use shared
controls. Its Help action stays available in both open and collapsed states.

`src/ui/tools.rs` adapts its source selections, Listen, Save, and Import details
requests to existing application operations; the panel does not render audio or
write files. Opening it from the tool rail initializes the original BPM draft
from the selected sample's source tempo.
The application owns Time Stretcher's background source read and processing job.
Each request captures its source catalog and tempo values, rejects duplicate
work, and retains one consumable result. Library or song replacement invalidates
that result. The frontend retains the requested action and rejects results after
the tool closes or its source, tempos, or catalog change. It opens the existing
save workflow or requests PCM preview only for a current result. Preview state
and a revision identify whether that tool's audition is still playing; a later
preview, transport change, or stop cannot inherit its active indicator.
Save completion updates the local feedback and the selected output's source
tempo; cancelling a file choice does not report success.

## Effect Studio

`src/ui/effects.rs` owns Effect Studio's sections and action responses. It uses
shared rotaries, value entry, switches, action buttons, header, and status styling.
Keep labels, values, and dial marks separate. Verify rendered bounds at 100%
display scaling; source coordinates alone do not establish visual acceptance.

`src/ui/tools.rs` adapts Listen, Save sound, Reset, Help, and Import details. Listen
and Save sound call `Session::processed_pcm` synchronously; Listen then uses the
existing PCM preview. Save uses the existing file request and
`Session::save_pcm`, registers the WAV in Effect at the source BPM, and selects
the saved sound. Source changes clear local feedback; save choice, cancellation,
failure, and success update it. Reset restores `EffectsState::default`.
Turning an optional section off discards its settings; enabling it uses that
section's defaults. This presentation does not change the algorithms or the
shared `effects_controls` form used by Step synth. It does not add a background
processing job.

## Missing audio and sample conflicts

Missing audio is explicit project state. The renderer leaves those placements
silent. The frontend reads the retained source metadata and sends a chosen WAV
path to `Session::resolve_missing_sample`. That operation resolves every matching
source identity in one history change, preserves musical trims, and retains the
chosen audio. The picker and context menu remain frontend responsibilities.

Duplicate file matches are stored in `Project.sample_conflicts`, with a stable
default, candidate paths, and source IDs. Readers select the first candidate.
`Session::resolve_sample_conflict` changes all affected placements atomically,
and updates history and rendering. The native modal exposes choices and playback;
confirmation and selected paths persist in the project. Transactions remove
conflicts that no longer affect any clip.
