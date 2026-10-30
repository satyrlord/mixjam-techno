# User guide

## Contents

- [Interface](#interface)
- [Sample libraries](#sample-libraries)
- [Arrange](#arrange)
- [Mixer](#mixer)
- [Mastering](#mastering)
- [Effects](#effects)
- [Sound tools and recording](#sound-tools-and-recording)
- [Album Review](#album-review)
- [Save and export](#save-and-export)
- [Open songs from other programs](#open-songs-from-other-programs)
- [Known limits](#known-limits)

## Interface

The interface uses a dark graphite workspace, bundled Syncopate in uppercase
for branding and titles, and Inter Medium for all other application text.
Transport sits above the arrangement. Sounds, Mixer, Mastering, and Effects share the lower panel;
use its left-side module selector to switch between them. Sample categories,
search, and library filters stay together in Sounds; search and filters sit
beside the selector.
The title branding, window, taskbar, and Windows executable use the tribal mask.
See [the icon assets](../assets/app/README.md) for source and regeneration details.
[DESIGN.md](DESIGN.md) is the visual contract.

Hover over the right-hand tool icons for action names.

## Sample libraries

On first launch, select **Libraries → Add library folder**. A library folder can
have WAV files in subfolders. You can add up to 9 library folders. The application
scans all of them and shows their sounds together in Sounds. The Libraries window
lists the folders in order. **Remove** next to a folder takes it out of the list;
the list keeps at least one folder. **Scan again** reads every folder again, for
example after you add files or connect a drive. A library folder cannot be inside
another one.

The scan runs in the background. The Libraries window shows progress and has a
Cancel button. You can use the window during the scan. The application changes
the sounds when the scan is complete; a cancelled scan changes nothing. It caches
probe results and decodes changed files again on a later scan. Unchanged scans
leave the cache file intact. Scans process WAV audio as a stream to limit memory
use. There is no limit on the number of WAV files. Expand **Folder tempos** to see
each folder's source tempo; long lists scroll.

If a library folder cannot be read, or has no readable WAV, the other folders
still load. The status line names the folder and the reason, and the Libraries
window shows it under the folder. The folder stays in the list, but its sounds
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
names. For other folders, the application uses loop lengths to find a tempo. If it
cannot find a tempo, it uses the fallback BPM from Libraries (140 by default). A loop
length can fit half or double its actual tempo. The application then prefers
88 to 176 BPM unless the fallback names the other tempo. Set the fallback to
180 before you scan a 180 BPM pack.

Settings, the scan cache, and generated audio use `%LOCALAPPDATA%\MixJam Techno`.
Set `MIXJAM_TECHNO_DATA_DIR` to use another writable folder.

## Arrange

- Filter sounds by library, category, or name. Double-click a sound to preview it.
- Drag a sound onto a track. Hold Ctrl for sixteenth-note positions. The
  arrangement and mixer omit unused tracks.
- Hold Shift to select clips. Drag an empty area to select a group. Hold Shift
  or Alt while dragging a clip to copy it. A ghost shows where the clips land.
  Every clip of a group must land on a shown track or the Add row. If one would
  land on an unused track, a label names that track and the drop does nothing.
  Use Delete, Undo, and Redo for edits.
- Drag a clip's side edges to trim it. Drag its top corner handles to set fades,
  or the small handle in its middle up or down to set gain in 1 dB steps (0.1 dB
  with Ctrl). Each completed drag is one undo step. Right-click a clip, or
  select it and press Shift+F10, for exact gain,
  trim start, length, fades in beats with their curves (Linear, Equal power, or
  Exponential), and pitch in semitones and cents. The fade out ends
  where the clip's sound ends, which can be before the clip's end, so a phrase
  that stops abruptly can die away instead. Pitch keeps the clip's length and
  timing. The arrangement shades each fade over its clip.
- Choose a lane in Automation above the arrangement to draw it on every track:
  Volume (dB over the fader), FX 1 to FX 4 send (the amount, which replaces
  the send knob while the lane has points), or Filter (low-pass below the
  middle, high-pass above it, open in the middle). Click a lane to add a
  point, drag a point to move it, and double-click or right-click a point to
  delete it. Points snap to 1/16 bar; hold Ctrl for single ticks. Between
  points the value moves in a straight line. Choose Off to edit clips again.
- Press Space to play or pause. Drag across the bar ruler to select a loop.
- Use Follow beside Zoom to keep the playhead visible. Turn it off to browse
  the timeline independently during playback.
- Drag the divider above the lower panel to give it more or less room.
- Select Sounds, Mixer, Mastering, or Effects from the fixed module menu at the
  lower panel's left. The panel retains its resized height; opening Mixer,
  Mastering, or Effects raises a shorter panel to fit its controls. The Mixer
  and Mastering tool-rail shortcuts open their module and return to Sounds when
  that module is already selected.

## Mixer

Use Mixer for track gain, pan, L/R routing, Mute, Solo, and master volume.
Master stays visible while you scroll through tracks. Each strip shows its
track number and the sound names on that track. The mouse wheel moves the
fader, knob, or pan under the pointer (Shift for fine steps). Gain and pan
change smoothly while the song plays, including when you undo or redo them.

- Gain runs from -inf dB (silence) to +12 dB; 0 dB is unity. Negative dB
  attenuates the signal. The rail gives more space to useful mixing levels.
  Click the gain value for exact dB entry, or select the small Unity button
  at the rail's 0 dB mark. Pan shows left, center, or right; its small Center
  button resets pan at the rail midpoint. The slashed-circle button reverses
  both channels' polarity without changing gain or pan. Amber means inverted;
  the polarity setting remains independent even at silence.
- L and R enable each output; both off shows No output. M mutes the track.
  S solos it: while any track is soloed, only soloed tracks play, and a muted
  track stays silent. The lane and mixer M/S/L/R controls mirror each other.
  Changing these controls does not change pan.
- EQ, beside M, opens the track's channel EQ window and lights while the EQ is
  on. The EQ has a low cut and a high cut at 12 dB per octave, a low shelf,
  two peaks with their Q, and a high shelf, and draws its response curve. Mid
  peak runs from 150 Hz to 10 kHz; Peak 2, from 40 Hz to 16 kHz, takes a
  narrow cut, such as one ringing drum line, while Mid peak does another
  job. It acts on
  the track's clips before its automation and fader. Each track's EQ starts
  off; an EQ that is off keeps its settings and leaves the track as it was.
  Reset flattens every band.
- The same window holds the track's ducker, which lowers the track while
  another track, its key, sounds: a bass that makes room for the kick, for
  example. Switch it on, pick the key track, and set Thresh (the key's level
  above which the track ducks, after the key's fader and before the master
  gain, so the master gain does not change the ducking), Depth (the most it
  ducks), Attack, and
  Release. The track ducks by as much as the key rises over the threshold,
  after its fader and before its sends. A muted key does not duck. The EQ
  button lights while the EQ, the compressor, or the ducker is on, or while
  the width is not 100%.
- Between the EQ and the ducker, the window holds the track's compressor,
  which evens out the track's level after its EQ, such as the hits of a drum
  loop. Switch it on and set Thresh (the level above which it compresses),
  Ratio, Attack, Release, and Makeup. It works before the track's width,
  automation, and fader, so moving the fader does not change how hard it
  compresses. Width, under Stereo at the end of the row, sets the track's
  stereo width before its pan: 0% folds the track to mono, 100% leaves it as
  it is, and 200% doubles the difference between its sides. Width keeps the
  track's mono sum: below 100% the track's stereo sound comes closer to that
  sum, and above 100% it moves further from it. Width works while both
  outputs are on.
- Bypass on the Master strip skips the master pedalboard, as Mastering's
  Bypass all does.
- Export tracks… in the Mixer's toolbar asks for a folder and writes each
  track that plays, and the return of each FX bus that it feeds, to its own
  WAV file, for mixing in another application. A track's file is the track
  alone after its EQ, compressor, width, automation, fader, pan, outputs, and ducker,
  without its sends;
  the FX returns hold every track's sends. The files leave the master
  pedalboard out and are as long as the mix export. They sum to the mix with
  the board bypassed when the summed mix does not clip. Export stops if an
  individual track or FX return exceeds the 16-bit range; lower the named
  track or return level, or the master gain, and export again.
- The four small knobs under the fader send the track to FX 1 to FX 4, from
  0 to 100%, after its fader and pan. A ringed knob has an automation lane,
  which sets the send while the song plays. After the tracks, a return strip for
  each bus sets its return level and switches its input and limiter; Edit
  opens the bus in the Effects module.

## Mastering

Use Mastering, the third module of the lower panel, for the master
pedalboard. The mix enters the Input unit, where Trim sets its level against
the VU meter; after the Trim the mix is held to full scale, so lower the Trim
when the VU meter shows a mix over full scale. It then passes each pedal that
is on from left to right, and leaves
through the Output unit, which shows loudness in LUFS, the true peak, and an
OVER lamp while the song plays.

The board holds the three Booster pedals (Booster EQ, Booster compressor, and
Booster wide), which reproduce the master effects of the original products, and
ten mastering pedals: Soft clip, Tube, Trim EQ, Bus compressor, Maximizer, Lift
EQ, Tape, Stereo imaging, Multiband, and Limiter. Drag a pedal by its name or
use its arrows to move it; the Limiter always stays last. Click a pedal in the
strip below the board to scroll to it. Every pedal starts off.

The dynamics pedals (Booster compressor, Bus compressor, Multiband, and
Limiter) each have a Key dial. At Own the pedal listens to the mix. Set it to
a track's number and the pedal listens to that track as it sounds after its
fader: the compressors duck the whole mix while the track sounds, the
Multiband holds each band under its ceiling while the track's same band is
loud, and the Limiter holds the louder of the mix and the track under the
ceiling. A muted key track is silent.

The Preset selector loads a factory board: Subtle, Revive, or Loud, or one of
seventeen genre boards. A preset sets every pedal and the order and turns the
Booster pedals off; it keeps your Trim, so a mix you trimmed down stays down,
and the pedals' keys.
Undo restores the board before it. Bypass all, beside the selector, skips the
whole board, Trim included, for playback and export, so you can master the
exported mix elsewhere. The pedals keep their settings, the board dims, and
presets leave the switch alone.

## Effects

Use Effects, the fourth module of the lower panel, for the four FX buses
that the Mixer's sends feed. Choose Echoform Delay or Aetherform Reverb for
a bus, load a preset, and set its controls; the returns join the mix before
the master pedalboard. Sync sets the delay in note lengths at the song's
tempo. Input off lets the tail ring out; Clear tail cuts it at once. Export
lets the tails ring past the song's end. Each effect's ducker lowers its
repeats or tail while its input plays; set its Key to a track's number and it
follows that track instead, such as the kick. A preset keeps the Key.

## Sound tools and recording

- Use Import for WAV files and enter the known source tempo.
- Use Sample Editor to trim and change pitch. Start frame is included; End frame
  is excluded. The adjacent buttons change one frame or 0.1 semitone; hold Shift
  for 100 frames or one semitone. Click a value for exact entry. Listen edit
  previews the draft; Save edited sound writes a new WAV in Wave.
- Use the other sample tools to stretch tempo or apply effects, then save a new
  WAV sample.
- Use Step synth to sequence one note per sixteenth step across up to four bars.
- Select a recording track and press Record. Recording starts after an eight-beat
  count-in. Stop and save the take as a WAV file.

## Album Review

Album Review is a library of deep-listening analyses of albums: for each
album the metadata, an overview, and for each track its energy, tags, part in
the album, and a timeline of timestamped events. Reviews of your own albums
and songs use the same form, so you can compare your work with the reference
albums. Select the record icon in the tool rail to open it.

- The grid shows one card for each album. Search finds titles, artists, and
  genre tags and ignores accents: "oxygene" finds "Oxygène". Artist filters
  show the albums of any selected artist; genre filters show the albums that
  have every selected tag, and a tag that would leave no album is disabled.
  The genre row shows the six most-used tags; "+N more" shows the others.
  Active filters show as chips; Clear filters removes them and the search.
  Sort by year, artist, title, or track count. The count reads "visible /
  total albums".
- Select a card for its page: the metadata, the overview, the listening
  links, a timeline chart with one row for each track, and each track's
  events. Select a timeline row to go to its track.
  Opening an album or returning to the grid starts at the top of that view.
- Review files contain links to cover art. **Display cover art** downloads the
  album's cover from its source and keeps it in the cover cache of the data
  folder, up to 256 MB; the least recently viewed covers go first. A cover
  that cannot be downloaded leaves the placeholder and shows the reason.
  **Clear cover cache** removes the downloaded covers after cover loading
  finishes. **Reload reviews** reads the folder again after its files change
  and finds a missing folder after you restore it. **Credits and rights** names the
  sources.
- The reviews are JSON files in `assets/album-review/` beside the application.
  Without that folder the window shows **No reviews folder**. Agents add and
  change reviews through [agent control](AGENT-CONTROL.md); the
  [writing guide](ALBUM-REVIEW-WRITING.md) gives the content rules, and the
  [research shortlist](ALBUM-REVIEW-RESEARCH.md) lists albums to review next.

## Save and export

Save songs as `.mixjam` files. Export writes stereo PCM16 WAV at 44.1 kHz.
If the used native samples of one product are on different volumes, saving
keeps copies of those WAV files together in the application's data folder.
The song retains its native sample identities. These copies are source audio
for the saved song; repeated saves reuse unchanged copies.

A `.mixjam` song is JSON, format version 16; [SONG-FORMAT.md](SONG-FORMAT.md)
describes it and how earlier versions open. A song links to WAV files; it is
not a portable audio bundle. A song finds a WAV that moved within a library
folder, or to another library folder, by the end of its path: the file name and
the folders just above it. After moving a library folder, add its new location
before you open the song.

## Open songs from other programs

**Open** accepts `.mixjam` songs, MixJam v7 `.mixjam` projects, JSON projects
from earlier builds, and eight legacy `.mix` profiles, named `d1`, `d2`, `d3`,
`d4`, `h1`, `t1`, `t2`, and `t3` after the original products that wrote them.
Import selects the nearest project tempo and shows a notice when the tempo
changes. Songs that use more than 64 tracks are rejected.

Legacy `.mix` files can be up to 500 MB (500,000,000 bytes). Supported embedded
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
paths. If the selected folder is named `_original`, the importer can also recover
those references from its containing folder (not a drive root), including moved
category folders.
Exact relative paths take priority. If several files match, the first match loads
and a **Sample conflicts** modal shows the choices. Select another path to update
all instances in the song immediately. Use **Play song** to compare the result,
then **Keep choices**. Esc hides the window without confirming. Reopen the
resolver from **Sample conflicts** in the status bar. This conflict behavior
applies to every supported project format and supports undo/redo and `.mixjam`
save/reopen. Resolved paths survive `.mixjam` save/reopen. An imported v7
`.mixjam`, `.mix`, or JSON file is unchanged: **Save** asks for a new `.mixjam`
file.

`d2` songs can also contain packed samples in the original compressed sample
format. The reader decodes them and retains them as WAV audio. Select the common
parent sample folder when a song uses sounds from earlier products. `d3` songs
can reference `d2` sounds; the supported `t2` compatibility bank uses `d2` IDs 1
through 1957.

A `.mix` song stores numeric sample IDs. The application needs locally generated
sample identity catalogs to resolve them. This repository does not contain those
catalogs. Each catalog entry has a digest of the original sample's audio. The
application identifies a WAV when its audio matches, regardless of filename.
Digest comparisons require mono audio at the original 44.1 kHz rate, matching
frame counts, and more than two frames. If a comparison is unavailable, the
reader can infer identity from an unambiguous native code or name, category,
and duration. It rejects a WAV when its available digest differs, even if its
name and length match. The import error names that WAV.

Without catalogs, the build embeds empty catalogs. WAV folders, MixJam projects,
and saved songs still work; unresolved `.mix` sources open as placeholders. To
embed catalogs, put `d1.json` through `t3.json` in `assets/catalog/` and rebuild.
The optional `source-tempos.json` adds tempo identities for two original hip-hop
products, whose shipped help states that all their samples were recorded at
90 BPM. It is generated from the original media by the owner-local
`examples/export_source_tempos.rs` (the `examples/` folder is not published),
with `MIXJAM_TECHNO_MEDIA_ROOT` set, then rebuilt into the executable. This
metadata adds tempo evidence; it does not add song-format support for those
products.

## Known limits

- Original-format writing and portable project bundles are not implemented.
- Imported songs do not reproduce every effect or level curve of their source
  programs. MixJam v7 clip edge fades are not rendered, and its master bus and return
  effects play through this application's own mastering pedals and FX buses;
  the importer shows a notice.
