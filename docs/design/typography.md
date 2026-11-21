# Typography

[Themes](themes.md#themes) names the heading and text faces of each theme.
Below, "heading face" and "text face" mean those of the current theme. The
sizes, uppercase titles, transliteration, and fit rules apply to every theme.
Syncopate Bold 700 is the face of the product name in every theme.

Show the branding and every project, section, window, modal, and empty-state
title in uppercase Latin text. Before the uppercase layout, change special
letters to their Latin equivalents: Ș/Ş to S, Ț/Ţ to T, Â/Ă to A, and Î to
I. Keep stored project titles and other user data in their original case.
Window IDs keep their identity, independent of the visible title format.

Every face is unchanged from its pinned source and includes ASCII. Every text
face also has the Romanian letters. Title letters use their Latin equivalents,
not a Romanian fallback face. Inter comes after the faces of each theme. It
supplies the symbols that they do not have. The bundled egui proportional
fonts come after Inter. Keep the license and provenance of every font with the
application. See `assets/fonts/README.md`.

## Text roles

- Display is the MixJam name. Techno follows it in 11-point Syncopate and
  Muted, or 9-point in a compact top bar.
- Headline is the standard native heading.
- Section labels identify Arrangement.
- The top row of the lower panel names the shown board in a 16-point heading.
- Title names the project.
- Body is for fields and explanatory text.

Body, Button, Clip, Small, and Metadata all use the text face. Track labels
and numeric controls also use it. Clip labels use the Clip size. They use the
Metadata size when their rectangle is less than 28 points high. Small and
Metadata are for compact controls and sample details. Status text uses the
text face at 12 points. These are local sizes in the same text family.

## Top bar text

The position readout shows the elapsed minutes and seconds (`MM : SS`) in the
text face, at 23 points in a fixed 112 by 42-point container. In a compact top
bar, it is 19 points in 76 by 40 points. It comes after Record and the
recording track, and before the tempo. In a wide top bar, the bar and beat
follow it in the 12-point muted text face. Its tooltip always adds them.
Changes to digit widths or song length must not move the adjacent controls.

The song name follows the brand in the top bar, before the transport, at the
Title size (15 points). A mark follows it when the song has unsaved
changes. Under a name that fits one line at that size, an 11-point muted line
gives the length of the song in bars and minutes. The line shows only if it
fits the room of the name. The name is never clipped, so it has no tooltip.

The slot of the name has the size of the name, and always keeps room for the
mark. Thus the first edit does not move the transport controls. A longer name
pushes the controls to the right. The controls after the name keep their
widths, and the name gets the remainder of the row. When the name does not
fit, it wraps between words onto two lines. Then it shrinks in 1-point steps
to a 9-point floor. At the floor, a very long name takes as many lines as the
48-point row holds. Only a name much longer than a real title goes below the
floor, and it can break words.

In a compact top bar (less than 1240 points of content), the eight transport
buttons narrow from 42 to 34 points. They narrow only when the name cannot
keep its full size. The top bar never removes or hides a transport control.
All of them stay on screen at the minimum window size.

## Windows

All floating windows use the same live uppercase title in the heading face of
the theme. They use the text face for body, button, value, and status text.
The shared shell uses 18-point titles, 13-point body and button text, and
11-point small text. Content can use the shared compact text roles where
necessary. Do not make typography for one tool only. Keep labels separate
from controls, and check the rendered glyph bounds. A resize never scales
text.

Center value text and button captions inside their own visible faces. Native
widget minimums must fit the readout rectangle. They must not move text below
the well. Check the visible glyph center at 100% display scaling.

## Mixer

Mixer uses the shared text-face sizes without a local title:

- Track names use Body.
- Gain values use Clip.
- Sound summaries and the pan or Muted text use Small.
- Rail marks and button lettering use Metadata.

These sizes follow the strip. The strip uses physical pixels.

## Values and clip names

Numeric format is only presentation. Rounded display values must not change
stored drafts. Keep live content selectable where possible, and independent of
the chassis bitmap.

**The Name Remains Rule.** Keep sample names visible on clips. Use one line
below 38 points of clip height, and up to two lines at larger heights.
Truncate long names, and keep the full name in a tooltip.
