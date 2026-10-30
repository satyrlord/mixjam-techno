"""Prepare rail display PNGs and check their sources, manifest, and variants."""
from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
from io import BytesIO
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "assets" / "rail"
RENDERER = ROOT / "local-data" / "icon-renderer"
sys.path.insert(0, str(RENDERER))

from PIL import Image, ImageDraw, ImageFont


SIZE = (96, 96)
DISPLAY_SIZES = (20, 30, 40)
INK = (5, 9, 42)
NAMES = (
    "open", "save", "save-as", "export", "import", "libraries", "new", "mixer", "master",
    "erase", "delete", "effects", "step-synth", "edit", "stretch", "record",
    "undo", "redo", "help", "album-review",
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_png(name: str) -> dict:
    path = ASSETS / f"{name}.png"
    with Image.open(path) as source:
        if source.format != "PNG" or source.size != SIZE or source.mode != "RGBA":
            raise ValueError(f"{name}: expected a 96 by 96 RGBA PNG")
        # Pillow before 11.3 has only getdata; later versions deprecate it.
        pixels = list(getattr(source, "get_flattened_data", source.getdata)())
        visible = [pixel for pixel in pixels if pixel[3] > 0]
        opaque = [pixel for pixel in pixels if pixel[3] == 255]
        transparent = sum(pixel[3] == 0 for pixel in pixels)
        antialiased = sum(0 < pixel[3] < 255 for pixel in pixels)
        if not visible or not opaque or not transparent or not antialiased:
            raise ValueError(f"{name}: expected opaque artwork, transparency, and antialiased edges")
        if any(pixel[:3] != INK for pixel in visible):
            raise ValueError(f"{name}: all visible artwork must use #05092a")
        bounds = source.getchannel("A").getbbox()
        if not bounds or min(bounds[:2]) <= 0 or max(bounds[2:]) >= 96:
            raise ValueError(f"{name}: artwork touches an image edge")
    return {
        "name": name,
        "png_sha256": sha256(path),
        "size": list(SIZE),
        "mode": "RGBA",
        "visible_pixels": len(visible),
        "opaque_pixels": len(opaque),
        "transparent_pixels": transparent,
        "antialiased_pixels": antialiased,
        "alpha_bounds": list(bounds),
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


def variants(name: str, check: bool) -> list[dict]:
    result = []
    with Image.open(ASSETS / f"{name}.png") as source:
        for size in DISPLAY_SIZES:
            # Resize offline. Keep coverage and the exact tintable artwork color.
            alpha = source.getchannel("A").resize((size, size), Image.Resampling.LANCZOS)
            resized = Image.new("RGBA", (size, size), (*INK, 0))
            resized.putalpha(alpha)
            output = BytesIO()
            resized.save(output, format="PNG")
            data = output.getvalue()
            path = ASSETS / str(size) / f"{name}.png"
            if check:
                if path.read_bytes() != data:
                    raise ValueError(f"{name}: {size}-pixel variant differs from its canonical PNG")
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            result.append({"size": size, "png_sha256": hashlib.sha256(data).hexdigest()})
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true",
                        help="Check canonical PNG files and their manifest without writing")
    args = parser.parse_args()
    results = [check_png(name) for name in NAMES]
    if len({result["png_sha256"] for result in results}) != len(NAMES):
        raise ValueError("Two UI icons have identical PNG bytes")
    for result in results:
        result["variants"] = variants(result["name"], args.check)
    manifest = json.loads((ASSETS / "manifest.json").read_text(encoding="utf-8"))
    if args.check:
        if manifest.get("icons") != results:
            raise ValueError("The asset hashes or pixel checks differ from the manifest")
    else:
        manifest["pillow"] = importlib.metadata.version("Pillow")
        manifest["icons"] = results
        (ASSETS / "manifest.json").write_text(
            json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
        )
        contact_sheet()
    print(json.dumps({"checked_icons": len(results), "size": list(SIZE),
                      "transparent": True, "antialiased": True, "distinct": True,
                      "display_sizes": DISPLAY_SIZES, "variants": len(results) * len(DISPLAY_SIZES)}))


if __name__ == "__main__":
    main()
