---
name: run-quality-gate
description: >
  Run or repair MixJam Techno's documented Rust and native-artifact checks.
  Use for repository checks, build failures, or release readiness.
---

# Run the MixJam Techno quality gate

The repository documents own the check commands. This skill owns the order,
the status of each check, and the report. Run each command exactly as its
owner writes it, through `tools/runtime/cargo.ps1` for Cargo. Keep the exact
command results.

## 1. Select the branch

1. If the user asks for results only, use verify mode.
   Completion criterion: You do not change source, tests, or configuration.
2. If the user asks for fixes, use repair mode.
   Completion criterion: The permitted repair scope is explicit.

Do not add suppressions, exclusions, disabled rules, or lower thresholds without
explicit approval.

## 2. Find the current gate

1. Read `docs/ARCHITECTURE.md`, `docs/DEVELOPMENT.md`, the Cargo manifests, and
   the affected source or tests. If they exist, also read `AGENTS.md`,
   `local-data/research/PRODUCT.md`, and `local-data/research/COMPATIBILITY.md`.
   Completion criterion: You know the current contracts and the documented checks.
2. Read `tools/runtime/cargo.ps1` and each verifier that applies before you run
   it.
   Completion criterion: You know the toolchain, the inputs, and the side effects.
3. Read the Applicability section of [REFERENCE.md](REFERENCE.md).
   Completion criterion: You know the phase rule for the scope.
4. Take a worktree baseline. Mark each documented check as applicable, blocked,
   or not available.
   Completion criterion: Every check has one current status.
5. Compare the package version in `Cargo.toml` with the version that `cargo.ps1`
   writes from the current commit count. In verify mode, if they differ, run
   the `cargo.ps1` checks in an isolated checkout with the same changes. If no
   isolated checkout is available, report those checks as blocked.
   Completion criterion: Verify mode does not rewrite the user's manifests.

If a check is necessary but has no documented procedure, report the gap. Do not
invent a pass.

## 3. Run the checks that apply

1. Run each command in the [Checks](../../../docs/DEVELOPMENT.md#checks) section
   of `docs/DEVELOPMENT.md`. Run the release build only when the built
   executable matters.
   Completion criterion: Each command has a current result, or a reason that it
   does not apply.
2. For native font assets, run the font check in
   [safe-frontend-iteration.md](../../../docs/architecture/safe-frontend-iteration.md).
   Completion criterion: The font check has a current result.
3. For visual assets, run the raster check in the
   [Visual assets](../../../docs/DEVELOPMENT.md#visual-assets) section of `docs/DEVELOPMENT.md`.
   Completion criterion: No tracked or non-ignored file contains an SVG asset.
4. For visual assets, run the `--check` command that the `README.md` of each
   affected `assets/` folder names.
   Completion criterion: Each prepared asset matches, or the report names the
   missing private input.
5. For UI changes, run the capture procedure in the
   [UI verification](../../../docs/DEVELOPMENT.md#ui-verification) section of
   `docs/DEVELOPMENT.md` against the release build.
   Completion criterion: The capture procedure has a current result.
6. For UI changes, inspect the captures at each size in that section.
   Completion criterion: Each capture has a recorded visual result.
7. For import, project, render, or audio changes, run the focused tests that apply.
   Completion criterion: Each focused test has a current result.
8. For those changes, run built-artifact scenarios with representative files.
   Completion criterion: Each scenario has a current result.
9. Record device behavior, listening, and clean-machine checks separately.
   Completion criterion: No check takes its result from a different check.

In verify mode, after a failure, run the independent checks that are safe.

In repair mode, read the Stop conditions section of [REFERENCE.md](REFERENCE.md)
before each fix. Before you edit, record the exact diagnostic. Fix the smallest
root cause. Then run the same check again before you continue.

Completion criterion: Every check that applies has a current result.

## 4. Report the gate

1. Report each command or documented procedure, its status, and short evidence.
   Completion criterion: Each check is `pass`, `fail`, `blocked`, or `not available`.
2. Keep pre-existing failures separate from failures that the scoped change causes.
   Completion criterion: Each attribution has repository evidence.
3. Claim an overall pass only if every necessary check that applies passes.
   Completion criterion: The overall status follows from the individual results.

Do not invent a coverage percentage or change the meaning of a configured
threshold. Native captures do not prove listening, microphone, file-picker, or
screen-reader behavior. Do not report 150% or 200% display scaling as not
verified, because it is out of scope.

## Completion criterion

Verify mode is complete when all safe checks that apply have a current status.
Repair mode is complete when you fix and check again each root cause in scope.
State every unavailable check, blocker, and runtime area that is not verified.
A focused check does not prove an overall pass.
