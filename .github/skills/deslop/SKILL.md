---
name: deslop
description: >
  Remove evidence-backed slop from code, prose, data, configuration, and tests.
  Use for a cleanup pass on a file scope, or to make technical prose conform to ASD-STE100.
---

# Remove slop

**Slop** is content that conflicts with file evidence, sibling patterns, or
repository contracts. Judge the artifact. Do not guess who wrote it.

## 1. Inventory the scope

If the user does not set a smaller scope, use first-party source,
documentation, data, configuration, and tests.

1. List every tracked and untracked file in the scope that is not generated.
   Completion criterion: Each file has a family and a representative sibling.
2. List generated, vendored, binary, and excluded paths
   separately.
   Completion criterion: Every excluded path has a reason.
3. Read each scoped file and its sibling completely.
   Completion criterion: You know the local style, behavior, and owner.

## 2. Read the reference for each family

1. For each family in the scope, read its reference:
   - [CODE.md](CODE.md) for source code
   - [PROSE.md](PROSE.md) for documentation, skills, agent definitions, and
     other prose
   - [DATA.md](DATA.md) for data and configuration
   - [TEST.md](TEST.md) for tests

   Completion criterion: You read each reference that a scoped family needs.

If a file has more than one family, read each matching reference. A listed
smell starts an investigation. It does not permit an edit.

## 3. Prove each candidate

1. Compare the candidate with its contract, siblings, and consumers.
   Completion criterion: The difference has or does not have a valid reason.
2. For a test candidate, follow the cleanup process in [TEST.md](TEST.md).
   Completion criterion: The test has the pre-edit result that TEST.md requires.
3. Classify the candidate as valid, slop, or unresolved.
   Completion criterion: Evidence excludes the other classifications.

Do not use syntax, file size, test count, coverage, or tone alone as proof.

## 4. Remove proven slop

1. Edit only enough to remove the proven problem.
   Completion criterion: True behavior, information, constraints, and required
   voice remain.
2. After each related group of edits, run the narrowest repository check that applies.
   Completion criterion: The check passes, or you record the exact pre-existing
   failure.

If the question is reachability, use `dead-code-audit`. Do not remove generated
output, vendored code, lockfiles, or binary assets without an explicit request.

## 5. Re-read and report

1. After all edits, re-read every scoped file.
   Completion criterion: A second complete pass finds no evidence-backed slop.
2. Account for every scoped file as edited or unchanged.
   Completion criterion: The report gives both counts and all exclusions.
3. Report each unresolved candidate and the proof that it needs.
   Completion criterion: Every uncertainty has a concrete verification step.

## Completion criterion

The pass is complete when you account for every scoped file. Every edit has a
repository or sibling reason. All affected checks pass, or you report a
pre-existing blocker.
