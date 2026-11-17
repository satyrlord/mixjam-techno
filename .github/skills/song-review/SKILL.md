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
   | You are launched as one of the roles | Role | The role rules below, then your role file only |
   | Feedback or a rating only | Evaluation | 2, 3, the evaluation rules below, 10 |
   | A mastering recommendation only | Recommendation | 2, 3, 8.1, 10 |
   | Master the song, with no mix or arrangement change | Mastering | 2, 3, 8, 9, 10 |
   | Improve the mix or the arrangement, with or without a master | Improvement loop | 2 to 7, then 8 only if the user asks for a master, then 9 and 10 |

   Completion criterion: You know your branch and its steps.
2. If the branch is Mastering and the user also asks for feedback, also apply
   the evaluation rules.
   Completion criterion: Each requested role is in your plan.

Role rules. If you are a role, obey all of these:

- Read your role file: [COMPOSITION-CRITIC.md](COMPOSITION-CRITIC.md),
  [MIX-CRITIC.md](MIX-CRITIC.md), [RHYTHM-CRITIC.md](RHYTHM-CRITIC.md),
  [DEVILS-ADVOCATE.md](DEVILS-ADVOCATE.md), [RATER.md](RATER.md), or
  [MASTERING-ENGINEER.md](MASTERING-ENGINEER.md).
