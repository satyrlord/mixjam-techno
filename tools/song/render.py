"""Render a song pass: every lane alone, then the master.

python -I tools/song/render.py SONG.mixjam OUT_DIR [--fit-gain DBFS] [--jobs N] [--no-master]
    [--cache DIR | --no-cache]

The tool renders each lane with its twin and its FX returns, without the board.
It writes each lane to OUT_DIR/lane-NN.wav. The default is half the number of
processors, with a maximum of 8 concurrent renders. From these files, it makes
OUT_DIR/premaster.wav, one OUT_DIR/stem-ROLE.wav per named role, and
OUT_DIR/audible.txt (see audible.py). It also renders OUT_DIR/master.wav with
the board. --no-master skips the master render.

--fit-gain sets the song's master gain so that the premaster peaks at DBFS (for example -0.5) and
saves the song before the master render. Without track keys, the premaster and
stems scale to the new gain. With track keys, the tool renders the lanes again
after each gain change, up to five times. It reports any remaining difference
from the target.

Lane names and roles come from the `.meta.json` beside the song, which mixjam.py
writes. Each used track without a lane name is its own lane. Without the file,
the tool writes no role stems. The premaster and stems are sums of separate
lane renders. An FX return that limits or saturates can differ from a render of
the whole mix. A group with external keys uses the native track/FX stem
exporter. Key tracks stay separate from the output and remain in the full song
context. Stem export rejects an out-of-range track or FX part. Lower that
part's gain before you try again.

Lane renders are cached in DIR (default: lane-cache beside OUT_DIR). A lane
whose tracks, source audio, FX buses, tempo, master gain, and renderer build did
not change since an earlier render comes from the cache. Only format-18 and 19 songs
with direct absolute imported WAV paths and no library, missing-source, or
conflict dependencies use the cache. The cache keeps the most recently used
4 GB in its own versioned subfolder. --no-cache renders every lane.

The renderer is the release build in target/release. It runs headless and plays
no sound. Each render keeps its scan cache in a `.data` folder under OUT_DIR.
"""
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
import wave
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from tempfile import NamedTemporaryFile, TemporaryDirectory

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import env  # noqa: E402
from mixjam import (MAX_BARS, MAX_SECTION_NAME, TICKS_PER_BAR,
                    board_off as _board_off, read_wav, write_wav)  # noqa: E402

# Lane renders kept for later rounds; the lanes of a 7-minute song take about 1.6 GB.
CACHE_LIMIT = 4 * 2 ** 30
CACHE_VERSION = 'lanes-v2'


def run(song_path, wav_path, data=None):
    """Render one song file to a WAV with the release build; `data` is its scan-cache folder."""
    data = data or Path(wav_path).resolve().parent / '.data'
    environment = dict(os.environ, MIXJAM_TECHNO_DATA_DIR=str(data))
    result = subprocess.run([str(env.exe()), 'render', '--mix', str(song_path), '--output', str(wav_path)],
                            capture_output=True, text=True, env=environment)
    if result.returncode != 0:
        raise SystemExit(f'Cannot render {song_path}: {result.stdout}{result.stderr}')


def run_stems(song_path, folder, data):
    """Render track and FX parts with complete key context; False when no track sounds."""
    environment = dict(os.environ, MIXJAM_TECHNO_DATA_DIR=str(data))
    result = subprocess.run([str(env.exe()), 'render', '--mix', str(song_path), '--stems', str(folder)],
                            capture_output=True, text=True, env=environment)
    if result.returncode != 0:
        if result.stderr.strip() == 'No track sounds: add clips, or check Mute, Solo, and L/R.':
            return False
        raise SystemExit(f'Cannot render stems for {song_path}: {result.stdout}{result.stderr}')
    return True


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
    """The premaster with only `tracks` playing, for a group without external keys."""
    part = board_off(copy(document))
    for index, lane in enumerate(part['lanes'], 1):
        if index not in tracks:
            lane['placements'] = []
            lane.pop('automation', None)
    return part


def ducker_keys(document, tracks):
    """The key tracks required by the group's lane duckers and FX returns."""
    lanes = document['lanes']
    keys = {lanes[t - 1]['ducker']['key'] for t in tracks
            if lanes[t - 1].get('ducker', {}).get('on')}
    keys.update(bus['module']['duckKey'] for bus in document['fxBuses']
                if bus['module'].get('duckKey', 0) and bus['powered'])
    return keys


