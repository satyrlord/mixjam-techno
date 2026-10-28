"""Render a master board over a premaster and measure it against the reference profile.

python -I tools/song/master.py PREMASTER.wav BOARD.json OUT_DIR [--song SONG.mixjam] [--bpm N]
                               [--fit DB] [--no-render] [--quiet]
python -I tools/song/master.py --album [DIR] [--track NAME ...] [--out PROFILE.json]

The premaster is the mix with the board and trim off (render.py writes it). The board
is the `master` object of a .mixjam (the shape of `board()` in mixjam.py, trim
included). The tool writes OUT_DIR/board.mixjam, which holds the premaster as one clip
with the board on it, renders it to OUT_DIR/master.wav, measures the render, prints the
measurements and saves them to OUT_DIR/measure.json.

Sections and tempo come from the `.meta.json` beside the song: `--song SONG.mixjam`
names it; otherwise the first `*.meta.json` in the premaster folder's parent is used, and
without one there is no section table and the tempo is --bpm (default 140). Levels are
mono-sum RMS in dB. "Loud" means within 3 dB of the loudest 8-bar block. The drum line
compares the master with the premaster in the loud blocks: how far the board pushes the
hits under the body, and the attack of the kick (below 120 Hz) and the hats (6-12 kHz); a
hat attack above the premaster's means the board sharpened the hats.

The comparison columns come from the reference profile (REFERENCE_PROFILE, or
reference.json beside ARTIST_PROFILE; see .env.example): its `targets` (ranges that flag
a miss with "!") and its `references` (measured songs or album medians). Without a
profile the tool measures and prints without those columns, and says so.

--fit DB      move the limiter gain until the loud blocks read DB, and save the fitted
              board as OUT_DIR/board.json; use it after the mix changes.
--no-render   measure OUT_DIR/master.wav as it is.
--quiet       print the summary line only.
--album [DIR] measure every WAV in DIR (default: REFERENCE_ALBUM_FOLDER) the same way and
              write the reference profile: the album median, plus one entry per --track NAME
              (a part of a file name). An existing profile keeps its targets and song targets;
              a new one gets the album's 10th-90th percentile ranges as targets.
"""
import json
import math
import os
import subprocess
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import env  # noqa: E402
from mixjam import Song, read_wav  # noqa: E402

BLOCK_BARS = 8
DEFAULT_BPM = 140.0
THIRDS = [31.5, 40, 50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800, 1000, 1250, 1600,
          2000, 2500, 3150, 4000, 5000, 6300, 8000, 10000, 12500, 16000]
KEYS = ['loud_rms', 'loudest', 'peak', 'crest', 'low', 'sub45', 'mid', 'hats', 'box', 'corr', 'spread',
        'top_pres', 'pres_box']
ROWS = [('loudest block RMS dB', 'loudest', '{:.1f}'), ('loud blocks RMS dB', 'loud_rms', '{:.1f}'),
        ('peak dBFS', 'peak', '{:.2f}'), ('crest dB (mean)', 'crest', '{:.1f}'),
        ('<120 Hz share', 'low', '{:.0%}'), ('<45 Hz share', 'sub45', '{:.1%}'),
        ('1-4 kHz share', 'mid', '{:.2%}'), ('6-12 kHz share', 'hats', '{:.2%}'),
        ('300-600 Hz share', 'box', '{:.1%}'), ('top-pres dB (8-16k over 1.6-5k)', 'top_pres', '{:+.1f}'),
        ('pres-box dB (1.6-5k over 250-630)', 'pres_box', '{:+.1f}'), ('L/R correlation', 'corr', '{:.2f}'),
        ('loud-to-quiet spread dB', 'spread', '{:.1f}')]


def db(x):
    return 20 * math.log10(x) if x > 1e-12 else -120.0


def load_profile():
    """The reference profile and its path; (None, path or None) when there is none."""
    path = env.reference_profile()
    if path is None or not path.exists():
        return None, path
    return json.loads(path.read_text(encoding='utf-8')), path


def run_render(song_path, wav_path):
    data = Path(wav_path).resolve().parent / '.data'
    environment = dict(os.environ, MIXJAM_TECHNO_DATA_DIR=str(data))
    result = subprocess.run([str(env.exe()), 'render', '--mix', str(song_path), '--output', str(wav_path)],
                            capture_output=True, text=True, env=environment)
    if result.returncode != 0:
        raise SystemExit(f'render failed for {song_path}: {result.stdout}{result.stderr}')