- Read only the inputs that [Role inputs](REFERENCE.md#role-inputs) permits
  for your role.
- Do not change the song, its renders, or the output of another role. Write
  your own numpy analysis and your test renders in the review folder only.
- Do not open the application window. Do not play sound.
- Base each claim on a measurement or on the song file. Say where it is, in
  bars and tracks. Say which judgements need ears.
- When you name a lane, also give its track number and the names of its
  sounds, for example "track 22 (Starshine Seq A 1, A 7)". The owner sees track
  numbers and sound names, not the lane names in `.meta.json`.
- Read the song's [standing notes](REFERENCE.md#standing-notes), if your brief
  names them. Do not challenge a settled judgement. Report mechanical faults
  in a noted area under "Conflicts with standing notes", with the note number.
- Give each finding its [kind](REFERENCE.md#finding-kinds): correctness, taste,
  or identity.
- Write your output to the output path in your brief. Return it as your final
  message.

Completion criterion for a role: Your output follows the format in your role
file and is at the output path.

Evaluation rules:

- Launch each requested critic as a fresh agent (step 4). For a general
  review, use the composition critic and the mix critic.
- If the user asks for a rating, launch the blind rater as a fresh agent
  (step 4), at the same time as the requested critics.
- Record the requested rating in step 5.1. If more than one role reports, use
  the merged list of step 5.2.
- Give each role its inputs from [Role inputs](REFERENCE.md#role-inputs) and
  the gate findings of step 3.
- If there is no usable render, report the blocker. Do not invent a rating.
- A failed gate is a finding. It does not permit you to change the song.
- If an artist rule requires a passed gate before review, do not send a pass
  that fails the gate. Report the gate findings and the blocked feedback.
- Do not change the song. Do not apply a board. Do not copy a result out of
  the review folder.

For the Recommendation branch, launch the mastering engineer only after the
gate passes. Do not apply its board. If the gate is blocked, report the
blocker.

## 2. Prepare the review

1. Read `AGENTS.md`, `.env.example`, and `tools/song/README.md`.
   Completion criterion: You know the tools and the `.env.local` keys that they
   need.
2. Read the artist profile at the path that `ARTIST_PROFILE` sets. An
   environment variable overrides `.env.local`.
   Completion criterion: You know the style, the rules, the targets, the
   target score, and the maximum number of rounds.
3. If there is no artist profile, use the
   [generic targets](REFERENCE.md#generic-targets). Use a reference-profile
   target where a configured reference profile gives one.
   Completion criterion: Your report and each brief name the source of each
   target, and say when there is no artist profile.
4. Select a private review folder for the song under `tmp/`.
   Completion criterion: The folder holds the round log, or you made the folder.
5. Copy the song and its `.meta.json`, if present, into the review folder.
   Do all work on this working copy. `render.py --fit-gain` changes the song
   that it gets.
   Completion criterion: The original song and its metadata are unchanged.
6. If the song's `formatVersion` is below 18, or its linked WAVs need library
   folders, convert the working copy to format 25. Set
   `MIXJAM_TECHNO_DATA_DIR` to a private folder in the review folder, then run
   `.\target\release\mixjam-techno.exe convert --mix INPUT --output OUTPUT.mixjam`
   with one `--samples FOLDER` for each necessary WAV folder. Copy the
   metadata beside the converted song with the matching `.meta.json` name.
   Completion criterion: The converted copy has the expected clips and
   sources. Each missing source or unconfirmed sample conflict is in your
   report as a blocker. Do not use a different sound in its place. Keep the
   data folder while the converted song needs its files.
7. Find the song's [standing notes](REFERENCE.md#standing-notes). If the owner
   has given judgements on the song and there is no file, make it from the
   round log or the handoff.
   Completion criterion: Each brief names the standing notes file, or the song
   has no owner judgements yet.

Obey the applicable creative, review, and delivery rules of the artist
profile. They outrank a critic finding and the generic targets. The current
instructions of the user outrank the profile. Do not copy private profile
details into a public file.

Before you open a visible application window or play sound on the owner's
computer, tell the owner what opens, when, if it plays sound, and for how long.
Use headless renders for all measurement.

## 3. Pass the mechanical gate

1. Render the working copy: `python -I tools/song/render.py SONG OUT`. In the
   Improvement loop, you can add `--fit-gain -0.5` to fit the master gain.
   Completion criterion: `OUT/master.wav`, `OUT/premaster.wav`,
   `OUT/audible.txt`, and one `OUT/stem-ROLE.wav` for each role of a used lane
   in the metadata exist. The resolved sources agree with the song.
2. Measure the render: `python -I tools/song/measure.py SONG OUT > report.md`.
   Completion criterion: The report shows 0 mechanical problems and no
   premaster peak problem. The peak meets the applicable target. Each check
   that [Measurement limits](REFERENCE.md#measurement-limits) makes unavailable
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

## 4. Get the critiques

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
   Steps 3 and 5 add rhythm and Devil's advocate reviews when requested.
   Write a brief for each selected role. Give it its role,
   its inputs from [Role inputs](REFERENCE.md#role-inputs), the standing
   notes, the review folder, and the output path from
   [Round log](REFERENCE.md#round-log).
   Completion criterion: Each brief names each path that its row of the role
   inputs table needs, and the source of each target.
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
   5 things to keep, and no score. The rhythm critique has each part that
   `RHYTHM-CRITIC.md` lists.
5. If the user or the artist profile asks for a Devil's advocate review,
   launch the Devil's advocate (`song-devils-advocate`) as a fresh agent. Do
   this after all other critiques of the round are written.
   Completion criterion: Its output follows `DEVILS-ADVOCATE.md`, and no
   proposal goes against an owner decision or a standing note.

The agents are in `.claude/agents/` for Claude Code and in `.codex/agents/` for
Codex. The Codex names use underscores, for example `song_composition_critic`.
If the harness has no named agents, launch a fresh general agent. Tell it to
follow this skill in the named role.

## 5. Record the rating and merge the findings

1. If a rater ran, record its score and three reasons in the round log.
   Name the reviewed pass. Do not launch a second rater for that pass.
   Completion criterion: Each score names its pass and has no later song changes
   attributed to it.
2. Merge the outputs of all roles of the milestone into one
   [merged list](REFERENCE.md#merged-list), `roundN-merged.md`. Merge the
   findings that name the same fault, sort them by
   [kind](REFERENCE.md#finding-kinds) and severity, and move each finding that
   goes against a standing note, breaks a rule of the artist profile, undoes an
   accepted fix, or repeats a rejected finding without new evidence to
   "Conflicts" with its reason. Keep measured faults in noted areas visible
   for the owner's decision. Do not silently discard them.
   Completion criterion: Each finding of each role is in one item of the list
   or under "Conflicts".
3. Give the owner the merged list. Mark unapplied correctness items as proposed.
   In Evaluation, do not apply any item. In the Improvement loop, proceed to
   step 6.
   Completion criterion: The list states each item's status and the pass that
   its score describes.

## 6. Apply the stop rules

Apply these rules in the Improvement loop before you change the reviewed pass.

1. Stop when the score reaches the target score.
2. Stop after the maximum number of rounds.
3. Stop when the score does not rise for 2 rounds.
4. Stop when the owner's notes say that the song is done, or change its
   direction.
5. If a stop rule applies, keep the reviewed pass. Skip step 7.
   Continue at step 8 only if the user asked for a master. Otherwise, continue
   at step 9.
6. If no stop rule applies, continue at step 7.

Completion criterion: The round log names the reviewed pass and its stop
decision.
For defaults without a profile, use [Generic targets](REFERENCE.md#generic-targets).

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

1. Launch the mastering engineer (`song-mastering-engineer`) as a fresh agent.
   Give it its inputs from [Role inputs](REFERENCE.md#role-inputs).
   Completion criterion: It returns a board JSON, a more restrained
   alternative, a bolder alternative, and a list of the mix problems that the
   board cannot fix.
2. Accept or reject the board and each mix problem in the round log.
   Completion criterion: Each decision has a reason.
3. Fit the accepted board:
   `python -I tools/song/master.py PREMASTER BOARD.json OUT_DIR --song SONG
   --fit DB`.
   Completion criterion: The fitted `OUT_DIR/board.json` exists. The loud
   blocks of the premaster test meet the target, or your report states the
   difference.
4. Put the fitted board in the working song.
   Completion criterion: The song has the settings of `OUT_DIR/board.json`.
5. Set `MIXJAM_TECHNO_DATA_DIR` to a private folder for this render. Then
   render the working song to an existing `FINAL_DIR`:
   `.\target\release\mixjam-techno.exe render --mix SONG --output FINAL_DIR/master.wav`.
   Completion criterion: The final WAV comes from the fitted song.
6. Measure that render:
   `python -I tools/song/master.py PREMASTER OUT_DIR/board.json FINAL_DIR --song
   SONG --no-render`.
   Completion criterion: The round log has the result and each difference
   from the premaster test. Your report states the limits in
   [Measurement limits](REFERENCE.md#measurement-limits) that apply.

Use the application render to verify the finished song. Without step 8 in the
Improvement loop, use the last master render that passed the gate for delivery.

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
