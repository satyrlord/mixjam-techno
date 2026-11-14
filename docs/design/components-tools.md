# Components: sound tools and Album Review

## Time Stretcher

`src/ui/stretch.rs` supplies the body inside the common shell. The source selector
supports search, bounded long names, full-name tooltips, and a virtualized list.
Each opening starts with an empty focused search. Original BPM supports 30-300;
Original uses shared rotary interaction. Target is the labelled tempo slider over
120, 125, 130, 135, 140, 160, and 180. Double-click Original to restore the
selected source tempo; double-click Target to reset to 140. Hover alone never
changes a value.

Listen and Save require available audio. Processing uses the application worker
and disables duplicate requests. Listen becomes Stop listening while this tool's
preview is active. Processing and preview indication must not be confused with a
successful-save indication. Cancelled or failed saves never report success.
A successful save retains the selected source and both tempo drafts, names the
saved file and category locally, and preserves the full path in application status.

Feedback shows progress, errors, save confirmation, or the source and tempo values
of the current audition. Draft changes remain distinct from the running preview.
Idle guidance explains duration change and approximate pitch preservation; equal
tempos state that duration is unchanged. Keep Listen before saving guidance until
the current settings have been heard. Errors begin with Cannot and carry an explicit
error indication. Full feedback remains available in a tooltip and polite accessible
live region. Missing audio offers Import details. Help explains ranges, input,
reset, processing, preview, and audible limitations.

Esc first closes a popup, leaves exact entry, or goes to a higher window. Otherwise
it closes the topmost tool. Collapse preserves the source, tempos, and feedback.
The body sends actions through `src/ui/tools.rs`; it does not process audio or write files.

## Effect Studio

`src/ui/effects.rs` supplies Filter, Echo, Robotize, and Distortion / Output inside
the common shell. Filter has Cutoff, Resonance, Overdrive, and Filter volume. Echo
has Delay (beats), Feedback, and Echo volume. Robotize has Color and Robotize
volume. Distortion / Output has Dirty distortion, Noise distortion, and Output
volume. Reuse the same control geometry and interaction as the other windows.
Keep dial marks, values, and labels in separate regions with measured clearance.

Optional sections pair a named switch with visible state. Disabled sections dim
their controls. Turning a section off discards its settings; turning it on uses its
defaults. Reset restores the existing effects draft. Cutoff and Color retain
logarithmic mapping; wheel, arrow, and accessible steps move them 1% of dial range.
Other parameter ranges, defaults, algorithms, and Step synth's effects form remain
unchanged. The source label truncates but retains its full tooltip and accessible
value.

Listen and Save sound require selected, available audio. Missing audio offers
Import details. Processing and preview use existing synchronous application calls;
the panel does not add Time Stretcher's worker or Stop listening behavior. Save
sound writes a WAV to Effect at the source BPM and selects it. Local feedback
covers preview, reset, file choice, cancellation, errors, and save confirmation.
Keep full feedback in a tooltip and polite accessible live region, with explicit
error text. Help and collapse retain the complete draft. No DSP equivalence follows
from this presentation change.

## Step synth

Step synth is an Olive board in the lower panel, chosen by its tab or its tool
rail button; the rail button folds the lower panel when the board already
shows. `src/ui/step_synth.rs` supplies the note grid, source and bar controls,
options, effects body, and actions on the board, and the status line in the tab
row. A short lower panel scrolls the board, and the focused grid cell scrolls
both the grid and the board into view. Source names truncate
with full tooltips and accessible values. Choose source file selects a WAV or PXD
path; Use palette sound returns to the selected sound or clip. An explicit file
takes precedence over missing palette audio and is read only when Listen sequence
or Save sequence requests processing.

