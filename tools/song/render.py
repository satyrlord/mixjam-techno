"""Render a song: the master, the premaster (board off), and one stem per role.

python -I tools/song/render.py SONG.mixjam OUT_DIR [--no-stems] [--fit-gain DBFS]

Writes OUT_DIR/master.wav, OUT_DIR/premaster.wav and OUT_DIR/stem-ROLE.wav. The roles
come from the `.meta.json` beside the song (mixjam.py writes it); without it, no stems.

--fit-gain first renders a probe at a low master gain, then sets the song's master
gain so that the premaster peaks at DBFS (for example -0.5), and saves the song
before the real renders.

Stems keep the song's FX buses, so a stem includes its own reverb and delay
returns. They use the premaster: the board is off.

The renderer is the release build in target/release; it runs headless and plays no
sound. Each render keeps its scan cache in OUT_DIR/.data.
"""
import json
import math
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import env  # noqa: E402
from mixjam import board_off as _board_off, read_wav  # noqa: E402


def run(song_path, wav_path):
    """Render one song file to a WAV with the release build."""
    data = Path(wav_path).resolve().parent / '.data'
    environment = dict(os.environ, MIXJAM_TECHNO_DATA_DIR=str(data))
    result = subprocess.run([str(env.exe()), 'render', '--mix', str(song_path), '--output', str(wav_path)],
                            capture_output=True, text=True, env=environment)
    if result.returncode != 0:
        raise SystemExit(f'render failed for {song_path}: {result.stdout}{result.stderr}')


def board_off(document):
    """The premaster: the mix before the board, so the trim is off too."""
    document['master'] = _board_off(document['master'])
    return document


def copy(document):
    return json.loads(json.dumps(document))


def fit_gain(song, out, target_db):
    """Set the master gain so the premaster peaks at target_db; save the song; return the new gain."""
    if not math.isfinite(target_db) or target_db > 0:
        raise SystemExit('the gain target must be finite and at or below 0 dBFS')
    document = json.loads(song.read_text(encoding='utf-8'))
    probe_gain = 0.2
    probe = board_off(copy(document))
    probe['song']['masterGain'] = probe_gain
    path = out / 'probe.mixjam'
    path.write_text(json.dumps(probe), encoding='utf-8')
    run(path, out / 'probe.wav')
    _, audio = read_wav(out / 'probe.wav')
    peak = float(abs(audio).max())
    path.unlink()
    (out / 'probe.wav').unlink()
    if peak >= 0.999:
        raise SystemExit('the probe still clips; lower the lane gains')
    if peak <= 0:
        raise SystemExit('the probe is silent; check the song for missing sounds')
    fitted_gain = round(probe_gain * 10 ** (target_db / 20) / peak, 6)
    # Match the song reader's mixer gain limit. Do not replace a valid song
    # with a value that the application will reject on the next render.
    if not math.isfinite(fitted_gain) or not 0 < fitted_gain <= 4.0:
        raise SystemExit('the required master gain is outside 0 to 4; adjust the lane gains first')
    document['song']['masterGain'] = fitted_gain
    song.write_text(json.dumps(document, indent=1), encoding='utf-8')
    return document['song']['masterGain']


def stems(document, meta):
    """One premaster document per role: the other roles' lanes emptied."""
    roles = sorted({lane['role'] for lane in meta['lanes'].values()})
    for role in roles:
        stem = board_off(copy(document))
        for index, lane in enumerate(stem['lanes']):
            info = meta['lanes'].get(str(index + 1))
            if info is None or info['role'] != role:
                lane['placements'] = []
                lane.pop('automation', None)
        yield role, stem


def main():
    args = sys.argv[1:]
    if len(args) < 2 or args[0].startswith('-'):
        raise SystemExit(__doc__)
    song, out = Path(args[0]), Path(args[1])
    out.mkdir(parents=True, exist_ok=True)
    if '--fit-gain' in args:
        target = float(args[args.index('--fit-gain') + 1])
        print(f'master gain {fit_gain(song, out, target)}')
    document = json.loads(song.read_text(encoding='utf-8'))
    run(song, out / 'master.wav')
    premaster = out / 'premaster.mixjam'
    premaster.write_text(json.dumps(board_off(copy(document))), encoding='utf-8')
    run(premaster, out / 'premaster.wav')
    premaster.unlink()
    meta_path = song.with_suffix('.meta.json')
    if '--no-stems' in args or not meta_path.exists():
        return
    meta = json.loads(meta_path.read_text(encoding='utf-8'))
    for role, stem in stems(document, meta):
        path = out / f'stem-{role}.mixjam'
        path.write_text(json.dumps(stem), encoding='utf-8')
        run(path, out / f'stem-{role}.wav')
        path.unlink()


if __name__ == '__main__':
    main()
