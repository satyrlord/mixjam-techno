"""Notes per bar of a sound as it plays in the song: barnotes.py [--song-bpm N] PATH BPM CENTS [...]

python -I tools/song/barnotes.py [--song-bpm 140] PATH BPM CENTS [PATH BPM CENTS ...]

BPM is the sound's own tempo and CENTS the clip pitch. Each bar (at the sound's own tempo) gets its
strongest pitch classes from spectral peaks, after the total shift (varispeed to the song tempo,
default 140, plus the clip's cents), and its lowest strong note (the root candidate, 40-400 Hz).
Use it for a WAV without a MIDI file.
"""
import math
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from harmony import NOTES  # noqa: E402
from mixjam import read_wav  # noqa: E402


def shift_semitones(source_bpm, song_bpm, cents):
    """The pitch shift in semitones of a sound varispeeded from source_bpm to song_bpm, plus cents."""
    return (cents + 1200 * math.log2(song_bpm / source_bpm)) / 100


def bar_notes(segment, rate, shift):
    """(strongest pitch classes, root candidate) of one bar."""
    spectrum = np.abs(np.fft.rfft(segment * np.hanning(len(segment))))
    freqs = np.fft.rfftfreq(len(segment), 1 / rate)
    peaks = np.where((spectrum[1:-1] > spectrum[:-2]) & (spectrum[1:-1] > spectrum[2:]))[0] + 1
    band = (freqs[peaks] >= 60) & (freqs[peaks] <= 2500)
    peaks = peaks[band]
    if len(peaks) == 0:
        return '', ''
    peaks = peaks[spectrum[peaks] > spectrum[peaks].max() * 0.08]
    midi = 69 + 12 * np.log2(freqs[peaks] / 440) + shift
    chroma = np.zeros(12)
    np.add.at(chroma, np.round(midi).astype(int) % 12, spectrum[peaks])
    top = [NOTES[i] for i in np.argsort(chroma)[::-1][:4] if chroma[i] > chroma.max() * 0.25]
    low = [m for m, f, a in zip(midi, freqs[peaks], spectrum[peaks]) if 40 <= f <= 400 and a > spectrum[peaks].max() * 0.25]
    root = NOTES[int(round(min(low))) % 12] if low else ''
    return ' '.join(top), root


def main():
    args = sys.argv[1:]
    song_bpm = 140.0
    if '--song-bpm' in args:
        i = args.index('--song-bpm')
        song_bpm = float(args[i + 1])
        del args[i:i + 2]
    if not args or len(args) % 3:
        raise SystemExit(__doc__)
    for path, bpm, cents in zip(args[::3], args[1::3], args[2::3]):
        bpm, cents = float(bpm), float(cents)
        shift = shift_semitones(bpm, song_bpm, cents)
        rate, data = read_wav(path)
        mono = data.mean(axis=1)
        bar = int(rate * 240 / bpm)
        print(f'{Path(path).name}  (shift {shift:+.2f} st)')
        for i in range(len(mono) // bar):
            top, root = bar_notes(mono[i * bar:(i + 1) * bar], rate, shift)
            print(f'  bar {i + 1}: root {root:3s} notes {top}')


if __name__ == '__main__':
    main()
