"""Prepare Effect Studio PNG skins and switches at the three display densities."""
from io import BytesIO
from pathlib import Path
import argparse
import hashlib
import json

from PIL import Image, PngImagePlugin

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / 'assets/effects'
SOURCES = {
    'panel': '4599df59374f5b06344fff1ae219551d5e99cd68faa5f8e1320a65475ce667c5',
    'collapsed': '54486d365aa0508da3e61339fe800892769ec541bdca365ba5077efeceb50258',
    'toggles': '8f0c6995397a3582c04a9a0f88f92110515823f2d79fc8e7c66e863f48b5882f',
    'knob': '8c0a9b81dd3dddf07e9e9bef25a65fd4c5f946882a4cad2112712bc1ce18005e',
}
ARTWORK = {
    'panel': ('panel', (640, 352), None),
    'collapsed': ('collapsed', (640, 48), None),
    'toggle-off': ('toggles', (24, 24), 0),
    'toggle-on': ('toggles', (24, 24), 1),
    'knob': ('knob', (32, 32), None),
}


def prepare_toggles():
    path = ROOT / 'local-data/artwork/effects/toggles-generated.png'
    if hashlib.sha256(path.read_bytes()).hexdigest() != '5079d3a47989c8d98ad521f4e4fc03bdef6a5bba19a1a9762e57e5a46b2da870':
        raise ValueError('The small-grid toggle generation does not match its recorded hash.')
    with Image.open(path) as source:
        image = source.convert('RGBA')
        cell = image.width // 2
        side = round(cell * 0.76)
        left = (cell - side) // 2
        top = (image.height - side) // 2
        sheet = Image.new('RGBA', (128, 64))
        for state in (0, 1):
            tile = image.crop((state * cell + left, top, state * cell + left + side, top + side))
            sheet.paste(tile.resize((64, 64), Image.Resampling.LANCZOS), (state * 64, 0))
    output = BytesIO()
    sheet.save(output, format='PNG', compress_level=9)
    return output.getvalue()


def render(name, density):
    source_name, size, cell = ARTWORK[name]
    source_path = ROOT / 'local-data/artwork/effects' / f'{source_name}.png'
    source_hash = SOURCES[source_name]
    if hashlib.sha256(source_path.read_bytes()).hexdigest() != source_hash:
        raise ValueError(f'Effect Studio {source_name} source does not match its recorded hash.')
    with Image.open(source_path) as source:
        image = source.convert('RGBA')
        if cell is None:
            bounds = image.getchannel('A').point(lambda alpha: 255 if alpha > 32 else 0).getbbox()
            if bounds is None:
                raise ValueError(f'Effect Studio {name} source has no visible artwork.')
            image = image.crop(bounds)
        else:
            # Equal cells retain the same socket pivot in both switch states.
            width = image.width // 2
            image = image.crop((cell * width, 0, (cell + 1) * width, image.height))
        # Restore solid metal and antialias the generated contour offline.
        image.putalpha(image.getchannel('A').point(lambda alpha: 255 if alpha >= 128 else 0))
        image = image.resize(tuple(axis * density // 100 for axis in size), Image.Resampling.LANCZOS)
    if name in ('panel', 'collapsed'):
        margin = 16 * density // 100
        interior = image.getchannel('A').crop((margin, margin, image.width - margin, image.height - margin))
        if interior.getextrema() != (255, 255):
            raise ValueError(f'Effect Studio {name} metal must be opaque.')
    prompts = json.loads((OUTPUT / 'prompts.json').read_text(encoding='utf-8'))
    metadata = PngImagePlugin.PngInfo()
    metadata.add_text('Description', f'Built-in OpenAI ImageGen Effect Studio {name}. Text, values, lamps and dial pointers are native.')
    metadata.add_text('Source', f'local-data/artwork/effects/{source_name}.png; SHA256: {source_hash}')
    metadata.add_text('Prompt', prompts[source_name])
    metadata.add_text('impeccable:prompt', prompts[source_name])
    output = BytesIO()
    image.save(output, format='PNG', pnginfo=metadata, compress_level=9)
    return output.getvalue()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    toggle_source = ROOT / 'local-data/artwork/effects/toggles.png'
    small_sheet = prepare_toggles()
    if args.check:
        if toggle_source.read_bytes() != small_sheet:
            raise ValueError('The 128 by 64-pixel toggle master differs from its preparation.')
    else:
        toggle_source.write_bytes(small_sheet)
    for name in ARTWORK:
        for density in (100, 150, 200):
            path = OUTPUT / str(density) / f'{name}.png'
            data = render(name, density)
            if args.check:
                if path.read_bytes() != data:
                    raise ValueError(f'{path} differs from the prepared artwork.')
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
    print('PASS: fifteen prepared Effect Studio PNGs match.' if args.check else 'Prepared fifteen Effect Studio PNGs.')


if __name__ == '__main__':
    main()
