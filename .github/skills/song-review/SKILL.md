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
   |---|---|---|
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
- Write your output to the output path in your brief. Return it as your final
  message.

Completion criterion for a role: Your output follows the format in your role
file and is at the output path.

Evaluation rules:

- Launch each requested critic as a fresh agent (step 4). For a general
  review, use the composition critic and the mix critic.
- If the user asks for a rating, launch the blind rater as a fresh agent
  (step 6).
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
   Completion criterion: You know the tools and the `.env.local` keys that they need.
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

1. Write a brief for each critic. Give it its role, its inputs from
   [Role inputs](REFERENCE.md#role-inputs), the review folder, and the output
   path from [Round log](REFERENCE.md#round-log).
   Completion criterion: Each brief names each path that its row of the role
   inputs table needs, and the source of each target.
2. Launch the composition critic (`song-composition-critic`) and the mix critic
   (`song-mix-critic`) at the same time, each as a fresh agent.
   Completion criterion: Both critics run in parallel, and neither gets the
   output of the other.
3. If the user or the artist profile asks for a rhythm review, and the song
   has drums, glitches, or ratchets, also launch the rhythm critic
   (`song-rhythm-critic`) as a fresh agent beside the other two.
   Completion criterion: The rhythm critic runs, or no rhythm review was asked for.
4. Check each critique against its role file.
   Completion criterion: Each composition and mix critique has at most 8
   findings with bars, tracks, evidence, a fix, and a severity, at most 5
   things to keep, and no score. The rhythm critique has each part that
   `RHYTHM-CRITIC.md` lists.
5. If the user or the artist profile asks for a Devil's advocate review,
   launch the Devil's advocate (`song-devils-advocate`) as a fresh agent. Do
   this after all other critiques of the round are written.
   Completion criterion: Its output follows `DEVILS-ADVOCATE.md`, and no
   proposal goes against an owner decision.

The agents are in `.claude/agents/` for Claude Code and in `.codex/agents/` for
Codex. The Codex names use underscores, for example `song_composition_critic`.
If the harness has no named agents, launch a fresh general agent. Tell it to
follow this skill in the named role.

## 5. Integrate the findings

1. Give the owner the audition plan of the rhythm critic and the proposals of
   the Devil's advocate. Do not apply one of these proposals until the owner
   selects it.
   Completion criterion: Each applied proposal of these two roles has the
   owner's choice in the round log.
2. Accept or reject each finding of the composition and mix critiques, and
   each fix that the owner selected. Give a reason for each decision.
   Completion criterion: The round log has each finding in the accepted table
   or in the rejected table.
3. Reject a finding that breaks a rule of the artist profile, undoes an
   accepted fix, or repeats a rejected finding without new evidence.
   Completion criterion: Each of these rejections names the rule or the earlier round.
4. Build the next pass with the accepted fixes.
   Completion criterion: The accepted table states each change as built, with
   bars and tracks.
5. Pass the mechanical gate (step 3) on the new pass.
   Completion criterion: The gate passes.

## 6. Get a blind rating

1. Launch the rater (`song-rater`) as a fresh agent. Give it only the rater's
   inputs from [Role inputs](REFERENCE.md#role-inputs).
   Completion criterion: The brief has no round log, critique, earlier rating,
   rating target, or score.
2. Record the score and the three reasons in the round log.
   Completion criterion: The round log has the score of this pass.

## 7. Apply the stop rules

1. Stop when the score reaches the target score.
2. Stop after the maximum number of rounds.
3. Stop when the score does not rise for 2 rounds.
4. Stop when the owner's notes say that the song is done, or change its direction.
5. If no stop rule applies, start the next round at step 4.

Completion criterion: The round log states the stop rule that stopped the loop,
or the next round started. The defaults for a song without a profile are in the
[generic targets](REFERENCE.md#generic-targets).

## 8. Master the song

1. Launch the mastering engineer (`song-mastering-engineer`) as a fresh agent.
   Give it its inputs from [Role inputs](REFERENCE.md#role-inputs).
   Completion criterion: It returns a board JSON, a more restrained
   alternative, a bolder alternative, and a list of the mix problems that the
   board cannot fix.
2. Accept or reject the board and each mix problem in the round log.
   Completion criterion: Each decision has a reason.
3. Fit the accepted board:
   `python -I tools/song/master.py PREMASTER BOARD.json OUT_DIR --song SONG --fit DB`.
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
   `python -I tools/song/master.py PREMASTER OUT_DIR/board.json FINAL_DIR --song SONG --no-render`.
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
2. Search the sample and library paths of the delivered song for paths in the
   review folder.
   Completion criterion: No path points into the review folder, or you keep
   those files and your report names the dependency.
3. If the owner gives notes, record them in the round log before the next round.
   Completion criterion: Each note is a rule in the next critic briefs or in
   the artist profile, or your report says that listening is pending.

## 10. Report

1. Report each pass, its gate result, each score, the stop rule, and the open
   questions that need ears. If a score is from a pass before the last board
   change, say so.
   Completion criterion: The owner can decide the next step without the logs.

## Completion criterion

- Improvement loop: a stop rule applies, the round log records each decision
  with a reason, and the report says which judgements need listening.
- Evaluation: the report gives the requested findings or rating and the
  limits of their evidence.
- Recommendation and Role: the role output is in the review folder.
- Mastering: the fitted master is in `SONG_FOLDER`, and the report states its
  measurement limits.
