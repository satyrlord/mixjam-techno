# Step synth

Step synth is an Olive board in the lower panel. Its tool rail button chooses
the board. If the board already shows, the button folds the lower panel.
`src/ui/step_synth.rs` supplies the board and the top row of the panel. The
board has no status line of its own. Its messages and its idle hint show in
the status bar.

## Board layout

A settings column 232 points wide holds the sections Source, Pattern, Tempo,
and Voice. It is 672 points high. If the lower panel is shorter, the column
scrolls on its own. A focused dial or the preset list scrolls into view.

Source starts with the mode keys Sample and Synth (Source mode Sample and
Source mode Synth). In Sample mode, Source holds the source name, the keys
Palette sound and Source file…, the Start dial, Find start, and Loop source. In
Synth mode, Source holds:

- the waveform keys Saw, Sq, and Tri (Oscillator Saw, Oscillator Square, and
  Oscillator Triangle),
- Octave, with Synth octave down and Synth octave up, from -2 to +2,
- the Sub dial, 0 to 100 percent,
- the pitch readout.

Pattern holds:

- the Length row with ×2 and ÷2,
- Load and Save, with the Open and Save rail icons, and Clear,
- the primary Save sound.

Tempo and Voice put their dials in pairs. Each knob has its name and value well
at its right. The actions are high in the column, so they show without a scroll
at the usual panel height. Use one noun for each thing: the pattern is the
editable notes, and the sound is the rendered WAV.

The grid column fills the remaining width and keeps the height of the panel.
From the top, it holds the bar strip, the step ruler, the pitch grid, and the
Accent, Slide, and Velocity lanes. It shows four bars when each step gets at
least 28 points, or else two bars, or else one. The focused grid cell scrolls
into view.

The bar strip has four groups, with a hairline in each gap:

- Listen,
- the bar buttons,
- Scale, Scale root, and Randomise,
- at the right end, Copy to N, Fill, and Clear bar.

From 820 points the gaps are 16 points and the keys are full size. A narrower
strip keeps every control. It has 12-point gaps and a shorter Scale list, and
Randomise shows as its die icon alone.

Listen is the board's play key, first in the bar strip. It shows a play
triangle and the word Listen, and becomes a stop square with Stop while the
pattern plays. Its accessible name is Listen or Stop listening. If the strip
is narrower than 620 points, the key shows only its icon. The L key does the
same while the board shows. In the effects pane, Listen sits above the form. A
strip of one cell per step beside it lights the step that plays.

Source names truncate, with full tooltips and accessible values. The choice keys
Palette sound and Source file… pick the source. Their names are their visible
text. Source file… selects a WAV or PXD path. Palette sound returns to the
selected sound or clip. Find start sets Start to the first hit of the source. An
explicit file has priority over missing palette audio. The board reads a new
source to find its first hit and duration. A change to another source sets Start
to the first hit and turns Loop source off. This is one undo step, and the
status line says so. A loaded pattern keeps its own Start. The Start dial stops
1 ms before the end of the source.

## Bars and length

The bar buttons choose the viewed bar and bring its page of bars into view:

- The other bars in view keep an olive outline.
- A bar in the loop has an olive mark under its button.
- A coral dot marks the bar that Listen plays.

Copy to N copies the viewed bar onto bar N, the next bar. On the last bar, the
key reads Copy bar and is disabled. Fill copies the viewed bar onto the other
three bars. Clear bar clears only the viewed bar. It is disabled if the bar is
already empty.

Length, under Pattern, sets the number of steps that the pattern repeats over,
from 1 to 64. To change it, use one of these actions:

- Drag or scroll its well.
- Use ×2 and ÷2.
- Drag the coral end marker on the step ruler.

×2 repeats the loop into the new steps when they are empty. If ×2 or ÷2 cannot
change the length, the key is disabled and its tooltip says why. The steps
after the end keep their contents and show dimmed.

The board keeps its own undo. The undo and redo keys come after Notes and
Effects (Undo pattern edit and Redo pattern edit). If there is nothing to
restore, the key is disabled. Ctrl+Z takes back the last edit of the board. It
works if the pointer is over the board, or the focus is in the board or its
toolbar. An edit is a note, a setting, an effect, the scale, the source, or a
load. Ctrl+Y or Ctrl+Shift+Z puts it back. The status line names the edit, for
example "Undid Clear bar.". If there is nothing to take back, the board says so
and does not undo the song.

