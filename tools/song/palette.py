"""A song's palette of sound families: palette.py census | families | suggest | make | add | remove | show | check

python -I tools/song/palette.py census [SONG.mixjam ...]
python -I tools/song/palette.py families SONG.mixjam
python -I tools/song/palette.py suggest [WORD ...] [--kind KIND] [--key KEY] [--tag TAG ...] [--bpm N] [--library TEXT]
                                        [--all] [--limit N]
python -I tools/song/palette.py make PALETTE.json [--from SONG.mixjam] [--title TEXT]
python -I tools/song/palette.py add PALETTE.json FAMILY [FAMILY ...]
python -I tools/song/palette.py remove PALETTE.json FAMILY [FAMILY ...]
python -I tools/song/palette.py show PALETTE.json
python -I tools/song/palette.py check SONG.mixjam PALETTE.json

A song draws its sounds from a palette of about 25 to 40 families (the owner's rule: careful sound
selection, less is more); each family brings its numbered variations, so a palette holds roughly
100 to 200 files. A family is the files of one folder that differ only in a number, chord name, tempo
and key tag, or left/right mark (see `verdicts.py`).

- `census` counts the families and files that songs place: by default the owner's converted songs
  in local-data/songs/review/album-mix and the latest Reality pass. Sounds imported from the original
  products count by their file name.
- `families` lists one song's families with the files placed from each.
- `suggest` lists families from the verdicts (`verdicts.py build`), the owner's approved ones first,
  then by their share of ok files. Families that the owner rejected, that sound out of tune or cheesy,
  or that the automatic verdicts reject stay out unless --all. --key matches the family's most common
  measured key; --tag keeps families whose files mostly carry that tag; --bpm matches within 0.5 BPM.
  The FAMILY column is what `add` takes.
- `make` writes a palette, from a song's families with --from or empty; a family of more than 12 files is a
  bank of different sounds, so only the song's own files from it go in. Pathless native sounds are kept
  by their product and sample ID. `add` and `remove` change the families.
  `show` lists it with its families' verdicts and tuning corrections.
- `check` lists every placed file outside the palette, and the palette's unused families; it exits
  with 1 when a file is outside. A composer script can also hold itself to a palette with
  `song.use_palette(PATH)` (mixjam.py): a sound outside it stops the script. A file the owner rejected,
  or judged out of tune or cheesy, is outside every palette.
"""
import argparse
import csv
import datetime
import hashlib
import json
import math
import ntpath
import statistics
import struct
import sys
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import env  # noqa: E402
import samples  # noqa: E402
import sampletags  # noqa: E402
import verdicts  # noqa: E402

KEEP_OUT = {'rejected', 'out-of-tune', 'cheesy', 'reject'}
OWNER_OUT = {'rejected', 'out-of-tune', 'cheesy'}
# A family this large is a bank of different sounds that share a name, not one sound's variations: a palette
# made from a song keeps only the files the song uses from it.
BIG_FAMILY = 12
CENSUS_DEFAULT = ['local-data/songs/review/album-mix/*.mixjam']


def roots():
    """The library folders, longest first, so a nested root wins."""
    found = {row['root'] for row in samples.load()}
    return sorted(found, key=len, reverse=True)


def windows_path(path):
    """Normalize drive and UNC paths, including their Windows verbatim forms."""
    text = str(path).replace('/', '\\')
    if text[:8].lower() == '\\\\?\\unc\\':
        text = '\\\\' + text[8:]
    elif (text.startswith('\\\\?\\') and len(text) >= 7 and text[4].isascii()
          and text[4].isalpha() and text[5:7] == ':\\'):
        text = text[4:]
    return ntpath.normpath(text)


def family_of_path(path, library_roots):
    """The index family of a file on disk, or None when it lies outside the libraries."""
    text = windows_path(path)
    for root in library_roots:
        prefix = windows_path(root).rstrip('\\') + '\\'
        if text.lower().startswith(prefix.lower()):
            return verdicts.family_of(root, text[len(prefix):].replace('\\', '/'))
    return None


def family_key(family):
    """Compare Windows family paths independent of case and slash direction."""
    root, separator, remainder = family.partition('|')
    return file_key(root).replace('\\', '/') + separator + remainder.replace('\\', '/').casefold()


