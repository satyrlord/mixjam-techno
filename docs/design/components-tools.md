# Components: sound tools and Album Review

## Contents

[Step synth](components-step-synth.md#step-synth) has its own page.

## Time Stretcher

`src/ui/stretch.rs` supplies the body inside the common shell. The source
selector has search, bounded long names, full-name tooltips, and a
virtualized list. Each opening starts with an empty search that has focus.
Original BPM accepts 30-300, and Original uses the shared rotary interaction.
Target is the labelled tempo slider over 120, 125, 130, 135, 140, 160, and
180. A double-click on Original restores the tempo of the selected source. A
double-click on Target resets it to 140. Hover alone never changes a value.

Listen and Save need available audio. Processing uses the application worker
and disables duplicate requests. Listen becomes Stop listening while the
preview of this tool plays. Processing and preview indication must not look
like a save indication. If a save is canceled or fails, the tool never reports
success. After a save, the tool keeps the selected source and both tempo
drafts. It names the saved file and category in the window, and the
application status keeps the full path.

Feedback shows progress, errors, save confirmation, or the source and tempo
values of the current audition. Draft changes stay separate from the running
preview. Idle guidance says that the duration changes and that the pitch
stays approximately the same. Equal tempos state that the duration does not
change. Keep the "Listen before saving" guidance until the user hears the
current settings. Errors start with Cannot and have an explicit error
indication. The full feedback stays available in a tooltip and a polite
accessible live region. Missing audio offers Import details. Help explains the
ranges, input, reset, processing, preview, and audible limits.

Esc first closes a popup, leaves exact entry, or goes to a higher window. If
none of these applies, Esc closes the topmost tool. Collapse keeps the source,
tempos, and feedback. The body sends actions through `src/ui/tools.rs`. It
does not process audio or write files.

## Effect Studio

Effect Studio and Step synth share six sections in `src/ui/effects_rack.rs`.
Effect Studio also shows a preset selector, tail controls, and response curves.

| Section | Controls |
| --- | --- |
| Filter | Switch, LP12/LP24/BP12/HP12, Cutoff, Resonance, Overdrive, Filter volume |
| Modulation | Switch, Chorus or Phaser, Rate, Depth, Regen, Wet |
| Echo | Switch, Ping-pong, Delay (beats), Feedback, Tone, Echo volume |
| Robotize | Switch, Ring or Shift, Color, Robotize volume |
| Reverb | Switch, Room/Hall/Plate, Decay, Damp, Wet |
| Distortion / output | Warm/Acid/Fuzz/Crush, Dirty distortion, Noise distortion, Output volume |

Use the same control geometry and interaction as the other windows. Keep dial
marks, values, and labels in separate regions with measured clearance.

An optional section pairs a named switch with a visible state. A disabled
section dims its controls and keeps its settings. Reset restores the default
draft. Cutoff and Color use a logarithmic scale. The source label truncates
and keeps its full tooltip and accessible value.

Listen and Save sound need selected, available audio. Missing audio offers
Import details. Processing and preview use synchronous application calls. The
window has no worker and no Stop listening, as Time Stretcher has. Save sound
writes a WAV to Effect at the source BPM and selects it. Local feedback
reports preview, reset, file choice, cancellation, errors, and save
confirmation. Keep the full feedback in a tooltip and a polite accessible live
region, with explicit error text. Help and collapse keep the full draft. The
presentation makes no claim of DSP equivalence.

## Sample Editor

`src/ui/sample_editor.rs` supplies trim, pitch, source, actions, and feedback
inside the common shell. Use the shared value fields, slider, buttons, and
header. The source name truncates, with a full tooltip and accessible value.

Start frame is inclusive and zero-based. End frame is exclusive. An End equal
to the frame count of the source includes its last frame. Keep unsigned
64-bit exact entry. The adjacent buttons change one frame, or 100 with Shift.
Pitch stays from -24 to +24 semitones. Its buttons change 0.1 semitone, or one
semitone with Shift. The slider and the field share the same draft. Explain
exact entry, the limits, and how pitch changes speed and duration.

While the editor is collapsed, selections in the background do not reset the
trim draft. When the editor expands, a different active source or catalog
revision resets the range, as the reset policy sets. While the editor is open,
a source change resets the range to the actual frames of the source.

Listen edit and Save edited sound use the shared secondary and primary
buttons. If the source is absent or missing, the frame range is not valid, or
the pitch is not valid, these buttons are disabled. A disabled button gives an
explicit reason. Feedback shows
the selected frames and the duration after pitch, or the reason that editing
is not available. Keep the full status in its tooltip and a polite accessible
live region. Missing audio offers Import details. Esc first leaves exact
entry, a popup, or a higher window. If none of these applies, Esc closes the
topmost editor.

The application owns the session trim, resampling, preview, WAV save,
registration in Wave, and source-BPM behavior, and runs them synchronously. A
save does not change the source. Collapse keeps the edits and does no
processing.

## Album Review

Album Review is a Utility window. The grid view starts with the search field,
the sort combo, and the "visible / total albums" count. Artist and genre
filters are Secondary buttons that become Primary while they are active. If a
genre has no album with the current selection, its button is disabled and says
why. The genre row shows the six most-used tags and "+N more". "+N more"
becomes "Show fewer" when the row is open. Active filters show again as chips
with × before Clear filters.

Cards are 150 by 214 points. A card shows the cover or a painted record
placeholder. It also shows the artist in muted capitals, the title in the
Legend color, and the year and track count.

The album page puts the cover column (220 points) next to the title, the
metadata grid, and the links. Below them are:

- the overview,
- a timeline chart with one row per track, in which sections with the same
  name share a color,
- each track, with its energy written out next to a colored label, so that
  color is never the only cue.

The empty states are explicit: No reviews folder, No albums available, and No
results with Clear filters. Covers are review content, and the raster asset
rules do not apply to them. The rules apply to the rail icon.