def keyed_document(document, tracks):
    """The premaster with every track playing and only `tracks` feeding the FX buses."""
    part = board_off(copy(document))
    part['song']['title'] = 'lane'
    for index, lane in enumerate(part['lanes'], 1):
        if index not in tracks:
            lane['sends'] = [0.0] * 4
            lane.get('automation', {}).pop('sends', None)
    return part


def keyed_parts(document, tracks, path, wav, data):
    """Extract the selected dry tracks and their FX; key audio stays in separate files."""
    path.write_text(json.dumps(keyed_document(document, tracks)), encoding='utf-8')
    with TemporaryDirectory(prefix=path.stem + '-parts-', dir=wav.parent) as directory:
        folder = Path(directory)
        if not run_stems(path, folder, data):
            # Native stem export has no parts for a silent song. Its whole mix is silence too.
            run(path, wav, data)
            path.unlink()
            return
        files = sorted(folder.glob('*.wav'))
        if not files:
            raise SystemExit(f'The stem render produced no audio for {path}.')
        rate, first = read_wav(files[0])
        total = np.zeros_like(first)
        selected = {f'lane - Track {track:02d}.wav' for track in tracks}
        for file in files:
            if file.name in selected or file.name.startswith('lane - FX '):
                part_rate, samples = read_wav(file)
                if part_rate != rate:
                    raise SystemExit(f'The stem sample rates differ for {path}.')
                total = add(total, samples)
        write_wav(wav, rate, total)
    path.unlink()


def exe_stamp():
    """Size and time of the renderer, so that a new build does not reuse older lane renders."""
    try:
        stat = env.exe().stat()
    except (OSError, SystemExit):
        return ''
    return f'{stat.st_size}:{stat.st_mtime_ns}'


def cacheable_sections(document):
    """Only valid sections may be left out of the audio cache key.

    Match the native song reader and engine.sections_error. Invalid sections
    must reach the native renderer's validation, even with --no-master.
    """
    song = document['song']
    if 'sections' not in song:
        return True
    sections = song['sections']
    if document['formatVersion'] < 19 or not isinstance(sections, list):
        return False
    previous = -1
    for section in sections:
        if not isinstance(section, dict) or section.keys() != {'startTick', 'name'}:
            return False
        tick, name = section['startTick'], section['name']
        if type(tick) is not int or not previous < tick < MAX_BARS * TICKS_PER_BAR:
            return False
        if (not isinstance(name, str) or name != name.strip()
                or not 1 <= len(name) <= MAX_SECTION_NAME
                or any(ord(c) < 32 or 127 <= ord(c) < 160 or 0xD800 <= ord(c) <= 0xDFFF for c in name)):
            return False
        previous = tick
    return True


def source_fingerprints(document):
    """Hash directly linked WAVs; None when native source resolution is needed.

    Read each distinct used file once per call. A path alone does not identify
    audio: the owner can replace its contents between rounds.
    """
    if document.get('formatVersion') not in (18, 19) or any(document.get(field) for field in
                                                ('libraries', 'missingSamples', 'sampleConflicts')):
        return None
    if not cacheable_sections(document):
        return None
    samples = {sample['id']: sample for sample in document['samples']}
    used = {placement['sampleId'] for lane in document['lanes'] for placement in lane['placements']}
    fingerprints, paths = {}, {}
    for sample_id in used:
        sample = samples.get(sample_id)
        if sample_id < 1_000_000 or sample is None or sample.get('origin') is not None:
            return None
        path = Path(sample.get('path', ''))
        if not path.is_absolute() or not sample.get('sourceBpm'):
            return None
        try:
            if path not in paths:
                digest = hashlib.sha256()
                with path.open('rb') as source:
                    for block in iter(lambda: source.read(1024 * 1024), b''):
                        digest.update(block)
                paths[path] = digest.hexdigest()
        except OSError:
            return None
        fingerprints[sample_id] = paths[path]
    return fingerprints


