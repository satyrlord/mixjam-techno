# Version history

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
