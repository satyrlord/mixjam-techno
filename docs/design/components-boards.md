# Components: Mixer, tool windows, and FX returns

## Mixer

Mixer uses compact ivory strips in a graphite material surround. Track
identity and gain are first in the hierarchy. Strips show only for occupied
tracks. Track and Master strips are 108 points wide. Master stays outside the
horizontal track scroller, and the scroller keeps its scroll bar visible.
Track numbers, sound summaries, dB gain values, pan descriptions, and output
state stay grouped in each strip. A track with several different sounds shows
their count, and lists their names on hover. Narrow strips keep the visible
tracks without larger controls.

L/R routing uses small named toggle buttons with live enabled indicators. Two
rows of buttons are at the foot of a track strip:

- The upper row: a 26 by 24-point EQ button, Mute, and Solo. The EQ button
  fills amber while the EQ, drive, compressor, or ducker is on, or while the
  width is not 100%.
- The lower row: polarity, L, and R.

The status line under pan tells why a track is silent: Muted, then No output
(both L and R off), then Solo elsewhere.

Gain runs from negative infinity (silence) to +12 dB, with 0 dB at unity.
Negative dB attenuates the signal. Values round to one decimal place at rest.
The active entry field shows full precision. Focus without an edit must not
change the saved amplitude or the audio. Relative increases must leave
silence and raise very quiet positive gains. A saved gain below -96 dB must
not become silent when it increases.

Master uses the same dB presentation and stays outside the scroller. In place
of pan and routing, it shows Main output and L + R. Under them, an 80 by
24-point Bypass button fills amber while the master pedalboard is bypassed. A
bypassed Mastering board draws at 45% opacity and stays usable.

### Channel EQ window

The channel EQ window uses the neutral Utility palette. A power switch and
Reset share its top row. The response curve follows: from 20 Hz to 20 kHz and
±24 dB, on a dark inset with grid lines at 100 Hz, 1 kHz, 10 kHz, and 12 dB
steps. Below the curve, six columns name the bands: Low cut, Low shelf, Mid
peak, Peak 2, High shelf, and High cut. Each column stacks the Mastering dials
of its band. The curve uses the print color of the palette while the EQ is on,
and faint cream while it is off. The dials are disabled while the EQ is off.

Each of the next sections starts under a divider, with its power switch first
in a row of five dials in the band columns:

| Section | Dials | Notes |
| --- | --- | --- |
| Drive | Amount, Character, Mix, Focus, Output | Disabled while the drive is off. |
| Compressor | Thresh, Ratio, Attack, Release, Makeup | Disabled while the compressor is off. The sixth column, headed Stereo, holds the Width dial, which has no switch. |
| Ducker | Key (a track number), Thresh, Depth, Attack, Release | Disabled while the ducker is off. The Key dial steps over the track of the window. |

The content of the window is 520 points wide. Its default outer width keeps
space for the vertical scroll bar. Thus a short viewport scrolls the content
vertically and adds no horizontal overflow.

### Sends and returns

Four send knobs sit below the gain rail of each track. After the tracks, four
return strips hold the FX bus return levels, input switches, limiter switches,
and Edit buttons. Edit opens the selected bus on the FX returns board.

While an automation lane sets a send or a pan, a thin ring marks its knob or
slider. The Mix view and the Inspector use the same mark.

### Gain taper

The rail uses a piecewise dB taper. From bottom to top, its normalized knots
are:

| Position | Level |
| --- | --- |
| 0 | silence |
| 0.08 | -60 dB |
| 0.16 | -36 dB |
| 0.28 | -24 dB |
| 0.46 | -12 dB |
| 0.70 | 0 dB (unity) |
| 1 | +12 dB |

The bottom segment maps linear amplitude to exact zero. The other segments
interpolate in dB. Thus 72% of the travel serves -24 through +12 dB. The scale
marks useful values and does not spread very quiet settings over most of the
rail. Its labels are +12, 0, -12, -24, -48, and negative infinity. The 0 label
sits left of the rail, beside the Unity button. These knots record the current
implementation, not a universal DAW taper.

### Strip controls

The small Unity button sits at the 0 dB gain mark. The small Center button
resets pan at the midpoint of its rail. A separate slashed-circle button
reverses the polarity of both channels. Amber means inverted, and its checked
state is named. Polarity stays independent of gain and pan, even at silence.
Unity does not change it. Pan keeps a readable left, center, or right value.
Every control has an accessible name for its track. Master has its own gain
labels. The header is the top row of the lower panel. Control tooltips and the
Help window explain silence, unity, attenuation, and independent polarity.

