# Colors

## Primary

Accent identifies Play/Pause, primary empty-state actions, the playhead,
enabled routing controls, and focus or active-state outlines. Accent is lime
in Studio and Aero, and amber in Vintage. Coral identifies recording, errors,
missing sources, and selected synth notes.

## Secondary

The Loop, Drum, Bass, Sequence, Sphere/Wave, Effect, Voice, and Other Sample
colors identify sounds. Sound rows use a 20% overlay of the category color.
Arrangement clips use the full category fill with dark text (the Canvas color
in Studio).
Selection adds a clip border in the Text color.

**The Shared Category Rule.** Use the same category color for a sound in the
Sounds panel and the arrangement. Pair the color with its name or category
label.

## Neutral

Canvas is the deepest structural surface. Panel is the surface of the top
bar, the side panels, the lower panel, and the overview. Floating tools use
their Folded Metal palette. Raised and Hover mark controls and interaction
states. Text is for primary labels. Muted is for counts, metadata, and status.
Line separates surfaces. Selection and Warning keep their native widget roles.
Drag Label fills the label that follows the pointer while a sound or clip
moves. It has no other use.

## Folded Metal

The hex values in this page are Vintage's. `src/ui/theme.rs` holds the Studio
and Aero values, and the Mastering enamels are the base colors that Studio and
Aero adjust.

The Folded Metal palettes share dark inset bays and controls, cream
lettering, polished silver knob caps, and pale metal primary buttons. Each
tool has its own chassis color:

| Palette | Window | Chassis | Active accent |
| --- | --- | --- | --- |
| Utility | Utility windows and dialogs | `#3c4140` | `#e79331` |
| Marine | Time Stretcher and FX returns board | `#20535b` | `#55aeb7` |
| Granite | Sample Editor | `#34393c` | `#adbabd` |
| Red | Effect Studio | `#9a2d27` | `#eb7468` |
| Olive | Step synth | `#3c4428` | `#9db25e` |
| Indigo | Mastering | `#2e3660` | `#96a6ec` |

Utility is neutral, with an orange-yellow accent. Use explicit names and
states together with color. Coral also identifies errors and destructive
actions. A palette change must not change control geometry or behavior. Keep
these colors local to the floating windows and the Mastering, FX returns, and
Step synth boards. The arrangement, Sounds panel, and Mixer keep their own
palettes.

Common controls use Face `#232624`, Cream `#f9f2dc`, Ink `#121412`, and pale
metal Primary `#d4cdb6`. Disabled faces use `#3d403c` with `#c9cbc0`
lettering. Palette accents identify focus, selection, and active indicators.
A focus ring that crosses the chassis needs 3:1 against it. In Vintage, the
Red accent gets only 2.6:1, so the Red ring uses Cream. The default frame
token uses Utility, and each tool body selects its named variant. Destructive
buttons use Danger `#833627` with cream text. Error text uses Coral in every
window.

## Mastering board

The Mastering board fills the lower panel under the header row. It is an
Indigo Folded Metal surface with a 2-point inset. Its pedals are painted
enclosures on the Indigo board. Each pedal has its own enamel and ink
lettering, as a stompbox has.

Light falls on the pedals from the upper left, at 135 degrees, as it does on
the knob caps. Each enclosure has:

- a 6-point rounded bevel, lit along its upper and left edges and in shade
  along its lower and right edges,
- 4-point side walls in shade, below and to the right of the face,
- a faint sheen across the face, toward the light,
- a soft drop shadow down and to the right.

Text stays inside the bevel. A pedal that is on shows anodised aluminium under
its enamel:

- fine brushed grain along the rows, at one texel per screen pixel,
- a sheen across the grain: a bright streak toward the light, a darker
  reflection, and a fainter streak,
- seeded hairline scratches where bare aluminium shows.

Each pedal keeps its own scratches, which stay below its printed name. The
finish darkens the enamel by a maximum of 5% for the grain and 8% for the
sheen, and ink keeps 5:1 there. A pedal that is off stays matte. The pedals of
a family share hues. The enamel always shows together with the printed name of
the pedal:

| Family | Pedals and enamels |
| --- | --- |
| Equaliser (golds) | Booster EQ `#deac40`, Trim EQ `#c4cc5c`, Lift EQ `#ecd06c` |
| Dynamics (blues and greens) | Booster compressor `#54b8a6`, Bus compressor `#78b2e4`, Maximizer `#82ce96`, Multiband `#64c4cc`, Limiter rose `#ee8ca0` |
| Saturation (warm oranges) | Soft clip `#f09678`, Tube `#f0a650`, Tape `#ceb48c` |
| Stereo (violets) | Booster wide `#c896dc`, Stereo imaging `#a6a0e8` |

The Booster pedals print short names (EQ, Compressor, Stereo wide) over a
"Booster" line. The mastering pedals print their names over their family. A
name that is too long for its pedal shrinks, down to 11 points, until it
clears the lamp. The rose of the Limiter marks it as the fixed last stage of
the board. It has no drag grip or move buttons, and its header line says that
it always runs last.

The Input and Output units are at the ends of the board. They are dark inset
bays with cream lettering, as in the other tools. The VU meter has a
Mastering Ivory face, an ink scale, a red zone above 0 VU, and Mastering Red
peak lamps. The Output unit shows:

- loudness and true-peak readouts in dark wells,
- a loudness bar with a Mastering Green band at -14 LUFS,
- a true-peak bar with a Mastering Red line at -1 dBTP,
- a Mastering Red OVER lamp in the position of the lamp of a pedal.

The gain-reduction lamps on the Bus compressor and Limiter, and the L, M, and
H reduction bars on the Multiband, are Mastering Amber. These four tokens are
local to the board.

Each dynamics pedal ends its dials with Key, which reads Own or a track
number. The five dials of the Bus compressor take three columns, and the three
dials of the Limiter take two rows. Each ducker of the FX returns board has the
same Key dial as its last dial.

The strip below the board is one 30-point row of chips in a dark well. It
shows the enamel of each pedal again, in signal order, with a cream outline
around the pedals in view. A click on a chip returns a scrolled board to its
top. Focus on a control above the strip does the same. At the top, the units
and the whole row show. Focus on a chip scrolls the board down to it.

The application paints the whole pedal. Knobs sit directly on the enamel, with
ink scale marks, ink labels, and an ink focus ring. Values show in small dark
windows with cream digits. The shared rotary draws ink marks where a caller
sets the egui `dark_mode` off for a light painted surface. All other windows
keep cream marks in dark bays.

A lit pedal lamp uses Accent with a soft glow: lime in Studio and Aero, amber
in Vintage. A pedal that is off keeps a dark lamp, darker knob caps, and a
darker enamel with less saturation. Its chip in the strip dims in the same
way. Ink on every enamel, lit or off, keeps a minimum of 5:1 contrast. Patch
cables have a metal plug at each socket or pedal.

## Mixer

Mixer Metal and Mixer Ink are for the ivory strips and their dark live text.
Mixer Cream is the lettering of the dark controls. Mixer Face and Mixer Edge
define the native value wells, buttons, and strip borders. Mixer Amber marks
inverted polarity, and the named checked state stays available. In Vintage,
prepared material tiles give the graphite surround and the fine ivory grain.
These tokens are the Vintage values and are local to the Mixer. Studio and Aero
paint the strips with their own `strip` roles in `src/ui/theme.rs`.
