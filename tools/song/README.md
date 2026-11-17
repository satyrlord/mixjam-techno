# Song tools

Python tools that build, render, and measure MixJam Techno songs. The
[song-review skill](../../.github/skills/song-review/SKILL.md) uses them.
They need Python 3.9 or later and numpy. They render with the release build
(`target/release/mixjam-techno.exe`, see "Build and run" in the root
`README.md`). Renders are headless and play no sound. Run each tool with
`python -I`.

## Settings

The tools read private paths from `.env.local` at the repository root. Copy
`.env.example` to `.env.local` and set the keys. An environment variable of the
same name overrides the file. A relative path is read from the repository
root. If a tool needs a key that is missing, it stops and names the key.

| Key | Used by |
| --- | --- |
| `SAMPLES_FOLDER_1` to `SAMPLES_FOLDER_9` | `samples.py` (the libraries it indexes) and `bright.py` (it tries relative patterns under each library) |
| `SAMPLE_INDEX` | `samples.py`. Optional. Default `local-data/samples/index.tsv`. |
| `SAMPLE_TAGS`, `SAMPLE_VERDICTS`, `SAMPLE_OWNER_VERDICTS` | `sampletags.py`, `verdicts.py`, and `palette.py`. Optional. Defaults `tags.tsv`, `verdicts.tsv`, and `owner-verdicts.tsv` beside the index. |
| `SAMPLE_CATALOGS` | `palette.py` (the product sounds that songs place by ID). Optional. Default `assets/catalog`. |
| `SAMPLE_CATALOG_NAMES` | `palette.py` (the names of those sounds, which the catalogs do not hold). Optional. Default `local-data/catalog-names`, which the catalog generator writes. |
| `REFERENCE_ALBUM_FOLDER` | `master.py --album` |
| `ARTIST_PROFILE` | The song-review skill. `reference.json` beside it is the default reference profile. |
| `REFERENCE_PROFILE` | `master.py` and `measure.py` (targets and reference columns). Optional. |
| `SONG_FOLDER` | The song-review skill (where finished songs go). |

## Tools

