# Runtime data folders and Album Review

Some application data is beside the executable, and the application reads it at
run time. The build does not compile it in. `src/runtime_data.rs` finds such a
folder, `assets/<name>`, in the folder of the executable or in up to four
folders above it. That is the repository root for a build in `target/release` or
`target/debug/deps`, or the installation folder.

`src/catalog/releases.rs` reads the sample identity catalogs,
`assets/catalog/*.json`. Each file (format 3) describes one release of one
original product. It holds these items:

- The product and a release name.
- The size and hash of each original source file.
- A fingerprint of those files.
- The identity of each sample.

A catalog holds no original names or paths. The name of a sound is its
`label_key`, a one-way digest that still matches a WAV named after the sound.

`catalog::catalogs()` reads the folder on first use. `catalog::refresh()` reads
it again when a JSON file changes, or when the folder gains or loses one.
`Session` calls it before each sample folder scan. Before the window opens a
song, `Session::refresh_catalogs` calls it. If the folder changed, the session
scans the sample folders again and opens the song after the scan.

Each scan uses one catalog reading for the native identities and for the
authenticated source tempos. A change during a scan starts the scan again, with
its current folders and fallback tempo. A cancellation or a failure keeps the
new scan pending. A deferred open protects the edits that the user makes after
the open enters the queue. It accepts the unsaved changes that the user already
accepted for replacement. A Playback quality Apply does not cancel it, and a
scan that starts again for a catalog change keeps it. If that scan cannot start,
the error names the song. Each reading stays valid for the remainder of the
process. Thus identities that the catalog gave out never dangle.

Releases of one product merge into one identity list when they agree on audio
and timing for each native ID that they share. The loader keeps alternative
label keys and the strongest available audio digest. These files do not load,
and `Catalogs::problems` lists them, which startup reports:

- A release that gives a shared ID different audio, tempo, or bar count.
- An invalid file.
- A second copy of a loaded release.

`project_io::open_song` adds a source notice for each product whose catalogs
do not have some native IDs of a `.mix` song. It adds no notice if an original
installation serves that product. The repository holds the generated catalogs in
`assets/catalog`. A build or installation without them cannot identify native
WAVs from those catalogs, and it keeps unresolved sources as placeholders.

`src/album_review.rs` owns the Album Review:

- `Shelf` reads `assets/album-review/<id>.json`, sorted by id. It lists each
  file that does not load, with its reason. A missing folder is an explicit
  state.
- `album_review/review.rs` defines the saved form (`Review`, `Track`, `Event`,
  `Energy`, `Kind`) and the implemented validation rules. `Shelf::add` and
  `Shelf::update` check those rules before `atomic_file` writes the file. They
  do not check the accuracy of factual or listening claims. New reviews use
  exclusive publication. They cannot replace a file that exists, also when that
  file did not load.
- `Query` and `Shelf::tracks` answer the agent filters (artist, tag, energy,
  kind).
- `album_review/collection.rs` owns the rules of the grid:
  - Accent-insensitive search with `deunicode`.
  - Artist union.
  - Genre intersection, with spelling variants joined.
  - The genre counts.
  - The five sorts with their tie-breaks.
- `album_review/covers.rs` owns the cover cache in the data folder
  (`album-review/covers/<id>.<ext>`).

The cover cache has a 256 MB target. It removes the least recently viewed
covers first, and keeps the view time as the modification time of the file.
The trim is best effort. If the cache cannot remove a file, the file stays
until a later store can remove it. A newly stored cover stays usable. In the
GUI build, `album_review/covers.rs` also owns the download:

- `ureq` with rustls, http and https only.
- A 20-second timeout and a maximum of 20 MB.
- Only a JPEG, PNG, or WebP image.
- A user agent that starts with `MixJamTechno/<version>`.

`CoverLoader` runs reads, downloads, and decodes on a background thread, and the
window polls it. A downloaded image must decode before it goes into the cache.
An explicit download can replace a corrupt cached image. A cache clear waits
until the window has taken all pending cover results. The cover download is the
only network access of the application.

`src/ui/album_review.rs` is the window, with its state, grid, album page, and
chart in `src/ui/album_review/`. It keeps the filter, the sort, the
page, and the cover textures. It gets to the reviews only through
`application::album_review`. `agent::album_reply` answers the review requests
from a `Shelf`. Thus they work headless and without the window.
