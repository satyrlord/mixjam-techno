# Dead-code evidence

If all these conditions are true, delete a candidate:

1. No production, test, build, CLI, serialization, migration, asset, or documentation path needs it.
2. No feature gate, generated include, manifest, or string-based path reaches it.
3. No approved contract keeps it for a later phase.
4. Removal leaves no invalid reference or public contract.
5. The narrowest affected checks pass after removal.

Common false positives include these items:

- Rust modules used only with `--no-default-features` or the `gui` feature
- font and rail assets embedded at build time
- runtime data that the application reads from `assets/`: the sample catalogs
  in `assets/catalog` and the reviews in `assets/album-review`
- CLI commands reached through string dispatch
- migrations used only by supported older projects
- optional fixture and oracle tests
- preservation archives and approved non-shipping research

For each finding, report the tool result, local proof, action, and post-action
verifier.
