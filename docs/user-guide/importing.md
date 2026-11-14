# Open songs from other programs

**Open** accepts `.mixjam` songs, MixJam v7 `.mixjam` projects, JSON projects
from earlier builds, and eight legacy `.mix` profiles, named `d1`, `d2`, `d3`,
`d4`, `h1`, `t1`, `t2`, and `t3` after the original products that wrote them.
Import selects the nearest project tempo and shows a notice when the tempo
changes. Songs that use more than 64 tracks are rejected.

Legacy `.mix` files can be up to 500 MB (500,000,000 bytes). Supported embedded
WAVs are imported automatically, so their original external files are not needed.
Songs can also reference factory samples from the selected library. Unavailable
audio appears as silent missing-sample placeholders at the saved or catalog length.
An inferred length is marked when the source duration is unknown. Hover over a
placeholder or open Import details to inspect the original metadata and reason.
Right-click a placeholder and select **Resolve manually** to choose a WAV file.
The choice resolves every identical missing reference across all tracks. Original
positions, spans, and musical trim offsets are retained. Cancel makes no change.
Imported audio is retained in the application's data folder for saved projects.
Keep that folder available when you reopen them. File size alone does not show
whether a song contains embedded audio.

Older app releases also saved version-7 `.mixjam` files with genre-based sample
paths. If the selected folder is named `_original`, the importer can also recover
those references from its containing folder (not a drive root), including moved
category folders.
Exact relative paths take priority. If several files match, the first match loads
and a **Sample conflicts** modal shows the choices. Select another path to update
all instances in the song immediately. Use **Play song** to compare the result,
then **Keep choices**. Esc hides the window without confirming. Reopen the
resolver from **Sample conflicts** in the status bar. This conflict behavior
applies to every supported project format and supports undo/redo and `.mixjam`
save/reopen. Resolved paths survive `.mixjam` save/reopen. An imported v7
`.mixjam`, `.mix`, or JSON file is unchanged: **Save** asks for a new `.mixjam`
file.

`d2` songs can also contain packed samples in the original compressed sample
format. The reader decodes them and retains them as WAV audio. Select the common
parent sample folder when a song uses sounds from earlier products. `d3` songs
can reference `d2` sounds, and `d4` songs `d3` and `d2` sounds; the supported
`t2` compatibility bank uses `d2` IDs 1 through 1957.

A `.mix` song stores numeric sample IDs. The application resolves them with
sample identity catalogs: data files in an `assets/catalog` folder beside the
application (for a build, the repository's `assets/catalog`). The application
reads them when it starts; they are not part of the executable. This
repository contains them in `assets/catalog`. Each catalog file describes one release of one
original product. It names the product and the release, lists the size and hash
of each original file it was generated from, and gives each sample's native ID,
archive code, category, length, tempo, a digest of its audio, and a one-way key
of its name. A catalog holds no sound names, file names, or audio. The
application identifies a WAV when its audio matches, regardless of filename, and
the sound keeps the name of your file.
Digest comparisons require mono audio at the original 44.1 kHz rate, matching
frame counts, and more than two frames. If a comparison is unavailable, the
reader can infer identity from an unambiguous native code or name key, category,
and duration. It rejects a WAV when its available digest differs, even if its
name and length match. The missing-source reason names that WAV.

Catalogs of several releases of one product can be present together. They
merge when the audio, tempo, and bar count agree for every native ID that they
share. The application retains alternate name keys for matching. A catalog with
conflicting audio or timing does not load, and the application names it when it
starts, as it does for an invalid catalog file. `mixjam-techno inspect`
lists the loaded catalogs and the files that did not load.

Without a catalog for a product, WAV folders still load as ordinary sounds.
With WAV-only libraries, native sources that cannot be identified open as
placeholders in imported or saved songs, named by product, category, and ID,
such as "D1 loop 731". Import details name the product that had no catalog at
import. If a song uses native IDs that no loaded catalog of its product lists,
Import details say so once for that product: the song may come from a release
that has no catalog here. Add the catalog file to `assets/catalog` and open the
song again; the application reads the changed folder and scans the sample
folders again before it opens the song, without a restart.

The optional `source-tempos.json` in the same folder adds tempo identities for
two original hip-hop products, whose shipped help states that all their samples
were recorded at 90 BPM. It is generated from the original media by the
owner-local `examples/export_source_tempos.rs` (the `examples/` folder is not
published), with `MIXJAM_TECHNO_MEDIA_ROOT` set. This metadata adds tempo
evidence; it does not add song-format support for those products.
