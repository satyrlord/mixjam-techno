"""Prepare fixed-scale Folded Metal material slices from the generated chassis.

The active screw-free source is retained privately as chassis-screwless.png.
This reproducible asset preparation crops and recolors real source material;
it does not synthesize substitute scratches or render a complete window skin.
"""
from pathlib import Path
from io import BytesIO
import argparse
import hashlib
import json
import math
import random
import statistics
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "local-data/artwork/folded-metal/chassis-screwless.png"
OUTPUT = ROOT / "assets/folded-metal"
PALETTES = {"utility": (60, 65, 64), "marine": (32, 83, 91),
            "granite": (52, 57, 60), "red": (154, 45, 39), "olive": (60, 68, 40)}
# The generated file includes a transparent surround and isolated faint pixels.
# This box follows the actual metal chassis, including its outer bevel.
CHASSIS_BOX = (30, 130, 1743, 755)


def save(image: Image.Image, path: Path, check: bool) -> None:
    buffer = BytesIO()
    image.save(buffer, format="PNG")
    data = buffer.getvalue()
    if check:
        if path.read_bytes() != data:
            raise ValueError(f"{path} differs from the prepared artwork.")
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)


def fade(position: float) -> float:
    t = max(0.0, min(1.0, (position - .35) / .65))
    return 1.0 - t * t * (3.0 - 2.0 * t)


def prepare(image: Image.Image, color: tuple[int, int, int], baseline: float,
            horizontal: bool, vertical: bool, unit: float,
            reverse_x=False, reverse_y=False) -> Image.Image:
    image = image.copy()
    pixels = image.load()
    for y in range(image.height):
        for x in range(image.width):
            red, green, blue, alpha = pixels[x, y]
            luminance = .2126 * red + .7152 * green + .0722 * blue
            # Preserve neutral worn bare metal, shadows and glints.
            if luminance < 55 or max(red, green, blue) - min(red, green, blue) < 20:
                shade = (round(luminance),) * 3
            else:
                delta = (luminance - baseline) * .75
                shade = tuple(max(0, min(255, round(channel + delta))) for channel in color)
            fx = (image.width - 1 - x if reverse_x else x) / max(1, image.width - 1)
            fy = (image.height - 1 - y if reverse_y else y) / max(1, image.height - 1)
            opacity = (fade(fx) if horizontal else 1) * (fade(fy) if vertical else 1)
            # Native antialiased geometry owns the continuous outside bevel.
            # Suppress the source silhouette and its fringe before introducing
            # material inside it. This also makes every corner/strip join fade
            # through the identical solid chassis color instead of a hard cut.
            def enter(distance: float) -> float:
                t = max(0.0, min(1.0, (distance / unit - 4.0) / 4.0))
                return t * t * (3.0 - 2.0 * t)
            if horizontal:
                opacity *= enter((image.width - 1 - x) if reverse_x else x)
            if vertical:
                opacity *= enter((image.height - 1 - y) if reverse_y else y)
            if horizontal != vertical:
                # A repeated strip also fades at both ends, so consecutive
                # native-size tiles cannot expose a discontinuous source seam.
                position, length = (y, image.height) if horizontal else (x, image.width)
                t = min(position, length-1-position) / (12.0 * unit)
                t = max(0.0, min(1.0, t))
                opacity *= t*t*(3.0-2.0*t)
            pixels[x, y] = (*shade, round(alpha * opacity))
    return image


def validate_material(image: Image.Image, unit: float,
                      horizontal: bool, vertical: bool, rx: bool, ry: bool) -> None:
    alpha = image.getchannel("A")
    width, height = image.size
    # Every edge is transparent, including repeated-strip endpoints. A faint
    # nonzero fringe here can become a visible seam over the native outline.
    for box in [(0,0,width,1), (0,height-1,width,height),
                (0,0,1,height), (width-1,0,width,height)]:
        if alpha.crop(box).getbbox() is not None:
            raise ValueError("Material boundary must have zero alpha.")
    margin = max(1, round(4*unit))
    if horizontal:
        box = (width-margin,0,width,height) if rx else (0,0,margin,height)
        if alpha.crop(box).getbbox() is not None:
            raise ValueError("Material fringe intrudes into the native side bevel.")
    if vertical:
        box = (0,height-margin,width,height) if ry else (0,0,width,margin)
        if alpha.crop(box).getbbox() is not None:
            raise ValueError("Material fringe intrudes into the native top/bottom bevel.")
    if alpha.getbbox() is None:
        raise ValueError("Material preparation removed all source detail.")


