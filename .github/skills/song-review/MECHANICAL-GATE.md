# Pass the mechanical gate

This file holds the body of step 3 of the [song-review skill](SKILL.md).

1. Render the working copy: `python -I tools/song/render.py SONG OUT`. In the
   Improvement loop, you can add `--fit-gain -0.5` to fit the master gain.
   Completion criterion: `OUT/master.wav`, `OUT/premaster.wav`,
   `OUT/audible.txt`, and one `OUT/stem-ROLE.wav` for each role of a used lane
   in the metadata exist.
2. Measure the render: `python -I tools/song/measure.py SONG OUT > report.md`.
   Completion criterion: The report shows 0 mechanical problems and no
   premaster peak problem. The peak meets the applicable target. Each check
   that [Measurement limits](REFERENCE-INPUTS.md#measurement-limits) makes
   unavailable is in your report.
3. If substep 2 fails and the user asked for improvements that cover the
   repair, repair the working copy. Collect all findings of the round and fix
   them in one edit. Use `render.py --no-master` for intermediate trials only.
   Use `master.py` on the premaster to compare candidate boards. Then do
   substeps 1 and 2 again on that copy, without `--no-master`. Do not change
   the identity of a catalog source that the checker cannot measure to force a
   pass.
   Completion criterion: Both substeps pass on the same copy, or your report
   names the blocker.
4. Apply the audibility limit of the artist profile to `OUT/audible.txt`, if
   the profile sets one.
   Completion criterion: Each lane over that limit is a gate failure. Without
   a profile limit, the table goes to the critics only.
5. Read the report's "Harmonic fit (tonal lanes)" section. In the
   Improvement loop, try each "better file" within the permitted repair scope.
   Do not change a part against a standing note or an artist rule.
   `clash.py SONG OUT --lanes N --alt` lists all candidates. Keep a replacement
   only if its fit improves and the mechanical gate still passes. In other
   branches, report candidates without a replacement. Send weak bars without a
   better file to the critics as possible clashes.
   Completion criterion: You tried each candidate and kept or reverted it, or
   reported it with its scope limit.
6. Remove superseded audio only when no comparison or delivery needs it. Keep
   songs, metadata, reports, critiques, ratings, boards, and the round log.
   Before you remove source audio or a data folder, check that no saved
   song needs it.
   Completion criterion: Each file that a comparison, a delivery, or a saved
   song needs still exists.
