# Components: Inspector, overview, and score toolbar

## Inspector

The Inspector is a 280-point column at the right of the score, above the
lower panel, on Panel fill with 12-point and 10-point margins. A muted
Inspector label and a fold chevron are at its top. Below them, one page
scrolls. The selection chooses the page:

- Nothing selected: the song. The page shows the title in the heading face,
  the tempo and length, and two lists:
  - CATEGORIES: a swatch, the name, and the clip count of each category, in
    the order of the Sounds panel, with missing sounds last.
  - SECTIONS: a 3-point bar in the color of each section, its name, and its
    bars. A click on a section row scrolls the score to that section. It
    never moves the playhead.
- A track: a click on its name in the Arrange lane header selects it. The
  click also opens a folded Inspector, and the name then has Raised fill and a
  2-point accent edge. The page shows:
  - LEVEL: the fader, dB field, Unity, the pan slider, Center, and the pan.
  - OUTPUTS: Mute, Solo, Left, Right, polarity, and why the track is silent.
  - SENDS: the four send knobs.
  - CHANNEL PROCESSING: the EQ response curve, the EQ, Drive, Compressor, and
    Ducker switches, and the EQ button that opens the channel EQ window with
    every dial.
- One clip: its sound, category, bars, and track, then its start in bars and
  the fields of the clip menu: gain, trim, length, both fades with their
  curves, pitch, and Reverse.
- Several clips: their count, span, and tracks, and their category counts.

The controls are named "Inspector track ..." and "Inspector clip ...", so
that they never share a name with the Mixer, the Mix view, or the clip menu.
Every edit uses the same operations, so each edit is one undo step, and the
handles on the clips stay. The folded strip is 28 points wide, with a chevron
and INSPECTOR read top to bottom. A click anywhere on the strip opens the
Inspector. A maximised score hides both.

## Room for the panels

The Sounds panel and the Inspector share the width of the score. The Sounds
panel and the board of the lower panel share its height. Both side panels
open together when 520 points stay between them, that is, from 1134 points of
window width (1114 with the icon rail). The Sounds panel stays open above a
board when it keeps 330 points: its search, filters, chips, sort, and three
rows.

If a pair does not fit, the panel that opened most recently shows, and the
other panel folds to its strip, or the lower panel folds away. The saved
layout keeps each choice, so a larger window shows all the panels again. If a
window is too narrow for both side panels, it starts with the Inspector
folded until the user opens it. Thus at 960x720 and 1024x768, the Sounds list
gets the whole height, not one or two rows beside the Mixer.

## Song overview

A 60-point strip above the score toolbar shows the whole song, on Panel fill
with a 4-point radius. It spans the end of the song, its last section, and the
playhead, and at least the visible width of the score, so that a short or
empty song keeps its true size. Inside a 12-point inset, from left to right,
it shows:

- With Sections on, a 2-point line in the section color along the top of each
  section, and its name in uppercase 10-point text. The current section uses
  Text and the other sections use Muted. Every second section has a shade of
  Canvas at 55%.
- One thin line per clip in its category color, one row for each shown track,
  from the same ticks as the clip rectangles of the score. A missing sound
  draws in Raised. A muted or solo-silenced track mixes 30% of its color into
  Canvas. The lines are one mesh. The mesh is built again only when the song,
  catalog, theme, strip, or span changes.
- The loop, as a 2-point accent line along the bottom.
- The visible window of the score, as a frame: Text at 8% fill and an 85%
  1-point stroke, inside the strip.
- The playhead, as a 1.5-point accent line.

A click or drag centers the score view on that point. With the strip focused,
Left and Right move it by one screen, and Home and End go to the start and the
end. The overview never edits the song or moves the playhead.

A 22-point hide button with an up chevron sits over the lower right corner.
When folded, a 20-point bar with a down chevron and SONG OVERVIEW opens the
overview again from anywhere on the bar. An 8-point gap separates the strip or
the bar from the toolbar.

If the score has no room for the open strip together with the toolbar, the
ruler, and a 150-point lane viewport, the bar shows. The bar then reads
"Needs a taller score" and a click on it does nothing. The saved choice stays.
If the bar also cannot fit with that score area, the overview hides until
there is space again. The Overview status switch stays available. This rule
keeps the 150-point lane viewport and the full Mastering board from 1024x768.
The choice is saved in `layout.json`.

## Top bar

One 64-point top bar spans the window, with 18-point side margins and
8 points above and below its 48-point row. From the left, it holds:

