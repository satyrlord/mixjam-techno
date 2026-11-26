# Architecture

MixJam Techno is a headless Rust backend and a native frontend that reaches it
only through `src/application.rs`. These pages describe that boundary, the
audio engine, the frontend, and the checks that keep them apart.

## Contents

- [Application boundary](architecture/boundary.md#application-boundary): the
  module map, what `Session` owns, options, and what the frontend owns
- [Playback and the prepared
  mix](architecture/playback.md#playback-and-the-prepared-mix): preferences,
  track levels, the prepared mix, and the output device
- [Voices and sample
  reading](architecture/voices-and-sample-reading.md#voices-and-sample-reading):
  voices, reversal, the Pitch lane, and sample reading
- [Track chains and render
  state](architecture/track-chains.md#track-chains-and-render-state): sample
  rates, the per-track chain, Repeat, keys, duckers, and the render state
- [Drive and channel EQ](architecture/drive-and-channel-eq.md#drive-and-channel-eq):
  the drive, the channel EQ, and live track levels
- [FX buses, master pedalboard, and
  export](architecture/buses-and-master.md#fx-buses-master-pedalboard-and-export):
  the FX buses, the master pedalboard, stems, and export
- [Library replacement and song
  opening](architecture/library-replacement.md#library-replacement-and-song-opening):
  library replacement and song opening
- [Native frontend](architecture/frontend.md#native-frontend): themes, the
  shared shell and controls, waveforms, and the panel views
- [Tool windows and missing
  audio](architecture/tools.md#tool-windows-and-missing-audio): the Sample
  Editor, Time Stretcher, Effect Studio, and missing audio
- [Lower-panel boards](architecture/boards.md#lower-panel-boards): the Mixer, FX
  returns, and the lower panel
- [Mastering board](architecture/mastering-board.md#mastering-board): the
  Mastering board, presets, and meters
- [Step synth board](architecture/step-synth-board.md#step-synth-board): the
  Step synth board, grid, effects, and actions
- [Startup, libraries, and data
  folders](architecture/startup-and-data.md#startup-libraries-and-data-folders):
  startup, library scans, file replacement, and packed samples
- [Runtime data folders and Album
  Review](architecture/runtime-data.md#runtime-data-folders-and-album-review):
  the runtime catalogs, the data folders, and the Album Review
- [Agent control](architecture/agent-control.md#agent-control): the control
  channel and how the window serves it
- [Safe frontend
  iteration](architecture/safe-frontend-iteration.md#safe-frontend-iteration):
  where frontend changes go and how to check them
