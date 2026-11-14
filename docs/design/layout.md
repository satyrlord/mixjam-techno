# Layout

The native client starts at 1440 by 1000 logical points, with a minimum of 960 by
720. Panels are placed in this order, so each earlier one spans the space the
later ones share:

1. The 64-point top bar across the window (see Top bar).
2. The 30-point status bar across the bottom.
3. The tool rail at the left, below the top bar: 84 points wide with tool
   names, 64 points with icons only (see Tool rail).
4. The lower panel, from the rail's edge to the window's right edge, or its
   folded toolbar row.
5. The Sounds panel after the rail, above the lower panel: 250 points open, or
   a 28-point strip that names it when folded (see Sounds panel).
6. The Inspector at the right, above the lower panel: 280 points open, or a
   28-point strip (see Inspector).
7. The score in the rest, inside 18-point side and 14-point top and bottom margins: the
   60-point song overview or its 20-point bar, the score toolbar, a 12-point
   gap, the 26-point ruler, and the lanes; the Mix view adds its 70-point
   Master and returns block under the lanes after an 8-point gap.

The lower panel (Mixer, Mastering, FX returns, or Step synth) starts at 28% of
the content height, clamped to 220-290 points. Drag its top divider to resize
it. A centered grip marks the divider. The panel keeps its height for the
current session. Each board has a minimum panel height: Mixer 400 points,
Mastering 432, FX returns 462, and Step synth 580, so each shows its whole
board under the tab row. Opening a board raises a shorter panel to its minimum;
switching boards otherwise keeps the resized height. In a short window a board
yields down to 370 points so the lane viewport keeps 150 points, and the board
then scrolls (Mastering scrolls its chip strip below the pedals). When space
permits, the maximum panel height leaves 290 points for the score toolbar, the
ruler, and the lanes; the largest board minimum wins over that reserve at the
minimum client height. While the Sounds panel shows, the divider stops where
the Sounds panel keeps its 330 points, unless the board's minimum is larger. A 400-point Mixer panel gives the physical gain rail 96
points; the short panel can reduce it to 68 points. The draggable area extends
16 points past each end of the rail to include the fader cap.

The status bar's application version, prefixed with `v`, stays at the right
inner margin, with space reserved before status text is truncated. It uses the
12-point text face and the theme's hyperlink color. Hover and keyboard focus
show the link underline. The version opens the GitHub repository in the system
browser.

The lower panel has 18-point side margins, 10 points above the header row, and 6
below its content. Mixer, Mastering, FX returns, and Step synth boards start 8
points under the tab row, so a board never touches the tabs; the margins give up that
space, so the boards keep their height.

Tabs occupy the left of the lower panel's 28-point tab row, in the order
Mixer, Mastering, FX returns, and Step synth. Each tab shows its whole
board name in the 14-point body face; the selected tab has a raised fill, a
2-point accent underline, and the text color, and the others are muted. While
the panel is folded, the chosen tab keeps only its text color. The
tabs keep their position and identity for every board. A separator follows,
then the shown board's own controls. With Mastering,
the row holds the 150-point preset selector, the 112-point Bypass all switch,
the board's state in 12-point text that truncates, and the Mastering help
button at the right margin. With Mixer, it holds the Export tracks… button.
With Step synth, it holds the status line, Import details when the palette
sound's audio is missing, and the Step synth help button. The
preset list puts a rule after Subtle, Revive, and Loud, before the genres; the
selector reads Choose a preset while every pedal is off at its defaults, and
Custom when changed pedals match no preset. The trim, which belongs to the
mix, affects neither. The board bodies start below the tab row. The tool
rail's Mixer shortcut switches the score between its Arrange and Mix views.
The Mix view and the Mixer tab never show together: the Mix view moves a lower
panel on Mixer to Mastering, and the Mixer tab returns the score to Arrange.
A launch whose Starting module is Mixer opens the Mix view over Mastering; one
whose Starting module is Mastering or FX returns opens the lower panel on that
tab. The Mix view is session state, not part of `layout.json`. Its Mastering
and Step synth buttons open their tab and fold the lower panel when that board
already shows.

