# Components: Options, dialogs, and feedback

## Options

Options uses the Utility Folded Metal shell. A vertical Audio, Workspace,
Files, and Sample libraries selector stays beside a scrollable body. Until the
user resizes it, the window follows the current page's height up to the
screen; a longer page scrolls. A fixed action row provides the tertiary
Restore options defaults, Apply, and Cancel. Sample libraries has a single
primary Done instead, because its actions take effect at once. Page selection and each page's
scroll position survive collapse and navigation while the window is open.
At 960x720, 1024x768, and 1440x1000, controls and actions must remain reachable
without horizontal scrolling. Ctrl+, opens or focuses this window. Section
titles use the 15-point uppercase heading face, 4 points below the bay edge and 8
points after an earlier section. On Workspace, Files, and Sample libraries,
each control has at most one help line.
Workspace and Mute previews use the shared switch.

Apply is enabled only while the draft differs from the active options, or
while restored defaults are to replace an invalid file. Each page with
unapplied edits shows a dot on its selector button (ink on the selected
Accent face, Accent otherwise), and the footer shows the count in Accent;
screen readers get the count as the button description and as a polite live
footer. Escape, close, Cancel, and Done close a clean draft at once. With
unapplied edits they open the blocking Discard changes? confirmation: Apply
changes (disabled with its reason when Apply is blocked), the Danger Discard
changes, and Keep editing; Escape or its close keeps editing. Collapse retains
edits without applying.

Place the 4x/Off explanation beside the separate Drive and master controls.
Below them, under the same Playback quality heading and in the same combo box
style, place the High/Low explanation and Sample playback quality. High, the
default, reads clips band-limited; Low reads them in straight lines between
samples for less CPU. Exports always use High, as they always use 4x; Low
changes live playback only and is a preference, not part of the song.
Show reduced playback quality, from any of the three controls, in Options and
the accessible workspace status as "Playback quality reduced; exports use full
quality".
Keep buffer target, requested frames, and measured callback interval distinct.
Label unavailable negotiated capacity explicitly. A known unsupported target
has a disabled reason; a failed opening leaves the current output in use.

Audio changes show a stopped-transport requirement until playback, previews,
and recording stop. Workspace and Files changes remain available during
playback. Show stale-draft feedback with Reload when preferences change outside
the window; retain unsaved edits until Reload is selected. Show persistence and
validation errors in the window. Restore options defaults edits the draft only.
Sample libraries contains the complete former library-management body: folder
registration and removal, progress and scan cancellation, Tempo for files
without one (the fallback tempo, placed under the actions it affects), folder
tempo and skipped-WAV reports, and optional product providers. These operations
use their existing Session operations. Scans adopt folder changes when complete;
the fallback tempo is used by the next Add library folder and Scan again. Library
and provider paths wrap in full; an enabled Remove describes its path. No
separate library window remains. Files holds only the song folder choice; its
fixed-folder field and picker appear only in Fixed mode.

Show tooltips controls hover hints; accessible names, focus, errors, and visible
guidance remain available. Reduced meter display samples at most 15 times per
second while retaining intervening peaks and the audio-owned OVER latch.

## Utility windows and blocking dialogs

Help, Time Stretcher help, Effect Studio help, Mastering help, Step synth help,
Sample editor help, Options, Album Review,
Import details, Recorded take, Save your changes?, Discard changes?, Sample
conflicts, Choose path,
and Import sound use the same shell and controls as musical tools. Preserve their
stable identities and exact accessible names independently of uppercase titles.
Native operating-system file pickers remain outside this system.

Every window can resize and collapse. Recorded take keeps its in-memory audio
while collapsed; Save take and Discard take remain the explicit resolving actions.
Discard take asks once more in place: Confirm discard erases the recording and
Keep take returns. Import details offers Resolve manually for each missing sound.
Collapsing a confirmation releases workspace and agent input and pauses its pending
action. Expanding restores blocking when a decision is still required. Choose path
suspends its parent confirmation while the path request is pending, including while
collapsed. Reject conflicting file requests instead of replacing paused data.
Close cancels a confirmation without saving, importing, or discarding implicitly.
Blocking dialogs and Recorded take follow the height of their content until the
user resizes them.
Sample conflicts confirms only through Keep choices; dismissal does not confirm.

Prepared material lives in `assets/folded-metal/<palette>/100/`;
shared controls live in `assets/folded-metal/controls/100/`.
`tools/assets/render-folded-metal.py` prepares both. The
shared material geometry supports Utility, Marine, Granite, Red, Olive, and
Indigo palettes. Fixed raster details
fade to a matching solid base; no window depends on a complete fixed-size skin.
Check the open and collapsed states, both resize directions, long content, focus,
and all button states at supported sizes. Native captures from
`tools/verify-ui.ps1` on 2026-10-27 show open windows with long content and
disabled actions: an overflowing body shows its scroll bar, and short blocking
dialogs follow the height of their content. They do not cover collapsed windows, resizing,
keyboard focus, hover or pressed states, accessibility, physical audio,
or native file pickers.

## Empty states and feedback

The empty Sounds panel offers Open sample libraries. Filtered empty
results offer Clear filters. The status strip truncates long messages and gives
the full text on hover. Import details remain available when a project carries
compatibility notices.

Keep task guidance in Help and control tooltips. Do not repeat drag, preview,
tempo, or save instructions across the workspace and tool windows. Keep concise
warnings for missing audio, unsaved recordings, polarity inversion, and approximate
pitch preservation. Import details retain all saved source and placement metadata
as selectable text. Help groups gestures by action and scrolls within the viewport.

Display full filesystem paths without the Windows extended-length prefix.
Show network paths as `\\server\share`. Apply this presentation rule to folder
labels, tooltips, status messages, and import details. Keep stored paths intact;
display formatting must not change file access, project data, or settings.
