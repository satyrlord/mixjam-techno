# Dead-code examples

These items are common false positives. For each one, trace the candidate from
its source to its consumer.

## Build asset

An asset can be live through `build.rs` or a Rust include macro. Trace the
source, the generated output, and the runtime consumer.

## Feature gate

Code can be live only with the `gui` feature or only in the headless build
(`--no-default-features`). Check both feature configurations before you remove
it.

## Migration

The normal save path can skip a migration. The migration stays live while the
import contract supports its older project version.

## Approved research

The contract keeps approved research and original product evidence, also when
the application does not load them. The absence of a Rust reference does not
make them dead.

## Runtime data

The application reads some data from `assets/` at run time: the sample catalogs
in `assets/catalog` and the reviews in `assets/album-review`. No Rust path
names such a file. Trace the loader that reads it.

## CLI command

String dispatch can reach a command that no direct call names. Trace the
dispatch from the command name to its handler.

## Optional test

An optional fixture or oracle test runs only when its input is present. It is
live also when it does not run on this machine.
