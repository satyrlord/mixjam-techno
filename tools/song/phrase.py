"""Clips that cut their loop's phrase: phrase.py SONG.mixjam [--all]

python -I tools/song/phrase.py SONG.mixjam [--all]

The artist profile's working rule 10: a clip plays its sample's whole phrase.
A clip cuts its phrase when it starts or ends inside its source loop and the
lane does not continue the figure there. A clip continues the
figure when it starts where the previous clip ends, at the source position where
that clip stopped. The next clip can come from any file of the lane, for
example the next chord's file at the same place in the phrase.

These cases are not cuts:
- A start or end on a repeat point. When the halves of a loop play the same
  notes (ALIKE), the loop repeats a shorter figure. A 4-bar loop can repeat a
  2-bar figure, and so on.
- A start in time: the clip starts on a bar or half-bar line at its place on
  the phrase grid, so the figure resumes after a rest. The grid starts at
  bar 1, or at the lane's `anchor` bar in the metadata.
- An in-time crossfade: a clip in time on the phrase grid starts or ends
  inside another clip of the lane or its twin that is also in time. Both play
  the same place in the phrase, so the figure continues across the files,
  for example from one chord's file into the next.
- A sound that is not a loop of whole bars (one-shots and hits).
- A lane with `"field": true` in the metadata: a field recording (crickets,
  traffic) is not a phrase.
- Clips inside a span of the lane's `cuts` entry in the metadata: approved
  devices, such as retriggered runs, and cuts with a serious reason (the
  owner asked, or no other tool fixes that part of the sample), as
  [first bar, last bar, reason]:
  `"lanes": {"50": {"name": ..., "role": "lead", "cuts": [[185, 192, "double-speed run"]]}}`

A fade out does not make a cut whole: a clip that fades out inside its phrase
still cuts it. measure.py reports the cuts on GATED lanes (all tonal roles:
leads, keys, arps, bass, vocals, pads and atmospheres) as gate problems. It
counts the cuts on effects and drums in its report without failing the gate;
drum slices are a permitted device. Here --all lists those lanes too.
"""
import json
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mixjam import TICKS_PER_BAR, loop_bars, playback_bpm, read_wav, twin_base  # noqa: E402
from pitch_clock import PitchClock  # noqa: E402

# Roles whose phrase cuts fail the gate.
GATED = {'lead', 'keys', 'arp', 'bass', 'vocal', 'pad', 'atmos'}
SLACK = 24  # ticks: clip edges closer than this to a loop edge or a continuation count as on it
# Halves whose note movement correlates at least this much repeat one figure. Measured on chord-file loops: repeated
# figures 0.57-0.72; different halves -0.5 to 0.38, a staccato riff with the same rhythm in both halves included.
ALIKE = 0.5
SIZE, HOP = 4096, 1024


def movement(x, rate):
    """Semitone-band log spectrogram (60 Hz-5 kHz), each band normalized over time: it follows the notes that move,
    not the sound's fixed timbre."""
    f = np.fft.rfftfreq(SIZE, 1 / rate)
    m = (f > 60) & (f < 5000)
    band = np.round(12 * np.log2(f[m] / 440.0)).astype(int)
    band -= band.min()
    size = band.max() + 1
    window = np.hanning(SIZE)
    frames = []
    for i in range(0, len(x) - SIZE + 1, HOP):
        v = np.bincount(band, weights=np.abs(np.fft.rfft(x[i:i + SIZE] * window))[m] ** 2, minlength=size)
        frames.append(np.log10(v + 1e-9))
    s = np.array(frames) if frames else np.zeros((1, size))
    return (s - s.mean(axis=0)) / (s.std(axis=0) + 1e-9)


