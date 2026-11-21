# Lower-panel boards

## Contents

These boards have their own pages:

- [Mastering board](mastering-board.md#mastering-board): the Mastering module
  of the lower panel.
- [Step synth board](step-synth-board.md#step-synth-board): the Step synth
  board of the lower panel.

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

`src/ui/mix_view.rs` and its modules in `src/ui/mix_view/` own the Mix view
of the score:

- `layout.rs` holds the column table. Each column is a `Column` constant, and
  the rows, the captions, the Master and returns rows, and the tests read the
  same table. `Columns` places a column on screen for a row, its scroll
  offset, and the extra room of a wide header. `return_parts` divides the
  returns row among the buses, and `reveal` gives the scroll that shows a
  focused control. This module has no Ui and no session.
- `rows.rs` paints a track's row from plain values: `RowFacts`, the
  `TrackMix` that it edits, and the row's names. It reports an `Outcome`.
- `block.rs` holds the captions, the Master row (Bypass, level, Export
  tracks), the returns row (the effect, its bus and `return_state`, the
  return knob, Input, Limiter, and `edit_button`), and `mix_block`, which
  places them below the lanes and sends their edits.
- `state.rs` holds `MixViewState`, `set_mix_view`, and `score_block_height`.

The `MixViewState::shown` flag of the frontend turns the header of each used
lane into a row of fixed columns. The clips keep the remainder of the lane,
and `score_frame` fits the whole song to the lanes. The rows use the widgets
of `src/ui/mixer/` (`switch_button`, `polarity_button`, `gain_value_in`,
`unity_button`, `pan_slider`, `center_button`, `send_knob`, `eq_button`, and
`bus_switches`) in `Look::Quiet`, and `strip_fader`, the lane fader on the
strip's rail roles. A quiet widget paints a row at its defaults without lit
faces. It keeps the name, keys, state, and hint of the strip's widget, so the
accessible names and the operations are the same as those of the strips.
While columns remain to the right, `edge_fade` paints each row's surface
again over its last 24 points with rising opacity, through `row_surface_on`
and a clipped painter.

Each change goes back to `apply_mixer_changes` in `src/ui/view/boards.rs`, the
same path that the Mixer board uses. A strip and a Mix view row edit the
`TrackMix` of their track: the level, polarity, pan, switches, and sends. It
is small and `Copy`, so no view clones a track each frame. `TrackMix::silence`
gives why the mix cannot hear a track (Muted, No output, or Solo elsewhere),
and `Track::processing_on` lights the EQ button. The render, the lanes, the
overview, and the rows all use these backend rules. The edits go to the
session as follows:

- One changed track goes as `EditCommand::SetTrackMix`. The session applies it
  as a `SetTrack` that keeps the processing and automation of the track, so
  undo steps and live levels are the same.
- The master level, or more than one track in a frame, goes as one
  `SetMixer`.
- The buses go as `SetEffects`, and the board goes as `SetMaster`.

A wider column gives up to 40 points to the sounds' column, and the
columns after it shift right. When the header column of the lanes is narrower
than the 944 points of the rows, the columns after the track number and arm
scroll sideways by
`MixViewState::scroll`. The caption bar, Shift with the wheel, or a control
that gets keyboard focus outside the visible part scrolls them. Header rows
take horizontal wheel input after their controls and before the score scrolls.
The track rows and the pinned Master and returns block include the frontend
generation in their control identities. Thus drafts and held controls cannot
edit a replacement project.

The Mix view and the Mixer board never show together. `set_mix_view` moves the
lower panel to Mastering, and the Mixer button of the tool rail turns the Mix
view off. `set_mix_view` also folds the lower panel and keeps
`MixViewState::restore_boards`, so leaving the view opens the panel again; a
`set_layout` call that opens or folds the panel in between clears it. A launch
with the Starting module Mixer folds the panel in the same way.

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

`src/ui/fx.rs` owns the FX returns board, with the unit faces and the unit
drawing in `src/ui/fx/`. It puts the four buses in raised
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
- Step synth: Notes and Effects, its undo and redo keys, Import details, and
  help. Its messages show in the status bar.

Each board renders only its body below that row. When the lower panel is
folded, it leaves no strip. The Mix view of the score has its own Arrange and
Mix switch in the score toolbar, and a Mix view command in the command list.

The selection and the shared panel height are frontend session state. A
switch keeps the resized height. When a board opens, it raises a shorter panel
to its minimum: 400 points for Mixer, or the whole board for Mastering, FX
returns, and Step synth. In a window that is too short to keep a 150-point lane
viewport, each minimum goes down to 370 points, and the board then scrolls.
While the Mix view shows, the score also keeps its 70-point Master and returns
rows below the lanes. The reserve then grows by 70 points, and the floor goes
down to 300 points.

If the Starting module is Sounds, the launch opens the Sounds panel and folds
the lower panel. `from_session` stores that layout, and the lower panel keeps
Mixer for when it opens.
