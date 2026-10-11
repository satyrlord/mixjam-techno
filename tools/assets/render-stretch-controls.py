"""Prepare the approved Time Stretcher artwork at the three display densities."""
from io import BytesIO
from pathlib import Path
import argparse
import hashlib

from PIL import Image, ImageDraw, PngImagePlugin

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'local-data/artwork/stretch/time-stretcher.png'
SOURCE_SHA256 = '2e727e71b30910fd12d43df475a84965d431d76f0304a3c39200c81006e016f6'
OUTPUT = ROOT / 'assets/stretch'


def render(density):
    if hashlib.sha256(SOURCE.read_bytes()).hexdigest() != SOURCE_SHA256:
        raise ValueError('Time Stretcher source artwork does not match its recorded hash.')
    width = 640 * density // 100
    height = width // 2
    with Image.open(SOURCE) as source:
        image = source.convert('RGB').resize((width, height), Image.Resampling.LANCZOS).convert('RGBA')
    # The module is opaque. Only its rounded outer corners expose the workspace.
    mask = Image.new('L', (width * 3, height * 3))
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, width * 3 - 1, height * 3 - 1), radius=round(20 * density / 100 * 3), fill=255)
    image.putalpha(mask.resize((width, height), Image.Resampling.LANCZOS))
    metadata = PngImagePlugin.PngInfo()
    metadata.add_text('Description', 'Built-in OpenAI ImageGen artwork based on the owner-supplied Time Stretcher mock. Text and dial pointers are native. Source SHA256: ' + SOURCE_SHA256)
    metadata.add_text('Source', 'local-data/artwork/stretch/time-stretcher.png; prompt: assets/stretch/prompt.txt')
    output = BytesIO()
    image.save(output, format='PNG', pnginfo=metadata, compress_level=9)
    return output.getvalue()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    for density in (100, 150, 200):
        path = OUTPUT / str(density) / 'panel.png'
        data = render(density)
        if args.check:
            if path.read_bytes() != data:
                raise ValueError(f'{path} differs from the prepared artwork.')
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    print('PASS: three prepared PNG density variants match.' if args.check else 'Prepared three PNG density variants.')


if __name__ == '__main__':
    main()
