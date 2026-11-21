---
name: song-review
description: >
  Review, rate, or master a song made with MixJam Techno. Use to run the song
  critique loop, to brief a composition critic, mix critic, rhythm critic,
  Devil's advocate, blind rater, or mastering engineer, or to act as one of
  these roles.
---

# Review a song

The loop improves a song in rounds. Each round has a mechanical gate,
independent critiques, an integration by the main agent, and a blind rating.
A mastering engineer recommends a master board when the mix is settled. The
tools measure, but no agent can hear the song. The owner's ear outranks every
score.

Read [REFERENCE.md](REFERENCE.md) before you write a brief or a role output. It
has the evidence package, the role inputs, the tools, the measurement limits,
the finding format, the round log, and the generic targets.

## 1. Select the branch

1. Find the row of the request in this table.

   | Request | Branch | Steps to do |
   | --- | --- | --- |
   | You are launched as one of the roles | Role | The [role rules](ROLE-RULES.md), then your role file only |
   | Feedback or a rating only | Evaluation | 2, 3, the [evaluation rules](EVALUATION-RULES.md), 10 |
   | A mastering recommendation only | Recommendation | 2, 3, step 1 of [MASTER-SONG.md](MASTER-SONG.md), 10 |
   | Master the song, with no mix or arrangement change | Mastering | 2, 3, 8, 9, 10 |
   | Improve the mix or the arrangement, with or without a master | Improvement loop | 2 to 7, then 8 only if the user asks for a master, then 9 and 10 |

   Completion criterion: You know your branch and its steps.
2. If the branch is Mastering and the user also asks for feedback, also apply
   the evaluation rules.
   Completion criterion: Each requested role is in your plan.

Core rules for every branch:

- Obey the applicable creative, review, and delivery rules of the artist
  profile. They outrank a critic finding and the generic targets. The current
  instructions of the user outrank the profile. Do not copy private profile
  details into a public file.
- Before you open a visible application window or play sound on the owner's
  computer, tell the owner what opens, when, if it plays sound, and for how
  long. Use headless renders for all measurement.
- In the Improvement loop and the Recommendation branch, do not give a pass
  that fails the mechanical gate to a role. In the Evaluation branch, tell each
  role and the report about each failed or unavailable check.

Role rules: if you are a role, read [ROLE-RULES.md](ROLE-RULES.md).

Evaluation rules: read [EVALUATION-RULES.md](EVALUATION-RULES.md). They
cover the Evaluation and Recommendation branches.

## 2. Prepare the review

Read [PREPARE-REVIEW.md](PREPARE-REVIEW.md). Do its substeps in order.
Completion criterion: The working copy, standing notes, level history, approved
version, and reference sheet are ready in the review folder.

## 3. Pass the mechanical gate

Read [MECHANICAL-GATE.md](MECHANICAL-GATE.md). Do its substeps in order.
Completion criterion: `render.py` and `measure.py` passed on the current pass,
or the report names the blocker.

## 4. Get the critiques

Read [GET-CRITIQUES.md](GET-CRITIQUES.md). Do its substeps in order.
Completion criterion: Every requested role output for this round passes its
role file and the control-range rules.

## 5. Record the rating and merge the findings

1. If a rater ran, record its score and three reasons in the round log.
   Name the reviewed pass. Do not launch a second rater for that pass.
   Completion criterion: Each score names its pass and has no later song changes
   attributed to it.
