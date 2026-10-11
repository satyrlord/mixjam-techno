# Time Stretcher artwork

The built-in OpenAI ImageGen tool produced this skin from the owner's mock.
The exact generation prompt is in `prompt.txt`. The source image is preserved
in private `local-data/artwork/stretch/time-stretcher.png`; its SHA256 is
`2e727e71b30910fd12d43df475a84965d431d76f0304a3c39200c81006e016f6`.

Text, dial pointers, input, focus, pressed, and disabled states are native.
The panel is 640 by 320 logical points. Prepared `100`, `150`, and `200`
variants measure 640 by 320, 960 by 480, and 1280 by 640 pixels. Paint each
texel at one physical screen pixel. Do not resize the skin at runtime.

`python tools/assets/render-stretch-controls.py` prepares the density variants
offline, makes the plate opaque, and masks its rounded outer corners.
`python tools/assets/render-stretch-controls.py --check` compares them byte for
byte with a fresh preparation. Both commands require Pillow and the private
source image.
