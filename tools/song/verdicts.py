"""Verdicts for every measured sound: verdicts.py build | show | summary | family

python -I tools/song/verdicts.py build
python -I tools/song/verdicts.py show [WORD ...] [--verdict V] [--reason R] [--kind KIND] [--library TEXT] [--limit N]
python -I tools/song/verdicts.py summary
python -I tools/song/verdicts.py family WORD [WORD ...]

`build` joins the index (`samples.py index`) and the measurements (`sampletags.py tag`) and writes
SAMPLE_VERDICTS (default: verdicts.tsv beside the index), one row per WAV. Nobody can listen to a
quarter of a million sounds, so the verdicts are automatic; a person judges only the sounds a song
will use, and their verdicts override these.

Automatic verdicts, from the measurements alone:
- reject: no audio (unreadable or silent), or a measurement that belongs to an older file; every member
  of a family whose numbered chord files do not
  play their chords (family-chord-mismatch: their chroma fits other triads much better than the ones
  the pack names, by the median of the family). Only pads, chords, strings, basses, leads, arps and
  brass are tested; a lead line passes through other notes, so it has a lower bar.
- caution: one chord file that does not play its chord (chord-mismatch); a family 15 cents or more
  out of tune (family-off-tune; the tune column gives the cents to add from 5 cents up, from the
  family's tonal members' median offset); a sound that starts late; a stray frame at the very end (end-spike); a loop whose wrap
  clicks (seam-click); a one-shot cut off while still sounding (cut-end); a very quiet file; a melodic
  loop whose measured key note lies outside its name's key (key-differs).
- ok: none of these.
A file with the same size, level and chroma as an earlier one names it in same_as.

The owner's verdicts are in SAMPLE_OWNER_VERDICTS (default: owner-verdicts.tsv beside the index): a
tab-separated file with the columns pattern, verdict (approved, rejected, out-of-tune or cheesy),
note and date. A pattern is a full path (root/path, with / or \\) where * matches any text; case does
not count. The owner's verdict replaces the automatic one, and the automatic reasons stay beside it.

A family is the sounds of one folder whose names differ only in a trailing number, chord name, tempo
and key tag, or left/right mark: Pad 1 to Pad 8, or BASS001_140_A to BASS035_140_A. `family` prints
the families whose name has every WORD, with their members' verdicts.
"""
import argparse
import csv
import re
import statistics
import sys
from collections import Counter, defaultdict
from functools import lru_cache
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import env  # noqa: E402
import samples  # noqa: E402
import sampletags  # noqa: E402

COLUMNS = ['root', 'path', 'bytes', 'mtime', 'analysis_version', 'family', 'kind', 'verdict', 'reasons', 'owner_note', 'chord', 'chord_fit', 'tune',
           'same_as']
ANALYSIS_VERSION = '2'
OWNER_VERDICTS = ('approved', 'rejected', 'out-of-tune', 'cheesy')
# A chord file plays its chord when that triad holds at least this share of the best triad's; a melodic line
# (lead, arp) passes through other notes, so it gets a lower bar. Calibrated on families a person judged.
CHORD_FIT = 0.7
CHORD_FIT_LINE = 0.55
LINE_KINDS = {'lead', 'arp', 'brass'}
CHORD_KINDS = {'pad', 'chord', 'strings', 'bass'} | LINE_KINDS  # vocals, guitars, drums and effects are not tested
# A family fails when its chord files' median fit is under this (a single file under the bar is only a caution).
FAMILY_FIT = 0.6
FAMILY_FIT_LINE = 0.55
TUNE_CENTS = 5  # a family's median offset that earns a tuning correction (the tune column)
TUNE_CAUTION = 15  # and the offset that makes it a caution: audible against the other layers
CAUTION_TAGS = {'late-start': 'late-start', 'end-spike': 'end-spike', 'seam-click': 'seam-click',
                'cut-end': 'cut-end', 'quiet': 'quiet'}
MELODIC_KINDS = {'lead', 'pad', 'chord', 'arp', 'strings', 'brass', 'guitar', 'bass'}

