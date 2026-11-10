# Song tools

Python tools that build, render, and measure MixJam Techno songs. The
[song-review skill](../../.github/skills/song-review/SKILL.md) uses them.
They need Python 3.9 or later and numpy, and render with the release build
(`target/release/mixjam-techno.exe`, see "Build and run" in the root `README.md`).
Renders are headless and play no sound. Run each tool with `python -I`.

## Settings

The tools read private paths from `.env.local` at the repository root. Copy
`.env.example` to `.env.local` and set the keys. An environment variable of the
same name overrides the file; a relative path is read from the repository root.
A tool that needs a missing key stops and names the key.

| Key | Used by |
|---|---|
| `SAMPLES_FOLDER_1` to `SAMPLES_FOLDER_9` | `samples.py` (the libraries it indexes) and `bright.py` (relative patterns are tried under each library) |
| `SAMPLE_INDEX` | `samples.py`; optional, default `local-data/samples/index.tsv` |
| `SAMPLE_TAGS`, `SAMPLE_VERDICTS`, `SAMPLE_OWNER_VERDICTS` | `sampletags.py`, `verdicts.py` and `palette.py`; optional, default `tags.tsv`, `verdicts.tsv` and `owner-verdicts.tsv` beside the index |
| `SAMPLE_CATALOGS` | `palette.py` (the product sounds that songs place by id); optional, default `assets/catalog` |
| `SAMPLE_CATALOG_NAMES` | `palette.py` (the names of those sounds, which the catalogs do not hold); optional, default `local-data/catalog-names`, which the catalog generator writes |
| `REFERENCE_ALBUM_FOLDER` | `master.py --album` |
| `ARTIST_PROFILE` | the song-review skill; `reference.json` beside it is the default reference profile |
| `REFERENCE_PROFILE` | `master.py` and `measure.py` (targets and reference columns); optional |
| `SONG_FOLDER` | the song-review skill (where finished songs go) |

## Tools

| Tool | Usage | What it does |
|---|---|---|
| `mixjam.py` | `from mixjam import Song, board, delay, reverb` | Library: writes format-24 songs with lanes, sections, clips, loops, phrases, fades, clip pitch and reverse, automation (the Repeat gate and Pitch lane too), FX buses and their return lanes, channel EQ (two peaks), drives, compressors, widths, duckers, pedal and FX keys, and the master board, plus `.meta.json` (lane roles, sections, key). |
| `render.py` | `render.py SONG.mixjam OUT [--fit-gain DBFS] [--jobs N] [--no-master] [--cache DIR \| --no-cache]` | Renders each lane alone, up to 4 at a time by default, and the master once. Renders run below normal priority and, on Windows, stop when the tool stops. The premaster and role stems are sums of the lane renders. The tool derives `audible.txt` from those renders. `--fit-gain` sets the master gain from the premaster's peak and rescales the lane files, premaster and stems to it. Eligible unchanged lanes come from the cache described below. |
| `measure.py` | `measure.py SONG.mixjam OUT [--key "A minor"] > report.md` | The mechanical gate and the measurement report: sections, levels, tone, stereo, harmony, arrangement, stems, targets. |
| `audible.py` | `audible.py SONG.mixjam OUT_DIR [--jobs N]` | Marks lanes that the rest of the mix drowns. `render.py` writes the same table as `audible.txt`; run this alone only for a song that `render.py` did not render. |
| `master.py` | `master.py PREMASTER.wav BOARD.json OUT_DIR [--song SONG] [--fit DB]` | Renders a board over a premaster and compares it with the reference profile. The board uses the format-16 keys `boosterEq`, `boosterCompressor`, and `boosterWide`; a board copied from a format-15 song has the earlier keys and fails to render until you rename them. |
| `master.py --album` | `master.py --album [DIR] [--track NAME ...] [--out PROFILE.json]` | Measures a reference album and writes the reference profile. |
| `stereo.py` | `stereo.py [--bpm N] WAV...` | Low-end share, width above 120 Hz, and side level of sounds. |
| `samples.py` | `samples.py index` / `samples.py find [WORD ...] [--bpm N] [--key KEY] [--kind KIND] [--loops]` / `samples.py summary` | Indexes WAV headers and WAV/MIDI path metadata (tempo, key, chord, root note, bars, kind, and where each came from), and searches the index. Later runs read headers again when file size, modification time, or pack metadata changes. MIDI notes are not read. |
| `sampletags.py` | `sampletags.py tag [--jobs N]` / `sampletags.py find [WORD ...] --tag TAG [--not TAG]` / `sampletags.py summary` | Measures every indexed WAV's audio (level, tone, attack, stereo, chroma, key, tuning, and its first and last 10 ms) and tags it: dark, bright, tonal, off-tune, late-start, end-spike, seam-click, cut-end and more. Later runs measure only changed files. |
| `verdicts.py` | `verdicts.py build` / `verdicts.py show [WORD ...] [--verdict V] [--reason R]` / `verdicts.py summary` / `verdicts.py family WORD...` | Gives every sound a verdict from its measurements (ok, caution, reject, with reasons), groups sounds into families, finds numbered chord files that do not play their chords and families out of tune (with the cents to add), and applies the owner's verdicts over them. |
| `palette.py` | `palette.py census` / `families SONG` / `suggest [WORD ...] [--kind] [--key] [--tag]` / `make PALETTE --from SONG` / `add` / `remove` / `show` / `check SONG PALETTE` | A song's palette of sound families: counts the families songs use, suggests families from the verdicts, and checks that a song uses only its palette. `Song.use_palette(PATH)` in `mixjam.py` holds a composer script to it. |
| `bright.py` | `bright.py PATTERN...` | Brightness of sounds, to find harsh hats. |
| `drumfamily.py` | `drumfamily.py [--bpm N] WAV...` | Rhythm and tone likeness of drum loops. |
| `midinotes.py` | `midinotes.py SEMITONES MIDI...` | Notes per bar of MIDI loops. |
| `midichords.py` | `midichords.py [--to KEY] FOLDER...` | Chord root per bar of MIDI loops, moved to the song's key. |
| `fit.py` | `fit.py [--key KEY] --progression "Am F Am G" ... FOLDER...` | Which MIDI loops fit which progressions. |
| `barnotes.py` | `barnotes.py [--song-bpm N] WAV BPM CENTS ...` | Notes per bar heard at the song tempo, including tempo octave selection, varispeed, and clip pitch. |

