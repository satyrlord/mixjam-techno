# RaveFold asset references

Source commit: `5965621be6ac2ea5330db9fb231755860617a285`.

Preservation date: 2026-10-10.

These files preserve exact RaveFold asset bytes for native interface reference.
RaveFold guides artistic direction only. These assets do not bring its feature
specifications, roadmap, architecture, or agent instructions into this product.
The [provenance record](../../../docs/legacy/ravefold/PROVENANCE.md) gives the
scope. The [manifest](../../../docs/legacy/ravefold/preservation-manifest.json)
records source paths, destination paths, sizes and SHA-256 hashes.

| Preserved location | Original source | Purpose |
| --- | --- | --- |
| `logo/ravefold-mask.svg` | `src/assets/ravefold-mask.svg` | Mask logo reference |
| `icons/tracker-icons.tsx` | `src/ui/tracker-icons.tsx` | Twenty SVG control symbols |
| `design/ravefold-mask-render.png` | `design/ravefold-mask-render.png` | Mask render; with the SVG, the application icon source |
| `fonts/space-grotesk/` | Installed font package, version `5.3.0` | Font files and source metadata |
| `licenses/` | Six notice files in `public/` | Original dependency and font notices |

The icon source uses React and SVG. The native application uses adapted PNG
icons from `assets/rail/`. This reference source is outside the Rust build.

The font package supplies three variable WOFF2 subsets: Latin, extended Latin
and Vietnamese. Its CSS declares weights from 300 to 700. The preserved WOFF2
files supplied the earlier native Space Grotesk assets. They remain preservation
sources. The current application embeds Syncopate and Inter from their own
upstream sources; `tools/assets/prepare-fonts.py` verifies those active assets.

## License notices

| Material | License | Preserved notice |
| --- | --- | --- |
| Space Grotesk | SIL Open Font License 1.1 | `licenses/font-license.txt` and `fonts/space-grotesk/LICENSE` |
| Plasma UI and copied reference settings | MIT | `licenses/third-party-notices.txt` |
| Signalsmith Stretch | MIT | `licenses/signalsmith-stretch-license.txt` |
| React | MIT | `licenses/react-license.txt` |
| React DOM | MIT | `licenses/react-dom-license.txt` |
| Scheduler | MIT | `licenses/scheduler-license.txt` |

Keep applicable notices with material used in later work. The donor repository
contains no separate license file for its mask, design images or icon source.
This archive records their origin without assigning a new license.
