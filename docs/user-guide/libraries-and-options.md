# Sample libraries and Options

## Sample libraries

On first launch, Options opens its **Sample libraries** page. Select
**Add library folder**. Later, select the **Options** gear and **Sample libraries**. A library folder can
have WAV files in subfolders. You can add up to 9 library folders. The application
scans all of them and shows their sounds together in Sounds. The Sample libraries page
lists the folders in order. **Remove** next to a folder takes it out of the list;
the list keeps at least one folder. **Scan again** reads every folder again, for
example after you add files or connect a drive. A library folder cannot be inside
another one.

The scan runs in the background. The Sample libraries page shows progress and has a
**Cancel library scan** button. You can use Options during the scan. The application changes
the sounds when the scan is complete; a cancelled scan changes nothing. It caches
probe results and decodes changed files again on a later scan. A decoder update
also causes older cached results to be read again. Unchanged scans
leave the cache file intact. Scans process WAV audio as a stream to limit memory
use. There is no limit on the number of WAV files. Expand **Folder tempos** to see
each folder's source tempo; long lists scroll.

Scans read mono and stereo WAVs in 8-, 16-, 24-, or 32-bit PCM and 32-bit float,
including extensible headers, long format chunks, and odd-length metadata.
Classic 24-bit PCM in four-byte containers is also supported. A scanned file has
no length limit of its own; it can be as long as a WAV file can be, which is just
under 4 GiB. Audio that ends inside its last frame loses only that partial frame.
**Skipped WAVs** on the Sample libraries page lists files that cannot be used, with
the reason: an empty or damaged file, audio that is cut off, or another encoding
such as ADPCM. macOS `._` metadata files beside the WAVs are ignored.

If a library folder cannot be read, or has no readable WAV, the other folders
still load. The status line names the folder and the reason, and the Sample libraries
page shows it under the folder. The folder stays in the list, but its sounds
are not in Sounds until a later scan can read it. Songs that use those sounds
show them as missing. When no folder can be read, the scan fails and the sounds
do not change. When several folders contain original product samples, the
first folder in the list that has a product's samples provides it, and later
folders add the samples that it lacks. A later folder's exact copy of a sample
that an earlier folder already provides is not listed again.

The application identifies extracted product samples by their audio, regardless
of filename. These samples keep their original tempo and length. A folder also
uses original source tempo evidence when at least three WAVs, covering at least
75% of the folder, match original audio and all known tempos agree. This resolves
the octave ambiguity of the original 180 BPM and 90 BPM packs without using folder
names. For other folders, the application uses loop lengths to find a tempo.
A pack's `BPM.INF` can correct that estimate. A tempo and key in a file name,
such as `HARDCORE_BASS001_180_C_SL3.WAV`, can set the tempo for that file.
An authenticated folder tempo takes precedence over these estimates and names.
An identified product sample keeps its catalog tempo.
If the application cannot find a tempo, it uses **Tempo for files without one** from the
Sample libraries page (140 BPM by default). A loop length can fit half or double its actual
tempo. The application then prefers 88 to 176 BPM unless that tempo names the other
one. Set it to 180 before you scan a 180 BPM pack when neither a
stated tempo nor enough agreeing authenticated audio establishes its tempo.

A song plays each sound in the octave of its tempo nearest the song tempo, so no
sound is sped up or slowed down by more than half an octave. A 90 BPM loop in a
180 BPM song plays at its own pitch as a half-time loop, and each of its bars
spans two of the song's bars. A new clip of it is placed at that length.
Imported MixJam v7 sounds keep their saved durations; their calculated playback
tempos do not use octave selection.

Settings, the scan cache, and generated audio use `%LOCALAPPDATA%\MixJam Techno`.
Set `MIXJAM_TECHNO_DATA_DIR` to use another writable folder.

## Options

