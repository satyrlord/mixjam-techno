# Native rail icons

The native right-hand rail uses the PNG files in this directory. Each PNG is
96 by 96 pixels with a transparent background and navy (`#05092a`) strokes.
The SVG files are the editable source. All use a `0 0 24 24` view box,
1.7-point strokes, round caps and round joins. The UI supplies each action's
name as a tooltip and accessible name.

## Origin

The source family is the authored RaveFold `TrackerIcon` component preserved
at [tracker-icons.tsx](../donors/ravefold/icons/tracker-icons.tsx), from commit
`5965621be6ac2ea5330db9fb231755860617a285`. The preserved file is unchanged.
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

## Render and check

The renderer is isolated from the global Python environment. From the
repository root, install the pinned renderer into the ignored local data
directory, then run the helper:

```powershell
python -m pip install --target local-data/icon-renderer resvg-py==0.3.1 Pillow==12.3.0
python tools/assets/render-rail-icons.py
```

The helper uses Pillow to check pixels and make the contact sheet. It reads the
SVG sources and renders each icon with resvg. It keeps the rendered alpha
coverage and sets each RGB pixel to navy. It checks PNG dimensions,
transparency, stroke color, and distinct hashes. It writes `manifest.json` and
`contact-sheet.png`. The native UI embeds the individual PNG files. Use the
contact sheet and manifest for review.
Use `python tools/assets/render-rail-icons.py --check` to check existing
assets and their manifest hashes without writing files.
