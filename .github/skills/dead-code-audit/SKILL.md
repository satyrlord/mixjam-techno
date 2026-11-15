---
name: dead-code-audit
description: >
  Audit MixJam Techno reachability and remove proven dead items. Use for code, assets,
  dependencies, false positives, or reachability proof requested by another skill.
---

# Audit dead code

## 1. Select the branch

1. If the user asks for findings only, use audit mode.
   Completion criterion: The scope is read-only and explicit.
2. If the user asks for removal, use cleanup mode.
   Completion criterion: The removal scope and the verification authority are explicit.

## 2. Make the graph

1. Read `docs/ARCHITECTURE.md`, the Cargo manifests, `build.rs`, and the
   contracts that apply. If they exist, also read `AGENTS.md`,
   `local-data/research/PRODUCT.md`, and `local-data/research/COMPATIBILITY.md`.
   Completion criterion: You know all configured graph tools and protected paths.
2. List each executable, library, feature, test, build script, CLI command,
   runtime catalog, runtime asset, and preservation entry.
   Completion criterion: Every configured entry point has an owner.
3. Run the build and test commands in the
   [Checks](../../../docs/DEVELOPMENT.md#checks) section of `docs/DEVELOPMENT.md`
   that apply to the scope.
   Completion criterion: Each compiler check that applies has a recorded result.
4. Run the Clippy command in that section.
   Completion criterion: The Clippy check has a recorded result.
5. Search the configured entry points and references for candidates.
   Completion criterion: Each search that applies has a recorded result.
6. Make a candidate inventory from the tool output and the repository searches.
   Completion criterion: Every candidate has a stable path or dependency name.

Do not invent entry points or planned tools. Current files and documented
commands must prove that they exist.

## 3. Prove each candidate

1. For every candidate, check each reachability path in this list:
   - Rust modules, feature gates, trait calls, macros, and public APIs
   - CLI dispatch, `Session` operations, serialization, migrations, and import profiles
   - runtime catalogs, `build.rs`, embedded font and rail assets, and runtime paths
   - test, fixture, optional oracle, build, and documentation paths
   - preservation snapshots, manifests, Git bundles, and approved research

   Completion criterion: Each path in the list has a result for each candidate.
2. If a dynamic path can cause a false positive, read [EXAMPLES.md](EXAMPLES.md).
   Completion criterion: Each dynamic path has a trace from source to consumer.
3. Before you classify a candidate as dead, read [REFERENCE.md](REFERENCE.md).
   Completion criterion: The candidate meets deletion conditions 1 to 4 in REFERENCE.md.
4. Classify each candidate as live, dead, or unresolved.
   Completion criterion: The recorded evidence excludes the other classifications.

## 4. Act within scope

1. In audit mode, report each classification. Do not edit.
   Completion criterion: Every inventoried candidate is in the report.
2. In cleanup mode, remove the smallest proven dead slice.
   Completion criterion: No live contract or reference depends on the removed slice.
3. In cleanup mode, run the narrowest affected compile, Clippy, test, and build
   checks again.
   Completion criterion: All checks that apply pass, or you report a blocker.

Do not delete approved research because production code does not reach it.
Static analysis alone does not permit a deletion.

## Completion criterion

Each candidate is live, false positive, removed, or unresolved. For each item,
the report gives the tool result, the local proof, the action, and the verifier
after the action. If a necessary tool or dynamic path is not available, do not
claim a clean audit.
