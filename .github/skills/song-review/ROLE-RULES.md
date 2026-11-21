# Role rules

This file holds the role rules of the [song-review skill](SKILL.md). If you
are a role, obey the [core rules](SKILL.md#core-rules) and all of these rules:

- Read only the inputs that [Role inputs](REFERENCE-INPUTS.md#role-inputs)
  permits for your role.
- Do not change the song, its renders, or the output of another role. Write
  your own numpy analysis and your test renders in the review folder only.
- Do not open the application window. Do not play sound.
- Base each claim on a measurement or on the song file. Say where it is, in
  bars and tracks. Say which judgements need ears.
- Give each proposed setting inside its
  [control range](REFERENCE-CONTROLS.md#control-ranges). Quote a current
  setting with the name of its control. If a control is at its limit, say so,
  and propose a different control.
- When you name a lane, also give its track number and the names of its
  sounds. Example: "track 22 (Starshine Seq A 1, A 7)". The owner sees track
  numbers and sound names, not the lane names in `.meta.json`.
- If your brief names a reference sheet, read it first. Apply the
  [reference test](REFERENCE-NOTES.md#reference-test) before each finding,
  deduction, or proposal.
- If your brief names the song's
  [standing notes](REFERENCE-NOTES.md#standing-notes), read them. Do not
  challenge a settled judgement. Report mechanical faults in a noted area under
  "Conflicts with standing notes", with the note number.
- If your brief names the song's
  [level history](REFERENCE-NOTES.md#level-history), read it. Do not move a
  control in a direction that it already tried.
- If you are a critic, apply the
  [audibility bar](REFERENCE-NOTES.md#audibility-bar) to each fix that changes
  a level, EQ, width, send, drive, or fade.
- Give each finding its [kind](REFERENCE-FINDINGS.md#finding-kinds):
  correctness, taste, or identity.
- Write your output to the output path in your brief. Return it as your final
  message.

Completion criterion: Your output follows the format in your role file and is
at the output path.
