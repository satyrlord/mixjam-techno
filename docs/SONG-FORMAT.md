# Song format

A `.mixjam` song is JSON. Its `formatVersion` selects the reader. Version 18 is
the current song format, and it is the only version that the application
writes. Versions 8 to 17 open and migrate. Version 7 is the earlier MixJam
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
to 18 keeps its `createdAt`.

Ticks are 256 per bar. A song links each WAV file by path; it is not a
portable audio bundle.

## Song and lanes

`song` holds `title`, `author`, `bpm` (120, 125, 130, 135, 140, 160, or 180),
`masterGain`, and `loopRegion`. Save names a new song after its file, and Save
As renames a song whose title is its earlier file name; an imported title stays.

`lanes` has exactly 64 entries in track order. A lane holds `gain` (amplitude
from 0 to 4; 1 is 0 dB), `polarityInverted`, `pan`, `leftEnabled`,
`rightEnabled`, `muted`, `solo`, `sends` (four amounts from 0 to 1), and
`placements`. A lane may also hold:

- `automation`, with any of `volume`, `sends` (four lists), and `filter`:
  lists of `{"tick", "value"}` points in strictly increasing tick order within
  the timeline. Volume is from -60 to 6 dB, a send from 0 to 1, and the filter
  from -1 to 1.
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
- `compressor`, the lane compressor after the EQ: `{"on", "threshold",
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
versions 16 to 18 are:

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
`returnLevel` from 0 to 1, and `limiter`. A `module` is `{"type": "empty"}`, or
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
and `pitchCents` from -2400 to 2400. Placements load in `id` order.

A placement names audio by ID. Product catalog IDs resolve through
`libraries`, other IDs through `samples`. `samples` links each used WAV by path
with its name, group, source tempo, sample rate, and origin product.
`missingSamples`, `sampleConflicts`, `importNotices`, and `sourceNotices` keep
unresolved sources, conflict choices, and import provenance, so an imported
song reopens without its original file.

## Version history

Each version opens as it sounded when it was saved. Saving an earlier song
writes version 18, which earlier builds cannot open.

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

A version rejects each key that a later version added.

Earlier plain JSON projects have a separate compatibility reader. It accepts
both generations of Booster names without weakening these versioned song rules.
