# Song-review reference

## Contents

- [Evidence package](#evidence-package)
- [Tools](#tools)
- [Finding format](#finding-format)
- [Round log](#round-log)
- [Generic targets](#generic-targets)

## Evidence package

Give each role the same evidence for a pass, as paths:

| Item | Source |
|---|---|
| The song | `SONG.mixjam` |
| Lane names and roles, sections, tempo, key | `SONG.meta.json`, when present, written by `tools/song/mixjam.py` |
| Renders | `OUT/master.wav`, `OUT/lane-NN.wav`, `OUT/premaster.wav`, and `OUT/stem-ROLE.wav` when role metadata exists, from `render.py` |
| Measurement report | `report.md` from `measure.py` |
| Audibility table | `OUT/audible.txt` from `render.py` |
| Composer script, if one built the song | the script; leave it out of the rater's package if it quotes critiques or scores |
| Artist profile | the file at `ARTIST_PROFILE`, when available; give the rater a screened extract |
| Reference studies | the files that the artist profile names, when available; give the rater screened extracts |
| Reference profile | `REFERENCE_PROFILE` or `reference.json` beside the artist profile, when available; screen the rater's copy; required for mastering measurements against a reference |
| Review folder and output path | where the role writes its output |

Only the critics and the main agent read the round log. The rater never reads
the round log, a critique, or an earlier rating. The mastering engineer does
not read the round log. Give it a decisions-only extract when earlier board
decisions are needed.

For the rater, screen every text item in the evidence package. Make extracts
that retain the artist's style rules and objective measurements. Remove
critiques, earlier ratings, and rating targets. Check each item before you send
it. If an item cannot be screened, leave it out and report the limit. Do not
use earlier song ratings as rater calibration.

A song without a `.meta.json` file still works: the tools treat each used track
as its own lane, and `measure.py` measures 32-bar sections. `render.py` does not
write role stems without role metadata. The mechanical report checks abrupt
melodic endings only for lanes with a recognized melodic role. Without role
metadata, a report of zero problems does not cover that check. State the limit.

The song tools expect a format-19 document (18 also works). `render.py` does not pass sample
folders to the application, and `measure.py` checks linked paths from its
working directory. Convert earlier formats or resolve library-relative WAVs
with the native application as described in the setup step. Do not substitute
another sound when a source cannot be resolved.

## Tools

The tools are in `tools/song/`. `tools/song/README.md` lists each tool, its
usage line, and the `.env.local` keys it needs. Run each tool with `python -I`.
They render with the release build, headless; they play no sound.

| Task | Tool |
|---|---|
| Build or change a song from a script | `mixjam.py` (library) |
| Render the lanes and the master; sum the premaster, stems, and audibility table; fit the master gain | `render.py` |
| Gate and measure | `measure.py` |
| Find drowned lanes in a song that `render.py` did not render | `audible.py` |
| Render a board over a premaster and compare it with the reference | `master.py` |
| Stereo width and mono safety of a sound | `stereo.py` |
| Brightness of a sound (harsh hats) | `bright.py` |
| Rhythm and tone likeness of drum loops | `drumfamily.py` |
| Notes and chords of MIDI loops; fit against a progression | `midinotes.py`, `midichords.py`, `fit.py` |
| Notes per bar of a WAV after varispeed and pitch | `barnotes.py` |

A role can write its own numpy analysis in the review folder. It must not change
the song, the renders, or another role's output.

## Finding format

Both critics use this format. Number the findings in order of importance, most
important first.

```markdown
### N. Short title (severity: high | medium | low)

**Where**: bars A-B, tracks T1, T2 (lane names).
**What is wrong**: one or two sentences.
**Evidence**: the numbers, with the file or measurement that gives them.
**Fix**: the change in the tool's terms: lane fader dB, pan, sends 0-1, clip
gain dB, fades in beats, clip pitch in cents, automation points
`[(bar, value)]`, channel EQ settings, sound choice with path, bars to add or
remove.
```

Then a list "Keep" with at most 5 things that work and must not be undone, each
with its bars and tracks. Give no score.

Lane names come from the song's `.meta.json`, which the tools and reviewers read. The
owner sees track numbers and the sound names on the clips. Whenever a critique
or rating names a lane for the owner, give its track number and the names of
its sounds as well, for example "track 22 (Starshine Seq A 1, A 7)". The same
applies to every judgement that needs ears.

## Round log

The main agent keeps one round log per song, `rounds.md` in the review folder.
For each round, write:

```markdown
## Round N (pass N -> pass N+1)

Critiques: `roundN-composition.md` (C1-C8), `roundN-mix.md` (M1-M8).

### Tool changes
- what changed in the tools, and why.

### Accepted
| Finding | Fix as built in pass N+1 |
|---|---|

### Rejected
| Finding | Reason |
|---|---|

### Rating
Score X.X (parts ...). Three reasons. Judgements that need ears.

### Owner notes
- the owner's verdict and decisions, dated.
```

Name the role outputs `roundN-composition.md`, `roundN-mix.md`,
`roundN-rating.md`, and `roundN-master.md`, and the boards `board-roundN.json`.

## Generic targets

Use these when there is no artist profile, except where a configured reference
profile supplies the applicable target. Name the source of each target in
every brief and report. State when there is no artist profile.

- Gate: 0 mechanical problems; the premaster peaks under -0.3 dBFS. A drowned
  lane in `audible.txt` is a critic finding, not a gate failure.
- Master: peak -1 dBFS; loud sections near -12 to -9 dB RMS (mono-sum); crest
  in loud blocks 8-12 dB; breakdowns at least 6 dB under the loud sections.
- Low end: mono below 120 Hz; the kick and the bass do not mask each other.
- Width: L/R correlation 0.75-0.95 in loud sections.
- Arrangement: every section has a job; transitions are prepared; no sound
  stops dead; no lane is inaudible.
- Target score 8; at most 5 rounds.
