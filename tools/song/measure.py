"""Measure a rendered song for the critics.

python -I tools/song/measure.py SONG.mixjam RENDER_DIR [--key "A minor"] > report.md

Reads the song, its `.meta.json` (lane roles, sections, key), and RENDER_DIR's
master.wav, premaster.wav and stem-*.wav from render.py. Writes a Markdown report:
mechanical checks first, then per-section level, tone, stereo and harmony,
arrangement figures, stem balance, and the targets.

The key comes from --key, else from the meta file; without one the report has no
out-of-key column. The targets come from the reference profile (REFERENCE_PROFILE,
see .env.example); without it the report says so and prints no targets.

The mechanical checks are the gate: a missing file, a sound that plays as silence,
overlapping clips on a lane, a clip longer than its sound (a silent gap), or a
melodic phrase that stops dead. A song with any of them is not ready for critics.
"""
import json
import math
import re
import sys
from collections import Counter
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import env  # noqa: E402
from harmony import NOTES, scale  # noqa: E402
from mixjam import MELODIC, TICKS_PER_BAR, Sound, read_wav, ticks_of  # noqa: E402


def db(x):
    return 20 * math.log10(x) if x > 1e-12 else -120.0


def blocks_of(audio, rate, bpm, first_bar, bars):
    bar = rate * 240 / bpm
    a, b = int(round((first_bar - 1) * bar)), int(round((first_bar - 1 + bars) * bar))
    return audio[a:b]


def tone(part, rate, in_key=None):
    """RMS dB, low/lowmid/mid/high energy shares, correlation, out-of-key chroma share, top notes."""
    if len(part) < rate // 10:
        return None
    mono = part.mean(axis=1)
    rms = db(float(np.sqrt(np.mean(mono ** 2))))
    spectrum = np.abs(np.fft.rfft(mono * np.hanning(len(mono)))) ** 2
    freqs = np.fft.rfftfreq(len(mono), 1 / rate)
    total = spectrum.sum() + 1e-20
    shares = [float(spectrum[(freqs >= lo) & (freqs < hi)].sum() / total)
              for lo, hi in ((20, 120), (120, 500), (500, 3000), (3000, 20000))]
    left, right = part[:, 0], part[:, -1]
    corr = float(np.corrcoef(left, right)[0, 1]) if left.std() > 1e-6 and right.std() > 1e-6 else 1.0
    # Chroma from spectral peaks only: summing every bin lets strong low notes
    # leak into their neighbours' pitch classes.
    magnitude = np.sqrt(spectrum)
    band = (freqs >= 110) & (freqs <= 2000)
    peaks = np.where((magnitude[1:-1] > magnitude[:-2]) & (magnitude[1:-1] > magnitude[2:]))[0] + 1
    floor = magnitude[band].max() * 0.03 if band.any() else 0
    peaks = peaks[band[peaks] & (magnitude[peaks] > floor)]
    chroma = np.zeros(12)
    np.add.at(chroma, np.round(69 + 12 * np.log2(freqs[peaks] / 440)).astype(int) % 12, magnitude[peaks])
    chroma /= chroma.sum() + 1e-20
    out_of_key = float(sum(chroma[i] for i in range(12) if i not in in_key)) if in_key else None
    top = ' '.join(NOTES[i] for i in np.argsort(chroma)[::-1][:4])
    return dict(rms=rms, low=shares[0], lowmid=shares[1], mid=shares[2], high=shares[3], corr=corr,
                out_of_key=out_of_key, top=top, peak=db(float(np.abs(part).max())))


def percent(value):
    return '-' if value is None else f'{value * 100:.0f}%'


def default_meta(document):
    """Meta for a song from elsewhere: every used lane is its own role, sections are 32-bar blocks."""
    end = max((p['startTick'] + p['durationTicks'] for lane in document['lanes']
               for p in lane['placements']), default=0)
    return dict(lanes={str(i + 1): dict(name=f'track {i + 1}', role=f'track{i + 1}')
                       for i, lane in enumerate(document['lanes']) if lane['placements']},
                sections=[dict(name=f'bars {b + 1}-{b + 32}', first_bar=b + 1, bars=32)
                          for b in range(0, int(math.ceil(end / TICKS_PER_BAR)), 32)])


def twin_base(name):
    """A lane and its cross-fade twin ("Name (twin)") count as one lane."""
    return name.replace(' (twin)', '')


