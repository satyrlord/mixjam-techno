"""Trim a render's silent tail for delivery: trim.py IN.wav OUT.wav [--floor -90] [--tail 0.25]

python -I tools/song/trim.py r1/master.wav delivery.wav

The application renders the FX buses until they ring below 1e-6, so a master
can end with up to a minute of near-silence. The tool cuts the file --tail
seconds (default 0.25) after its last frame above --floor dBFS (default -90),
and fades those last --tail seconds out linearly. Use --tail 0 for no fade.
It keeps the format (16-, 24- or 32-bit PCM) and prints the old and new length.
It replaces the output file after the write succeeds. Other hard links keep
their original audio.
"""
import argparse
import math
import os
import sys
from pathlib import Path
from tempfile import NamedTemporaryFile
import wave

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from wavio import unpack_pcm24  # noqa: E402


def read(path):
    with wave.open(str(path), 'rb') as w:
        params = w.getparams()
        raw = w.readframes(params.nframes)
    width, channels = params.sampwidth, params.nchannels
    if width == 3:
        ints = unpack_pcm24(raw)
    elif width in (2, 4):
        ints = np.frombuffer(raw, '<i2' if width == 2 else '<i4')
    else:
        raise SystemExit(f'The sample width {width} in {path} is not supported.')
    return params, ints.reshape(-1, channels).astype(np.float64), float(1 << (8 * width - 1))


def write(path, params, data):
    ints = np.round(data).astype(np.int64)
    if params.sampwidth == 3:
        ints = ints.reshape(-1) & 0xFFFFFF
        raw = np.stack([ints & 0xFF, (ints >> 8) & 0xFF, (ints >> 16) & 0xFF], axis=1).astype(np.uint8).tobytes()
    else:
        raw = ints.astype('<i2' if params.sampwidth == 2 else '<i4').tobytes()
    path = Path(path)
    with NamedTemporaryFile(dir=path.parent, suffix='.tmp', delete=False) as temporary:
        staged = Path(temporary.name)
    try:
        with wave.open(str(staged), 'wb') as w:
            w.setparams(params)
            w.writeframes(raw)
        os.replace(staged, path)
    finally:
        staged.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--floor', type=float, default=-90.0)
    parser.add_argument('--tail', type=float, default=0.25)
    args = parser.parse_args()
    floor, tail = args.floor, args.tail
    if not math.isfinite(floor) or floor > 0:
        parser.error('--floor must be a finite value at or below 0 dBFS.')
    if not math.isfinite(tail) or tail < 0:
        parser.error('--tail must be a finite value at or above 0 seconds.')
    params, data, full = read(args.source)
    loud = np.nonzero(np.abs(data).max(axis=1) > full * 10 ** (floor / 20))[0]
    if not len(loud):
        raise SystemExit(f'{args.source} has no frame above {floor:g} dBFS.')
    q = int(min(tail, len(data) / params.framerate) * params.framerate)
    end = min(len(data), loud[-1] + 1 + q)
    out = data[:end].copy()
    q = min(q, end)
    if q:
        out[end - q:] *= np.linspace(1, 0, q + 1)[1:, None]
    write(args.output, params, out)
    print(f'{len(data) / params.framerate:.2f} s -> {end / params.framerate:.2f} s')


if __name__ == '__main__':
    main()
