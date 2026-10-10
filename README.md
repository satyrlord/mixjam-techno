# MixJam Techno

A native Rust loop sequencer for Windows. Arrange WAV samples on up to 64 tracks
at 120, 140, 160, or 180 BPM, mix them, and export stereo WAV. The editor also
opens MixJam v7 `.mixjam` projects and several classic `.mix` song formats.

## Run

Open `Start MixJam Techno.cmd`, or run from this folder:

~~~powershell
.\run.ps1
~~~

The executable is `target\release\mixjam-techno.exe`. The launcher builds it when
it is absent. Use `-Build` to rebuild after source changes.

On first launch, use **Libraries → Choose WAV folder**. The folder can contain
WAV files in subfolders. WAVs use the source BPM selected in Libraries (140 by
default). To select a folder and an initial song from the command line:

~~~powershell
.\run.ps1 -Samples '.\samples' -Mix '.\song.mixjam'
~~~

Settings and generated audio use `%LOCALAPPDATA%\MixJam Techno`. Set
`MIXJAM_TECHNO_DATA_DIR` to use another writable folder.

## Use

- Hover over the right-hand tool icons for action names.
- Filter sounds by library, category, or name. Double-click a sound to preview it.
- Drag a sound onto a track. Hold Ctrl for sixteenth-note positions. The
  arrangement and mixer omit unused tracks.
- Hold Shift to select clips. Drag an empty area to select a group. Hold Alt
  while dragging to copy. Use Delete, Undo, and Redo for edits.
- Press Space to play or pause. Drag across the bar ruler to select a loop.
- Use Mixer for track gain, pan, L/R routing, and master volume.
- Use Import for WAV files and enter the known source tempo.
- Use the sample tools to trim, change pitch, stretch tempo, or apply effects.
  Save the result as a new WAV sample.
- Use Step synth to sequence one note per sixteenth step across up to four bars.
- Select a recording track and press Record. Recording starts after an eight-beat
  count-in. Stop and save the take as a WAV file.
- Save songs as native JSON. Export writes stereo PCM16 WAV at 44.1 kHz.

Native JSON links to WAV files; it is not a portable audio bundle. After moving
the sample folder, select its new location before opening the project.

## Song import

**Open** accepts native JSON, MixJam v7 `.mixjam`, and eight classic `.mix`
profiles, named `d1`, `d2`, `d3`, `d4`, `h1`, `t1`, `t2`, and `t3`. Import selects
the nearest project tempo and shows a notice when the tempo changes. Songs that
use more than 64 tracks are rejected.

A `.mix` song stores numeric sample IDs. Resolving them needs sample-identity
catalogs, which are generated locally and are not part of this repository. A
build without them embeds empty catalogs: WAV folders, MixJam projects, and
native JSON work; `.mix` import cannot resolve its samples. To embed catalogs,
place `d1.json` … `t3.json` in `assets/catalog/` before building.

## Build and checks

The build needs Rust, the Visual Studio C++ build tools, and the Windows SDK.
`tools/runtime/install-rust.ps1` installs Rust into `.toolchain/` without changing
the user PATH; `tools/runtime/cargo.ps1` runs that toolchain.

~~~powershell
.\tools\runtime\cargo.ps1 build --release --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --locked
~~~

## Headless commands

~~~powershell
.\target\release\mixjam-techno.exe inspect --samples '.\samples'
.\target\release\mixjam-techno.exe convert --samples '.\samples' --mix 'song.mix' --output 'song.json'
.\target\release\mixjam-techno.exe render --samples '.\samples' --mix 'song.mixjam' --output 'song.wav'
~~~

`--samples PATH` selects a WAV folder. `--source-bpm BPM` supplies the source
tempo for unrecognized WAVs. `convert` saves native JSON; `render` writes stereo WAV.

## Known limits

- Original-format writing and portable project bundles are not implemented.
- Imported songs do not reproduce every effect or level curve of their source
  programs. MixJam master processing, return effects, and clip fades are not
  rendered; the importer shows a notice.
- Full-song playback and export are limited to about 12 minutes at 44.1 kHz.
