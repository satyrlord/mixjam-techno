# Components: Mixer, tool windows, and FX returns

## Mixer and tool windows

Mixer uses compact ivory strips against a graphite material surround. Track
identity and gain lead the hierarchy. L/R routing uses small named toggle
buttons with live enabled indicators. Mute and Solo sit in their own row
above polarity and L/R, after a 26 by 24-point EQ button above polarity that
fills amber while the EQ, drive, compressor, or ducker is on, or the width differs
from 100%. The status line under pan names why a track is
silent: Muted, then No output (both L and R off), then Solo elsewhere.
Gain runs from
negative infinity (silence) to +12 dB with 0 dB at unity. Negative dB attenuates
the signal. Values round to one decimal place at rest; the active entry field
shows full precision. Focus without an edit must not change saved amplitude
or audio. Relative increases must leave silence and raise very quiet positive
gains. A saved gain below -96 dB must not become silent when increased.
Master uses the same dB presentation and remains outside the scroller. In
place of pan and routing it shows Main output and L + R, and under them an
80 by 24-point Bypass button that fills amber while the master pedalboard is
bypassed. A bypassed Mastering board draws at 45% opacity and stays usable.

The channel EQ window uses the neutral Utility palette. A power switch and
Reset share its top row; the response curve, from 20 Hz to 20 kHz and ±24 dB
on a dark inset with grid lines at 100 Hz, 1 kHz, 10 kHz, and 12 dB steps,
follows; six columns below name the bands, Low cut, Low shelf, Mid peak, Peak
2, High shelf, and High cut, with their Mastering dials stacked. The curve uses the
palette's print color while the EQ is on and faint cream while it is off;
the dials are disabled while it is off. Below the EQ, the drive's power switch
starts a row of five dials in the band columns: Amount, Character, Mix, Focus,
and Output. These dials are disabled while the drive is off. Below the drive,
the compressor's power switch starts a row of five dials in the band columns,
Thresh, Ratio, Attack,
Release, and Makeup, disabled while the compressor is off; the sixth column,
headed Stereo, holds the Width dial, which has no switch. Below them, the
ducker's power switch starts a row of five dials in the same columns, Key (a
track number), Thresh, Depth, Attack, and Release, disabled while the ducker
is off. Each section starts under a divider. The Key dial steps over the
window's own track. The window's content is 520 points wide. Its default outer
width reserves the vertical scrollbar, so a short viewport scrolls the content
vertically without adding horizontal overflow. The strip's EQ button lights
while the EQ, drive, compressor, or ducker is on, or while the width is not 100%.

Four send knobs sit below each track's gain rail. After the tracks, four return
strips hold the FX bus return levels, input switches, limiter switches, and
Edit buttons. Edit opens the selected bus on the FX returns board.

The rail uses a piecewise dB taper. From bottom to top, its normalized knots
are silence at 0, -60 dB at 0.08, -36 dB at 0.16, -24 dB at 0.28, -12 dB at
0.46, unity at 0.70, and +12 dB at 1. The bottom segment maps linear amplitude
to exact zero; the remaining segments interpolate in dB. Thus 72% of travel
serves -24 through +12 dB. The scale marks useful values without spreading
very quiet settings over most of the rail. Its labels are +12, 0, -12, -24,
-48, and negative infinity; 0 sits left of the rail beside the Unity button. These knots record the current
implementation, not a universal DAW taper.

The small Unity button sits at the 0 dB gain mark. The small Center button
resets pan at its rail midpoint. A separate slashed-circle button reverses
both channels' polarity. Amber means inverted and its checked state is named.
Polarity stays independent of gain and pan, even at silence; Unity leaves it
unchanged. Pan retains a readable left/center/right value. Every control has
a track-specific accessible name; Master has its own gain labels. The header
is the lower panel's tab row. Control-specific tooltips and the Help
window explain silence, unity, attenuation, and independent polarity.

