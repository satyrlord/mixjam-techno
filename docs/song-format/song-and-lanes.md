# Song and lanes

## Song

`song` holds `title`, `author`, `bpm`, `masterGain`, and `loopRegion`.

- `bpm` is 120, 125, 130, 135, 140, 160, or 180.
- `masterGain` is an amplitude from 0 to 4.
- `loopRegion` is `null`, or `[start, end]` in ticks. The start is before the
  end, and the end is within the timeline.

From version 19, `song` can also hold `sections`. This is a list of
`{"startTick", "name"}` in strictly increasing `startTick` order. Each
`startTick` is within the timeline. A section lasts until the next one starts,
or to the end of the song. A name is 1 to 48 characters on one line, without
leading or trailing spaces. If the last section starts at or after the end of
the song, its loop spans one bar, bounded by the timeline. A song without
sections omits the key. Earlier formats reject the key, even when its list is
empty. Sections do not change the sound.

**Save** names a new song after its file. **Save as** renames a song whose
title is its earlier file name. An imported title stays.

## Lanes

`lanes` has exactly 64 entries in track order. From version 18, a lane holds
these keys:

- `gain`, an amplitude from 0 to 4 (1 is 0 dB)
- `polarityInverted`, `pan`, `leftEnabled`, `rightEnabled`, `muted`, and `solo`
- `sends`, four amounts from 0 to 1
- `placements`

Earlier versions use the fields in the [version history](version-history.md).
A lane from version 18 can also hold the keys that follow. Each key is
described in its own section.

### `automation`

`automation` is a JSON object. It can hold these keys:

- `volume`, `sends` (four lists), and `filter`
- `pan`, from version 20
- `repeat`, from version 21
- `repeatGate`, from version 23
- `pitch`, from version 24

Each list holds `{"tick", "value"}` points in strictly increasing tick order
within the timeline. Automation points do not extend the duration of the song.

- Volume is from -60 to 6 dB.
- A send is from 0 to 1.
- The filter is from -1 to 1.
- The pan is from -1 (hard left) through 0 (center) to 1 (hard right).

A send or pan list with points replaces the send or `pan` of the lane for
playback and export. The pan uses the same balance law, after the width.
Earlier versions reject `pan`, even as an empty list.

#### `repeat`

`repeat` is the repeat division of the lane. The value 0 is off. The values 4,
8, 16, 32, and 64 are 1/4, 1/8, 1/16, 1/32, and 1/64 notes at the song tempo.
No other value is valid.

Unlike the other lists, `repeat` steps. The lane is off before its first
point. Each point value holds until the next point, with no interpolation.

- If the value changes from 0 to a division at tick T, the lane captures its
  own sound from T.
- Each division replays the slice that starts at T and is one division long.
- A later point with another division in the same run keeps T. A faster
  division makes the slice shorter.
- A point with the value that is already held changes nothing.
- At 0, the live sound returns. The next turn on captures at its own tick.

The repeats run on the sound of the clips. They run before the EQ, drive,
compressor, width, other automation, fader, pan, ducker, and sends. A repeat
can continue past its source clip until the song ends. Versions 20 and
earlier reject `repeat`, even as an empty list.

#### `repeatGate`

`repeatGate` is the share of each repeat that sounds, from 5 to 100 percent of
its division. It steps like `repeat`. It is 100 before its first point, and
each point value holds until the next point.

Each retrigger takes the value held at its own tick. The first retrigger, at
T, does the same. With G percent, a retrigger plays the first G percent of the
frames of its division, rounded to a frame. Then it falls to silence over the
same 132-frame raised-cosine ramp that crossfades the retriggers. It stays
silent until the next retrigger.

The gate also weighs the outgoing tail of a repeat in the next crossfade and
when the lane turns off. At 100, the repeats are exactly as without a gate.
While `repeat` is off, the gate does nothing. Versions 22 and earlier reject
`repeatGate`, even as an empty list.

#### `pitch`

`pitch` is the playback speed of the lane in cents. It is from -4800 (four
octaves down, 1/16 speed) to 2400 (two octaves up, 4x speed).

It is varispeed, as on a tape machine. At each frame, the clips of the lane
read their sources at `2^(cents / 1200)` times their normal rate. Pitch and
speed move together, so a fall to -4800 slows the sound to a tape-stop.

- The lane is 0 before its first point.
- It holds its last value after its last point.
- Between points, it moves in a straight line in cents, at every frame.
- The read position is the integral of that speed from the start of each
  placement, so it never jumps.
