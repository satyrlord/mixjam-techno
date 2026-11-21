# Explore-test reference

Read this file when [SKILL.md](SKILL.md) points to a section.

## Contents

- [Note kinds](#note-kinds)
- [Evidence methods](#evidence-methods)
- [Report format](#report-format)

## Note kinds

Write one line for each note: the kind, the plan step, your action, the
result, and the evidence path. Use these kinds.

| Kind | Use it for |
| --- | --- |
| `bug` | A function gives a wrong result, a crash, or a refused valid action. |
| `usability` | The function works, but the user has friction. |
| `agent-gap` | A control is on screen but not in `ui`, or its name differs from its visible label. |
| `docs-gap` | The docs or the help text are missing, stale, or wrong. |
| `works` | A function gave the expected result, verified with evidence. |
| `question` | A behavior can be a defect or a design choice. The code, the docs, and the help text do not decide it. Also each default that you used for a missing parameter. |

Examples of `usability` notes:

- A control needs many steps for a common action.
- An action gives no feedback.
- The status text is stale or wrong.
- An action has a surprising side effect.
- A control is below the fold, or a panel hides it.
- A state does not persist after a close and reopen.
- An action has no undo, or undo reverses more or less than the action.

Examples of `agent-gap` notes:

- A control is missing from the accessibility tree.
- A control has no accessible name, or two controls have the same name.
- A name in `ui` differs from the visible label.
- Only `pointer` or `key` input can operate a function.

## Evidence methods

- Screenshot: send `screenshot path=FULL_PATH.png` and keep the file in
  `tmp/SESSION/`. Name each file with the step number.
- State: keep the `state` reply that shows the value that you claim.
- Output file: write a small measurement script with the Write tool. Run it
  with `python -I`. Give it the path of the output file as an argument.
- Audio output: render the output to a file. Measure the peak level of each
  step, the positions of the onsets, and the offset of the first hit. Compare
  the numbers with the plan.
- Text or data output: compare the file with the expected content. Report each
  difference.
- Persistence: close and reopen the subject, or restart the session on the same
  data folder. Compare `state` before and after.

A claim with no evidence is a suspicion. Mark it as a suspicion in the report.

## Report format

Use this order. Do not write a group that has no issue.

1. Summary: the subject, the task, the output path, and the counts of issues.
2. Bugs.
3. Usability issues.
4. Agent and accessibility gaps.
5. Docs gaps.
6. Verified-working functions.
7. Questions for the owner: each `question` note, with your recommended answer.
8. Coverage checklist: each function with `used`, or `not used` and the reason.

Give each issue these fields:

- Title: one line.
- Steps: the numbered steps that reproduce it, from a fresh session.
- Result: what happened, and what you expected.
- Evidence: the path of the screenshot, the `state` reply, or the script output.

Do not write a fix in the report unless the owner asked for a fix.
