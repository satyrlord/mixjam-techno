# Song-review reference: merged list, round log, and targets

## Contents

- [Merged list](#merged-list)
- [Round log](#round-log)
- [Generic targets](#generic-targets)

## Merged list

At a milestone, the main agent merges the outputs of all roles into one list,
`roundN-merged.md`. The owner reads this list, not the role outputs.

```markdown
# Round N merged list (pass N)

Score X.X for pass N (blind rater, for information only).

## Applied without asking (correctness)
1. Short title. Bars, tracks. The change, and the measurement before and after. (sources: M2, R4)

## For the owner (taste)
2. Short title (severity). Bars, tracks. The fix in one or two lines. (sources: C1, M3)

## Auditions (identity)
3. Short title. What the audition compares. (sources: H2, D1)

## Conflicts
- M5 goes against note 3 (the hole at 65-68). Dropped.
- R2 repeats round 4's rejected C6, with no new evidence. Dropped.
- M6 measures a click in note 4's approved cut. Pending the owner's decision.
- C4 moves track 12 down again; the level history tried that in passes 5-6. Dropped.

## Measured only
- M7 (lane width 80 %): difference 26 dB under the premaster. Not on the A/B page.
```

- Apply the [audibility bar](REFERENCE-NOTES.md#audibility-bar) to each rater
  item and to each critic finding that does not state its value. Move each item
  under the bar to "Measured only".
- Move each finding to "Conflicts", with its reason, if it:
  - fails the [reference test](REFERENCE-NOTES.md#reference-test).
  - goes against a [standing note](REFERENCE-NOTES.md#standing-notes), the
    [level history](REFERENCE-NOTES.md#level-history), or a rule of the artist
    profile.
  - undoes an accepted fix.
  - repeats a rejected finding without new evidence.
  - names a trait that the approved version already has.
- Merge the findings that name the same fault at the same bars into one item,
  and name all its sources. The sources are C (composition), M (mix), H
  (rhythm), D (Devil's advocate) and R (the rater's items), with their numbers.
- Order each section by severity. Number the items straight through the list,
  so that the owner can answer with numbers.
- Before repairs, use "Proposed correctness" instead of "Applied without
  asking".
  Use that heading throughout Evaluation. Do not describe a proposed change as
  done.
- Keep each mechanical fault in a noted area visible, as pending for the
  owner's decision. A settled judgement does not make a measured fault
  disappear. Do not discard it.
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
  lane in `audible.txt` is a question for the owner's ear, not a gate failure.
- Master: peak -1 dBFS. Loud sections near -12 to -9 dB RMS (mono sum). Crest
  in loud blocks 8-12 dB. Breakdowns at least 6 dB under the loud sections.
- Low end: mono below 120 Hz. The kick and the bass do not mask each other.
- Width: L/R correlation 0.75-0.95 in loud sections.
- Arrangement: each section has a job, and each transition has a lead-in. No sound
  stops dead, and no lane is inaudible.
- At most 5 rounds.
