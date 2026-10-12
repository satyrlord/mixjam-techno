"""Prepare the approved Time Stretcher artwork at the three display densities."""
from io import BytesIO
from pathlib import Path
import argparse
import hashlib

from PIL import Image, PngImagePlugin

ROOT = Path(__file__).resolve().parents[2]
ARTWORK = {
    'panel': ('time-stretcher-refined.png',
              '3bc70d0b0e00bc4ea3a775313ace61591e4ce5adc4415fcc1272b82829c4d184', 320),
    'collapsed': ('time-stretcher-collapsed.png',
                  'b3c4ee60af97123c4e429dd14289d06743c856655d8908e3f7c5be71af99fa64', 64),
}
OUTPUT = ROOT / 'assets/stretch'


def render(name, density):
    filename, source_sha256, base_height = ARTWORK[name]
    source_path = ROOT / 'local-data/artwork/stretch' / filename
    if hashlib.sha256(source_path.read_bytes()).hexdigest() != source_sha256:
        raise ValueError(f'Time Stretcher {name} source does not match its recorded hash.')
    width = 640 * density // 100
    height = base_height * density // 100
    with Image.open(source_path) as source:
        image = source.convert('RGBA')
        # Ignore faint generation noise when finding transparent canvas padding.
        # Keep the generated bezel contour, including its rounded corners.
        bounds = image.getchannel('A').point(lambda alpha: 255 if alpha > 32 else 0).getbbox()
        if bounds is None:
            raise ValueError(f'Time Stretcher {name} source has no visible artwork.')
        image = image.crop(bounds)
        # The generator leaves slight transparency even inside solid metal.
        # Restore opaque coverage, then antialias the original contour offline.
        image.putalpha(image.getchannel('A').point(lambda alpha: 255 if alpha >= 128 else 0))
        image = image.resize((width, height), Image.Resampling.LANCZOS)
    metadata = PngImagePlugin.PngInfo()
    metadata.add_text('Description', f'Built-in OpenAI ImageGen Time Stretcher {name} skin. Text, header icons, and dial pointers are native. Source SHA256: {source_sha256}')
    metadata.add_text('Source', f'local-data/artwork/stretch/{filename}; prompt: assets/stretch/prompt.txt')
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
    print('PASS: six prepared PNG density variants match.' if args.check else 'Prepared six PNG density variants.')


if __name__ == '__main__':
    main()
