# Startup, libraries, and data folders

## Composition root

`src/main.rs` is the composition root. It resolves command-line options and builds
the initial library/project before it creates a session. Normal GUI startup then
scans the library folders through `Session`; a queued song opens only if no intervening
project change or unsaved take requires its cancellation. Successful folder
adoption tells the frontend to clear selections and active drags. Headless CLI
commands can use library services directly. The GUI modules are compiled into the native
binary, not exported by the production Rust library.

## Library folder scans

A scan lists up to 9 library folders, probes their WAVs with one progress
total, and merges them into one `Library`. A folder that cannot be listed, or
has no readable WAV, goes to `Library::unloaded_roots` and the others load; the
scan fails only when none loads. Below a folder, an unreadable subfolder or
entry is skipped and reported in the skipped-WAV list, and the rest of the
folder loads; an unreadable selected root still fails. `sample-folder.json`
keeps the ordered folder list, and reads the single `root` that earlier
versions wrote. A scan drops the
cache entries of files that its folders no longer list. Imported sample IDs
start at 1,000,000, above the 800,000 native IDs, and grow with the library.
Folder scans stream validated WAV samples into the catalog identity hash. Each
worker retains a reader buffer, not the decoded audio. File lists, metadata, and
the probe cache still grow with the corpus. `wav.rs` owns RIFF chunk traversal,
format validation, and buffered decoding for `audio::visit_wav_samples` and
the optional provider's WAV index. It supports mono/stereo PCM8/16/24/32,
float32, and their extensible subformats. Classic PCM24 in four-byte containers
retains its existing right-aligned decoding; extensible PCM uses its declared
container width with valid bits left-aligned. The reader validates the full
subformat GUID, extension length, frame size, byte rate, and required chunk
bounds. It discards a partial final audio frame but rejects a file missing a
whole frame. A scan writes the cache only after
new probes change it. Cached probes carry a decoder version; older probes are
decoded again so their audio identities agree with playback. `source_tempo.rs`
resolves folder tempo from matching
original audio before the duration-based fallback is used. A pack's `BPM.INF`
and tempo tokens in file names are stated tempos. They override the duration
reading but not matching original audio. Optional supplemental tempo identities
come from `source-tempos.json` in the runtime catalog folder.

## File replacement and MIX import

`src/atomic_file.rs` owns file replacement. Projects, settings, the scan cache,
synth patterns, and WAV exports are staged beside the destination and renamed
over it. A failed single-file write leaves the existing file intact. Stem and
sequence batches reject duplicate or non-file destinations, stage all files, and keep
the originals as sibling backups until every replacement succeeds. A commit
failure restores the originals; if restoration fails, the error names the
retained backups for recovery. A process interruption can leave a partial batch
and backups. The batch is not an atomic filesystem transaction.

MIX import caches resolved paths and audio metadata. Only the pending asset list
owns retained audio bytes, and distinct payloads share its 500 MB limit. Saving
a song reads the previous document header through a bounded stream to preserve
its creation time. The completed serialized song must fit the reader's 1 GiB
limit before it can replace the destination.

## Packed samples and size limits

`crates/legacy-format/src/packed.rs` owns the one packed-sample decoder,
`decode`, and the one header parser, `parse_header`. The library's archive
records and stand-alone files, the MIX importer's PXD sources, and the Step
synth's source file all call them. `DecodeOptions` names the two intended
differences. An archive record (`ARCHIVE`) starts the predictor at zero and
pads the audio with silence to the header's count. A stand-alone file (`FILE`)
starts from the header word, with the loader's first sample, and ends at the
last complete opcode. Both validate every opcode: the reserved opcodes and
dictionary slot 0 are errors. Decoding allocates only the audio that the
stream produces, and the padded tail of an archive record. An archive read
compares the header's count, and then the decoded length, with the catalog's
frame count. `MAX_PACKED_BYTES` and `MAX_DECODED_SAMPLES` in that file are the
only packed-sample limits.