class Loop:
    """A source loop at the song tempo: length in ticks, frames per tick, and its figure length."""

    def __init__(self, path, source_bpm, song_bpm):
        rate, data = read_wav(path)
        self.fpt = rate * 240 / playback_bpm(source_bpm or song_bpm, song_bpm) / TICKS_PER_BAR
        self.ticks = len(data) / self.fpt
        self.bars = loop_bars(self.ticks / TICKS_PER_BAR)
        self.figure = None
        if self.bars:
            notes = movement(data.mean(axis=1), rate)
            figure = self.bars * TICKS_PER_BAR
            # Halve while the halves are alike: the loop repeats the shorter figure.
            while figure >= 2 * TICKS_PER_BAR and figure % 2 == 0 \
                    and self.alike(notes, figure // 2) >= ALIKE:
                figure //= 2
            self.figure = figure

    def alike(self, notes, length):
        """The lowest correlation of the note movement between consecutive `length`-tick parts."""
        count = int(round(self.ticks / length))
        frames = len(notes) // max(1, count)
        if count < 2 or frames < 4:
            return 0.0
        parts = [notes[k * frames:(k + 1) * frames].ravel() for k in range(count)]
        worst = 1.0
        for a, b in zip(parts, parts[1:]):
            scale = np.linalg.norm(a) * np.linalg.norm(b)
            if len(a) < 4 or scale == 0:
                return 0.0
            worst = min(worst, float(a @ b / scale))
        return worst

    def on_edge(self, position):
        """Whether a source position in ticks is on the loop's start or end or on a repeat point."""
        if position <= SLACK or position >= self.ticks - SLACK:
            return True
        rest = position % self.figure
        return rest <= SLACK or rest >= self.figure - SLACK


def source_span(p, src, clock=None):
    """Source positions at the clip's timeline edges, in source ticks."""
    offset = p['offsetFrames'] / src.fpt
    distance = p['durationTicks']
    if clock:
        first = clock.frame(p['startTick'])
        last = clock.frame(p['startTick'] + p['durationTicks'])
        distance = (clock.at(last) - clock.at(first)) / clock.frames_per_tick
    bounds = (offset, offset + distance)
    return bounds[::-1] if p.get('reverse', False) else bounds


def cuts(document, lanes, roles=None):
    """[(track, lane name, role, clip, what)] of the clips that cut their phrase. `what` names the cut edges."""
    samples = {s['id']: s for s in document['samples']}
    song_bpm = document['song']['bpm']
    loops = {}  # Re-read sources on the next check, including files replaced at the same path.
    clocks = {}

    def clock_of(index):
        if index not in clocks:
            points = document['lanes'][index].get('automation', {}).get('pitch', [])
            clocks[index] = PitchClock(points, song_bpm) if points else None
        return clocks[index]

    def loop(sample_id):
        s = samples.get(sample_id)
        if not s:
            return None
        key = (s['path'], s.get('sourceBpm'), song_bpm)
        if key not in loops:
            loops[key] = Loop(*key) if Path(s['path']).exists() else None
        return loops[key]

    distances = {}  # id of a placement to its grid distance: a placement has one lane, clock and anchor

    def grid_distance(q, q_clock, q_anchor):
        """Ticks between a clip's source start and its place on the lane's phrase grid."""
        if id(q) not in distances:
            src = loop(q['sampleId'])
            grid = ((q['startTick'] - (q_anchor - 1) * TICKS_PER_BAR) % src.ticks) if src.ticks else 0
            if q_clock:
                grid = ((q_clock.at(q_clock.frame(q['startTick']))
                         - q_clock.at(q_clock.frame((q_anchor - 1) * TICKS_PER_BAR)))
                        / q_clock.frames_per_tick) % src.ticks
            if q.get('reverse', False):
                grid = -grid % src.ticks
            distance = abs(grid - source_span(q, src, q_clock)[0]) % src.ticks
            distances[id(q)] = min(distance, src.ticks - distance)
        return distances[id(q)]

    found = []
    for index, lane in enumerate(document['lanes']):
        info = lanes.get(str(index + 1)) or {}
        role = info.get('role', 'generic')
        if (roles is not None and role not in roles) or info.get('field'):
            continue
        devices = [(first, last) for first, last, *_ in info.get('cuts', [])]
        anchor = info.get('anchor', 1)
        placements = sorted(lane['placements'], key=lambda p: p['startTick'])
        clock = clock_of(index)
        group = twin_base(info.get('name', ''))
        neighbours = [(q, clock_of(other_index), (lanes.get(str(other_index + 1)) or {}).get('anchor', 1))
                      for other_index, other in enumerate(document['lanes'])
                      if other_index == index or (group and
                          twin_base((lanes.get(str(other_index + 1)) or {}).get('name', '')) == group)
                      for q in other['placements']]

        def source(p):
            src = loop(p['sampleId'])
            return source_span(p, src, clock)

        for k, p in enumerate(placements):
            src = loop(p['sampleId'])
            if src is None or not src.figure:
                continue
            bar = p['startTick'] / TICKS_PER_BAR + 1
            end_tick = p['startTick'] + p['durationTicks']
            if any(first <= bar and end_tick <= last * TICKS_PER_BAR for first, last in devices):
                continue
            start, stop = source(p)
            before = [(q, other_clock) for q, other_clock, _ in neighbours if q is not p
                      and abs(q['startTick'] + q['durationTicks'] - p['startTick']) <= SLACK
                      and loop(q['sampleId']) and loop(q['sampleId']).figure]
            after = [(q, other_clock) for q, other_clock, _ in neighbours if q is not p
                     and abs(q['startTick'] - end_tick) <= SLACK
                     and loop(q['sampleId']) and loop(q['sampleId']).figure]
            led_in = any(abs(source_span(q, loop(q['sampleId']), other_clock)[1] - start) <= SLACK
                         for q, other_clock in before)
            led_on = any(abs(source_span(q, loop(q['sampleId']), other_clock)[0] - stop) <= SLACK
                         for q, other_clock in after)
            # A figure that resumes in time after a rest starts at its place on the phrase grid.
            on_grid = grid_distance(p, clock, anchor) <= SLACK
            in_time = p['startTick'] % (TICKS_PER_BAR // 2) == 0 and on_grid
            # An in-time crossfade: another in-time clip of the lane or its twin plays across this edge.
            if on_grid:
                crossing = [q for q, q_clock, q_anchor in neighbours if q is not p
                            and loop(q['sampleId']) and loop(q['sampleId']).figure
                            and grid_distance(q, q_clock, q_anchor) <= SLACK]
                led_in = led_in or any(q['startTick'] < p['startTick'] < q['startTick'] + q['durationTicks']
                                       for q in crossing)
                led_on = led_on or any(q['startTick'] < end_tick < q['startTick'] + q['durationTicks']
                                       for q in crossing)
            what = []
            if not src.on_edge(start) and not led_in and not in_time:
                what.append('start')
            if not src.on_edge(stop) and not led_on:
                what.append('end')
            if what:
                found.append((index + 1, info.get('name', ''), role, p, src, ' and '.join(what)))
    return found


def describe(document, cut):
    track, name, role, p, src, what = cut
    samples = {s['id']: s for s in document['samples']}
    bar = p['startTick'] / TICKS_PER_BAR + 1
    points = document['lanes'][track - 1].get('automation', {}).get('pitch', [])
    clock = PitchClock(points, document['song']['bpm']) if points else None
    first, last = [position / TICKS_PER_BAR for position in source_span(p, src, clock)]
    figure = src.figure // TICKS_PER_BAR
    loop = f'{src.bars}-bar loop' + (f' ({figure}-bar figure)' if figure != src.bars else '')
    return (f"The {samples[p['sampleId']]['name']} clip on track {track} ({name}) at bar {bar:.2f} plays bars "
            f"{first + 1:.2f}-{last + 1:.2f} of its {loop}, and its {what} cut{'s' if ' and ' not in what else ''} "
            f"the phrase.")


def main():
    args = sys.argv[1:]
    if not args or args[0].startswith('-'):
        raise SystemExit(__doc__)
    song = Path(args[0])
    document = json.loads(song.read_text(encoding='utf-8'))
    meta_path = song.with_suffix('.meta.json')
    lanes = json.loads(meta_path.read_text(encoding='utf-8'))['lanes'] if meta_path.exists() else {}
    found = cuts(document, lanes, None if '--all' in args else GATED)
    by = defaultdict(list)
    for c in found:
        by[(c[0], c[1], c[2])].append(c)
    print(f'{len(found)} clips cut their phrase.')
    for (track, name, role), rows in sorted(by.items()):
        print(f'\ntrack {track} {name} ({role}): {len(rows)}')
        for c in rows:
            print('- ' + describe(document, c))


if __name__ == '__main__':
    main()
