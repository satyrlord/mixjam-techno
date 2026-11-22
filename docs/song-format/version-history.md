# Version history

Migration keeps the earlier settings within the limits below. A save of an
earlier song writes version 27, which earlier builds cannot open.

| Version | Change | Earlier songs open with |
| --- | --- | --- |
| 8 | Adapts the version-7 document to this editor. | — |
| 9 | Separates nonnegative lane amplitude from `polarityInverted`. | Version 8 signed gains split into amplitude and polarity. |
| 10 | Adds lane `muted` and `solo`. | Mute and Solo off, L/R outputs as saved. |
| 11 | Adds `master` with the three Booster pedals. | Every pedal off. |
| 12 | Adds the trim and the mastering pedals to `master`. | Version 11 Booster pedals first in their saved order, the mastering pedals off. |
| 13 | Adds `fxBuses` and the `sends` of each lane. | Every bus empty and every send at 0. |
| 14 | Adds clip `fades` and `pitchCents`, and lane `automation`. | No fades, no pitch change, no automation. |
| 15 | Adds the lane `eq` and the master `bypass`. | Every channel EQ off and the board in use. |
| 16 | Renames the keys of the Booster pedals in `master` and its `order`. | The same Booster settings under the new keys. |
| 17 | Adds the lane `ducker`, the `key` of the pedals, and the `duckKey` of the effects. | Every ducker off. Every pedal and effect on its own input. |
| 18 | Adds the second peak of the EQ (`peak2Hz`, `peak2Db`, `peak2Q`), the lane `compressor`, and the lane `width`. The mix measures the key of a lane ducker before the master gain. Version 17 measured it after the master gain. | The second peak flat, every compressor off, every lane at 100% width. See the note after this table for the ducker `threshold`. |
| 19 | Adds the `sections` of the song. | No sections. |
| 20 | Adds the lane automation `pan`. | No pan automation. |
| 21 | Adds the lane automation `repeat`. | No repeat automation. |
| 22 | Adds the lane `drive`. | Every drive off. |
| 23 | Adds FX bus return automation, the lane automation `repeatGate`, and the placement `reverse`. | Static returns, full-length repeats, no reversed clips. |
| 24 | Adds the lane automation `pitch`. | No pitch automation. |
| 25 | Adds the FX bus module `emberform-drive`. | No drive returns. |
| 26 | Adds the lane automation `compression`. | The lane compressor fully in. |
| 27 | Adds the FX settings `levelMatch`, `duckThresholdDb`, `linkTimes`, `smearPercent`, `filterResonance`, `bassMult`, and `trebleMult`. | `duckThresholdDb` at -27 dBFS, `levelMatch` on, `linkTimes` on when both time pairs match, and neutral smear, resonance, bass, and treble settings. |

The reader moves the `threshold` of each saved lane `ducker` of an earlier song,
on or off, by `-20 * log10(masterGain)` dB. It then holds the result to -60..0.
The lane then ducks as it did, wherever that range allows. A `masterGain` of 0
leaves the `threshold` as it is, because the key is silent either way.

The reader rejects settings of a later version in an earlier song. A version
that does not hold a field reads `null` as absent for `master`, `fxBuses`,
`sends`, `automation`, `eq`, `compressor`, `width`, `ducker`, `fades`, and
`pitchCents`. The fields `polarityInverted`, `muted`, `solo`, `reverse`,
`sections`, and `drive` reject `null`. Versions 12 to 14 also accept an explicit
`"bypass": false` in `master`.

A separate compatibility reader opens plain JSON projects. It accepts both
generations of Booster names. It does not weaken the versioned song rules in
this document.
