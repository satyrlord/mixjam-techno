---
name: improve-codebase-architecture
description: >
  Improve MixJam Techno architecture and ownership boundaries. Use for structural
  friction, leaking seams, broad edit cost, or an architecture gap from another skill.
---

# Improve MixJam Techno architecture

## 1. Select the branch

1. If the user does not ask for implementation, use analysis mode.
   Completion criterion: The work is read-only, and the expected proposal is clear.
2. If the user asks for an architecture change, use implementation mode.
   Completion criterion: The requested change and its scope are clear.

## 2. Make the system map

1. Read the design or feature documents that apply. If they exist, also read
   `AGENTS.md`, `local-data/research/PRODUCT.md`, and
   `local-data/research/COMPATIBILITY.md`.
   Completion criterion: You know the approved architecture and product boundary.
2. Read [ARCHITECTURE.md](../../../docs/ARCHITECTURE.md) and the Cargo manifests
   that apply.
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

## Completion criterion

In analysis mode, the work is complete when the system map has evidence. Each
ranked proposal must also have a measurable result. In implementation mode, the
contracts, source, tests, and verification must also agree.
