# Automation

To draw automation, choose a lane in the **Automation** menu above the
arrangement. The lane then shows on all tracks. The menu has 15 lanes in three
groups:

| Group | Lanes |
| --- | --- |
| On every track | Volume (dB over the fader), Pan, Filter, Compression, Pitch, Repeat, Repeat gate |
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

## Filter and send lanes

The Filter lane is a low-pass filter below the middle of the row and a
high-pass filter above it. In the middle, the filter is open. The lane moves
from low-pass to high-pass without a click.

An FX send lane sets the amount that the track sends to that bus. While the
lane has points, it replaces the send knob of the track.

## Pan lane

The Pan lane moves a track across the stereo field. The top of the row is
hard right, the bottom is hard left, and the middle is the center. A point
shows its value as L 40, C, or R 100. While the lane has points, it replaces
the pan knob of the track, and the Mixer shows Pan automated. The pan changes
at each sample. Thus a sweep on each 16th note of a ratchet sounds continuous.

## Repeat lane

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

## Repeat gate lane

The Repeat gate lane sets how much of each repeat you hear. The range is from
5% at the bottom of the row to 100% at the top, in 5% steps. The lane is at
100% before its first point. Like Repeat, it keeps each value until the next
point. Each repeat uses the value at its start.

At 25%, a repeat plays its first quarter and then fades out over 3 ms. Thus a
roll becomes short, strong plucks without volume points. At 100%, the repeats
play fully. The gate has no effect while Repeat is off.

## Pitch lane

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

## Compression lane

The Compression lane sets how much of the compressor of a track is mixed in.
At the bottom of the row, 0% gives the sound before compression. At the top,
100% gives the full effect of the compressor settings. Points snap in 5% steps.

Use it when one phrase of a compressed track must keep its dynamics. Draw the
lane down to 0% just before the phrase and back up to 100% just after it.

The lane holds its first and last values outside its points and moves in
straight lines between points. Without points, the compressor applies in
full. The lane has no effect while the track compressor is off.

## FX return lanes

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
