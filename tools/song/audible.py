"""Which lanes can be heard: audible.py SONG.mixjam OUT_DIR

python -I tools/song/audible.py SONG.mixjam OUT_DIR

Renders every lane alone (with its twin, "Name (twin)", and its FX returns; the board off) and the
whole premaster, then, for each lane over the bars where it plays, prints:
- its level against the rest of the mix (dB);
- the share of 1/3-octave cells (100 Hz to 12 kHz, 186 ms frames) where it is within 6 dB of the
  rest of the mix or louder, so not masked: under 10% marks a lane that is probably drowned.
A sound that cannot be heard is a wasted lane. Lane names come from the song's `.meta.json`;
without it each used track is its own lane.
"""
import copy
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mixjam import TICKS_PER_BAR, read_wav  # noqa: E402
from render import board_off, run  # noqa: E402

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


def lane_groups(document, meta):
    """Lane name to its 1-based tracks; a twin joins its lane."""
    if meta:
        names = {int(k): v['name'] for k, v in meta['lanes'].items()}
    else:
        names = {i: f'track {i}' for i, lane in enumerate(document['lanes'], 1) if lane['placements']}
    groups = {}
    for track, name in names.items():
        groups.setdefault(name.replace(' (twin)', ''), []).append(track)
    return groups


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    song_path, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    document = board_off(json.loads(song_path.read_text(encoding='utf-8')))
    meta_path = song_path.with_suffix('.meta.json')
    meta = json.loads(meta_path.read_text(encoding='utf-8')) if meta_path.exists() else None
    groups = lane_groups(document, meta)
    whole = out / 'premaster.mixjam'
    whole.write_text(json.dumps(document), encoding='utf-8')
    run(whole, out / 'premaster.wav')
    rate, mix = read_wav(str(out / 'premaster.wav'))
    mix = mix.mean(axis=1)
    _, frame = cells(mix, rate)
    bar_frames = rate * 240 / document['song']['bpm']
    print(f'{"lane":22s} {"bars":>5s} {"vs rest dB":>10s} {"heard":>6s}')
    for name, tracks in sorted(groups.items(), key=lambda item: min(item[1])):
        solo = copy.deepcopy(document)
        bars = set()
        for index, lane in enumerate(solo['lanes'], 1):
            if index in tracks:
                for p in lane['placements']:
                    first = p['startTick'] // TICKS_PER_BAR
                    bars.update(range(first, (p['startTick'] + p['durationTicks'] - 1) // TICKS_PER_BAR + 1))
            else:
                lane['placements'] = []
                lane.pop('automation', None)
        if not bars:
            continue
        path = out / f'lane-{min(tracks):02d}.mixjam'
        path.write_text(json.dumps(solo), encoding='utf-8')
        run(path, out / f'lane-{min(tracks):02d}.wav')
        _, part = read_wav(str(out / f'lane-{min(tracks):02d}.wav'))
        part = part.mean(axis=1)
        part = np.pad(part, (0, max(0, len(mix) - len(part))))[: len(mix)]
        rest = mix - part
        mask = np.zeros(len(mix), bool)
        for bar in bars:
            mask[int(bar * bar_frames): int((bar + 1) * bar_frames)] = True
        level = 10 * np.log10((np.mean(part[mask] ** 2) + 1e-20) / (np.mean(rest[mask] ** 2) + 1e-20))
        part_cells, _ = cells(part, rate)
        rest_cells, _ = cells(rest, rate)
        frames = [i for i in range(len(part_cells)) if mask[min(len(mask) - 1, i * frame + frame // 2)]]
        heard = heard_share(part_cells[frames], rest_cells[frames])
        flag = '  <- drowned' if heard < DROWNED else ''
        print(f'{name[:22]:22s} {len(bars):5d} {level:10.1f} {heard * 100:5.0f}%{flag}')


if __name__ == '__main__':
    main()
