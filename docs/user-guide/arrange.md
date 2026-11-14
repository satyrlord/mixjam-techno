# Arrange

Each lane's header shows, from the left, the track number (click it to record
on that track), the lane's name, Mute, Solo, the left and right outputs, and a
short level fader. The name is the sound family of the lane's first clip, such
as "deep chord" for "deep chord 3". The fader sets the same track gain as the
Mixer: drag it (Shift for fine steps), use the wheel or the arrow keys, and
double-click it for unity.

- Filter sounds by library, category, or name. Double-click a sound to preview it.
- Drag a sound onto a track. Hold Ctrl for sixteenth-note positions. The
  arrangement and mixer omit unused tracks.
- Hold Shift to select clips. Drag an empty area to select a group. Hold Shift
  or Alt while dragging a clip to copy it. A ghost shows where the clips land.
  Every clip of a group must land on a shown track or the Add row. If one would
  land on an unused track, a label names that track and the drop does nothing.
  Use Delete, Undo, and Redo for edits.
- Drag a clip's side edges to trim it. Drag its top corner handles to set fades,
  or the small handle in its middle up or down to set gain in 1 dB steps (0.1 dB
  with Ctrl). Each completed drag is one undo step. Right-click a clip, or
  select it and press Shift+F10, for exact gain,
  trim start, length, fades in beats with their curves (Linear, Equal power, or
  Exponential), pitch in semitones and cents, and Reverse. The fade out ends
  where the clip's sound ends, which can be before the clip's end, so a phrase
  that stops abruptly can die away instead. Pitch keeps the clip's length and
  timing. The arrangement shades each fade over its clip.
- Reverse plays a clip backwards: the same stretch of its sound, from the trim
  start over the clip's length, runs from its end to its start within the
  clip, for reversed cymbals, swells, and risers. The fades, gain, and pitch
  stay where they are, so the fade in still opens the clip and the fade out
  still closes it. A reversed clip whose sound is shorter than the clip
  starts with silence and ends with the clip. The clip reads "rev" in its
  corner. Each clip draws the waveform of the part of its sound it plays,
  reversed and faded as you hear it. The trim and length still describe the forward stretch: dragging
  the left edge trims what is heard at the clip's end. A clip of a Step
  synth sequence draws its notes instead: each note is a short bar, higher
  notes higher up, held to the next note (or for one sixteenth with the 1/16
  cut). Save sequence keeps the pattern beside the WAV for this; a Step synth
  sound without one shows its waveform.
- Choose a lane in Automation above the arrangement to draw it on every track.
  The menu lists all 14 lanes in three groups. On every track: Volume (dB
  over the fader), Pan, Filter (low-pass below the middle, high-pass above
  it, open in the middle; it crosses between the two without a click),
  Pitch, Repeat, and Repeat gate. Send amount, on every track: FX 1 to FX 4
  send (the amount, which replaces the send knob while the lane has points),
  each named with the effect its bus holds. Return level, one line across
  the song: FX 1 to FX 4 return. The current lane is marked, and each lane
  says how much it holds: how many tracks have points on it ("2 tracks"),
  or how many points a return lane has ("4 points"). Click a lane to add a
  point, drag a point to move it, and double-click or right-click a point to
  delete it. Points snap to 1/16 bar; hold Ctrl for single ticks. Between
  points the value moves in a straight line, except on the Repeat and Repeat
  gate lanes, which step. A return lane is one lane for a bus's return
  across the arrangement. Choose Off to edit clips again.
- The Pan lane moves a track across the stereo field: the top of the row is
  hard right, the bottom hard left, and the middle the center. A point reads
  as L 40, C, or R 100. While the lane has points it replaces the track's pan
  knob, which the Mixer then marks as Pan automated. The pan moves at every
  sample, so a sweep on each 16th note of a ratchet sounds continuous.
