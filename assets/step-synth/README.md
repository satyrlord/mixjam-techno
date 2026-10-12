# Step synth artwork

The built-in OpenAI ImageGen tool produced the expanded olive chassis from the
owner's Step synth reference and the separate collapsed chassis from that
material. The owner requested the compact controls and spacing principles used
for the earlier tools. Exact generation prompts are in `prompts.json`. Each
prepared PNG carries the matching prompt in `Prompt` and `impeccable:prompt`
metadata, plus its private source path and SHA256 in `Source` metadata.

Private source files remain in `local-data/artwork/step-synth/`:

- `panel.png`: SHA256
  `f1e0d249def2791b68a3d91eaffa9c42c2a7b93f76abc207f6270720ea25a646`.
- `collapsed.png`: SHA256
  `333d135a65fba75f4641e46a9149e3f6696f18487da3e22052fca1126c4aef53`.

Two assets at three densities make six prepared RGBA PNGs:

| Asset | `100` pixels | `150` pixels | `200` pixels |
| --- | --- | --- | --- |
| `panel.png` | 820 x 556 | 1230 x 834 | 1640 x 1112 |
| `collapsed.png` | 820 x 48 | 1230 x 72 | 1640 x 96 |

The current expanded window adds a 44-point native footer below the skin, for
820 by 600 logical points at the prepared scales. These dimensions record the
implementation; the owner selected the visual treatment, not an exact fixed
size. The collapsed state uses its own complete four-sided bezel, including
the lower rim. It is not a clipped portion of the expanded background.

The skin supplies olive grain, a charcoal bezel, a thin inner rim, and two
small lower screws. The metal interior stays empty and opaque. Native painting
supplies every title, label, source field, grid cell, button, switch, header
icon, footer well, and interaction state. Transparent pixels remain only
outside the rounded chassis. Small switches use simple native pill geometry;
no detailed control sprites are required. Controls retain 12-13-point Inter
text and a 22-point Syncopate title. Check rendered label clearance separately
from these artwork dimensions.

At runtime, select the nearest prepared density and align its bounds. Paint
one texel per physical screen pixel without runtime texture resizing.

Run `python tools/assets/render-step-synth.py` to prepare the six PNGs offline.
It verifies the private source hashes, crops visible artwork, thresholds alpha,
checks the opaque metal interior, resizes to the table above, and embeds
provenance. Run `python tools/assets/render-step-synth.py --check` to compare
every prepared PNG byte for byte with a fresh preparation. Both commands
require Pillow, `prompts.json`, and the two private masters. A checkout without
the private files can use the prepared assets but cannot run reproduction.

Run `python tools/assets/check-raster-assets.py` after asset or source changes.
These checks establish asset reproduction and provenance. They do not establish
visual acceptance, high-DPI behavior, screen-reader support, listening quality,
or original-product DSP parity.
