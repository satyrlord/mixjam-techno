# Colors

## Primary

Accent lime identifies Play/Pause, primary empty-state actions,
the playhead, enabled routing controls, and focus or active-state outlines.
Coral identifies recording, errors, missing sources, and selected synth notes.

## Secondary

Loop, Drum, Bass, Sequence, Sphere/Wave, Effect, Voice, and Other Sample colors
identify sounds. Sound rows use a 20% category-color overlay; arrangement clips use the
full category fill with canvas-colored text. Selection adds a text-colored clip border.

**The Shared Category Rule.** Use the same category color for a sound in the
Sounds panel and the arrangement. Pair color with its name or category label.

## Neutral

Canvas is the deepest structural surface. Panel supports the top bar, the side
panels, the lower panel, and the overview. Floating tools use their Folded Metal palette. Raised and Hover
distinguish controls and interaction
states. Text carries primary labels; Muted carries counts, metadata, and status.
Line separates surfaces. Selection and Warning retain their native widget roles.
Drag Label fills the label that follows the pointer while a sound or clip is
dragged; it has no other use.

Folded Metal palettes share dark inset bays and controls, cream lettering,
polished silver knob caps, and pale metal primary buttons. Tool chassis colors
remain distinct: Marine for Time Stretcher, Granite for Sample Editor, Red for
Effect Studio, Olive for Step synth, Indigo for Mastering, and Marine for the
FX returns board. Utility is neutral and permits an orange-yellow accent. Use explicit names and states
alongside color; Coral continues to identify errors and destructive actions.
Palette changes must not change control geometry or behavior. Keep these
colors local to floating windows and the Mastering, FX returns, and Step synth boards; the arrangement,
Sounds panel, and Mixer retain their palettes.

| Palette | Window | Chassis | Active accent |
| --- | --- | --- | --- |
| Utility | Utility windows and dialogs | `#3c4140` | `#e79331` |
| Marine | Time Stretcher and FX returns board | `#20535b` | `#55aeb7` |
| Granite | Sample Editor | `#34393c` | `#adbabd` |
| Red | Effect Studio | `#9a2d27` | `#eb7468` |
| Olive | Step synth | `#3c4428` | `#9db25e` |
| Indigo | Mastering | `#2e3660` | `#96a6ec` |

Mastering's board fills the lower panel under the header row, an Indigo
Folded Metal surface with a 2-point inset. Its pedals are painted enclosures
on the Indigo board, each with its
own enamel and ink lettering, as stompboxes are. Light falls on them from the
upper left, at 135 degrees, as it does on the knob caps. Each enclosure has a
6-point rounded bevel that catches the light along its upper and left edges
and falls into shade along its lower and right ones, 4-point side walls in
shade below and to the right of the face, a faint sheen across the face
toward the light, and a soft drop shadow cast down and right. Text stays
inside the bevel. A pedal that is on shows anodised aluminium under its
enamel: fine brushed grain along the rows at one texel per screen pixel, a
sheen stretched across the grain (a bright streak toward the light, a darker
reflection, a fainter streak), and seeded hairline scratches where bare
aluminium shows. Each pedal keeps its own scratches, which stay below its
printed name. The finish darkens the enamel by at most 5% for the grain and
8% for the sheen, and ink keeps 5:1 there. A pedal that is off stays
matte. Families share hues, and the
enamel always appears with the pedal's printed name:

| Family | Pedals and enamels |
| --- | --- |
| Equaliser (golds) | Booster EQ `#deac40`, Trim EQ `#c4cc5c`, Lift EQ `#ecd06c` |
| Dynamics (blues and greens) | Booster compressor `#54b8a6`, Bus compressor `#78b2e4`, Maximizer `#82ce96`, Multiband `#64c4cc`, Limiter rose `#ee8ca0` |
| Saturation (warm oranges) | Soft clip `#f09678`, Tube `#f0a650`, Tape `#ceb48c` |
| Stereo (violets) | Booster wide `#c896dc`, Stereo imaging `#a6a0e8` |

The Booster pedals print the original's names (EQ, Compressor, Stereo wide)
over a "Booster" line; the mastering pedals print their names over
their family. A name too long for its pedal shrinks, down to 11 points, until
it clears the lamp. The Limiter's rose sets it apart as the board's fixed last stage; it has no
drag grip or move buttons, and its header line says it always runs last.
The Input and Output units at the ends of the board are dark inset bays with
cream lettering, like the other tools' bays. The VU meter has a Mastering
Ivory face, ink scale, a red zone above 0 VU, and Mastering Red peak lamps.
The Output unit shows loudness and true-peak readouts in dark wells, a
loudness bar with a Mastering Green band at -14 LUFS, a true-peak bar with a
Mastering Red line at -1 dBTP, and a Mastering Red OVER lamp in the place of
a pedal's lamp. Gain-reduction lamps on the Bus compressor and Limiter are
Mastering Amber. These four tokens are local to the board. Each dynamics
pedal ends its dials with Key, which reads Own or a track number; the Bus
compressor's five dials take three columns, and the Limiter's three take two
rows. The FX returns board's duckers end with the same Key dial. The strip below
the board, one 30-point row of chips in a dark well, repeats each pedal's
enamel in signal order, with a cream outline around the pedals in view.
Choosing a chip, or focusing any control above the strip, returns a scrolled
board to its top, where the units and the whole row show; focusing a chip
scrolls the board down to it. The whole pedal is painted: knobs sit directly
on the enamel with ink scale marks, ink labels, and an ink focus ring, and
values show in small dark windows with cream digits. The shared rotary draws
ink marks wherever a caller turns egui's `dark_mode` off for a light painted
surface; every other window keeps cream marks in dark bays. A lit pedal lamp
uses Accent lime with a soft glow; a pedal that is off keeps a dark lamp,
darkened knob caps, and a desaturated, darker enamel, and its chip in the
strip dims the same way. Ink on every enamel, lit or off, keeps at least 5:1
contrast. Patch cables carry a metal plug wherever they meet a socket or a
pedal.

Common controls use Face `#232624`, Cream `#f9f2dc`, Ink `#121412`, and pale
metal Primary `#d4cdb6`. Disabled faces use `#3d403c` with `#c9cbc0` lettering.
Palette accents identify focus, selection, and active indicators. A focus ring
that crosses the chassis needs 3:1 against it; Red's accent reaches only 2.6:1,
so its ring uses Cream. The default frame token uses Utility; tool bodies select
their named variant. Destructive buttons use Danger `#833627` with cream text.
Error text uses Coral in every window.

Mixer Metal and Mixer Ink support the ivory strips and their dark live text.
Mixer Cream supplies dark-control lettering. Mixer Face and Edge define native
value wells, buttons, and strip borders. Mixer Amber marks inverted polarity;
the named checked state remains available. The graphite surround and fine
ivory grain come from prepared material tiles. These tokens are local to Mixer.
