# Audio captures and known limits

## Audio captures

The application keeps the last 30 seconds that playback sent to the audio
device. To save them, do one of these steps:

- Press **F12** anywhere in the window.
- On the Audio page of Options, open **Device details** and select **Save audio
  capture**.

The status line names the new folder: `diagnostics\<date>_<time>` in the data
folder (`%LOCALAPPDATA%\MixJam Techno`, or `MIXJAM_TECHNO_DATA_DIR`). The
folder holds:

- `device.wav`: the audio that the application wrote to the device, as 32-bit
  float.
- `source.wav`: the mixer samples before the conversion to the device format.
  It keeps samples that are not valid (NaN or infinity). The device audio has
  silence in their place.
- `state.json`: these values:
  - The title and the file of the song.
  - The transport, and the bar and tick of the playhead.
  - The time.
  - The audio device and its format.
  - The callback counts, the maximum fill time, and the latest measured
    interval.
  - The events for device open, fallback, and errors.
  - The Options.
  - The master pedalboard and the FX buses.
  - What a capture does not record.

The application also saves a capture automatically when one of these occurs:

- Loud, broadband noise in the output for more than one second.
- NaN or infinity in the output.
- A device error.

The application saves an automatic capture four seconds after the event. If a
new output replaces the old one, it saves a pending capture earlier. The
application saves a maximum of one automatic capture in five minutes, and the
status line tells you about it. Keep the application open until the status line
confirms the save.

The files do not record changes from the system mixer or the driver, or the
sound from the speakers. Send the folder with a description of what you heard.

## Known limits

A song has 4096 bars at most, and no clip can end after bar 4096. This is about
2.3 hours at 120 BPM and 1.5 hours at 180 BPM. A song holds up to 1,000,000
clips. A recording, a sample, or an edit has no fixed duration limit. The file
formats set these limits:

- A WAV file holds a maximum of 4 GiB. A 16-bit WAV file holds about 2.1
  billion samples across all its channels. This is more than 13 hours of mono
  audio at 44.1 kHz.
- A mix export is stereo, so it holds about 6.7 hours. The application cannot
  export a longer song as one WAV file. The export tells you this before it
  writes a file. Stems have the same limit.
- A recording stops at the 16-bit WAV limit, more than 13 hours at 44.1 kHz.
  It has no shorter time limit.
- An edited, stretched, or synthesized sample can be as long as a WAV file can
  hold. A sample with effects has the same limit.
- A sample file of the first original product is a maximum of 2 GiB. This is
  the largest file that the loader of that product can read.
- A packed sample is a maximum of 64 MiB. It can declare a maximum of
  67,108,864 samples (about 25 minutes at 44.1 kHz). Installed samples are
  much smaller. The limit rejects a damaged file.
- A legacy `.mix` file is a maximum of 500 MB, and a native song is a maximum
  of 1 GiB. The sample scan stops at 64 folder levels.

Other limits:

- The application cannot write the original formats or portable project
  bundles.
- The application cannot save a song that uses an original packed provider
  and supplemental WAV files for the same product. The current format records
  only one folder for each product. Save does not change an existing song and
  tells you the conflict.
- Imported songs do not play each effect or level curve of their source
  programs as the source does. The application does not render the clip edge
  fades of MixJam v7. The master bus and the return effects of MixJam v7 play
  through the mastering pedals and FX buses of this application. The importer
  shows a notice about this.
