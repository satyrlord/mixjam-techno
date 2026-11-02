# Song format

A `.mixjam` song is JSON. Its `formatVersion` selects the reader. Version 24 is
the current song format, and it is the only version that the application
writes. Versions 8 to 23 open and migrate. Version 7 is the earlier MixJam
project format, which is import-only: **Save** never replaces a version-7 file.
`src/mixjam/song.rs` owns the format; its tests cover each version.

## Contents

- [Envelope](#envelope)
- [Song and lanes](#song-and-lanes)
- [Master pedalboard](#master-pedalboard)
- [FX buses](#fx-buses)
- [Placements and samples](#placements-and-samples)
- [Version history](#version-history)

## Envelope

Every version from 8 uses strict camelCase JSON and rejects unknown keys. A
document holds `formatVersion`, `appVersion`, `createdAt`, `modifiedAt`, `song`,
`lanes`, `samples`, `missingSamples`, `sampleConflicts`, `importNotices`,
`sourceNotices`, and `libraries`. From version 11 it also holds `master`, and
from version 13 `fxBuses`. `sourceSettings` holds, as text, the settings that
the original product saved with an imported song; a song without such settings
omits it. Every other key is required. Saving to an existing file of version 8
to 24 keeps its `createdAt`.
Master settings reject duplicate fields, including nested fields and escaped
spellings of the same key. Versions 11 to 15 perform this check before renaming
the earlier Booster keys during migration.

Ticks are 256 per bar. A song links each WAV file by path; it is not a
portable audio bundle.

## Song and lanes

`song` holds `title`, `author`, `bpm` (120, 125, 130, 135, 140, 160, or 180),
`masterGain`, and `loopRegion`. From version 19 it may also hold `sections`: a
list of `{"startTick", "name"}` in strictly increasing `startTick` order, each
within the timeline. A section lasts until the next one starts, or to the end
of the song. A name is 1 to 48 characters on one line, without leading or
trailing spaces. If the final section starts at or beyond the song's end, its
loop spans one bar, bounded by the timeline. A song without sections omits
the key. Earlier formats reject the key even when its list is empty. Sections do not change
the sound. Save names a new song after its file, and Save
As renames a song whose title is its earlier file name; an imported title stays.

`lanes` has exactly 64 entries in track order. From version 18, a lane holds
`gain` (amplitude from 0 to 4; 1 is 0 dB), `polarityInverted`, `pan`,
`leftEnabled`, `rightEnabled`, `muted`, `solo`, `sends` (four amounts from 0
to 1), and `placements`. Earlier versions use the fields listed in the version
history below. A lane from version 18 may also hold:

- `automation`, a JSON object with any of `volume`, `sends` (four lists), `filter`,
  from version 20 `pan`, from version 21 `repeat`, from version 23
  `repeatGate`, and from version 24 `pitch`: lists of `{"tick", "value"}`
  points in strictly increasing tick order within the timeline.
  Volume is from -60 to 6 dB, a send from 0 to 1, the filter from -1 to 1, and
  the pan from -1 (hard left) through 0 (center) to 1 (hard right). A send or
  pan list with points replaces the lane's send or `pan` for playback and
  export; the pan uses the same balance law, after the width. Earlier versions
  reject `pan`, even as an empty list.
  `repeat` is the lane's repeat division: 0 (off), or 4, 8, 16, 32, or 64 for
  1/4, 1/8, 1/16, 1/32, or 1/64 notes at the song tempo; no other value is
  valid. Unlike the other lists it steps: the lane is off before its first
  point and holds each point's value until the next point, with no
  interpolation. Where the value turns from 0 to a division at tick T, the
  lane captures its own sound from T, and each division replays the slice
  that starts at T and is one division long. A later point with another
  division in the same run keeps T, so a faster division shortens the slice.
  A point with the value already held changes nothing. At 0 the live sound
  returns, and the next turn on captures at its own tick. The repeats run on
  the clips' sound before the EQ, drive, compressor, width, other automation,
  fader, pan, ducker, and sends. Versions 20 and earlier reject `repeat`, even as an
  empty list. Repeat can continue past its source clip until the song ends.
  `repeatGate` is the share of each repeat that sounds, from 5 to 100 percent
  of its division. It steps like `repeat`: it is 100 before its first point
  and holds each point's value until the next point. Each retrigger, the
  first at T included, takes the value held at its own tick: with G percent,
  it plays the first G percent of its division's frames (rounded to a frame),
  then falls to silence over the same 132-frame raised-cosine ramp that
  crossfades the retriggers, and stays silent until the next retrigger. The
  gate also weighs a repeat's outgoing tail in the next crossfade and when
  the lane turns off. At 100 the repeats are exactly as without a gate, and
  while `repeat` is off the gate does nothing. Versions 22 and earlier reject
  `repeatGate`, even as an empty list.
  `pitch` is the lane's playback speed in cents, from -4800 (four octaves
  down, 1/16 speed) to 2400 (two octaves up, 4x speed). It is varispeed, as
  on a tape machine: at each frame the lane's clips read their sources at
  `2^(cents / 1200)` times their normal rate, so pitch and speed move
  together and a fall to -4800 slows the sound to a tape-stop. It is 0
  before its first point, holds its last value after its last point, and
  moves in a straight line in cents between points, at every frame. The read
  position is the integral of that speed from each placement's start, so it
  never jumps; reversed, the read moves backwards at the same speed and
  reaches `offsetFrames` at the placement's end. The lane adds to each
  placement's `pitchCents`: the placement's pitch keeps its length, and the
  lane's then changes the speed of that pitched sound, on top of the tempo
  match to the song. A placement keeps its length in the arrangement: slowed,
  it ends at its end before its source does; sped up, its sound ends where
  the source runs out, and its fade out ends there. Fades stay in the
  placement's own time. The lane acts where the placements read their
  sources, before `repeat`, so the repeats replay the pitched sound, and
  before the EQ and everything after it. Versions 23 and earlier reject
  `pitch`, even as an empty list.
  Automation points do not extend the song's duration.
- `eq`, the channel EQ: `{"on", "lowCut", "lowHz", "lowDb", "midHz", "midDb",
  "midQ", "highHz", "highDb", "highCut"}`, every key required, and the second
  peak's `peak2Hz`, `peak2Db`, and `peak2Q`, each saved only when it differs
  from its default (250 Hz, 0 dB, and 1). `lowCut`, a 12 dB/octave high-pass,
  runs from 20 to 2000 Hz, where 20 is off; the low shelf's `lowHz` from 40 to
  500 Hz; the first peak's `midHz` from 150 to 10,000 Hz and its `midQ` from
  0.3 to 8; the second peak's `peak2Hz` from 40 to 16,000 Hz and its `peak2Q`
  from 0.3 to 8; the high shelf's `highHz` from 1500 to 16,000 Hz; each gain
  from -18 to 18 dB, where 0 is off; and `highCut`, a 12 dB/octave low-pass,
  from 1000 to 20,000 Hz, where 20,000 is off. The bands run in that order. A
  lane whose EQ is off at the default settings omits `eq`.
- `drive`, from version 22, the lane drive after the EQ and before the
  compressor: `{"on", "amount", "character", "mix", "focus", "output"}`, every
  key required. `amount` runs from 0 to 24 dB, how hard a copy of the lane is
  driven into the saturation curve; `character` from 0 to 100 percent, from a
  symmetric curve (odd harmonics) to a one-sided one (mostly even harmonics);
  `mix` from 0 to 100 percent, how much of the copy's change is added to the
  clean lane; `focus` from 20 to 2000 Hz, the corner of a 24 dB/octave
  high-pass on that change, below which the lane stays clean; and `output`
  from -12 to 12 dB. The drive matches the result to the input power, then
  applies the `output` trim. A `mix` of 0 leaves the lane unchanged when
  `output` is 0 dB. A lane whose drive is off at the default settings (12 dB, 25%, 50%,
  100 Hz, 0 dB) omits `drive`; versions 21 and earlier reject the key.
- `compressor`, the lane compressor after the drive: `{"on", "threshold",
  "ratio", "attack", "release", "makeup"}`, every key required. `threshold`
  runs from -60 to 0 dBFS, `ratio` from 1 to 20, `attack` from 0.1 to
  100 ms, `release` from 10 to 1000 ms, and `makeup` from 0 to 24 dB. A lane
  whose compressor is off at the default settings (-20 dBFS, 4:1, 10 ms,
  100 ms, 0 dB) omits `compressor`.
- `width`, the lane's stereo width in percent before its pan, from 0 (mono)
  to 200. A lane at 100% omits it.
- `ducker`, which lowers the lane while another lane, its key, sounds:
  `{"on", "key", "threshold", "depth", "attack", "release"}`, every key
  required. `key` is a track from 1 to 64, another than the lane's own while
  the ducker is on; `threshold` runs from -60 to 0 dBFS, measured on the key
  lane after its fader and pan and before the master gain; `depth` from 0 to
  24 dB, `attack` from 0.1 to 50 ms, and `release` from 10 to 1000 ms. The
  duckers that are on use at most 8 key tracks. A lane whose ducker is off at
  the default settings omits `ducker`.

## Master pedalboard

`master` holds `trim`, the gain before the first pedal, from -24 to 24 dB;
`order`, the signal path from the mixer to the output; one record per pedal
with `on` and its settings; and, when it is true, `bypass`, which skips the
whole board, the trim included, for playback and export.

`order` names each pedal once and ends with `limiter`. The pedal keys of
versions 16 to 24 are:

| Key | Settings |
|---|---|
| `boosterEq` | ten `bands` from 0 to 100 (50 is flat) |
| `boosterCompressor` | `drive`, `speed`, and `gain`, each from 0 to 1000, and `key` |
| `boosterWide` | `spread` from 0 to 1000 |
| `softClip` | `amount` from 0 to 6 dB and `ceiling` from -6 to 0 dBFS |
| `tube` | `drive` from 0 to 10 and `mix` from 0 to 100 percent |
| `trimEq` | `highPass` from 10 to 40 Hz, and `mud` and `harsh` from -3 to 0 dB |
| `busCompressor` | `threshold` from -30 to 0 dBFS, `ratio` from 1.5 to 10, `attack` from 0.1 to 100 ms, `release` from 50 to 1200 ms, and `key` |
| `maximizer` | `boost` from 0 to 25 percent |
| `liftEq` | `low` and `air` from 0 to 2 dB |
| `tape` | `drive` from 0 to 10 and `speed`, 15 or 30 inches per second |
| `imaging` | `width` from 60 to 140 percent and `monoBelow` from 60 to 300 Hz |
| `multiband` | `low`, `mid`, and `high` from 0 to 100 percent, and `key` |
| `limiter` | `gain` from 0 to 12 dB, `ceiling` from -3 to 0 dBTP, and `key` |

`key`, from version 17, is the track from 1 to 64 that the dynamics pedal
listens to, or 0 for its own input. A pedal whose key is 0 omits it.

Versions 11 to 15 name the three Booster pedals with earlier keys, in the
record and in `order`; `BOOSTER_KEYS` in `src/mixjam/song.rs` lists both key
sets. Those versions open with the same settings, and they reject the
version-16 keys. Version 16 rejects the earlier keys. Out-of-range or
non-finite values, unknown pedals or keys, and an order that misses or repeats
a pedal reject.

## FX buses

`fxBuses` holds exactly four buses in order, each with `module`, `powered`,
`returnLevel` from 0 to 1, and `limiter`. From version 23 a bus may also hold
`returnAutomation`, its return lane: a list of `{"tick", "value"}` points in
strictly increasing tick order within the timeline, each value from 0 to 2,
where 1 is the full return and 2 about +6 dB. A bus whose lane has no points
omits the key. While the lane has points it replaces `returnLevel` for
playback, export, and stems; like the volume lane it holds its first value
before its first point and its last after its last point, and between two
points it moves in a straight line at every frame. The bus's safety limiter
follows it. Versions 22 and earlier reject the key, even as an empty list.
A `module` is `{"type": "empty"}`, or
`echoform-delay` or `aetherform-reverb` with every setting under the
version-7 names (`mode`, `divisionL`, `timeMsL`, `feedback`, ...;
`spaceModel`, `preDelayMs`, `decaySeconds`, ...). From version 17 either
effect may hold `duckKey`, the track from 1 to 64 whose sound ducks the
effect instead of its input; an effect whose ducker follows its input omits
it. The duckers that are on, the pedals that are on, and the effects use at
most 8 key tracks together.

## Placements and samples

A placement holds `id`, `sampleId`, `startTick`, `durationTicks`,
`offsetFrames`, and `gain` (0 to 2). It may also hold `fades`, with `in` and
`out`, each `{"ticks", "curve"}` where the curve is `linear`, `equal-power`, or
`exponential` and the two lengths together are no longer than the placement,
and `pitchCents` from -2400 to 2400. From version 23 it may hold `reverse`,
a boolean: `true` plays the placement backwards, the slice that starts at
`offsetFrames` and lasts `durationTicks`, reversed within the placement, so
the placement's last frame plays the slice's first. A source shorter than the
slice leaves silence at the placement's start. Fades, pitch, and gain apply
in the placement's own time: the fade in from its start and the fade out to
its end. A placement that plays forward omits the key; versions 22 and
earlier reject it. Placements load in `id` order.

A placement names audio by ID. Product catalog IDs resolve through
`libraries`, other IDs through `samples`. `samples` links each used WAV by path
with its name, group, source tempo, sample rate, and origin product.
`missingSamples`, `sampleConflicts`, `importNotices`, and `sourceNotices` keep
unresolved sources, conflict choices, and import provenance, so an imported
song reopens without its original file.

`libraries` records at most one provider folder for each product. A song
that uses both packed original audio and supplemental WAVs for one product
cannot retain both providers in this format. Save rejects that combination
before replacing the destination.

## Version history

Migration preserves the earlier settings within the limits below. Saving an
earlier song writes version 24, which earlier builds cannot open.

| Version | Change | Earlier songs open with |
|---|---|---|
| 8 | Adapts the version-7 document to this editor. | — |
| 9 | Separates nonnegative lane amplitude from `polarityInverted`. | Version 8 signed gains split into amplitude and polarity. |
| 10 | Adds lane `muted` and `solo`. | Mute and Solo off, L/R outputs as saved. |
| 11 | Adds `master` with the three Booster pedals. | Every pedal off. |
| 12 | Adds the trim and the mastering pedals to `master`. | Version 11 Booster pedals first in their saved order, the mastering pedals off. |
| 13 | Adds `fxBuses` and each lane's `sends`. | Every bus empty and every send at 0. |
| 14 | Adds clip `fades` and `pitchCents`, and lane `automation`. | No fades, no pitch change, no automation. |
| 15 | Adds the lane `eq` and the master `bypass`. | Every channel EQ off and the board in use. |
| 16 | Renames the Booster pedals' keys in `master` and its `order`. | The same Booster settings under the new keys. |
| 17 | Adds the lane `ducker`, the pedals' `key`, and the effects' `duckKey`. | Every ducker off; every pedal and effect on its own input. |
| 18 | Adds the EQ's second peak (`peak2Hz`, `peak2Db`, `peak2Q`), the lane `compressor`, and the lane `width`. A lane ducker's `threshold` is measured before the master gain; version 17 measured it after. | The second peak flat, every compressor off, every lane at 100% width. Each saved lane `ducker`, on or off, opens with its `threshold` moved by `-20 * log10(masterGain)` dB and held to -60..0, so it ducks as it did wherever that range allows; a `masterGain` of 0 leaves it, as the key is silent either way. |
| 19 | Adds the song's `sections`. | No sections. |
| 20 | Adds the lane automation `pan`. | No pan automation. |
| 21 | Adds the lane automation `repeat`. | No repeat automation. |
| 22 | Adds the lane `drive`. | Every drive off. |
| 23 | Adds FX bus return automation, the lane automation `repeatGate`, and the placement `reverse`. | Static returns, full-length repeats, no reversed clips. |
| 24 | Adds the lane automation `pitch`. | No pitch automation. |

The readers reject later-version settings in earlier songs. Some optional fields
encoded as `null` are accepted by earlier readers. Versions 12 to 14 also accept
an explicit `"bypass": false` in `master`.

Earlier plain JSON projects have a separate compatibility reader. It accepts
both generations of Booster names without weakening these versioned song rules.
