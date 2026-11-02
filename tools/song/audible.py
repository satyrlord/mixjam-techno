"""Which lanes can be heard: audible.py SONG.mixjam OUT_DIR [--jobs N]

python -I tools/song/audible.py SONG.mixjam OUT_DIR

The tool renders each lane with its twin, "Name (twin)", and its FX returns.
The board is off. It renders N lanes at a time and sums them into the premaster.
For each lane, it prints two measurements for the bars where the lane plays:

- Its level against the rest of the mix, in dB.
- The share of 1/3-octave cells where it is within 6 dB of the rest of the mix
  or louder. The cells span 100 Hz to 12 kHz in 186 ms frames. A share below
  10% marks a lane that is probably drowned.

Lane names come from the song's `.meta.json`. Without it, each used track is
its own lane. render.py writes the same table as audible.txt from its lane
renders. Run this tool alone only for a song that render.py did not render.
"""
import json
import os
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mixjam import TICKS_PER_BAR, read_wav  # noqa: E402
import render  # noqa: E402

BANDS = [100 * 2 ** (k / 3) for k in range(0, 28)]  # 100 Hz .. 12.7 kHz
DROWNED = 0.10


def spectra(mono, rate):
    """Windowed spectrum per 186 ms frame, and the frame length."""
    frame = int(rate * 0.186)
    count = len(mono) // frame
    frames = mono[: count * frame].reshape(count, frame).astype(np.float32) * np.hanning(frame).astype(np.float32)
    return np.fft.rfft(frames, axis=1), frame


def band_energy(spectrum, frame, rate):
    """Energy per frame and 1/3-octave band of a spectrum from spectra()."""
    freqs = np.fft.rfftfreq(frame, 1 / rate)
    bands = np.zeros((len(freqs), len(BANDS) - 1), np.float32)
    for k in range(len(BANDS) - 1):
        bands[(freqs >= BANDS[k]) & (freqs < BANDS[k + 1]), k] = 1
    power = spectrum.real ** 2 + spectrum.imag ** 2
    return (power @ bands).astype(np.float64)


def cells(mono, rate):
    """Energy per 186 ms frame and 1/3-octave band."""
    spectrum, frame = spectra(mono, rate)
    return band_energy(spectrum, frame, rate), frame


def heard_share(part_cells, rest_cells):
    """The share of the lane's active cells where it is within 6 dB of the rest or louder."""
    active = part_cells > 1e-9 * part_cells.max() if part_cells.size else part_cells
    if not active.any():
        return 0.0
    ratio = 10 * np.log10((part_cells + 1e-20) / (rest_cells + 1e-20))
    return float(np.mean((ratio > -6)[active]))


def lane_bars(document, tracks):
    """The 0-based bars where clips play, through Repeat but without FX tails."""
    bars = set()

    def include(first, last):
        if first < last:
            bars.update(range(first // TICKS_PER_BAR, (last - 1) // TICKS_PER_BAR + 1))

    end = render.song_end_tick(document)
    for index in tracks:
        lane = document['lanes'][index - 1]
        clips = [(p['startTick'], p['startTick'] + p['durationTicks']) for p in lane['placements']]
        start, held, capture = 0, 0, 0
        points = lane.get('automation', {}).get('repeat', [])
        for point in [*points, dict(tick=end, value=0)]:
            stop = min(point['tick'], end)
            if point['value'] == held and stop < end:
                continue  # Holding the same division does not retrigger it.
            if held:
                division = TICKS_PER_BAR // int(held)
                slices = [(max(first, capture) - capture, min(last, capture + division) - capture)
                          for first, last in clips if first < capture + division and last > capture]
                for trigger in range(start, stop, division):
                    for first, last in slices:
                        include(trigger + first, min(trigger + last, stop))
                    # An outgoing slice continues for the 132-frame ramp.
                    # At every supported tempo that is less than one tick;
                    # conservatively include that tick in the bar selection.
                    if trigger > start and any(first < capture + division + 1 and last > capture + division
                                               for first, last in clips):
                        include(trigger, min(trigger + 1, end))
                if start < stop:
                    last_trigger = start + (stop - start - 1) // division * division
                    tail = capture + stop - last_trigger
                    if any(first < tail + 1 and last > tail for first, last in clips):
                        include(stop, min(stop + 1, end))
            else:
                for first, last in clips:
                    include(max(first, start), min(last, stop))
            if stop == end:
                break
            if not held and point['value']:
                capture = point['tick']
            start, held = point['tick'], point['value']
    return bars


def audibility(document, groups, rate, mix, part_of, jobs=None):
    """The table's lines: each lane's level against the rest and the share of it heard.

    `mix` is the whole premaster in mono; `part_of(name)` gives a lane's render in mono.
    The tool measures `jobs` lanes at a time (default: half the processors, at most 8).
    """
    bar_frames = rate * 240 / document['song']['bpm']
    # The windowed FFT is linear, so the rest of the mix is the mix's spectrum minus the lane's.
    mix_spectrum, frame = spectra(mix, rate)

    def line(item):
        name, tracks = item
        bars = lane_bars(document, tracks)
        if not bars:
            return None
        part = part_of(name)
        part = np.pad(part, (0, max(0, len(mix) - len(part))))[: len(mix)]
        rest = mix - part
        mask = np.zeros(len(mix), bool)
        for bar in bars:
            mask[int(bar * bar_frames): int((bar + 1) * bar_frames)] = True
        level = 10 * np.log10((np.mean(part[mask] ** 2) + 1e-20) / (np.mean(rest[mask] ** 2) + 1e-20))
        part_spectrum, _ = spectra(part, rate)
        centres = np.minimum(len(mask) - 1, np.arange(len(part_spectrum)) * frame + frame // 2)
        frames = mask[centres]
        part_cells = band_energy(part_spectrum[frames], frame, rate)
        rest_cells = band_energy(mix_spectrum[frames] - part_spectrum[frames], frame, rate)
        heard = heard_share(part_cells, rest_cells)
        flag = '  <- drowned' if heard < DROWNED else ''
        return f'{name[:22]:22s} {len(bars):5d} {level:10.1f} {heard * 100:5.0f}%{flag}'

    jobs = jobs or max(1, min(8, (os.cpu_count() or 2) // 2))
    ordered = sorted(groups.items(), key=lambda item: min(item[1]))
    with ThreadPoolExecutor(jobs) as pool:
        rows = [row for row in pool.map(line, ordered) if row is not None]
    return [f'{"lane":22s} {"bars":>5s} {"vs rest dB":>10s} {"heard":>6s}'] + rows


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
                               lambda name: read_wav(lanes[name])[1].mean(axis=1), jobs)))


if __name__ == '__main__':
    main()
