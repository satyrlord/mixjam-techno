# Components: Inspector, overview, and score toolbar

## Inspector

The Inspector is a 280-point column at the right of the score, above the
lower panel, on Panel fill with 12 and 10-point margins. A muted Inspector
label and a fold chevron head it; below, one page that the selection chooses
scrolls:

- Nothing selected: the song. The title in the heading face, the tempo and
  length, CATEGORIES with a swatch, name, and clip count per category in the
  Sounds panel's order (missing sounds last), and SECTIONS with a 3-point bar
  in each section's color, its name, and its bars. A section row scrolls the
  score to that section; it never moves the playhead.
- A track, chosen by a click on its name in the Arrange lane header, which
  also opens a folded Inspector (the name then carries Raised fill and a 2-point accent edge): LEVEL with the fader,
  dB field, Unity, the pan slider, Center, and the pan; OUTPUTS with Mute,
  Solo, Left, Right, polarity, and why the track is silent; SENDS with the
  four send knobs; CHANNEL PROCESSING with the EQ response curve, the EQ,
  Drive, Compressor, and Ducker switches, and the EQ button that opens the
  channel EQ window with every dial.
- One clip: its sound, category, bars, and track, then its start in bars and
  the clip menu's fields: gain, trim, length, both fades with their curves,
  pitch, and Reverse.
- Several clips: their count, span, and tracks, and their category counts.

Its controls are named "Inspector track ..." and "Inspector clip ...", so
they never share a name with the Mixer, the Mix view, or the clip menu. Every
edit goes through the same operations, so each is one undo step, and the
on-clip handles stay. The folded strip is 28 points with a chevron and
INSPECTOR read top to bottom; the whole strip opens it. A maximised score
hides both.

## Room for the panels

The Sounds panel and the Inspector share the score's width; the Sounds panel
and the lower panel's board share its height. Both side panels open together
when 520 points stay between them, from 1134 points of window (1114 with the
icon rail).
The Sounds panel stays open above a board when it keeps 330 points: its search,
filters, chips, sort, and three rows. Where a pair does not fit, the panel
opened most recently shows and the other folds to its strip or the board's
toolbar row. The saved layout keeps each choice, so a larger window shows
them all again. A window too narrow for both starts with the Inspector folded
until it is opened. At 960x720 and 1024x768 the Sounds list therefore gets the
whole height instead of one or two rows beside the Mixer.

## Song overview

A 60-point strip above the score toolbar shows the whole song at a glance, on
Panel fill with a 4-point radius. It spans the song's end, its last section,
and the playhead, and at least the score's visible width, so a short or empty
song keeps its true size. Inside a 12-point inset, left to right:

- With Sections on, each section gets a 2-point line in its section color along
  the top and its name in uppercase 10-point text, the current section in Text
  and the others Muted; every second section is shaded with Canvas at 55%.
- One thin line per clip in its category color, a row per shown track, from
  the same ticks as the score's clip rectangles. A missing sound draws Raised;
  a muted or solo-silenced track mixes 30% of its color into Canvas. The lines
  are one mesh, rebuilt only when the song, catalog, theme, strip, or span
  changes.
- The loop as a 2-point accent line along the bottom.
- The score's visible window as a frame: Text at 8% fill and an 85% 1-point
  stroke, kept inside the strip.
- The playhead as a 1.5-point accent line.

A click or drag centres the score's view on that point; with the strip focused,
Left and Right move it by one screen and Home and End go to the start and the
end. It never edits the song or moves the playhead. A 22-point hide button with
an up chevron sits over the lower right corner. Folded, a 20-point bar with a
down chevron and SONG OVERVIEW opens it again from anywhere on it. An 8-point
gap separates either from the toolbar. Where the score has no room for the open
strip as well as the toolbar, the ruler, and a 150-point lane viewport, the bar shows instead,
reading "Needs a taller score" and doing nothing on a click, and the saved
choice stays. If even the bar cannot fit with that score area, the overview
hides until space returns. The Overview status switch stays available. This
preserves the 150-point lane viewport and the full Mastering board from
1024x768. The choice is saved in `layout.json`.

