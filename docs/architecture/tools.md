# Tool windows and missing audio

## Sample-processing forms and the Sample Editor

Sample-processing forms hold drafts until the user requests playback or a
save. The application validates and processes those drafts. It also owns the
parse of synth-pattern files, the registration of generated audio, the import
resolution, and the settings files.

`src/ui/sample_editor.rs` owns the body of the Sample Editor and its native
action responses. It uses the shared shell and controls for the frame fields,
the pitch slider, the buttons, the header, and the feedback. Numeric format
does not change the draft. Frame drafts keep unsigned 64-bit values, with an
inclusive start and an exclusive end. The pitch range of the UI is -24 to +24
semitones. Display rounding does not change the draft.

While the editor is collapsed, selections in the background do not reset the
trim. When the editor expands, a changed active source ID or catalog revision
resets the trim. While the editor is expanded, a source change also resets the
trim. Thus a replaced source cannot keep stale bounds.

`src/ui/tools.rs` adapts Listen edit, Save edited sound, and Import details.
The Session trim and resampling are synchronous. Preview uses PCM playback.
Save uses the WAV workflow, registers the result in Wave at the source BPM, and
selects it. The panel and the artwork do no file or audio operations. Missing
sources and invalid ranges disable the processing, with tooltips that give the
reasons. The footer shows the range and the duration, or the recovery
guidance.

## Time Stretcher

`src/ui/stretch.rs` owns the content and action responses of Time Stretcher
inside the shared shell. The source, tempo, processing, preview, and save
feedback use shared controls. Its Help action is available in the open and the
collapsed states.

`src/ui/tools.rs` adapts its source selections and its Listen, Save, and Import
details requests to application operations. The panel does not render audio or
write files. When the tool rail opens Time Stretcher, the original BPM draft
starts from the source tempo of the selected sample.

The application owns the background source read and the processing job of
Time Stretcher:

- Each request records its source catalog and tempo values.
- The application rejects duplicate work.
- It keeps one result, which the frontend can take one time.
- A library or song replacement makes that result invalid.

The frontend keeps the requested action. It rejects a result that comes after
the tool closes, or after its source, tempos, or catalog change. It opens the
save workflow or requests a PCM preview only for a current result. A preview
state and a revision tell whether the audition of that tool still plays. Thus a
later preview, a transport change, or a stop cannot take its active indicator.
When a save completes, it updates the local feedback and the source tempo of
the selected output. A cancelled file choice does not report success.

## Effect Studio

`src/ui/effects.rs` owns the sections and action responses of Effect Studio.
It uses shared rotaries, value entry, switches, action buttons, header, and
status style. Keep labels, values, and dial marks separate. Verify rendered
bounds at 100% display scaling. Source coordinates alone do not give visual
acceptance.

`src/ui/tools.rs` adapts Listen, Save sound, Reset, Help, and Import details.
Listen and Save sound call `Session::processed_pcm` synchronously. Listen then
uses the PCM preview. Save uses the file request and `Session::save_pcm`,
registers the WAV in Effect at the source BPM, and selects the saved sound. A
source change clears the local feedback. The save choice, the cancellation, a
failure, and a success update it. Reset restores `EffectsState::default`.

Effect Studio and Step synth use the same six bays in `src/ui/effects_rack.rs`.
Bypass preserves the settings of each optional bay. Effect Studio also offers
eight presets and three tail modes. Trim keeps the source length. Wrap warms
the effects with the repeating source. Ring extends echo and reverb for up to
six seconds. Step synth always renders with Wrap.

`src/effects.rs` and submodules under `src/effects/` own the six DSP bays
(`Filter`, `Modulation`, `Echo`, `Robotize`, `Reverb`, `Distortion / output`).
The engine uses oversampled filter, drive, and ring processing, and accepts
per-frame filter modulation (`FilterMod`) for voiced Step synth patterns.

## Missing audio and sample conflicts

Missing audio is explicit project state. The renderer keeps those placements
silent. The frontend reads the source metadata that the project keeps, and
sends a chosen WAV path to `Session::resolve_missing_sample`. That operation
resolves each matching source identity in one history change, keeps the musical
trims, and keeps the chosen audio. The picker and the context menu are frontend
responsibilities.

Duplicate file matches are in `Project.sample_conflicts`, with a stable
default, the candidate paths, and the source IDs. Readers select the first
candidate. `Session::resolve_sample_conflict` changes all affected placements
in one atomic step, and updates the history and the rendering. The native
modal shows the choices and plays them. The project saves the confirmation and
the selected paths. Transactions remove conflicts that no longer have an
effect on a clip.
