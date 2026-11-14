# Audio captures and known limits

## Audio captures

The application keeps the last 30 seconds that playback sent to the audio
device. Press **F12**, anywhere in the window, or select **Save audio capture**
under **Device details** on the Audio page of Options, to save them. The status
line names the new folder: `diagnostics\<date>_<time>` in the data folder
(`%LOCALAPPDATA%\MixJam Techno`, or `MIXJAM_TECHNO_DATA_DIR`). It holds:

- `device.wav`: what was written to the device, as 32-bit float.
- `source.wav`: the mixer samples before conversion to the device format
  and before invalid samples (NaN or infinity) were replaced by silence.
- `state.json`: the song's title and file, the transport and the playhead's bar
  and tick, the time, the audio device and its format, callback counts,
  maximum fill time and latest measured interval, device open, fallback, and
  error events, the Options, the
  master pedalboard and FX buses, and what a capture does not record.

The application also saves one by itself, four seconds after it detects loud,
broadband noise in the output for more than a second, finds NaN or infinity,
or sees a device error. Replacing the output saves a pending
capture early. It saves at most one such capture in five minutes and says so
in the status line. Keep the application open until that line confirms the
save. These files do not record changes made by the system mixer or driver,
or the sound from the speakers. Send the folder with a description of what
you heard.

## Known limits

- No song, recording, sample, or edit has a fixed duration limit. The limits
  that remain come from the file formats:
  - A WAV file holds at most 4 GiB. A 16-bit WAV holds about 2.1 billion
    samples across all its channels: more than 13 hours of mono audio at
    44.1 kHz. A mix export is stereo, so it holds about 6.7 hours; a song
    that is longer cannot be exported as one WAV file, and the export says so
    before it writes any file. Stems follow the same limit.
  - A recording stops when its take reaches the 16-bit WAV limit, which is
    more than 13 hours at 44.1 kHz. It has no shorter time limit.
  - An edited, stretched, or synthesized sample, and a sample with effects,
    can be as long as a WAV file holds, and no longer.
  - A sample file of the first original product is at most 2 GiB, the
    largest file that product's loader can read.
  - A packed sample is at most 64 MiB, and it may declare at most 67,108,864
    samples (about 25 minutes at 44.1 kHz). Installed samples are far
    smaller; the limit rejects a damaged file.
  - A legacy `.mix` file is at most 500 MB, a native song is at most 1 GiB,
    and the sample scan stops at 64 folder levels.
- Original-format writing and portable project bundles are not implemented.
- A song that uses both an original packed provider and supplemental WAVs for
  the same product cannot be saved. The current format records only one folder
  per product. Save leaves an existing song unchanged and explains the conflict.
- Imported songs do not reproduce every effect or level curve of their source
  programs. MixJam v7 clip edge fades are not rendered, and its master bus and return
  effects play through this application's own mastering pedals and FX buses;
  the importer shows a notice.
