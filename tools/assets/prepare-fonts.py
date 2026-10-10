"""Prepare native static TTF files from the preserved Space Grotesk WOFF2 files.

Requires fonttools[woff]==4.66.1 and Brotli==1.2.0. No network access is used.
Run without arguments to generate assets, or with --check to verify exact bytes.
"""
from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
import json
from pathlib import Path

import brotli
import fontTools
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "assets/donors/ravefold/fonts/space-grotesk"
DESTINATION = ROOT / "assets/fonts"
WEIGHTS = {"Regular": 400, "Medium": 500, "Bold": 700}
SUBSETS = ("latin", "latin-ext", "vietnamese")
REQUIRED_CHARACTERS = set(range(32, 127)) | {ord(c) for c in "ĂăÂâÎîȘșȚț"}


def convert(source: Path, style: str, weight: int) -> bytes:
    font = TTFont(source, recalcTimestamp=False)
    timestamp = font["head"].modified
    font = instantiateVariableFont(font, {"wght": weight}, inplace=True)
    font.flavor = None
    # Keep the family and static face names explicit after variable instancing.
    names = {
        1: "Space Grotesk",
        2: style,
        3: f"SpaceGrotesk-{style}-MixJamNative",
        4: f"Space Grotesk {style}",
        6: f"SpaceGrotesk-{style}",
        16: "Space Grotesk",
        17: style,
    }
    for name_id, value in names.items():
        font["name"].setName(value, name_id, 3, 1, 0x409)
    font["head"].modified = timestamp
    result = BytesIO()
    font.save(result)
    return result.getvalue()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if (fontTools.__version__, brotli.__version__) != ("4.66.1", "1.2.0"):
        raise SystemExit("Use fonttools[woff]==4.66.1 and Brotli==1.2.0 for reproducible assets.")
    outputs: dict[str, bytes] = {}
    records = []
    for style, weight in WEIGHTS.items():
        coverage: set[int] = set()
        for subset in SUBSETS:
            source = SOURCE / "files" / f"space-grotesk-{subset}-wght-normal.woff2"
            name = f"SpaceGrotesk-{style}-{subset}.ttf"
            data = convert(source, style, weight)
            font = TTFont(BytesIO(data))
            coverage.update(font.getBestCmap())
            assert "fvar" not in font and data[:4] == b"\x00\x01\x00\x00", name
            assert font["OS/2"].usWeightClass == weight, name
            assert font["name"].getDebugName(2) == style, name
            outputs[name] = data
            records.append({
                "file": name,
                "weight": weight,
                "subset": subset,
                "bytes": len(data),
                "sha256": sha256(data).hexdigest(),
                "source": source.relative_to(ROOT).as_posix(),
                "source_sha256": sha256(source.read_bytes()).hexdigest(),
            })
        assert REQUIRED_CHARACTERS <= coverage, f"Missing ASCII or Romanian characters in {style}."
    outputs["LICENSE"] = (SOURCE / "LICENSE").read_bytes()
    outputs["manifest.json"] = (json.dumps({
        "fonttools": fontTools.__version__,
        "brotli": brotli.__version__,
        "source_package": "@fontsource-variable/space-grotesk@5.3.0",
        "files": records,
    }, indent=2) + "\n").encode()
    if not args.check:
        DESTINATION.mkdir(parents=True, exist_ok=True)
    for name, data in outputs.items():
        path = DESTINATION / name
        if args.check:
            if not path.is_file() or path.read_bytes() != data:
                raise SystemExit(f"Native font asset differs: {path.relative_to(ROOT)}")
        else:
            path.write_bytes(data)
    print(f"{'Verified' if args.check else 'Prepared'} 9 static TTF faces; ASCII and Romanian coverage passed.")


if __name__ == "__main__":
    main()
