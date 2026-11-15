# Lower-panel boards

## Mixer

`src/ui/mixer.rs`, with its `src/ui/mixer/` modules, owns the compact strip
presentation of the Mixer in the current theme. Its `gain_keys` and
`gain_semantics` (`mixer/gain.rs`) give the gain rail of the Mixer and the lane
fader the same wheel, key, accessibility, and double-click behaviour. Vintage
tiles the graphite and ivory artwork (`mixer/artwork.rs`). Studio and Aero
paint the strips and fader caps in code at the same physical-pixel scale.

Each strip is 108 logical points wide. Master stays outside the occupied-track
bank, which scrolls horizontally. On each track strip, the EQ button starts the
row of Mute and Solo, above polarity. The Bypass of the Master strip is under
its output labels. The two buttons show the project state.

`src/ui/eq.rs` owns the channel EQ window. The frontend keeps the track that
the window shows. The window edits copies of the `ChannelEq`, `Drive`,
`Compressor`, width, and `Ducker` of that track, and sends them as
`EditCommand::SetTrack`. It uses the Mastering dials and paints its response
curve. The toolbar of the Mixer holds Export tracks, which asks for a folder
through `ask_path`.

## Mix view

`src/ui/mix_view.rs` owns the Mix view of the score. The `MixViewState::shown`
flag of the frontend turns the header of each used lane into a row of fixed
columns. The clips keep the remainder of the lane. The row uses the control
helpers of `src/ui/mixer/` (`switch_button`, `polarity_button`, `gain_value`,
`unity_button`, `pan_slider`, `center_button`, `send_knob`, `eq_button`,
`track_state`, and `row_surface`) and the lane fader. Thus the accessible names
and the operations are the same as those of the strips. Below the lanes, a
pinned block holds the Master row (Bypass, level, Export tracks) and the
returns row (`return_state`, `bus_switches`, `edit_button`).

Each change goes back to `apply_mixer_changes` in `src/ui/view/boards.rs`, the
same path that the Mixer board uses. A strip or row copies only its own track,
and gives it back when it changed. The edits go to the session as follows:

- One changed track goes as `EditCommand::SetTrack`.
- The master level, or more than one track in a frame, goes as one
  `SetMixer`.
- The buses go as `SetEffects`, and the board goes as `SetMaster`.

When the header column of the lanes is narrower than the 876 points of the
rows, the columns after the track number and arm scroll sideways by
`MixViewState::scroll`. The caption bar, Shift with the wheel, or a control
that gets keyboard focus outside the visible part scrolls them. Header rows
take horizontal wheel input after their controls and before the score scrolls.
The track rows and the pinned Master and returns block include the frontend
generation in their control identities. Thus drafts and held controls cannot
edit a replacement project.

The Mix view and the Mixer board never show together. `set_mix_view` moves the
lower panel to Mastering, and the Mixer button of the tool rail turns the Mix
view off.

## Mixer controls and artwork

Native controls paint live labels, dB values, rail marks, pan, routing,
polarity, focus, and hover. Gain uses a piecewise dB taper, with 72% of the
travel between -24 and +12 dB. The bottom section maps linear amplitude to
exact silence. Unity is at 0 dB on the gain rail. Center aligns with the pan
midpoint, just below its rail. Polarity has a separate slashed-circle button.
Control tooltips and the Help window give detailed guidance.

Prepared material tiles and the fader cap are in `assets/mixer/100/`. The
fader is first reduced to a canonical grid of 48 by 64 pixels, and then
prepared at 24 by 32 pixels. Runtime painting aligns one texel to one physical
pixel, with no texture resize. The empty bank gives one instruction and keeps
Master available.

## FX returns board

`src/ui/fx.rs` owns the FX returns board. It puts the four buses in raised
units on a Marine surface, with the effect and preset choices, the dials, the
return level, the switches, and Clear tail. `src/ui/view/boards.rs` sends a
changed bus set as an edit, and sends Clear tail to the session. The Edit
button of a return strip in the Mixer shows the FX returns board and brings
that bus into view.

## The lower panel

The `BottomModule` enum of the frontend selects the board of the lower panel:
Mixer, Mastering, FX returns, or Step synth. The four Mix buttons of the tool
rail choose it. Each button shows its board. If that board already shows, the
button folds the lower panel.

