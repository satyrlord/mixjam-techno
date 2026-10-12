"""Prepare Sample Editor chassis skins at the three supported display densities."""
from io import BytesIO
from pathlib import Path
import argparse
import hashlib
import json

from PIL import Image, PngImagePlugin

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / 'assets/sample-editor'
ARTWORK = {
    'panel': ((640, 320), 'beda7f4c608f05b8cadad38b0d811d8de5eba130d1b7eec3b2dae0855f7a770e'),
    'collapsed': ((640, 48), '9e15db07d8978739274b3a314877128d77742cf9bf0de5a6f0810c29e2342cae'),
    'thumb': ((16, 28), 'b406554bb70f42d0f6c78255bc37470843ee8889913fddf52993532047034852'),
}


def render(name, density):
    size, expected_hash = ARTWORK[name]
    source_path = ROOT / 'local-data/artwork/sample-editor' / f'{name}.png'
    if hashlib.sha256(source_path.read_bytes()).hexdigest() != expected_hash:
        raise ValueError(f'Sample Editor {name} source does not match its recorded hash.')
    with Image.open(source_path) as source:
        image = source.convert('RGBA')
        bounds = image.getchannel('A').point(lambda alpha: 255 if alpha > 32 else 0).getbbox()
        if bounds is None:
            raise ValueError(f'Sample Editor {name} source has no visible artwork.')
        image = image.crop(bounds)
        image.putalpha(image.getchannel('A').point(lambda alpha: 255 if alpha >= 128 else 0))
        if name == 'thumb':
            image = image.resize((32, 56), Image.Resampling.LANCZOS)
        image = image.resize(tuple(axis * density // 100 for axis in size), Image.Resampling.LANCZOS)
    if name != 'thumb':
        margin = 16 * density // 100
        interior = image.getchannel('A').crop((margin, margin, image.width - margin, image.height - margin))
        if interior.getextrema() != (255, 255):
            raise ValueError(f'Sample Editor {name} metal must be opaque.')
    prompt = json.loads((OUTPUT / 'prompts.json').read_text(encoding='utf-8'))[name]
    metadata = PngImagePlugin.PngInfo()
    metadata.add_text('Description', f'Built-in OpenAI ImageGen Sample Editor {name} chassis. All labels, readouts, controls and focus states are native.')
    metadata.add_text('Source', f'local-data/artwork/sample-editor/{name}.png; SHA256: {expected_hash}')
    metadata.add_text('Prompt', prompt)
    metadata.add_text('impeccable:prompt', prompt)
    output = BytesIO()
    image.save(output, format='PNG', pnginfo=metadata, compress_level=9)
    return output.getvalue()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
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
    print('PASS: nine prepared Sample Editor PNGs match.' if args.check else 'Prepared nine Sample Editor PNGs.')


if __name__ == '__main__':
    main()