## Top bar

One 64-point top bar spans the window, with 18-point side margins and 8 above
and below its 48-point row. From the left: the tribal mask, the brand, the song
name, the eight transport buttons (Loop, Go to start, Back one bar, Stop,
Play / pause, Forward one bar, Go to end, Record), the microphone and the
recording-track selector, the position readout, the BPM label and tempo slider,
and, at the right end, Undo, Redo, and Help. Tab follows the same order. Below
1240 points of content the bar is compact: tighter spacing, a smaller brand, no
separator after it, and narrower readout, tempo, and action buttons, so every
control stays on screen at the minimum window size. Help lives only here.

## Score toolbar and Maximise

A toolbar row heads the arrangement: the Arrangement heading and its counts at
the left, and at the right the Arrange and Mix pair, the Automation lane menu,
Follow, Sections, the Zoom slider (the mouse wheel over it zooms), and the
Maximise switch. The pair is two selectable labels, 2 points apart, with the
shown view selected. When the score is narrower than 840 points, its smaller
heading and counts share a two-line block, and the Automation menu omits its
separate printed label and narrows to 96 points. Below 600 points, the heading
and counts hide so every action stays within the score. When less than 700 points remain after
the heading and counts, the row's
spacing tightens to 4 points and its buttons' side padding to 6. The menu keeps
its accessible name. All actions stay on one row beside the Sounds panel so
the lane viewport keeps its height.

The Automation menu lists Off and all 14 lanes in three groups under small
muted uppercase headings: On every track (Volume, Pan, Filter, Pitch, Repeat,
Repeat gate), Send amount, on every track (FX 1 to FX 4 send), and Return
level, one line across the song (FX 1 to FX 4 return). Each row is 22 points
tall and at least 300 points wide: the lane name in Text, then a muted detail
(Off says edit clips; a send or return names its bus's effect, or empty), and
at the right a muted count of what the lane holds, in tracks for a track lane
and in points for a return lane. The current lane uses the Selection fill and
a 3-point Accent bar at its left edge. Each row's accessible name is the lane
name alone, with its selected state. The menu does not scroll. Maximise, or
F, hides the tool rail, the Sounds panel, the Inspector, and the lower panel so the score
fills the window; the top bar, the song overview, the score toolbar, and the
status bar stay. F
again, the switch, or the Tools, Boards, Sounds, or Inspector switch restores
them (Overview only folds or opens the overview), and each panel's own
choices come back unchanged. While the score is maximised, Tools, Boards,
Sounds, and Inspector read off. The state is saved in `layout.json`. A text field that has focus
keeps F for typing.

The top bar places a 40-point tribal mask before the MixJam Techno brand:
MixJam over a muted Techno, at 20 and 11 points, or 14 and 9 in a compact bar.
The artwork retains its original colors and transparent background. The native
window, taskbar, and Windows executable use the same preserved mask.
`assets/app/` contains its PNG and multiple-size ICO. The viewport embeds
the PNG; `build.rs` links the ICO into the executable.

Transparent PNG artwork supplies alpha masks. `src/ui_icons.rs` retains
their alpha and normalizes RGB to white; the view applies text, cream, muted, or
accent tints. Textures are cached per egui context. Action names are available through
tooltips and accessibility labels. Recording destination remains a visible
track number. Canonical PNG artwork and provenance remain in `assets/rail/`.
SVG files and embedded SVG artwork are not allowed in tracked or non-ignored
project files. Ignored `local-data/` and `tmp/` files are outside this check.
Original preservation material stays in `local-data/` under the owner's layout.

Control icons use prepared 20-pixel PNGs and the title mask uses a 40-pixel PNG
at 100% Windows scaling. Three display variants cover 100%, 150%, and 200%:
20/30/40-pixel controls and 40/60/80-pixel title artwork.
Each image texel maps to exactly one physical screen pixel, with
pixel-aligned bounds and no runtime resizing. At other scale factors, select
the nearest prepared size; its logical size can differ slightly. Reload cached
textures when the required density changes. Windows ICO packaging is generated
from the canonical PNG for the shell icon.
