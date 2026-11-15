# Components: sound tools and Album Review

## Time Stretcher

`src/ui/stretch.rs` supplies the body inside the common shell. The source
selector has search, bounded long names, full-name tooltips, and a
virtualized list. Each opening starts with an empty search that has focus.
Original BPM accepts 30-300, and Original uses the shared rotary interaction.
Target is the labelled tempo slider over 120, 125, 130, 135, 140, 160, and
180. A double-click on Original restores the tempo of the selected source. A
double-click on Target resets it to 140. Hover alone never changes a value.

Listen and Save need available audio. Processing uses the application worker
and disables duplicate requests. Listen becomes Stop listening while the
preview of this tool plays. Processing and preview indication must not look
like a save indication. A save that is canceled or fails never reports
success. After a save, the tool keeps the selected source and both tempo
drafts. It names the saved file and category in the window, and the
application status keeps the full path.

Feedback shows progress, errors, save confirmation, or the source and tempo
values of the current audition. Draft changes stay separate from the running
preview. Idle guidance tells that the duration changes and that the pitch
stays approximately the same. Equal tempos state that the duration does not
change. Keep the "Listen before saving" guidance until the user hears the
current settings. Errors start with Cannot and have an explicit error
indication. The full feedback stays available in a tooltip and a polite
accessible live region. Missing audio offers Import details. Help explains the
ranges, input, reset, processing, preview, and audible limits.

Esc first closes a popup, leaves exact entry, or goes to a higher window. If
none of these applies, Esc closes the topmost tool. Collapse keeps the source,
tempos, and feedback. The body sends actions through `src/ui/tools.rs`. It
does not process audio or write files.

## Effect Studio

`src/ui/effects.rs` supplies four sections inside the common shell:

| Section | Controls |
| --- | --- |
| Filter | Cutoff, Resonance, Overdrive, Filter volume |
| Echo | Delay (beats), Feedback, Echo volume |
| Robotize | Color, Robotize volume |
| Distortion / output | Dirty distortion, Noise distortion, Output volume |

Use the same control geometry and interaction as the other windows. Keep dial
marks, values, and labels in separate regions with measured clearance.

An optional section pairs a named switch with a visible state. A disabled
section dims its controls. When a section goes off, its settings are lost.
When it comes on, it uses its defaults. Reset restores the current effects
draft. Cutoff and Color keep their logarithmic mapping. Wheel, arrow, and
accessible steps move them by 1% of the dial range. The window does not change
the parameter ranges, defaults, or algorithms. The source label truncates, but
keeps its full tooltip and accessible value.

Listen and Save sound need selected, available audio. Missing audio offers
Import details. Processing and preview use synchronous application calls. The
window has no worker and no Stop listening, as Time Stretcher has. Save sound
writes a WAV to Effect at the source BPM and selects it. Local feedback tells
about preview, reset, file choice, cancellation, errors, and save
confirmation. Keep the full feedback in a tooltip and a polite accessible live
region, with explicit error text. Help and collapse keep the full draft. The
presentation makes no claim of DSP equivalence.

## Step synth

Step synth is an Olive board in the lower panel. Its tool rail button chooses
it, and folds the lower panel when the board already shows.
`src/ui/step_synth.rs` supplies the board and the status line in the top row
of the panel.

### Board layout

A settings column 232 points wide holds the sections Source, Pattern, Tempo,
Voice, and Scale. The column scrolls on its own when the lower panel is short.
Pattern holds Load and Save, with the Open and Save rail icons, and the
primary Save sound. The actions are high in the column, so they show without a
scroll at the usual panel height. Use one noun for each thing: the pattern is
the editable notes, and the sound is the rendered WAV.

The grid column fills the remaining width and keeps the height of the panel.
From the top, it holds the bar strip, the step ruler, the pitch grid, and the
Accent, Slide, and Velocity lanes. It shows four bars when each step gets at
least 28 points, or else two bars, or else one. The focused grid cell scrolls
into view.

Listen is the board's play key, first in the bar strip. It shows a play
triangle and the word Listen, and becomes a stop square with Stop while the
pattern plays. Its accessible name is Listen or Stop listening. When the strip
is narrower than 620 points, the key shows only its icon. The L key does the
same while the board shows. In the effects pane, Listen sits above the form.

Source names truncate, with full tooltips and accessible values. The choice
keys Palette and File… (Use palette sound and Choose source file) pick the
source. File… selects a WAV or PXD path. Palette goes back to the selected
sound or clip. An explicit file has priority over missing palette audio. The
file is read only when Listen or Save sound starts processing.

### Bars and length

The bar buttons choose the viewed bar and bring its page of bars into view:

- The other bars in view keep an olive outline.
- A bar in the loop has an olive mark under its button.
- A coral dot marks the bar that Listen plays.

Copy to N copies the viewed bar onto bar N, the next bar; on the last bar it
reads Copy bar and is disabled. Clear bar clears only the viewed bar, and is
disabled when the bar is already empty. Length sets the number of steps that
the pattern repeats over, from 1 to 64. To change it, drag or scroll its well,
use ×2 and ÷2, or drag the coral end marker on the step ruler. ×2 and ÷2 are
disabled when they would not change the length. The steps after the end keep
their contents and show dimmed.

The board keeps its own undo. With the pointer over the board, or the focus in
it or its toolbar, Ctrl+Z takes back the last note edit, copy, clear, or
Randomise. Ctrl+Y or Ctrl+Shift+Z puts it back. When there is nothing to take back, the
board says so and does not undo the song.

### Notes and lanes

Each bar has sixteen sixteenth steps. A step is a rest, a note, or a tie that
holds the note before it. A note has an accent, a slide, and a velocity. A
note draws as one coral bar across its ties. An accented note has a bright top
edge. A note that slides has a line into the next note.