| Tool | Usage | What it does |
| --- | --- | --- |
| `mixjam.py` | `from mixjam import Song, board, delay, reverb, drive` | Library. It writes format-25 songs. A song can hold lanes, sections, clips, loops, phrases, fades, clip pitch and reverse, and automation (including the Repeat gate and Pitch lanes). It can also hold FX buses (delay, reverb, and drive returns) with their return lanes, channel EQ (two peaks), drives, compressors, widths, duckers, pedal and FX keys, and the master board. It also writes `.meta.json` (lane roles, sections, key). |
| `render.py` | `render.py SONG.mixjam OUT [--fit-gain DBFS] [--jobs N] [--no-master] [--cache DIR \| --no-cache]` | Renders each lane alone, up to 4 at a time by default, and the master once. Renders run below normal priority. On Windows, they stop when the tool stops. The premaster and the role stems are sums of the lane renders. The tool derives `audible.txt` from those renders. `--fit-gain` sets the master gain from the peak of the premaster. It rescales the lane files, the premaster, and the stems to that gain. Unchanged lanes come from the cache that this file describes below, when they are eligible. |
| `measure.py` | `measure.py SONG.mixjam OUT [--key "A minor"] > report.md` | The mechanical gate and the measurement report: sections, levels, tone, stereo, harmony, arrangement, phrase cuts, harmonic fit, stems, and targets. The gate includes the artist profile's working rules that a tool can check: faders at whole dB and whole phrases on tonal lanes. The report lists clip gains other than 0 dB for review. |
| `phrase.py` | `phrase.py SONG.mixjam [--all]` | Clips that cut their loop's phrase: they start or end inside the loop, and the lane does not continue the figure there. Repeated figures, starts in time on the phrase grid (on a bar or half-bar line), edges where another in-time clip of the lane or its twin plays across (an in-time crossfade), and approved devices (`cuts` in the lane's metadata) pass; a fade out inside the phrase is still a cut. Lanes with `"field": true` in their metadata (field recordings) are not checked. The gate runs it on tonal lanes, pads and atmospheres included. `--all` adds effects and drums. |
| `clash.py` | `clash.py SONG.mixjam OUT [--lanes 36,46] [--alt] [--drop 0.15]` | Harmonic fit per bar of each tonal lane against the rest of the mix, from the lane renders. Lists the weak bars, where a lane may leave the bed's chord. `--alt` scores the lane's other files at the same place in the phrase, to find the file that fits there. |
| `trim.py` | `trim.py IN.wav OUT.wav [--floor -90] [--tail 0.25]` | Cuts a render's near-silent FX tail for delivery, with a short fade. Keeps 16-, 24-, or 32-bit PCM. Use `--tail 0` for no fade. Replaces the output after a successful write, so other hard links keep their audio. |
| `audible.py` | `audible.py SONG.mixjam OUT_DIR [--jobs N]` | Marks lanes that the rest of the mix drowns. `render.py` writes the same table as `audible.txt`. Run `audible.py` alone only for a song that `render.py` did not render. |
| `master.py` | `master.py PREMASTER.wav BOARD.json OUT_DIR [--song SONG] [--bpm N] [--fit DB \| --no-render] [--quiet]` | Renders a board over a premaster and compares it with the reference profile. `--no-render` measures the `master.wav` that is already in `OUT_DIR`. `--quiet` prints only the summary line. The board uses the format-16 keys `boosterEq`, `boosterCompressor`, and `boosterWide`. A board from a format-15 song has the earlier keys. It fails to render until you rename them. |
| `master.py --album` | `master.py --album [DIR] [--track NAME ...] [--out PROFILE.json]` | Measures a reference album and writes the reference profile. |
| `stereo.py` | `stereo.py [--bpm N] WAV...` | Low-end share, width above 120 Hz, and side level of sounds. |
| `samples.py` | `samples.py index` / `samples.py find [WORD ...] [--bpm N] [--key KEY] [--kind KIND] [--loops]` / `samples.py summary` | Indexes WAV headers and WAV/MIDI path metadata (tempo, key, chord, root note, bars, kind, and where each came from). Searches the index. A later run reads the headers again when the file size, the modification time, or the pack metadata changes. It does not read MIDI notes. |
| `sampletags.py` | `sampletags.py tag [--jobs N]` / `sampletags.py find [WORD ...] --tag TAG [--not TAG]` / `sampletags.py summary` | Measures the audio of every indexed WAV (level, tone, attack, stereo, chroma, key, tuning, and its first and last 10 ms). Tags each WAV: dark, bright, tonal, off-tune, late-start, end-spike, seam-click, cut-end, and more. A later run measures only changed files. |
| `verdicts.py` | `verdicts.py build` / `verdicts.py show [WORD ...] [--verdict V] [--reason R]` / `verdicts.py summary` / `verdicts.py family WORD...` | Gives each sound a verdict from its measurements (ok, caution, or reject, with reasons). Groups sounds into families. Finds numbered chord files that do not play their chords. Finds families that are out of tune, with the cents to add. Applies the verdicts of the owner over its own verdicts. |
| `palette.py` | `palette.py census` / `families SONG` / `suggest [WORD ...] [--kind] [--key] [--tag]` / `make PALETTE --from SONG` / `add` / `remove` / `show` / `check SONG PALETTE` | Handles the palette of sound families of a song. Counts the families that songs use. Suggests families from the verdicts. Checks that a song uses only its palette. `Song.use_palette(PATH)` in `mixjam.py` holds a composer script to the palette. |
| `bright.py` | `bright.py PATTERN...` | Brightness of sounds, to find harsh hats. |
| `drumfamily.py` | `drumfamily.py [--bpm N] WAV...` | Rhythm and tone likeness of drum loops. |
| `midinotes.py` | `midinotes.py SEMITONES MIDI...` | Notes per bar of MIDI loops. |
| `midichords.py` | `midichords.py [--to KEY] FOLDER...` | Chord root per bar of MIDI loops, moved to the key of the song. |
| `fit.py` | `fit.py [--key KEY] --progression "Am F Am G" ... FOLDER...` | Which MIDI loops fit which progressions. |
| `barnotes.py` | `barnotes.py [--song-bpm N] WAV BPM CENTS ...` | Notes per bar that a listener hears at the song tempo. It includes the choice of tempo octave, varispeed, and clip pitch. |

`env.py` reads `.env.local`. `harmony.py` holds keys, scales, and chords.

## Gain fitting

If a song has no track keys, `render.py --fit-gain` checks each scaled lane and
role stem before it saves the gain or rescales the audio. If a part would
exceed full scale, the tool stops and names the part. Lower the target or
adjust the lane gains. Parts of opposite polarity can exceed full scale even
when their sum does not.

If a song has track keys, the tool renders the lanes again after each gain
change. It makes at most five changes. It reports any remaining error above
0.05 dB. The saved gain, lane audio, premaster, and role stems come from the
same final render. Without track keys, the tool scales the premaster and the
role stems.

