# Evaluation rules

This file holds the evaluation rules of the [song-review skill](SKILL.md).

Evaluation rules:

- Launch each requested critic as a fresh agent (step 4). For a general
  review, use the composition critic and the mix critic.
- If the user asks for a rating, launch the blind rater as a fresh agent
  (step 4). Launch it at the same time as the requested critics.
- Record the requested rating in step 5.1. If more than one role reports, use
  the merged list of step 5.2.
- Give each role its inputs from [Role inputs](REFERENCE-INPUTS.md#role-inputs)
  and the gate findings of step 3.
- If there is no usable render, report the blocker. Do not invent a rating.
- A failed gate is a finding. It does not permit you to change the song.
- If an artist rule requires a passed gate before review, do not send a pass
  that fails the gate. Report the gate findings and the blocked feedback.
- Do not change the song. Do not apply a board. Do not copy a result out of
  the review folder.

For the Recommendation branch, launch the mastering engineer only after the
gate passes. Do not apply its board. If a blocker stops the gate, report the
blocker.
