---
name: full-code-review
description: >
  Review a MixJam Techno change set against current code and product contracts.
  Use for a diff, branch, pending work, or repair of confirmed review findings.
---

# Review MixJam Techno code

Start with a read-only review. Repair findings only when the user requests fixes.

## 1. Establish the scope

1. Inspect the worktree status, complete diff, untracked files, and recent history.
   Completion criterion: Every changed path has a review or exclusion reason.
2. Identify pre-existing user changes before you edit.
   Completion criterion: The repair scope does not include unrelated work.
3. Read each scoped file and its relevant callers and tests.
   Completion criterion: You can describe the changed behavior and its consumers.
4. Read the owning product and architecture documents.
   Completion criterion: Each changed behavior has a current contract or a stated gap.
5. Map affected behavior to current tests and artifact evidence.
   Completion criterion: Each relevant check is passed, failed, blocked, or not run.

Use the owner's current decisions, `PRODUCT.md`, and `COMPATIBILITY.md` for
product scope. Use `ARCHITECTURE.md` for application ownership. Read `DESIGN.md`
for native UI work. Treat `docs/MERGE.md` and donor material as dated evidence.
Use original product artifacts and measured behavior to assess parity claims.
If skills are in scope, read them through `.github/skills` once. `.agents`,
`.claude`, and `.codex` link to that directory.

## 2. Review each affected boundary

Check each item that applies to the change:

- product behavior, compatibility limits, and acceptance evidence
- Rust crate, `Session`, engine, native UI, and composition ownership
- typed edits, history, gesture boundaries, and project validation
- JSON schema, migrations, import rejection, and file replacement
- product and sample identity, catalogs, WAV folders, and source tempo
- render timing, channel routing, recording, device lifetime, and background work
- native input, focus, accessibility, layout, and bundled assets
- original-product parity claims and preservation boundaries
- focused tests, both Cargo feature sets, release artifact, visual, and audio evidence

Judge each boundary from current source and evidence. A passing unit test does
not prove native interaction, physical audio, or original-product parity. Do not
use an arbitrary file-size limit; check ownership, cohesion, and edit cost.

If a confirmed defect needs a common repair pattern, read
[REFERENCE.md](REFERENCE.md).

## 3. Report findings

1. Order confirmed findings by user impact and release risk.
   Completion criterion: Blocking findings appear first.
2. Give a path, location, broken contract, evidence, impact, and remedy.
   Completion criterion: Another agent can reproduce each finding.
3. Account for every scoped path and affected contract.
   Completion criterion: The report distinguishes findings from unverified areas.

Treat existing review comments as hypotheses until current evidence confirms
them. If no defect is confirmed, say so and state the review limits.

## 4. Repair when requested

1. Fix each confirmed defect within the authorized scope.
   Completion criterion: The repair addresses its stated cause.
2. Add a focused regression check for each behavior repair.
   Completion criterion: The check fails for the defect and passes for the repair.
3. Run the applicable documented checks.
   Completion criterion: Each affected check has a current result.
4. Review the final diff and workspace state.
   Completion criterion: Each accepted finding is fixed or has a stated blocker.

For broad or high-risk repairs, obtain an independent review from a clean
context when available. Do not stage, commit, or publish as part of a review
unless the user requests that action.

## Completion criterion

The review accounts for each scoped path, contract, and verification result.
Every finding has reproducible evidence and an actionable remedy. Requested
repairs have direct regression evidence and applicable check results.
