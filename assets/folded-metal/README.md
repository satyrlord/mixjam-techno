# Folded Metal artwork

The owner selected Folded Metal on 2026-10-12.
The common chassis uses folded painted metal without fasteners, fine grain,
minor scratches, dark recessed control bays, cream lettering, and pale metal
primary buttons. Each tool retains its own color identity.

| Palette | Chassis | Accent |
| --- | --- | --- |
| Utility | `#3c4140` | `#e79331` |
| Marine | `#20535b` | `#55aeb7` |
| Granite | `#34393c` | `#adbabd` |
| Red | `#9a2d27` | `#eb7468` |
| Olive | `#3c4428` | `#9db25e` |

The current generated chassis is retained at
`local-data/artwork/folded-metal/chassis-screwless.png`. The earlier original
remains preserved as `chassis.png`. `provenance.json` records the active source's
SHA-256, crop, palette values, and prepared display variants. It came from the
built-in image-generation tool. The material slices preserve its actual grain,
scratches and highlights. Recoloring changes the painted material while
retaining neutral exposed metal details. No full-window bitmap is
stretched at runtime.

Each palette has four 112 by 64 corner patches, two 192 by 12 horizontal edge
strips, and two 12 by 96 vertical edge strips at 100%. All material boundaries
fade to zero alpha, including both ends of repeated strips. Four logical pixels
at the outer edges remain fully transparent. Continuous antialiased native
rounded paths own the outline and folded bevel, so source silhouettes and tile
joins cannot produce a broken window edge. Corner patches blend through alpha
overlap when collapsed; they are not cut at the window midpoint.
The 150% and 200% variants have proportional pixel dimensions. Every image
pixel paints one physical screen pixel; clipping supports smaller windows.

Shared 36 by 36 rotary caps and 48 by 24 primary-button grain tiles are original
procedural control artwork. Knobs are rendered at eight times each target size,
then reduced once with premultiplied-alpha Lanczos filtering. This antialiases
the silhouette and grip details without runtime scaling. Interaction, text, focus, position indicators and
inset panel borders remain native controls.

Prepare assets with `python tools/assets/render-folded-metal.py`. Verify the
prepared images and source provenance without changes with
`python tools/assets/render-folded-metal.py --check`. The private original is
required for preparation and verification, but not for running the application.
