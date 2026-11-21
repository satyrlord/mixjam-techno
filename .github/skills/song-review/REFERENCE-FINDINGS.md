# Song-review reference: findings

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

A distance from a median, a range, or a habit of the reference album is never
correctness. Apply the [reference test](REFERENCE-NOTES.md#reference-test)
first. If the finding remains, it is taste.

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
remove. Each value is inside its control range.
```

Start the output with a comparison against the approved version, when the
brief gives one:

1. Say what changed between the approved version and this pass, and whether
   each change is better, worse, or the same, with its measurement.
2. For each finding, say whether the approved version has the same trait. If
   it has the trait and the owner approved that version, the trait is not a
   finding. A mechanical fault is the exception.

"Change nothing" is a valid result. Zero findings is a valid output. Do not
fill the finding limit.

After the findings, write a list "Keep" with at most 5 things that work and
must not be undone, each with its bars and tracks. Then write these lists,
when they have items. They do not count toward the finding limit:

- "Measured only": the fixes under the [audibility
  bar](REFERENCE-NOTES.md#audibility-bar), each with its measured value.
- "Judgements that need ears": the traits from the
  [reference test](REFERENCE-NOTES.md#reference-test) and the questions that
  only the owner's ear can answer.

Give no score.