def render_board(premaster, board, out, bpm):
    """Write OUT/board.mixjam with the premaster as one clip under the board, render OUT/master.wav."""
    out.mkdir(parents=True, exist_ok=True)
    song = Song('board check', bpm=bpm)
    sound = song.sound(str(premaster))
    song.clip(1, str(premaster), 0, int(math.ceil(sound.ticks)))
    song.master = board
    song.master_gain = 1.0
    song_path = out / 'board.mixjam'
    song.save(song_path)
    song_path.with_suffix('.meta.json').unlink(missing_ok=True)
    run_render(song_path, out / 'master.wav')
    return out / 'master.wav'


def spectrum_shares(mono, rate):
    """Energy shares of the bands this report uses, and the 1/3-octave bands relative to the total."""
    spectrum = np.abs(np.fft.rfft(mono * np.hanning(len(mono)))) ** 2
    freqs = np.fft.rfftfreq(len(mono), 1 / rate)
    total = spectrum.sum() + 1e-30
    band = lambda lo, hi: float(spectrum[(freqs >= lo) & (freqs < hi)].sum() / total)
    thirds = [band(c / 2 ** (1 / 6), c * 2 ** (1 / 6)) for c in THIRDS]
    return dict(low=band(20, 120), sub45=band(20, 45), mid=band(1000, 4000), hats=band(6000, 12000),
                box=band(300, 600), thirds=thirds)


def tonal_indices(thirds_db):
    """top-pres: 8-16 kHz over 1.6-5 kHz (hat fizz over presence); pres-box: 1.6-5 kHz over 250-630 Hz, in dB."""
    power = lambda lo, hi: 10 * math.log10(sum(10 ** (v / 10) for c, v in zip(THIRDS, thirds_db) if lo <= c <= hi))
    return power(8000, 16000) - power(1600, 5000), power(1600, 5000) - power(250, 630)


def correlation(part):
    left, right = part[:, 0], part[:, -1]
    if left.std() < 1e-6 or right.std() < 1e-6:
        return 1.0
    return float(np.corrcoef(left, right)[0, 1])


def measure_part(part, rate):
    """Level, peak, crest, shares and correlation of one span."""
    mono = part.mean(axis=1)
    rms = db(float(np.sqrt(np.mean(mono ** 2))))
    peak = db(float(np.abs(part).max()))
    shares = spectrum_shares(mono, rate)
    return dict(rms=rms, peak=peak, crest=peak - rms, corr=correlation(part), **shares)