`env.py` reads `.env.local`; `harmony.py` holds keys, scales, and chords.

For songs without track keys, `render.py --fit-gain` checks each scaled lane
and role stem before saving the gain or rescaling the audio. If a part would
exceed full scale, the tool stops and names it. Lower the target or adjust the
lane gains. Opposite-polarity parts can exceed full scale even when their sum
does not. Songs with track keys render again after each gain change.

Song-writing methods accept whole track numbers from 1 to 64 and whole FX bus
numbers from 1 to 4. Invalid destinations fail before changing the song.
Automation accepts `volume`, `filter`, `pan`, `repeat`, `repeatgate`, `pitch`,
and `send1` through `send4`; a rejected batch leaves earlier points unchanged. A `pan` lane, from -1
(left) to 1 (right), replaces the lane's pan knob while it has points and rests
at the knob before its first point. A `repeat` lane takes 0 (off) or 4, 8, 16,
32, or 64 for 1/4 to 1/64 repeats, holds each value until its next point, and
is off before its first; `song.automate(3, 'repeat', [(5, 8), (5.25, 16),
(5.375, 32), (5.5, 0)])` rolls the slice from bar 5. Loop ranges must advance in time, and
an explicit tile size must be finite, positive, and at least one tick long.
Automatic end fades use the energy of both channels. Opposite channel
polarities do not make an audible ending count as silence.

`song.drive(track, amount=12, character=25, mix=50, focus=100, output=0)`
switches on a lane's drive, after its EQ and before its compressor. It adds
harmonics above `focus` Hz and keeps the lane's power, so a held bass can be
heard on small speakers. Set `focus` to about twice the lane's lowest note to
keep the fundamental clean. `on=False` at the default settings removes the
drive, as the app saves it.

`song.automate(track, 'repeatgate', [(bar, percent), ...])` shortens each
Repeat retrigger to 5 to 100 percent of its slice, after which it fades out
over 3 ms. It steps like `repeat`, is at 100 before its first point, and acts
only while `repeat` is on, so it may end at any value.
`song.automate_return(bus, [(bar, value), ...])` writes an FX bus's return lane,
0 to 2 (1 is the full return, 2 about +6 dB), which replaces the bus's
`returnLevel` while it has points; use it to throw a delay or reverb at the end
of a phrase. `song.clip(..., reverse=True)` plays the clip's slice backwards
within the clip; fades, pitch and gain stay in the clip's own time.

`song.automate(track, 'pitch', [(bar, cents), ...])` bends the lane's playback
speed from -4800 to 2400 cents. It is varispeed, like a tape: pitch and speed
move together, so `song.automate(3, 'pitch', [(8.5, 0), (9, -4800),
(9.0625, 0)])` is a tape-stop over the second half of bar 8. The lane is 0
before its first point, so it gets no resting points, moves in straight lines
in cents, adds to each clip's own pitch, and runs before the Repeat lane. It
must be back at 0 before a later clip on the lane. A clip keeps its length:
slowed, it ends at its end; sped up, its sound ends where its source runs out.
The mechanical gate follows this speed clock when it checks source duration,
silent gaps, and the source window at a clip's ending, including reversal.

The song tools expect format 24 (18 to 23 also work). `render.py` does not pass sample folders to
the application, and `measure.py` checks linked paths from its working
directory. For an earlier song or a library-relative WAV, use the native
`convert` command to make a format-24 working copy with the required
`--samples` folders and an isolated `MIXJAM_TECHNO_DATA_DIR`. Copy any
`.meta.json` beside the new song with the matching name. Check for missing
sources and unconfirmed conflicts before using its renders. Do not replace the
original. Retain any files that the converted copy needs in the data folder.

