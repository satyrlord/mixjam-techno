"""Pitch classes, keys, scales and chords for the song tools.

Pitch classes count from C = 0. A key is written 'A minor', 'A natural minor', 'Am',
'C major' or 'C'; a chord 'Am', 'F', 'E', 'Bdim', 'G7', 'Dsus4'.
"""
import re

NOTES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B']
ROOTS = {'C': 0, 'D': 2, 'E': 4, 'F': 5, 'G': 7, 'A': 9, 'B': 11}
MODES = {'major': (0, 2, 4, 5, 7, 9, 11), 'minor': (0, 2, 3, 5, 7, 8, 10)}
QUALITIES = {'': (0, 4, 7), 'm': (0, 3, 7), 'dim': (0, 3, 6), 'aug': (0, 4, 8), 'sus2': (0, 2, 7),
             'sus4': (0, 5, 7), '7': (0, 4, 7, 10), 'm7': (0, 3, 7, 10), 'maj7': (0, 4, 7, 11)}


def pitch_class(name):
    """The pitch class of a note name such as 'A', 'C#', 'Bb' or 'Db'."""
    match = re.fullmatch(r'([A-Ga-g])([#b]?)', name.strip())
    if not match:
        raise ValueError(f'not a note name: {name!r}')
    shift = {'#': 1, 'b': -1, '': 0}[match.group(2)]
    return (ROOTS[match.group(1).upper()] + shift) % 12


def parse_key(text):
    """(tonic pitch class, 'major' or 'minor') of a key name."""
    match = re.fullmatch(r'\s*([A-Ga-g][#b]?)\s*[- ]?\s*(m|min|minor|natural minor|maj|major)?\s*', text)
    if not match:
        raise ValueError(f'not a key: {text!r} (write it as "A minor" or "C major")')
    mode = 'minor' if (match.group(2) or '') in ('m', 'min', 'minor', 'natural minor') else 'major'
    return pitch_class(match.group(1)), mode


def scale(key):
    """The pitch classes of a key's scale (natural minor for a minor key)."""
    tonic, mode = parse_key(key) if isinstance(key, str) else key
    return {(tonic + step) % 12 for step in MODES[mode]}


def parse_chord(text):
    """(root pitch class, chord tone pitch classes) of a chord name."""
    match = re.fullmatch(r'\s*([A-G][#b]?)(maj7|m7|dim|aug|sus2|sus4|m|7)?\s*', text)
    if not match:
        raise ValueError(f'not a chord: {text!r}')
    root = pitch_class(match.group(1))
    return root, {(root + step) % 12 for step in QUALITIES[match.group(2) or '']}


def clash_cost(held, chord, key):
    """The share of held note time that clashes with `chord` in `key`.

    `held` maps pitch class to held time. A pitch class clashes when it is outside the
    key's scale and not a chord tone (a major V in a minor key brings its own third), or
    when it lies a semitone above a chord tone without being one (F over Am, C over G).
    """
    _, tones = parse_chord(chord) if isinstance(chord, str) else chord
    allowed = scale(key) | tones
    total = sum(held.values()) or 1
    bad = sum(t for pc, t in held.items()
              if pc not in allowed or ((pc - 1) % 12 in tones and pc not in tones))
    return bad / total


def best_triad(held):
    """The major or minor triad that best covers held pitch classes (root weighted 1.2, fifth 0.8)."""
    best, score = None, -1
    for root in range(12):
        for quality, third in (('m', 3), ('', 4)):
            s = held.get(root, 0) * 1.2 + held.get((root + third) % 12, 0) + held.get((root + 7) % 12, 0) * 0.8
            if s > score:
                best, score = NOTES[root] + quality, s
    return best


def shift_to(source_key, target_key):
    """Semitones (-6..+5) that move `source_key`'s tonic onto `target_key`'s."""
    a, _ = parse_key(source_key) if isinstance(source_key, str) else source_key
    b, _ = parse_key(target_key) if isinstance(target_key, str) else target_key
    return (b - a + 6) % 12 - 6
