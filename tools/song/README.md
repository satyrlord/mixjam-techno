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
| `mixjam.py` | `from mixjam import Song, board, delay, reverb` | Library: writes format-18 songs with lanes, clips, loops, phrases, fades, clip pitch, automation, FX buses, channel EQ (two peaks), compressors, widths, duckers, pedal and FX keys, and the master board, plus `.meta.json` (lane roles, sections, key). |
| `render.py` | `render.py SONG.mixjam OUT [--fit-gain DBFS] [--jobs N] [--no-master]` | Renders each lane alone, several at a time, and the master once. The premaster and role stems are sums of the lane renders. The tool derives `audible.txt` from those renders. `--fit-gain` sets the master gain from the premaster's peak. |
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

Song-writing methods accept whole track numbers from 1 to 64 and whole FX bus
numbers from 1 to 4. Invalid destinations fail before changing the song.
Automation accepts `volume`, `filter`, and `send1` through `send4`; a rejected
batch leaves earlier points unchanged. Loop ranges must advance in time, and
an explicit tile size must be finite, positive, and at least one tick long.
Automatic end fades use the energy of both channels. Opposite channel
polarities do not make an audible ending count as silence.

The song tools expect format 18. `render.py` does not pass sample folders to
the application, and `measure.py` checks linked paths from its working
directory. For an earlier song or a library-relative WAV, use the native
`convert` command to make a format-18 working copy with the required
`--samples` folders and an isolated `MIXJAM_TECHNO_DATA_DIR`. Copy any
`.meta.json` beside the new song with the matching name. Check for missing
sources and unconfirmed conflicts before using its renders. Do not replace the
original. Retain any files that the converted copy needs in the data folder.

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

Gain fitting stops before changing the song if the required master gain is
outside the song format's range. Adjust the lane gains before trying again.
With track keys, gain fitting renders the lanes again after each gain change.
It permits five changes and reports any remaining error above 0.05 dB. The
saved gain, lane audio, premaster, and role stems use the same final render.
Without track keys, gain fitting scales the premaster and role stems.
Lane groups with external keys use native track and FX stems, with every key
track retained and only the group's tracks feeding the FX buses. Key audio
never mixes into that group's output. Native stem export rejects any track
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
