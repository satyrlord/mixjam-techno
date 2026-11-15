# Quality-gate reference

## Applicability

Use the phase rule for the scope:

- For documentation work, inspect links, claims, and `git diff --check`.
- For project or compatibility work, test serialization, migrations, sample
  identity, and import and export cases under the supported feature sets.
- For frontend work, run the Rust tests that apply and a release build. Then
  run the native captures and inspect each capture size.
- For final release evidence, run every check in step 3 of `SKILL.md`. Run the
  asset checks for every folder in `assets/`. Also run the manual native, audio,
  and compatibility procedures that apply.
- For asset work, run the check of each affected asset folder.

## Stop conditions

Stop the repair work if the next action needs a product decision. Also stop if
the action changes the quality policy, makes the scope much larger, or has no
verifier. Report the exact blocker.