def measure(audio, rate, bpm=DEFAULT_BPM, sections=None):
    """Blocks, loud blocks and (when given) sections of a whole render."""
    mono = audio.mean(axis=1)
    n = int(round(BLOCK_BARS * 240 / bpm * rate))
    count = max(1, len(mono) // n)
    block_rms = np.array([db(float(np.sqrt(np.mean(mono[i * n:(i + 1) * n] ** 2)))) for i in range(count)])
    loudest = float(block_rms.max())
    loud = [i for i in range(count) if block_rms[i] >= loudest - 3]
    parts = [measure_part(audio[i * n:(i + 1) * n], rate) for i in loud]
    mean = lambda key: float(np.mean([p[key] for p in parts]))
    played = block_rms[block_rms > -40]
    quiet = float(np.percentile(played, 10)) if len(played) else loudest
    result = dict(
        rms=db(float(np.sqrt(np.mean(mono ** 2)))), peak=db(float(np.abs(audio).max())),
        loudest=loudest, loud_blocks=len(loud), loud_rms=mean('rms'), loud_peak=mean('peak'),
        crest=mean('crest'), crest_min=float(min(p['crest'] for p in parts)),
        crest_max=float(max(p['crest'] for p in parts)),
        low=mean('low'), sub45=mean('sub45'), mid=mean('mid'), hats=mean('hats'), box=mean('box'),
        corr=mean('corr'), spread=loudest - quiet, block_rms=[round(float(v), 1) for v in block_rms],
        loud_blocks_index=loud,
        thirds_db=[10 * math.log10(v + 1e-12) for v in np.mean([p['thirds'] for p in parts], axis=0)],
    )
    result['top_pres'], result['pres_box'] = tonal_indices(result['thirds_db'])
    if sections:
        bar = rate * 240 / bpm
        result['sections'] = []
        for section in sections:
            a = int(round((section['first_bar'] - 1) * bar))
            b = min(int(round((section['first_bar'] - 1 + section['bars']) * bar)), len(audio))
            if b - a < rate // 10:
                continue
            part = measure_part(audio[a:b], rate)
            part.pop('thirds')
            result['sections'].append(dict(name=section['name'], first_bar=section['first_bar'],
                                           bars=section['bars'], **part))
    return result


def band(x, rate, lo, hi):
    spectrum = np.fft.rfft(x)
    freqs = np.fft.rfftfreq(len(x), 1 / rate)
    spectrum[(freqs < lo) | (freqs >= hi)] = 0
    return np.fft.irfft(spectrum, n=len(x))


def frame_rms(x, n):
    count = len(x) // n
    return np.sqrt(np.mean(x[:count * n].reshape(count, n) ** 2, axis=1) + 1e-20)


def attack(x, rate, period, head, window):
    """Median over periods of the loudest `window` RMS in the first `head` seconds over the period's RMS, dB."""
    p, w = int(round(period * rate)), int(round(window * rate))
    values = [20 * math.log10(frame_rms(x[s:s + p][:int(head * rate)], w).max() / (np.sqrt(np.mean(x[s:s + p] ** 2)) + 1e-12))
              for s in range(0, len(x) - p, p)]
    return float(np.median(values)) if values else 0.0


def drums(pre_audio, post_audio, rate, loud_blocks, bpm):
    """Drum handling in the master's loud blocks, against the premaster:
    squash: median board gain over 10 ms frames minus its median on the premaster's loudest 2% (the hits);
    kick: loudest 10 ms in the first 60 ms of each beat over the beat's RMS, below 120 Hz;
    hats: loudest 5 ms in the first 30 ms of each 1/16 over its RMS, 6-12 kHz. Both in dB, median."""
    block = int(round(BLOCK_BARS * 240 / bpm * rate))
    pre, post = pre_audio.mean(axis=1), post_audio.mean(axis=1)
    n = int(0.01 * rate)
    gains, levels, result = [], [], dict(kick=[], kick_pre=[], hats=[], hats_pre=[])
    for i in loud_blocks:
        q, m = pre[i * block:(i + 1) * block], post[i * block:(i + 1) * block]
        fq = frame_rms(q, n)
        gains.append(20 * np.log10(frame_rms(m, n) / fq))
        levels.append(fq)
        for key, x in (('', m), ('_pre', q)):
            result['kick' + key].append(attack(band(x, rate, 20, 120), rate, 60 / bpm, 0.06, 0.01))
            result['hats' + key].append(attack(band(x, rate, 6000, 12000), rate, 15 / bpm, 0.03, 0.005))
    gains, levels = np.concatenate(gains), np.concatenate(levels)
    hits = levels >= np.percentile(levels, 98)
    out = {key: float(np.median(values)) for key, values in result.items()}
    out['squash'] = float(np.median(gains) - np.median(gains[hits]))
    out['full_scale'] = int(np.sum(np.abs(post_audio) >= 32767 / 32768))
    return out


def find_meta(premaster, song):
    """The song's `.meta.json`: beside --song, else the first in the premaster folder's parent."""
    if song:
        meta = Path(song).with_suffix('.meta.json')
    else:
        found = sorted(Path(premaster).resolve().parent.parent.glob('*.meta.json'))
        meta = found[0] if found else None
    if meta and meta.exists():
        return json.loads(meta.read_text(encoding='utf-8')), meta
    return None, None


def in_range(value, bounds):
    return bounds[0] <= value <= bounds[1]


def flag(value, bounds, fmt='{:.1f}'):
    """The value, with ' !' when it is outside its target range."""
    return fmt.format(value) + ('' if in_range(value, bounds) else ' !')


def report(pre, post, sections_from, board, profile):
    say = print
    targets = (profile or {}).get('targets', {})
    references = (profile or {}).get('references', {})
    on = [name for name in board['order'] if board[name]['on']]
    say(f"board: trim {board['trim']:+.1f} dB, on: {', '.join(on) or 'nothing'}")
    say(f"whole song: premaster peak {pre['peak']:.2f} dBFS RMS {pre['rms']:.1f} dB; "
        f"master peak {post['peak']:.2f} dBFS RMS {post['rms']:.1f} dB (board {post['rms'] - pre['rms']:+.1f} dB)")
    if profile is None:
        say('no reference profile (REFERENCE_PROFILE or ARTIST_PROFILE in .env.local; see .env.example): '
            'measured without targets or reference columns')
    say('')
    say('loud blocks (within 3 dB of the loudest 8-bar block), master against premaster' +
        (' and targets:' if profile else ':'))
    say('| ' + ' | '.join(['measure', 'premaster', 'master', 'target', *references]) + ' |')
    say('|---|---|---|---|' + '---|' * len(references))
    for label, key, fmt in ROWS:
        bounds = targets.get(key)
        target = f'{fmt.format(bounds[0])} to {fmt.format(bounds[1])}' if bounds else '-'
        value = flag(post[key], bounds, fmt) if bounds else fmt.format(post[key])
        cells = [fmt.format(r[key]) if key in r else '-' for r in references.values()]
        say('| ' + ' | '.join([label, fmt.format(pre[key]), value, target, *cells]) + ' |')
    say(f"crest range in loud blocks {post['crest_min']:.1f} to {post['crest_max']:.1f} dB over {post['loud_blocks']} blocks")
    d = post['drums']
    say(f"drums in the loud blocks: hits pushed down {d['squash']:.1f} dB against the body; kick attack "
        f"{d['kick']:.1f} dB (premaster {d['kick_pre']:.1f}); hat attack {d['hats']:.1f} dB (premaster {d['hats_pre']:.1f}); "
        f"samples at 16-bit full scale: {d['full_scale']}")
    say('')
    say('1/3-octave balance of the loud blocks (dB relative to the block energy), master minus the references, '
        'and what the board changed:')
    say('| Hz | ' + ' | '.join(f'{c:g}' for c in THIRDS) + ' |')
    say('|---|' + '---|' * len(THIRDS))
    line = lambda label, values: say(f'| {label} | ' + ' | '.join(f'{v:+.1f}' for v in values) + ' |')
    for name, reference in references.items():
        if 'thirds_db' in reference:
            line(f'vs {name}', np.subtract(post['thirds_db'], reference['thirds_db']))
    line('board (master - premaster)', np.subtract(post['thirds_db'], pre['thirds_db']))
    say('')
    say('level per 8 bars (master RMS dB): ' + ' '.join(
        f'{i * BLOCK_BARS + 1}:{v:.0f}' for i, v in enumerate(post['block_rms'])))
    if post.get('sections'):
        say('')
        say(f'sections ({sections_from}):')
        say('| section | bars | premaster | master | lift | peak | crest | <120 Hz | 1-4k | 6-12k | corr |')
        say('|---|---|---|---|---|---|---|---|---|---|---|')
        for a, b in zip(pre['sections'], post['sections']):
            say(f"| {b['name']} | {b['first_bar']}-{b['first_bar'] + b['bars'] - 1} | {a['rms']:.1f} | {b['rms']:.1f} | "
                f"{b['rms'] - a['rms']:+.1f} | {b['peak']:.2f} | {b['crest']:.1f} | {b['low']:.0%} | {b['mid']:.2%} | "
                f"{b['hats']:.2%} | {b['corr']:.2f} |")


def misses(post, targets):
    """The target keys whose value is outside the range."""
    return [key for key, bounds in targets.items() if key in post and not in_range(post[key], bounds)]


def summary(post, targets=None):
    missed = misses(post, targets or {})
    return (f"loud {post['loud_rms']:.1f} loudest {post['loudest']:.1f} peak {post['peak']:.2f} crest {post['crest']:.1f} "
            f"low {post['low']:.0%} mid {post['mid']:.2%} hats {post['hats']:.2%} box {post['box']:.1%} "
            f"top-pres {post['top_pres']:+.1f} pres-box {post['pres_box']:+.1f} corr {post['corr']:.2f} "
            f"spread {post['spread']:.1f}" + (f"  misses: {' '.join(missed)}" if missed else ''))


def album_profile(results, tracks, old=None):
    """A reference profile from per-file measurements: the album median and the named tracks.

    `old` keeps its name, targets and song targets; without it the targets are the album's
    10th-90th percentile range of each measure.
    """
    median = {k: float(np.median([r[k] for r in results.values()])) for k in KEYS}
    median['thirds_db'] = [float(v) for v in np.median([r['thirds_db'] for r in results.values()], axis=0)]
    references = {'album median': median}
    for track in tracks:
        name = next((name for name in results if track.lower() in name.lower()), None)
        if name is None:
            raise SystemExit(f'no album file name contains {track!r}')
        references[track] = {k: float(results[name][k]) for k in KEYS} | {'thirds_db': results[name]['thirds_db']}
    if old:
        return dict(old, references=references)
    targets = {k: [round(float(np.percentile([r[k] for r in results.values()], 10)), 3),
                   round(float(np.percentile([r[k] for r in results.values()], 90)), 3)] for k in KEYS}
    return dict(name='reference album', method='master.py loud-block method', targets=targets,
                references=references, song_targets=[])


def album(args):
    """Measure the album folder and write the reference profile."""
    position = args.index('--album')
    given = args[position + 1] if position + 1 < len(args) and not args[position + 1].startswith('--') else None
    folder = Path(given) if given else env.path('REFERENCE_ALBUM_FOLDER')
    tracks = [args[i + 1] for i, a in enumerate(args) if a == '--track']
    out = Path(args[args.index('--out') + 1]) if '--out' in args else env.reference_profile()
    if out is None:
        raise SystemExit('set REFERENCE_PROFILE or ARTIST_PROFILE in .env.local (see .env.example), or pass --out')
    bpm = float(args[args.index('--bpm') + 1]) if '--bpm' in args else DEFAULT_BPM
    results = {}
    for path in sorted(folder.glob('*.wav')):
        rate, audio = read_wav(path)
        results[path.stem] = measure(audio, rate, bpm)
        print(f'{path.stem}: {summary(results[path.stem])}', file=sys.stderr)
    if not results:
        raise SystemExit(f'no WAV files in {folder}')
    old = json.loads(out.read_text(encoding='utf-8')) if out.exists() else None
    profile = album_profile(results, tracks, old)
    profile['block_bars'], profile['bpm'] = BLOCK_BARS, bpm
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(profile, indent=1), encoding='utf-8')
    print(f'reference profile written to {out}')


