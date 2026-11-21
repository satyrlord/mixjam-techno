# Dead-code evidence

Delete a candidate only if all these conditions are true:

1. No production, test, build, CLI, serialization, migration, asset, or
   documentation path needs it.
2. No feature gate, generated include, manifest, or string-based path reaches it.
3. No approved contract keeps it for a later phase.
4. Removal leaves no invalid reference and does not break a public contract.
5. The narrowest affected checks pass after removal.
