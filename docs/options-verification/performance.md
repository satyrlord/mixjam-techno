# Options verification: buffering, Drive, and playback performance

## Physical output buffering

The silent probe ran on Windows 11 Pro build 26300 with an Intel i7-11700 and
the default USB audio interface at 48 kHz. Each target ran for three seconds.

| Target | Accepted request | Initial writable frames | Mean callback interval | Maximum interval |
|---|---|---|---|---|
| Automatic | System default | 1,056 | 10.101 ms | 39.709 ms |
| 20 ms | 960 frames | 1,056 | 10.100 ms | 40.259 ms |
| 40 ms | 1,920 frames | 1,920 | 10.102 ms | 39.431 ms |

All three streams opened, and the probe reported no stream errors. On this
device, the 20 ms request did not change the initial writable frames. The
40 ms request changed them. Neither request changed the mean callback interval
to its target. CPAL does not expose the negotiated capacity. The initial
writable frames, the frames of the last callback, the timestamp lead, and the
buffer capacity are different values. These silent runs do not prove song
playback without glitches, or the acoustic latency.

## Drive tradeoff

The test used a 5 kHz tone, 24 dB Drive, 100% Mix, and 20 Hz Focus. The
19.1 kHz alias was -99.80 dBc at 4x and -20.23 dBc with Off. The 4x acceptance
threshold did not change.

Both modes keep the 69-frame integer stage delay and its chain compensation.
Off keeps the antiderivative anti-aliasing at the base rate. Its matching
linear reference has a phase lag of half a base frame. The 4x pair of curve
and reference has a lag of one eighth of a frame. Matching integer delays do
not make these paths equivalent in phase.

The Off linear reference measured -0.022 dB at 1 kHz, -0.563 dB at 5 kHz,
-2.420 dB at 10 kHz, and -13.328 dB at 19 kHz. These values describe that
reference, not the full mixed output of the Drive.

## Playback performance

The optimized callback harness measured 38,480 callbacks in 552 wall and CPU
reports. It used 480-frame callbacks at 48 kHz and 441-frame callbacks at
44.1 kHz, with a fixed deadline of 10 ms. These are modeled callbacks, not
physical playback. The CPU values use the thread-cycle calibration of the
harness. No compiler ran during the measurements. Other activity of the
operating system was not controlled.

Two passes covered these cases:

- Sparse, dense Drive, master-heavy, and 64-track synthetic arrangements.
- All four quality combinations.
- Starts, seeks, replacements, and continuous playback after all lanes played.

At 48 kHz, with the master quality at 4x:

| Sustained workload | CPU mean, Drive 4x to Off, pass 1 | Pass 2 |
|---|---|---|
| 22 active Drives | 10.88 to 2.27 ms | 10.80 to 2.24 ms |
| 64-track stress | 31.76 to 6.72 ms | 31.59 to 6.74 ms |

In the master-heavy case, the Drive quality did not change. When the master
saturation changed from 4x to Off, the CPU mean went from 1.54 to 0.64 ms in
both passes.

The dense and stress cases include starts, seeks, replacements, and continuous
playback. In these cases, Drive 4x missed 4,414 of 6,400 deadlines, and Off
missed 16 of 6,400. The slowest Off callback took 18.14 ms. Reduced quality
does not make each callback fit 10 ms.

A representative saved arrangement had 492 placements, 22 used lanes with
active Drives, and 114 used WAV identities. Its saved list of missing sources
was empty. Each WAV path existed before and after the measurement, and each
preparation was successful. The timing harness did not count the missing
sources at runtime. A conversion of the same fixture with the release
executable reported zero missing sources at runtime. The hashes of the input
and the executable did not change during the measurement.

Tests from a cold position covered all four quality combinations. The
sustained comparison kept the master at 4x and compared the Drive modes at
both rates. It played from the start of the song for 338 seconds, then
measured seconds 338 to 348. The last lane starts at 336 seconds.

| Rate | Drive | CPU mean | Wall mean / p99 / maximum | Missed / callbacks |
|---|---|---|---|---|
| 48 kHz | 4x | 11.27 ms | 11.28 / 15.93 / 19.44 ms | 720 / 1,000 |
| 48 kHz | Off | 4.09 ms | 4.09 / 5.04 / 5.63 ms | 0 / 1,000 |
| 44.1 kHz | 4x | 11.23 ms | 11.24 / 14.86 / 18.41 ms | 722 / 1,000 |
| 44.1 kHz | Off | 4.08 ms | 4.08 / 5.27 / 8.24 ms | 0 / 1,000 |

In this sustained arrangement, Drive Off decreased the CPU mean by about
63.7%. At 48 kHz and full quality, the mean from a cold position was 9.40 ms.
After continuous playback, it was 11.28 ms. Thus a measurement that starts at
a song position does not prove the cost of sustained playback.

The private fixtures, the full results for each phase and rate, and the raw
logs stay in the local verification evidence. This file contains no private
source paths.
