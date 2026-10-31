---
name: song-review
description: >
  Review, rate, or master a song made with MixJam Techno. Use to run the song
  critique loop, to brief a composition critic, mix critic, blind rater, or
  mastering engineer, or to act as one of these roles.
---

# Review a song

The loop improves a song in rounds. Each round has a mechanical gate, two
independent critics, an integration by the main agent, and a blind rating. A
mastering engineer recommends a master board when the mix is settled. The tools
measure; no agent can hear the song. The owner's ear outranks every score.

Read [REFERENCE.md](REFERENCE.md) for the evidence package, the tools, the
round log format, and the generic targets.

## 1. Select the branch

1. If you are a critic, the rater, or the mastering engineer, read your role
   file and do only that role:
   [COMPOSITION-CRITIC.md](COMPOSITION-CRITIC.md), [MIX-CRITIC.md](MIX-CRITIC.md),
   [RATER.md](RATER.md), or [MASTERING-ENGINEER.md](MASTERING-ENGINEER.md).
   Completion criterion: Your output follows the format in your role file.
2. If the user asks only for feedback or a rating, select the evaluation branch.
   Completion criterion: The requested feedback or rating is reported with its limits.
3. If the user asks only for a mastering recommendation, select the single-role branch.
   Completion criterion: The mastering recommendation is in the review folder.
4. If the user asks to master the song without mix or arrangement improvements,
   select the mastering branch.
   Completion criterion: The requested master is prepared and its limits are reported.
5. If the user asks to improve the mix or arrangement, run the improvement
   loop: steps 2 to 7. This includes a request for both improvement and mastering.
   Completion criterion: A stop rule in step 7 applies.

