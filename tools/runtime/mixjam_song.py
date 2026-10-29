"""Read a saved .mixjam song (format versions 8 through 17) for verification scripts.

The result uses the field names of the earlier JSON project format, which the
application still opens. The scripts compare songs in that form and write it
back as input. The file's application version and timestamps are left out, so
two saves of one song compare equal.
"""
import json
import math
from copy import deepcopy
from pathlib import Path


def _reference(reference):
    if reference['type'] == 'native':
        return {'type': 'native', 'product': reference['product'],
                'native_id': reference['nativeId']}
    return reference


# The Booster pedals' keys in a saved board and its order: the key of versions 11
# to 15, then the key of version 16, which the projection uses. Match BOOSTER_KEYS
# in src/mixjam/song.rs.
BOOSTER_KEYS = (('ejayEq', 'boosterEq'), ('ejayCompressor', 'boosterCompressor'),
                ('ejayWide', 'boosterWide'))


def _current_booster_keys(master):
    current = dict(BOOSTER_KEYS)
    board = {current.get(key, key): value for key, value in master.items()}
    board['order'] = [current.get(name, name) for name in master['order']]
    return board


def _master(document):
    if document['formatVersion'] >= 16:
        return document['master']
    if document['formatVersion'] >= 12:
        return _current_booster_keys(document['master'])
    # Match MasterChain::default and the version-11 migration in song.rs.
    # Keep this projection usable as a schema-4 JSON input to the executable.
    pedals = {
        'boosterEq': {'on': False, 'bands': [50] * 10},
        'boosterCompressor': {'on': False, 'drive': 500, 'speed': 500, 'gain': 500},
        'boosterWide': {'on': False, 'spread': 500},
        'softClip': {'on': False, 'amount': 1.5, 'ceiling': -0.5},
        'tube': {'on': False, 'drive': 2.5, 'mix': 100.0},
        'trimEq': {'on': False, 'highPass': 20.0, 'mud': -1.5, 'harsh': -1.0},
        'busCompressor': {'on': False, 'threshold': -16.0, 'ratio': 2.0,
                          'attack': 10.0, 'release': 300.0},
        'maximizer': {'on': False, 'boost': 10.0},
        'liftEq': {'on': False, 'low': 1.0, 'air': 1.0},
        'tape': {'on': False, 'drive': 2.0, 'speed': 30},
        'imaging': {'on': False, 'width': 105.0, 'monoBelow': 120.0},
        'multiband': {'on': False, 'low': 20.0, 'mid': 15.0, 'high': 20.0},
        'limiter': {'on': False, 'gain': 4.0, 'ceiling': -1.0},
    }
    master = {'trim': 0.0, 'order': list(pedals), **pedals}
    if document['formatVersion'] == 11:
        saved = _current_booster_keys(document['master'])
        master['order'][:3] = saved['order']
        for _, name in BOOSTER_KEYS:
            master[name] = saved[name]
    return master


def _fx_buses(document):
    # Match FxBuses::default: every bus empty, on, at full return, limited.
    if document['formatVersion'] >= 13:
        return document['fxBuses']
    return [{'module': {'type': 'empty'}, 'powered': True, 'returnLevel': 1.0,
             'limiter': True} for _ in range(4)]


# Match ChannelEq::default: the EQ off, every band flat.
_FLAT_EQ = {'on': False, 'lowCut': 20.0, 'lowHz': 100.0, 'lowDb': 0.0, 'midHz': 1000.0,
            'midDb': 0.0, 'midQ': 1.0, 'highHz': 8000.0, 'highDb': 0.0, 'highCut': 20000.0}

# Match Ducker::default: disabled, with track 1 as its retained key.
_NO_DUCKER = {'on': False, 'key': 1, 'threshold': -30.0, 'depth': 6.0,
              'attack': 2.0, 'release': 120.0}

# Match Fades::default: no fade at either edge.
_NO_FADES = {'in': {'ticks': 0, 'curve': 'linear'}, 'out': {'ticks': 0, 'curve': 'linear'}}


def read_song(path):
    document = json.loads(Path(path).read_text(encoding='utf-8'))
    version = document['formatVersion']
    if version not in range(8, 18):
        raise RuntimeError(f'{path} is not a format version 8 through 17 .mixjam song.')
    song = document['song']
    clips = [{'id': item['id'], 'sample_id': item['sampleId'], 'track': track,
              'start_tick': item['startTick'], 'length_ticks': item['durationTicks'],
              'offset_frames': item['offsetFrames'], 'gain': item['gain'],
              'fades': deepcopy(item.get('fades', _NO_FADES)),
              'pitch_cents': item.get('pitchCents', 0)}
             for track, lane in enumerate(document['lanes']) for item in lane['placements']]
    imported = []
    for item in document['samples']:
        sample = {'id': item['id'], 'name': item['name'], 'group_name': item['groupName'],
                  'path': item['path'], 'source_bpm': item['sourceBpm']}
        if item['sourceSampleRate'] is not None:
            sample['source_sample_rate'] = item['sourceSampleRate']
        sample['origin'] = item['origin']
        imported.append(sample)
    return {
        'schema_version': 4,
        'title': song['title'],
        'author': song['author'],
        'bpm': song['bpm'],
        'master_gain': song['masterGain'],
        'master': _master(document),
        'fx': _fx_buses(document),
        'pitch_ratio': 1.0,
        'tracks': [{'gain': abs(lane['gain']) if version == 8 else lane['gain'],
                    'polarity_inverted': (math.copysign(1, lane['gain']) < 0
                                          if version == 8 else lane['polarityInverted']),
                    'pan': lane['pan'],
                    'left_enabled': lane['leftEnabled'],
                    'right_enabled': lane['rightEnabled'],
                    'muted': lane.get('muted', False),
                    'solo': lane.get('solo', False),
                    'sends': lane.get('sends', [0.0] * 4),
                    'automation': lane.get('automation', {}),
                    'eq': deepcopy(lane.get('eq', _FLAT_EQ)),
                    'ducker': deepcopy(lane.get('ducker', _NO_DUCKER))} for lane in document['lanes']],
        'clips': sorted(clips, key=lambda clip: clip['id']),
        'loop_region': song['loopRegion'],
        'imported_samples': imported,
        'missing_samples': [
            {'id': item['id'], 'name': item['name'], 'source_bpm': item['sourceBpm'],
             'sample_rate': item['sampleRate'], 'nominal_ticks': item['nominalTicks'],
             'length_inferred': item['lengthInferred'],
             'reference': _reference(item['reference']), 'reason': item['reason'],
             'metadata': item['metadata']} for item in document['missingSamples']],
        'sample_conflicts': [
            {'reference': item['reference'], 'sample_ids': item['sampleIds'],
             'candidates': item['candidates'], 'selected': item['selected'],
             'confirmed': item['confirmed'], 'source_spans': item['sourceSpans']}
            for item in document['sampleConflicts']],
        'import_notices': [{'source_bpm': item['sourceBpm'], 'target_bpm': item['targetBpm']}
                           for item in document['importNotices']],
        'source_notices': document['sourceNotices'],
        **({'source_settings': document['sourceSettings']}
           if document.get('sourceSettings') else {}),
        'libraries': document['libraries'],
    }
