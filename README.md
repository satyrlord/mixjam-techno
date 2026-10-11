# MixJam Techno

A native Rust loop sequencer for Windows. Arrange WAV samples on up to 64 tracks
at 120, 140, 160, or 180 BPM, mix them, and export stereo WAV. The editor also
opens MixJam v7 `.mixjam` projects and several classic `.mix` song formats.

The interface uses a dark graphite workspace and bundled Space Grotesk fonts.
Transport sits above the arrangement. Sounds and Mixer share the lower panel;
sample categories, search, and library filters stay together.

## Run

Open `Start MixJam Techno.cmd`, or run from this folder:

~~~powershell
.\run.ps1
~~~

The executable is `target\release\mixjam-techno.exe`. The launcher builds it when
it is absent. Use `-Build` to rebuild after source changes.

On first launch, select **Libraries → Choose WAV folder**. The folder can have
WAV files in subfolders. The scan runs in the background. The Libraries window
shows progress and has a Cancel button. You can use the window during the scan.
The application changes the active folder when the scan is complete. It caches
decoded results and decodes changed files again on a later scan.

The application identifies extracted product samples by their audio, regardless
of filename. These samples keep their original tempo and length. For other WAVs,
the application uses loop lengths to find a tempo for each folder. If it cannot
find a tempo, it uses the fallback BPM from Libraries (140 by default). A loop
length can fit half or double its actual tempo. The application then prefers
88 to 176 BPM unless the fallback names the other tempo. Set the fallback to
180 before you scan a 180 BPM pack.

To select a folder and an initial song from the command line:

~~~powershell
.\run.ps1 -Samples '.\samples' -Mix '.\song.mixjam'
~~~

Settings, the scan cache, and generated audio use `%LOCALAPPDATA%\MixJam Techno`.
Set `MIXJAM_TECHNO_DATA_DIR` to use another writable folder.

## Use

- Hover over the right-hand tool icons for action names.
- Filter sounds by library, category, or name. Double-click a sound to preview it.
- Drag a sound onto a track. Hold Ctrl for sixteenth-note positions. The
  arrangement and mixer omit unused tracks.
- Hold Shift to select clips. Drag an empty area to select a group. Hold Alt
  while dragging to copy. Use Delete, Undo, and Redo for edits.
- Press Space to play or pause. Drag across the bar ruler to select a loop.
- Use Follow beside Zoom to keep the playhead visible. Turn it off to browse
  the timeline independently during playback.
- Drag the divider above Sounds or Mixer to give that panel more or less room.
- Use Mixer for track gain, pan, L/R routing, and master volume.
  Master stays visible while you scroll through tracks. Each strip shows its
  track number and the sound names on that track.
  Gain is a signed multiplier: 0 is silence, +1 is unity, and negative values
  reverse polarity. Edit the gain value for an exact setting, or select Unity
  to reset it. Pan shows left, center, or right; select Center to reset it.
  L and R enable each output. Both off shows Muted.
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

A `.mix` song stores numeric sample IDs. The application needs locally generated
sample identity catalogs to resolve them. This repository does not contain those
catalogs. Each catalog entry has a digest of the original sample's audio. The
application identifies a WAV when its audio matches, regardless of filename.
Digest comparisons require mono audio at the original 44.1 kHz rate, matching
frame counts, and more than two frames. If a comparison is unavailable, the
reader can infer identity from an unambiguous native code or name, category,
and duration. It rejects a WAV when
its available digest differs, even if its name and length match. The import
error names that WAV.

Without catalogs, the build embeds empty catalogs. WAV folders, MixJam projects,
and native JSON still work, but `.mix` import cannot resolve its samples. To
embed catalogs, put `d1.json` through `t3.json` in `assets/catalog/` and rebuild.

## Build and checks

The build needs Rust, the Visual Studio C++ build tools, and the Windows SDK.
`tools/runtime/install-rust.ps1` installs Rust into `.toolchain/` without changing
the user PATH; `tools/runtime/cargo.ps1` runs that toolchain.

~~~powershell
.\tools\runtime\cargo.ps1 build --release --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --no-default-features --locked
.\tools\runtime\cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
.\tools\runtime\cargo.ps1 fmt --all -- --check
~~~

See [ARCHITECTURE.md](ARCHITECTURE.md) for the enforced frontend/application
boundary. `tools/verify-ui.ps1` captures the built native interface at all three
supported window sizes with isolated synthetic fixtures and settings.
Mixer cases cover ordinary, 64-track, empty, and signed-gain/routing states.

## Headless commands

~~~powershell
.\target\release\mixjam-techno.exe inspect --samples '.\samples'
.\target\release\mixjam-techno.exe convert --samples '.\samples' --mix 'song.mix' --output 'song.json'
.\target\release\mixjam-techno.exe render --samples '.\samples' --mix 'song.mixjam' --output 'song.wav'
~~~

`--samples PATH` selects a WAV folder. `--source-bpm BPM` supplies the fallback
tempo for unrecognized WAVs in folders whose loop lengths do not agree on one.
`inspect` reports how many identities the audio verified and lists WAVs whose
audio differs from the sample they are named after. `convert` saves native JSON;
`render` writes stereo WAV.

## Known limits

- Original-format writing and portable project bundles are not implemented.
- Imported songs do not reproduce every effect or level curve of their source
  programs. MixJam master processing, return effects, and clip fades are not
  rendered; the importer shows a notice.
- Full-song playback and export are limited to about 12 minutes at 44.1 kHz.
