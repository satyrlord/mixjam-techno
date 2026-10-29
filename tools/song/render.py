"""Render a song pass: every lane alone, then the master.

python -I tools/song/render.py SONG.mixjam OUT_DIR [--fit-gain DBFS] [--jobs N] [--no-master]

Renders only what a pass uses, and nothing twice:
- every lane alone, with its twin, its FX returns and the board off, to OUT_DIR/lane-NN.wav,
  N renders at a time (default: half the processors, at most 8);
- from their sum, without rendering: OUT_DIR/premaster.wav (the mix before the board), one
  OUT_DIR/stem-ROLE.wav per role, and OUT_DIR/audible.txt (see audible.py);
- the master, with the board, once: OUT_DIR/master.wav. --no-master skips it.

--fit-gain sets the song's master gain so that the premaster peaks at DBFS (for example -0.5) and
saves the song before the master render. The lanes render at the song's gain; their sum, which
cannot clip, gives the change, and the premaster and the stems are scaled to the new gain.

Lane names and roles come from the `.meta.json` beside the song (mixjam.py writes it); a used track
without one is its own lane, and without the file there are no stems. The premaster and the stems
are sums of separate lane renders: an FX return that limits or saturates can differ from a render
of the whole mix by a fraction of a dB.

The renderer is the release build in target/release; it runs headless and plays no sound. Each
render keeps its scan cache in a `.data` folder under OUT_DIR.
"""
import json
import math
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import env  # noqa: E402
from mixjam import board_off as _board_off, read_wav, write_wav  # noqa: E402


def run(song_path, wav_path, data=None):
    """Render one song file to a WAV with the release build; `data` is its scan-cache folder."""
    data = data or Path(wav_path).resolve().parent / '.data'
    environment = dict(os.environ, MIXJAM_TECHNO_DATA_DIR=str(data))
    result = subprocess.run([str(env.exe()), 'render', '--mix', str(song_path), '--output', str(wav_path)],
                            capture_output=True, text=True, env=environment)
    if result.returncode != 0:
        raise SystemExit(f'render failed for {song_path}: {result.stdout}{result.stderr}')


def board_off(document):
    """The premaster: the mix before the board, so the trim is off too."""
    document['master'] = _board_off(document['master'])
    return document


def copy(document):
    return json.loads(json.dumps(document))


def read_meta(song):
    path = Path(song).with_suffix('.meta.json')
    return json.loads(path.read_text(encoding='utf-8')) if path.exists() else None


def lane_groups(document, meta):
    """Lane name to its 1-based tracks, for every used track: the names in the meta, a twin
    ("NAME (twin)") with its lane, and a track the meta does not name as its own lane."""
    names = {int(k): v['name'] for k, v in meta['lanes'].items()} if meta else {}
    groups = {}
    for track, lane in enumerate(document['lanes'], 1):
        if lane['placements']:
            name = names.get(track, f'track {track}').replace(' (twin)', '')
            groups.setdefault(name, []).append(track)
    return groups


def lane_roles(groups, meta):
    """Lane name to its role, for lanes that the meta names."""
    roles = {}
    for name, tracks in groups.items():
        info = (meta or {}).get('lanes', {}).get(str(min(tracks)))
        if info is not None:
            roles[name] = info['role']
    return roles


def solo(document, tracks):
    """The premaster with only `tracks` playing."""
    part = board_off(copy(document))
    for index, lane in enumerate(part['lanes'], 1):
        if index not in tracks:
            lane['placements'] = []
            lane.pop('automation', None)
    return part


def render_lanes(document, groups, out, jobs=None):
    """Render every lane alone, `jobs` at a time; lane name to its WAV."""
    jobs = jobs or max(1, min(8, (os.cpu_count() or 2) // 2))

    def one(item):
        name, tracks = item
        path = out / f'lane-{min(tracks):02d}.mixjam'
        path.write_text(json.dumps(solo(document, tracks)), encoding='utf-8')
        wav = path.with_suffix('.wav')
        run(path, wav, out / '.data' / path.stem)
        path.unlink()
        return name, wav

    with ThreadPoolExecutor(jobs) as pool:
        return dict(pool.map(one, groups.items()))


def add(total, data):
    """`total` plus `data`, padded to the longer of the two."""
    if total is None:
        return data.astype(np.float32)
    if len(data) > len(total):
        total = np.pad(total, ((0, len(data) - len(total)), (0, 0)))
    total[:len(data)] += data
    return total


def mix_down(lanes, roles):
    """The premaster and one stem per role, as sums of the lane renders: rate, premaster, stems."""
    rate, premaster, stems = None, None, {}
    for name, wav in lanes.items():
        rate, data = read_wav(wav)
        premaster = add(premaster, data)
        if name in roles:
            stems[roles[name]] = add(stems.get(roles[name]), data)
    return rate, premaster, stems


def fit_gain(song, peak, target_db):
    """Set the master gain so that a premaster now peaking at `peak` (linear) peaks at target_db;
    save the song; return the factor the gain changed by."""
    if not math.isfinite(target_db) or target_db > 0:
        raise SystemExit('the gain target must be finite and at or below 0 dBFS')
    if peak <= 0:
        raise SystemExit('the premaster is silent; check the song for missing sounds')
    document = json.loads(song.read_text(encoding='utf-8'))
    gain = document['song']['masterGain']
    fitted = round(gain * 10 ** (target_db / 20) / peak, 6)
    # Match the song reader's mixer gain limit. Do not replace a valid song
    # with a value that the application will reject on the next render.
    if not math.isfinite(fitted) or not 0 < fitted <= 4.0:
        raise SystemExit('the required master gain is outside 0 to 4; adjust the lane gains first')
    document['song']['masterGain'] = fitted
    song.write_text(json.dumps(document, indent=1), encoding='utf-8')
    return fitted / gain


def main():
    args = sys.argv[1:]
    if len(args) < 2 or args[0].startswith('-'):
        raise SystemExit(__doc__)
    import audible  # it imports this module
    song, out = Path(args[0]), Path(args[1])
    out.mkdir(parents=True, exist_ok=True)
    jobs = int(args[args.index('--jobs') + 1]) if '--jobs' in args else None
    document = json.loads(song.read_text(encoding='utf-8'))
    meta = read_meta(song)
    groups = lane_groups(document, meta)
    lanes = render_lanes(document, groups, out, jobs)
    rate, premaster, stems = mix_down(lanes, lane_roles(groups, meta))
    scale = 1.0
    if '--fit-gain' in args:
        scale = fit_gain(song, float(np.abs(premaster).max()), float(args[args.index('--fit-gain') + 1]))
        print(f'master gain {json.loads(song.read_text(encoding="utf-8"))["song"]["masterGain"]}')
    write_wav(out / 'premaster.wav', rate, premaster * scale)
    for role, stem in stems.items():
        write_wav(out / f'stem-{role}.wav', rate, stem * scale)
    lines = audible.audibility(document, groups, rate, premaster.mean(axis=1),
                               lambda name: read_wav(lanes[name])[1].mean(axis=1))
    (out / 'audible.txt').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    if '--no-master' not in args:
        run(song, out / 'master.wav')


if __name__ == '__main__':
    main()
