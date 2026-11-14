# Themes

The theme is an application preference on the Options Workspace page. Studio
is the default. Apply changes the theme at once, even during playback; it never
changes songs, history, audio, or renders. All three themes share one layout:
the same control positions, sizes, ranges, keyboard order, accessible names,
and features. A theme changes only color, material, silhouette, and type.
Fixed slots, rows, strips, boards, and window geometry are identical in every
theme. A control sized to its label, such as a toolbar button, follows the
label's width in the theme's text face: Fira Sans runs about 6% and Barlow
about 9% narrower than Inter, so such a control and the controls after it in
its row can sit a few points to the left.

`src/ui/theme.rs` owns one set of theme roles: workspace colors, chassis and
control colors, tool palettes, Mixer strip colors, lamps and meters, and grid
colors. It also owns each theme's material, its corner radii, and its fonts.
Renderers ask for a role and, where shape differs, branch on the material in
the shared painters (`controls/`, `mixer/`, `master/`, and the clip and
transport painters in `view/clip_paint.rs` and `view/header.rs`). Tool bodies
never name a theme. Interaction
code is the same for every material.

| Theme | Material | Workspace | Tools and boards | Accent |
| --- | --- | --- | --- | --- |
| Studio | Flat matte paint | Graphite `#15171a` / `#1c1f23` | Neutral panels tinted by tool, with the tool's accent along the top edge | Lime `#bee860` |
| Aero / Y2K | Molded plastic, softer gloss | Cobalt `#144084` and navy `#0b2450` | Colored plastic per tool: cobalt, teal, slate, red, green, indigo | Lime `#96ec5a` |
| Vintage | Worn painted metal | Warm dark `#181715` / `#1f1e1b` | The Folded Metal artwork, enamel pedals, and ivory Mixer strips | Amber `#f2aa3e` |

**Studio.** Flat fills with hairline edges, radii of half the shared values
up to 4 points, and no cast
shadow on menus or tooltips. Dials are flat: a dark face, a pointer, and a
270-degree value arc in the tool's accent (ink on light surfaces) in place of
tick marks. Faders have a flat 24 by 16-point cap. Pedals and FX units are flat
modules: each enamel settles 35% toward a matte light gray and keeps a hairline
ink edge. Clips are tight tabs.

**Aero / Y2K.** Every raised surface is molded plastic: a vertical shade from
6% lighter at the top to 10% darker at the foot, a quiet white highlight (alpha
10) across the upper 45%, and a restrained rim. These soften the owner's chosen
gloss further, so that text on plastic keeps its contrast; they are not the
superseded high-gloss study. Sound rows use a darker cobalt than other raised
surfaces, because they carry small text under a category tint.
`text_on_painted_surfaces_keeps_its_contrast` checks text against the lightest
and darkest points that each material paints. Radii are 1.6 times the shared values, up to 14
points. Knobs are colored molded caps in the tool's accent with tick marks.
Faders have a 24 by 24-point molded cap. Inset bays are luminous navy displays.
Clips are rounded bubbles, up to 8-point radius so their names clear the curve.
Unlit plastic stays 12% lighter than unlit enamel so ink keeps 5:1 at the foot.

**Vintage.** The original Folded Metal chassis, knob, and primary-button
artwork, the ivory and graphite Mixer artwork, and the enamel pedals with
brushed grain, sheen, and seeded scratches. Wear stays on chassis folds and
pedal faces; dense working areas (arrangement, Sounds panel, value wells) carry no
added wear. The position readout is an amber display window. Indicators are
warm amber.

**Category colors** for sounds and **section colors** are the same in every
theme, and states keep their non-color cues (outlines, bars, names, and
selected states) in every theme.

**Typography.** Every theme keeps Syncopate Bold for the MIXJAM TECHNO product
name. Headings and window titles use the theme's heading face in uppercase.
Their rows are pinned to 1.041 em, Syncopate's line box, which the shared
layout was measured with. Each face is raised by its measured offset, so its
capitals center in that row. Text faces have line boxes close to Inter's
1.21 em, so text-driven heights stay the same. Inter covers symbols that a
theme face lacks.

| Theme | Headings | Text |
| --- | --- | --- |
| Studio | IBM Plex Sans SemiBold, raised 0.155 em | Inter Medium |
| Aero | Michroma at 86%, raised 0.176 em | Fira Sans Medium |
| Vintage | Barlow Condensed SemiBold at 120%, raised 0.216 em | Barlow Medium, raised 0.05 em |

Every font file is unchanged from its pinned source, carries its license, and
permits installable embedding; `tools/assets/prepare-fonts.py` checks this. The
application embeds only the current theme's faces with the brand face and Inter.

**Code-only rendering.** Studio and Aero load no material bitmaps. All of their
surfaces, dials, caps, bevels, and highlights are bounded painter geometry
drawn each frame, with no texture creation and no continuous repaint. Changing
the theme rebuilds the style and fonts once. Do not generate images or bitmap
skins for a theme. Existing approved PNG brand, rail, and Vintage artwork stays.