def lane_key(part, tracks, keyed, fingerprints=None):
    """A hash of what the lane render of `part` depends on.

    Placements name their sample by its record, not by its id, because the ids follow the
    order in which a script adds sounds. Global Solo and master gain also affect
    the output. Cached PCM cannot be rescaled after clipping or nonlinear FX.
    A keyed lane depends on every track.
    """
    if fingerprints is None:
        fingerprints = source_fingerprints(part)
    renderer = exe_stamp()
    if fingerprints is None or not renderer:
        return None
    samples = {s['id']: {k: v for k, v in s.items() if k != 'id'} for s in part['samples']}
    lanes = {}
    for index, lane in enumerate(part['lanes'], 1):
        if keyed or index in tracks:
            placements = [dict({k: v for k, v in p.items() if k not in ('id', 'sampleId')},
                               sample=samples[p['sampleId']], audio=fingerprints[p['sampleId']])
                          for p in lane['placements']]
            lanes[index] = dict(lane, placements=placements)
    # Titles and section bookmarks do not change the sound.
    song = {k: v for k, v in part['song'].items() if k not in ('title', 'sections')}
    context = {k: v for k, v in part.items() if k not in
               ('song', 'master', 'fxBuses', 'lanes', 'samples', 'createdAt', 'modifiedAt')}
    blob = json.dumps(dict(formatVersion=part['formatVersion'], keyed=keyed, tracks=sorted(tracks),
                           anySolo=any(lane['solo'] for lane in part['lanes']), song=song, master=part['master'],
                           fxBuses=part['fxBuses'], context=context, lanes=lanes,
                           renderer=renderer), sort_keys=True)
    return hashlib.sha256(blob.encode('utf-8')).hexdigest()[:32]


