# Components: command list and status bar

## Command list

One registry (`src/ui/commands.rs`) names every command, its group, its
shortcut, and its action. The rail, the panel switches of the status bar, the
keyboard, and the command list read it. All commands or Ctrl+K opens the list
as a modal over a dimmed window, up to 580 points wide and centered.

A Raised field at the top holds the text cursor and the muted hint "Everything
is listed below. Type to narrow it." Before you type, every command shows below
the field, in its group. The groups are File, Clips, Mix, Sound, Library,
Application, Transport, Edit, Panels, and Automation. Each group has the same
muted uppercase heading as the Automation menu.

A row is 30 points tall. From the left, it holds:

- a 6-point Accent dot when the tool, panel, or lane is on,
- the name in 13-point Text,
- 8 points after the name, a one-line description of the command in 12-point
  Muted. An ellipsis cuts it before the shortcut. It is also the accessible
  description of the row.
- the shortcut in 11-point Muted text at the right.

The current row has the Hover fill and a 3-point Accent bar. Up and Down move
the current row and scroll it into view. Typed words narrow the list to the
commands whose name or group holds every word. A changed search returns to the
top of the list. Enter uses the current search. This includes text that arrives
in the same frame. Enter or a click runs the command and closes the list. Esc or
a click outside closes the list. If nothing matches, the list reads "Nothing
matches. Clear the field to see the whole list again." The list holds no concept
entries for themes, sections, tracks, or sounds.

The search has the accessible name "Find a command". It keeps the text cursor
while the list shows. Tab and Shift+Tab do not move the cursor, because the
rows and the scroll parts of the list are not Tab stops. The modal blocks the
workspace controls, arrangement requests, and song drops. A file or
confirmation modal closes the command list before the list can take more
input.

## Status bar

The left end of the status bar holds Options and All commands. They are bare 28
by 24-point buttons, 10 points before the status text. Their Muted icons
brighten to Text under the pointer. Options is the gear PNG. While the window is
open, its icon takes the accent. A second activation focuses the open window and
keeps its draft. All commands draws a magnifier with the painter (no icon
asset). The magnifier uses the 24-unit grid of the rail icons, with their 2-unit
stroke.

The idle status is a usage hint in Muted: "Space plays the song. Drag a sound
from the Sounds panel onto the score. Ctrl+K lists every command." While the
Step synth board shows, its own hint takes the place of the idle text. The
Step synth board has no status line of its own. Where the whole idle text does
not fit, it shows a short form that keeps the command list: "Space plays.
Ctrl+K lists every command. Drag sounds onto the score."

Errors and application notices replace the idle text. An error is Coral.
After a minute it turns Muted, so that an old error does not look new. It
stays until another message replaces it or the user clears it. A message that
is not an error returns to the idle text after 8 seconds. Messages about work
in progress stay while playback, a preview, a recording, or a library scan
continues.

Once there is a message in the log, a clock mark starts the status line. The
painter draws a 12-point face in a 1.5-point stroke. The mark is Muted. It
brightens to Text under the pointer, in focus, or while the popup is open. An
error that the popup has not shown yet counts once it fades or another message
replaces it. While the count is above zero, a 12-point Coral disc with the count
takes the place of the clock. So an old error still shows at a glance. The error
that the line shows in Coral does not count yet. The status gives the count as
its accessible description. A faded error without the count keeps a 6-point
Coral dot before its text.

The status truncates, and its hover hint gives the full text. The pointer shows
a hand over the mark and the text, which share one Tab stop. A click, or Enter
in focus, opens a popup above the bar. The text has the accessible name
"Status", the Button role, and the full message as its value. The popup shows
the message in full. It also lists up to 8 messages in all, the current one
included, with their age. Once the popup shows, the clock replaces the count.
Long histories scroll within the window. Their scroll controls have names that
start with "Recent messages". Under a message, Clear returns the bar to the idle
text until the next message. The message stays in the list. Escape only closes
the popup. With missing samples, the bar has no idle text, and the clock mark
alone opens the popup.

A failed file action reads `Cannot <action> <file>. <reason>`. Common file I/O
failures use a plain reason without the Windows error code. Other diagnostics
keep their original detail. The status keeps filenames and other user text.
Where there is a next step, the message ends with it.

Notices sit between the icons and the status. Each notice is flat 12-point text
that opens what it reports. Its hover fill has the theme corner:

- Sample conflicts, in Warning, opens the Sample conflicts window.
- Import details opens the Import details window. With missing samples, the
  count comes first in Coral: "1 missing · silent".
- Reduced quality, in Warning, opens Options. Its accessible value is
  "Playback quality reduced; exports use full quality".

The status has the first claim on the width. When a message needs the room, the
notices shorten in steps. They show "1 missing", then the count without "Import
details", then "Conflicts" and "Reduced". The idle text does not make the
notices shorten.

The right end of the status bar holds a PANELS heading, five panel switches, and
the version link. The heading has the style of the rail group headings: 10-point
muted uppercase with 0.6-point tracking. The switches are:

- Tools shows or hides the names in the rail.
- Boards folds the lower panel away and gives the score its height. Boards
  again, or a board button in the rail, opens the panel on that board. The
  same rail button folds the panel when its board already shows.
- Sounds opens or folds the Sounds panel.
- Overview opens or folds the song overview. The overview belongs to the
  score, so it keeps its state when the score is maximised.
- Inspector opens or folds the Inspector.

Boards, Sounds, and Inspector show what is on screen. If the window has no room
for a panel, the panel folds and its switch reads off. A click opens the panel
and gives it the room. A switch that is on has an accent underline. The
application saves the choices in `layout.json` beside `options.json`. They do
not change the Options window or its revision. Without a saved layout, Tools,
Boards, Overview, and Inspector start on, and Sounds and Maximise start off. If
the Starting module is Sounds, each launch then opens the Sounds panel and folds
the lower panel.