`src/ui/mixer/artwork.rs` paints prepared ivory and graphite tiles and a
separate 24 by 32-point fader cap. Three PNGs hold the one prepared size of
each. The small cap is first reduced to a 48 by 64-pixel canonical grid, then
prepared at its final size, to keep the broad ridges and the ivory indicator
stripe. All text, values, rails, ticks, reset buttons, polarity, routing,
focus, and hover stay native. Paint one texel per physical pixel with aligned
bounds. Tile the materials without resizing textures at runtime. See
`assets/mixer/README.md` for the exact prompts, source hashes, and
reproduction steps.

Native mixer buttons use Mixer Face at rest and the shared Raised on hover.
Keyboard focus changes their one-point border to Mixer Amber and adds a
two-point Mixer Ink ring, because amber alone is not legible on ivory. The
gain rail, pan rail, and value field show the same ink ring or a cream edge.
Each 28 by 24-point M/S/L/R button pairs its letter with a 12 by 2-point
indicator bar. Arrangement and mixer M/S/L/R labels use Accent when on and
Muted when off. Both expose the same checked state and an on/off tooltip for
the track. Lane controls show keyboard focus. A lane edit requests a repaint,
so that the mixer refreshes even if the pointer stops. Pan stays independent
of L/R.

The empty Mixer shows one instruction that tells how to add a sound to the
arrangement. The frontend sends edits to the application layer. Visual
changes keep those commands.

## FX returns board

The FX returns board shows four bus units side by side on a Marine board. If
necessary, the row scrolls in both directions. Each raised unit identifies its
bus and effect. It holds the effect and preset selectors, the settings, the
return level, the input and limiter switches, and Clear tail. An empty unit
asks the user to choose an effect. The enamel of each effect is:

- Echoform Delay: blue.
- Aetherform Reverb: violet.
- Emberform Drive: ember.

An inactive or empty unit dims its enclosure and lamp.

## Shared shell and controls

All tool windows and utility dialogs use the same Folded Metal shell.
`src/ui/dialog.rs` owns the layout, position, resize, and open or collapsed
state of the shell. `src/ui/controls.rs` and `src/ui/controls/` own the shared
header controls, action buttons, rotary dials, sliders, switches, and value
entry. Tool bodies own values, validation, and actions. A tool must not add
another button renderer or a complete skin of its own. The same rules apply to
Help, task confirmations, and musical editors.

Secondary buttons use dark faces and cream text. Primary buttons use pale
metal and dark ink. Destructive actions have explicit names and the rust-red
Danger face. Tertiary buttons are for rare reversible actions, such as Restore
options defaults. They show underlined cream text, and get the dark face only
on hover or press. All roles use the same padding, label alignment, focus,
hover, pressed, and disabled behavior.

A help line under a control uses the 12-point text face in the help color of
the theme, quieter than the 13-point label. It sits 2 points below its control,
with more space before the next group. Enabled button labels keep at least
4.5:1 contrast in their resting, hovered, and pressed states, including the
highlights and shading of the material. Disabled controls tell why they are
not available. Keep accessible names, numeric values, ranges, selected states,
and keyboard actions.

Rotaries share 150 logical points of drag travel for the full range.
Accumulate the unrounded movement. Apply the Shift fine adjustment only to new
movement. Stop the change of a value when the pointer stops. Use each wheel
event once. Keep reset, exact entry, arrow keys, Home/End, and accessible
numeric actions. Parameter definitions keep logarithmic mapping and discrete
choices. Display rounding never changes a draft.

Tempo choices use the stepped tempo slider: one notch per supported tempo at
even spacing, and a silver cap. The slider snaps to the nearest notch on
press, drag, wheel, arrow keys, Home/End, and accessible numeric actions.
Arrow keys keep focus. The compact form, in the top bar and Step synth, reads
the value at its right end. The form in Time Stretcher labels every notch.

Keep status inside the shared shell, with error text and recovery actions
where necessary. Long feedback wraps or scrolls. Keep the full tooltip and
accessible status. Help actions stay available after collapse. The main
workspace and Mixer do not use the floating-window shell.
