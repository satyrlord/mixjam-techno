# Colors

## Primary

Accent identifies Play/Pause, primary empty-state actions, the playhead,
enabled routing controls, and focus or active-state outlines. Accent is lime
in Studio and Aero, and warm modular orange `#f89e3e` in Vintage. Coral
identifies recording, errors, missing sources, and selected synth notes.

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
their palette from `controls::Palette`. Raised and Hover mark controls and
interaction states. Text is for primary labels. Muted is for counts, metadata,
and status. Line separates surfaces. Selection and Warning keep their native
widget roles. Drag Label fills the label that follows the pointer while a
sound or clip moves. It has no other use.

## Modular metal (`Vintage`)

The hex values below belong to Vintage. `src/ui/theme.rs` holds all Studio,
Aero, and Vintage tokens, and `src/ui/master/enclosure.rs` and `src/ui/fx.rs`
define the per-unit stompbox and rack motifs shared across all three themes.

The modular metal palettes pair olive-sage anodized panels with dark inset
bays, cream silk-screen lettering, fluted dark-skirted silver-cap rotary dials,
and cool metal primary buttons. Each tool has its own chassis color:

| Palette | Window | Chassis | Active accent |
| --- | --- | --- | --- |
| Utility | Utility windows and dialogs | `#444f46` | `#f89e3e` |
| Marine | Time Stretcher and FX returns rack | `#263e42` | `#68ced6` |
| Granite | Sample Editor | `#404844` | `#d8e0d6` |
| Red | Effect Studio | `#922c26` | `#ec786a` |
| Olive | Step synth | `#505e52` | `#f69a3a` |
| Indigo | Mastering rack well | `#1a1e1c` | `#f69a3a` |

Utility uses olive-sage grey with an orange signal accent. The Vintage Indigo
palette keeps its name but paints a near-black graphite well. Use explicit names
and states together with color. Coral marks errors and destructive actions. A
palette change never alters control geometry or behavior. Keep these tokens
local to the floating windows, boards, and rails; the arrangement and Sounds
panel keep their neutral workspace roles.

Common controls use Face `#181c1a`, Cream `#f8f4e8`, Ink `#0c0e0d`, and brushed
Primary `#daded6`. Disabled faces use `#2e3530` with `#c6ccc2` lettering.
Palette accents mark focus, selection, and active indicators. A focus ring that
crosses the chassis needs 3:1 against it; in Vintage Red, the ring uses Cream.
Destructive buttons use Danger `#8a3024` with cream text. Error text uses Coral
in every window.

## Mastering board and FX rack

The Mastering board fills the lower panel on an Indigo rack bay, while the FX
returns board sits on a Marine rack bay. In all three themes, every Mastering
pedal and FX card renders a procedural per-unit `MotifSpec`: a base and a
secondary color, a trim ink, a face polarity, one of 16 `MotifStyle` decks, and
a scoped `KnobStyle`. Active units show bold colors, and bypassed units show
muted tones that keep their hue. `MASTER_MOTIFS` in
`src/ui/master/enclosure.rs` and `fx_motif` in `src/ui/fx.rs` hold the values:

| Unit | Face | Colors | Motif | Knob |
| --- | --- | --- | --- | --- |
| Booster EQ | Light | Gold | `GraphicGrid` | `Default` |
| Booster compressor | Light | Teal | `SteppedFrame` | `Anodized` |
| Booster wide | Dark | Black with wood cheeks | `WoodCheeks` | `DarkPhenolic` |
| Soft clip | Light | Amber | `DiagonalStripe` | `Anodized` |
| Tube | Dark | Deep crimson | `TubeVents` | `DarkPhenolic` |
| Trim EQ | Light | Brushed aluminum | `AluminumCurve` | `BrushedChrome` |
| Bus compressor | Dark | Gunmetal | `RackMeterDeck` | `BrushedChrome` |
| Maximizer | Light | Green | `RadialSunburst` | `Anodized` |
| Lift EQ | Light | Light sky blue | `AtmosphericGradient` | `BrushedChrome` |
| Tape | Dark | Grey-green | `TapeDeckInset` | `DarkPhenolic` |
| Stereo imaging | Dark | Black and silver | `PhaseBezel` | `BrushedChrome` |
| Multiband | Dark | Navy and cyan | `TriBandWells` | `Anodized` |
| Limiter | Dark | Plum and magenta | `SteppedFrame` | `DarkPhenolic` |
| Echoform | Dark | Navy and cyan | `DualSubBays` | `Anodized` |
| Aetherform | Dark | Indigo and violet | `CosmicNebula` | `BrushedChrome` |
| Emberform | Dark | Crimson and amber | `DriveRail` | `Anodized` |
| Empty FX slot | Dark | Charcoal | `EmptyBay` | `Default` |

Vintage cards add a 6-point lit bevel, 4-point shade walls, corner slotted
screws, brushed grain, sheen, and seeded scratches when active. Studio cards
render crisp matte color blocks, and Aero renders glossy molded shells. Silk
screen inks and dial legends follow per-unit `light_face` polarity (`>= 5:1`
minimum contrast across both motif zones in lit and bypassed states).

## Mixer

Mixer Metal and Mixer Ink are for the ivory strips and their dark live text.
Mixer Cream is the lettering of the dark controls. Mixer Face and Mixer Edge
define the native value wells, buttons, and strip borders. Mixer Amber marks
inverted polarity, and the named checked state stays available. In Vintage,
prepared material tiles give the graphite surround and the fine ivory grain.
These tokens are the Vintage values and are local to the Mixer. Studio and Aero
paint the strips with their own `strip` roles in `src/ui/theme.rs`.
