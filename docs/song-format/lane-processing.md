# Lane processing keys

## `eq`

`eq` is the channel EQ, from version 15: `{"on", "lowCut", "lowHz", "lowDb",
"midHz", "midDb", "midQ", "highHz", "highDb", "highCut"}`. The saved `eq` must
hold every one of these keys. The second peak adds `peak2Hz`, `peak2Db`, and
`peak2Q` from version 18. A lane saves each of these keys only when it differs
from its default (250 Hz, 0 dB, and 1).

The bands run in this order:

| Band | Keys | Range |
| --- | --- | --- |
| Low cut, a high-pass | `lowCut` | 20 to 2000 Hz (20 is off) |
| Low shelf | `lowHz` | 40 to 500 Hz |
| First peak | `midHz`, `midQ` | 150 to 10,000 Hz, 0.3 to 8 |
| Second peak | `peak2Hz`, `peak2Q` | 40 to 16,000 Hz, 0.3 to 8 |
| High shelf | `highHz` | 1500 to 16,000 Hz |
| High cut, a low-pass | `highCut` | 1000 to 20,000 Hz (20,000 is off) |

Each gain (`lowDb`, `midDb`, `peak2Db`, and `highDb`) is from -18 to 18 dB. A
gain of 0 is off. A lane whose EQ is off at the default settings omits `eq`.
From version 29, `cutSlope` sets both cuts to `12db` or `24db` per octave. A
lane at the default `12db` omits `cutSlope`. Earlier versions reject the key.

## `drive`

`drive` is the lane drive, from version 22. It runs after the EQ and before the
compressor. The saved `drive` must hold every key: `{"on", "amount",
"character", "mix", "focus", "output"}`.

- `amount`, from 0 to 24 dB, is the level that drives a copy of the lane into
  the saturation curve.
- `character`, from 0 to 100 percent, goes from a symmetric curve (odd
  harmonics) to a one-sided curve (mostly even harmonics).
- `mix`, from 0 to 100 percent, is the share of the change in the copy that the
  drive adds to the clean lane.
- `focus`, from 20 to 2000 Hz, is the corner of a 24 dB/octave high-pass on
  that change. Below it, the lane stays clean.
- `output`, from -12 to 12 dB, is the trim after the drive.

The drive matches the result to the input power. Then it applies the `output`
trim. A `mix` of 0 leaves the lane unchanged when `output` is 0 dB. A lane
whose drive is off at the default settings (12 dB, 25%, 50%, 100 Hz, 0 dB)
omits `drive`. Versions 21 and earlier reject the key.

## `compressor`

`compressor` is the lane compressor after the drive, from version 18. The saved
`compressor` must hold every key: `{"on", "threshold", "ratio", "attack",
"release", "makeup"}`.

- `threshold` is from -60 to 0 dBFS.
- `ratio` is from 1 to 20.
- `attack` is from 0.1 to 100 ms.
- `release` is from 10 to 1000 ms.
- `makeup` is from 0 to 24 dB.
- `scHpfHz`, from version 29, is the detector's high-pass frequency from 20 to
  250 Hz. Values at or below 20.5 Hz bypass it. The default of 20 Hz omits the
  key. Earlier versions reject the key.

A lane whose compressor is off at the default settings (-20 dBFS, 4:1, 10 ms,
100 ms, 0 dB, 20 Hz) omits `compressor`.

## `width`

`width` is the stereo width of the lane in percent, before its pan. It is from 0
(mono) to 200. It exists from version 18. A lane at 100% omits it.

`monoBelowHz` is the crossover frequency from 20 to 300 Hz. It reduces the
stereo difference below that frequency. Values at or below 20.5 Hz bypass the
crossover. The key exists from version 29 and is omitted at 20 Hz.

## Filter settings

`filterResonance` is the resonance of the automated Filter lane, from 0 to
100 percent. `filterSlope` sets its slope to `12db` or `24db` per octave. Both
keys exist from version 29. A lane at 0 percent or `12db` omits that key.
Earlier versions reject both Filter keys and `monoBelowHz`.

## `ducker`

`ducker` lowers the lane while another lane, its key, sounds. It exists from
version 17. The saved `ducker` must hold every key: `{"on", "key", "threshold",
"depth", "attack", "release"}`.

- `key` is a track from 1 to 64. While the ducker is on, it is not the track of
  the lane itself.
- `threshold` is from -60 to 0 dBFS. The mix measures it on the key lane after
  its fader and pan, and before the master gain.
- `depth` is from 0 to 24 dB.
- `attack` is from 0.1 to 50 ms.
- `release` is from 10 to 1000 ms.

All duckers, pedals, and effects together use at most 8 key tracks. See [FX
buses](master-fx-and-placements.md#fx-buses). A lane whose ducker is off at the
default settings omits `ducker`.