TEMPO_KEY_TAG = re.compile(r'[ _-]+\d{2,3}[ _-]*[a-z#]{0,4}(?:[ _-]+[a-z]{1,3}\d?)?$')
CHORD_NAME = re.compile(r'[ _-]+[a-g](?:#|b)?(?:m|min|maj|maj7|m7|7|sus2|sus4|dim|aug)?$')
TRAILING = [re.compile(p) for p in (
    r'[ _-]*\d+[a-z]?$',  # a number, also glued on (BASS001, Loop 4b)
    r'[ _-]+(?:l|r)$',  # a left or right channel file
)]


def stem(name):
    """The family part of a file name: Deep Pad 6 -> deep pad, BASS002_140_A_SL1 -> bass. A chord name
    counts only at the very end (Cellophane Am), so Drone Bass A 1 and Drone Bass B 1 stay two families."""
    text = Path(name).stem.lower().strip()
    text = TEMPO_KEY_TAG.sub('', text)
    shorter = CHORD_NAME.sub('', text)
    text = shorter if shorter.strip(' _-') else text
    changed = True
    while changed and text:
        changed = False
        for pattern in TRAILING:
            shorter = pattern.sub('', text)
            if shorter != text and shorter.strip(' _-'):
                text, changed = shorter.strip(), True
    return text.strip(' _-') or Path(name).stem.lower()


def family_of(root, path):
    folder, _, name = path.replace('\\', '/').rpartition('/')
    return f'{root}|{folder}|{stem(name)}'


def family_label(family):
    root, folder, name = family.split('|')
    return f'{folder}/{name}' if folder else name


def triad_shares(chroma):
    """{(root, minor): share} of the 24 major and minor triads in a 12-bin chroma."""
    shares = {}
    for root in range(12):
        for minor, third in ((False, 4), (True, 3)):
            shares[(root, minor)] = chroma[root] + chroma[(root + third) % 12] + chroma[(root + 7) % 12]
    return shares


def parse_chord(text):
    match = re.fullmatch(r'([A-G])([#b]?)(m?)', text.strip())
    if not match:
        return None
    root = samples.NOTES.index(match.group(1)) + {'#': 1, 'b': -1, '': 0}[match.group(2)]
    return root % 12, bool(match.group(3))


def chord_fit(row, chord):
    expected = parse_chord(chord)
    if expected is None or not row.get('chroma'):
        return None
    chroma = [int(v) for v in row['chroma'].split(',')]
    if sum(chroma) <= 0:
        return None
    shares = triad_shares(chroma)
    best = max(shares.values())
    return shares[expected] / best if best else None


def in_scale(name_key, measured_key):
    """Whether the measured key's note lies in the named key's scale (natural minor or major): a loop on
    the pack key's IV or V reads as that chord's key and still belongs to the pack."""
    a, b = samples.parse_key(name_key), samples.parse_key(measured_key)
    named, measured = parse_chord(a or ''), parse_chord(b or '')
    if not named or not measured:
        return True
    root, minor = named
    steps = (0, 2, 3, 5, 7, 8, 10) if minor else (0, 2, 4, 5, 7, 9, 11)
    return (measured[0] - root) % 12 in steps


def owner_path():
    return env.path('SAMPLE_OWNER_VERDICTS', required=False) or samples.index_path().with_name('owner-verdicts.tsv')


def verdicts_path():
    return env.path('SAMPLE_VERDICTS', required=False) or samples.index_path().with_name('verdicts.tsv')


def load_owner():
    path = owner_path()
    if not path.exists():
        return []
    with open(path, encoding='utf-8', newline='') as f:
        rules = list(csv.DictReader(f, delimiter='\t'))
    for rule in rules:
        if rule['verdict'] not in OWNER_VERDICTS:
            raise SystemExit(f'{path}: verdict {rule["verdict"]!r} is not one of {", ".join(OWNER_VERDICTS)}')
        rule['match'] = rule['pattern'].replace('\\', '/').lower()
    return rules


