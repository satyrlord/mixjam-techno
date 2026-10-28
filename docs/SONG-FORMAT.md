# Song format

A `.mixjam` song is JSON. Its `formatVersion` selects the reader. Version 16 is
the current song format, and it is the only version that the application
writes. Versions 8 to 15 open and migrate. Version 7 is the earlier MixJam
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
to 16 keeps its `createdAt`.

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
  "midQ", "highHz", "highDb", "highCut"}`, every key required. `lowCut`, a
  12 dB/octave high-pass, runs from 20 to 2000 Hz, where 20 is off; the low
  shelf's `lowHz` from 40 to 500 Hz; the peak's `midHz` from 150 to 10,000 Hz
  and its `midQ` from 0.3 to 8; the high shelf's `highHz` from 1500 to
  16,000 Hz; each gain from -18 to 18 dB, where 0 is off; and `highCut`, a
  12 dB/octave low-pass, from 1000 to 20,000 Hz, where 20,000 is off. A lane
  whose EQ is off at the default settings omits `eq`.

## Master pedalboard

`master` holds `trim`, the gain before the first pedal, from -24 to 24 dB;
`order`, the signal path from the mixer to the output; one record per pedal
with `on` and its settings; and, when it is true, `bypass`, which skips the
whole board, the trim included, for playback and export.

`order` names each pedal once and ends with `limiter`. The pedal keys of
version 16 are:

| Key | Settings |
|---|---|
| `boosterEq` | ten `bands` from 0 to 100 (50 is flat) |
| `boosterCompressor` | `drive`, `speed`, and `gain`, each from 0 to 1000 |
| `boosterWide` | `spread` from 0 to 1000 |
| `softClip` | `amount` from 0 to 6 dB and `ceiling` from -6 to 0 dBFS |
| `tube` | `drive` from 0 to 10 and `mix` from 0 to 100 percent |
| `trimEq` | `highPass` from 10 to 40 Hz, and `mud` and `harsh` from -3 to 0 dB |
| `busCompressor` | `threshold` from -30 to 0 dBFS, `ratio` from 1.5 to 10, `attack` from 0.1 to 100 ms, and `release` from 50 to 1200 ms |
| `maximizer` | `boost` from 0 to 25 percent |
| `liftEq` | `low` and `air` from 0 to 2 dB |
| `tape` | `drive` from 0 to 10 and `speed`, 15 or 30 inches per second |
| `imaging` | `width` from 60 to 140 percent and `monoBelow` from 60 to 300 Hz |
| `multiband` | `low`, `mid`, and `high` from 0 to 100 percent |
| `limiter` | `gain` from 0 to 12 dB and `ceiling` from -3 to 0 dBTP |

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
`spaceModel`, `preDelayMs`, `decaySeconds`, ...).

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
writes version 16, which earlier builds cannot open.

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

A version rejects each key that a later version added.

Earlier plain JSON projects have a separate compatibility reader. It accepts
both generations of Booster names without weakening these versioned song rules.
