# RaveFold asset references

Source commit: `5965621be6ac2ea5330db9fb231755860617a285`.

Preservation date: 2026-10-10.

This folder preserves RaveFold font and license bytes for reference. RaveFold
guides artistic direction only. These assets do not bring its feature
specifications, roadmap, architecture, or agent instructions into this product.
The owner-local provenance record
`local-data/research/legacy/ravefold/PROVENANCE.md` gives the scope. The
owner-local manifest
`local-data/research/legacy/ravefold/preservation-manifest.json` records source
paths, destination paths, sizes and SHA-256 hashes. The `local-data/` folder is
not published.

| Preserved location | Original source | Purpose |
| --- | --- | --- |
| `fonts/space-grotesk/` | Installed font package, version `5.3.0` | Font files and source metadata |
| `licenses/` | Six notice files in `public/` | Original dependency and font notices |

The original mask artwork, control artwork and vector source are not in this
folder. No vector artwork is in the application assets. The application uses
canonical transparent PNGs from `assets/rail/` and `assets/app/`.

The font package supplies three variable WOFF2 subsets: Latin, extended Latin
and Vietnamese. Its CSS declares weights from 300 to 700. The preserved WOFF2
files are preservation sources only. The application embeds Syncopate and Inter
from their own upstream sources. `tools/assets/prepare-fonts.py` verifies those
active assets.

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
