"""Index and search the sample libraries: samples.py index | find | summary

python -I tools/song/samples.py index [--jobs N]
python -I tools/song/samples.py find [WORD ...] [--bpm N] [--any-octave] [--key KEY] [--kind KIND]
                                     [--loops | --shots] [--library TEXT] [--midi] [--limit N]
python -I tools/song/samples.py summary

`index` walks every library in SAMPLES_FOLDER_1 to SAMPLES_FOLDER_9 (see .env.example) and writes
one row per WAV and MIDI file to SAMPLE_INDEX (default local-data/samples/index.tsv). It reads WAV
headers only, never the audio. Later runs read headers again when the file's size or modification
time changes, or when its pack metadata changes.

Per file: length, rate, channels and bits; tempo and key from the name, the parent folders, a
pack's BPM.INF (its tempo and its chord table for numbered files) or the WAV's loop chunk; the
root note of a one-shot; whole bars at that tempo; and a kind (kick, pad, lead, ...) from the path.
Each value says where it came from, since a name can be wrong; check a sound by ear or measure it
before you rely on it.

`find` prints the matching files, one per line: seconds, tempo, key or chord, kind, full path.
Every WORD must appear in the path (case ignored). --bpm N matches within 0.5 BPM; --any-octave
also matches half and double tempo. --key accepts Am, A min, A minor, F#m, Gb and similar.
"""
import argparse
import csv
import json
import os
import re
import struct
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import env  # noqa: E402

DEFAULT_INDEX = 'local-data/samples/index.tsv'
COLUMNS = ['root', 'path', 'type', 'bytes', 'mtime', 'channels', 'rate', 'bits', 'seconds', 'bpm', 'bpm_from',
           'key', 'key_from', 'note', 'chord', 'beats', 'bars', 'kind', 'pack']
NOTES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B']
FLATS = {'Db': 'C#', 'Eb': 'D#', 'Gb': 'F#', 'Ab': 'G#', 'Bb': 'A#', 'Cb': 'B', 'Fb': 'E', 'E#': 'F', 'B#': 'C'}
# Kind keywords, matched against the file name first and then the folders; the first kind wins.
KINDS = [
    ('kick', ('kick', 'kicks', 'bassdrum', 'bassdrums', 'bd')),
    ('snare', ('snare', 'snares', 'rim', 'rimshot')),
    ('clap', ('clap', 'claps')),
    ('hat', ('hat', 'hats', 'hihat', 'hihats', 'hh', 'oh', 'ch')),
    ('cymbal', ('cymbal', 'cymbals', 'crash', 'ride', 'crashrev')),
    ('perc', ('perc', 'percs', 'percussion', 'tom', 'toms', 'shaker', 'conga', 'bongo', 'tamb', 'tambourine')),
    ('drums', ('drum', 'drums', 'beat', 'beats', 'groove', 'top', 'tops', 'break', 'breaks')),
    ('bass', ('bass', 'basses', 'bassline', '808', 'sub', 'reese')),
    ('pad', ('pad', 'pads', 'atmo', 'atmos', 'atmosphere', 'ambient', 'drone', 'drones', 'texture', 'textures',
             'layer', 'layers', 'sphere', 'spheres')),
    ('arp', ('arp', 'arps', 'arpeggio', 'sequence', 'sequences', 'seq', 'seqs', 'pluck', 'plucks')),
    ('chord', ('chord', 'chords', 'stab', 'stabs', 'keys', 'piano', 'organ', 'rhodes')),
    ('lead', ('lead', 'leads', 'synth', 'synths', 'melody', 'melodies', 'melodic', 'hook', 'riff', 'riffs')),
    ('vocal', ('vocal', 'vocals', 'vox', 'voice', 'voices', 'choir', 'spoken', 'rap', 'raps')),
    ('fx', ('fx', 'sfx', 'effect', 'effects', 'sweep', 'sweeps', 'riser', 'risers', 'impact', 'impacts',
            'noise', 'swell', 'swells', 'boom', 'booms', 'downlifter', 'uplifter', 'transition', 'spaceships')),
    ('guitar', ('guitar', 'guitars')),
    ('strings', ('string', 'strings', 'violin', 'cello', 'harp', 'harps')),
    ('brass', ('brass', 'horn', 'horns', 'trumpet', 'sax')),
    ('extra', ('xtra', 'extra', 'extras', 'special', 'specials')),
]