The lane cache defaults to `lane-cache` beside the output folder. It retains
the most recently used 4 GB in a versioned subfolder. Cache cleanup touches
only its named entries; earlier cache versions and unrelated files remain.
A lane WAV in an output folder and its cache entry are hard links to one file,
so an unchanged lane takes no more disk space in each new output folder. On
another volume, the lane is copied. The tools replace a lane WAV and do not
write into it. If another program writes into a lane WAV, the cache entry no
longer matches its recorded digest and is rendered again; copy a lane WAV
before you edit it. A missing digest also requires a new render, including for
entries from earlier tools. The staged link or copy must pass WAV and digest
checks before it replaces an output. Missing or damaged entries are rendered
again. Use `--no-cache` to render every lane.

Repeat lane renders retain the whole song's duration, so captured audio can
continue after the lane's last clip. Their cache keys include that duration.
The audibility report follows Repeat captures when it selects the bars to
measure; FX tails remain outside that selection.

Caching requires format 18 to 24, directly linked absolute imported WAV paths with
source tempo, and no native, library, missing-source, conflict, or origin
dependencies. Other songs still render, with no cache reuse. Cache keys include
source-file content, render settings, global Solo state, master gain, and the
renderer build. A gain change requires a new render; cached PCM is not rescaled.
Use `--no-master` for intermediate trials only. `measure.py` and the song-review
gate require a current `master.wav` from a render without that option.

After indexing changed WAVs, run `sampletags.py tag` before `verdicts.py build`.
A verdict does not use a measurement from an older file. After this update, run
`sampletags.py tag` once to refresh the cached measurements with channel-aware
levels, then run `verdicts.py build`. The refresh remeasures older rows. For
rows with current measurements, old edge measures are updated without repeating
the other measurements, and a failed edge read is retried on the next run.
The palette reader checks each verdict against the current file and analysis.
Palette files made from songs retain pathless native
samples by ID. The palette reader uses format-3 release catalogs and reports
invalid or conflicting files. Catalogs hold no sound names: the reader takes
them from valid private name files for the accepted product and release.
Invalid name files are rejected in full. Without a matching name, the reader
identifies a sound by its archive code.

When `Song.sound` links an extracted WAV, it can use the product name for a
matching archive code and duration, including codes from compatible releases.
Comparable mono 44.1 kHz PCM digests must match. Without comparable digests,
the name is inferred from code and duration. Ambiguous matches keep the file
name. This changes the display name only; it does not replace the linked WAV.

Gain fitting stops before changing the song if the required master gain is
outside the song format's range. Adjust the lane gains before trying again.
With track keys, gain fitting renders the lanes again after each gain change.
It permits five changes and reports any remaining error above 0.05 dB. The
saved gain, lane audio, premaster, and role stems use the same final render.
Without track keys, gain fitting scales the premaster and role stems.
Lane groups with external keys use native track and FX stems. Only the group's
tracks and its direct key tracks play, and only the group's tracks feed the FX
buses. The engine measures each key before its own ducker, so keys do not chain.
Other tracks do not contribute audio. The render keeps the full song's duration,
including for Repeat on a selected track or a key track. That duration and global
Solo state remain cache dependencies. Key audio never mixes into that group's
output. Native stem export rejects any track
or FX part outside the PCM16 range, including a key track. Lower the indicated
gain before retrying. Each group's FX runs separately, so summed lane returns
can differ from the full song's shared FX when an effect is nonlinear.
Limiter fitting keeps the board and measurements from the same render and
reports when five attempts do not reach the requested loudness.

For `master.py`, `--bpm` overrides the tempo. Without that option, a supplied
song gives its tempo. With no song, the tool uses the metadata tempo, or 140 BPM
when none is available. A missing section list leaves out the section table.

The mechanical report checks source duration for WAVs linked in `samples`.
It checks phrase endings only on lanes with a melodic role in the song's
`.meta.json`: `pad`, `lead`, `keys`, `arp`, `fx`, `vocal`, or `atmos`. Without
that file, it assigns generic `trackN` roles and does not check phrase endings.
`render.py` also needs the metadata to make role stems. Catalog-only sources
require the application's library resolver. The report marks those source
checks as unavailable and does not pass the gate. Missing and catalog sources
still appear in the arrangement report.

## Reference profile

`master.py --album` writes a JSON file with `targets` (a range per measure;
`master.py` flags a value outside it with `!`), `references` (the album median
and named tracks, per measure and per 1/3-octave band), and `song_targets`
(lines that `measure.py` prints). A new profile gets the album's 10th to 90th
percentile ranges as targets; a later run keeps the targets and replaces the
references. Without a profile, the tools measure and print no targets.

## Tests

`python -m unittest discover -s tools/tests -v` runs the tests of `env.py`,
`harmony.py`, and the pure functions of `mixjam.py`, `fit.py`, `master.py`,
`render.py`, and `audible.py`.
