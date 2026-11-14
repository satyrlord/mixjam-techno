# Typography

[Themes](themes.md#themes) names each theme's heading and text faces. Below, "heading
face" and "text face" mean the current theme's; the sizes, uppercase titles,
transliteration, and fitting rules apply to every theme. Syncopate Bold 700 is
the product name's face in every theme. Render branding and every
project, section, window, modal, and empty-state title in uppercase Latin text.
Transliterate special letters to their Latin counterparts before uppercase
layout, including Ș/Ş to S, Ț/Ţ to T, Â/Ă to A, and Î to I. Keep stored
project titles and other user data in their original case. Window IDs retain
their original identity independently of visible title formatting.

Every face is unchanged from its pinned source and includes ASCII; every text
face also covers the Romanian letters. Title letters use their Latin
counterparts instead of a Romanian fallback face. Inter follows each theme's
faces for symbols they lack, and egui's bundled proportional fonts follow it.
Keep every font's license and provenance with the application; see
`assets/fonts/README.md`.

Display is the MixJam name; Techno follows it in 14-point Syncopate and Muted.
Headline is the standard native heading. Section
labels identify Arrangement. Bottom Module supplies the lower panel's board
tabs their 14-point text-face names. Title names the project. Body
serves fields and explanatory text. Body, Button, Clip, Small, and Metadata all
use the text face. Track labels and numeric controls also use it. Clip labels use
the clip size, reduced to metadata size when their rectangle is below 28 points
high. Small and Metadata support compact controls and sample details.

The position readout shows elapsed minutes and seconds (`MM : SS`) in the text
face at 23 points in a fixed 112 by 42-point container, or at 19 points in 76
by 40 points in a compact top bar. It follows Record and the recording track
and comes before the tempo. In a wide top bar the bar and beat follow it in the
12-point muted text face; its tooltip always adds them. Changes to digit widths
or song length must not move the adjacent controls.

The song name follows the brand in the top bar and leads the transport at the
Headline size (15 points), followed by a mark when the song has unsaved changes.
Under a name that fits one line at that size, an 11-point muted line gives the
song's length in bars and minutes, where it fits the name's room. The name is never clipped, so it has
no tooltip. Its slot is sized to the name and always keeps room for the mark,
so the first edit does not move the transport controls; a longer name pushes
the controls to the right. The controls after the name keep their widths, and
the name takes the rest of the row. When it does not fit, it wraps between words
onto two lines, then shrinks in 1-point steps to a 9-point floor. At the floor,
a very long name takes as many lines as the 48-point row holds. Only an
absurdly long name, far longer than a real title, goes below the floor,
breaking words if it must. In a compact top bar (less than 1240 points of
content) the eight transport buttons narrow from 42 to 34 points when the name
would otherwise lose its full size. No transport control is dropped
or hidden, and all of them stay on screen at the minimum window size. Status text
uses the text face at 12 points. These are local sizes within the same text family.

All floating windows use the same live uppercase title in the theme's heading
face, and the theme's text face for body, button, value, and status text. The shared shell uses 18-point titles, 13-point
body and button text, and 11-point small text. Content can use the shared compact
text roles where needed, without inventing per-tool typography. Keep labels separate
from controls and check rendered glyph bounds. Resizing never scales text.
Center value text and button captions inside their own visible faces. Native
widget minimums must fit the readout rectangle; they must not shift text below
the well. Check the visible glyph center at 100% display scaling.

Mixer uses the shared text-face sizes without a local title. Track names use Body,
gain values use Clip, sound summaries and pan or Muted text use Small, and
rail marks and button lettering use Metadata. These sizes follow the strip,
which is measured in physical pixels.

Numeric formatting is presentation only. Rounded display values must not change
stored drafts. Keep live content selectable where supported and independent of the
chassis bitmap.

**The Name Remains Rule.** Keep sample names visible on clips. Use one line below
38 points of clip height and up to two lines at larger heights. Truncate long names and
retain the full name in a tooltip.
