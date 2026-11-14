# Lower-panel boards

## Mixer

`src/ui/mixer.rs`, with its `src/ui/mixer/` modules, owns Mixer's compact strip
presentation in the current theme. Its `gain_keys` and `gain_semantics`
(`mixer/gain.rs`) give the Mixer's gain rail and the lane fader the same wheel,
key, accessibility, and double-click behaviour. Vintage tiles the graphite and
ivory artwork (`mixer/artwork.rs`); Studio and Aero paint the strips and fader
caps in code at the same density scale.
Each strip is 108 logical points wide. Master remains outside the horizontally
scrolling occupied-track bank. Each track strip's EQ button starts the row of
Mute and Solo, above polarity, and the Master strip's Bypass sits under its
output labels; both mirror project state. `src/ui/eq.rs` owns the channel EQ
window: the frontend keeps which track it shows, and the window edits copies
of that track's `ChannelEq`, `Drive`, `Compressor`, width, and `Ducker`, sent
as `EditCommand::SetTrack`. It reuses the
Mastering dials and paints its response curve. The Mixer's toolbar holds
Export tracks, which asks for a folder through `ask_path`.

## Mix view

`src/ui/mix_view.rs` owns the score's Mix view. The frontend's `mix_view`
flag turns each used lane's header into a row of fixed columns; the clips keep
the rest of the lane. The row reuses the control helpers of `src/ui/mixer/`
(`switch_button`, `polarity_button`, `gain_value`, `unity_button`,
`pan_slider`, `center_button`, `send_knob`, `eq_button`, `track_state`, and
`row_surface`) and the lane fader, so the accessible names and operations are
the strips'. Below the lanes, a pinned block holds the Master row (Bypass,
level, Export tracks) and the returns row (`return_state`, `bus_switches`,
`edit_button`). Every change returns to `apply_mixer_changes` in
`src/ui/view/boards.rs`, the path the Mixer board uses. A strip or row copies
only its own track and returns it when it changed. One changed track is sent
as `EditCommand::SetTrack`; the master level or more than one track in a frame
as one `SetMixer`; the buses and the board as `SetEffects` and `SetMaster`. When the lanes' header column is
narrower than the rows' 876 points, the columns past the track number and arm
scroll sideways by `mix_scroll`, through the caption bar, Shift and the wheel,
or a control that takes keyboard focus outside the visible part. Header rows
take horizontal wheel input after their controls and before the score scrolls.
Track rows and the pinned Master and returns block include the frontend
generation in their control identities, so drafts and held controls cannot
edit a replacement project. The Mix view
and the Mixer tab never show together: `set_mix_view` moves the lower panel to
Mastering, and selecting the Mixer tab turns the Mix view off.

## Mixer controls and artwork

Native controls paint live labels, dB values,
rail marks, pan, routing, polarity, focus, and hover. Gain uses a piecewise dB
taper with 72% of travel between -24 and +12 dB; the bottom section maps linear
amplitude to exact silence. Unity sits at 0 dB on the gain rail. Center aligns
with the pan midpoint just below its rail. Polarity has a separate
slashed-circle button. Control-specific tooltips and the Help window carry
detailed guidance.
Prepared material tiles and fader caps live in `assets/mixer/{100,150,200}/`.
The fader is first reduced to a 48 by 64-pixel canonical grid, then prepared
at 24 by 32, 36 by 48, and 48 by 64 pixels. Runtime painting aligns one texel
to one physical pixel, with no texture resizing. The empty bank gives one
instruction and leaves Master available.

## FX returns board

`src/ui/fx.rs` owns the FX returns board. It places the four buses in raised
units on a Marine surface, with the effect and preset choices, dials, return
level, switches, and Clear tail. `src/ui/view/boards.rs` sends a changed bus set as
an edit and sends Clear tail to the session. The Mixer's return-strip Edit
button selects the FX returns tab and brings that bus into view.

## The lower panel and its tabs

