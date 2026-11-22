---
name: ub-mode
description: >
  Apply the reduced engineering principles to every code change. Use before the
  first edit of a code change or when the user runs /ub-mode.
license: MIT
---

# Apply the ub-mode principles

Read this skill before you change code. A code change is a change to source,
tests, build files, or native artifacts. A documentation-only change skips the
code phases. The practice phase applies to every change.

Each code change follows three phases in order:

1. First make it work.
2. Then make it right.
3. Follow practice.

Do not report a change as complete before all three phases pass.

## 1. First make it work

1. Name the outcome that you can check.
   - Aim at the end state.
   - Plan any break in the middle. You must be able to undo it.
   Completion criterion: You can state the result that shows the change
   worked.

2. Attack the premise.
   - When two fixes that share one premise fail the same check, write that
     premise in one sentence.
   - Count which actors hold the imbalance.
   - Remove the assignment.
   - Do not start another fix before you do that.
   Completion criterion: The premise has one sentence, or the repeated failure
   does not exist.

3. Subtract before you add.
   - Take complexity out before you add anything.
   Completion criterion: Each remaining part of the change is necessary.

4. Get the data shape right before you write the logic.
   - Put one decision in one place.
   - Do first whatever helps every later phase.
   - If a concurrent writer is possible, do not share state.
   Completion criterion: The data structure makes the illegal states
   impossible.

5. Write a test that fails before you change the code.
   - See the test fail for the reason that you expect.
   - If an honest test is impractical, do not write a bad test.
   Completion criterion: The test fails on the current code for the expected
   reason.

6. Make the smallest change that makes the test pass.
   - Ship the smallest edit that solves the problem.
   Completion criterion: The new test passes and the existing tests pass.

7. Build the lever when the work is not obvious at a glance.
   - Build the tool that does the work.
   - Do one unit by hand, then make the tool match that unit.
   Completion criterion: The tool reproduces the unit that you did by hand.

8. Use a redesign when a new requirement does not fit the old design.
   - Do not attach the requirement to the old design.
   - Ask which design you build if the requirement is part of the
     first design.
   - Think that through, then deliver it in pieces.
   Completion criterion: The design has the requirement as an original
   requirement.

9. Apply the architecture rules.
   - Model the domain. Put the domain in a structure that makes the illegal
     states impossible. Maintain a glossary of domain terms. If the code is
     clear and local, leave it.
   - Check the data where it enters. Trust the data inside. Keep the business
     logic in a pure function.
   - Make the illegal value impossible to write. Parse outside data once,
     where it comes in. Do not silence the compiler.
   - Make each operation idempotent. A second run, or a run after a crash
     halfway through, ends in the same state.
   - Separate shared state before you serialize it. If two parts do not need
     the same mutable value, give each part its own. If they do, permit one
     writer, and enforce that rule in the structure.
   Completion criterion: Each rule holds, or you record why it does not apply.

## 2. Then make it right

1. Fix the root cause.
   - Reproduce the defect.
   - Fix the cause.
   - A guard that only hides the crash is not a fix.
   Completion criterion: You show the reproduction that no longer fails.

2. Finish one small unit and check it. Then start the next unit.
   Completion criterion: Each unit has a result before the next unit starts.

3. Test behavior, not implementation.
   - The test calls the code as its user does, and checks the result that the
     user sees.
   - If the test passes when each function returns nothing, remove
     the test.
   Completion criterion: Each test can fail when the behavior changes.

4. Measure complexity and duplication.
   - Run `python tools/measure-change.py` from
     [Change measurements](../../../docs/DEVELOPMENT.md#change-measurements).
     Do not run `cargo llvm-cov` under `ub-mode`; `llvm-cov` is allowed only when
     the user explicitly calls
     [run-quality-gate](../run-quality-gate/SKILL.md) for a full gate or
     coverage run.
   - Reduce each changed production function with complexity 10 or more (or CRAP
     score 10 or more when `--lcov` is explicitly ordered). Split the function,
     test it, or do both.
   - Remove each duplication that `tools/measure-change.py` reports.
   - If a tool is not installed, run
     `tools/runtime/install-quality-tools.ps1` or `npm ci`. Do not report a
     result from a tool that did not run.
   Completion criterion: `python tools/measure-change.py` passes with zero
   complexity findings and zero clones.

5. Refactor only through the refactor mode of the
   [improve-codebase-architecture](../improve-codebase-architecture/SKILL.md)
   skill.
   - Pin what the code does.
   - Change the structure.
   - Prove that the behavior did not change.
   Completion criterion: The refactor procedure confirms the preserved
   behavior.

## 3. Follow practice

1. Keep a log of each action and its reason. Report the log when you finish.
   Completion criterion: The final report has the log.

2. Guard the context window.
   - Keep the main thread small.
   - Send the bulk of the work elsewhere, and keep the summary.
   Completion criterion: The main thread keeps a summary of each large read or
   task.

3. Remove temporary work.
   - Find what is safe to remove.
   - Confirm that nothing still in use needs it.
   - Reclaim the disk space.
   - Uncommitted work waits for the user's decision.
   Completion criterion: Each removed item has a check that nothing uses it.

4. Commit and push only when the user asks.
   - Commit often, in small ordered commits.
   - Each commit says why the change happened, what changed, and how you proved
     it.
   - Push at the end.
   Completion criterion: Each commit message has these three items.

## Completion criterion

The change is complete when all three phases pass. The affected tests pass. The
final report gives the log, each measurement, and each check that is not
available.

## License

This skill adapts the [ub-mode](https://github.com/unclebob/ub-mode) skill by
Robert C. Martin. It uses the MIT license. The license text is in
[LICENSE](LICENSE).
