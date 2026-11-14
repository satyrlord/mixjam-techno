# Architecture

MixJam Techno is a headless Rust backend and a native frontend that reaches it
only through `src/application.rs`. These pages describe that boundary, the
audio engine, the frontend, and the checks that keep them apart.

## Contents

- [Application boundary](architecture/boundary.md#application-boundary): the module map, what `Session` owns, options, and what the frontend owns
- [Playback and the prepared mix](architecture/playback.md#playback-and-the-prepared-mix): preferences, track levels, the prepared mix, the output device, and sample reading
- [Track chains and render state](architecture/track-chains.md#track-chains-and-render-state): the per-track chain, drive, EQ, live levels, and the render state
- [FX buses, master pedalboard, and export](architecture/buses-and-master.md#fx-buses-master-pedalboard-and-export): the FX buses, the master pedalboard, stems, export, and library replacement
- [Native frontend](architecture/frontend.md#native-frontend): themes, the shared shell and controls, waveforms, and the panel views
- [Tool windows and missing audio](architecture/tools.md#tool-windows-and-missing-audio): the Sample Editor, Time Stretcher, Effect Studio, and missing audio
- [Lower-panel boards](architecture/boards.md#lower-panel-boards): the Mixer, the Mix view, FX returns, Mastering, and Step synth boards
- [Startup, libraries, and data folders](architecture/startup-and-data.md#startup-libraries-and-data-folders): startup, library scans, file replacement, runtime data, and Album Review
- [Agent control](architecture/agent-control.md#agent-control): the control channel and how the window serves it
- [Safe frontend iteration](architecture/safe-frontend-iteration.md#safe-frontend-iteration): where frontend changes go and how to check them