Gain fitting stops before it changes the song if the necessary master gain is
outside the range of the song format. Adjust the lane gains and try again.
Limiter fitting keeps the board and the measurements from the same render. It
reports when five attempts do not reach the requested loudness.

## Lane groups with external keys

- Only the tracks of the group and its direct key tracks play. Only the tracks
  of the group feed the FX buses.
- The engine measures each key before its own ducker, so keys do not chain.
- Other tracks do not contribute audio. Key audio never mixes into the output of
  the group.
- The render keeps the duration of the full song, including for Repeat on a
  selected track or a key track. That duration and the global Solo state stay
  cache dependencies.
- The native stem export rejects a track or FX part outside the PCM16 range,
  including a key track. Lower the gain that the tool shows, then try again.
- The FX of each group runs separately. If an effect is nonlinear, the summed
  lane returns can differ from the shared FX of the full song.

## Song-writing methods

The methods accept whole track numbers from 1 to 64 and whole FX bus numbers
from 1 to 4. An invalid destination fails before it changes the song. Loop
ranges must advance in time. An explicit tile size must be finite, positive,
and at least one tick long. Automatic end fades use the energy of both
channels. Channels of opposite polarity do not make an audible ending count
as silence.

`song.automate(track, lane, points)` accepts the lanes `volume`, `filter`,
`pan`, `repeat`, `repeatgate`, `pitch`, and `send1` to `send4`. A rejected
batch leaves the earlier points unchanged.

- A `pan` lane is from -1 (left) to 1 (right). While it has points, it replaces
  the pan knob of the lane. Before its first point, it rests at the knob.
- A `repeat` lane takes 0 (off) or 4, 8, 16, 32, or 64 for 1/4 to 1/64 repeats.
  It holds each value until its next point. It is off before its first point.
  `song.automate(3, 'repeat', [(5, 8), (5.25, 16), (5.375, 32), (5.5, 0)])`
  rolls the slice from bar 5.
- A `repeatgate` lane is `[(bar, percent), ...]`. It shortens each Repeat
  retrigger to 5 to 100 percent of its slice. After that, the retrigger fades
  out over 3 ms. The lane steps like `repeat` and is at 100 before its first
  point. It acts only while `repeat` is on, so it can end at any value.
- A `pitch` lane is `[(bar, cents), ...]`, from -4800 to 2400 cents. It bends
  the playback speed of the lane. It is varispeed, like a tape. Pitch and speed
  move together. `song.automate(3, 'pitch', [(8.5, 0), (9, -4800), (9.0625,
  0)])` is a tape-stop over the second half of bar 8.

A `pitch` lane is 0 before its first point, so it needs no resting points. It
moves in straight lines in cents. It adds to the own pitch of each clip. It
runs before the Repeat lane. It must be back at 0 before a later clip on the
lane. A clip keeps its length. If the lane slows a clip, the clip ends at its
end. If the lane speeds it up, its sound ends where its source runs out. The
mechanical gate follows this speed clock when it checks the source duration,
the silent gaps, and the source window at the ending of a clip. This includes
reversal.

`song.automate_return(bus, [(bar, value), ...])` writes the return lane of an
FX bus. The value is from 0 to 2 (1 is the full return, and 2 is about +6 dB).
While the lane has points, it replaces the `returnLevel` of the bus. Use it to
throw a delay or reverb at the end of a phrase.

`song.clip(..., reverse=True)` plays the slice of the clip backwards within the
clip. Fades, pitch, and gain stay in the own time of the clip.

`song.drive(track, amount=12, character=25, mix=50, focus=100, output=0)`
switches on the drive of a lane. The drive runs after the EQ of the lane and
before its compressor. It adds harmonics above `focus` Hz and keeps the power
of the lane, so small speakers can play a held bass. Set `focus` to about twice
the lowest note of the lane to keep the fundamental clean. `on=False` at the
default settings removes the drive, as the app saves it.

## Earlier songs

The song tools expect format 25. Formats 18 to 24 also work. `render.py` does
not pass sample folders to the application. `measure.py` checks linked paths
from its working directory. For an earlier song or a library-relative WAV, make
a format-25 working copy:

1. Use the native `convert` command with the necessary `--samples` folders and
   an isolated `MIXJAM_TECHNO_DATA_DIR`.
2. Copy any `.meta.json` beside the new song with the matching name.
3. Check for missing sources and unconfirmed conflicts before you use its
   renders.
