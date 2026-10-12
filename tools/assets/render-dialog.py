"""Prepare a reusable hardware dialog bezel as nine fixed-scale raster pieces."""
from io import BytesIO
from pathlib import Path
import argparse
import hashlib
import json

from PIL import Image, ImageFilter, ImageStat, PngImagePlugin

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / 'assets/dialog'
SOURCE_HASH = '6b24831f1bd31035e70beed0596588dfd5d387823513fc7f37dbd019d8abcc98'
SLICES = {
    'top-left': (0, 0, 16, 64), 'top': (288, 0, 352, 64),
    'top-right': (624, 0, 640, 64),
    'left': (0, 160, 16, 224), 'body': (288, 160, 352, 224),
    'right': (624, 160, 640, 224),
    'bottom-left': (0, 368, 16, 384), 'bottom': (288, 368, 352, 384),
    'bottom-right': (624, 368, 640, 384),
}


def prepare():
    path = ROOT / 'local-data/artwork/dialog/chassis.png'
    if hashlib.sha256(path.read_bytes()).hexdigest() != SOURCE_HASH:
        raise ValueError('Dialog chassis source does not match its recorded hash.')
    with Image.open(path) as source:
        image = source.convert('RGBA')
        bounds = image.getchannel('A').point(lambda value: 255 if value > 32 else 0).getbbox()
        image = image.crop(bounds)
        image.putalpha(image.getchannel('A').point(lambda value: 255 if value >= 128 else 0))
        return image.resize((640, 384), Image.Resampling.LANCZOS)


def render(image, name, density):
    piece = image.crop(SLICES[name])
    if name in ('body', 'left', 'right', 'top', 'bottom'):
        # Preserve the rim profile but remove broad lighting along repeated axes.
        pixels = list(piece.get_flattened_data())
        lighting = list(piece.filter(ImageFilter.GaussianBlur(12)).get_flattened_data())
        base = ImageStat.Stat(piece).mean
        if name in ('left', 'right'):
            means = [tuple(sum(pixels[y * piece.width + x][c] for y in range(piece.height)) / piece.height
                           for c in range(3)) for x in range(piece.width)]
        elif name in ('top', 'bottom'):
            means = [tuple(sum(pixels[y * piece.width + x][c] for x in range(piece.width)) / piece.width
                           for c in range(3)) for y in range(piece.height)]
        else:
            means = [base]
        prepared = []
        for index, (pixel, shade) in enumerate(zip(pixels, lighting)):
            mean = means[index % piece.width] if name in ('left', 'right') else means[index // piece.width] if name in ('top', 'bottom') else means[0]
            prepared.append(tuple(round(max(0, min(255, mean[c] + (pixel[c] - shade[c]) * 0.4))) for c in range(3)) + (pixel[3],))
        piece.putdata(prepared)
    piece = piece.resize(tuple(axis * density // 100 for axis in piece.size), Image.Resampling.LANCZOS)
    prompt = json.loads((OUTPUT / 'prompts.json').read_text(encoding='utf-8'))['chassis']
    metadata = PngImagePlugin.PngInfo()
    metadata.add_text('Description', f'Built-in OpenAI ImageGen utility dialog chassis: {name}. All text and controls are native.')
    metadata.add_text('Source', f'local-data/artwork/dialog/chassis.png; SHA256: {SOURCE_HASH}')
    metadata.add_text('Prompt', prompt)
    metadata.add_text('impeccable:prompt', prompt)
    result = BytesIO()
    piece.save(result, format='PNG', pnginfo=metadata, compress_level=9)
    return result.getvalue()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    image = prepare()
    for name in SLICES:
        for density in (100, 150, 200):
            path = OUTPUT / str(density) / f'{name}.png'
            data = render(image, name, density)
            if args.check:
                if path.read_bytes() != data:
                    raise ValueError(f'{path} differs from its prepared artwork.')
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
    print('PASS: 27 prepared dialog PNGs match.' if args.check else 'Prepared 27 dialog PNGs.')


if __name__ == '__main__':
    main()
