# Components: Options, dialogs, and feedback

## Options

Options uses the Utility Folded Metal shell. A vertical selector with Audio,
Workspace, Files, and Sample libraries is beside a body that scrolls. Until
the user resizes the window, it follows the height of the current page, up to
the height of the screen. A longer page scrolls. A fixed action row holds the
tertiary Restore options defaults, Apply, and Cancel. Sample libraries has one
primary Done in place of these, because its actions apply at once. The page
selection and the scroll position of each page stay through collapse and
navigation while the window is open.

At 960x720, 1024x768, and 1440x1000, all controls and actions must be
reachable without horizontal scrolling. Ctrl+, opens or focuses the window.
Section titles use the 15-point uppercase heading face. A section title is
4 points below the edge of the bay, or 8 points after an earlier section. On
Workspace, Files, and Sample libraries, each control has a maximum of one help
line. Switches, such as Mute previews and Show tooltips, use the shared
switch.

Apply is enabled only while the draft is different from the active options,
or while restored defaults must replace a file that is not valid. Each page
with edits that are not applied shows a dot on its selector button. The dot
is ink on the selected Accent face, and Accent on the other buttons. The
footer shows the count in Accent. Screen readers get the count as the button
description and as a polite live footer.

Escape, close, Cancel, and Done close a clean draft at once. With edits that
are not applied, they open the blocking Discard changes? confirmation. It
holds:

- Apply changes, disabled with its reason when Apply is blocked,
- the Danger Discard changes,
- Keep editing.

Escape or the close of the confirmation keeps the edits. Collapse keeps the
edits and does not apply them.

### Audio page

The Playback quality section holds Drive playback oversampling and Master
playback oversampling (4x or Off), then Sample playback quality (High or Low).
Each control has its help line under it. High is the default and reads clips
band-limited. Low reads them in straight lines between samples, for less CPU.
Exports always use High, as they always use 4x. Low changes only live
playback. It is a preference, not part of the song. If one of the three
controls reduces the quality, Options and the accessible workspace status show
"Playback quality reduced; exports use full quality".

Keep the buffer target, the requested frames, and the measured callback
interval as separate facts. Give a negotiated capacity that is not available
an explicit label. A known target that is not supported has a disabled reason.
If the output does not open, the current output stays in use.

Meter display rate set to Reduced draws meters a maximum of 15 times a second.
It keeps the peaks between the draws and the OVER latch of the audio engine.

### Changes and errors

Audio changes show that the transport must stop, until playback, previews,
and recording stop. Workspace and Files changes stay available during
playback. When the preferences change outside the window, show stale-draft
feedback with Reload active options. Keep the edits that are not saved until
the user selects it. Show persistence and validation errors in the window.
Restore options defaults edits only the draft.

### Files and Sample libraries

Files holds only the song folder choice. Its fixed-folder field and picker
show only in Fixed folder mode.

Sample libraries holds all library management:

- folder registration and removal,
- progress and scan cancellation,
- Tempo for files without one (the fallback tempo, under the actions that use
  it),
- the folder tempo and skipped-WAV reports,
- the optional product providers.

These operations use the Session operations. A scan applies the folder
changes when it is complete. The next Add library folder and Scan again use
the fallback tempo. Library and provider paths wrap in full. An enabled Remove
describes its path. The page scrolls vertically inside the shared window. A
large expanded Folder tempos list must keep the provider controls reachable.
The provider product list shows all its rows and does not scroll.

Show tooltips controls the hover hints. Accessible names, focus, errors, and
visible guidance stay available when it is off.

## Utility windows and blocking dialogs

These windows use the same shell and controls as the musical tools:

- Help, Time Stretcher help, Effect Studio help, Mastering help, Step synth
  help, and Sample editor help,
- Options, Album Review, Import details, and Recorded take,
- Save your changes?, Discard changes?, Sample conflicts, Choose path, and
  Import sound.

Keep their stable identities and exact accessible names, independent of the
uppercase titles. Native file pickers of the operating system are not part of
this system.

Every window can resize and collapse. Recorded take keeps its audio in memory
while it is collapsed. Save take and Discard take are the explicit actions
that resolve it. Discard take asks once more in place: Confirm discard erases
the recording, and Keep take goes back. Import details offers Resolve manually
for each missing sound.

The collapse of a confirmation releases workspace and agent input and pauses
its pending action. Expansion blocks again when a decision is still necessary.
Choose path suspends its parent confirmation while the path request is
pending, also while it is collapsed. Reject a file request that conflicts,
and do not replace paused data. Close cancels a confirmation, and does not
save, import, or discard. Blocking dialogs and Recorded take follow the
height of their content until the user resizes them.

Sample conflicts confirms only through Keep choices, because the confirmation
is a project change that the user can undo. Dismissal does not confirm. Esc
hides the window without an edit, and Sample conflicts in the status bar opens
it again.

Prepared material is in `assets/folded-metal/<palette>/100/`. Shared controls
are in `assets/folded-metal/controls/100/`. `tools/assets/render-folded-metal.py`
prepares both. The shared material geometry supports the Utility, Marine,
Granite, Red, Olive, and Indigo palettes. Fixed raster details fade to a
matching solid base. No window depends on a complete skin of fixed size. Check
the open and collapsed states, both resize directions, long content, focus,
and all button states.

## Empty states and feedback

The empty Sounds panel offers Open sample libraries. Filtered results that
are empty offer Clear filters. The status strip truncates long messages and
gives the full text on hover. Import details stays available when a project
has compatibility notices.

Keep task guidance in Help and control tooltips. Do not repeat drag, preview,
tempo, or save instructions across the workspace and tool windows. Keep short
warnings for missing audio, recordings that are not saved, polarity inversion,
and approximate pitch preservation. Import details keeps all saved source and
placement metadata as text that the user can select.

Help has one page for each topic. A page list is at the left of the page, or
above it when the window is narrow. Shortcuts is the first page. It shows one
action on each line, with its keys as key caps in a column. The other pages
use short headed sections, numbered steps, and a list of terms. Mastering
explains dBFS, dBTP, LUFS, and VU. Each page has a heading for assistive
technology. In the list, the up and down arrow keys change the page. Page
Up, Page Down, Home, and End scroll the page. F1 opens and closes Help.
The selected topic scrolls into view during keyboard navigation. In a short,
narrow window, the topic list scrolls separately and leaves room for the page.
Page scroll keys also work when a Help header control has focus. Topic controls
have names of the form `Help page: Mixer` to keep command names unique.

The Help of a tool or a board shows only its own page, in the palette of that
tool. It opens beside the tool window, or above the lower panel for a board,
so that it does not cover the controls that it explains. A test makes sure
that each tool of the rail has a page. At compact sizes, a short scrolling
page fits above the tool. If no adjacent area can fit a page, Help opens at
the centre; move or resize either window to make room.

Show full file system paths without the Windows extended-length prefix. Show
network paths as `\\server\share`. Apply this rule to folder labels, tooltips,
status messages, and import details. Keep the stored paths unchanged. The
display format must not change file access, project data, or settings.
