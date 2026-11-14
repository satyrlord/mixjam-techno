# Options verification: buffering, Drive, and playback performance

## Physical output buffering

The silent probe ran on Windows 11 Pro build 26300 with an Intel i7-11700 and
the default Steinberg UR-RT2 at 48 kHz. Each target ran for three seconds.

| Target | Accepted request | Initial writable frames | Mean callback interval | Maximum interval |
|---|---|---|---|---|
| Automatic | System default | 1,056 | 10.101 ms | 39.709 ms |
| 20 ms | 960 frames | 1,056 | 10.100 ms | 40.259 ms |
| 40 ms | 1,920 frames | 1,920 | 10.102 ms | 39.431 ms |

All three streams opened and the probe reported no stream errors. The 20 ms
request did not change the observed initial writable frames on this device.
The 40 ms request did. Neither request changed the mean callback cadence to
its target. CPAL does not expose negotiated capacity. Initial writable frames,
the last callback's frames, timestamp lead, and buffer capacity are distinct
values. These silent runs do not prove glitch-free song playback or acoustic
latency.

## Drive tradeoff

With a 5 kHz tone, 24 dB Drive, 100% Mix, and 20 Hz Focus, the measured
19.1 kHz alias was -99.80 dBc at 4x and -20.23 dBc with Off. The existing 4x
acceptance threshold was retained.

Both modes retain the 69-frame integer stage delay and its chain compensation.
Off keeps base-rate antiderivative anti-aliasing. Its matching linear reference
has a half-base-frame phase lag; the 4x curve/reference pair has an eighth-frame
lag. Matching integer delays does not make those paths phase-equivalent.
The Off linear reference response measured -0.022 dB at 1 kHz, -0.563 dB at
5 kHz, -2.420 dB at 10 kHz, and -13.328 dB at 19 kHz. These values characterize
that reference, not the complete mixed Drive output.

## Playback performance

The optimized callback harness measured 38,480 callbacks in 552 wall/CPU
reports. It used 480-frame callbacks at 48 kHz and 441-frame callbacks at
44.1 kHz, with the deadline fixed at 10 ms. These are modeled callbacks, not
physical playback. CPU values use the harness's thread-cycle calibration.
No compiler overlapped measurements. Other operating-system activity was not
controlled.

Two passes covered sparse, dense Drive, master-heavy, and 64-track synthetic
arrangements, all four quality combinations, starts, seeks, replacements,
and uninterrupted playback after all lanes had sounded. At 48 kHz with
master quality held at 4x:

| Sustained workload | CPU mean, Drive 4x to Off, pass 1 | Pass 2 |
|---|---|---|
| 22 active Drives | 10.88 to 2.27 ms | 10.80 to 2.24 ms |
| 64-track stress | 31.76 to 6.72 ms | 31.59 to 6.74 ms |

The master-heavy case, with Drive quality held fixed, measured 1.54 to
0.64 ms when master saturation changed from 4x to Off, in both passes.
Across dense/stress starts, seeks, replacements, and continuous cases,
Drive 4x missed 4,414 of 6,400 deadlines; Off missed 16 of 6,400. The worst
Off callback was 18.14 ms. Reduced quality does not guarantee every callback
will fit 10 ms.

A representative saved arrangement had 492 placements, 22 used lanes with
active Drives, and 114 used WAV identities. Its saved missing-source list
was empty and every WAV path existed before and after measurement. Every
preparation succeeded; the loaded runtime missing-source count was not
instrumented in the timing harness. A later release-executable conversion of
the same fixture reported zero runtime missing sources. Input and executable
hashes stayed unchanged during measurement.

Cold-position tests covered all four quality combinations. The sustained
comparison held the master at 4x and compared Drive modes at both rates.
It played from the song's start for 338 seconds before measuring seconds
338 through 348; the last lane first enters at 336 seconds.

| Rate | Drive | CPU mean | Wall mean / p99 / maximum | Missed / callbacks |
|---|---|---|---|---|
| 48 kHz | 4x | 11.27 ms | 11.28 / 15.93 / 19.44 ms | 720 / 1,000 |
| 48 kHz | Off | 4.09 ms | 4.09 / 5.04 / 5.63 ms | 0 / 1,000 |
| 44.1 kHz | 4x | 11.23 ms | 11.24 / 14.86 / 18.41 ms | 722 / 1,000 |
| 44.1 kHz | Off | 4.08 ms | 4.08 / 5.27 / 8.24 ms | 0 / 1,000 |

Drive Off reduced CPU mean by about 63.7% in this sustained arrangement.
The 48 kHz cold-position full-quality mean was 9.40 ms, compared with
11.28 ms after uninterrupted playback. A measurement that starts at a song
position does not establish its sustained playback cost. Private fixtures,
full per-phase/per-rate results, and raw logs remain in local verification
evidence; no private source paths are included here.
