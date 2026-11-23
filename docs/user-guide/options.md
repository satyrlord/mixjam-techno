# Options

To open Options, select the **Options** gear at the left end of the status bar,
or press **Ctrl+,**. The window has three pages: **Audio**, **Workspace**, and
**Files**. The window keeps one height as you change pages, up to the height
of the screen. A longer page scrolls.

## Apply, Cancel, and Restore

- **Apply** saves the changes to the preferences and keeps the window open.
  Apply is available only when there are changes.
- A dot marks each page that has unapplied changes. The footer shows how many
  there are.
- **Restore all defaults** puts the defaults of every page in the draft. To
  save them, select **Apply**.

If the draft has unapplied changes and you select **Cancel**, press Escape, or
close the window, the window asks **Discard changes?**:

- **Apply changes** applies the changes and closes the window.
- **Discard changes** closes the window without the changes.
- **Keep editing** returns to the window.

Without changes, these actions close the window immediately. They do not
change the song, the undo history, the libraries, or the caches.

If a different control changes a preference while you have Options open, select
**Reload active options** before you select Apply. Reload replaces the draft
with the active preferences.

## Audio page

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
- Exports and song-tool renders use 4x for track Drive and High for sample
  playback. Renders through the master pedalboard use 8x master saturation.
  While one of these three settings is lower, Options shows **Playback
  quality reduced; exports use full quality**. The status bar shows **Reduced
  quality**. Select it to open Options.
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

## Workspace page

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

## Files page

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

## Sample libraries window

To open it, select **Open sample libraries** on the **Files** page of Options.
The library folder, scan, and provider actions in this window apply
immediately, so it has no Apply or Cancel. Close it with its close button.

Folder changes apply after a scan succeeds. The next **Add library
folder** or **Scan again** uses **Tempo for files without one**. If you close
the window, the library scan continues and its changes stay. To stop a scan,
select **Cancel library scan**.

## Options file

The application stores the options in `options.json` in the settings folder. If
the settings are not valid, the application uses safe defaults and shows an
error. To replace the settings that are not valid, select **Restore all
defaults**, then **Apply**. This build does not overwrite settings from a newer
options schema.
