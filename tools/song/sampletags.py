"""Measure the audio of every indexed WAV and tag it: sampletags.py tag | find | summary

python -I tools/song/sampletags.py tag [--jobs N] [--limit N]
python -I tools/song/sampletags.py find [WORD ...] --tag TAG [--tag TAG ...] [--not TAG ...] [--key KEY]
                                        [--kind KIND] [--library TEXT] [--limit N]
python -I tools/song/sampletags.py summary

`tag` reads the audio of each WAV that `samples.py index` listed, measures it, and writes one row per
file to SAMPLE_TAGS (default: tags.tsv beside the index). It reads at most the first 20 seconds of a
file. A later run measures only files whose size or modification time changed. It saves its work
every 10,000 files, so a stopped run resumes where it stopped.

Measured, per file:
- level: RMS and peak (dBFS), and the delay before the sound starts (ms);
- tone: the share of energy under 120 Hz and over 5 kHz, the spectral centroid (Hz) and flatness
  (0 is a pure tone, 1 is white noise);
- attack: the time from 10% to 90% of the first second's peak (ms);
- stereo: the side level against the mid (dB);
- pitch: a 12-bin chroma, the best-fitting key and how well it fits (0-1), and the tuning offset of
  the strong spectral peaks from equal temperament (cents).

Tags derived from those values: dark, bright, sub, noise, tonal, soft, hard, wide, mono, quiet,
late-start (sound starts over 30 ms late, as in MP3-decoded files), off-tune (a tonal sound more
than 20 cents from equal temperament). The thresholds are in TAG_RULES. Measurements describe the
file, not its musical worth: a person or a critic still judges it.

`find` searches the tags joined with the index. --tag and --not can repeat.
"""
import argparse
import csv
import os
import struct
import sys
from collections import Counter
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import env  # noqa: E402
import samples  # noqa: E402

COLUMNS = ['root', 'path', 'bytes', 'mtime', 'rms_db', 'peak_db', 'lead_ms', 'low', 'bright', 'centroid',
           'flatness', 'attack_ms', 'width_db', 'key', 'key_fit', 'tuning', 'chroma', 'tags', 'error']
NOTES = samples.NOTES
SECONDS = 20.0
CHECKPOINT = 10_000
# Krumhansl-Kessler key profiles, from C.
MAJOR = np.array([6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88])
MINOR = np.array([6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17])
TAG_RULES = {
    'dark': lambda m: m['centroid'] < 700,
    'bright': lambda m: m['centroid'] > 3500 or m['bright'] > 0.15,
    'sub': lambda m: m['low'] > 0.6,
    'noise': lambda m: m['flatness'] > 0.25,
    'tonal': lambda m: m['flatness'] < 0.05 and m['key_fit'] > 0.6,
    'soft': lambda m: m['attack_ms'] > 80,
    'hard': lambda m: m['attack_ms'] < 5,
    'wide': lambda m: m['width_db'] > -8,
    'mono': lambda m: m['width_db'] < -40,
    'quiet': lambda m: m['rms_db'] < -35,
    'late-start': lambda m: m['lead_ms'] > 30,
    'off-tune': lambda m: m['flatness'] < 0.05 and m['key_fit'] > 0.6 and abs(m['tuning']) > 20,
}


