"""Prepare Step synth chassis skins at the three supported display densities."""
from io import BytesIO
from pathlib import Path
import argparse
import hashlib
import json

from PIL import Image, PngImagePlugin

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / 'assets/step-synth'
ARTWORK = {
    'panel': ((820, 556), 'f1e0d249def2791b68a3d91eaffa9c42c2a7b93f76abc207f6270720ea25a646'),
    'collapsed': ((820, 48), '333d135a65fba75f4641e46a9149e3f6696f18487da3e22052fca1126c4aef53'),
}


def render(name, density):
    size, expected_hash = ARTWORK[name]
    source_path = ROOT / 'local-data/artwork/step-synth' / f'{name}.png'
    if hashlib.sha256(source_path.read_bytes()).hexdigest() != expected_hash:
        raise ValueError(f'Step synth {name} source does not match its recorded hash.')
    with Image.open(source_path) as source:
        image = source.convert('RGBA')
        bounds = image.getchannel('A').point(lambda alpha: 255 if alpha > 32 else 0).getbbox()
        if bounds is None:
            raise ValueError(f'Step synth {name} source has no visible artwork.')
        image = image.crop(bounds)
        image.putalpha(image.getchannel('A').point(lambda alpha: 255 if alpha >= 128 else 0))
        image = image.resize(tuple(axis * density // 100 for axis in size), Image.Resampling.LANCZOS)
    margin = 16 * density // 100
    interior = image.getchannel('A').crop((margin, margin, image.width - margin, image.height - margin))
    if interior.getextrema() != (255, 255):
        raise ValueError(f'Step synth {name} metal must be opaque.')
    prompt = json.loads((OUTPUT / 'prompts.json').read_text(encoding='utf-8'))[name]
    metadata = PngImagePlugin.PngInfo()
    metadata.add_text('Description', f'Built-in OpenAI ImageGen Step synth {name} chassis. All controls, grid cells and lettering are native.')
    metadata.add_text('Source', f'local-data/artwork/step-synth/{name}.png; SHA256: {expected_hash}')
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
    print('PASS: six prepared Step synth PNGs match.' if args.check else 'Prepared six Step synth PNGs.')


if __name__ == '__main__':
    main()