def file_key(path):
    return ntpath.normcase(windows_path(path))


PRODUCT_SLOTS = {'t2': 0, 't1': 1, 'd1': 2, 'd2': 3, 'd3': 4, 'd4': 5, 'h1': 6, 't3': 7}  # src/products.rs
PRODUCT_STRIDE = 100_000
_catalog = None


def catalog_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f'duplicate field {key}')
        result[key] = value
    return result


def product_slug(name):
    # The earlier product slugs, escaped as in src/products.rs.
    aliases = {'\x74echno2': 't2', '\x72ave': 't1', '\x64ance1': 'd1', '\x64ance2': 'd2', '\x64ance3': 'd3',
               '\x64ance4': 'd4', '\x68ouse': 'h1', '\x74echno3': 't3'}
    return aliases.get(name, name)


def read_release(path):
    """Read one format-3 catalog with the same identity checks as the application."""
    file = json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=catalog_object)
    if not isinstance(file, dict) or set(file) != {'schema', 'product', 'release', 'fingerprint', 'sources', 'samples'}:
        raise ValueError('not a format-3 sample catalog')
    product = product_slug(file['product'])
    if type(file['schema']) is not int or file['schema'] != 3 or product not in PRODUCT_SLOTS:
        raise ValueError('unsupported format or product')
    if not isinstance(file['release'], str):
        raise ValueError('invalid release name')
    release = file['release'].strip()
    if not release or len(release) > 64 or any(ord(c) < 32 or 127 <= ord(c) < 160 for c in release):
        raise ValueError('invalid release name')
    if not isinstance(file['sources'], list) or not file['sources']:
        raise ValueError('no source files')
    sources = []
    for source in file['sources']:
        if (not isinstance(source, dict) or set(source) != {'bytes', 'sha256'}
                or not isinstance(source['sha256'], str)
                or type(source['bytes']) is not int or not 0 <= source['bytes'] < 2 ** 64):
            raise ValueError('invalid source file')
        sha = source['sha256']
        if len(sha) != 64 or any(c not in '0123456789abcdefABCDEF' for c in sha):
            raise ValueError('invalid source SHA-256')
        sources.append((sha.lower(), source['bytes']))
    digest = hashlib.sha256()
    for sha, size in sorted(sources):
        digest.update(sha.encode('utf-8') + struct.pack('<Q', size) + b'\n')
    if not isinstance(file['fingerprint'], str) or digest.hexdigest() != file['fingerprint'].lower():
        raise ValueError('source fingerprint differs')
    if not isinstance(file['samples'], list) or not file['samples']:
        raise ValueError('no sample identities')
    seen = set()
    required = {'native_id', 'code', 'label_key', 'group_name', 'bars', 'source_bpm', 'frame_count'}
    for entry in file['samples']:
        if not isinstance(entry, dict) or not required <= set(entry) <= required | {'pcm_digest'}:
            raise ValueError('invalid sample identity')
        ident, bars, frames, bpm = (entry[k] for k in ('native_id', 'bars', 'frame_count', 'source_bpm'))
        pcm = entry.get('pcm_digest')
        if (type(ident) is not int or not 0 <= ident < PRODUCT_STRIDE or ident in seen
                or type(bars) is not int or not 0 < bars < 2 ** 32
                or type(frames) is not int or not 0 < frames < 2 ** 32
                or isinstance(bpm, bool) or not isinstance(bpm, (float, int)) or not math.isfinite(bpm) or bpm <= 0
                or not isinstance(entry['code'], str) or not entry['code'].strip()
                or not isinstance(entry['label_key'], str) or len(entry['label_key']) != 16
                or any(c not in '0123456789abcdef' for c in entry['label_key'])
                or not isinstance(entry['group_name'], str)
                or (pcm is not None and (not isinstance(pcm, str) or len(pcm) != 32
                                         or any(c not in '0123456789abcdefABCDEF' for c in pcm)))):
            raise ValueError(f'invalid identity {ident}')
        seen.add(ident)
    return product, release, file['fingerprint'].lower(), file['samples']