def read_audio(path, seconds=SECONDS):
    """(rate, float frames x channels) of the first `seconds` of a PCM or float WAV."""
    with open(path, 'rb') as f:
        head = f.read(12)
        if head[:4] != b'RIFF' or head[8:12] != b'WAVE':
            raise ValueError('not a RIFF WAVE file')
        size = os.fstat(f.fileno()).st_size
        position, fmt = 12, None
        while position + 8 <= size:
            f.seek(position)
            name, length = f.read(4), struct.unpack('<I', f.read(4))[0]
            if name == b'fmt ':
                data = f.read(min(length, 40))
                tag, channels, rate, _, align, bits = struct.unpack('<HHIIHH', data[:16])
                if tag == 0xFFFE and len(data) >= 26:
                    tag = struct.unpack('<H', data[24:26])[0]
                fmt = (tag, channels, rate, align, bits)
            elif name == b'data':
                if fmt is None:
                    raise ValueError('data before fmt')
                tag, channels, rate, align, bits = fmt
                available = min(length, size - position - 8)
                frames = min(available // align, int(rate * seconds))
                raw = f.read(frames * align)
                return rate, decode(raw, tag, channels, bits)
            position += 8 + length + (length & 1)
    raise ValueError('no data chunk')


def decode(raw, tag, channels, bits):
    if tag == 1 and bits == 16:
        x = np.frombuffer(raw, '<i2').astype(np.float32) / 32768
    elif tag == 1 and bits == 24:
        b = np.frombuffer(raw, np.uint8).reshape(-1, 3).astype(np.int32)
        ints = b[:, 0] | (b[:, 1] << 8) | (b[:, 2] << 16)
        x = np.where(ints >= 1 << 23, ints - (1 << 24), ints).astype(np.float32) / (1 << 23)
    elif tag == 1 and bits == 32:
        x = np.frombuffer(raw, '<i4').astype(np.float32) / (1 << 31)
    elif tag == 1 and bits == 8:
        x = (np.frombuffer(raw, np.uint8).astype(np.float32) - 128) / 128
    elif tag == 3 and bits == 32:
        x = np.frombuffer(raw, '<f4').astype(np.float32)
    elif tag == 3 and bits == 64:
        x = np.frombuffer(raw, '<f8').astype(np.float32)
    else:
        raise ValueError(f'unsupported format {tag}/{bits}-bit')
    return x[: len(x) // channels * channels].reshape(-1, channels)


def measure(rate, x):
    """The measurements of one file's audio (frames x channels, floats)."""
    mono = x.mean(axis=1).astype(np.float64)
    peak = float(np.abs(x).max()) if x.size else 0.0
    if peak < 1e-6 or len(mono) < rate // 20:
        raise ValueError('silent or shorter than 50 ms')
    rms = float(np.sqrt(np.mean(mono ** 2)))
    lead = int(np.argmax(np.abs(mono) > peak * 0.01))
    # Tone from averaged 4096-point frames.
    frame = 4096
    usable = len(mono) // frame * frame
    if usable:
        frames = mono[:usable].reshape(-1, frame) * np.hanning(frame)
        power = (np.abs(np.fft.rfft(frames, axis=1)) ** 2).mean(axis=0)
    else:
        power = np.abs(np.fft.rfft(mono * np.hanning(len(mono)), n=frame)) ** 2
    freqs = np.fft.rfftfreq(frame, 1 / rate)
    total = power.sum() + 1e-30
    band = (freqs > 100) & (freqs < 8000)
    flatness = float(np.exp(np.mean(np.log(power[band] + 1e-30))) / (np.mean(power[band]) + 1e-30))
    # Attack: 5 ms envelope of the first second.
    window = max(1, rate // 200)
    envelope = np.convolve(np.abs(mono[lead:lead + rate]), np.ones(window) / window, mode='same')
    top = envelope.max()
    attack = (np.argmax(envelope >= 0.9 * top) - np.argmax(envelope >= 0.1 * top)) / rate * 1000
    # Stereo.
    if x.shape[1] >= 2:
        mid, side = (x[:, 0] + x[:, 1]) / 2, (x[:, 0] - x[:, 1]) / 2
        width = 10 * np.log10((np.mean(side.astype(np.float64) ** 2) + 1e-20) / (np.mean(mid.astype(np.float64) ** 2) + 1e-20))
    else:
        width = -99.0
    # Pitch from one long FFT of up to 8 s (fine enough for cents at 150 Hz and up).
    piece = mono[lead:lead + rate * 8]
    spectrum = np.abs(np.fft.rfft(piece * np.hanning(len(piece)))) ** 2
    fine = np.fft.rfftfreq(len(piece), 1 / rate)
    keep = (fine > 80) & (fine < 4000)
    midi = 69 + 12 * np.log2(fine[keep] / 440)
    chroma = np.bincount(np.round(midi).astype(int) % 12, weights=spectrum[keep], minlength=12)
    chroma = chroma / (chroma.sum() + 1e-30)
    fits = [(np.corrcoef(np.roll(profile, shift), chroma)[0, 1], shift, quality)
            for quality, profile in (('', MAJOR), ('m', MINOR)) for shift in range(12)]
    fit, shift, quality = max(fits)
    strong_band = (fine > 150) & (fine < 4000)
    s, f = spectrum[strong_band], fine[strong_band]
    strong = s > s.max() * 0.02 if s.size else np.array([], bool)
    deviation = (69 + 12 * np.log2(f[strong] / 440)) % 1
    tuning = float(np.angle(np.sum(s[strong] * np.exp(2j * np.pi * deviation))) / (2 * np.pi) * 100) if strong.any() else 0.0
    return dict(rms_db=20 * np.log10(rms), peak_db=20 * np.log10(peak), lead_ms=lead / rate * 1000,
                low=power[freqs < 120].sum() / total, bright=power[freqs > 5000].sum() / total,
                centroid=float((freqs * power).sum() / total), flatness=flatness, attack_ms=float(attack),
                width_db=float(width), key=NOTES[shift] + quality, key_fit=float(fit if np.isfinite(fit) else 0),
                tuning=tuning, chroma=chroma)


def tags_of(m):
    return [name for name, rule in TAG_RULES.items() if rule(m)]


def row_for(item):
    """One tags row; never raises, so one bad file cannot stop a run."""
    root, path, size, mtime = item
    row = dict(root=root, path=path, bytes=size, mtime=mtime)
    try:
        m = measure(*read_audio(Path(root) / path))
        if not all(np.isfinite(v) for k, v in m.items() if k not in ('key', 'chroma'))                 or not np.isfinite(m['chroma']).all():
            raise ValueError('a measurement is not a number')
        values = {k: (round(v, 4) if isinstance(v, float) else v) for k, v in m.items() if k != 'chroma'}
        values['chroma'] = ','.join(str(int(round(v * 1000))) for v in m['chroma'])
        values['tags'] = ' '.join(tags_of(m))
    except Exception as error:  # noqa: BLE001 - the row records why the file has no tags
        row['error'] = str(error)[:120]
        return row
    row.update(values)
    return row


def tags_path():
    return env.path('SAMPLE_TAGS', required=False) or samples.index_path().with_name('tags.tsv')


def load(path=None):
    path = Path(path or tags_path())
    if not path.exists():
        raise SystemExit(f'{path} does not exist. Build it with: python -I tools/song/sampletags.py tag')
    with open(path, encoding='utf-8', newline='') as f:
        return list(csv.DictReader(f, delimiter='\t'))


def save(rows, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_suffix('.tmp')
    with open(temporary, 'w', encoding='utf-8', newline='') as f:
        writer = csv.DictWriter(f, COLUMNS, delimiter='\t', extrasaction='ignore', lineterminator='\n')
        writer.writeheader()
        writer.writerows({k: ('' if v is None else v) for k, v in row.items()} for row in rows)
    temporary.replace(target)


def tag(jobs, limit=None):
    target = tags_path()
    old = {(r['root'], r['path']): r for r in load(target)} if target.exists() else {}
    done, todo = {}, []
    for row in samples.load():
        if row['type'] != 'wav' or row['kind'] == 'unreadable':
            continue
        key = (row['root'], row['path'])
        previous = old.get(key)
        if previous and previous['bytes'] == row['bytes'] and previous['mtime'] == row['mtime']:
            done[key] = previous
        else:
            todo.append((row['root'], row['path'], row['bytes'], row['mtime']))
    todo = todo[:limit] if limit else todo
    print(f'{len(done)} unchanged, {len(todo)} to measure', file=sys.stderr)
    with ProcessPoolExecutor(jobs) as pool:
        for i, row in enumerate(pool.map(row_for, todo, chunksize=32), 1):
            done[(row['root'], row['path'])] = row
            if i % CHECKPOINT == 0:
                save(done.values(), target)
                print(f'{i}/{len(todo)}', file=sys.stderr, flush=True)
    save(done.values(), target)
    errors = sum(1 for r in done.values() if r.get('error'))
    print(f'{target}: {len(done)} files ({errors} without tags)')


def find(args):
    index = {(r['root'], r['path']): r for r in samples.load()}
    key = samples.parse_key(args.key) if args.key else None
    shown = 0
    for row in load():
        tags = set(row['tags'].split())
        if row['error'] or not set(args.tag) <= tags or set(args.not_) & tags:
            continue
        info = index.get((row['root'], row['path']), {})
        text = (row['root'] + '/' + row['path']).lower()
        if any(word.lower() not in text for word in args.words):
            continue
        if args.library and args.library.lower() not in row['root'].lower():
            continue
        if args.kind and info.get('kind') != args.kind:
            continue
        if key and key != row['key']:
            continue
        print(f"{float(row['rms_db']):6.1f} dB {row['key']:<4} fit {float(row['key_fit']):.2f} "
              f"{float(row['tuning']):+4.0f}c {row['tags']:<28} {Path(row['root']) / row['path']}")
        shown += 1
        if shown >= args.limit:
            print(f'(first {args.limit}; raise --limit for more)', file=sys.stderr)
            return


def summary():
    rows = load()
    for root in dict.fromkeys(r['root'] for r in rows):
        mine = [r for r in rows if r['root'] == root]
        counts = Counter(t for r in mine for t in r['tags'].split())
        errors = sum(1 for r in mine if r['error'])
        print(f'{root}: {len(mine)} measured, {errors} without tags; ' + ', '.join(f'{t} {n}' for t, n in counts.most_common()))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    commands = parser.add_subparsers(dest='command', required=True)
    run = commands.add_parser('tag')
    run.add_argument('--jobs', type=int, default=max(1, (os.cpu_count() or 2) - 2))
    run.add_argument('--limit', type=int)
    search = commands.add_parser('find')
    search.add_argument('words', nargs='*')
    search.add_argument('--tag', action='append', default=[])
    search.add_argument('--not', dest='not_', action='append', default=[])
    search.add_argument('--key')
    search.add_argument('--kind')
    search.add_argument('--library')
    search.add_argument('--limit', type=int, default=50)
    commands.add_parser('summary')
    args = parser.parse_args()
    if args.command == 'tag':
        tag(args.jobs, args.limit)
    elif args.command == 'find':
        find(args)
    else:
        summary()


if __name__ == '__main__':
    main()
