# Dead-code evidence

Delete a candidate only if all these conditions are true:

1. No production, test, build, CLI, serialization, migration, asset, or
   documentation path needs it.
2. No feature gate, generated include, manifest, or string-based path reaches it.
3. No approved contract keeps it for a later phase.
4. Removal leaves no invalid reference and does not break a public contract.
5. The narrowest affected checks pass after removal.

These items are common false positives:

- Rust modules that only the `gui` feature or only `--no-default-features` uses
- font and rail assets that the build embeds
- runtime data that the application reads from `assets/`: the sample catalogs
  in `assets/catalog` and the reviews in `assets/album-review`
- CLI commands that string dispatch reaches
- migrations that only supported older projects use
- optional fixture and oracle tests
- preservation archives and approved research that does not ship
