# Open songs from other programs

**Open** accepts these files:

- `.mixjam` songs.
- MixJam v7 `.mixjam` projects.
- JSON projects from earlier builds.
- Eight legacy `.mix` profiles, named `d1`, `d2`, `d3`, `d4`, `h1`, `t1`,
  `t2`, and `t3` after the original products that wrote them.

The import selects the nearest project tempo. If the tempo changes, the
application shows a notice. The application does not open a song that uses
more than 64 tracks.

## Legacy songs and their audio

A legacy `.mix` file can be a maximum of 500 MB (500,000,000 bytes). The
application imports the supported embedded WAV files automatically, so it does
not need their original external files. Songs can also refer to factory
samples from the selected library. The file size alone does not show if a song
contains embedded audio.

Audio that is not available shows as a silent placeholder for a missing
sample, at the saved length or the catalog length. If the source duration is
not known, the placeholder shows that its length is inferred. To see the
original metadata and the reason, put the pointer on a placeholder, or open
**Import details**.

To select a WAV file for a missing sample:

1. Right-click the placeholder.
2. Select **Resolve manually**.
3. Select a WAV file.

The choice resolves all identical missing references on all tracks. The
original positions, spans, and musical trim offsets do not change. If you
cancel, nothing changes.

The application keeps imported audio in its data folder for saved projects.
Keep that folder available when you open these projects again.

## Sample paths of older releases

Older releases of the application also saved version-7 `.mixjam` files with
sample paths that use genres. If the selected folder has the name
`_original`, the importer can also find these references from the folder that
contains it. That folder must not be a drive root. The importer also finds
category folders that were moved. Exact relative paths have priority.

## Sample conflicts

If two or more files match a reference, the first match loads, and a **Sample
conflicts** window shows the choices. To use a different file:

1. Select a different path. All instances in the song change immediately.
2. To compare the result, select **Play song**.
3. Select **Keep choices**.

Esc hides the window, but does not confirm the choices. To open the window
again, select **Sample conflicts** in the status bar. This conflict behavior
applies to all supported project formats. It supports undo and redo. The
resolved paths stay after you save the song as `.mixjam` and open it again.

The application does not change an imported v7 `.mixjam`, `.mix`, or JSON file.
**Save** asks for a new `.mixjam` file.

## Products that share sounds

`d2` songs can also contain packed samples in the original compressed sample
format. The reader decodes them and keeps them as WAV audio. If a song uses
sounds from earlier products, select the common parent sample folder. These
products can refer to the sounds of earlier products:

- `d3` songs can refer to `d2` sounds.
- `d4` songs can refer to `d3` and `d2` sounds.
- The supported `t2` compatibility bank uses `d2` IDs 1 through 1957.

## Sample identity catalogs

A `.mix` song stores numeric sample IDs. The application resolves them with
sample identity catalogs. These are data files in an `assets/catalog` folder
beside the application. For a build, this is the `assets/catalog` folder of the
repository. The application reads the catalogs when it starts. They are not
part of the executable.

Each catalog file describes one release of one original product. It contains:

- The name of the product and the release.
- The size and the hash of each original file that it was made from.
- For each sample: the native ID, the archive code, the category, the length,
  the tempo, a digest of the audio, and a one-way key of the name.

A catalog does not contain sound names, file names, or audio. The application
identifies a WAV file when its audio matches. The file name is not important,
and the sound keeps the name of your file.

A digest comparison is possible only with mono audio at the original 44.1 kHz
rate, the same frame count, and more than two frames. If a comparison is not
possible, the reader can identify the sample from an unambiguous native code
or name key, the category, and the duration. If an available digest is
different, the reader rejects the WAV file, also when its name and length
match. The reason for the missing source names that WAV file.

### Catalogs of more than one release

Catalogs of two or more releases of one product can be present together. They
merge when the audio, the tempo, and the bar count agree for each native ID
that they share. The application keeps the alternative name keys for
matching. A catalog with conflicting audio or timing does not load. The
application names it at startup, as it does for a catalog file that is not
valid. `mixjam-techno inspect` lists the loaded catalogs and the files that did
not load.

### Products without a catalog

Without a catalog for a product, WAV folders still load as usual sounds. With
WAV-only libraries, native sources that the application cannot identify open
as placeholders in imported or saved songs. A placeholder has the name of its
product, category, and ID: for example, "D1 loop 731". **Import details** name
the product that had no catalog at import.

A song can use native IDs that no loaded catalog of its product lists. In this
case, **Import details** tell you this one time for that product: the song can
come from a release that has no catalog here. To add the release:

1. Add the catalog file to `assets/catalog`.
2. Open the song again.

The application reads the changed folder and scans the sample folders again
before it opens the song. You do not have to start the application again.

### Source tempos

The optional `source-tempos.json` in the same folder adds tempo identities for
two original hip-hop products. The help of these products states that all
their samples were recorded at 90 BPM. A tool that is not published makes this
file offline from the original media. This metadata adds tempo evidence only.
It does not add song-format support for these products.
