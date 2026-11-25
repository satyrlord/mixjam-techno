# Native UI icons

The native left-hand tool rail and the header Undo, Redo, and Help actions use
the PNG files in this directory. Each PNG is 96 by 96 pixels with a transparent
background, antialiased edges, and navy (`#05092a`) artwork. These PNGs are the
canonical source assets. Edit them directly with raster tools. The UI supplies
each action's name as a tooltip and accessible name.
Help has its own question-mark artwork. Options uses a gear. Sample library
management is part of Options and has no icon of its own.

The `20` subfolder contains the prepared 20-pixel display PNGs. They serve a
nominal 20-point control at 100% display scaling, the only scale in scope.
The UI draws each at exactly one image pixel per display pixel. It does not
resize these images at run time. The 96-pixel canonical PNGs are authoring
inputs.

## Drawing rules

The icons follow one drawing rule. Draw each on a 24-unit grid at 16 times the
size and reduce it once. Use a 2-unit stroke with round caps and joins. Center
the straight strokes on pixel boundaries of the 20-pixel display size.

| Asset | Form |
| --- | --- |
| `save` | Floppy disk |
| `export` | Open tray with an upward arrow |
| `mixer` | Three vertical sliders |
| `erase` | Tilted eraser over a baseline |
| `delete` | Trash can with a lid |
| `effects` | "fx" lettering |
| `undo` | Counterclockwise loop arrow pointing left |
| `redo` | Clockwise loop arrow pointing right |
| `save-as` | Floppy disk opened at lower right with a pencil |
| `import` | Open tray with a downward arrow |
| `open` | Open folder |
| `options` | Eight-tooth gear with shallow teeth |
| `new` | Document with a plus |
| `step-synth` | Piano roll: a key ladder and three notes stepping up |
| `edit` | Waveform between two brackets, the sample's edges |
| `stretch` | Outward arrows between two sample boundaries |
| `record` | Microphone on its stand |
| `help` | Ringed question mark |
| `master` | Level gauge for Mastering |
| `album-review` | Record with its tonearm, for Album Review |
| `fx-returns` | Square-cornered loop that leaves a baseline and returns into it, the return to the mix |

## Update and check

The helper needs only Pillow. From the repository root, install it into the
ignored local data directory, then run the helper:

```powershell
python -m pip install --target local-data/icon-renderer Pillow==12.3.0
python tools/assets/render-rail-icons.py
```

The helper reads the canonical PNGs and does not change their pixels or bytes.
It prepares each display PNG offline with LANCZOS resampling of the alpha
channel and keeps the exact artwork color.
It checks PNG dimensions, transparency, antialiased edges, artwork color, and
distinct hashes. It writes the display PNGs and updates `manifest.json` and
`contact-sheet.png`. The native UI embeds the individual PNG files. Use the
contact sheet and manifest for review.
To check the existing assets, every prepared display PNG, and their manifest
hashes without writing files, run
`python tools/assets/render-rail-icons.py --check`.
To check the raster-only asset policy of the repository, run
`python tools/assets/check-raster-assets.py`.
