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
`gain_semantics` (`mixer/gain.rs`) give the gain rail of the Mixer and the clip
fader the same wheel, key, accessibility, and double-click behavior.
`mixer/material.rs` paints the Vintage graphite and ivory materials and the
Vintage fader cap in code. Studio and Aero paint the strips and fader caps
with their theme surfaces. All three use the same physical-pixel scale.

Each strip is 108 logical points wide. Master stays outside the occupied-track
bank, which scrolls horizontally. On each track strip, the EQ button starts the
row of Mute and Solo, above polarity. The Bypass of the Master strip is under
its meter, which `master::strip_meter` draws from the same `Metering` sample
and OVER latch as Mastering. The two buttons show the project state.

`src/ui/eq.rs` owns the Channel Strip window. The frontend keeps the track that
the window shows. The window edits copies of the EQ, Drive, compressor, stereo
width, automated filter, and ducker settings. It sends the changes through
`EditCommand::SetTrack`. The scope row uses the channel EQ response that the
Inspector also shows, the Drive transfer curve, and the live compressor and
ducker gain reduction from the session. The toolbar of the Mixer holds Export
tracks, which asks for a folder through `ask_path`.

## Mixer controls and materials

Each strip change goes through `apply_mixer_changes` in `src/ui/view/boards.rs`,
editing the `TrackMix` of its track (level, polarity, pan, switches, and sends)
so views never clone a whole `Track` each frame:

- One changed track goes as `EditCommand::SetTrackMix`, preserving channel
  processing and automation.
- The master level, or more than one track in a frame, goes as one
  `SetMixer`.
- The buses go as `SetEffects`, and the board goes as `SetMaster`.

Backend rules such as `TrackMix::silence` (Muted, No output, or Solo elsewhere)
and `Track::processing_on` keep mute/solo and EQ status consistent across the
render, lane headers, overview, and strips.

Native controls paint live labels, dB values, rail marks, pan, routing,
polarity, focus, and hover. Gain uses a piecewise dB taper, with 72% of the
travel between -24 and +12 dB. The bottom section maps linear amplitude to
exact silence. Unity is at 0 dB on the gain rail. Center aligns with the pan
midpoint, just below its rail. Polarity has a separate slashed-circle button.
Control tooltips and the Help window give detailed guidance.

Vintage makes its ivory and graphite tiles at runtime from seeded noise in
`src/ui/grain.rs`, which also makes the brushed grain of the pedals. A tile is
256 by 256 texels and repeats without a seam. Painting starts each tile at the
physical pixel nearest to the top left of its strip and puts one texel on
each physical pixel, with no texture resize. The fader cap is vector
geometry of 24 by 32 physical pixels. The empty bank gives one instruction and
keeps Master available.

## FX returns board

`src/ui/fx.rs` owns the FX returns board. `src/ui/fx/` draws four cards with
effect and preset choices, macro dials, a return level, and switches. The Edit
button on a card opens FX deep edit, which holds the full settings, the scope
for Delay or Drive, and Clear tail. Populated cards also offer Clear tail.
The board and the editor send settings through the session as edits. Clear
tail calls the session separately. The Edit button on a Mixer return strip
shows the FX returns board and brings that bus into view.

## The lower panel

The `BottomModule` enum of the frontend selects the board of the lower panel:
Mixer, Mastering, FX returns, or Step synth. The four Mix buttons of the tool
rail choose it. Each button shows its board. If that board already shows, the
button folds the lower panel.

`src/ui/view/boards.rs` owns the 28-point top row of the panel. The row holds
the name of the shown board as a heading, a separator, and the controls of that
board:

- Mastering: its preset, Bypass all, status, and help.
- Mixer: Export tracks….
- Step synth: Notes and Effects, its undo and redo keys, Import details, and
  help. Its messages show in the status bar.

Each board renders only its body below that row. A folded lower panel leaves
no strip.

The selection and the shared panel height are frontend session state. A
switch keeps the resized height. When a board opens, it raises a shorter panel
to its minimum. The minimum is 400 points for Mixer, or the whole board for
Mastering, FX returns, and Step synth. If the window is too short to keep a
150-point lane viewport, boards taller than 370 points can shrink to that
height and scroll.

If the Starting module is Sounds, the launch opens the Sounds panel and folds
the lower panel while keeping `BottomModule::Mixer` selected for when it
opens. If Starting module is Mixer, Mastering, or FX returns, the launch opens
the lower panel on that board.
