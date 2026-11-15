# Song-review reference

## Contents

- [Evidence package](#evidence-package)
- [Role inputs](#role-inputs)
- [Tools](#tools)
- [Measurement limits](#measurement-limits)
- [Finding format](#finding-format)
- [Round log](#round-log)
- [Generic targets](#generic-targets)

## Evidence package

The evidence package of a pass is a set of paths:

| Item | Source |
|---|---|
| The song | `SONG.mixjam` |
| Lane names and roles, sections, tempo, key | `SONG.meta.json`, when present, from `tools/song/mixjam.py` |
| Renders | `OUT/master.wav`, `OUT/lane-NN.wav`, `OUT/premaster.wav`, and one `OUT/stem-ROLE.wav` for each role, from `render.py` |
| Measurement report | `report.md` from `measure.py` |
| Audibility table | `OUT/audible.txt` from `render.py` |
| Composer script | the script, if one built the song |
| Artist profile | the file at `ARTIST_PROFILE`, when available |
| Reference studies | the files that the artist profile names, when available |
| Reference profile | `REFERENCE_PROFILE`, or `reference.json` beside the artist profile, when available. Mastering measurements against a reference need it. |
| Review folder and output path | where the role writes its output |

## Role inputs

Each role gets the evidence package and the items in its row. A role must not
read an item in its "Never" column.

| Role | Also gets | Never |
|---|---|---|
| Composition critic | from round 2, the round log | the critiques of the current round; a rating |
| Mix critic | from round 2, the round log | the critiques of the current round; a rating |
| Rhythm critic | the composer script, with the drum map and the lists of glitches and ratchets; the owner's earlier notes on drums and glitches, or the file that holds them; from round 2, the round log | the critiques of the current round; a rating |
| Devil's advocate | the critiques of the current round; the round log or the handoff; the owner verdicts file (`SAMPLE_OWNER_VERDICTS` in `tools/song/README.md`) | a rating or a score |
| Blind rater | nothing more; each text item of its package is a screened extract (see below) | the round log; a critique; an earlier rating; a rating target |
| Mastering engineer | the premaster; the current board, as a JSON file or as the `master` object of the song; the loudness target, if the caller sets one; a decisions-only extract, if earlier board decisions are necessary | a critique; a rating; the round log |

A critic that reads the round log does not undo an accepted fix. It does not
raise a rejected finding again without new evidence.

Screen each text item before you give it to the rater:

1. Make extracts that keep the style rules of the artist and the objective
   measurements.
2. Remove critiques, earlier ratings, and rating targets.
3. Leave out a composer script that quotes a critique or a score.
4. If you cannot screen an item, leave it out and report the limit.

Do not use earlier song ratings to calibrate the rater.

## Tools

The tools are in `tools/song/`. `tools/song/README.md` gives the usage line of
each tool and the `.env.local` keys that it needs. Run each tool with
`python -I`. The tools render headless with the release build and play no sound.

| Task | Tool |
|---|---|
| Build or change a song from a script | `mixjam.py` (library) |
| Render the lanes and the master; sum the premaster, the stems, and the audibility table; fit the master gain | `render.py` |
| Gate and measure | `measure.py` |
| Find drowned lanes in a song that `render.py` did not render | `audible.py` |
| Render a board over a premaster and compare it with the reference | `master.py` |
| Stereo width and mono safety of a sound | `stereo.py` |
| Brightness of a sound (harsh hats) | `bright.py` |
| Rhythm and tone likeness of drum loops | `drumfamily.py` |
| Notes and chords of MIDI loops; fit against a progression | `midinotes.py`, `midichords.py`, `fit.py` |
| Notes per bar of a WAV after varispeed and pitch | `barnotes.py` |

## Measurement limits

State each limit that applies in the brief or in the report.

- Without a `.meta.json` file, the tools use each used track as its own lane,
  and `measure.py` measures 32-bar sections. `render.py` writes no role stems.
  The mechanical report does not check abrupt melodic endings, so a report of
  zero problems does not cover that check. The `master.py` report has no
  section table.
- `master.py` puts the premaster on one track. Its key signals do not
  reproduce the external master keys of the song. Measure a keyed board from
  the application render of the song.
- The sum of the lane renders can differ from the full mix when a shared
  effect is nonlinear.
- The meter of `audible.txt` reads narrow high sounds and parts that a delay
  carries as quieter than they sound. The owner's ear decides what is audible.
- `measure.py` cannot measure a catalog-only source. It marks that check as
  unavailable.

## Finding format

The composition, mix, and rhythm critics use this format. Number the findings
in order of importance, most important first.

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

After the findings, write a list "Keep" with at most 5 things that work and
must not be undone, each with its bars and tracks. Give no score.

## Round log

The main agent keeps one round log for each song: `rounds.md` in the review
folder. For each round, write:

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
`roundN-rhythm.md`, `roundN-devils-advocate.md`, `roundN-rating.md`, and
`roundN-master.md`. Name the boards `board-roundN.json`.

## Generic targets

Use these targets when there is no artist profile. Where a configured
reference profile gives a target, use that target.

- Gate: 0 mechanical problems, and the premaster peaks under -0.3 dBFS. A drowned
  lane in `audible.txt` is a critic finding, not a gate failure.
- Master: peak -1 dBFS; loud sections near -12 to -9 dB RMS (mono sum); crest
  in loud blocks 8-12 dB; breakdowns at least 6 dB under the loud sections.
- Low end: mono below 120 Hz. The kick and the bass do not mask each other.
- Width: L/R correlation 0.75-0.95 in loud sections.
- Arrangement: each section has a job, and transitions are prepared. No sound
  stops dead, and no lane is inaudible.
- Target score 8, at most 5 rounds.