2. Merge the outputs of all roles of the milestone into one
   [merged list](REFERENCE-ROUNDS.md#merged-list), `roundN-merged.md`. Merge the
   findings that name the same fault, sort them by
   [kind](REFERENCE-FINDINGS.md#finding-kinds) and severity, and move each
   finding that goes against a standing note, breaks a rule of the artist
   profile, undoes an accepted fix, or repeats a rejected finding without new
   evidence to "Conflicts" with its reason. Keep measured faults in noted areas
   visible for the owner's decision. Do not silently discard them.
   Completion criterion: Each finding of each role is in one item of the list
   or under "Conflicts".
3. Give the owner the merged list. Mark unapplied correctness items as proposed.
   In Evaluation, do not apply any item. In the Improvement loop, proceed to
   step 6.
   Completion criterion: The list states each item's status and the pass that
   its score describes.

## 6. Apply the stop rules

Apply these rules in the Improvement loop before you change the reviewed pass.
The rater's score is information for the owner. It does not start or stop a
round.

1. Stop when the merged list has no correctness item over the audibility bar,
   and the owner selects no taste item or audition.
2. Stop after the maximum number of rounds.
3. Stop when the owner's notes say that the song is done, or change its
   direction.
4. If a stop rule applies, keep the reviewed pass. Skip step 7.
   Continue at step 8 only if the user asked for a master. Otherwise, continue
   at step 9.
5. If no stop rule applies, continue at step 7.

Completion criterion: The round log names the reviewed pass and its stop
decision.
For defaults without a profile, use [Generic targets](REFERENCE-ROUNDS.md#generic-targets).

## 7. Integrate the findings

1. Apply each correctness item without asking the owner. Verify each one with
   the tool that measured it, and keep it only if the measurement improved and
   the gate passes.
   Completion criterion: Each correctness item is built and verified, or
   reverted with its reason, in one line of the merged list.
2. Update the merged list: the correctness items as done, then the
   taste items, the auditions, and the conflicts. Do not apply a taste item or
   an audition until the owner selects it. The owner can select "all" and then
   keep or reject each change by ear on the version's A/B page (step 9.2).
   Completion criterion: Each applied taste item and audition has the owner's
   choice in the round log.
3. Record each decided item of the merged list in the accepted table or in the
   rejected table of the round log, with a reason.
   Keep undecided owner choices pending.
   Completion criterion: Each rejection of a conflict names the note, the rule,
   or the earlier round.
4. Build the next pass with the accepted items.
   Completion criterion: The accepted table states each change as built, with
   bars and tracks.
5. Pass the mechanical gate (step 3) on the new pass.
   Completion criterion: The gate passes.
6. Start the next round at step 4 at the next milestone.
   Do not apply a previous pass's score to the new pass.
   Completion criterion: The next round reviews the new pass, or the report
   names the pending milestone.

## 8. Master the song

Read [MASTER-SONG.md](MASTER-SONG.md). Do its substeps in order.
Completion criterion: The mastering engineer's recommendation is decided, and
for Mastering any fitted master is saved and measured.

## 9. Get the owner's ear

1. Copy the completed pass and its final application render to `SONG_FOLDER`.
   If metadata exists, copy it beside the song with the matching `.meta.json`
   name.
   Completion criterion: `SONG_FOLDER` has the song, the WAV, the available
   metadata, and each other output that the artist profile requires.
2. If the artist profile names an A/B listening tool, make the page that
   compares this pass with the last delivered one, with one row for each
   change, and give the owner its path. Do not open the page or play it
   yourself.
   Completion criterion: The owner has the page's path, or the profile names
   no tool.
3. Search the sample and library paths of the delivered song for paths in the
   review folder.
   Completion criterion: No path points into the review folder, or you keep
   those files and your report names the dependency.
4. If the owner gives notes, record them in the round log before the next
   round. Put each judgement that outlasts the next version in the standing
   notes. Apply the owner's keep and reject choices from the A/B page.
   Completion criterion: Each note is in the round log, the standing notes, or
   the artist profile, or your report says that listening is pending.

## 10. Report

1. Report each pass, its gate result, each score, the stop rule, and the open
   questions that need ears. Name the pass for each score.
   If the song changed after that rating, state that the final pass is unrated.
   Completion criterion: The owner can decide the next step without the logs.

## Completion criterion

- Improvement loop: a stop rule applies, the round log records each decision
  with a reason, and the report says which judgements need listening.
- Evaluation: the report gives the requested findings or rating and the
  limits of their evidence.
- Recommendation and Role: the role output is in the review folder.
- Mastering: the fitted master is in `SONG_FOLDER`, and the report states its
  measurement limits.