4. Do not replace the original song.
5. Keep the files that the converted copy needs in the data folder.

## Lane cache

The lane cache is in `lane-cache` beside the output folder by default. It keeps
the most recently used 4 GB in a versioned subfolder. Cache cleanup touches
only its named entries. It does not touch other cache versions or unrelated
files.

A lane WAV in an output folder and its cache entry are hard links to one file.
An unchanged lane takes no more disk space in each new output folder. On
another volume, the tool copies the lane. The tools replace a lane WAV. They do
not write into it. If another program writes into a lane WAV, the cache entry
no longer matches its recorded digest, and the tool renders the lane again.
Copy a lane WAV before you edit it. A missing digest also needs a new render.

A staged link or copy must pass the WAV and digest checks before it replaces an
output. The tool renders missing or damaged entries again. Use `--no-cache` to
render every lane.

Caching needs these conditions:

- The song is format 18 to 25.
- The song has directly linked, absolute, imported WAV paths with a source
  tempo.
- The song has no native, library, missing-source, conflict, or origin
  dependency.

Other songs still render, with no cache reuse. Cache keys include the content
of the source files, the render settings, the global Solo state, the master
gain, and the renderer build. A gain change needs a new render. The tool does
not rescale cached PCM.

Repeat lane renders keep the duration of the whole song, so captured audio can
continue after the last clip of the lane. Their cache keys include that
duration. When the audibility report selects the bars to measure, it follows
the Repeat captures. FX tails stay outside that selection.

Use `--no-master` for intermediate trials only. `measure.py` and the
song-review gate need a current `master.wav` from a render without that
option.

## Sample measurements and palettes

After you index changed WAVs, run `sampletags.py tag` before `verdicts.py
build`. A verdict does not use a measurement from an older file. The palette
reader checks each verdict against the current file and analysis.

Palette files made from songs keep pathless native samples by ID. The palette
reader uses format-3 release catalogs. It reports invalid or conflicting
files. Catalogs hold no sound names. The reader takes the names from valid
private name files for the accepted product and release. It rejects an invalid
name file in full. If no name matches, the reader identifies a sound by its
archive code.

When `Song.sound` links an extracted WAV, it can use the product name for a
matching archive code and duration. This includes codes from compatible
releases. Comparable mono 44.1 kHz PCM digests must match. If no comparable
digests exist, the tool infers the name from the code and the duration. If a
match is ambiguous, the file name stays. This changes the display name only. It
does not replace the linked WAV.

## Mechanical report

For `master.py`, `--bpm` overrides the tempo. Without that option, a supplied
song gives its tempo. With no song, the tool uses the metadata tempo, or 140
BPM when none is available. A missing section list leaves out the section
table.

The mechanical report checks the source duration of WAV files linked in
`samples`. It checks abrupt endings on lanes with a melodic role in the
`.meta.json`: `pad`, `lead`, `keys`, `arp`, `fx`, `vocal`, or `atmos`.
The separate phrase check gates `lead`, `keys`, `arp`, `bass`, `vocal`, `pad`,
and `atmos`. Both checks follow Pitch automation and reversed slices.
An approved `cuts` span exempts only clips fully inside that span.
Without metadata, the report assigns generic `trackN` roles and cannot gate
these role-specific checks. `render.py` also needs metadata to make role stems.

Catalog-only sources need the library resolver of the application. The report
marks those source checks as unavailable and does not pass the gate. Missing
and catalog sources still appear in the arrangement report.

Harmonic fit needs the lane renders and the premaster at the same sample rate.
Dry-source alternatives need linked WAV files. The comparison excludes
reversed clips, lanes with Pitch automation, and simultaneous clips in a
rendered lane. It cannot model their source positions separately.
It does not model EQ or drive. The report marks unavailable comparisons.
A candidate needs a new render before acceptance. Tracks with the same lane
name, including `(twin)` tracks, share the renderer's lane file and candidates.

## Reference profile

`master.py --album` writes a JSON file with these keys:

- `targets`: a range for each measure. `master.py` flags a value outside the
  range with `!`.
- `references`: the album median and named tracks, for each measure and for
  each 1/3-octave band.
- `song_targets`: lines that `measure.py` prints.

A new profile gets the 10th to 90th percentile ranges of the album as targets.
A later run keeps the targets and replaces the references. Without a profile,
the tools measure and print no targets.

## Tests

`python -m unittest discover -s tools/tests -v` runs the tests of the tools in
this folder, the runtime song helper, the Rust format check, the font checkout,
and the album lookup.
