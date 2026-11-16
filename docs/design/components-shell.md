# Components: actions, rail, and Sounds panel

## Actions and transport

Workspace primary action buttons use Accent with Canvas text. Secondary
buttons use Raised with Text. Undo and Redo use transparent PNG arrow icons
from the shared artwork cache. Their tooltips and accessibility labels give
the action names. Help uses its own ringed question-mark PNG, with Help as its
accessibility label. Tooltips name each action with its window or shortcut.
Accessible names stay short.

The Export icon in the tool rail gives Export WAV. Play/Pause stays prominent
in the transport. The other transport controls use raised surfaces and hover
fills. Record uses a coral circle with a white ring during capture. The
symbols are painted native geometry with tooltips and accessible action names.

## Library loading

Library loading shows cream status text above a recessed dark progress rail.
While the files are listed, an amber segment moves. After the total is known,
the fill shows the completed fraction. Keep Cancel beside the rail, and fit
the rail to the available window width. The text never overlaps the changing
fill.

## Tool rail

The rail at the left uses the utility Folded Metal finish: dark neutral
paint, subtle monochromatic grain, and smooth folded edges. The workspace
accent of the theme marks active tools and focus: lime in Studio and Aero,
amber in Vintage.

The rail has two widths:

- With names (the default), it is 84 points wide. Each tool is a 68 by
  44-point target. Its 20-point icon has its center 15 points below the top,
  above a 10.5-point name that ends 5 points above the bottom.
- With icons only, it is 64 points wide, with 48 by 30-point targets.

Every name prints at the same size. If a name is too long for that size, the
rail shows a shorter name: Album Review shows "Review". The tooltip,
accessible name, and window title keep the full name. The tools stack with no
gap between them.

A double-chevron switch at the right of the first heading changes the width.
Without headings (icons only, or compact), the switch has its own row at the
top. The Tools switch in the status bar does the same.

The `rail_keys` role of the theme sets the tool faces. On Vintage, every tool
rests on a dark key face. On Studio and Aero, a tool is bare until hover or
press shows its face. An active tool keeps its face and adds a 2-point accent
bar at the left in every theme. A bare tool has the bar at the edge of the
target. A key has the bar 2 points inside its border. An accent outline means
keyboard focus only. The icon of an active tool also takes the accent, and its
name brightens to Text, as it does under the pointer.

Five groups run from top to bottom:

| Group | Tools |
| --- | --- |
| File | New, Open, Save, Save as, Import, Export |
| Clips | Erase, Delete |
| Mix | Mixer, Mastering, FX returns, Step synth: the boards of the lower panel |
| Sound | Edit, Stretch, Effects: the windows that write a new sample |
| Library | Album Review |

Edit opens the Sample Editor, Stretch opens the Time Stretcher, and Effects
opens Effect Studio. With names, each group has an uppercase 10-point muted
heading with 0.6-point letter spacing. A heading has 14 points of space above
it (4 above the first heading) and 6 points below it. With icons only,
recessed separators divide the groups. The rail holds only the tools. Options
and All commands start the status bar.

If the window is too short for every tool under its heading, the rail becomes
compact. The names stay, and the targets shrink to 68 by 36 points, with the
icon center 12 points below the top and the name 2 points above the bottom.
Separators replace the headings, each centered in a 12-point gap. This form
fits every tool in a 768-point window. In a shorter window, the rail scrolls
with the mouse wheel, or to keep the tool with keyboard focus visible. An
engraved line marks each edge past which more tools lie. The material and
icons paint one texel per physical pixel. The narrow chassis does not use the
wide corner patches of the windows.

The musical tools toggle their open state. Erase toggles its editing mode.
Active toggles expose their selected state to accessibility. A collapsed
window stays active until it closes. The file commands and Delete are one-shot
actions.

## Command list

One registry (`src/ui/commands.rs`) names every command, its group, its
shortcut, and its action. The rail, the panel switches of the status bar, the
keyboard, and the command list read it. All commands or Ctrl+K opens the list
as a modal over a dimmed window, up to 580 points wide and centered.

A Raised field at the top holds the text cursor and the muted hint
"Everything is listed below. Type to narrow it." Before anything is typed,
every command shows below the field, in its group. The groups are File, Clips,
Mix, Sound, Library, Application, Transport, Edit, Panels, and Automation.
Each group has the same muted uppercase heading as the Automation menu.

A row is 30 points tall. From the left, it holds:

- a 6-point Accent dot when the tool, panel, or lane is on,
- the name in 13-point Text,
- 8 points after the name, a one-line description of the command in 12-point
  Muted. An ellipsis cuts it before the shortcut. It is also the accessible
  description of the row.
- the shortcut in 11-point Muted text at the right.