`src/ui/view/boards.rs` owns the 28-point top row of the panel. The row holds
the name of the shown board as a heading, a separator, and then the controls of
that board:

- Mastering: its preset, Bypass all, status, and help.
- Mixer: Export tracks….
- Step synth: Notes and Effects, its status line, Import details, and help.

Each board renders only its body below that row. When the lower panel is
folded, it leaves no strip. The Mix view of the score has its own Arrange and
Mix switch in the score toolbar, and a Mix view command in the command list.

The selection and the shared panel height are frontend session state. A
switch keeps the resized height. When a board opens, it raises a shorter panel
to its minimum: 400 points for Mixer, or the whole board for Mastering, FX
returns, and Step synth. In a window that is too short to keep a 150-point lane
viewport, each minimum goes down to 370 points, and the board then scrolls.

If the Starting module is Sounds, the launch opens the Sounds panel and folds
the lower panel. `from_session` stores that layout, and the lower panel keeps
Mixer for when it opens.

## Mastering board

`src/ui/master.rs`, with its `src/ui/master/` modules, owns the Mastering
module of the lower panel, its preset selector, and its Bypass all switch. The
module is a horizontal pedalboard in signal order:

- The Input unit (trim and VU meter) stays at the left.
- The Output unit (loudness, true peak, OVER) stays at the right.
- Between them, the pedal row scrolls, and painted cables join the pedals.
- A strip below names each pedal in order and outlines the pedals in view. It
  scrolls the row to the pedal that the user chooses.

Keyboard focus also scrolls a control into view. Each pedal shows its name,
lamp, dials, arrow buttons, and switch. The dynamics pedals also show their
gain-reduction lamps. A pedal that is off is dimmed. A drag of the header of a
pedal moves the pedal, with an insertion marker during the drag, and the row
scrolls near its ends. The arrow buttons give the same move to the keyboard,
screen readers, and agents.

A project or library replacement advances a frontend generation for the
control identities of the board. Thus a held pedal or knob cannot edit the
replacement project. This resets the board view. Meter latches keep a separate
stable identity, so an OVER count that the user acknowledged does not come on
again without another event.

The board edits a copy of the `MasterChain` of the project.
`src/ui/view/boards.rs` sends a changed copy as `EditCommand::SetMaster` and
gives the meter reading of the session to the board. The preset selector
offers the factory boards of `src/master/presets.rs`, which the frontend gets
through `application::presets`. A chosen preset is one `SetMaster` edit that
keeps the trim, bypass, and keys of the board (`Preset::applied_to`). The
selector shows the name of the preset whose pedals are equal to the pedals of
the board, independently of the trim and keys.

Dial values show in their units: the Booster EQ in decibels, and the other
Booster knobs in the 0 to 1000 steps of the original. Dials move on the grid of
their knob, and a fine step still moves one step. A typed value takes the
nearest step. When a dial shows or gets focus, the saved values between steps
do not change. The OVER control gives its latched state as text to assistive
technology.

## Step synth board

`src/ui/step_synth.rs` and the files in `src/ui/step_synth/` own the source,
bar, grid, effects, and action content of Step synth, on an Olive board in the
lower panel. The settings are in a
fixed column at the left. The grid column fills the remainder of the panel and
shows one, two, or four bars of sixteen steps: as many as keep each step a
minimum of 28 points wide. The grid has twenty-five pitch rows around the
octave in view, and the octave moves from -3 to +3. Below the grid are the
Accent, Slide, and Velocity lanes.

The board is an accessible tab panel named Step synth, and agents scope to it.
Its status line and Import details are in a named Step synth toolbar in the top
row of the panel. The agent scopes this toolbar with the board. The board is
512 points high, and the panel gives it its whole height when the window has
the space. Text, grid cells, switches, and input stay live. The pitch grid,
the settings column, and the replacement effects form have bounded scrolling.
The source, the step labels, the actions, and the feedback stay available. The
shared `effects_controls` form keeps its algorithms and ranges, and uses the
common controls.

