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

Step synth is an Olive board in the lower panel, chosen by its tool rail
button; the button folds the lower panel when the board already shows.
`src/ui/step_synth.rs` supplies the board and the status line in the panel's
top row. A settings column 232 points wide holds Source, Pattern (the actions),
Tempo, Voice, and Scale, and scrolls on its own when the lower panel is short.
The grid column fills the rest of the width and keeps the panel's height: the bar
strip, the step ruler, the pitch grid, and the Accent, Slide, and Velocity lanes.
It shows four bars when each step gets at least 28 points, otherwise two, then
one. The focused grid cell scrolls into view. Source names truncate
with full tooltips and accessible values. Choose source file selects a WAV or PXD
path; Use palette sound returns to the selected sound or clip. An explicit file
takes precedence over missing palette audio and is read only when Listen sequence
or Save sequence requests processing.

The bar buttons pick the viewed bar and bring its page of bars into view; the
other bars in view keep an olive outline, a bar in the loop has an olive mark
under its button, and a coral dot marks the bar Listen sequence is playing.
Copy bar copies the viewed bar onto the next one, and Clear bar clears only the
viewed bar. Length sets the steps the pattern repeats over, from 1 to 64: drag
or scroll its well, use x2 and /2, or drag the coral end marker on the step
ruler. The steps after it keep their contents and show dimmed.

Each bar has sixteen sixteenth steps. A step is a rest, a note, or a tie that
holds the note before it, and a note carries an accent, a slide, and a velocity.
A note draws as one coral bar across its ties, with a bright top edge when
accented and a line into the next note when it slides. Click a cell to place a
note, replacing the step's note; click a note to clear it with its ties. Drag a
note's last step sideways to tie it on or shorten it; the tie replaces the notes
it covers. Shift-click a note to accent it and Alt-click it to slide. The Accent
and Slide lanes toggle on click and paint on drag; the Velocity lane draws
levels. Drag along the step ruler to select steps: lane edits then apply to
every selected note, a level edit shifting each by the same amount, and a coral
dot by a lane's name shows that the selected notes differ there.
The ruler and each value lane are keyboard stops. Left and Right move their
shared step cursor; Home and End go to the first and last step in view.
On the ruler, Shift with Left or Right selects a range, and Space or Enter
selects the current step. On Accent and Slide, Space or Enter toggles the flag,
and Up or Down sets or clears it. On Velocity, Up or Down changes the level by
one. These lane edits also apply to the selected notes.

The grid shows 25 rows, +12 through -12 semitones around the octave in view,
with A 0 as source pitch; the octave buttons and Page Up and Page Down shift it
by up to three octaves. A note outside the rows marks the grid's top or bottom
edge in coral, and every note stays valid under the -48 through +48 range. Scale
and its root shade the rows outside the scale and notch the corner of a note
outside it; they never block a pitch, and they are not saved with the pattern.
Randomise, under Scale, writes a new line in the scale over the pattern's
length. Tab reaches named controls and the grid as one stop. Arrow keys move a
cell cursor, Home and End jump to the first and last step in view, Ctrl with Left or Right turns the
bar page, and Space or Enter places or clears the note; the cursor cell scrolls
into view. Every cell stays a named pointer and assistive target. Expose
selected state through accessible names and states.

BPM is the compact tempo slider over 120, 125, 130, 135, 140, 160, and 180.
Swing (50 to 75 percent), Gate (5 to 100 percent), Accent (0 to 12 dB), and
Slide time (1 to 1000 ms) are dials with value wells. Gate sets how much of its
last step a note sounds for; a note that slides into the next ignores it. Loop
source repeats audio while a note sounds. Keep tooltips that explain
retriggering, duration, whole-file repetition, and slides.
Sequence effects replaces the grid with the common effects form; Note grid
restores it.
The actions sit high in the settings column, so they show without scrolling at
the usual panel height. The pane switch retains keyboard focus when its label
changes.

Listen and Save sequence need a source. Disabled actions explain why. Pattern
saving retains validation. Processing is
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
