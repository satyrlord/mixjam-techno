"""How bright each sound is: bright.py PATTERN...

python -I tools/song/bright.py PATTERN...

A pattern is a path or a glob (`**` allowed). A relative pattern that matches nothing from the
current folder is tried under SAMPLES_FOLDER (see .env.example). Per file, darkest first: RMS in
dB, the share of energy above 5 kHz and above 8 kHz, and the level of the 6-12 kHz band against
the whole, so that harsh hi-hats can be found before they are placed.
"""
import glob
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import env  # noqa: E402
from mixjam import read_wav  # noqa: E402


def expand(pattern):
    """The files a pattern names, from the current folder or else from SAMPLES_FOLDER."""
    found = glob.glob(pattern, recursive=True)
    if found or Path(pattern).is_absolute():
        return found or [pattern]
    root = env.path('SAMPLES_FOLDER')
    return glob.glob(str(root / pattern), recursive=True) or [pattern]


def brightness(mono, rate):
    """(RMS dB, share above 5 kHz, share above 8 kHz, 6-12 kHz share)."""
    spectrum = np.abs(np.fft.rfft(mono)) ** 2
    freqs = np.fft.rfftfreq(len(mono), 1 / rate)
    total = spectrum.sum() + 1e-20
    above5 = spectrum[freqs > 5000].sum() / total
    above8 = spectrum[freqs > 8000].sum() / total
    band = spectrum[(freqs > 6000) & (freqs < 12000)].sum() / total
    rms = 10 * np.log10(np.mean(mono ** 2) + 1e-20)
    return float(rms), float(above5), float(above8), float(band)


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    paths = []
    for pattern in sys.argv[1:]:
        paths += expand(pattern)
    rows = []
    for path in sorted(set(paths)):
        rate, data = read_wav(path)
        rms, above5, above8, band = brightness(data.mean(axis=1), rate)
        rows.append((band, Path(path).stem, rms, above5, above8))
    for band, name, rms, above5, above8 in sorted(rows):
        print(f'{name:36s} rms {rms:6.1f} dB  >5k {above5 * 100:5.1f}%  >8k {above8 * 100:5.1f}%  '
              f'6-12k {10 * np.log10(band + 1e-20):6.1f} dB')


if __name__ == '__main__':
    main()
