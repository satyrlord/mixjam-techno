# Components: sounds and clips

## Sounds and clips

For sound rows and category chips, see
[Sounds panel](components-shell.md#sounds-panel).

### Clip drawing

Arrangement clips use flat category fills and names, with the waveform of the
clip under the name. A clip narrower than 48 points shows no name, and its
waveform fills it. A missing clip always shows its name. The waveform uses the
category color darkened to 55%. It fills the band between the name and the
bottom edge of the clip. In a 20-point lane that band is too short. The waveform
then fills the clip in a lighter tone (82%), and the name stays readable over
it.

The waveform shows the part of the sound that the clip plays. It starts at the
trim and ends where the sound ends, which can be before the end of the clip. A
reversed clip mirrors it, so any silence comes first. Fades shape it to silence
at their ends. Clips never repeat their sound, so the waveform does not repeat.
The waveform does not show Pitch-lane varispeed. The `rev` mark of a reversed
clip is on a small plate of the clip fill. A clip with a missing sound has no
waveform. Only clips on screen paint a waveform. Each clip keeps its shape until
its sound, trim, length, fades, Reverse, tempo, or zoom changes.

A clip of a Step synth sound with a saved pattern draws notes in place of the
waveform. It uses the same band and tones. It draws one bar per note, 2 to 4
points thick as the lane height sets. The pitch of each note maps over at least
an octave. A bar holds through the ties and the gate of its note, and Reverse
mirrors it. WAV clips never draw notes.

A selected clip, or a clip under the pointer, shows a gain line. The gain line
is a control indicator. A clip with a missing source uses Raised with a
one-point Coral outline, and a Text-colored name that starts with Missing.

### Drag and placement

If you drag a sound or clip over the lanes, clip ghosts at 60% opacity show the
destination bar and track. A drop puts every clip on a row, or does nothing. If
a group needs a clip on a track without a row, or outside the timeline, no
ghosts show. A Drag Label then names that track or reads Outside the timeline.
The drop cancels with a status message. Off the lanes, a dragged sound shows a
150 by 30-point Drag Label with its name in Canvas text.

A palette sound that you arm for click placement shows a tinted outline where it
will land. The selection of a clip, or Esc, ends click placement. A click on an
empty lane then only clears the selection. Enter on the focused Sounds list
places its current sound on the recording track at the playhead.

### Lane keys

Lane headers carry small square M and S switches that fill Coral and Accent when
on, and a small pan knob. The lanes are one Tab stop with an Accent outline:

- Arrow keys choose a clip.
- Alt+arrows move the selection by a bar or a track.
- Ctrl+Alt+arrows move it by a sixteenth.
- Enter previews.
- Delete removes.
- Shift+F10 opens the menu of the selected clip under the clip.

Delete and Shift+F10 also work when no control has focus. Delete also removes
the selected clips while the Sound list has focus. Ctrl+A selects every clip,
and Ctrl+D copies the selection to the bars that follow. A right-click on a clip
shows Preview, Duplicate (Ctrl+D), and Delete (Delete). For a missing sound,
Resolve manually comes first.

### Clip handles

The user edits a clip on the clip itself, as in other DAWs:

- The side edges trim the clip. The right edge sets its length. The left edge
  moves its start and trim together, so the sound stays where it plays. The
  edges snap to bars, or to sixteenths with Ctrl.
- Small dark squares on the top edge set the fade in and fade out. They snap
  to sixteenths, or to ticks with Ctrl.
- A small handle on the gain line, in the middle of the visible part of the
  clip, sets the gain. Each 4 points up or down is 1 dB, or 0.1 dB with Ctrl,
  from +6 dB to silence. A label at the pointer shows the value, and the
  release makes one undo step.

If the gain handle covers another handle, the visible gain handle receives the
input. A drag on the rest of the clip moves it. Shift, Alt, or the right button
copies it. The right-click menu also holds the exact values:

- gain in dB, as in the Mixer,
- trim start in frames,
- length in bars,
- fade in and fade out, each with its curve,
- pitch in semitones and cents,
- Reverse.

The Inspector clip pages edit the same values with the same number, gain, and
curve fields. See [Inspector](components-score.md#inspector).

Clips with available audio show their name without a hover tooltip. Missing
clips keep their metadata and resolution guidance in a tooltip.