- Click a cell to put a note there. The new note replaces the note of the
  step.
- Click a note to clear it with its ties.
- Drag the last step of a note sideways to add ties or to make it shorter. The
  ties replace the notes that they cover.
- Shift-click a note to accent it. Alt-click it to make it slide.

The Accent and Slide lanes toggle on click and paint on drag. The Velocity
lane draws levels. Drag along the step ruler to select steps. Lane edits then
apply to each selected note. A level edit moves each note by the same amount.
A coral dot next to the name of a lane shows that the selected notes are
different in that lane.

The ruler and each value lane are keyboard stops. They share one step cursor:

- Left and Right move the cursor. Home and End go to the first and last step
  in view.
- On the ruler, Shift with Left or Right selects a range. Space or Enter
  selects the current step.
- On Accent and Slide, Space or Enter toggles the flag. Up sets it and Down
  clears it.
- On Velocity, Up or Down changes the level by one.

These lane edits also apply to the selected notes.

### Pitch grid and scale

The grid shows 25 rows: +12 through -12 semitones around the octave in view.
The row A 0 is the source pitch. The − and + keys beside the step ruler, with
the octave between them, and Page Up and Page Down, move the view by a maximum
of three octaves. A note outside the rows
marks the top or bottom edge of the grid in coral. Every note stays valid in
the -48 through +48 range.

Scale and Scale root shade the rows outside the scale, and cut a notch in the
corner of a note outside the scale. They never block a pitch, and the pattern
does not save them. Randomise, under Scale, writes a new line in the scale
over the length of the pattern.

Tab reaches the named controls, and reaches the grid as one stop. In the
grid:

- Arrow keys move a cell cursor.
- Home and End go to the first and last step in view.
- Ctrl with Left or Right turns the bar page.
- Space or Enter puts or clears the note.

The cursor cell scrolls into view. Every cell stays a named pointer and
assistive target. Show the selected state through accessible names and
states.

### Tempo and voice

BPM is the compact tempo slider over 120, 125, 130, 135, 140, 160, and 180.
Swing (50 to 75 percent), Gate (5 to 100 percent), Accent (0 to 12 dB), and
Slide time (1 to 1000 ms) are dials with value wells. Gate sets how much of
its last step a note sounds. A note that slides into the next note ignores the
gate. Loop source repeats the audio while a note sounds. Keep the tooltips
that explain retriggers, duration, repetition of the whole file, and slides.

The choice keys Notes and Effects (Note grid and Pattern effects), at the
left of the top row, switch between the grid and the common effects form. The
selected key has an accent tint and edge, and reports its toggled state.

### Actions and feedback

Listen and Save sound need a source. Disabled actions tell why. Pattern
saving keeps its validation. Processing is synchronous. Save sound
writes a WAV in Step synth at the BPM of the draft, and selects it. It keeps
the pattern next to the WAV as `Name.pattern.json`. Pattern JSON does not
replace the source file or the viewed bar.

Local feedback tells about source guidance, preview, file choice,
cancellation, errors, and save confirmation. A change of the palette source
resets the feedback. An explicit file keeps its own feedback. Missing palette
audio offers Import details. The full feedback stays in a tooltip and a polite
accessible live region, and errors include an Error label. Help explains the
source, grid, bars, tempo, effects, and file workflow. If the lower panel
folds, or another board shows, the draft and the source selection stay.

## Sample Editor

`src/ui/sample_editor.rs` supplies trim, pitch, source, actions, and feedback
inside the common shell. Use the shared value fields, slider, buttons, and
header. The source name truncates, with a full tooltip and accessible value.

Start frame is inclusive and zero-based. End frame is exclusive. An End equal
to the frame count of the source includes its last frame. Keep unsigned
64-bit exact entry. The adjacent buttons change one frame, or 100 with Shift.
Pitch stays from -24 to +24 semitones. Its buttons change 0.1 semitone, or one
semitone with Shift. The slider and the field share the same draft. Explain
exact entry, the limits, and how pitch changes speed and duration.

Selections in the background do not reset the trim draft while the editor is
collapsed. On expansion, a different active source or catalog revision resets
the range, as the reset policy sets. While the editor is expanded, a source
change resets the range to the actual frames of the source.

Listen edit and Save edited sound use the shared secondary and primary
buttons. An absent or missing source, a frame range that is not valid, or a
pitch that is not valid disables them, with an explicit reason. Feedback shows
the selected frames and the duration after pitch, or the reason that editing
is not available. Keep the full status in its tooltip and a polite accessible
live region. Missing audio offers Import details. Esc first leaves exact
entry, a popup, or a higher window. If none of these applies, Esc closes the
topmost editor.

The application owns the session trim, resampling, preview, WAV save,
registration in Wave, and source-BPM behavior, and runs them synchronously. A
save does not change the source. Collapse keeps the edits and does no
processing.

## Album Review

Album Review is a Utility window. The grid view starts with the search field,
the sort combo, and the "visible / total albums" count. Artist and genre
filters are Secondary buttons that become Primary while they are active. A
genre that would show no album is disabled and tells why. The genre row shows
the six most-used tags and "+N more". Active filters show again as chips with
× before Clear filters.

Cards are 150 by 214 points. A card shows the cover or a painted record
placeholder, the artist in muted capitals, the title in cream, and the year
and track count.

The album page puts the cover column (220 points) next to the title, the
metadata grid, and the links. Below them are:

- the overview,
- a timeline chart with one row per track, in which sections with the same
  name share a color,
- each track, with its energy written out next to a colored label, so that
  color is never the only cue.

The empty states are explicit: No reviews folder, No albums available, and No
results with Clear filters. Covers are review content, and the raster asset
rules do not apply to them. The rules apply to the rail icon.
