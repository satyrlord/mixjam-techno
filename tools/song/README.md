# Song tools

Python tools that build, render, and measure MixJam Techno songs. The
[song-review skill](../../.github/skills/song-review/SKILL.md) uses them.
They need Python 3.9 or later and numpy. They render with the release build
(`target/release/mixjam-techno.exe`, see "Build and run" in the root
`README.md`). Renders are headless and play no sound. Run each tool with
`python -I`.

## Contents

These parts have their own pages:

- [Song-writing methods](WRITING-METHODS.md#song-writing-methods)
- [Earlier songs](WRITING-METHODS.md#earlier-songs)
- [Lane cache](CACHE-AND-REPORTS.md#lane-cache)
- [Sample measurements and
  palettes](CACHE-AND-REPORTS.md#sample-measurements-and-palettes)
- [Mechanical report](CACHE-AND-REPORTS.md#mechanical-report)

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
| `mixjam.py` | `from mixjam import Song, board, delay, reverb, drive` | Library. It writes format-26 songs. A song can hold lanes, sections, clips, loops, phrases, fades, clip pitch and reverse, and automation (including the Repeat gate, Pitch and Compression lanes). It can also hold FX buses (delay, reverb, and drive returns) with their return lanes, channel EQ (two peaks), drives, compressors, widths, duckers, pedal and FX keys, and the master board. It also writes `.meta.json` (lane roles, sections, key). |
| `render.py` | `render.py SONG.mixjam OUT [--fit-gain DBFS] [--jobs N] [--no-master] [--cache DIR \| --no-cache]` | Renders each lane alone, up to 4 at a time by default, and the master once. Renders run below normal priority. On Windows, they stop when the tool stops. The premaster and the role stems are sums of the lane renders. The tool derives `audible.txt` from those renders. `--fit-gain` sets the master gain from the peak of the premaster. It rescales the lane files, the premaster, and the stems to that gain. Unchanged lanes come from the [lane cache](CACHE-AND-REPORTS.md#lane-cache), when they are eligible. |
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

`env.py` reads `.env.local`, also when the file starts with a byte order mark.
`harmony.py` holds keys, scales, and chords. `lanes.py` reads the lane roles of
a song and sums lane renders for `render.py`, `audible.py`, and `clash.py`.
`wavio.py` reads 24-bit PCM. `cli.py` reads the flags of the tools that parse
their own. A flag in the last position, without its value, stops the tool with
"FLAG needs a value.".

`render.py` and `audible.py` stop with "The song has no clips." for a song
without clips. `midinotes.py` refuses a MIDI file with SMPTE time. A note
without a note-off lasts to the end of its track.

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
