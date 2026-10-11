# Native fonts

The application embeds Syncopate Bold 700 for branding and titles. Convert
visible titles to uppercase Latin text without changing saved project names. Inter Medium
500 serves all other application text.

## Syncopate Bold

`Syncopate-Bold.ttf` is unchanged from the Google Fonts repository at commit
`bd8f81ddb5c74d5c8897b36ad88b440266245103`, path
`apache/syncopate/Syncopate-Bold.ttf`. The original Apache License 2.0 is in
`LICENSE-Syncopate.txt`. The font retains its original copyright metadata:
Copyright (c) 2010 by Brian J. Bonislawsky DBA Astigmatic (AOETI).

Transliterate accented title letters to their Latin counterparts before
uppercase layout: Ș/Ş to S, Ț/Ţ to T, Â/Ă to A, and Î to I. This also handles
combining marks and extended Latin ligatures. The stored title remains intact.
Egui's proportional fallbacks remain available for UI symbols; the title family
does not use Inter as a Romanian fallback.

## Inter Medium

`Inter-Medium.ttf` is the unchanged static text face from the official
[Inter 4.1 release](https://github.com/rsms/inter/releases/tag/v4.1), archive
`Inter-4.1.zip`, entry `extras/ttf/Inter-Medium.ttf`. Its weight is 500. It includes
ASCII. `LICENSE-Inter.txt` is the archive's unchanged
`LICENSE.txt`, the SIL Open Font License 1.1 and copyright notice.

## Verify and restore

`manifest.json` records pinned source URLs, file sizes, and font/license hashes.
The check runs offline:

```powershell
python tools/assets/prepare-fonts.py
python tools/assets/prepare-fonts.py --check
```

The script requires `fonttools`. It verifies the source files, static weights,
family names, and ASCII title coverage. Without `--check`, it
updates only the manifest. To restore a font or license, retrieve its named
upstream file or archive entry unchanged. Do not copy installed system fonts.

Keep both licenses with the application. Original Space Grotesk donor assets
and preservation records remain in `assets/donors/ravefold/`; the application
does not load or ship them as native font assets.
