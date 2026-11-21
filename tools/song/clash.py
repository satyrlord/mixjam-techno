"""Harmonic fit of each tonal lane against the rest of the mix, per bar: clash.py SONG.mixjam RENDER_DIR [...]

python -I tools/song/clash.py SONG.mixjam RENDER_DIR [--lanes 36,46] [--alt] [--drop 0.15]

RENDER_DIR holds premaster.wav and the lane-NN.wav files of render.py. For each
bar, the fit is the cosine of two chroma vectors (150 Hz-4 kHz): the lane's and
the rest of the mix's (premaster minus the lane). A bar where the rest of the
mix is much quieter than the lane in that band (a hole, a solo) does not count.
A lane's usual fit is its
median over the bars it plays. A weak bar is a bar whose fit is more than
--drop (default 0.15) under the usual fit: the lane may leave the bed's chord
there. Typical cause: the second half of a chord file under a bed that has
moved to the next chord.

--alt also scores, for each weak bar, every file of the lane's family (the
sounds that the lane plays with the same loop length) at the same place in the
phrase. The score includes the varispeed to the song tempo and the clip pitch.
It does not model reversed clips or Pitch automation, so their alternatives
are unavailable. It does not include the lane's EQ and drive. A file
that scores clearly higher at that place is a candidate: take that part of the
phrase from it (the rule in phrase.py). Check each change in a new render.

measure.py adds the weak bars to its report when the lane files are present,
with the file that fits at least BETTER better at the same place, if any.
"""
import json
import math
import sys
import wave
from functools import lru_cache
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cli import option  # noqa: E402
from harmony import NOTES  # noqa: E402
from lanes import lane_groups  # noqa: E402
from mixjam import TICKS_PER_BAR, loop_bars, playback_bpm, read_wav  # noqa: E402
from phrase import GATED as TONAL  # noqa: E402  (the roles with a phrase; field recordings are skipped)

SIZE, HOP = 8192, 4096
LOW, HIGH = 150.0, 4000.0
DROP = 0.15
QUIET = 10 ** (-60 / 20)  # a bar under -60 dBFS RMS in the lane does not count
BETTER = 0.1  # a family file at the same phrase place must fit this much better than the clip's own file
BED = 0.3 # a bar where the rest of the mix has under 0.3 of the lane's power in the chroma band does not count


@lru_cache(maxsize=256)
def _bins(rate, cents=0.0):
    """The chroma band's mask and one-hot pitch-class matrix. Callers must not change them."""
    f = np.fft.rfftfreq(SIZE, 1 / rate)
    m = (f > LOW) & (f < HIGH)
    pc = np.round(69 + 12 * np.log2(f[m] / 440.0) + cents / 100).astype(int) % 12
    onehot = np.zeros((m.sum(), 12), np.float32)
    onehot[np.arange(m.sum()), pc] = 1
    return m, onehot


def spectra(x, rate):
    """Complex spectra (frames, bins) of a mono signal in the chroma band."""
    m, _ = _bins(rate)
    if len(x) < SIZE:
        return np.zeros((0, m.sum()), np.complex64)
    frames = np.lib.stride_tricks.sliding_window_view(x, SIZE)[::HOP]
    window = np.hanning(SIZE).astype(np.float32)
    out = np.empty((len(frames), m.sum()), np.complex64)
    for k in range(0, len(frames), 256):
        out[k:k + 256] = np.fft.rfft(frames[k:k + 256] * window, axis=1)[:, m]
    return out


def chroma(power, onehot):
    return np.sqrt(np.maximum(power @ onehot, 0))


def cosine(a, b):
    scale = np.linalg.norm(a) * np.linalg.norm(b)
    return float(a @ b / scale) if scale > 0 else None


def mono(path, length=None):
    rate, data = read_wav(path)
    x = data.mean(axis=1).astype(np.float32)
    if length is not None:
        x = np.pad(x, (0, max(0, length - len(x))))[:length]
    return rate, x


