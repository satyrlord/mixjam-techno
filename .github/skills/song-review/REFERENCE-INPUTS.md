# Song-review reference: evidence, inputs, and tools

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
| Approved version | the master render of the last version that the owner approved by ear, when there is one. A pass that the owner did not hear is not approved. |
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
| Composition critic | from round 2, the round log | the critiques of the current round; a rating |
| Mix critic | from round 2, the round log | the critiques of the current round; a rating |
| Rhythm critic | the composer script, with the drum map and the lists of glitches and ratchets; the owner's earlier notes on drums and glitches, or the file that holds them; from round 2, the round log | the critiques of the current round; a rating |
| Devil's advocate | the critiques of the current round; the round log or the handoff; the owner verdicts file (`SAMPLE_OWNER_VERDICTS` in `tools/song/README.md`) | a rating or a score |
| Blind rater | nothing more; each text item of its package is a screened extract (see below). It gets the reference sheet and the level history when they hold no score. | the round log; a critique; an earlier rating; a rating target; the approved version |
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
  "% heard" measures masking. A low value alone is not a fault. A hand-made
  mix of a reference album can have half of its lanes under 10% heard.
- Mastering changes the tone balance. For example, a master can have about
  3 dB more low end against the rest than its premaster. Compare a premaster,
  a lane sum, or a pass without a board only with a premaster target. If a
  profile gives a target measured on masters, apply it to a master only.
- Without a song file, a bar grid from the tempo alone can sit a fraction of
  a bar off the real bar lines. Before you place an event inside a bar, or
  call it early or late against a section change, find the real bar lines
  from the step times of clear entries, such as a beat or a hat layer that
  comes in.
- `measure.py` cannot measure a catalog-only source. It marks that check as
  unavailable.
- Without metadata, the gate cannot check phrase cuts by tonal role.
- Harmonic fit needs matching lane and premaster sample rates. Dry-source
  alternatives need linked WAV files and exclude reversed clips and Pitch
  automation. They also exclude simultaneous clips in a rendered lane.
  They do not model EQ or drive. Report each unavailable comparison.
  Verify each candidate with a new render.