@lru_cache(maxsize=4096)
def owner_pattern(pattern):
    """Only * is special in an owner rule; brackets and ? are literal."""
    return re.compile(re.escape(pattern).replace(r'\*', '.*'), re.DOTALL)


def owner_verdict(rules, root, path):
    full = (f'{root}/{path}' if root else str(path)).replace('\\', '/').lower()
    found = None
    for rule in rules:  # the last matching line wins, so a file line can follow its family's line
        if owner_pattern(rule['match']).fullmatch(full):
            found = rule
    return found


def has_audio_measures(measured):
    """Whether a tags row holds the audio measurements; a failed edge read keeps them."""
    return bool(measured) and (not measured['error'] or measured['error'].startswith('edges:'))


def load(path=None):
    path = Path(path or verdicts_path())
    if not path.exists():
        raise SystemExit(f'{path} does not exist. Build it with: python -I tools/song/verdicts.py build')
    with open(path, encoding='utf-8', newline='') as f:
        return list(csv.DictReader(f, delimiter='\t'))


def build():
    index = [r for r in samples.load() if r['type'] == 'wav']
    tags = {(r['root'], r['path']): r for r in sampletags.load()}
    owner = load_owner()
    rows, members = [], defaultdict(list)
    first_of = {}
    for info in index:
        key = (info['root'], info['path'])
        measured = tags.get(key)
        stale = measured is not None and not sampletags.matches_measurement(measured, info)
        if stale:
            measured = None
        row = dict(root=info['root'], path=info['path'], bytes=info['bytes'], mtime=info['mtime'],
                   analysis_version=ANALYSIS_VERSION,
                   family=family_of(*key), kind=info['kind'], chord=info['chord'])
        reasons = []
        if stale:
            reasons.append('stale-measurement')
        elif not has_audio_measures(measured) or info['kind'] == 'unreadable':
            reasons.append('no-audio')
        else:
            tagset = set(measured['tags'].split())
            reasons += [name for tag, name in CAUTION_TAGS.items() if tag in tagset]
            if info['chord'] and info['kind'] in CHORD_KINDS and float(measured['flatness'] or 1) < 0.15:
                fit = chord_fit(measured, info['chord'])
                if fit is not None:
                    row['chord_fit'] = f'{fit:.2f}'
                    if fit < (CHORD_FIT_LINE if info['kind'] in LINE_KINDS else CHORD_FIT):
                        reasons.append('chord-mismatch')
            if (info['key'] and info['key_from'] in ('name', 'pack') and not info['chord'] and info['bars']
                    and info['kind'] in MELODIC_KINDS and float(measured['key_fit'] or 0) > 0.75
                    and not in_scale(info['key'], measured['key'])):
                reasons.append('key-differs')
            identity = (measured['bytes'], measured['rms_db'], measured['peak_db'], measured['chroma'])
            if identity in first_of:
                row['same_as'] = first_of[identity]
            else:
                first_of[identity] = f'{info["root"]}/{info["path"]}'
        row['reasons'] = reasons
        rows.append(row)
        members[row['family']].append((row, measured))
    # Family checks: chord files that do not follow their chords, and the family's tuning.
    for family, group in members.items():
        fits = [float(row['chord_fit']) for row, _ in group if row.get('chord_fit')]
        line = group[0][0]['kind'] in LINE_KINDS
        if len(fits) >= 3 and statistics.median(fits) < (FAMILY_FIT_LINE if line else FAMILY_FIT):
            for row, _ in group:
                row['reasons'].append('family-chord-mismatch')
        tonal = [float(m['tuning']) for _, m in group
                 if has_audio_measures(m) and m['tuning'] and float(m['flatness']) < 0.05 and float(m['key_fit']) > 0.6]
        if tonal:
            offset = statistics.median(tonal)
            if abs(offset) >= TUNE_CENTS:
                for row, _ in group:
                    row['tune'] = f'{-offset:+.0f}'
                    if abs(offset) >= TUNE_CAUTION:
                        row['reasons'].append('family-off-tune')
    for row in rows:
        reasons = row['reasons']
        if {'no-audio', 'stale-measurement', 'family-chord-mismatch'} & set(reasons):
            verdict = 'reject'
        elif reasons:
            verdict = 'caution'
        else:
            verdict = 'ok'
        rule = owner_verdict(owner, row['root'], row['path'])
        if rule:
            verdict, row['owner_note'] = rule['verdict'], f'{rule["note"]} ({rule["date"]})'
        row['verdict'], row['reasons'] = verdict, ' '.join(dict.fromkeys(reasons))
    target = verdicts_path()
    temporary = target.with_suffix('.tmp')
    with open(temporary, 'w', encoding='utf-8', newline='') as f:
        writer = csv.DictWriter(f, COLUMNS, delimiter='\t', extrasaction='ignore', lineterminator='\n')
        writer.writeheader()
        writer.writerows(rows)
    temporary.replace(target)
    counts = Counter(row['verdict'] for row in rows)
    print(f'{target}: {len(rows)} sounds in {len(members)} families; '
          + ', '.join(f'{verdict} {count}' for verdict, count in counts.most_common()))


