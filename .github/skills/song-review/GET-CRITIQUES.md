# Get the critiques

This file holds the body of step 4 of the [song-review skill](SKILL.md).

Run the critics at a milestone only:

- the end of a build stage, such as the bed, the drums, or the placement of a
  new instrument through the song;
- the pass before a master or before the owner masters by hand;
- a request from the owner.

Between milestones, repair with the gate and the owner's notes only. Do not
launch a critic for one small change.

1. Select the primary roles for this branch.
   The Improvement loop needs composition, mix, and blind rating.
   Evaluation needs only the requested primary roles.
   Substeps 3 and 5 add rhythm and Devil's advocate reviews when requested.
   Write a brief for each selected role. Give it its role,
   its inputs from [Role inputs](REFERENCE-INPUTS.md#role-inputs), the reference
   sheet, the standing notes, the level history, the approved version (not to
   the rater), the review folder, and the output path from
   [Round log](REFERENCE-ROUNDS.md#round-log). Say that "change nothing" is a valid
   result. Tell it to keep each proposed setting
   inside the [control ranges](REFERENCE-CONTROLS.md#control-ranges).
   Completion criterion: Each brief names each path that its row of the role
   inputs table needs, the source of each target, and the control ranges.
2. Launch the selected roles as fresh agents on the same pass.
   Use `song-composition-critic`, `song-mix-critic`, and `song-rater` for the
   corresponding roles.
   Run independent roles in parallel within the harness capacity.
   Completion criterion: Each selected role runs once. No role gets another
   role's output.
3. If the user or the artist profile asks for a rhythm review, and the song
   has drums, glitches, or ratchets, also launch the rhythm critic
   (`song-rhythm-critic`) as a fresh agent beside the others.
   Completion criterion: The rhythm critic runs, or no rhythm review was asked
   for.
4. Check each critique against its role file.
   Completion criterion: Each composition and mix critique has at most 8
   findings with a kind, bars, tracks, evidence, a fix, and a severity, at most
   5 things to keep, and no score. Each level, EQ, width, send, drive, or fade
   fix states its measured size from the audibility bar. The rhythm critique has
   each part that `RHYTHM-CRITIC.md` lists. Each proposed setting of each role
   is inside its [control range](REFERENCE-CONTROLS.md#control-ranges), or the
   merged list names the limit.
5. If the user or the artist profile asks for a Devil's advocate review,
   launch the Devil's advocate (`song-devils-advocate`) as a fresh agent. Do
   this after all other critiques of the round are written.
   Completion criterion: Its output follows `DEVILS-ADVOCATE.md`, and no
   proposal goes against an owner decision or a standing note.

The agents are in `.claude/agents/` for Claude Code and in `.codex/agents/` for
Codex. The Codex names use underscores, for example `song_composition_critic`.
If the harness has no named agents, launch a fresh general agent. Tell it to
follow this skill in the named role.
