# Native fonts

The application embeds Syncopate Bold 700 for the product name in every theme.
Each theme adds a heading face and a text face. Inter Medium 500 is Studio's
text face and every theme's fallback for symbols that a theme face lacks.

| Theme | Headings | Text |
| --- | --- | --- |
| Studio | IBM Plex Sans SemiBold 600 | Inter Medium 500 |
| Aero | Michroma 400 | Fira Sans Medium 500 |
| Vintage | Barlow Condensed SemiBold 600 | Barlow Medium 500 |

Convert visible titles to uppercase Latin text without changing saved project
names. The application embeds only the current theme's faces, the brand face,
and Inter.

## Syncopate Bold

`Syncopate-Bold.ttf` is unchanged from the Google Fonts repository at commit
`bd8f81ddb5c74d5c8897b36ad88b440266245103`, path
`apache/syncopate/Syncopate-Bold.ttf`. The original Apache License 2.0 is in
`LICENSE-Syncopate.txt`. The font retains its original copyright metadata:
Copyright (c) 2010 by Brian J. Bonislawsky DBA Astigmatic (AOETI).

Transliterate accented title letters to their Latin counterparts before
uppercase layout: Ș/Ş to S, Ț/Ţ to T, Â/Ă to A, and Î to I. This also handles
combining marks and extended Latin ligatures. The stored title remains intact.

## Inter Medium

`Inter-Medium.ttf` is the unchanged static text face from the official
[Inter 4.1 release](https://github.com/rsms/inter/releases/tag/v4.1), archive
`Inter-4.1.zip`, entry `extras/ttf/Inter-Medium.ttf`. Its weight is 500.
`LICENSE-Inter.txt` is the archive's unchanged `LICENSE.txt`, the SIL Open Font
License 1.1 and copyright notice.

## Theme faces

All theme faces are static fonts under the SIL Open Font License 1.1, with
installable embedding permission (`fsType` 0). Each is unchanged from its
pinned source; `manifest.json` records the URLs and hashes.

- `IBMPlexSans-SemiBold.ttf`: the IBM Plex release `@ibm/plex-sans@1.1.0`,
  archive `ibm-plex-sans.zip`, entry
  `ibm-plex-sans/fonts/complete/ttf/IBMPlexSans-SemiBold.ttf`. License:
  `LICENSE-IBMPlexSans.txt`, the archive's `ibm-plex-sans/LICENSE.txt`. "Plex"
  is a Reserved Font Name, so the file must not be modified.
- `FiraSans-Medium.ttf`, `Michroma-Regular.ttf`, `Barlow-Medium.ttf`, and
  `BarlowCondensed-SemiBold.ttf`: the Google Fonts repository at commit
  `2fe1e8c6b137fb13041a7cfca6fdcd8a910ea4dc`, under `ofl/`. Licenses:
  `LICENSE-FiraSans.txt`, `LICENSE-Michroma.txt`, and `LICENSE-Barlow.txt`,
  which covers both Barlow families.

Studio's text face is Inter rather than IBM Plex Sans Medium: Plex Sans has a
1.3 em line box, which would make every text-driven row of the shared layout
taller. Heading faces may have taller line boxes, because the application pins
heading rows to Syncopate's 1.041 em line box.

## Verify and restore

The check runs offline:

```powershell
python tools/assets/prepare-fonts.py
python tools/assets/prepare-fonts.py --check
```

The script requires `fonttools`. It verifies the font and license hashes,
static weights, family names, embedding permission, ASCII coverage, and the
Romanian letters of every text face. Without `--check`, it updates only the
manifest. To restore a font or license, retrieve its named upstream file or
archive entry unchanged. Do not copy installed system fonts.

Git preserves license files byte for byte through `.gitattributes`. Keep their
upstream line endings: the offline check compares the exact license bytes.

Keep every license with the application. Original Space Grotesk donor assets
and preservation records remain in `assets/donors/ravefold/`; the application
does not load or ship them as native font assets.