The frontend's `BottomModule` enum selects the lower panel's board: Mixer,
Mastering, FX returns, or Step synth. `src/ui/view/boards.rs` owns the panel's
28-point tab row: one tab per board in that order, each named after its board
and "tab". The selected tab has a raised fill and an accent underline, and its
accessible state is toggled. The shown board's own controls follow a separator:
Mastering's preset, Bypass all, status, and help; Mixer's Export tracks…; and
Step synth's status line, Import details, and help. The tabs keep their
identity and position across boards. Each board renders only its body below
the tab row. The tool rail's Mixer shortcut toggles the score's Mix view; Mastering
and Step synth show their tab, and fold the lower panel when it already
shows. Selection and shared panel height are frontend
session state. Switching preserves resized height; opening a board raises a
shorter panel to its minimum: 400 points for Mixer, or the whole board for
Mastering, FX returns, and Step synth. Each yields down to 370 points in a
window too short to keep a 150-point lane viewport, and the board then
scrolls. A launch whose Starting module is Sounds opens the Sounds panel and
folds the lower panel; `from_session` stores that layout, and the lower panel
keeps Mixer for when it opens.

## Mastering board

`src/ui/master.rs`, with its `src/ui/master/` modules, owns the Mastering
module of the lower panel, its preset selector, and its Bypass all switch: a
horizontal pedalboard in signal order. The Input unit (trim and VU meter) stays at the left and the
Output unit (loudness, true peak, OVER) at the right; between them the pedal
row scrolls, joined by painted cables. A strip below names every pedal in
order, outlines those in view, and scrolls the row to the one chosen; keyboard
focus scrolls a control into view too. Each pedal shows its name, lamp, dials,
arrow buttons, and switch, and the dynamics pedals their gain-reduction lamps;
a pedal that is off is dimmed. Dragging a pedal's header moves it, with an
insertion marker while it is dragged, and the row scrolls near its ends; the
arrow buttons give the same move to the keyboard, screen readers, and agents.
Project or library replacement advances a frontend generation for the board's
control identities. A held pedal or knob cannot edit the replacement project.
This resets the board view; meter latches keep a separate stable identity so
an acknowledged OVER count does not light again without another event.
It edits a copy of the project's `MasterChain`; `src/ui/view/boards.rs` sends a changed
copy as `EditCommand::SetMaster` and passes the session's meter reading. The
preset selector offers the factory boards of `src/master/presets.rs`, which
the frontend reaches through `application::presets`; a chosen preset is one
`SetMaster` edit that keeps the board's trim, bypass, and keys (`Preset::applied_to`), and the
selector names the preset whose pedals the board's equal, whatever the trim or keys. Dial
values read in their units, the Booster EQ in decibels and the other Booster knobs in
the original's 0 to 1000 steps. Dials move on their knob's grid, and a fine
step still moves one; a typed value takes the nearest step. Displaying or
focusing a dial preserves saved values between steps. The OVER control exposes
its latched state as text to assistive technology.

## Step synth board

`src/ui/step_synth.rs` owns Step synth's source, bar, grid, effects, and action
content on an Olive board in the lower panel. The board is an accessible tab
panel named Step synth, so agents scope to it as they did to its window. Its
status line and Import details sit in a named Step synth toolbar in the tab
row. The agent scopes this toolbar with the board. This keeps the board no
taller than the Mastering board. Text, grid cells, switches, and input stay live.
The pitch grid and replacement effects form have bounded scrolling; source,
step labels, actions, and feedback remain reachable. The grid exposes sixteen
steps and twenty-five pitch rows. The shared `effects_controls` form retains its
algorithms and ranges and uses the common controls.

`MixJamApp` in `src/ui/app.rs` owns the source path, viewed bar, effects-view
selection, draft, and local feedback. `src/ui/view/boards.rs` adapts the
panel's actions to existing `Session` operations. An explicit WAV or PXD path
overrides palette or clip selection, including missing palette audio.
Selecting it does not read audio. An explicit path that is not a WAV is read
as a stand-alone packed sample file. It decodes like the library's
stand-alone packed files: the predictor starts from the header word, and the
audio ends at the last complete opcode, even when the header declares a
longer duration.
`Session::step_synth_pcm` reads the source and renders synchronously when Listen
sequence or Save sequence is requested. The frontend then requests PCM preview
or the existing WAV save workflow. Save registers the result in Step synth at
the draft BPM and selects it. `Session::save_sequence` stages the WAV and draft
at `synth::pattern_beside` as one batch, with rollback if replacement fails;
registration follows successful publication. Save choice, cancellation, failure,
and success update local feedback. Missing palette audio offers Import details.
Pattern save/load uses `Session::save_pattern` and `Session::load_pattern`.
It replaces only the four-bar draft; the source path and viewed bar remain.
Clear bar clears only the viewed sixteen steps. The visible +12 through -12
pitch rows do not narrow the existing -48 through +48 serialized-note range.
This presentation does not add a worker.
