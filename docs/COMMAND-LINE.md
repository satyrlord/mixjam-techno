# Command line

The executable has headless commands for inspection, conversion, and render.
They open no window and play no sound. With no command, the executable runs
`gui`, which opens the window.

~~~powershell
.\target\release\mixjam-techno.exe inspect --samples '.\samples'
.\target\release\mixjam-techno.exe convert --samples '.\samples' --mix 'song.mix' --output 'song.mixjam'
.\target\release\mixjam-techno.exe render --samples '.\samples' --mix 'song.mixjam' --output 'song.wav'
.\target\release\mixjam-techno.exe render --samples '.\samples' --mix 'song.mixjam' --stems '.\song tracks'
~~~

## Sample folders

`--samples PATH` selects a WAV folder. To use more than one library folder, give
`--samples` up to 9 times. The application merges the folders into one set of
sounds, as the Sample libraries window does. The application skips a folder that
it cannot read and prints a warning on standard error. The command fails when it
cannot read any folder.

`--source-bpm BPM` gives the fallback tempo for unrecognized WAVs in folders
whose loop lengths do not agree on one tempo. The default is 140. The tempo
must be a number above zero.

~~~powershell
.\target\release\mixjam-techno.exe render --samples '.\drums' --samples '.\loops' --mix 'song.mixjam' --output 'song.wav'
~~~

## Provider folders

`--library PRODUCT=PATH` gives the folder of the optional provider for one
product. `PRODUCT` is a product code that `--help` lists. Give `--library`
once for each product. `--app PATH` does the same for the provider that the
development fixtures use. A provider that you name here replaces the provider
of the same product in the WAV folder.

## Commands

`inspect` reports how many identities the audio verified. It lists the WAVs
whose audio is different from the sample that gives them their name. It also
lists the sample catalogs that loaded from `assets/catalog`. It lists each
catalog file that did not load, with the reason. Each command reports such a
file on standard error.

`convert` needs `--mix` and `--output`. It saves a `.mixjam` song.

`render` needs `--mix` and one of `--output` or `--stems`. It writes a stereo
WAV.

With `--stems FOLDER` in place of `--output`, `render` writes the same files as
Export tracks. It writes each track that plays and each FX return that gets
signal to its own WAV in that folder. It skips the master pedalboard. If the
folder does not exist, `render` makes it. If a track or return goes above the
16-bit range, the export stops before the destination files change. Then
decrease its level or the master gain, and try again. When the bypassed mix
does not clip, the sum of the files is equal to that mix. If no track plays,
`render` exits with code 3.

`verify-fixtures` needs `--app`. It is a development command. It reads the demo
songs in the fixture folder of that provider and prints the clip count of each
one.

`--help` or `-h` prints the usage and exits.

Every command that fails exits with code 1, except as stated above.

## Launcher

To start the window with a WAV folder and a song, use the launcher. `-Samples`
takes a list of up to 9 folders:

~~~powershell
.\run.ps1 -Samples '.\samples' -Mix '.\song.mixjam'
.\run.ps1 -Samples '.\drums','.\loops' -Mix '.\song.mixjam'
~~~

These launcher parameters also exist:

- `-SourceBpm BPM` sets the fallback tempo. It applies with `-Samples`.
- `-App PATH` passes `--app`.
- `-Build` builds the release executable before it starts the window.
- `-Agent` passes `--agent`. `-AgentPort PORT` passes `--agent-port`.

## Agent control

`--agent` lets a local agent operate the window through port 47800 and
`--agent-port PORT` selects another port. To send a request to the running
window, use `mixjam-techno agent [--port PORT] COMMAND [name=value ...]`.
`mixjam-techno --help` lists all options. [AGENT-CONTROL.md](AGENT-CONTROL.md)
describes the `agent` command.
