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
| `REFERENCE_ALBUM_FOLDER` | `master.py --album` |
| `ARTIST_PROFILE` | the song-review skill; `reference.json` beside it is the default reference profile |
| `REFERENCE_PROFILE` | `master.py` and `measure.py` (targets and reference columns); optional |
| `SONG_FOLDER` | the song-review skill (where finished songs go) |

## Tools

| Tool | Usage | What it does |
|---|---|---|
| `mixjam.py` | `from mixjam import Song, board, delay, reverb` | Library: writes format-16 songs with lanes, clips, loops, phrases, fades, clip pitch, automation, FX buses, channel EQ, and the master board, plus `.meta.json` (lane roles, sections, key). |
| `render.py` | `render.py SONG.mixjam OUT [--no-stems] [--fit-gain DBFS]` | Renders the master, the premaster (board off), and one stem per role; `--fit-gain` sets the master gain from a probe. |
| `measure.py` | `measure.py SONG.mixjam OUT [--key "A minor"] > report.md` | The mechanical gate and the measurement report: sections, levels, tone, stereo, harmony, arrangement, stems, targets. |
| `audible.py` | `audible.py SONG.mixjam OUT_DIR` | Renders each lane alone and marks lanes that the rest of the mix drowns. |
| `master.py` | `master.py PREMASTER.wav BOARD.json OUT_DIR [--song SONG] [--fit DB]` | Renders a board over a premaster and compares it with the reference profile. The board uses the format-16 keys `boosterEq`, `boosterCompressor`, and `boosterWide`; a board copied from a format-15 song has the earlier keys and fails to render until you rename them. |
| `master.py --album` | `master.py --album [DIR] [--track NAME ...] [--out PROFILE.json]` | Measures a reference album and writes the reference profile. |
| `stereo.py` | `stereo.py [--bpm N] WAV...` | Low-end share, width above 120 Hz, and side level of sounds. |
| `samples.py` | `samples.py index` / `samples.py find [WORD ...] [--bpm N] [--key KEY] [--kind KIND] [--loops]` / `samples.py summary` | Indexes WAV headers and WAV/MIDI path metadata (tempo, key, chord, root note, bars, kind, and where each came from), and searches the index. Later runs read headers again when file size, modification time, or pack metadata changes. MIDI notes are not read. |
| `bright.py` | `bright.py PATTERN...` | Brightness of sounds, to find harsh hats. |
| `drumfamily.py` | `drumfamily.py [--bpm N] WAV...` | Rhythm and tone likeness of drum loops. |
| `midinotes.py` | `midinotes.py SEMITONES MIDI...` | Notes per bar of MIDI loops. |
| `midichords.py` | `midichords.py [--to KEY] FOLDER...` | Chord root per bar of MIDI loops, moved to the song's key. |
| `fit.py` | `fit.py [--key KEY] --progression "Am F Am G" ... FOLDER...` | Which MIDI loops fit which progressions. |
| `barnotes.py` | `barnotes.py [--song-bpm N] WAV BPM CENTS ...` | Notes per bar of a WAV after varispeed and clip pitch. |

`env.py` reads `.env.local`; `harmony.py` holds keys, scales, and chords.

Gain fitting stops before changing the song if the required master gain is
outside the song format's range. Adjust the lane gains before trying again.
Limiter fitting keeps the board and measurements from the same render and
reports when five attempts do not reach the requested loudness.

The mechanical report checks source duration and phrase endings for WAVs
linked in `samples`. Catalog-only sources require the application's library
resolver; the report marks those checks as unavailable and does not pass the
gate. Missing and catalog sources still appear in the arrangement report.

## Reference profile

`master.py --album` writes a JSON file with `targets` (a range per measure;
`master.py` flags a value outside it with `!`), `references` (the album median
and named tracks, per measure and per 1/3-octave band), and `song_targets`
(lines that `measure.py` prints). A new profile gets the album's 10th to 90th
percentile ranges as targets; a later run keeps the targets and replaces the
references. Without a profile, the tools measure and print no targets.

## Tests

`python -m unittest discover -s tools/tests -v` runs the tests of `env.py`,
`harmony.py`, and the pure functions of `mixjam.py`, `fit.py`, `master.py`, and
`audible.py`.
