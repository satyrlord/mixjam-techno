# MixJam Techno

A native Rust loop sequencer for Windows. Arrange WAV samples on up to 64 tracks
at 120, 140, 160, or 180 BPM, mix them, and export stereo WAV. The editor also
opens MixJam v7 `.mixjam` projects and several classic `.mix` song formats.

The interface uses a dark graphite workspace, bundled Syncopate in uppercase
for branding and titles, and Inter Medium for all other application text.
Transport sits above the arrangement. Sounds and Mixer share the lower panel;
sample categories, search, and library filters stay together.
The title branding, window, taskbar, and Windows executable use the tribal mask.
See [the icon assets](assets/app/README.md) for source and regeneration details.

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
probe results and decodes changed files again on a later scan. Unchanged scans
leave the cache file intact. Scans process WAV audio as a stream to limit memory
use. Expand **Folder tempos** to see each folder's source tempo; long lists scroll.

The application identifies extracted product samples by their audio, regardless
of filename. These samples keep their original tempo and length. A folder also
uses original source tempo evidence when at least three WAVs, covering at least
75% of the folder, match original audio and all known tempos agree. This resolves
the Rave 180 BPM and HipHop 2/3 90 BPM octave ambiguity without using folder names.
For other folders, the application uses loop lengths to find a tempo. If it cannot
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
  The lane and mixer L/R controls mirror each other. Changing these controls
  does not change pan.
- Use Import for WAV files and enter the known source tempo.
- Use the sample tools to trim, change pitch, stretch tempo, or apply effects.
  Save the result as a new WAV sample.
- Use Step synth to sequence one note per sixteenth step across up to four bars.
- Select a recording track and press Record. Recording starts after an eight-beat
  count-in. Stop and save the take as a WAV file.
- Save songs as `.mixjam` files. Export writes stereo PCM16 WAV at 44.1 kHz.

A `.mixjam` song is JSON, format version 8. It links to WAV files; it is not a
portable audio bundle. After moving the sample folder, select its new location
before opening the song.

## Song import

**Open** accepts `.mixjam` songs, MixJam v7 `.mixjam` projects, JSON projects
from earlier builds, and eight classic `.mix`
profiles, named `d1`, `d2`, `d3`, `d4`, `h1`, `t1`, `t2`, and `t3`. Import selects
the nearest project tempo and shows a notice when the tempo changes. Songs that
use more than 64 tracks are rejected.

Classic `.mix` files can be up to 500 MB (500,000,000 bytes). Supported embedded
WAVs are imported automatically, so their original external files are not needed.
Songs can also reference factory samples from the selected library. Unavailable
audio appears as silent missing-sample placeholders at the saved or catalog length.
An inferred length is marked when the source duration is unknown. Hover over a
placeholder or open Import details to inspect the original metadata and reason.
Right-click a placeholder and select **Resolve manually** to choose a WAV file.
The choice resolves every identical missing reference across all tracks. Original
positions, spans, and musical trim offsets are retained. Cancel makes no change.
Imported audio is retained in the application's data folder for saved projects.
Keep that folder available when you reopen them. File size alone does not show
whether a song contains embedded audio.

Older app releases also saved version-7 `.mixjam` files with genre-based sample
paths. If the selected folder is `eJay\_original`, the importer can recover those
references from the containing `eJay` folder, including moved category folders.
Exact relative paths take priority. If several files match, the first match loads
and a **Sample conflicts** modal shows the choices. Select another path to update
all instances in the song immediately. Use **Play song** to compare the result,
then **Keep choices**. Esc hides the window without confirming. Reopen the
resolver from **Sample conflicts** in the status bar. This conflict behavior applies to every supported project format and
supports undo/redo and `.mixjam` save/reopen.
Resolved paths survive `.mixjam` save/reopen. An imported v7 `.mixjam`, `.mix`,
or JSON file is unchanged: **Save** asks for a new `.mixjam` file.

