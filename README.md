# MixJam Techno

A native Rust loop sequencer for Windows. Arrange WAV samples on up to 64 tracks
at 120, 125, 130, 135, 140, 160, or 180 BPM, mix them with channel EQ and four
FX buses, master them on a pedalboard, and export stereo WAV. The editor also
opens MixJam v7 `.mixjam` projects and several legacy `.mix` song formats.

## Requirements

- Windows.
- Git with a complete checkout history (for a shallow clone, run
  `git fetch --unshallow`).
- Rust. `tools/runtime/install-rust.ps1` installs it in `local-data/toolchain/`
  without changing the user PATH.
- The Visual Studio C++ build tools and the Windows SDK.
- Python 3 for the checks. The song tools need Python 3.9 or later and numpy.

## Build and run

Open `Start MixJam Techno.cmd`, or run from this folder:

~~~powershell
.\run.ps1
~~~

The launcher builds `target\release\mixjam-techno.exe` when it is absent. Use
`-Build` to rebuild after source changes, or build it yourself:

~~~powershell
.\tools\runtime\cargo.ps1 build --release --locked
~~~

On first launch, select **Libraries → Choose WAV folder**. Settings, the scan
cache, and generated audio use `%LOCALAPPDATA%\MixJam Techno`; set
`MIXJAM_TECHNO_DATA_DIR` to use another writable folder.

## Documentation

- [User guide](docs/USER-GUIDE.md): libraries, arranging, the Mixer,
  Mastering, Effects, sound tools, export, and opening songs from other programs.
- [Song format](docs/SONG-FORMAT.md): the `.mixjam` format and its versions.
- [Command line](docs/COMMAND-LINE.md): headless inspection, conversion, and
  rendering.
- [Agent control](docs/AGENT-CONTROL.md): operate the window from a script or
  an AI agent.
- [Development](docs/DEVELOPMENT.md): the toolchain, the version number, the
  checks, and UI verification.
- [Architecture](docs/ARCHITECTURE.md) and [design](docs/DESIGN.md): the
  application boundaries and the visual contract.
- [Song tools](tools/song/README.md): Python tools that build, render, and
  measure songs.
