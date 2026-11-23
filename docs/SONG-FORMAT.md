# Song format

A `.mixjam` song is JSON. Its `formatVersion` selects the reader. Version 29 is
the current song format. The application writes only this version. Versions 8
to 28 open and migrate. Version 7 is the earlier MixJam project format. It is
import-only: **Save** never replaces a version-7 file. `src/mixjam/song.rs`
owns the format.

## Contents

- [Envelope](#envelope)
- [Song and lanes](song-format/song-and-lanes.md#song-and-lanes)
- [Lane processing keys](song-format/lane-processing.md#lane-processing-keys)
- [Master pedalboard](song-format/master-fx-and-placements.md#master-pedalboard)
- [FX buses](song-format/master-fx-and-placements.md#fx-buses)
- [Placements and samples](song-format/master-fx-and-placements.md#placements-and-samples)
- [Version history](song-format/version-history.md#version-history)

## Envelope

Every version from 8 uses strict camelCase JSON and rejects unknown keys. A
document holds these keys:

- `formatVersion`, `appVersion`, `createdAt`, and `modifiedAt`
- `song`, `lanes`, and `samples`
- `missingSamples`, `sampleConflicts`, `importNotices`, and `sourceNotices`
- `libraries`

From version 11, a document also holds `master`. From version 13, it also holds
`fxBuses`. A song of the version that adds a key must hold that key.

`sourceSettings` holds, as text, the settings that the original product saved
with an imported song. A song without such settings omits the key.

A save to an existing file of version 8 to 29 keeps the `createdAt` of that
file. The reader rejects a file larger than 1 GiB.

Master settings reject duplicate fields. This includes nested fields and
escaped spellings of the same key. Versions 11 to 15 make this check before
they rename the earlier Booster keys during migration.

Ticks are 256 per bar. A song links each WAV file by path. It is not a
portable audio bundle.
