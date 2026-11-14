# Master pedalboard, FX buses, and placements

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
