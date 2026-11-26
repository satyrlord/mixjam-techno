# Arrange

## Contents

[Automation](automation.md#automation) has its own page.

## Lane headers

Each lane header shows these items, from the left:

- The track number. Click it to select the destination for the next recording.
- The name of the lane. This is the sound family of the first clip on the
  lane: for example, "deep chord" for "deep chord 3".
- Small **M** (Mute) and **S** (Solo) buttons.
- A small pan knob. To move it, drag it or use the arrow keys. For fine steps,
  hold Shift while you drag. To center the pan, double-click the knob.

## Place and edit clips

To find and hear sounds, use the [Sounds panel](interface.md#sounds-panel).

- To place a sound, drag it onto a track. For sixteenth-note positions, hold
  Ctrl. The arrangement and the Mixer do not show unused tracks.
- To select clips, hold Shift and click them. To select a group, drag across
  an empty area.
- To copy a clip, hold Shift or Alt while you drag it. A ghost shows where the
  clips will go. Each clip of a group must go onto a shown track or the Add row.
  If a clip goes onto an unused track, a label names that track and the drop
  does nothing.
- To delete the selected clips, press Delete. Use Undo and Redo to move back and
  forward through your edits.
- To trim a clip, drag its left or right edge. Edges snap to whole bars.
  Hold Ctrl for sixteenth-note steps.
- To set the fades, drag the handles at the top corners of the clip.
- To set the gain, drag the small handle in the middle of the clip up or down.
  Each step is 1 dB. For 0.1 dB steps, hold Ctrl.

Each completed drag is one undo step. The arrangement shades each fade over
its clip.

## Clip properties

To open the properties of a clip, right-click it. You can also select the
clip and press Shift+F10. The menu sets these values:

- Gain.
- Trim start.
- Length.
- Fade in and fade out, in beats, each with its curve: Linear, Equal power, or
  Exponential.
- Pitch, in semitones and cents.
- Reverse.

The fade out ends where the sound of the clip ends. This can be before the end
of the clip, so a phrase that stops suddenly can fade away. Pitch does not
change the length or the timing of the clip.

If the clip's audio is missing, Preview is disabled. Select **Resolve
manually** in the clip menu to choose its audio file.

Reverse plays a clip backwards: for example, for reversed cymbals, swells,
and risers. The clip plays the same part of its sound: from the trim start, for
the length of the clip. The part plays from its end to its start. The fades,
the gain, and the pitch do not move. Thus the fade in still opens the clip, and
the fade out still closes it. If the sound is shorter than a reversed clip, the
clip starts with silence and ends with the clip. A reversed clip shows "rev" in
its corner.

The trim and the length always refer to the forward part of the sound. Thus,
on a reversed clip, a drag of the left edge trims the sound at the end of the
clip.

Each clip shows the waveform of the part of its sound that it plays, reversed
and faded as you hear it. A clip of a Step synth sequence shows its notes
instead. Each note is a short bar, and a higher note is higher in the clip. A
note bar continues through the ties and the gate of the note. Save sound
keeps the pattern beside the WAV for this. A Step synth sound without a
pattern file shows its waveform.

## Playback and loop

- To play or pause, press Space.
- To select a loop, drag across the bar ruler.
- Playback uses the **Playback buffer target** in
  [Options](options.md#options). The default is 40 ms where the
  device supports it. The device controls the actual callback interval.
- If playback sounds wrong, for example a sudden loud hiss, press F12 while it
  occurs. Refer to [Audio captures](captures-and-limits.md#audio-captures).

## Sections

A song can have sections: named parts such as Intro or Break. Each section is a
bookmark. It shows as a colored line across the lanes, with its name on the bar
ruler. A section continues until the next section starts. The song keeps its
sections. Agents set them with `set_sections`.

With sections shown, the Loop button loops the section at the playhead.
Without sections, it loops the full song. Before the first bookmark, it loops
the lead-in. If the last bookmark is after the end of the song, the loop is one
bar, and the timeline limits it.

To add a section:

1. Double-click the bar ruler at the bar where the section starts.
2. Type the name.
3. Press Enter. To keep the old name, press Escape.

To rename a section, double-click its name. To rename or delete it,
right-click its name.

To hide the bookmarks in the score and in the song overview, turn off
**Sections** beside Follow. The application keeps that choice. If you hide the
bookmarks during a name edit, the application cancels the edit.

## View

- To zoom the timeline in or out, drag **Zoom**, or turn the mouse wheel over
  it.
- To keep the playhead in view, turn on **Follow** beside Zoom. To move
  through the timeline freely during playback, turn it off.
- To give the lower panel more or less space, drag the divider above it.
- To show Mixer, Mastering, FX returns, or Step synth, select its button in
  the tool rail. If that board already shows, the button folds the lower
  panel.

The lower panel keeps the height that you set. When a board needs more space,
the panel becomes taller to fit its controls. In a short window, the board
scrolls.