_KEY = r'([A-G])(#|b|♯|♭)?'
_QUALITY = r'((?i:min(?:or)?|maj(?:or)?)|m)'  # a capital M (FM, AM) is not minor
BPM_WORD = re.compile(r'(?<![\d.])(\d{2,3}(?:\.\d+)?)\s*-?\s*bpm', re.I)
PACK_STYLE = re.compile(r'_(\d{2,3})_([A-G][#b]?m?|X)_')  # PREFIX_140_Am_SET
TEMPO_KEY_END = re.compile(r'(?:^|[\s_-])(\d{2,3})[\s_-]+' + _KEY + r'\s?' + _QUALITY + r'?$')
KEY_TOKEN = re.compile(r'(?:^|[\s_\-(])' + _KEY + r'\s?' + _QUALITY + r'(?=$|[\s_\-).,])')
KEY_AFTER_BPM = re.compile(r'(?i:bpm)[\s_-]*' + _KEY + r'\s?' + _QUALITY + r'?(?=$|[\s_\-).,])')
ROOT_NOTE = re.compile(r'(?:root|note)\s*-?\s*' + _KEY + r'(?=$|[\s_\-).,\d])', re.I)
NOTE_END = re.compile(r'_' + _KEY + r'$')
NUMBER_END = re.compile(r'_(\d{2,3})$')
TRAILING_NUMBER = re.compile(r'(\d+)$')


def note_name(letter, accidental=''):
    accidental = {'♯': '#', '♭': 'b'}.get(accidental or '', accidental or '')
    name = letter.upper() + accidental
    return FLATS.get(name, name)


def key_name(letter, accidental, quality):
    """C, C#m, ... from a letter, an accidental and an optional min/maj/m suffix."""
    minor = bool(quality) and quality.lower().startswith('m') and not quality.lower().startswith('maj')
    return note_name(letter, accidental) + ('m' if minor else '')


def parse_key(text):
    """A key typed by a person (Am, A min, F#m, Gb, a minor) in the index's spelling, or None."""
    match = re.fullmatch(r'\s*' + _KEY + r'\s*(min(?:or)?|maj(?:or)?|m)?\s*', text[:1].upper() + text[1:], re.I)
    return key_name(*match.groups()) if match else None


def parse_name(text):
    """Tempo, key and root note that a file or folder name states: (bpm, key, note), each or None."""
    bpm = key = note = None
    if match := PACK_STYLE.search(text):
        bpm, token = float(match.group(1)), match.group(2)
        if token != 'X':
            key = key_name(token[0], token[1] if token[1:2] in ('#', 'b') else '', 'm' if token.endswith('m') else None)
    if bpm is None and (match := BPM_WORD.search(text)):
        bpm = float(match.group(1))
    if match := TEMPO_KEY_END.search(text):
        if 50 <= float(match.group(1)) <= 200:
            bpm = bpm or float(match.group(1))
            key = key or key_name(*match.groups()[1:])
    if key is None and (match := KEY_AFTER_BPM.search(text)):
        key = key_name(*match.groups())
    if key is None and (match := KEY_TOKEN.search(text)):
        key = key_name(*match.groups())
    if match := ROOT_NOTE.search(text):
        note = note_name(*match.groups())
    elif match := NOTE_END.search(text):
        note = note_name(*match.groups())
    if bpm is not None and not 40 <= bpm <= 300:
        bpm = None
    return bpm, key, note


def german_chord(name):
    """A chord-table entry (c, Dis, gis, H, b) as C minor = Cm, D# major = D#, ...; None if unreadable."""
    name = name.strip()
    if not name:
        return None
    minor = name[0].islower()
    low = name.lower()
    base = {'h': 'B', 'b': 'A#'}.get(low[0], low[0].upper())
    rest = low[1:]
    if base not in NOTES and base != 'A#':
        return None
    if rest in ('is',):
        base = note_name(base, '#')
    elif rest in ('es', 's'):
        base = note_name(base, 'b')
    elif rest:
        return None
    return base + ('m' if minor else '')


