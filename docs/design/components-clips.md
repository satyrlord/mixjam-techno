# Components: sounds, clips, and Mix view

## Sounds and clips

For sound rows and category chips, see
[Sounds panel](components-shell.md#sounds-panel).

### Clip drawing

Arrangement clips use flat category fills and names, with the waveform of the
clip under the name. A clip narrower than 48 points shows no name, and its
waveform fills it; a missing clip always shows its name. The waveform uses the
category color darkened to 55% and fills the band between the name and the
bottom edge of the clip. In a
20-point lane that band is too short. The waveform then fills the clip in a
lighter tone (82%), and the name stays readable over it.

The waveform shows the part of the sound that the clip plays. It starts at
the trim and ends where the sound ends, which can be before the end of the
clip. A reversed clip mirrors it, so any silence comes first. Fades shape it
to silence at their ends. Clips never repeat their sound, so the waveform
does not repeat. Pitch-lane varispeed is not drawn. The `rev` mark of a
reversed clip sits on a small plate of the clip fill. A clip with a missing
sound has no waveform. Only clips on screen paint a waveform. Each clip keeps
its shape until its sound, trim, length, fades, Reverse, tempo, or zoom
changes.

A clip of a Step synth sound that was saved with its pattern draws notes in
place of the waveform, in the same band and tones. It draws one bar per note,
2 to 4 points thick as the lane height sets. The pitch of each note maps over
at least an octave. A bar holds through the ties and the gate of its note, and
Reverse mirrors it. WAV clips never draw notes.

A selected clip can show a gain line. The gain line is a control indicator.
A clip with a missing source uses Raised with a one-point Coral outline, and a
Text-colored name that starts with Missing.

### Drag and placement

While a sound or clip is dragged over the lanes, clip ghosts at 60% opacity
show the destination bar and track. A drop puts every clip on a row, or does
nothing. If a group would put a clip on a track without a row, or outside the
timeline, no ghosts show. A Drag Label then names that track or reads Outside
the timeline, and the drop is canceled with a status message. Off the lanes,
a dragged sound shows a 150 by 30-point Drag Label with its name in Canvas
text.

A palette sound that is armed for click placement shows a tinted outline
where it will land. The selection of a clip, or Esc, ends click placement. A
click on an empty lane then only clears the selection. Enter on the focused
Sounds list places its current sound on the recording track at the playhead.

### Lane keys

The lane M/S/L/R letters get a short bar when their switch is on. The lanes
are one Tab stop with an Accent outline:

- Arrow keys choose a clip.
- Alt+arrows move the selection by a bar or a track.
- Ctrl+Alt+arrows move it by a sixteenth.
- Enter previews.
- Delete removes.
- Shift+F10 opens the menu of the selected clip under the clip.

Delete and Shift+F10 also work when no control has focus. Delete also removes
the selected clips while the Sound list has focus. Ctrl+A selects every
clip, and Ctrl+D copies the selection to the bars that follow. A right-click
on a clip shows Preview, Duplicate, Delete, and, for a missing sound, Resolve
manually.

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

If the gain handle covers another handle, the visible gain handle receives
the input. A drag on the rest of the clip moves it. Shift, Alt, or the right
button copies it. The right-click menu also holds the exact values: gain in
dB as in the Mixer, trim start in frames, length, fades with their curves, and
pitch in semitones and cents.

The arrangement has no clip strip under the lanes. Clips with available audio
show their name without a hover tooltip. Missing clips keep their metadata and
resolution guidance in a tooltip.

## Mix view

The Mix view is its own mode of the arrangement: the header of each used lane
widens into a mixer row, and choosing the view folds the lower panel so the
rows take the whole score. Leaving the view opens the panel again, unless it
was opened or folded in the meantime. The clips keep the rest of the lane as
an activity strip: the lanes fit the whole song (at least 16 bars), so the
score does not scroll sideways and Zoom rests. The fitted range keeps room for
the last section's name, up to half the lane width. The ruler numbers every
bar, or every second, fourth, or later bar to keep at least 28 points between
numbers, with more room for wider numbers. The lanes draw bar lines at least
6 points apart. Empty lanes keep their drop target. A row keeps the height of
the lane. When the rows overflow,
the viewport shows whole rows only. From the left, a row holds:

- the track number and record arm,
- a 7-point category mark in the colour of the track's first sound,
- the sounds (12 points, or 11 in 20-point lanes, left-aligned and elided):
  the first sound to play, then "+N" for the others, with the full list in
  the hint,
- M, S, L, and R,
- polarity,
- the level fader with its dB value and Unity,
- pan with Center (the fader and pan rails use the strip's Strip well and
  Strip fill: a dark well with a Muted fill on Studio and Aero, and a cream
  well with a dark fill on Vintage's ivory, so the level reads from the left
  on every strip; the pan handle keeps a 1.5-point ring of strip ink at rest
  and in use, so it shows on the cream well),
- the state text in 11 points, only for an exception: Muted, No output, Solo
  elsewhere, Pan automated, or a pan off center,
- the four send cells: each knob, and its amount in 10 points while it sends
  (a send at 0 rests as a thin ring until hover or focus),
- EQ.

A row at its defaults stays quiet. M, S, L, R, and polarity rest as Muted
marks on dark faces. Mute fills its face Coral, Solo fills its face Accent,
and an inverted polarity lights its face while on, and an output that is off
turns its letter to the warning lamp colour. Unity and Center have no face and
show their label only on hover or focus, since the dB value and the pan state
already show a moved value; they stay in the Tab order with their names. A row
that cannot be heard (muted, soloed elsewhere, or with no output) keeps its
switches clear and dims the rest to 45%.

The header column is the width of the score less 160 points, from 250 to
984 points. The content of the rows is 944 points wide; a wider column gives
its extra width to the sounds, and the columns after them shift right. The
header part of the ruler strip shows the column captions (Track, Out over L
and R, Level, Pan,
FX 1 to FX 4 over the send cells, EQ) in 10-point uppercase Muted text. If the
column is narrower than its rows, everything right of the 34-point number and
arm scrolls sideways. A 3-point bar with a Muted thumb at the foot of the
captions then shows the offset, and a drag on it changes the offset. While columns
remain to the right, the last 24 points of each row fade into the row's own
surface, so a word or control at the edge fades out instead of being cut, and
a caption that either edge would cut is left out.

The header part of the drop row under the last track shows a Muted "+" under
the track numbers and "New track" in the sounds' column. A song without tracks
shows its "No tracks" note instead.

The first switch to the Mix view in a session puts its shortcuts in the
status bar: a double-click resets a fader or knob, and Shift with the wheel scrolls
the columns. From the Mix view, a track's channel EQ window first opens beside
its row's number through Pan, or above or below the row when the window does
not fit beside it, so the row's level and pan stay in view.

A 70-point block follows the last row, and stays at the foot of the score
while more rows scroll. It holds two rows:

- The Master row: Master over "Stereo output", Bypass under the switches, the
  level, and Export tracks….
- The returns row: four groups that share the width of the columns, with a
  Strip slot rule between them. Each
  group holds the bus name and its effect, the return state ("Return 88%" or
  "Return automated"), the return level knob, Input, Limiter, and Edit. Input
  and Limiter rest as Muted labels with a lit lamp; off, the label turns to
  the warning lamp colour. A bus with the Empty effect returns silence, so its
  knob, Input, and Limiter dim to 45%; they keep their names, and Edit stays
  at full strength.

While the FX returns board shows, each group keeps its name, state, and Edit,
and reads "On the board below" in place of the knob and switches. Thus no
control has two names on screen.

The rows have the material of the Mixer. Vintage tiles the ivory strip
artwork for the track and Master rows, and the graphite return artwork for the
returns row. Studio and Aero fill the strip and return-strip surfaces. Row
text uses the strip ink, and each row has a 1-point strip-edge outline. The
track number keeps its arrangement position without its panel backing.
