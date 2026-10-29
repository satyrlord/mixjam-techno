"""Which lanes can be heard: audible.py SONG.mixjam OUT_DIR [--jobs N]

python -I tools/song/audible.py SONG.mixjam OUT_DIR

Renders every lane alone (with its twin, "Name (twin)", and its FX returns; the board off), N at a
time, and takes their sum as the whole premaster. Then, for each lane over the bars where it plays,
it prints:
- its level against the rest of the mix (dB);
- the share of 1/3-octave cells (100 Hz to 12 kHz, 186 ms frames) where it is within 6 dB of the
  rest of the mix or louder, so not masked: under 10% marks a lane that is probably drowned.
A sound that cannot be heard is a wasted lane. Lane names come from the song's `.meta.json`;
without it each used track is its own lane. render.py writes the same table as audible.txt from
the lane renders it makes anyway; run this tool alone only for a song that render.py did not render.
"""
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mixjam import TICKS_PER_BAR, read_wav  # noqa: E402
import render  # noqa: E402

BANDS = [100 * 2 ** (k / 3) for k in range(0, 28)]  # 100 Hz .. 12.7 kHz
DROWNED = 0.10


def cells(mono, rate):
    """Energy per 186 ms frame and 1/3-octave band."""
    frame = int(rate * 0.186)
    count = len(mono) // frame
    frames = mono[: count * frame].reshape(count, frame) * np.hanning(frame)
    spectrum = np.abs(np.fft.rfft(frames, axis=1)) ** 2
    freqs = np.fft.rfftfreq(frame, 1 / rate)
    out = np.zeros((count, len(BANDS) - 1))
    for k in range(len(BANDS) - 1):
        band = (freqs >= BANDS[k]) & (freqs < BANDS[k + 1])
        out[:, k] = spectrum[:, band].sum(axis=1)
    return out, frame


def heard_share(part_cells, rest_cells):
    """The share of the lane's active cells where it is within 6 dB of the rest or louder."""
    active = part_cells > 1e-9 * part_cells.max() if part_cells.size else part_cells
    if not active.any():
        return 0.0
    ratio = 10 * np.log10((part_cells + 1e-20) / (rest_cells + 1e-20))
    return float(np.mean((ratio > -6)[active]))


def lane_bars(document, tracks):
    """The 0-based bars where any clip of `tracks` plays."""
    bars = set()
    for index in tracks:
        for p in document['lanes'][index - 1]['placements']:
            first = p['startTick'] // TICKS_PER_BAR
            bars.update(range(first, (p['startTick'] + p['durationTicks'] - 1) // TICKS_PER_BAR + 1))
    return bars


def audibility(document, groups, rate, mix, part_of):
    """The table's lines: each lane's level against the rest and the share of it heard.

    `mix` is the whole premaster in mono; `part_of(name)` gives a lane's render in mono.
    """
    bar_frames = rate * 240 / document['song']['bpm']
    lines = [f'{"lane":22s} {"bars":>5s} {"vs rest dB":>10s} {"heard":>6s}']
    for name, tracks in sorted(groups.items(), key=lambda item: min(item[1])):
        bars = lane_bars(document, tracks)
        if not bars:
            continue
        part = part_of(name)
        part = np.pad(part, (0, max(0, len(mix) - len(part))))[: len(mix)]
        rest = mix - part
        mask = np.zeros(len(mix), bool)
        for bar in bars:
            mask[int(bar * bar_frames): int((bar + 1) * bar_frames)] = True
        level = 10 * np.log10((np.mean(part[mask] ** 2) + 1e-20) / (np.mean(rest[mask] ** 2) + 1e-20))
        part_cells, frame = cells(part, rate)
        rest_cells, _ = cells(rest, rate)
        frames = [i for i in range(len(part_cells)) if mask[min(len(mask) - 1, i * frame + frame // 2)]]
        heard = heard_share(part_cells[frames], rest_cells[frames])
        flag = '  <- drowned' if heard < DROWNED else ''
        lines.append(f'{name[:22]:22s} {len(bars):5d} {level:10.1f} {heard * 100:5.0f}%{flag}')
    return lines


def main():
    args = sys.argv[1:]
    if len(args) < 2 or args[0].startswith('-'):
        raise SystemExit(__doc__)
    song_path, out = Path(args[0]), Path(args[1])
    out.mkdir(parents=True, exist_ok=True)
    jobs = int(args[args.index('--jobs') + 1]) if '--jobs' in args else None
    document = json.loads(song_path.read_text(encoding='utf-8'))
    groups = render.lane_groups(document, render.read_meta(song_path))
    lanes = render.render_lanes(document, groups, out, jobs)
    rate, premaster, _ = render.mix_down(lanes, {})
    print('\n'.join(audibility(document, groups, rate, premaster.mean(axis=1),
                               lambda name: read_wav(lanes[name])[1].mean(axis=1))))


if __name__ == '__main__':
    main()
