---
name: refactor
description: >
  Refactor MixJam Techno without a change in behavior. Use for a structural change
  that the user permits, or for a behavior-preserving change that another skill sends here.
---

# Refactor MixJam Techno

## 1. Set the invariant

1. Read `docs/ARCHITECTURE.md`, the relevant source, and its tests. If they
   exist, also read `AGENTS.md`, `local-data/research/PRODUCT.md`, and
   `local-data/research/COMPATIBILITY.md`.
   Completion criterion: The preserved behavior and the affected seams are explicit.
2. Exclude feature work and unrelated cleanup from the scope.
   Completion criterion: Every planned line serves the stated refactor.
3. Select one before-and-after complexity measure.
   Completion criterion: Imports, branches, duplicate owners, or changed file count can show the improvement.
4. Before you change a critical seam, add a focused regression test.
   Completion criterion: The test can fail for the behavior at risk.

## 2. Protect the seams

1. For each contract in this list that applies, select a direct test or an inspection method:
   - `Session` API, engine, native UI, persistence, and composition ownership
   - typed edits, history, gestures, and immutable frontend views
   - background tasks, cancellation, device resources, and native focus behavior
   - render frame counts, channel routing, sample timing, and export behavior
   - schema versions, migrations, import validation, assets, and user settings
   - namespaced sample identity and original-format compatibility limits

   Completion criterion: Every seam that applies has a direct test or inspection method.

## 3. Refactor in small steps

1. Change the structure only as much as the refactor requires.
   Completion criterion: The code compiles, and the preserved behavior stays testable.
2. Run the narrowest affected checks.
   Completion criterion: The step passes, or you record the exact pre-existing failure.
3. If the next step serves the same refactor, do steps 1 and 2 again.
   Completion criterion: No planned step adds product behavior.
4. Compare the complexity measure with its baseline.
   Completion criterion: The result is lower, and no new owner or branch cancels the gain.

For broad reachability cleanup, use `dead-code-audit`. If the product contract
must change, send that work to an architecture or feature task that the user
permits. If documentation describes the preserved behavior incorrectly, correct it.

## Completion criterion

The refactor is complete when you confirm that behavior and contracts did not
change. Every affected check passes, or you report a verified pre-existing
blocker. The report gives the before-and-after complexity measure and accounts
for every changed line.
