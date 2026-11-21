# Components: actions, rail, and Sounds panel

## Contents

These parts have their own page,
[Components: command list and status
bar](components-commands-and-status.md):

- [Command list](components-commands-and-status.md#command-list)
- [Status bar](components-commands-and-status.md#status-bar)

## Actions and transport

An active workspace action button uses Accent with On light text. An inactive
one uses Raised with Text. Undo and Redo use transparent PNG arrow icons from
the shared artwork cache. Their tooltips and accessibility labels give the
action names. Help uses its own ringed question-mark PNG, with Help as its
accessibility label. Tooltips name each action with its window or shortcut.
Accessible names stay short.

The Export icon in the tool rail has the tooltip "Export the song as a stereo
WAV". Play / pause (Space) stays prominent in the transport, on an Accent
fill. The other transport controls use raised surfaces and hover fills. While
Record is on, it shows a coral circle with a Text ring. The symbols are native
geometry that the application paints. They have tooltips and accessible action
names.

## Library loading

A library scan shows Legend status text above a recessed dark progress rail.
The Options library page holds them. While the application lists the files, a
segment in the selection color moves along the rail. When the total is known,
the fill shows the completed fraction. The Cancel library scan button sits
beside the rail. The rail is 80 to 300 points wide and fits the space that is
available. The text never overlaps the fill.

## Tool rail

The rail at the left uses the utility Folded Metal finish. It has dark neutral
paint, subtle monochromatic grain, and smooth folded edges. The workspace accent
of the theme marks active tools and focus: lime in Studio and Aero, amber in
Vintage.

The rail has two widths:

- With names (the default), it is 84 points wide. Each tool is a 68 by
  44-point target. The center of its 20-point icon is 15 points below the top.
  A 10.5-point name under the icon ends 5 points above the bottom.
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
keyboard focus only. The icon of an active tool also takes the accent. Its
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
compact. The names stay. The targets shrink to 68 by 36 points. The icon
center is 12 points below the top, and the name is 2 points above the bottom.
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

## Sounds panel

The Sounds panel lists the library beside the score. From the top, its
250-point column holds:

1. The painted Sounds title, a muted count ("N of total"), and the fold
   chevron.
2. The search field across the full width.
3. The library and category menus, side by side. The library menu shows all
   its rows and does not scroll.
4. The category chips.
5. The sort menu with its direction arrow.

Chips are 22 points high: a 6-point square in the category color before a
12-point name. An unselected chip uses Panel fill, Muted text, and a Hover
border. The chosen chip uses Hover fill, Text, and a Muted border. In a panel
shorter than 440 points, the chips share one row that scrolls sideways. Then
the list keeps its room. Otherwise the chips wrap.

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

Hover or selection uses the Tile hover fill. Selection or keyboard focus adds a
one-point border in the category color. One tooltip groups these facts:

- the full name, product, category, and code,
- sample IDs and source tempo,
- nominal bars and actual audio duration,
- frames, rate, and channels,
- the WAV path, when available.

Nominal bars and audio duration stay separate facts.

The waveform shows the lowest and highest sample of the sound per point, in
its category color. Its scale follows the peak of the sound. It sits over a
centre line in the category color at 45% opacity. If the waveform has no
data yet, or the file cannot be read, the row shows only the centre line. The
list draws only the rows on screen, and only those rows request waveforms.

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
