---
name: create-skill
description: >
  Create, change, or audit a repository skill: its triggers, steps, completion
  criteria, references, and writing. Use for a new skill, a skill change, or a skill audit.
---

# Create predictable skills

A skill makes an agent use a predictable process. It does not make the output
the same on each run.

If a term in this skill needs a definition, read [GLOSSARY.md](GLOSSARY.md).

## 1. Set the contract

1. Read the repository instructions and the complete current skill.
   Completion criterion: You know all rules that apply and all existing user changes.
2. Collect a concrete request example for each valid branch.
   Completion criterion: Each branch has a trigger, an output, a boundary, and a
   failure case.
3. Keep the skill model-invoked, unless the user asks for a user-invoked skill only.
   Completion criterion: The invocation policy agrees with the user's request.

## 2. Design the information hierarchy

1. Put the ordered actions in `SKILL.md`.
   Completion criterion: The process has one clear order.
2. End each step with a checkable completion criterion.
   Completion criterion: The agent can tell complete work from incomplete work.
3. Keep the rules that every branch needs in `SKILL.md`.
   Completion criterion: No branch depends on a core rule in a reference file.
4. Move branch-only facts and examples to reference files that `SKILL.md` links
   directly (progressive disclosure).
   Completion criterion: Each context pointer states exactly when to read its target.
5. Give each meaning one owner (single source of truth). If a repository
   document owns a fact, point to that external reference.
   Completion criterion: No instruction, definition, command list, or policy has
   a second owner.
6. If one skill has two unrelated triggers or processes, split it (granularity).
   Completion criterion: Each skill has one action and one process.

If a reference file is long, add a contents list. If a test shows premature
completion of a step, make its criterion more precise. If later steps still
cause it, split the sequence.

## 3. Write the skill

1. Start the description with the action.
   Completion criterion: The description states each distinct trigger once,
   without synonyms.
2. Write each procedure as numbered imperative steps.
   Completion criterion: Each numbered step has one action.
3. Apply the writing policy in
   [deslop/PROSE.md](../deslop/PROSE.md#writing-policy-asd-ste100).
   Completion criterion: The text passes the STE checks in that file.
4. Keep code, identifiers, paths, and command syntax unchanged.
   Completion criterion: Prose edits did not change a technical token.
5. Remove sediment, duplication, sprawl, and no-op instructions.
   Completion criterion: Each remaining sentence changes behavior or supplies
   necessary reference.

## 4. Validate the package

1. Check the folder name, frontmatter, links, and files.
   Completion criterion: `name` is the folder name in hyphen case. The frontmatter
   keys are only `name`, `description`, `license`, `allowed-tools`, and
   `metadata`. The description has no angle brackets. Each relative link resolves.
2. Run the Codex quick validator on each changed skill folder:
   `python <codex-home>/skills/.system/skill-creator/scripts/quick_validate.py <skill-folder>`.
   `<codex-home>` is `CODEX_HOME`, or `.codex` in the user's home folder. The
   validator needs PyYAML.
   Completion criterion: Every run prints `Skill is valid!`. If the validator is
   not installed, the report says so.
3. Test each changed branch with a realistic request or artifact. For a simple
   instruction change, a written walkthrough is sufficient.
   Completion criterion: Each branch follows its process without hidden context.
4. After validation, re-read the complete package.
   Completion criterion: No defect in a criterion, trigger, reference, or STE
   rule remains.

For a forward test of a complex skill or a skill with many changes, use a clean
context.
Give the test agent the request and the raw artifacts. Do not give it the
expected answer or the earlier audit findings.

## Completion criterion

The work is complete when each branch has a clear trigger and result. The
package checks pass, and each remaining instruction has a clear purpose.
