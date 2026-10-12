"""Prepare compact mixer materials and a small fader at supported densities."""
from io import BytesIO
from pathlib import Path
import argparse
import hashlib
import json

from PIL import Image, ImageFilter, ImageStat, PngImagePlugin

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / 'assets/mixer'
SOURCES = {
    'materials': '284bfd4d9ce3360625e9238afb3ea77b449c1a62d100771646c116de62bb2a48',
    'fader': '70cf045f7a6207500525aaefcf648d6e4c674902ca2c6bc123de534591d2d152',
}


def render(name, density):
    source_name = 'fader' if name == 'fader' else 'materials'
    path = ROOT / 'local-data/artwork/mixer' / f'{source_name}.png'
    if hashlib.sha256(path.read_bytes()).hexdigest() != SOURCES[source_name]:
        raise ValueError(f'Mixer {source_name} source does not match its recorded hash.')
    with Image.open(path) as original:
        image = original.convert('RGBA')
        if name == 'fader':
            bounds = image.getchannel('A').point(lambda value: 255 if value > 32 else 0).getbbox()
            image = image.crop(bounds)
            image.putalpha(image.getchannel('A').point(lambda value: 255 if value >= 128 else 0))
            # Resolve the tiny control on its 200% grid before preparing lower densities.
            image = image.resize((48, 64), Image.Resampling.LANCZOS)
            size = (24, 32)
        else:
            half = image.width // 2
            x = 0 if name == 'ivory' else half
            image = image.crop((x, 0, x + half, image.height))
            image = image.resize((216, 128), Image.Resampling.LANCZOS)
            # Remove broad lighting before repetition. Retain only the generated
            # material grain so adjacent tiles do not form a visible checkerboard.
            base = ImageStat.Stat(image).mean
            lighting = image.filter(ImageFilter.GaussianBlur(16))
            image.putdata([
                tuple(round(max(0, min(255, base[channel] + (pixel[channel] - shade[channel]) * 0.65)))
                      for channel in range(3)) + (255,)
                for pixel, shade in zip(image.get_flattened_data(), lighting.get_flattened_data())
            ])
            size = (108, 64)
        image = image.resize(tuple(axis * density // 100 for axis in size), Image.Resampling.LANCZOS)
    prompt = json.loads((OUTPUT / 'prompts.json').read_text(encoding='utf-8'))[source_name]
    metadata = PngImagePlugin.PngInfo()
    metadata.add_text('Description', f'Built-in OpenAI ImageGen compact mixer {name}. Labels and rail marks are native.')
    metadata.add_text('Source', f'local-data/artwork/mixer/{source_name}.png; SHA256: {SOURCES[source_name]}')
    metadata.add_text('Prompt', prompt)
    metadata.add_text('impeccable:prompt', prompt)
    result = BytesIO()
    image.save(result, format='PNG', pnginfo=metadata, compress_level=9)
    return result.getvalue()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    for name in ('ivory', 'graphite', 'fader'):
        for density in (100, 150, 200):
            path = OUTPUT / str(density) / f'{name}.png'
            data = render(name, density)
            if args.check:
                if path.read_bytes() != data:
                    raise ValueError(f'{path} differs from its prepared artwork.')
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
    print('PASS: nine prepared mixer PNGs match.' if args.check else 'Prepared nine mixer PNGs.')


if __name__ == '__main__':
    main()
