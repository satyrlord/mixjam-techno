---
name: improve-codebase-architecture
description: >
  Improve MixJam Techno architecture and ownership boundaries, or refactor its
  code without a change in behavior. Use for structural friction, broad edit
  cost, a refactor, or an architecture gap from another skill.
---

# Improve MixJam Techno architecture

## 1. Select the branch

1. If the user does not ask for implementation, use analysis mode. Do sections
   2, 3, and 4.
   Completion criterion: The work is read-only, and the proposal that the user
   expects is clear.
2. If the user asks for an architecture change, use implementation mode. Do
   sections 2, 3, 4, and 5.
   Completion criterion: The requested change and its scope are clear.
3. If the user asks for a refactor, or another skill sends a structural change
   that must keep behavior, use refactor mode. Do sections 2 and 6.
   Completion criterion: The preserved behavior is the stated scope.

## 2. Make the system map

1. Read the design or feature documents that apply. Also read the product and
   compatibility authorities that
   [Toolchain and folders](../../../docs/DEVELOPMENT.md#toolchain-and-folders)
   names.
   Completion criterion: You know the approved architecture and product boundary.
2. Read [ARCHITECTURE.md](../../../docs/ARCHITECTURE.md), the architecture pages
   that apply, and the Cargo manifests that apply.
   Completion criterion: You know the `Session` API, the native UI, the
   composition root, and the crate boundaries.
3. Inspect the source tree, the dependency graph, and the tests that apply.
   Completion criterion: Each suspected seam has direct file and dependency evidence.

## 3. Find friction at seams

1. Check the map for each condition in this list:
   - `Session`, engine, UI, persistence, or composition responsibilities leak
   - a wrapper hides no policy or complexity
   - product-format branches go around their owning import adapter
   - edit, undo, validation, or migration policy has more than one owner
   - audio device state or temporary visual state goes into project persistence
   - render frame count, latency, or allocation assumptions stay implicit
   - egui views own engine objects or leave background work active
   - native schema, catalog, asset, or presentation rules span unrelated modules
   - a local feature needs broad unrelated edits
   - tests do not isolate one owner

   Completion criterion: Every problem has a concrete path, dependency, or
   contract check.

Prefer a change that removes caller dependencies and gives the complexity to
one owner.

## 4. Rank proposals

1. State the current friction and its direct evidence.
   Completion criterion: Another agent can reproduce the structural cost.
2. Name the target owner and the necessary contract change.
   Completion criterion: The new boundary has one accountable owner.
3. Define a before-and-after measure.
   Completion criterion: The measure shows fewer imports, branches, owners, or
   changed files.
4. State the migration effects, the risks, and the objective verifier.
   Completion criterion: The proposal includes all necessary transition work.
5. Rank the proposals by benefit, risk, and prerequisite order.
   Completion criterion: The order has no hidden dependency.

## 5. Implement permitted changes

1. Before a behavior change, update the owning contract.
   Completion criterion: The approved boundary is explicit.
2. Change the structure only as much as the contract requires.
   Completion criterion: The target owner has the intended responsibility.
3. If a focused test or static guard can detect a boundary regression, add it
   for each changed seam.
   Completion criterion: Each changed contract has an objective verifier.
4. Run the checks that apply, with the `run-quality-gate` skill.
   Completion criterion: All affected checks pass, or you report a blocker.

## 6. Refactor without a change in behavior

1. Exclude feature work and unrelated cleanup from the scope.
   Completion criterion: Every planned line serves the stated refactor.
2. Select one before-and-after complexity measure.
   Completion criterion: Imports, branches, duplicate owners, or changed file
   count can show the improvement.
3. For each contract in this list that applies, select a direct test or an
   inspection method:
   - `Session` API, engine, native UI, persistence, and composition ownership
   - typed edits, history, gestures, and immutable frontend views
   - background tasks, cancellation, device resources, and native focus behavior
   - render frame counts, channel routing, sample timing, and export behavior
   - schema versions, migrations, import validation, assets, and user settings
   - namespaced sample identity and original-format compatibility limits

   Completion criterion: Every seam that applies has a direct test or inspection
   method.
4. Before you change a critical seam, add a focused regression test.
   Completion criterion: The test can fail for the behavior at risk.
5. Change the structure only as much as the refactor requires.
   Completion criterion: The code compiles, and the preserved behavior stays testable.
6. Run the narrowest affected checks.
   Completion criterion: The step passes, or you record the exact pre-existing failure.
7. If the next step serves the same refactor, do steps 5 and 6 again.
   Completion criterion: No planned step adds product behavior.
8. Compare the complexity measure with its baseline.
   Completion criterion: The result is lower, and no new owner or branch cancels
   the gain.

For broad reachability cleanup, use `dead-code-audit`. If the product contract
must change, use implementation mode with the user's permission. If
documentation describes the preserved behavior incorrectly, correct it.

## Completion criterion

In analysis mode, the work is complete when the system map has evidence. Each
ranked proposal must also have a measurable result. In implementation mode, the
contracts, source, tests, and verification must also agree. In refactor mode,
behavior and contracts did not change, and every affected check passes or has a
verified pre-existing blocker. The refactor report gives the before-and-after
complexity measure and accounts for every changed line.
