"""Prepare header PNGs and the Windows ICO from the canonical tribal mask PNG."""
from __future__ import annotations

import argparse
from io import BytesIO
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "local-data" / "icon-renderer"))

from PIL import Image


SIZES = (16, 24, 32, 48, 64, 128, 256)
DISPLAY_SIZES = (40, 60, 80)
DESTINATION = ROOT / "assets/app"
SOURCE = DESTINATION / "mixjam-techno.png"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    with Image.open(SOURCE) as image:
        if image.format != "PNG" or image.size != (256, 256) or image.mode != "RGBA":
            raise ValueError("The canonical tribal mask must be a 256 by 256 RGBA PNG")
        alpha = image.getchannel("A")
        if alpha.getextrema() != (0, 255) or not any(alpha.histogram()[1:255]):
            raise ValueError("The tribal mask needs transparent space and antialiased artwork")
        output = BytesIO()
        image.save(output, format="ICO", sizes=[(size, size) for size in SIZES])
        assets = {DESTINATION / "mixjam-techno.ico": output.getvalue()}
        for size in DISPLAY_SIZES:
            output = BytesIO()
            image.resize((size, size), Image.Resampling.LANCZOS).save(output, format="PNG")
            assets[DESTINATION / str(size) / "mixjam-techno.png"] = output.getvalue()
    for path, data in assets.items():
        if args.check:
            if path.read_bytes() != data:
                raise ValueError(f"{path.relative_to(DESTINATION)} differs from the canonical tribal mask PNG")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    with Image.open(DESTINATION / "mixjam-techno.ico") as icon:
        if icon.ico.sizes() != {(size, size) for size in SIZES}:
            raise ValueError("The Windows ICO does not contain every required size")
    print(f"Tribal mask PNG and ICO verified; ICO sizes: {SIZES}; display sizes: {DISPLAY_SIZES}")


if __name__ == "__main__":
    main()