def same_catalog_sound(first, other):
    if (first['frame_count'] != other['frame_count'] or first['bars'] != other['bars']
            or first['source_bpm'] != other['source_bpm']):
        return False
    a, b = first.get('pcm_digest'), other.get('pcm_digest')
    return a.lower() == b.lower() if a is not None and b is not None else first['code'].lower() == other['code'].lower()


def catalog_names():
    """{(product, release, native id): name} from the private files that the catalog generator writes
    (SAMPLE_CATALOG_NAMES, default local-data/catalog-names). Catalogs hold no names; without these
    files a sound is named by its archive code."""
    folder = Path(env.path('SAMPLE_CATALOG_NAMES', required=False) or env.ROOT / 'local-data' / 'catalog-names')
    names = {}
    for path in sorted(folder.glob('*.json'), key=lambda path: path.name) if folder.is_dir() else []:
        try:
            file = json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=catalog_object)
            if (not isinstance(file, dict) or set(file) != {'product', 'release', 'names'}
                    or not isinstance(file['product'], str) or not isinstance(file['release'], str)
                    or not isinstance(file['names'], list)):
                raise ValueError('invalid catalog name file')
            product = product_slug(file['product'])
            release = file['release'].strip()
            if (product not in PRODUCT_SLOTS or not release or len(release) > 64
                    or any(ord(c) < 32 or 127 <= ord(c) < 160 for c in release)):
                raise ValueError('invalid product or release')
            pending = {}
            for entry in file['names']:
                if not isinstance(entry, list) or len(entry) != 2:
                    raise ValueError('invalid sample name entry')
                native_id, name = entry
                if (type(native_id) is not int or not 0 <= native_id < PRODUCT_STRIDE
                        or native_id in pending or not isinstance(name, str) or not name.strip()):
                    raise ValueError('invalid or duplicate sample name')
                pending[native_id] = name
            for native_id, name in pending.items():
                names.setdefault((product, release, native_id), name)
        except (OSError, UnicodeError, ValueError, TypeError, KeyError) as error:
            print(f'{path}: catalog names were not loaded: {error}', file=sys.stderr)
    return names


def code_name(code):
    """A sound's archive code as a name: its file stem, as in DABSH015(L)."""
    return ntpath.splitext(ntpath.basename(code.replace('/', '\\')))[0]


def product_catalog():
    """{song sample id: (product, group, name)} from the product catalogs (SAMPLE_CATALOGS, default
    assets/catalog), for the products' own sounds that songs place by id; empty without them."""
    global _catalog
    if _catalog is None:
        _catalog = {}
        folder = env.path('SAMPLE_CATALOGS', required=False) or env.ROOT / 'assets' / 'catalog'
        identities, releases = {}, set()
        names = catalog_names()
        folder = Path(folder)
        # By name with case, as the application orders them: of two conflicting releases, the same one loads.
        for path in sorted(folder.iterdir(), key=lambda path: path.name) if folder.is_dir() else []:
            if not path.is_file() or path.suffix.lower() != '.json' or path.name.lower() == 'source-tempos.json':
                continue
            try:
                product, release, fingerprint, entries = read_release(path)
                if (product, fingerprint) in releases:
                    raise ValueError('duplicate release')
                slot = PRODUCT_SLOTS[product]
                if any((known := identities.get(slot * PRODUCT_STRIDE + entry['native_id'])) is not None
                       and not same_catalog_sound(known, entry) for entry in entries):
                    raise ValueError('a shared native ID conflicts with a loaded release')
                for entry in entries:
                    sample_id = slot * PRODUCT_STRIDE + entry['native_id']
                    if sample_id not in identities:
                        identities[sample_id] = entry
                        name = names.get((product, release, entry['native_id'])) or code_name(entry['code'])
                        _catalog[sample_id] = (product, entry['group_name'], name)
                    elif identities[sample_id].get('pcm_digest') is None and entry.get('pcm_digest') is not None:
                        identities[sample_id] = dict(identities[sample_id], pcm_digest=entry['pcm_digest'])
                releases.add((product, fingerprint))
            except (OSError, UnicodeError, ValueError, TypeError, KeyError, OverflowError) as error:
                print(f'{path}: catalog was not loaded: {error}', file=sys.stderr)
    return _catalog