- For a reversed placement, the read moves backwards at the same speed. It
  reaches `offsetFrames` at the end of the placement.

The lane adds to the `pitchCents` of each placement. The pitch of the
placement keeps its length. The pitch of the lane then changes the speed of
that pitched sound, on top of the tempo match to the song.

A placement keeps its length in the arrangement. If the lane slows a
placement, the placement ends before its source ends. If the lane speeds it
up, its sound ends where the source runs out, and its fade out ends there.
Fades stay in the own time of the placement.

The lane acts where the placements read their sources. It acts before
`repeat`, so the repeats replay the pitched sound. It also acts before the EQ
and everything after the EQ. Versions 23 and earlier reject `pitch`, even as
an empty list.

### `eq`

`eq` is the channel EQ: `{"on", "lowCut", "lowHz", "lowDb", "midHz", "midDb",
"midQ", "highHz", "highDb", "highCut"}`. Every key is required. The second
peak adds `peak2Hz`, `peak2Db`, and `peak2Q`. A lane saves each of these keys
only when it differs from its default (250 Hz, 0 dB, and 1).

The bands run in this order:

| Band | Keys | Range |
|---|---|---|
| Low cut, a 12 dB/octave high-pass | `lowCut` | 20 to 2000 Hz (20 is off) |
| Low shelf | `lowHz` | 40 to 500 Hz |
| First peak | `midHz`, `midQ` | 150 to 10,000 Hz, 0.3 to 8 |
| Second peak | `peak2Hz`, `peak2Q` | 40 to 16,000 Hz, 0.3 to 8 |
| High shelf | `highHz` | 1500 to 16,000 Hz |
| High cut, a 12 dB/octave low-pass | `highCut` | 1000 to 20,000 Hz (20,000 is off) |

Each gain (`lowDb`, `midDb`, `peak2Db`, and `highDb`) is from -18 to 18 dB. A
gain of 0 is off. A lane whose EQ is off at the default settings omits `eq`.

### `drive`

`drive` is the lane drive, from version 22. It runs after the EQ and before
the compressor. Every key is required: `{"on", "amount", "character", "mix",
"focus", "output"}`.

- `amount`, from 0 to 24 dB, is how hard a copy of the lane is driven into the
  saturation curve.
- `character`, from 0 to 100 percent, goes from a symmetric curve (odd
  harmonics) to a one-sided curve (mostly even harmonics).
- `mix`, from 0 to 100 percent, is how much of the change in the copy is added
  to the clean lane.
- `focus`, from 20 to 2000 Hz, is the corner of a 24 dB/octave high-pass on
  that change. Below it, the lane stays clean.
- `output`, from -12 to 12 dB, is the trim after the drive.

The drive matches the result to the input power. Then it applies the `output`
trim. A `mix` of 0 leaves the lane unchanged when `output` is 0 dB. A lane
whose drive is off at the default settings (12 dB, 25%, 50%, 100 Hz, 0 dB)
omits `drive`. Versions 21 and earlier reject the key.

### `compressor`

`compressor` is the lane compressor after the drive. Every key is required:
`{"on", "threshold", "ratio", "attack", "release", "makeup"}`.

- `threshold` is from -60 to 0 dBFS.
- `ratio` is from 1 to 20.
- `attack` is from 0.1 to 100 ms.
- `release` is from 10 to 1000 ms.
- `makeup` is from 0 to 24 dB.

A lane whose compressor is off at the default settings (-20 dBFS, 4:1, 10 ms,
100 ms, 0 dB) omits `compressor`.

### `width`

`width` is the stereo width of the lane in percent, before its pan. It is from
0 (mono) to 200. A lane at 100% omits it.

### `ducker`

`ducker` lowers the lane while another lane, its key, sounds. Every key is
required: `{"on", "key", "threshold", "depth", "attack", "release"}`.

- `key` is a track from 1 to 64. While the ducker is on, it is not the track of
  the lane itself.
- `threshold` is from -60 to 0 dBFS. It is measured on the key lane after its
  fader and pan, and before the master gain.
- `depth` is from 0 to 24 dB.
- `attack` is from 0.1 to 50 ms.
- `release` is from 10 to 1000 ms.

The key tracks of all duckers, pedals, and effects are limited together. See
[FX buses](master-fx-and-placements.md#fx-buses). A lane whose ducker is off
at the default settings omits `ducker`.