def render(check: bool) -> None:
    original = Image.open(SOURCE).convert("RGBA")
    chassis = original.crop(CHASSIS_BOX)
    patch = chassis.crop((500, 160, 700, 300))
    baseline = statistics.median(.2126*r + .7152*g + .0722*b for r, g, b, _ in patch.get_flattened_data())
    grain_stats = {}
    for density in (100, 150, 200):
        scale = density / 200
        material = chassis.convert("RGBa").resize(
            (round(chassis.width*scale), round(chassis.height*scale)),
            Image.Resampling.LANCZOS).convert("RGBA")
        unit = density / 100
        cw, ch = round(112*unit), round(64*unit)
        width, height = material.size
        specs = {
            "corner-tl": ((0, 0, cw, ch), True, True, False, False),
            "corner-tr": ((width-cw, 0, width, ch), True, True, True, False),
            "corner-bl": ((0, height-ch, cw, height), True, True, False, True),
            "corner-br": ((width-cw, height-ch, width, height), True, True, True, True),
            "edge-top": ((cw, 0, cw+round(192*unit), round(12*unit)), False, True, False, False),
            "edge-bottom": ((cw, height-round(12*unit), cw+round(192*unit), height), False, True, False, True),
            "edge-left": ((0, ch, round(12*unit), ch+round(96*unit)), True, False, False, False),
            "edge-right": ((width-round(12*unit), ch, width, ch+round(96*unit)), True, False, True, False),
        }
        grain_size = round(256*unit)
        rng = random.Random(41723 + density)
        # Quantized symmetric Gaussian pairs give an exactly zero-mean field.
        # Shuffle removes pairing patterns; the saved PNG is static at runtime.
        noise = []
        for _ in range(grain_size*grain_size//2):
            value = round(rng.gauss(0.0, 1.5))
            noise.extend((value, -value))
        rng.shuffle(noise)
        deviation = statistics.pstdev(noise)
        if statistics.mean(noise) != 0 or not 1.2 <= deviation <= 1.8:
            raise ValueError("Paint grain must be zero-mean with restrained finite variance.")
        grain_stats[str(density)] = {"width": grain_size, "height": grain_size,
            "mean": statistics.mean(noise), "standard_deviation": deviation,
            "minimum": min(noise), "maximum": max(noise)}
        for name, color in PALETTES.items():
            for part, (box, horizontal, vertical, rx, ry) in specs.items():
                prepared = prepare(material.crop(box), color, baseline, horizontal, vertical, unit, rx, ry)
                validate_material(prepared, unit, horizontal, vertical, rx, ry)
                save(prepared, OUTPUT / name / str(density) / f"{part}.png", check)
            grain = Image.new("RGBA", (grain_size, grain_size))
            grain.putdata([(*(channel+delta for channel in color), 255) for delta in noise])
            save(grain, OUTPUT / name / str(density) / "paint-grain.png", check)
        # Prepare each physical-size cap from an 8x supersampled original.
        # The one-time premultiplied-alpha reduction smooths silhouettes, ribs,
        # and metal grain without introducing runtime texture scaling.
        size = round(36*unit)
        supersample = 8
        render_size = size * supersample
        render_unit = unit * supersample
        knob = Image.new("RGBA", (render_size, render_size))
        center = (render_size-1)/2
        for y in range(render_size):
            for x in range(render_size):
                dx, dy = (x-center)/render_unit, (y-center)/render_unit
                radius = math.hypot(dx,dy)
                if radius > 17.8: continue
                light = (-dx-dy)/36
                if radius > 16: level = 24+light*22
                elif radius > 11.8: level = 44+light*30+math.cos(math.atan2(dy,dx)*36)*6
                else: level = 184+light*55+math.sin(y/render_unit*4)*3
                level = max(0,min(255,round(level)))
                knob.putpixel((x,y),(level,level,max(0,level-3),255))
        knob = knob.convert("RGBa").resize((size, size), Image.Resampling.LANCZOS).convert("RGBA")
        save(knob, OUTPUT / "controls" / str(density) / "knob.png", check)
        # Fixed grain tile for the light primary face; native geometry owns edges.
        brushed = Image.new("RGBA", (round(48*unit), round(24*unit)))
        for y in range(brushed.height):
            for x in range(brushed.width):
                grain = round(4*math.sin(y*2.4/unit) + 2*math.sin((x+y)*.37/unit))
                brushed.putpixel((x,y), (max(0,212+grain), max(0,205+grain), max(0,182+grain), 255))
        save(brushed, OUTPUT / "controls" / str(density) / "primary.png", check)
    manifest = {"source": "local-data/artwork/folded-metal/chassis-screwless.png", "sha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
                "crop": CHASSIS_BOX, "palettes": PALETTES, "variants": [100,150,200], "source_material": "generated screw-free Folded Metal chassis",
                "perimeter": "continuous native antialiased bevel; source material fades at all boundaries",
                "paint_grain": {"distribution": "quantized Gaussian, symmetric shuffled pairs", "seed": "41723 + density",
                    "requested_standard_deviation": 1.5, "channel_policy": "identical additive RGB delta", "measured": grain_stats}}
    text = json.dumps(manifest, indent=2) + "\n"
    path = OUTPUT / "provenance.json"
    if check:
        if path.read_text() != text: raise ValueError("Folded Metal provenance differs.")
    else: path.write_text(text)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    render(parser.parse_args().check)
    print("PASS: Folded Metal assets and source provenance match.")