def placed_samples(song_path):
    """The samples a song places, each with its family: an index family, 'product:P|GROUP|stem' for a
    product's own sound, or 'song|ORIGIN|stem' for a sound imported from a product song."""
    song = json.loads(Path(song_path).read_text(encoding='utf-8'))
    used = Counter(p['sampleId'] for lane in song['lanes'] for p in lane['placements'])
    library_roots = roots()
    out = []
    listed = {sample['id'] for sample in song['samples']}
    for sample_id in sorted(set(used) - listed):
        product, group, name = product_catalog().get(sample_id, ('unknown', '', str(sample_id)))
        out.append(dict(id=sample_id, name=name, path='', family=f'product:{product}|{group}|{verdicts.stem(name)}',
                        clips=used[sample_id]))
    for sample in song['samples']:
        if sample['id'] not in used:
            continue
        path = sample.get('path') or ''
        family = family_of_path(path, library_roots) if path else None
        if family is None:
            origin = sample.get('origin') or ''
            family = f'song|{origin}|{verdicts.stem(sample.get("name") or Path(path).name)}' if origin else None
        out.append(dict(id=sample['id'], name=sample.get('name'), path=path, family=family, clips=used[sample['id']]))
    return out


def song_files(arguments):
    if arguments:
        return [Path(a) for a in arguments]
    files = []
    for pattern in CENSUS_DEFAULT:
        files += sorted(env.ROOT.glob(pattern))
    passes = sorted(env.ROOT.glob('local-data/songs/melancholia/reality/pass*/reality.mixjam'),
                    key=lambda p: int(p.parent.name[4:]) if p.parent.name[4:].isdigit() else -1)
    return files + passes[-1:]


def census(arguments):
    rows = []
    for path in song_files(arguments):
        placed = placed_samples(path)
        families = {s['family'] or f'file|{s["path"]}' for s in placed}
        rows.append((path, len(families), len(placed)))
        print(f'{len(families):4d} families {len(placed):4d} files  {path.name}')
    if rows:
        fams, files = [r[1] for r in rows], [r[2] for r in rows]
        print(f'median {statistics.median(fams):.0f} families (range {min(fams)}-{max(fams)}), '
              f'{statistics.median(files):.0f} files (range {min(files)}-{max(files)}) over {len(rows)} songs')


def families(song_path):
    groups = defaultdict(list)
    for sample in placed_samples(song_path):
        groups[sample['family'] or f'file|{sample["path"]}'].append(sample)
    for family, group in sorted(groups.items(), key=lambda item: -len(item[1])):
        print(f'{len(group):3d} files {sum(s["clips"] for s in group):4d} clips  {label(family)}')
    print(f'{len(groups)} families, {sum(len(g) for g in groups.values())} files')


def label(family):
    if family.startswith(('song|', 'file|')):
        return family
    return f'{verdicts.family_label(family)} [{family.split("|")[0]}]'


def family_table():
    """{family: summary} from the verdicts, the measurements and the index."""
    index = {(r['root'], r['path']): r for r in samples.load()}
    measured = {(r['root'], r['path']): r for r in sampletags.load()
                if sampletags.matches_measurement(r, index.get((r['root'], r['path'])))}
    groups = defaultdict(list)
    for row in verdicts.load():
        if (not sampletags.matches_index(row, index.get((row['root'], row['path'])))
                or row.get('analysis_version') != verdicts.ANALYSIS_VERSION):
            # Verdict files from an older sound, including pre-versioned rows,
            # cannot recommend the current audio or its tuning.
            row = dict(row, verdict='reject', tune='')
        groups[row['family']].append(row)
    table = {}
    for family, group in groups.items():
        keys = Counter(measured[(r['root'], r['path'])]['key'] for r in group
                       if (r['root'], r['path']) in measured and measured[(r['root'], r['path'])]['key'])
        tags = Counter(t for r in group if (r['root'], r['path']) in measured
                       for t in measured[(r['root'], r['path'])]['tags'].split())
        infos = [index.get((r['root'], r['path']), {}) for r in group]
        bpms = Counter(i.get('bpm') for i in infos if i.get('bpm'))
        table[family] = dict(files=len(group), verdicts=Counter(r['verdict'] for r in group),
                             kind=Counter(r['kind'] for r in group).most_common(1)[0][0],
                             key=keys.most_common(1)[0][0] if keys else '',
                             bpm=bpms.most_common(1)[0][0] if bpms else '',
                             tags={t for t, n in tags.items() if n * 2 > len(group)},
                             tune=next((r['tune'] for r in group if r['tune']), ''),
                             paths=[f'{r["root"]}/{r["path"]}' for r in group])
    return table


