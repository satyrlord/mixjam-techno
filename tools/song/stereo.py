"""Stereo and band profile of sounds: stereo.py [--bpm N] PATH...

python -I tools/song/stereo.py [--bpm 140] PATH...

Per file: length in bars at --bpm (default 140), the share below 120 Hz, the correlation above
120 Hz (1 is mono, near 0 is wide), and the side level against the mid above 120 Hz. Use it to
check that a pad or bass folds to mono safely before you place it.
"""
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mixjam import read_wav  # noqa: E402


def profile(data, rate):
    """(share below 120 Hz, correlation above 120 Hz, side over mid above 120 Hz in dB)."""
    if data.shape[1] == 1:
        data = np.repeat(data, 2, axis=1)
    spectrum = np.fft.rfft(data, axis=0)
    freqs = np.fft.rfftfreq(len(data), 1 / rate)
    low = (np.abs(spectrum[freqs < 120]) ** 2).sum() / ((np.abs(spectrum) ** 2).sum() + 1e-20)
    high = spectrum.copy()
    high[freqs < 120] = 0
    left, right = np.fft.irfft(high[:, 0], n=len(data)), np.fft.irfft(high[:, 1], n=len(data))
    corr = float(np.sum(left * right) / (np.sqrt(np.sum(left ** 2) * np.sum(right ** 2)) + 1e-20))
    mid, side = (left + right) / 2, (left - right) / 2
    side_db = 10 * np.log10((np.sum(side ** 2) + 1e-20) / (np.sum(mid ** 2) + 1e-20))
    return float(low), corr, float(side_db)


def main():
    args = sys.argv[1:]
    bpm = 140.0
    if '--bpm' in args:
        i = args.index('--bpm')
        bpm = float(args[i + 1])
        del args[i:i + 2]
    if not args:
        raise SystemExit(__doc__)
    for path in args:
        rate, data = read_wav(path)
        low, corr, side_db = profile(data, rate)
        bars = len(data) / rate / (240 / bpm)
        print(f'{Path(path).name:34s} {bars:5.2f} bars  low {low * 100:3.0f}%  corr>120 {corr:5.2f}  side {side_db:6.1f} dB')


if __name__ == '__main__':
    main()
