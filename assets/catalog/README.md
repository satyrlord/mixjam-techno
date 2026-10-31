# Standalone WAV sample identities

This folder holds the sample identity catalogs: one JSON file per release of an
original product, `<product>-<release>.json`, with 31,620 factual sample
identities from the original catalogs and source headers for the eight products.
The application reads the folder when it starts, and again when a catalog is
added, changed or removed before a sample folder scan or a song import; the
executable does not contain these files. Keep the folder at `assets/catalog`
beside the application (the repository root for a build). End users do not
need the source catalogs or original programs.

A catalog holds numbers, digests, archive codes and generic categories only.
It holds no audio, no executable code, no original sound names, and no original
file names or paths. The JSON files are generated on a research machine and
are part of the repository.

Without a product's catalog, WAV folders still load as ordinary samples, and
that product's native song import cannot resolve sample identities; the import
names the missing catalog.

Each catalog file (format 3) has `schema`, `product`, `release` (a name given
when it is generated), `sources` (the size and SHA-256 of each original file it
was generated from), `fingerprint` (`catalog::fingerprint` of the sources), and
`samples`. Each sample has `native_id`, `code` (its archive code), `label_key`,
`group_name` (its category), `bars`, `source_bpm`, `frame_count` and
`pcm_digest`. Catalogs of several releases of one product merge when they agree
on every shared native ID; a conflicting one does not load. Format 2 files,
which held names and paths, do not load. `source-tempos.json` is not a
catalog: it adds tempo identities by audio digest.

Regenerate the catalogs on a research machine with configured original providers
(set `MIXJAM_TECHNO_MEDIA_ROOT`; see the original media notes):

```powershell
tools/runtime/cargo.ps1 -CargoArguments @('run','--release','--no-default-features','--example','export_catalog')
```

`export_catalog --release NAME PRODUCT=ROOT` catalogs another release from its
installation folder, for example `--release cd d1=E:\path\to\install`. The
generator also writes the sound names privately to `local-data/catalog-names/`
for the song tools; those files never ship.

Each identity carries `pcm_digest`: the first 128 bits of SHA-256 over the first
differences of the original's declared-length mono PCM16, omitting the final
frame (`catalog::pcm_identity`). For a stand-alone packed file with a metadata
header the declared frames start after the pending initial sample, zero-padded
like the original's cleared buffer. A WAV whose audio has that digest is the
sample, whatever its filename or folder. A constant offset (a decoder that
starts from a different predictor) or a differently padded final frame leaves
the digest unchanged; any other difference fails it. Identical audio under
several identities serves each of them, pairing files in path order.

`label_key` is the first 64 bits of SHA-256 over the compact form of the
original's name for the sound (lower-case letters and digits, leading zeros of
numbers dropped; `catalog::label_key`). It recognizes a WAV named after the
sound without the catalog holding the name. An identified WAV keeps the name
its own file gives it; a missing sound shows as, for example, "D1 loop 731".

WAV identity matching without a digest match uses a native code filename, or a
label key and category, plus measured audio duration. The duration comparison
allows four frames at 44.1 kHz for complete runs. This is a matching policy,
not proof of identical audio. A WAV that has an identity's label and duration
but a different digest is never assigned to it; the library reports it instead.
Arbitrary WAVs remain available as ordinary samples.

A native code filename takes precedence over a label. A label that maps one WAV
to multiple native identities remains unresolved. A product folder such as
`dance1`, `techno2`, or `techno3`, or a known extracted folder name, limits
label matching to that product. A native code filename takes precedence over the
folder hint. Without that folder context, cross-product collisions remain
unresolved. The app keeps unresolved files available as ordinary WAVs. Multiple
files that match the same native identity by name leave that identity
unresolved.

Original catalog bar counts and tempos apply to identified native samples.
Generic WAVs use the source tempo selected by the user. A song that needs a
missing or unresolved native identity cannot use a different sound in its place.