def kept_out(summary):
    verdict_counts = summary['verdicts']
    owner_out = sum(verdict_counts[v] for v in ('rejected', 'out-of-tune', 'cheesy'))
    return owner_out > 0 or verdict_counts['reject'] * 2 > summary['files']


def suggest(args):
    key = samples.parse_key(args.key) if args.key else None
    found = []
    for family, s in family_table().items():
        name = label(family).lower()
        if any(word.lower() not in name for word in args.words):
            continue
        if args.kind and s['kind'] != args.kind:
            continue
        if key and s['key'] != key:
            continue
        if set(args.tag) - s['tags']:
            continue
        if args.bpm and (not s['bpm'] or abs(float(s['bpm']) - args.bpm) > 0.5):
            continue
        if args.library and args.library.lower() not in family.split('|')[0].lower():
            continue
        if not args.all and kept_out(s):
            continue
        ok_share = (s['verdicts']['ok'] + s['verdicts']['approved']) / s['files']
        found.append((-s['verdicts']['approved'], -ok_share, -s['files'], family, s))
    for _, _, _, family, s in sorted(found)[:args.limit]:
        counts = ', '.join(f'{v} {n}' for v, n in s['verdicts'].most_common())
        extra = ' '.join(x for x in (s['key'], s['bpm'] and f'{float(s["bpm"]):g} BPM',
                                     s['tune'] and f'tune {s["tune"]}c') if x)
        print(f'{s["files"]:3d} {s["kind"]:<7} {extra:<24} {counts:<28} {family}')
    if len(found) > args.limit:
        print(f'({len(found)} families; first {args.limit})', file=sys.stderr)


