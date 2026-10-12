# Time Stretcher artwork

The built-in OpenAI ImageGen tool produced these skins from the owner's mock
and the first panel skin. The generation prompts are in `prompt.txt`, including
the first skin's prompt and recorded hash; its private master is not retained.
Private masters are in `local-data/artwork/stretch/`:

- `time-stretcher-refined.png`: SHA256
  `3bc70d0b0e00bc4ea3a775313ace61591e4ce5adc4415fcc1272b82829c4d184`.
- `time-stretcher-collapsed.png`: SHA256
  `b3c4ee60af97123c4e429dd14289d06743c856655d8908e3f7c5be71af99fa64`.

Text, header icons, dial pointers, input, focus, pressed, and disabled states
are native. The upper screws and former inset header icons are absent. Collapse
and Close use the former screw columns; Help uses a drawn question-mark icon.
The panel is 640 by 320 logical points. Prepared `100`, `150`, and `200`
variants measure 640 by 320, 960 by 480, and 1280 by 640 pixels. Paint each
texel at one physical screen pixel. Do not resize the skin at runtime.
The separate `collapsed.png` skin has a complete lower bezel. Its `100`, `150`,
and `200` variants measure 640 by 64, 960 by 96, and 1280 by 128 pixels.

`python tools/assets/render-stretch-controls.py` prepares the density variants
offline, trims transparent canvas padding, and makes the metal opaque. It
retains the generated bezel contour and antialiases the rounded outer corners.
`python tools/assets/render-stretch-controls.py --check` compares them byte for
byte with a fresh preparation. Both commands require Pillow and the private
source images.
