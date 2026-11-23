# Master pedalboard, FX buses, and placements

## Master pedalboard

`master` and its individual pedal records are JSON objects. `master` holds
these keys:

- `trim`, the gain before the first pedal, from -24 to 24 dB.
- `order`, the signal path from the mixer to the output.
- One record for each pedal, with `on` and its settings.
- `bypass`, saved only when it is true. It skips the whole board, the trim
  included, for playback and export.

`order` names each pedal once and ends with `limiter`. From version 16, the
pedal keys are:

| Key | Settings |
| --- | --- |
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
listens to. The value 0 is the own input of the pedal. A pedal whose key is 0
omits it.

Version 28 adds these saved values:

| Pedal | Key | Valid values | Default |
| --- | --- | --- | --- |
| `busCompressor` | `scHpfHz` | 20 to 250 Hz | 20 Hz |
| `busCompressor` | `mix` | 0 to 100 percent | 100 percent |
| `busCompressor` | `makeupDb` | 0 to 12 dB | 0 dB |
| `limiter` | `style` | `transparent`, `punchy`, `modern` | `transparent` |

Earlier songs reject these keys, including explicit defaults. The reader uses
the defaults for missing keys. The writer omits values at their defaults.
Playback and export use these values. At their defaults, the Bus compressor
has no sidechain filter, no dry blend, and no makeup gain.

Versions 11 to 15 name the three Booster pedals with earlier keys, in the
record and in `order`. `BOOSTER_KEYS` in `src/mixjam/song/migration.rs` lists
both key sets. Those versions open with the same settings, and they reject
the version-16 keys. Version 16 rejects the earlier keys.

The reader rejects these defects:

- A value that is out of range or not finite.
- An unknown pedal or key.
- An `order` that misses a pedal or repeats a pedal.

## FX buses

`fxBuses` holds exactly four buses in order. Each bus holds `module`,
`powered`, `returnLevel` from 0 to 1, and `limiter`.

From version 23, a bus can also hold `returnAutomation`, its return lane. It
is a list of `{"tick", "value"}` points in strictly increasing tick order
within the timeline. Each value is from 0 to 2. The value 1 is the full
return, and 2 is about +6 dB.

- A bus whose lane has no points omits the key.
- While the lane has points, it replaces `returnLevel` for playback, export,
  and stems.
- Like the volume lane, it holds its first value before its first point and
  its last value after its last point.
- Between two points, it moves in a straight line at every frame.
- The safety limiter of the bus follows it.

Versions 22 and earlier reject the key, even as an empty list.

A `module` is one of these:

- `{"type": "empty"}`.
- `echoform-delay`, with every setting under the version-7 names (`mode`,
  `divisionL`, `timeMsL`, `feedback`, and so on).
- `aetherform-reverb`, with every setting under the version-7 names
  (`spaceModel`, `preDelayMs`, `decaySeconds`, and so on).
- From version 25, `emberform-drive`. Versions 24 and earlier reject it.

`emberform-drive` holds these settings:

| Key | Value |
| --- | --- |
| `shape` | `tube`, `tape`, `overdrive`, `fuzz`, `hard-clip`, `foldback`, `rectifier`, or `bitcrush` |
| `driveDb` | 0 to 48 |
| `asymmetryPercent` | 0 to 100 |
| `lowCutHz` | 20 to 2000 |
| `highCutHz` | 1000 to 20000 |
| `bits` | 1 to 16 |
| `crushRateHz` | 500 to 44100 |
| `widthPercent` | 0 to 200 |
| `duckAmountPercent` | 0 to 100 |
| `duckReleaseMs` | 50 to 2500 |
| `duckThresholdDb` | -48 to 0 dBFS, from version 27 |
| `levelMatch` | `true` or `false`, from version 27 |
| `outputDb` | -24 to 12 |
| `bypass` | `true` or `false` |

Version 27 adds these settings:

| Module | Key | Value | Migration default |
| --- | --- | --- | --- |
| All effects | `duckThresholdDb` | -48 to 0 dBFS | -27 dBFS |
| `echoform-delay` | `linkTimes` | `true` or `false` | `true` when both free times and both sync divisions match |
| `echoform-delay` | `smearPercent` | 0 to 100 | 0 |
| `echoform-delay` | `filterResonance` | 0 to 100 | 0 |
| `aetherform-reverb` | `bassMult` | 0.25 to 2.5 | 1 |
| `aetherform-reverb` | `trebleMult` | 0.2 to 1.5 | 1 |
| `emberform-drive` | `levelMatch` | `true` or `false` | `true` |

Earlier native songs and known version-7 modules reject version-27 keys. The
reader applies the migration defaults to effects from those files. Earlier JSON
projects use the same defaults for any missing version-27 effect settings.

From version 17, any effect can hold `duckKey`. This is the track from 1 to 64
whose sound ducks the effect, instead of the input of the effect. An effect
whose ducker follows its input omits `duckKey`. The duckers that are on, the
pedals that are on, and the effects use at most 8 key tracks together.

## Placements and samples

A placement holds `id`, `sampleId`, `startTick`, `durationTicks`,
`offsetFrames`, and `gain` (0 to 2). It can also hold these keys:

- `fades`, with `in` and `out`. Each is `{"ticks", "curve"}`. The curve is
  `linear`, `equal-power`, or `exponential`. The two lengths together are no
  longer than the placement.
- `pitchCents`, from -2400 to 2400.
- `reverse`, from version 23. It is a boolean.

`reverse` set to `true` plays the slice of the placement backwards. The slice
starts at `offsetFrames` and lasts `durationTicks`. The slice plays in reverse
within the placement, so the last frame of the placement plays the first frame
of the slice. If the source is shorter than the slice, silence fills the start
of the placement. A placement that plays forward omits `reverse`. Versions 22
and earlier reject it.

Fades, pitch, and gain apply in the time of the placement. The fade in starts at
the start of the placement, and the fade out ends at its end. Placements load in
`id` order.

A placement names audio by ID. Product catalog IDs resolve through
`libraries`. Other IDs resolve through `samples`. `samples` links each used
WAV file by path, with its name, group, source tempo, sample rate, and origin
product. `missingSamples`, `sampleConflicts`, `importNotices`, and
`sourceNotices` keep unresolved sources, conflict choices, and import
provenance. An imported song can then reopen without its original file.

`libraries` records at most one provider folder for each product. This format
cannot keep two providers for one product. A song that uses packed original
audio and supplemental WAV files for one product has two providers. Save rejects
that combination before it replaces the destination.
