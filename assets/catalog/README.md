# Standalone WAV sample identities

This folder holds the sample identity catalogs: one JSON file per release of an
original product, `<product>-<release>.json`. The files hold 31,620 factual
sample identities from the original catalogs and source headers for the eight
products. The application reads the folder when it starts. It reads the folder
again before a sample folder scan or a song import if a catalog changed. The
executable does not contain these files. Keep the folder at `assets/catalog`
beside the application (the repository root for a build). End users do not need
the source catalogs or original programs.

A catalog holds numbers, digests, archive codes and generic categories only.
It holds no audio, no executable code, no original sound names, and no original
file names or paths. A research machine generates the JSON files. They are part
of the repository.

If a product has no catalog, WAV folders still load as ordinary samples. The
native song import of that product cannot resolve sample identities, and it
names the missing catalog.

Each catalog file (format 3) has these fields:

- `schema`
- `product`
- `release`: a name of 1 to 64 characters that you give when you generate it
- `sources`: the size and SHA-256 of each original file that the generator read
- `fingerprint`: `catalog::fingerprint` of the sources
- `samples`

A file with an unknown or repeated field does not load. Each sample has
`native_id`, `code` (its archive code), `label_key`, `group_name` (its
category), `bars`, `source_bpm`, `frame_count` and `pcm_digest`. The
`pcm_digest` field is optional. Catalogs of several releases of one product
merge if they agree on every shared native ID. A catalog that conflicts does
not load. Format 2 files, which held names and paths, do not load.

To regenerate the catalogs, use a research machine with configured original
providers. Put each installation in `local-data/installations`, or set
`MIXJAM_TECHNO_MEDIA_ROOT` to the folder that holds the setup folders. Then
run:

```powershell
tools/runtime/cargo.ps1 -CargoArguments @('run','--release','--no-default-features','--example','export_catalog')
```

`export_catalog --release NAME PRODUCT=ROOT` catalogs another release from its
installation folder, for example `--release cd d1=E:\path\to\install`. The
generator also writes the sound names privately to `local-data/catalog-names/`
for the song tools. Those files never ship.

Each identity can carry `pcm_digest`. It is the first 128 bits of SHA-256 over
the first differences of the declared-length mono PCM16 of the original, without
the final frame (`catalog::pcm_identity`). For a stand-alone packed file with a
metadata header, the declared frames start after the pending initial sample.
They are zero-padded like the cleared buffer of the original. A WAV whose audio
has that digest is the sample, whatever its filename or folder. A constant
offset (a decoder that starts from a different predictor) or a differently
padded final frame leaves the digest unchanged. Any other difference fails the
match. If identical audio is under several identities, it serves each of them.
The match pairs files in path order.

`label_key` is the first 64 bits of SHA-256 over the compact form of the name
that the original gives the sound. The compact form keeps lower-case letters and
digits and drops the leading zeros of numbers (`catalog::label_key`). It
recognizes a WAV that has the name of the sound. The catalog does not hold the
name. An identified WAV keeps the name that its own file gives it. A missing
sound shows a name such as "D1 loop 731".

If the digest does not match, WAV identity matching uses a native code
filename, or a label key and category, plus the measured audio duration. The
duration comparison allows four frames at 44.1 kHz for complete runs. This is a
matching policy, not proof of identical audio. The library never assigns a WAV
to an identity if the WAV has the label and duration of that identity but a
different digest. It reports the WAV instead. Arbitrary WAVs remain available
as ordinary samples.

A native code filename takes precedence over a label and over a folder hint. If
a label maps one WAV to several native identities, the WAV stays unresolved. A
folder named for one product, or a known extracted folder name, limits label
matching to that product. Without that folder context, cross-product collisions
stay unresolved. The app keeps unresolved files available as ordinary WAVs. If
several files match the same native identity by name, that identity stays
unresolved.

Original catalog bar counts and tempos apply to identified native samples.
For other WAVs, the application uses authenticated folder evidence, stated pack
or filename tempos, or loop-length estimates. It uses the selected fallback when
those sources do not give a tempo. A song that needs a missing or unresolved
native identity cannot use a different sound in its place.
