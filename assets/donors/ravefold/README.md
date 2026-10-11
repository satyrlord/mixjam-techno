# RaveFold asset references

Source commit: `5965621be6ac2ea5330db9fb231755860617a285`.

Preservation date: 2026-10-10.

The remaining files preserve RaveFold font and license bytes for reference.
RaveFold guides artistic direction only. These assets do not bring its feature
specifications, roadmap, architecture, or agent instructions into this product.
The [provenance record](../../../docs/legacy/ravefold/PROVENANCE.md) gives the
scope. The [manifest](../../../docs/legacy/ravefold/preservation-manifest.json)
records source paths, destination paths, sizes and SHA-256 hashes.

| Preserved location | Original source | Purpose |
| --- | --- | --- |
| External `retired-assets/assets/donors/ravefold/logo/` | Original mask artwork | Historical source only |
| External `retired-assets/assets/donors/ravefold/icons/` | Original control artwork | Historical source only |
| External `retired-assets/assets/donors/ravefold/design/` | Original mask render with vector metadata | Historical source only |
| `fonts/space-grotesk/` | Installed font package, version `5.3.0` | Font files and source metadata |
| `licenses/` | Six notice files in `public/` | Original dependency and font notices |

The vector source and the original render containing vector metadata were
moved unchanged to the sibling `mixjam-techno-support/retired-assets/` directory
on 2026-10-11. No vector artwork remains here. The application uses canonical
transparent PNGs from `assets/rail/` and `assets/app/`. The relocation receipt is
`evidence/review-20261011-resumed/retired-assets.json`; archived manifests retain
their original historical paths and hashes.

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
