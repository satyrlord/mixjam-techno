# Startup, libraries, and data folders

## Contents

[Runtime data folders and Album
Review](runtime-data.md#runtime-data-folders-and-album-review) has the runtime
catalogs, the data folders, and the Album Review.

## Composition root

`src/main.rs` is the composition root. It resolves the command-line options and
builds the initial library and an empty project before it makes a session. For
the window, it makes `--mix` absolute, so Save replaces that song.
`Session::start_up` starts the library folder scan and opens `--mix` after it,
or at once when no scan runs. A queued song opens only if no project change or
unsaved take in the meantime makes a cancellation necessary. An open error shows
in the window and does not stop startup. A successful folder adoption tells the
frontend to clear the selections and the active drags. The `render` command uses
`Session::headless` and `Session::export` or `Session::export_tracks`. Other
headless CLI commands can use the library services directly. The build compiles
the GUI modules into the native binary. The production Rust library does not
export them.

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
duration reading, but not over matching original audio.

## File replacement and MIX import

`src/atomic_file.rs` owns file replacement. It stages projects, settings, the
scan cache, synth patterns, and WAV exports beside the destination and renames
them over it. If a single-file write fails, the current file stays intact.

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
`decode`, and the one header parser, `parse_header`. These callers use them:

- The archive records and stand-alone files of the library.
- The PXD sources of the MIX importer.
- The source file of the Step synth.

`DecodeOptions` names the two intended differences:

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
