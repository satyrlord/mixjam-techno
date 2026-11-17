# Command line

The executable has headless commands for inspection, conversion, and render.
They open no window and play no sound.

~~~powershell
.\target\release\mixjam-techno.exe inspect --samples '.\samples'
.\target\release\mixjam-techno.exe convert --samples '.\samples' --mix 'song.mix' --output 'song.mixjam'
.\target\release\mixjam-techno.exe render --samples '.\samples' --mix 'song.mixjam' --output 'song.wav'
.\target\release\mixjam-techno.exe render --samples '.\samples' --mix 'song.mixjam' --stems '.\song tracks'
~~~

## Sample folders

`--samples PATH` selects a WAV folder. To use more than one library folder,
give `--samples` up to 9 times. The application merges the folders into one set
of sounds, as the Sample libraries window does. A folder that cannot be
read is left out, with a warning on standard error. The command fails when no
folder can be read.

`--source-bpm BPM` gives the fallback tempo for unrecognized WAVs in folders
whose loop lengths do not agree on one tempo. The default is 140.

~~~powershell
.\target\release\mixjam-techno.exe render --samples '.\drums' --samples '.\loops' --mix 'song.mixjam' --output 'song.wav'
~~~

## Commands

`inspect` reports how many identities the audio verified. It lists the WAVs
whose audio is different from the sample that gives them their name. It also
lists the sample catalogs that loaded from `assets/catalog`, and each catalog
file that did not load, with the reason. Each command reports such a file on
standard error.

`convert` saves a `.mixjam` song. `render` writes a stereo WAV.

With `--stems FOLDER` in place of `--output`, `render` writes the same files as
Export tracks. It writes each track that plays and each FX return that gets
signal to its own WAV in that folder, without the master pedalboard. If the
folder does not exist, `render` makes it. If a track or return goes above the
16-bit range, the export stops before the destination files change. Then
decrease its level or the master gain, and try again. When the bypassed mix
does not clip, the sum of the files is equal to that mix.

## Launcher

To start the window with a WAV folder and a song, use the launcher. `-Samples`
takes a list of up to 9 folders:

~~~powershell
.\run.ps1 -Samples '.\samples' -Mix '.\song.mixjam'
.\run.ps1 -Samples '.\drums','.\loops' -Mix '.\song.mixjam'
~~~

`mixjam-techno --help` lists all options. [AGENT-CONTROL.md](AGENT-CONTROL.md)
describes the `agent` command.