Dance 2 imports can also contain packed PXD samples. The reader decodes embedded
PXD data and retains it as WAV audio. Select the common parent sample folder when
a song uses sounds from earlier products. Dance 3 can reference Dance 2 sounds;
the supported Techno 2 compatibility bank uses Dance 2 IDs 1 through 1957.

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
and saved songs still work; unresolved `.mix` sources open as placeholders. To
embed catalogs, put `d1.json` through `t3.json` in `assets/catalog/` and rebuild.
The optional `source-tempos.json` adds HipHop 2/3 tempo identities. It is
generated from the original media by the owner-local
`examples/export_source_tempos.rs` (the `examples/` folder is not published),
with `MIXJAM_TECHNO_MEDIA_ROOT` set, then rebuilt into the executable. The source products' shipped help
states that all their samples were recorded at 90 BPM. This metadata adds tempo
evidence; it does not add HipHop song-format support.

## Build and checks

The build needs Git with a complete checkout history, Rust, the Visual Studio
C++ build tools, and the Windows SDK. For a shallow clone, run
`git fetch --unshallow` before building.
`tools/runtime/install-rust.ps1` installs Rust in `local-data/toolchain/`, without
changing the user PATH. `tools/runtime/cargo.ps1` runs that toolchain. Required
private project data and tools belong in `local-data/`; temporary files belong in
`tmp/`. Preserved project snapshots and bundles remain in `local-data/legacy/`.
The repo is the public-facing files. The project includes public and private
files; Gitignored `local-data/` and `tmp/` remain private project files.

The bottom bar shows the application version with a `v` prefix, such as
`v1.005`, at its right margin. Select it to
open the GitHub repository in the operating system's default browser. The link
requests a new tab; the browser controls tab or window placement. `build.rs`
combines the manually maintained `APP_MAJOR_VERSION` (currently 1) with
`git rev-list --count HEAD`, padded to at least three digits. Five commits gives
`1.005`. Cargo recalculates it when Git references change. Saved `.mixjam` songs
use the same version. The installed application does not need Git. Cargo needs a
semantic package version, so `tools/runtime/cargo.ps1` writes the same version to
`Cargo.toml` and `Cargo.lock` in that form before each run: `1.005` is `1.5.0`.
The major of the package version must equal `APP_MAJOR_VERSION`.

~~~powershell
.\tools\runtime\cargo.ps1 build --release --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --no-default-features --locked
.\tools\runtime\cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
.\tools\runtime\cargo.ps1 -CargoArguments @('fmt','--all','--','--check')
~~~

See [ARCHITECTURE.md](ARCHITECTURE.md) for the enforced frontend/application
boundary. `tools/verify-ui.ps1` captures the built native interface at all three
supported window sizes with isolated synthetic fixtures and settings.
Mixer cases cover ordinary, 64-track, empty, and signed-gain/routing states.
The capture set also covers every in-app dialog and tool window, expanded effects
and provider settings, and missing-source processing states. Recorded-take captures
check the window layout with a synthetic presentation state; they do not record audio.

Visual assets are transparent PNGs painted at one image pixel per screen pixel.
Prepared density variants support Windows scaling. SVG files and embedded SVG
artwork are prohibited in tracked and non-ignored project files. Run
`python tools/assets/check-raster-assets.py` as part of the quality gate. Original
legacy archives and required dependency caches stay in ignored `local-data/`;
temporary files stay in ignored `tmp/`. The asset check respects Git ignore rules.

## Headless commands

~~~powershell
.\target\release\mixjam-techno.exe inspect --samples '.\samples'
.\target\release\mixjam-techno.exe convert --samples '.\samples' --mix 'song.mix' --output 'song.mixjam'
.\target\release\mixjam-techno.exe render --samples '.\samples' --mix 'song.mixjam' --output 'song.wav'
~~~

`--samples PATH` selects a WAV folder. `--source-bpm BPM` supplies the fallback
tempo for unrecognized WAVs in folders whose loop lengths do not agree on one.
`inspect` reports how many identities the audio verified and lists WAVs whose
audio differs from the sample they are named after. `convert` saves a `.mixjam` song;
`render` writes stereo WAV.

## Known limits

- Original-format writing and portable project bundles are not implemented.
- Imported songs do not reproduce every effect or level curve of their source
  programs. MixJam master processing, return effects, and clip fades are not
  rendered; the importer shows a notice.
