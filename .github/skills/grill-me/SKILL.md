---
name: grill-me
description: >
  Challenge a MixJam Techno plan or design before implementation. Use to resolve open
  product, architecture, persistence, audio, UI, verification, or delivery decisions.
---

# Challenge a MixJam Techno decision

If the user does not permit workspace edits, stay read-only.

## 1. Prepare the decision tree

1. Read `docs/ARCHITECTURE.md`, the architecture pages that apply, and the
   design or feature document that applies. Also read the product and
   compatibility authorities that
   [Toolchain and folders](../../../docs/DEVELOPMENT.md#toolchain-and-folders)
   names.
   Completion criterion: You know the approved contract and the open questions.
2. Inspect the implementation that the plan claims already exists.
   Completion criterion: Each implementation claim has direct evidence.
3. Answer the factual questions that current repository files or primary sources
   can answer.
   Completion criterion: Only real product choices remain.
4. List the decision branches from parent to child.
   Completion criterion: The branch with the highest risk and the least
   reversibility is first.

Do not ask a question that available evidence can answer.

## 2. Resolve one branch at a time

1. State the current branch and its material trade-offs.
   Completion criterion: The user can compare the valid choices.
2. Recommend one choice that evidence supports.
   Completion criterion: The recommendation has a clear reason and a verifier.
3. Ask one question.
   Completion criterion: The user gives an answer or explicitly defers the branch.
4. Record the answer and its effects on child branches.
   Completion criterion: No child branch depends on an unstated parent choice.
5. If the answer shows a material dependency, add a branch.
   Completion criterion: The tree contains no speculative branch.

Examine the product and delivery risks that affect the decision: ownership,
persistence, audio, accessibility, native layout, original-product parity, and
verification.

If you need a concrete question sequence, read [EXAMPLES.md](EXAMPLES.md).

## 3. Record permitted decisions

1. Identify the specification or domain document that owns each decision.
   Completion criterion: Every durable fact has one owner.
2. If the user permits edits, update only that document.
   Completion criterion: No parallel decision document exists.
3. If the user does not permit edits, report the exact document change for later.
   Completion criterion: Another agent can apply it without a new discussion of
   the decision.

## Completion criterion

The session is complete when you resolve or defer every branch in the tree.
Each deferral names an owner, a reason, and a verification step. After you
record the decisions, send permitted implementation work to the skill that owns
it.
