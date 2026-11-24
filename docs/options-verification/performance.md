# Options verification: buffering, Drive, and playback performance

## Physical output buffering

Run the ignored `native_output_buffer_probe` test on the physical device. The
command is in
[Audio output verification](../development/verification.md#audio-output-verification).
The probe runs each target (Automatic, 20 ms, and 40 ms) for three seconds.
Record the device, the OS, and the output rate. For each target, report these
values:

- The accepted request.
- The initial writable frames.
- The mean and maximum callback interval.

All three streams must open, and the probe must report no stream errors.

CPAL does not expose the negotiated capacity. The initial writable frames, the
frames of the last callback, the timestamp lead, and the buffer capacity are
different values. Do not state that a request changes the callback interval
or the delay unless the measurement shows it. These silent runs do not prove
song playback without glitches, or the acoustic latency.

## Drive tradeoff

The test `every_quality_keeps_the_playback_alias_threshold` in
`src/drive/tests.rs` uses a 5 kHz tone, 24 dB Drive, 100% Mix, and 20 Hz Focus.
It prints the 19.1 kHz alias level of each mode. The export and `2x` aliases
must be below -65 dBc, and the Off alias must be at least 10 dB higher than
`2x`. Do not weaken these thresholds. On 2026-11-24 the measured levels were
-99.5 dBc for export, -116.6 dBc for `2x`, and -27.4 dBc for Off.

The test `playback_quality_and_offline_8x_aliasing_targets` uses a 4048 Hz tone
at -1 dBFS, 24 dB Drive, and 50% Character. Export must keep the total
non-harmonic alias power below -90 dB and each spur below -85 dBc. The `2x`
total and worst spur must be at least 15 dB below those of Off.

The test `base_rate_curve_and_reference_share_one_sample_adaa2_delay` checks
the Off reference: `(x0 + x1 + x2) / 3`, with the magnitude
`(1 + 2 cos(2 pi f / rate)) / 3`. This value describes that reference, not the
full mixed output of the Drive.

Export keeps the 71-frame integer stage delay and its chain compensation. Its
curve and reference lag by one eighth of a frame. Both playback modes have no
lookahead and do not delay the track. Their curve and reference lag the track
by one sample at the processing rate: one frame at Off and half a frame at
`2x`, plus the phase of the IIR filters at `2x`. Thus the difference that Mix
adds lags the track a little in playback.

## Playback performance

Run the ignored `realtime_options_bench` and `realtime_bench` tests in an
optimized build on a quiet machine. The commands and the `MIXJAM_BENCH_*`
variables are in
[Audio output verification](../development/verification.md#audio-output-verification)
and in the header of `src/audio/realtime_bench.rs`. Do not run a compiler
during the measurement, and control the other activity of the operating
system.

The harness uses 480-frame callbacks at 48 kHz and 441-frame callbacks at
44.1 kHz, with a fixed deadline of 10 ms. The callbacks are modeled. They are
not physical playback. The CPU values use the thread-cycle calibration of the
harness.

Make two passes. Each pass covers these cases:

- Sparse, dense Drive, master-heavy, and 64-track synthetic arrangements.
- All four quality combinations.
- Starts, seeks, replacements, and continuous playback after all lanes played.

Use these rules to read the results:

- In a Drive-bound case, Drive Off must lower the CPU mean.
- In the master-heavy case, the Drive quality must not change the CPU mean.
  Master Off must lower it.
- Reduced quality does not make each callback fit 10 ms in the dense and
  stress cases. Do not claim that it does.
- A measurement that starts at a song position does not prove the cost of
  sustained playback. Set `MIXJAM_BENCH_CONTINUOUS=1`, and set
  `MIXJAM_BENCH_START` after the start of the last lane.

For a representative saved arrangement, check these items:

- Each WAV path exists before and after the measurement, and each preparation
  succeeds.
- The saved list of missing sources is empty. The timing harness does not
  count missing sources at runtime.
- A conversion of the same fixture with the release executable reports zero
  missing sources at runtime.
- The hashes of the input and the executable do not change during the
  measurement.

Put the private fixtures, the full results for each phase and rate, and the
raw logs in the local verification evidence. Do not put private source paths
in this file.
