# Song-review reference: notes and tests

## Contents

- [Standing notes](#standing-notes)
- [Level history](#level-history)
- [Reference test](#reference-test)
- [Audibility bar](#audibility-bar)

## Standing notes

The standing notes of a song are the owner's judgements. They hold until the
owner changes them. A note can be a verdict on a sound, an intended character,
a level intent, an approved exception or device, or a rejected proposal. They
stop the roles from repeating a settled point in every round. The main agent
keeps them in `standing-notes.md` in the song's folder:

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

A role does not raise a finding or a proposal against a note. If a role's
evidence shows a mechanical fault in a noted area, it reports the fault under
the heading "Conflicts with standing notes". It gives the note's number.
Examples are a click, a clip, or a phase loss. These do not count toward the
finding limit.

## Level history

A reviewer that sees one pass does not know why a level is where it is. The
level history keeps a control from going back and forth over the rounds. The
main agent keeps it in `level-history.md` in the song's folder:

```markdown
# Level history: SONG TITLE

| Control | Passes and values | Who moved it, and why | Owner's ear |
| --- | --- | --- | --- |
| track 12 fader (String Pad A) | pass 3 -4, pass 5 -7, pass 6 -10, pass 7 -7, pass 8 -4 dB | critics: masking; then the repairs went back | pass 8: -4 dB is right |
```

- Add a row when a control moves in two directions over the passes. Add a row
  when the owner settles a value by ear.
- Do not put a score, a rating, or a critique in the file.

A role does not propose to move a control in the history in a direction that
the history already tried. The exception is new evidence that names the
earlier move and says why the result is now different.

## Reference test

The artist profile can name a reference sheet. It is a short extract of the
reference album. It sets each habit of the profile against what the album's own
tracks do. Read it before you judge the song.

A finding, a deduction, or a proposal can rest on a number, a median, a range,
or a habit. Before you write one, ask: "Does the same sentence fault a track
of the reference album?"

1. If yes, it is a trait of the artist. Do not list it as a fault. Do not
   reduce a score for it. Do not propose a change for it.
2. If the trait is possibly not the owner's intent, put one line
   under "Judgements that need ears". Give the measurement, not a fix.
3. A rule that the profile marks as the owner's decision stands. Standing
   notes stand. The test is for habits and medians, not for the owner's
   decisions.

If there is no reference sheet, apply the test to the reference studies.

## Audibility bar

A fix that nobody can hear wastes a round and can harm the song. Apply this
bar to each finding with a fix that changes a level, EQ, width, send, drive,
or fade:

1. Render the pass with the fix: `render.py SONG_WITH_FIX OUT --no-master
   --cache DIR`.
2. In the bars of the fix, subtract the premaster of the pass from the
   premaster with the fix.
3. Measure the difference signal against the premaster of the pass in
   1/3-octave bands. Write the band with the largest value and that value in
   dB in the "Evidence" line.
4. If the difference is 20 dB or more under the premaster in every band, the
   fix is under the bar. Move it to the list "Measured only" with its value.

A finding under the bar does not count toward the finding limit. It is not a
row on an A/B page. The owner can still request it. A mechanical fault is
never under the bar because of its size. Examples are a click, a clip, or a
cut phrase.

The blind rater does not render. The main agent applies the bar to the
rater's items before the merged list.
