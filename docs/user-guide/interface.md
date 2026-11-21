# Interface

## Themes

The application has three themes:

- **Studio**, the default: plain, flat, and dark.
- **Aero**: colorful cobalt and aqua plastic.
- **Vintage**: worn painted metal with ivory mixer strips.

To change the theme, use **Theme** on the **Workspace** page of
[Options](options.md#options). All themes have the same layout,
controls, and keyboard order. Only the colors, materials, control shapes, and
typefaces change. Titles are uppercase in all themes.
[DESIGN.md](../DESIGN.md) is the visual contract.

The title, the window, the taskbar, and the Windows executable use the tribal
mask icon. [The icon assets](../../assets/app/README.md) give its source and
the steps to make it again.

## Top bar and score toolbar

The top bar holds these items:

- The brand.
- The song name. An asterisk shows that the song has unsaved changes. Below a
  short name, the song length shows in bars and minutes.
- The transport, with the recording track beside Record.
- The position readout, in minutes and seconds. To see the bar and the beat,
  put the pointer on the readout. A wide window also shows them beside it.
- The tempo.
- Undo, Redo, and Help.

The full song name always shows. A long name wraps onto two lines or uses
smaller text. In a narrow window, the transport buttons become narrower to
make room.

Help, or **F1**, opens the manual. The first page lists the shortcuts. To
read about a part of the application, select it in the list at the left. In
the list, the up and down arrow keys change the page. Page Up, Page Down,
Home, and End scroll the page. Press **F1** again to close Help. The Help
button of a tool or a board opens only its own page, beside the tool.

The score toolbar above the lanes holds Arrange and Mix, the Automation menu,
Follow, Sections, Zoom, and Maximise. Maximise, or **F**, hides the tool rail,
the Sounds panel, the Inspector, and the lower panel. The score then fills the
window. To show the panels again, press **F** again. You can also select the
**Tools**, **Boards**, **Sounds**, or **Inspector** switch in the status bar.

## Tool rail and lower panel

The tool rail at the left has five groups. Each tool shows its name under its
icon.

| Group | Tools |
| --- | --- |
| File | New, Open, Save, Save as, Import, Export |
| Clips | Erase, Delete |
| Mix | Mixer, Mastering, FX returns, Step synth: the boards of the lower panel |
| Sound | Edit, Stretch, Effects: the windows that make a new sample |
| Library | Review (Album Review) |

To see the window and the shortcut of a tool, put the pointer on it. In a
shorter window, the rail makes its tools smaller and uses lines, not headings,
between the groups. If the rail does not fit the window, turn the mouse wheel
over it to scroll it.

To shrink the rail to icons, select the double chevron at the top of the rail.
You can also select the **Tools** switch at the right of the status bar. Do the
same step again to show the names.

Mixer, Mastering, FX returns, and Step synth share the lower panel. The name
of the board that shows is at the start of the top row of the panel. To show a
board, select its button in the tool rail. If that board already shows, the
button folds the panel.

The **Boards** switch in the status bar also folds the lower panel. To open the
panel again, select **Boards** again, or select a board in the rail.

## Command list

To open the command list, press **Ctrl+K**. You can also select the magnifier
beside the Options gear at the left end of the status bar. The list holds all
that the application can do:

- The tools of the rail.
- Options and Help.
- Playback, Undo, Redo, and the other edits.
- Maximise and the Mix view.
- The panel switches of the status bar.
- Each automation lane.

All commands show in their groups before you type. Each command shows a
one-line description, and its shortcut shows at the right. A dot marks a tool,
a panel, or a lane that is on.

To use the list:

1. Type words to make the list shorter.
2. Press Up or Down to move through the list.
3. Press Enter, or click a command, to run it.
4. To close the list, press Esc.

## Sounds panel

The Sounds panel shows the sample library as a list. At the top of the panel
are the search, the library and category filters, the category chips, and the
sort. Each row of the list shows the name, the category, the length in bars,
and the waveform of a sound.

To open the Sounds panel, select the strip named SOUNDS beside the rail. You
can also select the **Sounds** switch in the status bar. To fold the panel,
select the chevron at its top.

To use a sound:

- To place it, click the sound, then click a lane.
- To place it at the playhead on the recording track, select it and press
  Enter.
- To hear it, double-click it.
- To place it at a position, drag it to a lane.

The list also works from the keyboard. To give it the focus, click a sound, or
press Tab until an outline shows on the list. Then use these keys:

| Key | Result |
| --- | --- |
| Up, Down | Selects the previous or the next sound. |
| Page Up, Page Down | Moves one page of the list. |
| Home, End | Selects the first or the last sound. |
| Enter | Places the selected sound at the playhead on the recording track. |
| Space | Plays or pauses the song. |
| Delete | Deletes the selected clips. |

The **Sort** list sorts the sounds in library order, by name, by length, by
BPM, or by category. The arrow beside the list reverses the order: for
example, longest first or fastest first.

The application keeps these choices for the next session. If **Starting
module** is Sounds, the application starts with the Sounds panel open and the
lower panel folded.

## Inspector

The Inspector at the right of the score shows the selected item:

- Nothing selected: the song, with its tempo and length, a clip count for
  each category, and its sections. Click a section to scroll the score to it.
- A track: click the name of a track in its lane header. This opens a
  folded Inspector. The track page shows the EQ curve, processing switches,
  level, pan, outputs, and sends. Click the EQ curve to open the channel EQ
  window. On the track page, click the name again to see the song.
- One clip: a picture of the clip with fade handles that you can drag. The page
  also shows its fades, gain, pitch, Reverse, start, length, and trim start.
  If its sound is missing, **Locate the sound…** asks for the file.
- Several clips: their shared fades, gain, pitch, and Reverse. A change
  sets every selected clip. A dash marks mixed numbers and fade curves.
  If some of the clips play backwards and some do not, Reverse shows
  **Some play backwards**.

You can undo all changes that you make in the Inspector. To fold the Inspector
to a strip, select the chevron at its top. You can also select the
**Inspector** switch in the status bar.

In a small window, the Sounds panel, the Inspector, and the lower panel share
the space. The panel that you opened last shows, and the others fold until
there is space for them.

## Song overview

The song overview above the score shows the full song:

- Its sections, when Sections is on.
- A thin line for each clip, in the color of its category.
- The loop and the playhead.
- A frame around the part of the song that the score shows.

To move the score, click or drag in the overview. When the overview has the
focus, Left and Right move by one screen. Home and End go to the start and the
end. The overview does not change the song.

To fold the overview, select the button at its lower right, or the
**Overview** switch in the status bar. The overview then shows as a bar named
SONG OVERVIEW. To open it again, click the bar.

If the window is too short for the overview and the lanes, the bar shows
until there is space. If the bar also does not fit, the overview hides until
there is space. Your saved choice does not change. To get more space, fold the
lower panel or maximise the score.
