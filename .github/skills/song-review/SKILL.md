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
2. If the user asks for one role only, such as one rating, do steps 2 and 3,
   then launch that role (step 4, 6, or 8).
   Completion criterion: The role's output is in the review folder.
3. Otherwise, run the full loop: steps 2 to 10.
   Completion criterion: A stop rule in step 7 applies.

## 2. Set up

1. Read `AGENTS.md`, `.env.example`, and `tools/song/README.md`.
   Completion criterion: You know the tools and the keys they need.
2. Read the artist profile at the path in the `.env.local` key `ARTIST_PROFILE`.
   Completion criterion: You know the style, the rules, the targets, the
   target score, and the maximum number of rounds.
3. If the key or the file is missing, use the generic targets in REFERENCE.md.
   Completion criterion: Your report and every brief say "generic targets, no
   artist profile".
4. Select a private review folder under `tmp/` for the song.
   Completion criterion: The folder holds the round log, or you created it.

The artist profile is the authority on style and targets. It outranks a critic
finding and the generic targets. Do not copy its private details into a
public-facing file.

Before you open a visible application window or play sound on the owner's
computer, tell the owner what opens, when, whether it plays sound, and for how
long. Use headless renders for all measurement.

## 3. Pass the mechanical gate

1. Render the pass: `render.py SONG OUT --fit-gain -0.5`. It renders each
   lane and the master, and sums the lanes into the premaster, the stems, and
   the audibility table. Render nothing else for a pass.
   Completion criterion: `master.wav`, `premaster.wav`, one stem per role, and
   `audible.txt` exist.
2. Measure it: `measure.py SONG OUT > report.md`.
   Completion criterion: The report shows 0 mechanical problems and the
   premaster does not reach full scale.
3. If substep 2 fails, fix the song and go back to substep 1.
   Completion criterion: Substeps 1 and 2 pass on the same pass.

A drowned lane in `OUT/audible.txt` does not fail the gate. Every placed sound must
be audible, but the meter reads narrow high sounds and parts that a delay
carries as quieter than they sound. Give the table to the critics: a drowned
lane is a finding for the mix critic, and the owner's ear decides it.

Do not give a pass that fails the gate to a critic or to the rater.

## 4. Get two critiques

1. Write each critic's brief: the role, the evidence package, the review
   folder, the output path, and, from round 2, the round log.
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
   package only.
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

Do this when the mix is settled, or when the owner asks for a board.

1. Launch the mastering engineer as a fresh agent: `song-mastering-engineer`.
   Give it the premaster, the song, the current board, and the artist profile.
   Completion criterion: It returns a board JSON, a more restrained and a bolder
   alternative, and a list of mix problems that the board cannot fix.
2. Accept or reject the board and each mix problem in the round log.
   Completion criterion: Each decision has a reason.
3. Put the accepted board in the song and fit its limiter gain with
   `master.py --fit`.
   Completion criterion: The loud blocks meet the loudness target.

## 9. Get the owner's ear

1. Copy the rated pass and its master render to `SONG_FOLDER` for the owner.
   Completion criterion: The owner has the song file and the WAV.
2. Record the owner's notes in the round log before the next round.
   Completion criterion: Each note is a rule for the next critics' briefs, or
   it is in the artist profile.

## 10. Report

1. Report each pass, its gate result, its score, the stop rule, and the open
   questions that need ears.
   Completion criterion: The owner can decide the next step without the logs.

## Completion criterion

The loop is complete when a stop rule applies, the round log records every
decision with a reason, and the report says which judgements need listening.
A single-role run is complete when the role's output is in the review folder.
