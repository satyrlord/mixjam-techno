"""Lane groups, roles and sums of a song, shared by render.py, audible.py and clash.py.

A lane is a named group of tracks: a lane and its twin ("NAME (twin)") count as one. The names and
roles come from the `.meta.json` that `Song.save` writes beside the song.
"""
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mixjam import TICKS_PER_BAR, read_wav, twin_base  # noqa: E402


def read_meta(song):
    path = Path(song).with_suffix('.meta.json')
    return json.loads(path.read_text(encoding='utf-8')) if path.exists() else None


def lane_groups(document, meta):
    """Lane name to its 1-based tracks, for every used track: the names in the meta, a twin
    ("NAME (twin)") with its lane, and a track the meta does not name as its own lane."""
    names = {int(k): v['name'] for k, v in meta['lanes'].items()} if meta else {}
    groups = {}
    for track, lane in enumerate(document['lanes'], 1):
        if lane['placements']:
            name = twin_base(names.get(track, f'track {track}'))
            groups.setdefault(name, []).append(track)
    return groups


def lane_roles(groups, meta):
    """Lane name to its role, for lanes that the meta names."""
    roles = {}
    for name, tracks in groups.items():
        info = (meta or {}).get('lanes', {}).get(str(min(tracks)))
        if info is not None:
            roles[name] = info['role']
    return roles


def song_end_tick(document):
    """The native Project.end_tick bound, including an explicit loop end."""
    end = max((p['startTick'] + p['durationTicks'] for lane in document['lanes']
               for p in lane['placements']), default=4 * TICKS_PER_BAR)
    loop = document['song'].get('loopRegion')
    return max(end, loop[1] if loop else 0)


def add(total, data):
    """`total` plus `data`, padded to the longer of the two."""
    if total is None:
        return data.astype(np.float32)
    if len(data) > len(total):
        total = np.pad(total, ((0, len(data) - len(total)), (0, 0)))
    total[:len(data)] += data
    return total


def mix_down(lanes, roles, peaks=None):
    """The premaster and one stem per role, as sums of the lane renders: rate, premaster, stems.

    When `peaks` is a dict, it receives each lane's peak (linear) by lane name."""
    rate, premaster, stems = None, None, {}
    for name, wav in lanes.items():
        rate, data = read_wav(wav)
        if peaks is not None:
            peaks[name] = float(np.abs(data).max()) if data.size else 0.0
        premaster = add(premaster, data)
        if name in roles:
            stems[roles[name]] = add(stems.get(roles[name]), data)
    return rate, premaster, stems
