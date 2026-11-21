# Song-review reference: evidence, inputs, and tools

## Contents

- [Evidence package](#evidence-package)
- [Role inputs](#role-inputs)
- [Tools](#tools)
- [Measurement limits](#measurement-limits)

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
| Standing notes | `standing-notes.md` in the song's folder, when present (see [Standing notes](REFERENCE-NOTES.md#standing-notes)) |
| Level history | `level-history.md` in the song's folder, when present (see [Level history](REFERENCE-NOTES.md#level-history)) |
| Approved version | the master render of the last version that the owner approved by ear, when there is one. A pass has no approval until the owner hears it. |
| Artist profile | the file at `ARTIST_PROFILE`, when available |
| Reference sheet | the file that the artist profile names as the reference sheet for review roles, when present (see [Reference test](REFERENCE-NOTES.md#reference-test)) |
| Reference studies | the files that the artist profile names, when available |
| Reference profile | `REFERENCE_PROFILE`, or `reference.json` beside the artist profile, when available. Mastering measurements against a reference need it. |
| Review folder and output path | where the role writes its output |

## Role inputs

Each role gets the evidence package and the items in its row. A role must not
read an item in its "Never" column.

| Role | Also gets | Never |
| --- | --- | --- |
| Composition critic | from round 2, the round log | the critiques of the current round, and a rating |
| Mix critic | from round 2, the round log | the critiques of the current round, and a rating |
| Rhythm critic | the composer script, with the drum map and the lists of glitches and ratchets. The owner's earlier notes on drums and glitches, or the file that holds them. From round 2, the round log. | the critiques of the current round, and a rating |
| Devil's advocate | the critiques of the current round. The round log or the handoff. The owner verdicts file (`SAMPLE_OWNER_VERDICTS` in `tools/song/README.md`). | a rating or a score |
| Blind rater | nothing more. Each text item of its package is a screened extract (see below). It gets the reference sheet and the level history when they hold no score. | the round log, a critique, an earlier rating, a rating target, and the approved version |
| Mastering engineer | the premaster. The current board, as a JSON file or as the `master` object of the song. The loudness target, if the caller sets one. A decisions-only extract, if earlier board decisions are necessary. | a critique, a rating, and the round log |

A critic that reads the round log does not undo an accepted fix. It does not
raise a rejected finding again without new evidence.

Give critics and the Devil's advocate round-log extracts without ratings or
scores. The standing notes hold the owner's intent. Give them to every role.
Screen them for prohibited material before you give them to the blind rater.

Screen each text item before you give it to the rater:

1. Make extracts that keep the style rules of the artist and the objective
   measurements.
2. Remove critiques, earlier ratings, and rating targets.
3. Omit a composer script that quotes a critique or a score.
4. If you cannot screen an item, omit it and report the limit.

Do not use earlier song ratings to calibrate the rater.

## Tools

The tools are in `tools/song/`. `tools/song/README.md` owns the usage line of
each tool, what it does, and the `.env.local` keys that it needs. Run each tool
with `python -I`. The tools render headless with the release build and play no
sound.

## Measurement limits

State each limit that applies in the brief or in the report.

- Tool limits are in `tools/song/README.md` and in
  [Mechanical report](../../../tools/song/CACHE-AND-REPORTS.md#mechanical-report).
  They cover a song without a `.meta.json` file, a catalog-only source, the
  harmonic-fit alternatives, and the sum of lane renders against the full mix.
  Report each unavailable check.
- `master.py` cannot reproduce the external keys of a master board. Measure a
  keyed board from the application render of the song.
- The meter of `audible.txt` reads narrow high sounds and parts that a delay
  carries as quieter than they sound. The owner's ear decides what is audible.
  "% heard" measures masking. A low value alone is not a fault. A hand-made
  mix of a reference album can have half of its lanes under 10% heard.
- Mastering changes the tone balance. For example, a master can have about
  3 dB more low end against the rest than its premaster. Compare a premaster,
  a lane sum, or a pass without a board only with a premaster target. If a
  profile gives a target measured on masters, apply it to a master only.
- Without a song file, a bar grid from the tempo alone can sit a fraction of
  a bar off the real bar lines. Find the real bar lines before you place an
  event inside a bar, or call it early or late against a section change. Use
  the step times of clear entries, such as a beat or a hat layer that starts.
