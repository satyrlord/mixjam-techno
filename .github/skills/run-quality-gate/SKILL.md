---
name: run-quality-gate
description: >
  Run or repair MixJam Techno's documented Rust and native-artifact checks.
  Use for repository checks, build failures, or release readiness.
---

# Run the MixJam Techno quality gate

## 1. Select the branch

1. If the user requests results only, use verify mode.
   Completion criterion: You do not change source, tests, or configuration.
2. If the user requests fixes, use repair mode.
   Completion criterion: The authorized repair scope is explicit.

Do not add suppressions, exclusions, disabled rules, or lower thresholds without
explicit approval.

## 2. Discover the current gate

1. Read `docs/ARCHITECTURE.md`, `docs/DEVELOPMENT.md`, Cargo manifests, and
   affected source or tests. If they exist, also read `AGENTS.md`,
   `local-data/research/PRODUCT.md`, and `local-data/research/COMPATIBILITY.md`.
   Completion criterion: You know the current contracts and documented checks.
2. Read `tools/runtime/cargo.ps1` and any applicable verifier before running it.
   Completion criterion: You know the toolchain, inputs, and side effects.
3. Take a worktree baseline and mark each documented check as applicable,
   blocked, or not available.
   Completion criterion: Every requirement has one current status.

Read [REFERENCE.md](REFERENCE.md) for applicability and stop conditions.

## 3. Run applicable checks

Use the current commands in `docs/DEVELOPMENT.md` and `docs/ARCHITECTURE.md`:

1. Run `cargo.ps1 -CargoArguments @('fmt','--all','--','--check')` and
   `python tools/check-rust-format.py`.
   Completion criterion: Both format checks have a current result.
2. Run Clippy with warnings denied.
   Completion criterion: The documented Clippy check has a current result.
3. Run workspace tests with default features.
   Completion criterion: The default test suite has a current result.
4. Run workspace tests with `--no-default-features`.
   Completion criterion: The headless test suite has a current result.
5. Run `cargo.ps1 build --release --locked` when the built executable matters.
   Completion criterion: The release build has a current result or is not applicable.
6. Run `python tools/assets/prepare-fonts.py --check` for native font assets.
   Completion criterion: The font check has a current result.
7. Run `python tools/assets/check-raster-assets.py`,
   `python tools/assets/render-rail-icons.py --check`, and
   `python tools/assets/render-app-icon.py --check` for visual assets. If the
   private masters are in `local-data/artwork/`, also run
   `python tools/assets/render-folded-metal.py --check` and
   `python tools/assets/render-mixer.py --check`.
   Completion criterion: Tracked/non-ignored files contain no SVG assets and PNG variants match.
8. For UI changes, run `tools/verify-ui.ps1` against the release build.
   Completion criterion: The native capture procedure has a current result.
9. For UI changes, inspect the captures at the documented sizes.
   Completion criterion: Each capture has a recorded visual result.
10. For import, project, render, or audio changes, run applicable focused tests.
    Completion criterion: Each focused test has a current result.
11. For those changes, run built-artifact scenarios with representative files.
    Completion criterion: Each scenario has a current result.
12. Record original-engine parity, device behavior, listening, and clean-machine
    checks separately.
    Completion criterion: None of these checks inherit a result from another check.

In verify mode, after a failure, run independent safe checks.

In repair mode, before you edit, capture the exact diagnostic. Fix the smallest
root cause. Before you continue, re-run the same check.

Completion criterion: Every applicable check has a current result.

## 4. Report the gate

1. Report each command or documented procedure, status, and concise evidence.
   Completion criterion: Each check is `pass`, `fail`, `blocked`, or `not available`.
2. Separate pre-existing failures from failures caused by the scoped change.
   Completion criterion: Attribution has repository evidence.
3. If every required applicable check passes, claim an overall pass.
   Completion criterion: The overall status follows the individual results.

## Completion criterion

Verify mode is complete after all safe applicable checks have a current status.
Repair mode is complete after you fix and recheck each in-scope root cause.
State every unavailable gate, blocker, and unverified runtime area. A focused
check does not establish an aggregate pass.
