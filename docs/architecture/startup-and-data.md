# Startup, libraries, and data folders

## Composition root

`src/main.rs` is the composition root. It resolves the command-line options and
builds the initial library and project before it makes a session. Normal GUI
startup then scans the library folders through `Session`. A queued song opens
only if no project change or unsaved take in the meantime makes a cancellation
necessary. A successful folder adoption tells the frontend to clear the
selections and the active drags. Headless CLI commands can use the library
services directly. The GUI modules are compiled into the native binary. The
production Rust library does not export them.

## Library folder scans

A scan lists up to 9 library folders, probes their WAVs with one progress
total, and merges them into one `Library`. A folder that cannot be listed, or
that has no readable WAV, goes to `Library::unloaded_roots`, and the other
folders load. The scan fails only when no folder loads. Below a folder, the
scan skips an unreadable subfolder or entry, reports it in the skipped-WAV
list, and loads the remainder of the folder. An unreadable selected root still
fails.

`sample-folder.json` keeps the ordered folder list. The reader also accepts the
single `root` field of older files. A scan drops the cache entries of files
that its folders do not list. Imported sample IDs start at 1,000,000, above the
800,000 native IDs, and increase with the library.

Folder scans stream validated WAV samples into the catalog identity hash. Each
worker keeps a reader buffer, not the decoded audio. The file lists, the
metadata, and the probe cache still increase with the corpus.

`wav.rs` owns the RIFF chunk traversal, the format validation, and the buffered
decode for `audio::visit_wav_samples` and the WAV index of the optional
provider. It supports mono and stereo PCM8, PCM16, PCM24, PCM32, float32, and
their extensible subformats:

- Classic PCM24 in four-byte containers decodes right-aligned.
- Extensible PCM uses its declared container width, with the valid bits
  left-aligned.

The reader validates the full subformat GUID, the extension length, the frame
size, the byte rate, and the necessary chunk bounds. It discards a partial
final audio frame, but it rejects a file that does not have a whole frame.

A scan writes the cache only when new probes change it. Cached probes carry a
decoder version. The scan decodes older probes again, so that their audio
identities agree with playback.

`source_tempo.rs` resolves the folder tempo from matching original audio before
it uses the fallback from the duration. The `BPM.INF` file of a pack and the
tempo tokens in file names are stated tempos. They have priority over the
duration reading, but not over matching original audio. Optional supplemental
tempo identities come from `source-tempos.json` in the runtime catalog folder.

## File replacement and MIX import

`src/atomic_file.rs` owns file replacement. Projects, settings, the scan cache,
synth patterns, and WAV exports are staged beside the destination and renamed
over it. If a single-file write fails, the current file stays intact.

Stem and sequence batches work as follows:

1. They reject duplicate destinations and destinations that are not files.
2. They stage all files.
3. They keep the originals as sibling backups until each replacement succeeds.
4. If a commit fails, they restore the originals. If the restore fails, the
   error names the backups for recovery.

A process interruption can leave a partial batch and backups. The batch is not
an atomic filesystem transaction.

MIX import caches the resolved paths and the audio metadata. Only the pending
asset list owns the kept audio bytes, and different payloads share its 500 MB
limit. To keep the creation time, a song save reads the previous document
header through a bounded stream. The completed serialized song must fit the
1 GiB limit of the reader before it can replace the destination.

## Packed samples and size limits

`crates/legacy-format/src/packed.rs` owns the one packed-sample decoder,
`decode`, and the one header parser, `parse_header`. The archive records and
stand-alone files of the library, the PXD sources of the MIX importer, and the
source file of the Step synth all call them. `DecodeOptions` names the two
intended differences:

- An archive record (`ARCHIVE`) starts the predictor at zero. It pads the audio
  with silence to the count of the header.
- A stand-alone file (`FILE`) starts from the header word, with the first
  sample of the loader. It ends at the last complete opcode.

The two options validate each opcode: the reserved opcodes and dictionary slot
0 are errors. The decode allocates only the audio that the stream makes, and
the padded tail of an archive record. An archive read compares the count of the
header, and then the decoded length, with the frame count of the catalog.
`MAX_PACKED_BYTES` and `MAX_DECODED_SAMPLES` in that file are the only
packed-sample limits.

Audio has no fixed duration limit. The format that holds the audio is its
limit:

- `MAX_WAV_SAMPLES` (`audio/pcm.rs`) is the quantity that a 16-bit WAV holds.
- `MAX_WAV_FILE_BYTES` (`wav.rs`) is the longest WAV file.

The reader, the library scan, the kept audio copies, and the importer read a
WAV of that length. The file itself is their limit. Save and export refuse only
audio that no WAV can hold. A recording keeps its take in chunks, so a long
take never copies itself. A recording ends at `MAX_WAV_SAMPLES`.

The other limits protect against a damaged or foreign file. Each is a named
constant with its reason: `MAX_PACKED_BYTES`, `MAX_DECODED_SAMPLES`,
`MIX_BYTE_LIMIT`, `MAX_PROJECT_BYTES`, `MAX_SONG_BYTES`, `MAX_INDEX_RECORDS`,
`MAX_CATALOG_TEXT_BYTES`, `MAX_EMBEDDED_WAVES`, `MAX_FOLDER_DEPTH`,
`MAX_BAR_FRAMES`, and `TAIL_LIMIT`. Put a new limit in one of these two groups,
with its reason. If it does not fit in a group, do not add it.

## Runtime data folders and Album Review

Some application data is beside the executable, and the application reads it
at run time. It is not compiled in. `src/runtime_data.rs` finds such a folder,
`assets/<name>`, in the folder of the executable or in up to four folders
above it. That is the repository root for a build in `target/release` or
`target/debug/deps`, or the installation folder.

`src/catalog/releases.rs` reads the sample identity catalogs,
`assets/catalog/*.json`. Each file (format 3) describes one release of one
original product: the product, a release name, the size and hash of each
original source file, a fingerprint of those files, and the identity of each
sample. A catalog holds no original names or paths. The name of a sound is its
`label_key`, a one-way digest that still matches a WAV named after the sound.

`catalog::catalogs()` reads the folder on first use. `catalog::refresh()` reads
it again when a JSON file was added, changed, or removed. `Session` calls it
before each sample folder scan. Before the window opens a song,
`Session::refresh_catalogs` calls it. If the folder changed, the session scans
the sample folders again and opens the song after the scan.

Each scan uses one catalog reading for the native identities and for the
authenticated source tempos. A change during a scan starts the scan again, with
its current folders and fallback tempo. A cancellation or a failure keeps the
new scan pending. A deferred open protects the edits that come after the open
was queued. It accepts the unsaved changes that the user already accepted for
replacement. Each reading stays valid for the remainder of the process. Thus
identities that the catalog gave out never dangle.

Releases of one product merge into one identity list when they agree on audio
and timing for each native ID that they share. The loader keeps alternative
label keys and the strongest available audio digest. These files do not load,
and `Catalogs::problems` lists them, which startup reports:

- A release that gives a shared ID different audio, tempo, or bar count.
- An invalid file.
- A second copy of a loaded release.

`project_io::open_song` adds a source notice for each product whose catalogs
do not have some native IDs of a `.mix` song, unless an original installation
serves that product. The repository holds the generated catalogs in
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
- `album_review/collection.rs` owns the rules of the grid: accent-insensitive
  search with `deunicode`, artist union, genre intersection with spelling
  variants joined, the genre counts, and the five sorts with their tie-breaks.
- `album_review/covers.rs` owns the cover cache in the data folder
  (`album-review/covers/<id>.<ext>`).

The cover cache has a 256 MB target. It removes the covers that were viewed
least recently first, and keeps the view time as the modification time of the
file. The trim is best effort: a file that cannot be removed stays until a
later store can remove it, and a newly stored cover stays usable. In the GUI
build, `album_review/covers.rs` also owns the download:

- `ureq` with rustls, http and https only.
- A 20-second timeout and a maximum of 20 MB.
- Only a JPEG, PNG, or WebP image.
- A user agent that starts with `MixJamTechno/<version>`.

`CoverLoader` runs reads, downloads, and decodes on a background thread, and the
window polls it. A downloaded image must decode before it goes into the cache.
An explicit download can replace a corrupt cached image. A cache clear waits
until the window has taken all pending cover results. The cover download is the
only network access of the application.

`src/ui/album_review.rs` is the window. It keeps the filter, the sort, the
page, and the cover textures. It gets to the reviews only through
`application::album_review`. `agent::album_reply` answers the review requests
from a `Shelf`. Thus they work headless and without the window.