`StepSynthBoard` in `src/ui/view/boards.rs`, which `MixJamApp` holds, owns the
draft, the source path, the viewed bar, the effects-view selection, the grid
view (`step_synth::Editor`: octave, scale, root, and selected steps), and the
local feedback. The grid view is not saved with the pattern.
`src/ui/view/boards.rs` adapts the actions of the panel to current `Session`
operations.

The board only draws and reads input. The pattern edits and the board's undo
history do not use the GUI. They are in `src/synth/edit.rs` and
`src/synth/history.rs`, and the board calls them through
`application::pattern`. Thus a change of look stays in the board files, and
`src/synth/edit_tests.rs` tests the edits without a window. The board files
are these:

- `step_synth.rs`: the source, feedback, actions, and the board's frame.
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
hit and duration. An explicit path that is not a WAV is read as a stand-alone
packed sample file. It decodes as the stand-alone packed files of the library
do: the predictor starts from
the header word, and the audio ends at the last complete opcode, also when the
header declares a longer duration.

When the user selects Listen or Save sound,
`Session::step_synth_pcm` reads the source and renders synchronously. The
frontend then requests the PCM preview or the WAV save workflow. Save registers
the result in Step synth at the draft BPM and keeps the source selection.
`Session::save_sequence` stages the WAV and the draft at
`synth::pattern_beside` as one batch, with rollback if a replacement fails.
Registration comes after a successful publication. The save choice, the
cancellation, a failure, and a success update the local feedback.
`MixJamApp::step_synth_say` puts each board message on the local feedback
and on the main status line together. For missing palette audio, the board
offers Import details.

`synth::render_measured` gives the render and the dB that it was turned
down. `effects::process_synth` runs the pattern effects without a clip, and
when the peak is over full scale it scales the whole render to full scale.
Listen and Save sound name the dB in their message.

`MixJamApp::step_synth_adopt_source` watches the source key: the file path,
or the palette sound. The first source after the start or a load is kept as
it is. A later change sets Start to the first hit of the new source and turns
Loop source off, as one undo step. It also gives the source length, which
limits the Start dial.

Pattern save and load use `Session::save_pattern` and
`Session::load_pattern_snapped`, which also gives the BPM of the file. A load
moves that BPM to the nearest supported tempo, and the message says so. A
load replaces the four-bar draft and its source path. If the source file is
missing, the pattern plays the palette sound, and the message says so. The
viewed bar does not change. Clear bar clears only the sixteen steps of the
viewed bar.
`pattern::PatternHistory`, which `step_synth::Editor` holds, keeps the
board's undo history: each pointer gesture,
bar action, or Randomise that changes the notes or the length is one step,
with a name for the status line, such as "Undid Clear bar.". The undo and
redo keys of the toolbar return `step_synth::Action::Undo` and
`Action::Redo`. A change that comes from outside the board, such as a load,
and Randomise cause the next frame to move the grid to
the octave nearest the middle of the notes and scrolls to it.
`src/ui/input.rs` sends Ctrl+Z and Ctrl+Y to it while the pointer or the
focus is in the board or its toolbar, and to the song history elsewhere.
Input ownership uses the visible interaction area and the layer, so a floating
window over the board keeps its own input. The L key runs
`Command::StepSynthListen` while the board shows. The 25
visible pitch rows do not limit the serialized note range of -48 through +48.

`synth::render` plays `StepSynthState::notes`. Each note continues through its
ties. It then sounds for the share of its last step that the gate sets, or up
to the next note when it slides into that note. A slide glides pitch and level
over the slide time, and does not restart the source. A new target starts from
the pitch and level that the slide has reached. Accent headroom goes through
the sequence effects and the output gain before the final sample clamp. The
public PCM boundaries still need normalized samples.

Swing delays each second sixteenth. The audition marker uses that timing and
the preview revision, so another preview cannot keep an old marker. A pattern
repeats until its length aligns with the bar. Thus a twelve-step pattern renders
48 steps. Each note that does not slide ends with a 3 ms fade.

Patterns saved with Active bar switches and the 1/16 cut still load. The
Active bars come first and set the length, and the other bars follow. Without
the cut, the empty steps after a note become ties. A pattern of that form with
no Active bar is invalid. Because of the end fade, a loaded pattern of that
form does not render sample for sample as it did when it was saved. The board
adds no worker.