def load_palette(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def save_palette(path, palette):
    palette['families'] = sorted(set(palette['families']))
    palette['files'] = sorted(set(palette.get('files', [])))
    if 'native_samples' in palette:
        palette['native_samples'] = sorted(set(palette['native_samples']))
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_text(json.dumps(palette, indent=1) + '\n', encoding='utf-8')
    print(f'{path}: {len(palette["families"])} families, {len(palette["files"])} single files')


def make(args):
    palette = dict(title=args.title or Path(args.palette).parent.name,
                   made=datetime.date.today().isoformat(), families=[], files=[], native_samples=[])
    if args.source:
        sizes = Counter(row['family'] for row in verdicts.load())
        for sample in placed_samples(args.source):
            family = sample['family']
            if family and family in sizes and sizes[family] <= BIG_FAMILY:
                palette['families'].append(family)
            elif sample['path']:
                palette['files'].append(sample['path'])
            elif 0 <= sample['id'] < len(PRODUCT_SLOTS) * PRODUCT_STRIDE:
                palette['native_samples'].append(sample['id'])
            elif family:
                palette['families'].append(family)
    save_palette(args.palette, palette)


def change(path, names, add):
    palette = load_palette(path)
    table = family_table() if add else {}
    for name in names:
        if add and name not in table:
            raise SystemExit(f'{name}: not a family in the verdicts (copy the FAMILY column of `suggest`)')
        if add:
            palette['families'].append(name)
        elif name in palette['families']:
            palette['families'].remove(name)
        else:
            raise SystemExit(f'{name}: not in {path}')
    save_palette(path, palette)


def show(path):
    palette = load_palette(path)
    table = family_table()
    total = 0
    for family in palette['families']:
        s = table.get(family)
        if not s:
            print(f'  (not in the verdicts) {family}')
            continue
        total += s['files']
        counts = ', '.join(f'{v} {n}' for v, n in s['verdicts'].most_common())
        print(f'{s["files"]:3d} {s["kind"]:<7} {s["key"]:<4} {s["tune"] and "tune " + s["tune"] + "c":<10} {counts:<28} '
              f'{label(family)}')
    for file in palette.get('files', []):
        print(f'  1 file    {file}')
    for sample_id in palette.get('native_samples', []):
        product, group, name = product_catalog().get(sample_id, ('unknown', '', str(sample_id)))
        print(f'  1 native  {product} {group} {name}')
    print(f'{len(palette["families"])} families ({total} files) and {len(palette.get("files", []))} single files')


_owner_rules = None


def owner_out(path, library_roots):
    """Whether the owner rejected the file, or judged it out of tune or cheesy."""
    global _owner_rules
    if _owner_rules is None:
        _owner_rules = verdicts.load_owner()
    rule = verdicts.owner_verdict(_owner_rules, '', path)
    return bool(rule) and rule['verdict'] in OWNER_OUT


def allowed(palette, path, library_roots=None):
    """Whether a file on disk belongs to the palette (its family, or the file itself). A file that the owner
    rejected, or judged out of tune or cheesy, never does."""
    normal = windows_path(Path(path))
    if library_roots is None:
        library_roots = roots()
    if owner_out(normal, library_roots):
        return False
    if file_key(normal) in {file_key(f) for f in palette.get('files', [])}:
        return True
    family = family_of_path(normal, library_roots)
    return family is not None and family_key(family) in {family_key(f) for f in palette['families']}


def check(song_path, palette_path):
    palette = load_palette(palette_path)
    library_roots = roots()
    outside, used = [], set()
    for sample in placed_samples(song_path):
        native = not sample['path'] and sample['id'] in palette.get('native_samples', [])
        path_allowed = sample['path'] and allowed(palette, sample['path'], library_roots)
        family_allowed = (not sample['path'] and sample['family'] is not None and
                          family_key(sample['family']) in {family_key(f) for f in palette['families']})
        if native or path_allowed or family_allowed:
            family = sample['family']
            match = next((f for f in palette['families'] if family and family_key(f) == family_key(family)), None)
            used.add(match if match else sample['path'])
        else:
            outside.append(sample)
    for sample in outside:
        print(f'outside the palette: {sample["path"] or sample["name"]}')
    unused = [f for f in palette['families'] if f not in used]
    for family in unused:
        print(f'unused family: {label(family)}')
    print(f'{len(outside)} files outside the palette; {len(palette["families"]) - len(unused)} of '
          f'{len(palette["families"])} families used')
    return 1 if outside else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    commands = parser.add_subparsers(dest='command', required=True)
    c = commands.add_parser('census')
    c.add_argument('songs', nargs='*')
    f = commands.add_parser('families')
    f.add_argument('song')
    s = commands.add_parser('suggest')
    s.add_argument('words', nargs='*')
    s.add_argument('--kind')
    s.add_argument('--key')
    s.add_argument('--tag', action='append', default=[])
    s.add_argument('--bpm', type=float)
    s.add_argument('--library')
    s.add_argument('--all', action='store_true')
    s.add_argument('--limit', type=int, default=40)
    m = commands.add_parser('make')
    m.add_argument('palette')
    m.add_argument('--from', dest='source')
    m.add_argument('--title')
    a = commands.add_parser('add')
    a.add_argument('palette')
    a.add_argument('names', nargs='+')
    r = commands.add_parser('remove')
    r.add_argument('palette')
    r.add_argument('names', nargs='+')
    sh = commands.add_parser('show')
    sh.add_argument('palette')
    ch = commands.add_parser('check')
    ch.add_argument('song')
    ch.add_argument('palette')
    args = parser.parse_args()
    if args.command == 'census':
        census(args.songs)
    elif args.command == 'families':
        families(args.song)
    elif args.command == 'suggest':
        suggest(args)
    elif args.command == 'make':
        make(args)
    elif args.command in ('add', 'remove'):
        change(args.palette, args.names, args.command == 'add')
    elif args.command == 'show':
        show(args.palette)
    else:
        sys.exit(check(args.song, args.palette))


if __name__ == '__main__':
    main()
