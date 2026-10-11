# Quality-gate reference

## Applicability

`README.md` and `ARCHITECTURE.md` document the current Cargo checks. Use the
repository toolchain wrapper and keep exact command results. For a required
check without a documented procedure, report the gap. Do not invent a pass.

Use these phase rules:

- For documentation work, inspect links, claims, and `git diff --check`.
- For project or compatibility work, test serialization, migrations, sample
  identity, and import/export cases under the supported feature sets.
- For frontend work, run applicable Rust tests, a release build, native captures,
  and visual inspection at 1440x1000, 1024x768, and 960x720.
- For final release evidence, run the documented full Rust checks, built-artifact
  scenarios, and relevant manual native, audio, and compatibility procedures.
- For asset work, run the owning reproducibility checker when available.

## Stop conditions

If the next action needs a product decision, stop repair work. Also stop if the
action changes quality policy, expands scope materially, or lacks a verifier.

Report the exact blocker. Do not invent a coverage percentage or reinterpret a
configured threshold. Native captures do not prove listening, microphone,
file-picker, screen-reader, high-DPI, or original-engine parity behavior.
