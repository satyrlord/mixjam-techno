# Sample libraries

## Contents

[Options](options.md#options) has its own page.

## Library folders

At the first start, the **Sample libraries** window opens. To add a library:

1. Select **Add library folder**.
2. Select a folder of WAV files. The WAV files can be in subfolders.

To open the window later, select the **Options** gear, then **Files**, then
**Open sample libraries**. If the Sounds panel has no sounds, **Open sample
libraries** in the panel also opens the window.

You can add a maximum of 9 library folders. The application scans all of them
and shows their sounds together in the Sounds panel. A library folder cannot be
in a different library folder, and it cannot be in the list two times. On
Windows, these two checks ignore letter case. The window lists the folders in
order:

- **Remove**, next to a folder, removes it from the list. The list always keeps
  one folder or more.
- **Scan again** reads all folders again: for example, after you add files or
  connect a drive.

### Optional product providers

Under the folder list, the **Optional product providers** section adds a
provider folder for one original product. Select the product in **Provider
product**, then select **Choose folder**. **No providers** shows when the list
is empty. **Remove**, next to a provider, removes it. A product has one
provider folder. To use another folder, remove the first one.

### Scans

The scan runs in the background. The Sample libraries window shows the progress.
To stop a scan, select **Cancel library scan**. You can use Options during the
scan. The application changes the sounds only when the scan is complete. A
cancelled scan changes nothing.

The application keeps the probe results in a cache. A later scan decodes only
the changed files again. A decoder update also makes the application read the
older cached results again. A scan without changes does not change the cache
file. A scan reads WAV audio as a stream, to use less memory. There is no limit
on the number of WAV files. To see the source tempo of each folder, expand
**Folder tempos**. A long list scrolls.

### WAV files that a scan reads

A scan reads mono and stereo WAV files in these encodings:

- 8-, 16-, 24-, or 32-bit PCM.
- 32-bit float.
- 24-bit PCM in four-byte containers.

A scan also reads extensible headers, long format chunks, and odd-length
metadata. A scanned file has no length limit of its own. It can be as long as a
WAV file can be, which is almost 4 GiB. If the audio ends in the middle of its
last frame, the scan drops only that partial frame. The scan ignores macOS `._`
metadata files beside the WAV files.

**Skipped WAVs** on the Sample libraries window lists the files that the
application cannot use, with the reason:

- An empty or damaged file.
- Truncated audio.
- A different encoding, for example ADPCM.

### Folders that cannot be read

If the application cannot read a library folder, the other folders still load.
This also applies when the folder has no WAV file that it can read. The status
line names the folder and the reason. The Sample libraries window shows the
reason under the folder. The folder stays in the list. Its sounds are not in the
Sounds panel until a later scan can read it. Songs that use these sounds show
them as missing. If the application cannot read any folder, the scan fails and
the sounds do not change.

The scan skips a link in a library folder that goes to a place outside that
folder. The rest of the folder loads.

If two or more folders contain original product samples, the first folder in
the list that has samples of a product supplies that product. Later folders add
only the samples that it does not have. If a later folder has an exact copy of
a sample that an earlier folder supplies, the list does not show it again.

### Sample tempos

The application identifies extracted product samples by their audio. The file
name is not important. These samples keep their original tempo and length. An
identified product sample keeps its catalog tempo.

A folder also uses the original source tempo when these conditions are all
true:

- Three WAV files or more match original audio.
- These files are 75% or more of the folder.
- All known tempos agree.

This rule finds the tempo of an extracted product pack without its folder name.
For other folders, the application uses the loop lengths to find a tempo. These
sources can also set the tempo:

- A `BPM.INF` file of a pack can correct the estimate.
- A tempo and a key in a file name, for example
  `HARDCORE_BASS001_180_C_SL3.WAV`, can set the tempo of that file.

A folder tempo from authenticated audio overrides these estimates and names.

If the application cannot find a tempo, it uses **Tempo for files without one**
from the Sample libraries window. The default is 140 BPM. A loop length can fit
half or double its actual tempo. In this case, the application uses the tempo
from 88 to 176 BPM, unless **Tempo for files without one** is the other tempo.

Before you scan a 180 BPM pack, set **Tempo for files without one** to 180 if
both conditions are true:

- No file or folder states the tempo.
- Not enough authenticated audio agrees on the tempo.

### Sample tempo in a song

A song plays each sound in the octave of its tempo that is nearest to the song
tempo. Thus no sound plays more than half an octave faster or slower. For
example, a 90 BPM loop in a 180 BPM song plays at its own pitch as a half-time
loop. Each bar of the loop then fills two bars of the song, and a new clip of
the loop gets that length. Imported MixJam v7 sounds keep their saved
durations. Their calculated playback tempos do not use octave selection.

### Data folder

The settings, the scan cache, and the generated audio are in
`%LOCALAPPDATA%\MixJam Techno`. To use a different writable folder, set
`MIXJAM_TECHNO_DATA_DIR`.