def atomic_copy(source, destination, validate=False):
    """Publish a complete independent copy. A failed copy leaves its destination intact."""
    with NamedTemporaryFile(dir=destination.parent, suffix='.tmp', delete=False) as temporary:
        path = Path(temporary.name)
    try:
        shutil.copyfile(source, path)
        if validate:
            with wave.open(str(path), 'rb') as audio:
                if audio.getnchannels() != 2 or audio.getsampwidth() != 2 or audio.getnframes() <= 0:
                    raise ValueError('The cached lane is not a stereo PCM16 WAV.')
                remaining = audio.getnframes() * 4
                while remaining:
                    block = audio.readframes(min(262144, (remaining + 3) // 4))
                    if not block:
                        raise ValueError('The cached lane WAV is incomplete.')
                    remaining -= len(block)
        os.replace(path, destination)
    finally:
        path.unlink(missing_ok=True)


def from_cache(cache, key, wav):
    """Copy a complete cached lane to `wav`; treat a missing or damaged entry as a miss."""
    entry = cache / CACHE_VERSION / f'{key}.wav'
    try:
        atomic_copy(entry, wav, validate=True)
    except (OSError, EOFError, ValueError, wave.Error):
        return False
    try:
        os.utime(entry)
    except OSError:
        pass  # Another render can prune the entry after the copy.
    return True


def to_cache(cache, key, wav):
    """Publish one complete lane in the cache, without sharing its writable file."""
    folder = cache / CACHE_VERSION
    try:
        folder.mkdir(parents=True, exist_ok=True)
        atomic_copy(wav, folder / f'{key}.wav')
    except OSError:
        pass  # Cache failure must not discard a successful render.


def prune_cache(cache, limit=CACHE_LIMIT):
    """Delete the least recently used renders until the cache holds at most `limit` bytes."""
    entries = []
    for entry in (cache / CACHE_VERSION).glob('*.wav'):
        if len(entry.stem) != 32 or any(char not in '0123456789abcdef' for char in entry.stem):
            continue
        try:
            stat = entry.stat()
            entries.append((stat.st_mtime, stat.st_size, entry))
        except OSError:
            continue
    entries.sort()
    total = sum(size for _, size, _ in entries)
    for _, size, entry in entries:
        if total <= limit:
            break
        try:
            entry.unlink(missing_ok=True)
            total -= size
        except OSError:
            continue


def render_lanes(document, groups, out, jobs=None, cache=None):
    """Render every lane alone, `jobs` at a time; lane name to its WAV.

    With a `cache` folder, a lane whose render inputs did not change since an earlier render
    comes from the cache instead of a new render.
    """
    jobs = jobs or max(1, min(8, (os.cpu_count() or 2) // 2))
    fingerprints = source_fingerprints(document) if cache else None

    def one(item):
        name, tracks = item
        path = out / f'lane-{min(tracks):02d}.mixjam'
        wav = path.with_suffix('.wav')
        keys = ducker_keys(document, tracks) - set(tracks)
        key = None
        if cache and fingerprints is not None:
            part = solo(document, tracks) if not keys else keyed_document(document, tracks)
            key = lane_key(part, tracks, bool(keys), fingerprints)
            if key is not None and from_cache(cache, key, wav):
                return name, wav
        # The lane file can be a link to a cache entry; replace it rather than write through it.
        wav.unlink(missing_ok=True)
        if not keys:
            path.write_text(json.dumps(solo(document, tracks)), encoding='utf-8')
            run(path, wav, out / '.data' / path.stem)
            path.unlink()
        else:
            keyed_parts(document, tracks, path, wav, out / '.data' / path.stem)
        # Do not publish under an earlier identity when a source or the renderer
        # changed while the native command ran.
        if cache and key is not None and key == lane_key(part, tracks, bool(keys)):
            to_cache(cache, key, wav)
        return name, wav

    with ThreadPoolExecutor(jobs) as pool:
        lanes = dict(pool.map(one, groups.items()))
    if cache:
        prune_cache(cache)
    return lanes


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
        raise SystemExit('The gain target must be finite and at or below 0 dBFS.')
    if peak <= 0:
        raise SystemExit('The premaster is silent. Check the song for missing sounds.')
    document = json.loads(song.read_text(encoding='utf-8'))
    gain = document['song']['masterGain']
    fitted = round(gain * 10 ** (target_db / 20) / peak, 6)
    # Match the song reader's mixer gain limit. Do not replace a valid song
    # with a value that the application will reject on the next render.
    if not math.isfinite(fitted) or not 0 < fitted <= 4.0:
        raise SystemExit('The required master gain is outside 0 to 4. Adjust the lane gains first.')
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
    cache = None if '--no-cache' in args else (
        Path(args[args.index('--cache') + 1]) if '--cache' in args else out.resolve().parent / 'lane-cache')
    document = json.loads(song.read_text(encoding='utf-8'))
    meta = read_meta(song)
    groups = lane_groups(document, meta)
    lanes = render_lanes(document, groups, out, jobs, cache)
    rate, premaster, stems = mix_down(lanes, lane_roles(groups, meta))
    scale = 1.0
    if '--fit-gain' in args:
        target = float(args[args.index('--fit-gain') + 1])
        if ducker_keys(document, range(1, len(document['lanes']) + 1)):
            for _ in range(5):
                peak = float(np.abs(premaster).max())
                if peak > 0 and math.isfinite(target) and target <= 0 and abs(20 * math.log10(peak) - target) <= 0.05:
                    break
                fit_gain(song, peak, target)
                document = json.loads(song.read_text(encoding='utf-8'))
                lanes = render_lanes(document, groups, out, jobs, cache)
                rate, premaster, stems = mix_down(lanes, lane_roles(groups, meta))
            peak = float(np.abs(premaster).max())
            actual = 20 * math.log10(peak) if peak > 0 else -math.inf
            if abs(actual - target) > 0.05:
                print(f'The gain fit stopped after five changes. The peak is {actual:.2f} dBFS '
                      f'and the target is {target:.2f} dBFS.',
                      file=sys.stderr)
        else:
            scale = fit_gain(song, float(np.abs(premaster).max()), target)
        print(f'master gain {json.loads(song.read_text(encoding="utf-8"))["song"]["masterGain"]}')
    write_wav(out / 'premaster.wav', rate, premaster * scale)
    for role, stem in stems.items():
        write_wav(out / f'stem-{role}.wav', rate, stem * scale)
    lines = audible.audibility(document, groups, rate, premaster.mean(axis=1),
                               lambda name: read_wav(lanes[name])[1].mean(axis=1), jobs)
    (out / 'audible.txt').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    if '--no-master' not in args:
        run(song, out / 'master.wav')


if __name__ == '__main__':
    main()