## Notes and lanes

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
Shift on Velocity gives each selected note the level that you draw. A coral
dot next to the name of a lane shows that the selected notes have different
values in that lane.

The ruler and each value lane are keyboard stops. They share one step cursor:

- Left and Right move the cursor. Home goes to the first step in view. End
  goes to the last step in view inside the length.
- On the ruler, Shift with Left or Right selects a range. Space or Enter
  selects the current step.
- On Accent and Slide, Space or Enter toggles the flag. Up sets it and Down
  clears it.
- On Velocity, Up or Down changes the level by one.

These lane edits also apply to the selected notes.

## Pitch grid and scale

The grid shows 25 rows: +12 through -12 semitones around the octave in view.
The row 0, in olive with a tick at the gutter, is the source pitch. The grid
opens on the octave nearest the middle of the notes. It goes there
again after a load or Randomise. The − and + keys beside the step ruler show
the octave between them. These keys and Page Up and Page Down move the view
one octave at a time, within three octaves up or down. A note outside the rows
marks the top or bottom edge of the grid in coral. Every note stays valid in
the -48 through +48 range.

Scale and Scale root shade the rows outside the scale. They cut a notch in the
corner of a note outside the scale. They never block a pitch. The pattern file
saves them. Their lists show all their rows and do not scroll.
Randomise writes a new line in the scale over the length of the pattern.

Tab reaches the named controls. It reaches the grid as one stop. In the
grid:

- Arrow keys move a cell cursor.
- Home goes to the first step in view. End goes to the last step in view
  inside the length.
- Ctrl with Left or Right turns the bar page. If no other page exists, it
  does nothing.
- Space or Enter puts or clears the note.

The cursor cell scrolls into view. Every cell stays a named pointer and
assistive target. Show the selected state through accessible names and
states.

## Tempo and voice

BPM is the compact tempo slider over 120, 125, 130, 135, 140, 160, and 180.
Voice starts with the Preset list (Step synth preset). Its eight presets set
the voice and the effects, and keep the notes, the length, Swing, and Gate.
These dials have value wells:

- Swing, 50 to 75 percent,
- Gate, 5 to 100 percent,
- Accent, 0 to 12 dB,
- Slide time, 1 to 1000 ms,
- Amp decay, Off or 15 to 2000 ms,
- Filter env, -100 to +100 percent,
- Env decay, 20 to 2000 ms.

Gate sets how much of its last step a note sounds. A note that slides into the
next note ignores the gate. Loop source, in Source, repeats the audio while a
note sounds in Sample mode. Keep the tooltips that explain retriggers,
duration, repetition of the whole file, and slides.

The choice keys Notes and Effects, at the left of the top row, switch between
the grid and the common effects form. Their accessible names are Notes pane and
Effects pane. The selected key has an accent tint and edge. It reports its
toggled state.

## Actions and feedback

In Sample mode, Listen and Save sound need a source. In Synth mode, they need
no WAV. Disabled actions say why. A pattern save keeps its validation.
Processing is synchronous. Save sound writes a WAV in Step synth at the BPM of
the draft. It keeps the source selection and saves the pattern next to the WAV
as `Name.pattern.json`. A pattern load adopts the saved source path and keeps
the viewed bar. If that source file is missing, the board uses the palette
sound and shows a warning. If a loaded tempo is not one of the choices, it
moves to the nearest one. The status line says so.

If a pattern peaks over full scale, for example from Accent and filter drive,
the whole pattern plays and saves turned down to full scale. Listen and Save
sound then say by how many dB. The render does not clip.

The status bar reports source guidance, preview, file choice, cancellation,
errors, and save confirmation. At rest, while the board shows, it gives the
board's hint. The hint says how to place notes or how to leave the Effects pane.
In Sample mode, if no source is available or its audio is missing, the hint
says how to choose a source. A change of the palette source removes the
message of the old source from the status bar, even if the board is hidden.
Unrelated application notices stay visible. An explicit file keeps its own
message. The full feedback stays in a tooltip and a polite accessible live
region. Errors include an Error label. Missing palette audio offers Import
details in the board's top row. Help explains the source, grid, bars, tempo,
effects, and file workflow. If the lower panel folds, or another board shows,
the draft and the source selection stay.