`src/ui/mixer/artwork.rs` paints prepared ivory and graphite tiles and a separate
24 by 32-point fader cap. Nine PNGs cover 100%, 150%, and 200% density. The tiny
cap is first reduced to a 48 by 64-pixel canonical grid before preparing the
lower densities, preserving broad ridges and its ivory indicator stripe. All
text, values, rails, ticks, reset buttons, polarity, routing, focus, and hover
remain native. Select the nearest density and paint one texel per physical
pixel with aligned bounds; tile materials without runtime texture resizing.
See `assets/mixer/README.md` for exact prompts, source hashes, and reproduction.
Native mixer buttons use Mixer Face at rest and shared Raised on hover;
keyboard focus changes their one-point border to Mixer Amber and adds a
two-point Mixer Ink ring, because amber alone is illegible on ivory. The gain
rail, pan rail, and value field show the same ink ring or a cream edge. Each 28 by
24-point M/S/L/R button pairs its letter with a 12 by 2-point indicator bar.
Arrangement and mixer M/S/L/R labels use Accent when on and Muted when off.
Both expose the same checked state and a track-specific on/off tooltip. Lane
controls show keyboard focus. A lane edit requests a repaint so the already
drawn mixer refreshes even if the pointer stops. Pan remains independent of L/R.
The sample-conflict modal confirms only through Keep choices, because
confirmation is an undoable project change; Esc hides it without an edit and
the status bar reopens it.
The idle status is a usage hint in Muted: "Space plays the song. Drag a sound
from the Sounds panel onto the score. Ctrl+K lists every command." It
truncates with the full text as a hover hint. Errors and application notices
replace it and remain visible.
The empty mixer has one instruction for adding a sound to the arrangement.
All tool windows and utility dialogs use the same Folded Metal shell. The
frontend submits edits to the application layer; visual changes preserve those
commands.

## FX returns board

The FX returns board shows four bus units side by side on a Marine board. The
row scrolls in both directions when necessary. Each raised unit identifies its
bus and effect, and provides the effect and preset selectors, settings, return
level, input and limiter switches, and Clear tail. Empty units ask the user to
choose an effect. Echoform Delay uses blue enamel and Aetherform Reverb uses
violet enamel. An inactive or empty unit dims its enclosure and lamp.

## Shared shell and controls

`src/ui/dialog.rs` owns shell layout, position, resizing, and open/collapsed state.
`src/ui/controls.rs` owns the shared header controls, action buttons, rotary dials,
sliders, switches, and value entry. Tool bodies own values, validation, and actions.
A tool must not introduce another button renderer or a complete per-tool skin.
The same rules apply to Help, task confirmations, and musical editors.

Secondary buttons use dark faces and cream text; primary buttons use pale metal
and dark ink. Destructive actions have explicit names and the rust-red Danger face.
Tertiary buttons, for rare reversible actions such as Restore options defaults,
show underlined cream text and gain the dark face only on hover or press. All roles use
the same padding, label alignment, focus, hover, pressed, and disabled behavior.
Help lines under a control use the 12-point text face in the theme's help
colour, quieter than the 13-point label, with 4 points to their control
and more space before the next group.
Enabled button labels retain at least 4.5:1 contrast in their resting, hovered,
and pressed states, including the material's highlights and shading.
Disabled controls explain why they are unavailable. Preserve accessible names,
numeric values, ranges, selected states, and keyboard actions.

Rotaries share 150 logical points of full-range drag travel. Accumulate unrounded
movement, apply Shift fine adjustment only to new movement, and stop changing a
value when the pointer stops. Consume wheel events once. Preserve reset, exact
entry, arrow keys, Home/End, and accessible numeric actions. Parameter definitions
retain logarithmic mapping and discrete choices. Display rounding never changes a
draft.

Tempo choices use the stepped tempo slider, after the original products' speed slider: one
notch per supported tempo at even spacing, a silver cap, and snapping to the
nearest notch on press, drag, wheel, arrow keys, Home/End, and accessible numeric
actions. Arrow keys keep focus. The compact form, in the top bar and Step synth,
reads the value at its right end; Time Stretcher's form labels every notch.

Keep status inside the shared shell, with error text and recovery actions where
needed. Long feedback wraps or scrolls. Preserve the full tooltip and accessible
status. Existing Help actions remain available after collapse. The main workspace
and Mixer do not adopt this floating-window redesign.
