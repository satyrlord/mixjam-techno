# Song-writing methods and earlier songs

## Song-writing methods

The methods accept whole track numbers from 1 to 64 and whole FX bus numbers
from 1 to 4. An invalid destination fails before it changes the song. Loop
ranges must advance in time. An explicit tile size must be finite, positive,
and at least one tick long. Automatic end fades use the energy of both
channels. Channels of opposite polarity do not make an audible ending count
as silence.

`song.save(path)` stages the song and its `.meta.json` before it replaces either
file. If the replacement of the song fails, it restores the earlier metadata.
Use one writer per song path.

`song.automate(track, lane, points)` accepts the lanes `volume`, `filter`,
`pan`, `repeat`, `repeatgate`, `pitch`, `compression`, and `send1` to `send4`.
Each bar is a finite number from 1 to 4097. Each value is a finite number in
its control's range. A rejected batch leaves the earlier points unchanged.

- A `pan` lane is from -1 (left) to 1 (right). While it has points, it replaces
  the pan knob of the lane. Before its first point, it rests at the knob.
- A `repeat` lane takes 0 (off) or 4, 8, 16, 32, or 64 for 1/4 to 1/64 repeats.
  It holds each value until its next point. It is off before its first point.
  `song.automate(3, 'repeat', [(5, 8), (5.25, 16), (5.375, 32), (5.5, 0)])`
  rolls the slice from bar 5.
- A `repeatgate` lane is `[(bar, percent), ...]`. It shortens each Repeat
  retrigger to 5 to 100 percent of its slice. After that, the retrigger fades
  out over 3 ms. The lane steps like `repeat` and is at 100 before its first
  point. It acts only while `repeat` is on, so it can end at any value.
- A `pitch` lane is `[(bar, cents), ...]`, from -4800 to 2400 cents. It bends
  the playback speed of the lane. It works like varispeed on a tape. Pitch and
  speed move together. `song.automate(3, 'pitch', [(8.5, 0), (9, -4800), (9.0625,
  0)])` is a tape-stop over the second half of bar 8.

A `pitch` lane is 0 before its first point, so it needs no extra point at 0
there. It moves in straight lines in cents. It adds to the own pitch of each
clip. It runs before the Repeat lane. It must be back at 0 before a later clip
on the lane. A clip keeps its length. If the lane slows a clip, the clip ends at
its end. If the lane speeds it up, its sound ends where its source runs out. The
mechanical gate follows this speed clock. It does so when it checks the source
duration, the silent gaps, and the source window at the end of a clip. This
includes reversal.

A `compression` lane sets the compressor mix from 0 to 100 percent. The
writer adds points at 100 before the first point if needed. The mix changes
in a straight line between points. Return it to 100 by the start of any
later clip. The lane has an effect only when the compressor is on.

`song.automate_return(bus, [(bar, value), ...])` writes the return lane of an
FX bus. Each bar is from 1 to 4097. The value is from 0 to 2 (1 is the full
return, and 2 is about +6 dB).
While the lane has points, it replaces the `returnLevel` of the bus. Use it to
throw a delay or reverb at the end of a phrase.

`song.clip(..., reverse=True)` plays the slice of the clip backwards within the
clip. Fades, pitch, and gain stay in the own time of the clip.

`song.drive(track, amount=12, character=25, mix=50, focus=100, output=0)`
enables the drive of a lane. The drive runs after the EQ of the lane and
before its compressor. It adds harmonics above `focus` Hz and keeps the power
of the lane. Small speakers can therefore play a held bass. Set `focus` to about
twice the lowest note of the lane to keep the fundamental clean. `on=False` at the
default settings removes the drive, as the app saves it.

The ranges of the drive are:

- `amount`: 0 to 24 dB
- `character` and `mix`: 0 to 100 percent
- `focus`: 20 to 2000 Hz
- `output`: -12 to 12 dB

The library and the application reject a lane setting outside the range of its
control. The
ranges of all lane, clip, and automation controls are in
[Control ranges](../../.github/skills/song-review/REFERENCE-CONTROLS.md#control-ranges).

## Earlier songs

The song tools accept formats 18 to 26. `render.py` does not pass sample folders
to the application. `measure.py` checks linked paths from its working directory.
For an earlier song or a library-relative WAV, make a working copy in the
[current song format](../../docs/SONG-FORMAT.md).

1. Use the native `convert` command with the necessary `--samples` folders and
   an isolated `MIXJAM_TECHNO_DATA_DIR`.
2. Copy any `.meta.json` beside the new song with the matching name.
3. Check for missing sources and unconfirmed conflicts before you use its
   renders.
4. Do not replace the original song.
5. Keep the files that the converted copy needs in the data folder.
