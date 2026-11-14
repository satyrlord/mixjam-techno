# Components: actions, rail, and Sounds panel

## Actions and transport

Workspace primary action buttons use Accent with Canvas text. Secondary buttons use Raised
with Text. Undo and Redo use transparent PNG arrow icons from the shared artwork
cache, with action names in tooltips and accessibility labels.
Help uses its own ringed question-mark PNG with Help as its accessibility
label. Tooltips name each action with its window or shortcut; accessible names
stay short.
Export WAV is available through the Export icon in the left-hand tool rail.
Play/Pause stays prominent in the transport; other transport controls
use raised surfaces and hover fills. Record uses a coral circle with a white ring
during capture. Symbols are painted native geometry with tooltips and accessible
action names.

## Library loading

Library loading uses cream status text above a recessed dark progress rail.
An amber segment moves while listing files; after the total is known, the fill
shows the completed fraction. Keep Cancel beside it and fit the rail to the
available window width. Text never overlaps the changing fill.

## Tool rail

The rail at the left uses the utility Folded Metal finish: dark neutral
paint, subtle monochromatic grain, smooth folded edges, and shared dark button
faces. Orange marks active controls and focus. It has two widths. With names
(the default) it is 84 points wide; each tool is a 72 by 42-point target with
its 20-point icon above an 11-point name, shrunk in half-point steps to 9 points
when a long name needs it. Icons only, it is 64 points wide with 48 by 30-point
targets. A double-chevron switch at the top changes the width; the Tools switch
in the status bar does the same.

Three groups run from top to bottom: File (New, Open, Save, Save as, Import,
Export), Clips (Erase, Delete), and Tools (Mixer, Mastering, FX returns,
Effects, Step synth, Edit, Stretch, Album Review); Effects opens Effect Studio, Edit
the Sample Editor, and Stretch the Time Stretcher. With names, each group
has an uppercase 9.5-point muted heading; icons only, recessed separators divide
them. Options and All commands sit at the foot of the rail, below a separator.
When the window is too short for every name, the rail scrolls under the mouse
wheel or to keep the keyboard-focused tool visible. The foot stays fixed.
All commands draws a magnifier with the painter (no icon asset) in the rail
icon's colors and target sizes. Material and icons retain their prepared physical
pixel density; the narrow chassis omits the windows' wide corner patches.

Musical tools toggle their open state. Erase toggles its editing
mode. Active toggles use a subtle accent outline and icon tint and expose their
selected state to accessibility. A collapsed window stays active until closed.
File commands and Delete remain one-shot actions.
Options uses a gear PNG icon at the foot of the rail. Its second activation
focuses the existing window and retains its draft.

## Command list

One registry (`src/ui/commands.rs`) names every command, its group, its
shortcut and its action; the rail, the status bar's panel switches, the
keyboard and the command list read it. All commands or Ctrl+K opens the list
as a modal over a dimmed window, up to 580 points wide and centered. A Raised
field at the top holds the text cursor and the muted hint "Everything is listed
below. Type to narrow it." Below it, every command shows in its group before
anything is typed: File, Clips, Tools, Application, Transport, Edit, Panels,
Automation, each under the Automation menu's muted uppercase heading. A row is
30 points tall: a 6-point Accent dot when the tool, panel or lane is on, the
name in 13-point Text, and the shortcut in 11-point muted monospace at the
right. The current row has the Hover fill and a 3-point Accent bar; Up and Down
move it and scroll it into view. Typed words narrow the list to commands whose
name or group holds every word. A changed search returns to the top of the list.
Enter uses the current search, including text received in the same frame.
Enter or a click runs the command and closes
the list; Esc or a click outside closes it. When nothing matches, the list says
"Nothing matches. Clear the field to see the whole list again." Concept
entries for themes, sections, tracks and sounds stay out (decision M).
The search has the accessible name "Find a command". The modal blocks workspace
controls, arrangement requests, and song drops. A file or confirmation modal
dismisses the command list before it can consume more input.

## Status bar panel switches

The right end of the status bar holds a muted 12-point Panels label, five
switches, and the version link. Tools shows or hides the rail's names. Boards
folds the lower panel to its 28-point toolbar row (44 points with its
margins), which keeps the board
tabs; choosing a tab there, or the rail's Mastering or Step synth, opens
the requested board again. Repeating that rail action while open folds the
panel.
Sounds opens or folds the Sounds panel. Overview opens or folds the song
overview; it belongs to the score, so it keeps its state when the score is
maximised. Inspector opens or folds the Inspector. Boards, Sounds, and
Inspector show what is on screen, so a panel folded for room reads off and
opening it takes the room back. A switch that is on carries an accent
underline. The choices are saved in `layout.json` beside `options.json` and do not touch the Options
window or its revision. Without a saved layout, Tools, Boards, Overview, and
Inspector start on, and Sounds and Maximise off; a Starting module of Sounds
then opens the Sounds panel and folds the lower panel at each launch.

## Sounds panel

The Sounds panel lists the library beside the score. Its 250-point column holds,
top to bottom: the painted Sounds title with a muted "shown of total" count and
the fold chevron, the search field across the full width, the library and
category menus side by side, the category chips, and the sort menu with its
direction arrow. Chips are 22 points high: a 6-point square in the category's
color before a 12-point name, Panel fill with a Hover border, and Hover fill
with a Muted border when chosen. In a panel shorter than 440 points the chips
share one row that scrolls sideways, so the list keeps its room; otherwise they
wrap.

Each sound is a 44-point row with a 4-point gap and a 20% category tint: the name
on one line, the category and bar count below it at 90% Text (only the bar
count when space is short), and an 84 by 30-point waveform at the right. The
waveform shows the sound's lowest and highest sample per point in its category
color, scaled to the sound's own peak, over a centre line at 45%. A sound whose
waveform is still being read, or whose file cannot be read, shows the centre
line only. A click selects for placement, Enter places on the recording track at the
playhead, a double-click previews, and a drag carries the sound to a lane. The
list draws only the rows on screen, and only those rows ask for waveforms.

The panel is the library's only sound list. A launch whose Starting module is
Sounds opens it and folds the lower panel. The folded strip shows a chevron and
SOUNDS read top to bottom; the whole strip opens the panel. A maximised score
hides both.
