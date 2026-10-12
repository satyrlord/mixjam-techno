# Mixer artwork

The built-in OpenAI ImageGen tool produced the material atlas and small fader
from the owner's graphite and ivory Mixer direction. The design keeps the
compact controls and readable spacing learned from the earlier tools. Exact
generation prompts are in `prompts.json`. Each prepared PNG carries its
matching prompt in `Prompt` and `impeccable:prompt` metadata, plus the private
source path and SHA256 in `Source` metadata.

Private source files remain in `local-data/artwork/mixer/`:

- `materials.png`: SHA256
  `284bfd4d9ce3360625e9238afb3ea77b449c1a62d100771646c116de62bb2a48`.
- `fader.png`: SHA256
  `70cf045f7a6207500525aaefcf648d6e4c674902ca2c6bc123de534591d2d152`.

Three assets at three densities make nine prepared RGBA PNGs:

| Asset | `100` pixels | `150` pixels | `200` pixels |
| --- | --- | --- | --- |
| `ivory.png` | 108 x 64 | 162 x 96 | 216 x 128 |
| `graphite.png` | 108 x 64 | 162 x 96 | 216 x 128 |
| `fader.png` | 24 x 32 | 36 x 48 | 48 x 64 |

The atlas contains equal ivory and graphite halves. Offline preparation crops
each half and reduces it to a 216 by 128-pixel canonical tile before preparing
the densities above. Preparation removes broad lighting and retains the fine
generated grain so repetition does not make a visible checkerboard. The fader master
is cropped to its visible alpha bounds, its alpha is thresholded, and it is
first reduced to a 48 by 64-pixel canonical grid. Lower density caps are made
from that small grid. Broad ridges and a central ivory stripe remain legible;
fine photographic detail is not required for a 24 by 32-pixel control.

The current Mixer strip is 108 logical points wide. Native painting supplies
all titles, sound summaries, dB values, rails and marks, pan, small Unity and
Center buttons, polarity and routing controls, and interaction states. Master
stays visible beside the horizontally scrolling track bank. Artwork does not
encode any control value or application behavior. At runtime, select the
nearest prepared density, align bounds to physical pixels, and paint one texel
per physical pixel without texture resizing. Repeat material tiles and clip
them to their destination bounds.

Run `python tools/assets/render-mixer.py` to prepare the PNGs offline. Run
`python tools/assets/render-mixer.py --check` to compare all nine files byte
for byte with a fresh preparation. Both commands require Pillow,
`prompts.json`, and the two private hash-checked masters. A checkout without
those masters can use the prepared assets but cannot run reproduction.

Run `python tools/assets/check-raster-assets.py` after asset or source changes.
These checks establish reproduction and provenance. They do not establish
owner visual acceptance, physical high-DPI behavior, screen-reader support,
listening quality, or original-product mixer parity.