def fit(premaster, board, out, target, bpm, sections):
    """Move the limiter gain (0-12 dB, 0.1 steps) until the loud blocks read `target` within 0.15 dB.

    Up to five renders; the slope of the last two sets each step. Returns the board and its measurement.
    """
    board = json.loads(json.dumps(board))
    board['limiter']['on'] = True
    tried = []
    for _ in range(5):
        rate, audio = read_wav(render_board(premaster, board, out, bpm))
        post = measure(audio, rate, bpm, sections)
        gain = board['limiter']['gain']
        tried.append((gain, post['loud_rms']))
        miss = target - post['loud_rms']
        if abs(miss) <= 0.15:
            break
        if len(tried) == 5:
            print(f'the limiter fit stopped after five renders at {post["loud_rms"]:.2f} dB; '
                  f'target {target} dB was not reached', file=sys.stderr)
            break
        if len(tried) >= 2 and tried[-1][0] != tried[-2][0]:
            slope = (tried[-1][1] - tried[-2][1]) / (tried[-1][0] - tried[-2][0])
        else:
            slope = 0.7
        new = round(min(12.0, max(0.0, gain + miss / max(slope, 0.2))), 1)
        if new == gain:
            print(f'the limiter gain cannot reach {target} dB (stuck at {gain}); move the trim', file=sys.stderr)
            break
        board['limiter']['gain'] = new
    return board, post