The arrangement shows occupied tracks and one drop target for a new track, while
the model retains 64 slots. A soloed track counts as occupied even without
clips, in the arrangement and the Mixer, so its Solo can be turned off.
Original track numbers remain stable. Rows are
20, 30, or 40 points high, as the Lane height option sets, at every window and
panel size, so more room shows more rows, and they scroll vertically. The new
track's drop hint stays visible at each height. Compact reversed clips leave
space between their name and the `rev` mark. The lane header is 250 points wide
and holds, from left, the track number, the lane's name, M, S, L, R, and a
short horizontal level fader. The song stores no lane names: a lane is named
after the sound family of its first clip (the sound's name without its trailing
number), or "Track N" while it has no clips. The name is elided to its span and
uses 11-point text in 20-point lanes, 12-point otherwise. The fader edits the
same track gain as the Mixer, with the same taper, keys, and double-click to
unity; a Canvas slot, a fill to the cap, a unity tick, and a Cap-colored cap.
A lane that Mute or another track's Solo silences is dimmed, and its track
number and name use Muted. The
fixed ruler is 26 points high. Bars use 62 points multiplied by zoom. The
timeline scrolls horizontally without moving track labels.

Follow is enabled by default in the score toolbar. Options can change its startup
default without changing the current session's toggle. It moves the horizontal viewport
when the playhead approaches an edge, including seeks and backward loop jumps.
It leaves vertical scrolling unchanged. While paused, manual horizontal scroll
stays where the user leaves it until the position or viewport geometry changes.
Switch Follow off to keep horizontal scrolling under manual control during
playback. The toggle is view state for the current session, not a project edit.

The Sounds panel's sort menu orders the filtered sounds by library order, name, length, BPM, or
category. The arrow beside it reverses the sort direction and exposes the
current direction as an accessible value. Equal sort keys retain library order
in either direction. Sort order and direction are view state for the current
session.

Mixer strips appear only for occupied tracks. Track and Master strips are
108 points wide at the supported prepared scales. Master stays outside the
horizontal track scroller, whose scrollbar remains visible. Track numbers,
sound summaries, dB gain values, pan descriptions, and output state stay
grouped within each strip. A track with
several distinct sounds shows their count and lists their names on hover.
Narrow strips preserve visible tracks without enlarging every control.

The Sample libraries page in Options scrolls vertically inside the shared
window. A large expanded Folder tempos list must leave the provider
controls reachable.

All in-app tools, utility windows, and blocking dialogs use one resizable shell.
Initial dimensions follow the content; they are not fixed raster dimensions.
Drag any edge or corner to resize. Reflow or scroll the body without scaling its
controls. A body that overflows shows a solid scroll bar with a cream handle, so
hidden content is visible as such. Source names, action labels, and long feedback must remain reachable at
the minimum supported viewport. Keep a common title bar, margins, gaps, and action
hierarchy. Specialized bodies such as the note grid keep their domain layout.

The only window states are open and collapsed. Collapse leaves a complete header
at the same position, with the same title and applicable controls. Expansion
restores the previous dimensions and draft. Keep Help available while collapsed
where a Help action exists. A collapsed blocking dialog releases workspace input
and pauses its pending action; expanding restores its decision workflow. Do not
replace a paused request when another file action is requested.

Fixed raster detail fades from opaque texture to transparency over the matching
solid chassis color. Resizing grows the solid regions instead of stretching bitmap
edges or repeating a visible texture seam. Prepare no PNG sizes beyond the
existing 100 set; paint one texel per physical screen pixel. Keep labels,
control values, indicators, and interaction states live.

Painted backgrounds use restrained, static monochromatic Gaussian grain. Its
mean is zero and its standard deviation is about 1.5 RGB levels. Apply the same
offset to all channels to retain each module's color. Align the prepared grain
to physical pixels and fade it before the perimeter. Do not animate the noise or
add grain over labels and control values.

This is a resizable Windows desktop surface at 100% display scaling. Display
scaling of 150% and 200% is out of scope, and no PNG sets are prepared for
it. No browser, mobile, or touch acceptance is implied by these
logical dimensions. Native capture cases
are defined by `tools/verify-ui.ps1`; execution results belong in evidence reports.
