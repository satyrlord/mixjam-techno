"""The chord root per bar of each MIDI loop, moved to the song's key: midichords.py [--to KEY] FOLDER...

python -I tools/song/midichords.py [--to "A minor"] FOLDER...

One line per MIDI file in each FOLDER. The file's key comes from the end of its name ("... Abm",
"... E"); a name without a key takes it from the WAV in the folder's parent that has the same loop
number. Minor files move to the minor key of --to (default A minor), major files to its relative
major. Per bar: the bass note (lowest, held longest) for a folder whose name contains "Bass", else
the triad that best covers the held pitch classes. Loops that share a number in a pack often share
a progression; this shows which.
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from harmony import NOTES, best_triad, parse_key, pitch_class  # noqa: E402
from midinotes import notes, per_bar, pitch_classes  # noqa: E402

KEY_AT_END = r'\b([A-G][#b]?)(m?)\s*$'


def targets(key):
    """(minor tonic, major tonic) that minor and major files move to for a song in `key`."""
    tonic, mode = parse_key(key)
    minor = tonic if mode == 'minor' else (tonic + 9) % 12
    return minor, (minor + 3) % 12


def file_key(name, wav_names=()):
    """(tonic pitch class, is minor, key text) from a file name, or None."""
    key = re.search(KEY_AT_END, name)
    if not key:
        number = re.search(r'(\d+)', name)
        match = [w for w in wav_names if number and re.search(rf'\b{number.group(1)} ', w)]
        key = re.search(KEY_AT_END, match[0]) if match else None
    if not key:
        return None
    return pitch_class(key.group(1)), bool(key.group(2)), key.group(0).strip()


def shift_for(name, wav_names=(), to='A minor'):
    """(semitones -6..+5, key text) that move a file's key to the song's; (None, '?') when unknown."""
    found = file_key(name, wav_names)
    if found is None:
        return None, '?'
    tonic, minor, text = found
    minor_target, major_target = targets(to)
    target = minor_target if minor else major_target
    return (target - tonic + 6) % 12 - 6, text


def number_of(path):
    match = re.search(r'(\d+)', path.stem)
    return (int(match.group(1)) if match else 0, path.stem)


def main():
    args = sys.argv[1:]
    to = 'A minor'
    if '--to' in args:
        i = args.index('--to')
        to = args[i + 1]
        del args[i:i + 2]
    if not args:
        raise SystemExit(__doc__)
    for folder in map(Path, args):
        wav_names = [p.stem for p in folder.parent.glob('*.wav')]
        is_bass = 'bass' in folder.name.lower()
        for path in sorted(folder.glob('*.mid'), key=number_of):
            shift, key = shift_for(path.stem, wav_names, to)
            if shift is None:
                continue
            found, ppq = notes(path)
            bars = per_bar(found, ppq, shift)
            chords = []
            for bar in sorted(bars):
                if is_bass:
                    note = min(bars[bar], key=lambda n: (n // 12, -bars[bar][n]))
                    chords.append(NOTES[note % 12])
                else:
                    chords.append(best_triad(pitch_classes(bars[bar])))
            print(f'{path.stem:28s} ({key:>3s} {shift:+d})  ' + ' | '.join(chords))


if __name__ == '__main__':
    main()