def read_pack_info(path):
    """A pack's BPM.INF: {'bpm': float or None, 'key': str or None, 'chords': {n: chord}}."""
    info = {'bpm': None, 'key': None, 'chords': {}}
    try:
        text = Path(path).read_text(encoding='latin-1')
    except OSError:
        return info
    for line in text.splitlines():
        name, _, value = line.partition('=')
        name, value = name.strip().lower(), value.strip()
        if name == 'bpm':
            try:
                bpm = float(value)
                info['bpm'] = bpm if 40 <= bpm <= 300 else None
            except ValueError:
                pass
        elif name == 'scale':
            info['key'] = german_chord(value)
        elif name.startswith('sample') and name[6:].isdigit():
            info['chords'][int(name[6:])] = german_chord(value)
    return info


def wav_header(path, size):
    """channels, rate, bits, seconds, loop tempo, loop beats, root note of a WAV; None when not a WAV."""
    with open(path, 'rb') as f:
        head = f.read(12)
        if len(head) < 12 or head[:4] != b'RIFF' or head[8:12] != b'WAVE':
            return None
        found = dict(channels=None, rate=None, bits=None, data=None, tempo=None, beats=None, note=None)
        position = 12
        while position + 8 <= size:
            f.seek(position)
            chunk = f.read(8)
            if len(chunk) < 8:
                break
            name, length = chunk[:4], struct.unpack('<I', chunk[4:])[0]
            if name == b'fmt ' and length >= 16:
                data = f.read(16)
                _, found['channels'], found['rate'], _, _, found['bits'] = struct.unpack('<HHIIHH', data)
            elif name == b'data':
                found['data'] = min(length, size - position - 8)
            elif name == b'acid' and length >= 24:
                flags, root, _, _, beats, _, _, tempo = struct.unpack('<IHHfIHHf', f.read(24))
                if not flags & 1 and 40 <= tempo <= 300 and beats:
                    found['tempo'], found['beats'] = round(tempo, 3), beats
                if flags & 2 and 0 < root < 128:
                    found['note'] = NOTES[root % 12]
            elif name == b'smpl' and length >= 16 and found['note'] is None:
                unity = struct.unpack('<I', f.read(16)[12:16])[0]
                if 0 < unity < 128 and unity != 60:  # 60 is most tools' unset default
                    found['note'] = NOTES[unity % 12]
            position += 8 + length + (length & 1)
        seconds = None
        if found['rate'] and found['channels'] and found['bits'] and found['data'] is not None:
            frame = found['channels'] * max(1, found['bits'] // 8)
            seconds = found['data'] / frame / found['rate']
        return found['channels'], found['rate'], found['bits'], seconds, found['tempo'], found['beats'], found['note']


def kind_of(relative):
    """The kind of sound a path names, from the file name first and then from the nearest folder."""
    parts = relative.split('/')
    for part in reversed(parts):
        words = set(re.findall(r'[a-z]+|\d+', re.sub(r'([a-z])([A-Z])', r'\1 \2', Path(part).stem).lower()))
        for kind, keywords in KINDS:
            if words & set(keywords):
                return kind
    return ''


def whole_bars(seconds, bpm):
    """Bars at this tempo when the length is within 1% of a whole number of beats, else None."""
    if not seconds or not bpm:
        return None
    beats = seconds * bpm / 60
    whole = round(beats)
    if whole >= 2 and abs(beats - whole) <= 0.01 * whole:
        return round(whole / 4, 2)
    return None


class Packs:
    """The nearest BPM.INF above a folder, cached per folder, never above the library root."""

    def __init__(self, root):
        self.root, self.cache = Path(root), {}

    def find(self, folder):
        folder = Path(folder)
        if folder in self.cache:
            return self.cache[folder]
        found = None
        try:
            names = {p.name.lower(): p for p in folder.iterdir() if p.is_file() and p.name.lower() == 'bpm.inf'}
        except OSError:
            names = {}
        if 'bpm.inf' in names:
            found = read_pack_info(names['bpm.inf'])
        elif folder != self.root and self.root in folder.parents:
            found = self.find(folder.parent)
        self.cache[folder] = found
        return found

    def signature(self, folder):
        return json.dumps(self.find(folder), sort_keys=True)


def describe(root, relative, size, mtime, packs):
    """One index row for a file."""
    path = Path(root) / relative
    row = dict(root=str(root), path=relative, type='mid' if relative.lower().endswith(('.mid', '.midi')) else 'wav',
               bytes=size, mtime=mtime, pack=packs.signature(path.parent))
    header = None
    if row['type'] == 'wav':
        try:
            header = wav_header(path, size)
        except (OSError, struct.error):
            header = None
        if header is None:
            row['kind'] = 'unreadable'
            return row
        row['channels'], row['rate'], row['bits'], seconds, tempo, beats, note = header
        row['seconds'] = round(seconds, 3) if seconds is not None else None
        row['beats'], row['note'] = beats, note
    stem = Path(relative).stem
    bpm, key, name_note = parse_name(stem)
    row['bpm'], row['bpm_from'] = (bpm, 'name') if bpm else (None, None)
    row['key'], row['key_from'] = (key, 'name') if key else (None, None)
    row['note'] = row.get('note') or name_note
    for folder in list(Path(relative).parents)[:-1][:3]:
        folder_bpm, folder_key, folder_note = parse_name(folder.name)
        if not row['bpm'] and folder_bpm:
            row['bpm'], row['bpm_from'] = folder_bpm, 'folder'
        if not row['key'] and folder_key:
            row['key'], row['key_from'] = folder_key, 'folder'
        row['note'] = row['note'] or folder_note
    pack = packs.find(path.parent)
    if pack:
        if not row['bpm'] and pack['bpm']:
            row['bpm'], row['bpm_from'] = pack['bpm'], 'pack'
        if not row['key'] and pack['key']:
            row['key'], row['key_from'] = pack['key'], 'pack'
        if match := TRAILING_NUMBER.search(stem):
            row['chord'] = pack['chords'].get(int(match.group(1)))
    if header and header[4] and not row['bpm']:
        row['bpm'], row['bpm_from'] = header[4], 'header'
    if not row['bpm'] and (match := NUMBER_END.search(stem)):
        # A bare trailing number is a tempo only when the length is whole bars at it.
        guess = float(match.group(1))
        if 60 <= guess <= 200 and whole_bars(row.get('seconds'), guess):
            row['bpm'], row['bpm_from'] = guess, 'guess'
    row['bars'] = whole_bars(row.get('seconds'), row['bpm'])
    row['kind'] = kind_of(relative)
    return row


def libraries():
    roots = env.sample_folders()
    if not roots:
        env.require('SAMPLES_FOLDER_1')
    return roots


def index_path():
    return env.path('SAMPLE_INDEX', required=False) or env.ROOT / DEFAULT_INDEX


def load(path=None):
    path = Path(path or index_path())
    if not path.exists():
        raise SystemExit(f'{path} does not exist. Build it with: python -I tools/song/samples.py index')
    with open(path, encoding='utf-8', newline='') as f:
        return list(csv.DictReader(f, delimiter='\t'))


def walk(root):
    """(relative path, size, mtime) of every WAV and MIDI file below root."""
    pending = [Path(root)]
    while pending:
        folder = pending.pop()
        try:
            entries = list(os.scandir(folder))
        except OSError as error:
            print(f'skipped {folder}: {error}', file=sys.stderr)
            continue
        for entry in entries:
            if entry.is_dir(follow_symlinks=False):
                pending.append(Path(entry.path))
            elif entry.name.lower().endswith(('.wav', '.mid', '.midi')):
                stat = entry.stat()
                relative = Path(entry.path).relative_to(root).as_posix()
                yield relative, stat.st_size, stat.st_mtime_ns


def build(jobs):
    target = index_path()
    old = {}
    if target.exists():
        old = {(row['root'], row['path']): row for row in load(target)}
    rows, reused = [], 0
    for root in libraries():
        if not root.is_dir():
            print(f'skipped {root}: not a folder', file=sys.stderr)
            continue
        packs = Packs(root)
        todo = []
        for relative, size, mtime in walk(root):
            previous = old.get((str(root), relative))
            if (previous and previous['bytes'] == str(size) and previous['mtime'] == str(mtime)
                    and previous.get('pack') == packs.signature((root / relative).parent)):
                if previous['kind'] != 'unreadable':
                    previous['kind'] = kind_of(relative)
                rows.append(previous)
                reused += 1
            else:
                todo.append((relative, size, mtime))
        with ThreadPoolExecutor(jobs) as pool:
            rows.extend(pool.map(lambda item: describe(root, *item, packs), todo, chunksize=64))
        print(f'{root}: {len(todo)} read', file=sys.stderr)
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_suffix('.tmp')
    with open(temporary, 'w', encoding='utf-8', newline='') as f:
        writer = csv.DictWriter(f, COLUMNS, delimiter='\t', extrasaction='ignore', lineterminator='\n')
        writer.writeheader()
        writer.writerows({k: ('' if v is None else v) for k, v in row.items()} for row in rows)
    temporary.replace(target)
    print(f'{target}: {len(rows)} files ({reused} unchanged)')


def matches(row, args, key):
    if args.midi != (row['type'] == 'mid'):
        return False
    text = (row['root'] + '/' + row['path']).lower()
    if any(word.lower() not in text for word in args.words):
        return False
    if args.library and args.library.lower() not in row['root'].lower():
        return False
    if args.kind and row['kind'] != args.kind:
        return False
    if key and key not in (row['key'], row['chord']):
        return False
    bars = row['bars']
    if args.loops and not bars:
        return False
    if args.shots and (bars or not row['seconds'] or float(row['seconds']) > 4):
        return False
    if args.bpm:
        if not row['bpm']:
            return False
        bpm = float(row['bpm'])
        wanted = [args.bpm] + ([args.bpm / 2, args.bpm * 2] if args.any_octave else [])
        if all(abs(bpm - w) > 0.5 for w in wanted):
            return False
    return True


def find(args):
    key = parse_key(args.key) if args.key else None
    if args.key and key is None:
        raise SystemExit(f'Cannot read the key {args.key!r}; write it like Am, A min or F#.')
    shown = 0
    for row in load():
        if matches(row, args, key):
            seconds = f"{float(row['seconds']):6.2f}s" if row['seconds'] else '      -'
            bpm = f"{float(row['bpm']):g}" if row['bpm'] else '-'
            harmony = row['chord'] or row['key'] or row['note'] or '-'
            print(f"{seconds} {bpm:>6} {harmony:<4} {row['kind'] or '-':<8} {Path(row['root']) / row['path']}")
            shown += 1
            if shown >= args.limit:
                print(f'(first {args.limit}; raise --limit for more)', file=sys.stderr)
                return


def summary():
    rows = load()
    for root in dict.fromkeys(row['root'] for row in rows):
        mine = [row for row in rows if row['root'] == root]
        wavs = [row for row in mine if row['type'] == 'wav']
        def share(test):
            return f'{100 * sum(map(test, wavs)) / max(1, len(wavs)):.0f}%'
        kinds = Counter(row['kind'] or '?' for row in wavs).most_common(6)
        tempos = Counter(f"{float(row['bpm']):g}" for row in wavs if row['bpm']).most_common(5)
        print(f"{root}: {len(wavs)} WAV, {len(mine) - len(wavs)} MIDI; tempo {share(lambda r: bool(r['bpm']))}, "
              f"key {share(lambda r: bool(r['key'] or r['chord']))}, loops {share(lambda r: bool(r['bars']))}, "
              f"unreadable {sum(r['kind'] == 'unreadable' for r in wavs)}")
        print('    kinds ' + ', '.join(f'{k} {n}' for k, n in kinds) + '; tempos ' + ', '.join(f'{t} ({n})' for t, n in tempos))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    commands = parser.add_subparsers(dest='command', required=True)
    index = commands.add_parser('index')
    index.add_argument('--jobs', type=int, default=16)
    search = commands.add_parser('find')
    search.add_argument('words', nargs='*')
    search.add_argument('--bpm', type=float)
    search.add_argument('--any-octave', action='store_true')
    search.add_argument('--key')
    search.add_argument('--kind', choices=[kind for kind, _ in KINDS] + ['unreadable'])
    shape = search.add_mutually_exclusive_group()
    shape.add_argument('--loops', action='store_true')
    shape.add_argument('--shots', action='store_true')
    search.add_argument('--library')
    search.add_argument('--midi', action='store_true')
    search.add_argument('--limit', type=int, default=50)
    commands.add_parser('summary')
    args = parser.parse_args()
    if args.command == 'index':
        build(args.jobs)
    elif args.command == 'find':
        find(args)
    else:
        summary()


if __name__ == '__main__':
    main()
