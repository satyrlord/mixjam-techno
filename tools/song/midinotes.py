"""Notes per bar from Standard MIDI files: midinotes.py SEMITONES FILE.mid [FILE.mid ...]

python -I tools/song/midinotes.py SEMITONES FILE.mid [FILE.mid ...]

Transposes by SEMITONES (for example +1 to move A-flat minor to A minor) and prints, per bar, the
lowest note and the pitch classes with their total held beats. Bars are 4/4 at the file's PPQ.
Many sample packs ship a MIDI file beside each melodic loop; this reads what the loop plays.
"""
import struct
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from harmony import NOTES  # noqa: E402


def read_vlq(data, i):
    value = 0
    while True:
        byte = data[i]
        i += 1
        value = (value << 7) | (byte & 0x7F)
        if byte < 0x80:
            return value, i


def notes(path):
    """([(start tick, stop tick, MIDI note)], PPQ) of every note in a Standard MIDI file."""
    data = Path(path).read_bytes()
    if data[:4] != b'MThd':
        raise ValueError(f'{path}: not a MIDI file')
    _, _, tracks, ppq = struct.unpack('>IHHH', data[4:14])
    i, found = 14, []
    for _ in range(tracks):
        if data[i:i + 4] != b'MTrk':
            break
        length = struct.unpack('>I', data[i + 4:i + 8])[0]
        j, end, tick, status, held = i + 8, i + 8 + length, 0, 0, {}
        while j < end:
            delta, j = read_vlq(data, j)
            tick += delta
            byte = data[j]
            if byte == 0xFF:
                length2, k = read_vlq(data, j + 2)
                j = k + length2
                continue
            if byte in (0xF0, 0xF7):
                length2, k = read_vlq(data, j + 1)
                j = k + length2
                continue
            if byte & 0x80:
                status = byte
                j += 1
            kind = status & 0xF0
            if kind in (0x80, 0x90):
                note, velocity = data[j], data[j + 1]
                j += 2
                voice = (status & 0x0F, note)
                if kind == 0x90 and velocity > 0:
                    held[voice] = tick
                elif voice in held:
                    found.append((held.pop(voice), tick, note))
            elif kind in (0xC0, 0xD0):
                j += 1
            else:
                j += 2
        i = end
    return found, ppq


def per_bar(found, ppq, shift=0):
    """{bar (0-based): {MIDI note + shift: held beats}} for 4/4 bars."""
    bar_ticks = ppq * 4
    bars = defaultdict(lambda: defaultdict(float))
    for start, stop, note in found:
        for bar in range(start // bar_ticks, max(stop - 1, start) // bar_ticks + 1):
            a, b = max(start, bar * bar_ticks), min(stop, (bar + 1) * bar_ticks)
            if b > a:
                bars[bar][note + shift] += (b - a) / ppq
    return bars


def pitch_classes(bar):
    """{pitch class: held beats} of one bar from per_bar()."""
    out = defaultdict(float)
    for note, beats in bar.items():
        out[note % 12] += beats
    return dict(out)


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    shift = int(sys.argv[1])
    for path in sys.argv[2:]:
        found, ppq = notes(path)
        bars = per_bar(found, ppq, shift)
        print(Path(path).name)
        for bar in sorted(bars):
            pcs = sorted(pitch_classes(bars[bar]).items(), key=lambda kv: -kv[1])
            low = min(bars[bar])
            print(f'  bar {bar + 1}: low {NOTES[low % 12]}{low // 12 - 1}  '
                  + ' '.join(f'{NOTES[pc]}:{d:.1f}' for pc, d in pcs))


if __name__ == '__main__':
    main()
