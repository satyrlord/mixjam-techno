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

The test `playback_quality_records_alias_and_frequency_response_tradeoffs` in
`src/drive/tests.rs` uses a 5 kHz tone, 24 dB Drive, 100% Mix, and 20 Hz Focus.
It prints the 19.1 kHz alias level of each mode. The 4x alias must be below
-65 dBc, and the Off alias must be at least 10 dB higher. Do not weaken the 4x
threshold.

The test also prints the response of the Off linear reference at 55 Hz, 1, 5,
10, and 19 kHz. These values describe that reference, not the full mixed
output of the Drive.

Both modes keep the 69-frame integer stage delay and its chain compensation.
Off keeps the antiderivative anti-aliasing at the base rate. Its matching
linear reference has a phase lag of half a base frame. The 4x pair of curve
and reference has a lag of one eighth of a frame. Matching integer delays do
not make these paths equivalent in phase.

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
