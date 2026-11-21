# Lane cache, measurements, and reports

## Lane cache

The lane cache is in `lane-cache` beside the output folder by default. It keeps
the most recently used 4 GB in a versioned subfolder. Cache cleanup touches
only its named entries. It also removes orphaned digests and temporary files
more than a day old. The age of a temporary file starts when the tool stages
it. Cleanup leaves other cache versions and unrelated files alone.

A lane WAV in an output folder and its cache entry are hard links to one file.
An unchanged lane takes no more disk space in each new output folder. On
another volume, the tool copies the lane. The tools replace a lane WAV. They do
not write into it. If another program writes into a lane WAV, the cache entry
does not match its recorded digest. The tool then renders the lane again.
Copy a lane WAV before you edit it. A missing digest also needs a new render.

A staged link or copy must pass the WAV and digest checks before it replaces an
output. The tool renders missing or damaged entries again. A failed lane render
keeps that lane's previous WAV. Use `--no-cache` to render every lane.

Caching needs these conditions:

- The song is format 18 to 26.
- The song has directly linked, absolute, imported WAV paths with a source
  tempo.
- The song has no native, library, missing-source, conflict, or origin
  dependency.
- The song has no sections, or all its sections are valid.

Other songs still render, with no cache reuse. Cache keys include the content
of the source files, the render settings, the global Solo state, the master
gain, and the renderer build. A gain change needs a new render. The tool does
not rescale cached PCM.

Repeat lane renders keep the duration of the whole song. Captured audio can
therefore continue after the last clip of the lane. Their cache keys include
that duration. When the audibility report selects the bars to measure, it
follows the Repeat captures. FX tails stay outside that selection.

Use `--no-master` for intermediate trials only. `measure.py` and the
song-review gate need a current `master.wav` from a render without that
option.

## Sample measurements and palettes

After you index changed WAVs, run `sampletags.py tag` before `verdicts.py
build`. A verdict does not use a measurement from an older file. The palette
reader checks each verdict against the current file and analysis.

Palette files made from songs keep pathless native samples by ID. The palette
reader uses format-3 release catalogs. It reports files that are invalid
or in conflict. Catalogs hold no sound names. The reader takes the names from valid
private name files for the accepted product and release. It rejects an invalid
name file in full. If no name matches, the reader identifies a sound by its
archive code.

When `Song.sound` links an extracted WAV, it can use the product name for a
matching archive code and duration. This includes codes from compatible
releases. Comparable mono 44.1 kHz PCM digests must match. If no comparable
digests exist, the tool infers the name from the code and the duration. If a
match is ambiguous, the file name stays. This changes the display name only. It
does not replace the linked WAV.

## Mechanical report

For `master.py`, `--bpm` overrides the tempo. Without that option, a supplied
song gives its tempo. With no song, the tool uses the metadata tempo, or 140
BPM when none is available. If the section list is missing, the report has
no section table.

The mechanical report checks the source duration of WAV files linked in
`samples`. It checks abrupt endings on lanes with a melodic role in the
`.meta.json`: `pad`, `lead`, `keys`, `arp`, `fx`, `vocal`, or `atmos`.
The separate phrase check gates `lead`, `keys`, `arp`, `bass`, `vocal`, `pad`,
and `atmos`. Both checks follow Pitch automation and reversed slices.
An approved `cuts` span exempts only clips fully inside that span.
Without metadata, the report assigns generic `trackN` roles and cannot gate
these role-specific checks. `render.py` also needs metadata to make role stems.

Catalog-only sources need the library resolver of the application. The report
marks those source checks as unavailable and does not pass the gate. Missing
and catalog sources still appear in the arrangement report.

Harmonic fit needs the lane renders and the premaster at the same sample rate.
Dry-source alternatives need linked WAV files. The comparison excludes
reversed clips, lanes with Pitch automation, and simultaneous clips in a
rendered lane. It cannot model their source positions separately.
It does not model EQ or drive. The report marks unavailable comparisons.
A candidate needs a new render before acceptance. Tracks with the same lane
name share the renderer's lane file and candidates. This includes `(twin)`
tracks.