def show(args):
    shown = 0
    for row in load():
        text = f'{row["root"]}/{row["path"]}'.lower()
        if any(word.lower() not in text for word in args.words):
            continue
        if args.verdict and row['verdict'] != args.verdict:
            continue
        if args.reason and args.reason not in row['reasons'].split():
            continue
        if args.kind and row['kind'] != args.kind:
            continue
        if args.library and args.library.lower() not in row['root'].lower():
            continue
        extra = ' '.join(x for x in (row['chord'] and f'chord {row["chord"]} fit {row["chord_fit"]}',
                                     row['tune'] and f'tune {row["tune"]}c') if x)
        print(f'{row["verdict"]:<11} {row["reasons"]:<36} {extra:<26} {row["root"]}/{row["path"]}')
        shown += 1
        if shown >= args.limit:
            print(f'(first {args.limit}; raise --limit for more)', file=sys.stderr)
            return


def summary():
    rows = load()
    for root in dict.fromkeys(r['root'] for r in rows):
        mine = [r for r in rows if r['root'] == root]
        verdicts = Counter(r['verdict'] for r in mine)
        reasons = Counter(reason for r in mine for reason in r['reasons'].split())
        print(f'{root}: {len(mine)} sounds, {len({r["family"] for r in mine})} families; '
              + ', '.join(f'{v} {n}' for v, n in verdicts.most_common()) + '; reasons: '
              + ', '.join(f'{v} {n}' for v, n in reasons.most_common()))


def family(words):
    groups = defaultdict(list)
    for row in load():
        if all(word.lower() in family_label(row['family']).lower() for word in words):
            groups[row['family']].append(row)
    for name, group in sorted(groups.items()):
        verdicts = Counter(r['verdict'] for r in group)
        tune = next((r['tune'] for r in group if r['tune']), '')
        fits = [r['chord_fit'] for r in group if r['chord_fit']]
        print(f'{family_label(name)} [{name.split("|")[0]}]: {len(group)} files; '
              + ', '.join(f'{v} {n}' for v, n in verdicts.most_common())
              + (f'; tune {tune}c' if tune else '') + (f'; chord fits {" ".join(fits)}' if fits else ''))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('build')
    search = commands.add_parser('show')
    search.add_argument('words', nargs='*')
    search.add_argument('--verdict')
    search.add_argument('--reason')
    search.add_argument('--kind')
    search.add_argument('--library')
    search.add_argument('--limit', type=int, default=50)
    commands.add_parser('summary')
    families = commands.add_parser('family')
    families.add_argument('words', nargs='+')
    args = parser.parse_args()
    if args.command == 'build':
        build()
    elif args.command == 'show':
        show(args)
    elif args.command == 'summary':
        summary()
    else:
        family(args.words)


if __name__ == '__main__':
    main()