def sample_names(document):
    """Names for imported, missing and catalog placements, including unresolved IDs."""
    names = {sample['id']: sample['name'] for sample in
             [*document['samples'], *document.get('missingSamples', [])]}
    for lane in document['lanes']:
        for placement in lane['placements']:
            sample_id = placement['sampleId']
            names.setdefault(sample_id, f'sample #{sample_id}')
    return names


def mechanical(document, lanes):
    """The gate's problem list."""
    samples = {s['id']: s for s in document['samples']}
    missing_ids = {s['id']: s['name'] for s in document.get('missingSamples', [])}
    names = sample_names(document)
    problems = []
    sounds = {}
    # Catalog audio resolves through the application and its provider libraries.
    # This file-only check cannot establish its source duration or end level.
    for sample_id in sorted(set(names) - set(samples) - set(missing_ids)):
        problems.append(f'{names[sample_id]}: source duration and silence checks are unavailable; '
                        'the source is not linked in samples')
    for sample in samples.values():
        if not Path(sample['path']).exists():
            problems.append(f"missing file: {sample['path']}")
        else:
            sounds[sample['id']] = Sound(sample['path'], sample['sourceBpm'], document['song']['bpm'])
    # A sound the song lists as missing plays as silence.
    silent = Counter(p['sampleId'] for lane in document['lanes'] for p in lane['placements']
                     if p['sampleId'] in missing_ids)
    total = sum(len(lane['placements']) for lane in document['lanes'])
    if silent:
        problems.append(f"{len(silent)} missing sounds play as silence in {sum(silent.values())} of {total} "
                        f"placements: " + ', '.join(missing_ids[i] for i, _ in silent.most_common(8)))
    name_of = lambda i: twin_base((lanes.get(str(i + 1)) or {}).get('name', ''))
    for index, lane in enumerate(document['lanes']):
        info = lanes.get(str(index + 1))
        placements = sorted(lane['placements'], key=lambda p: p['startTick'])
        group = name_of(index) if info else None
        starts = {p['startTick'] for i, other in enumerate(document['lanes'])
                  if (group and name_of(i) == group) or i == index for p in other['placements']}
        for a, b in zip(placements, placements[1:]):
            if b['startTick'] < a['startTick'] + a['durationTicks']:
                problems.append(f"track {index + 1} bar {b['startTick'] / TICKS_PER_BAR + 1:.2f}: "
                                f"{names[b['sampleId']]} overlaps {names[a['sampleId']]}")
        for p in placements:
            sound = sounds.get(p['sampleId'])
            if sound is None:
                continue
            bar = p['startTick'] / TICKS_PER_BAR + 1
            name = samples[p['sampleId']]['name']
            plays = sound.ticks - ticks_of(p['offsetFrames'], sound.rate, sound.play_bpm)
            if p['durationTicks'] > plays + 4:
                problems.append(f"track {index + 1} bar {bar:.2f}: {name} clip outlasts its sound by "
                                f"{(p['durationTicks'] - plays) / TICKS_PER_BAR:.2f} bars (silent gap)")
            end = p['startTick'] + p['durationTicks']
            fade_out = p.get('fades', {}).get('out', {}).get('ticks', 0)
            cut = p['durationTicks'] < plays - 4
            continued = any(t in starts for t in range(end - 16, end + 1))
            if info and info['role'] in MELODIC and not continued and not fade_out and (cut or sound.ends_loud):
                problems.append(f"track {index + 1} ({info['name']}) bar {bar:.2f}: {name} stops dead at bar "
                                f"{end / TICKS_PER_BAR + 1:.2f} ({'cut short' if cut else 'ends loud'}, no fade out)")
    return problems


