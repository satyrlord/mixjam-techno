# Sample libraries and Options

## Sample libraries

At the first start, Options opens on its **Sample libraries** page. To add a
library:

1. Select **Add library folder**.
2. Select a folder of WAV files. The WAV files can be in subfolders.

To open the page later, select the **Options** gear, then **Sample
libraries**. If the Sounds panel has no sounds, **Open sample libraries** in
the panel also opens the page.

You can add a maximum of 9 library folders. The application scans all of them
and shows their sounds together in the Sounds panel. A library folder cannot be
in a different library folder. The page lists the folders in order:

- **Remove**, next to a folder, removes it from the list. The list always keeps
  one folder or more.
- **Scan again** reads all folders again: for example, after you add files or
  connect a drive.

### Scans

The scan runs in the background. The Sample libraries page shows the progress.
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
last frame, only that partial frame is lost. The scan ignores macOS `._`
metadata files beside the WAV files.

**Skipped WAVs** on the Sample libraries page lists the files that the
application cannot use, with the reason:

- An empty or damaged file.
- Audio that is cut off.
- A different encoding, for example ADPCM.

### Folders that cannot be read

If the application cannot read a library folder, or the folder has no WAV file
that it can read, the other folders still load. The status line names the
folder and the reason. The Sample libraries page shows the reason under the
folder. The folder stays in the list, but its sounds are not in the Sounds
panel until a later scan can read it. Songs that use these sounds show them as
missing. If the application cannot read any folder, the scan fails and the
sounds do not change.

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

This rule resolves the octave ambiguity of the original 180 BPM and 90 BPM
packs without folder names. For other folders, the application uses the loop
lengths to find a tempo. These sources can also set the tempo:

- A `BPM.INF` file of a pack can correct the estimate.
- A tempo and a key in a file name, for example
  `HARDCORE_BASS001_180_C_SL3.WAV`, can set the tempo of that file.

A folder tempo from authenticated audio overrides these estimates and names.

If the application cannot find a tempo, it uses **Tempo for files without
one** from the Sample libraries page. The default is 140 BPM. A loop length can
fit half or double its actual tempo. In this case, the application uses the
tempo from 88 to 176 BPM, unless **Tempo for files without one** is the other
tempo.

Before you scan a 180 BPM pack, set **Tempo for files without one** to 180 if
these conditions are both true:

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

## Options

To open Options, select the **Options** gear at the left end of the status bar,
or press **Ctrl+,**. The window has four pages: **Audio**, **Workspace**,
**Files**, and **Sample libraries**. The window fits the current page, up to
the height of the screen. A longer page scrolls.

### Apply, Cancel, and Restore

- **Apply** saves the changes to the preferences and keeps the window open.
  Apply is available only when there are changes.
- A dot marks each page that has changes that are not applied. The footer
  shows how many there are.
- **Restore options defaults** changes only the draft. To save the defaults,
  select **Apply**.

If you select **Cancel**, press Escape, or close the window with changes that
are not applied, the window asks **Discard changes?**:

- **Apply changes** applies the changes and closes the window.
- **Discard changes** closes the window without the changes.
- **Keep editing** goes back to the window.

Without changes, these actions close the window immediately. They do not
change the song, the undo history, the libraries, or the caches.

If a different control changes a preference while Options is open, select
**Reload active options** before you select Apply. Reload replaces the draft
with the active preferences.

### Audio page

- **Drive playback oversampling** and **Master playback oversampling**: the
  default is **4x**. To use less CPU during playback, set one or both to
  **Off**. Off still applies the saturation, which can sound harsher. The
  master setting applies only to Soft clip, Tube, Maximizer, and Tape.
- **Sample playback quality**: the default is **High**. The application reads
  between the samples of a sound that plays at a different tempo or sample
  rate, or under a Pitch lane. **High** reads through a filter that decreases
  aliasing: harsh, unrelated tones in the top frequencies. **Low** reads in
  straight lines between samples. Low uses less CPU, but it adds aliasing and
  makes the highest frequencies duller. A sound at its own tempo and sample
  rate, without a Pitch lane, sounds the same with both settings. When a sound
  plays slower, tones near its highest frequency can still lose level or make
  unwanted tones.
- Normal exports, track exports, and song-tool renders always use 4x and
  High. While one of these three settings is lower, Options and the status line
  show **Playback quality reduced; exports use full quality**.
- **Playback buffer target**: **Automatic**, **20 ms**, or **40 ms**. The
  default is 40 ms where the device supports it. A larger buffer can absorb
  bursts of processing, but it increases the delay between an action and its
  sound. The target is a request, not a measured callback interval. To see the
  request and the measured callback, open **Device details** under the target.
  The negotiated capacity is not available. If a target cannot open, Apply
  keeps the output that works and shows the error. At startup, if a target is
  not available, the application can use the system default with a warning.
- **Save audio capture**, under **Device details**, does the same as F12.
  Refer to [Audio captures](captures-and-limits.md#audio-captures).
- **Preview level** makes previews quieter, from -30 to 0 dB. **Mute
  previews** makes them silent. These controls apply to sound, clip, tool, and
  recorded-take previews. They do not change recordings, saved sounds, song
  volume, or exports.
- **Meter display rate**: **Reduced** shows meter values a maximum of 15 times
  each second. The audio analysis and the peak detection continue.

Before you apply audio changes, stop playback, previews, and recording.
Options does not stop them for you. You can apply Workspace and Files changes
during playback.

### Workspace page

The Workspace page has these settings:

- **Theme**.
- **Lane height**: 20, 30, or 40 px. The default is 30 px.
- **Follow playhead on startup**.
- **Show section bookmarks**.
- **Starting module**: Sounds, Mixer, Mastering, or FX returns.
- **Show tooltips**.

A theme or a lane height applies when you select Apply, also during playback.
The application then uses it at each later start. These two settings do not
change your song or its undo history.

Follow and Starting module apply at the next start. The **Follow** button in
the toolbar still sets Follow for the current session. **Show section
bookmarks** has the same saved value as the **Sections** button in the
toolbar. When tooltips are off, the control names, the keyboard focus, the
errors, and the visible instructions stay available.

### Files page

**Song dialog folder** sets where Open and Save as start:

- **Last used** starts in the last folder that you used.
- **Fixed folder** shows a folder field.

To use a fixed folder:

1. Select **Fixed folder**.
2. Select **Choose fixed song folder**, or type the full path of a folder that
   exists.
3. Select **Apply**.

If you cancel the folder picker, the draft does not change. If the folder is
not available later, the application keeps its path and shows the problem.
Open and Save as then use the last-used or default folder. Existing songs do
not move.

### Sample libraries page

The library folder, scan, and provider actions on this page apply
immediately. Thus the page has one **Done** button, not Apply and Cancel.
Done closes the window. If other pages have changes that are not applied, Done
asks first.

Folder changes apply after a scan is successful. The next **Add library
folder** or **Scan again** uses **Tempo for files without one**. If you close
Options, the library scan continues and its changes stay. To stop a scan,
select **Cancel library scan**.

### Options file

Options are stored in `options.json` in the settings folder. If the settings
are not valid, the application uses safe defaults and shows an error. To
replace the settings that are not valid, select **Restore options defaults**,
then **Apply**. This build does not overwrite settings from a newer options
schema.
