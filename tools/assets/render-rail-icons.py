"""Render and check the native UI's SVG sources and transparent PNG assets."""
from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
from io import BytesIO
import json
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "assets" / "rail"
RENDERER = ROOT / "local-data" / "icon-renderer"
sys.path.insert(0, str(RENDERER))

from PIL import Image, ImageDraw, ImageFont


SIZE = (96, 96)
INK = (5, 9, 42)
NAMES = (
    "open", "save", "save-as", "export", "import", "libraries", "new", "mixer",
    "erase", "delete", "effects", "step-synth", "edit", "stretch", "record", "mix",
    "undo", "redo",
)
EXACT_DONOR_PATHS = {
    "save": "save",
    "export": "export",
    "mixer": "mixer",
    "erase": "erase",
    "delete": "trash",
    "effects": "inspector",
    "undo": "undo",
    "redo": "redo",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_source(name: str) -> None:
    source = ET.parse(ASSETS / f"{name}.svg").getroot()
    expected = {
        "width": "96", "height": "96", "viewBox": "0 0 24 24",
        "fill": "none", "stroke": "#05092a", "stroke-width": "1.7",
        "stroke-linecap": "round", "stroke-linejoin": "round",
    }
    for key, value in expected.items():
        if source.get(key) != value:
            raise ValueError(f"{name}: expected {key}={value}")
    paths = source.findall("{http://www.w3.org/2000/svg}path")
    if len(paths) != 1 or not paths[0].get("d"):
        raise ValueError(f"{name}: expected one nonempty path")
    donor_name = EXACT_DONOR_PATHS.get(name)
    if donor_name:
        donor = ROOT / "assets/donors/ravefold/icons/tracker-icons.tsx"
        marker = f'{donor_name}: "{paths[0].get("d")}",'
        if marker not in donor.read_text(encoding="utf-8"):
            raise ValueError(f"{name}: path differs from preserved {donor_name}")


def check_png(name: str) -> dict:
    path = ASSETS / f"{name}.png"
    with Image.open(path) as source:
        if source.size != SIZE or source.mode != "RGBA":
            raise ValueError(f"{name}: expected a 96 by 96 RGBA PNG")
        pixels = list(source.get_flattened_data())
        visible = [pixel for pixel in pixels if pixel[3] > 0]
        opaque = [pixel for pixel in pixels if pixel[3] == 255]
        transparent = sum(pixel[3] == 0 for pixel in pixels)
        if not visible or not opaque or not transparent:
            raise ValueError(f"{name}: expected artwork and transparency")
        if any(pixel[:3] != INK for pixel in visible):
            raise ValueError(f"{name}: all visible artwork must use #05092a")
        bounds = source.getchannel("A").getbbox()
        if not bounds or min(bounds[:2]) <= 0 or max(bounds[2:]) >= 96:
            raise ValueError(f"{name}: artwork touches an image edge")
    return {
        "name": name,
        "svg_sha256": sha256(ASSETS / f"{name}.svg"),
        "png_sha256": sha256(path),
        "size": list(SIZE),
        "mode": "RGBA",
        "visible_pixels": len(visible),
        "opaque_pixels": len(opaque),
        "transparent_pixels": transparent,
        "alpha_bounds": list(bounds),
        "preserved_path": EXACT_DONOR_PATHS.get(name),
    }


def contact_sheet() -> None:
    cell_width, cell_height = 200, 146
    rows = (len(NAMES) + 3) // 4
    sheet = Image.new("RGB", (4 * cell_width, rows * cell_height), "#bed6eb")
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.load_default(size=14)
    small_font = ImageFont.load_default(size=11)
    for index, name in enumerate(NAMES):
        x, y = index % 4 * cell_width, index // 4 * cell_height
        draw.rectangle((x, y, x + cell_width - 1, y + cell_height - 1),
                       outline="#5d7faa", width=1)
        with Image.open(ASSETS / f"{name}.png") as icon:
            sheet.paste(icon, (x + 14, y + 8), icon)
            native = icon.resize((24, 24), Image.Resampling.LANCZOS)
            sheet.paste(native, (x + 144, y + 44), native)
        draw.text((x + 140, y + 78), "24 px", font=small_font, fill=INK)
        draw.text((x + 16, y + 116), name, font=font, fill=INK)
    sheet.save(ASSETS / "contact-sheet.png")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true",
                        help="Check existing source and PNG files without writing")
    args = parser.parse_args()
    for name in NAMES:
        check_source(name)
    if not args.check:
        import resvg_py
        for name in NAMES:
            data = resvg_py.svg_to_bytes(
                svg_string=(ASSETS / f"{name}.svg").read_text(encoding="utf-8"),
                width=SIZE[0], height=SIZE[1],
                skip_system_fonts=True,
            )
            # Keep resvg's coverage. Use exact ink RGB at antialias edges to
            # remove rounding from its premultiplied-colour conversion.
            with Image.open(BytesIO(data)) as rendered:
                icon = Image.new("RGBA", SIZE, (*INK, 0))
                icon.putalpha(rendered.getchannel("A"))
                icon.save(ASSETS / f"{name}.png")
    results = [check_png(name) for name in NAMES]
    if len({result["png_sha256"] for result in results}) != len(NAMES):
        raise ValueError("Two UI icons have identical PNG bytes")
    if args.check:
        manifest = json.loads((ASSETS / "manifest.json").read_text(encoding="utf-8"))
        if manifest.get("icons") != results:
            raise ValueError("The asset hashes or pixel checks differ from the manifest")
    else:
        manifest = {
            "renderer": f"resvg-py {importlib.metadata.version('resvg-py')}",
            "pillow": importlib.metadata.version("Pillow"),
            "view_box": [0, 0, 24, 24], "stroke_width": 1.7,
            "stroke_colour": "#05092a", "icons": results,
        }
        (ASSETS / "manifest.json").write_text(
            json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
        )
        contact_sheet()
    print(json.dumps({"checked_icons": len(results), "size": list(SIZE),
                      "transparent": True, "distinct": True,
                      "preserved_paths": len(EXACT_DONOR_PATHS)}))


if __name__ == "__main__":
    main()
