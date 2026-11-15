# Dead-code examples

## Build asset

An asset can be live through `build.rs` or a Rust include macro. Trace the
source, the generated output, and the runtime consumer.

## Feature gate

Code can be live only with the `gui` feature or only in the headless build.
Check both feature configurations before you remove it.

## Migration

The normal save path can skip a migration. The migration stays live while the
import contract supports its older project version.

## Preservation

The contract keeps snapshots, manifests, and Git bundles, also when the
application does not load them. The absence of a Rust reference does not make
them dead.
