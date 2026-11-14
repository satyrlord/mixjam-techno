# Native UI icons

The native left-hand tool rail and the header Undo, Redo, and Help actions use the PNG
files in this directory. Each PNG is 96 by 96 pixels with a transparent
background, antialiased edges, and navy (`#05092a`) artwork. These PNGs are the
canonical source assets. Edit them directly with raster tools. The UI supplies each action's
name as a tooltip and accessible name.
Help has its own question-mark artwork. Options uses a gear. Sample library
management is part of Options; its former books icon is retired.

The `20` subfolder contains the prepared 20-pixel display PNGs. They serve a
nominal 20-point control at 100% display scaling, the only scale in scope.
The UI draws each at exactly one image pixel per display pixel and does not
resize these images at runtime. The 96-pixel canonical PNGs are authoring inputs.

## Origin

The artwork was adapted from the RaveFold `TrackerIcon` component at commit
`5965621be6ac2ea5330db9fb231755860617a285`. The historical component hash is
`f7e2791cb0c9641a654d219be24619d01070983b7732eb4a424a89644d65ba9b`.
The manifest retains the earlier authoring-source hashes as historical evidence.
They are not current asset inputs. The following table records that adaptation.
See the [donor provenance](../donors/ravefold/README.md) for its notices and
license status. This adaptation does not assign a new license to donor work.

Every icon is now drawn with Pillow on a 24-unit grid at 16 times the size and
reduced once: a 2-unit stroke with round caps and joins, its straight strokes
centred on pixel boundaries of the 20-pixel display size. Icons marked as a
donor form follow that donor path's shape; the rest are new in its family.

| Asset | Source or adaptation |
| --- | --- |
| `save` | Donor `save` form |
| `export` | Donor `export` form |
| `mixer` | Donor `mixer` form |
| `erase` | Donor `erase` form |
| `delete` | Donor `trash` form |
| `effects` | New "fx" lettering |
| `undo` | Donor `undo` form |
| `redo` | Donor `redo` form |
| `save-as` | Donor `save` outline, opened at lower right for a new pencil |
| `import` | Donor `export` tray with its arrow reversed |
| `open` | New open folder |
| `options` | New eight-tooth gear with shallow teeth |
| `new` | New document with a plus |
| `step-synth` | New piano roll: a key ladder and three notes stepping up |
| `edit` | New waveform between two brackets, the sample's edges |
| `stretch` | New outward arrows between two sample boundaries |
| `record` | New microphone on its stand |
| `help` | New ringed question mark |
| `master` | New level gauge for Mastering |
| `album-review` | New record with its tonearm, for Album Review |
| `fx-returns` | New square-cornered loop that leaves a baseline and returns into it, the return to the mix |

## Update and check

The helper needs only Pillow. From the repository root, install it into the
ignored local data directory, then run the helper:

```powershell
python -m pip install --target local-data/icon-renderer Pillow==12.3.0
python tools/assets/render-rail-icons.py
```

The helper reads the canonical PNGs without changing their pixels or bytes.
It prepares each display PNG offline with LANCZOS resampling of the alpha
channel and retains the exact artwork color.
It checks PNG dimensions, transparency, antialiased edges, artwork color, and
distinct hashes. It writes the display PNGs and updates `manifest.json` and
`contact-sheet.png`. The native UI embeds the individual PNG files. Use the
contact sheet and manifest for review.
Use `python tools/assets/render-rail-icons.py --check` to check existing
assets, every prepared display PNG, and their manifest hashes without writing files.
Run `python tools/assets/check-raster-assets.py` to check the repository's
raster-only asset policy.
