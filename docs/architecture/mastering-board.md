# Mastering board

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
