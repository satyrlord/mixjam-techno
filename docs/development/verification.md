# Audio and UI verification

## Audio output verification

The optional `audio::output::tests::native_output_buffer_probe` test opens the
default physical output for nine seconds of silence. Run it separately with
`cargo.ps1 -CargoArguments @('test','--lib','native_output_buffer_probe',
'--','--ignored','--nocapture')`. It compares the Automatic, 20 ms, and 40 ms
requests. It reports these items separately: unsupported targets, open
failures, callback sizes, and measured intervals.

The production output status also reports the frames of the last callback and
the last measured callback interval. These values are not the negotiated
buffer capacity, which CPAL does not expose. CPAL calculates its timestamp lead
from the available frames. The lead is not an independent measurement of the
output latency. This probe does not prove that a heavy song plays without
underruns. Report benchmark limits against the measured or explicitly modeled
callback period.

[OPTIONS-PAGE-SPEC.md](../OPTIONS-PAGE-SPEC.md) gives the Options contract. The
preference tests cover these items:

- The defaults and the section-bookmarks reading.
- Malformed and future files, and the validation.
- An atomic write failure.
- The independence of the preferences from the song and the history.

The playback quality tests cover the Drive and master timing, and the
separation of the export. For CPU claims, run the callback benchmark in
`src/audio/realtime_bench.rs` in an optimized build. Record the workload, rate,
cadence, mean, p99, maximum, and missed deadlines. An accepted larger buffer
request does not give a longer processing deadline.

Physical listening, microphone operation, native pickers, screen readers, and
minimum-machine capacity need separate results. Automated tests and screenshots
do not give those results.

The ignored `realtime_options_bench` test covers these synthetic arrangements:
sparse, dense Drive, master-heavy, and 64-track. It uses all four quality
combinations at 44.1 and 48 kHz. It measures starts, seeks, replacements, and
uninterrupted playback after eight seconds.

The representative `realtime_bench` accepts `MIXJAM_BENCH_DRIVE` and
`MIXJAM_BENCH_MASTER` (`4x` or `off`). Set `MIXJAM_BENCH_CONTINUOUS=1` to play
from the start of the song before the measurement of the position that
`MIXJAM_BENCH_START` selects. The measured sample excludes the warmup. The
benchmark reports the audio and wall duration of the warmup. Do not change the
10 ms modeled deadline. [OPTIONS-VERIFICATION.md](../OPTIONS-VERIFICATION.md)
gives the checks and limits of the implementation.

## UI verification

`tools/verify-ui.ps1` captures the built native executable at the three
supported window sizes: 1440x1000, 1024x768, and 960x720. It makes synthetic WAV
and project fixtures, and uses isolated settings. Inspect each capture
visually. It has these parameters:

- `-Executable PATH` selects another build.
- `-OutputDirectory PATH` selects the capture folder. The default is
  `tmp\evidence\ui\redesign`.
- `-Views` selects a subset of the cases. The default is all cases.
- `-Themes` selects a subset of `studio`, `aero`, and `vintage`. The default is
  all three.
- `-Sizes` selects a subset of the three window sizes.
- `-Jobs N` sets how many cases run at the same time. The default is half the
  processors, with a maximum of 8.

Each case runs in its own process, with its own data folder. Each Aero and
Vintage case starts from an options file with that theme. Studio cases start
without an options file, which checks the default of a fresh start. Captures
have the name `theme-view-size`. Captures run at a display scale of 100% only.
Display scaling of 150% and 200% is out of scope.

The capture cases include these views:

- `sections` has named bookmarks and a long section name.
- `options-audio`, `options-workspace`, and `options-files` show the Options
  pages. `options-unapplied` adds unapplied edits on two pages.
  `options-discard` shows its Discard changes? prompt.
- `rail-icons` shows the tool rail without names.
- `boards-folded` folds the lower panel.
- `maximised` hides the tool rail, the Sounds panel, the Inspector, and the
  lower panel, so that the score fills the window.
- `sounds-panel` opens the Sounds panel with the lower panel on Mixer.
- `overview-folded` folds the song overview to its bar.
- `inspector-song`, `inspector-track`, `inspector-clip`, and `inspector-clips`
  open the Inspector on each of its pages.
- `automation-menu` opens the Automation lane menu over the song with fades and
  automation. Track 9 of that song plays a Step synth sequence with its saved
  pattern. The clip shows the notes.
- `command-list` opens the command list over the default song.
- `fx-board` shows the four FX Returns cards. `fx-deep-edit`,
  `fx-deep-edit-delay`, and `fx-deep-edit-reverb` open the editors for Drive,
  Delay, and Reverb.
- `mix-view` shows the Mix view of the score, with the lower panel on
  Mastering. `mix-view-dense` fills it with the full song. `mix-view-empty`
  shows it with no tracks. `mix-view-states` uses the song with the Mixer
  states. `mix-view-eq` adds the EQ window of the first track.

The Mixer cases cover these states:

- An ordinary arrangement and a full 64-track bank.
- An empty arrangement.
- Attenuation, silence, and unity gain.
- Independent polarity, pan, and output routing.
- The EQ window of a track.

The Mastering cases include a bypassed board and the Deep Edit window for
Soft clip, Tape, Booster EQ, Bus compressor, Multiband, and Limiter. The last
three show sample meter readings. The capture set also covers Help,
Sample libraries, Import sound, Import details, Save your changes?, and Sample
conflicts. The Album Review cases are the grid, a filtered grid, an album page,
and a missing reviews folder. The set covers the three tool windows and the
Step synth board. It also covers expanded effects, provider settings, and the
processing states of a missing source. It has no case for the Help windows of
the tools or for Choose path. UI tests check that a tool's Help opens beside
the tool. Recorded-take captures check the window layout with a synthetic
presentation state. They do not record audio.

Use the agent channel of the built application to verify these items:

- Keyboard page selection and draft edits.
- Apply, Cancel, and Restore all defaults.
- Focus restoration.

Inspect the accessibility tree separately from the capture appearance.
[Native frontend](../architecture/frontend.md) describes the themes, the shared
shell and controls, and the collapse behavior that the captures show.

The ignored `arrangement_frame_time` test times the Arrange view. The song has
64 tracks of 256 bars, with waveforms. The test covers each lane height and the
zoom limits. It prints the median, p95, and maximum frame time of each case.
Run it optimized with `cargo.ps1 -CargoArguments @('test','--release',
'--locked','--lib','arrangement_frame_time','--','--ignored','--nocapture')`.
The p95 frame time of each case must stay below 16.7 ms.

A body that is taller than its window shows a scroll bar. A short dialog that
blocks the window follows the height of its content. The captures do not verify
screen readers or other accessibility, physical audio, or native file pickers.
