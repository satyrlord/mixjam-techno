# MixJam Techno

MixJam Techno is a native Rust loop sequencer for Windows. With it, you can do
these tasks:

- Arrange WAV samples on up to 64 tracks at 120, 125, 130, 135, 140, 160, or
  180 BPM.
- Mix them with a channel EQ and four FX buses.
- Master them on a pedalboard, and export a stereo WAV.

The editor also opens MixJam v7 `.mixjam` projects and some legacy `.mix` song
formats. Album Review keeps deep-listening analyses of reference albums and of
your own songs, for you and for song-building agents.

## Requirements

- Windows.
- Git with a complete checkout history. For a shallow clone, run
  `git fetch --unshallow`.
- Rust. `tools/runtime/install-rust.ps1` installs it in `local-data/toolchain/`,
  and does not change the user PATH.
- The Visual Studio C++ build tools and the Windows SDK.
- Python 3 for the checks. The song tools need Python 3.9 or later and numpy.

## Build and run

Open `Start MixJam Techno.cmd`, or run this command from this folder:

~~~powershell
.\run.ps1
~~~

The launcher builds `target\release\mixjam-techno.exe` when it is absent. To
build again after source changes, use `-Build`, or build it yourself:

~~~powershell
.\tools\runtime\cargo.ps1 build --release --locked
~~~

At the first start, select **Options → Sample libraries → Add library folder**.
The gear at the left end of the status bar, or **Ctrl+,**, opens Options.
Settings, the scan cache, and generated audio use
`%LOCALAPPDATA%\MixJam Techno`. To use another writable folder, set
`MIXJAM_TECHNO_DATA_DIR`.

## Documentation

- [User guide](docs/USER-GUIDE.md): libraries, arrangement, the Mixer,
  Mastering, FX returns, sound tools, export, and songs from other programs.
- [Song format](docs/SONG-FORMAT.md): the `.mixjam` format and its versions.
- [Command line](docs/COMMAND-LINE.md): headless inspection, conversion, and
  render.
- [Album Review writing guide](docs/ALBUM-REVIEW-WRITING.md) and
  [research shortlist](docs/ALBUM-REVIEW-RESEARCH.md): the content rules of
  the reviews and the albums to review next.
- [Agent control](docs/AGENT-CONTROL.md): operate the window from a script or
  an AI agent.
- [Development](docs/DEVELOPMENT.md): the toolchain, the version number, the
  checks, and UI verification.
- [Architecture](docs/ARCHITECTURE.md) and [design](docs/DESIGN.md): the
  application boundaries and the visual contract.
- [Song tools](tools/song/README.md): Python tools that build, render, and
  measure songs.
