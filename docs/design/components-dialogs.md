# Components: Options, dialogs, and feedback

## Options

Options uses the Utility Folded Metal shell. A vertical selector with Audio,
Workspace, and Files is beside a body that scrolls. Until the user resizes the
window, it keeps the height of the tallest page shown since it opened, up to the
height of the screen. So the footer does not move between pages. A longer page
scrolls. A fixed action row holds the tertiary Restore all defaults at the left.
The status, Apply, and Cancel are at the right. While changes wait, the status
and the page markers use the warning colour. The selected page takes the
selection colour. The page selection and the scroll position of each page stay
through collapse and navigation while the window is open.

At 960x720, 1024x768, and 1440x1000, all controls and actions must be
reachable without horizontal scrolling. Ctrl+, opens or focuses the window.
Section titles use the 15-point uppercase heading face. A section title is
4 points below the edge of the bay, or 8 points after an earlier section. On
Workspace and Files, each control has a maximum of one help line. Switches,
such as Mute previews and Show tooltips, use the shared switch.

Apply is enabled only while the draft differs from the active options. It is
also enabled while restored defaults must replace a file that is not valid. Each
page with changes that wait shows a dot on its selector button. The dot is ink
on the selected face, and the warning colour on the other buttons. The footer
shows the count in the warning colour. Screen readers get the count as the
button description and as a polite live footer.

Escape, close, and Cancel close a clean draft at once. If the draft has changes
that wait, they open the Discard changes? confirmation. This confirmation blocks
the other controls. It holds:

- Apply changes, disabled with its reason when a value blocks Apply,
- the Danger Discard changes,
- Keep editing.

Escape or the close of the confirmation keeps the edits. Collapse keeps the
edits and does not apply them.

### Audio page

The Playback quality section holds Drive playback oversampling (2x or Off) and
Master playback oversampling (4x or Off), then Sample playback quality (High or
Low). Each control has its help line under it. High is the default and reads
clips band-limited. Low reads them in straight lines between samples, for less
CPU. Exports use High for sample reading, 8x for track Drive, and 8x for master
saturation when the board runs. Low is a playback preference. If one of the three
controls reduces the quality, Options and the accessible workspace status show
"Playback quality reduced; exports use full quality".

Keep the buffer target, the requested frames, and the measured callback
interval as separate facts. Give a negotiated capacity that is not available
an explicit label. A known target that is not supported has a disabled reason.
If the output does not open, the current output stays in use.

Meter display rate set to Reduced draws meters a maximum of 15 times a second.
It keeps the peaks between the draws and the OVER latch of the audio engine.

### Changes and errors

Audio changes show that the transport must stop, until playback, previews, and
recording stop. Workspace and Files changes stay available during playback. When
the preferences change outside the window, show stale-draft feedback with Reload
active options. Keep the draft edits until the user selects Reload active
options. Show persistence and validation errors in the window. Restore all
defaults edits only the draft, on every page.

Each colour has one meaning:

- coral for an error or a value that blocks Apply,
- the warning colour for a state that waits or that reduces quality,
- the help colour for guidance.

The application checks the fixed song folder where the user types it, with the
same words that keep Apply disabled.

### Files and Sample libraries

Files holds the song folder choice. Its fixed-folder field and picker show
only in Fixed folder mode. A Sample libraries section under it opens the
Sample libraries tool window.

The Sample libraries tool window holds all library management. It has no
footer, because its actions apply at once:

- folder registration and removal,
- progress and scan cancellation,
- Tempo for files without one (the fallback tempo, under the actions that use
  it),
- the folder tempo and skipped-WAV reports,
- the optional product providers.

These operations use the Session operations. A scan applies the folder changes
when it is complete. The next Add library folder and Scan again use the fallback
tempo. Library and provider paths wrap in full. An enabled Remove describes its
path. The window scrolls vertically. A large expanded Folder tempos list must
keep the provider controls reachable. The provider product list shows all its
rows and does not scroll.

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
while collapsed. Save take and Discard take are the explicit actions that
resolve it. Discard take asks once more in place: Confirm discard erases the
recording, and Keep take returns. Import details offers Resolve manually for
each missing sound.

The collapse of a confirmation releases workspace and agent input and pauses its
pending action. Expansion blocks again when a decision is still necessary.
Choose path suspends its parent confirmation while the path request is pending,
also while collapsed. Reject a file request that conflicts, and do not replace
paused data. Close cancels a confirmation, and does not save, import, or
discard. Blocking dialogs and Recorded take follow the height of their content
until the user resizes them.

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
warnings for missing audio, unsaved recordings, polarity inversion, and
approximate pitch preservation. Import details keeps all saved source and
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
tool. It opens beside the tool window, or above the lower panel for a board. So
it does not cover the controls that it explains. A test checks that each tool of
the rail has a page. At compact sizes, a short scrolling page fits above the
tool. If no adjacent area can fit a page, Help opens at the centre. Move or
resize either window to make room.

Show full file system paths without the Windows extended-length prefix. Show
network paths as `\\server\share`. Apply this rule to folder labels, tooltips,
status messages, and import details. Keep the stored paths unchanged. The
display format must not change file access, project data, or settings.
