# Song-review reference

## Contents

- [Evidence package](#evidence-package)
- [Role inputs](#role-inputs)
- [Tools](#tools)
- [Measurement limits](#measurement-limits)
- [Standing notes](#standing-notes)
- [Finding kinds](#finding-kinds)
- [Finding format](#finding-format)
- [Merged list](#merged-list)
- [Round log](#round-log)
- [Generic targets](#generic-targets)

## Evidence package

The evidence package of a pass is a set of paths:

| Item | Source |
| --- | --- |
| The song | `SONG.mixjam` |
| Lane names and roles, sections, tempo, key | `SONG.meta.json`, when present, from `tools/song/mixjam.py` |
| Renders | `OUT/master.wav`, `OUT/lane-NN.wav`, `OUT/premaster.wav`, and one `OUT/stem-ROLE.wav` for each role, from `render.py` |
| Measurement report | `report.md` from `measure.py` |
| Audibility table | `OUT/audible.txt` from `render.py` |
| Composer script | the script, if one built the song |
| Standing notes | `standing-notes.md` in the song's folder, when present (see [Standing notes](#standing-notes)) |
| Artist profile | the file at `ARTIST_PROFILE`, when available |
| Reference studies | the files that the artist profile names, when available |
| Reference profile | `REFERENCE_PROFILE`, or `reference.json` beside the artist profile, when available. Mastering measurements against a reference need it. |
| Review folder and output path | where the role writes its output |

## Role inputs

Each role gets the evidence package and the items in its row. A role must not
read an item in its "Never" column.

| Role | Also gets | Never |
| --- | --- | --- |
| Composition critic | from round 2, the round log | the critiques of the current round; a rating |
| Mix critic | from round 2, the round log | the critiques of the current round; a rating |
| Rhythm critic | the composer script, with the drum map and the lists of glitches and ratchets; the owner's earlier notes on drums and glitches, or the file that holds them; from round 2, the round log | the critiques of the current round; a rating |
| Devil's advocate | the critiques of the current round; the round log or the handoff; the owner verdicts file (`SAMPLE_OWNER_VERDICTS` in `tools/song/README.md`) | a rating or a score |
| Blind rater | nothing more; each text item of its package is a screened extract (see below) | the round log; a critique; an earlier rating; a rating target |
| Mastering engineer | the premaster; the current board, as a JSON file or as the `master` object of the song; the loudness target, if the caller sets one; a decisions-only extract, if earlier board decisions are necessary | a critique; a rating; the round log |

A critic that reads the round log does not undo an accepted fix. It does not
raise a rejected finding again without new evidence.

Give critics and the Devil's advocate round-log extracts without ratings or
scores.
The standing notes hold the owner's intent. Every role receives them.
Screen them for prohibited material before you give them to the blind rater.

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
| --- | --- |
| Build or change a song from a script | `mixjam.py` (library) |
| Render the lanes and the master; sum the premaster, the stems, and the audibility table; fit the master gain | `render.py` |
| Gate and measure | `measure.py` |
| Clips that cut their loop's phrase (part of the gate) | `phrase.py` |
| Bars where a tonal lane leaves the bed's chord, and the files that fit there | `clash.py` |
| Cut a render's silent tail for delivery | `trim.py` |
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
- Without metadata, the gate cannot check phrase cuts by tonal role.
- Harmonic fit needs matching lane and premaster sample rates. Dry-source
  alternatives need linked WAV files and exclude reversed clips and Pitch
  automation. They also exclude simultaneous clips in a rendered lane.
  They do not model EQ or drive. Report each unavailable comparison.
  Verify each candidate with a new render.

## Standing notes

The standing notes of a song are the owner's judgements that hold until the
owner changes them: a verdict on a sound, an intended character, a level
intent, an approved exception or device, a rejected proposal. They keep the
roles from raising the same settled point in every round. The main agent keeps
them in `standing-notes.md` in the song's folder:

```markdown
# Standing notes: SONG TITLE

| # | Date | Where | Owner's judgement | For the roles |
| --- | --- | --- | --- | --- |
| 1 | 2026-01-01 | whole song | The dark top end is intended. | No finding on missing air or brightness. |
| 2 | 2026-01-02 | bars 65-68, all tracks | The hole works. | Do not fill it or shorten it. |
```

- Write the judgement in the owner's words where possible. Give the bars and
  the tracks it covers.
- Add a note when the owner gives a judgement that outlasts the next version.
  A note for the next version only goes in the round log.
- When the owner reverses a note, delete it, and record the change in the round
  log.
- Do not put a score, a rating, or a critique in the file.

A role does not raise a finding or a proposal against a note. If its evidence
shows a mechanical fault in a noted area, such as a click, a clip, or a phase
loss, it reports the fault under the heading "Conflicts with standing notes",
with the note's number. These do not count toward the finding limit.

## Finding kinds

Each finding has one kind. The kind decides who chooses.

| Kind | What it is | Who chooses |
| --- | --- | --- |
| correctness | A fault with one right answer that a tool measures: a cut phrase, a file off the bed's chord where a fitting file exists, tuning between layers, a clip or a click, an overlap, a phase or mono loss, a fader off the whole-dB grid, a level that misses a numeric target the owner set. | Within the authorized Improvement scope, the main agent applies and verifies it. Other branches report it without a song change. No audition. |
| taste | A choice that ears decide on sounds that are already in the song: a level, EQ, width, send, or fade without a numeric owner target; a part's place; a section's form. | The owner, from the merged list. |
| identity | A new sound family or instrument, the replacement of a sound, a new device or section. | The owner, by ear, from an audition of candidates. |

A measured fix that changes how a part sounds, such as another chord file, is
still correctness, but it is a row on the version's A/B page, so the owner can
reject it by ear. When you are not sure of the kind, use the kind that gives
the owner the choice.

## Finding format

The composition, mix, and rhythm critics use this format. Number the findings
in order of importance, most important first.

```markdown
### N. Short title (severity: high | medium | low)

**Kind**: correctness | taste | identity.
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

## Merged list

At a milestone, the main agent merges the outputs of all roles into one list,
`roundN-merged.md`. The owner reads this list, not the role outputs.

```markdown
# Round N merged list (pass N)

Score X.X for pass N (blind rater).

## Applied without asking (correctness)
1. Short title. Bars, tracks. The change, and the measurement before and after. (sources: M2, R4)

## For the owner (taste)
2. Short title (severity). Bars, tracks. The fix in one or two lines. (sources: C1, M3)

## Auditions (identity)
3. Short title. What the audition compares. (sources: rhythm plan 2, DA 1)

## Conflicts
- M5 goes against note 3 (the hole at 65-68). Dropped.
- R2 repeats round 4's rejected C6, with no new evidence. Dropped.
- M6 measures a click in note 4's approved cut. Pending the owner's decision.
```

- Merge the findings that name the same fault at the same bars into one item,
  and name all its sources. The sources are C (composition), M (mix), H
  (rhythm), D (Devil's advocate) and R (the rater's items), with their numbers.
- Order each section by severity. Number the items straight through the list,
  so that the owner can answer with numbers.
- Each finding of each role is in one item or under "Conflicts".
- Before repairs, use "Proposed correctness" instead of "Applied without
  asking".
  Use that heading throughout Evaluation. Do not describe a proposed change as
  done.
- Keep mechanical faults from "Conflicts with standing notes" pending for the
  owner.
  A settled judgement does not make a measured fault disappear.
- If no rater ran, omit the score. A score describes only the pass that the
  rater reviewed.

## Round log

The main agent keeps one round log for each song: `rounds.md` in the review
folder. For each round, write:

```markdown
## Round N (pass N -> pass N+1)

Critiques: `roundN-composition.md` (C1-C8), `roundN-mix.md` (M1-M8).
Merged list: `roundN-merged.md`.

### Tool changes
- what changed in the tools, and why.

### Accepted
| Finding | Fix as built in pass N+1 |
| --- | --- |

### Rejected
| Finding | Reason |
| --- | --- |

### Rating
Pass N: score X.X (parts ...). Three reasons. Judgements that need ears.
Pass N+1 is unrated until its own review.

### Owner notes
- the owner's verdict and decisions, dated.
```

Name the role outputs `roundN-composition.md`, `roundN-mix.md`,
`roundN-rhythm.md`, `roundN-devils-advocate.md`, `roundN-rating.md`, and
`roundN-master.md`. Name the merged list `roundN-merged.md` and the boards
`board-roundN.json`.

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