For evaluation, do steps 2 and 3. Launch each requested critic and, if the user
asks for a rating, the blind rater as fresh agents. For a general review, use
both critics. Give each role the available
[evidence package](REFERENCE.md#evidence-package) and any mechanical problems.
Screen the rater's artist and reference extracts as the evidence package requires.
The rater must not receive critiques, earlier ratings, or rating targets. A failed gate is a
finding, not permission to alter the song. If there is no usable render, report
the blocker and do not invent a rating. Do not change the song, apply a board,
or copy a result outside the review folder for an evaluation-only request.
If an applicable artist rule requires a passed gate before review, report the
mechanical findings and the blocked feedback instead of sending a failed pass.

For a single mastering recommendation, do steps 2 and 3. After the gate passes,
launch the mastering engineer as a fresh agent. Check its output against its
role file. Do not apply the board. If the gate is blocked, report the blocker.

For mastering, do steps 2 and 3. If the user also asks for feedback, run the
requested evaluation roles. After the gate passes, do steps 8 to 10. Do not
change the arrangement unless the user also asks for improvements.

After an improvement loop, do step 8 only if the user also asks for a master.
Otherwise use the last gated master render for delivery. Then do steps 9 and 10.

## 2. Set up

1. Read `AGENTS.md`, `.env.example`, and `tools/song/README.md`.
   Completion criterion: You know the tools and the keys they need.
2. Read the artist profile at the path set by `ARTIST_PROFILE`. The environment
   takes precedence over `.env.local`.
   Completion criterion: You know the style, the rules, the targets, the
   target score, and the maximum number of rounds.
3. If the artist profile is missing, use the generic targets in REFERENCE.md
   unless a configured reference profile supplies the applicable target.
   Completion criterion: Your report and every brief name the source of each
   target and state when there is no artist profile.
4. Select a private review folder under `tmp/` for the song.
   Completion criterion: The folder holds the round log, or you created it.
5. Copy the song and its `.meta.json`, if present, into the review folder.
   Run the gate on a working copy. `--fit-gain` changes the song it receives.
   Completion criterion: The original song and its metadata are unchanged.
6. For an earlier song format or a song whose linked WAVs need library roots,
   convert the copy to format 18 with the native release executable. Set
   `MIXJAM_TECHNO_DATA_DIR` to a private folder in the review folder and use
   `convert --mix INPUT --output OUTPUT.mixjam`, with `--samples FOLDER` for
   each required WAV folder. Copy any metadata beside the converted song with
   the matching `.meta.json` name.
   Completion criterion: The converted copy has the expected clips and sources.
   No missing source or unconfirmed sample conflict is mistaken for a resolved
   sound. Report an unresolved source or conflict as a blocker. Keep the data
   folder while the converted song needs files retained there.

Follow the artist profile's applicable creative, review, and delivery rules.
It outranks a critic finding and the generic targets. The user's current
instructions take precedence. Do not copy private profile details into a
public-facing file.

Before you open a visible application window or play sound on the owner's
computer, tell the owner what opens, when, whether it plays sound, and for how
long. Use headless renders for all measurement.

## 3. Pass the mechanical gate

1. Render the working copy with `render.py SONG OUT`. If the user asked you
   to improve the song, you can add `--fit-gain -0.5` to fit its gain.
   The tool renders each lane and the master, then sums the lanes into the
   premaster, the stems, and the audibility table.
   Completion criterion: `master.wav`, `premaster.wav`, `audible.txt`, and a
   stem for each role of a used lane named in the metadata exist. Without
   metadata, there are no role stems. The resolved sources match the song.
2. Measure it: `measure.py SONG OUT > report.md`.
   Completion criterion: The report shows 0 mechanical problems and no
   premaster peak warning. The peak also meets the applicable target. Report
   the melodic-ending check as unavailable if role metadata is missing.
3. If substep 2 fails, repair the working copy only when the user requested
   improvements that cover the repair. Run substeps 1 and 2 again on that copy.
   Completion criterion: Both substeps pass on the same copy, or the report
   names the blocker. A catalog source that the checker cannot measure is an
   unavailable check; do not change its identity to force a pass.

A drowned lane in `OUT/audible.txt` does not fail the generic gate. If the
artist profile sets a stricter limit, apply that limit. Every placed sound must
be audible, but the meter reads narrow high sounds and parts that a delay
carries as quieter than they sound. Give the table to the critics. The owner's
ear decides what is audible.

For an improvement loop or mastering recommendation, do not give a pass that
fails the gate to a critic or rater. For evaluation only, disclose each failed
or unavailable check to any roles permitted to receive the pass, and in the report.

## 4. Get two critiques

1. Write each critic's brief: the role, the evidence package, the review
   folder, the output path, and, from round 2, the round log. Include the
   artist profile and its named reference studies when available.
   Completion criterion: Each brief names every path that REFERENCE.md requires.
2. Launch the composition critic and the mix critic at the same time, each as a
   fresh agent: `song-composition-critic` and `song-mix-critic`.
   Completion criterion: Both run in parallel without each other's output.
3. Check each critique against its role file.
   Completion criterion: Each has at most 8 findings with bars, tracks,
   evidence, a fix, and a severity, at most 5 things to keep, and no score.

The agents are in `.claude/agents/` for Claude Code and in `.codex/agents/` for
Codex, where the names use underscores (`song_composition_critic`). If the
harness has no named agents, launch a fresh general agent and tell it to follow
this skill in the named role.

## 5. Integrate the findings

1. Accept or reject each finding. Give a reason for each decision.
   Completion criterion: The round log has every finding in the accepted or
   the rejected table.
2. Reject a finding that breaks a rule in the artist profile, undoes an
   accepted fix, or repeats a rejected finding without new evidence.
   Completion criterion: Each such rejection names the rule or the earlier round.
3. Build the next pass with the accepted fixes. Record each fix as built.
   Completion criterion: The accepted table states what changed, with bars and tracks.
4. Pass the mechanical gate (step 3) on the new pass.
   Completion criterion: The gate passes.

## 6. Get a blind rating

1. Launch the rater as a fresh agent: `song-rater`. Give it the evidence
   package only, including screened artist and reference extracts when
   available. Keep critiques, earlier ratings, and rating targets out.
   Completion criterion: The brief has no round log, critique, earlier rating,
   or score.
2. Record the score and the three reasons in the round log.
   Completion criterion: The round log has the score of this pass.

## 7. Apply the stop rules

1. Stop when the score reaches the target score of the artist profile.
2. Stop after the maximum number of rounds.
3. Stop when the score does not rise for 2 rounds.
4. Stop when the owner's notes say the song is done or change its direction.
5. Otherwise, start the next round at step 4.

Completion criterion: The round log states which rule stopped the loop, or the
next round has started. The defaults without a profile are in REFERENCE.md.

## 8. Master the song

Apply a board when the user asks to master the song. For a recommendation-only
request, use the single-role branch.

1. Launch the mastering engineer as a fresh agent: `song-mastering-engineer`.
   Give it the premaster, the song, the current board, the artist profile and
   named reference studies, and the reference profile when available. Give it
   a decisions-only extract if earlier board decisions are needed.
   Completion criterion: It returns a board JSON, a more restrained and a bolder
   alternative, and a list of mix problems that the board cannot fix.
2. Accept or reject the board and each mix problem in the round log.
   Completion criterion: Each decision has a reason.
3. Fit the accepted board with
   `master.py PREMASTER BOARD.json OUT_DIR --song SONG --fit DB`.
   Completion criterion: The fitted `OUT_DIR/board.json` exists. The loud blocks
   meet the target on the premaster render, or the report states the difference.
4. Put the fitted board in the working song.
   Completion criterion: The song contains the settings in `OUT_DIR/board.json`.
5. Set `MIXJAM_TECHNO_DATA_DIR` to a private folder for this render. Render the
   working song to an existing `FINAL_DIR` with
   `.\target\release\mixjam-techno.exe render --mix SONG --output FINAL_DIR/master.wav`.
   Completion criterion: The final WAV comes from the fitted song.
6. Measure that render with
   `master.py PREMASTER OUT_DIR/board.json FINAL_DIR --song SONG --no-render`.
   Completion criterion: The result and any difference from the premaster test
   are recorded.

`master.py` renders the premaster as one track. Its key signals do not reproduce
the song's external master keys. The sum of lane renders can also differ from
the full mix when shared effects are nonlinear. Use the application render to
verify the finished song. Without metadata, the master report has no section
table. State that limit.

## 9. Get the owner's ear

1. Copy the completed pass and its final application render
   to `SONG_FOLDER` for the owner. If metadata exists, copy it beside the song
   with the matching `.meta.json` name.
   Completion criterion: The owner has the song, WAV, available metadata, and
   any other output required by the applicable artist profile.
   Check its sample and library paths for review-data dependencies. Reopen it
   with a separate data folder and confirm its sources. If it depends on files
   in the review folder, keep them and report the dependency.
2. If the owner gives notes, record them in the round log before the next round.
   Completion criterion: Each received note is a rule for the next critics'
   briefs, or it is in the artist profile. Otherwise report that listening is pending.

## 10. Report

1. Report each pass, its gate result, any score, any stop rule, and the open
   questions that need ears.
   Completion criterion: The owner can decide the next step without the logs.
   Identify a score recorded before the final board change as a score for that
   earlier pass.

## Completion criterion

The loop is complete when a stop rule applies, the round log records every
decision with a reason, and the report says which judgements need listening.
An evaluation reports the requested findings and their evidence limits. A
single-role run is complete when the role's output is in the review folder.
