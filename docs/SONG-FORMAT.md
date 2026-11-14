# Song format

A `.mixjam` song is JSON. Its `formatVersion` selects the reader. Version 24 is
the current song format, and it is the only version that the application
writes. Versions 8 to 23 open and migrate. Version 7 is the earlier MixJam
project format, which is import-only: **Save** never replaces a version-7 file.
`src/mixjam/song.rs` owns the format; its tests cover each version.

## Contents

- [Envelope](#envelope)
- [Song and lanes](song-format/song-and-lanes.md#song-and-lanes)
- [Master pedalboard](song-format/master-fx-and-placements.md#master-pedalboard)
- [FX buses](song-format/master-fx-and-placements.md#fx-buses)
- [Placements and samples](song-format/master-fx-and-placements.md#placements-and-samples)
- [Version history](song-format/version-history.md#version-history)

## Envelope

Every version from 8 uses strict camelCase JSON and rejects unknown keys. A
document holds `formatVersion`, `appVersion`, `createdAt`, `modifiedAt`, `song`,
`lanes`, `samples`, `missingSamples`, `sampleConflicts`, `importNotices`,
`sourceNotices`, and `libraries`. From version 11 it also holds `master`, and
from version 13 `fxBuses`. `sourceSettings` holds, as text, the settings that
the original product saved with an imported song; a song without such settings
omits it. Every other key is required. Saving to an existing file of version 8
to 24 keeps its `createdAt`.
Master settings reject duplicate fields, including nested fields and escaped
spellings of the same key. Versions 11 to 15 perform this check before renaming
the earlier Booster keys during migration.

Ticks are 256 per bar. A song links each WAV file by path; it is not a
portable audio bundle.