View bar selects one of four editable bars. Active 1-4 chooses rendered bars in
ascending order. Each bar has sixteen sixteenth steps with one optional note per
step. Another pitch replaces the note; selecting the current note clears it.
Clear bar affects only the viewed bar. The grid shows +12 through -12 semitones,
with A 0 as source pitch. Loaded notes outside that display remain valid under
the existing -48 through +48 range. Tab reaches named controls and the grid as
one stop. Arrow keys move a cell cursor, Home and End jump to the first and last
step, and Space or Enter sets or clears the note; the cursor cell scrolls into
view. Every cell stays a named pointer and assistive target. Expose selected
state through accessible names and states.

BPM is the compact tempo slider over 120, 125, 130, 135, 140, 160, and 180. 1/16
cut ends a trigger after one sixteenth;
otherwise it continues until the next trigger or sequence end. Loop source repeats
audio within that interval. Keep tooltips that explain retriggering, duration,
combined behavior, whole-file repetition, and effects after source gating.
Sequence effects replaces the grid with the common effects form; Note grid
restores it.
Keep actions reachable while the body scrolls. The pane switch retains keyboard
focus when its label changes.

Listen and Save sequence need a source and active bar. Disabled actions explain
why. Pattern saving retains validation, including one active bar. Processing is
synchronous. Save sequence writes a WAV in Step synth at draft BPM and selects it,
and keeps its pattern beside it as `Name.pattern.json`.
Pattern JSON does not replace the source file or viewed bar. Local feedback covers
source guidance, preview, file choice, cancellation, errors, and save confirmation.
Changing palette source resets feedback; an explicit file keeps its own feedback.
Missing palette audio offers Import details. Full feedback remains in a tooltip
and polite accessible live region; errors include an Error label. Help explains
the source, grid, bars, tempo, effects, and file workflow. Folding the lower
panel or choosing another board preserves the draft and source selection.

## Sample Editor

`src/ui/sample_editor.rs` supplies trim, pitch, source, actions, and feedback inside
the common shell. Reuse the shared value fields, slider, buttons, and header.
The source name truncates with a full tooltip and accessible value. Start frame
is inclusive and zero-based; End frame is exclusive. Setting End to the source
frame count includes its last frame. Preserve unsigned 64-bit exact entry.
Adjacent buttons change one frame, or 100 with Shift. Pitch stays within -24 to
+24 semitones; its buttons change 0.1 semitone, or one with Shift. The slider and
field share the same draft. Explain exact entry, boundaries, and how pitch changes
speed and duration. Background selections do not reset the trim draft while the
editor is collapsed. On expansion, a different active source or catalog revision
resets the range under the existing policy. Expanded source changes still reset
the range to actual source frames.

Listen edit and Save edited sound use the shared secondary and primary buttons.
An absent or missing source, invalid frame range, or invalid pitch disables them
with an explicit reason. Feedback shows selected frames and duration after pitch,
or the reason editing is unavailable. Keep full status in its tooltip and polite
accessible live region. Missing audio offers Import details. Esc first leaves
exact entry, a popup, or a higher window; otherwise it closes the topmost editor.

Session trim, resampling, preview, WAV saving, registration in Wave, and source-BPM
behavior remain application-owned and synchronous. Saving leaves the source
unchanged. Collapse preserves edits and performs no processing.

## Album Review

Album Review is a Utility window. The grid view leads with the search field,
the sort combo, and the "visible / total albums" count. Artist and genre
filters are Secondary buttons that turn Primary while active; a genre that
would leave no album is disabled and says why. The genre row shows the six
most-used tags and "+N more". Active filters repeat as chips with × before
Clear filters. Cards are 150 by 214 points: the cover or a painted record
placeholder, the artist in muted capitals, the title in cream, and the year
and track count. The album page puts the cover column (220 points) beside the
title, the metadata grid, and the links, then the overview, a timeline chart
with one row per track whose sections share a color by name, and each track
with its energy written out beside a colored label, so color is never the
only cue. Empty states are explicit: No reviews folder, No albums available,
and No results with Clear filters. Covers are review content and are exempt
from the raster asset rules; the rail icon is not.
