# Command line

The executable has headless commands for inspection, conversion, and rendering.
They open no window and play no sound.

~~~powershell
.\target\release\mixjam-techno.exe inspect --samples '.\samples'
.\target\release\mixjam-techno.exe convert --samples '.\samples' --mix 'song.mix' --output 'song.mixjam'
.\target\release\mixjam-techno.exe render --samples '.\samples' --mix 'song.mixjam' --output 'song.wav'
.\target\release\mixjam-techno.exe render --samples '.\samples' --mix 'song.mixjam' --stems '.\song tracks'
~~~

`--samples PATH` selects a WAV folder. `--source-bpm BPM` supplies the fallback
tempo for unrecognized WAVs in folders whose loop lengths do not agree on one.
`inspect` reports how many identities the audio verified and lists WAVs whose
audio differs from the sample they are named after. `convert` saves a `.mixjam` song;
`render` writes stereo WAV. With `--stems FOLDER` instead of `--output`, `render`
writes what Export tracks writes: each track that plays and each fed FX
return to its own WAV in that folder, which it creates if needed, without the
master pedalboard. A track or return that exceeds the 16-bit range stops the
export before destination files change. Lower its level or the master gain
and retry. The files sum to the bypassed mix when that mix does not clip.

To start the window with a WAV folder and a song, use the launcher:

~~~powershell
.\run.ps1 -Samples '.\samples' -Mix '.\song.mixjam'
~~~

`mixjam-techno --help` lists every option. [AGENT-CONTROL.md](AGENT-CONTROL.md)
describes the `agent` command.
