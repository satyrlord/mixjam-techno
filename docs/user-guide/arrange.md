# Arrange

## Lane headers

Each lane header shows these items, from the left:

- The track number. Click it to record on that track.
- The name of the lane. This is the sound family of the first clip on the
  lane: for example, "deep chord" for "deep chord 3".
- Mute and Solo.
- The left and right outputs.
- A short level fader.

The fader sets the same track gain as the Mixer. To move it, drag it, turn the
mouse wheel, or use the arrow keys. For fine steps, hold Shift while you drag.
To set unity gain, double-click the fader.

## Place and edit clips

To find and hear sounds, use the [Sounds panel](interface.md#sounds-panel).

- To place a sound, drag it onto a track. For sixteenth-note positions, hold
  Ctrl. The arrangement and the Mixer do not show unused tracks.
- To select clips, hold Shift and click them. To select a group, drag across
  an empty area.
- To copy a clip, hold Shift or Alt while you drag it. A ghost shows where the
  clips will go. Each clip of a group must go onto a shown track or the Add row.
  If a clip would go onto an unused track, a label names that track and the
  drop does nothing.
- To delete the selected clips, press Delete. Use Undo and Redo to go back and
  forward through your edits.
- To trim a clip, drag its left or right edge.
- To set the fades, drag the handles at the top corners of the clip.
- To set the gain, drag the small handle in the middle of the clip up or down.
  Each step is 1 dB. For 0.1 dB steps, hold Ctrl.

Each completed drag is one undo step. The arrangement shades each fade over
its clip.

## Clip properties

To open the properties of a clip, right-click it. Alternatively, select the
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

Reverse plays a clip backwards: for example, for reversed cymbals, swells,
and risers. The clip plays the same part of its sound, from the trim start for
the length of the clip, from its end to its start. The fades, the gain, and
the pitch do not move. Thus the fade in still opens the clip, and the fade out
still closes it. If the sound is shorter than a reversed clip, the clip starts
with silence and ends with the clip. A reversed clip shows "rev" in its
corner.

The trim and the length always refer to the forward part of the sound. Thus,
on a reversed clip, a drag of the left edge trims the sound at the end of the
clip.

Each clip shows the waveform of the part of its sound that it plays, reversed
and faded as you hear it. A clip of a Step synth sequence shows its notes
instead. Each note is a short bar, and a higher note is higher in the clip. A
note bar continues through the ties and the gate of the note. Save sequence
keeps the pattern beside the WAV for this. A Step synth sound without a
pattern file shows its waveform.

## Automation

To draw automation, choose a lane in the **Automation** menu above the
arrangement. The lane then shows on all tracks. The menu has 14 lanes in three
groups:

| Group | Lanes |
|---|---|
| On every track | Volume (dB over the fader), Pan, Filter, Pitch, Repeat, Repeat gate |
| Send amount, on every track | FX 1 send to FX 4 send. Each has the name of the effect on its bus. |
| Return level, one line across the song | FX 1 return to FX 4 return |

The menu marks the current lane. Each lane shows how much it holds: the number
of tracks with points ("2 tracks"), or the number of points on a return lane
("4 points"). To edit clips again, choose **Off**.

To edit points on a lane:

- To add a point, click the lane.
- To move a point, drag it.
- To delete a point, double-click or right-click it.

Points snap to 1/16 bar. For single ticks, hold Ctrl. Between two points, the
value moves in a straight line. The Repeat and Repeat gate lanes are different:
they move in steps.

### Filter and send lanes

The Filter lane is a low-pass filter below the middle of the row and a
high-pass filter above it. In the middle, the filter is open. The lane moves
from low-pass to high-pass without a click.

An FX send lane sets the amount that the track sends to that bus. While the
lane has points, it replaces the send knob of the track.

### Pan lane

The Pan lane moves a track across the stereo field. The top of the row is
hard right, the bottom is hard left, and the middle is the center. A point
shows its value as L 40, C, or R 100. While the lane has points, it replaces
the pan knob of the track, and the Mixer shows Pan automated. The pan changes
at each sample. Thus a sweep on each 16th note of a ratchet sounds continuous.

### Repeat lane

The Repeat lane cuts the sound of a track into repeats, for rolls and
ratchets. Its row has six steps. From the bottom, they are Off, 1/4, 1/8,
1/16, 1/32, and 1/64 notes. A point snaps to the nearest step. The lane is off
before its first point. It keeps the value of each point until the next point,
and it shows these values as steps.

Where the lane turns on, it takes the sound at that point. It then plays one
note length of that sound again and again. A faster value later in the same
run keeps the start and makes the slice shorter. Thus 1/8, then 1/16, then
1/32 makes the repeats faster. Off plays the live sound again.

Each repeat fades in over a few milliseconds, so it does not click. The
repeats go through the EQ, the pan, and the FX sends of the track.

### Repeat gate lane

The Repeat gate lane sets how much of each repeat you hear. The range is from
5% at the bottom of the row to 100% at the top, in 5% steps. The lane is at
100% before its first point. Like Repeat, it keeps each value until the next
point. Each repeat uses the value at its start.

At 25%, a repeat plays its first quarter and then fades out over 3 ms. Thus a
roll becomes short, strong plucks without volume points. At 100%, the repeats
play fully. The gate has no effect while Repeat is off.

### Pitch lane

The Pitch lane changes the playback speed of a track, for tape stops and
pitch falls. The range is from four octaves down (-48 st) at the bottom of the
row to two octaves up (+24 st) at the top. No change is at two thirds of the
height. A point snaps to whole semitones.

The lane works like a tape machine: pitch and speed change together. One
octave down plays at half speed. A fall to the bottom over half a bar slows
the sound until it stops.

The lane is at 0 before its first point. It then moves in straight lines, at
each sample, and adds to the pitch of each clip. A clip keeps its position and
length:

- When the lane slows the clip, the clip ends at its usual end.
- When the lane makes the clip faster, the sound ends where the sample ends,
  with the fade out of the clip.

Before the next clip on the track, draw the lane back to 0. The Repeat lane
repeats the pitched sound. The EQ, the drive, and the other processing of the
track come after the Pitch lane.

### FX return lanes

An FX return lane (FX 1 return to FX 4 return) changes the return level of one
bus during the song. It is one line across the arrangement:

- The bottom of the row is silence.
- The middle, at the dashed line, is the full return (100%).
- The top is 200%, about 6 dB louder.

While the lane has points, it replaces the Return knob of the bus. The level
changes at each sample, so it moves smoothly.

To throw a delay or a reverb:

1. Keep the return low.
2. Raise it on the last repeat of a roll, or at the end of a phrase.
3. Let the echoes continue.
4. Lower the return again.

The limiter of the bus still keeps the return below -1 dBFS.

## Playback and loop

- To play or pause, press Space.
- To select a loop, drag across the bar ruler.
- Playback uses the **Playback buffer target** in
  [Options](libraries-and-options.md#options). The default is 40 ms where the
  device supports it. The device controls the actual callback interval.
- If playback sounds wrong, for example a sudden loud hiss, press F12 while it
  occurs. Refer to [Audio captures](captures-and-limits.md#audio-captures).

## Sections

A song can have sections: named parts such as Intro or Break. Each section is a
bookmark. It shows as a colored line across the lanes, with its name on the bar
ruler. A section continues until the next section starts. Sections are saved
with the song. Agents set them with `set_sections`.

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