def main():
    args = sys.argv[1:]
    if '--album' in args:
        album(args)
        return
    if len(args) < 3 or args[0].startswith('-'):
        raise SystemExit(__doc__)
    premaster, board_path, out = Path(args[0]), Path(args[1]), Path(args[2])
    song = args[args.index('--song') + 1] if '--song' in args else None
    board = json.loads(board_path.read_text(encoding='utf-8'))
    out.mkdir(parents=True, exist_ok=True)
    meta, meta_path = find_meta(premaster, song)
    sections = meta['sections'] if meta else None
    bpm = float(args[args.index('--bpm') + 1]) if '--bpm' in args else float((meta or {}).get('bpm') or DEFAULT_BPM)
    profile, _ = load_profile()
    targets = (profile or {}).get('targets', {})
    rate, pre_audio = read_wav(premaster)
    pre = measure(pre_audio, rate, bpm, sections)
    if '--fit' in args:
        board, post = fit(premaster, board, out, float(args[args.index('--fit') + 1]), bpm, sections)
        (out / 'board.json').write_text(json.dumps(board, indent=1), encoding='utf-8')
        print(f"fitted limiter gain {board['limiter']['gain']:.1f} dB; board saved to {out / 'board.json'}")
    else:
        master = out / 'master.wav' if '--no-render' in args else render_board(premaster, board, out, bpm)
        if not master.exists():
            raise SystemExit(f'{master} does not exist; run without --no-render first')
        rate, post_audio = read_wav(master)
        post = measure(post_audio, rate, bpm, sections)
    rate, post_audio = read_wav(out / 'master.wav')
    post['drums'] = drums(pre_audio, post_audio[:len(pre_audio)], rate, post['loud_blocks_index'], bpm)
    (out / 'measure.json').write_text(json.dumps(dict(board=board, premaster=pre, master=post), indent=1),
                                      encoding='utf-8')
    if '--quiet' in args:
        print(summary(post, targets))
    else:
        report(pre, post, meta_path.name if meta_path else 'no meta file', board, profile)
        print('')
        print('summary: ' + summary(post, targets))


if __name__ == '__main__':
    main()
