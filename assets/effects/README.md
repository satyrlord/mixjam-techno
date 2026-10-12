# Effect Studio artwork

The built-in OpenAI ImageGen tool produced the raw hardware artwork from the
owner's reference and panel material. Offline preparation produces the switch
master and all display variants below. The current compact implementation is
landscape. The owner rejected oversized controls and crowded labels; revised
spacing still requires native capture review. Exact generation prompts are in
`prompts.json`. Each prepared PNG
also carries its prompt in `Prompt` and `impeccable:prompt` metadata, plus its
private source path and SHA256 in `Source` metadata.

Private source files remain in `local-data/artwork/effects/`:

- `panel.png`: SHA256
  `4599df59374f5b06344fff1ae219551d5e99cd68faa5f8e1320a65475ce667c5`.
- `collapsed.png`: SHA256
  `54486d365aa0508da3e61339fe800892769ec541bdca365ba5077efeceb50258`.
- `toggles.png`: SHA256
  `8f0c6995397a3582c04a9a0f88f92110515823f2d79fc8e7c66e863f48b5882f`.
- `toggles-generated.png`: SHA256
  `5079d3a47989c8d98ad521f4e4fc03bdef6a5bba19a1a9762e57e5a46b2da870`.
- `knob.png`: SHA256
  `8c0a9b81dd3dddf07e9e9bef25a65fd4c5f946882a4cad2112712bc1ce18005e`.

Five assets at three densities make fifteen prepared PNGs:

| Asset | `100` pixels | `150` pixels | `200` pixels |
| --- | --- | --- | --- |
| `panel.png` | 640 x 352 | 960 x 528 | 1280 x 704 |
| `collapsed.png` | 640 x 48 | 960 x 72 | 1280 x 96 |
| `toggle-off.png` | 24 x 24 | 36 x 36 | 48 x 48 |
| `toggle-on.png` | 24 x 24 | 36 x 36 | 48 x 48 |
| `knob.png` | 32 x 32 | 48 x 48 | 64 x 64 |

The panel has a separate 48-point native status footer. Its complete expanded
window is 640 by 400 logical points at the prepared scales. The collapsed state
uses its own complete bezel. It is not a clipped portion of the expanded skin.
The switch prompt specifies a 32-pixel grid with broad, simple chrome shading.
ImageGen returned a 1774 by 887-pixel canvas, despite the requested 64 by
32-pixel working canvas. `prepare_toggles()` verifies that raw generation's
hash, takes a centered square covering 76% of each equal state cell, and
prepares `toggles.png` at 128 by 64 pixels. This canonical master has two
64-pixel square cells. Those cells produce the 24, 36, and 48-pixel state
variants. The simplified shading keeps fine photographic detail out of the
small controls.
The recorded collapsed-generation prompt requests 480 by 48 pixels; offline
preparation produces the current 640 by 48-pixel variant. Keep that prompt exact.

The panel skin supplies red metal, charcoal bezel, empty bank recesses, and
black and ivory button faces. A separate transparent sprite supplies each
32-point knob body. Native painting supplies ticks, numeric wells, title,
source, labels, values, pointers, section lamps, header controls, and
interaction feedback. Standard 12-14-point Inter text and a 22-point Syncopate
title preserve desktop density. At runtime, select the nearest prepared density
and align its bounds. Paint each texel at one physical screen pixel; do not resize
textures at runtime.
Keep knobs, values, and captions in separate vertical bands. Check actual glyph
bounds against dial arcs and neighboring controls at each prepared density.
The asset checks below verify reproduction and provenance, not rendered clearance.

`python tools/assets/render-effects-controls.py` prepares the switch master and
variants offline. It checks the raw switch generation and private master hashes,
trims transparent canvas padding from panel states and the knob, splits the
prepared switch cells, restores opaque metal, and resizes to the table above.
It preserves transparency outside the rounded bezel and switches.
`python tools/assets/render-effects-controls.py --check` compares every prepared
PNG byte for byte with a fresh preparation. Both commands require Pillow, the
four private masters, the raw switch generation, and `prompts.json`. A checkout
without these private files can use the prepared assets but cannot run this
reproduction check.

Run `python tools/assets/check-raster-assets.py` after asset or source changes.
These assets establish presentation provenance, not original-product DSP parity.
