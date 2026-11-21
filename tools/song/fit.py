"""Which MIDI loops fit which chord progressions: fit.py [--key KEY] --progression "CHORDS" [...] FOLDER...

python -I tools/song/fit.py --key "A minor" --progression "Am F Am G" --progression "Am Dm F E" FOLDER...

The tool reads every MIDI loop in each FOLDER. It moves minor files to the minor
key of --key (default A minor) and major files to the relative major, as
midichords.py does.
It scores each loop against each progression, with one chord per bar. It uses
each start bar that is a multiple of the progression length inside the loop.
The cost per bar is the share of held note time on clashing pitch classes. See
harmony.clash_cost for the scale, chord, and semitone rules. A loop fits when its
worst bar is at most 40% and its mean is at most 15%. Each result has the form
PROGRESSION@START:MEAN%.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from harmony import clash_cost, parse_chord  # noqa: E402
from cli import options, pop_option  # noqa: E402
from midichords import midi_loops  # noqa: E402
from midinotes import notes, per_bar, pitch_classes  # noqa: E402

WORST = 0.40
MEAN = 0.15


def bars_of(path, shift):
    """Held pitch classes per bar, padded to whole 4-bar blocks."""
    found, ppq = notes(path)
    bars = per_bar(found, ppq, shift)
    length = max(bars) + 1 if bars else 0
    length = 4 * -(-length // 4)
    return [pitch_classes(bars[i]) if i in bars else {} for i in range(length)]


def fits(bars, progressions, key):
    """[(name, start bar 1-based, mean cost)] of every progression the bars fit."""
    out = []
    for name, chords in progressions.items():
        parsed = [parse_chord(c) for c in chords]
        for start in range(0, len(bars), len(chords)):
            costs = [clash_cost(bars[start + i], parsed[i], key)
                     for i in range(len(chords)) if start + i < len(bars) and bars[start + i]]
            if costs and max(costs) <= WORST and sum(costs) / len(costs) <= MEAN:
                out.append((name, start + 1, sum(costs) / len(costs)))
    return out


def main():
    args = sys.argv[1:]
    key = 'A minor'
    progressions = {}
    keys = options(args, '--key')
    progs = options(args, '--progression')
    if keys:  # the last one counts
        key = keys[-1]
    for raw in progs:
        chords = raw.split()
        if not chords:
            raise SystemExit('--progression needs a value.')
        progressions[' '.join(chords)] = chords
    while '--key' in args:
        pop_option(args, '--key')
    while '--progression' in args:
        pop_option(args, '--progression')
    folders = [Path(arg) for arg in args]
    if not progressions or not folders:
        raise SystemExit(__doc__)
    for folder in folders:
        print(f'== {folder.name}')
        for path, shift, file_key in midi_loops(folder, key):
            bars = bars_of(path, shift)
            found = [f'{name}@{start}:{mean * 100:.0f}%' for name, start, mean in fits(bars, progressions, key)]
            print(f'  {path.stem:26s} ({file_key:>3s} {shift:+d}, {len(bars)} bars)  {", ".join(found) or "-"}')


if __name__ == '__main__':
    main()