- The Repeat lane chops a track's sound into repeats, for rolls and ratchets.
  Its row has six steps, Off at the bottom, then 1/4, 1/8, 1/16, 1/32, and
  1/64 notes, and a point snaps to the nearest one. The lane is off before its
  first point and holds each point's value until the next, drawn as steps.
  Where it turns on, it takes the sound from that point and plays one note's
  length of it again and again; a faster value later in the same run keeps
  the start and shortens the slice, so 1/8, then 1/16, then 1/32 speeds the
  repeats up. Off brings the live sound back. Each repeat fades in over a few
  milliseconds, so it does not click, and the repeats go through the track's
  EQ, pan, and FX sends.
- The Repeat gate lane sets how much of each repeat sounds, from 5% at the
  bottom of the row to 100% at the top, in 5% steps. It is at 100% before its
  first point and holds each value until the next, like Repeat; each repeat
  takes the value held where it starts. At 25% a repeat plays its first
  quarter and then fades out over 3 ms, so a roll becomes short, punchy
  plucks without drawing volume points; at 100% the repeats play whole. The
  gate does nothing while Repeat is off.
- The Pitch lane bends a track's playback speed, for tape-stops and pitch
  falls. Its row runs from four octaves down (-48 st) at the bottom to two
  octaves up (+24 st) at the top, with no change two thirds of the way
  up, and a point snaps to whole semitones. It works like a tape machine:
  pitch and speed move together, so an octave down plays at half speed, and
  a fall to the bottom over half a bar slows the sound to a stop. The lane is
  at 0 before its first point, then moves in straight lines, at every sample,
  and adds to each clip's own pitch. A clip keeps its place and length: when
  slowed it ends at its usual end, and when sped up its sound ends where the
  sample runs out, with its fade out. Draw the lane back to 0 before the
  track's next clip. The Repeat lane repeats the pitched sound, and the EQ,
  drive, and the rest of the track come after it.
- An FX return lane (FX 1 return to FX 4 return in the Automation menu) moves
  one bus's return level over the song, as one line across the arrangement:
  silence at the bottom, the full return (100%, the dashed line) in the
  middle, and 200%, about 6 dB louder, at the top. While it has points it
  replaces the bus's Return knob, and it moves at every sample, so it glides
  without steps. Use it to throw a delay or reverb: hold the return low, raise
  it on the last repeat of a roll or the end of a phrase, and let the echoes
  ring out before you bring it back down. The bus's limiter still holds the
  return under -1 dBFS.
- Press Space to play or pause. Drag across the bar ruler to select a loop.
- Playback uses the output-buffer target in [Options](libraries-and-options.md#options), 40 ms by
  default where supported. The actual callback interval remains device-controlled.
- If playback sounds wrong, such as a sudden loud hiss, press F12 while it
  does. See [Audio captures](captures-and-limits.md#audio-captures).
- A song can have sections: named parts such as Intro or Break. Each one is a
  bookmark, a colored line across the lanes with its name on the bar ruler, and
  lasts until the next one starts. With sections shown, the Loop button loops
  the section at the playhead; without them, it loops the whole song.
  Before the first bookmark, it loops the lead-in. A final bookmark beyond the
  song's end loops one bar, bounded by the timeline.
  Hiding bookmarks cancels an unfinished name edit.
  Double-click the bar ruler to add a section at that bar; type its name and
  press Enter, or Escape to keep the old one. Double-click a name to rename it,
  and right-click a name to rename or delete it. Turn Sections beside Follow
  off to hide the bookmarks, here and in the song overview. The application keeps that choice. Sections are
  saved with the song; agents set them with `set_sections`.
- Drag Zoom, or turn the mouse wheel over it, to zoom the timeline in or out.
- Use Follow beside Zoom to keep the playhead visible. Turn it off to browse
  the timeline independently during playback.
- Drag the divider above the lower panel to give it more or less room.
- Choose Mixer, Mastering, FX returns, or Step synth with the tabs at
  the lower panel's top left. The panel retains its resized height; opening a
  board raises a shorter panel to fit its controls, and in a short window the
  board scrolls. The Mixer tool-rail button switches the score between its
  Arrange and Mix views. The Mastering, FX returns, and Step synth buttons open
  their tab, and fold the lower panel when that board already shows.