class Mix:
    """The premaster's spectra and the bar grid."""

    def __init__(self, render, bpm):
        self.render = Path(render)
        self.rate, self.x = mono(self.render / 'premaster.wav')
        self.spec = spectra(self.x, self.rate)
        self.bar_frames = self.rate * 240 / bpm
        self.bars = int(len(self.x) / self.bar_frames)
        _, self.onehot = _bins(self.rate)
        self.unavailable = {}

    def frames_of(self, bar):
        """STFT frame indices inside bar `bar` (1-based)."""
        a, b = (bar - 1) * self.bar_frames, bar * self.bar_frames - SIZE
        return range(int(math.ceil(a / HOP)), int(b // HOP) + 1)

    def lane(self, track):
        """{bar: (fit, rest chroma, lane chroma)} of the bars where the lane plays."""
        path = self.render / f'lane-{track:02d}.wav'
        if not path.exists():
            self.unavailable[track] = f'Track {track}: harmonic fit unavailable; {path.name} is missing.'
            return None
        rate, x = mono(path, len(self.x))
        if rate != self.rate:
            self.unavailable[track] = f'Track {track}: harmonic fit unavailable; the lane and premaster sample rates differ.'
            return None
        spec = spectra(x, self.rate)
        out = {}
        for bar in range(1, self.bars + 1):
            frames = self.frames_of(bar)
            if not len(frames):
                continue
            a = int((bar - 1) * self.bar_frames)
            seg = x[a:int(bar * self.bar_frames)]
            if not len(seg) or float(np.sqrt(np.mean(seg ** 2))) < QUIET:
                continue
            lane = spec[frames.start:frames.stop]
            rest = self.spec[frames.start:frames.stop] - lane
            p_lane = (np.abs(lane) ** 2).sum(axis=0)
            p_rest = (np.abs(rest) ** 2).sum(axis=0)
            if p_rest.sum() < BED * p_lane.sum():
                continue  # the lane is the bed here: nothing tonal to clash with
            c_lane, c_rest = chroma(p_lane, self.onehot), chroma(p_rest, self.onehot)
            fit = cosine(c_lane, c_rest)
            if fit is not None:
                out[bar] = (fit, c_rest, c_lane)
        return out


def top(c, n=3):
    return ' '.join(NOTES[k] for k in np.argsort(-c)[:n])  # chroma index = MIDI note % 12, 0 is C


def weak(fits, drop=DROP):
    """(usual fit, [(bar, fit)]) of a lane's fits."""
    if not fits:
        return None, []
    usual = float(np.median([f for f, _, _ in fits.values()]))
    return usual, [(bar, f) for bar, (f, _, _) in sorted(fits.items()) if f < usual - drop]


def rendered_groups(document, lanes):
    """The renderer's lane groups, with unnamed metadata entries left as separate tracks."""
    return lane_groups(document, {'lanes': {key: info for key, info in lanes.items() if 'name' in info}})


def tonal_tracks(document, lanes):
    return [min(tracks) for tracks in rendered_groups(document, lanes).values()
            if (lanes.get(str(min(tracks))) or {}).get('role') in TONAL
            and not (lanes.get(str(min(tracks))) or {}).get('field')]


class Sources:
    """Source loops of a song, for --alt."""

    def __init__(self, document, lanes=None):
        self.samples = {s['id']: s for s in document['samples']}
        self.bpm = document['song']['bpm']
        self.cache = {}
        self.unavailable = {}
        self.groups = rendered_groups(document, lanes or {})

    def get(self, sample_id):
        if sample_id not in self.cache:
            self.cache[sample_id] = None
            s = self.samples.get(sample_id)
            if s is None:
                self.unavailable[sample_id] = f'Sample {sample_id}: alternatives unavailable; no imported WAV path.'
                return None
            try:
                rate, x = mono(s['path'])
            except (OSError, ValueError, wave.Error, EOFError) as error:
                self.unavailable[sample_id] = f"Sample {sample_id} ({s['name']}): alternatives unavailable; {error}"
                return None
            if not len(x):
                self.unavailable[sample_id] = f"Sample {sample_id} ({s['name']}): alternatives unavailable; the source is empty."
                return None
            source_bpm = s.get('sourceBpm') or self.bpm
            play = playback_bpm(source_bpm, self.bpm)
            fpt = rate * 240 / play / TICKS_PER_BAR
            varispeed = 1200 * math.log2(self.bpm / play)
            self.cache[sample_id] = (rate, x, fpt, varispeed, loop_bars(len(x) / fpt / TICKS_PER_BAR))
        return self.cache[sample_id]

    def bar_chroma(self, sample_id, position, cents):
        """Chroma of one song bar of a source from `position` ticks into it (wrapping), with the clip pitch."""
        rate, x, fpt, varispeed, _ = self.get(sample_id)
        a = int(position * fpt) % len(x)
        # The bar from `a`, wrapping at the end of the source.
        seg = np.take(x, (np.arange(min(len(x), int(TICKS_PER_BAR * fpt))) + a) % len(x))
        _, onehot = _bins(rate, cents + varispeed)
        spec = spectra(seg, rate)
        return chroma((np.abs(spec) ** 2).sum(axis=0), onehot)


def alternatives(document, track, bar, rest, sources):
    """[(fit, name, bars into the file, is the clip's own file)] of the lane's family at the place in the
    phrase of the clip at `bar`, best fit first."""
    tracks = next((tracks for tracks in sources.groups.values() if track in tracks), [track])
    tick = (bar - 1) * TICKS_PER_BAR
    active = [(document['lanes'][t - 1], p) for t in tracks for p in document['lanes'][t - 1]['placements']
              if p['startTick'] <= tick < p['startTick'] + p['durationTicks']]
    if not active:
        return []
    if len(active) > 1:
        sources.unavailable[f'track-{track}'] = (
            f'Track {track}: alternatives unavailable while multiple clips in the rendered lane overlap.')
        return []
    lane, clip = active[0]
    if clip.get('reverse', False) or lane.get('automation', {}).get('pitch'):
        sources.unavailable[f'track-{track}'] = (
            f'Track {track}: alternatives unavailable for reversed clips or Pitch automation; '
            'the dry-source comparison does not model their source positions.')
        return []
    source = sources.get(clip['sampleId'])
    if source is None:
        return []
    _, _, fpt, _, bars = source
    if not bars:
        return []
    position = (clip['offsetFrames'] / fpt + tick - clip['startTick']) % (bars * TICKS_PER_BAR)
    family = {p['sampleId'] for t in tracks for p in document['lanes'][t - 1]['placements']
              if sources.get(p['sampleId']) is not None and sources.get(p['sampleId'])[4] == bars}
    out = []
    for sample_id in family:
        c = sources.bar_chroma(sample_id, position, clip.get('pitchCents') or 0)
        fit = cosine(c, rest)
        if fit is not None:
            out.append((fit, sources.samples[sample_id]['name'], position / TICKS_PER_BAR,
                        sample_id == clip['sampleId']))
    return sorted(out, reverse=True)


def runs(bars):
    """'31-32, 79' from [31, 32, 79]."""
    out, start = [], None
    for k, b in enumerate(bars):
        if start is None:
            start = b
        if k + 1 == len(bars) or bars[k + 1] != b + 1:
            out.append(f'{start}' if start == b else f'{start}-{b}')
            start = None
    return ', '.join(out)


def report(document, lanes, render, say, drop=DROP):
    """The report section for measure.py. Silent when the render has no lane files."""
    if not list(Path(render).glob('lane-*.wav')):
        return
    mix = Mix(render, document['song']['bpm'])
    sources = Sources(document, lanes)
    rows = []
    for track in tonal_tracks(document, lanes):
        fits = mix.lane(track)
        usual, low = weak(fits or {}, drop)
        if not low:
            continue
        better = []
        for bar, _ in low:
            alt = alternatives(document, track, bar, fits[bar][1], sources)
            now = next((f for f, _, _, is_now in alt if is_now), None)
            if alt and now is not None and not alt[0][3] and alt[0][0] >= now + BETTER:
                better.append(f'{bar} {alt[0][1]} ({now:.2f} -> {alt[0][0]:.2f})')
        rows.append(f"| {track} {(lanes.get(str(track)) or {}).get('name', '')} | {usual:.2f} | "
                    f"{runs([b for b, _ in low])} | {'; '.join(better) or '-'} |")
    say('## Harmonic fit (tonal lanes)\n')
    for message in [*mix.unavailable.values(), *sources.unavailable.values()]:
        say(f'- {message}')
    if not rows:
        say(f'No measured tonal lane has a bar more than {drop:.2f} under its usual fit.\n')
        return
    say(f'Weak bars: a lane fits the rest of the mix more than {drop:.2f} worse than usual there. "Better file" '
        f'names a file of the lane that fits at least {BETTER:.2f} better at the same place in the phrase (dry '
        'sources compared): a repair to try. Weak bars without one are for the ear only.\n')
    say('| track | usual fit | weak bars | better file |')
    say('|---|---|---|---|')
    for row in rows:
        say(row)
    say('')


def main():
    args = sys.argv[1:]
    if len(args) < 2 or args[0].startswith('-'):
        raise SystemExit(__doc__)
    song = Path(args[0])
    document = json.loads(song.read_text(encoding='utf-8'))
    meta_path = song.with_suffix('.meta.json')
    lanes = json.loads(meta_path.read_text(encoding='utf-8'))['lanes'] if meta_path.exists() else {}
    drop = option(args, '--drop', float, DROP)
    tracks = option(args, '--lanes', lambda text: [int(t) for t in text.split(',')]) or tonal_tracks(document, lanes)
    owners = {track: min(group) for group in rendered_groups(document, lanes).values() for track in group}
    tracks = list(dict.fromkeys(owners.get(track, track) for track in tracks))
    mix = Mix(args[1], document['song']['bpm'])
    sources = Sources(document, lanes) if '--alt' in args else None
    for track in tracks:
        fits = mix.lane(track)
        if fits is None:
            print(mix.unavailable.get(track, f'track {track}: no lane-{track:02d}.wav'))
            continue
        usual, low = weak(fits, drop)
        name = (lanes.get(str(track)) or {}).get('name', '')
        if usual is None:
            print(f'track {track} {name}: silent')
            continue
        print(f'track {track} {name}: usual fit {usual:.2f} over {len(fits)} bars; weak: '
              f'{runs([b for b, _ in low]) or "none"}')
        for bar, fit in low:
            _, rest, lane = fits[bar]
            line = f'  bar {bar}: {fit:.2f}, lane {top(lane)} over {top(rest, 4)}'
            if sources:
                alt = alternatives(document, track, bar, rest, sources)
                line += '; at this place: ' + ', '.join(
                    f"{n}{' (now)' if now else ''} @{q + 1:.0f} {f:.2f}" for f, n, q, now in alt[:4])
            print(line)
    if sources:
        for message in sources.unavailable.values():
            print(message)


if __name__ == '__main__':
    main()
