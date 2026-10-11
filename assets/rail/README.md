# Native UI icons

The native right-hand rail and the header Undo, Redo, and Help actions use the PNG
files in this directory. Each PNG is 96 by 96 pixels with a transparent
background, antialiased edges, and navy (`#05092a`) artwork. These PNGs are the
canonical source assets. Edit them directly with raster tools. The UI supplies each action's
name as a tooltip and accessible name.
Help reuses the library artwork and retains its own action name.

The numbered subfolders contain prepared display PNGs at 20, 30, and 40 pixels.
These cover 100%, 150%, and 200% display density for a nominal 20-point control.
The UI chooses the closest prepared size and draws it at exactly one image pixel
per display pixel.
Unusual display scales can vary the logical icon size slightly. The UI does not
resize these images at runtime. The 96-pixel canonical PNGs are authoring inputs.

## Origin

The artwork was adapted from the RaveFold `TrackerIcon` component at commit
`5965621be6ac2ea5330db9fb231755860617a285`. The historical component hash is
`f7e2791cb0c9641a654d219be24619d01070983b7732eb4a424a89644d65ba9b`.
The manifest retains the earlier authoring-source hashes as historical evidence.
They are not current asset inputs. The following table records that adaptation.
See the [donor provenance](../donors/ravefold/README.md) for its notices and
license status. This adaptation does not assign a new license to donor work.

| Asset | Source or adaptation |
| --- | --- |
| `save` | Exact donor `save` path |
| `export` | Exact donor `export` path |
| `mixer` | Exact donor `mixer` path |
| `erase` | Exact donor `erase` path |
| `delete` | Exact donor `trash` path |
| `effects` | Exact donor `inspector` waveform path |
| `undo` | Exact donor `undo` path |
| `redo` | Exact donor `redo` path |
| `save-as` | Donor `save` outline, opened at lower right for a new pencil |
| `import` | Donor `export` tray with its arrow reversed |
| `open` | New open-folder path in the donor family |
| `libraries` | New books path in the donor family |
| `new` | New document and plus path in the donor family |
| `step-synth` | New lightning path in the donor family |
| `edit` | New pencil path in the donor family |
| `stretch` | New outward arrows and sample boundaries in the donor family |
| `record` | New microphone path in the donor family |
| `mix` | New crossing channel paths in the donor family |

## Update and check

The helper needs only Pillow. From the repository root, install it into the
ignored local data directory, then run the helper:

```powershell
python -m pip install --target local-data/icon-renderer Pillow==12.3.0
python tools/assets/render-rail-icons.py
```

The helper reads the canonical PNGs without changing their pixels or bytes.
It prepares every display variant offline with LANCZOS resampling of the alpha
channel and retains the exact artwork color.
It checks PNG dimensions, transparency, antialiased edges, artwork color, and
distinct hashes. It writes the display PNGs and updates `manifest.json` and
`contact-sheet.png`. The native UI embeds the individual PNG files. Use the
contact sheet and manifest for review.
Use `python tools/assets/render-rail-icons.py --check` to check existing
assets, every prepared display size, and their manifest hashes without writing files.
Run `python tools/assets/check-raster-assets.py` to check the repository's
raster-only asset policy.
