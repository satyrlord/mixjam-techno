# Dead-code examples

## Build asset

An asset can be live through `build.rs` or a Rust include macro. Trace the source,
generated output, and runtime consumer.

## Feature gate

Code can be live only with the `gui` feature or only in the headless build. Check
both feature configurations before you remove it.

## Migration

A migration can be absent from the normal save path. It remains live when the
import contract supports its older project version.

## Preservation

Snapshots, manifests, and Git bundles are retained by contract even when the
application does not load them. Do not classify them as dead from Rust references.
