# Pass the mechanical gate

This file holds the body of step 3 of the [song-review skill](SKILL.md).

1. Render the working copy: `python -I tools/song/render.py SONG OUT`. In the
   Improvement loop, you can add `--fit-gain -0.5` to fit the master gain.
   Completion criterion: `OUT/master.wav`, `OUT/premaster.wav`,
   `OUT/audible.txt`, and one `OUT/stem-ROLE.wav` for each role of a used lane
   in the metadata exist. The resolved sources agree with the song.
2. Measure the render: `python -I tools/song/measure.py SONG OUT > report.md`.
   Completion criterion: The report shows 0 mechanical problems and no
   premaster peak problem. The peak meets the applicable target. Each check
   that [Measurement limits](REFERENCE-INPUTS.md#measurement-limits) makes unavailable
   is in your report.
3. If substep 2 fails and the user asked for improvements that cover the
   repair, repair the working copy. Then do substeps 1 and 2 again on that copy.
   Completion criterion: Both substeps pass on the same copy, or your report
   names the blocker. Do not change the identity of a catalog source that the
   checker cannot measure to force a pass.
4. Apply the audibility limit of the artist profile to `OUT/audible.txt`, if
   the profile sets one.
   Completion criterion: Each lane over that limit is a gate failure. Without
   a profile limit, the table goes to the critics only.
5. Read the report's "Harmonic fit (tonal lanes)" section.
   In the Improvement loop, try each "better file" within the permitted repair
   scope.
   Do not change a part against a standing note or an artist rule.
   `clash.py SONG OUT --lanes N --alt` lists all candidates.
   Keep a replacement only if its fit improves and the mechanical gate still
   passes.
   In other branches, report candidates without a replacement.
   Completion criterion: Each candidate is tried, reverted, or reported with its
   scope limit.
   Send weak bars without a better file to the critics as possible clashes.

In the Improvement loop and the Recommendation branch, do not give a pass that
fails the gate to a role. In the Evaluation branch, tell each role and the
report about each failed or unavailable check.

Keep repair rounds short:

- Collect all findings of the round and fix them in one edit. Do not render
  after each fix.
- Use `render.py --no-master` for intermediate repair trials. Render without
  that option before the gate, because `measure.py` and the evidence package
  need a current `master.wav`. `render.py` uses its cache for eligible
  unchanged lanes.
- Use `master.py` on the premaster to compare candidate boards.
- Remove superseded audio only when no comparison or delivery needs it. Keep
  songs, metadata, reports, critiques, ratings, boards, and the round log.
  Before you remove source audio or a data folder, make sure that no saved
  song needs it.