def main():
    args = sys.argv[1:]
    if len(args) < 2 or args[0].startswith('-'):
        raise SystemExit(__doc__)
    song_path, render = Path(args[0]), Path(args[1])
    document = json.loads(song_path.read_text(encoding='utf-8'))
    meta_path = song_path.with_suffix('.meta.json')
    meta = json.loads(meta_path.read_text(encoding='utf-8')) if meta_path.exists() else default_meta(document)
    key = args[args.index('--key') + 1] if '--key' in args else meta.get('key')
    in_key = scale(key) if key else None
    bpm = document['song']['bpm']
    lanes = meta['lanes']
    out = []
    say = out.append

    say(f"# Measurement report: {document['song']['title']}\n")
    say('## Mechanical checks\n')
    problems = mechanical(document, lanes)
    say(f'{len(problems)} problems.\n')
    for problem in problems[:60]:
        say(f'- {problem}')
    if len(problems) > 60:
        say(f'- ... and {len(problems) - 60} more')
    say('')

    # Audio.
    rate, master = read_wav(render / 'master.wav')
    _, premaster = read_wav(render / 'premaster.wav')
    total_bars = len(master) / rate / (240 / bpm)
    whole = tone(master, rate)
    pre_peak = db(float(np.abs(premaster).max()))
    say('## Master\n')
    say(f'- length {len(master) / rate / 60:.2f} min ({total_bars:.1f} bars at {bpm:g} BPM)')
    if whole:
        say(f"- master peak {whole['peak']:.2f} dBFS, RMS {whole['rms']:.1f} dB; premaster peak {pre_peak:.2f} dBFS")
        say(f"- whole song: {whole['low'] * 100:.0f}% below 120 Hz, correlation {whole['corr']:.2f}")
    else:
        say('- Master tone measurements unavailable: the render is shorter than the 0.1-second measurement window.')
        say(f'- premaster peak {pre_peak:.2f} dBFS')
    if pre_peak > -0.3:
        say('- **problem: the premaster reaches full scale, so the mix sum clips. Lower the master gain.**')
    say('')

    sections = meta['sections'] or [dict(name='song', first_bar=1, bars=max(1, math.ceil(total_bars)))]
    rows = []
    for section in sections:
        t = tone(blocks_of(master, rate, bpm, section['first_bar'], section['bars']), rate, in_key)
        if t:
            rows.append((section, t))
    loudest = max((t['rms'] for _, t in rows), default=None)
    say('## Sections (master)\n')
    if rows:
        say('| section | bars | RMS dB | vs loudest | <120 Hz | 120-500 | 0.5-3k | >3k | corr | out of key | strongest notes |')
        say('|---|---|---|---|---|---|---|---|---|---|---|')
    else:
        say('Section measurements unavailable: no section contains at least 0.1 seconds of rendered audio.\n')
    for section, t in rows:
        say(f"| {section['name']} | {section['first_bar']}-{section['first_bar'] + section['bars'] - 1} | "
            f"{t['rms']:.1f} | {t['rms'] - loudest:+.1f} | {t['low'] * 100:.0f}% | {t['lowmid'] * 100:.0f}% | "
            f"{t['mid'] * 100:.0f}% | {t['high'] * 100:.0f}% | {t['corr']:.2f} | {percent(t['out_of_key'])} | {t['top']} |")
    if key:
        say(f'\nOut of key: the share of spectral peaks from 110 Hz to 2 kHz outside {key} (natural minor for a '
            'minor key). Read it for tonal stems (pads, keys, leads, arps) only: drums and noise have no key, and '
            'a bright or distorted bass adds its own harmonics, such as the major third above its root (the 5th '
            'harmonic), which count as out of key without being wrong notes.\n')
    else:
        say('\nNo key given (`--key` or the meta file), so no out-of-key share.\n')

    say('## Level per 8 bars (master RMS dB)\n')
    eight = []
    for first in range(1, math.ceil(total_bars) + 1, 8):
        t = tone(blocks_of(master, rate, bpm, first, 8), rate)
        if t:
            eight.append(f"{first}:{t['rms']:.0f}")
    say(' '.join(eight) + '\n')
    if loudest is not None:
        quiet = sum(1 for _, t in rows if t['rms'] <= loudest - 4)
        say(f'Sections at least 4 dB below the loudest: {quiet} of {len(rows)}.\n')

    arrangement(document, lanes, sections, total_bars, say)
    stem_tables(render, rate, bpm, sections, in_key, say)
    targets(say)
    print('\n'.join(out))


