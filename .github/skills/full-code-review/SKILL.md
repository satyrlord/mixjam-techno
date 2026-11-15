---
name: full-code-review
description: >
  Review a MixJam Techno change set against current code and product contracts.
  Use for a diff, branch, pending work, or repair of confirmed review findings.
---

# Review MixJam Techno code

Start with a read-only review. Repair findings only when the user asks for fixes.

## 1. Set the scope

1. Inspect the worktree status, the complete diff, untracked files, and recent history.
   Completion criterion: Every changed path has a review or an exclusion reason.
2. Identify the user changes that exist before you edit.
   Completion criterion: The repair scope does not include unrelated work.
3. Read each scoped file and its callers and tests.
   Completion criterion: You can describe the changed behavior and its consumers.
4. Read the documents that own the changed behavior:
   - the owner's current decisions and, if they exist,
     `local-data/research/PRODUCT.md` and `local-data/research/COMPATIBILITY.md`
     for product scope
   - `docs/ARCHITECTURE.md` for application ownership
   - `docs/DESIGN.md` for native UI work

   Completion criterion: Each changed behavior has a current contract or a stated gap.
5. Map the affected behavior to current tests and artifact evidence.
   Completion criterion: Each check that applies is passed, failed, blocked, or not run.

Treat donor material as dated evidence. Use original product artifacts and
measured behavior to assess parity claims. If skills are in scope, read them
once in `.github/skills`. The folders `.agents/skills`, `.claude/skills`, and
`.codex/skills` link to it.

## 2. Review each affected boundary

1. Check each item in this list that applies to the change:
   - product behavior, compatibility limits, and acceptance evidence
   - Rust crate, `Session`, engine, native UI, and composition ownership
   - typed edits, history, gesture boundaries, and project validation
   - JSON schema, migrations, import rejection, and file replacement
   - product and sample identity, catalogs, WAV folders, and source tempo
   - render timing, channel routing, recording, device lifetime, and background work
   - native input, focus, accessibility, layout, and bundled assets
   - original-product parity claims and preservation boundaries
   - focused tests, both Cargo feature sets, release artifact, visual, and audio evidence

   Completion criterion: Each item that applies has a result from current source and evidence.
2. If a confirmed defect needs a common repair pattern, read [REFERENCE.md](REFERENCE.md).
   Completion criterion: The remedy agrees with the owning contract.

A passing unit test does not prove native interaction, physical audio, or
original-product parity. Do not use a file-size limit. Check ownership,
cohesion, and edit cost.

## 3. Report findings

1. Order the confirmed findings by user impact and release risk.
   Completion criterion: Blocking findings come first.
2. For each finding, give the path, location, broken contract, evidence, impact, and remedy.
   Completion criterion: Another agent can reproduce each finding.
3. Account for every scoped path and affected contract.
   Completion criterion: The report keeps findings separate from areas that are not verified.

Treat existing review comments as hypotheses until current evidence confirms
them. If no defect is confirmed, say so and state the limits of the review.

## 4. Repair when the user asks

1. Fix each confirmed defect within the permitted scope.
   Completion criterion: The repair removes the stated cause.
2. Add a focused regression check for each behavior repair.
   Completion criterion: The check fails for the defect and passes for the repair.
3. Run the checks that apply, with the `run-quality-gate` skill.
   Completion criterion: Each affected check has a current result.
4. Review the final diff and the workspace state.
   Completion criterion: Each accepted finding is fixed or has a stated blocker.

For a broad or high-risk repair, get an independent review from a clean context
if one is available. Do not stage, commit, or publish as part of a review,
unless the user asks for that action.

## Completion criterion

The review accounts for each scoped path, contract, and verification result.
Every finding has reproducible evidence and a remedy that an agent can apply.
Each requested repair has direct regression evidence and current check results.