The current row has the Hover fill and a 3-point Accent bar. Up and Down move
the current row and scroll it into view. Typed words narrow the list to the
commands whose name or group holds every word. A changed search goes back to
the top of the list. Enter uses the current search, including text that
arrives in the same frame. Enter or a click runs the command and closes the
list. Esc or a click outside closes the list. If nothing matches, the list
reads "Nothing matches. Clear the field to see the whole list again." The
list holds no concept entries for themes, sections, tracks, or sounds.

The search has the accessible name "Find a command". It keeps the text cursor
while the list shows. Tab and Shift+Tab do not move the cursor, because the
rows and the scroll parts of the list are not Tab stops. The modal blocks the
workspace controls, arrangement requests, and song drops. A file or
confirmation modal closes the command list before the list can take more
input.

## Status bar

The left end of the status bar holds Options and All commands. They are bare
28 by 24-point buttons, 10 points before the status text, with Muted icons
that brighten to Text under the pointer. Options is the gear PNG. Its icon
takes the accent while the window is open. A second activation focuses the
open window and keeps its draft. All commands draws a magnifier with the
painter (no icon asset), on the 24-unit grid of the rail icons with their
2-unit stroke.

The idle status is a usage hint in Muted: "Space plays the song. Drag a sound
from the Sounds panel onto the score. Ctrl+K lists every command." It
truncates, and its hover hint gives the full text. While the Step synth board
shows, its own hint takes the place of the idle text. Errors and application
notices replace it and stay visible. The Step synth board has no status line
of its own.

The right end of the status bar holds a PANELS heading in the style of the
rail group headings (10-point muted uppercase, 0.6-point tracking), five panel
switches, and the version link:

- Tools shows or hides the names in the rail.
- Boards folds the lower panel away and gives the score its height. Boards
  again, or a board button in the rail, opens the panel on that board. The
  same rail button folds the panel when its board already shows.
- Sounds opens or folds the Sounds panel.
- Overview opens or folds the song overview. The overview belongs to the
  score, so it keeps its state when the score is maximised.
- Inspector opens or folds the Inspector.

Boards, Sounds, and Inspector show what is on screen. If a panel is folded
for lack of room, its switch reads off, and a click opens it and takes the
room back. A switch that is on has an accent underline. The choices are saved
in `layout.json` beside `options.json`. They do not change the Options window
or its revision. Without a saved layout, Tools, Boards, Overview, and
Inspector start on, and Sounds and Maximise start off. If the Starting module
is Sounds, each launch then opens the Sounds panel and folds the lower panel.

## Sounds panel

The Sounds panel lists the library beside the score. From the top, its
250-point column holds:

1. The painted Sounds title, a muted count of the shown sounds of the total
   ("N of total"), and the fold chevron.
2. The search field across the full width.
3. The library and category menus, side by side. The library menu shows all
   its rows and does not scroll.
4. The category chips.
5. The sort menu with its direction arrow.

Chips are 22 points high: a 6-point square in the category color before a
12-point name. An unselected chip uses Panel fill, Muted text, and a Hover
border. The chosen chip uses Hover fill, Text, and a Muted border. In a panel
shorter than 440 points, the chips share one row that scrolls sideways, so
that the list keeps its room. Otherwise the chips wrap.

The sort menu orders the filtered sounds by library order, name, length, BPM,
or category. The arrow beside it reverses the sort direction and exposes the
current direction as an accessible value. Equal sort keys keep library order
in either direction. Sort order and direction are view state for the current
session.

Each sound is a 44-point row with a 4-point gap and a 20% category tint. The
whole row carries the tint, and there is no category dot. The row shows:

- the name, on one line,
- below it, the category and bar count at 90% Text. If space is short, only
  the bar count shows.
- at the right, an 84 by 30-point waveform.

Hover or selection uses the Hover fill. Selection or keyboard focus adds a
one-point border in the category color. One tooltip groups the full name,
product, category, code, sample IDs, source tempo, nominal bars, actual audio
duration, frames, rate, channels, and WAV path when available. Nominal bars
and audio duration stay separate facts.

The waveform shows the lowest and highest sample of the sound per point, in
its category color, scaled to the peak of the sound. It sits over a centre
line in the category color at 45% opacity. If the waveform is not read yet, or
the file cannot be read, the row shows only the centre line. The list draws
only the rows on screen, and only those rows request waveforms.

The list is one Tab stop. While it has focus, an Accent outline marks the
current sound. If the list shows no current sound, the outline marks the list
itself. Up and Down choose a sound, and Page Up, Page Down, Home, and
End move further. Enter places the current sound on the recording track at
the playhead. Delete removes the selected clips, as it does when no control
has focus. A click selects a sound for placement. A double-click previews it.
A drag carries the sound to a lane.

The panel is the only sound list of the library. If the Starting module is
Sounds, the application starts with the panel open and the lower panel folded.
The folded strip shows a chevron and SOUNDS, read top to bottom. A click
anywhere on the strip opens the panel. A maximised score hides both.
