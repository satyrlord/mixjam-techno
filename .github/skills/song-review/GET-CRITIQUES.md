# Get the critiques

This file holds the body of step 4 of the [song-review skill](SKILL.md).

Run the critics at a milestone only:

- the end of a build stage, such as the bed, the drums, or a new instrument
  placed through the song.
- the pass before a master or before the owner masters by hand.
- a request from the owner.

Between milestones, repair with the gate and the owner's notes only. Do not
launch a critic for one small change.

1. Select the roles of the round. The Improvement loop needs the composition
   critic, the mix critic, and the blind rater. Evaluation needs only the
   requested roles. Add the rhythm critic if the song has drums,
   glitches, or ratchets. Do this only when the user or the artist profile
   asks for a rhythm review.
   Completion criterion: The plan lists each role of the round, or the report
   says that no role applies.
2. Write a brief for each selected role. Give it its role and the review folder.
   Give it the evidence package and the extra items of its row in
   [Role inputs](REFERENCE-INPUTS.md#role-inputs). Give it the output path from
   [Round log](REFERENCE-ROUNDS.md#round-log). Say that "change nothing" is a
   valid result.
   Completion criterion: Each brief names the paths that its row of the role
   inputs needs. It also names the source of each target.
3. Launch the selected roles as fresh agents on the same pass. Run independent
   roles in parallel within the harness capacity. Use these agents:
   `song-composition-critic`, `song-mix-critic`, `song-rater`, and
   `song-rhythm-critic`.
   Completion criterion: Each selected role runs once. No role gets another
   role's output.
4. Check each critique against its role file and the
   [finding format](REFERENCE-FINDINGS.md#finding-format).
   Completion criterion: Each critique has the parts and the limits that its
   role file gives, and no score. Each level, EQ, width, send, drive, or fade
   fix states its measured size from the
   [audibility bar](REFERENCE-NOTES.md#audibility-bar). Each proposed setting
   is inside its [control range](REFERENCE-CONTROLS.md#control-ranges), or the
   merged list names the limit.
5. If the user or the artist profile asks for a Devil's advocate review, wait
   for all other critiques of the round. Then write its brief as in substep 2.
   Launch `song-devils-advocate` as a fresh agent.
   Completion criterion: Its output follows `DEVILS-ADVOCATE.md`, and no
   proposal goes against an owner decision or a standing note.

The agents are in `.claude/agents/` for Claude Code and in `.codex/agents/` for
Codex. The Codex names use underscores, for example `song_composition_critic`.
If the harness has no named agents, launch a fresh general agent. Tell it to
follow this skill in the named role.
