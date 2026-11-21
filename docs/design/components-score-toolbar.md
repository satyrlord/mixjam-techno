# Score toolbar and Maximise

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
