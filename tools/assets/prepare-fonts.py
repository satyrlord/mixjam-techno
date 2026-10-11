"""Verify bundled native fonts offline and write their provenance manifest.

Requires fonttools. Run with --check to verify the manifest without writing.
The original font and license files are copied unchanged from pinned releases.
"""
from __future__ import annotations

import argparse
from hashlib import sha256
import json
from pathlib import Path

from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[2]
DESTINATION = ROOT / "assets/fonts"
ASCII = set(range(32, 127))
SYNCOPATE_SOURCE = "https://raw.githubusercontent.com/google/fonts/bd8f81ddb5c74d5c8897b36ad88b440266245103/apache/syncopate/"
FONTS = (
    {
        "file": "Syncopate-Bold.ttf",
        "family": "Syncopate",
        "weight": 700,
        "sha256": "c71f5a40a83115518443c13474e5132804f4a91ce26e26fd9718cd3814ad4b2e",
        "source": SYNCOPATE_SOURCE + "Syncopate-Bold.ttf",
        "license": "LICENSE-Syncopate.txt",
        "license_sha256": "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30",
        "license_source": SYNCOPATE_SOURCE + "LICENSE.txt",
    },
    {
        "file": "Inter-Medium.ttf",
        "family": "Inter Medium",
        "weight": 500,
        "sha256": "97ad806f526e41546d46365bb3a393145f75b7b1568913db74549ad8b8dba872",
        "source": "https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip",
        "archive_path": "extras/ttf/Inter-Medium.ttf",
        "license": "LICENSE-Inter.txt",
        "license_sha256": "262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a",
        "license_archive_path": "LICENSE.txt",
    },
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    records = []
    coverage = {}
    for record in FONTS:
        for filename, digest in ((record["file"], record["sha256"]),
                                 (record["license"], record["license_sha256"])):
            path = DESTINATION / filename
            if not path.is_file() or sha256(path.read_bytes()).hexdigest() != digest:
                raise SystemExit(f"Bundled font asset differs: {path.relative_to(ROOT)}")
        path = DESTINATION / record["file"]
        with TTFont(path) as font:
            if "fvar" in font or font["OS/2"].usWeightClass != record["weight"]:
                raise SystemExit(f"Unexpected native font weight: {record['file']}")
            if font["name"].getDebugName(1) != record["family"]:
                raise SystemExit(f"Unexpected native font family: {record['file']}")
            coverage[record["family"]] = set(font.getBestCmap())
        if not ASCII <= coverage[record["family"]]:
            raise SystemExit(f"Missing ASCII characters: {record['file']}")
        records.append(dict(record, bytes=path.stat().st_size))
    data = {
        "files": records,
        "title_case": "Latin transliteration and uppercase presentation; saved text retains its original spelling",
        "title_fallback": "egui proportional fallbacks for symbols; title letters use Latin counterparts",
    }
    encoded = (json.dumps(data, indent=2, ensure_ascii=False) + "\n").encode()
    manifest = DESTINATION / "manifest.json"
    if args.check:
        if not manifest.is_file() or manifest.read_bytes() != encoded:
            raise SystemExit("Native font manifest differs: assets/fonts/manifest.json")
    else:
        manifest.write_bytes(encoded)
    print("Verified Syncopate Bold 700 and Inter Medium 500, licenses and ASCII title coverage.")


if __name__ == "__main__":
    main()