Select the **Options** gear at the foot of the tool rail, or press **Ctrl+,**.
The window has Audio, Workspace, Files, and Sample libraries pages. It fits the
current page, up to the screen height; a longer page scrolls. **Apply** saves
preference changes and keeps the window open. It is available only when
something has changed. A dot marks each page that has unapplied changes, and
the footer shows how many there are. **Cancel**, Escape, or close with
unapplied changes asks **Discard changes?**: **Apply changes** applies them and
closes the window, **Discard changes** closes it without them, and **Keep
editing** returns to the window. Without changes, these close the window at
once. **Restore options defaults** changes the draft; select Apply to save
it. These actions do not change the song, undo history, libraries, or caches.
If another control changes a preference while Options is open, select **Reload active options**
before Apply. Reload replaces the draft with the active preferences.

On **Audio**:

- **Drive playback oversampling** and **Master playback oversampling** default
  to **4x**. Set either to **Off** to use less CPU during playback. Off still
  runs saturation, which can sound harsher. The master switch affects Soft
  clip, Tube, Maximizer, and Tape only. Normal exports, track exports, and
  song-tool renders always use 4x.
- **Sample playback quality** defaults to **High**. A sound that plays off its
  own tempo or sample rate, or under a Pitch lane, is read between its
  samples. High reads it through a filter that reduces aliasing: harsh,
  unrelated tones in the top end. **Low** reads it in straight lines between
  samples, which uses less CPU but adds that aliasing and dulls the highest
  frequencies. A sound at its own tempo and sample rate without a Pitch lane
  sounds the same in both. Tones near the source's highest frequency can
  still lose level or produce unwanted tones when slowed. Normal exports,
  track exports, and song-tool renders always use High.
- Options and the status line show **Playback quality reduced; exports use
  full quality** while any of these three is lowered.
- **Playback buffer target** offers **Automatic**, **20 ms**, and **40 ms**.
  The default is 40 ms where supported. A larger buffer can absorb processing
  bursts and increase the delay between an action and its sound. The target
  is a request, not a measured callback interval. Open **Device details** under
  it to see the request and the observed callback separately. Negotiated
  capacity is unavailable.
  If a target cannot open, Apply keeps the working output and reports the error.
  At startup, an unavailable target can use the system default with a warning.
- **Save audio capture**, under **Device details**, does what F12 does: see
  [Audio captures](captures-and-limits.md#audio-captures).
- **Preview level** attenuates previews from -30 to 0 dB; **Mute previews**
  silences them. These controls cover sound, clip, tool, and recorded-take
  previews. They do not change recordings, saved sounds, song volume, or exports.
- **Meter display rate → Reduced** displays meter values at most 15 times per
  second. Audio analysis and peak detection remain active.

Stop playback, previews, and recording before you apply audio changes.
Options does not stop them for you. Workspace and Files changes can apply
while playback continues.

On **Workspace**, choose the **Theme** and the **Lane height** (20, 30, or
40 px; 30 by default), and set **Follow playhead on startup**,
**Show section bookmarks**, **Starting module**, and **Show tooltips**. A theme
or lane height applies as soon as you select Apply, even while a song plays,
and the app opens with it at every later launch. Neither changes your song or
its undo history. Follow and Starting module apply on the next launch. The toolbar still controls Follow for the current session.
Show section bookmarks shares its saved value with the Sections toolbar toggle.
Turning tooltips off keeps control names, keyboard focus, errors, and visible
instructions available.

On **Files**, **Song dialog folder → Last used** keeps the current Open and
Save as behavior. Select **Fixed folder** to show the folder field, then choose
or enter an existing absolute folder and Apply to use it as the initial
location. Cancelling the
picker leaves the draft unchanged. If the folder later becomes unavailable,
the app keeps its configured path, reports the problem, and uses the last-used
or default location. Existing songs stay in place.

On **Sample libraries**, library folder, scan, and provider actions take effect
right away, so the page has a single **Done** button instead of Apply and
Cancel. Done closes the window, and asks first if other pages have unapplied
changes. Folder changes take effect after a successful scan. **Tempo for files
without one** is used by the next Add library folder or Scan again. Closing
Options does not stop or roll back a library scan. Use **Cancel library scan**
to cancel a scan.

Options are stored in `options.json` in the settings folder. Invalid settings
use safe defaults with an error; Restore options defaults and Apply can replace
them. Settings from a newer options schema stay intact and cannot be overwritten
by this build.