Audio has no fixed duration limit. What bounds it is the format that holds it:
`MAX_WAV_SAMPLES` (`audio/pcm.rs`) is what a 16-bit WAV holds, and
`MAX_WAV_FILE_BYTES` (`wav.rs`) is the longest WAV file. The reader, the
library scan, the retained-audio copies, and the importer read a WAV as long as
that; they are bounded by the file itself. Saving or exporting refuses only
audio that no WAV can hold. A recording keeps its take in chunks, so a long
take never copies itself, and ends at `MAX_WAV_SAMPLES`. The other bounds
guard against a damaged or foreign file, and each is a named constant with
its reason: `MAX_PACKED_BYTES`, `MAX_DECODED_SAMPLES`, `MIX_BYTE_LIMIT`,
`MAX_PROJECT_BYTES`, `MAX_SONG_BYTES`, `MAX_INDEX_RECORDS`,
`MAX_CATALOG_TEXT_BYTES`, `MAX_EMBEDDED_WAVES`, `MAX_FOLDER_DEPTH`,
`MAX_BAR_FRAMES`, and `TAIL_LIMIT`. A new bound belongs in one of those two
groups, with its reason, or it does not belong.

## Runtime data folders and Album Review

Some application data lives beside the executable and is read at run time, not
compiled in. `src/runtime_data.rs` finds such a folder, `assets/<name>`, in the
executable's folder or up to four folders above it: the repository root for a
build in `target/release` or `target/debug/deps`, or the installation folder.

`src/catalog/releases.rs` reads the sample identity catalogs,
`assets/catalog/*.json`. Each file describes one release of one original
product (format 3): its product, a release name, the size and hash of each
original source file, a fingerprint of those files, and the identity of each
sample. A catalog holds no original names or paths: a sound's name is its
`label_key`, a one-way digest that still matches a WAV named after the sound.
`catalog::catalogs()` reads the folder on first use; `catalog::refresh()` reads
it again when a JSON file was added, changed or removed. `Session` calls it
before each sample folder scan. Before the window opens a song,
`Session::refresh_catalogs` calls it, and a changed folder rescans the sample
folders and opens the song after the scan. Each scan uses one catalog reading
for both native identities and authenticated source tempos. A change during a
scan restarts it with its current folders and fallback tempo. Cancellation or
failure keeps the rescan pending. A deferred open protects edits made after it
was queued, while accepting unsaved changes already approved for replacement.
Each reading stays valid for the rest
of the process, so identities already handed out never dangle. Releases of
one product merge into one identity list when they agree on audio and timing
for every native ID they share. The loader retains alternative label keys and
the strongest available audio digest. A release that gives a shared ID different
audio, tempo, or bar count, an invalid file, or
a second copy of a loaded release does not load and is listed in
`Catalogs::problems`, which startup reports. `project_io::open_song` adds a
source notice for each product whose catalogs lack some of a `.mix` song's
native IDs, unless an original installation serves that product.
The repository carries the generated catalogs in `assets/catalog`. A build or
installation without them cannot identify native WAVs from those catalogs and
retains unresolved sources as placeholders.

`src/album_review.rs` owns the Album Review: `Shelf` reads
`assets/album-review/<id>.json`, sorted by id, and lists each file that does
not load with its reason; a missing folder is an explicit state.
`album_review/review.rs` defines the saved form (`Review`, `Track`, `Event`,
`Energy`, `Kind`) and the implemented validation rules. `Shelf::add` and
`Shelf::update` check those rules before `atomic_file` writes the file. They do
not check the accuracy of factual or listening claims. New reviews use exclusive publication
and cannot replace an existing file, even when that file did not load.
`Query` and `Shelf::tracks` answer the agent
filters (artist, tag, energy, kind). `album_review/collection.rs` owns the
grid's rules: accent-insensitive search with `deunicode`, artist union, genre
intersection with spelling variants joined, the genre counts, and the five
sorts with their tie-breaks. `album_review/covers.rs` owns the cover cache in
the data folder (`album-review/covers/<id>.<ext>`, with a 256 MB target, least
recently viewed first, with the view time kept as the file's modification
time). Trimming is best effort: files that cannot be removed stay until a
later store can remove them, and a newly stored cover remains usable. In the
GUI build it also owns the download: `ureq` with rustls, http and https
only, a 20-second timeout, at most 20 MB, and only a JPEG, PNG, or WebP image,
with the user agent `MixJamTechno/<version>`. `CoverLoader` runs reads,
downloads, and decoding on a background thread and the window polls it.
Downloaded images must decode before they enter the cache; an explicit
download can replace a corrupt cached image. Cache clearing waits until all
pending cover results have been consumed. Cover download is the
application's only network access.

`src/ui/album_review.rs` is the window. It keeps the filter, the sort, the
page, and the cover textures, and reaches the reviews only through
`application::album_review`. `agent::album_reply` answers the review requests
from a `Shelf`, so they work headless and without the window.