def arrangement(document, lanes, sections, total_bars, say):
    say('## Arrangement\n')
    bars = int(math.ceil(total_bars))
    # A family's numbered chord files ("Name 1" ... "Name 9") count as one sound.
    family = {sample_id: re.sub(r' \d$', '', name) for sample_id, name in sample_names(document).items()}
    names = [twin_base((lanes.get(str(i + 1)) or {}).get('name', str(i))) for i in range(len(document['lanes']))]
    lane_group = [names.index(name) for name in names]
    active = [set() for _ in range(bars + 1)]
    runs = []
    for index, lane in enumerate(document['lanes']):
        current = None
        for p in sorted(lane['placements'], key=lambda p: p['startTick']):
            first = p['startTick'] // TICKS_PER_BAR
            last = (p['startTick'] + p['durationTicks'] - 1) // TICKS_PER_BAR
            for b in range(first, min(last, bars) + 1):
                active[b].add((lane_group[index], family[p['sampleId']]))
            if current and current[0] == family[p['sampleId']] and current[2] == p['startTick']:
                current = (family[p['sampleId']], current[1], p['startTick'] + p['durationTicks'])
            else:
                if current:
                    runs.append((current[2] - current[1]) / TICKS_PER_BAR)
                current = (family[p['sampleId']], p['startTick'], p['startTick'] + p['durationTicks'])
        if current:
            runs.append((current[2] - current[1]) / TICKS_PER_BAR)
    active = active[:bars]
    layers = [len(a) for a in active] or [0]
    change_bars = [b for b in range(1, bars) if active[b] != active[b - 1]]
    used = {p['sampleId'] for lane in document['lanes'] for p in lane['placements']}
    used_lanes = len({lane_group[i] for i, lane in enumerate(document['lanes']) if lane['placements']})
    say(f'- {len(used)} sounds, {used_lanes} lanes used (a twin counts with its lane)')
    say(f'- layers per bar: median {int(np.median(layers))}, 90th percentile {int(np.percentile(layers, 90))}, '
        f'max {max(layers)}')
    say(f'- a change every {bars / max(1, len(change_bars)):.2f} bars ({len(change_bars)} changes)')
    if runs:
        say(f'- sound runs: median {np.median(runs):.1f} bars, 75th percentile {np.percentile(runs, 75):.1f}, '
            f'max {max(runs):.0f}')
    phase = Counter(b % 8 for b in change_bars)
    say(f'- change position in the 8-bar phrase: {dict(sorted(phase.items()))}\n')
    say('| section | median layers | changes | sounds | roles playing |')
    say('|---|---|---|---|---|')
    for section in sections:
        a, b = section['first_bar'] - 1, section['first_bar'] - 1 + section['bars']
        span = active[a:b]
        if not span:
            continue
        roles = Counter(lanes.get(str(i + 1), {}).get('role', '?') for bar in span for i, _ in bar)
        sounds = {s for bar in span for _, s in bar}
        say(f"| {section['name']} | {int(np.median([len(x) for x in span]))} | "
            f"{sum(1 for c in change_bars if a < c < b)} | {len(sounds)} | "
            f"{', '.join(f'{r} {n // len(span)}' for r, n in roles.most_common())} |")
    say('')


def stem_tables(render, rate, bpm, sections, in_key, say):
    stems = sorted(render.glob('stem-*.wav'))
    if not stems:
        return
    say('## Stem levels per section (RMS dB, premaster, with their FX returns)\n')
    say('| section | ' + ' | '.join(p.stem[5:] for p in stems) + ' |')
    say('|---|' + '---|' * len(stems))
    data = {p.stem[5:]: read_wav(p)[1] for p in stems}
    for section in sections:
        cells = []
        for name in data:
            part = blocks_of(data[name], rate, bpm, section['first_bar'], section['bars'])
            cells.append(f"{db(float(np.sqrt(np.mean(part.mean(axis=1) ** 2)))):.0f}" if len(part) else '-')
        say(f"| {section['name']} | " + ' | '.join(cells) + ' |')
    say('')
    say('| stem | <120 Hz | 120-500 | 0.5-3k | >3k | corr | out of key |')
    say('|---|---|---|---|---|---|---|')
    for name, audio in data.items():
        t = tone(audio, rate, in_key)
        if t:
            say(f"| {name} | {t['low'] * 100:.0f}% | {t['lowmid'] * 100:.0f}% | {t['mid'] * 100:.0f}% | "
                f"{t['high'] * 100:.0f}% | {t['corr']:.2f} | {percent(t['out_of_key'])} |")
    say('')


def targets(say):
    path = env.reference_profile()
    profile = json.loads(path.read_text(encoding='utf-8')) if path and path.exists() else None
    if not profile or not profile.get('song_targets'):
        say('## Targets\n')
        say('No reference profile with song targets (REFERENCE_PROFILE or ARTIST_PROFILE in .env.local; '
            'see .env.example), so this report has no targets.')
        return
    say(f"## Targets ({profile.get('name', 'reference profile')})\n")
    for line in profile['song_targets']:
        say(f'- {line}')


if __name__ == '__main__':
    main()
