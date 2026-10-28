"""How alike drum loops are: drumfamily.py [--bpm N] FILE...

python -I tools/song/drumfamily.py [--bpm 140] FILE...

Per pair: the correlation of their onset patterns on a 16th grid (rhythm, from the energy rise in
three bands: kick below 150 Hz, snare/clap 150-2500 Hz, hats above 5 kHz) and the distance of their
band balance (tone, in dB). Loops are compared over their bar-length cycles at --bpm, their own
tempo (default 140), so give loops of one tempo at a time. Prints, per loop, its band balance, then
the pair matrix. Use it to choose a drum family whose members alternate without a change of style.
"""
import itertools
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mixjam import read_wav  # noqa: E402

BANDS = [(20, 150), (150, 2500), (5000, 16000)]


def profile(mono, rate, bpm):
    """(onset pattern over three bands x 16 steps, band balance in dB under the loudest band)."""
    step = int(round(rate * 60 / bpm / 4))  # one 16th
    bars = len(mono) // (step * 16)
    if bars == 0:
        raise ValueError(f'shorter than one bar at {bpm:g} BPM')
    mono = mono[: bars * step * 16]
    spectrum = np.fft.rfft(mono)
    freqs = np.fft.rfftfreq(len(mono), 1 / rate)
    onsets, balance = [], []
    for low, high in BANDS:
        band = np.fft.irfft(np.where((freqs >= low) & (freqs < high), spectrum, 0), n=len(mono))
        energy = (band ** 2).reshape(-1, step).sum(axis=1)
        balance.append(10 * np.log10(energy.sum() + 1e-20))
        cycle = energy.reshape(bars, 16).mean(axis=0)
        rise = np.maximum(np.diff(np.concatenate([[cycle[-1]], cycle])), 0)
        onsets.append(rise / (rise.max() + 1e-20))
    balance = np.array(balance)
    return np.concatenate(onsets), balance - balance.max()


def main():
    args = sys.argv[1:]
    bpm = 140.0
    if '--bpm' in args:
        i = args.index('--bpm')
        bpm = float(args[i + 1])
        del args[i:i + 2]
    if len(args) < 2:
        raise SystemExit(__doc__)
    profiles = {}
    for path in args:
        rate, data = read_wav(path)
        profiles[Path(path).stem] = profile(data.mean(axis=1), rate, bpm)
    for name, (_, balance) in profiles.items():
        print(f'{name:28s} bands (kick, snare, hats) dB: ' + ' '.join(f'{b:6.1f}' for b in balance))
    print()
    for a, b in itertools.combinations(profiles, 2):
        rhythm = np.corrcoef(profiles[a][0], profiles[b][0])[0, 1]
        tone = np.sqrt(np.mean((profiles[a][1] - profiles[b][1]) ** 2))
        print(f'{a:28s} {b:28s} rhythm {rhythm:5.2f}  tone {tone:4.1f} dB')


if __name__ == '__main__':
    main()
