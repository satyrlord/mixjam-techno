# Step synth board

`src/ui/step_synth.rs` and the files in `src/ui/step_synth/` own the Step synth
board, an Olive board in the lower panel. The board holds the source, the bars,
the grid, the effects, and the actions. The settings are in a fixed column at
the left. The grid column fills the remainder of the panel. It shows one, two,
or four bars of sixteen steps. It shows as many bars as keep each step at least
28 points wide. The grid has twenty-five pitch rows around the octave in view.
The octave moves from -3 to +3. Below the grid are the Accent, Slide, and
Velocity lanes.

The board is an accessible tab panel named Step synth, and agents scope to it.
Its pane switch, undo keys, and Import details are in a named Step synth
toolbar in the top row of the panel. The agent scopes this toolbar with the
board. The board is 512 points high, and the panel gives it its whole height
when the window has the space. Text, grid cells, switches, and input stay
live. The pitch grid, the settings column, and the replacement effects form
have bounded scrolling. The source, the step labels, the actions, and the
feedback stay available. The board uses the same six effect bays and controls
as Effect Studio. Their settings are part of the pattern.

`StepSynthBoard` in `src/ui/view/boards.rs`, which `MixJamApp` holds, owns
these items:

- The draft, the source path, and the viewed bar.
- The effects-view selection.
- The grid view (`step_synth::Editor`: octave, scale, root, and selected
  steps).
- The local feedback.

The pattern does not save the grid view. `src/ui/view/boards.rs` adapts the
actions of the panel to current `Session` operations.

The board only draws and reads input. The pattern edits and the board's undo
history do not use the GUI. They are in `src/synth/edit.rs` and
`src/synth/history.rs`, and the board calls them through
`application::pattern`. Thus a change of look stays in the board files, and
`src/synth/edit_tests.rs` tests the edits without a window. The board files
are these:

- `step_synth.rs`: the source, actions, and the board's frame.
- `editor.rs`: the grid view, and its link to the undo history.
- `layout.rs`: the board's measures, and the steps that are in view.
- `keys.rs` and `parts.rs`: the keys, dials, wells, and lettering.
- `settings.rs`, `bar_strip.rs`, `grid.rs`, `notes.rs`, `lanes.rs`, and
  `toolbar.rs`: the areas of the board.

The UI tests are in `src/ui/tests/step_synth_tests.rs`, which holds their
shared fixtures, and in `src/ui/tests/step_synth_tests/`: `editing.rs`,
`layout.rs`, `readiness.rs`, and `undo.rs`.

An explicit WAV or PXD path has priority over the palette or clip selection,
missing palette audio included. The board reads a new source to find its first
hit and duration. The board reads an explicit path that is not a WAV as a
stand-alone packed sample file. It decodes this file as the library decodes its
stand-alone packed files. The predictor starts from the header word. The audio
ends at the last complete opcode, also if the header declares a longer
duration.

When the user selects Listen or Save sound, `Session::step_synth_pcm` reads the
source and renders it synchronously. The
frontend then requests the PCM preview or the WAV save workflow. Save registers
the result in Step synth at the draft BPM and keeps the source selection.
`Session::save_sequence` stages the WAV and the draft at
`synth::pattern_beside` as one batch, with rollback if a replacement fails.
Registration comes after a successful publication. The save choice, the
cancellation, a failure, and a success update the main status line.
`MixJamApp::step_synth_say` also stores the message and its palette source in
`StepSynthFeedback`. A change of palette source clears that message, also if
the board is hidden. An unrelated application notice stays visible, and an
explicit file keeps its own feedback. For missing palette audio, the board
offers Import details.

`synth::render_measured` gives the render and the number of dB that it turned
the render down. `effects::process_synth` runs the pattern effects without a
clip. If the peak is over full scale, it scales the whole render to full scale.
Robotize fits whole carrier cycles into the pattern loop when a cycle fits
below 45% of the sample rate. This keeps the carrier in phase at the loop seam.
Listen and Save sound name the dB in their message.

`MixJamApp::step_synth_adopt_source` watches the source key: the file path,
or the palette sound. The board keeps the first source after the start or a
load as it is. A later change sets Start to the first hit of the new source and
turns Loop source off, as one undo step. It also gives the source length, which
limits the Start dial.

Pattern save and load use `Session::save_pattern` and
`Session::load_pattern_snapped`, which also gives the BPM of the file. A load
moves that BPM to the nearest supported tempo, and the message says so. A
load replaces the four-bar draft and its source path. If the source file is
missing, the pattern plays the palette sound, and the message says so. The
viewed bar does not change. Clear bar clears only the sixteen steps of the
viewed bar.

`pattern::PatternHistory`, which `step_synth::Editor` holds, keeps the board's
undo history. Each pointer gesture, bar action, or Randomise that changes the
notes or the length is one step. Each step has a name for the status line, such
as "Undid Clear bar.". The undo and redo keys of the toolbar return
`step_synth::Action::Undo` and `Action::Redo`. A load and Randomise change the
notes from outside the grid. After such a change, the next frame moves the grid
to the octave nearest the middle of the notes and scrolls to it.
`src/ui/input.rs` sends Ctrl+Z and Ctrl+Y to the board history if the pointer
or the focus is in the board or its toolbar. Elsewhere it sends them to the
song history. Input ownership uses the visible interaction area and the layer,
so a window above the board keeps its own input. The L key runs
`Command::StepSynthListen` while the board shows. The 25 visible pitch rows do
not limit the serialized note range of -48 through +48.

`synth::render` plays `StepSynthState::notes`. Each note continues through its
ties. It then sounds for the share of its last step that the gate sets. If it
slides into the next note, it sounds up to that note. A slide glides pitch and
level over the slide time, and does not restart the source. A new target starts
from the pitch and level that the slide has reached. Accent headroom passes
through the sequence effects and the output gain before the final sample clamp.
The public PCM boundaries still need normalized samples.

Swing delays each second sixteenth. The audition marker uses that timing and
the preview revision, so another preview cannot keep an old marker. A pattern
repeats until its length aligns with the bar. Thus a twelve-step pattern renders
48 steps. Each note that does not slide ends with a 3 ms fade.

Patterns saved with Active bar switches and the 1/16 cut still load. The
Active bars come first and set the length, and the other bars follow. Without
the cut, the empty steps after a note become ties. A pattern of that form with
no Active bar is invalid. Because of the end fade, a loaded pattern of that
form does not render the same samples that it rendered when saved. The board
adds no worker.
