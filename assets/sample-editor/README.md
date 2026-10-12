# Sample Editor artwork

The built-in OpenAI ImageGen tool produced a blank charcoal and gunmetal chassis
from the owner's Sample Editor mock, a separate collapsed chassis, and a simple
silver pitch cap. The owner requested the same compact scale and spacing
principles as the earlier tools. A later header edit shortened both title
recesses to leave clear corner-control mounting areas. The exact initial and
header-edit prompt chains are in `prompts.json`. Every prepared PNG carries
the matching prompt chain in `Prompt` and
`impeccable:prompt` metadata, plus its private source path and SHA256 in `Source`.

Private source files remain in `local-data/artwork/sample-editor/`:

- `panel.png`: SHA256
  `beda7f4c608f05b8cadad38b0d811d8de5eba130d1b7eec3b2dae0855f7a770e`.
- `collapsed.png`: SHA256
  `9e15db07d8978739274b3a314877128d77742cf9bf0de5a6f0810c29e2342cae`.
- `thumb.png`: SHA256
  `b406554bb70f42d0f6c78255bc37470843ee8889913fddf52993532047034852`.

The original header inputs are retained privately for the edit history:

- `panel-header-v1.png`: SHA256
  `f5b68afd5f3c6972df95043a78920958a5dac5186de4f59ebdf4150031823c19`.
- `collapsed-header-v1.png`: SHA256
  `0fcbb0300d3e1e992d8d1edf4feb91826548c5c9a1cb069fa27706559c811859`.

These originals are not used for current density preparation.

Three assets at three densities make nine prepared RGBA PNGs:

| Asset | `100` pixels | `150` pixels | `200` pixels |
| --- | --- | --- | --- |
| `panel.png` | 640 x 320 | 960 x 480 | 1280 x 640 |
| `collapsed.png` | 640 x 48 | 960 x 72 | 1280 x 96 |
| `thumb.png` | 16 x 28 | 24 x 42 | 32 x 56 |

The current expanded window adds a 48-point native footer below the panel, for
640 by 368 logical points at the prepared scales. These dimensions describe
the implementation; the owner selected the visual treatment, not an exact
fixed size. The collapsed state has its own complete four-sided bezel and lower
rim. It is not a clipped portion of the expanded panel. The title recess is
confined to the central 80% of both headers, nominally x64..576 at the 100%
density. Keep x0..64 and x576..640 free of inset seams behind the native corner
buttons. The expanded title and controls share y32; the collapsed state shares
y24. Each button keeps its 32 by 32-point input bounds at x10..42 or x598..630.
Check the painted recess and button outlines together in native captures.

The chassis supplies a dark recessed surface, rounded gunmetal frame, fine grain,
and small lower screws. Its interior remains opaque; transparency belongs
outside the silhouette. The pitch cap supplies only simple silver material,
broad bevels, and a few broad grip lines. All titles, source names, labels,
frame and pitch readouts, rails, step buttons, action buttons, header icons,
status, and interaction states remain native. The native status footer uses an
inset dark well with an edge outline, or coral when editing is unavailable.
Native action buttons show hover, focus, pressed, and disabled feedback.
Normal labels and actions use
13-point Inter, values use 16-point Inter, and the title uses 21-point Syncopate.
The pitch readout uses egui's Monospace role, which the application maps to Inter.
Check actual rendered label clearance separately from artwork dimensions.

Offline preparation checks each private source hash, crops to alpha above 32,
and thresholds coverage at 128. The thumb first reduces to a 32 by 56-pixel
canonical grid before density preparation. This keeps the small cap legible
without photographic detail that collapses at its 16 by 28-pixel display size.
Lanczos sampling prepares the table's density variants. The panel and collapsed
asset must have opaque interiors. At runtime, choose the nearest density, align
bounds to physical pixels, and paint one texel per physical screen pixel without
texture resizing.

Run `python tools/assets/render-sample-editor.py` to prepare the nine PNGs
offline. Run `python tools/assets/render-sample-editor.py --check` to compare
all nine byte for byte with fresh preparation. Both commands require Pillow,
`prompts.json`, and the three private hash-checked masters. A checkout without
the private sources can use the prepared assets but cannot run reproduction.

Run `python tools/assets/check-raster-assets.py` after asset or source changes.
These checks establish reproduction and provenance. They do not establish owner
visual acceptance, physical high-DPI behavior, screen-reader support, listening
quality, or original-product DSP parity.
