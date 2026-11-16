# Layout

The native client starts at 1440 by 1000 logical points. Its minimum size is
960 by 720. The panels are placed in this order, so each earlier panel spans
the space that the later panels share:

1. The 64-point top bar across the window (see
   [Top bar](components-score.md#top-bar)).
2. The 30-point status bar across the bottom.
3. The tool rail at the left, below the top bar: 84 points wide with tool
   names, 64 points with icons only (see
   [Tool rail](components-shell.md#tool-rail)).
4. The lower panel, from the edge of the rail to the right edge of the window,
   when it is open.
5. The Sounds panel after the rail, above the lower panel: 250 points when
   open, or a 28-point strip that names it when folded (see
   [Sounds panel](components-shell.md#sounds-panel)).
6. The Inspector at the right, above the lower panel: 280 points when open,
   or a 28-point strip (see [Inspector](components-score.md#inspector)).
7. The score in the remaining space, inside 18-point side margins and 14-point
   top and bottom margins. From the top, the score holds:
   - the 60-point song overview or its 20-point bar,
   - the score toolbar,
   - a 12-point gap,
   - the 26-point ruler,
   - the lanes.

   The Mix view adds its 70-point block of Master and return rows under the
   lanes, after an 8-point gap. Its lanes end at the last row, so the block
   follows a short song's rows.

A scroll area that has more content than space shows its scroll bar at all
times, also when the pointer is not on it. Thus a short window shows where
there is more content. The shared bars float over the content, 4 points wide,
and take no layout width. The Mixer strips, the Step synth, and the
[windows](#windows) keep their own solid bars.

## Lower panel

The lower panel (Mixer, Mastering, FX returns, or Step synth) starts at 28% of
the content height, clamped to 220-290 points. Drag its top divider to resize
it. A centered grip marks the divider. The panel keeps its height for the
current session.

Each board has a minimum panel height, so that it shows its whole board under
the top row:

| Board | Minimum height |
| --- | --- |
| Mixer | 400 points |
| Mastering | 432 points |
| FX returns | 462 points |
| Step synth | 580 points |

When a board opens, a shorter panel grows to that minimum. A change to a
different board otherwise keeps the resized height. In a short window, a board
gives up height down to 370 points, so that the lane viewport keeps
150 points. The board then scrolls. Mastering scrolls its chip strip below
the pedals. In the Mix view, the 70-point Master and returns rows stay below
the lanes. The board then gives up 70 more points, down to 300 points, so that
the lane viewport keeps its 150 points above those rows.

When there is space, the maximum panel height leaves 290 points for the score
toolbar, the ruler, and the lanes. At the minimum client height, the largest
board minimum wins over that reserve. While the Sounds panel shows, the
divider stops where the Sounds panel keeps its 330 points, unless the minimum
of the board is larger. A 400-point Mixer panel gives the physical gain rail
96 points. A short panel can reduce the rail to 68 points. The drag area
extends 16 points past each end of the rail, to include the fader cap.

The lower panel has 18-point side margins, 10 points above the top row, and
6 points below its content. Each board starts 8 points under the top row, so
that a board never touches its controls. The margins give up that space, so
the boards keep their height.

The 28-point top row of the lower panel starts with the name of the shown
board as a 16-point heading, 6 points in from the margin. A separator follows,
then the controls of that board:

- Mastering: the 150-point preset selector, the 112-point Bypass all switch,
  the state of the board in 12-point text that truncates, and the Mastering
  help button at the right margin.
- Mixer: the Export tracks… button.
- Step synth: Notes and Effects, the undo and redo keys, Import details when
  the audio of the palette sound is missing, and the Step synth help button.

The preset list puts a rule after Subtle, Revive, and Loud, before the genre
presets. The list shows 560 points of its rows and scrolls to the others. The
selector reads Choose a preset while every pedal is off at its defaults. It
reads Custom when the changed pedals match no preset. The trim belongs to the
mix and changes neither reading. The board bodies start below the top row.

The Mixer, Mastering, FX returns, and Step synth buttons of the tool rail
choose the board. Each of these buttons shows its board, and folds the lower
panel when that board already shows.

The Arrange and Mix switch of the score toolbar, or the Mix view command,
changes the view of the score. The Mix view and the Mixer board never show
together. The Mix view moves a lower panel on Mixer to Mastering, and the
Mixer button of the rail returns the score to Arrange. The Mix view is its
own mode: choosing it folds the lower panel, and leaving it opens the panel
again unless the panel was opened or folded in between. If the Starting
module is Mixer, the application starts with the Mix view, the lower panel
folded on Mastering. If the
Starting module is Mastering or FX returns, the application starts with the
lower panel on that board. The Mix view is session state, not part of
`layout.json`.

## Status bar

The application version, with the prefix `v`, stays at the right inner margin
of the status bar. Space is kept for it before the status text truncates. It
uses the 12-point text face in Muted, so that only the panel switches that are
on show the accent. A 12-point vertical rule in the Line color separates it
from the panel switches. Hover and keyboard focus change it to the accent,
which is the hyperlink color of each theme, and show the link underline. The
version opens the GitHub repository in the system browser.

## Arrangement

The arrangement shows the occupied tracks and one drop target for a new
track. The model keeps 64 slots. A soloed track counts as occupied even
without clips, in the arrangement and the Mixer, so that its Solo can be
turned off. The original track numbers stay stable.

Rows are 20, 30, or 40 points high, as the Lane height option sets, at every
window and panel size. More room shows more rows, and the rows scroll
vertically. The drop hint for the new track stays visible at each height.
In narrow lanes it shortens to "Drop a sound for a new track", then "Drop a
sound here", and it hides where even that would be cut.
Compact reversed clips keep space between their name and the `rev` mark.

The lane header is 250 points wide. From the left, it holds the track number,
the name of the lane, M, S, L, R, and a short horizontal level fader. The song
stores no lane names. A lane takes the sound family of its first clip as its
name: the name of the sound without its trailing number. A lane without clips
is named "Track N". The name is elided to its span. It uses 11-point text in
20-point lanes, and 12-point text in other lanes.

The lane fader edits the same track gain as the Mixer, with the same taper,
keys, and double-click to unity. It has a Canvas slot, a fill to the cap, a
unity tick, and a Cap-colored cap. If Mute or the Solo of another track
silences a lane, the lane is dimmed, and its track number and name use Muted.

The fixed ruler is 26 points high. A bar is 62 points multiplied by the zoom.
The timeline scrolls horizontally without moving the track labels.

## Windows

All in-app tools, utility windows, and blocking dialogs use one resizable
shell. The initial dimensions follow the content. They are not fixed raster
dimensions. Drag any edge or corner to resize a window. Reflow or scroll the
body without scaling its controls. A body that overflows shows a solid scroll
bar with a cream handle, so that hidden content is visible as such. Source
names, action labels, and long feedback must stay reachable at the minimum
supported viewport. Keep a common title bar, margins, gaps, and action
hierarchy. Specialized bodies such as the note grid keep their domain layout.

The only window states are open and collapsed. A collapsed window keeps a
complete header at the same position, with the same title and applicable
controls. Expansion restores the previous dimensions and draft. Where a Help
action exists, keep it available while the window is collapsed. A collapsed
blocking dialog releases workspace input and pauses its pending action.
Expansion restores its decision workflow. Do not replace a paused request when
the user requests another file action.

## Material and grain

Fixed raster detail fades from opaque texture to transparency over the
matching solid chassis color. A resize grows the solid regions. It does not
stretch bitmap edges or repeat a visible texture seam. Keep labels, control
values, indicators, and interaction states live. For the prepared PNG sizes,
see [Top bar](components-score.md#top-bar).

Painted backgrounds use restrained, static, monochromatic Gaussian grain. Its
mean is zero and its standard deviation is about 1.5 RGB levels. Apply the
same offset to all channels, so that each module keeps its color. Align the
prepared grain to physical pixels and fade it before the perimeter. Do not
animate the noise or add grain over labels and control values.

## Scope

This is a resizable Windows desktop surface at 100% display scaling, the only
scaling in scope. These logical dimensions imply no browser, mobile, or touch
acceptance. `tools/verify-ui.ps1` defines the native capture cases. Execution
results belong in evidence reports.
