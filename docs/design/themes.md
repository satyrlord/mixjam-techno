# Themes

The theme is an application preference on the Options Workspace page. Studio
is the default. Apply changes the theme at once, also during playback. The
theme never changes songs, history, audio, or renders.

All three themes share one layout: the same control positions, sizes, ranges,
keyboard order, accessible names, and features. A theme changes only color,
material, silhouette, and type. Fixed slots, rows, strips, boards, and window
geometry are the same in every theme. A control that has the size of its
label, such as a toolbar button, follows the width of the label in the text
face of the theme. Fira Sans is approximately 6% narrower than Inter, and
Barlow approximately 9%. Thus such a control, and the controls after it in its
row, can move a few points to the left.

`src/ui/theme.rs` owns one set of theme roles: workspace colors, chassis and
control colors, tool palettes, Mixer strip colors, lamps and meters, and grid
colors. It also owns the material, the corner radii, and the fonts of each
theme. Renderers ask for a role. Where the shape is different, they branch on
the material in the shared painters: `controls/`, `mixer/`, `master/`, and the
clip and transport painters in `view/clip_paint.rs` and `view/header.rs`. Tool
bodies never name a theme. The interaction code is the same for every
material.

| Theme | Material | Workspace | Tools and boards | Accent |
| --- | --- | --- | --- | --- |
| Studio | Flat matte paint | Graphite `#15171a` / `#1c1f23` | Neutral panels with a tool tint, and the tool accent along the top edge | Lime `#bee860` |
| Aero | Y2K molded plastic, softer gloss | Cobalt `#144084` and navy `#0b2450` | Colored plastic per tool: cobalt, teal, slate, red, green, indigo | Lime `#96ec5a` |
| Vintage | Worn painted metal | Warm dark `#181715` / `#1f1e1b` | The Folded Metal artwork, enamel pedals, and ivory Mixer strips | Amber `#f2aa3e` |

**Studio.** Flat fills with hairline edges, and no cast shadow on menus or
tooltips. Radii are half the shared values, up to 4 points. Dials are flat: a
dark face, a pointer, and a 270-degree value arc in the tool accent (ink on
light surfaces) in place of tick marks. Faders have a flat 24 by 16-point cap.
Pedals and FX units are flat modules. Each enamel moves 35% toward a matte
light gray and keeps a hairline ink edge. Clips are tight tabs.

**Aero.** Every raised surface is molded plastic:

- a vertical shade, from 6% lighter at the top to 10% darker at the foot,
- a quiet white highlight (alpha 10) across the upper 45%,
- a restrained rim.

These keep the gloss soft, so that text on plastic keeps its contrast. Do not
use a high-gloss finish. Sound rows use a darker cobalt than other raised
surfaces, because they have small text under a category tint. The test
`text_on_painted_surfaces_keeps_its_contrast` checks text against the lightest
and darkest points that each material paints. Radii are 1.6 times the shared
values, up to 14 points. Knobs are colored molded caps in the tool accent,
with tick marks. Faders have a 24 by 24-point molded cap. Inset bays are
luminous navy displays. Clips are rounded bubbles, with a maximum radius of
8 points, so that their names clear the curve. Unlit plastic stays 12% lighter
than unlit enamel, so that ink keeps 5:1 at the foot.

**Vintage.** The Folded Metal chassis, knob, and primary-button artwork, the
ivory and graphite Mixer artwork, and the enamel pedals with brushed grain,
sheen, and seeded scratches. Wear stays on chassis folds and pedal faces.
Dense working areas (the arrangement, the Sounds panel, and value wells) get
no added wear. The position readout is an amber display window. Indicators
are warm amber.

**Category colors** for sounds and **section colors** are the same in every
theme. States keep their cues that are not color (outlines, bars, names, and
selected states) in every theme.

**Typography.** Every theme keeps Syncopate Bold for the MIXJAM TECHNO
product name. Headings and window titles use the heading face of the theme,
in uppercase. Their rows have a fixed height of 1.041 em, the line box of
Syncopate, with which the shared layout was measured. Each face moves up by
its measured offset, so that its capitals are centered in that row. Text faces
have line boxes near the 1.21 em of Inter, so heights that follow the text
stay the same. Inter supplies symbols that a theme face does not have.

| Theme | Headings | Text |
| --- | --- | --- |
| Studio | IBM Plex Sans SemiBold, raised 0.155 em | Inter Medium |
| Aero | Michroma at 86%, raised 0.176 em | Fira Sans Medium |
| Vintage | Barlow Condensed SemiBold at 120%, raised 0.216 em | Barlow Medium, raised 0.05 em |

Every font file is unchanged from its pinned source, has its license, and
permits installable embedding. `tools/assets/prepare-fonts.py` checks this.
The executable embeds every face. The font setup loads only the faces of the
current theme, with the brand face and Inter.

**Code-only rendering.** Studio and Aero load no material bitmaps. All of
their surfaces, dials, caps, bevels, and highlights are bounded painter
geometry. They are drawn each frame, with no texture creation and no
continuous repaint. A theme change builds the style and fonts again one time.
Do not generate images or bitmap skins for a theme. The approved PNG brand,
rail, and Vintage artwork stays.