1. the tribal mask,
2. the brand,
3. the song name,
4. the eight transport buttons (Loop, Go to start, Back one bar, Stop,
   Play / pause, Forward one bar, Go to end, Record),
5. the microphone and the recording-track selector,
6. the position readout,
7. the BPM label and tempo slider,
8. at the right end, Undo, Redo, and Help.

Tab follows the same order. Below 1240 points of content, the bar is compact:
tighter spacing, a smaller brand, no separator after the brand, and a
narrower readout, tempo, and action buttons. Thus every control stays on
screen at the minimum window size. Help is only in the top bar.

The top bar puts a 40-point tribal mask before the MixJam Techno brand. The
brand is MixJam over a muted Techno, at 20 and 11 points, or at 14 and
9 points in a compact bar. The artwork keeps its original colors and
transparent background. The native window, the taskbar, and the Windows
executable use the same preserved mask. `assets/app/` contains its PNG and
its ICO with several sizes. The viewport embeds the PNG, and `build.rs` links
the ICO into the executable.

Transparent PNG artwork supplies alpha masks. `src/ui_icons.rs` keeps their
alpha and normalizes RGB to white. The view applies text, cream, muted, or
accent tints. Textures are cached per egui context. Tooltips and
accessibility labels give the action names. The recording destination stays
a visible track number. The canonical PNG artwork and its provenance stay in
`assets/rail/`. SVG files and embedded SVG artwork are not allowed in tracked
or non-ignored project files. Ignored `local-data/` and `tmp/` files are
outside this check. Original preservation material stays in `local-data/`
under the layout of the owner.

Control icons use prepared 20-pixel PNGs, and the title mask uses a 40-pixel
PNG, at 100% Windows scaling, the only scale in scope. These are the only
prepared sizes. Each image texel maps to exactly one physical screen pixel,
with pixel-aligned bounds and no resize at runtime. The Windows ICO for the
shell icon is generated from the canonical PNG.

## Score toolbar and Maximise

A toolbar row is at the top of the arrangement. At the left, it holds the
Arrangement heading and its counts. At the right, it holds:

- the Arrange and Mix pair,
- the Automation lane menu,
- Follow,
- Sections,
- the Zoom slider (the mouse wheel over it zooms),
- the Maximise switch.

The pair is two selectable labels, 2 points apart, with the shown view
selected. If the score is narrower than 840 points, its smaller heading and
counts share a two-line block. The Automation menu then omits its separate
printed label and narrows to 96 points, and its Off choice reads No lane.
Below 600 points, the heading and
counts hide, so that every action stays within the score. If less than
700 points remain after the heading and counts, the row spacing tightens to
4 points and the side padding of its buttons to 6 points. The menu keeps its
accessible name. All actions stay on one row beside the Sounds panel, so that
the lane viewport keeps its height.

The Automation menu lists Off and all 14 lanes in three groups, under small
muted uppercase headings:

| Heading | Lanes |
| --- | --- |
| On every track | Volume, Pan, Filter, Pitch, Repeat, Repeat gate |
| Send amount, on every track | FX 1 to FX 4 send |
| Return level, one line across the song | FX 1 to FX 4 return |

Each row is 22 points tall and at least 300 points wide. It shows the lane
name in Text, then a muted detail, and at the right a muted count of what the
lane holds. The Off detail says that it edits clips. A send or return detail
names the effect of its bus, or is empty. The count is in tracks for a track
lane and in points for a return lane. The current lane uses the Selection fill
and a 3-point Accent bar at its left edge. The accessible name of each row is
only the lane name, with its selected state. The menu does not scroll.

Follow is on by default. Options can change its startup default without a
change to the toggle of the current session. Follow moves the horizontal
viewport when the playhead approaches an edge, including seeks and backward
loop jumps. It does not change vertical scrolling. While playback is paused,
a manual horizontal scroll stays where the user leaves it, until the position
or viewport geometry changes. To keep horizontal scrolling under manual
control during playback, switch Follow off. The toggle is view state for the
current session, not a project edit.

Maximise, or F, hides the tool rail, the Sounds panel, the Inspector, and the
lower panel, so that the score fills the window. The top bar, the song
overview, the score toolbar, and the status bar stay. F again, the Maximise
switch, or the Tools, Boards, Sounds, or Inspector switch restores the panels.
Overview only folds or opens the overview. Each panel gets back its own
choices unchanged. While the score is maximised, Tools, Boards, Sounds, and
Inspector read off. The state is saved in `layout.json`. A text field that has
focus keeps F for typing.
