# Mixer artwork

The built-in image-generation tool produced the material atlas and small fader
from the owner's graphite and ivory Mixer direction. The design keeps compact
controls and readable spacing. `prompts.json` holds the exact generation
prompts. Each prepared PNG carries its matching prompt in `Prompt` and
`impeccable:prompt` metadata. The `Source` metadata holds the private source
path and SHA256.

The private source files are in `local-data/artwork/mixer/`:

- `materials.png`: SHA256
  `284bfd4d9ce3360625e9238afb3ea77b449c1a62d100771646c116de62bb2a48`.
- `fader.png`: SHA256
  `70cf045f7a6207500525aaefcf648d6e4c674902ca2c6bc123de534591d2d152`.

The `100` subfolder holds three prepared RGBA PNGs. They have one texel per
screen pixel at 100% display scaling, the only scale in scope:

| Asset | Pixels |
| --- | --- |
| `ivory.png` | 108 x 64 |
| `graphite.png` | 108 x 64 |
| `fader.png` | 24 x 32 |

The atlas contains equal ivory and graphite halves. Offline preparation crops
each half and reduces it to a 216 by 128-pixel canonical tile. It then prepares
the size above from that tile. Preparation removes broad lighting and keeps the
fine generated grain, so repetition does not make a visible checkerboard.
Preparation crops the fader master to its visible alpha bounds, thresholds its
alpha, and reduces it to a 48 by 64-pixel canonical grid. It makes the prepared
cap from that small grid. Broad ridges and a central ivory stripe stay legible.
A 24 by 32-pixel control does not need fine photographic detail.

The current Mixer strip is 108 logical points wide. Native painting supplies
all titles, sound summaries, dB values, rails and marks, pan, small Unity and
Center buttons, polarity and routing controls, and interaction states. Master
stays visible beside the horizontally scrolling track bank. Artwork does not
encode any control value or application behavior. At runtime, align bounds to
physical pixels and paint one texel per physical pixel. Do not resize the
texture. Repeat material tiles and clip them to their destination bounds.

Run `python tools/assets/render-mixer.py` to prepare the PNGs offline. Run
`python tools/assets/render-mixer.py --check` to compare all three files byte
for byte with a fresh preparation. Both commands require Pillow,
`prompts.json`, and the two private hash-checked masters. A checkout without
those masters can use the prepared assets but cannot run reproduction.

Run `python tools/assets/check-raster-assets.py` after asset or source changes.
These checks establish reproduction and provenance. They do not establish
owner visual acceptance, screen-reader support,
listening quality, or original-product mixer parity.
