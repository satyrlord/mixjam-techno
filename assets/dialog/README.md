# Utility dialog artwork

The built-in OpenAI ImageGen tool produced this reusable blank graphite chassis
for non-musical utility windows and dialogs. The owner's request extends the
compact hardware treatment used by the musical tools. The chassis supplies
only material: all titles, content, fields, buttons, and title-bar controls are
native egui widgets. It contains no lettering, icons, screws, or control faces.

The exact generation prompt is in `prompts.json`. Every prepared PNG carries
the matching prompt in `Prompt` and `impeccable:prompt` metadata, plus the private
source path and SHA256 in `Source` metadata. The private master remains at
`local-data/artwork/dialog/chassis.png`, SHA256
`6b24831f1bd31035e70beed0596588dfd5d387823513fc7f37dbd019d8abcc98`.

Offline preparation crops the master to its visible alpha bounds above 32,
thresholds alpha at 128, and reduces it to a 640 by 384-pixel canonical chassis
with Lanczos sampling. The script then extracts nine pieces and prepares their
three densities. Before density preparation, repeated pieces have broad lighting
removed while retaining fine grain. The body uses its overall mean; left and
right edges use column means; top and bottom edges use row means. This keeps the
cross-edge bezel profile and prevents repeated lighting bands. Corners retain
their original profile. Nine pieces at three densities make 27 RGBA PNGs:

| Pieces | `100` pixels | `150` pixels | `200` pixels |
| --- | --- | --- | --- |
| `top-left.png`, `top-right.png` | 16 x 64 | 24 x 96 | 32 x 128 |
| `top.png` | 64 x 64 | 96 x 96 | 128 x 128 |
| `left.png`, `right.png` | 16 x 64 | 24 x 96 | 32 x 128 |
| `body.png` | 64 x 64 | 96 x 96 | 128 x 128 |
| `bottom-left.png`, `bottom-right.png` | 16 x 16 | 24 x 24 | 32 x 32 |
| `bottom.png` | 64 x 16 | 96 x 24 | 128 x 32 |

The canonical side edges are 16 pixels wide, the upper artwork band is 64
pixels high, and the lower rim is 16 pixels high. The canonical dimensions
describe asset preparation, not a fixed dialog size. `src/ui/dialog.rs` chooses
the nearest prepared density, aligns outer bounds to physical pixels, and tiles
the pieces at one texel per physical pixel. Partial final tiles use cropped UVs;
the texture is never stretched. A collapsed native utility retains a complete
lower rim. The caller controls window size, scrolling, close behavior, and content.

Run `python tools/assets/render-dialog.py` to prepare the PNGs offline. Run
`python tools/assets/render-dialog.py --check` to compare all 27 files byte for
byte with fresh preparation. Both commands require Pillow, `prompts.json`, and
the private hash-checked master. A checkout without the master can use the
prepared assets but cannot run reproduction.

Run `python tools/assets/check-raster-assets.py` after asset or source changes.
These checks establish reproduction and provenance. They do not establish
owner visual acceptance, physical high-DPI behavior, screen-reader support,
native file-picker behavior, audio quality, or original-product parity.
